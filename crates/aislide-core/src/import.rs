use crate::{model::{ChartKind, ChartSeries, Connection, Crop, Deck, Element, Slide, validate_deck}, package::Package, pptx::{parse, relationship_targets, slide_paths}, Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use roxmltree::Node;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::ops::Range;

const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";

#[derive(Serialize)]
pub struct ImportedDeck { pub deck: Deck, pub source_sha256: String, pub warnings: Vec<String>, pub objects: Vec<ImportedObject> }
#[derive(Serialize)]
pub struct ImportedObject { pub slide_id: String, pub element_id: String, pub part: String, pub editable_fields: Vec<String> }

fn child<'a>(node: Node<'a, '_>, namespace: &str, tag: &str) -> Option<Node<'a, 'a>> { node.children().find(|child| child.has_tag_name((namespace, tag))) }
fn numeric(node: Node<'_, '_>, name: &str, default: f64) -> f64 { node.attribute(name).and_then(|value| value.parse().ok()).unwrap_or(default) }
fn properties<'a>(node: Node<'a, '_>) -> Option<Node<'a, 'a>> { node.children().find(|child| child.tag_name().namespace() == Some(P) && ["nvSpPr", "nvPicPr", "nvGraphicFramePr", "nvCxnSpPr", "nvGrpSpPr"].contains(&child.tag_name().name())).and_then(|nonvisual| child(nonvisual, P, "cNvPr")) }
fn identity(node: Node<'_, '_>) -> Result<String> { Ok(format!("shape-{}", properties(node).and_then(|value| value.attribute("id")).ok_or_else(|| Error::Unsupported("shape has no stable ID".into()))?)) }
fn shape_properties<'a>(node: Node<'a, '_>) -> Option<Node<'a, 'a>> { child(node, P, if node.has_tag_name((P, "grpSp")) { "grpSpPr" } else { "spPr" }) }
fn transform<'a>(node: Node<'a, '_>) -> Option<Node<'a, 'a>> { child(node, P, "xfrm").or_else(|| shape_properties(node).and_then(|properties| child(properties, A, "xfrm"))) }
fn bounds(node: Node<'_, '_>, scale: (f64, f64)) -> Result<[f64; 4]> {
    let transform = transform(node).ok_or_else(|| Error::Unsupported("inherited or missing geometry".into()))?;
    if numeric(transform, "rot", 0.0) != 0.0 || transform.attribute("flipH") == Some("1") || (transform.attribute("flipV") == Some("1") && !node.has_tag_name((P, "cxnSp"))) { return Err(Error::Unsupported("rotated or flipped shape geometry".into())); }
    let offset = child(transform, A, "off").ok_or_else(|| Error::Unsupported("missing shape offset".into()))?;
    let size = child(transform, A, "ext").ok_or_else(|| Error::Unsupported("missing shape size".into()))?;
    Ok([numeric(offset, "x", 0.0) * scale.0, numeric(offset, "y", 0.0) * scale.1, (numeric(size, "cx", 0.0) * scale.0).max(0.01), (numeric(size, "cy", 0.0) * scale.1).max(0.01)])
}
fn solid(node: Node<'_, '_>, default: &str) -> String { child(node, A, "solidFill").and_then(|fill| child(fill, A, "srgbClr")).and_then(|color| color.attribute("val")).unwrap_or(default).into() }
fn paragraph_text(node: Node<'_, '_>) -> String {
    node.children().filter(|node| node.has_tag_name((A, "p"))).map(|paragraph| {
        paragraph.children().filter_map(|part| {
            if part.has_tag_name((A, "br")) { Some("\n".into()) }
            else if part.has_tag_name((A, "r")) || part.has_tag_name((A, "fld")) { Some(part.children().filter(|text| text.has_tag_name((A, "t"))).filter_map(|text| text.text()).collect::<String>()) }
            else { None }
        }).collect::<String>()
    }).collect::<Vec<_>>().join("\n")
}
fn run_properties<'a>(body: Node<'a, '_>) -> Option<Node<'a, 'a>> { body.descendants().find(|node| node.has_tag_name((A, "rPr"))) }
fn string_cache(node: Node<'_, '_>) -> Vec<String> { node.descendants().filter(|value| value.has_tag_name((C, "pt"))).filter_map(|point| child(point, C, "v").and_then(|value| value.text()).map(String::from)).collect() }

fn read_chart(package: &Package, part: &str, node: Node<'_, '_>, common: (&str, [f64; 4])) -> Result<Element> {
    let reference = node.descendants().find(|node| node.has_tag_name((C, "chart"))).and_then(|node| node.attribute((R, "id"))).ok_or_else(|| Error::Unsupported("chart relationship missing".into()))?;
    let targets = relationship_targets(package, part, "chart")?;
    let path = targets.get(reference).ok_or_else(|| Error::Unsupported("chart part missing".into()))?;
    let document = parse(package.text(path)?)?;
    let chart = document.descendants().find(|node| node.has_tag_name((C, "barChart")) || node.has_tag_name((C, "lineChart"))).ok_or_else(|| Error::Unsupported("only native bar/column/line charts are previewed".into()))?;
    let kind = if chart.has_tag_name((C, "lineChart")) { ChartKind::Line } else if child(chart, C, "barDir").and_then(|node| node.attribute("val")) == Some("bar") { ChartKind::Bar } else { ChartKind::Column };
    let mut series = Vec::new(); let mut categories = Vec::new();
    for entry in chart.children().filter(|node| node.has_tag_name((C, "ser"))) {
        let category_node = child(entry, C, "cat").ok_or_else(|| Error::Unsupported("chart category cache missing".into()))?;
        let current_categories = string_cache(category_node);
        if categories.is_empty() { categories = current_categories; } else if categories != current_categories { return Err(Error::Unsupported("chart category caches differ".into())); }
        let values_node = child(entry, C, "val").ok_or_else(|| Error::Unsupported("chart value cache missing".into()))?;
        let values = string_cache(values_node).into_iter().map(|value| value.parse::<f64>().map_err(|_| Error::Unsupported("nonnumeric chart cache".into()))).collect::<Result<Vec<_>>>()?;
        let name = child(entry, C, "tx").map(|node| { let cached = string_cache(node); cached.first().cloned().or_else(|| child(node, C, "v").and_then(|value| value.text()).map(String::from)).unwrap_or_else(|| "Series".into()) }).unwrap_or_else(|| "Series".into());
        let color = child(entry, C, "spPr").map(|node| child(node, A, "ln").map_or_else(|| solid(node, "087F73"), |line| solid(line, "087F73"))).unwrap_or_else(|| "087F73".into());
        series.push(ChartSeries { name, values, color });
    }
    let (id, [x, y, width, height]) = common;
    Ok(Element::Chart { id: id.into(), x, y, width, height, kind, categories, series })
}

fn read_element(package: &Package, part: &str, node: Node<'_, '_>, scale: (f64, f64)) -> Result<Element> {
    let id = identity(node)?;
    let [x, y, width, height] = bounds(node, scale)?;
    if node.has_tag_name((P, "grpSp")) {
        let transform = transform(node).ok_or_else(|| Error::Unsupported("group transform".into()))?;
        let offset = child(transform, A, "chOff").ok_or_else(|| Error::Unsupported("group child offset".into()))?;
        if numeric(offset, "x", 0.0) != 0.0 || numeric(offset, "y", 0.0) != 0.0 { return Err(Error::Unsupported("nonzero group coordinate origin".into())); }
        let extent = child(transform, A, "chExt").ok_or_else(|| Error::Unsupported("group child extent".into()))?;
        let view_width = numeric(extent, "cx", 0.0) / 9525.0; let view_height = numeric(extent, "cy", 0.0) / 9525.0;
        let children = node.children().filter(|node| node.is_element() && ["sp", "pic", "grpSp", "cxnSp", "graphicFrame"].contains(&node.tag_name().name())).map(|child| read_element(package, part, child, (1.0 / 9525.0, 1.0 / 9525.0))).collect::<Result<Vec<_>>>()?;
        return Ok(Element::Group { id, x, y, width, height, view_width, view_height, children });
    }
    if node.has_tag_name((P, "pic")) {
        let fill = child(node, P, "blipFill").ok_or_else(|| Error::Unsupported("picture fill".into()))?;
        let reference = child(fill, A, "blip").and_then(|blip| blip.attribute((R, "embed"))).ok_or_else(|| Error::Unsupported("external or missing picture".into()))?;
        let targets = relationship_targets(package, part, "image")?;
        let path = targets.get(reference).ok_or_else(|| Error::Unsupported("picture part missing".into()))?;
        let bytes = package.part(path)?;
        let mime_type = match image::guess_format(bytes) { Ok(image::ImageFormat::Png) => "image/png", Ok(image::ImageFormat::Jpeg) => "image/jpeg", _ => return Err(Error::Unsupported("non-raster picture".into())) }.into();
        let crop = child(fill, A, "srcRect").map(|node| Crop { left: numeric(node, "l", 0.0) / 100000.0, top: numeric(node, "t", 0.0) / 100000.0, right: numeric(node, "r", 0.0) / 100000.0, bottom: numeric(node, "b", 0.0) / 100000.0 }).unwrap_or_default();
        return Ok(Element::Picture { id, x, y, width, height, base64: STANDARD.encode(bytes), mime_type, alt: properties(node).and_then(|node| node.attribute("descr")).unwrap_or("").into(), crop });
    }
    if node.has_tag_name((P, "cxnSp")) {
        let connection = |tag| node.descendants().find(|node| node.has_tag_name((A, tag))).and_then(|node| Some(Connection { element_id: format!("shape-{}", node.attribute("id")?), site: node.attribute("idx")?.parse().ok()? }));
        let line = shape_properties(node).and_then(|node| child(node, A, "ln"));
        return Ok(Element::Connector { id, x, y, width, height, color: line.map_or_else(|| "586563".into(), |line| solid(line, "586563")), stroke_width: line.map_or(1.0, |line| numeric(line, "w", 9525.0) / 9525.0), arrow: line.and_then(|line| child(line, A, "tailEnd")).is_some_and(|tail| tail.attribute("type").is_some_and(|value| value != "none")), flip_v: transform(node).and_then(|node| node.attribute("flipV")) == Some("1"), start: connection("stCxn"), end: connection("endCxn"), routing: None });
    }
    if node.has_tag_name((P, "graphicFrame")) {
        if let Some(table) = node.descendants().find(|node| node.has_tag_name((A, "tbl"))) {
            let rows = table.children().filter(|node| node.has_tag_name((A, "tr"))).map(|row| row.children().filter(|node| node.has_tag_name((A, "tc"))).map(|cell| child(cell, A, "txBody").map(paragraph_text).unwrap_or_default()).collect()).collect();
            let size = table.descendants().find(|node| node.has_tag_name((A, "rPr"))).map_or(20.0, |node| numeric(node, "sz", 1500.0) / 75.0);
            return Ok(Element::Table { id, x, y, width, height, rows, font_size: size });
        }
        return read_chart(package, part, node, (&id, [x, y, width, height]));
    }
    if node.has_tag_name((P, "sp")) {
        if let Some(body) = child(node, P, "txBody") {
            let text = paragraph_text(body);
            if !text.is_empty() {
                let style = run_properties(body);
                return Ok(Element::Text { id, x, y, width, height, text, font_size: style.map_or(24.0, |node| numeric(node, "sz", 1800.0) / 75.0), color: style.map_or_else(|| "202525".into(), |node| solid(node, "202525")), bold: style.and_then(|node| node.attribute("b")) == Some("1"), format: Default::default() });
            }
        }
        let properties = shape_properties(node).ok_or_else(|| Error::Unsupported("shape properties missing".into()))?;
        if child(properties, A, "prstGeom").and_then(|node| node.attribute("prst")) != Some("rect") { return Err(Error::Unsupported("non-rectangle geometry".into())); }
        return Ok(Element::Rect { id, x, y, width, height, fill: solid(properties, "FFFFFF") });
    }
    Err(Error::Unsupported("unsupported visual object".into()))
}

fn text_is_writable(node: Node<'_, '_>) -> bool {
    let Some(body) = child(node, P, "txBody") else { return false };
    let paragraphs: Vec<_> = body.children().filter(|node| node.has_tag_name((A, "p"))).collect();
    !paragraphs.is_empty() && paragraphs.iter().all(|paragraph| {
        let runs: Vec<_> = paragraph.children().filter(|node| node.has_tag_name((A, "r"))).collect();
        runs.len() == 1 && !paragraph.children().any(|node| node.has_tag_name((A, "fld")) || node.has_tag_name((A, "br"))) && child(runs[0], A, "t").is_some_and(|text| text.children().count() == 1 && text.first_child().is_some_and(|child| child.is_text()))
    })
}

pub fn import_pptx(bytes: Vec<u8>) -> Result<ImportedDeck> {
    let source_sha256 = format!("{:x}", Sha256::digest(&bytes));
    let package = Package::open(bytes)?;
    let main = relationship_targets(&package, "", "officeDocument")?.values().next().cloned().ok_or_else(|| Error::Invalid("presentation root missing".into()))?;
    let presentation = parse(package.text(&main)?)?;
    let size = child(presentation.root_element(), P, "sldSz").ok_or_else(|| Error::Unsupported("slide dimensions missing".into()))?;
    let native_width = numeric(size, "cx", 0.0); let native_height = numeric(size, "cy", 0.0);
    if !native_width.is_finite() || !native_height.is_finite() || native_width <= 0.0 || native_height <= 0.0 { return Err(Error::Invalid("invalid slide dimensions".into())); }
    let scale = (1280.0 / native_width, 720.0 / native_height);
    let paths = slide_paths(&package)?;
    if paths.is_empty() || paths.len() > 32 { return Err(Error::Limit("visual import supports 1-32 slides".into())); }
    let mut slides = Vec::new(); let mut objects = Vec::new();
    let mut warnings = vec!["Import preview is approximate: unsupported objects, master content, mixed-run formatting and effects may be absent. Original package bytes remain preserved; do not treat this preview as Office visual parity.".into()];
    for (index, part) in paths.iter().enumerate() {
        let document = parse(package.text(part)?)?;
        let common = child(document.root_element(), P, "cSld").ok_or_else(|| Error::Invalid("slide content missing".into()))?;
        let tree = child(common, P, "spTree").ok_or_else(|| Error::Invalid("slide shape tree missing".into()))?;
        let slide_id = format!("slide-{}", index + 1);
        let mut elements = Vec::new();
        for node in tree.children().filter(|node| node.is_element() && !["nvGrpSpPr", "grpSpPr", "extLst"].contains(&node.tag_name().name())) {
            match read_element(&package, part, node, scale) {
                Ok(element) => {
                    let probe = Deck { version: 1, title: "Import".into(), width: 1280, height: 720, slides: vec![Slide { id: slide_id.clone(), title: "Import".into(), background: "FFFFFF".into(), elements: vec![element.clone()], notes: String::new(), layout_id: None, inherit_background: false, hide_master_graphics: false, native_source_id: None }], design: None };
                    if !matches!(element, Element::Connector { .. }) && validate_deck(&probe).is_err() { warnings.push(format!("{part} / {} preserved but not previewed: geometry or content outside scene limits", element.bounds().0)); continue; }
                    let mut fields = vec!["x".into(), "y".into(), "width".into(), "height".into()];
                    if matches!(element, Element::Text { .. }) && text_is_writable(node) { fields.push("text".into()); }
                    if matches!(element, Element::Group { .. } | Element::Connector { .. }) { fields.clear(); }
                    objects.push(ImportedObject { slide_id: slide_id.clone(), element_id: element.bounds().0.into(), part: part.clone(), editable_fields: fields });
                    elements.push(element);
                }
                Err(error) => warnings.push(format!("{part} / {} preserved but not previewed: {error}", node.tag_name().name())),
            }
        }
        let background = child(common, P, "bg").and_then(|node| child(node, P, "bgPr")).map_or_else(|| "FFFFFF".into(), |node| solid(node, "FFFFFF"));
        let notes = relationship_targets(&package, part, "notesSlide").ok().and_then(|targets| targets.values().next().cloned()).and_then(|path| package.text(&path).ok()).and_then(|value| parse(value).ok()).map(|document| document.descendants().filter(|node| node.has_tag_name((P, "txBody"))).map(paragraph_text).collect::<Vec<_>>().join("\n")).unwrap_or_default();
        let title = common.attribute("name").filter(|title| title.chars().count() <= 120).map(String::from).unwrap_or_else(|| format!("Imported slide {}", index + 1));
        slides.push(Slide { id: slide_id, title, background, elements, notes, layout_id: None, inherit_background: false, hide_master_graphics: false, native_source_id: None });
    }
    let deck = Deck { version: 1, title: "Imported presentation".into(), width: 1280, height: 720, slides, design: None };
    validate_deck(&deck)?;
    Ok(ImportedDeck { deck, source_sha256, warnings, objects })
}

fn replace_attribute(node: Node<'_, '_>, name: &str, value: String, edits: &mut Vec<(Range<usize>, String)>) -> Result<()> {
    let attribute = node.attributes().find(|attribute| attribute.name() == name).ok_or_else(|| Error::Unsupported("missing geometry attribute cannot be patched".into()))?;
    edits.push((attribute.range_value(), value)); Ok(())
}

pub fn save_import(bytes: Vec<u8>, deck: &Deck) -> Result<Vec<u8>> {
    validate_deck(deck)?;
    let imported = import_pptx(bytes.clone())?;
    if crate::canonical::bytes(&imported.deck)? == crate::canonical::bytes(deck)? { return Ok(bytes); }
    let mut package = Package::open(bytes)?;
    if package.parts().keys().any(|path| path.to_ascii_lowercase().starts_with("_xmlsignatures/")) { return Err(Error::Unsupported("editing signed packages".into())); }
    if deck.title != imported.deck.title || deck.slides.len() != imported.deck.slides.len() || crate::canonical::bytes(&deck.design)? != crate::canonical::bytes(&imported.deck.design)? { return Err(Error::Unsupported("imported title, design or slide structure change".into())); }
    let main = relationship_targets(&package, "", "officeDocument")?.values().next().cloned().ok_or_else(|| Error::Invalid("presentation missing".into()))?;
    let presentation = parse(package.text(&main)?)?;
    let size = child(presentation.root_element(), P, "sldSz").ok_or_else(|| Error::Invalid("slide size missing".into()))?;
    let scale = (numeric(size, "cx", 0.0) / 1280.0, numeric(size, "cy", 0.0) / 720.0);
    let paths = slide_paths(&package)?;
    for ((before, after), part) in imported.deck.slides.iter().zip(&deck.slides).zip(paths) {
        if before.id != after.id || before.title != after.title || before.background != after.background || before.notes != after.notes || before.elements.len() != after.elements.len() || before.layout_id != after.layout_id || before.inherit_background != after.inherit_background || before.hide_master_graphics != after.hide_master_graphics { return Err(Error::Unsupported("only supported imported object fields may be changed".into())); }
        let original = package.text(&part)?.to_string();
        let document = parse(&original)?;
        let mut edits = Vec::new();
        for (old, new) in before.elements.iter().zip(&after.elements) {
            if crate::canonical::bytes(old)? == crate::canonical::bytes(new)? { continue; }
            if old.bounds().0 != new.bounds().0 { return Err(Error::Unsupported("imported object order or identity changed".into())); }
            let policy = imported.objects.iter().find(|object| object.slide_id == before.id && object.element_id == old.bounds().0).ok_or_else(|| Error::Unsupported("imported object is read-only".into()))?;
            let old_json = serde_json::to_value(old)?; let new_json = serde_json::to_value(new)?;
            let keys: std::collections::BTreeSet<_> = old_json.as_object().into_iter().flatten().chain(new_json.as_object().into_iter().flatten()).map(|(key, _)| key.as_str()).collect();
            let changed: Vec<_> = keys.into_iter().filter(|key| !crate::canonical::equal(old_json.get(*key).unwrap_or(&Value::Null), new_json.get(*key).unwrap_or(&Value::Null))).collect();
            if changed.iter().any(|field| !policy.editable_fields.iter().any(|allowed| allowed == field)) { return Err(Error::Unsupported("imported shape change exceeds its writable fields".into())); }
            let mut nodes = document.descendants().filter(|node| node.is_element() && ["sp", "pic", "graphicFrame", "cxnSp", "grpSp"].contains(&node.tag_name().name()) && identity(*node).ok().as_deref() == Some(old.bounds().0));
            let node = nodes.next().ok_or_else(|| Error::Conflict("imported shape disappeared".into()))?;
            if nodes.next().is_some() { return Err(Error::Conflict("ambiguous imported shape ID".into())); }
            if changed.contains(&"text") {
                let Element::Text { text, .. } = new else { return Err(Error::Unsupported("text edit on non-text shape".into())) };
                let body = child(node, P, "txBody").ok_or_else(|| Error::Unsupported("text body missing".into()))?;
                let text_nodes: Vec<_> = body.descendants().filter(|node| node.has_tag_name((A, "t"))).collect();
                let lines: Vec<_> = text.split('\n').collect();
                if lines.len() != text_nodes.len() { return Err(Error::Unsupported("imported text edit must preserve paragraph count".into())); }
                for (text_node, replacement) in text_nodes.iter().zip(lines) {
                    let range = text_node.first_child().ok_or_else(|| Error::Unsupported("empty imported text node".into()))?.range();
                    if original[range.clone()].contains('<') { return Err(Error::Unsupported("CDATA or mixed text cannot be patched".into())); }
                    edits.push((range, quick_xml::escape::escape(replacement).into_owned()));
                }
            }
            if changed.iter().any(|field| ["x", "y", "width", "height"].contains(field)) {
                let transform = transform(node).ok_or_else(|| Error::Unsupported("shape transform missing".into()))?;
                let offset = child(transform, A, "off").ok_or_else(|| Error::Unsupported("shape offset missing".into()))?;
                let extent = child(transform, A, "ext").ok_or_else(|| Error::Unsupported("shape extent missing".into()))?;
                let (_, x, y, width, height) = new.bounds();
                for (field, target, attribute, value) in [("x", offset, "x", x * scale.0), ("y", offset, "y", y * scale.1), ("width", extent, "cx", width * scale.0), ("height", extent, "cy", height * scale.1)] {
                    if changed.contains(&field) { replace_attribute(target, attribute, value.round().to_string(), &mut edits)?; }
                }
            }
        }
        if edits.is_empty() { continue; }
        edits.sort_by_key(|edit| edit.0.start);
        if edits.windows(2).any(|pair| pair[0].0.end > pair[1].0.start) { return Err(Error::Conflict("overlapping imported edits".into())); }
        let mut updated = original;
        for (range, value) in edits.into_iter().rev() { updated.replace_range(range, &value); }
        parse(&updated)?; package.replace_part(&part, updated.into_bytes())?;
    }
    package.save()
}