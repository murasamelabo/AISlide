use crate::{Error, Result, model::valid_text};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub id: String,
    pub kind: String,
}

impl Field {
    pub fn validate(&self) -> Result<()> {
        parse_id(&self.id)?;
        valid_text(&self.kind, 128)?;
        if self.kind.is_empty() || self.kind.chars().any(char::is_control) { return Err(Error::Invalid("empty field type or control character".into())); }
        Ok(())
    }

    pub fn cached_text(&self, slide_number: usize, reference_date: &str) -> Result<Option<String>> {
        self.cached_text_with_options(slide_number, reference_date, None, "en-US")
    }

    pub fn cached_text_with_options(&self, slide_number: usize, reference_date: &str, reference_time: Option<&str>, locale: &str) -> Result<Option<String>> {
        self.validate()?;
        let (year, month, day) = parse_date(reference_date)?;
        let time = reference_time.map(parse_time).transpose()?;
        if self.kind == "slidenum" { return Ok(Some(slide_number.to_string())); }
        if locale != "en-US" { return Ok(None); }
        let months = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
        let month_name = months[month as usize - 1];
        let short = &month_name[..3];
        let short_year = year % 100;
        let date = format!("{month}/{day}/{year}");
        let adjusted_year = year - u32::from(month < 3);
        let offset = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4][month as usize - 1];
        let weekday = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"][((adjusted_year + adjusted_year / 4 - adjusted_year / 100 + adjusted_year / 400 + offset + day) % 7) as usize];
        let with_time = |seconds: bool, clock24: bool| time.map(|(hour, minute, second)| {
            let display_hour = if clock24 { hour } else if hour % 12 == 0 { 12 } else { hour % 12 };
            let mut value = if clock24 { format!("{display_hour:02}:{minute:02}") } else { format!("{display_hour}:{minute:02}") };
            if seconds { value.push_str(&format!(":{second:02}")); }
            if !clock24 { value.push_str(if hour < 12 { " AM" } else { " PM" }); }
            value
        });
        Ok(match self.kind.as_str() {
            "datetime1" => Some(date.clone()),
            "datetime2" => Some(format!("{weekday}, {month_name} {day}, {year}")),
            "datetime3" => Some(format!("{day} {month_name} {year}")),
            "datetime4" => Some(format!("{month_name} {day}, {year}")),
            "datetime5" => Some(format!("{day:02}-{short}-{short_year:02}")),
            "datetime6" => Some(format!("{month_name} {short_year:02}")),
            "datetime7" => Some(format!("{short}-{short_year:02}")),
            "datetime8" => with_time(false, false).map(|time| format!("{date} {time}")),
            "datetime9" => with_time(true, false).map(|time| format!("{date} {time}")),
            "datetime10" => with_time(false, true),
            "datetime11" => with_time(true, true),
            "datetime12" => with_time(false, false),
            "datetime13" => with_time(true, false),
            _ => None,
        })
    }
}

fn parse_time(value: &str) -> Result<(u32, u32, u32)> {
    let invalid = || Error::Invalid("reference time must be HH:mm:ss in 00:00:00..23:59:59".into());
    let bytes = value.as_bytes();
    if bytes.len() != 8 || bytes[2] != b':' || bytes[5] != b':' || bytes.iter().enumerate().any(|(index, byte)| index != 2 && index != 5 && !byte.is_ascii_digit()) { return Err(invalid()); }
    let hour = value[..2].parse().map_err(|_| invalid())?;
    let minute = value[3..5].parse().map_err(|_| invalid())?;
    let second = value[6..].parse().map_err(|_| invalid())?;
    if hour > 23 || minute > 59 || second > 59 { return Err(invalid()); }
    Ok((hour, minute, second))
}

pub(crate) fn supported_kind(kind: &str) -> bool {
    kind == "slidenum" || matches!(kind, "datetime1" | "datetime2" | "datetime3" | "datetime4" | "datetime5" | "datetime6" | "datetime7" | "datetime8" | "datetime9" | "datetime10" | "datetime11" | "datetime12" | "datetime13")
}

pub fn parse_id(value: &str) -> Result<[u8; 16]> {
    let value = if value.starts_with('{') && value.ends_with('}') { &value[1..value.len() - 1] } else { value };
    if value.len() != 36 || [8, 13, 18, 23].iter().any(|index| value.as_bytes()[*index] != b'-') { return Err(Error::Invalid("field ID must be a UUID".into())); }
    let digits: Vec<_> = value.bytes().enumerate().filter(|(index, _)| ![8, 13, 18, 23].contains(index)).map(|(_, byte)| byte).collect();
    let mut result = [0; 16];
    for (index, pair) in digits.chunks_exact(2).enumerate() {
        let high = (pair[0] as char).to_digit(16).ok_or_else(|| Error::Invalid("field UUID contains non-hex digits".into()))?;
        let low = (pair[1] as char).to_digit(16).ok_or_else(|| Error::Invalid("field UUID contains non-hex digits".into()))?;
        result[index] = (high * 16 + low) as u8;
    }
    Ok(result)
}

