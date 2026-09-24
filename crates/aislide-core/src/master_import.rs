use crate::{design::{Master, SlideLayout}, document::{self, Document, Transaction, TransactionResult}, model::{Deck, Element, Slide}, native::{NativeDeck, NativePart}, package::Package, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind { Pptx, Potx }

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode { Masters, Slides }

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MasterImportInput {
    pub kind: SourceKind,
    pub base64: String,
    pub source_sha256: String,
    pub mode: ImportMode,
    pub ids: Vec<String>,
    pub prefix: String,
    pub name: String,
}

struct Source { package: Package, generated: Package, native: NativeDeck, hash: String, scale: (f64, f64) }

fn load(kind: SourceKind, encoded: &str, profile: crate::limits::CapacityProfile) -> Result<Source> {
    let bytes = crate::preflight::decode_archive(encoded, profile.limits().archive_bytes)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let package = crate::templates::master_source(bytes, matches!(kind, SourceKind::Potx))?;
    crate::review::ensure_unprotected(&package).map_err(|error| match error {
        Error::Unsupported(_) => Error::Unsupported("master import does not copy signed, labelled or protected presentations".into()),
        other => other,
    })?;
    let native = crate::native::read(&package)?;
    crate::preflight::deck(&native.deck, profile.limits())?;
    let main = crate::pptx::relationship_targets(&package, "", "officeDocument")?.into_values().next().ok_or_else(|| Error::Invalid("source presentation missing".into()))?;
    let dimensions = crate::canvas::physical_size(&package, &main)?;
    let scale = (dimensions.0 as f64 / f64::from(native.deck.width), dimensions.1 as f64 / f64::from(native.deck.height));
    let mut shell = native.deck.clone();
    shell.embedded_fonts.clear(); shell.auxiliary_design = None;
    for slide in &mut shell.slides { slide.elements.clear(); slide.notes.clear(); slide.notes_paragraphs.clear(); slide.review = None; }
    if let Some(design) = &mut shell.design {
        for master in &mut design.masters { master.elements.clear(); }
        for layout in &mut design.layouts { layout.elements.clear(); }
    }
    let generated = Package::open(crate::pptx::export_pptx(&shell)?)?;
    Ok(Source { package, generated, native, hash, scale })
}

fn xml_value(node: roxmltree::Node<'_, '_>, shell: bool) -> Option<Value> {
    use crate::native::{A, P, R};
    if shell && crate::native::is_shape(node) { return None; }
    if shell && node.has_tag_name((P, "clrMapOvr")) && node.attributes().len() == 0 {
        let children: Vec<_> = node.children().filter(|child| child.is_element()).collect();
        if children.len() == 1 && children[0].has_tag_name((A, "masterClrMapping")) && children[0].attributes().len() == 0 && !children[0].children().any(|child| child.is_element()) { return None; }
    }
    let attributes: BTreeMap<_, _> = node.attributes().filter(|attribute| {
        !(shell && (node.has_tag_name((P, "sldLayoutId")) && attribute.name() == "id" && (attribute.namespace().is_none() || attribute.namespace() == Some(R))
            || node.has_tag_name((P, "sldLayout")) && attribute.namespace().is_none() && ["type", "preserve"].contains(&attribute.name())))
    }).map(|attribute| (format!("{}:{}", attribute.namespace().unwrap_or(""), attribute.name()), attribute.value())).collect();
    let children: Vec<_> = node.children().filter_map(|child| {
        if child.is_element() { xml_value(child, shell) }
        else if child.is_text() && !child.text().unwrap_or("").trim().is_empty() { Some(json!(child.text())) }
        else { None }
    }).collect();
    Some(json!({"namespace":node.tag_name().namespace(),"name":node.tag_name().name(),"attributes":attributes,"children":children}))
}

fn expected_part(source: &Source, part: &NativePart) -> Result<String> {
    for (bindings, directory, name) in [(&source.native.masters, "slideMasters", "slideMaster"), (&source.native.layouts, "slideLayouts", "slideLayout"), (&source.native.slides, "slides", "slide")] {
        if let Some(index) = bindings.iter().position(|binding| binding.path == part.path) { return Ok(format!("ppt/{directory}/{name}{}.xml", index + 1)); }
    }
    Err(Error::Invalid("source part binding missing".into()))
}

fn check_part(source: &Source, part: &NativePart, elements: &[Element], theme: &crate::design::Theme) -> Result<()> {
    let xml = crate::pptx::parse(source.package.text(&part.path)?)?;
    let expected = crate::pptx::parse(source.generated.text(&expected_part(source, part)?)?)?;
    if xml_value(xml.root_element(), true) != xml_value(expected.root_element(), true) {
        return Err(Error::Unsupported("source background, master text styles or XML extensions cannot be preserved by editable master import".into()));
    }
    let tree = crate::native::child(xml.root_element(), crate::native::P, "cSld")
        .and_then(|node| crate::native::child(node, crate::native::P, "spTree"))
        .ok_or_else(|| Error::Unsupported("source shape tree is missing".into()))?;
    if tree.children().filter(|node| node.is_element() && !node.has_tag_name((crate::native::P, "nvGrpSpPr")) && !node.has_tag_name((crate::native::P, "grpSpPr"))).count() != elements.len() {
        return Err(Error::Unsupported("source contains objects not represented by an editable master".into()));
    }
    let relationships = crate::native::relations_path(&part.path);
    if source.package.part_names().contains(relationships.as_str()) {
        let relationships = crate::pptx::parse(source.package.text(&relationships)?)?;
        for relationship in relationships.root_element().children().filter(|node| node.is_element()) {
            let kind = relationship.attribute("Type").unwrap_or("");
            if !["theme", "slideLayout", "slideMaster", "image", "chart", "chartEx", "hyperlink", "notesSlide"].iter().any(|suffix| kind == format!("{}/{suffix}", crate::native::R) || *suffix == "chartEx" && kind == "http://schemas.microsoft.com/office/2014/relationships/chartEx") {
                return Err(Error::Unsupported("source contains a relationship not supported by master import".into()));
            }
        }
    }
    for element in elements { crate::native_save::check_copy_element(&source.package, part, element, theme, source.scale)?; }
    Ok(())
}

fn check_master(source: &Source, id: &str) -> Result<()> {
    let design = source.native.deck.design.as_ref().ok_or_else(|| Error::Unsupported("source has no slide masters".into()))?;
    let master = design.masters.iter().find(|master| master.id == id).ok_or_else(|| Error::Invalid("source master not found".into()))?;
    let theme = crate::design::master_theme(design, id);
    let part = source.native.masters.iter().find(|part| part.id == id).ok_or_else(|| Error::Invalid("source master binding missing".into()))?;
    let targets = crate::pptx::relationship_targets(&source.package, &part.path, "theme")?;
    if targets.len() != 1 { return Err(Error::Unsupported("source master requires one unambiguous theme".into())); }
    let theme_path = targets.values().next().ok_or_else(|| Error::Invalid("source theme missing".into()))?;
    let actual_theme = crate::pptx::parse(source.package.text(theme_path)?)?;
    let expected_theme = Package::open(crate::templates::export_thmx(theme)?)?;
    let expected_theme = crate::pptx::parse(expected_theme.text("theme/theme/theme1.xml")?)?;
    if xml_value(actual_theme.root_element(), false) != xml_value(expected_theme.root_element(), false) {
        return Err(Error::Unsupported("source theme effects or extensions are not represented by editable master import".into()));
    }
    check_part(source, part, &master.elements, theme)?;
    for layout in design.layouts.iter().filter(|layout| layout.master_id == id) {
        let part = source.native.layouts.iter().find(|part| part.id == layout.id).ok_or_else(|| Error::Invalid("source layout binding missing".into()))?;
        check_part(source, part, &layout.elements, theme)?;
    }
    Ok(())
}

fn check_slide(source: &Source, id: &str) -> Result<()> {
    let deck = &source.native.deck;
    let slide = deck.slides.iter().find(|slide| slide.id == id).ok_or_else(|| Error::Invalid("source slide not found".into()))?;
    let design = deck.design.as_ref().ok_or_else(|| Error::Unsupported("source has no slide masters".into()))?;
    let layout = design.layouts.iter().find(|layout| Some(&layout.id) == slide.layout_id.as_ref()).ok_or_else(|| Error::Unsupported("source slide layout is missing".into()))?;
    check_master(source, &layout.master_id)?;
    let part = source.native.slides.iter().find(|part| part.id == id).ok_or_else(|| Error::Invalid("source slide binding missing".into()))?;
    check_part(source, part, &slide.elements, crate::design::master_theme(design, &layout.master_id))?;
    let master = design.masters.iter().find(|master| master.id == layout.master_id).ok_or_else(|| Error::Invalid("source master missing".into()))?;
    sample_elements(slide, master, layout)?;
    Ok(())
}

pub fn inspect(kind: SourceKind, encoded: &str, profile: crate::limits::CapacityProfile) -> Result<Value> {
    let source = load(kind, encoded, profile)?;
    let design = source.native.deck.design.as_ref().ok_or_else(|| Error::Unsupported("source has no slide masters".into()))?;
    let masters: Vec<_> = design.masters.iter().map(|master| {
        let reason = check_master(&source, &master.id).err().map(|error| error.to_string());
        json!({"id":master.id,"name":master.name,"layout_count":design.layouts.iter().filter(|layout| layout.master_id == master.id).count(),"importable":reason.is_none(),"reason":reason})
    }).collect();
    let slides: Vec<_> = source.native.deck.slides.iter().map(|slide| {
        let reason = check_slide(&source, &slide.id).err().map(|error| error.to_string());
        json!({"id":slide.id,"name":slide.title,"importable":reason.is_none(),"reason":reason})
    }).collect();
    Ok(json!({"kind":kind,"source_sha256":source.hash,"width":source.native.deck.width,"height":source.native.deck.height,"masters":masters,"slides":slides,"warnings":source_warnings(&source),"office_visual_parity":false}))
}

fn source_warnings(source: &Source) -> Vec<String> {
    let mut warnings: Vec<_> = source.native.warnings.iter().filter(|warning| !warning.starts_with("Opened from standard PPTX XML.") && !warning.starts_with("Embedded fonts are retained.")).map(|warning| format!("Source inspection: {warning}")).collect();
    warnings.push("Only selected editable design content is imported; source slides, notes, comments, source bindings and embedded fonts are not added. Ordinary sample text remains fixed, while existing placeholders stay editable.".into());
    warnings.push("Preview is not Office visual parity. Use locally available source fonts or explicitly embed licensed fonts separately.".into());
    warnings
}

fn remap(elements: &[Element], prefix: &str) -> Result<Vec<Element>> {
    let mapping: BTreeMap<_, _> = crate::model::element_list(elements).iter().enumerate().map(|(index, element)| (element.bounds().0.to_owned(), format!("{prefix}-{index}"))).collect();
    for element in crate::model::element_list(elements) {
        if let Element::Connector { start, end, .. } = element {
            if start.iter().chain(end.iter()).any(|connection| !mapping.contains_key(&connection.element_id)) {
                return Err(Error::Unsupported("sample connector references an excluded placeholder or another layer".into()));
            }
        }
    }
    fn visit(elements: &mut [Element], mapping: &BTreeMap<String, String>) {
        for element in elements {
            match element {
                Element::Text { id, format, .. } | Element::Shape { id, format, .. } => { *id = mapping[id].clone(); format.inherit_layout = false; }
                Element::Rect { id, .. } | Element::Polygon { id, .. } | Element::Picture { id, .. } | Element::Chart { id, .. } | Element::Table { id, .. } => *id = mapping[id].clone(),
                Element::Connector { id, start, end, .. } => {
                    *id = mapping[id].clone();
                    for connection in [start, end].into_iter().flatten() { if let Some(id) = mapping.get(&connection.element_id) { connection.element_id = id.clone(); } }
                }
                Element::Group { id, children, .. } => { *id = mapping[id].clone(); visit(children, mapping); }
            }
        }
    }
    let mut result = elements.to_vec(); visit(&mut result, &mapping); Ok(result)
}

fn sample_elements(slide: &Slide, master: &Master, layout: &SlideLayout) -> Result<Vec<Element>> {
    let placeholder = |element: &Element| matches!(element, Element::Text { format, .. } if format.placeholder.is_some());
    let bounds = |element: &Element| {
        let (_, x, y, width, height) = element.bounds();
        let rotation = match element { Element::Shape { rotation, .. } => *rotation, _ => element.visual().and_then(|visual| visual.rotation).unwrap_or(0.0) };
        let transform = kurbo::Affine::translate((x + width / 2.0, y + height / 2.0)) * kurbo::Affine::rotate(rotation.to_radians());
        transform.transform_rect_bbox(kurbo::Rect::new(-width / 2.0, -height / 2.0, width / 2.0, height / 2.0)).inflate(20.0, 20.0)
    };
    let effects = |element: &Element| crate::model::element_list(std::slice::from_ref(element)).iter().any(|element| element.visual().is_some_and(|visual| visual.shadow.is_some() || visual.glow.is_some() || visual.reflection.is_some() || visual.text_warp.is_some()));
    for (index, element) in slide.elements.iter().enumerate().filter(|(_, element)| placeholder(element)) {
        if slide.elements[index + 1..].iter().filter(|later| !placeholder(later)).any(|later| effects(element) || effects(later) || bounds(element).intersect(bounds(later)).area() > 0.0) {
            return Err(Error::Unsupported("sample placeholder stacking would change against later layout artwork".into()));
        }
    }
    let mut elements = Vec::new();
    for (layer, original) in [&master.elements, &layout.elements].into_iter().enumerate() {
        if layer == 0 && slide.hide_master_graphics { continue; }
        let visible: Vec<_> = original.iter().filter(|element| !placeholder(element)).cloned().collect();
        elements.extend(remap(&visible, &format!("layer-{layer}"))?);
    }
    elements.extend(remap(&slide.elements, "sample")?);
    Ok(elements)
}

fn candidate(document: &Document, revision: u64, hash: &str, input: &MasterImportInput) -> Result<(Deck, Vec<String>, Vec<String>, Vec<String>)> {
    document::verify(document)?;
    if document.revision != revision || document.hash != hash { return Err(Error::Conflict("stale master import document".into())); }
    if input.ids.is_empty() || input.ids.len() > 8 || input.ids.iter().collect::<BTreeSet<_>>().len() != input.ids.len() { return Err(Error::Limit("select 1-8 distinct source masters or slides".into())); }
    for id in &input.ids { crate::model::valid_text(id, 80)?; if id.trim().is_empty() { return Err(Error::Invalid("source identity cannot be empty".into())); } }
    if input.prefix.is_empty() || input.prefix.len() > 32 || !input.prefix.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_') { return Err(Error::Invalid("master import prefix must be 1-32 ASCII letters, digits, hyphens or underscores".into())); }
    crate::model::valid_text(&input.name, 60)?;
    if input.name.trim().is_empty() { return Err(Error::Invalid("master import name is required".into())); }
    let source = load(input.kind, &input.base64, document.capacity_profile)?;
    if source.hash != input.source_sha256 { return Err(Error::Conflict("master source hash changed".into())); }
    if (source.native.deck.width, source.native.deck.height) != (document.deck.width, document.deck.height) { return Err(Error::Unsupported(format!("source page is {}x{}, target is {}x{}; resize a separate source copy explicitly before importing", source.native.deck.width, source.native.deck.height, document.deck.width, document.deck.height))); }
    let source_design = source.native.deck.design.as_ref().ok_or_else(|| Error::Unsupported("source has no slide masters".into()))?;
    let mut deck = document.deck.clone();
    let design = deck.design.get_or_insert_with(|| {
        let mut design = crate::design::Design::default();
        design.layouts.retain(|layout| layout.elements.is_empty());
        design
    });
    let mut master_ids = Vec::new(); let mut layout_ids = Vec::new();
    for (index, id) in input.ids.iter().enumerate() {
        let master_id = format!("{}-m{}", input.prefix, index + 1);
        if design.masters.iter().any(|master| master.id == master_id) { return Err(Error::Conflict("master import prefix already exists".into())); }
        let name = if input.ids.len() == 1 { input.name.clone() } else { format!("{} {}", input.name, index + 1) };
        match input.mode {
            ImportMode::Masters => {
                check_master(&source, id)?;
                let mut master = source_design.masters.iter().find(|master| &master.id == id).ok_or_else(|| Error::Invalid("source master not found".into()))?.clone();
                master.id = master_id.clone(); master.name = name; master.theme = Some(crate::design::master_theme(source_design, id).clone());
                design.masters.push(master);
                for layout in source_design.layouts.iter().filter(|layout| &layout.master_id == id) {
                    let mut layout = layout.clone(); layout.id = format!("{}-l{}", input.prefix, layout_ids.len() + 1); layout.master_id = master_id.clone();
                    layout_ids.push(layout.id.clone()); design.layouts.push(layout);
                }
            }
            ImportMode::Slides => {
                check_slide(&source, id)?;
                let slide = source.native.deck.slides.iter().find(|slide| &slide.id == id).ok_or_else(|| Error::Invalid("source slide not found".into()))?;
                let layout = source_design.layouts.iter().find(|layout| Some(&layout.id) == slide.layout_id.as_ref()).ok_or_else(|| Error::Invalid("source layout missing".into()))?;
                let master = source_design.masters.iter().find(|master| master.id == layout.master_id).ok_or_else(|| Error::Invalid("source master missing".into()))?;
                let elements = sample_elements(slide, master, layout)?;
                let layout_id = format!("{}-l{}", input.prefix, layout_ids.len() + 1);
                let background = crate::design::slide_background(slide, Some(source_design)).to_owned();
                design.masters.push(Master { id: master_id.clone(), name, background, elements: Vec::new(), theme: Some(crate::design::master_theme(source_design, &master.id).clone()) });
                design.layouts.push(SlideLayout { id: layout_id.clone(), name: if slide.title.chars().count() <= 100 { slide.title.clone() } else { format!("{} {}", input.name, index + 1) }, master_id: master_id.clone(), background: None, elements });
                layout_ids.push(layout_id);
            }
        }
        master_ids.push(master_id);
    }
    if layout_ids.iter().any(|id| document.deck.design.as_ref().is_some_and(|design| design.layouts.iter().any(|layout| &layout.id == id))) { return Err(Error::Conflict("master import layout prefix already exists".into())); }
    crate::design::validate_design_on_canvas(design, deck.width, deck.height)?;
    crate::model::validate_deck(&deck)?;
    Ok((deck, master_ids, layout_ids, source_warnings(&source)))
}

fn transaction(document: &Document, revision: u64, hash: &str, deck: &Deck) -> Result<TransactionResult> {
    document::transact(document, Transaction { expected_revision: revision, expected_hash: hash.into(), operations: serde_json::from_value(json!([{"op":if document.deck.design.is_some() { "replace" } else { "add" },"path":"/deck/design","value":deck.design}]))? })
}

fn previews(deck: &Deck, layouts: &[String]) -> Result<Vec<Slide>> {
    let mut slides = Vec::new();
    for id in layouts {
        let mut preview = crate::editing::create("master-preview".into(), "Master preview".into())?.deck;
        preview.width = deck.width; preview.height = deck.height; preview.design = deck.design.clone();
        let slide_id = preview.slides[0].id.clone();
        let mut slide = crate::design::assign_layout(preview, &slide_id, id)?.slides.remove(0);
        slide.id = format!("preview-{id}");
        slides.push(slide);
    }
    Ok(slides)
}

pub fn preview(document: &Document, revision: u64, hash: &str, input: &MasterImportInput) -> Result<Value> {
    let (deck, masters, layouts, warnings) = candidate(document, revision, hash, input)?;
    let result = transaction(document, revision, hash, &deck)?;
    Ok(json!({"base_revision":revision,"base_hash":hash,"source_sha256":input.source_sha256,"candidate_hash":result.document.hash,"design":deck.design,"master_ids":masters,"layout_ids":layouts,"preview_slides":previews(&deck, &layouts)?,"warnings":warnings,"office_visual_parity":false}))
}

pub fn import(document: &Document, revision: u64, hash: &str, input: &MasterImportInput, expected_candidate_hash: &str) -> Result<TransactionResult> {
    let (deck, _, _, _) = candidate(document, revision, hash, input)?;
    let result = transaction(document, revision, hash, &deck)?;
    if result.document.hash != expected_candidate_hash { return Err(Error::Conflict("master import preview no longer matches".into())); }
    Ok(result)
}