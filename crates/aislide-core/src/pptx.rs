use crate::{model::{Deck, Element, TextFormat, TextAlign, VerticalAlign, Bullet, PlaceholderKind, validate_deck, valid_text}, design::{Design, Theme}, package::Package, Error, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use xmlwriter::{Options, Indent, XmlWriter};
use base64::{Engine, engine::general_purpose::STANDARD};

const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";

pub(crate) fn xml(root: &str, build: impl FnOnce(&mut XmlWriter)) -> Vec<u8> {
    let mut writer = XmlWriter::new(Options { indent: Indent::None, ..Options::default() });
    writer.start_element(root);
    writer.write_attribute("xmlns:p", P);
    writer.write_attribute("xmlns:a", A);
    writer.write_attribute("xmlns:r", R);
    build(&mut writer);
    writer.end_element();
    writer.end_document().into_bytes()
}

pub(crate) fn empty(writer: &mut XmlWriter, name: &str, attrs: &[(&str, &str)]) {
    writer.start_element(name);
    for (key, value) in attrs { writer.write_attribute(key, &quick_xml::escape::escape(*value)); }
    writer.end_element();
}

fn group(writer: &mut XmlWriter) {
    writer.start_element("p:nvGrpSpPr");
    empty(writer, "p:cNvPr", &[("id", "1"), ("name", "")]);
    empty(writer, "p:cNvGrpSpPr", &[]);
    empty(writer, "p:nvPr", &[]);
    writer.end_element();
    writer.start_element("p:grpSpPr");
    writer.start_element("a:xfrm");
    empty(writer, "a:off", &[("x", "0"), ("y", "0")]);
    empty(writer, "a:ext", &[("cx", "0"), ("cy", "0")]);
    empty(writer, "a:chOff", &[("x", "0"), ("y", "0")]);
    empty(writer, "a:chExt", &[("cx", "0"), ("cy", "0")]);
    writer.end_element();
    writer.end_element();
}

pub(crate) fn color(writer: &mut XmlWriter, value: &str) {
    writer.start_element("a:solidFill");
    if let Some(slot) = value.strip_prefix('@') { empty(writer, "a:schemeClr", &[("val", slot)]); }
    else { empty(writer, "a:srgbClr", &[("val", value)]); }
    writer.end_element();
}

fn transform(writer: &mut XmlWriter, name: &str, x: f64, y: f64, width: f64, height: f64) {
    writer.start_element(name);
    empty(writer, "a:off", &[("x", &emu(x)), ("y", &emu(y))]);
    empty(writer, "a:ext", &[("cx", &emu(width)), ("cy", &emu(height))]);
    writer.end_element();
}

fn emu(value: f64) -> String { (value * 9525.0).round().to_string() }

fn text_body(writer: &mut XmlWriter, name: &str, text: &str, size: f64, shade: &str, bold: bool) {
    formatted_text_body(writer, name, text, size, shade, bold, &TextFormat::default(), None, &Theme::default());
}

fn formatted_text_body(writer: &mut XmlWriter, name: &str, text: &str, size: f64, shade: &str, bold: bool, format: &TextFormat, link_id: Option<&str>, theme: &Theme) {
    writer.start_element(name);
    if format.inherit_layout && format.placeholder.is_some() {
        empty(writer, "a:bodyPr", &[]); empty(writer, "a:lstStyle", &[]);
        for line in text.split('\n') {
            writer.start_element("a:p"); writer.start_element("a:r"); writer.start_element("a:rPr"); writer.write_attribute("lang", "ja-JP");
            if let Some(link) = link_id { empty(writer, "a:hlinkClick", &[("r:id", link)]); } writer.end_element();
            writer.start_element("a:t"); writer.write_text(&quick_xml::escape::escape(line)); writer.end_element(); writer.end_element(); writer.end_element();
        }
        writer.end_element(); return;
    }
    writer.start_element("a:bodyPr");
    for key in ["lIns", "tIns", "rIns", "bIns"] { writer.write_attribute(key, "0"); }
    writer.write_attribute("wrap", "square");
    writer.write_attribute("anchor", match format.vertical { VerticalAlign::Top => "t", VerticalAlign::Middle => "ctr", VerticalAlign::Bottom => "b" });
    empty(writer, "a:noAutofit", &[]);
    writer.end_element();
    writer.start_element("a:lstStyle");
    if format.placeholder.is_some() {
        writer.start_element("a:lvl1pPr"); paragraph_properties(writer, size, format);
        run_properties(writer, "a:defRPr", size, shade, bold, format, None, theme);
        writer.end_element();
    }
    writer.end_element();
    for line in text.split('\n') {
        writer.start_element("a:p");
        writer.start_element("a:pPr");
        paragraph_properties(writer, size, format);
        writer.end_element();
        writer.start_element("a:r");
        run_properties(writer, "a:rPr", size, shade, bold, format, link_id, theme);
        writer.start_element("a:t");
        writer.write_text(&quick_xml::escape::escape(line));
        writer.end_element();
        writer.end_element();
        empty(writer, "a:endParaRPr", &[("lang", "ja-JP"), ("sz", &(size * 75.0).round().to_string())]);
        writer.end_element();
    }
    writer.end_element();
}

fn paragraph_properties(writer: &mut XmlWriter, size: f64, format: &TextFormat) {
    writer.write_attribute("algn", match format.alignment { TextAlign::Center => "ctr", TextAlign::Right => "r", TextAlign::Justify => "just", TextAlign::Left => "l" });
    if format.bullet != Bullet::None { writer.write_attribute("marL", &emu(size)); writer.write_attribute("indent", &emu(-size * 0.7)); }
    writer.start_element("a:lnSpc"); empty(writer, "a:spcPct", &[("val", "115000")]); writer.end_element();
    match format.bullet {
        Bullet::Bullet => empty(writer, "a:buChar", &[("char", "\u{2022}")]),
        Bullet::Numbered => empty(writer, "a:buAutoNum", &[("type", "arabicPeriod")]),
        Bullet::None => empty(writer, "a:buNone", &[]),
    }
}

fn run_properties(writer: &mut XmlWriter, tag: &str, size: f64, shade: &str, bold: bool, format: &TextFormat, link_id: Option<&str>, theme: &Theme) {
    writer.start_element(tag); writer.write_attribute("lang", "ja-JP"); writer.write_attribute("sz", &(size * 75.0).round().to_string()); writer.write_attribute("b", if bold { "1" } else { "0" });
    writer.write_attribute("i", if format.italic { "1" } else { "0" }); writer.write_attribute("u", if format.underline { "sng" } else { "none" });
    color(writer, shade);
    let (latin, east, complex) = match format.font_family.as_deref() {
        Some("@major") => ("+mj-lt", "+mj-ea", "+mj-cs"),
        None | Some("@minor") => ("+mn-lt", "+mn-ea", "+mn-cs"),
        Some(family) => (family, theme.fonts.east_asian.as_str(), theme.fonts.complex_script.as_str()),
    };
    empty(writer, "a:latin", &[("typeface", latin)]); empty(writer, "a:ea", &[("typeface", east)]); empty(writer, "a:cs", &[("typeface", complex)]);
    if let Some(link) = link_id { empty(writer, "a:hlinkClick", &[("r:id", link)]); }
    writer.end_element();
}

fn shape(writer: &mut XmlWriter, element: &Element, id: usize, ids: &BTreeMap<&str, usize>, theme: &Theme) {
    let (name, x, y, width, height) = element.bounds();
    if let Element::Group { view_width, view_height, children, .. } = element {
        writer.start_element("p:grpSp"); writer.start_element("p:nvGrpSpPr");
        empty(writer, "p:cNvPr", &[("id", &id.to_string()), ("name", name)]); empty(writer, "p:cNvGrpSpPr", &[]); empty(writer, "p:nvPr", &[]); writer.end_element();
        writer.start_element("p:grpSpPr"); writer.start_element("a:xfrm");
        empty(writer, "a:off", &[("x", &emu(x)), ("y", &emu(y))]); empty(writer, "a:ext", &[("cx", &emu(width)), ("cy", &emu(height))]);
        empty(writer, "a:chOff", &[("x", "0"), ("y", "0")]); empty(writer, "a:chExt", &[("cx", &emu(*view_width)), ("cy", &emu(*view_height))]); writer.end_element(); writer.end_element();
        for child in children { shape(writer, child, ids[child.bounds().0], ids, theme); }
        writer.end_element(); return;
    }
    if let Element::Picture { alt, crop, .. } = element {
        writer.start_element("p:pic"); writer.start_element("p:nvPicPr");
        empty(writer, "p:cNvPr", &[("id", &id.to_string()), ("name", name), ("descr", alt)]);
        writer.start_element("p:cNvPicPr"); empty(writer, "a:picLocks", &[("noChangeAspect", "1")]); writer.end_element(); empty(writer, "p:nvPr", &[]); writer.end_element();
        writer.start_element("p:blipFill"); empty(writer, "a:blip", &[("r:embed", &format!("rIdShape{id}"))]);
        empty(writer, "a:srcRect", &[("l", &(crop.left * 100000.0).round().to_string()), ("t", &(crop.top * 100000.0).round().to_string()), ("r", &(crop.right * 100000.0).round().to_string()), ("b", &(crop.bottom * 100000.0).round().to_string())]);
        writer.start_element("a:stretch"); empty(writer, "a:fillRect", &[]); writer.end_element(); writer.end_element();
        writer.start_element("p:spPr"); transform(writer, "a:xfrm", x, y, width, height);
        writer.start_element("a:prstGeom"); writer.write_attribute("prst", "rect"); empty(writer, "a:avLst", &[]); writer.end_element(); writer.end_element(); writer.end_element(); return;
    }
    if let Element::Connector { color: shade, stroke_width, arrow, flip_v, start, end, routing, .. } = element {
        writer.start_element("p:cxnSp"); writer.start_element("p:nvCxnSpPr"); empty(writer, "p:cNvPr", &[("id", &id.to_string()), ("name", name)]);
        writer.start_element("p:cNvCxnSpPr");
        for (tag, connection) in [("a:stCxn", start), ("a:endCxn", end)] {
            if let Some(connection) = connection { empty(writer, tag, &[("id", &ids[connection.element_id.as_str()].to_string()), ("idx", &connection.site.to_string())]); }
        }
        writer.end_element(); empty(writer, "p:nvPr", &[]); writer.end_element();
        let preset = routing.as_ref().and_then(|route| route.native_preset());
        writer.start_element("p:spPr"); writer.start_element("a:xfrm");
        if preset.as_ref().is_some_and(|preset| preset.flip_h) { writer.write_attribute("flipH", "1"); }
        if *flip_v || preset.as_ref().is_some_and(|preset| preset.flip_v) { writer.write_attribute("flipV", "1"); }
        let native_width = if preset.as_ref().is_some_and(|preset| preset.zero_width) { 0.0 } else { width };
        let native_height = if preset.as_ref().is_some_and(|preset| preset.zero_height) { 0.0 } else { height };
        empty(writer, "a:off", &[("x", &emu(x)), ("y", &emu(y))]); empty(writer, "a:ext", &[("cx", &emu(native_width)), ("cy", &emu(native_height))]); writer.end_element();
        if let Some(preset) = preset {
            writer.start_element("a:prstGeom"); writer.write_attribute("prst", preset.name); writer.start_element("a:avLst");
            for (name, value) in preset.adjustments { empty(writer, "a:gd", &[("name", name), ("fmla", &format!("val {}", value.round()))]); }
            writer.end_element(); writer.end_element();
        } else { writer.start_element("a:prstGeom"); writer.write_attribute("prst", "line"); empty(writer, "a:avLst", &[]); writer.end_element(); }
        writer.start_element("a:ln"); writer.write_attribute("w", &emu(*stroke_width)); color(writer, shade); empty(writer, "a:prstDash", &[("val", if routing.as_ref().is_some_and(|route| route.dashed) { "dash" } else { "solid" })]);
        if routing.as_ref().is_some_and(|route| route.start_arrow) { empty(writer, "a:headEnd", &[("type", "triangle")]); }
        if *arrow { empty(writer, "a:tailEnd", &[("type", "triangle")]); }
        writer.end_element(); writer.end_element(); writer.end_element(); return;
    }
    if let Element::Chart { .. } = element {
        writer.start_element("p:graphicFrame"); writer.start_element("p:nvGraphicFramePr");
        empty(writer, "p:cNvPr", &[("id", &id.to_string()), ("name", name)]);
        empty(writer, "p:cNvGraphicFramePr", &[]); empty(writer, "p:nvPr", &[]); writer.end_element();
        transform(writer, "p:xfrm", x, y, width, height);
        writer.start_element("a:graphic"); writer.start_element("a:graphicData"); writer.write_attribute("uri", crate::charts::CHART_NS);
        empty(writer, "c:chart", &[("xmlns:c", crate::charts::CHART_NS), ("r:id", &format!("rIdShape{id}"))]);
        writer.end_element(); writer.end_element(); writer.end_element();
        return;
    }
    if let Element::Table { rows, font_size, .. } = element {
        writer.start_element("p:graphicFrame");
        writer.start_element("p:nvGraphicFramePr");
        empty(writer, "p:cNvPr", &[("id", &id.to_string()), ("name", name)]);
        empty(writer, "p:cNvGraphicFramePr", &[]);
        empty(writer, "p:nvPr", &[]);
        writer.end_element();
        transform(writer, "p:xfrm", x, y, width, height);
        writer.start_element("a:graphic");
        writer.start_element("a:graphicData");
        writer.write_attribute("uri", "http://schemas.openxmlformats.org/drawingml/2006/table");
        writer.start_element("a:tbl");
        empty(writer, "a:tblPr", &[("firstRow", "1"), ("bandRow", "1")]);
        writer.start_element("a:tblGrid");
        for _column in &rows[0] { empty(writer, "a:gridCol", &[("w", &emu(width / rows[0].len() as f64))]); }
        writer.end_element();
        for (row_index, row) in rows.iter().enumerate() {
            writer.start_element("a:tr");
            writer.write_attribute("h", &emu(height / rows.len() as f64));
            for cell in row {
                writer.start_element("a:tc");
                formatted_text_body(writer, "a:txBody", cell, *font_size, if row_index == 0 { "@lt1" } else { "@dk1" }, row_index == 0, &TextFormat::default(), None, theme);
                writer.start_element("a:tcPr");
                for key in ["marL", "marR", "marT", "marB"] { writer.write_attribute(key, "57150"); }
                color(writer, if row_index == 0 { "@accent1" } else if row_index % 2 == 0 { "@lt2" } else { "@lt1" });
                writer.end_element();
                writer.end_element();
            }
            writer.end_element();
        }
        for _close in 0..4 { writer.end_element(); }
        return;
    }
    writer.start_element("p:sp");
    writer.start_element("p:nvSpPr");
    empty(writer, "p:cNvPr", &[("id", &id.to_string()), ("name", name)]);
    empty(writer, "p:cNvSpPr", if matches!(element, Element::Text { .. }) { &[("txBox", "1")] } else { &[] });
    writer.start_element("p:nvPr");
    if let Element::Text { format, .. } = element {
        if let Some(placeholder) = &format.placeholder {
            empty(writer, "p:ph", &[("type", match placeholder.kind { PlaceholderKind::Title => "title", PlaceholderKind::Body => "body", PlaceholderKind::Subtitle => "subTitle", PlaceholderKind::Footer => "ftr", PlaceholderKind::Date => "dt", PlaceholderKind::SlideNumber => "sldNum" }), ("idx", &placeholder.index.to_string())]);
        }
    }
    writer.end_element();
    writer.end_element();
    writer.start_element("p:spPr");
    let inherited = matches!(element, Element::Text { format, .. } if format.inherit_layout && format.placeholder.is_some());
    if !inherited {
    if let Element::Shape { rotation, .. } = element {
        writer.start_element("a:xfrm"); writer.write_attribute("rot", &((*rotation * 60000.0).round() as i64).to_string());
        empty(writer, "a:off", &[("x", &emu(x)), ("y", &emu(y))]); empty(writer, "a:ext", &[("cx", &emu(width)), ("cy", &emu(height))]); writer.end_element();
    } else { transform(writer, "a:xfrm", x, y, width, height); }
    if let Element::Polygon { points, .. } = element {
        writer.start_element("a:custGeom");
        for tag in ["a:avLst", "a:gdLst", "a:ahLst", "a:cxnLst"] { empty(writer, tag, &[]); }
        empty(writer, "a:rect", &[("l", "0"), ("t", "0"), ("r", "r"), ("b", "b")]);
        writer.start_element("a:pathLst"); writer.start_element("a:path"); writer.write_attribute("w", "1000000"); writer.write_attribute("h", "1000000");
        for (index, point) in points.iter().enumerate() { writer.start_element(if index == 0 { "a:moveTo" } else { "a:lnTo" }); empty(writer, "a:pt", &[("x", &(point[0] * 1000000.0).round().to_string()), ("y", &(point[1] * 1000000.0).round().to_string())]); writer.end_element(); }
        empty(writer, "a:close", &[]); writer.end_element(); writer.end_element(); writer.end_element();
    } else {
        writer.start_element("a:prstGeom");
        writer.write_attribute("prst", if let Element::Shape { preset, .. } = element { preset.as_str() } else { "rect" });
        empty(writer, "a:avLst", &[]); writer.end_element();
    }
    if let Element::Rect { fill, .. } | Element::Shape { fill, .. } | Element::Polygon { fill, .. } = element { if fill == "none" { empty(writer, "a:noFill", &[]); } else { color(writer, fill); } }
    else { empty(writer, "a:noFill", &[]); }
    writer.start_element("a:ln");
    if let Element::Shape { stroke, stroke_width, .. } | Element::Polygon { stroke, stroke_width, .. } = element {
        writer.write_attribute("w", &emu(*stroke_width));
        if *stroke_width > 0.0 { color(writer, stroke); } else { empty(writer, "a:noFill", &[]); }
    } else { empty(writer, "a:noFill", &[]); }
    writer.end_element();
    }
    writer.end_element();
    if let Element::Text { text, font_size, color, bold, format, .. } | Element::Shape { text, font_size, color, bold, format, .. } = element { formatted_text_body(writer, "p:txBody", text, *font_size, color, *bold, format, format.hyperlink.as_ref().map(|_| format!("rIdLink{id}")).as_deref(), theme); }
    writer.end_element();
}

pub(crate) fn element_xml(element: &Element, id: usize, ids: &BTreeMap<&str, usize>, theme: &Theme) -> String {
    let wrapper = xml("root", |writer| shape(writer, element, id, ids, theme));
    let wrapper = String::from_utf8(wrapper).expect("XML writer produces UTF-8");
    let document = roxmltree::Document::parse(&wrapper).expect("validated element XML");
    let node = document.root_element().children().find(|node| node.is_element()).expect("shape element");
    let mut result = wrapper[node.range()].to_owned();
    result.insert_str(node.tag_name().name().len() + 3, &format!(" xmlns:p=\"{P}\" xmlns:a=\"{A}\" xmlns:r=\"{R}\""));
    result
}

fn rels(entries: &[(&str, &str, String)]) -> Vec<u8> {
    let mut writer = XmlWriter::new(Options::default());
    writer.start_element("Relationships");
    writer.write_attribute("xmlns", REL);
    for (id, kind, target) in entries {
        if *kind == "hyperlink" { empty(&mut writer, "Relationship", &[("Id", id), ("Type", &format!("{R}/{kind}")), ("Target", target), ("TargetMode", "External")]); }
        else { empty(&mut writer, "Relationship", &[("Id", id), ("Type", &format!("{R}/{kind}")), ("Target", target)]); }
    }
    writer.end_element();
    writer.end_document().into_bytes()
}

fn clr_map(writer: &mut XmlWriter) {
    empty(writer, "p:clrMap", &[("bg1", "lt1"), ("tx1", "dk1"), ("bg2", "lt2"), ("tx2", "dk2"), ("accent1", "accent1"), ("accent2", "accent2"), ("accent3", "accent3"), ("accent4", "accent4"), ("accent5", "accent5"), ("accent6", "accent6"), ("hlink", "hlink"), ("folHlink", "folHlink")]);
}

fn theme(theme: &Theme) -> Vec<u8> {
    xml("a:theme", |writer| {
        writer.write_attribute("name", &quick_xml::escape::escape(&theme.name));
        writer.start_element("a:themeElements");
        writer.start_element("a:clrScheme");
        writer.write_attribute("name", "Report");
        for key in crate::design::COLOR_KEYS {
            let value = theme.colors.get(key).map(String::as_str).unwrap_or("202525");
            writer.start_element(&format!("a:{key}"));
            empty(writer, "a:srgbClr", &[("val", value)]);
            writer.end_element();
        }
        writer.end_element();
        writer.start_element("a:fontScheme");
        writer.write_attribute("name", "Office fonts");
        for kind in ["a:majorFont", "a:minorFont"] {
            writer.start_element(kind);
            empty(writer, "a:latin", &[("typeface", if kind == "a:majorFont" { &theme.fonts.major } else { &theme.fonts.minor })]);
            empty(writer, "a:ea", &[("typeface", &theme.fonts.east_asian)]);
            empty(writer, "a:cs", &[("typeface", &theme.fonts.complex_script)]);
            writer.end_element();
        }
        writer.end_element();
        writer.start_element("a:fmtScheme");
        writer.write_attribute("name", "Report");
        writer.start_element("a:fillStyleLst");
        for _style in 0..3 { color(writer, "FFFFFF"); }
        writer.end_element();
        writer.start_element("a:lnStyleLst");
        for _style in 0..3 { writer.start_element("a:ln"); writer.write_attribute("w", "12700"); color(writer, "202525"); writer.end_element(); }
        writer.end_element();
        writer.start_element("a:effectStyleLst");
        for _style in 0..3 { writer.start_element("a:effectStyle"); empty(writer, "a:effectLst", &[]); writer.end_element(); }
        writer.end_element();
        writer.start_element("a:bgFillStyleLst");
        for _style in 0..3 { color(writer, "FFFFFF"); }
        writer.end_element();
        writer.end_element();
        writer.end_element();
    })
}

pub(crate) fn resource_parts(parts: &mut BTreeMap<String, Vec<u8>>, elements: &[&Element], chart_number: &mut usize, picture_number: &mut usize, extra_types: &mut Vec<(String, String)>, ids: Option<&BTreeMap<&str, usize>>) -> Result<Vec<(String, &'static str, String)>> {
    let mut extras = Vec::new();
    for (index, element) in elements.iter().enumerate() {
    let object_id = ids.and_then(|ids| ids.get(element.bounds().0)).copied().unwrap_or(index + 2);
        if let Element::Chart { kind, categories, series, .. } = element {
            *chart_number += 1;
            let chart_path = format!("ppt/charts/chart{chart_number}.xml");
            let workbook_path = format!("ppt/embeddings/chart{chart_number}.xlsx");
            parts.insert(chart_path.clone(), crate::charts::chart(*kind, categories, series));
            parts.insert(workbook_path.clone(), crate::charts::workbook(*kind, categories, series)?);
            parts.insert(format!("ppt/charts/_rels/chart{chart_number}.xml.rels"), rels(&[("rIdWorkbook", "package", format!("../embeddings/chart{chart_number}.xlsx"))]));
            extra_types.push((chart_path, "application/vnd.openxmlformats-officedocument.drawingml.chart+xml".into()));
            extra_types.push((workbook_path, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into()));
            extras.push((format!("rIdShape{object_id}"), "chart", format!("../charts/chart{chart_number}.xml")));
        }
        if let Element::Picture { base64, mime_type, .. } = element {
            *picture_number += 1;
            let extension = if mime_type == "image/png" { "png" } else { "jpg" };
            let path = format!("ppt/media/image{picture_number}.{extension}");
            parts.insert(path.clone(), STANDARD.decode(base64).map_err(|_| Error::Invalid("invalid image base64".into()))?);
            extra_types.push((path, mime_type.clone()));
            extras.push((format!("rIdShape{object_id}"), "image", format!("../media/image{picture_number}.{extension}")));
        }
        if let Element::Text { format, .. } | Element::Shape { format, .. } = element { if let Some(link) = &format.hyperlink { extras.push((format!("rIdLink{object_id}"), "hyperlink", link.clone())); } }
    }
    Ok(extras)
}

pub fn export_pptx(deck: &Deck) -> Result<Vec<u8>> {
    validate_deck(deck)?;
    let mut fallback = Design::default(); fallback.layouts.truncate(1);
    let design = deck.design.as_ref().unwrap_or(&fallback);
    let mut parts = BTreeMap::new();
    let mut overrides = vec![("ppt/presentation.xml".to_string(), "presentation"), ("ppt/notesMasters/notesMaster1.xml".into(), "notesMaster")];
    parts.insert("_rels/.rels".into(), format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"rId1\" Type=\"{R}/officeDocument\" Target=\"ppt/presentation.xml\"/><Relationship Id=\"rIdCoreProperties\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/></Relationships>").into_bytes());
    parts.insert("docProps/core.xml".into(), format!("<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>{}</dc:title></cp:coreProperties>", quick_xml::escape::escape(&deck.title)).into_bytes());
    parts.insert("ppt/theme/theme1.xml".into(), theme(&design.theme));
    parts.insert("ppt/theme/theme2.xml".into(), theme(&design.theme));
    parts.insert("ppt/presentation.xml".into(), xml("p:presentation", |writer| {
        writer.start_element("p:sldMasterIdLst");
        for (index, _) in design.masters.iter().enumerate() { empty(writer, "p:sldMasterId", &[("id", &(2147483648u64 + index as u64).to_string()), ("r:id", &format!("rIdMaster{}", index + 1))]); } writer.end_element();
        writer.start_element("p:notesMasterIdLst"); empty(writer, "p:notesMasterId", &[("r:id", "rIdNotesMaster")]); writer.end_element();
        writer.start_element("p:sldIdLst");
        for index in 0..deck.slides.len() { empty(writer, "p:sldId", &[("id", &(256 + index).to_string()), ("r:id", &format!("rId{}", index + 1))]); }
        writer.end_element();
        empty(writer, "p:sldSz", &[("cx", "12192000"), ("cy", "6858000"), ("type", "screen16x9")]);
        empty(writer, "p:notesSz", &[("cx", "6858000"), ("cy", "9144000")]);
    }));
    let relationship_ids: Vec<_> = (1..=deck.slides.len()).map(|index| format!("rId{index}")).collect();
    let master_relationship_ids: Vec<_> = (1..=design.masters.len()).map(|index| format!("rIdMaster{index}")).collect();
    let mut relationships = vec![("rIdNotesMaster", "notesMaster", "notesMasters/notesMaster1.xml".into())];
    for (index, id) in master_relationship_ids.iter().enumerate() { relationships.push((id.as_str(), "slideMaster", format!("slideMasters/slideMaster{}.xml", index + 1))); }
    for (index, id) in relationship_ids.iter().enumerate() { relationships.push((id.as_str(), "slide", format!("slides/slide{}.xml", index + 1))); }
    parts.insert("ppt/_rels/presentation.xml.rels".into(), rels(&relationships));
    let mut chart_number = 0; let mut picture_number = 0; let mut extra_types = Vec::new();
    for (index, master) in design.masters.iter().enumerate() {
        let number = index + 1;
        let elements = crate::model::element_list(&master.elements);
        let ids: BTreeMap<_, _> = elements.iter().enumerate().map(|(index, element)| (element.bounds().0, index + 2)).collect();
        let layouts: Vec<_> = design.layouts.iter().enumerate().filter(|(_, layout)| layout.master_id == master.id).collect();
        let path = format!("ppt/slideMasters/slideMaster{number}.xml");
        parts.insert(path.clone(), xml("p:sldMaster", |writer| {
            writer.start_element("p:cSld"); writer.write_attribute("name", &quick_xml::escape::escape(&master.name));
            writer.start_element("p:bg"); writer.start_element("p:bgPr"); color(writer, &master.background); empty(writer, "a:effectLst", &[]); writer.end_element(); writer.end_element();
            writer.start_element("p:spTree"); group(writer); for element in &master.elements { shape(writer, element, ids[element.bounds().0], &ids, &design.theme); } writer.end_element(); writer.end_element();
            clr_map(writer); writer.start_element("p:sldLayoutIdLst");
            for (layout_index, _) in &layouts { empty(writer, "p:sldLayoutId", &[("id", &(2147483648u64 + design.masters.len() as u64 + *layout_index as u64).to_string()), ("r:id", &format!("rIdLayout{}", layout_index + 1))]); } writer.end_element();
            writer.start_element("p:txStyles"); for (tag, size) in [("p:titleStyle", "3200"), ("p:bodyStyle", "2400"), ("p:otherStyle", "1800")] {
                writer.start_element(tag); writer.start_element("a:lvl1pPr"); writer.start_element("a:defRPr"); writer.write_attribute("sz", size); color(writer, "@dk1"); empty(writer, "a:latin", &[("typeface", if tag == "p:titleStyle" { "+mj-lt" } else { "+mn-lt" })]); writer.end_element(); writer.end_element(); writer.end_element();
            } writer.end_element();
        }));
        overrides.push((path, "slideMaster"));
        let mut entries = resource_parts(&mut parts, &elements, &mut chart_number, &mut picture_number, &mut extra_types, None)?;
        let theme_number = if index == 0 { 1 } else { index + 2 };
        if index > 0 {
            let theme_path = format!("ppt/theme/theme{theme_number}.xml");
            parts.insert(theme_path.clone(), theme(&design.theme));
            extra_types.push((theme_path, "application/vnd.openxmlformats-officedocument.theme+xml".into()));
        }
        entries.push(("rIdTheme".into(), "theme", format!("../theme/theme{theme_number}.xml")));
        for (layout_index, _) in layouts { entries.push((format!("rIdLayout{}", layout_index + 1), "slideLayout", format!("../slideLayouts/slideLayout{}.xml", layout_index + 1))); }
        parts.insert(format!("ppt/slideMasters/_rels/slideMaster{number}.xml.rels"), rels(&entries.iter().map(|(id, kind, target)| (id.as_str(), *kind, target.clone())).collect::<Vec<_>>()));
    }
    for (index, layout) in design.layouts.iter().enumerate() {
        let number = index + 1;
        let master = design.masters.iter().position(|master| master.id == layout.master_id).ok_or_else(|| Error::Invalid("layout master missing".into()))? + 1;
        let elements = crate::model::element_list(&layout.elements);
        let ids: BTreeMap<_, _> = elements.iter().enumerate().map(|(index, element)| (element.bounds().0, index + 2)).collect();
        let path = format!("ppt/slideLayouts/slideLayout{number}.xml");
        parts.insert(path.clone(), xml("p:sldLayout", |writer| {
            writer.write_attribute("type", if layout.elements.is_empty() { "blank" } else { "cust" }); writer.write_attribute("preserve", "1");
            writer.start_element("p:cSld"); writer.write_attribute("name", &quick_xml::escape::escape(&layout.name));
            if let Some(background) = &layout.background { writer.start_element("p:bg"); writer.start_element("p:bgPr"); color(writer, background); empty(writer, "a:effectLst", &[]); writer.end_element(); writer.end_element(); }
            writer.start_element("p:spTree"); group(writer); for element in &layout.elements { shape(writer, element, ids[element.bounds().0], &ids, &design.theme); } writer.end_element(); writer.end_element();
        })); overrides.push((path, "slideLayout"));
        let mut entries = resource_parts(&mut parts, &elements, &mut chart_number, &mut picture_number, &mut extra_types, None)?;
        entries.push(("rIdMaster".into(), "slideMaster", format!("../slideMasters/slideMaster{master}.xml")));
        parts.insert(format!("ppt/slideLayouts/_rels/slideLayout{number}.xml.rels"), rels(&entries.iter().map(|(id, kind, target)| (id.as_str(), *kind, target.clone())).collect::<Vec<_>>()));
    }
    parts.insert("ppt/notesMasters/notesMaster1.xml".into(), xml("p:notesMaster", |writer| {
        writer.start_element("p:cSld"); writer.start_element("p:spTree"); group(writer); writer.end_element(); writer.end_element(); clr_map(writer);
    }));
    parts.insert("ppt/notesMasters/_rels/notesMaster1.xml.rels".into(), rels(&[("rId1", "theme", "../theme/theme2.xml".into())]));
    for (index, slide) in deck.slides.iter().enumerate() {
        let number = index + 1;
        let flat_elements = crate::model::element_list(&slide.elements);
        let ids: BTreeMap<&str, usize> = flat_elements.iter().enumerate().map(|(position, element)| (element.bounds().0, position + 2)).collect();
        let path = format!("ppt/slides/slide{number}.xml");
        parts.insert(path.clone(), xml("p:sld", |writer| {
            if slide.hide_master_graphics { writer.write_attribute("showMasterSp", "0"); }
            writer.start_element("p:cSld"); writer.write_attribute("name", &quick_xml::escape::escape(&slide.title));
            if !slide.inherit_background { writer.start_element("p:bg"); writer.start_element("p:bgPr"); color(writer, &slide.background); empty(writer, "a:effectLst", &[]); writer.end_element(); writer.end_element(); }
            writer.start_element("p:spTree"); group(writer);
            for element in &slide.elements { shape(writer, element, ids[element.bounds().0], &ids, &design.theme); }
            writer.end_element(); writer.end_element();
            writer.start_element("p:clrMapOvr"); empty(writer, "a:masterClrMapping", &[]); writer.end_element();
        }));
        overrides.push((path, "slide"));
        let extras = resource_parts(&mut parts, &flat_elements, &mut chart_number, &mut picture_number, &mut extra_types, None)?;
        let layout_number = slide.layout_id.as_ref().and_then(|id| design.layouts.iter().position(|layout| &layout.id == id)).unwrap_or(0) + 1;
        let mut slide_relationships = vec![("rId1", "slideLayout", format!("../slideLayouts/slideLayout{layout_number}.xml")), ("rId2", "notesSlide", format!("../notesSlides/notesSlide{number}.xml"))];
        slide_relationships.extend(extras.iter().map(|(id, kind, target)| (id.as_str(), *kind, target.clone())));
        parts.insert(format!("ppt/slides/_rels/slide{number}.xml.rels"), rels(&slide_relationships));
        let notes_path = format!("ppt/notesSlides/notesSlide{number}.xml");
        parts.insert(notes_path.clone(), xml("p:notes", |writer| {
            writer.start_element("p:cSld"); writer.start_element("p:spTree"); group(writer);
            writer.start_element("p:sp"); writer.start_element("p:nvSpPr");
            empty(writer, "p:cNvPr", &[("id", "2"), ("name", "Notes")]); empty(writer, "p:cNvSpPr", &[]);
            writer.start_element("p:nvPr"); empty(writer, "p:ph", &[("type", "body"), ("idx", "1")]); writer.end_element(); writer.end_element();
            empty(writer, "p:spPr", &[]);
            text_body(writer, "p:txBody", &slide.notes, 16.0, "202525", false);
            writer.end_element(); writer.end_element(); writer.end_element();
        }));
        overrides.push((notes_path, "notesSlide"));
        parts.insert(format!("ppt/notesSlides/_rels/notesSlide{number}.xml.rels"), rels(&[("rId1", "notesMaster", "../notesMasters/notesMaster1.xml".into()), ("rId2", "slide", format!("../slides/slide{number}.xml"))]));
    }
    let mut content_types = XmlWriter::new(Options::default());
    content_types.start_element("Types"); content_types.write_attribute("xmlns", CT);
    empty(&mut content_types, "Default", &[("Extension", "rels"), ("ContentType", "application/vnd.openxmlformats-package.relationships+xml")]);
    empty(&mut content_types, "Default", &[("Extension", "xml"), ("ContentType", "application/xml")]);
    empty(&mut content_types, "Override", &[("PartName", "/docProps/core.xml"), ("ContentType", "application/vnd.openxmlformats-package.core-properties+xml")]);
    for (part, kind) in overrides {
        let content_type = if kind == "presentation" { "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml".to_string() } else { format!("application/vnd.openxmlformats-officedocument.presentationml.{kind}+xml") };
        empty(&mut content_types, "Override", &[("PartName", &format!("/{part}")), ("ContentType", &content_type)]);
    }
    empty(&mut content_types, "Override", &[("PartName", "/ppt/theme/theme1.xml"), ("ContentType", "application/vnd.openxmlformats-officedocument.theme+xml")]);
    empty(&mut content_types, "Override", &[("PartName", "/ppt/theme/theme2.xml"), ("ContentType", "application/vnd.openxmlformats-officedocument.theme+xml")]);
    for (part, content_type) in extra_types { empty(&mut content_types, "Override", &[("PartName", &format!("/{part}")), ("ContentType", &content_type)]); }
    content_types.end_element();
    parts.insert("[Content_Types].xml".into(), content_types.end_document().into_bytes());
    Package::from_parts(parts)?.save()
}

