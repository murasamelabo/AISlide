//! Literal text operations on scene content, not metadata, chart data, or picture descriptions.
//! Paths are relative to Deck; offsets count Unicode scalars and end is exclusive.
//! Case-insensitive matching uses per-scalar Unicode lowercase, without normalization or
//! locale-specific/full case folding. Whole words have no adjacent alphanumeric or underscore.
//! Matches are nonoverlapping. Exceeding the match budget returns Limit, never partial success.
//! Rich text is synchronized one match at a time; dynamic field caches are searchable but protected.

use crate::{model::{Deck, Element, TextFormat, valid_text, validate_deck}, Error, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_MATCHES: usize = 1024;
const MAX_TEXT: usize = 8000;
const SNIPPET_SCALARS: usize = 160;

fn default_case_sensitive() -> bool { true }
fn default_max_matches() -> usize { 100 }

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchOptions {
    pub query: String,
    #[serde(default = "default_case_sensitive")]
    pub case_sensitive: bool,
    #[serde(default)]
    pub whole_word: bool,
    #[serde(default)]
    pub include_notes: bool,
    #[serde(default)]
    pub include_masters: bool,
    #[serde(default = "default_max_matches")]
    pub max_matches: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextMatch {
    pub path: String,
    pub start: usize,
    pub end: usize,
    pub expected: String,
    pub snippet: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplaceOptions {
    pub search: SearchOptions,
    pub replacement: String,
    #[serde(default)]
    pub replace_all: bool,
    #[serde(default)]
    pub selected: Option<Vec<TextMatch>>,
}

struct TextField<'a> {
    path: String,
    value: &'a str,
    limit: usize,
}

fn check_options(options: &SearchOptions) -> Result<()> {
    valid_text(&options.query, MAX_TEXT)?;
    if options.query.is_empty() { return Err(Error::Invalid("literal query must not be empty".into())); }
    if !(1..=MAX_MATCHES).contains(&options.max_matches) {
        return Err(Error::Limit(format!("max_matches must be 1-{MAX_MATCHES}")));
    }
    Ok(())
}

fn element_fields<'a>(elements: &'a [Element], path: &str, fields: &mut Vec<TextField<'a>>) -> Result<()> {
    for (index, element) in elements.iter().enumerate() {
        let path = format!("{path}/{index}");
        match element {
            Element::Text { text, .. } | Element::Shape { text, .. } => {
                fields.push(TextField { path: format!("{path}/text"), value: text, limit: 4000 });
            }
            Element::Table { rows, .. } => {
                for (row_index, row) in rows.iter().enumerate() {
                    for (column, value) in row.iter().enumerate() {
                        fields.push(TextField { path: format!("{path}/rows/{row_index}/{column}"), value, limit: 200 });
                    }
                }
            }
            Element::Group { children, .. } => element_fields(children, &format!("{path}/children"), fields)?,
            Element::Rect { .. } | Element::Polygon { .. } | Element::Chart { .. }
            | Element::Picture { .. } | Element::Connector { .. } => {}
        }
    }
    Ok(())
}

fn text_fields<'a>(deck: &'a Deck, options: &SearchOptions) -> Result<Vec<TextField<'a>>> {
    let mut fields = Vec::new();
    for (index, slide) in deck.slides.iter().enumerate() {
        element_fields(&slide.elements, &format!("/slides/{index}/elements"), &mut fields)?;
        if options.include_notes {
            fields.push(TextField { path: format!("/slides/{index}/notes"), value: &slide.notes, limit: MAX_TEXT });
        }
    }
    if let Some(design) = deck.design.as_ref().filter(|_| options.include_masters) {
        for (index, master) in design.masters.iter().enumerate() {
            element_fields(&master.elements, &format!("/design/masters/{index}/elements"), &mut fields)?;
        }
        for (index, layout) in design.layouts.iter().enumerate() {
            element_fields(&layout.elements, &format!("/design/layouts/{index}/elements"), &mut fields)?;
        }
    }
    Ok(fields)
}

fn lowercase(value: &str) -> String { value.chars().flat_map(char::to_lowercase).collect() }
fn word_character(character: char) -> bool { character.is_alphanumeric() || character == '_' }

