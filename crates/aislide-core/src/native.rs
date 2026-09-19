use crate::{design::{Design, Master, SlideLayout, Theme, ThemeFonts}, model::{Bullet, Connection, Crop, Deck, Element, Placeholder, PlaceholderKind, Slide, TextAlign, TextFormat, VerticalAlign, validate_deck}, package::Package, pptx::{parse, relationship_targets, slide_paths}, Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use roxmltree::Node;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
pub(crate) const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub(crate) const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub(crate) const C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
pub(crate) const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";

pub(crate) struct NativePart { pub path: String, pub id: String, pub nodes: BTreeMap<String, String> }
pub(crate) struct NativeDeck { pub deck: Deck, pub slides: Vec<NativePart>, pub masters: Vec<NativePart>, pub layouts: Vec<NativePart>, pub warnings: Vec<String>, pub metadata: Option<crate::provenance::Metadata> }

pub(crate) fn child<'a>(node: Node<'a, '_>, namespace: &str, name: &str) -> Option<Node<'a, 'a>> { node.children().find(|node| node.has_tag_name((namespace, name))) }
fn number(node: Node<'_, '_>, attribute: &str, default: f64) -> f64 { node.attribute(attribute).and_then(|value| value.parse().ok()).unwrap_or(default) }
pub(crate) fn properties<'a>(node: Node<'a, '_>) -> Option<Node<'a, 'a>> { node.children().find(|node| node.tag_name().namespace() == Some(P) && ["nvSpPr", "nvPicPr", "nvGraphicFramePr", "nvCxnSpPr", "nvGrpSpPr"].contains(&node.tag_name().name())).and_then(|node| child(node, P, "cNvPr")) }
fn transform<'a>(node: Node<'a, '_>) -> Option<Node<'a, 'a>> { child(node, P, "xfrm").or_else(|| child(node, P, if node.has_tag_name((P, "grpSp")) { "grpSpPr" } else { "spPr" }).and_then(|node| child(node, A, "xfrm"))) }
pub(crate) fn is_shape(node: Node<'_, '_>) -> bool { node.tag_name().namespace() == Some(P) && ["sp", "pic", "graphicFrame", "grpSp", "cxnSp"].contains(&node.tag_name().name()) }

fn color(node: Node<'_, '_>, default: &str) -> String {
    let Some(fill) = child(node, A, "solidFill") else { return default.into() };
    if let Some(rgb) = child(fill, A, "srgbClr").and_then(|node| node.attribute("val")) { return rgb.into(); }
    if let Some(rgb) = child(fill, A, "sysClr").and_then(|node| node.attribute("lastClr")) { return rgb.into(); }
    if let Some(slot) = child(fill, A, "schemeClr").and_then(|node| node.attribute("val")) {
        let slot = match slot { "tx1" => "dk1", "bg1" => "lt1", "tx2" => "dk2", "bg2" => "lt2", other => other };
        if crate::design::COLOR_KEYS.contains(&slot) { return format!("@{slot}"); }
    }
    default.into()
}

fn background(node: Node<'_, '_>) -> Option<String> { child(node, P, "cSld").and_then(|node| child(node, P, "bg")).and_then(|node| child(node, P, "bgPr")).map(|node| color(node, "@lt1")) }

fn shape_fill(node: Node<'_, '_>) -> String {
    if let Some(stop) = child(node, A, "gradFill").and_then(|node| child(node, A, "gsLst")).and_then(|node| child(node, A, "gs")) {
        if let Ok((color, _)) = crate::visual::read_color(stop) { return color; }
    }
    if child(node, A, "noFill").is_some() { "none".into() } else { color(node, "@lt1") }
}

fn placeholder(node: Node<'_, '_>) -> Option<Placeholder> {
    let value = node.children().find(|node| node.tag_name().namespace() == Some(P) && node.tag_name().name().starts_with("nv"))
        .and_then(|node| child(node, P, "nvPr")).and_then(|node| child(node, P, "ph"))?;
    let kind = match value.attribute("type").unwrap_or("body") { "title" | "ctrTitle" => PlaceholderKind::Title, "body" | "obj" => PlaceholderKind::Body, "subTitle" => PlaceholderKind::Subtitle, "ftr" => PlaceholderKind::Footer, "dt" => PlaceholderKind::Date, "sldNum" => PlaceholderKind::SlideNumber, _ => return None };
    Some(Placeholder { kind, index: number(value, "idx", 0.0) as u32 })
}

pub(crate) fn body_text(node: Node<'_, '_>) -> String {
    node.children().filter(|node| node.has_tag_name((A, "p"))).map(|paragraph| {
        paragraph.children().filter_map(|part| {
            if part.has_tag_name((A, "br")) { Some("\n".into()) }
            else if part.has_tag_name((A, "r")) || part.has_tag_name((A, "fld")) { Some(part.children().filter(|node| node.has_tag_name((A, "t"))).filter_map(|node| node.text()).collect::<String>()) }
            else { None }
        }).collect::<String>()
    }).collect::<Vec<_>>().join("\n")
}

pub(crate) fn relations_path(part: &str) -> String { part.rsplit_once('/').map_or_else(|| format!("_rels/{part}.rels"), |(folder, name)| format!("{folder}/_rels/{name}.rels")) }

fn hyperlink(package: &Package, part: &str, id: &str) -> Result<Option<String>> {
    let document = parse(package.text(&relations_path(part))?)?;
    let target = document.root_element().children().find(|node| node.has_tag_name((REL, "Relationship")) && node.attribute("Id") == Some(id)
        && node.attribute("Type") == Some(format!("{R}/hyperlink").as_str()) && node.attribute("TargetMode") == Some("External"))
        .and_then(|node| node.attribute("Target"));
    if let Some(value) = target { crate::model::validate_hyperlink(value)?; }
    Ok(target.map(str::to_owned))
}

fn text_format(package: &Package, part: &str, body: Node<'_, '_>, template: Option<&Element>) -> Result<(f64, String, bool, TextFormat)> {
    let (mut size, mut shade, mut bold, mut format) = match template { Some(Element::Text { font_size, color, bold, format, .. }) => (*font_size, color.clone(), *bold, format.clone()), _ => (24.0, "@dk1".into(), false, TextFormat::default()) };
    let inherited_style = crate::rich_text::RunStyle::frame(size, &shade, bold, &format);
    let inherited_format = format.clone(); format.paragraphs.clear();
    let list = child(body, A, "lstStyle").and_then(|node| child(node, A, "lvl1pPr"));
    let paragraph = child(body, A, "p").and_then(|node| child(node, A, "pPr"));
    for node in [list, paragraph].into_iter().flatten() {
        if let Some(alignment) = node.attribute("algn") { format.alignment = match alignment { "ctr" => TextAlign::Center, "r" => TextAlign::Right, "just" | "dist" => TextAlign::Justify, _ => TextAlign::Left }; }
        if child(node, A, "buNone").is_some() { format.bullet = Bullet::None; }
        if child(node, A, "buChar").is_some() { format.bullet = Bullet::Bullet; }
        if child(node, A, "buAutoNum").is_some() { format.bullet = Bullet::Numbered; }
    }
    if let Some(anchor) = child(body, A, "bodyPr").and_then(|node| node.attribute("anchor")) { format.vertical = match anchor { "ctr" => VerticalAlign::Middle, "b" => VerticalAlign::Bottom, _ => VerticalAlign::Top }; }
    let run = body.descendants().find(|node| node.has_tag_name((A, "rPr")));
    for node in [list.and_then(|node| child(node, A, "defRPr")), paragraph.and_then(|node| child(node, A, "defRPr")), run].into_iter().flatten() {
        size = number(node, "sz", size * 75.0) / 75.0;
        shade = color(node, &shade);
        if let Some(value) = node.attribute("b") { bold = value == "1" || value == "true"; }
        if let Some(value) = node.attribute("i") { format.italic = value == "1" || value == "true"; }
        if let Some(value) = node.attribute("u") { format.underline = value != "none"; }
        if let Some(family) = child(node, A, "latin").and_then(|node| node.attribute("typeface")) { format.font_family = Some(match family { "+mj-lt" => "@major", "+mn-lt" => "@minor", value => value }.into()); }
        if let Some(id) = child(node, A, "hlinkClick").and_then(|node| node.attribute((R, "id"))) { format.hyperlink = hyperlink(package, part, id)?; }
    }
    format.inherit_layout = false;
    if let Ok(paragraphs) = read_rich_paragraphs(body, &inherited_style, &inherited_format) {
        let frame = crate::rich_text::RunStyle::frame(size, &shade, bold, &format);
        let complex = !(8.0..=120.0).contains(&size) || paragraphs.iter().any(|paragraph| {
            paragraph.runs.len() != 1 || paragraph.runs.iter().any(|run| run.style != frame || run.field.is_some())
                || paragraph.alignment.is_some_and(|alignment| alignment != format.alignment)
                || paragraph.bullet.is_some_and(|bullet| bullet != format.bullet)
                || paragraph.level.is_some() || paragraph.number_start.is_some() || !paragraph.tabs.is_empty()
                || paragraph.numbering.as_deref().is_some_and(|value| value != "arabicPeriod")
                || paragraph.bullet_character.as_deref().is_some_and(|value| value != "\u{2022}")
                || paragraph.margin_left.is_some_and(|value| value != if format.bullet == Bullet::None { 0 } else { (size * 9525.0).round() as i32 })
                || paragraph.indent.is_some_and(|value| value != if format.bullet == Bullet::None { 0 } else { (-size * 0.7 * 9525.0).round() as i32 })
                || paragraph.line_spacing.is_some_and(|value| value != crate::rich_text::Spacing::Percent(115000))
                || paragraph.space_before.is_some() || paragraph.space_after.is_some()
        });
        if complex {
            format.paragraphs = paragraphs;
            if !(8.0..=120.0).contains(&size) { size = inherited_style.font_size.unwrap_or(24.0).clamp(8.0, 120.0); }
        }
    }
    Ok((size, shade, bold, format))
}

fn rich_number<T: std::str::FromStr>(node: Node<'_, '_>, name: &str) -> Result<Option<T>> {
    node.attribute(name).map(|value| value.parse().map_err(|_| Error::Unsupported(format!("unrepresentable text {name}")))).transpose()
}

fn rich_run_style(node: Node<'_, '_>, style: &mut crate::rich_text::RunStyle) -> Result<()> {
    for (name, target) in [("b", &mut style.bold), ("i", &mut style.italic)] {
        if let Some(value) = node.attribute(name) { *target = Some(match value { "1" | "true" => true, "0" | "false" => false, _ => return Err(Error::Unsupported("native text boolean".into())) }); }
    }
    if let Some(value) = node.attribute("u") { style.underline = Some(match value { "sng" => true, "none" => false, _ => return Err(Error::Unsupported("native underline variant".into())) }); }
    if let Some(size) = rich_number::<u32>(node, "sz")? { style.font_size = Some(size as f64 / 75.0); }
    if let Some(baseline) = rich_number(node, "baseline")? { style.baseline = Some(baseline); }
    if let Some(language) = node.attribute("lang") { style.language = Some(language.into()); }
    if child(node, A, "solidFill").is_some() { style.color = Some(color(node, "@dk1")); }
    if let Some(highlight) = child(node, A, "highlight") {
        style.highlight = Some(if let Some(rgb) = child(highlight, A, "srgbClr").and_then(|node| node.attribute("val")) { rgb.into() }
            else if let Some(slot) = child(highlight, A, "schemeClr").and_then(|node| node.attribute("val")) { format!("@{slot}") }
            else { return Err(Error::Unsupported("native highlight color".into())); });
    }
    if let Some(family) = child(node, A, "latin").and_then(|node| node.attribute("typeface")) { style.font_family = Some(match family { "+mj-lt" => "@major", "+mn-lt" => "@minor", value => value }.into()); }
    style.validate()
}

fn rich_spacing(node: Node<'_, '_>) -> Result<crate::rich_text::Spacing> {
    use crate::rich_text::Spacing;
    if let Some(value) = child(node, A, "spcPct").and_then(|node| node.attribute("val")) { return value.parse().map(Spacing::Percent).map_err(|_| Error::Unsupported("native percentage spacing".into())); }
    if let Some(value) = child(node, A, "spcPts").and_then(|node| node.attribute("val")) { return value.parse().map(Spacing::Points).map_err(|_| Error::Unsupported("native point spacing".into())); }
    Err(Error::Unsupported("native paragraph spacing".into()))
}

fn rich_paragraph_style(node: Node<'_, '_>, paragraph: &mut crate::rich_text::RichParagraph) -> Result<()> {
    use crate::rich_text::{TabAlign, TabStop};
    if let Some(value) = node.attribute("algn") { paragraph.alignment = Some(match value { "l" => TextAlign::Left, "ctr" => TextAlign::Center, "r" => TextAlign::Right, "just" => TextAlign::Justify, _ => return Err(Error::Unsupported("native paragraph alignment".into())) }); }
    if let Some(level) = rich_number(node, "lvl")? { paragraph.level = Some(level); }
    if let Some(margin) = rich_number(node, "marL")? { paragraph.margin_left = Some(margin); }
    if let Some(indent) = rich_number(node, "indent")? { paragraph.indent = Some(indent); }
    for (tag, target) in [("lnSpc", &mut paragraph.line_spacing), ("spcBef", &mut paragraph.space_before), ("spcAft", &mut paragraph.space_after)] { if let Some(node) = child(node, A, tag) { *target = Some(rich_spacing(node)?); } }
    if child(node, A, "buNone").is_some() { paragraph.bullet = Some(Bullet::None); paragraph.bullet_character = None; paragraph.numbering = None; paragraph.number_start = None; }
    if let Some(bullet) = child(node, A, "buChar") { paragraph.bullet = Some(Bullet::Bullet); paragraph.bullet_character = bullet.attribute("char").map(str::to_owned); paragraph.numbering = None; paragraph.number_start = None; }
    if let Some(numbering) = child(node, A, "buAutoNum") { paragraph.bullet = Some(Bullet::Numbered); paragraph.bullet_character = None; paragraph.numbering = numbering.attribute("type").map(str::to_owned); paragraph.number_start = rich_number(numbering, "startAt")?; }
    if let Some(tabs) = child(node, A, "tabLst") {
        paragraph.tabs.clear();
        for tab in tabs.children().filter(|node| node.has_tag_name((A, "tab"))) {
            paragraph.tabs.push(TabStop { position: rich_number(tab, "pos")?.ok_or_else(|| Error::Unsupported("native tab position".into()))?, alignment: match tab.attribute("algn").unwrap_or("l") { "l" => TabAlign::Left, "ctr" => TabAlign::Center, "r" => TabAlign::Right, "dec" => TabAlign::Decimal, _ => return Err(Error::Unsupported("native tab alignment".into())) } });
        }
    }
    Ok(())
}

pub(crate) fn read_rich_paragraphs(body: Node<'_, '_>, inherited_style: &crate::rich_text::RunStyle, inherited_format: &TextFormat) -> Result<Vec<crate::rich_text::RichParagraph>> {
    read_rich_paragraphs_with_limit(body, inherited_style, inherited_format, 4000)
}

pub(crate) fn read_rich_paragraphs_with_limit(body: Node<'_, '_>, inherited_style: &crate::rich_text::RunStyle, inherited_format: &TextFormat, limit: usize) -> Result<Vec<crate::rich_text::RichParagraph>> {
    use crate::rich_text::{RichParagraph, RichRun};
    if body.descendants().any(|node| node.has_tag_name((A, "br"))) { return Err(Error::Unsupported("soft breaks are preserved as native XML".into())); }
    let mut result = Vec::new();
    for paragraph in body.children().filter(|node| node.has_tag_name((A, "p"))) {
        let properties = child(paragraph, A, "pPr");
        let level = properties.and_then(|node| node.attribute("lvl")).unwrap_or("0").parse::<u8>().map_err(|_| Error::Unsupported("native paragraph level".into()))?;
        if level > 8 { return Err(Error::Unsupported("native paragraph level".into())); }
        let list_name = format!("lvl{}pPr", level + 1);
        let list = child(body, A, "lstStyle").and_then(|node| child(node, A, &list_name));
        let mut value = RichParagraph { alignment: Some(inherited_format.alignment), bullet: Some(inherited_format.bullet), ..Default::default() };
        let mut style = inherited_style.clone();
        for properties in [list, properties].into_iter().flatten() {
            rich_paragraph_style(properties, &mut value)?;
            if let Some(default) = child(properties, A, "defRPr") { rich_run_style(default, &mut style)?; }
        }
        for run in paragraph.children().filter(|node| node.has_tag_name((A, "r")) || node.has_tag_name((A, "fld"))) {
            let mut resolved = style.clone();
            if let Some(properties) = child(run, A, "rPr") { rich_run_style(properties, &mut resolved)?; }
            let text = run.children().filter(|node| node.has_tag_name((A, "t"))).filter_map(|node| node.text()).collect();
            value.runs.push(RichRun { text, style: resolved, field: crate::fields::read_field(run)? });
        }
        if value.runs.is_empty() {
            if let Some(end) = child(paragraph, A, "endParaRPr") { rich_run_style(end, &mut style)?; }
            value.runs.push(RichRun { text: String::new(), style, field: None });
        }
        result.push(value);
    }
    crate::rich_text::validate_paragraphs_with_limit(&result, limit)?;
    Ok(result)
}

fn names(tree: Node<'_, '_>, aliases: Option<&crate::provenance::Identity>) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new(); let mut used = BTreeSet::new();
    for node in tree.descendants().filter(|node| is_shape(*node)) {
        let props = properties(node).ok_or_else(|| Error::Unsupported("native shape has no identity".into()))?;
        let numeric = props.attribute("id").ok_or_else(|| Error::Invalid("native shape ID missing".into()))?;
        let name = aliases.and_then(|entry| entry.objects.get(numeric)).map(String::as_str).or_else(|| props.attribute("name")).filter(|name| !name.is_empty() && name.chars().count() <= 80).unwrap_or("");
        let mut id = if name.is_empty() { format!("shape-{numeric}") } else { name.to_owned() };
        if used.contains(&id) { id = format!("shape-{numeric}"); }
        if !used.insert(id.clone()) || result.insert(numeric.into(), id).is_some() { return Err(Error::Invalid("duplicate native shape identity".into())); }
    }
    Ok(result)
}