pub(crate) fn parse_date(value: &str) -> Result<(u32, u32, u32)> {
    let invalid = || Error::Invalid("reference date must be a valid YYYY-MM-DD date".into());
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' || bytes.iter().enumerate().any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit()) { return Err(invalid()); }
    let year: u32 = value[..4].parse().map_err(|_| invalid())?;
    let month: u32 = value[5..7].parse().map_err(|_| invalid())?;
    let day: u32 = value[8..].parse().map_err(|_| invalid())?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month { 2 => if leap { 29 } else { 28 }, 4 | 6 | 9 | 11 => 30, 1 | 3 | 5 | 7 | 8 | 10 | 12 => 31, _ => return Err(invalid()) };
    if year == 0 || day == 0 || day > days { return Err(invalid()); }
    Ok((year, month, day))
}

pub fn has_fields(element: &crate::model::Element) -> bool {
    match element {
        crate::model::Element::Text { format, .. } | crate::model::Element::Shape { format, .. } => format.paragraphs.iter().flat_map(|paragraph| &paragraph.runs).any(|run| run.field.is_some()),
        _ => false,
    }
}

pub fn refresh(deck: &crate::model::Deck, reference_date: &str) -> Result<crate::model::Deck> {
    refresh_with_options(deck, reference_date, None, "en-US")
}

pub fn refresh_with_options(deck: &crate::model::Deck, reference_date: &str, reference_time: Option<&str>, locale: &str) -> Result<crate::model::Deck> {
    parse_date(reference_date)?;
    reference_time.map(parse_time).transpose()?;
    valid_text(locale, 64)?;
    crate::model::validate_deck(deck)?;
    fn visit(elements: &mut [crate::model::Element], number: usize, date: &str, time: Option<&str>, locale: &str) -> Result<()> {
        for element in elements {
            match element {
                crate::model::Element::Group { children, .. } => visit(children, number, date, time, locale)?,
                crate::model::Element::Text { text, format, .. } | crate::model::Element::Shape { text, format, .. } => {
                    for run in format.paragraphs.iter_mut().flat_map(|paragraph| &mut paragraph.runs) {
                        if let Some(field) = &run.field { if let Some(value) = field.cached_text_with_options(number, date, time, locale)? { run.text = value; } }
                    }
                    if !format.paragraphs.is_empty() { *text = crate::rich_text::plain_text(&format.paragraphs); }
                }
                _ => {}
            }
        }
        Ok(())
    }
    let mut next = deck.clone();
    for (index, slide) in next.slides.iter_mut().enumerate() {
        visit(&mut slide.elements, index + 1, reference_date, reference_time, locale)?;
        for run in slide.notes_paragraphs.iter_mut().flat_map(|paragraph| &mut paragraph.runs) {
            if let Some(field) = &run.field { if let Some(value) = field.cached_text_with_options(index + 1, reference_date, reference_time, locale)? { run.text = value; } }
        }
        if !slide.notes_paragraphs.is_empty() { slide.notes = crate::rich_text::plain_text(&slide.notes_paragraphs); }
    }
    crate::model::validate_deck(&next)?;
    Ok(next)
}

pub(crate) fn replace_body(mut element: crate::model::Element, replacement: String) -> Result<crate::model::Element> {
    use crate::model::Element;
    let (text, format) = match &mut element { Element::Text { text, format, .. } | Element::Shape { text, format, .. } => (text, format), _ => return Err(Error::Unsupported("field body".into())) };
    if *text == replacement { return Ok(element); }
    let lines: Vec<_> = replacement.split('\n').collect();
    if replacement.contains('\r') || lines.len() != format.paragraphs.len() { return Err(Error::Unsupported("field body edits must retain paragraph boundaries".into())); }
    for (paragraph, line) in format.paragraphs.iter_mut().zip(lines) {
        let old: Vec<_> = paragraph.runs.iter().flat_map(|run| run.text.chars()).collect();
        let new: Vec<_> = line.chars().collect();
        let prefix = old.iter().zip(&new).take_while(|(left, right)| left == right).count();
        let suffix = old[prefix..].iter().rev().zip(new[prefix..].iter().rev()).take_while(|(left, right)| left == right).count();
        if prefix == old.len() && prefix == new.len() { continue; }
        let end = old.len() - suffix;
        let mut offset = 0;
        let mut inserted = false;
        let mut runs = Vec::new();
        for run in &paragraph.runs {
            let chars: Vec<_> = run.text.chars().collect();
            let limit = offset + chars.len();
            if run.field.is_some() && ((prefix < limit && end > offset) || (prefix == end && prefix > offset && prefix < limit) || (chars.is_empty() && prefix <= offset && end >= offset)) { return Err(Error::Unsupported("plain body edits cannot modify or remove dynamic fields".into())); }
            if !inserted && prefix <= limit {
                let left = prefix.saturating_sub(offset).min(chars.len());
                if left > 0 { let mut kept = run.clone(); kept.text = chars[..left].iter().collect(); runs.push(kept); }
                let added: String = new[prefix..new.len() - suffix].iter().collect();
                if !added.is_empty() { runs.push(crate::rich_text::RichRun { text: added, style: run.style.clone(), field: None }); }
                inserted = true;
                let right = end.saturating_sub(offset).min(chars.len());
                if right < chars.len() { let mut kept = run.clone(); kept.text = chars[right..].iter().collect(); runs.push(kept); }
            } else if !inserted || offset >= end { runs.push(run.clone()); }
            else if limit > end { let mut kept = run.clone(); kept.text = chars[end - offset..].iter().collect(); runs.push(kept); }
            offset = limit;
        }
        paragraph.runs = runs;
    }
    *text = replacement;
    crate::rich_text::validate_element(&element)?;
    Ok(element)
}

