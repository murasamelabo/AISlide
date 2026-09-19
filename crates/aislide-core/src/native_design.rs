use crate::{design::SlideLayout, model::Deck, native::{NativeDeck, NativePart, P, R, REL, child, relations_path}, native_save::{apply, insert_child}, package::Package, pptx::{parse, relationship_targets, resolve}, Error, Result};
use std::collections::{BTreeMap, BTreeSet};

const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const NS: &str = "urn:aislide:design:1";

pub(crate) struct Topology { pub masters: Vec<NativePart>, pub layouts: Vec<NativePart> }

pub(crate) struct Identities { pub masters: Vec<(String, String)>, pub layouts: Vec<(String, String)> }

impl Identities {
    pub(crate) fn identity(&self, path: &str) -> Option<&str> {
        self.masters.iter().chain(&self.layouts).find(|(_, part)| part == path).map(|(id, _)| id.as_str())
    }

    pub(crate) fn order(&self, masters: &[NativePart], layouts: &mut Vec<SlideLayout>, parts: &mut Vec<NativePart>) -> Result<()> {
        if self.masters.iter().map(|(id, path)| (id, path)).ne(masters.iter().map(|part| (&part.id, &part.path)))
            || self.layouts.len() != parts.len() || self.layouts.iter().any(|(id, path)| !parts.iter().any(|part| &part.id == id && &part.path == path)) {
            return Err(Error::Conflict("native design metadata no longer matches the declared topology".into()));
        }
        let positions: BTreeMap<_, _> = self.layouts.iter().enumerate().map(|(index, (id, _))| (id.as_str(), index)).collect();
        layouts.sort_by_key(|layout| positions[layout.id.as_str()]);
        parts.sort_by_key(|part| positions[part.id.as_str()]);
        Ok(())
    }
}

pub(crate) fn read_identities(package: &Package, main: &str) -> Result<Option<Identities>> {
    let parsed = parse(package.text(main)?)?;
    let entries: Vec<_> = child(parsed.root_element(), P, "extLst").into_iter().flat_map(|list| list.children())
        .filter(|node| node.has_tag_name((P, "ext")) && node.attribute("uri") == Some(NS)).collect();
    if entries.len() > 1 { return Err(Error::Invalid("duplicate native design metadata".into())); }
    let Some(entry) = entries.first() else { return Ok(None) };
    let root = child(*entry, NS, "design").ok_or_else(|| Error::Invalid("native design metadata missing".into()))?;
    let mut result = Identities { masters: vec![], layouts: vec![] };
    let mut paths = BTreeSet::new();
    for node in root.children().filter(|node| node.is_element()) {
        let entries = if node.has_tag_name((NS, "master")) { &mut result.masters } else if node.has_tag_name((NS, "layout")) { &mut result.layouts } else { return Err(Error::Unsupported("native design metadata element".into())); };
        let id = node.attribute("id").ok_or_else(|| Error::Invalid("native design identity missing".into()))?;
        let path = node.attribute("part").ok_or_else(|| Error::Invalid("native design part missing".into()))?;
        crate::model::valid_text(id, 80)?;
        if id.is_empty() || entries.iter().any(|(other, _)| other == id) || !paths.insert(path.to_owned()) || resolve("", path)? != path || !package.part_names().contains(path) { return Err(Error::Invalid("invalid native design identity or part".into())); }
        entries.push((id.into(), path.into()));
    }
    if result.masters.is_empty() || result.masters.len() > 8 || result.layouts.is_empty() || result.layouts.len() > 32 { return Err(Error::Limit("native design metadata limits".into())); }
    Ok(Some(result))
}

fn write_identities(package: &mut Package, main: &str, topology: &Topology) -> Result<()> {
    let mut body = String::new();
    for (tag, parts) in [("master", &topology.masters), ("layout", &topology.layouts)] {
        for part in parts { body.push_str(&format!("<d:{tag} id=\"{}\" part=\"{}\"/>", quick_xml::escape::escape(&part.id), quick_xml::escape::escape(&part.path))); }
    }
    let fragment = format!("<p:ext xmlns:p=\"{P}\" uri=\"{NS}\"><d:design xmlns:d=\"{NS}\">{body}</d:design></p:ext>");
    let xml = package.text(main)?.to_owned(); let parsed = parse(&xml)?; let mut edits = Vec::new();
    if let Some(list) = child(parsed.root_element(), P, "extLst") {
        if let Some(old) = list.children().find(|node| node.has_tag_name((P, "ext")) && node.attribute("uri") == Some(NS)) { edits.push((old.range(), fragment)); }
        else { insert_child(&xml, list, &fragment, None, &mut edits)?; }
    } else { insert_child(&xml, parsed.root_element(), &format!("<p:extLst xmlns:p=\"{P}\">{fragment}</p:extLst>"), None, &mut edits)?; }
    package.replace_part(main, apply(xml, edits)?)
}