fn read_chart(package: &Package, part: &str, node: Node<'_, '_>, id: String, bounds: [f64; 4]) -> Result<Element> {
    let extended = node.descendants().any(|node| node.has_tag_name((crate::chart_extended::NS, "chart")));
    let namespace = if extended { crate::chart_extended::NS } else { C };
    let relationship = node.descendants().find(|node| node.has_tag_name((namespace, "chart"))).and_then(|node| node.attribute((R, "id"))).ok_or_else(|| Error::Unsupported("chart reference missing".into()))?;
    let targets = relationship_targets(package, part, if extended { "chartEx" } else { "chart" })?;
    let path = targets.get(relationship).ok_or_else(|| Error::Invalid("chart target missing".into()))?;
    let document = parse(package.text(path)?)?;
    let (kind, categories, series, options) = crate::charts::read(&document, color)?;
    let [x, y, width, height] = bounds;
    Ok(Element::Chart { id, x, y, width, height, kind, categories, series, options })
}

fn read_element(package: &Package, part: &str, node: Node<'_, '_>, scale: (f64, f64), ids: &BTreeMap<String, String>, templates: &[Element], depth: usize) -> Result<Element> {
    let mut element = read_element_base(package, part, node, scale, ids, templates, depth)?;
    let mut visual = crate::visual::read(node)?;
    if matches!(element, Element::Shape { .. }) {
        if let Some(style) = &mut visual { style.rotation = None; }
        if visual.as_ref() == Some(&crate::visual::VisualStyle::default()) { visual = None; }
    }
    if let Some(target) = element.visual_mut() { *target = visual; }
    Ok(element)
}

