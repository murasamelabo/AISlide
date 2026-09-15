use crate::{model::{Deck, Element}, native::{A, P, NativeDeck, NativePart, child, is_shape, properties, relations_path}, package::Package, pptx::parse, Error, Result};
use roxmltree::Node;
use std::{collections::{BTreeMap, BTreeSet}, ops::Range};

type Edits = Vec<(Range<usize>, String)>;

fn equal(left: &impl serde::Serialize, right: &impl serde::Serialize) -> Result<bool> { Ok(crate::canonical::bytes(left)? == crate::canonical::bytes(right)?) }

pub(crate) fn check_duplicate(document: &crate::document::Document, slide_id: &str, element: &Element) -> Result<()> {
    let Some(origin) = document.origin.as_ref().filter(|origin| origin.native) else { return Ok(()) };
    use base64::Engine;
    let package = Package::open(base64::engine::general_purpose::STANDARD.decode(&origin.base64).map_err(|_| Error::Invalid("origin base64".into()))?)?;
    let native = crate::native::read(&package)?;
    let Some(binding) = native.slides.iter().find(|part| part.id == slide_id) else { return Ok(()) };
    let Some(numeric) = binding.nodes.get(element.bounds().0) else { return Ok(()) };
    let xml = package.text(&binding.path)?; let parsed = parse(xml)?;
    let node = parsed.descendants().find(|node| is_shape(*node) && properties(*node).and_then(|node| node.attribute("id")) == Some(numeric)).ok_or_else(|| Error::Invalid("native element missing".into()))?;
    let ids = binding.nodes.iter().map(|(id, numeric)| Ok((id.as_str(), numeric.parse::<usize>().map_err(|_| Error::Invalid("native shape identity".into()))?))).collect::<Result<BTreeMap<_, _>>>()?;
    let fallback = crate::design::Theme::default();
    let generated = crate::pptx::element_xml(element, ids[element.bounds().0], &ids, document.deck.design.as_ref().map(|design| &design.theme).unwrap_or(&fallback));
    if xml_value(node) != xml_value(parse(&generated)?.root_element()) { return Err(Error::Unsupported("native object has formatting not represented by an individual copy; duplicate its slide instead".into())); }
    Ok(())
}

fn xml_value(node: Node<'_, '_>) -> serde_json::Value {
    let attributes: BTreeMap<_, _> = node.attributes().map(|attribute| (format!("{}:{}", attribute.namespace().unwrap_or(""), attribute.name()), attribute.value())).collect();
    let children: Vec<_> = node.children().filter_map(|node| {
        if node.is_element() { Some(xml_value(node)) }
        else if node.is_text() && !node.text().unwrap_or("").trim().is_empty() { Some(serde_json::json!(node.text())) }
        else { None }
    }).collect();
    serde_json::json!({"namespace":node.tag_name().namespace(),"name":node.tag_name().name(),"attributes":attributes,"children":children})
}

fn chart_is_representable(package: &Package, binding: &NativePart, id: &str, kind: crate::model::ChartKind, categories: &[String], series: &[crate::model::ChartSeries]) -> Result<bool> {
    let document = parse(package.text(&binding.path)?)?;
    let numeric = binding.nodes.get(id).ok_or_else(|| Error::Invalid("native chart identity missing".into()))?;
    let node = document.descendants().find(|node| is_shape(*node) && properties(*node).and_then(|node| node.attribute("id")) == Some(numeric)).ok_or_else(|| Error::Invalid("native chart missing".into()))?;
    let relation = node.descendants().find(|node| node.has_tag_name((crate::native::C, "chart"))).and_then(|node| node.attribute((crate::native::R, "id"))).ok_or_else(|| Error::Invalid("native chart relationship missing".into()))?;
    let targets = crate::pptx::relationship_targets(package, &binding.path, "chart")?;
    let path = targets.get(relation).ok_or_else(|| Error::Invalid("native chart target missing".into()))?;
    let actual = parse(package.text(path)?)?;
    let expected = crate::charts::chart(kind, categories, series);
    let expected = parse(std::str::from_utf8(&expected).map_err(|_| Error::Invalid("chart XML encoding".into()))?)?;
    Ok(xml_value(actual.root_element()) == xml_value(expected.root_element()))
}

pub(crate) fn remap_resources(xml: String, mapping: &BTreeMap<String, String>) -> Result<String> {
    let document = parse(&xml)?; let mut edits = Vec::new();
    for node in document.descendants() { for attribute in node.attributes() {
        if attribute.namespace() == Some(crate::native::R) { if let Some(value) = mapping.get(attribute.value()) { edits.push((attribute.range_value(), quick_xml::escape::escape(value).into_owned())); } }
    } }
    String::from_utf8(apply(xml, edits)?).map_err(|_| Error::Invalid("resource XML is not UTF-8".into()))
}

pub(crate) fn scaled_element(element: &Element, ids: &BTreeMap<&str, usize>, theme: &crate::design::Theme, scale: (f64, f64)) -> Result<String> {
    let xml = crate::pptx::element_xml(element, ids[element.bounds().0], ids, theme);
    if scale == (9525.0, 9525.0) { return Ok(xml); }
    let document = parse(&xml)?; let root = document.root_element();
    let transform = child(root, P, "xfrm").or_else(|| child(root, P, if matches!(element, Element::Group { .. }) { "grpSpPr" } else { "spPr" }).and_then(|node| child(node, A, "xfrm")));
    let mut edits = Vec::new();
    if let Some(transform) = transform {
        for (tag, horizontal, vertical) in [("off", "x", "y"), ("ext", "cx", "cy")] {
            let node = child(transform, A, tag).ok_or_else(|| Error::Invalid("generated native geometry missing".into()))?;
            for (name, factor) in [(horizontal, scale.0 / 9525.0), (vertical, scale.1 / 9525.0)] {
                let value: f64 = node.attribute(name).and_then(|value| value.parse().ok()).ok_or_else(|| Error::Invalid("generated native coordinate".into()))?;
                set_attribute(&xml, node, name, &(value * factor).round().to_string(), &mut edits)?;
            }
        }
    }
    String::from_utf8(apply(xml, edits)?).map_err(|_| Error::Invalid("native geometry XML encoding".into()))
}

