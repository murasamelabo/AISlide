use crate::{Error, Result, model::{Slide, element_list, valid_text}};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SlideReview {
    #[serde(skip_serializing_if = "Vec::is_empty")] pub comments: Vec<crate::comments::Comment>,
    #[serde(skip_serializing_if = "Option::is_none")] pub modern_threads: Option<Vec<crate::comments::modern::ModernThread>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")] pub table_headers: BTreeMap<String, TableHeaders>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")] pub accessibility: BTreeMap<String, ElementAccessibility>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub reading_order: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ElementAccessibility {
    #[serde(skip_serializing_if = "Option::is_none")] pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub decorative: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableHeaders { #[default] Unknown, None, FirstRow, FirstColumn, Both }

pub fn validate_slide(slide: &Slide) -> Result<()> {
    let Some(review) = &slide.review else { return Ok(()); };
    crate::comments::validate(&review.comments)?;
    if let Some(threads) = &review.modern_threads { crate::comments::modern::validate(threads)?; }
    let ids: BTreeSet<_> = element_list(&slide.elements).iter().map(|element| element.bounds().0).collect();
    for id in review.table_headers.keys() {
        if !element_list(&slide.elements).iter().any(|element| matches!(element, crate::model::Element::Table { id: table_id, .. } if table_id == id)) { return Err(Error::Invalid("table header target missing".into())); }
    }
    for (id, metadata) in &review.accessibility {
        if !ids.contains(id.as_str()) { return Err(Error::Invalid("accessibility target missing".into())); }
        if let Some(title) = &metadata.title { valid_text(title, 500)?; }
        if let Some(description) = &metadata.description { valid_text(description, 4000)?; }
        if metadata.title.iter().chain(metadata.description.iter()).any(|value| value.chars().any(char::is_control)) { return Err(Error::Invalid("native accessibility attributes cannot contain control characters".into())); }
    }
    if !review.reading_order.is_empty() {
        let top: BTreeSet<_> = slide.elements.iter().map(|element| element.bounds().0).collect();
        let order: BTreeSet<_> = review.reading_order.iter().map(String::as_str).collect();
        if top != order || order.len() != review.reading_order.len() { return Err(Error::Invalid("reading order must name each top-level element exactly once".into())); }
    }
    Ok(())
}

pub(crate) const NS: &str = "urn:aislide:review:1";
pub(crate) const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";

pub(crate) fn put_part(package: &mut crate::package::Package, path: &str, bytes: Vec<u8>) -> Result<()> {
    if package.part_names().contains(path) { package.replace_part(path, bytes) } else { package.add_part(path.into(), bytes) }
}

pub(crate) fn unused_path(package: &crate::package::Package, prefix: &str) -> String {
    let mut number = 1;
    loop {
        let path = format!("{prefix}{number}.xml");
        if !package.part_names().iter().any(|name| name.eq_ignore_ascii_case(&path)) { return path; }
        number += 1;
    }
}

pub(crate) fn add_type(package: &mut crate::package::Package, path: &str, kind: &str) -> Result<()> {
    let xml = package.text("[Content_Types].xml")?.to_owned();
    let parsed = crate::pptx::parse(&xml)?;
    let entries: Vec<_> = parsed.root_element().children().filter(|node| node.attribute("PartName") == Some(format!("/{path}").as_str())).collect();
    if !entries.is_empty() {
        if entries.len() != 1 || entries[0].attribute("ContentType") != Some(kind) { return Err(Error::Invalid("content type mismatch".into())); }
        return Ok(());
    }
    let mut edits = Vec::new();
    crate::native_save::insert_child(&xml, parsed.root_element(), &format!("<Override xmlns=\"{CT}\" PartName=\"/{}\" ContentType=\"{}\"/>", quick_xml::escape::escape(path), quick_xml::escape::escape(kind)), None, &mut edits)?;
    package.replace_part("[Content_Types].xml", crate::native_save::apply(xml, edits)?)
}

pub(crate) fn link(package: &mut crate::package::Package, source: &str, kind: &str, target: &str) -> Result<()> {
    let path = crate::native::relations_path(source);
    let xml = if package.part_names().contains(path.as_str()) { package.text(&path)?.to_owned() } else { format!("<Relationships xmlns=\"{}\"/>", crate::native::REL) };
    let parsed = crate::pptx::parse(&xml)?;
    let mut number = 1;
    let id = loop { let id = format!("rIdAislideReview{number}"); if !parsed.root_element().children().any(|node| node.attribute("Id") == Some(&id)) { break id; } number += 1; };
    let mut edits = Vec::new();
    crate::native_save::insert_child(&xml, parsed.root_element(), &format!("<Relationship xmlns=\"{}\" Id=\"{id}\" Type=\"{}/{kind}\" Target=\"/{}\"/>", crate::native::REL, crate::native::R, quick_xml::escape::escape(target)), None, &mut edits)?;
    put_part(package, &path, crate::native_save::apply(xml, edits)?)
}

pub(crate) fn targets(package: &crate::package::Package, source: &str, kind: &str) -> Result<BTreeMap<String, String>> {
    if !package.part_names().contains(crate::native::relations_path(source).as_str()) { return Ok(BTreeMap::new()); }
    crate::pptx::relationship_targets(package, source, kind)
}

const DECORATIVE: &str = "http://schemas.microsoft.com/office/drawing/2017/decorative";
const DECORATIVE_URI: &str = "{C183D7F6-B498-43B3-948B-1728B52AA6E4}";
const ORDER_URI: &str = "urn:aislide:reading-order:1";
const HEADERS_URI: &str = "urn:aislide:table-headers:1";

pub(crate) fn read_slide(package: &crate::package::Package, binding: &crate::native::NativePart, elements: &[crate::model::Element]) -> Result<Option<SlideReview>> {
    use crate::native::{P, A, child, properties};
    let parsed = crate::pptx::parse(package.text(&binding.path)?)?;
    let mut review = SlideReview { comments: crate::comments::read(package, &binding.path)?, modern_threads: crate::comments::modern::read(package, &binding.path)?, ..Default::default() };
    for shape in parsed.descendants().filter(|node| crate::native::is_shape(*node)) {
        let Some(props) = properties(shape) else { continue };
        let Some((id, _)) = binding.nodes.iter().find(|(_, numeric)| Some(numeric.as_str()) == props.attribute("id")) else { continue };
        if !element_list(elements).iter().any(|element| element.bounds().0 == id) { continue; }
        let decorative = child(props, A, "extLst").into_iter().flat_map(|node| node.children()).flat_map(|node| node.children()).find(|node| node.has_tag_name((DECORATIVE, "decorative"))).and_then(|node| node.attribute("val")).map(|value| matches!(value, "1" | "true"));
        let metadata = ElementAccessibility { title: props.attribute("title").map(str::to_owned), description: props.attribute("descr").filter(|_| !shape.has_tag_name((P, "pic"))).map(str::to_owned), decorative };
        if metadata != ElementAccessibility::default() { review.accessibility.insert(id.clone(), metadata); }
        if let Some(headers) = child(props, A, "extLst").into_iter().flat_map(|node| node.children()).filter(|node| node.has_tag_name((A, "ext")) && node.attribute("uri") == Some(HEADERS_URI)).flat_map(|node| node.children()).find(|node| node.has_tag_name((NS, "tableHeaders"))) {
            let policy = headers.attribute("policy").ok_or_else(|| Error::Invalid("table headers policy missing".into()))?;
            review.table_headers.insert(id.clone(), serde_json::from_value(serde_json::Value::String(policy.into()))?);
        }
    }
    if child(parsed.root_element(), P, "extLst").into_iter().flat_map(|node| node.children()).any(|node| node.attribute("uri") == Some(ORDER_URI)) {
        review.reading_order = elements.iter().map(|element| element.bounds().0.to_owned()).collect();
    }
    Ok((review != SlideReview::default()).then_some(review))
}

fn write_accessibility(package: &mut crate::package::Package, binding: &crate::native::NativePart, before: Option<&SlideReview>, slide: &Slide) -> Result<()> {
    use crate::native::{P, A, child, properties};
    use crate::native_save::{apply, insert_child, set_attribute};
    let empty = SlideReview::default(); let before = before.unwrap_or(&empty); let after = slide.review.as_ref().unwrap_or(&empty);
    if before.accessibility == after.accessibility && before.reading_order == after.reading_order && before.table_headers == after.table_headers { return Ok(()); }
    let mut xml = package.text(&binding.path)?.to_owned();
    let parsed = crate::pptx::parse(&xml)?; let mut edits = Vec::new();
    let ids: BTreeSet<_> = before.accessibility.keys().chain(after.accessibility.keys()).chain(before.table_headers.keys()).chain(after.table_headers.keys()).collect();
    for id in ids {
        if before.accessibility.get(id) == after.accessibility.get(id) && before.table_headers.get(id) == after.table_headers.get(id) { continue; }
        let numeric = binding.nodes.get(id);
        let shape = parsed.descendants().filter(|node| crate::native::is_shape(*node)).find(|node| properties(*node).is_some_and(|props| numeric.is_some_and(|numeric| props.attribute("id") == Some(numeric)) || (numeric.is_none() && props.attribute("name") == Some(id))));
        let Some(shape) = shape else {
            if after.accessibility.contains_key(id) || after.table_headers.contains_key(id) { return Err(Error::Unsupported("accessibility shape unavailable".into())); }
            continue;
        };
        let props = properties(shape).ok_or_else(|| Error::Invalid("accessibility properties missing".into()))?;
        let mut added_extensions = String::new();
        if before.table_headers.get(id) != after.table_headers.get(id) {
            let policy = serde_json::to_value(after.table_headers.get(id).copied().unwrap_or_default())?;
            let extension = format!("<a:ext xmlns:a=\"{A}\" uri=\"{HEADERS_URI}\"><rv:tableHeaders xmlns:rv=\"{NS}\" policy=\"{}\"/></a:ext>", policy.as_str().ok_or_else(|| Error::Invalid("header policy".into()))?);
            if let Some(list) = child(props, A, "extLst") {
                if let Some(existing) = list.children().find(|node| node.has_tag_name((A, "ext")) && node.attribute("uri") == Some(HEADERS_URI)) { edits.push((existing.range(), extension)); }
                else { added_extensions.push_str(&extension); }
            } else { added_extensions.push_str(&extension); }
        }
        let defaults = ElementAccessibility::default(); let old = before.accessibility.get(id).unwrap_or(&defaults); let new = after.accessibility.get(id).unwrap_or(&defaults);
        for (name, previous, next) in [("title", &old.title, &new.title), ("descr", &old.description, &new.description)] {
            if previous != next { set_attribute(&xml, props, name, next.as_deref().unwrap_or(""), &mut edits)?; }
        }
        if old.decorative != new.decorative {
            let extension = format!("<a:ext xmlns:a=\"{A}\" uri=\"{DECORATIVE_URI}\"><adec:decorative xmlns:adec=\"{DECORATIVE}\" val=\"{}\"/></a:ext>", if new.decorative == Some(true) { 1 } else { 0 });
            if let Some(list) = child(props, A, "extLst") {
                if let Some(existing) = list.children().find(|node| node.has_tag_name((A, "ext")) && node.attribute("uri") == Some(DECORATIVE_URI)) { edits.push((existing.range(), extension)); }
                else { added_extensions.push_str(&extension); }
            } else { added_extensions.push_str(&extension); }
        }
        if !added_extensions.is_empty() {
            if let Some(list) = child(props, A, "extLst") { insert_child(&xml, list, &added_extensions, None, &mut edits)?; }
            else { insert_child(&xml, props, &format!("<a:extLst xmlns:a=\"{A}\">{added_extensions}</a:extLst>"), None, &mut edits)?; }
        }
    }
    if !edits.is_empty() { xml = String::from_utf8(apply(xml, edits)?).map_err(|_| Error::Invalid("accessibility XML encoding".into()))?; }
    if before.reading_order != after.reading_order {
        let parsed = crate::pptx::parse(&xml)?; let mut edits = Vec::new();
        if !after.reading_order.is_empty() {
            let tree = child(parsed.root_element(), P, "cSld").and_then(|node| child(node, P, "spTree")).ok_or_else(|| Error::Unsupported("reading order tree missing".into()))?;
            let shapes: Vec<_> = tree.children().filter(|node| crate::native::is_shape(*node)).collect();
            if shapes.len() != after.reading_order.len() { return Err(Error::Unsupported("reading order includes unmodeled native shapes".into())); }
            let ordered = after.reading_order.iter().map(|id| {
                let numeric = binding.nodes.get(id);
                shapes.iter().find(|shape| properties(**shape).is_some_and(|props| numeric.is_some_and(|numeric| props.attribute("id") == Some(numeric)) || (numeric.is_none() && props.attribute("name") == Some(id))))
                    .map(|node| xml[node.range()].to_owned()).ok_or_else(|| Error::Invalid("reading order target missing".into()))
            }).collect::<Result<Vec<_>>>()?;
            for (node, fragment) in shapes.iter().zip(ordered) { edits.push((node.range(), fragment)); }
        }
        let marker = format!("<p:ext xmlns:p=\"{P}\" uri=\"{ORDER_URI}\"><rv:readingOrder xmlns:rv=\"{NS}\"/></p:ext>");
        if let Some(list) = child(parsed.root_element(), P, "extLst") {
            if let Some(existing) = list.children().find(|node| node.attribute("uri") == Some(ORDER_URI)) { edits.push((existing.range(), if after.reading_order.is_empty() { String::new() } else { marker })); }
            else if !after.reading_order.is_empty() { insert_child(&xml, list, &marker, None, &mut edits)?; }
        } else if !after.reading_order.is_empty() { insert_child(&xml, parsed.root_element(), &format!("<p:extLst xmlns:p=\"{P}\">{marker}</p:extLst>"), None, &mut edits)?; }
        xml = String::from_utf8(apply(xml, edits)?).map_err(|_| Error::Invalid("reading order XML encoding".into()))?;
    }
    package.replace_part(&binding.path, xml.into_bytes())
}

pub(crate) fn write_generated(package: &mut crate::package::Package, deck: &crate::model::Deck) -> Result<()> {
    crate::comments::modern::validate_deck(deck)?;
    for (slide, path) in deck.slides.iter().zip(crate::pptx::slide_paths(package)?) {
        let binding = crate::native::NativePart { path, id: slide.id.clone(), nodes: BTreeMap::new() };
        let comments = slide.review.as_ref().map(|review| review.comments.as_slice()).unwrap_or(&[]);
        crate::comments::write(package, &binding.path, &[], comments, false)?;
        crate::comments::modern::write(package, &binding.path, None, slide.review.as_ref().and_then(|review| review.modern_threads.as_deref()), false)?;
        write_accessibility(package, &binding, None, slide)?;
    }
    Ok(())
}

pub(crate) fn save_end(package: &mut crate::package::Package, original: &crate::native::NativeDeck, deck: &crate::model::Deck, bindings: &[crate::native::NativePart]) -> Result<()> {
    crate::comments::modern::validate_deck(deck)?;
    crate::fields::write_bound(package, deck, bindings)?;
    for (slide, binding) in deck.slides.iter().zip(bindings) {
        let before = original.deck.slides.iter().find(|old| old.id == slide.native_source_id.as_deref().unwrap_or(&slide.id));
        let old = before.and_then(|slide| slide.review.as_ref()); let new = slide.review.as_ref();
        crate::comments::write(package, &binding.path, old.map(|review| review.comments.as_slice()).unwrap_or(&[]), new.map(|review| review.comments.as_slice()).unwrap_or(&[]), slide.native_source_id.is_some())?;
        crate::comments::modern::write(package, &binding.path, old.and_then(|review| review.modern_threads.as_deref()), new.and_then(|review| review.modern_threads.as_deref()), slide.native_source_id.is_some())?;
        write_accessibility(package, binding, old, slide)?;
    }
    Ok(())
}

pub fn set_accessibility(deck: &crate::model::Deck, slide_id: &str, element_id: &str, mut metadata: Option<ElementAccessibility>) -> Result<crate::model::Deck> {
    let mut next = deck.clone();
    let slide = next.slides.iter_mut().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("slide missing".into()))?;
    fn picture_description(elements: &mut [crate::model::Element], id: &str, metadata: &mut Option<ElementAccessibility>) {
        for element in elements {
            match element {
                crate::model::Element::Picture { id: picture_id, alt, .. } if picture_id == id => { if let Some(description) = metadata.as_mut().and_then(|metadata| metadata.description.take()) { *alt = description; } }
                crate::model::Element::Group { children, .. } => picture_description(children, id, metadata),
                _ => {},
            }
        }
    }
    picture_description(&mut slide.elements, element_id, &mut metadata);
    let entries = &mut slide.review.get_or_insert_with(Default::default).accessibility;
    if let Some(metadata) = metadata { entries.insert(element_id.into(), metadata); } else { entries.remove(element_id); }
    crate::model::validate_deck(&next)?; Ok(next)
}