fn read_element_base(package: &Package, part: &str, node: Node<'_, '_>, scale: (f64, f64), ids: &BTreeMap<String, String>, templates: &[Element], depth: usize) -> Result<Element> {
    if depth > 8 { return Err(Error::Limit("group nesting > 8".into())); }
    let numeric = properties(node).and_then(|node| node.attribute("id")).ok_or_else(|| Error::Invalid("shape identity".into()))?;
    let id = ids.get(numeric).ok_or_else(|| Error::Invalid("shape identity map".into()))?.clone();
    let ph = placeholder(node);
    let template = ph.as_ref().and_then(|ph| templates.iter().find(|element| matches!(element, Element::Text { format, .. } if format.placeholder.as_ref() == Some(ph))));
    let fallback = template.map(|element| { let (_, x, y, width, height) = element.bounds(); [x, y, width, height] });
    let xfrm = transform(node);
    let bounds = match xfrm {
        Some(transform) => {
            if node.has_tag_name((P, "graphicFrame")) && (number(transform, "rot", 0.0) != 0.0 || matches!(transform.attribute("flipH"), Some("1" | "true"))) { return Err(Error::Unsupported("flipped or rotated native frame".into())); }
            let offset = child(transform, A, "off"); let extent = child(transform, A, "ext");
            match (offset, extent) { (Some(offset), Some(extent)) => [number(offset, "x", 0.0) * scale.0, number(offset, "y", 0.0) * scale.1, (number(extent, "cx", 0.0) * scale.0).max(0.01), (number(extent, "cy", 0.0) * scale.1).max(0.01)], _ => fallback.ok_or_else(|| Error::Unsupported("missing or unresolved inherited geometry".into()))? }
        }
        None => fallback.ok_or_else(|| Error::Unsupported("missing or unresolved inherited geometry".into()))?,
    };
    let [x, y, width, height] = bounds;
    if node.has_tag_name((P, "grpSp")) {
        let transform = xfrm.ok_or_else(|| Error::Unsupported("group transform".into()))?;
        let offset = child(transform, A, "chOff").ok_or_else(|| Error::Unsupported("group child origin".into()))?;
        let origin_x = number(offset, "x", 0.0) / 9525.0; let origin_y = number(offset, "y", 0.0) / 9525.0;
        if !origin_x.is_finite() || !origin_y.is_finite() || origin_x.abs() > 1e6 || origin_y.abs() > 1e6 { return Err(Error::Unsupported("group child origin exceeds supported range".into())); }
        let extent = child(transform, A, "chExt").ok_or_else(|| Error::Unsupported("group child extent".into()))?;
        let mut children = node.children().filter(|node| is_shape(*node)).map(|node| read_element(package, part, node, (1.0 / 9525.0, 1.0 / 9525.0), ids, &[], depth + 1)).collect::<Result<Vec<_>>>()?;
        for element in &mut children {
            let (x,y) = match element {
                Element::Text {x,y,..} | Element::Rect {x,y,..} | Element::Polygon {x,y,..} | Element::Shape {x,y,..} | Element::Picture {x,y,..} | Element::Group {x,y,..} | Element::Connector {x,y,..} | Element::Table {x,y,..} | Element::Chart {x,y,..} => (x,y),
            };
            *x -= origin_x; *y -= origin_y;
        }
        return Ok(Element::Group { visual: None, id, x, y, width, height, view_width: number(extent, "cx", 0.0) / 9525.0, view_height: number(extent, "cy", 0.0) / 9525.0, children });
    }
    if node.has_tag_name((P, "pic")) {
        let fill = child(node, P, "blipFill").ok_or_else(|| Error::Unsupported("picture fill".into()))?;
        let reference = child(fill, A, "blip").and_then(|node| node.attribute((R, "embed"))).ok_or_else(|| Error::Unsupported("external or missing image".into()))?;
        let targets = relationship_targets(package, part, "image")?; let path = targets.get(reference).ok_or_else(|| Error::Invalid("image target missing".into()))?;
        let bytes = package.part(path)?;
        let mime_type = match image::guess_format(bytes) { Ok(image::ImageFormat::Png) => "image/png", Ok(image::ImageFormat::Jpeg) => "image/jpeg", _ => return Err(Error::Unsupported("non-raster picture".into())) }.into();
        let crop = child(fill, A, "srcRect").map(|node| Crop { left: number(node, "l", 0.0) / 100000.0, top: number(node, "t", 0.0) / 100000.0, right: number(node, "r", 0.0) / 100000.0, bottom: number(node, "b", 0.0) / 100000.0 }).unwrap_or_default();
        return Ok(Element::Picture { visual: None, svg: crate::vector::read_svg(package, part, node)?, id, x, y, width, height, base64: STANDARD.encode(bytes), mime_type, alt: properties(node).and_then(|node| node.attribute("descr")).unwrap_or("").into(), crop });
    }
    if node.has_tag_name((P, "cxnSp")) {
        let properties = child(node, P, "spPr").ok_or_else(|| Error::Unsupported("connector properties".into()))?;
        let line = child(properties, A, "ln");
        let flip_h = matches!(xfrm.and_then(|node| node.attribute("flipH")), Some("1" | "true"));
        let flip_v = matches!(xfrm.and_then(|node| node.attribute("flipV")), Some("1" | "true"));
        let connection = |name| node.descendants().find(|node| node.has_tag_name((A, name))).and_then(|node| Some(Connection { element_id: ids.get(node.attribute("id")?)?.clone(), site: number(node, "idx", 0.0) as u32 }));
        let mut points = if let Some(geometry) = child(properties, A, "custGeom") {
            let paths: Vec<_> = child(geometry, A, "pathLst").into_iter().flat_map(|node| node.children()).filter(|node| node.has_tag_name((A, "path"))).collect();
            if paths.len() != 1 { return Err(Error::Unsupported("complex connector path".into())); }
            let path = paths[0]; let path_width = number(path, "w", 0.0); let path_height = number(path, "h", 0.0);
            if !path_width.is_finite() || !path_height.is_finite() || path_width <= 0.0 || path_height <= 0.0 { return Err(Error::Unsupported("connector coordinate space".into())); }
            let mut points = Vec::new();
            for (index, command) in path.children().filter(|node| node.is_element()).enumerate() {
                if !command.has_tag_name((A, if index == 0 { "moveTo" } else { "lnTo" })) { return Err(Error::Unsupported("curved connector path".into())); }
                let point = child(command, A, "pt").ok_or_else(|| Error::Unsupported("connector point".into()))?;
                let coordinate = |name| point.attribute(name).and_then(|value| value.parse::<f64>().ok()).ok_or_else(|| Error::Unsupported("formula connector path".into()));
                points.push([coordinate("x")? / path_width, coordinate("y")? / path_height]);
            }
            Some(points)
        } else {
            let geometry = child(properties, A, "prstGeom").ok_or_else(|| Error::Unsupported("connector geometry".into()))?;
            let preset = geometry.attribute("prst").unwrap_or("");
            let allowed = match preset { "line" | "straightConnector1" | "bentConnector2" => 0, "bentConnector3" => 1, "bentConnector4" => 2, _ => return Err(Error::Unsupported("connector preset".into())) };
            let mut adjustments = BTreeMap::new();
            for guide in child(geometry, A, "avLst").into_iter().flat_map(|node| node.children()).filter(|node| node.is_element()) {
                let name = guide.attribute("name").unwrap_or("");
                let value = guide.attribute("fmla").and_then(|value| value.strip_prefix("val ")).and_then(|value| value.parse::<i64>().ok()).filter(|value| (0..=100000).contains(value));
                if !guide.has_tag_name((A, "gd")) || !["adj1", "adj2"][..allowed].contains(&name) || value.is_none() || adjustments.insert(name, value.unwrap_or_default() as f64 / 100000.0).is_some() { return Err(Error::Unsupported("connector adjustment formula".into())); }
            }
            if preset == "line" && !flip_h { None }
            else {
                let extent = xfrm.and_then(|node| child(node, A, "ext"));
                let right = if extent.is_some_and(|node| number(node, "cx", 0.0) == 0.0) { 0.0 } else { 1.0 };
                let bottom = if extent.is_some_and(|node| number(node, "cy", 0.0) == 0.0) { 0.0 } else { 1.0 };
                let horizontal = *adjustments.get("adj1").unwrap_or(&0.5) * right;
                let vertical = *adjustments.get("adj2").unwrap_or(&0.5) * bottom;
                Some(match preset {
                    "bentConnector2" => vec![[0.0, 0.0], [right, 0.0], [right, bottom]],
                    "bentConnector3" => vec![[0.0, 0.0], [horizontal, 0.0], [horizontal, bottom], [right, bottom]],
                    "bentConnector4" => vec![[0.0, 0.0], [horizontal, 0.0], [horizontal, vertical], [right, vertical], [right, bottom]],
                    _ => vec![[0.0, 0.0], [right, bottom]],
                })
            }
        };
        if let Some(points) = &mut points {
            points.dedup();
            for point in points { if flip_h { point[0] = ((1.0 - point[0]) * 1e6).round() / 1e6; } if flip_v { point[1] = ((1.0 - point[1]) * 1e6).round() / 1e6; } }
        }
        let routing = points.map(|points| crate::model::ConnectorRouting { points, start_arrow: line.and_then(|node| child(node, A, "headEnd")).is_some_and(|node| node.attribute("type") == Some("triangle")), dashed: line.and_then(|node| child(node, A, "prstDash")).is_some_and(|node| node.attribute("val") == Some("dash")) });
        return Ok(Element::Connector { visual: None, id, x, y, width, height, color: line.map_or_else(|| "@dk2".into(), |node| color(node, "@dk2")), stroke_width: line.map_or(1.0, |node| number(node, "w", 9525.0) / 9525.0), arrow: line.and_then(|node| child(node, A, "tailEnd")).is_some_and(|node| node.attribute("type").is_some_and(|value| value != "none")), flip_v: routing.is_none() && flip_v, start: connection("stCxn"), end: connection("endCxn"), routing });
    }
    if node.has_tag_name((P, "graphicFrame")) {
        if node.descendants().any(|node| node.has_tag_name((crate::chart_extended::NS, "chart"))) { return read_chart(package, part, node, id, bounds); }
        if let Some(table) = node.descendants().find(|node| node.has_tag_name((A, "tbl"))) {
            let (rows, font_size, format) = crate::table_format::read_native(table, width, height)?;
            return Ok(Element::Table { id, x, y, width, height, rows, font_size, format });
        }
        return read_chart(package, part, node, id, bounds);
    }
    let properties = child(node, P, "spPr");
    let body = child(node, P, "txBody");
    let preset = properties.and_then(|node| child(node, A, "prstGeom")).and_then(|node| node.attribute("prst")).unwrap_or("rect");
    let filled = properties.is_some_and(|node| child(node, A, "solidFill").is_some() || child(node, A, "gradFill").is_some());
    let line = properties.and_then(|node| child(node, A, "ln"));
    if let Some(geometry) = properties.and_then(|node| child(node, A, "custGeom")) {
        if crate::vector::native_requires_override(geometry) {
            if body.is_some() { return Err(Error::Unsupported("custom path text body".into())); }
            let path = crate::vector::read_path(geometry)?;
            return Ok(Element::Polygon { visual: None, id, x, y, width, height, points: path.points(), fill: properties.map(shape_fill).unwrap_or_else(|| "none".into()), stroke: line.map_or_else(|| "@dk1".into(), |node| color(node,"@dk1")), stroke_width: line.filter(|node| child(*node,A,"noFill").is_none()).map_or(0.0,|node| number(node,"w",9525.0)/9525.0) });
        }
        let paths: Vec<_> = child(geometry, A, "pathLst").into_iter().flat_map(|node| node.children()).filter(|node| node.has_tag_name((A, "path"))).collect();
        if paths.len() != 1 || body.is_some() { return Err(Error::Unsupported("complex custom geometry".into())); }
        let path = paths[0]; let path_width = number(path, "w", 0.0); let path_height = number(path, "h", 0.0);
        if path_width <= 0.0 || path_height <= 0.0 { return Err(Error::Unsupported("custom geometry coordinate space".into())); }
        let commands: Vec<_> = path.children().filter(|node| node.is_element()).collect();
        if !commands.first().is_some_and(|node| node.has_tag_name((A, "moveTo"))) || !commands.last().is_some_and(|node| node.has_tag_name((A, "close"))) { return Err(Error::Unsupported("open custom geometry".into())); }
        let mut points = Vec::new();
        for (index, command) in commands.iter().enumerate().take(commands.len() - 1) {
            if !command.has_tag_name((A, if index == 0 { "moveTo" } else { "lnTo" })) { return Err(Error::Unsupported("curved custom geometry".into())); }
            let point = child(*command, A, "pt").ok_or_else(|| Error::Unsupported("custom geometry point".into()))?;
            let coordinate = |name: &str| point.attribute(name).and_then(|value| value.parse::<f64>().ok()).ok_or_else(|| Error::Unsupported("formula custom geometry".into()));
            points.push([coordinate("x")? / path_width, coordinate("y")? / path_height]);
        }
        return Ok(Element::Polygon { visual: None, id, x, y, width, height, points, fill: properties.map(shape_fill).unwrap_or_else(|| "none".into()), stroke: line.map_or_else(|| "@dk1".into(), |node| color(node, "@dk1")), stroke_width: line.filter(|node| child(*node, A, "noFill").is_none()).map_or(0.0, |node| number(node, "w", 9525.0) / 9525.0) });
    }
    let tx_box = child(node, P, "nvSpPr").and_then(|node| child(node, P, "cNvSpPr")).and_then(|node| node.attribute("txBox")) == Some("1");
    if body.is_some() || ph.is_some() {
        let body = body.ok_or_else(|| Error::Unsupported("placeholder text body missing".into()))?;
        let text = body_text(body);
        let (font_size, shade, bold, mut format) = text_format(package, part, body, template)?;
        format.placeholder = ph;
        if template.is_some() && xfrm.is_none() {
            let mut inherited = template.unwrap().clone();
            let matches = if let Element::Text { font_size: template_size, color, bold: template_bold, format: template_format, .. } = &inherited { *template_size == font_size && color == &shade && *template_bold == bold && crate::fields::same_inherited_format(template_format, &format) } else { false };
            if matches {
                format.inherit_layout = true;
                if let Element::Text { id: next_id, text: next_text, format: next_format, .. } = &mut inherited { *next_id = id; *next_text = text; *next_format = format; }
                return Ok(inherited);
            }
        }
        if tx_box || format.placeholder.is_some() || (!filled && preset == "rect") {
            return Ok(Element::Text { visual: None, id, x, y, width, height, text, font_size, color: shade, bold, format });
        }
        return Ok(Element::Shape { visual: None, id, x, y, width, height, preset: preset.into(), fill: properties.map(shape_fill).unwrap_or_else(|| "@lt1".into()), stroke: line.map_or_else(|| "@dk1".into(), |node| color(node, "@dk1")), stroke_width: line.filter(|node| child(*node, A, "noFill").is_none()).map_or(0.0, |node| number(node, "w", 9525.0) / 9525.0), rotation: xfrm.map_or(0.0, |node| number(node, "rot", 0.0) / 60000.0), text, font_size, color: shade, bold, format });
    }
    if preset != "rect" { return Ok(Element::Shape { visual: None, id, x, y, width, height, preset: preset.into(), fill: properties.map(shape_fill).unwrap_or_else(|| "@lt1".into()), stroke: line.map_or_else(|| "@dk1".into(), |node| color(node, "@dk1")), stroke_width: line.map_or(0.0, |node| number(node, "w", 0.0) / 9525.0), rotation: xfrm.map_or(0.0, |node| number(node, "rot", 0.0) / 60000.0), text: String::new(), font_size: 24.0, color: "@dk1".into(), bold: false, format: TextFormat::default() }); }
    Ok(Element::Rect { visual: None, id, x, y, width, height, fill: properties.map(shape_fill).unwrap_or_else(|| "@lt1".into()) })
}

