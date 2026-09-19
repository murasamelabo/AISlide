use crate::{model::{Deck, Element}, native::{A, P, NativeDeck, NativePart, child, is_shape, properties, relations_path}, package::Package, pptx::parse, Error, Result};
use roxmltree::Node;
use std::{collections::{BTreeMap, BTreeSet}, ops::Range};

type Edits = Vec<(Range<usize>, String)>;

fn equal(left: &impl serde::Serialize, right: &impl serde::Serialize) -> Result<bool> { Ok(crate::canonical::bytes(left)? == crate::canonical::bytes(right)?) }

pub(crate) fn check_duplicate(document: &crate::document::Document, slide_id: &str, element: &Element) -> Result<()> {
    let Some(origin) = document.origin.as_ref().filter(|origin| origin.native) else { return Ok(()) };
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(&origin.base64).map_err(|_| Error::Invalid("origin base64".into()))?;
    let package = Package::open(crate::native::save(bytes, &document.deck)?)?;
    let native = crate::native::read(&package)?;
    let binding = document.deck.slides.iter().position(|slide| slide.id == slide_id).and_then(|index| native.slides.get(index))
        .or_else(|| native.slides.iter().find(|part| part.id == slide_id)).ok_or_else(|| Error::Unsupported("native individual copy source cannot be verified".into()))?;
    let numeric = binding.nodes.get(element.bounds().0).ok_or_else(|| Error::Unsupported("native individual copy identity cannot be verified".into()))?;
    let xml = package.text(&binding.path)?; let parsed = parse(xml)?;
    let node = parsed.descendants().find(|node| is_shape(*node) && properties(*node).and_then(|node| node.attribute("id")) == Some(numeric)).ok_or_else(|| Error::Invalid("native element missing".into()))?;
    let ids = binding.nodes.iter().map(|(id, numeric)| Ok((id.as_str(), numeric.parse::<usize>().map_err(|_| Error::Invalid("native shape identity".into()))?))).collect::<Result<BTreeMap<_, _>>>()?;
    let fallback = crate::design::Theme::default();
    let generated = crate::pptx::element_xml(element, ids[element.bounds().0], &ids, document.deck.design.as_ref().map(|design| &design.theme).unwrap_or(&fallback));
    fn copy_value(node: Node<'_, '_>) -> serde_json::Value {
        let attributes: BTreeMap<_, _> = node.attributes().map(|attribute| (format!("{}:{}", attribute.namespace().unwrap_or(""), attribute.name()), if attribute.namespace() == Some(crate::native::R) { "resource" } else { attribute.value() })).collect();
        let children: Vec<_> = node.children().filter_map(|node| {
            if node.is_element() { Some(copy_value(node)) }
            else if node.is_text() && !node.text().unwrap_or("").trim().is_empty() { Some(serde_json::json!(node.text())) }
            else { None }
        }).collect();
        serde_json::json!({"namespace":node.tag_name().namespace(),"name":node.tag_name().name(),"attributes":attributes,"children":children})
    }
    if copy_value(node) != copy_value(parse(&generated)?.root_element()) { return Err(Error::Unsupported("native object has formatting not represented by an individual copy; duplicate its slide instead".into())); }
    for descendant in crate::model::element_list(std::slice::from_ref(element)) {
        if let Element::Chart { id, kind, categories, series, options, .. } = descendant {
            if !chart_is_representable(&package, binding, id, *kind, categories, series, options)? { return Err(Error::Unsupported("native chart or workbook has content not represented by an individual copy; duplicate its slide instead".into())); }
        }
    }
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

fn chart_is_representable(package: &Package, binding: &NativePart, id: &str, kind: crate::model::ChartKind, categories: &[String], series: &[crate::model::ChartSeries], options: &crate::model::ChartOptions) -> Result<bool> {
    let document = parse(package.text(&binding.path)?)?;
    let numeric = binding.nodes.get(id).ok_or_else(|| Error::Invalid("native chart identity missing".into()))?;
    let node = document.descendants().find(|node| is_shape(*node) && properties(*node).and_then(|node| node.attribute("id")) == Some(numeric)).ok_or_else(|| Error::Invalid("native chart missing".into()))?;
    let namespace = if kind.is_extended() { crate::chart_extended::NS } else { crate::native::C };
    let relation = node.descendants().find(|node| node.has_tag_name((namespace, "chart"))).and_then(|node| node.attribute((crate::native::R, "id"))).ok_or_else(|| Error::Invalid("native chart relationship missing".into()))?;
    let targets = crate::pptx::relationship_targets(package, &binding.path, if kind.is_extended() { "chartEx" } else { "chart" })?;
    let path = targets.get(relation).ok_or_else(|| Error::Invalid("native chart target missing".into()))?;
    if kind.is_extended() {
        let relationships = parse(package.text(&relations_path(path))?)?;
        let entries: Vec<_> = relationships.root_element().children().filter(|node| node.is_element()).collect();
        if entries.len() != 3 || entries.iter().any(|node| !node.has_tag_name((crate::native::REL, "Relationship")) || node.attributes().any(|attribute| attribute.namespace().is_some() || !["Id", "Type", "Target", "TargetMode"].contains(&attribute.name()))) { return Ok(false); }
        for (relation, bytes) in [("chartStyle", crate::chart_extended::style()), ("chartColorStyle", crate::chart_extended::color_style())] {
            let targets = crate::pptx::relationship_targets(package, path, relation)?;
            if targets.len() != 1 { return Ok(false); }
            let style_path = targets.values().next().ok_or_else(|| Error::Invalid("chartEx style target missing".into()))?;
            if package.part_names().contains(relations_path(style_path).as_str()) { return Ok(false); }
            let actual = parse(package.text(style_path)?)?;
            let expected = parse(std::str::from_utf8(&bytes).map_err(|_| Error::Invalid("chartEx style encoding".into()))?)?;
            if xml_value(actual.root_element()) != xml_value(expected.root_element()) { return Ok(false); }
        }
        if crate::pptx::relationship_targets(package, path, "package")?.len() != 1 { return Ok(false); }
    }
    let actual = parse(package.text(path)?)?;
    let expected = crate::charts::chart(kind, categories, series, options);
    let expected = parse(std::str::from_utf8(&expected).map_err(|_| Error::Invalid("chart XML encoding".into()))?)?;
    if xml_value(actual.root_element()) != xml_value(expected.root_element()) {
        use crate::model::chart_format::{automatic_label_position, LabelPosition};
        let legacy = if kind == crate::model::ChartKind::Sunburst {
            crate::chart_extended::legacy_sunburst_chart(categories, series, options)
        } else {
            if options.data_labels.as_ref().and_then(|labels| labels.position) != Some(LabelPosition::Center) || !(automatic_label_position(kind) || series.iter().any(|entry| entry.kind.is_some_and(automatic_label_position))) { return Ok(false); }
            crate::charts::legacy_label_position_chart(kind, categories, series, options)
        };
        let legacy = parse(std::str::from_utf8(&legacy).map_err(|_| Error::Invalid("chart XML encoding".into()))?)?;
        if xml_value(actual.root_element()) != xml_value(legacy.root_element()) { return Ok(false); }
    }
    let Some(relation) = actual.descendants().find(|node| node.has_tag_name((namespace, "externalData"))).and_then(|node| node.attribute((crate::native::R, "id"))) else { return Ok(false); };
    let targets = crate::pptx::relationship_targets(package, path, "package")?;
    let Some(workbook_path) = targets.get(relation) else { return Ok(false); };
    let actual_workbook = Package::open(package.part(workbook_path)?.to_vec())?;
    let expected_workbook = Package::open(crate::charts::workbook(kind, categories, series, options)?)?;
    Ok(actual_workbook.parts() == expected_workbook.parts())
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
            (Some(Element::Chart { kind: old_kind, categories: old_categories, series: old_series, options: old_options, .. }), Element::Chart { kind, categories, series, options, .. }) => {
                if old_kind.is_extended() != kind.is_extended() { return Err(Error::Unsupported("native chart/chartEx conversion requires a new chart object".into())); }
                let changed = old_kind != kind || old_categories != categories || !equal(old_series, series)? || !equal(old_options, options)?;
                if changed && !chart_is_representable(package, binding, element.bounds().0, *old_kind, old_categories, old_series, old_options)? { return Err(Error::Unsupported("custom native chart settings or workbook changes are preserved; data/style replacement is not safely representable".into())); }
                changed
            }
            (Some(Element::Picture { base64: old_data, mime_type: old_mime, svg: old_svg, .. }), Element::Picture { base64, mime_type, svg, .. }) => old_data != base64 || old_mime != mime_type || old_svg != svg,
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
    let relationships = crate::pptx::resource_parts(&mut parts, &resources, &mut chart_number, &mut picture_number, &mut types, Some(ids), &mut crate::pptx::MediaResources::new())?;
    for (path, data) in parts { package.add_part(path, data)?; }
    let relation_path = relations_path(&binding.path);
    let xml = if package.part_names().contains(relation_path.as_str()) { package.text(&relation_path)?.to_owned() } else { format!("<Relationships xmlns=\"{}\"/>", crate::native::REL) };
    let parsed = parse(&xml)?;
    let mut taken: BTreeSet<String> = parsed.root_element().children().filter_map(|node| node.attribute("Id").map(str::to_owned)).collect();
    let mut counter = 1; let mut mapping = BTreeMap::new(); let mut extra = String::new();
    for (old_id, kind, target) in relationships {
        let id = loop { let candidate = format!("rIdAislideResource{counter}"); counter += 1; if taken.insert(candidate.clone()) { break candidate; } };
        let target = if kind == "hyperlink" { target } else { format!("/ppt/{}", target.strip_prefix("../").ok_or_else(|| Error::Invalid("generated resource target".into()))?) };
        extra.push_str(&format!("<Relationship xmlns=\"{}\" Id=\"{id}\" Type=\"{}\" Target=\"{}\"{}/>", crate::native::REL, crate::pptx::relationship_type(kind), quick_xml::escape::escape(&target), if kind == "hyperlink" { " TargetMode=\"External\"" } else { "" }));
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

pub(crate) fn fragment(xml: &str, node: Node<'_, '_>) -> String {
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

fn edited_fragment(xml: &str, node: Node<'_, '_>, mut edits: Edits) -> Result<String> {
    edits.sort_by_key(|(range, _)| range.start);
    if edits.windows(2).any(|pair| pair[0].0.end > pair[1].0.start) { return Err(Error::Conflict("overlapping native fragment edits".into())); }
    let range = node.range();
    let mut value = xml[range.clone()].to_owned();
    for (edited, replacement) in edits.into_iter().rev() {
        if edited.start < range.start || edited.end > range.end { return Err(Error::Conflict("edit outside its native container".into())); }
        value.replace_range(edited.start-range.start..edited.end-range.start, &replacement);
    }
    Ok(value)
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

fn supports_rich_style(body: Node<'_, '_>) -> bool {
    use crate::native::R;
    for node in body.descendants().filter(|node| node.is_element()) {
        if node != body && node.tag_name().namespace() != Some(A) { return false; }
        let (attributes, children): (&[&str], &[&str]) = match node.tag_name().name() {
            "txBody" => (&[], &["bodyPr", "lstStyle", "p"]),
            "bodyPr" => (&["lIns", "rIns", "tIns", "bIns", "wrap", "anchor"], &["noAutofit"]),
            "lstStyle" => (&[], &["defPPr", "lvl1pPr", "lvl2pPr", "lvl3pPr", "lvl4pPr", "lvl5pPr", "lvl6pPr", "lvl7pPr", "lvl8pPr", "lvl9pPr"]),
            "p" => (&[], &["pPr", "r", "endParaRPr"]),
            "pPr" | "lvl1pPr" | "lvl2pPr" | "lvl3pPr" | "lvl4pPr" | "lvl5pPr" | "lvl6pPr" | "lvl7pPr" | "lvl8pPr" | "lvl9pPr" => (&["algn", "marL", "indent", "lvl"], &["lnSpc", "spcBef", "spcAft", "buNone", "buChar", "buAutoNum", "tabLst", "defRPr"]),
            "r" => (&[], &["rPr", "t"]),
            "rPr" | "defRPr" | "endParaRPr" => (&["lang", "sz", "b", "i", "u", "baseline", "dirty"], &["solidFill", "highlight", "latin", "ea", "cs", "hlinkClick"]),
            "solidFill" | "highlight" => (&[], &["srgbClr", "schemeClr"]),
            "lnSpc" | "spcBef" | "spcAft" => (&[], &["spcPct", "spcPts"]),
            "tabLst" => (&[], &["tab"]),
            "tab" => (&["pos", "algn"], &[]),
            "buChar" => (&["char"], &[]), "buAutoNum" => (&["type", "startAt"], &[]),
            "latin" | "ea" | "cs" => (&["typeface"], &[]),
            "spcPct" | "spcPts" | "srgbClr" | "schemeClr" => (&["val"], &[]),
            "hlinkClick" => (&["id"], &[]), "t" => (&["space"], &[]),
            "noAutofit" | "buNone" => (&[], &[]),
            _ => return false,
        };
        if node.attributes().any(|attribute| !attributes.contains(&attribute.name()) || attribute.namespace().is_some_and(|namespace| !(node.has_tag_name((A, "hlinkClick")) && attribute.name() == "id" && namespace == R) && !(node.has_tag_name((A, "t")) && attribute.name() == "space" && namespace == "http://www.w3.org/XML/1998/namespace"))) { return false; }
        let mut seen = BTreeSet::new();
        for child in node.children().filter(|node| node.is_element()) {
            let name = child.tag_name().name();
            if !children.contains(&name) || (!matches!(name, "p" | "r" | "tab") && !seen.insert(name)) { return false; }
        }
        if node.has_tag_name((A, "schemeClr")) && !node.attribute("val").is_some_and(|value| crate::design::COLOR_KEYS.contains(&value)) { return false; }
        if matches!(node.tag_name().name(), "rPr" | "defRPr" | "endParaRPr") {
            if node.attribute("u").is_some_and(|value| !["none", "sng"].contains(&value)) { return false; }
            let latin = child(node, A, "latin").and_then(|node| node.attribute("typeface"));
            if node.tag_name().name() != "rPr" && child(node, A, "hlinkClick").is_some() { return false; }
            let font_count = ["latin", "ea", "cs"].iter().filter(|tag| child(node, A, tag).is_some()).count();
            if font_count != 0 && font_count != 3 { return false; }
            for (tag, major, minor) in [("ea", "+mj-ea", "+mn-ea"), ("cs", "+mj-cs", "+mn-cs")] {
                if let Some(family) = child(node, A, tag).and_then(|node| node.attribute("typeface")) {
                    if Some(family) != latin && !matches!((latin, family), (Some("+mj-lt"), value) if value == major) && !matches!((latin, family), (Some("+mn-lt"), value) if value == minor) { return false; }
                }
            }
        }
    }
    let links: Vec<_> = body.descendants().filter(|node| node.has_tag_name((A, "r"))).map(|node| child(node, A, "rPr").and_then(|node| child(node, A, "hlinkClick")).and_then(|node| node.attribute((R, "id")))).collect();
    links.windows(2).all(|pair| pair[0] == pair[1])
}

fn replace_rich_paragraphs(xml: &str, body: Node<'_, '_>, generated: &str, next_body: Node<'_, '_>, edits: &mut Edits) -> Result<()> {
    if !supports_rich_style(body) { return Err(Error::Unsupported("unrepresentable native text styles or fields are preserved; rich text edit rejected".into())); }
    crate::native::read_rich_paragraphs(body, &crate::rich_text::RunStyle::default(), &crate::model::TextFormat::default())?;
    let original: Vec<_> = body.children().filter(|node| node.has_tag_name((A, "p"))).collect();
    let next: Vec<_> = next_body.children().filter(|node| node.has_tag_name((A, "p"))).collect();
    for (index, paragraph) in original.iter().enumerate() {
        let replacement = if let Some(next) = next.get(index) {
            let mut replacement = fragment(generated, *next);
            if let Some(end) = child(*paragraph, A, "endParaRPr") {
                let parsed = parse(&replacement)?;
                let generated_end = child(parsed.root_element(), A, "endParaRPr").map(|node| node.range());
                if let Some(range) = generated_end { replacement.replace_range(range, &fragment(xml, end)); }
            }
            replacement
        } else { String::new() };
        edits.push((paragraph.range(), replacement));
    }
    if next.len() > original.len() {
        let position = original.last().map(|node| node.range().end).unwrap_or_else(|| body.range().start + xml[body.range()].rfind("</").unwrap_or(0));
        edits.push((position..position, next[original.len()..].iter().map(|node| fragment(generated, *node)).collect::<String>()));
    }
    Ok(())
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
            let (_, old_x, old_y, _, _) = old.bounds();
            let original = |name| offset.attribute(name).and_then(|value| value.parse::<f64>().ok()).filter(|value| value.is_finite()).ok_or_else(|| Error::Unsupported("native origin coordinate".into()));
            for (node, name, value) in [(offset, "x", original("x")? + (x-old_x) * scale.0), (offset, "y", original("y")? + (y-old_y) * scale.1), (extent, "cx", width * scale.0), (extent, "cy", height * scale.1)] { set_attribute(xml, node, name, &value.round().to_string(), edits)?; }
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
    crate::rich_text::validate_element(new)?;
    let (old_text, old_size, old_color, old_bold, old_format) = match old { Element::Text { text, font_size, color, bold, format, .. } | Element::Shape { text, font_size, color, bold, format, .. } => (text, font_size, color, bold, format), _ => return Ok(()) };
    let (text, size, color, bold, format) = match new { Element::Text { text, font_size, color, bold, format, .. } | Element::Shape { text, font_size, color, bold, format, .. } => (text, font_size, color, bold, format), _ => return Err(Error::Unsupported("native object type change".into())) };
    let style_changed = !equal(&(old_size, old_color, old_bold, old_format), &(size, color, bold, format))?;
    let rich = !old_format.paragraphs.is_empty() || !format.paragraphs.is_empty();
    if old_text == text && (!style_changed || (!rich && old_format.inherit_layout && format.inherit_layout)) { return Ok(()); }
    let body = child(node, P, "txBody").ok_or_else(|| Error::Unsupported("text body missing".into()))?;
    if rich {
        if format.paragraphs.is_empty() { return Err(Error::Unsupported("rich native text must be edited through the rich text API".into())); }
        if !equal(&(old_size, old_color, old_bold, old_format.italic, old_format.underline, &old_format.font_family, old_format.alignment, old_format.bullet), &(size, color, bold, format.italic, format.underline, &format.font_family, format.alignment, format.bullet))? {
            return Err(Error::Unsupported("rich text has explicit run and paragraph styles; use the rich range or paragraph API instead of frame-style replacement".into()));
        }
        let next_body = child(next, P, "txBody").ok_or_else(|| Error::Invalid("generated text body missing".into()))?;
        replace_rich_paragraphs(xml, body, generated, next_body, edits)?;
        if old_format.vertical != format.vertical {
            let properties = child(body, A, "bodyPr").ok_or_else(|| Error::Unsupported("native body properties missing".into()))?;
            set_attribute(xml, properties, "anchor", match format.vertical { crate::model::VerticalAlign::Top => "t", crate::model::VerticalAlign::Middle => "ctr", crate::model::VerticalAlign::Bottom => "b" }, edits)?;
        }
        return Ok(());
    }
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
    if crate::fields::patch_cache_only(xml, node, old, new, edits)? { return Ok(()); }
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
    crate::visual::patch(xml, node, &generated, next, previous, old, new, edits)?;
    match (old, new) {
        (Element::Group { children: old, view_width, view_height, .. }, Element::Group { children: new, view_width: next_width, view_height: next_height, .. }) => {
            if view_width != next_width || view_height != next_height { return Err(Error::Unsupported("changing a native group coordinate system".into())); }
            patch_elements(xml, node, old, new, ids, theme, binding, resources, (9525.0, 9525.0), edits)?;
        }
        (Element::Table { rows, font_size, format, .. }, Element::Table { rows: next_rows, font_size: next_size, format: next_format, .. }) => {
            let table = node.descendants().find(|node| node.has_tag_name((A, "tbl"))).ok_or_else(|| Error::Unsupported("table missing".into()))?;
            let expected = previous.descendants().find(|node| node.has_tag_name((A, "tbl"))).ok_or_else(|| Error::Invalid("previous table missing".into()))?;
            let generated_table = next.descendants().find(|node| node.has_tag_name((A, "tbl"))).ok_or_else(|| Error::Invalid("generated table missing".into()))?;
            if rows.len() != next_rows.len() || rows[0].len() != next_rows[0].len() {
                if xml_value(table) != xml_value(expected) { return Err(Error::Unsupported("custom native table formatting is preserved; structural replacement is not safely representable".into())); }
                edits.push((table.range(), fragment(&generated, generated_table)));
            } else {
                fn cells<'node, 'input>(table: Node<'node, 'input>) -> Vec<Node<'node, 'input>> { table.children().filter(|node| node.has_tag_name((A,"tr"))).flat_map(|row| row.children().filter(|cell| cell.has_tag_name((A,"tc")))).collect() }
                let actual_cells = cells(table); let expected_cells = cells(expected); let generated_cells = cells(generated_table);
                if actual_cells.len() != rows.len() * rows[0].len() || expected_cells.len() != actual_cells.len() || generated_cells.len() != actual_cells.len() { return Err(Error::Unsupported("native table grid mismatch".into())); }
                for (index, ((before, after), cell)) in rows.iter().flatten().zip(next_rows.iter().flatten()).zip(actual_cells).enumerate() {
                    let row = index / rows[0].len(); let column = index % rows[0].len();
                    let old_style = format.cell_style(row,column); let style = next_format.cell_style(row,column);
                    let next_cell = generated_cells[index]; let previous_cell = expected_cells[index];
                    let body = child(cell, A, "txBody").ok_or_else(|| Error::Unsupported("table cell text missing".into()))?;
                    let rich = [&old_style.text_format,&style.text_format].iter().any(|format| format.as_ref().is_some_and(|format| !format.paragraphs.is_empty()));
                    let style_changed = old_style.text_style != style.text_style || old_style.text_format != style.text_format || font_size != next_size;
                    if style_changed || (before != after && rich) {
                        if !supports_rich_style(body) { return Err(Error::Unsupported("unrepresentable native cell body is preserved; rich text edit rejected".into())); }
                        if body.descendants().any(|node| node.has_tag_name((A,"hlinkClick")) || node.has_tag_name((A,"hlinkMouseOver"))) { return Err(Error::Unsupported("native cell hyperlinks are preserved; text style replacement is unsupported".into())); }
                        let next_body = child(next_cell,A,"txBody").ok_or_else(|| Error::Invalid("generated table text missing".into()))?;
                        crate::table_format::patch_paragraphs(xml,body,&generated,next_body,edits)?;
                    } else if before != after { replace_text(xml,body,after,edits)?; }
                    if format.merges != next_format.merges {
                        let before_flags = crate::table_format::merge_flags(format,row,column);
                        let after_flags = crate::table_format::merge_flags(next_format,row,column);
                        if before_flags != after_flags {
                            if !supports_rich_style(body) { return Err(Error::Unsupported("complex cell body is preserved; merge change rejected".into())); }
                            for name in ["gridSpan","rowSpan","hMerge","vMerge"] {
                                match (cell.attribute(name),next_cell.attribute(name)) {
                                    (Some(old),Some(new)) if old != new => set_attribute(xml,cell,name,new,edits)?,
                                    (None,Some(new)) => set_attribute(xml,cell,name,new,edits)?,
                                    (Some(_),None) => if let Some(attribute) = cell.attributes().find(|attribute| attribute.namespace().is_none() && attribute.name() == name) { edits.push((attribute.range(),String::new())); },
                                    _ => {}
                                }
                            }
                        }
                    }
                    if old_style.fill != style.fill || old_style.outline != style.outline || old_style.padding != style.padding || old_style.vertical != style.vertical {
                        let mut property_edits = Vec::new();
                        let properties = child(cell,A,"tcPr").ok_or_else(|| Error::Unsupported("native cell properties missing".into()))?;
                        let next_properties = child(next_cell,A,"tcPr").ok_or_else(|| Error::Invalid("generated cell properties missing".into()))?;
                        let previous_properties = child(previous_cell,A,"tcPr").ok_or_else(|| Error::Invalid("previous cell properties missing".into()))?;
                        for (changed,names) in [(old_style.padding != style.padding,&["marL","marR","marT","marB"][..]),(old_style.vertical != style.vertical,&["anchor"][..])] {
                            if changed { for name in names {
                                let value = next_properties.attribute(*name).unwrap_or("t");
                                if properties.attribute(*name) != Some(value) { set_attribute(xml,properties,name,value,&mut property_edits)?; }
                            } }
                        }
                        let order = ["lnL","lnR","lnT","lnB","lnTlToBr","lnBlToTr","cell3D","noFill","solidFill","gradFill","blipFill","pattFill","grpFill","headers","extLst"];
                        for (changed,names) in [(old_style.outline != style.outline,&["lnL","lnR","lnT","lnB"][..]),(old_style.fill != style.fill,&["noFill","solidFill","gradFill","blipFill","pattFill","grpFill"][..])] {
                            if changed {
                                for name in names {
                                    if child(properties,A,name).map(xml_value) != child(previous_properties,A,name).map(xml_value) { return Err(Error::Unsupported("custom native cell fill or outline is preserved; replacement is not safely representable".into())); }
                                }
                                for name in names { merge_child(xml,properties,&generated,next_properties,A,name,&order,&mut property_edits)?; }
                            }
                        }
                        property_edits.sort_by_key(|(range,_)| (range.start,range.end));
                        edits.extend(property_edits);
                    }
                }
                for (changed,tag,attribute) in [(old_bounds.3 != new_bounds.3 || format.column_widths != next_format.column_widths,"gridCol","w"),(old_bounds.4 != new_bounds.4 || format.row_heights != next_format.row_heights,"tr","h")] {
                    if changed {
                        let nodes: Vec<_> = table.descendants().filter(|node| node.has_tag_name((A,tag))).collect();
                        let next_nodes: Vec<_> = generated_table.descendants().filter(|node| node.has_tag_name((A,tag))).collect();
                        if nodes.len() != next_nodes.len() { return Err(Error::Unsupported("native table dimension count mismatch".into())); }
                        for (node,next_node) in nodes.into_iter().zip(next_nodes) { set_attribute(xml,node,attribute,next_node.attribute(attribute).ok_or_else(|| Error::Invalid("generated table dimension missing".into()))?,edits)?; }
                    }
                }
            }
        }
        (Element::Chart { kind, categories, series, options, .. }, Element::Chart { kind: next_kind, categories: next_categories, series: next_series, options: next_options, .. }) => {
            if kind != next_kind || categories != next_categories || !equal(series, next_series)? || !equal(options, next_options)? {
                let namespace = if kind.is_extended() { crate::chart_extended::NS } else { crate::native::C };
                let chart = node.descendants().find(|node| node.has_tag_name((namespace, "chart"))).ok_or_else(|| Error::Invalid("chart reference missing".into()))?;
                let attribute = chart.attributes().find(|attribute| attribute.namespace() == Some(crate::native::R) && attribute.name() == "id").ok_or_else(|| Error::Invalid("chart relationship missing".into()))?;
                let relation = resources.get(&format!("rIdShape{}", ids[new.bounds().0])).ok_or_else(|| Error::Invalid("new chart resource missing".into()))?;
                edits.push((attribute.range_value(), relation.clone()));
            }
        }
        (Element::Picture { alt, crop, .. }, Element::Picture { alt: next_alt, crop: next_crop, .. }) => {
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

fn selection_parents<'element>(elements: &'element [Element], parent: Option<&'element str>, result: &mut BTreeMap<&'element str, Option<&'element str>>) {
    for element in elements {
        result.insert(element.bounds().0, parent);
        if let Element::Group { id, children, .. } = element { selection_parents(children, Some(id), result); }
    }
}

fn selection_fragment(xml: &str, node: Node<'_, '_>, edits: Edits) -> Result<String> {
    let mut value = edited_fragment(xml, node, edits)?;
    let end = start_end(xml, node)?;
    let header = &xml[node.range().start..=end];
    let mut reader = quick_xml::Reader::from_str(header);
    let declarations: BTreeSet<_> = match reader.read_event().map_err(|_| Error::Invalid("native selection start tag".into()))? {
        quick_xml::events::Event::Start(start) | quick_xml::events::Event::Empty(start) => start.attributes().map(|attribute| attribute.map(|attribute| attribute.key.as_ref().to_vec()).map_err(|_| Error::Invalid("native namespace declaration".into()))).collect::<Result<_>>()?,
        _ => return Err(Error::Invalid("native selection start tag missing".into())),
    };
    let mut namespaces = String::new();
    for namespace in node.namespaces() {
        let name = namespace.name().map(|prefix| format!("xmlns:{prefix}")).unwrap_or_else(|| "xmlns".into());
        if !declarations.contains(name.as_bytes()) {
            namespaces.push_str(&format!(" {name}=\"{}\"", quick_xml::escape::escape(namespace.uri())));
        }
    }
    let position = value[1..].find([' ', '\t', '\r', '\n', '/', '>']).ok_or_else(|| Error::Invalid("native selection element name".into()))? + 1;
    value.insert_str(position, &namespaces);
    parse(&value)?;
    Ok(value)
}

fn selection_transform(xml: &str, node: Node<'_, '_>, element: &Element, scale: (f64, f64), normalize_group: bool, edits: &mut Edits) -> Result<()> {
    let Some(transform) = child(node, P, "xfrm").or_else(|| child(node, P, if matches!(element, Element::Group { .. }) { "grpSpPr" } else { "spPr" }).and_then(|parent| child(parent, A, "xfrm"))) else {
        if matches!(element, Element::Text { format, .. } if !format.inherit_layout) { return Ok(()); }
        return Err(Error::Unsupported("reparenting inherited native geometry requires an explicit transform".into()));
    };
    edits.retain(|(range, _)| range.end <= transform.range().start || range.start >= transform.range().end);
    let (_, x, y, width, height) = element.bounds();
    for (tag, horizontal, vertical, values) in [("off", "x", "y", (x * scale.0, y * scale.1)), ("ext", "cx", "cy", (width * scale.0, height * scale.1))] {
        let child = child(transform, A, tag).ok_or_else(|| Error::Unsupported("partial native selection transform".into()))?;
        set_attribute(xml, child, horizontal, &values.0.round().to_string(), edits)?;
        set_attribute(xml, child, vertical, &values.1.round().to_string(), edits)?;
    }
    if let (Element::Group { view_width, view_height, .. }, true) = (element, normalize_group) {
        for (tag, horizontal, vertical, values) in [("chOff", "x", "y", (0.0, 0.0)), ("chExt", "cx", "cy", (view_width * 9525.0, view_height * 9525.0))] {
            let child = child(transform, A, tag).ok_or_else(|| Error::Unsupported("partial native group canvas".into()))?;
            set_attribute(xml, child, horizontal, &values.0.round().to_string(), edits)?;
            set_attribute(xml, child, vertical, &values.1.round().to_string(), edits)?;
        }
    }
    Ok(())
}

fn selection_group_shell(element: &Element) -> Element {
    let mut shell = element.clone();
    if let Element::Group { children, .. } = &mut shell { children.clear(); }
    shell
}

fn selection_scaled_style(node: Node<'_, '_>, old: &Element, next: &Element, edits: &mut Edits) -> Result<bool> {
    if !matches!(old, Element::Text { .. } | Element::Shape { .. } | Element::Polygon { .. } | Element::Connector { .. }) { return Ok(false); }
    let (_, old_x, old_y, old_width, old_height) = old.bounds();
    let (_, x, y, width, height) = next.bounds();
    let scale = [width / old_width, height / old_height];
    if scale == [1.0, 1.0] { return Ok(false); }
    let mut expected = old.clone();
    crate::selection::transform(&mut expected, [old_x, old_y], [x, y], scale)?;
    if !equal(&expected, next)? { return Ok(false); }
    let scalar = scale[0].min(scale[1]);
    for current in node.descendants().filter(|current| current.tag_name().namespace() == Some(A) && !current.ancestors().take_while(|ancestor| *ancestor != node).any(|ancestor| ancestor.tag_name().name() == "extLst")) {
        if ["r", "fld"].contains(&current.tag_name().name()) && child(current, A, "rPr").and_then(|properties| properties.attribute("sz")).is_none() {
            return Err(Error::Unsupported("scaled native ungroup requires explicit run font sizes".into()));
        }
        for attribute in current.attributes().filter(|attribute| attribute.namespace().is_none()) {
            let factor = match (current.tag_name().name(), attribute.name()) {
                ("rPr" | "defRPr" | "endParaRPr", "sz" | "kern" | "spc") | ("spcPts", "val") | ("ln", "w") => scalar,
                ("pPr", "marL" | "marR" | "indent") | ("tab", "pos") | ("bodyPr", "lIns" | "rIns") => scale[0],
                ("bodyPr", "tIns" | "bIns") => scale[1],
                _ => continue,
            };
            let value = attribute.value().parse::<f64>().ok().filter(|value| value.is_finite()).ok_or_else(|| Error::Unsupported("native group style dimension".into()))? * factor;
            if !value.is_finite() || value.abs() > i32::MAX as f64 { return Err(Error::Limit("native group style dimension overflow".into())); }
            edits.push((attribute.range_value(), value.round().to_string()));
        }
    }
    Ok(true)
}

fn selection_connection_containers(tree: Node<'_, '_>, after: &[Element], originals: &BTreeMap<&str, (&Element, Node<'_, '_>)>, ids: &BTreeMap<&str, usize>) -> Result<()> {
    let mut parents = BTreeMap::new(); selection_parents(after, None, &mut parents);
    let remaining: BTreeSet<_> = parents.keys().copied().collect();
    let mut previous = BTreeMap::new();
    for node in tree.descendants().filter(|node| is_shape(*node)) {
        if let Some(numeric) = properties(node).and_then(|node| node.attribute("id")).and_then(|id| id.parse::<usize>().ok()) {
            let parent = node.parent().filter(|node| is_shape(*node)).and_then(properties).and_then(|node| node.attribute("id")).and_then(|id| id.parse::<usize>().ok());
            previous.insert(numeric, parent);
        }
    }
    let mut current = previous.clone();
    for id in originals.keys().filter(|id| !remaining.contains(**id)) { current.remove(&ids[*id]); }
    for (id, parent) in parents { current.insert(ids[id], parent.map(|parent| ids[parent])); }
    for connection in tree.descendants().filter(|node| node.has_tag_name((A, "stCxn")) || node.has_tag_name((A, "endCxn"))) {
        let owner = connection.ancestors().find(|node| is_shape(*node)).and_then(properties).and_then(|node| node.attribute("id")).and_then(|id| id.parse::<usize>().ok()).ok_or_else(|| Error::Unsupported("raw native connector owner missing".into()))?;
        let Some(owner_parent) = current.get(&owner) else { continue; };
        let target = connection.attribute("id").and_then(|id| id.parse::<usize>().ok()).ok_or_else(|| Error::Unsupported("raw native connector target missing".into()))?;
        if current.get(&owner) != previous.get(&owner) || current.get(&target) != previous.get(&target) {
            if current.get(&target) != Some(owner_parent) { return Err(Error::Unsupported("native connector reference would cross the new group container".into())); }
        }
    }
    Ok(())
}

struct NativeSelection<'context, 'node, 'input> {
    xml: &'input str,
    originals: BTreeMap<&'context str, (&'context Element, Node<'node, 'input>)>,
    ids: &'context BTreeMap<&'context str, usize>,
    theme: &'context crate::design::Theme,
    binding: &'context NativePart,
    resources: &'context BTreeMap<String, String>,
}

impl NativeSelection<'_, '_, '_> {
    fn render(&self, element: &Element, scale: (f64, f64)) -> Result<String> {
        let Some((old, node)) = self.originals.get(element.bounds().0) else {
            if let Element::Group { children, .. } = element {
                let shell = scaled_element(&selection_group_shell(element), self.ids, self.theme, scale)?;
                let parsed = parse(&shell)?;
                let mut edits = Vec::new();
                let content = children.iter().map(|child| self.render(child, (9525.0, 9525.0))).collect::<Result<Vec<_>>>()?.concat();
                insert_child(&shell, parsed.root_element(), &content, None, &mut edits)?;
                return String::from_utf8(apply(shell, edits)?).map_err(|_| Error::Invalid("native group encoding".into()));
            }
            return remap_resources(scaled_element(element, self.ids, self.theme, scale)?, self.resources);
        };
        let mut edits = Vec::new();
        if let (Element::Group { children: before, view_width, view_height, .. }, Element::Group { children: after, view_width: next_width, view_height: next_height, .. }) = (old, element) {
            if view_width == next_width && view_height == next_height && equal(before, after)? {
                patch_element(self.xml, *node, old, element, self.ids, self.theme, self.binding, self.resources, scale, &mut edits)?;
                selection_transform(self.xml, *node, element, scale, false, &mut edits)?;
                return selection_fragment(self.xml, *node, edits);
            }
        }
        if let (Element::Group { children: before, .. }, Element::Group { children: after, .. }) = (old, element) {
            let old_shell = selection_group_shell(old);
            let mut next_shell = selection_group_shell(element);
            if let (Element::Group { view_width, view_height, .. }, Element::Group { view_width: next_width, view_height: next_height, .. }) = (&old_shell, &mut next_shell) {
                *next_width = *view_width; *next_height = *view_height;
            }
            patch_element(self.xml, *node, &old_shell, &next_shell, self.ids, self.theme, self.binding, self.resources, scale, &mut edits)?;
            let modeled: BTreeSet<_> = before.iter().filter_map(|child| self.binding.nodes.get(child.bounds().0)).collect();
            if node.children().filter(|child| child.is_element()).any(|child| !is_shape(child) && !child.has_tag_name((P,"nvGrpSpPr")) && !child.has_tag_name((P,"grpSpPr")) && !child.has_tag_name((P,"extLst"))) {
                return Err(Error::Unsupported("native group canvas has opaque children; normalization is rejected".into()));
            }
            if node.children().filter(|child| is_shape(*child)).any(|child| !properties(child).and_then(|child| child.attribute("id")).is_some_and(|numeric| modeled.iter().any(|id| id.as_str() == numeric))) {
                return Err(Error::Unsupported("native group has unmapped children; reparenting is rejected".into()));
            }
            let mut fragments = after.iter().map(|child| self.render(child, (9525.0, 9525.0))).collect::<Result<Vec<_>>>()?.into_iter();
            for child in node.children().filter(|child| is_shape(*child)) { edits.push((child.range(), fragments.next().unwrap_or_default())); }
            let extra = fragments.collect::<String>();
            if !extra.is_empty() { insert_child(self.xml, *node, &extra, child(*node, P, "extLst"), &mut edits)?; }
        } else if !selection_scaled_style(*node, old, element, &mut edits)? {
            patch_element(self.xml, *node, old, element, self.ids, self.theme, self.binding, self.resources, scale, &mut edits)?;
        }
        selection_transform(self.xml, *node, element, scale, true, &mut edits)?;
        selection_fragment(self.xml, *node, edits)
    }
}

fn patch_selection_topology(xml: &str, tree: Node<'_, '_>, before: &[Element], after: &[Element], ids: &BTreeMap<&str, usize>, theme: &crate::design::Theme, binding: &NativePart, resources: &BTreeMap<String, String>, scale: (f64, f64), edits: &mut Edits) -> Result<()> {
    let mut nodes = BTreeMap::new();
    let mut raw_ids = BTreeSet::new();
    for node in tree.descendants().filter(|node| node.has_tag_name((P, "cNvPr"))) {
        let numeric: usize = node.attribute("id").and_then(|id| id.parse().ok()).ok_or_else(|| Error::Invalid("native shape identity".into()))?;
        if !raw_ids.insert(numeric) { return Err(Error::Conflict("duplicate raw native shape ID".into())); }
    }
    for node in tree.descendants().filter(|node| is_shape(*node)) {
        if let Some(numeric) = properties(node).and_then(|node| node.attribute("id")) { nodes.insert(numeric, node); }
    }
    let mut originals = BTreeMap::new();
    let mut anchors = BTreeMap::new();
    let root_nodes: Vec<_> = tree.children().filter(|node| node.is_element()).collect();
    for element in before {
        let numeric = binding.nodes.get(element.bounds().0).ok_or_else(|| Error::Conflict("native selection identity missing".into()))?;
        let root = nodes.get(numeric.as_str()).ok_or_else(|| Error::Conflict("native selection node missing".into()))?;
        let anchor = root_nodes.iter().position(|node| node == root).ok_or_else(|| Error::Conflict("native selection container mismatch".into()))?;
        for descendant in crate::model::element_list(std::slice::from_ref(element)) {
            let numeric = binding.nodes.get(descendant.bounds().0).ok_or_else(|| Error::Conflict("native descendant identity missing".into()))?;
            let node = nodes.get(numeric.as_str()).ok_or_else(|| Error::Conflict("native descendant node missing".into()))?;
            originals.insert(descendant.bounds().0, (descendant, *node));
            anchors.insert(descendant.bounds().0, anchor);
        }
    }
    let remaining: BTreeSet<_> = crate::model::element_list(after).iter().map(|element| element.bounds().0).collect();
    for (id, (element, node)) in &originals {
        if matches!(element, Element::Group { .. }) && !remaining.contains(id) {
            let shell = scaled_element(&selection_group_shell(element), ids, theme, scale)?;
            let expected = parse(&shell)?;
            if child(*node, P, "nvGrpSpPr").map(xml_value) != child(expected.root_element(), P, "nvGrpSpPr").map(xml_value) {
                return Err(Error::Unsupported("ungroup would discard unmodeled native group metadata".into()));
            }
            if node.children().filter(|node| node.is_element()).any(|node| !is_shape(node) && !node.has_tag_name((P,"nvGrpSpPr")) && !node.has_tag_name((P,"grpSpPr"))) {
                return Err(Error::Unsupported("ungroup would discard unmodeled native group content".into()));
            }
            let properties = child(*node, P, "grpSpPr").ok_or_else(|| Error::Unsupported("group properties missing".into()))?;
            if properties.children().filter(|node| node.is_element()).any(|node| !node.has_tag_name((A,"xfrm"))) {
                return Err(Error::Unsupported("ungroup would discard native group effects".into()));
            }
            let transform = child(properties, A, "xfrm").ok_or_else(|| Error::Unsupported("group transform missing".into()))?;
            if ["rot", "flipH", "flipV"].iter().any(|name| transform.attribute(*name).is_some_and(|value| value != "0" && value != "false")) {
                return Err(Error::Unsupported("rotated or flipped native group cannot be ungrouped safely".into()));
            }
            for child in node.children().filter(|node| is_shape(*node)) {
                if !originals.values().any(|(_, mapped)| *mapped == child) { return Err(Error::Unsupported("ungroup would drop an unmapped native child".into())); }
            }
        }
    }
    for element in crate::model::element_list(after) {
        if !originals.contains_key(element.bounds().0) && raw_ids.contains(&ids[element.bounds().0]) { return Err(Error::Conflict("new selection ID collides with retained native XML".into())); }
    }
    selection_connection_containers(tree, after, &originals, ids)?;
    let context = NativeSelection { xml, originals, ids, theme, binding, resources };
    let mut replacements: BTreeMap<usize, String> = BTreeMap::new();
    let mut last_anchor = 0;
    for element in after {
        let source_anchors: BTreeSet<_> = if let Some(anchor) = anchors.get(element.bounds().0) { BTreeSet::from([*anchor]) } else {
            crate::model::element_list(std::slice::from_ref(element)).iter().filter_map(|child| anchors.get(child.bounds().0).copied()).collect()
        };
        let anchor = source_anchors.first().copied().ok_or_else(|| Error::Unsupported("combine native reparenting and insertion in separate transactions".into()))?;
        if anchor < last_anchor { return Err(Error::Unsupported("native reparenting cannot reorder unrelated intervals".into())); }
        last_anchor = anchor;
        if source_anchors.len() > 1 && source_anchors.last().unwrap() - anchor + 1 != source_anchors.len() { return Err(Error::Unsupported("native grouping must select one contiguous interval without opaque siblings".into())); }
        let rendered = if let Some((old, node)) = context.originals.get(element.bounds().0).filter(|(_, node)| node.parent() == Some(tree)) {
            if equal(old, element)? { xml[node.range()].to_owned() } else { context.render(element, scale)? }
        } else { context.render(element, scale)? };
        replacements.entry(anchor).or_default().push_str(&rendered);
    }
    for element in before {
        let anchor = anchors[element.bounds().0];
        edits.push((root_nodes[anchor].range(), replacements.remove(&anchor).unwrap_or_default()));
    }
    Ok(())
}

fn patch_elements(xml: &str, tree: Node<'_, '_>, before: &[Element], after: &[Element], ids: &BTreeMap<&str, usize>, theme: &crate::design::Theme, binding: &NativePart, resources: &BTreeMap<String, String>, scale: (f64, f64), edits: &mut Edits) -> Result<()> {
    if tree.has_tag_name((P, "spTree")) {
        let mut reserved = BTreeSet::new();
        for node in tree.descendants().filter(|node| node.has_tag_name((P, "cNvPr"))) {
            let numeric = node.attribute("id").and_then(|id| id.parse::<usize>().ok()).ok_or_else(|| Error::Invalid("native selection ID".into()))?;
            if !reserved.insert(numeric) { return Err(Error::Conflict("duplicate raw native shape ID".into())); }
        }
        let old: BTreeSet<_> = crate::model::element_list(before).iter().map(|element| element.bounds().0).collect();
        let mut allocated = BTreeSet::new();
        for element in crate::model::element_list(after) {
            let numeric = *ids.get(element.bounds().0).ok_or_else(|| Error::Conflict("native ID allocation missing".into()))?;
            if numeric <= 1 || numeric > u32::MAX as usize || !allocated.insert(numeric) || (!old.contains(element.bounds().0) && reserved.contains(&numeric)) {
                return Err(Error::Conflict("native selection ID collides with retained XML".into()));
            }
        }
        let removed: BTreeSet<_> = old.iter().filter_map(|id| ids.get(*id)).filter(|numeric| !allocated.contains(numeric)).copied().collect();
        if !removed.is_empty() {
            for node in tree.document().root_element().descendants().filter(|node| node.is_element()) {
                let referenced = node.attributes().any(|attribute| attribute.name().eq_ignore_ascii_case("spid") && attribute.value().parse::<usize>().is_ok_and(|target| removed.contains(&target)));
                let owner = node.ancestors().find(|node| is_shape(*node)).and_then(properties).and_then(|node| node.attribute("id")).and_then(|id| id.parse::<usize>().ok());
                if referenced && owner.is_none_or(|owner| !removed.contains(&owner)) {
                    return Err(Error::Unsupported("native removal would leave a retained shape reference without its target".into()));
                }
            }
            for connection in tree.descendants().filter(|node| node.has_tag_name((A,"stCxn")) || node.has_tag_name((A,"endCxn"))) {
                let owner = connection.ancestors().find(|node| is_shape(*node)).and_then(properties).and_then(|node| node.attribute("id")).and_then(|id| id.parse::<usize>().ok());
                let target = connection.attribute("id").and_then(|id| id.parse::<usize>().ok());
                if owner.is_some_and(|owner| !removed.contains(&owner)) && target.is_some_and(|target| removed.contains(&target)) {
                    return Err(Error::Unsupported("native cut/delete would leave a retained connector without its target".into()));
                }
            }
        }
    }
    let mut old_parents = BTreeMap::new(); let mut new_parents = BTreeMap::new();
    selection_parents(before, None, &mut old_parents); selection_parents(after, None, &mut new_parents);
    if old_parents.iter().any(|(id, parent)| new_parents.get(id).is_some_and(|next| next != parent)) {
        return patch_selection_topology(xml, tree, before, after, ids, theme, binding, resources, scale, edits);
    }
    let old_order: Vec<_> = before.iter().map(|element| element.bounds().0).collect();
    let new_order: Vec<_> = after.iter().map(|element| element.bounds().0).collect();
    if old_order != new_order {
        let mut fragments = Vec::new();
        for next in after {
            if let Some(old) = before.iter().find(|old| old.bounds().0 == next.bounds().0) {
                let numeric = binding.nodes.get(old.bounds().0).ok_or_else(|| Error::Conflict("native object identity missing".into()))?;
                let node = tree.children().find(|node| is_shape(*node) && properties(*node).and_then(|node| node.attribute("id")) == Some(numeric)).ok_or_else(|| Error::Conflict("native object moved outside its container".into()))?;
                let mut local = Vec::new();
                patch_element(xml, node, old, next, ids, theme, binding, resources, scale, &mut local)?;
                fragments.push(edited_fragment(xml, node, local)?);
            } else {
                fragments.push(remap_resources(scaled_element(next, ids, theme, scale)?, resources)?);
            }
        }
        let mut fragments = fragments.into_iter();
        for node in tree.children().filter(|node| is_shape(*node)) {
            if properties(node).and_then(|node| node.attribute("id")).is_some_and(|numeric| before.iter().any(|old| binding.nodes.get(old.bounds().0).is_some_and(|id| id == numeric))) {
                edits.push((node.range(), fragments.next().unwrap_or_default()));
            }
        }
        let added = fragments.collect::<String>();
        if !added.is_empty() { insert_child(xml, tree, &added, child(tree, P, "extLst"), edits)?; }
        return Ok(());
    }
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

pub(crate) fn patch_part(package: &mut Package, binding: &NativePart, before: &[Element], after: &[Element], theme: &crate::design::Theme, name: Option<&str>, background: Option<Option<&str>>, scale: (f64, f64)) -> Result<()> {
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

pub(crate) fn patch_theme(package: &mut Package, path: &str, before: &crate::design::Theme, after: &crate::design::Theme) -> Result<()> {
    if equal(before, after)? { return Ok(()); }
    let xml = package.text(path)?.to_owned(); let document = parse(&xml)?; let root = document.root_element(); let mut edits = Vec::new();
    if before.name != after.name { set_attribute(&xml, root, "name", &after.name, &mut edits)?; }
    let elements = child(root, A, "themeElements").ok_or_else(|| Error::Unsupported("theme elements missing".into()))?;
    let scheme = child(elements, A, "clrScheme").ok_or_else(|| Error::Unsupported("theme colors missing".into()))?;
    for key in crate::design::COLOR_KEYS {
        if before.colors[key] != after.colors[key] {
            let parent = child(scheme, A, key).ok_or_else(|| Error::Unsupported("theme color missing".into()))?;
            let entry = parent.children().find(|node| node.is_element()).ok_or_else(|| Error::Unsupported("theme color missing".into()))?;
            let attribute = if entry.has_tag_name((A, "srgbClr")) { "val" } else if entry.has_tag_name((A, "sysClr")) { "lastClr" } else { return Err(Error::Unsupported("theme color representation".into())); };
            set_attribute(&xml, entry, attribute, &after.colors[key], &mut edits)?;
        }
    }
    if !equal(&before.fonts, &after.fonts)? {
        let scheme = child(elements, A, "fontScheme").ok_or_else(|| Error::Unsupported("theme font scheme missing".into()))?;
        for (kind, old_latin, latin) in [("majorFont", &before.fonts.major, &after.fonts.major), ("minorFont", &before.fonts.minor, &after.fonts.minor)] {
            let group = child(scheme, A, kind).ok_or_else(|| Error::Unsupported("theme font collection missing".into()))?;
            for (tag, previous, value) in [("latin", old_latin, latin), ("ea", &before.fonts.east_asian, &after.fonts.east_asian), ("cs", &before.fonts.complex_script, &after.fonts.complex_script)] {
                if previous == value { continue; }
                let font = child(group, A, tag).ok_or_else(|| Error::Unsupported("theme font missing".into()))?; set_attribute(&xml, font, "typeface", value, &mut edits)?;
            }
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

fn unused_notes_path(package: &Package, stem: &str) -> Result<String> {
    let types = parse(package.text("[Content_Types].xml")?)?;
    let mut taken: BTreeSet<_> = package.part_names().into_iter().map(str::to_ascii_lowercase).collect();
    taken.extend(types.root_element().children().filter_map(|node| node.attribute("PartName").map(|name| name.trim_start_matches('/').to_ascii_lowercase())));
    for number in 1..=taken.len() + 1 {
        let path = format!("{stem}{number}.xml");
        if !taken.contains(&path.to_ascii_lowercase()) && !taken.contains(&relations_path(&path).to_ascii_lowercase()) { return Ok(path); }
    }
    Err(Error::Limit("native notes part names exhausted".into()))
}

fn add_notes_type(package: &mut Package, path: &str, kind: &str) -> Result<()> {
    let xml = package.text("[Content_Types].xml")?.to_owned(); let parsed = parse(&xml)?;
    let mut edits = Vec::new();
    insert_child(&xml, parsed.root_element(), &format!("<Override xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\" PartName=\"/{}\" ContentType=\"{kind}\"/>", quick_xml::escape::escape(path)), None, &mut edits)?;
    package.replace_part("[Content_Types].xml", apply(xml, edits)?)
}

fn add_notes_relation(package: &mut Package, source: &str, kind: &str, target: &str) -> Result<String> {
    let path = relations_path(source);
    let exists = package.part_names().contains(path.as_str());
    let xml = if exists { package.text(&path)?.to_owned() } else { format!("<Relationships xmlns=\"{}\"/>", crate::native::REL) };
    let parsed = parse(&xml)?;
    let taken: BTreeSet<_> = parsed.root_element().children().filter_map(|node| node.attribute("Id")).collect();
    let id = (1..=taken.len() + 1).map(|number| format!("rIdAislideNotes{number}")).find(|id| !taken.contains(id.as_str())).ok_or_else(|| Error::Limit("native notes relationship IDs exhausted".into()))?;
    let mut edits = Vec::new();
    insert_child(&xml, parsed.root_element(), &format!("<Relationship xmlns=\"{}\" Id=\"{id}\" Type=\"{}/{kind}\" Target=\"/{}\"/>", crate::native::REL, crate::native::R, quick_xml::escape::escape(target)), None, &mut edits)?;
    let data = apply(xml, edits)?;
    if exists { package.replace_part(&path, data)?; } else { package.add_part(path, data)?; }
    Ok(id)
}

fn notes_template(slide: &crate::model::Slide, deck: &Deck) -> Result<Package> {
    let blank = crate::model::Slide { elements: Vec::new(), layout_id: None, native_source_id: None, review: None, ..slide.clone() };
    Package::open(crate::pptx::export_pptx(&Deck { slides: vec![blank], design: None, ..deck.clone() })?)
}

fn ensure_notes_master(package: &mut Package, main: &str, generated: &Package) -> Result<String> {
    let masters = crate::pptx::relationship_targets(package, main, "notesMaster")?;
    if masters.len() > 1 { return Err(Error::Unsupported("multiple native notes masters are preserved".into())); }
    let master = if let Some(master) = masters.into_values().next() { master } else {
        let path = unused_notes_path(package, "ppt/notesMasters/aislide-notes-master-")?;
        let theme = unused_notes_path(package, "ppt/theme/aislide-notes-theme-")?;
        package.add_part(path.clone(), generated.part("ppt/notesMasters/notesMaster1.xml")?.to_vec())?;
        package.add_part(theme.clone(), generated.part("ppt/theme/theme2.xml")?.to_vec())?;
        add_notes_type(package, &path, "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml")?;
        add_notes_type(package, &theme, "application/vnd.openxmlformats-officedocument.theme+xml")?;
        add_notes_relation(package, &path, "theme", &theme)?;
        let relation = add_notes_relation(package, main, "notesMaster", &path)?;
        let xml = package.text(main)?.to_owned(); let parsed = parse(&xml)?; let root = parsed.root_element();
        let entry = format!("<p:notesMasterId xmlns:p=\"{P}\" xmlns:r=\"{}\" r:id=\"{relation}\"/>", crate::native::R);
        let mut edits = Vec::new();
        if let Some(list) = child(root, P, "notesMasterIdLst") {
            if list.children().any(|node| node.has_tag_name((P, "notesMasterId"))) { return Err(Error::Unsupported("unresolved native notes master identity".into())); }
            insert_child(&xml, list, &entry, None, &mut edits)?;
        } else {
            let before = root.children().find(|node| node.is_element() && !node.has_tag_name((P, "sldMasterIdLst")));
            insert_child(&xml, root, &format!("<p:notesMasterIdLst xmlns:p=\"{P}\">{entry}</p:notesMasterIdLst>"), before, &mut edits)?;
        }
        package.replace_part(main, apply(xml, edits)?)?;
        path
    };
    Ok(master)
}

fn create_notes(package: &mut Package, main: &str, slide_path: &str, slide: &crate::model::Slide, deck: &Deck) -> Result<()> {
    let generated = notes_template(slide, deck)?;
    let master = ensure_notes_master(package, main, &generated)?;
    let path = unused_notes_path(package, "ppt/notesSlides/aislide-notes-")?;
    package.add_part(path.clone(), generated.part("ppt/notesSlides/notesSlide1.xml")?.to_vec())?;
    add_notes_type(package, &path, "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml")?;
    add_notes_relation(package, &path, "notesMaster", &master)?;
    add_notes_relation(package, &path, "slide", slide_path)?;
    add_notes_relation(package, slide_path, "notesSlide", &path)?;
    Ok(())
}

fn notes_paragraph(xml: &str, paragraph: Node<'_, '_>, text: &str) -> Result<String> {
    let mut edits = Vec::new();
    if let Some(run) = child(paragraph, A, "r") {
        let node = child(run, A, "t").ok_or_else(|| Error::Unsupported("native notes run has no text".into()))?;
        let escaped = quick_xml::escape::escape(text);
        let mut written = false;
        for child in node.children().filter(|node| node.is_text()) {
            edits.push((child.range(), if written { String::new() } else { written = true; escaped.to_string() }));
        }
        if !written { insert_child(xml, node, &escaped, None, &mut edits)?; }
    } else {
        let run = format!("<a:r xmlns:a=\"{A}\"><a:t>{}</a:t></a:r>", quick_xml::escape::escape(text));
        insert_child(xml, paragraph, &run, child(paragraph, A, "endParaRPr"), &mut edits)?;
    }
    edited_fragment(xml, paragraph, edits)
}

fn replace_notes_text(xml: &str, body: Node<'_, '_>, text: &str, edits: &mut Edits) -> Result<()> {
    let paragraphs: Vec<_> = body.children().filter(|node| node.has_tag_name((A, "p"))).collect();
    let lines: Vec<_> = text.split('\n').collect();
    for paragraph in &paragraphs {
        let runs: Vec<_> = paragraph.children().filter(|node| node.has_tag_name((A, "r"))).collect();
        let texts = paragraph.descendants().filter(|node| node.has_tag_name((A, "t"))).count();
        if runs.len() > 1 || texts != runs.len()
            || runs.iter().any(|run| child(*run, A, "t").is_none_or(|node| node.children().any(|child| child.is_element())))
            || paragraph.children().filter(|node| node.is_element()).any(|node| node.tag_name().namespace() != Some(A) || !["pPr", "r", "endParaRPr"].contains(&node.tag_name().name()))
            || paragraph.descendants().any(|node| node.has_tag_name((A, "fld")) || node.has_tag_name((A, "br"))) {
            return Err(Error::Unsupported("mixed-run or field notes are preserved; this text change cannot be represented safely".into()));
        }
    }
    if lines.len() != paragraphs.len() && paragraphs.iter().any(|paragraph| paragraph.descendants().filter(|node| node.is_element()).any(|node| node.tag_name().namespace() != Some(A) || node.has_tag_name((A, "extLst")))) {
        return Err(Error::Unsupported("notes paragraph extensions are preserved; paragraph count changes are not supported".into()));
    }
    for (index, paragraph) in paragraphs.iter().enumerate() {
        edits.push((paragraph.range(), if let Some(line) = lines.get(index) { notes_paragraph(xml, *paragraph, line)? } else { String::new() }));
    }
    let mut added = String::new();
    for line in lines.iter().skip(paragraphs.len()) {
        if let Some(prototype) = paragraphs.last() { added.push_str(&notes_paragraph(xml, *prototype, line)?); }
        else { added.push_str(&format!("<a:p xmlns:a=\"{A}\"><a:r><a:t>{}</a:t></a:r></a:p>", quick_xml::escape::escape(*line))); }
    }
    if !added.is_empty() {
        if let Some(last) = paragraphs.last() { let position = last.range().end; edits.push((position..position, added)); }
        else { insert_child(xml, body, &added, None, edits)?; }
    }
    Ok(())
}

fn patch_notes(package: &mut Package, main: &str, slide_path: &str, slide: &crate::model::Slide, deck: &Deck) -> Result<()> {
    let targets = crate::pptx::relationship_targets(package, slide_path, "notesSlide")?;
    if targets.len() > 1 { return Err(Error::Unsupported("multiple native notes parts are preserved".into())); }
    let Some(path) = targets.values().next() else { return create_notes(package, main, slide_path, slide, deck); };
    for other in crate::pptx::slide_paths(package)?.iter().filter(|other| other.as_str() != slide_path) {
        if crate::pptx::relationship_targets(package, other, "notesSlide")?.values().any(|target| target == path) {
            return Err(Error::Unsupported("shared notes parts are preserved; editing one slide must not change another".into()));
        }
    }
    let xml = package.text(path)?.to_owned();
    let parsed = parse(&xml)?;
    let placeholders = parsed.descendants().filter(|node| node.has_tag_name((P, "sp")))
        .filter(|node| child(*node, P, "nvSpPr").and_then(|node| child(node, P, "nvPr")).and_then(|node| child(node, P, "ph")).is_some_and(|node| node.attribute("type") == Some("body")))
        .collect::<Vec<_>>();
    if placeholders.len() > 1 { return Err(Error::Unsupported("notes require at most one body placeholder".into())); }
    let mut edits = Vec::new();
    if let Some(body) = placeholders.first().and_then(|node| child(*node, P, "txBody")) {
        if slide.notes_paragraphs.is_empty() { replace_notes_text(&xml, body, &slide.notes, &mut edits)?; }
        else {
            let previous = crate::native::read_rich_paragraphs_with_limit(body, &Default::default(), &Default::default(), 8000)?;
            let generated = notes_template(slide, deck)?;
            let template = generated.text("ppt/notesSlides/notesSlide1.xml")?;
            let parsed = parse(template)?;
            let next_body = parsed.descendants().find(|node| node.has_tag_name((P, "txBody"))).ok_or_else(|| Error::Invalid("generated rich notes missing".into()))?;
            let originals: Vec<_> = body.children().filter(|node| node.has_tag_name((A, "p"))).collect();
            let next: Vec<_> = next_body.children().filter(|node| node.has_tag_name((A, "p"))).collect();
            for (index, paragraph) in originals.iter().enumerate() {
                if previous.get(index) == slide.notes_paragraphs.get(index) { continue; }
                if let (Some(previous), Some(next)) = (previous.get(index), slide.notes_paragraphs.get(index)) {
                    if crate::fields::patch_paragraph_cache(&xml, *paragraph, previous, next, &mut edits)? { continue; }
                }
                if !supports_rich_style(*paragraph) || paragraph.descendants().any(|node| node.has_tag_name((A, "hlinkClick"))) { return Err(Error::Unsupported("notes paragraph fields, links or unmodeled styles are preserved".into())); }
                let replacement = if let Some(next) = next.get(index) {
                    let mut replacement = fragment(template, *next);
                    if let Some(end) = child(*paragraph, A, "endParaRPr") {
                        let parsed = parse(&replacement)?;
                        let range = child(parsed.root_element(), A, "endParaRPr").map(|node| node.range());
                        if let Some(range) = range { replacement.replace_range(range, &crate::native_design::isolated_row(&xml, end)?); }
                    }
                    replacement
                } else { String::new() };
                edits.push((paragraph.range(), replacement));
            }
            if next.len() > originals.len() {
                let added: String = next[originals.len()..].iter().map(|node| fragment(template, *node)).collect();
                if let Some(last) = originals.last() { let position = last.range().end; edits.push((position..position, added)); }
                else { insert_child(&xml, body, &added, None, &mut edits)?; }
            }
        }
    } else {
        let generated = notes_template(slide, deck)?;
        let template = generated.text("ppt/notesSlides/notesSlide1.xml")?; let document = parse(template)?;
        let shape = document.descendants().find(|node| node.has_tag_name((P, "sp"))).ok_or_else(|| Error::Invalid("generated notes placeholder missing".into()))?;
        if let Some(placeholder) = placeholders.first() {
            let body = child(shape, P, "txBody").ok_or_else(|| Error::Invalid("generated notes body missing".into()))?;
            insert_child(&xml, *placeholder, &fragment(template, body), child(*placeholder, P, "extLst"), &mut edits)?;
        } else {
            let tree = child(parsed.root_element(), P, "cSld").and_then(|node| child(node, P, "spTree")).ok_or_else(|| Error::Unsupported("native notes shape tree missing".into()))?;
            let maximum = parsed.descendants().filter(|node| node.has_tag_name((P, "cNvPr"))).filter_map(|node| node.attribute("id").and_then(|id| id.parse::<u32>().ok())).max().unwrap_or(1);
            let id = maximum.checked_add(1).ok_or_else(|| Error::Limit("native notes shape IDs exhausted".into()))?;
            let shape = fragment(template, shape); let document = parse(&shape)?;
            let mut local = Vec::new(); set_attribute(&shape, properties(document.root_element()).ok_or_else(|| Error::Invalid("generated notes identity missing".into()))?, "id", &id.to_string(), &mut local)?;
            let shape = edited_fragment(&shape, document.root_element(), local)?;
            insert_child(&xml, tree, &shape, child(tree, P, "extLst"), &mut edits)?;
        }
    }
    package.replace_part(path, apply(xml, edits)?)
}

pub(crate) fn save(package: &mut Package, original: &NativeDeck, deck: &Deck) -> Result<Vec<u8>> {
    let old = original.deck.design.as_ref().ok_or_else(|| Error::Unsupported("native design missing".into()))?;
    let design = deck.design.as_ref().ok_or_else(|| Error::Unsupported("native design cannot be detached".into()))?;
    if !equal(old, design)? { crate::review::ensure_unprotected(package)?; }
    let roots = crate::pptx::relationship_targets(package, "", "officeDocument")?;
    let main = roots.values().next().ok_or_else(|| Error::Invalid("presentation missing".into()))?;
    let presentation = parse(package.text(main)?)?;
    let size = child(presentation.root_element(), P, "sldSz").ok_or_else(|| Error::Invalid("slide size missing".into()))?;
    let scale = (size.attribute("cx").and_then(|value| value.parse::<f64>().ok()).unwrap_or(12192000.0) / f64::from(original.deck.width), size.attribute("cy").and_then(|value| value.parse::<f64>().ok()).unwrap_or(6858000.0) / f64::from(original.deck.height));
    if deck.width != original.deck.width || deck.height != original.deck.height { crate::canvas::write_page(package, main, deck.width, deck.height, scale)?; }
    if deck.title != original.deck.title { rename_presentation(package, &deck.title)?; }
    crate::native_design::patch_auxiliary(package, original, deck, main)?;
    let topology = crate::native_design::prepare(package, original, deck, main)?;
    crate::native_design::patch_themes(package, design, &topology.masters)?;
    for (after, binding) in design.masters.iter().zip(&topology.masters) {
        let before = old.masters.iter().find(|before| before.id == after.id);
        patch_part(package, binding, before.map(|before| before.elements.as_slice()).unwrap_or(&[]), &after.elements, crate::design::master_theme(design, &after.id), before.is_some_and(|before| before.name != after.name).then_some(after.name.as_str()), before.is_some_and(|before| before.background != after.background).then_some(Some(after.background.as_str())), scale)?;
        crate::fields::write_part(package, &binding.path, &after.elements, Some(binding))?;
    }
    for (after, binding) in design.layouts.iter().zip(&topology.layouts) {
        let before = old.layouts.iter().find(|before| before.id == after.id);
        patch_part(package, binding, before.map(|before| before.elements.as_slice()).unwrap_or(&[]), &after.elements, crate::design::master_theme(design, &after.master_id), before.is_some_and(|before| before.name != after.name).then_some(after.name.as_str()), before.is_some_and(|before| before.background != after.background).then_some(after.background.as_deref()), scale)?;
        crate::fields::write_part(package, &binding.path, &after.elements, Some(binding))?;
    }
    if let Some(slide) = deck.slides.iter().find(|slide| !slide.notes.is_empty() && slide.native_source_id.is_none() && !original.slides.iter().any(|binding| binding.id == slide.id)) {
        ensure_notes_master(package, main, &notes_template(slide, deck)?)?;
    }
    let bindings = crate::native_slides::prepare(package, original, deck, main, scale, &topology.layouts)?;
    for (after, binding) in deck.slides.iter().zip(&bindings) {
        let source = after.native_source_id.as_deref().unwrap_or(&after.id);
        let Some(before) = original.deck.slides.iter().find(|slide| slide.id == source) else { continue };
        if before.layout_id != after.layout_id {
            let path = after.layout_id.as_ref().or_else(|| design.layouts.first().map(|layout| &layout.id)).and_then(|id| topology.layouts.iter().find(|part| &part.id == id)).map(|part| &part.path).ok_or_else(|| Error::Unsupported("native slide layout missing".into()))?;
            let relation_path = relations_path(&binding.path); let xml = package.text(&relation_path)?.to_owned(); let document = parse(&xml)?;
            let relation = document.root_element().children().find(|node| node.attribute("Type") == Some(format!("{}/slideLayout", crate::native::R).as_str())).ok_or_else(|| Error::Invalid("slide layout relationship missing".into()))?;
            let mut edits = Vec::new(); set_attribute(&xml, relation, "Target", &format!("/{path}"), &mut edits)?; package.replace_part(&relation_path, apply(xml, edits)?)?;
        }
        if before.notes != after.notes || before.notes_paragraphs != after.notes_paragraphs { patch_notes(package, main, &binding.path, after, deck)?; }
        let background = (before.background != after.background || before.inherit_background != after.inherit_background).then_some(if after.inherit_background { None } else { Some(after.background.as_str()) });
        patch_part(package, binding, &before.elements, &after.elements, crate::design::slide_theme(after, Some(design)).unwrap_or(&design.theme), (before.title != after.title).then_some(after.title.as_str()), background, scale)?;
        if before.hide_master_graphics != after.hide_master_graphics {
            let xml = package.text(&binding.path)?.to_owned(); let document = parse(&xml)?; let mut edits = Vec::new(); set_attribute(&xml, document.root_element(), "showMasterSp", if after.hide_master_graphics { "0" } else { "1" }, &mut edits)?; package.replace_part(&binding.path, apply(xml, edits)?)?;
        }
    }
    crate::native_design::remove_deleted(package, original, &topology)?;
    crate::review::save_end(package, original, deck, &bindings)?;
    crate::fonts::write_native(package, main, &original.deck.embedded_fonts, deck)?;
    package.save()
}