pub(crate) fn read_field(node: roxmltree::Node<'_, '_>) -> Result<Option<Field>> {
    if !node.has_tag_name((crate::native::A, "fld")) { return Ok(None); }
    let field = Field { id: node.attribute("id").unwrap_or("").into(), kind: node.attribute("type").unwrap_or("unknown").into() };
    field.validate()?;
    Ok(Some(field))
}

fn write_cache(xml: &str, field: roxmltree::Node<'_, '_>, value: &str, edits: &mut Vec<(std::ops::Range<usize>, String)>) -> Result<()> {
    use crate::native::A;
    valid_text(value, 4000)?;
    let texts: Vec<_> = field.children().filter(|node| node.has_tag_name((A, "t"))).collect();
    if texts.len() > 1 { return Err(Error::Unsupported("field cache has multiple text nodes".into())); }
    if let Some(text) = texts.first() {
        if text.children().any(|node| !node.is_text()) { return Err(Error::Unsupported("field cache contains non-text content".into())); }
        let current: String = text.children().filter_map(|node| node.text()).collect();
        if current == value { return Ok(()); }
        let escaped = quick_xml::escape::escape(value).into_owned();
        if let (Some(first), Some(last)) = (text.first_child(), text.last_child()) {
            edits.push((first.range().start..last.range().end, escaped));
        } else { crate::native_save::insert_child(xml, *text, &escaped, None, edits)?; }
    } else {
        crate::native_save::insert_child(xml, field, &format!("<a:t xmlns:a=\"{A}\">{}</a:t>", quick_xml::escape::escape(value)), crate::native::child(field, A, "extLst"), edits)?;
    }
    Ok(())
}

pub(crate) fn patch_paragraph_cache(xml: &str, node: roxmltree::Node<'_, '_>, before: &crate::rich_text::RichParagraph, after: &crate::rich_text::RichParagraph, edits: &mut Vec<(std::ops::Range<usize>, String)>) -> Result<bool> {
    let mut normalized = after.clone();
    if before.runs.len() != after.runs.len() { return Ok(false); }
    for (previous, next) in before.runs.iter().zip(&mut normalized.runs) {
        if previous.field.as_ref().is_some_and(|field| supported_kind(&field.kind)) { next.text = previous.text.clone(); }
    }
    if before != &normalized { return Ok(false); }
    let runs: Vec<_> = node.children().filter(|node| node.has_tag_name((crate::native::A, "r")) || node.has_tag_name((crate::native::A, "fld"))).collect();
    if runs.len() != before.runs.len() { return Err(Error::Conflict("notes field run count mismatch".into())); }
    let mut pending = Vec::new();
    for ((previous, next), native) in before.runs.iter().zip(&after.runs).zip(runs) {
        if previous.text == next.text { continue; }
        if read_field(native)? != previous.field { return Err(Error::Conflict("notes field identity/type changed".into())); }
        let cached: String = native.children().filter(|node| node.has_tag_name((crate::native::A, "t"))).filter_map(|node| node.text()).collect();
        if cached != previous.text { return Err(Error::Conflict("notes field cache changed".into())); }
        write_cache(xml, native, &next.text, &mut pending)?;
    }
    edits.extend(pending);
    Ok(true)
}