pub(crate) fn read_part(package: &Package, path: &str, id: String, scale: (f64, f64), templates: &[Element], warnings: &mut Vec<String>, metadata: Option<&crate::provenance::Metadata>, canvas: (u32, u32)) -> Result<(String, Option<String>, Vec<Element>, NativePart)> {
    let document = parse(package.text(path)?)?;
    let common = child(document.root_element(), P, "cSld").ok_or_else(|| Error::Invalid("native common slide data missing".into()))?;
    let tree = child(common, P, "spTree").ok_or_else(|| Error::Invalid("native shape tree missing".into()))?;
    let ids = names(tree, metadata.and_then(|metadata| metadata.identities.iter().find(|identity| identity.part == path)))?;
    let mut elements = Vec::new();
    for node in tree.children().filter(|node| node.is_element() && !["nvGrpSpPr", "grpSpPr", "extLst"].contains(&node.tag_name().name())) {
        let result = if is_shape(node) { read_element(package, path, node, scale, &ids, templates, 0) } else { Err(Error::Unsupported(node.tag_name().name().into())) };
        match result {
            Ok(mut element) => {
                if !document.root_element().has_tag_name((P, "sld")) { if let Element::Text { format, .. } = &mut element { format.inherit_layout = false; } }
                if !matches!(element, Element::Connector { .. }) && crate::model::validate_elements(std::slice::from_ref(&element), (f64::from(canvas.0), f64::from(canvas.1)), 0, &mut BTreeSet::new(), &mut 0, &mut 0).is_err() { warnings.push(format!("{path}: {} retained but outside supported scene bounds", element.bounds().0)); }
                else { elements.push(element); }
            }
            Err(error) => warnings.push(format!("{path}: retained unsupported object ({error})")),
        }
    }
    let available: BTreeSet<_> = elements.iter().map(|element| element.bounds().0.to_owned()).collect();
    for element in &mut elements {
        if let Element::Connector { start, end, .. } = element {
            for connection in [start, end] { if connection.as_ref().is_some_and(|connection| !available.contains(&connection.element_id)) { *connection = None; warnings.push(format!("{path}: connection to an unpreviewed object retained in source")); } }
        }
    }
    Ok((common.attribute("name").unwrap_or(&id).chars().take(100).collect(), background(document.root_element()), elements, NativePart { path: path.into(), id, nodes: ids.into_iter().map(|(numeric, id)| (id, numeric)).collect() }))
}