pub fn set_table_headers(deck: &crate::model::Deck, slide_id: &str, element_id: &str, policy: TableHeaders) -> Result<crate::model::Deck> {
    let mut next = deck.clone();
    let slide = next.slides.iter_mut().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("slide missing".into()))?;
    slide.review.get_or_insert_with(Default::default).table_headers.insert(element_id.into(), policy);
    crate::model::validate_deck(&next)?;
    Ok(next)
}

pub fn set_reading_order(deck: &crate::model::Deck, slide_id: &str, order: Vec<String>) -> Result<crate::model::Deck> {
    let mut next = deck.clone();
    let slide = next.slides.iter_mut().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("slide missing".into()))?;
    slide.review.get_or_insert_with(Default::default).reading_order = order.clone(); validate_slide(slide)?;
    if !order.is_empty() { slide.elements.sort_by_key(|element| order.iter().position(|id| id == element.bounds().0)); }
    crate::model::validate_deck(&next)?; Ok(next)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessibilityIssue { pub code: String, pub slide_id: String, pub element_id: Option<String>, pub status: String, pub contrast_ratio: Option<f64>, pub required_ratio: Option<f64>, pub repair_target: Option<String> }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessibilityReport { pub issues: Vec<AccessibilityIssue>, pub checked_features: Vec<String>, pub limitations: Vec<String>, pub wcag_certified: bool }

