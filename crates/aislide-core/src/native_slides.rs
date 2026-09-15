use crate::{model::{Deck, Slide, element_list}, native::{P, R, REL, NativeDeck, NativePart, child, relations_path}, native_save::{apply, insert_child, scaled_element, resource_changes, remap_resources}, package::Package, pptx::{parse, relationship_targets, resolve}, Error, Result};
use std::collections::{BTreeMap, BTreeSet};

fn add_type(package: &mut Package, path: &str, kind: &str) -> Result<()> {
    let xml = package.text("[Content_Types].xml")?.to_owned(); let parsed = parse(&xml)?; let mut edits = Vec::new();
    insert_child(&xml, parsed.root_element(), &format!("<Override xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\" PartName=\"/{path}\" ContentType=\"{kind}\"/>"), None, &mut edits)?;
    package.replace_part("[Content_Types].xml", apply(xml, edits)?)
}

fn shared_resource(kind: &str) -> bool { matches!(kind.rsplit('/').next(), Some("slideLayout" | "slideMaster" | "notesMaster" | "theme" | "image" | "slide")) }

fn clone_resource(package: &mut Package, source: &str, scope: &str, copied: &mut BTreeMap<String, String>, total: &mut usize) -> Result<String> {
    if let Some(path) = copied.get(source) { return Ok(path.clone()); }
    if copied.len() >= 128 { return Err(Error::Limit("slide copy exceeds 128 related resources".into())); }
    let bytes = package.part(source)?.to_vec();
    *total += bytes.len();
    if *total > 8 * 1024 * 1024 { return Err(Error::Limit("slide copy resources exceed 8 MiB".into())); }
    let extension = source.rsplit('.').next().filter(|extension| extension.bytes().all(|byte| byte.is_ascii_alphanumeric())).ok_or_else(|| Error::Unsupported("native resource extension".into()))?;
    let types = parse(package.text("[Content_Types].xml")?)?;
    let kind = types.root_element().children().find(|node| node.attribute("PartName") == Some(format!("/{source}").as_str())).or_else(|| types.root_element().children().find(|node| node.attribute("Extension") == Some(extension))).and_then(|node| node.attribute("ContentType")).ok_or_else(|| Error::Unsupported("native resource content type missing".into()))?.to_owned();
    let mut counter = copied.len() + 1;
    let path = loop { let candidate = format!("ppt/aislideCopies/{scope}-{counter}.{extension}"); counter += 1; if !package.part_names().contains(candidate.as_str()) { break candidate; } };
    package.add_part(path.clone(), bytes)?; add_type(package, &path, &kind)?; copied.insert(source.into(), path.clone());
    let source_rel = relations_path(source);
    if package.part_names().contains(source_rel.as_str()) {
        let xml = package.text(&source_rel)?.to_owned(); let document = parse(&xml)?; let mut edits = Vec::new();
        for relation in document.root_element().children().filter(|node| node.is_element()) {
            if relation.attribute("TargetMode") == Some("External") { continue; }
            let attribute = relation.attributes().find(|attribute| attribute.name() == "Target").ok_or_else(|| Error::Invalid("native resource relationship".into()))?;
            let target = resolve(source, attribute.value())?;
            let target = if shared_resource(relation.attribute("Type").unwrap_or("")) { copied.get(&target).cloned().unwrap_or(target) } else { clone_resource(package, &target, scope, copied, total)? };
            edits.push((attribute.range_value(), quick_xml::escape::escape(&format!("/{target}")).into_owned()));
        }
        package.add_part(relations_path(&path), apply(xml, edits)?)?;
    }
    Ok(path)
}