fn unused(package: &Package, prefix: &str) -> Result<String> {
    for number in 1..=8192 {
        let path = format!("{prefix}{number}.xml");
        if !package.part_names().iter().any(|existing| existing.eq_ignore_ascii_case(&path) || existing.eq_ignore_ascii_case(&relations_path(&path))) { return Ok(path); }
    }
    Err(Error::Limit("native design part names exhausted".into()))
}

fn add_type(package: &mut Package, path: &str, kind: &str) -> Result<()> {
    let xml = package.text("[Content_Types].xml")?.to_owned(); let parsed = parse(&xml)?; let mut edits = Vec::new();
    let content_type = if kind == "theme" { "application/vnd.openxmlformats-officedocument.theme+xml".into() } else { format!("application/vnd.openxmlformats-officedocument.presentationml.{kind}+xml") };
    let fragment = format!("<Override xmlns=\"{CT}\" PartName=\"/{path}\" ContentType=\"{content_type}\"/>");
    insert_child(&xml, parsed.root_element(), &fragment, None, &mut edits)?;
    package.replace_part("[Content_Types].xml", apply(xml, edits)?)
}

fn relation(package: &mut Package, source: &str, kind: &str, target: &str) -> Result<String> {
    if let Some((id, _)) = relationship_targets(package, source, kind)?.into_iter().find(|(_, path)| path == target) { return Ok(id); }
    let path = relations_path(source);
    let xml = if package.part_names().contains(path.as_str()) { package.text(&path)?.to_owned() } else { format!("<Relationships xmlns=\"{REL}\"/>") };
    let parsed = parse(&xml)?;
    let used: BTreeSet<_> = parsed.root_element().children().filter_map(|node| node.attribute("Id")).collect();
    let id = (1..=8192).map(|number| format!("rIdAislideDesign{number}")).find(|id| !used.contains(id.as_str())).ok_or_else(|| Error::Limit("native relationship IDs exhausted".into()))?;
    let mut edits = Vec::new();
    insert_child(&xml, parsed.root_element(), &format!("<Relationship xmlns=\"{REL}\" Id=\"{id}\" Type=\"{R}/{kind}\" Target=\"/{}\"/>", quick_xml::escape::escape(target)), None, &mut edits)?;
    let bytes = apply(xml, edits)?;
    if package.part_names().contains(path.as_str()) { package.replace_part(&path, bytes)?; } else { package.add_part(path, bytes)?; }
    Ok(id)
}

fn retain_relations(package: &mut Package, source: &str, kind: &str, keep: &BTreeSet<String>) -> Result<()> {
    let path = relations_path(source); let xml = package.text(&path)?.to_owned(); let parsed = parse(&xml)?;
    let owner = parse(package.text(source)?)?;
    let mut edits = Vec::new();
    for entry in parsed.root_element().children().filter(|node| node.has_tag_name((REL, "Relationship")) && node.attribute("Type") == Some(format!("{R}/{kind}").as_str())) {
        let id = entry.attribute("Id").ok_or_else(|| Error::Invalid("design relationship ID missing".into()))?;
        if keep.contains(id) { continue; }
        if owner.descendants().any(|node| node.attributes().any(|attribute| attribute.namespace() == Some(R) && attribute.value() == id)) { return Err(Error::Unsupported("retained XML references a removed design relationship".into())); }
        edits.push((entry.range(), String::new()));
    }
    if !edits.is_empty() { package.replace_part(&path, apply(xml, edits)?)?; }
    Ok(())
}