pub(crate) fn resource_changes(package: &mut Package, binding: &NativePart, before: &[Element], after: &[Element], ids: &BTreeMap<&str, usize>) -> Result<BTreeMap<String, String>> {
    let old: BTreeMap<_, _> = crate::model::element_list(before).into_iter().map(|element| (element.bounds().0, element)).collect();
    let mut resources = Vec::new();
    for element in crate::model::element_list(after) {
        let previous = old.get(element.bounds().0).copied();
        let needed = match (previous, element) {
            (Some(Element::Chart { kind: old_kind, categories: old_categories, series: old_series, .. }), Element::Chart { kind, categories, series, .. }) => {
                let changed = old_kind != kind || old_categories != categories || !equal(old_series, series)?;
                if changed && !chart_is_representable(package, binding, element.bounds().0, *old_kind, old_categories, old_series)? { return Err(Error::Unsupported("custom native chart settings are preserved; data/style replacement is not safely representable".into())); }
                changed
            }
            (Some(Element::Picture { base64: old_data, mime_type: old_mime, .. }), Element::Picture { base64, mime_type, .. }) => old_data != base64 || old_mime != mime_type,
            (_, Element::Chart { .. } | Element::Picture { .. }) => true,
            (_, Element::Text { format, .. } | Element::Shape { format, .. }) => format.hyperlink.is_some() && previous.map(|old| equal(old, element)).transpose()?.map(|same| !same).unwrap_or(true),
            _ => false,
        };
        if needed { resources.push(element); }
    }
    if resources.is_empty() { return Ok(BTreeMap::new()); }
    let maximum = |prefix: &str| package.part_names().iter().filter_map(|name| name.strip_prefix(prefix)).filter_map(|name| name.split('.').next()?.parse::<usize>().ok()).max().unwrap_or(0);
    let mut chart_number = maximum("ppt/charts/chart").max(maximum("ppt/embeddings/chart"));
    let mut picture_number = maximum("ppt/media/image");
    if chart_number > u32::MAX as usize - 2048 || picture_number > u32::MAX as usize - 2048 { return Err(Error::Limit("native resource IDs exhausted".into())); }
    let mut parts = BTreeMap::new(); let mut types = Vec::new();
    let relationships = crate::pptx::resource_parts(&mut parts, &resources, &mut chart_number, &mut picture_number, &mut types, Some(ids))?;
    for (path, data) in parts { package.add_part(path, data)?; }
    let relation_path = relations_path(&binding.path);
    let xml = if package.part_names().contains(relation_path.as_str()) { package.text(&relation_path)?.to_owned() } else { format!("<Relationships xmlns=\"{}\"/>", crate::native::REL) };
    let parsed = parse(&xml)?;
    let mut taken: BTreeSet<String> = parsed.root_element().children().filter_map(|node| node.attribute("Id").map(str::to_owned)).collect();
    let mut counter = 1; let mut mapping = BTreeMap::new(); let mut extra = String::new();
    for (old_id, kind, target) in relationships {
        let id = loop { let candidate = format!("rIdAislideResource{counter}"); counter += 1; if taken.insert(candidate.clone()) { break candidate; } };
        let target = if kind == "hyperlink" { target } else { format!("/ppt/{}", target.strip_prefix("../").ok_or_else(|| Error::Invalid("generated resource target".into()))?) };
        extra.push_str(&format!("<Relationship xmlns=\"{}\" Id=\"{id}\" Type=\"{}/{kind}\" Target=\"{}\"{}/>", crate::native::REL, crate::native::R, quick_xml::escape::escape(&target), if kind == "hyperlink" { " TargetMode=\"External\"" } else { "" }));
        mapping.insert(old_id, id);
    }
    let mut edits = Vec::new(); insert_child(&xml, parsed.root_element(), &extra, None, &mut edits)?;
    let data = apply(xml, edits)?;
    if package.part_names().contains(relation_path.as_str()) { package.replace_part(&relation_path, data)?; } else { package.add_part(relation_path, data)?; }
    if !types.is_empty() {
        let xml = package.text("[Content_Types].xml")?.to_owned(); let parsed = parse(&xml)?;
        let entries = types.into_iter().map(|(path, kind)| format!("<Override xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\" PartName=\"/{path}\" ContentType=\"{}\"/>", quick_xml::escape::escape(&kind))).collect::<String>();
        let mut edits = Vec::new(); insert_child(&xml, parsed.root_element(), &entries, None, &mut edits)?; package.replace_part("[Content_Types].xml", apply(xml, edits)?)?;
    }
    Ok(mapping)
}

fn start_end(xml: &str, node: Node<'_, '_>) -> Result<usize> {
    let segment = &xml[node.range()];
    let mut quote = None;
    for (offset, character) in segment.char_indices() {
        if let Some(active) = quote { if character == active { quote = None; } }
        else if character == '\'' || character == '"' { quote = Some(character); }
        else if character == '>' { return Ok(node.range().start + offset); }
    }
    Err(Error::Invalid("XML start tag is incomplete".into()))
}