pub fn search(deck: &Deck, options: &SearchOptions) -> Result<Vec<TextMatch>> {
    check_options(options)?;
    validate_deck(deck)?;
    let needle = if options.case_sensitive { options.query.clone() } else { lowercase(&options.query) };
    let mut matches = Vec::new();
    for field in text_fields(deck, options)? {
        let characters: Vec<_> = field.value.chars().collect();
        let mut haystack = String::new();
        let mut boundaries = Vec::with_capacity(characters.len() + 1);
        for character in &characters {
            boundaries.push(haystack.len());
            if options.case_sensitive { haystack.push(*character); }
            else { haystack.extend(character.to_lowercase()); }
        }
        boundaries.push(haystack.len());
        let mut cursor = 0;
        while let Some(relative) = haystack[cursor..].find(&needle) {
            let byte_start = cursor + relative;
            let byte_end = byte_start + needle.len();
            let offsets = boundaries.binary_search(&byte_start).ok().zip(boundaries.binary_search(&byte_end).ok());
            if let Some((start, end)) = offsets {
                let whole_word = (start == 0 || !word_character(characters[start - 1]))
                    && (end == characters.len() || !word_character(characters[end]));
                if !options.whole_word || whole_word {
                    if matches.len() == options.max_matches {
                        return Err(Error::Limit("text match budget exceeded; narrow the query or raise max_matches".into()));
                    }
                    let snippet_start = start.saturating_sub(32);
                    let snippet_end = characters.len().min(snippet_start + SNIPPET_SCALARS);
                    matches.push(TextMatch { path: field.path.clone(), start, end, expected: field.value.into(),
                        snippet: characters[snippet_start..snippet_end].iter().collect() });
                    cursor = byte_end;
                    continue;
                }
            }
            cursor = byte_start + haystack[byte_start..].chars().next().map_or(1, char::len_utf8);
        }
    }
    Ok(matches)
}

pub fn replace(mut deck: Deck, options: &ReplaceOptions) -> Result<Deck> {
    valid_text(&options.replacement, MAX_TEXT)?;
    if options.replace_all == options.selected.is_some() {
        return Err(Error::Invalid("choose replace_all or selected, exclusively".into()));
    }
    if options.selected.as_ref().is_some_and(|selected| selected.len() > MAX_MATCHES) {
        return Err(Error::Limit("too many selected text matches".into()));
    }
    let available = search(&deck, &options.search)?;
    let chosen = if let Some(selected) = &options.selected {
        let indexed: BTreeMap<_, _> = available.iter().map(|found| ((found.path.as_str(), found.start, found.end), found)).collect();
        let mut seen = BTreeSet::new();
        let mut chosen = Vec::with_capacity(selected.len());
        for requested in selected {
            let key = (requested.path.as_str(), requested.start, requested.end);
            let actual = indexed.get(&key).filter(|found| found.expected == requested.expected)
                .ok_or_else(|| Error::Conflict("selected text path, range, or expected value changed".into()))?;
            if !seen.insert(key) { return Err(Error::Conflict("duplicate selected text match".into())); }
            chosen.push((*actual).clone());
        }
        chosen
    } else { available };
    let mut grouped: BTreeMap<&str, Vec<&TextMatch>> = BTreeMap::new();
    for found in &chosen { grouped.entry(&found.path).or_default().push(found); }
    let replacement_length = options.replacement.chars().count();
    for field in text_fields(&deck, &options.search)? {
        let Some(found) = grouped.get_mut(field.path.as_str()) else { continue; };
        found.sort_by_key(|found| found.start);
        let old_length = field.value.chars().count();
        let removed: usize = found.iter().map(|found| found.end - found.start).sum();
        let next_length = replacement_length.checked_mul(found.len())
            .and_then(|inserted| (old_length - removed).checked_add(inserted))
            .ok_or_else(|| Error::Limit("replacement length overflow".into()))?;
        if next_length > field.limit { return Err(Error::Limit(format!("{} exceeds {} characters", field.path, field.limit))); }
    }
    for (path, found) in grouped {
        for found in found.into_iter().rev() {
            text_destination(&mut deck, path).ok_or_else(|| Error::Conflict("text destination changed".into()))?
                .replace(found.start, found.end, &options.replacement)?;
        }
    }
    validate_deck(&deck)?;
    Ok(deck)
}

enum TextDestination<'a> {
    Body(&'a mut Element),
    Cell(&'a mut String, Option<&'a mut TextFormat>),
    Plain(&'a mut String),
}

fn replace_range(text: &str, start: usize, end: usize, replacement: &str) -> String {
    text.chars().take(start).chain(replacement.chars()).chain(text.chars().skip(end)).collect()
}

fn check_field_range(format: &TextFormat, start: usize, end: usize) -> Result<()> {
    let mut offset = 0;
    for paragraph in &format.paragraphs {
        for run in &paragraph.runs {
            let limit = offset + run.text.chars().count();
            if run.field.is_some() && ((start < limit && end > offset) || (offset == limit && start <= offset && end >= offset)) {
                return Err(Error::Unsupported("selected text range overlaps a dynamic field cache".into()));
            }
            offset = limit;
        }
        offset += 1;
    }
    Ok(())
}

impl TextDestination<'_> {
    fn replace(self, start: usize, end: usize, replacement: &str) -> Result<()> {
        match self {
            Self::Body(element) => {
                let (text, format) = match &*element {
                    Element::Text { text, format, .. } | Element::Shape { text, format, .. } => (text, format),
                    _ => return Err(Error::Unsupported("text destination is not a body".into())),
                };
                check_field_range(format, start, end)?;
                *element = crate::rich_text::replace_text_content(element.clone(), replace_range(text, start, end, replacement))?;
            }
            Self::Cell(text, Some(format)) if !format.paragraphs.is_empty() => {
                check_field_range(format, start, end)?;
                let next = replace_range(text, start, end, replacement);
                let proxy = Element::Text { id: "cell".into(), x: 0.0, y: 0.0, width: 1.0, height: 1.0,
                    text: text.clone(), font_size: 24.0, color: "000000".into(), bold: false, format: format.clone(), visual: None };
                let Element::Text { text: next, format: updated, .. } = crate::rich_text::replace_text_content(proxy, next)? else { unreachable!() };
                *text = next;
                *format = updated;
            }
            Self::Cell(text, _) | Self::Plain(text) => *text = replace_range(text, start, end, replacement),
        }
        Ok(())
    }
}