pub(crate) fn patch_cache_only(xml: &str, node: roxmltree::Node<'_, '_>, old: &crate::model::Element, new: &crate::model::Element,
    edits: &mut Vec<(std::ops::Range<usize>, String)>) -> Result<bool> {
    use crate::{model::Element, native::{A, P, child}};
    if !has_fields(old) { return Ok(false); }
    crate::rich_text::validate_element(old)?;
    crate::rich_text::validate_element(new)?;
    let (old_text, before) = match old { Element::Text { text, format, .. } | Element::Shape { text, format, .. } => (text, format), _ => return Ok(false) };
    let after = match new { Element::Text { format, .. } | Element::Shape { format, .. } => format, _ => return Ok(false) };
    let mut normalized = new.clone();
    let (normalized_text, normalized_format) = match &mut normalized { Element::Text { text, format, .. } | Element::Shape { text, format, .. } => (text, format), _ => return Ok(false) };
    if before.paragraphs.len() != after.paragraphs.len() { return Ok(false); }
    let mut changed = false;
    for (previous, next) in before.paragraphs.iter().zip(&mut normalized_format.paragraphs) {
        if previous.runs.len() != next.runs.len() { return Ok(false); }
        for (previous, next) in previous.runs.iter().zip(&mut next.runs) {
            if previous.text != next.text && previous.field.as_ref().is_some_and(|field| supported_kind(&field.kind)) {
                next.text = previous.text.clone();
                changed = true;
            }
        }
    }
    *normalized_text = old_text.clone();
    if !changed || crate::canonical::bytes(old)? != crate::canonical::bytes(&normalized)? { return Ok(false); }
    let body = child(node, P, "txBody").ok_or_else(|| Error::Conflict("field text body missing".into()))?;
    let paragraphs: Vec<_> = body.children().filter(|node| node.has_tag_name((A, "p"))).collect();
    if paragraphs.len() != before.paragraphs.len() { return Err(Error::Conflict("field paragraph count mismatch".into())); }
    let mut pending = Vec::new();
    for ((previous, next), paragraph) in before.paragraphs.iter().zip(&after.paragraphs).zip(paragraphs) {
        let runs: Vec<_> = paragraph.children().filter(|node| node.has_tag_name((A, "r")) || node.has_tag_name((A, "fld"))).collect();
        if runs.len() != previous.runs.len() { return Err(Error::Conflict("field run count mismatch".into())); }
        for ((previous, next), native) in previous.runs.iter().zip(&next.runs).zip(runs) {
            if read_field(native)? != previous.field { return Err(Error::Conflict("native field identity/type changed".into())); }
            if previous.text != next.text {
                let cached: String = native.children().filter(|node| node.has_tag_name((A, "t"))).flat_map(|node| node.children()).filter_map(|node| node.text()).collect();
                if cached != previous.text { return Err(Error::Conflict("native field cache changed".into())); }
                write_cache(xml, native, &next.text, &mut pending)?;
            }
        }
    }
    edits.extend(pending);
    Ok(true)
}

pub(crate) fn write_generated(package: &mut crate::package::Package, deck: &crate::model::Deck) -> Result<()> {
    write_bound(package, deck, &[])?;
    if let Some(design) = &deck.design {
        for (index, master) in design.masters.iter().enumerate() {
            write_part(package, &format!("ppt/slideMasters/slideMaster{}.xml", index + 1), &master.elements, None)?;
        }
        for (index, layout) in design.layouts.iter().enumerate() {
            write_part(package, &format!("ppt/slideLayouts/slideLayout{}.xml", index + 1), &layout.elements, None)?;
        }
    }
    Ok(())
}

pub(crate) fn write_bound(package: &mut crate::package::Package, deck: &crate::model::Deck, bindings: &[crate::native::NativePart]) -> Result<()> {
    for (index, (slide, path)) in deck.slides.iter().zip(crate::pptx::slide_paths(package)?).enumerate() {
        write_part(package, &path, &slide.elements, bindings.get(index))?;
    }
    Ok(())
}