pub(crate) fn set_attribute(xml: &str, node: Node<'_, '_>, name: &str, value: &str, edits: &mut Edits) -> Result<()> {
    let escaped = quick_xml::escape::escape(value).into_owned();
    if let Some(attribute) = node.attributes().find(|attribute| attribute.namespace().is_none() && attribute.name() == name) { edits.push((attribute.range_value(), escaped)); }
    else {
        let end = start_end(xml, node)?;
        let position = if xml.as_bytes()[end - 1] == b'/' { end - 1 } else { end };
        edits.push((position..position, format!(" {name}=\"{escaped}\"")));
    }
    Ok(())
}

pub(crate) fn insert_child(xml: &str, node: Node<'_, '_>, fragment: &str, before: Option<Node<'_, '_>>, edits: &mut Edits) -> Result<()> {
    let end = start_end(xml, node)?;
    if xml.as_bytes()[end - 1] == b'/' {
        let qualified = &xml[node.range().start + 1..].split([' ', '\t', '\r', '\n', '/', '>']).next().ok_or_else(|| Error::Invalid("XML element name missing".into()))?;
        edits.push((end - 1..end + 1, format!(">{fragment}</{qualified}>")));
    } else {
        let position = before.map(|node| node.range().start).unwrap_or_else(|| node.range().start + xml[node.range()].rfind("</").unwrap_or(xml[node.range()].len()));
        edits.push((position..position, fragment.into()));
    }
    Ok(())
}

fn fragment(xml: &str, node: Node<'_, '_>) -> String {
    let mut result = xml[node.range()].to_owned();
    let name = result[1..].split([' ', '\t', '\r', '\n', '/', '>']).next().unwrap_or("");
    let position = name.len() + 1;
    result.insert_str(position, &format!(" xmlns:p=\"{P}\" xmlns:a=\"{A}\" xmlns:r=\"{}\"", crate::native::R));
    result
}

