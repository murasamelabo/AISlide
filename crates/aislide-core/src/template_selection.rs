use super::{TemplateImportOptions, CT, POTX, PPTX};
use crate::{native::{child, relations_path, P, R, REL}, native_save::{apply, insert_child}, package::Package, pptx::{parse, relationship_targets, resolve}, Error, Result};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const DESIGN: &str = "urn:aislide:design:1";
const P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";
const PROVENANCE: &str = "urn:aislide:provenance:1";

#[derive(Serialize)]
struct Master { id: String, name: String }
#[derive(Serialize)]
struct Layout { id: String, name: String, master_id: String }
struct Topology { main: String, masters: Vec<Master>, layouts: Vec<Layout>, slides: Vec<(String, String)>, sections: bool, custom_shows: bool }

fn checked_source(bytes: Vec<u8>) -> Result<Package> {
    let package = Package::open(bytes)?;
    super::safe(&package)?;
    crate::review::ensure_unprotected(&package)?;
    let main = super::main_part(&package)?;
    if super::content_type(&package, &main)? != POTX { return Err(Error::Unsupported("template factory requires explicitly selected non-macro POTX".into())); }
    for path in package.part_names().into_iter().filter(|path| path.ends_with(".rels")) {
        let source = if path == "_rels/.rels" { String::new() }
            else if let Some((folder, name)) = path.rsplit_once("/_rels/") { format!("{folder}/{}", name.trim_end_matches(".rels")) }
            else if let Some(name) = path.strip_prefix("_rels/") { name.trim_end_matches(".rels").into() }
            else { return Err(Error::Invalid("template relationship part path".into())); };
        let parsed = parse(package.text(path)?)?;
        if !parsed.root_element().has_tag_name((REL, "Relationships")) { return Err(Error::Invalid("template relationships root".into())); }
        let mut ids = BTreeSet::new();
        for entry in parsed.root_element().children().filter(|node| node.is_element()) {
            if !entry.has_tag_name((REL, "Relationship")) { return Err(Error::Unsupported("template relationship element".into())); }
            let id = entry.attribute("Id").ok_or_else(|| Error::Invalid("template relationship ID missing".into()))?;
            if id.is_empty() || !ids.insert(id) { return Err(Error::Invalid("duplicate template relationship ID".into())); }
            let target = entry.attribute("Target").filter(|target| !target.is_empty()).ok_or_else(|| Error::Invalid("template relationship target missing".into()))?;
            if entry.attribute("TargetMode").is_some_and(|mode| mode != "Internal") {
                if entry.attribute("TargetMode") == Some("External") && entry.attribute("Type") == Some(format!("{R}/hyperlink").as_str()) { continue; }
                return Err(Error::Unsupported("template inspection/selection rejects external resources except inert hyperlinks; no external content is fetched".into()));
            }
            let target = resolve(&source, target)?;
            package.part(&target)?;
        }
    }
    Ok(package)
}

fn listed_parts(package: &Package, path: &str, root: &str, list: &str, item: &str, kind: &str, limit: usize) -> Result<Vec<String>> {
    let parsed = parse(package.text(path)?)?;
    if !parsed.root_element().has_tag_name((P, root)) { return Err(Error::Unsupported(format!("template requires Transitional {root}"))); }
    let lists: Vec<_> = parsed.root_element().children().filter(|node| node.has_tag_name((P, list))).collect();
    if lists.len() != 1 { return Err(Error::Invalid(format!("template requires one {list}"))); }
    let targets = relationship_targets(package, path, kind)?;
    let mut result = Vec::new(); let mut seen = BTreeSet::new(); let mut numeric = BTreeSet::new();
    for node in lists[0].children().filter(|node| node.has_tag_name((P, item))) {
        let target = node.attribute((R, "id")).and_then(|id| targets.get(id)).ok_or_else(|| Error::Invalid(format!("template {item} relationship missing")))?;
        let number = node.attribute("id").and_then(|id| id.parse::<u32>().ok()).ok_or_else(|| Error::Invalid(format!("template {item} numeric ID missing")))?;
        if !seen.insert(target.clone()) || !numeric.insert(number) { return Err(Error::Invalid(format!("duplicate template {item}"))); }
        if result.len() == limit { return Err(Error::Limit(format!("template inspection supports up to {limit} {kind} entries"))); }
        result.push(target.clone());
    }
    Ok(result)
}