fn replace_list(package: &mut Package, path: &str, name: &str, entries: &str) -> Result<()> {
    let xml = package.text(path)?.to_owned(); let parsed = parse(&xml)?;
    let list = child(parsed.root_element(), P, name).ok_or_else(|| Error::Unsupported(format!("native {name} missing")))?;
    let mut edits = Vec::new();
    let mut first = true;
    for entry in list.children().filter(|node| node.has_tag_name((P, if name == "sldMasterIdLst" { "sldMasterId" } else { "sldLayoutId" }))) {
        edits.push((entry.range(), if first { entries.to_owned() } else { String::new() }));
        first = false;
    }
    if first { insert_child(&xml, list, entries, child(list, P, "extLst"), &mut edits)?; }
    package.replace_part(path, apply(xml, edits)?)
}

pub(crate) fn isolated_row(xml: &str, node: roxmltree::Node<'_, '_>) -> Result<String> {
    let mut row = xml[node.range()].to_owned();
    let mut reader = quick_xml::Reader::from_str(&row);
    let declared = match reader.read_event().map_err(|_| Error::Invalid("native topology row XML".into()))? {
        quick_xml::events::Event::Start(tag) | quick_xml::events::Event::Empty(tag) => tag.attributes().map(|attribute| attribute.map(|attribute| String::from_utf8_lossy(attribute.key.as_ref()).into_owned()).map_err(|_| Error::Invalid("native topology row attribute".into()))).collect::<Result<BTreeSet<_>>>()?,
        _ => return Err(Error::Invalid("native topology row missing".into())),
    };
    let mut namespaces = String::new();
    for namespace in node.namespaces() {
        let name = namespace.name().map(|prefix| format!("xmlns:{prefix}")).unwrap_or_else(|| "xmlns".into());
        if !declared.contains(&name) { namespaces.push_str(&format!(" {name}=\"{}\"", quick_xml::escape::escape(namespace.uri()))); }
    }
    let position = row[1..].find([' ', '\t', '\r', '\n', '/', '>']).ok_or_else(|| Error::Invalid("native row start tag".into()))? + 1;
    row.insert_str(position, &namespaces);
    Ok(row)
}

fn row(previous: Option<&String>, name: &str, numeric: u32, relation: &str) -> Result<String> {
    if let Some(xml) = previous {
        let parsed = parse(xml)?;
        let attribute = parsed.root_element().attributes().find(|attribute| attribute.namespace() == Some(R) && attribute.name() == "id").ok_or_else(|| Error::Invalid("native topology row relationship".into()))?;
        return String::from_utf8(apply(xml.clone(), vec![(attribute.range_value(), relation.into())])?).map_err(|_| Error::Invalid("native topology encoding".into()));
    }
    Ok(format!("<p:{name} xmlns:p=\"{P}\" xmlns:r=\"{R}\" id=\"{numeric}\" r:id=\"{relation}\"/>"))
}

fn binding(original: &[NativePart], id: &str, path: String) -> NativePart {
    original.iter().find(|part| part.id == id).map(|part| NativePart { id: part.id.clone(), path: part.path.clone(), nodes: part.nodes.clone() })
        .unwrap_or_else(|| NativePart { id: id.into(), path, nodes: BTreeMap::new() })
}

