use crate::{
    document::Document,
    export_static::RenderWarning,
    model::{element_list, Deck, Element, PlaceholderKind},
    native::{self, NativeDeck, NativePart, P},
    package::Package,
    Error, Result,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{cell::RefCell, collections::{BTreeMap, BTreeSet, VecDeque}, sync::Arc};

const MAX_ORIGINS: usize = 8;
const MAX_METADATA_BYTES: usize = 2 * crate::limits::MIB;

#[derive(Default, Serialize)]
struct ProjectionIndex {
    slides: BTreeMap<String, ProjectionPart>,
    layouts: BTreeMap<String, ProjectionPart>,
    masters: BTreeMap<String, ProjectionPart>,
}

#[derive(Default, Serialize)]
struct ProjectionPart {
    path: String,
    missing: Vec<String>,
    hide_master_graphics: bool,
    unsupported_color_mapping: bool,
}

struct CacheEntry {
    hash: String,
    index: Arc<ProjectionIndex>,
    metadata_bytes: usize,
}

thread_local! {
    static CACHE: RefCell<VecDeque<CacheEntry>> = const { RefCell::new(VecDeque::new()) };
}

fn cached(hash: &str) -> Option<Arc<ProjectionIndex>> {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let position = cache.iter().position(|entry| entry.hash == hash)?;
        let entry = cache.remove(position)?;
        let index = Arc::clone(&entry.index);
        cache.push_back(entry);
        Some(index)
    })
}

fn store(hash: String, index: ProjectionIndex) -> Result<Arc<ProjectionIndex>> {
    let metadata_bytes = serde_json::to_vec(&index)?.len();
    let index = Arc::new(index);
    if metadata_bytes <= MAX_METADATA_BYTES {
        CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            while cache.len() >= MAX_ORIGINS
                || cache.iter().map(|entry| entry.metadata_bytes).sum::<usize>() + metadata_bytes > MAX_METADATA_BYTES
            {
                cache.pop_front();
            }
            cache.push_back(CacheEntry { hash, index: Arc::clone(&index), metadata_bytes });
        });
    }
    Ok(index)
}

fn part(package: &Package, binding: &NativePart, elements: &[Element], design: bool, unsupported_color_mapping: bool) -> Result<ProjectionPart> {
    let parsed = crate::pptx::parse(package.text(&binding.path)?)?;
    let root = parsed.root_element();
    let tree = native::child(root, P, "cSld").and_then(|node| native::child(node, P, "spTree"))
        .ok_or_else(|| Error::Invalid("native projection shape tree missing".into()))?;
    let projected: BTreeSet<_> = element_list(elements).into_iter().map(|element| element.bounds().0).collect();
    let identities: BTreeMap<_, _> = binding.nodes.iter().map(|(id, numeric)| (numeric.as_str(), id.as_str())).collect();
    let mut missing = BTreeSet::new();
    for container in tree.children().filter(|node| node.is_element()) {
        let container = crate::interactive::office::static_fallback(container).unwrap_or(container);
        for node in container.descendants().filter(|node| native::is_shape(*node)) {
            let Some(id) = native::properties(node).and_then(|properties| properties.attribute("id"))
                .and_then(|numeric| identities.get(numeric)).copied() else { continue; };
            if projected.contains(id) { continue; }
            if node.ancestors().filter(|ancestor| native::is_shape(*ancestor))
                .any(|ancestor| native::properties(ancestor).is_some_and(|properties| matches!(properties.attribute("hidden"), Some("1" | "true"))))
            { continue; }
            if design && native::placeholder(node).is_some_and(|placeholder| matches!(
                placeholder.kind, PlaceholderKind::Date | PlaceholderKind::Footer | PlaceholderKind::SlideNumber
            )) { continue; }
            missing.insert(id.to_owned());
        }
    }
    Ok(ProjectionPart {
        path: binding.path.clone(), missing: missing.into_iter().collect(),
        hide_master_graphics: matches!(root.attribute("showMasterSp"), Some("0" | "false")),
        unsupported_color_mapping: native::unsupported_color_mapping(root) || unsupported_color_mapping,
    })
}