fn name(package: &Package, path: &str, root: &str) -> Result<String> {
    let parsed = parse(package.text(path)?)?;
    if !parsed.root_element().has_tag_name((P, root)) { return Err(Error::Unsupported(format!("template requires Transitional {root}"))); }
    let name = child(parsed.root_element(), P, "cSld").and_then(|node| node.attribute("name")).unwrap_or("").to_owned();
    crate::model::valid_text(&name, 100)?;
    Ok(name)
}

fn topology(package: &Package) -> Result<Topology> {
    let main = super::main_part(package)?;
    let masters = listed_parts(package, &main, "presentation", "sldMasterIdLst", "sldMasterId", "slideMaster", 8)?;
    if masters.is_empty() { return Err(Error::Invalid("template has no declared masters".into())); }
    let mut result = Topology { main, masters: Vec::new(), layouts: Vec::new(), slides: Vec::new(), sections: false, custom_shows: false };
    let mut seen = BTreeSet::new();
    for master in masters {
        result.masters.push(Master { name: name(package, &master, "sldMaster")?, id: master.clone() });
        for path in listed_parts(package, &master, "sldMaster", "sldLayoutIdLst", "sldLayoutId", "slideLayout", 256)? {
            if result.layouts.len() == 256 { return Err(Error::Limit("template inspection supports up to 256 layouts".into())); }
            if !seen.insert(path.clone()) { return Err(Error::Invalid("template layout has multiple declared owners".into())); }
            let owners = relationship_targets(package, &path, "slideMaster")?;
            if owners.len() != 1 || owners.values().next() != Some(&master) { return Err(Error::Invalid("template layout owning master disagrees with declared topology".into())); }
            result.layouts.push(Layout { name: name(package, &path, "sldLayout")?, id: path, master_id: master.clone() });
        }
    }
    if result.layouts.is_empty() { return Err(Error::Invalid("template has no declared layouts".into())); }
    let parsed = parse(package.text(&result.main)?)?;
    let slides = if child(parsed.root_element(), P, "sldIdLst").is_some() { crate::pptx::slide_paths(package)? } else { Vec::new() };
    for path in slides {
        let layouts = relationship_targets(package, &path, "slideLayout")?;
        if layouts.len() != 1 { return Err(Error::Invalid("template sample requires one layout".into())); }
        let layout = layouts.into_values().next().ok_or_else(|| Error::Invalid("sample layout missing".into()))?;
        if !seen.contains(&layout) { return Err(Error::Invalid("template sample layout is undeclared".into())); }
        result.slides.push((path, layout));
    }
    result.sections = parsed.descendants().any(|node| node.has_tag_name((P14, "sectionLst")));
    result.custom_shows = parsed.descendants().any(|node| node.has_tag_name((P, "custShowLst")));
    validate_design_identities(package, &result)?;
    Ok(result)
}

fn design_entries<'a>(parsed: &'a roxmltree::Document<'a>) -> Result<Vec<roxmltree::Node<'a, 'a>>> {
    let extensions: Vec<_> = child(parsed.root_element(), P, "extLst").into_iter().flat_map(|list| list.children())
        .filter(|node| node.has_tag_name((P, "ext")) && node.attribute("uri") == Some(DESIGN)).collect();
    if extensions.len() > 1 { return Err(Error::Invalid("duplicate native design metadata".into())); }
    let Some(extension) = extensions.first() else { return Ok(Vec::new()) };
    let roots: Vec<_> = extension.children().filter(|node| node.has_tag_name((DESIGN, "design"))).collect();
    if roots.len() != 1 { return Err(Error::Invalid("native design metadata missing or ambiguous".into())); }
    Ok(roots[0].children().filter(|node| node.is_element()).collect())
}