pub(crate) fn write_part(package: &mut crate::package::Package, path: &str, elements: &[crate::model::Element], binding: Option<&crate::native::NativePart>) -> Result<()> {
        if !crate::model::element_list(elements).iter().any(|element| has_fields(element)) { return Ok(()); }
        let xml = package.text(&path)?.to_owned();
        let parsed = crate::pptx::parse(&xml)?;
        let mut edits = Vec::new();
        for element in crate::model::element_list(elements).into_iter().filter(|element| has_fields(element)) {
            let format = match element { crate::model::Element::Text { format, .. } | crate::model::Element::Shape { format, .. } => format, _ => continue };
            let numeric = binding.and_then(|binding| binding.nodes.get(element.bounds().0));
            let shape = parsed.descendants().find(|node| crate::native::is_shape(*node) && crate::native::properties(*node).is_some_and(|props| if let Some(numeric) = numeric { props.attribute("id") == Some(numeric) } else { props.attribute("name") == Some(element.bounds().0) })).ok_or_else(|| Error::Conflict("field shape missing".into()))?;
            let body = crate::native::child(shape, crate::native::P, "txBody").ok_or_else(|| Error::Conflict("field text body missing".into()))?;
            if body.children().filter(|node| node.has_tag_name((crate::native::A, "p"))).count() != format.paragraphs.len() { return Err(Error::Conflict("field paragraph count mismatch".into())); }
            for (paragraph, node) in format.paragraphs.iter().zip(body.children().filter(|node| node.has_tag_name((crate::native::A, "p")))) {
                let natives: Vec<_> = node.children().filter(|node| node.has_tag_name((crate::native::A, "r")) || node.has_tag_name((crate::native::A, "fld"))).collect();
                if natives.len() != paragraph.runs.len() { return Err(Error::Conflict("field run count mismatch".into())); }
                for (run, native) in paragraph.runs.iter().zip(natives) {
                    if let Some(field) = &run.field {
                        if native.has_tag_name((crate::native::A, "fld")) {
                            if read_field(native)?.as_ref() != Some(field) { return Err(Error::Unsupported("native field identity/type cannot be overwritten".into())); }
                            write_cache(&xml, native, &run.text, &mut edits)?;
                            continue;
                        }
                        let content: String = native.children().filter(|node| node.is_element()).map(|node| &xml[node.range()]).collect();
                        edits.push((native.range(), format!("<a:fld xmlns:a=\"{}\" id=\"{}\" type=\"{}\">{content}</a:fld>", crate::native::A, quick_xml::escape::escape(&field.id), quick_xml::escape::escape(&field.kind))));
                    }
                    else if native.has_tag_name((crate::native::A, "fld")) { return Err(Error::Unsupported("native field cannot be silently flattened".into())); }
                }
            }
        }
        if !edits.is_empty() { package.replace_part(&path, crate::native_save::apply(xml, edits)?)?; }
    Ok(())
}

pub fn refresh_native(bytes: Vec<u8>, reference_date: &str) -> Result<Vec<u8>> {
    refresh_native_with_options(bytes, reference_date, None, "en-US")
}