fn merge_child(xml: &str, parent: Node<'_, '_>, generated: &str, next: Node<'_, '_>, namespace: &str, name: &str, order: &[&str], edits: &mut Edits) -> Result<()> {
    let old = child(parent, namespace, name); let new = child(next, namespace, name);
    match (old, new) {
        (Some(old), Some(new)) => edits.push((old.range(), fragment(generated, new))),
        (Some(old), None) => edits.push((old.range(), String::new())),
        (None, Some(new)) => {
            let rank = order.iter().position(|candidate| *candidate == name).unwrap_or(0);
            let before = parent.children().find(|node| node.is_element() && order.iter().position(|candidate| *candidate == node.tag_name().name()).is_some_and(|position| position > rank));
            insert_child(xml, parent, &fragment(generated, new), before, edits)?;
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn apply(xml: String, mut edits: Edits) -> Result<Vec<u8>> {
    edits.sort_by_key(|(range, _)| range.start);
    if edits.windows(2).any(|pair| pair[0].0.end > pair[1].0.start) { return Err(Error::Conflict("overlapping native XML edits".into())); }
    let mut output = xml;
    for (range, replacement) in edits.into_iter().rev() { output.replace_range(range, &replacement); }
    parse(&output)?; Ok(output.into_bytes())
}

fn plain_paragraphs(body: Node<'_, '_>) -> bool {
    body.children().filter(|node| node.has_tag_name((A, "p"))).all(|paragraph| {
        paragraph.children().filter(|node| node.has_tag_name((A, "r"))).count() == 1
            && paragraph.descendants().filter(|node| node.has_tag_name((A, "t"))).count() == 1
            && !paragraph.descendants().any(|node| node.has_tag_name((A, "fld")) || node.has_tag_name((A, "br")))
    })
}

fn replace_text(xml: &str, body: Node<'_, '_>, text: &str, edits: &mut Edits) -> Result<()> {
    let paragraphs: Vec<_> = body.children().filter(|node| node.has_tag_name((A, "p"))).collect();
    if paragraphs.is_empty() || !plain_paragraphs(body) { return Err(Error::Unsupported("mixed-run or field text is preserved; this text change cannot be represented safely".into())); }
    let lines: Vec<_> = text.split('\n').collect();
    for (index, paragraph) in paragraphs.iter().enumerate() {
        if let Some(line) = lines.get(index) {
            let node = paragraph.descendants().find(|node| node.has_tag_name((A, "t"))).unwrap();
            edits.push((node.range(), format!("<a:t xmlns:a=\"{A}\">{}</a:t>", quick_xml::escape::escape(*line))));
        } else { edits.push((paragraph.range(), String::new())); }
    }
    if lines.len() > paragraphs.len() {
        let prototype = paragraphs[paragraphs.len() - 1];
        let node = prototype.descendants().find(|node| node.has_tag_name((A, "t"))).unwrap();
        let mut added = String::new();
        for line in &lines[paragraphs.len()..] {
            let mut paragraph = xml[prototype.range()].to_owned();
            paragraph.replace_range(node.range().start - prototype.range().start..node.range().end - prototype.range().start, &format!("<a:t xmlns:a=\"{A}\">{}</a:t>", quick_xml::escape::escape(*line)));
            added.push_str(&paragraph);
        }
        let position = prototype.range().end; edits.push((position..position, added));
    }
    Ok(())
}

fn supports_style(body: Node<'_, '_>) -> bool {
    if !plain_paragraphs(body) { return false; }
    let allowed = ["txBody", "bodyPr", "noAutofit", "lstStyle", "lvl1pPr", "p", "pPr", "lnSpc", "spcPct", "buNone", "buChar", "buAutoNum", "r", "rPr", "defRPr", "endParaRPr", "t", "solidFill", "srgbClr", "schemeClr", "latin", "ea", "cs", "hlinkClick"];
    if body.descendants().filter(|node| node.is_element()).any(|node| !allowed.contains(&node.tag_name().name())) { return false; }
    for node in body.descendants().filter(|node| node.is_element()) {
        let attributes: &[&str] = match node.tag_name().name() {
            "bodyPr" => &["lIns", "rIns", "tIns", "bIns", "wrap", "anchor"],
            "lvl1pPr" | "pPr" => &["algn", "marL", "indent"],
            "rPr" | "defRPr" | "endParaRPr" => &["lang", "sz", "b", "i", "u", "dirty"],
            "latin" | "ea" | "cs" => &["typeface"],
            "buChar" => &["char"], "buAutoNum" => &["type"], "hlinkClick" => &["id"],
            "spcPct" | "srgbClr" | "schemeClr" => &["val"], "t" => &["space"], _ => &[],
        };
        if node.attributes().any(|attribute| !attributes.contains(&attribute.name())) { return false; }
        if node.has_tag_name((A, "buChar")) && node.attribute("char") != Some("\u{2022}") { return false; }
        if node.has_tag_name((A, "buAutoNum")) && node.attribute("type") != Some("arabicPeriod") { return false; }
    }
    if child(body, A, "bodyPr").is_some_and(|node| ["lIns", "rIns", "tIns", "bIns"].iter().any(|name| node.attribute(*name).is_some_and(|value| value != "0"))) { return false; }
    let runs: Vec<_> = body.descendants().filter(|node| node.has_tag_name((A, "rPr"))).collect();
    runs.windows(2).all(|pair| {
        let left: BTreeMap<_, _> = pair[0].attributes().filter(|attr| attr.name() != "lang").map(|attr| ((attr.namespace(), attr.name()), attr.value())).collect();
        let right: BTreeMap<_, _> = pair[1].attributes().filter(|attr| attr.name() != "lang").map(|attr| ((attr.namespace(), attr.name()), attr.value())).collect();
        left == right && pair[0].children().filter(|node| node.is_element()).map(xml_value).eq(pair[1].children().filter(|node| node.is_element()).map(xml_value))
    })
}

fn geometry(xml: &str, node: Node<'_, '_>, generated: &str, next: Node<'_, '_>, old: &Element, new: &Element, scale: (f64, f64), edits: &mut Edits) -> Result<()> {
    let inherited = matches!(new, Element::Text { format, .. } if format.inherit_layout);
    let parent = child(node, P, if matches!(old, Element::Group { .. }) { "grpSpPr" } else { "spPr" });
    let xfrm = child(node, P, "xfrm").or_else(|| parent.and_then(|node| child(node, A, "xfrm")));
    let (_, x, y, width, height) = new.bounds();
    if inherited {
        if let Some(xfrm) = xfrm { edits.push((xfrm.range(), String::new())); }
        return Ok(());
    }
    if let Some(xfrm) = xfrm {
        let offset = child(xfrm, A, "off"); let extent = child(xfrm, A, "ext");
        if let (Some(offset), Some(extent)) = (offset, extent) {
            for (node, name, value) in [(offset, "x", x * scale.0), (offset, "y", y * scale.1), (extent, "cx", width * scale.0), (extent, "cy", height * scale.1)] { set_attribute(xml, node, name, &value.round().to_string(), edits)?; }
        } else { return Err(Error::Unsupported("partial inherited transforms are preserved; detach using a supported layout".into())); }
        if let Element::Shape { rotation, .. } = new { set_attribute(xml, xfrm, "rot", &((*rotation * 60000.0).round()).to_string(), edits)?; }
        if let Element::Connector { flip_v, .. } = new { set_attribute(xml, xfrm, "flipV", if *flip_v { "1" } else { "0" }, edits)?; }
    } else {
        let parent = parent.ok_or_else(|| Error::Unsupported("shape properties missing".into()))?;
        let generated_parent = child(next, P, "spPr").ok_or_else(|| Error::Invalid("generated shape properties".into()))?;
        merge_child(xml, parent, generated, generated_parent, A, "xfrm", &["xfrm", "prstGeom", "custGeom", "solidFill", "noFill", "ln", "effectLst", "extLst"], edits)?;
    }
    Ok(())
}

fn text_changes(xml: &str, node: Node<'_, '_>, generated: &str, next: Node<'_, '_>, previous: Node<'_, '_>, old: &Element, new: &Element, edits: &mut Edits) -> Result<()> {
    let (old_text, old_size, old_color, old_bold, old_format) = match old { Element::Text { text, font_size, color, bold, format, .. } | Element::Shape { text, font_size, color, bold, format, .. } => (text, font_size, color, bold, format), _ => return Ok(()) };
    let (text, size, color, bold, format) = match new { Element::Text { text, font_size, color, bold, format, .. } | Element::Shape { text, font_size, color, bold, format, .. } => (text, font_size, color, bold, format), _ => return Err(Error::Unsupported("native object type change".into())) };
    let style_changed = !equal(&(old_size, old_color, old_bold, old_format), &(size, color, bold, format))?;
    if old_text == text && (!style_changed || (old_format.inherit_layout && format.inherit_layout)) { return Ok(()); }
    let body = child(node, P, "txBody").ok_or_else(|| Error::Unsupported("text body missing".into()))?;
    if style_changed && !(old_format.inherit_layout && format.inherit_layout) {
        let expected = child(previous, P, "txBody").ok_or_else(|| Error::Invalid("previous text body missing".into()))?;
        if !supports_style(body) || xml_value(body) != xml_value(expected) { return Err(Error::Unsupported("complex native text formatting is preserved; only its supported text/geometry fields are writable".into())); }
        let next_body = child(next, P, "txBody").ok_or_else(|| Error::Invalid("generated text body".into()))?;
        edits.push((body.range(), fragment(generated, next_body)));
    } else if old_text != text { replace_text(xml, body, text, edits)?; }
    Ok(())
}

fn patch_element(xml: &str, node: Node<'_, '_>, old: &Element, new: &Element, ids: &BTreeMap<&str, usize>, theme: &crate::design::Theme, binding: &NativePart, resources: &BTreeMap<String, String>, scale: (f64, f64), edits: &mut Edits) -> Result<()> {
    if equal(old, new)? { return Ok(()); }
    if std::mem::discriminant(old) != std::mem::discriminant(new) { return Err(Error::Unsupported("native shape type replacement".into())); }
    let generated = remap_resources(scaled_element(new, ids, theme, scale)?, resources)?;
    let parsed = parse(&generated)?; let next = parsed.root_element();
    let previous = scaled_element(old, ids, theme, scale)?;
    let previous = parse(&previous)?; let previous = previous.root_element();
    if let (Element::Text { format: before, .. }, Element::Text { format: after, .. }) = (old, new) {
        if before.placeholder != after.placeholder {
            let parent = child(node, P, "nvSpPr").and_then(|node| child(node, P, "nvPr")).ok_or_else(|| Error::Unsupported("native placeholder properties missing".into()))?;
            let next_parent = child(next, P, "nvSpPr").and_then(|node| child(node, P, "nvPr")).ok_or_else(|| Error::Invalid("generated placeholder properties".into()))?;
            merge_child(xml, parent, &generated, next_parent, P, "ph", &["ph", "audioCd", "wavAudioFile", "audioFile", "videoFile", "quickTimeFile", "custDataLst", "extLst"], edits)?;
        }
    }
    let old_bounds = old.bounds(); let new_bounds = new.bounds();
    if old_bounds != new_bounds || matches!((old, new), (Element::Text { format: old, .. }, Element::Text { format: new, .. }) if old.inherit_layout != new.inherit_layout)
        || matches!((old, new), (Element::Shape { rotation: old, .. }, Element::Shape { rotation: new, .. }) if old != new) { geometry(xml, node, &generated, next, old, new, scale, edits)?; }
    text_changes(xml, node, &generated, next, previous, old, new, edits)?;
    match (old, new) {
        (Element::Polygon { points, fill, stroke, stroke_width, .. }, Element::Polygon { points: next_points, fill: next_fill, stroke: next_stroke, stroke_width: next_width, .. }) => {
            if points != next_points || fill != next_fill || stroke != next_stroke || stroke_width != next_width { return Err(Error::Unsupported("native polygon shape/style replacement is not supported; geometry remains editable".into())); }
        }
        (Element::Group { children: old, view_width, view_height, .. }, Element::Group { children: new, view_width: next_width, view_height: next_height, .. }) => {
            if view_width != next_width || view_height != next_height { return Err(Error::Unsupported("changing a native group coordinate system".into())); }
            patch_elements(xml, node, old, new, ids, theme, binding, resources, (9525.0, 9525.0), edits)?;
        }
        (Element::Rect { fill: old, .. }, Element::Rect { fill: new, .. }) if old != new => {
            let parent = child(node, P, "spPr").ok_or_else(|| Error::Unsupported("shape properties missing".into()))?;
            merge_child(xml, parent, &generated, child(next, P, "spPr").unwrap(), A, "solidFill", &["xfrm", "prstGeom", "custGeom", "solidFill", "ln", "effectLst", "extLst"], edits)?;
            if let Some(none) = child(parent, A, "noFill") { edits.push((none.range(), String::new())); }
        }
        (Element::Shape { fill, stroke, stroke_width, preset, .. }, Element::Shape { fill: next_fill, stroke: next_stroke, stroke_width: next_width, preset: next_preset, .. }) => {
            let parent = child(node, P, "spPr").ok_or_else(|| Error::Unsupported("shape properties missing".into()))?;
            let next_parent = child(next, P, "spPr").unwrap();
            if fill != next_fill { for tag in ["solidFill", "noFill"] { merge_child(xml, parent, &generated, next_parent, A, tag, &["xfrm", "prstGeom", "solidFill", "noFill", "ln", "effectLst", "extLst"], edits)?; } }
            for (changed, tag) in [(stroke != next_stroke || stroke_width != next_width, "ln"), (preset != next_preset, "prstGeom")] {
                if changed { merge_child(xml, parent, &generated, next_parent, A, tag, &["xfrm", "prstGeom", "solidFill", "ln", "effectLst", "extLst"], edits)?; }
            }
        }
        (Element::Table { rows, font_size, .. }, Element::Table { rows: next_rows, font_size: next_size, .. }) => {
            let table = node.descendants().find(|node| node.has_tag_name((A, "tbl"))).ok_or_else(|| Error::Unsupported("table missing".into()))?;
            if rows.len() != next_rows.len() || rows[0].len() != next_rows[0].len() {
                let expected = previous.descendants().find(|node| node.has_tag_name((A, "tbl"))).ok_or_else(|| Error::Invalid("previous table missing".into()))?;
                if xml_value(table) != xml_value(expected) { return Err(Error::Unsupported("custom native table formatting is preserved; structural replacement is not safely representable".into())); }
                let generated_table = next.descendants().find(|node| node.has_tag_name((A, "tbl"))).unwrap();
                edits.push((table.range(), fragment(&generated, generated_table)));
            } else {
                for ((before, after), cell) in rows.iter().flatten().zip(next_rows.iter().flatten()).zip(table.descendants().filter(|node| node.has_tag_name((A, "tc")))) {
                    let body = child(cell, A, "txBody").ok_or_else(|| Error::Unsupported("table cell text missing".into()))?;
                    if before != after { replace_text(xml, body, after, edits)?; }
                    if font_size != next_size { for properties in body.descendants().filter(|node| node.has_tag_name((A, "rPr")) || node.has_tag_name((A, "endParaRPr"))) { set_attribute(xml, properties, "sz", &(next_size * 75.0).round().to_string(), edits)?; } }
                }
                if old_bounds.3 != new_bounds.3 { for column in table.descendants().filter(|node| node.has_tag_name((A, "gridCol"))) { let width: f64 = column.attribute("w").and_then(|value| value.parse().ok()).ok_or_else(|| Error::Invalid("table column width".into()))?; set_attribute(xml, column, "w", &(width * new_bounds.3 / old_bounds.3).round().to_string(), edits)?; } }
                if old_bounds.4 != new_bounds.4 { for row in table.children().filter(|node| node.has_tag_name((A, "tr"))) { let height: f64 = row.attribute("h").and_then(|value| value.parse().ok()).ok_or_else(|| Error::Invalid("table row height".into()))?; set_attribute(xml, row, "h", &(height * new_bounds.4 / old_bounds.4).round().to_string(), edits)?; } }
            }
        }
        (Element::Chart { kind, categories, series, .. }, Element::Chart { kind: next_kind, categories: next_categories, series: next_series, .. }) => {
            if kind != next_kind || categories != next_categories || !equal(series, next_series)? {
                let chart = node.descendants().find(|node| node.has_tag_name((crate::native::C, "chart"))).ok_or_else(|| Error::Invalid("chart reference missing".into()))?;
                let attribute = chart.attributes().find(|attribute| attribute.namespace() == Some(crate::native::R) && attribute.name() == "id").ok_or_else(|| Error::Invalid("chart relationship missing".into()))?;
                let relation = resources.get(&format!("rIdShape{}", ids[new.bounds().0])).ok_or_else(|| Error::Invalid("new chart resource missing".into()))?;
                edits.push((attribute.range_value(), relation.clone()));
            }
        }
        (Element::Picture { base64, mime_type, alt, crop, .. }, Element::Picture { base64: next_base64, mime_type: next_mime, alt: next_alt, crop: next_crop, .. }) => {
            if base64 != next_base64 || mime_type != next_mime {
                let blip = child(node, P, "blipFill").and_then(|node| child(node, A, "blip")).ok_or_else(|| Error::Invalid("image reference missing".into()))?;
                let attribute = blip.attributes().find(|attribute| attribute.namespace() == Some(crate::native::R) && attribute.name() == "embed").ok_or_else(|| Error::Invalid("image relationship missing".into()))?;
                let relation = resources.get(&format!("rIdShape{}", ids[new.bounds().0])).ok_or_else(|| Error::Invalid("new picture resource missing".into()))?;
                edits.push((attribute.range_value(), relation.clone()));
            }
            if alt != next_alt { set_attribute(xml, properties(node).unwrap(), "descr", next_alt, edits)?; }
            if !equal(crop, next_crop)? { let fill = child(node, P, "blipFill").unwrap(); merge_child(xml, fill, &generated, child(next, P, "blipFill").unwrap(), A, "srcRect", &["blip", "srcRect", "tile", "stretch"], edits)?; }
        }
        (Element::Connector { color, stroke_width, arrow, start, end, flip_v, routing, .. }, Element::Connector { color: next_color, stroke_width: next_width, arrow: next_arrow, start: next_start, end: next_end, flip_v: next_flip, routing: next_routing, .. }) => {
            if !equal(routing, next_routing)? { return Err(Error::Unsupported("native connector path replacement requires graph metadata editing".into())); }
            let parent = child(node, P, "spPr").unwrap(); let next_parent = child(next, P, "spPr").unwrap();
            if color != next_color || stroke_width != next_width || arrow != next_arrow { merge_child(xml, parent, &generated, next_parent, A, "ln", &["xfrm", "prstGeom", "ln", "extLst"], edits)?; }
            if flip_v != next_flip && old_bounds == new_bounds { geometry(xml, node, &generated, next, old, new, scale, edits)?; }
            if !equal(&(start, end), &(next_start, next_end))? { let parent = child(node, P, "nvCxnSpPr").and_then(|node| child(node, P, "cNvCxnSpPr")).unwrap(); let next_parent = child(next, P, "nvCxnSpPr").and_then(|node| child(node, P, "cNvCxnSpPr")).unwrap(); for tag in ["stCxn", "endCxn"] { merge_child(xml, parent, &generated, next_parent, A, tag, &["cxnSpLocks", "stCxn", "endCxn", "extLst"], edits)?; } }
        }
        _ => {}
    }
    Ok(())
}

fn patch_elements(xml: &str, tree: Node<'_, '_>, before: &[Element], after: &[Element], ids: &BTreeMap<&str, usize>, theme: &crate::design::Theme, binding: &NativePart, resources: &BTreeMap<String, String>, scale: (f64, f64), edits: &mut Edits) -> Result<()> {
    let old_order: Vec<_> = before.iter().filter(|element| after.iter().any(|next| next.bounds().0 == element.bounds().0)).map(|element| element.bounds().0).collect();
    let new_order: Vec<_> = after.iter().filter(|element| before.iter().any(|old| old.bounds().0 == element.bounds().0)).map(|element| element.bounds().0).collect();
    if old_order != new_order { return Err(Error::Unsupported("reordering existing native objects is not supported yet".into())); }
    for old in before {
        let numeric = binding.nodes.get(old.bounds().0).ok_or_else(|| Error::Conflict("native object identity missing".into()))?;
        let node = tree.children().find(|node| is_shape(*node) && properties(*node).and_then(|node| node.attribute("id")) == Some(numeric)).ok_or_else(|| Error::Conflict("native object moved outside its container".into()))?;
        if let Some(new) = after.iter().find(|element| element.bounds().0 == old.bounds().0) { patch_element(xml, node, old, new, ids, theme, binding, resources, scale, edits)?; }
        else { edits.push((node.range(), String::new())); }
    }
    let mut inserted = String::new();
    for element in after.iter().filter(|element| !before.iter().any(|old| old.bounds().0 == element.bounds().0)) {
        inserted.push_str(&remap_resources(scaled_element(element, ids, theme, scale)?, resources)?);
    }
    if !inserted.is_empty() { insert_child(xml, tree, &inserted, child(tree, P, "extLst"), edits)?; }
    Ok(())
}

fn patch_part(package: &mut Package, binding: &NativePart, before: &[Element], after: &[Element], theme: &crate::design::Theme, name: Option<&str>, background: Option<Option<&str>>, scale: (f64, f64)) -> Result<()> {
    let xml = package.text(&binding.path)?.to_owned(); let document = parse(&xml)?;
    let common = child(document.root_element(), P, "cSld").ok_or_else(|| Error::Invalid("common slide data missing".into()))?;
    let tree = child(common, P, "spTree").ok_or_else(|| Error::Invalid("shape tree missing".into()))?;
    let mut maximum = binding.nodes.values().filter_map(|value| value.parse::<usize>().ok()).max().unwrap_or(1);
    let mut ids: BTreeMap<&str, usize> = binding.nodes.iter().map(|(id, value)| value.parse().map(|number| (id.as_str(), number)).map_err(|_| Error::Invalid("shape ID is not numeric".into()))).collect::<Result<_>>()?;
    for element in crate::model::element_list(after) { if !ids.contains_key(element.bounds().0) { maximum = maximum.checked_add(1).filter(|value| *value <= u32::MAX as usize).ok_or_else(|| Error::Limit("native shape IDs exhausted".into()))?; ids.insert(element.bounds().0, maximum); } }
    let resources = resource_changes(package, binding, before, after, &ids)?;
    let mut edits = Vec::new();
    if let Some(name) = name { set_attribute(&xml, common, "name", name, &mut edits)?; }
    if let Some(background) = background {
        let old = child(common, P, "bg");
        if let Some(shade) = background {
            let generated = crate::pptx::xml("p:bg", |writer| { writer.start_element("p:bgPr"); crate::pptx::color(writer, shade); crate::pptx::empty(writer, "a:effectLst", &[]); writer.end_element(); });
            let generated = String::from_utf8(generated).map_err(|_| Error::Invalid("background XML".into()))?;
            if let Some(old) = old { edits.push((old.range(), generated)); } else { insert_child(&xml, common, &generated, Some(tree), &mut edits)?; }
        } else if let Some(old) = old { edits.push((old.range(), String::new())); }
    }
    patch_elements(&xml, tree, before, after, &ids, theme, binding, &resources, scale, &mut edits)?;
    if !edits.is_empty() { package.replace_part(&binding.path, apply(xml, edits)?)?; }
    Ok(())
}

fn patch_theme(package: &mut Package, path: &str, before: &crate::design::Theme, after: &crate::design::Theme) -> Result<()> {
    if equal(before, after)? { return Ok(()); }
    let xml = package.text(path)?.to_owned(); let document = parse(&xml)?; let root = document.root_element(); let mut edits = Vec::new();
    if before.name != after.name { set_attribute(&xml, root, "name", &after.name, &mut edits)?; }
    let elements = child(root, A, "themeElements").ok_or_else(|| Error::Unsupported("theme elements missing".into()))?;
    let scheme = child(elements, A, "clrScheme").ok_or_else(|| Error::Unsupported("theme colors missing".into()))?;
    for key in crate::design::COLOR_KEYS {
        if before.colors[key] != after.colors[key] {
            let parent = child(scheme, A, key).ok_or_else(|| Error::Unsupported("theme color missing".into()))?;
            let entry = parent.children().find(|node| node.is_element()).ok_or_else(|| Error::Unsupported("theme color missing".into()))?;
            edits.push((entry.range(), format!("<a:srgbClr xmlns:a=\"{A}\" val=\"{}\"/>", after.colors[key])));
        }
    }
    if !equal(&before.fonts, &after.fonts)? {
        let scheme = child(elements, A, "fontScheme").ok_or_else(|| Error::Unsupported("theme font scheme missing".into()))?;
        for (kind, latin) in [("majorFont", &after.fonts.major), ("minorFont", &after.fonts.minor)] {
            let group = child(scheme, A, kind).ok_or_else(|| Error::Unsupported("theme font collection missing".into()))?;
            for (tag, value) in [("latin", latin), ("ea", &after.fonts.east_asian), ("cs", &after.fonts.complex_script)] { let font = child(group, A, tag).ok_or_else(|| Error::Unsupported("theme font missing".into()))?; set_attribute(&xml, font, "typeface", value, &mut edits)?; }
        }
    }
    package.replace_part(path, apply(xml, edits)?)
}

fn rename_presentation(package: &mut Package, title: &str) -> Result<()> {
    let paths: Vec<_> = package.parts().keys().filter(|path| path.starts_with("docProps/") && path.ends_with(".xml")).cloned().collect();
    for path in paths {
        let xml = package.text(&path)?.to_owned(); let parsed = parse(&xml)?;
        if let Some(node) = parsed.descendants().find(|node| node.has_tag_name(("http://purl.org/dc/elements/1.1/", "title"))) {
            if node.children().any(|child| child.is_element()) { return Err(Error::Unsupported("complex native title metadata".into())); }
            let mut edits = Vec::new(); let mut written = false;
            for text in node.children().filter(|child| child.is_text()) { edits.push((text.range(), if written { String::new() } else { written = true; quick_xml::escape::escape(title).into_owned() })); }
            if !written { insert_child(&xml, node, &quick_xml::escape::escape(title), None, &mut edits)?; }
            return package.replace_part(&path, apply(xml, edits)?);
        }
    }
    Err(Error::Unsupported("native title metadata is missing".into()))
}

pub(crate) fn save(package: &mut Package, original: &NativeDeck, deck: &Deck) -> Result<Vec<u8>> {
    let old = original.deck.design.as_ref().ok_or_else(|| Error::Unsupported("native design missing".into()))?;
    let design = deck.design.as_ref().ok_or_else(|| Error::Unsupported("native design cannot be detached".into()))?;
    if old.masters.len() != design.masters.len() || old.layouts.len() != design.layouts.len() { return Err(Error::Unsupported("changing native master/layout counts is not supported yet".into())); }
    let mut themes = BTreeSet::new();
    for path in &original.themes { if themes.insert(path) { patch_theme(package, path, &old.theme, &design.theme)?; } }
    let roots = crate::pptx::relationship_targets(package, "", "officeDocument")?;
    let main = roots.values().next().ok_or_else(|| Error::Invalid("presentation missing".into()))?;
    let presentation = parse(package.text(main)?)?;
    let size = child(presentation.root_element(), P, "sldSz").ok_or_else(|| Error::Invalid("slide size missing".into()))?;
    let scale = (size.attribute("cx").and_then(|value| value.parse::<f64>().ok()).unwrap_or(12192000.0) / 1280.0, size.attribute("cy").and_then(|value| value.parse::<f64>().ok()).unwrap_or(6858000.0) / 720.0);
    if deck.title != original.deck.title { rename_presentation(package, &deck.title)?; }
    for ((before, after), binding) in old.masters.iter().zip(&design.masters).zip(&original.masters) {
        if before.id != after.id || after.id != binding.id { return Err(Error::Unsupported("native master identity changed".into())); }
        patch_part(package, binding, &before.elements, &after.elements, &design.theme, (before.name != after.name).then_some(after.name.as_str()), (before.background != after.background).then_some(Some(after.background.as_str())), scale)?;
    }
    for ((before, after), binding) in old.layouts.iter().zip(&design.layouts).zip(&original.layouts) {
        if before.id != after.id || before.master_id != after.master_id { return Err(Error::Unsupported("native layout owner or identity changed".into())); }
        patch_part(package, binding, &before.elements, &after.elements, &design.theme, (before.name != after.name).then_some(after.name.as_str()), (before.background != after.background).then_some(after.background.as_deref()), scale)?;
    }
    let bindings = crate::native_slides::prepare(package, original, deck, main, scale)?;
    for (after, binding) in deck.slides.iter().zip(&bindings) {
        let source = after.native_source_id.as_deref().unwrap_or(&after.id);
        let Some(before) = original.deck.slides.iter().find(|slide| slide.id == source) else { continue };
        if before.layout_id != after.layout_id {
            let path = after.layout_id.as_ref().and_then(|id| original.layouts.iter().find(|part| &part.id == id)).map(|part| &part.path).ok_or_else(|| Error::Unsupported("native slide layout missing".into()))?;
            let relation_path = relations_path(&binding.path); let xml = package.text(&relation_path)?.to_owned(); let document = parse(&xml)?;
            let relation = document.root_element().children().find(|node| node.attribute("Type") == Some(format!("{}/slideLayout", crate::native::R).as_str())).ok_or_else(|| Error::Invalid("slide layout relationship missing".into()))?;
            let mut edits = Vec::new(); set_attribute(&xml, relation, "Target", &format!("/{path}"), &mut edits)?; package.replace_part(&relation_path, apply(xml, edits)?)?;
        }
        if before.notes != after.notes { return Err(Error::Unsupported("native notes editing is not supported yet".into())); }
        let background = (before.background != after.background || before.inherit_background != after.inherit_background).then_some(if after.inherit_background { None } else { Some(after.background.as_str()) });
        patch_part(package, binding, &before.elements, &after.elements, &design.theme, (before.title != after.title).then_some(after.title.as_str()), background, scale)?;
        if before.hide_master_graphics != after.hide_master_graphics {
            let xml = package.text(&binding.path)?.to_owned(); let document = parse(&xml)?; let mut edits = Vec::new(); set_attribute(&xml, document.root_element(), "showMasterSp", if after.hide_master_graphics { "0" } else { "1" }, &mut edits)?; package.replace_part(&binding.path, apply(xml, edits)?)?;
        }
    }
    package.save()
}