fn validate_design_identities(package: &Package, topology: &Topology) -> Result<()> {
    let parsed = parse(package.text(&topology.main)?)?;
    let entries = design_entries(&parsed)?;
    if entries.is_empty() {
        if parsed.descendants().any(|node| node.has_tag_name((DESIGN, "design"))) { return Err(Error::Invalid("empty or misplaced native design metadata".into())); }
        return Ok(());
    }
    let mut masters = Vec::new(); let mut layouts = BTreeSet::new(); let mut ids = BTreeSet::new();
    for entry in entries {
        let id = entry.attribute("id").ok_or_else(|| Error::Invalid("native design identity missing".into()))?;
        let path = entry.attribute("part").ok_or_else(|| Error::Invalid("native design part missing".into()))?;
        crate::model::valid_text(id, 80)?;
        if id.is_empty() || !ids.insert(id) || resolve("", path)? != path { return Err(Error::Invalid("duplicate or invalid native design identity".into())); }
        if entry.has_tag_name((DESIGN, "master")) { masters.push(path); }
        else if entry.has_tag_name((DESIGN, "layout")) { if !layouts.insert(path) { return Err(Error::Invalid("duplicate native design part".into())); } }
        else { return Err(Error::Unsupported("native design metadata element".into())); }
    }
    if masters.iter().copied().ne(topology.masters.iter().map(|master| master.id.as_str())) || layouts != topology.layouts.iter().map(|layout| layout.id.as_str()).collect() {
        return Err(Error::Conflict("native design metadata no longer matches declared template topology".into()));
    }
    Ok(())
}

pub(super) fn inspect(bytes: Vec<u8>) -> Result<Value> {
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let package = checked_source(bytes)?;
    let topology = topology(&package)?;
    Ok(json!({"source_sha256":hash,"masters":topology.masters,"layouts":topology.layouts,
        "sample_slide_count":topology.slides.len(),"has_sections":topology.sections,"has_custom_shows":topology.custom_shows,
        "limits":{"masters":8,"layouts":256,"sample_slides":256,"selected_masters":8,"selected_layouts":32},
        "warnings":["IDs are exact source package part names, not filesystem paths. Selection is not sanitization; unselected payloads remain embedded.","Metadata inspection does not certify editable content or Office visual parity."],"office_visual_parity":false}))
}

fn selectors(ids: &[String], names: &[String], available: &[(String, String)], limit: usize, kind: &str) -> Result<BTreeSet<String>> {
    if ids.len() + names.len() > limit { return Err(Error::Limit(format!("select at most {limit} distinct {kind} entries"))); }
    let mut selected = BTreeSet::new();
    for id in ids {
        crate::model::valid_text(id, 512)?;
        if !available.iter().any(|(path, _)| path == id) { return Err(Error::Invalid(format!("{kind} ID not found in this template: {id}"))); }
        if !selected.insert(id.clone()) { return Err(Error::Invalid(format!("duplicate {kind} selector"))); }
    }
    for name in names {
        crate::model::valid_text(name, 100)?;
        if name.is_empty() { return Err(Error::Invalid(format!("{kind} name selector cannot be empty"))); }
        let matches: Vec<_> = available.iter().filter(|(_, candidate)| candidate == name).collect();
        if matches.len() != 1 { return Err(Error::Invalid(format!("{kind} name is missing or ambiguous: {name}; use an inspected ID"))); }
        if !selected.insert(matches[0].0.clone()) { return Err(Error::Invalid(format!("duplicate {kind} selector"))); }
    }
    Ok(selected)
}