pub(crate) fn prepare(package: &mut Package, original: &NativeDeck, deck: &Deck, main: &str) -> Result<Topology> {
    let old = original.deck.design.as_ref().ok_or_else(|| Error::Invalid("native design missing".into()))?;
    let design = deck.design.as_ref().ok_or_else(|| Error::Invalid("native design missing".into()))?;
    let changed = old.masters.iter().map(|master| &master.id).ne(design.masters.iter().map(|master| &master.id))
        || old.layouts.iter().map(|layout| (&layout.id, &layout.master_id)).ne(design.layouts.iter().map(|layout| (&layout.id, &layout.master_id)));
    if !changed { return Ok(Topology { masters: original.masters.iter().map(|part| binding(&original.masters, &part.id, String::new())).collect(), layouts: original.layouts.iter().map(|part| binding(&original.layouts, &part.id, String::new())).collect() }); }
    let mut empty = deck.clone();
    empty.slides.truncate(1); empty.slides[0].elements.clear(); empty.slides[0].layout_id = None;
    let empty_design = empty.design.as_mut().ok_or_else(|| Error::Invalid("native design missing".into()))?;
    for master in &mut empty_design.masters { master.elements.clear(); }
    for layout in &mut empty_design.layouts { layout.elements.clear(); }
    let generated = Package::open(crate::pptx::export_pptx(&empty)?)?;
    let mut topology = Topology { masters: Vec::new(), layouts: Vec::new() };
    let mut numeric = BTreeMap::new(); let mut used = BTreeSet::new(); let mut rows = BTreeMap::new();
    for (source, name, kind) in std::iter::once((main, "sldMasterId", "slideMaster")).chain(original.masters.iter().map(|part| (part.path.as_str(), "sldLayoutId", "slideLayout"))) {
        let parsed = parse(package.text(source)?)?; let targets = relationship_targets(package, source, kind)?;
        for node in child(parsed.root_element(), P, if kind == "slideMaster" { "sldMasterIdLst" } else { "sldLayoutIdLst" }).into_iter().flat_map(|list| list.children()).filter(|node| node.has_tag_name((P, name))) {
            let number = node.attribute("id").and_then(|id| id.parse::<u32>().ok()).filter(|id| *id >= 2147483648).ok_or_else(|| Error::Invalid("native design numeric ID".into()))?;
            let path = node.attribute((R, "id")).and_then(|id| targets.get(id)).ok_or_else(|| Error::Invalid("native design relationship missing".into()))?;
            if !used.insert(number) || numeric.insert(path.clone(), number).is_some() { return Err(Error::Invalid("colliding native design numeric IDs or duplicate parts".into())); }
            rows.insert(path.clone(), isolated_row(package.text(source)?, node)?);
        }
    }
    for (index, master) in design.masters.iter().enumerate() {
        if let Some(part) = original.masters.iter().find(|part| part.id == master.id) { topology.masters.push(binding(&original.masters, &master.id, part.path.clone())); continue; }
        let path = unused(package, "ppt/slideMasters/aislide-master-")?;
        package.add_part(path.clone(), generated.part(&format!("ppt/slideMasters/slideMaster{}.xml", index + 1))?.to_vec())?;
        package.add_part(relations_path(&path), format!("<Relationships xmlns=\"{REL}\"/>").into_bytes())?;
        add_type(package, &path, "slideMaster")?;
        let theme = unused(package, "ppt/theme/aislide-theme-")?;
        let theme_number = if index == 0 { 1 } else { index + 2 };
        package.add_part(theme.clone(), generated.part(&format!("ppt/theme/theme{theme_number}.xml"))?.to_vec())?; add_type(package, &theme, "theme")?;
        relation(package, &path, "theme", &theme)?;
        topology.masters.push(binding(&[], &master.id, path));
    }
    for (index, layout) in design.layouts.iter().enumerate() {
        if let Some(part) = original.layouts.iter().find(|part| part.id == layout.id) { topology.layouts.push(binding(&original.layouts, &layout.id, part.path.clone())); continue; }
        let path = unused(package, "ppt/slideLayouts/aislide-layout-")?;
        package.add_part(path.clone(), generated.part(&format!("ppt/slideLayouts/slideLayout{}.xml", index + 1))?.to_vec())?;
        package.add_part(relations_path(&path), format!("<Relationships xmlns=\"{REL}\"/>").into_bytes())?;
        add_type(package, &path, "slideLayout")?; topology.layouts.push(binding(&[], &layout.id, path));
    }
    let mut next = 2147483648u32;
    for part in topology.masters.iter().chain(&topology.layouts) {
        if numeric.contains_key(&part.path) { continue; }
        while used.contains(&next) { next = next.checked_add(1).ok_or_else(|| Error::Limit("native design numeric IDs exhausted".into()))?; }
        numeric.insert(part.path.clone(), next); used.insert(next);
    }
    let mut entries = String::new(); let mut keep = BTreeSet::new();
    for part in &topology.masters {
        let id = relation(package, main, "slideMaster", &part.path)?; keep.insert(id.clone());
        entries.push_str(&row(rows.get(&part.path), "sldMasterId", numeric[&part.path], &id)?);
    }
    replace_list(package, main, "sldMasterIdLst", &entries)?; retain_relations(package, main, "slideMaster", &keep)?;
    for master in &topology.masters {
        let mut entries = String::new(); let mut keep = BTreeSet::new();
        for (layout, part) in design.layouts.iter().zip(&topology.layouts).filter(|(layout, _)| layout.master_id == master.id) {
            let id = relation(package, &master.path, "slideLayout", &part.path)?; keep.insert(id.clone());
            entries.push_str(&row(rows.get(&part.path), "sldLayoutId", numeric[&part.path], &id)?);
            let before = old.layouts.iter().find(|before| before.id == layout.id);
            if before.is_none_or(|before| before.master_id != layout.master_id) {
                let id = relation(package, &part.path, "slideMaster", &master.path)?;
                retain_relations(package, &part.path, "slideMaster", &BTreeSet::from([id]))?;
            }
        }
        let before: Vec<_> = old.layouts.iter().filter(|layout| layout.master_id == master.id).map(|layout| &layout.id).collect();
        let after: Vec<_> = design.layouts.iter().filter(|layout| layout.master_id == master.id).map(|layout| &layout.id).collect();
        if before != after || !original.masters.iter().any(|part| part.id == master.id) { replace_list(package, &master.path, "sldLayoutIdLst", &entries)?; }
        retain_relations(package, &master.path, "slideLayout", &keep)?;
    }
    write_identities(package, main, &topology)?;
    Ok(topology)
}