fn build(package: &Package, source: &NativeDeck) -> Result<ProjectionIndex> {
    let mut index = ProjectionIndex::default();
    let master_docs = source.masters.iter().map(|b| Ok((b.id.as_str(), (crate::pptx::parse(package.text(&b.path)?)?, native::part_color_map(package, &b.path))))).collect::<Result<BTreeMap<_, _>>>()?;
    let layout_docs = source.layouts.iter().map(|b| Ok((b.id.as_str(), (crate::pptx::parse(package.text(&b.path)?)?, native::part_color_map(package, &b.path))))).collect::<Result<BTreeMap<_, _>>>()?;
    for binding in &source.slides {
        let slide = source.deck.slides.iter().find(|slide| slide.id == binding.id)
            .ok_or_else(|| Error::Invalid("native projection slide missing".into()))?;
        let slide_map = native::part_color_map(package, &binding.path);
        let mut unsupported_color = false;
        if let Some(design) = &source.deck.design {
            if let Some(layout) = slide.layout_id.as_ref().and_then(|id| design.layouts.iter().find(|l| &l.id == id)).or_else(|| design.layouts.first()) {
                if let (Some(lb), Some((lx, lm))) = (source.layouts.iter().find(|e| e.id == layout.id), layout_docs.get(layout.id.as_str())) {
                    unsupported_color |= native::has_remapped_nonplaceholder_scheme_color(package, &lb.path, lx.root_element(), *lm, slide_map);
                    if !slide.hide_master_graphics && !matches!(lx.root_element().attribute("showMasterSp"), Some("0" | "false")) {
                        if let (Some(mb), Some((mx, mm))) = (source.masters.iter().find(|e| e.id == layout.master_id), master_docs.get(layout.master_id.as_str())) {
                            unsupported_color |= native::has_remapped_nonplaceholder_scheme_color(package, &mb.path, mx.root_element(), *mm, slide_map);
                        }
                    }
                }
            }
        }
        index.slides.insert(binding.id.clone(), part(package, binding, &slide.elements, false, unsupported_color)?);
    }
    if let Some(design) = &source.deck.design {
        for binding in &source.layouts {
            let layout = design.layouts.iter().find(|layout| layout.id == binding.id)
                .ok_or_else(|| Error::Invalid("native projection layout missing".into()))?;
            let (lx, lm) = layout_docs.get(binding.id.as_str()).ok_or_else(|| Error::Invalid("native projection layout XML missing".into()))?;
            let unsupported_color = !matches!(lx.root_element().attribute("showMasterSp"), Some("0" | "false"))
                && source.masters.iter().find(|e| e.id == layout.master_id).zip(master_docs.get(layout.master_id.as_str())).is_some_and(|(mb, (mx, mm))| native::has_remapped_nonplaceholder_scheme_color(package, &mb.path, mx.root_element(), *mm, *lm));
            index.layouts.insert(binding.id.clone(), part(package, binding, &layout.elements, true, unsupported_color)?);
        }
        for binding in &source.masters {
            let master = design.masters.iter().find(|master| master.id == binding.id)
                .ok_or_else(|| Error::Invalid("native projection master missing".into()))?;
            index.masters.insert(binding.id.clone(), part(package, binding, &master.elements, true, false)?);
        }
    }
    Ok(index)
}

// Native verification already reads the origin, including in one-request CLI processes.
pub(crate) fn remember(bytes: &[u8], package: &Package, source: &NativeDeck) -> Result<()> {
    let hash = format!("{:x}", Sha256::digest(bytes));
    if cached(&hash).is_none() { store(hash, build(package, source)?)?; }
    Ok(())
}

fn append_warning(part: &ProjectionPart, page_index: usize, warnings: &mut Vec<RenderWarning>) {
    let missing = &part.missing;
    if missing.is_empty() { return; }
    let message = if missing.len() == 1 {
        format!("Native object {} in {} is retained in the PPTX but has no editing projection and is not drawn in this preview; inspect it in Office", missing[0], part.path)
    } else {
        let examples = missing.iter().take(4).map(|id| id.as_str()).collect::<Vec<_>>().join(", ");
        let rest = missing.len().saturating_sub(4);
        let more = if rest > 0 { format!("; {rest} more") } else { String::new() };
        format!("{} native objects in {} are retained in the PPTX but have no editing projection and are not drawn in this preview. Example IDs (lexicographic): {examples}{more}. Inspect the source part in Office; shared design parts are reported once per selected page batch.", missing.len(), part.path)
    };
    warnings.push(RenderWarning {
        code: "NATIVE_OBJECT_NOT_PREVIEWED".into(), page_index,
        element_id: if missing.len() == 1 { missing[0].to_string() } else { String::new() },
        scope: None,
        message,
    });
}

fn append_color_warning(part: &ProjectionPart, page_index: usize, seen: &mut BTreeSet<String>, warnings: &mut Vec<RenderWarning>) {
    if part.unsupported_color_mapping && seen.insert(part.path.clone()) {
        warnings.push(RenderWarning { code: "NATIVE_COLOR_MAPPING_UNSUPPORTED".into(), page_index, element_id: String::new(), scope: None, message: native::color_mapping_warning(&part.path) });
    }
}