fn retain_list(package: &mut Package, source: &str, list_name: &str, item: &str, kind: &str, keep: &BTreeSet<String>) -> Result<()> {
    let targets = relationship_targets(package, source, kind)?;
    let xml = package.text(source)?.to_owned(); let parsed = parse(&xml)?;
    let edits = match child(parsed.root_element(), P, list_name) {
        Some(list) => list.children().filter(|node| node.has_tag_name((P, item)))
            .filter(|node| node.attribute((R, "id")).and_then(|id| targets.get(id)).is_some_and(|path| !keep.contains(path)))
            .map(|node| (node.range(), String::new())).collect::<Vec<_>>(),
        None if list_name == "sldIdLst" => Vec::new(),
        None => return Err(Error::Invalid(format!("template {list_name} missing"))),
    };
    if !edits.is_empty() { package.replace_part(source, apply(xml, edits)?)?; }
    let path = relations_path(source); let xml = package.text(&path)?.to_owned(); let parsed = parse(&xml)?;
    let owner = parse(package.text(source)?)?; let mut edits = Vec::new();
    for node in parsed.root_element().children().filter(|node| node.has_tag_name((REL, "Relationship")) && node.attribute("Type") == Some(format!("{R}/{kind}").as_str())) {
        let id = node.attribute("Id").ok_or_else(|| Error::Invalid("template relationship ID missing".into()))?;
        if targets.get(id).is_some_and(|target| keep.contains(target)) { continue; }
        if owner.descendants().any(|node| node.attributes().any(|attribute| attribute.namespace() == Some(R) && attribute.value() == id)) { return Err(Error::Unsupported("retained template XML references an excluded relationship".into())); }
        edits.push((node.range(), String::new()));
    }
    if !edits.is_empty() { package.replace_part(&path, apply(xml, edits)?)?; }
    Ok(())
}

fn reconcile_metadata(package: &mut Package, topology: &Topology, keep: &BTreeSet<String>, keep_samples: bool) -> Result<()> {
    if let Some((path, metadata)) = crate::provenance::read(package)? {
        if !keep_samples && (metadata.references.is_some() || metadata.guided_record.is_some()) { return Err(Error::Unsupported("sample removal with managed references or a guided record is not supported".into())); }
        let active: BTreeSet<_> = topology.masters.iter().map(|master| master.id.as_str()).chain(topology.layouts.iter().map(|layout| layout.id.as_str())).chain(topology.slides.iter().map(|(path, _)| path.as_str())).collect();
        let removed_slides: BTreeSet<_> = metadata.identities.iter().filter(|identity| topology.slides.iter().any(|(path, _)| path == &identity.part) && !keep_samples).map(|identity| identity.id.as_str()).collect();
        if !keep_samples && metadata.bindings.iter().any(|binding| !removed_slides.contains(binding.slide_id.as_str())) { return Err(Error::Unsupported("sample removal requires bound slide identities in template metadata".into())); }
        if !keep_samples && metadata.parts.iter().any(|part| !removed_slides.contains(part.slide_id.as_str())) { return Err(Error::Unsupported("sample removal requires managed part slide identities in template metadata".into())); }
        let xml = package.text(&path)?.to_owned(); let parsed = parse(&xml)?; let mut edits = Vec::new();
        let root = child(parsed.root_element(), PROVENANCE, "object").ok_or_else(|| Error::Invalid("template provenance object missing".into()))?;
        for (field, flags) in [
            ("identities", metadata.identities.iter().map(|identity| !active.contains(identity.part.as_str()) || keep.contains(&identity.part)).collect::<Vec<_>>()),
            ("bindings", metadata.bindings.iter().map(|_| keep_samples).collect()),
            ("parts", metadata.parts.iter().map(|_| keep_samples).collect()),
        ] {
            let Some(node) = root.children().find(|node| node.has_tag_name((PROVENANCE, "field")) && node.attribute("name") == Some(field)) else { continue };
            let array = child(node, PROVENANCE, "array").ok_or_else(|| Error::Invalid("template provenance array missing".into()))?;
            let entries: Vec<_> = array.children().filter(|node| node.is_element()).collect();
            if entries.len() != flags.len() { return Err(Error::Invalid("template provenance array disagrees with parsed records".into())); }
            for (entry, retain) in entries.into_iter().zip(flags) { if !retain { edits.push((entry.range(), String::new())); } }
        }
        if !edits.is_empty() { package.replace_part(&path, apply(xml, edits)?)?; }
    }
    let xml = package.text(&topology.main)?.to_owned(); let parsed = parse(&xml)?;
    let edits = design_entries(&parsed)?.into_iter().filter(|entry| entry.attribute("part").is_some_and(|part| !keep.contains(part))).map(|node| (node.range(), String::new())).collect::<Vec<_>>();
    if !edits.is_empty() { package.replace_part(&topology.main, apply(xml, edits)?)?; }
    Ok(())
}