fn element_destination<'a>(elements: &'a mut [Element], segments: &[&str]) -> Option<TextDestination<'a>> {
    let (index, rest) = segments.split_first()?;
    let element = elements.get_mut(index.parse::<usize>().ok()?)?;
    match (element, rest) {
        (element @ (Element::Text { .. } | Element::Shape { .. }), ["text"]) => Some(TextDestination::Body(element)),
        (Element::Table { rows, format, .. }, ["rows", row, column]) => {
            let row = row.parse::<usize>().ok()?;
            let column = column.parse::<usize>().ok()?;
            let text = rows.get_mut(row)?.get_mut(column)?;
            let format = format.cells.iter_mut().find(|cell| cell.row == row && cell.column == column)
                .and_then(|cell| cell.style.text_format.as_mut());
            Some(TextDestination::Cell(text, format))
        }
        (Element::Group { children, .. }, ["children", rest @ ..]) => element_destination(children, rest),
        _ => None,
    }
}

fn text_destination<'a>(deck: &'a mut Deck, path: &str) -> Option<TextDestination<'a>> {
    let segments: Vec<_> = path.split('/').collect();
    match segments.as_slice() {
        ["", "slides", index, "notes"] => Some(TextDestination::Plain(&mut deck.slides.get_mut(index.parse::<usize>().ok()?)?.notes)),
        ["", "slides", index, "elements", rest @ ..] => element_destination(&mut deck.slides.get_mut(index.parse::<usize>().ok()?)?.elements, rest),
        ["", "design", "masters", index, "elements", rest @ ..] => element_destination(&mut deck.design.as_mut()?.masters.get_mut(index.parse::<usize>().ok()?)?.elements, rest),
        ["", "design", "layouts", index, "elements", rest @ ..] => element_destination(&mut deck.design.as_mut()?.layouts.get_mut(index.parse::<usize>().ok()?)?.elements, rest),
        _ => None,
    }
}

fn check_font(family: &str) -> Result<()> {
    valid_text(family, 100)?;
    if family.trim().is_empty() || family.chars().any(char::is_control) || family.starts_with('@') {
        return Err(Error::Invalid("font replacement requires explicit nonempty families, not theme tokens".into()));
    }
    Ok(())
}

fn format_fonts(format: &mut TextFormat, from: &str, to: &str) {
    if format.font_family.as_deref() == Some(from) { format.font_family = Some(to.into()); }
    for run in format.paragraphs.iter_mut().flat_map(|paragraph| &mut paragraph.runs) {
        if run.style.font_family.as_deref() == Some(from) { run.style.font_family = Some(to.into()); }
    }
}

fn element_fonts(elements: &mut [Element], from: &str, to: &str) {
    for element in elements {
        match element {
            Element::Text { format, .. } | Element::Shape { format, .. } => {
                format_fonts(format, from, to);
            }
            Element::Table { format, .. } => {
                for cell in &mut format.cells {
                    if let Some(style) = &mut cell.style.text_style {
                        if style.font_family.as_deref() == Some(from) { style.font_family = Some(to.into()); }
                    }
                    if let Some(format) = &mut cell.style.text_format { format_fonts(format, from, to); }
                }
            }
            Element::Group { children, .. } => element_fonts(children, from, to),
            Element::Rect { .. } | Element::Polygon { .. }
            | Element::Chart { .. } | Element::Picture { .. } | Element::Connector { .. } => {}
        }
    }
}

/// Replace exact, case-sensitive explicit family names, including master/layout and theme fonts.
/// Unspecified families and theme references such as @major remain unchanged.
pub fn replace_font(mut deck: Deck, from: &str, to: &str) -> Result<Deck> {
    check_font(from)?;
    check_font(to)?;
    validate_deck(&deck)?;
    if from == to { return Ok(deck); }
    for slide in &mut deck.slides { element_fonts(&mut slide.elements, from, to); }
    if let Some(design) = &mut deck.design {
        for master in &mut design.masters { element_fonts(&mut master.elements, from, to); }
        for layout in &mut design.layouts { element_fonts(&mut layout.elements, from, to); }
        let fonts = &mut design.theme.fonts;
        for family in [&mut fonts.major, &mut fonts.minor, &mut fonts.east_asian, &mut fonts.complex_script] {
            if family == from { *family = to.into(); }
        }
    }
    validate_deck(&deck)?;
    Ok(deck)
}