pub fn contrast_ratio(foreground: &str, background: &str) -> Result<f64> {
    fn luminance(color: &str) -> Result<f64> {
        if color.len() != 6 || !color.bytes().all(|byte| byte.is_ascii_hexdigit()) { return Err(Error::Invalid("contrast requires resolved six-digit RGB".into())); }
        let mut result = 0.0;
        for (offset, weight) in [(0, 0.2126), (2, 0.7152), (4, 0.0722)] {
            let channel = u8::from_str_radix(&color[offset..offset + 2], 16).map_err(|_| Error::Invalid("RGB channel".into()))? as f64 / 255.0;
            result += weight * if channel <= 0.04045 { channel / 12.92 } else { ((channel + 0.055) / 1.055).powf(2.4) };
        }
        Ok(result)
    }
    let foreground = luminance(foreground)?; let background = luminance(background)?;
    Ok((foreground.max(background) + 0.05) / (foreground.min(background) + 0.05))
}

pub fn check_accessibility(deck: &crate::model::Deck) -> AccessibilityReport {
    use crate::model::Element;
    let mut issues = Vec::new();
    for slide in &deck.slides {
        let resolve = |color: &str| crate::design::resolve_color(color, crate::design::slide_theme(slide, deck.design.as_ref()));
        let mut issue = |code: &str, element: Option<&str>, status: &str, ratio, required| issues.push(AccessibilityIssue { code: code.into(), slide_id: slide.id.clone(), element_id: element.map(str::to_owned), status: status.into(), contrast_ratio: ratio, required_ratio: required, repair_target: if code.contains("alt_missing") { Some("alternative_text".into()) } else if code.starts_with("table_") { Some("table_headers".into()) } else { None } });
        if slide.title.trim().is_empty() { issue("title_missing", None, "finding", None, None); }
        if slide.review.as_ref().is_none_or(|review| review.reading_order.is_empty()) { issue("reading_order_review", None, "manual_review", None, None); }
        for element in element_list(&slide.elements) {
            let id = element.bounds().0;
            let metadata = slide.review.as_ref().and_then(|review| review.accessibility.get(id));
            if metadata.is_some_and(|metadata| metadata.decorative == Some(true)) { issue("decoration_meaning_unverified", Some(id), "manual_review", None, None); continue; }
            match element {
                Element::Picture { alt, .. } => {
                    if metadata.and_then(|metadata| metadata.description.as_deref()).unwrap_or(alt).trim().is_empty() { issue("image_alt_missing", Some(id), "finding", None, None); }
                }
                Element::Table { rows, format, .. } => {
                    let policy = slide.review.as_ref().and_then(|review| review.table_headers.get(id)).copied().unwrap_or_default();
                    match policy {
                        TableHeaders::Unknown => issue("table_headers_unknown", Some(id), "manual_review", None, None),
                        TableHeaders::None => issue("table_headers_none_review", Some(id), "manual_review", None, None),
                        _ => {
                            let empty_row = matches!(policy, TableHeaders::FirstRow | TableHeaders::Both) && rows.first().is_none_or(|row| row.iter().any(|cell| cell.trim().is_empty()));
                            let empty_column = matches!(policy, TableHeaders::FirstColumn | TableHeaders::Both) && rows.iter().any(|row| row.first().is_none_or(|cell| cell.trim().is_empty()));
                            if empty_row || empty_column { issue("table_declared_header_empty", Some(id), "finding", None, None); }
                        }
                    }
                    if !format.merges.is_empty() { issue("table_merged_semantics_unverified", Some(id), "manual_review", None, None); }
                }
                Element::Text { text, color, font_size, bold, format, .. } | Element::Shape { text, color, font_size, bold, format, .. } if !text.trim().is_empty() => {
                    let (_, left, top, width, height) = element.bounds();
                    let simple = element.visual().is_none() && !slide.inherit_background && !format.paragraphs.iter().flat_map(|paragraph| &paragraph.runs).any(|run| run.style.highlight.as_deref().is_some_and(|value| value != "none") || run.style.baseline.is_some_and(|value| value != 0)) && slide.elements.iter().any(|candidate| candidate.bounds().0 == id)
                        && !slide.elements.iter().any(|other| { let (other_id, other_left, other_top, other_width, other_height) = other.bounds(); other_id != id && left < other_left + other_width && other_left < left + width && top < other_top + other_height && other_top < top + height });
                    if !simple { issue("contrast_background_unverified", Some(id), "unsupported", None, None); continue; }
                    let background = match element { Element::Shape { fill, .. } if fill != "none" => resolve(fill), _ => resolve(&slide.background) };
                    let mut check = |color: &str, size: f64, bold: bool| {
                        let required = if size >= 24.0 || (bold && size >= 56.0 / 3.0) { 3.0 } else { 4.5 };
                        match contrast_ratio(&resolve(color), &background) { Ok(ratio) if ratio < required => issue("text_contrast_low", Some(id), "finding", Some(ratio), Some(required)), Err(_) => issue("contrast_color_unverified", Some(id), "unsupported", None, None), _ => {} }
                    };
                    if format.paragraphs.is_empty() { check(color, *font_size, *bold); }
                    else { for run in format.paragraphs.iter().flat_map(|paragraph| &paragraph.runs).filter(|run| !run.text.trim().is_empty()) { check(run.style.color.as_deref().unwrap_or(color), run.style.font_size.unwrap_or(*font_size), run.style.bold.unwrap_or(*bold)); } }
                }
                Element::Chart { .. } | Element::Group { .. } => {
                    if metadata.and_then(|metadata| metadata.description.as_deref()).is_none_or(|description| description.trim().is_empty()) { issue(if matches!(element, Element::Chart { .. }) { "chart_alt_missing" } else { "group_alt_missing" }, Some(id), "finding", None, None); }
                    if matches!(element, Element::Group { .. }) { issue("group_reading_order_unverified", Some(id), "manual_review", None, None); }
                }
                _ => {},
            }
        }
    }
    AccessibilityReport { issues, checked_features: ["slide_titles", "image_descriptions", "solid_rgb_text_contrast", "table_header_presence", "merged_table_flags", "top_level_reading_order", "decoration_flags"].map(str::to_owned).to_vec(), limitations: ["Not a WCAG certification or screen-reader test", "Alternative text meaning and arbitrary personal data are not inferred", "Complex backgrounds, inherited content, table header associations and group reading order require review"].map(str::to_owned).to_vec(), wcag_certified: false }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionCategory { Sources, CustomXml, Comments, Notes, UnusedMedia, OffSlide, Invisible }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InspectionFinding { pub category: InspectionCategory, pub count: usize, pub paths: Vec<String> }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersonalDataCandidate { pub rule: String, pub scope: String, pub surface: String, pub count: usize, pub locations: Vec<Vec<usize>>, pub masked_value: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InspectionReport { pub findings: Vec<InspectionFinding>, pub candidates: Vec<PersonalDataCandidate>, pub candidate_scan_truncated: bool, pub limitations: Vec<String>, pub complete_personal_data_detection: bool }

#[derive(Default)]
struct CandidateScan {
    entries: BTreeMap<(String, String, String), PersonalDataCandidate>,
    bytes: usize,
    locations: usize,
    truncated: bool,
}

impl CandidateScan {
    fn record(&mut self, rule: &str, scope: &str, surface: &str, location: &[usize], offset: usize) {
        if self.locations >= 4096 { self.truncated = true; return; }
        self.locations += 1;
        let entry = self.entries.entry((rule.into(), scope.into(), surface.into())).or_insert_with(|| PersonalDataCandidate { rule: rule.into(), scope: scope.into(), surface: surface.into(), count: 0, locations: Vec::new(), masked_value: "[REDACTED]".into() });
        entry.count += 1;
        let mut location = location.to_vec(); location.push(offset); entry.locations.push(location);
    }

    fn text(&mut self, text: &str, scope: &str, surface: &str, location: &[usize]) {
        self.bytes = self.bytes.saturating_add(text.len());
        if self.bytes > 8 * 1024 * 1024 { self.truncated = true; return; }
        let email_character = |character: char| character.is_ascii_alphanumeric() || "._%+-@".contains(character);
        let mut start = 0;
        for token in text.split(|character| !email_character(character)) {
            if let Some(relative) = text[start..].find(token) { start += relative; }
            let token = token.trim_matches('.');
            if let Some((local, domain)) = token.split_once('@') {
                if !local.is_empty() && local.len() <= 64 && !local.contains('@') && !domain.contains('@') && domain.len() <= 253 && domain.split('.').count() >= 2 && domain.split('.').all(|label| !label.is_empty() && label.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')) && domain.rsplit('.').next().is_some_and(|label| label.len() >= 2 && label.bytes().all(|byte| byte.is_ascii_alphabetic())) { self.record("email_candidate", scope, surface, location, text[..start].chars().count()); }
            }
            start = (start + token.len()).min(text.len());
        }
        let bytes = text.as_bytes(); let mut index = 0;
        while index < bytes.len() {
            if !(bytes[index].is_ascii_digit() || matches!(bytes[index], b'+' | b'(')) || (index > 0 && (bytes[index - 1].is_ascii_alphanumeric() || matches!(bytes[index - 1], b'-' | b'@'))) { index += 1; continue; }
            let begin = index;
            while index < bytes.len() && (bytes[index].is_ascii_digit() || matches!(bytes[index], b'+' | b'(' | b')' | b' ' | b'-' | b'.')) { index += 1; }
            let token = text[begin..index].trim();
            let digits = token.bytes().filter(u8::is_ascii_digit).count();
            let groups: Vec<_> = token.split(|character: char| !character.is_ascii_digit()).filter(|group| !group.is_empty()).collect();
            let phone = (10..=15).contains(&digits) && groups.len() >= 3 && groups.len() <= 5 && (token.starts_with('+') || token.starts_with('(') || token.bytes().filter(|byte| *byte == b'-').count() == 2) && !groups.iter().any(|group| group.len() > 4);
            if phone { self.record("phone_candidate", scope, surface, location, text[..begin].chars().count()); }
        }
    }

    fn document(&mut self, document: &crate::document::Document) {
        let scope = "current_deck";
        self.text(&document.deck.title, scope, "document_title", &[]);
        for (slide_index, slide) in document.deck.slides.iter().enumerate() {
            self.text(&slide.title, scope, "slide_title", &[slide_index]);
            self.text(&slide.notes, scope, "notes", &[slide_index]);
            for (paragraph_index, paragraph) in slide.notes_paragraphs.iter().enumerate() { self.text(&crate::rich_text::plain_text(std::slice::from_ref(paragraph)), scope, "rich_notes", &[slide_index, paragraph_index]); }
            for (element_index, element) in element_list(&slide.elements).iter().enumerate() {
                match element {
                    crate::model::Element::Text { text, .. } | crate::model::Element::Shape { text, .. } => self.text(text, scope, "text", &[slide_index, element_index]),
                    crate::model::Element::Table { rows, .. } => for (row_index, row) in rows.iter().enumerate() { for (column_index, cell) in row.iter().enumerate() { self.text(cell, scope, "table_cell", &[slide_index, element_index, row_index, column_index]); } },
                    crate::model::Element::Picture { alt, .. } => self.text(alt, scope, "alternative_text", &[slide_index, element_index]),
                    _ => {},
                }
            }
            if let Some(review) = &slide.review {
                for (index, comment) in review.comments.iter().enumerate() {
                    self.text(&comment.text, scope, "legacy_comment", &[slide_index, index]);
                    self.text(&comment.author, scope, "comment_author", &[slide_index, index]);
                }
                for (index, thread) in review.modern_threads.iter().flatten().enumerate() {
                    self.text(&crate::rich_text::plain_text(&thread.body), scope, "modern_comment", &[slide_index, index]);
                    self.text(&thread.author.name, scope, "comment_author", &[slide_index, index, 0]);
                    self.text(&thread.author.user_id, scope, "comment_author", &[slide_index, index, 1]);
                    for (reply_index, reply) in thread.replies.iter().enumerate() {
                        self.text(&crate::rich_text::plain_text(&reply.body), scope, "modern_reply", &[slide_index, index, reply_index]);
                        self.text(&reply.author.name, scope, "comment_author", &[slide_index, index, reply_index, 0]);
                        self.text(&reply.author.user_id, scope, "comment_author", &[slide_index, index, reply_index, 1]);
                    }
                }
            }
        }
        self.sources(&document.sources, scope);
    }

    fn sources(&mut self, sources: &[crate::sources::SourceDocument], scope: &str) {
        for (source_index, source) in sources.iter().enumerate() {
            self.text(&source.text, scope, "source_plain", &[source_index]);
            for (table_index, table) in source.tables.iter().enumerate() {
                for (column_index, column) in table.columns.iter().enumerate() { self.text(column, scope, "source_cell", &[source_index, table_index, 0, column_index]); }
                for (row_index, row) in table.rows.iter().enumerate() { for (column_index, value) in row.iter().enumerate() { if let Some(text) = value.as_str() { self.text(text, scope, "source_cell", &[source_index, table_index, row_index + 1, column_index]); } } }
            }
        }
    }

    fn package(&mut self, package: &crate::package::Package, scope: &str, properties_only: bool) -> Result<()> {
        for (part_index, (path, kind)) in content_types(package)?.into_iter().enumerate() {
            let core_properties = kind == "application/vnd.openxmlformats-package.core-properties+xml";
            let custom_properties = kind == "application/vnd.openxmlformats-officedocument.custom-properties+xml";
            let surface = if core_properties { "core_properties" } else if custom_properties { "custom_properties" }
                else if properties_only { continue; }
                else if comment_type(&kind) { "comments_xml" }
                else if kind == "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml" { "notes_xml" }
                else if kind == "application/vnd.openxmlformats-officedocument.presentationml.slide+xml" { "slide_xml" }
                else { continue; };
            let xml = package.text(&path)?;
            if self.bytes.saturating_add(xml.len()) > 8 * 1024 * 1024 { self.truncated = true; continue; }
            let parsed = crate::pptx::parse(xml)?;
            for (node_index, node) in parsed.descendants().filter(|node| node.is_element()).enumerate() {
                let location = [part_index, node_index];
                if core_properties || custom_properties {
                    if node.children().any(|node| node.is_element()) { continue; }
                    if let Some(text) = node.text().filter(|text| !text.trim().is_empty()) {
                        self.text(text, scope, surface, &location);
                        if custom_properties || matches!(node.tag_name().name(), "creator" | "lastModifiedBy") { self.record("personal_property_candidate", scope, surface, &location, 0); }
                    }
                } else if node.has_tag_name((crate::native::A, "p")) {
                    let text: String = node.descendants().filter(|node| node.has_tag_name((crate::native::A, "t"))).filter_map(|node| node.text()).collect();
                    self.text(&text, scope, surface, &location);
                } else if node.has_tag_name((crate::native::P, "text")) { self.text(node.text().unwrap_or(""), scope, surface, &location); }
                else if node.has_tag_name((crate::comments::modern::NS, "author")) || node.has_tag_name((crate::native::P, "cmAuthor")) {
                    for name in ["name", "userId", "initials"] { if let Some(value) = node.attribute(name) { self.text(value, scope, "comment_author", &location); } }
                }
            }
        }
        if !properties_only { if let Some((_, provenance)) = crate::provenance::read(package)? { self.sources(&provenance.sources, scope); } }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanCopyOptions { pub new_document_id: String, pub categories: BTreeSet<InspectionCategory>, pub confirmed: bool }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CleanCopy { pub bytes: Vec<u8>, pub document: crate::document::Document, pub inspection: InspectionReport }

#[derive(Clone)]
struct Relationship { source: String, part: String, id: String, kind: String, target: Option<String> }

fn relationships(package: &crate::package::Package) -> Result<Vec<Relationship>> {
    let mut result = Vec::new();
    for path in package.part_names().into_iter().filter(|path| path.ends_with(".rels")) {
        let source = if path == "_rels/.rels" { String::new() }
            else if let Some((folder, name)) = path.rsplit_once("/_rels/") { format!("{folder}/{}", name.trim_end_matches(".rels")) }
            else if let Some(name) = path.strip_prefix("_rels/") { name.trim_end_matches(".rels").into() }
            else { return Err(Error::Unsupported("nonstandard OPC relationship path".into())); };
        let parsed = crate::pptx::parse(package.text(path)?)?;
        if !parsed.root_element().has_tag_name((crate::native::REL, "Relationships")) { return Err(Error::Invalid("OPC relationships root".into())); }
        let mut ids = BTreeSet::new();
        for node in parsed.root_element().children().filter(|node| node.has_tag_name((crate::native::REL, "Relationship"))) {
            let id = node.attribute("Id").ok_or_else(|| Error::Invalid("OPC relationship ID missing".into()))?;
            if !ids.insert(id) { return Err(Error::Invalid("duplicate OPC relationship ID".into())); }
            let raw = node.attribute("Target").ok_or_else(|| Error::Invalid("OPC relationship target missing".into()))?;
            let target = match node.attribute("TargetMode") { Some("External") => None, Some("Internal") | None => Some(crate::pptx::resolve(&source, raw)?), _ => return Err(Error::Unsupported("OPC relationship mode".into())) };
            result.push(Relationship { source: source.clone(), part: path.into(), id: id.into(), kind: node.attribute("Type").unwrap_or("").into(), target });
        }
    }
    Ok(result)
}

fn content_types(package: &crate::package::Package) -> Result<BTreeMap<String, String>> {
    let parsed = crate::pptx::parse(package.text("[Content_Types].xml")?)?;
    let mut result = BTreeMap::new();
    for path in package.part_names() {
        let kind = parsed.root_element().children().find(|node| node.has_tag_name((CT, "Override")) && node.attribute("PartName") == Some(format!("/{path}").as_str()))
            .or_else(|| parsed.root_element().children().find(|node| node.has_tag_name((CT, "Default")) && node.attribute("Extension") == path.rsplit('.').next())).and_then(|node| node.attribute("ContentType"));
        if let Some(kind) = kind { result.insert(path.into(), kind.into()); }
    }
    Ok(result)
}

fn reachable(relations: &[Relationship], removed: &BTreeSet<String>) -> BTreeSet<String> {
    let mut visited = BTreeSet::from([String::new()]); let mut queue = vec![String::new()];
    while let Some(source) = queue.pop() {
        for target in relations.iter().filter(|relation| relation.source == source).filter_map(|relation| relation.target.as_ref()).filter(|target| !removed.contains(*target)) {
            if visited.insert(target.clone()) { queue.push(target.clone()); }
        }
    }
    visited
}

pub(crate) fn ensure_unprotected(package: &crate::package::Package) -> Result<()> {
    if package.part_names().iter().any(|path| { let path = path.to_ascii_lowercase(); path.starts_with("_xmlsignatures/") || path.contains("labelinfo") || path.contains("encryption") }) { return Err(Error::Unsupported("clean-copy/field refresh does not alter signed, labelled or protected packages".into())); }
    for (path, kind) in content_types(package)? {
        let lower = path.to_ascii_lowercase(); let kind = kind.to_ascii_lowercase();
        if lower.starts_with("_xmlsignatures/") || lower.contains("labelinfo") || lower.contains("encryption") || kind.contains("signature") || kind.contains("sensitivitylabel") { return Err(Error::Unsupported("clean-copy/field refresh does not alter signed, labelled or protected packages".into())); }
        if lower.ends_with(".xml") || kind.ends_with("+xml") || kind == "application/xml" || kind == "text/xml" {
            let parsed = crate::pptx::parse(package.text(&path)?)?;
            if parsed.descendants().filter(|node| node.is_element()).any(|node| {
                let namespace = node.tag_name().namespace().unwrap_or("").to_ascii_lowercase();
                namespace.contains("sensitivitylabel") || namespace.contains("miplabel") || namespace.contains("labelmetadata") || namespace.contains("digital-signature") || matches!(node.tag_name().name(), "modifyVerifier" | "EncryptedData" | "Signature")
                    || node.attribute("name").is_some_and(|name| name.to_ascii_lowercase().starts_with("msip_label_"))
            }) { return Err(Error::Unsupported("clean-copy/field refresh does not alter signed, labelled or protected packages".into())); }
        }
    }
    for relation in relationships(package)? { if relation.kind.to_ascii_lowercase().contains("digital-signature") { return Err(Error::Unsupported("signed package".into())); } }
    Ok(())
}

fn current_package(document: &crate::document::Document) -> Result<crate::package::Package> {
    use base64::Engine;
    let bytes = if let Some(origin) = &document.origin {
        let bytes = base64::engine::general_purpose::STANDARD.decode(&origin.base64).map_err(|_| Error::Invalid("origin encoding".into()))?;
        if origin.native { crate::native::save(bytes, &document.deck)? } else { crate::import::save_import(bytes, &document.deck)? }
    } else { crate::pptx::export_pptx(&document.deck)? };
    crate::package::Package::open(crate::provenance::attach(bytes, document)?)
}

fn hidden_or_offslide(package: &crate::package::Package) -> Result<BTreeMap<InspectionCategory, BTreeMap<String, BTreeSet<String>>>> {
    use crate::native::{P, A, child, properties};
    let main = crate::pptx::relationship_targets(package, "", "officeDocument")?.into_values().next().ok_or_else(|| Error::Invalid("presentation missing".into()))?;
    let parsed = crate::pptx::parse(package.text(&main)?)?;
    let size = child(parsed.root_element(), P, "sldSz").ok_or_else(|| Error::Invalid("slide size missing".into()))?;
    let number = |node: roxmltree::Node<'_, '_>, name: &str| node.attribute(name).and_then(|value| value.parse::<f64>().ok()).filter(|value| value.is_finite());
    let width = number(size, "cx").ok_or_else(|| Error::Invalid("slide width".into()))?;
    let height = number(size, "cy").ok_or_else(|| Error::Invalid("slide height".into()))?;
    let mut result: BTreeMap<InspectionCategory, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
    for path in crate::pptx::slide_paths(package)? {
        let parsed = crate::pptx::parse(package.text(&path)?)?;
        let Some(tree) = child(parsed.root_element(), P, "cSld").and_then(|node| child(node, P, "spTree")) else { continue };
        for shape in tree.children().filter(|node| crate::native::is_shape(*node)) {
            let Some(props) = properties(shape) else { continue };
            let Some(id) = props.attribute("id") else { continue };
            let mut categories = Vec::new();
            if matches!(props.attribute("hidden"), Some("1" | "true")) { categories.push(InspectionCategory::Invisible); }
            let transform = child(shape, P, "xfrm").or_else(|| child(shape, P, if shape.has_tag_name((P, "grpSp")) { "grpSpPr" } else { "spPr" }).and_then(|node| child(node, A, "xfrm")));
            if let Some(transform) = transform.filter(|node| number(*node, "rot").unwrap_or(0.0) == 0.0) {
                if let (Some(offset), Some(extent)) = (child(transform, A, "off"), child(transform, A, "ext")) {
                    if let (Some(left), Some(top), Some(object_width), Some(object_height)) = (number(offset, "x"), number(offset, "y"), number(extent, "cx"), number(extent, "cy")) {
                        if object_width > 0.0 && object_height > 0.0 && (left >= width || top >= height || left + object_width <= 0.0 || top + object_height <= 0.0) { categories.push(InspectionCategory::OffSlide); }
                    }
                }
            }
            for category in categories { result.entry(category).or_default().entry(path.clone()).or_default().insert(id.into()); }
        }
    }
    Ok(result)
}

fn comment_type(kind: &str) -> bool {
    ["application/vnd.openxmlformats-officedocument.presentationml.comments+xml", "application/vnd.openxmlformats-officedocument.presentationml.commentAuthors+xml", crate::comments::modern::MIME, crate::comments::modern::AUTHORS_MIME].contains(&kind)
}

pub fn inspect_document(document: &crate::document::Document) -> Result<InspectionReport> {
    inspect_document_inner(document).map_err(|_| Error::Invalid("document inspection failed validation; no candidate values returned".into()))
}

fn inspect_document_inner(document: &crate::document::Document) -> Result<InspectionReport> {
    use InspectionCategory::*;
    use base64::Engine;
    crate::document::verify(document)?;
    let mut candidates = CandidateScan::default();
    candidates.document(document);
    let mut found: BTreeMap<InspectionCategory, BTreeSet<String>> = [Sources, CustomXml, Comments, Notes, UnusedMedia, OffSlide, Invisible].into_iter().map(|category| (category, BTreeSet::new())).collect();
    for index in 0..document.sources.len() { found.entry(Sources).or_default().insert(format!("document/sources/{index}")); }
    for (index, slide) in document.deck.slides.iter().enumerate() {
        if !slide.notes.is_empty() { found.entry(Notes).or_default().insert(format!("document/deck/slides/{index}/notes")); }
        for position in 0..slide.review.as_ref().map_or(0, |review| review.comments.len()) { found.entry(Comments).or_default().insert(format!("document/deck/slides/{index}/review/comments/{position}")); }
    }
    let mut packages = vec![("current", current_package(document)?)];
    if let Some(origin) = &document.origin { packages.push(("original", crate::package::Package::open(base64::engine::general_purpose::STANDARD.decode(&origin.base64).map_err(|_| Error::Invalid("origin encoding".into()))?)?)); }
    for (scope, package) in packages {
        candidates.package(&package, if scope == "current" { "current_deck" } else { "embedded_origin" }, scope == "current")?;
        let relations = relationships(&package)?; let reached = reachable(&relations, &BTreeSet::new());
        let source = crate::provenance::read(&package)?.map(|(path, metadata)| (path, !metadata.sources.is_empty() || !metadata.bindings.is_empty()));
        for (path, kind) in content_types(&package)? {
            let category = if source.as_ref().is_some_and(|(source, evidence)| source == &path && *evidence) { Some(Sources) }
                else if comment_type(&kind) { Some(Comments) }
                else if kind == "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml" { Some(Notes) }
                else if relations.iter().any(|relation| relation.kind == format!("{}/customXml", crate::native::R) && relation.target.as_deref() == Some(&path)) || path.starts_with("customXml/") { Some(CustomXml) }
                else if (kind.starts_with("image/") || path.starts_with("ppt/media/")) && !reached.contains(&path) { Some(UnusedMedia) }
                else { None };
            if let Some(category) = category { found.entry(category).or_default().insert(format!("{scope}/{path}")); }
        }
        for (category, parts) in hidden_or_offslide(&package)? { for (path, ids) in parts { for id in ids { found.entry(category).or_default().insert(format!("{scope}/{path}#shape-{id}")); } } }
    }
    Ok(InspectionReport { findings: found.into_iter().map(|(category, paths)| InspectionFinding { category, count: paths.len(), paths: Vec::new() }).collect(), candidates: candidates.entries.into_values().collect(), candidate_scan_truncated: candidates.truncated, limitations: ["Candidates require manual review; email, formatted phone and personal-property rules are not proof of personal data or completeness", "Numeric locations and Unicode-scalar offsets are scope-local; matched values and paths are never returned", "Current deck and embedded origin are scanned separately; history and recovery snapshots retain previous content", "The scan is bounded to 8 MiB of text and 4096 candidate locations; no OCR or embedded binary inspection", "Off-slide covers wholly outside, unrotated top-level objects; invisible covers native hidden top-level objects", "External relationships are inspected as metadata only; no targets are fetched or executed"].map(str::to_owned).to_vec(), complete_personal_data_detection: false })
}

fn remove_parts(package: &mut crate::package::Package, removed: &BTreeSet<String>) -> Result<()> {
    if removed.is_empty() { return Ok(()); }
    let mut all = removed.clone();
    for part in removed { let rels = crate::native::relations_path(part); if package.part_names().contains(rels.as_str()) { all.insert(rels); } }
    let mut updates: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for relation in relationships(package)? {
        if !all.contains(&relation.part) && relation.target.as_ref().is_some_and(|target| all.contains(target)) { updates.entry(relation.part).or_default().insert(relation.id); }
    }
    for (path, ids) in updates {
        let xml = package.text(&path)?.to_owned(); let parsed = crate::pptx::parse(&xml)?;
        let edits = parsed.root_element().children().filter(|node| node.has_tag_name((crate::native::REL, "Relationship")) && node.attribute("Id").is_some_and(|id| ids.contains(id))).map(|node| (node.range(), String::new())).collect();
        package.replace_part(&path, crate::native_save::apply(xml, edits)?)?;
    }
    let xml = package.text("[Content_Types].xml")?.to_owned(); let parsed = crate::pptx::parse(&xml)?;
    let edits = parsed.root_element().children().filter(|node| node.has_tag_name((CT, "Override")) && node.attribute("PartName").is_some_and(|path| all.contains(path.trim_start_matches('/')))).map(|node| (node.range(), String::new())).collect();
    package.replace_part("[Content_Types].xml", crate::native_save::apply(xml, edits)?)?;
    for path in all { if package.part_names().contains(path.as_str()) { package.remove_part(&path)?; } }
    Ok(())
}

fn remove_shapes(package: &mut crate::package::Package, selected: &BTreeSet<InspectionCategory>, document: &crate::document::Document) -> Result<BTreeSet<(String, String)>> {
    let native = crate::native::read(package)?;
    let mut paths: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (category, parts) in hidden_or_offslide(package)? { if selected.contains(&category) { for (path, ids) in parts { paths.entry(path).or_default().extend(ids); } } }
    let mut removed = BTreeSet::new();
    for (path, ids) in paths {
        let binding = native.slides.iter().find(|binding| binding.path == path).ok_or_else(|| Error::Invalid("redaction slide missing".into()))?;
        let xml = package.text(&path)?.to_owned(); let parsed = crate::pptx::parse(&xml)?;
        let shapes: Vec<_> = parsed.descendants().filter(|node| crate::native::is_shape(*node) && crate::native::properties(*node).and_then(|node| node.attribute("id")).is_some_and(|id| ids.contains(id))).collect();
        let ids: BTreeSet<_> = shapes.iter().flat_map(|shape| shape.descendants()).filter(|node| crate::native::is_shape(*node)).filter_map(|node| crate::native::properties(node).and_then(|props| props.attribute("id"))).map(str::to_owned).collect();
        for (element_id, numeric) in &binding.nodes { if ids.contains(numeric) { removed.insert((binding.id.clone(), element_id.clone())); } }
        let mut edits = Vec::new();
        for node in parsed.descendants().filter(|node| node.is_element()) {
            if shapes.iter().any(|shape| node.ancestors().any(|ancestor| ancestor == *shape)) { continue; }
            if node.attributes().any(|attribute| ["spid", "spId"].contains(&attribute.name()) && ids.contains(attribute.value())) { return Err(Error::Unsupported("redacted shape is referenced by native animation/metadata".into())); }
            if ["stCxn", "endCxn"].contains(&node.tag_name().name()) && node.attribute("id").is_some_and(|id| ids.contains(id)) { edits.push((node.range(), String::new())); }
        }
        for shape in shapes { edits.push((shape.range(), String::new())); }
        package.replace_part(&path, crate::native_save::apply(xml, edits)?)?;
    }
    if !selected.contains(&InspectionCategory::Sources) && document.bindings.iter().any(|binding| removed.contains(&(binding.slide_id.clone(), binding.element_id.clone()))) { return Err(Error::Unsupported("removing a source-bound object also requires explicit sources selection".into())); }
    Ok(removed)
}

fn remove_modern_links(package: &mut crate::package::Package) -> Result<()> {
    for path in crate::pptx::slide_paths(package)? {
        let xml = package.text(&path)?.to_owned(); let parsed = crate::pptx::parse(&xml)?;
        let edits: Vec<_> = crate::native::child(parsed.root_element(), crate::native::P, "extLst").into_iter().flat_map(|list| list.children()).filter(|node| node.has_tag_name((crate::native::P, "ext")) && node.attribute("uri") == Some(crate::comments::modern::EXT)).map(|node| (node.range(), String::new())).collect();
        if !edits.is_empty() { package.replace_part(&path, crate::native_save::apply(xml, edits)?)?; }
    }
    Ok(())
}

pub fn export_clean_copy(document: &crate::document::Document, options: &CleanCopyOptions) -> Result<CleanCopy> {
    use InspectionCategory::*;
    crate::document::verify(document)?;
    valid_text(&options.new_document_id, 80)?;
    if !options.confirmed || options.new_document_id.is_empty() || options.new_document_id == document.id || options.categories.is_empty() { return Err(Error::Invalid("clean-copy export requires confirmation, explicit categories and a new document ID".into())); }
    let mut package = current_package(document)?;
    ensure_unprotected(&package)?;
    let mut sanitized = document.clone();
    if options.categories.contains(&Sources) { sanitized.sources.clear(); sanitized.bindings.clear(); sanitized.report = None; }
    let removed_elements = remove_shapes(&mut package, &options.categories, document)?;
    sanitized.parts.retain(|part| !removed_elements.contains(&(part.slide_id.clone(), part.element_id.clone())));
    let types = content_types(&package)?; let relations = relationships(&package)?;
    let provenance = crate::provenance::read(&package)?.map(|(path, _)| path);
    let mut removed = BTreeSet::new();
    for (path, kind) in &types {
        if (options.categories.contains(&Comments) && comment_type(kind))
            || (options.categories.contains(&Notes) && ["application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml", "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"].contains(&kind.as_str()))
            || (options.categories.contains(&CustomXml) && Some(path) != provenance.as_ref() && (path.starts_with("customXml/") || relations.iter().any(|relation| relation.kind == format!("{}/customXml", crate::native::R) && relation.target.as_ref() == Some(path)))) { removed.insert(path.clone()); }
    }
    if options.categories.contains(&Notes) {
        let main = crate::pptx::relationship_targets(&package, "", "officeDocument")?.into_values().next().ok_or_else(|| Error::Invalid("presentation missing".into()))?;
        let xml = package.text(&main)?.to_owned(); let parsed = crate::pptx::parse(&xml)?;
        if let Some(list) = crate::native::child(parsed.root_element(), crate::native::P, "notesMasterIdLst") { package.replace_part(&main, crate::native_save::apply(xml.clone(), vec![(list.range(), String::new())])?)?; }
    }
    if options.categories.contains(&UnusedMedia) {
        let reached = reachable(&relations, &removed);
        for (path, kind) in &types { if (kind.starts_with("image/") || path.starts_with("ppt/media/")) && !reached.contains(path) { removed.insert(path.clone()); } }
    }
    if options.categories.contains(&Comments) { remove_modern_links(&mut package)?; }
    remove_parts(&mut package, &removed)?;
    let native = crate::native::read(&package)?;
    sanitized.deck = native.deck;
    let bytes = crate::provenance::attach(package.save()?, &sanitized)?;
    let verified = crate::package::Package::open(bytes.clone())?;
    for relation in relationships(&verified)? { if let Some(target) = relation.target { if !verified.part_names().contains(target.as_str()) { return Err(Error::Invalid("clean copy would contain a dangling OPC relationship".into())); } } }
    let clean: crate::document::Document = serde_json::from_value(crate::document::open_presentation(options.new_document_id.clone(), bytes.clone())?["document"].clone())?;
    let inspection = inspect_document(&clean)?;
    Ok(CleanCopy { bytes, document: clean, inspection })
}