#[derive(Debug, Serialize)]
pub struct Inspection { pub slides: Vec<InspectedSlide>, pub warnings: Vec<String> }
#[derive(Debug, Serialize)]
pub struct InspectedSlide { pub part: String, pub texts: Vec<InspectedText> }
#[derive(Debug, Serialize)]
pub struct InspectedText { pub shape_id: String, pub run_index: usize, pub text: String }

pub(crate) fn parse(value: &str) -> Result<roxmltree::Document<'_>> {
    if value.len() > 4 * 1024 * 1024 { return Err(Error::Limit("XML part > 4 MiB".into())); }
    let document = roxmltree::Document::parse_with_options(value, roxmltree::ParsingOptions { allow_dtd: false, nodes_limit: 100_000, ..Default::default() })?;
    if document.descendants().any(|node| node.ancestors().take(66).count() > 64) { return Err(Error::Limit("XML depth > 64".into())); }
    Ok(document)
}

pub(crate) fn resolve(source: &str, target: &str) -> Result<String> {
    if target.is_empty() || target.contains(['\\', ':', '?', '#', '%', '\0']) { return Err(Error::Unsupported("relationship target syntax".into())); }
    let mut components: Vec<&str> = if target.starts_with('/') { Vec::new() } else { source.rsplit_once('/').map_or(Vec::new(), |(dir, _)| dir.split('/').collect()) };
    for component in target.trim_start_matches('/').split('/') {
        match component {
            "." => {},
            ".." => { if components.pop().is_none() { return Err(Error::Invalid("relationship escapes package".into())); } },
            "" => return Err(Error::Invalid("empty relationship path component".into())),
            value => components.push(value),
        }
    }
    Ok(components.join("/"))
}