pub(crate) fn read_theme(package: &Package, path: &str) -> Result<Theme> {
    let document = parse(package.text(path)?)?;
    let root = document.root_element();
    let elements = child(root, A, "themeElements").ok_or_else(|| Error::Unsupported("theme elements missing".into()))?;
    let scheme = child(elements, A, "clrScheme").ok_or_else(|| Error::Unsupported("theme colors missing".into()))?;
    let mut theme = Theme::default(); theme.name = root.attribute("name").unwrap_or("Imported theme").chars().take(80).collect();
    for key in crate::design::COLOR_KEYS {
        let entry = child(scheme, A, key).ok_or_else(|| Error::Unsupported("theme color slot missing".into()))?;
        let rgb = child(entry, A, "srgbClr").and_then(|node| node.attribute("val")).or_else(|| child(entry, A, "sysClr").and_then(|node| node.attribute("lastClr"))).ok_or_else(|| Error::Unsupported("theme color representation".into()))?;
        theme.colors.insert(key.into(), rgb.into());
    }
    if let Some(scheme) = child(elements, A, "fontScheme") {
        let major = child(scheme, A, "majorFont"); let minor = child(scheme, A, "minorFont");
        let face = |node: Option<Node<'_, '_>>, tag, fallback: &str| node.and_then(|node| child(node, A, tag)).and_then(|node| node.attribute("typeface")).filter(|value| !value.is_empty()).unwrap_or(fallback).to_owned();
        theme.fonts = ThemeFonts { major: face(major, "latin", "Aptos"), minor: face(minor, "latin", "Aptos"), east_asian: face(minor, "ea", "Yu Gothic"), complex_script: face(minor, "cs", "Arial") };
    }
    crate::design::validate_theme(&theme)?; Ok(theme)
}

