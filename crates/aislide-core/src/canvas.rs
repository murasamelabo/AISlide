use crate::{model::{Deck, Element, validate_deck}, native::{P, child}, package::Package, pptx::parse, Error, Result};
use serde::{Deserialize, Serialize};

const NS: &str = "urn:aislide:canvas:1";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResizeMode { Scale, Keep }

pub fn validate_size(width: u32, height: u32) -> Result<()> {
    if !(320..=4096).contains(&width) || !(320..=4096).contains(&height) {
        return Err(Error::Limit("page dimensions must each be 320..4096".into()));
    }
    Ok(())
}

pub fn resize(mut deck: Deck, width: u32, height: u32, mode: ResizeMode) -> Result<Deck> {
    validate_deck(&deck)?;
    validate_size(width, height)?;
    if matches!(mode, ResizeMode::Scale) {
        let factors = (f64::from(width) / f64::from(deck.width), f64::from(height) / f64::from(deck.height));
        for slide in &mut deck.slides { scale_elements(&mut slide.elements, factors); }
        if let Some(design) = &mut deck.design {
            for master in &mut design.masters { scale_elements(&mut master.elements, factors); }
            for layout in &mut design.layouts { scale_elements(&mut layout.elements, factors); }
        }
    }
    deck.width = width; deck.height = height;
    validate_deck(&deck)?;
    Ok(deck)
}

fn scale_elements(elements: &mut [Element], factors: (f64, f64)) {
    scale_elements_with_styles(elements, factors, factors.0.min(factors.1));
}

fn scale_elements_with_styles(elements: &mut [Element], factors: (f64, f64), uniform: f64) {
    for element in elements {
        let (x, y, width, height) = match element {
            Element::Text { x, y, width, height, .. } | Element::Rect { x, y, width, height, .. }
            | Element::Polygon { x, y, width, height, .. } | Element::Shape { x, y, width, height, .. }
            | Element::Table { x, y, width, height, .. } | Element::Chart { x, y, width, height, .. }
            | Element::Picture { x, y, width, height, .. } | Element::Connector { x, y, width, height, .. }
            | Element::Group { x, y, width, height, .. } => (x, y, width, height),
        };
        *x *= factors.0; *y *= factors.1; *width *= factors.0; *height *= factors.1;
        match element {
            Element::Text { font_size, format, .. } | Element::Shape { font_size, format, .. } => {
                *font_size *= uniform;
                for paragraph in &mut format.paragraphs {
                    for run in &mut paragraph.runs { if let Some(size) = &mut run.style.font_size { *size *= uniform; } }
                    for value in [&mut paragraph.margin_left, &mut paragraph.indent].into_iter().flatten() { *value = (f64::from(*value) * uniform).round() as i32; }
                    for tab in &mut paragraph.tabs { tab.position = (f64::from(tab.position) * uniform).round() as i32; }
                    for spacing in [&mut paragraph.line_spacing, &mut paragraph.space_before, &mut paragraph.space_after].into_iter().flatten() {
                        if let crate::rich_text::Spacing::Points(value) = spacing { *value = (f64::from(*value) * uniform).round() as u32; }
                    }
                }
            }
            Element::Table { font_size, .. } => *font_size *= uniform,
            Element::Group { children, .. } => {
                scale_elements_with_styles(children, (1.0, 1.0), uniform);
            }
            _ => {}
        }
        match element {
            Element::Shape { stroke_width, .. } | Element::Polygon { stroke_width, .. } | Element::Connector { stroke_width, .. } => *stroke_width *= uniform,
            _ => {}
        }
    }
}

pub(crate) fn physical_size(package: &Package, main: &str) -> Result<(u64, u64)> {
    let xml = parse(package.text(main)?)?;
    let size = child(xml.root_element(), P, "sldSz").ok_or_else(|| Error::Invalid("slide size missing".into()))?;
    let dimension = |name| size.attribute(name).and_then(|value| value.parse::<u64>().ok()).filter(|value| *value > 0 && *value <= 51206400).ok_or_else(|| Error::Limit("native slide dimension outside supported positive EMU range".into()));
    Ok((dimension("cx")?, dimension("cy")?))
}