pub(crate) fn refresh_native_with_options(bytes: Vec<u8>, reference_date: &str, reference_time: Option<&str>, locale: &str) -> Result<Vec<u8>> {
    parse_date(reference_date)?;
    reference_time.map(parse_time).transpose()?;
    let mut package = crate::package::Package::open(bytes)?;
    crate::review::ensure_unprotected(&package)?;
    crate::native::read(&package)?;
    for (index, path) in crate::pptx::slide_paths(&package)?.iter().enumerate() {
        let xml = package.text(path)?.to_owned(); let parsed = crate::pptx::parse(&xml)?; let mut edits = Vec::new();
        for node in parsed.descendants().filter(|node| node.has_tag_name((crate::native::A, "fld"))) {
            let field = read_field(node)?.ok_or_else(|| Error::Invalid("field missing".into()))?;
            let Some(value) = field.cached_text_with_options(index + 1, reference_date, reference_time, locale)? else { continue };
            write_cache(&xml, node, &value, &mut edits)?;
        }
        if !edits.is_empty() { package.replace_part(path, crate::native_save::apply(xml, edits)?)?; }
    }
    let bytes = package.save()?;
    crate::native::read(&crate::package::Package::open(bytes.clone())?)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_ids_and_reference_dates_are_validated() {
        let field = Field { id: "{00112233-4455-6677-8899-aabbccddeeff}".into(), kind: "slidenum".into() };
        assert_eq!(parse_id(&field.id).unwrap()[15], 255);
        assert_eq!(field.cached_text(7, "2024-02-29").unwrap(), Some("7".into()));
        assert!(field.cached_text(7, "2023-02-29").is_err());
        assert!(parse_id("00112233-4455-6677-8899-aabbccddeefx").is_err());
        let unknown = Field { kind: "vendor-field".into(), ..field };
        assert_eq!(unknown.cached_text(7, "2026-09-17").unwrap(), None);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignField {
    pub master_id: String,
    #[serde(default)] pub layout_id: Option<String>,
    pub kind: DesignFieldKind,
    pub reference_date: String,
    #[serde(default)] pub text: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignFieldKind { SlideNumber, Date, Footer }

fn instance_id(scope: &str, element: &str, ordinal: usize) -> String {
    use sha2::{Digest, Sha256};
    let bytes = Sha256::digest(format!("aislide-field\0{scope}\0{element}\0{ordinal}"));
    let hex = format!("{bytes:x}");
    format!("{}-{}-4{}-8{}-{}", &hex[..8], &hex[8..12], &hex[13..16], &hex[17..20], &hex[20..32])
}

fn instantiate(element: &crate::model::Element, scope: &str, number: usize) -> crate::model::Element {
    let mut result = element.clone();
    if let crate::model::Element::Text { id, text, format, .. } = &mut result {
        for (index, run) in format.paragraphs.iter_mut().flat_map(|paragraph| &mut paragraph.runs).enumerate() {
            if let Some(field) = &mut run.field {
                field.id = instance_id(scope, id, index);
                if field.kind == "slidenum" { run.text = number.to_string(); }
            }
        }
        if !format.paragraphs.is_empty() { *text = crate::rich_text::plain_text(&format.paragraphs); }
    }
    result
}

fn special(element: &crate::model::Element) -> Option<&crate::model::Placeholder> {
    use crate::model::{Element, PlaceholderKind};
    match element {
        Element::Text { format, .. } => format.placeholder.as_ref().filter(|placeholder| matches!(placeholder.kind, PlaceholderKind::SlideNumber | PlaceholderKind::Date | PlaceholderKind::Footer)),
        _ => None,
    }
}

pub(crate) fn materialize(deck: &mut crate::model::Deck) -> Result<()> {
    use crate::model::Element;
    let Some(design) = &mut deck.design else { return Ok(()); };
    for layout in &mut design.layouts {
        let master = design.masters.iter().find(|master| master.id == layout.master_id).ok_or_else(|| Error::Invalid("master missing".into()))?;
        for template in master.elements.iter().filter(|element| special(element).is_some()) {
            let placeholder = special(template).unwrap();
            if layout.elements.iter().any(|element| special(element).is_some_and(|other| other.kind == placeholder.kind)) { continue; }
            if layout.elements.iter().any(|element| element.bounds().0 == template.bounds().0 || matches!(element, Element::Text { format, .. } if format.placeholder.as_ref().is_some_and(|other| other.index == placeholder.index))) { return Err(Error::Conflict("master field conflicts with layout content".into())); }
            layout.elements.push(instantiate(template, &format!("layout:{}", layout.id), 1));
        }
    }
    for (index, slide) in deck.slides.iter_mut().enumerate() {
        let layout = slide.layout_id.as_ref().and_then(|id| design.layouts.iter().find(|layout| &layout.id == id)).or_else(|| design.layouts.first()).ok_or_else(|| Error::Invalid("layout missing".into()))?;
        for template in layout.elements.iter().filter(|element| special(element).is_some()) {
            let placeholder = special(template).unwrap();
            if slide.elements.iter().any(|element| special(element) == Some(placeholder)) { continue; }
            let mut added = instantiate(template, &format!("slide:{}", slide.id), index + 1);
            if let Element::Text { id, format, .. } = &mut added {
                if slide.elements.iter().any(|element| element.bounds().0 == id) {
                    *id = (1..=256).map(|number| format!("dynamic-field-{number}")).find(|id| !slide.elements.iter().any(|element| element.bounds().0 == id)).ok_or_else(|| Error::Limit("field element IDs exhausted".into()))?;
                }
                format.inherit_layout = true;
            }
            slide.elements.push(added);
        }
    }
    Ok(())
}

pub fn set_design_field(mut deck: crate::model::Deck, field: DesignField) -> Result<crate::model::Deck> {
    use crate::model::{Element, Placeholder, PlaceholderKind, TextFormat};
    parse_date(&field.reference_date)?; valid_text(&field.text, 4000)?;
    let mut design = deck.design.clone().unwrap_or_default();
    if !design.masters.iter().any(|master| master.id == field.master_id) { return Err(Error::Invalid("master not found".into())); }
    let (kind, label, native) = match field.kind { DesignFieldKind::SlideNumber => (PlaceholderKind::SlideNumber, "slide-number", Some("slidenum")), DesignFieldKind::Date => (PlaceholderKind::Date, "date", Some("datetime1")), DesignFieldKind::Footer => (PlaceholderKind::Footer, "footer", None) };
    let indices: std::collections::BTreeSet<_> = design.masters.iter().flat_map(|master| &master.elements).chain(design.layouts.iter().flat_map(|layout| &layout.elements)).filter_map(|element| match element { Element::Text { format, .. } => format.placeholder.as_ref().map(|placeholder| placeholder.index), _ => None }).collect();
    let target = if let Some(id) = &field.layout_id { &mut design.layouts.iter_mut().find(|layout| &layout.id == id && layout.master_id == field.master_id).ok_or_else(|| Error::Invalid("layout owner mismatch".into()))?.elements } else { &mut design.masters.iter_mut().find(|master| master.id == field.master_id).unwrap().elements };
    let position = target.iter().position(|element| special(element).is_some_and(|placeholder| placeholder.kind == kind));
    let index = position.and_then(|position| special(&target[position]).map(|placeholder| placeholder.index)).or_else(|| (0..=255).find(|index| !indices.contains(index))).ok_or_else(|| Error::Limit("placeholder indices exhausted".into()))?;
    let scope = field.layout_id.as_deref().unwrap_or(&field.master_id);
    let id = position.map(|position| target[position].bounds().0.to_owned()).unwrap_or_else(|| format!("dynamic-{label}-{index}"));
    let dynamic = native.map(|kind| Field { id: instance_id(scope, &id, 0), kind: kind.into() });
    let text = if let Some(dynamic) = &dynamic { dynamic.cached_text(1, &field.reference_date)?.unwrap_or_default() } else { field.text };
    let width = f64::from(deck.width); let height = f64::from(deck.height);
    let (left, size) = match kind { PlaceholderKind::Date => (0.05, 0.25), PlaceholderKind::SlideNumber => (0.85, 0.10), _ => (0.32, 0.5) };
    let mut element = position.map(|position| target[position].clone()).unwrap_or_else(|| Element::Text { visual: None, id, x: width * left, y: height - 48.0, width: width * size, height: 32.0, text: String::new(), font_size: 16.0, color: "@dk1".into(), bold: false, format: TextFormat { font_family: Some("@minor".into()), placeholder: Some(Placeholder { kind, index }), ..Default::default() } });
    if element.visual().is_some_and(|visual| visual.locked) { return Err(Error::Unsupported("locked field template".into())); }
    if position.is_some() {
        set_value(&mut element, &text, native)?;
    } else if let Element::Text { text: content, format, .. } = &mut element {
        *content = text.clone();
        format.paragraphs = vec![crate::rich_text::RichParagraph { runs: vec![crate::rich_text::RichRun { text: text.clone(), style: Default::default(), field: dynamic }], ..Default::default() }];
    }
    if let Some(position) = position { target[position] = element; } else { target.push(element); }
    let layouts: std::collections::BTreeSet<_> = design.layouts.iter().filter(|layout| layout.master_id == field.master_id && field.layout_id.as_ref().is_none_or(|id| &layout.id == id)).map(|layout| layout.id.clone()).collect();
    for layout in design.layouts.iter_mut().filter(|layout| layouts.contains(&layout.id)) {
        for element in layout.elements.iter_mut().filter(|element| special(element).is_some_and(|placeholder| placeholder.kind == kind && placeholder.index == index)) {
            set_value(element, &text, native)?;
        }
    }
    let default_layout = design.layouts.first().map(|layout| layout.id.as_str());
    for slide in deck.slides.iter_mut().filter(|slide| slide.layout_id.as_deref().or(default_layout).is_some_and(|id| layouts.contains(id))) {
        for element in slide.elements.iter_mut().filter(|element| special(element).is_some_and(|placeholder| placeholder.kind == kind && placeholder.index == index)) {
            set_value(element, &text, native)?;
        }
    }
    deck = crate::design::update_design(deck, design)?;
    refresh(&deck, &field.reference_date)
}

fn set_value(element: &mut crate::model::Element, value: &str, kind: Option<&str>) -> Result<()> {
    if element.visual().is_some_and(|visual| visual.locked) { return Err(Error::Unsupported("locked field instance".into())); }
    if kind.is_none() {
        if has_fields(element) { return Err(Error::Unsupported("footer contains dynamic fields".into())); }
        *element = crate::rich_text::replace_text_content(element.clone(), value.into())?;
        return Ok(());
    }
    if let crate::model::Element::Text { text, format, .. } = element {
        if format.paragraphs.len() != 1 || format.paragraphs[0].runs.len() != 1 || format.paragraphs[0].runs[0].field.as_ref().map(|field| field.kind.as_str()) != kind { return Err(Error::Unsupported("field template requires one matching dynamic run".into())); }
        format.paragraphs[0].runs[0].text = value.into();
        *text = value.into();
    }
    Ok(())
}

pub(crate) fn display_element(element: &crate::model::Element, slide_number: usize) -> std::borrow::Cow<'_, crate::model::Element> {
    use crate::model::Element;
    if !crate::model::element_list(std::slice::from_ref(element)).iter().any(|element| has_fields(element)) { return std::borrow::Cow::Borrowed(element); }
    let mut result = element.clone();
    match &mut result {
        Element::Group { children, .. } => { for child in children { *child = display_element(child, slide_number).into_owned(); } }
        Element::Text { text, format, .. } | Element::Shape { text, format, .. } => {
            for run in format.paragraphs.iter_mut().flat_map(|paragraph| &mut paragraph.runs) {
                if run.field.as_ref().is_some_and(|field| field.kind == "slidenum") { run.text = slide_number.to_string(); }
            }
            *text = crate::rich_text::plain_text(&format.paragraphs);
        }
        _ => {}
    }
    std::borrow::Cow::Owned(result)
}

pub(crate) fn propagate_defaults(previous: Option<&crate::design::Design>, deck: &mut crate::model::Deck) -> Result<()> {
    use crate::model::Element;
    let Some(previous) = previous else { return Ok(()); };
    let Some(design) = &mut deck.design else { return Ok(()); };
    fn propagate(before: &[Element], after: &[Element], targets: &mut [Element]) -> Result<()> {
        for old in before.iter().filter(|element| special(element).is_some()) {
            let Some(new) = after.iter().find(|element| special(element) == special(old)) else { continue; };
            let (Element::Text { text: old_text, .. }, Element::Text { text: new_text, format, .. }) = (old, new) else { continue; };
            if old_text == new_text { continue; }
            let kind = format.paragraphs.iter().flat_map(|paragraph| &paragraph.runs).find_map(|run| run.field.as_ref()).map(|field| field.kind.as_str());
            if kind.is_some_and(|kind| !matches!(kind, "slidenum" | "datetime1")) { continue; }
            for target in targets.iter_mut().filter(|element| special(element) == special(old) && matches!(element, Element::Text { text, .. } if text == old_text)) { set_value(target, new_text, kind)?; }
        }
        Ok(())
    }
    for layout in &mut design.layouts {
        if let (Some(old), Some(new)) = (previous.masters.iter().find(|master| master.id == layout.master_id), design.masters.iter().find(|master| master.id == layout.master_id)) {
            propagate(&old.elements, &new.elements, &mut layout.elements)?;
        }
    }
    for slide in &mut deck.slides {
        let id = slide.layout_id.as_ref().or_else(|| design.layouts.first().map(|layout| &layout.id));
        if let (Some(old), Some(new)) = (previous.layouts.iter().find(|layout| Some(&layout.id) == id), design.layouts.iter().find(|layout| Some(&layout.id) == id)) {
            propagate(&old.elements, &new.elements, &mut slide.elements)?;
        }
    }
    Ok(())
}

pub fn capabilities() -> serde_json::Value {
    serde_json::json!({
        "master_theme_override":true,"default_theme_preserves_explicit_masters":true,"native_theme_copy_on_write":true,
        "field_kinds":["slidenum","datetime1","datetime2","datetime3","datetime4","datetime5","datetime6","datetime7","datetime8","datetime9","datetime10","datetime11","datetime12","datetime13"],"placeholder_kinds":["slide_number","date","footer"],
        "scopes":["master","layout"],"reference_date":"caller supplied YYYY-MM-DD","page_number_projection":true,
        "native_fields":true,"native_field_cache_refresh":true,"unknown_fields":"preserved without evaluation",
        "notes_master":true,"handout_master":true,"rich_notes":true,"table_fields":false,"office_recalculation_verified":false,
        "studio":{"master_theme":true,"master_layout_fields":true,"review_field_scope":true},
        "limitations":["Only en-US date formats are evaluated. datetime8..13 require explicit HH:mm:ss; omitted time and other locales retain caches.","Dates use explicit refreshed caches; no clock or locale inference.","Unknown fields and changed unmodeled native paragraphs fail closed; unchanged field metadata is preserved.","Native auxiliary master removal and notes-page resizing are unsupported. Handout imposition is not implemented.","Shared master page caches remain untouched; per-page preview is a read-only projection.","Office recalculation and visual parity have not been verified."]
    })
}

pub(crate) fn evaluation_warnings(deck: &crate::model::Deck, reference_time: Option<&str>, locale: &str) -> Vec<String> {
    let mut kinds = std::collections::BTreeSet::new();
    for slide in &deck.slides {
        for element in crate::model::element_list(&slide.elements) {
            if let crate::model::Element::Text { format, .. } | crate::model::Element::Shape { format, .. } = element {
                kinds.extend(format.paragraphs.iter().flat_map(|paragraph| &paragraph.runs).filter_map(|run| run.field.as_ref().map(|field| field.kind.clone())));
            }
        }
        kinds.extend(slide.notes_paragraphs.iter().flat_map(|paragraph| &paragraph.runs).filter_map(|run| run.field.as_ref().map(|field| field.kind.clone())));
    }
    kinds.into_iter().filter_map(|kind| {
        let reason = if !supported_kind(&kind) { "unknown field type" } else if kind != "slidenum" && locale != "en-US" { "unsupported date locale" }
            else if reference_time.is_none() && matches!(kind.as_str(), "datetime8" | "datetime9" | "datetime10" | "datetime11" | "datetime12" | "datetime13") { "explicit reference time required" } else { return None; };
        Some(format!("{kind}: cached text retained ({reason})"))
    }).collect()
}

pub(crate) fn same_inherited_format(template: &crate::model::TextFormat, instance: &crate::model::TextFormat) -> bool {
    let mut normalized = instance.clone();
    if template.paragraphs.len() != normalized.paragraphs.len() { return false; }
    for (expected, actual) in template.paragraphs.iter().zip(&mut normalized.paragraphs) {
        if expected.runs.len() != actual.runs.len() { return false; }
        for (expected, actual) in expected.runs.iter().zip(&mut actual.runs) {
            if let (Some(before), Some(after)) = (&expected.field, &mut actual.field) {
                if before.kind == after.kind && supported_kind(&before.kind) {
                    after.id = before.id.clone();
                    actual.text = expected.text.clone();
                }
            }
        }
    }
    template == &normalized
}