pub(crate) fn warnings(document: &Document, pages: &[usize]) -> Result<Vec<RenderWarning>> {
    let Some(origin) = document.origin.as_ref().filter(|origin| origin.native) else { return Ok(Vec::new()); };
    let index = if let Some(index) = cached(&origin.sha256) { index } else {
        let bytes = STANDARD.decode(&origin.base64).map_err(|_| Error::Invalid("native origin encoding".into()))?;
        if format!("{:x}", Sha256::digest(&bytes)) != origin.sha256 { return Err(Error::Conflict("import origin hash mismatch".into())); }
        let package = Package::open(bytes)?;
        let source = native::read(&package)?;
        store(origin.sha256.clone(), build(&package, &source)?)?
    };
    selected_warnings(&document.deck, pages, &index)
}

fn selected_warnings(deck: &Deck, pages: &[usize], index: &ProjectionIndex) -> Result<Vec<RenderWarning>> {
    let mut warnings = Vec::new();
    let mut seen = BTreeSet::new();
    let mut seen_colors = BTreeSet::new();
    for &page in pages {
        let slide = deck.slides.get(page).ok_or_else(|| Error::Invalid("projection warning page index out of range".into()))?;
        let source_id = slide.native_source_id.as_deref().unwrap_or(&slide.id);
        if let Some(part) = index.slides.get(source_id) { append_warning(part, page, &mut warnings); append_color_warning(part, page, &mut seen_colors, &mut warnings); }
        let layout = deck.design.as_ref().and_then(|design| {
            slide.layout_id.as_ref().and_then(|id| design.layouts.iter().find(|layout| &layout.id == id)).or_else(|| design.layouts.first())
        });
        if let Some(layout) = layout {
            let source_layout = index.layouts.get(&layout.id);
            let source_master = index.masters.get(&layout.master_id);
            if let Some(part) = source_layout {
                append_color_warning(part, page, &mut seen_colors, &mut warnings);
                if seen.insert(part.path.as_str()) { append_warning(part, page, &mut warnings); }
            }
            if let Some(part) = source_master { append_color_warning(part, page, &mut seen_colors, &mut warnings); }
            if !slide.hide_master_graphics && source_layout.is_none_or(|part| !part.hide_master_graphics) {
                if let Some(part) = source_master {
                    if seen.insert(part.path.as_str()) { append_warning(part, page, &mut warnings); }
                }
            }
        }
    }
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_origin_inventory_is_reused_without_entering_document_content() {
        CACHE.with(|cache| cache.borrow_mut().clear());
        let generated = crate::editing::create("projection-cache".into(), "Synthetic cache".into()).unwrap();
        let bytes = crate::pptx::export_pptx(&generated.deck).unwrap();
        let opened = crate::document::open_presentation("projection-cache".into(), bytes).unwrap();
        let document: Document = serde_json::from_value(opened["document"].clone()).unwrap();
        let hash = &document.origin.as_ref().unwrap().sha256;
        let first = cached(hash).expect("native verification primes the inventory");
        assert!(warnings(&document, &[0]).unwrap().is_empty());
        crate::document::verify(&document).unwrap();
        assert!(Arc::ptr_eq(&first, &cached(hash).unwrap()));
        assert!(!serde_json::to_string(&document).unwrap().contains("projection_index"));
        let mut forged = document;
        forged.origin.as_mut().unwrap().base64 = STANDARD.encode(b"not the verified package");
        assert!(crate::document::verify(&forged).is_err());
    }

    #[test]
    fn inventory_cache_evicts_by_origin_count_and_metadata_bytes() {
        CACHE.with(|cache| cache.borrow_mut().clear());
        for index in 0..MAX_ORIGINS {
            store(format!("origin-{index}"), ProjectionIndex::default()).unwrap();
        }
        assert!(cached("origin-0").is_some());
        store("next-origin".into(), ProjectionIndex::default()).unwrap();
        assert!(cached("origin-0").is_some());
        assert!(cached("origin-1").is_none());
        for index in 0..MAX_ORIGINS {
            let mut inventory = ProjectionIndex::default();
            inventory.masters.insert("master".into(), ProjectionPart {
                path: "ppt/slideMasters/slideMaster1.xml".into(),
                missing: vec!["x".repeat(MAX_METADATA_BYTES / 3)], ..Default::default()
            });
            store(format!("sized-{index}"), inventory).unwrap();
        }
        CACHE.with(|cache| {
            let cache = cache.borrow();
            assert!(cache.len() < MAX_ORIGINS);
            assert!(cache.iter().map(|entry| entry.metadata_bytes).sum::<usize>() <= MAX_METADATA_BYTES);
        });
        let mut oversized = ProjectionIndex::default();
        oversized.masters.insert("master".into(), ProjectionPart {
            path: "ppt/slideMasters/slideMaster1.xml".into(),
            missing: vec!["x".repeat(MAX_METADATA_BYTES)], ..Default::default()
        });
        let retained = store("oversized".into(), oversized).unwrap();
        assert_eq!(retained.masters["master"].missing[0].len(), MAX_METADATA_BYTES);
        assert!(cached("oversized").is_none());
    }
}
