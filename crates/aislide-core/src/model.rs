use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deck {
    pub version: u32,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub slides: Vec<Slide>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slide {
    pub id: String,
    pub title: String,
    pub background: String,
    pub elements: Vec<Element>,
    pub notes: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Element {
    Text { id: String, x: f64, y: f64, width: f64, height: f64, text: String, font_size: f64, color: String, bold: bool },
    Rect { id: String, x: f64, y: f64, width: f64, height: f64, fill: String },
    Table { id: String, x: f64, y: f64, width: f64, height: f64, rows: Vec<Vec<String>>, font_size: f64 },
    Chart { id: String, x: f64, y: f64, width: f64, height: f64, kind: ChartKind, categories: Vec<String>, series: Vec<ChartSeries> },
    Picture { id: String, x: f64, y: f64, width: f64, height: f64, base64: String, mime_type: String, alt: String, #[serde(default)] crop: Crop },
    Connector { id: String, x: f64, y: f64, width: f64, height: f64, color: String, stroke_width: f64, arrow: bool, #[serde(default)] flip_v: bool, #[serde(default)] start: Option<Connection>, #[serde(default)] end: Option<Connection> },
    Group { id: String, x: f64, y: f64, width: f64, height: f64, view_width: f64, view_height: f64, children: Vec<Element> },
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Crop { pub left: f64, pub top: f64, pub right: f64, pub bottom: f64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection { pub element_id: String, pub site: u32 }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind { Column, Bar, Line }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChartSeries { pub name: String, pub values: Vec<f64>, pub color: String }

impl Element {
    pub fn bounds(&self) -> (&str, f64, f64, f64, f64) {
        match self {
            Self::Text { id, x, y, width, height, .. }
            | Self::Rect { id, x, y, width, height, .. }
            | Self::Table { id, x, y, width, height, .. }
            | Self::Chart { id, x, y, width, height, .. }
            | Self::Picture { id, x, y, width, height, .. }
            | Self::Connector { id, x, y, width, height, .. }
            | Self::Group { id, x, y, width, height, .. } => (id, *x, *y, *width, *height),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Issue {
    pub severity: String,
    pub code: String,
    pub message: String,
}

pub fn valid_text(value: &str, limit: usize) -> Result<()> {
    if value.chars().count() > limit {
        return Err(Error::Limit(format!("text exceeds {limit} characters")));
    }
    if value.chars().any(|character| !matches!(character, '\u{9}' | '\u{a}' | '\u{d}' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')) {
        return Err(Error::Invalid("character not permitted in XML 1.0".into()));
    }
    Ok(())
}

pub fn valid_color(value: &str) -> Result<()> {
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::Invalid("color must be six hexadecimal digits".into()));
    }
    Ok(())
}

pub fn validate_deck(deck: &Deck) -> Result<()> {
    if deck.version != 1 || deck.width != 1280 || deck.height != 720 {
        return Err(Error::Unsupported("expected scene version 1, 1280 x 720".into()));
    }
    valid_text(&deck.title, 120)?;
    if deck.slides.is_empty() || deck.slides.len() > 32 {
        return Err(Error::Limit("expected 1-32 slides".into()));
    }
    let mut slide_ids = BTreeSet::new();
    let mut total = 0;
    let mut image_bytes = 0;
    for slide in &deck.slides {
        valid_text(&slide.id, 80)?;
        valid_text(&slide.title, 120)?;
        valid_text(&slide.notes, 8000)?;
        valid_color(&slide.background)?;
        if slide.id.is_empty() || !slide_ids.insert(&slide.id) {
            return Err(Error::Invalid("duplicate/empty slide ID or too many elements".into()));
        }
        let mut ids = BTreeSet::new();
        validate_elements(&slide.elements, (1280.0, 720.0), 0, &mut ids, &mut total, &mut image_bytes)?;
    }
    Ok(())
}

fn validate_elements(elements: &[Element], canvas: (f64, f64), depth: usize, ids: &mut BTreeSet<String>, total: &mut usize, image_bytes: &mut usize) -> Result<()> {
    if depth > 8 { return Err(Error::Limit("group nesting > 8".into())); }
    *total += elements.len();
    if elements.len() > 256 || *total > 2048 { return Err(Error::Limit("too many scene elements".into())); }
    for element in elements {
            let (id, x, y, width, height) = element.bounds();
            valid_text(id, 80)?;
            if id.is_empty() || !ids.insert(id.into()) || ids.len() > 256 || [x, y, width, height].iter().any(|value| !value.is_finite())
                || x < 0.0 || y < 0.0 || width <= 0.0 || height <= 0.0 || x + width > canvas.0 + 0.01 || y + height > canvas.1 + 0.01 {
                return Err(Error::Invalid(format!("invalid element geometry or ID: {id}")));
            }
            match element {
                Element::Text { text, font_size, color, .. } => {
                    valid_text(text, 4000)?;
                    valid_color(color)?;
                    font_size_check(*font_size)?;
                }
                Element::Rect { fill, .. } => valid_color(fill)?,
                Element::Table { rows, font_size, .. } => {
                    font_size_check(*font_size)?;
                    validate_rows(rows)?;
                }
                Element::Chart { categories, series, .. } => validate_chart(categories, series)?,
                Element::Picture { base64, mime_type, alt, crop, .. } => {
                    valid_text(alt, 500)?;
                    *image_bytes += base64.len();
                    if *image_bytes > 3 * 1024 * 1024 { return Err(Error::Limit("scene image payload > 3 MiB encoded".into())); }
                    if [crop.left, crop.right, crop.top, crop.bottom].iter().any(|value| !value.is_finite() || !(0.0..1.0).contains(value)) || crop.left + crop.right >= 1.0 || crop.top + crop.bottom >= 1.0 {
                        return Err(Error::Invalid("image crop must retain positive area".into()));
                    }
                    crate::media::inspect_raster(base64, mime_type)?;
                }
                Element::Connector { color, stroke_width, start, end, .. } => {
                    valid_color(color)?;
                    if !stroke_width.is_finite() || !(0.5..=20.0).contains(stroke_width) { return Err(Error::Invalid("connector stroke must be 0.5-20 pixels".into())); }
                    for connection in [start, end].into_iter().flatten() {
                        if connection.site > 3 || connection.element_id == id || !elements.iter().any(|candidate| candidate.bounds().0 == connection.element_id && matches!(candidate, Element::Rect { .. } | Element::Text { .. } | Element::Picture { .. })) {
                            return Err(Error::Invalid("connector target must be a sibling text, rectangle or picture with an anchor 0-3".into()));
                        }
                    }
                }
                Element::Group { view_width, view_height, children, .. } => {
                    if children.is_empty() || [*view_width, *view_height].iter().any(|value| !value.is_finite() || !(1.0..=4096.0).contains(value)) { return Err(Error::Invalid("group requires children and a 1-4096px coordinate space".into())); }
                    validate_elements(children, (*view_width, *view_height), depth + 1, ids, total, image_bytes)?;
                }
            }
    }
    Ok(())
}

pub(crate) fn element_list(elements: &[Element]) -> Vec<&Element> {
    let mut result = Vec::new();
    for element in elements {
        result.push(element);
        if let Element::Group { children, .. } = element { result.extend(element_list(children)); }
    }
    result
}

fn font_size_check(size: f64) -> Result<()> {
    if !size.is_finite() || !(8.0..=120.0).contains(&size) {
        return Err(Error::Invalid("font size must be 8-120 pixels".into()));
    }
    Ok(())
}

pub fn validate_chart(categories: &[String], series: &[ChartSeries]) -> Result<()> {
    if categories.is_empty() || categories.len() > 32 || series.is_empty() || series.len() > 6 {
        return Err(Error::Invalid("chart requires 1-32 categories and 1-6 series".into()));
    }
    for category in categories { valid_text(category, 80)?; }
    for entry in series {
        valid_text(&entry.name, 80)?;
        valid_color(&entry.color)?;
        if entry.values.len() != categories.len() || entry.values.iter().any(|value| !value.is_finite() || value.abs() > 1e15) {
            return Err(Error::Invalid("chart series must match categories with finite values in +/-1e15".into()));
        }
    }
    Ok(())
}

pub fn validate_rows(rows: &[Vec<String>]) -> Result<()> {
    let columns = rows.first().map_or(0, Vec::len);
    if rows.is_empty() || rows.len() > 12 || columns == 0 || columns > 8 || rows.iter().any(|row| row.len() != columns) {
        return Err(Error::Invalid("table must be rectangular, 1-12 rows, 1-8 columns".into()));
    }
    for cell in rows.iter().flatten() { valid_text(cell, 200)?; }
    Ok(())
}