pub(crate) fn relationship_targets(package: &Package, source: &str, kind: &str) -> Result<BTreeMap<String, String>> {
    let path = if source.is_empty() { "_rels/.rels".into() } else if let Some((dir, name)) = source.rsplit_once('/') { format!("{dir}/_rels/{name}.rels") } else { format!("_rels/{source}.rels") };
    let document = parse(package.text(&path)?)?;
    if !document.root_element().has_tag_name((REL, "Relationships")) { return Err(Error::Invalid("relationships root".into())); }
    let mut seen = BTreeSet::new();
    let mut targets = BTreeMap::new();
    for node in document.root_element().children().filter(|node| node.has_tag_name((REL, "Relationship"))) {
        let id = node.attribute("Id").ok_or_else(|| Error::Invalid("relationship ID".into()))?;
        if !seen.insert(id) { return Err(Error::Invalid("duplicate relationship ID".into())); }
        if node.attribute("Type") == Some(format!("{R}/{kind}").as_str()) {
            if node.attribute("TargetMode").is_some_and(|mode| mode != "Internal") { return Err(Error::Unsupported("external slide/presentation relationship".into())); }
            let target = resolve(source, node.attribute("Target").ok_or_else(|| Error::Invalid("missing target".into()))?)?;
            package.part(&target)?;
            targets.insert(id.to_string(), target);
        }
    }
    Ok(targets)
}

