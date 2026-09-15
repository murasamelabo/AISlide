use crate::{data_report::SourceBinding, document::Document, native::{REL, relations_path}, native_save::{apply, insert_child}, package::Package, pptx::{empty, parse, relationship_targets}, sources::SourceDocument, Error, Result};
use roxmltree::Node;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use xmlwriter::{Options, XmlWriter};

const NS: &str = "urn:aislide:provenance:1";
const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Identity { pub part: String, pub id: String, pub objects: BTreeMap<String, String> }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Metadata { pub version: u32, pub sources: Vec<SourceDocument>, pub bindings: Vec<SourceBinding>, pub identities: Vec<Identity>, #[serde(default,skip_serializing_if="Vec::is_empty")] pub parts: Vec<crate::parts::state::PartInstance> }

fn write_value(writer: &mut XmlWriter, value: &Value) {
    match value {
        Value::Object(values) => { writer.start_element("m:object"); for (name, value) in values { writer.start_element("m:field"); writer.write_attribute("name", &quick_xml::escape::escape(name)); write_value(writer, value); writer.end_element(); } writer.end_element(); }
        Value::Array(values) => { writer.start_element("m:array"); for value in values { write_value(writer, value); } writer.end_element(); }
        Value::Null => empty(writer, "m:null", &[]),
        value => { writer.start_element(match value { Value::String(_) => "m:string", Value::Bool(_) => "m:boolean", _ => "m:number" }); let text = value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string()); writer.write_text(&quick_xml::escape::escape(&text)); writer.end_element(); }
    }
}

fn read_value(node: Node<'_, '_>, depth: usize) -> Result<Value> {
    if depth > 32 || node.tag_name().namespace() != Some(NS) { return Err(Error::Limit("provenance XML shape/depth".into())); }
    match node.tag_name().name() {
        "object" => {
            let mut values = serde_json::Map::new();
            for field in node.children().filter(|node| node.is_element()) {
                if !field.has_tag_name((NS, "field")) { return Err(Error::Invalid("provenance object field".into())); }
                let name = field.attribute("name").ok_or_else(|| Error::Invalid("provenance field name".into()))?;
                let children: Vec<_> = field.children().filter(|node| node.is_element()).collect();
                if children.len() != 1 || values.contains_key(name) { return Err(Error::Invalid("duplicate or ambiguous provenance field".into())); }
                values.insert(name.into(), read_value(children[0], depth + 1)?);
            }
            Ok(Value::Object(values))
        }
        "array" => Ok(Value::Array(node.children().filter(|node| node.is_element()).map(|node| read_value(node, depth + 1)).collect::<Result<_>>()?)),
        "string" => { if node.children().any(|node| node.is_element()) { return Err(Error::Invalid("nested provenance scalar".into())); } Ok(Value::String(node.text().unwrap_or("").into())) }
        "number" => Ok(Value::Number(node.text().unwrap_or("").parse().map_err(|_| Error::Invalid("provenance number".into()))?)),
        "boolean" => match node.text() { Some("true") => Ok(Value::Bool(true)), Some("false") => Ok(Value::Bool(false)), _ => Err(Error::Invalid("provenance boolean".into())) },
        "null" => Ok(Value::Null),
        _ => Err(Error::Unsupported("provenance XML value".into())),
    }
}

fn encode(metadata: &Metadata) -> Result<Vec<u8>> {
    let mut writer = XmlWriter::new(Options { indent: xmlwriter::Indent::None, ..Options::default() });
    writer.start_element("m:provenance"); writer.write_attribute("xmlns:m", NS); write_value(&mut writer, &serde_json::to_value(metadata)?); writer.end_element();
    let bytes = writer.end_document().into_bytes();
    if bytes.len() > MAX_BYTES { return Err(Error::Limit("provenance XML > 2 MiB".into())); }
    Ok(bytes)
}

pub(crate) fn read(package: &Package) -> Result<Option<(String, Metadata)>> {
    let roots = relationship_targets(package, "", "officeDocument")?;
    let main = roots.values().next().ok_or_else(|| Error::Invalid("presentation missing".into()))?;
    let targets = relationship_targets(package, main, "customXml")?;
    let mut found = None;
    for path in targets.values() {
        let xml = package.text(path)?;
        if xml.len() > MAX_BYTES { return Err(Error::Limit("custom provenance XML > 2 MiB".into())); }
        let document = parse(xml)?;
        if !document.root_element().has_tag_name((NS, "provenance")) { continue; }
        if found.is_some() { return Err(Error::Invalid("multiple AISlide provenance parts".into())); }
        let children: Vec<_> = document.root_element().children().filter(|node| node.is_element()).collect();
        if children.len() != 1 { return Err(Error::Invalid("provenance root must contain one object".into())); }
        let value: Metadata = serde_json::from_value(read_value(children[0], 0)?)?;
        if value.version != 1 { return Err(Error::Unsupported("provenance version".into())); }
        if value.sources.len() > 8 || value.bindings.len() > 4096 || value.identities.len() > 72 || value.identities.iter().any(|entry| entry.objects.len() > 256) { return Err(Error::Limit("provenance resources".into())); }
        found = Some((path.clone(), value));
    }
    Ok(found)
}