pub(crate) fn remove_deleted(package: &mut Package, original: &NativeDeck, topology: &Topology) -> Result<()> {
    let mut removed = BTreeSet::new();
    for (before, after) in [(&original.masters, &topology.masters), (&original.layouts, &topology.layouts)] {
        for part in before.iter().filter(|part| !after.iter().any(|entry| entry.id == part.id)) { removed.insert(part.path.clone()); removed.insert(relations_path(&part.path)); }
    }
    if removed.is_empty() { return Ok(()); }
    for path in package.part_names().into_iter().filter(|path| path.ends_with(".rels") && !removed.contains(*path)) {
        let source = if path == "_rels/.rels" { String::new() } else if let Some((folder, name)) = path.rsplit_once("/_rels/") { format!("{folder}/{}", name.trim_end_matches(".rels")) } else if let Some(name) = path.strip_prefix("_rels/") { name.trim_end_matches(".rels").into() } else { return Err(Error::Unsupported("relationship part path".into())); };
        let parsed = parse(package.text(path)?)?;
        for node in parsed.root_element().children().filter(|node| node.has_tag_name((REL, "Relationship")) && node.attribute("TargetMode") != Some("External")) {
            if removed.contains(&resolve(&source, node.attribute("Target").ok_or_else(|| Error::Invalid("relationship target".into()))?)?) { return Err(Error::Unsupported("a retained resource references the deleted master/layout".into())); }
        }
    }
    let xml = package.text("[Content_Types].xml")?.to_owned(); let parsed = parse(&xml)?;
    let edits = parsed.root_element().children().filter(|node| node.has_tag_name((CT, "Override")) && node.attribute("PartName").is_some_and(|path| removed.contains(path.trim_start_matches('/')))).map(|node| (node.range(), String::new())).collect();
    package.replace_part("[Content_Types].xml", apply(xml, edits)?)?;
    for path in removed { if package.part_names().contains(path.as_str()) { package.remove_part(&path)?; } }
    Ok(())
}

pub(crate) fn patch_themes(package: &mut Package, design: &crate::design::Design, masters: &[NativePart]) -> Result<()> {
    for master in masters {
        patch_owner_theme(package, &master.path, crate::design::master_theme(design, &master.id))?;
    }
    Ok(())
}