pub(crate) fn slide_paths(package: &Package) -> Result<Vec<String>> {
    let roots = relationship_targets(package, "", "officeDocument")?;
    if roots.len() != 1 { return Err(Error::Invalid("expected one officeDocument".into())); }
    let main = roots.values().next().ok_or_else(|| Error::Invalid("missing presentation".into()))?;
    let document = parse(package.text(main)?)?;
    if !document.root_element().has_tag_name((P, "presentation")) { return Err(Error::Unsupported("not Transitional PresentationML".into())); }
    let targets = relationship_targets(package, main, "slide")?;
    let lists: Vec<_> = document.root_element().children().filter(|node| node.has_tag_name((P, "sldIdLst"))).collect();
    if lists.len() != 1 { return Err(Error::Invalid("expected one slide list".into())); }
    let mut paths = Vec::new();
    let mut ids = BTreeSet::new();
    for slide in lists[0].children().filter(|node| node.has_tag_name((P, "sldId"))) {
        let id = slide.attribute((R, "id")).ok_or_else(|| Error::Invalid("slide relationship ID".into()))?;
        if !ids.insert(id) { return Err(Error::Invalid("duplicate slide relationship".into())); }
        paths.push(targets.get(id).ok_or_else(|| Error::Invalid("missing slide target".into()))?.clone());
    }
    if paths.len() > 256 { return Err(Error::Limit("inspection supports up to 256 slides".into())); }
    Ok(paths)
}