pub(crate) fn attach(bytes: Vec<u8>, document: &Document) -> Result<Vec<u8>> {
    let mut package = Package::open(bytes.clone())?;
    let existing = read(&package)?;
    let native = crate::native::read(&package)?;
    let different_ids = native.deck.slides.iter().map(|slide| &slide.id).ne(document.deck.slides.iter().map(|slide| &slide.id))
        || document.deck.design.as_ref().is_some_and(|design| native.masters.iter().map(|master| &master.id).ne(design.masters.iter().map(|master| &master.id)) || native.layouts.iter().map(|layout| &layout.id).ne(design.layouts.iter().map(|layout| &layout.id)));
    if document.sources.is_empty() && document.bindings.is_empty() && document.parts.is_empty() && existing.is_none() && !different_ids { return Ok(bytes); }
    let mut identities = Vec::new();
    for (part, slide) in native.slides.iter().zip(&document.deck.slides) { identities.push(Identity { part: part.path.clone(), id: slide.id.clone(), objects: part.nodes.iter().map(|(id, numeric)| (numeric.clone(), id.clone())).collect() }); }
    if let Some(design) = document.deck.design.as_ref().or(native.deck.design.as_ref()) {
        for (part, master) in native.masters.iter().zip(&design.masters) { identities.push(Identity { part: part.path.clone(), id: master.id.clone(), objects: part.nodes.iter().map(|(id, numeric)| (numeric.clone(), id.clone())).collect() }); }
        for (part, layout) in native.layouts.iter().zip(&design.layouts) { identities.push(Identity { part: part.path.clone(), id: layout.id.clone(), objects: part.nodes.iter().map(|(id, numeric)| (numeric.clone(), id.clone())).collect() }); }
    }
    let mut parts=document.parts.clone();
    for part in &mut parts {
        if !part.stale {
            let slide_index=document.deck.slides.iter().position(|slide|slide.id==part.slide_id).ok_or_else(||Error::Invalid("part slide identity".into()))?;
            let native_slide=&native.deck.slides[slide_index];
            let element=native_slide.elements.iter().find(|element|element.bounds().0==part.element_id).ok_or_else(||Error::Invalid("exported part cannot be reopened".into()))?;
            part.render_sha256=crate::parts::state::render_hash(element)?;
            part.native_sha256=Some(crate::parts::state::native_hash(&package,&native,&native_slide.id,&part.element_id)?);
        } else if let Some(old)=existing.as_ref().and_then(|(_,metadata)|metadata.parts.iter().find(|old|old.slide_id==part.slide_id && old.element_id==part.element_id)) {part.stale=old.stale;}
    }
    let metadata = Metadata { version: 1, sources: document.sources.clone(), bindings: document.bindings.clone(), identities, parts };
    if existing.as_ref().is_some_and(|(_, old)| crate::canonical::bytes(old).ok() == crate::canonical::bytes(&metadata).ok()) { return Ok(bytes); }
    let encoded = encode(&metadata)?;
    if let Some((path, _)) = existing { package.replace_part(&path, encoded)?; return package.save(); }
    let mut index = 1;
    let path = loop { let path = format!("customXml/aislide-provenance{index}.xml"); if !package.part_names().iter().any(|name| name.eq_ignore_ascii_case(&path)) { break path; } index += 1; };
    package.add_part(path.clone(), encoded)?;
    let main = relationship_targets(&package, "", "officeDocument")?.into_values().next().ok_or_else(|| Error::Invalid("presentation missing".into()))?;
    let relation_path = relations_path(&main); let xml = package.text(&relation_path)?.to_owned(); let parsed = parse(&xml)?;
    let mut counter = 1;
    let id = loop { let candidate = format!("rIdAislideProvenance{counter}"); if !parsed.root_element().children().any(|node| node.attribute("Id") == Some(&candidate)) { break candidate; } counter += 1; };
    let relation = format!("<Relationship xmlns=\"{REL}\" Id=\"{id}\" Type=\"{}/customXml\" Target=\"/{path}\"/>", crate::native::R);
    let mut edits = Vec::new(); insert_child(&xml, parsed.root_element(), &relation, None, &mut edits)?; package.replace_part(&relation_path, apply(xml, edits)?)?;
    let xml = package.text("[Content_Types].xml")?.to_owned(); let parsed = parse(&xml)?;
    let entry = format!("<Override xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\" PartName=\"/{path}\" ContentType=\"application/xml\"/>");
    let mut edits = Vec::new(); insert_child(&xml, parsed.root_element(), &entry, None, &mut edits)?; package.replace_part("[Content_Types].xml", apply(xml, edits)?)?;
    package.save()
}