fn patch_owner_theme(package: &mut Package, path: &str, after: &crate::design::Theme) -> Result<()> {
        let targets = relationship_targets(package, path, "theme")?;
        if targets.len() != 1 { return Err(Error::Unsupported("master requires exactly one native theme".into())); }
        let (id, source) = targets.iter().next().ok_or_else(|| Error::Invalid("master theme missing".into()))?;
        let before = crate::native::read_theme(package, source)?;
        if crate::canonical::bytes(&before)? == crate::canonical::bytes(after)? { return Ok(()); }
        let owner = relations_path(path);
        let mut shared = false;
        for path in package.part_names().into_iter().filter(|path| path.ends_with(".rels")) {
            let base = if path == "_rels/.rels" { String::new() } else if let Some((folder, name)) = path.rsplit_once("/_rels/") { format!("{folder}/{}", name.trim_end_matches(".rels")) } else if let Some(name) = path.strip_prefix("_rels/") { name.trim_end_matches(".rels").into() } else { return Err(Error::Unsupported("relationship part path".into())); };
            let parsed = parse(package.text(path)?)?;
            for node in parsed.root_element().children().filter(|node| node.has_tag_name((REL, "Relationship")) && node.attribute("TargetMode") != Some("External")) {
                if path == owner && node.attribute("Id") == Some(id) { continue; }
                if resolve(&base, node.attribute("Target").ok_or_else(|| Error::Invalid("relationship target missing".into()))?)? == *source { shared = true; }
            }
        }
        let target = if shared {
            let directory = source.rsplit_once('/').map(|(directory, _)| format!("{directory}/")).unwrap_or_default();
            let target = unused(package, &format!("{directory}aislide-theme-"))?;
            package.add_part(target.clone(), package.part(source)?.to_vec())?;
            let source_rels = relations_path(source);
            if package.part_names().contains(source_rels.as_str()) { package.add_part(relations_path(&target), package.part(&source_rels)?.to_vec())?; }
            add_type(package, &target, "theme")?;
            let xml = package.text(&owner)?.to_owned(); let parsed = parse(&xml)?;
            let node = parsed.root_element().children().find(|node| node.attribute("Id") == Some(id)).ok_or_else(|| Error::Invalid("theme relationship missing".into()))?;
            let attribute = node.attributes().find(|attribute| attribute.name() == "Target").ok_or_else(|| Error::Invalid("theme relationship target missing".into()))?;
            package.replace_part(&owner, apply(xml.clone(), vec![(attribute.range_value(), format!("/{target}"))])?)?;
            target
        } else { source.clone() };
        crate::native_save::patch_theme(package, &target, &before, after)?;
    Ok(())
}

pub(crate) fn read_auxiliary(package: &Package, main: &str, warnings: &mut Vec<String>) -> Result<Option<crate::model::AuxiliaryDesign>> {
    let parsed = parse(package.text(main)?)?;
    let Some(size) = child(parsed.root_element(), P, "notesSz") else { return Ok(None); };
    let dimension = |name| size.attribute(name).and_then(|value| value.parse::<f64>().ok()).map(|value| (value / 9525.0).round() as u32).ok_or_else(|| Error::Invalid("notes page dimensions missing".into()));
    let (width, height) = (dimension("cx")?, dimension("cy")?);
    crate::canvas::validate_size(width, height)?;
    let scale = (f64::from(width) / size.attribute("cx").and_then(|value| value.parse::<f64>().ok()).unwrap_or(0.0), f64::from(height) / size.attribute("cy").and_then(|value| value.parse::<f64>().ok()).unwrap_or(0.0));
    let mut design = crate::model::AuxiliaryDesign { width, height, notes_master: None, handout_master: None };
    for (kind, target) in [("notesMaster", &mut design.notes_master), ("handoutMaster", &mut design.handout_master)] {
        let targets = relationship_targets(package, main, kind)?;
        if targets.len() > 1 { warnings.push(format!("Multiple {kind} parts retained; auxiliary editing unavailable")); continue; }
        if let Some(path) = targets.values().next() {
            let (name, background, elements, _) = crate::native::read_part(package, path, kind.into(), scale, &[], warnings, None, (width, height))?;
            let themes = relationship_targets(package, path, "theme")?;
            if themes.len() != 1 { warnings.push(format!("{kind} theme unavailable; part retained")); continue; }
            let theme = crate::native::read_theme(package, themes.values().next().unwrap())?;
            *target = Some(crate::model::AuxiliaryMaster { name, background: background.unwrap_or_else(|| "@lt1".into()), elements, theme });
        }
    }
    Ok(Some(design))
}