fn shapes<'a, 'input>(document: &'a roxmltree::Document<'input>) -> Vec<roxmltree::Node<'a, 'input>> {
    document.root_element().children().filter(|node| node.has_tag_name((P, "cSld")))
        .flat_map(|node| node.children().filter(|child| child.has_tag_name((P, "spTree"))))
        .flat_map(|node| node.children().filter(|child| child.has_tag_name((P, "sp")))).collect()
}

fn shape_id<'a>(node: roxmltree::Node<'a, '_>) -> Option<&'a str> {
    node.children().find(|child| child.has_tag_name((P, "nvSpPr")))?.children().find(|child| child.has_tag_name((P, "cNvPr")))?.attribute("id")
}

fn runs<'a, 'input>(node: roxmltree::Node<'a, 'input>) -> Vec<roxmltree::Node<'a, 'input>> {
    node.children().filter(|child| child.has_tag_name((P, "txBody")))
        .flat_map(|body| body.children().filter(|child| child.has_tag_name((A, "p"))))
        .flat_map(|paragraph| paragraph.children().filter(|child| child.has_tag_name((A, "r"))))
        .flat_map(|run| run.children().filter(|child| child.has_tag_name((A, "t")))).collect()
}

pub fn inspect_pptx(bytes: Vec<u8>) -> Result<Inspection> {
    let package = Package::open(bytes)?;
    let mut slides = Vec::new();
    for part in slide_paths(&package)? {
        let document = parse(package.text(&part)?)?;
        let mut texts = Vec::new();
        for shape in shapes(&document) {
            if let Some(id) = shape_id(shape) {
                for (run_index, run) in runs(shape).iter().enumerate() { texts.push(InspectedText { shape_id: id.into(), run_index, text: run.text().unwrap_or("").into() }); }
            }
        }
        slides.push(InspectedSlide { part, texts });
    }
    Ok(Inspection { slides, warnings: vec!["Inspection lists simple top-level text runs only. Groups, charts, SmartArt, media and external content are not rendered or executed.".into()] })
}