fn clone_slide(package: &mut Package, original: &NativePart, path: &str, id: &str) -> Result<NativePart> {
    package.add_part(path.into(), package.part(&original.path)?.to_vec())?;
    let mut copied = BTreeMap::from([(original.path.clone(), path.to_owned())]); let mut total = 0;
    let scope = path.rsplit('/').next().unwrap_or("copy").trim_end_matches(".xml");
    let xml = package.text(&relations_path(&original.path))?.to_owned(); let parsed = parse(&xml)?; let mut edits = Vec::new();
    for relation in parsed.root_element().children().filter(|node| node.is_element()) {
        if relation.attribute("TargetMode") == Some("External") { continue; }
        let attribute = relation.attributes().find(|attribute| attribute.name() == "Target").ok_or_else(|| Error::Invalid("slide relationship target".into()))?;
        let target = resolve(&original.path, attribute.value())?;
        let target = if relation.attribute("Type") == Some(format!("{R}/notesSlide").as_str()) {
            let note = path.replace("/slides/", "/notesSlides/");
            package.add_part(note.clone(), package.part(&target)?.to_vec())?;
            add_type(package, &note, "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml")?;
            let relationships = package.text(&relations_path(&target))?.to_owned(); let document = parse(&relationships)?; let mut changes = Vec::new();
            for entry in document.root_element().children().filter(|node| node.is_element()) {
                if entry.attribute("TargetMode") == Some("External") { continue; }
                let attribute = entry.attributes().find(|attribute| attribute.name() == "Target").ok_or_else(|| Error::Invalid("notes relationship".into()))?;
                let value = if entry.attribute("Type") == Some(format!("{R}/slide").as_str()) { path.to_owned() } else { resolve(&target, attribute.value())? };
                changes.push((attribute.range_value(), quick_xml::escape::escape(&format!("/{value}")).into_owned()));
            }
            package.add_part(relations_path(&note), apply(relationships, changes)?)?;
            note
        } else if shared_resource(relation.attribute("Type").unwrap_or("")) { copied.get(&target).cloned().unwrap_or(target) }
        else { clone_resource(package, &target, scope, &mut copied, &mut total)? };
        edits.push((attribute.range_value(), quick_xml::escape::escape(&format!("/{target}")).into_owned()));
    }
    package.add_part(relations_path(path), apply(xml, edits)?)?;
    Ok(NativePart { path: path.into(), id: id.into(), nodes: original.nodes.clone() })
}

fn insert_slide(package: &mut Package, original: &NativeDeck, deck: &Deck, slide: &Slide, path: &str, main: &str, scale: (f64, f64)) -> Result<NativePart> {
    let design = deck.design.as_ref().ok_or_else(|| Error::Invalid("native design missing".into()))?;
    let layout_id = slide.layout_id.as_ref().unwrap_or(&design.layouts[0].id);
    let layout = original.layouts.iter().find(|layout| &layout.id == layout_id).ok_or_else(|| Error::Invalid("native layout missing".into()))?;
    let blank = Slide { id: slide.id.clone(), elements: Vec::new(), layout_id: None, native_source_id: None, ..slide.clone() };
    let generated = Package::open(crate::pptx::export_pptx(&Deck { slides: vec![blank], design: None, ..deck.clone() })?)?;
    let xml = generated.text("ppt/slides/slide1.xml")?.to_owned(); let parsed = parse(&xml)?;
    let tree = child(parsed.root_element(), P, "cSld").and_then(|node| child(node, P, "spTree")).ok_or_else(|| Error::Invalid("generated slide tree".into()))?;
    let ids: BTreeMap<_, _> = element_list(&slide.elements).into_iter().enumerate().map(|(index, element)| (element.bounds().0, index + 2)).collect();
    let binding = NativePart { path: path.into(), id: slide.id.clone(), nodes: ids.iter().map(|(id, numeric)| ((*id).into(), numeric.to_string())).collect() };
    let mut relationships = format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"rIdLayout\" Type=\"{R}/slideLayout\" Target=\"/{}\"/>", layout.path);
    if let Some(master) = relationship_targets(package, main, "notesMaster")?.into_values().next() {
        let notes = path.replace("/slides/", "/notesSlides/");
        package.add_part(notes.clone(), generated.part("ppt/notesSlides/notesSlide1.xml")?.to_vec())?;
        add_type(package, &notes, "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml")?;
        package.add_part(relations_path(&notes), format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"rIdSlide\" Type=\"{R}/slide\" Target=\"/{path}\"/><Relationship Id=\"rIdMaster\" Type=\"{R}/notesMaster\" Target=\"/{master}\"/></Relationships>").into_bytes())?;
        relationships.push_str(&format!("<Relationship Id=\"rIdNotes\" Type=\"{R}/notesSlide\" Target=\"/{notes}\"/>"));
    } else if !slide.notes.is_empty() { return Err(Error::Unsupported("new notes require a native notes master".into())); }
    relationships.push_str("</Relationships>");
    package.add_part(relations_path(path), relationships.into_bytes())?;
    let resources = resource_changes(package, &binding, &[], &slide.elements, &ids)?;
    let mut elements = String::new();
    for element in &slide.elements { elements.push_str(&remap_resources(scaled_element(element, &ids, &design.theme, scale)?, &resources)?); }
    let mut edits = Vec::new(); insert_child(&xml, tree, &elements, None, &mut edits)?;
    package.add_part(path.into(), apply(xml, edits)?)?;
    Ok(binding)
}