pub(crate) fn native_size(package: &Package, main: &str) -> Result<(u32, u32)> {
    let (physical_width, physical_height) = physical_size(package, main)?;
    let xml = parse(package.text(main)?)?;
    let size = child(xml.root_element(), P, "sldSz").ok_or_else(|| Error::Invalid("slide size missing".into()))?;
    let markers: Vec<_> = child(xml.root_element(), P, "extLst").into_iter().flat_map(|list| list.children())
        .filter(|node| node.has_tag_name((P, "ext")) && node.attribute("uri") == Some(NS))
        .flat_map(|node| node.children()).filter(|node| node.has_tag_name((NS, "canvas"))).collect();
    if markers.len() > 1 { return Err(Error::Invalid("ambiguous native page metadata".into())); }
    if let Some(marker) = markers.first() {
        let value = |name| marker.attribute(name).and_then(|value| value.parse::<u64>().ok());
        if value("cx") == Some(physical_width) && value("cy") == Some(physical_height) {
            let width = value("width").and_then(|value| u32::try_from(value).ok()).ok_or_else(|| Error::Invalid("page width metadata".into()))?;
            let height = value("height").and_then(|value| u32::try_from(value).ok()).ok_or_else(|| Error::Invalid("page height metadata".into()))?;
            validate_size(width, height)?;
            return Ok((width, height));
        }
    }
    let width = physical_width as f64 / 9525.0; let height = physical_height as f64 / 9525.0;
    if size.attribute("type") == Some("custom") && width.fract() == 0.0 && height.fract() == 0.0 && validate_size(width as u32, height as u32).is_ok() {
        return Ok((width as u32, height as u32));
    }
    let factor = (1280.0 / width.max(height)).max(320.0 / width.min(height));
    let dimensions = ((width * factor).round() as u32, (height * factor).round() as u32);
    validate_size(dimensions.0, dimensions.1)?;
    Ok(dimensions)
}

pub(crate) fn write_page(package: &mut Package, main: &str, width: u32, height: u32, scale: (f64, f64)) -> Result<()> {
    use crate::native_save::{apply, insert_child, set_attribute};
    let physical_width = (f64::from(width) * scale.0).round() as u64;
    let physical_height = (f64::from(height) * scale.1).round() as u64;
    if physical_width == 0 || physical_height == 0 || physical_width > 51206400 || physical_height > 51206400 { return Err(Error::Limit("resized native page exceeds the physical EMU limit".into())); }
    let xml = package.text(main)?.to_owned(); let parsed = parse(&xml)?;
    let size = child(parsed.root_element(), P, "sldSz").ok_or_else(|| Error::Invalid("native slide size missing".into()))?;
    let mut edits = Vec::new();
    for (name, value) in [("cx", physical_width.to_string()), ("cy", physical_height.to_string()), ("type", "custom".into())] { set_attribute(&xml, size, name, &value, &mut edits)?; }
    let marker = format!("<p:ext xmlns:p=\"{P}\" uri=\"{NS}\"><c:canvas xmlns:c=\"{NS}\" width=\"{width}\" height=\"{height}\" cx=\"{physical_width}\" cy=\"{physical_height}\"/></p:ext>");
    if let Some(list) = child(parsed.root_element(), P, "extLst") {
        if let Some(old) = list.children().find(|node| node.has_tag_name((P, "ext")) && node.attribute("uri") == Some(NS)) { edits.push((old.range(), marker)); }
        else { insert_child(&xml, list, &marker, None, &mut edits)?; }
    } else { insert_child(&xml, parsed.root_element(), &format!("<p:extLst xmlns:p=\"{P}\">{marker}</p:extLst>"), None, &mut edits)?; }
    package.replace_part(main, apply(xml, edits)?)
}