fn blank_slide(package: &mut Package, main: &str, layout: &str) -> Result<()> {
    let mut deck = crate::editing::create("template-blank".into(), "Template".into())?.deck;
    deck.slides[0].inherit_background = true;
    let generated = Package::open(crate::pptx::export_pptx(&deck)?)?;
    let path = (1..=8192).map(|number| format!("ppt/slides/aislide-template-blank-{number}.xml"))
        .find(|path| !package.part_names().iter().any(|existing| existing.eq_ignore_ascii_case(path) || existing.eq_ignore_ascii_case(&relations_path(path))))
        .ok_or_else(|| Error::Limit("template blank slide names exhausted".into()))?;
    package.add_part(path.clone(), generated.part("ppt/slides/slide1.xml")?.to_vec())?;
    package.add_part(relations_path(&path), format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"rId1\" Type=\"{R}/slideLayout\" Target=\"/{}\"/></Relationships>", quick_xml::escape::escape(layout)).into_bytes())?;
    let relation_path = relations_path(main); let xml = package.text(&relation_path)?.to_owned(); let parsed = parse(&xml)?;
    let used: BTreeSet<_> = parsed.root_element().children().filter_map(|node| node.attribute("Id")).collect();
    let id = (1..=8192).map(|number| format!("rIdTemplateBlank{number}")).find(|id| !used.contains(id.as_str())).ok_or_else(|| Error::Limit("template relationship IDs exhausted".into()))?;
    let mut edits = Vec::new();
    insert_child(&xml, parsed.root_element(), &format!("<Relationship xmlns=\"{REL}\" Id=\"{id}\" Type=\"{R}/slide\" Target=\"/{path}\"/>"), None, &mut edits)?;
    package.replace_part(&relation_path, apply(xml, edits)?)?;
    let xml = package.text(main)?.to_owned(); let parsed = parse(&xml)?;
    let mut edits = Vec::new();
    let fragment = format!("<p:sldId xmlns:p=\"{P}\" xmlns:r=\"{R}\" id=\"256\" r:id=\"{id}\"/>");
    if let Some(list) = child(parsed.root_element(), P, "sldIdLst") {
        insert_child(&xml, list, &fragment, child(list, P, "extLst"), &mut edits)?;
    } else {
        let before = parsed.root_element().children().find(|node| node.tag_name().namespace() == Some(P) && matches!(node.tag_name().name(),
            "sldSz" | "notesSz" | "smartTags" | "embeddedFontLst" | "custShowLst" | "photoAlbum" | "custDataLst" | "kinsoku" | "defaultTextStyle" | "modifyVerifier" | "extLst"));
        insert_child(&xml, parsed.root_element(), &format!("<p:sldIdLst xmlns:p=\"{P}\">{fragment}</p:sldIdLst>"), before, &mut edits)?;
    }
    package.replace_part(main, apply(xml, edits)?)?;
    let xml = package.text("[Content_Types].xml")?.to_owned(); let parsed = parse(&xml)?;
    let mut edits = Vec::new();
    insert_child(&xml, parsed.root_element(), &format!("<Override xmlns=\"{CT}\" PartName=\"/{path}\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>"), None, &mut edits)?;
    package.replace_part("[Content_Types].xml", apply(xml, edits)?)?;
    Ok(())
}