fn remove_deleted(package: &mut Package, original: &NativeDeck, deck: &Deck, main: &str) -> Result<()> {
    let mut removed = BTreeSet::new();
    for slide in original.slides.iter().filter(|part| !deck.slides.iter().any(|slide| slide.id == part.id)) {
        removed.insert(slide.path.clone());
        removed.insert(relations_path(&slide.path));
        for note in relationship_targets(package, &slide.path, "notesSlide")?.into_values() {
            removed.insert(relations_path(&note)); removed.insert(note);
        }
    }
    if removed.is_empty() { return Ok(()); }
    let relations: Vec<_> = package.part_names().into_iter().filter(|path| path.ends_with(".rels") && !removed.contains(*path)).map(str::to_owned).collect();
    for path in relations {
        let source = if path == "_rels/.rels" { String::new() }
            else if let Some((folder, name)) = path.rsplit_once("/_rels/") { format!("{folder}/{}", name.trim_end_matches(".rels")) }
            else if let Some(name) = path.strip_prefix("_rels/") { name.trim_end_matches(".rels").to_owned() }
            else { return Err(Error::Unsupported("nonstandard relationship part path".into())); };
        let xml = package.text(&path)?.to_owned(); let parsed = parse(&xml)?; let mut edits = Vec::new();
        for relation in parsed.root_element().children().filter(|node| node.is_element()) {
            if relation.attribute("TargetMode") == Some("External") { continue; }
            let target = relation.attribute("Target").ok_or_else(|| Error::Invalid("native relationship target missing".into()))?;
            if removed.contains(&resolve(&source, target)?) {
                if source == main && relation.attribute("Type") == Some(format!("{R}/slide").as_str()) { edits.push((relation.range(), String::new())); }
                else { return Err(Error::Unsupported("a remaining object references the deleted slide or notes; remove the link before deleting the slide".into())); }
            }
        }
        if !edits.is_empty() { package.replace_part(&path, apply(xml, edits)?)?; }
    }
    let xml = package.text("[Content_Types].xml")?.to_owned(); let parsed = parse(&xml)?;
    let edits = parsed.root_element().children().filter(|node| node.attribute("PartName").is_some_and(|name| removed.contains(name.trim_start_matches('/')))).map(|node| (node.range(), String::new())).collect();
    package.replace_part("[Content_Types].xml", apply(xml, edits)?)?;
    for path in removed { if package.part_names().contains(path.as_str()) { package.remove_part(&path)?; } }
    Ok(())
}