pub(crate) fn patch_auxiliary(package: &mut Package, original: &NativeDeck, deck: &Deck, main: &str) -> Result<()> {
    if crate::canonical::bytes(&original.deck.auxiliary_design)? == crate::canonical::bytes(&deck.auxiliary_design)? { return Ok(()); }
    crate::review::ensure_unprotected(package)?;
    let Some(after) = &deck.auxiliary_design else { return Err(Error::Unsupported("auxiliary master removal is not supported".into())); };
    if original.deck.auxiliary_design.as_ref().is_some_and(|before| (before.width, before.height) != (after.width, after.height)) { return Err(Error::Unsupported("resizing existing native notes pages is not supported".into())); }
    let parsed = parse(package.text(main)?)?;
    let size = child(parsed.root_element(), P, "notesSz");
    let scale = (size.and_then(|node| node.attribute("cx")).and_then(|value| value.parse::<f64>().ok()).map(|value| value / f64::from(after.width)).unwrap_or(9525.0), size.and_then(|node| node.attribute("cy")).and_then(|value| value.parse::<f64>().ok()).map(|value| value / f64::from(after.height)).unwrap_or(9525.0));
    for (kind, previous, next) in [
        ("notesMaster", original.deck.auxiliary_design.as_ref().and_then(|design| design.notes_master.as_ref()), after.notes_master.as_ref()),
        ("handoutMaster", original.deck.auxiliary_design.as_ref().and_then(|design| design.handout_master.as_ref()), after.handout_master.as_ref()),
    ] {
        if crate::canonical::bytes(&previous)? == crate::canonical::bytes(&next)? { continue; }
        let Some(next) = next else { return Err(Error::Unsupported("auxiliary master removal is not supported".into())); };
        let targets = relationship_targets(package, main, kind)?;
        let binding = if let Some(previous) = previous {
            if targets.len() != 1 { return Err(Error::Unsupported("ambiguous auxiliary master".into())); }
            let path = targets.values().next().unwrap();
            let (_, _, _, binding) = crate::native::read_part(package, path, kind.into(), (1.0 / scale.0, 1.0 / scale.1), &[], &mut Vec::new(), None, (after.width, after.height))?;
            crate::native_save::patch_part(package, &binding, &previous.elements, &next.elements, &next.theme, (previous.name != next.name).then_some(next.name.as_str()), (previous.background != next.background).then_some(Some(next.background.as_str())), scale)?;
            patch_owner_theme(package, path, &next.theme)?;
            binding
        } else {
            if !targets.is_empty() { return Err(Error::Unsupported("unmodeled auxiliary master is preserved".into())); }
            let path = unused(package, &format!("ppt/{kind}s/aislide-{kind}-"))?;
            let theme = unused(package, "ppt/theme/aislide-auxiliary-theme-")?;
            let mut empty = next.clone(); empty.elements.clear();
            package.add_part(path.clone(), crate::pptx::auxiliary_master_xml(kind, &empty))?; add_type(package, &path, kind)?;
            package.add_part(relations_path(&path), format!("<Relationships xmlns=\"{REL}\"/>").into_bytes())?;
            package.add_part(theme.clone(), crate::pptx::theme(&next.theme))?; add_type(package, &theme, "theme")?;
            relation(package, &path, "theme", &theme)?;
            let id = relation(package, main, kind, &path)?;
            let xml = package.text(main)?.to_owned(); let parsed = parse(&xml)?; let root = parsed.root_element(); let mut edits = Vec::new();
            let list = format!("{kind}IdLst");
            if child(root, P, &list).is_some() { return Err(Error::Unsupported("unresolved auxiliary master list is preserved".into())); }
            let before = root.children().find(|node| node.is_element() && !["sldMasterIdLst", "notesMasterIdLst"].contains(&node.tag_name().name()));
            insert_child(&xml, root, &format!("<p:{list} xmlns:p=\"{P}\" xmlns:r=\"{R}\"><p:{kind}Id r:id=\"{id}\"/></p:{list}>"), before, &mut edits)?;
            package.replace_part(main, apply(xml, edits)?)?;
            let binding = NativePart { path, id: kind.into(), nodes: BTreeMap::new() };
            crate::native_save::patch_part(package, &binding, &[], &next.elements, &next.theme, None, None, scale)?;
            binding
        };
        crate::fields::write_part(package, &binding.path, &next.elements, Some(&binding))?;
    }
    let xml = package.text(main)?.to_owned(); let parsed = parse(&xml)?; let root = parsed.root_element(); let mut edits = Vec::new();
    if child(root, P, "notesSz").is_none() { insert_child(&xml, root, &format!("<p:notesSz xmlns:p=\"{P}\" cx=\"{}\" cy=\"{}\"/>", u64::from(after.width) * 9525, u64::from(after.height) * 9525), root.children().find(|node| node.has_tag_name((P, "extLst"))), &mut edits)?; }
    if !edits.is_empty() { package.replace_part(main, apply(xml, edits)?)?; }
    Ok(())
}