pub fn patch_text(bytes: Vec<u8>, part: &str, id: &str, run_index: usize, expected: &str, replacement: &str) -> Result<Vec<u8>> {
    valid_text(replacement, 4000)?;
    let mut package = Package::open(bytes)?;
    if package.parts().keys().any(|name| name.to_ascii_lowercase().starts_with("_xmlsignatures/")) { return Err(Error::Unsupported("editing signed packages".into())); }
    if !slide_paths(&package)?.contains(&part.to_string()) { return Err(Error::Invalid("target is not a presentation slide".into())); }
    let original = package.text(part)?;
    let document = parse(original)?;
    let matching: Vec<_> = shapes(&document).into_iter().filter(|node| shape_id(*node) == Some(id)).collect();
    if matching.len() != 1 { return Err(Error::Invalid("shape not found or ambiguous".into())); }
    let text_runs = runs(matching[0]);
    let run = text_runs.get(run_index).ok_or_else(|| Error::Invalid("text run not found".into()))?;
    if run.text().unwrap_or("") != expected { return Err(Error::Conflict("text changed since inspection".into())); }
    if expected == replacement { return package.save(); }
    let children: Vec<_> = run.children().collect();
    if children.len() != 1 || !children[0].is_text() { return Err(Error::Unsupported("only a nonempty, single XML text node is writable".into())); }
    let range = children[0].range();
    if original[range.clone()].contains('<') { return Err(Error::Unsupported("CDATA or mixed text".into())); }
    let mut changed = original.to_string();
    changed.replace_range(range, &quick_xml::escape::escape(replacement));
    parse(&changed)?;
    package.replace_part(part, changed.into_bytes())?;
    package.save()
}