pub(crate) fn prepare(package: &mut Package, original: &NativeDeck, deck: &Deck, main: &str, scale: (f64, f64)) -> Result<Vec<NativePart>> {
    if original.deck.slides.iter().map(|slide| &slide.id).eq(deck.slides.iter().map(|slide| &slide.id)) {
        if deck.slides.iter().any(|slide| slide.native_source_id.is_some()) { return Err(Error::Unsupported("existing native slide cannot change its source".into())); }
        return original.slides.iter().map(|part| Ok(NativePart { path: part.path.clone(), id: part.id.clone(), nodes: part.nodes.clone() })).collect();
    }
    let xml = package.text(main)?.to_owned(); let presentation = parse(&xml)?;
    if presentation.descendants().any(|node| ["sectionLst", "custShowLst"].contains(&node.tag_name().name())) { return Err(Error::Unsupported("native sections/custom slide shows must be removed in PowerPoint before changing slide structure".into())); }
    let list = child(presentation.root_element(), P, "sldIdLst").ok_or_else(|| Error::Invalid("slide list missing".into()))?;
    let relation_path = relations_path(main); let relationships = package.text(&relation_path)?.to_owned(); let rels = parse(&relationships)?;
    let resolved = relationship_targets(package, main, "slide")?;
    let mut used: BTreeSet<String> = rels.root_element().children().filter_map(|node| node.attribute("Id").map(str::to_owned)).collect();
    let mut next_id = list.children().filter_map(|node| node.attribute("id").and_then(|value| value.parse::<u32>().ok())).max().unwrap_or(255);
    let mut entries = String::new(); let mut extra = String::new(); let mut bindings = Vec::new(); let mut counter = 1;
    for slide in &deck.slides {
        if let Some(binding) = original.slides.iter().find(|part| part.id == slide.id) {
            if slide.native_source_id.is_some() { return Err(Error::Unsupported("existing native slide cannot change its source".into())); }
            let relation = resolved.iter().find(|(_, path)| *path == &binding.path).map(|(id, _)| id).ok_or_else(|| Error::Invalid("slide relation missing".into()))?;
            let entry = list.children().find(|node| node.attribute((R, "id")) == Some(relation)).ok_or_else(|| Error::Invalid("slide list relation missing".into()))?;
            entries.push_str(&xml[entry.range()]);
            bindings.push(NativePart { path: binding.path.clone(), id: binding.id.clone(), nodes: binding.nodes.clone() });
            continue;
        }
        let path = loop { let candidate = format!("ppt/slides/aislide-added-{counter}.xml"); counter += 1; if !package.part_names().contains(candidate.as_str()) && !package.part_names().contains(candidate.replace("/slides/", "/notesSlides/").as_str()) { break candidate; } };
        let binding = if let Some(source) = &slide.native_source_id {
            let source = original.slides.iter().find(|part| &part.id == source).ok_or_else(|| Error::Invalid("native duplicate source missing".into()))?;
            clone_slide(package, source, &path, &slide.id)?
        } else { insert_slide(package, original, deck, slide, &path, main, scale)? };
        add_type(package, &path, "application/vnd.openxmlformats-officedocument.presentationml.slide+xml")?;
        let relation = loop { let candidate = format!("rIdAislideSlide{counter}"); counter += 1; if used.insert(candidate.clone()) { break candidate; } };
        next_id = next_id.checked_add(1).filter(|id| *id < 2147483648).ok_or_else(|| Error::Limit("native slide IDs exhausted".into()))?;
        extra.push_str(&format!("<Relationship xmlns=\"{REL}\" Id=\"{relation}\" Type=\"{R}/slide\" Target=\"/{path}\"/>"));
        entries.push_str(&format!("<p:sldId xmlns:p=\"{P}\" xmlns:r=\"{R}\" id=\"{next_id}\" r:id=\"{relation}\"/>"));
        bindings.push(binding);
    }
    let mut edits = Vec::new(); insert_child(&relationships, rels.root_element(), &extra, None, &mut edits)?;
    package.replace_part(&relation_path, apply(relationships, edits)?)?;
    package.replace_part(main, apply(xml.clone(), vec![(list.range(), format!("<p:sldIdLst xmlns:p=\"{P}\">{entries}</p:sldIdLst>"))])?)?;
    remove_deleted(package, original, deck, main)?;
    Ok(bindings)
}