pub(crate) fn read(package: &Package) -> Result<NativeDeck> {
    let metadata = crate::provenance::read(package)?.map(|(_, value)| value);
    let main = relationship_targets(package, "", "officeDocument")?.into_values().next().ok_or_else(|| Error::Invalid("presentation missing".into()))?;
    let design_identities = crate::native_design::read_identities(package, &main)?;
    let identity = |path: &str, fallback: String| {
        if let Some(id) = design_identities.as_ref().and_then(|metadata| metadata.identity(path)) { return id.to_owned(); }
        if let Some(entry) = metadata.as_ref().and_then(|metadata| metadata.identities.iter().find(|entry| entry.part == path)) { return entry.id.clone(); }
        let mut id = fallback.clone(); let mut suffix = 1;
        while metadata.as_ref().is_some_and(|metadata| metadata.identities.iter().any(|entry| entry.id == id)) { id = format!("{fallback}-native-{suffix}"); suffix += 1; }
        id
    };
    let roots = relationship_targets(package, "", "officeDocument")?;
    if roots.len() != 1 { return Err(Error::Invalid("expected one presentation part".into())); }
    let main = roots.values().next().unwrap();
    let types = parse(package.text("[Content_Types].xml")?)?;
    let content_type = types.root_element().children().find(|node| node.has_tag_name((CT, "Override")) && node.attribute("PartName") == Some(format!("/{main}").as_str())).and_then(|node| node.attribute("ContentType"));
    if content_type != Some("application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml") || package.parts().keys().any(|name| name.to_ascii_lowercase().ends_with("vbaproject.bin")) { return Err(Error::Unsupported("only non-macro Open XML PPTX presentations are supported".into())); }
    let presentation = parse(package.text(main)?)?;
    let size = child(presentation.root_element(), P, "sldSz").ok_or_else(|| Error::Unsupported("expected Transitional Open XML PPTX slide size".into()))?;
    let width = number(size, "cx", 0.0); let height = number(size, "cy", 0.0);
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 { return Err(Error::Invalid("invalid native slide dimensions".into())); }
    let canvas = crate::canvas::native_size(package, main)?;
    let scale = (f64::from(canvas.0) / width, f64::from(canvas.1) / height);
    let master_targets = relationship_targets(package, main, "slideMaster")?;
    let master_list = child(presentation.root_element(), P, "sldMasterIdLst").ok_or_else(|| Error::Unsupported("master list missing".into()))?;
    let mut masters = Vec::new(); let mut layouts = Vec::new(); let mut master_parts = Vec::new(); let mut layout_parts = Vec::new(); let mut warnings = Vec::new(); let mut theme = None;
    for node in master_list.children().filter(|node| node.has_tag_name((P, "sldMasterId"))) {
        let path = node.attribute((R, "id")).and_then(|id| master_targets.get(id)).ok_or_else(|| Error::Invalid("master relationship missing".into()))?;
        let master_id = identity(path, format!("master-{}", masters.len() + 1));
        let (name, background, elements, binding) = read_part(package, path, master_id.clone(), scale, &[], &mut warnings, metadata.as_ref(), canvas)?;
        let theme_targets = relationship_targets(package, path, "theme")?;
        let theme_path = theme_targets.values().next().ok_or_else(|| Error::Unsupported("master theme missing".into()))?;
        let current_theme = read_theme(package, theme_path)?;
        if theme.is_none() { theme = Some(current_theme.clone()); }
        let override_theme = if crate::canonical::bytes(&theme)? != crate::canonical::bytes(&Some(&current_theme))? { Some(current_theme) } else { None };
        let master_xml = parse(package.text(path)?)?;
        let layout_list = child(master_xml.root_element(), P, "sldLayoutIdLst").ok_or_else(|| Error::Unsupported("master layout list missing".into()))?;
        let targets = relationship_targets(package, path, "slideLayout")?;
        for layout in layout_list.children().filter(|node| node.has_tag_name((P, "sldLayoutId"))) {
            let path = layout.attribute((R, "id")).and_then(|id| targets.get(id)).ok_or_else(|| Error::Invalid("layout relationship missing".into()))?;
            let id = identity(path, format!("layout-{}", layouts.len() + 1));
            let (name, background, elements, binding) = read_part(package, path, id.clone(), scale, &elements, &mut warnings, metadata.as_ref(), canvas)?;
            layouts.push(SlideLayout { id, name, master_id: master_id.clone(), background, elements }); layout_parts.push(binding);
        }
        masters.push(Master { id: master_id, name, background: background.unwrap_or_else(|| "@lt1".into()), elements, theme: override_theme }); master_parts.push(binding);
    }
    if let Some(identities) = &design_identities { identities.order(&master_parts, &mut layouts, &mut layout_parts)?; }
    let design = Design { theme: theme.unwrap_or_default(), masters, layouts };
    let paths = slide_paths(package)?;
    let mut slides = Vec::new(); let mut slide_parts = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        let targets = relationship_targets(package, path, "slideLayout")?;
        let layout = targets.values().next().and_then(|path| layout_parts.iter().position(|entry| &entry.path == path)).map(|position| &design.layouts[position]).ok_or_else(|| Error::Unsupported("slide layout unavailable".into()))?;
        let id = identity(path, format!("slide-{}", index + 1));
        let (name, background, elements, binding) = read_part(package, path, id.clone(), scale, &layout.elements, &mut warnings, metadata.as_ref(), canvas)?;
        let xml = parse(package.text(path)?)?;
        let note_targets = relationship_targets(package, path, "notesSlide")?;
        let mut notes_paragraphs = Vec::new();
        let notes = if let Some(path) = note_targets.values().next() {
            let document = parse(package.text(path)?)?;
            let bodies: Vec<_> = document.descendants().filter(|node| node.has_tag_name((P, "sp")) && placeholder(*node).is_some_and(|ph| ph.kind == PlaceholderKind::Body)).filter_map(|node| child(node, P, "txBody")).collect();
            if note_targets.len() == 1 && bodies.len() == 1 {
                let body = bodies[0];
                let has_text = body.descendants().any(|node| node.has_tag_name((A, "t")) && node.text().is_some_and(|value| !value.is_empty()));
                let rich = body.children().filter(|node| node.has_tag_name((A, "p"))).any(|paragraph| paragraph.children().filter(|node| node.has_tag_name((A, "r"))).count() > 1)
                    || body.descendants().any(|node| node.has_tag_name((A, "fld")) || node.has_tag_name((A, "tabLst")) || node.has_tag_name((A, "buAutoNum")) || node.has_tag_name((A, "buChar")) || node.attribute("b") == Some("1") || node.attribute("i") == Some("1") || node.attribute("u") == Some("sng") || node.attribute("baseline").is_some() || node.has_tag_name((A, "highlight")));
                let rich = rich || body.descendants().any(|node| node.attribute("sz").is_some_and(|size| size != "1200")
                    || node.has_tag_name((A, "srgbClr")) && node.attribute("val") != Some("202525")
                    || node.has_tag_name((A, "schemeClr"))
                    || node.has_tag_name((A, "latin")) && node.attribute("typeface").is_some_and(|family| !["+mn-lt", "Aptos"].contains(&family))
                    || has_text && node.attribute("lang").is_some_and(|language| language != "ja-JP")
                    || has_text && ["marL", "indent", "lvl"].iter().any(|name| node.attribute(*name).is_some_and(|value| value != "0"))
                    || node.attribute("algn").is_some_and(|alignment| alignment != "l")
                    || node.has_tag_name((A, "spcBef")) || node.has_tag_name((A, "spcAft"))
                    || has_text && node.has_tag_name((A, "spcPts"))
                    || node.has_tag_name((A, "spcPct")) && node.attribute("val") != Some("115000"));
                if rich { match read_rich_paragraphs_with_limit(body, &Default::default(), &Default::default(), 8000) { Ok(value) => notes_paragraphs = value, Err(error) => warnings.push(format!("{path}: rich notes retained ({error})")) } }
            }
            bodies.into_iter().map(body_text).collect::<Vec<_>>().join("\n")
        } else { String::new() };
        let review = crate::review::read_slide(package, &binding, &elements)?;
        slides.push(Slide { id, title: name, background: background.clone().unwrap_or_else(|| "@lt1".into()), elements, notes, notes_paragraphs, layout_id: Some(layout.id.clone()), inherit_background: background.is_none(), hide_master_graphics: xml.root_element().attribute("showMasterSp") == Some("0"), native_source_id: None, review });
        slide_parts.push(binding);
    }
    let title = package.parts().iter().filter(|(path, _)| path.starts_with("docProps/") && path.ends_with(".xml")).find_map(|(_, value)| std::str::from_utf8(value).ok().and_then(|value| parse(value).ok()).and_then(|document| document.descendants().find(|node| node.has_tag_name(("http://purl.org/dc/elements/1.1/", "title"))).and_then(|node| node.text()).map(str::to_owned)))
        .unwrap_or_else(|| slides.first().map(|slide| slide.title.clone()).unwrap_or_else(|| "Presentation".into()));
    let (embedded_fonts, font_inventory) = crate::fonts::read_native(package, main)?;
    if !font_inventory.is_empty() { warnings.push("Embedded fonts are retained. Only supported full fonts can be loaded, after license acknowledgement; other font encodings remain opaque.".into()); }
    let auxiliary_design = crate::native_design::read_auxiliary(package, main, &mut warnings)?;
    let deck = Deck { version: 1, title: title.chars().take(120).collect(), width: canvas.0, height: canvas.1, slides, design: Some(design), embedded_fonts, auxiliary_design };
    warnings.extend(crate::fields::evaluation_warnings(&deck, None, "en-US"));
    validate_deck(&deck)?;
    warnings.insert(0, "Opened from standard PPTX XML. Unknown content is retained; unsupported edits are rejected. Preview is not Office visual parity.".into());
    Ok(NativeDeck { deck, slides: slide_parts, masters: master_parts, layouts: layout_parts, warnings, metadata })
}

pub(crate) fn save(bytes: Vec<u8>, deck: &Deck) -> Result<Vec<u8>> {
    validate_deck(deck)?;
    let mut package = Package::open(bytes.clone())?;
    let original = read(&package)?;
    let mut compatible = deck.clone();
    if compatible.auxiliary_design.is_none() { compatible.auxiliary_design = original.deck.auxiliary_design.clone(); }
    for slide in &mut compatible.slides {
        let source = slide.native_source_id.as_deref().unwrap_or(&slide.id);
        if let Some(before) = original.deck.slides.iter().find(|before| before.id == source) {
            if slide.notes_paragraphs.is_empty() && slide.notes == before.notes { slide.notes_paragraphs = before.notes_paragraphs.clone(); }
        }
    }
    if crate::canonical::bytes(&original.deck)? == crate::canonical::bytes(&compatible)? { return Ok(bytes); }
    if package.parts().keys().any(|name| name.to_ascii_lowercase().starts_with("_xmlsignatures/")) { return Err(Error::Unsupported("editing signed packages".into())); }
    crate::native_save::save(&mut package, &original, &compatible)
}