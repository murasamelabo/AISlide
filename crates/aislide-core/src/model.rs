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
}

impl Element {
    pub fn bounds(&self) -> (&str, f64, f64, f64, f64) {
        match self {
            Self::Text { id, x, y, width, height, .. }
            | Self::Rect { id, x, y, width, height, .. }
            | Self::Table { id, x, y, width, height, .. } => (id, *x, *y, *width, *height),
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
    for slide in &deck.slides {
        valid_text(&slide.id, 80)?;
        valid_text(&slide.title, 120)?;
        valid_text(&slide.notes, 8000)?;
        valid_color(&slide.background)?;
        total += slide.elements.len();
        if slide.id.is_empty() || !slide_ids.insert(&slide.id) || slide.elements.len() > 256 || total > 2048 {
            return Err(Error::Invalid("duplicate/empty slide ID or too many elements".into()));
        }
        let mut ids = BTreeSet::new();
        for element in &slide.elements {
            let (id, x, y, width, height) = element.bounds();
            valid_text(id, 80)?;
            if id.is_empty() || !ids.insert(id) || [x, y, width, height].iter().any(|value| !value.is_finite())
                || x < 0.0 || y < 0.0 || width <= 0.0 || height <= 0.0 || x + width > 1280.01 || y + height > 720.01 {
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
            }
        }
    }
    Ok(())
}

fn font_size_check(size: f64) -> Result<()> {
    if !size.is_finite() || !(8.0..=120.0).contains(&size) {
        return Err(Error::Invalid("font size must be 8-120 pixels".into()));
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