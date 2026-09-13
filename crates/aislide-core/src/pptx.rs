use crate::{model::{Deck, Element, validate_deck, valid_text}, package::Package, Error, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use xmlwriter::{Options, Indent, XmlWriter};

const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";

fn xml(root: &str, build: impl FnOnce(&mut XmlWriter)) -> Vec<u8> {
    let mut writer = XmlWriter::new(Options { indent: Indent::None, ..Options::default() });
    writer.start_element(root);
    writer.write_attribute("xmlns:p", P);
    writer.write_attribute("xmlns:a", A);
    writer.write_attribute("xmlns:r", R);
    build(&mut writer);
    writer.end_element();
    writer.end_document().into_bytes()
}

fn empty(writer: &mut XmlWriter, name: &str, attrs: &[(&str, &str)]) {
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

fn color(writer: &mut XmlWriter, value: &str) {
    writer.start_element("a:solidFill");
    empty(writer, "a:srgbClr", &[("val", value)]);
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
    writer.start_element(name);
    writer.start_element("a:bodyPr");
    for key in ["lIns", "tIns", "rIns", "bIns"] { writer.write_attribute(key, "0"); }
    writer.write_attribute("wrap", "square");
    writer.write_attribute("anchor", "t");
    empty(writer, "a:noAutofit", &[]);
    writer.end_element();
    empty(writer, "a:lstStyle", &[]);
    for line in text.split('\n') {
        writer.start_element("a:p");
        writer.start_element("a:pPr");
        writer.start_element("a:lnSpc");
        empty(writer, "a:spcPct", &[("val", "115000")]);
        writer.end_element();
        writer.end_element();
        writer.start_element("a:r");
        writer.start_element("a:rPr");
        writer.write_attribute("lang", "ja-JP");
        writer.write_attribute("sz", &(size * 75.0).round().to_string());
        writer.write_attribute("b", if bold { "1" } else { "0" });
        color(writer, shade);
        empty(writer, "a:latin", &[("typeface", "Aptos")]);
        empty(writer, "a:ea", &[("typeface", "Yu Gothic")]);
        empty(writer, "a:cs", &[("typeface", "Arial")]);
        writer.end_element();
        writer.start_element("a:t");
        writer.write_text(&quick_xml::escape::escape(line));
        writer.end_element();
        writer.end_element();
        empty(writer, "a:endParaRPr", &[("lang", "ja-JP"), ("sz", &(size * 75.0).round().to_string())]);
        writer.end_element();
    }
    writer.end_element();
}

fn shape(writer: &mut XmlWriter, element: &Element, id: usize) {
    let (name, x, y, width, height) = element.bounds();
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
                text_body(writer, "a:txBody", cell, *font_size, if row_index == 0 { "FFFFFF" } else { "202525" }, row_index == 0);
                writer.start_element("a:tcPr");
                for key in ["marL", "marR", "marT", "marB"] { writer.write_attribute(key, "101600"); }
                color(writer, if row_index == 0 { "087F73" } else if row_index % 2 == 0 { "EDF3F0" } else { "FFFFFF" });
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
    empty(writer, "p:nvPr", &[]);
    writer.end_element();
    writer.start_element("p:spPr");
    transform(writer, "a:xfrm", x, y, width, height);
    writer.start_element("a:prstGeom");
    writer.write_attribute("prst", "rect");
    empty(writer, "a:avLst", &[]);
    writer.end_element();
    if let Element::Rect { fill, .. } = element { color(writer, fill); }
    else { empty(writer, "a:noFill", &[]); }
    writer.start_element("a:ln");
    empty(writer, "a:noFill", &[]);
    writer.end_element();
    writer.end_element();
    if let Element::Text { text, font_size, color, bold, .. } = element { text_body(writer, "p:txBody", text, *font_size, color, *bold); }
    writer.end_element();
}

fn rels(entries: &[(&str, &str, String)]) -> Vec<u8> {
    let mut writer = XmlWriter::new(Options::default());
    writer.start_element("Relationships");
    writer.write_attribute("xmlns", REL);
    for (id, kind, target) in entries {
        empty(&mut writer, "Relationship", &[("Id", id), ("Type", &format!("{R}/{kind}")), ("Target", target)]);
    }
    writer.end_element();
    writer.end_document().into_bytes()
}

fn clr_map(writer: &mut XmlWriter) {
    empty(writer, "p:clrMap", &[("bg1", "lt1"), ("tx1", "dk1"), ("bg2", "lt2"), ("tx2", "dk2"), ("accent1", "accent1"), ("accent2", "accent2"), ("accent3", "accent3"), ("accent4", "accent4"), ("accent5", "accent5"), ("accent6", "accent6"), ("hlink", "hlink"), ("folHlink", "folHlink")]);
}

fn theme() -> Vec<u8> {
    xml("a:theme", |writer| {
        writer.write_attribute("name", "AISlide Report");
        writer.start_element("a:themeElements");
        writer.start_element("a:clrScheme");
        writer.write_attribute("name", "Report");
        for (key, value) in [("dk1", "202525"), ("lt1", "FFFFFF"), ("dk2", "586563"), ("lt2", "EDF3F0"), ("accent1", "087F73"), ("accent2", "CF5847"), ("accent3", "416CA5"), ("accent4", "C68B19"), ("accent5", "677C54"), ("accent6", "9A506B"), ("hlink", "0066CC"), ("folHlink", "734C8C")] {
            writer.start_element(&format!("a:{key}"));
            empty(writer, "a:srgbClr", &[("val", value)]);
            writer.end_element();
        }
        writer.end_element();
        writer.start_element("a:fontScheme");
        writer.write_attribute("name", "Office fonts");
        for kind in ["a:majorFont", "a:minorFont"] {
            writer.start_element(kind);
            empty(writer, "a:latin", &[("typeface", "Aptos")]);
            empty(writer, "a:ea", &[("typeface", "Yu Gothic")]);
            empty(writer, "a:cs", &[("typeface", "Arial")]);
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

pub fn export_pptx(deck: &Deck) -> Result<Vec<u8>> {
    validate_deck(deck)?;
    let mut parts = BTreeMap::new();
    let mut overrides = vec![("ppt/presentation.xml".to_string(), "presentation"), ("ppt/slideMasters/slideMaster1.xml".into(), "slideMaster"), ("ppt/slideLayouts/slideLayout1.xml".into(), "slideLayout"), ("ppt/notesMasters/notesMaster1.xml".into(), "notesMaster")];
    parts.insert("_rels/.rels".into(), rels(&[("rId1", "officeDocument", "ppt/presentation.xml".into())]));
    parts.insert("ppt/theme/theme1.xml".into(), theme());
    parts.insert("ppt/theme/theme2.xml".into(), theme());
    parts.insert("ppt/presentation.xml".into(), xml("p:presentation", |writer| {
        writer.start_element("p:sldMasterIdLst");
        empty(writer, "p:sldMasterId", &[("id", "2147483648"), ("r:id", "rIdMaster")]); writer.end_element();
        writer.start_element("p:notesMasterIdLst"); empty(writer, "p:notesMasterId", &[("r:id", "rIdNotesMaster")]); writer.end_element();
        writer.start_element("p:sldIdLst");
        for index in 0..deck.slides.len() { empty(writer, "p:sldId", &[("id", &(256 + index).to_string()), ("r:id", &format!("rId{}", index + 1))]); }
        writer.end_element();
        empty(writer, "p:sldSz", &[("cx", "12192000"), ("cy", "6858000"), ("type", "screen16x9")]);
        empty(writer, "p:notesSz", &[("cx", "6858000"), ("cy", "9144000")]);
    }));
    let relationship_ids: Vec<_> = (1..=deck.slides.len()).map(|index| format!("rId{index}")).collect();
    let mut relationships = vec![("rIdMaster", "slideMaster", "slideMasters/slideMaster1.xml".into()), ("rIdNotesMaster", "notesMaster", "notesMasters/notesMaster1.xml".into())];
    for (index, id) in relationship_ids.iter().enumerate() { relationships.push((id.as_str(), "slide", format!("slides/slide{}.xml", index + 1))); }
    parts.insert("ppt/_rels/presentation.xml.rels".into(), rels(&relationships));
    parts.insert("ppt/slideMasters/slideMaster1.xml".into(), xml("p:sldMaster", |writer| {
        writer.start_element("p:cSld"); writer.start_element("p:spTree"); group(writer); writer.end_element(); writer.end_element();
        clr_map(writer);
        writer.start_element("p:sldLayoutIdLst"); empty(writer, "p:sldLayoutId", &[("id", "2147483649"), ("r:id", "rId1")]); writer.end_element();
        writer.start_element("p:txStyles"); for name in ["p:titleStyle", "p:bodyStyle", "p:otherStyle"] { empty(writer, name, &[]); } writer.end_element();
    }));
    parts.insert("ppt/slideMasters/_rels/slideMaster1.xml.rels".into(), rels(&[("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml".into()), ("rId2", "theme", "../theme/theme1.xml".into())]));
    parts.insert("ppt/slideLayouts/slideLayout1.xml".into(), xml("p:sldLayout", |writer| {
        writer.write_attribute("type", "blank"); writer.write_attribute("preserve", "1");
        writer.start_element("p:cSld"); writer.write_attribute("name", "AISlide Blank"); writer.start_element("p:spTree"); group(writer); writer.end_element(); writer.end_element();
    }));
    parts.insert("ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(), rels(&[("rId1", "slideMaster", "../slideMasters/slideMaster1.xml".into())]));
    parts.insert("ppt/notesMasters/notesMaster1.xml".into(), xml("p:notesMaster", |writer| {
        writer.start_element("p:cSld"); writer.start_element("p:spTree"); group(writer); writer.end_element(); writer.end_element(); clr_map(writer);
    }));
    parts.insert("ppt/notesMasters/_rels/notesMaster1.xml.rels".into(), rels(&[("rId1", "theme", "../theme/theme2.xml".into())]));
    for (index, slide) in deck.slides.iter().enumerate() {
        let number = index + 1;
        let path = format!("ppt/slides/slide{number}.xml");
        parts.insert(path.clone(), xml("p:sld", |writer| {
            writer.start_element("p:cSld"); writer.write_attribute("name", &quick_xml::escape::escape(&slide.title));
            writer.start_element("p:bg"); writer.start_element("p:bgPr"); color(writer, &slide.background); empty(writer, "a:effectLst", &[]); writer.end_element(); writer.end_element();
            writer.start_element("p:spTree"); group(writer);
            for (element_index, element) in slide.elements.iter().enumerate() { shape(writer, element, element_index + 2); }
            writer.end_element(); writer.end_element();
            writer.start_element("p:clrMapOvr"); empty(writer, "a:masterClrMapping", &[]); writer.end_element();
        }));
        overrides.push((path, "slide"));
        parts.insert(format!("ppt/slides/_rels/slide{number}.xml.rels"), rels(&[("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml".into()), ("rId2", "notesSlide", format!("../notesSlides/notesSlide{number}.xml"))]));
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
    for (part, kind) in overrides {
        let content_type = if kind == "presentation" { "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml".to_string() } else { format!("application/vnd.openxmlformats-officedocument.presentationml.{kind}+xml") };
        empty(&mut content_types, "Override", &[("PartName", &format!("/{part}")), ("ContentType", &content_type)]);
    }
    empty(&mut content_types, "Override", &[("PartName", "/ppt/theme/theme1.xml"), ("ContentType", "application/vnd.openxmlformats-officedocument.theme+xml")]);
    empty(&mut content_types, "Override", &[("PartName", "/ppt/theme/theme2.xml"), ("ContentType", "application/vnd.openxmlformats-officedocument.theme+xml")]);
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

fn parse(value: &str) -> Result<roxmltree::Document<'_>> {
    if value.len() > 4 * 1024 * 1024 { return Err(Error::Limit("XML part > 4 MiB".into())); }
    let document = roxmltree::Document::parse_with_options(value, roxmltree::ParsingOptions { allow_dtd: false, nodes_limit: 100_000, ..Default::default() })?;
    if document.descendants().any(|node| node.ancestors().take(66).count() > 64) { return Err(Error::Limit("XML depth > 64".into())); }
    Ok(document)
}

fn resolve(source: &str, target: &str) -> Result<String> {
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

fn relationship_targets(package: &Package, source: &str, kind: &str) -> Result<BTreeMap<String, String>> {
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

fn slide_paths(package: &Package) -> Result<Vec<String>> {
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