pub(super) fn select(bytes: Vec<u8>, options: &TemplateImportOptions) -> Result<Package> {
    if let Some(expected) = &options.source_sha256 {
        if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()) { return Err(Error::Invalid("source_sha256 must be 64 lowercase hexadecimal characters".into())); }
        if *expected != format!("{:x}", Sha256::digest(&bytes)) { return Err(Error::Conflict("template source SHA-256 changed; inspect the current source again".into())); }
    }
    let mut package = checked_source(bytes)?;
    let has_selection = !options.master_ids.is_empty() || !options.master_names.is_empty() || !options.layout_ids.is_empty() || !options.layout_names.is_empty();
    if !has_selection && options.include_sample_slides {
        let main = super::main_part(&package)?;
        if options.strip_sections { super::strip_sections(&mut package, &main)?; }
        return Package::open(super::convert(package, POTX, PPTX)?);
    }
    let topology = topology(&package)?;
    let requested_masters = selectors(&options.master_ids, &options.master_names, &topology.masters.iter().map(|master| (master.id.clone(), master.name.clone())).collect::<Vec<_>>(), 8, "master")?;
    let requested_layouts = selectors(&options.layout_ids, &options.layout_names, &topology.layouts.iter().map(|layout| (layout.id.clone(), layout.name.clone())).collect::<Vec<_>>(), 32, "layout")?;
    let layouts: BTreeSet<_> = topology.layouts.iter().filter(|layout| {
        if !requested_layouts.is_empty() { requested_layouts.contains(&layout.id) }
        else { requested_masters.is_empty() || requested_masters.contains(&layout.master_id) }
    }).map(|layout| layout.id.clone()).collect();
    if layouts.is_empty() || layouts.len() > 32 { return Err(Error::Limit("template import requires 1-32 selected layouts; run inspect_template and select layout_ids/layout_names or a smaller master".into())); }
    if !requested_masters.is_empty() && topology.layouts.iter().filter(|layout| layouts.contains(&layout.id)).any(|layout| !requested_masters.contains(&layout.master_id)) { return Err(Error::Invalid("selected layout is outside the selected masters".into())); }
    let mut masters: BTreeSet<_> = topology.layouts.iter().filter(|layout| layouts.contains(&layout.id)).map(|layout| layout.master_id.clone()).collect();
    masters.extend(requested_masters);
    if options.include_sample_slides && topology.slides.iter().any(|(_, layout)| !layouts.contains(layout)) { return Err(Error::Unsupported("retained sample slides reference excluded layouts; include their layouts or set include_sample_slides=false".into())); }
    if !options.include_sample_slides && topology.sections && !options.strip_sections { return Err(Error::Unsupported("sample removal requires strip_sections=true for a template with sections".into())); }
    if !options.include_sample_slides && topology.custom_shows { return Err(Error::Unsupported("sample removal would invalidate custom slide shows; remove them in PowerPoint first".into())); }
    if options.strip_sections { super::strip_sections(&mut package, &topology.main)?; }
    let mut keep = layouts.clone(); keep.extend(masters.iter().cloned());
    if options.include_sample_slides { keep.extend(topology.slides.iter().map(|(path, _)| path.clone())); }
    reconcile_metadata(&mut package, &topology, &keep, options.include_sample_slides)?;
    retain_list(&mut package, &topology.main, "sldMasterIdLst", "sldMasterId", "slideMaster", &masters)?;
    for master in &topology.masters { if masters.contains(&master.id) { retain_list(&mut package, &master.id, "sldLayoutIdLst", "sldLayoutId", "slideLayout", &layouts)?; } }
    if !options.include_sample_slides {
        retain_list(&mut package, &topology.main, "sldIdLst", "sldId", "slide", &BTreeSet::new())?;
        let layout = topology.layouts.iter().find(|layout| layouts.contains(&layout.id)).ok_or_else(|| Error::Invalid("selected template layout missing".into()))?;
        blank_slide(&mut package, &topology.main, &layout.id)?;
    }
    Package::open(super::convert(package, POTX, PPTX)?)
}