use crate::{model::{Bullet, Element, TextAlign, TextFormat, valid_color, valid_text}, Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RunStyle {
    #[serde(skip_serializing_if = "Option::is_none")] pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")] pub font_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub baseline: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")] pub highlight: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub language: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RichRun { pub text: String, pub style: RunStyle, #[serde(skip_serializing_if = "Option::is_none")] pub field: Option<crate::fields::Field> }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Spacing { Percent(u32), Points(u32) }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabAlign { #[default] Left, Center, Right, Decimal }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabStop { pub position: i32, #[serde(default)] pub alignment: TabAlign }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RichParagraph {
    pub runs: Vec<RichRun>,
    #[serde(skip_serializing_if = "Option::is_none")] pub alignment: Option<TextAlign>,
    #[serde(skip_serializing_if = "Option::is_none")] pub bullet: Option<Bullet>,
    #[serde(skip_serializing_if = "Option::is_none")] pub bullet_character: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub numbering: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub number_start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")] pub level: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")] pub margin_left: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")] pub indent: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")] pub line_spacing: Option<Spacing>,
    #[serde(skip_serializing_if = "Option::is_none")] pub space_before: Option<Spacing>,
    #[serde(skip_serializing_if = "Option::is_none")] pub space_after: Option<Spacing>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub tabs: Vec<TabStop>,
}

pub const NUMBERING: &[&str] = &["arabicPeriod", "arabicParenR", "arabicParenBoth", "arabicPlain", "alphaLcPeriod", "alphaUcPeriod", "alphaLcParenR", "alphaUcParenR", "romanLcPeriod", "romanUcPeriod"];

impl RunStyle {
    pub fn validate(&self) -> Result<()> {
        if self.font_size.is_some_and(|size| !size.is_finite() || !(1.0..=400.0).contains(&size)) { return Err(Error::Invalid("rich font size must be finite and in 1..400px".into())); }
        if let Some(color) = &self.color { valid_color(color)?; }
        if let Some(color) = &self.highlight { if color != "none" { valid_color(color)?; } }
        if let Some(family) = &self.font_family {
            valid_text(family, 100)?;
            if family.trim().is_empty() || family.chars().any(char::is_control) || (family.starts_with('@') && !["@major", "@minor"].contains(&family.as_str())) { return Err(Error::Invalid("invalid rich font family".into())); }
        }
        if self.baseline.is_some_and(|value| !(-100000..=100000).contains(&value)) { return Err(Error::Invalid("baseline must be in -100000..100000".into())); }
        if let Some(language) = &self.language {
            if language.is_empty() || language.len() > 64 || !language.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-') { return Err(Error::Invalid("invalid rich text language".into())); }
        }
        Ok(())
    }

    pub(crate) fn overlay(&mut self, style: &Self) {
        macro_rules! merge { ($($field:ident),*) => { $(if style.$field.is_some() { self.$field = style.$field.clone(); })* }; }
        merge!(bold, italic, underline, font_size, color, font_family, baseline, highlight, language);
    }

    pub(crate) fn frame(size: f64, color: &str, bold: bool, format: &TextFormat) -> Self {
        Self { bold: Some(bold), italic: Some(format.italic), underline: Some(format.underline), font_size: Some(size), color: Some(color.into()), font_family: Some(format.font_family.clone().unwrap_or_else(|| "@minor".into())), baseline: None, highlight: None, language: Some("ja-JP".into()) }
    }
}

pub fn plain_text(paragraphs: &[RichParagraph]) -> String {
    paragraphs.iter().map(|paragraph| paragraph.runs.iter().map(|run| run.text.as_str()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

pub fn validate_paragraphs(paragraphs: &[RichParagraph]) -> Result<()> {
    validate_paragraphs_with_limit(paragraphs, 4000)
}

pub(crate) fn validate_paragraphs_with_limit(paragraphs: &[RichParagraph], limit: usize) -> Result<()> {
    if paragraphs.len() > 256 || paragraphs.iter().map(|paragraph| paragraph.runs.len()).sum::<usize>() > 4096 { return Err(Error::Limit("rich text exceeds 256 paragraphs or 4096 runs".into())); }
    let mut count = paragraphs.len().saturating_sub(1);
    let mut field_ids = std::collections::BTreeSet::new();
    for paragraph in paragraphs {
        if paragraph.level.is_some_and(|level| level > 8) || paragraph.margin_left.is_some_and(|value| !(0..=51_206_400).contains(&value)) || paragraph.indent.is_some_and(|value| !(-51_206_400..=51_206_400).contains(&value)) { return Err(Error::Invalid("invalid rich paragraph indentation".into())); }
        if paragraph.number_start.is_some_and(|value| !(1..=32767).contains(&value)) || paragraph.numbering.as_ref().is_some_and(|value| !NUMBERING.contains(&value.as_str())) { return Err(Error::Invalid("invalid rich paragraph numbering".into())); }
        if (paragraph.number_start.is_some() || paragraph.numbering.is_some()) && paragraph.bullet != Some(Bullet::Numbered) { return Err(Error::Invalid("numbering requires a numbered paragraph".into())); }
        if let Some(character) = &paragraph.bullet_character { valid_text(character, 1)?; if paragraph.bullet != Some(Bullet::Bullet) || character.chars().count() != 1 || character.chars().any(char::is_control) { return Err(Error::Invalid("invalid bullet character".into())); } }
        for spacing in [paragraph.line_spacing, paragraph.space_before, paragraph.space_after].into_iter().flatten() {
            if match spacing { Spacing::Percent(value) => value > 1_000_000, Spacing::Points(value) => value > 158400 } { return Err(Error::Invalid("rich paragraph spacing out of range".into())); }
        }
        if matches!(paragraph.line_spacing, Some(Spacing::Percent(0) | Spacing::Points(0))) { return Err(Error::Invalid("line spacing must be positive".into())); }
        if paragraph.tabs.len() > 32 || paragraph.tabs.iter().any(|tab| !(0..=51_206_400).contains(&tab.position)) || paragraph.tabs.windows(2).any(|pair| pair[0].position >= pair[1].position) { return Err(Error::Invalid("tab positions must be bounded and strictly increasing".into())); }
        for run in &paragraph.runs {
            if let Some(field) = &run.field { field.validate()?; if !field_ids.insert(crate::fields::parse_id(&field.id)?) { return Err(Error::Invalid("duplicate field UUID in text body".into())); } }
            valid_text(&run.text, limit)?;
            if run.text.contains(['\n', '\r']) { return Err(Error::Invalid("rich runs cannot contain paragraph separators".into())); }
            count += run.text.chars().count(); run.style.validate()?;
        }
    }
    if count > limit { return Err(Error::Limit(format!("rich text exceeds {limit} Unicode scalars"))); }
    Ok(())
}

pub(crate) fn deserialize_paragraphs<'de, D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Vec<RichParagraph>, D::Error> {
    let paragraphs = Vec::<RichParagraph>::deserialize(deserializer)?;
    validate_paragraphs(&paragraphs).map_err(serde::de::Error::custom)?;
    Ok(paragraphs)
}

pub fn validate_element(element: &Element) -> Result<()> {
    if let Element::Text { text, format, .. } | Element::Shape { text, format, .. } = element {
        validate_paragraphs(&format.paragraphs)?;
        if !format.paragraphs.is_empty() && plain_text(&format.paragraphs) != *text { return Err(Error::Invalid("plain text must equal joined rich paragraph run texts".into())); }
    }
    Ok(())
}

fn editable(element: &mut Element) -> Result<(&mut String, &mut TextFormat)> {
    match element {
        Element::Text { text, format, .. } | Element::Shape { text, format, .. } => Ok((text, format)),
        _ => Err(Error::Unsupported("rich text requires a text box or shape".into())),
    }
}

fn paragraphs(text: &str, format: &TextFormat) -> Vec<RichParagraph> {
    if !format.paragraphs.is_empty() { return format.paragraphs.clone(); }
    text.split('\n').map(|line| RichParagraph { runs: vec![RichRun { text: line.into(), style: RunStyle::default(), field: None }], ..Default::default() }).collect()
}

fn append(runs: &mut Vec<RichRun>, text: String, style: RunStyle) {
    if text.is_empty() { return; }
    if let Some(last) = runs.last_mut().filter(|last| last.field.is_none() && last.style == style) { last.text.push_str(&text); }
    else { runs.push(RichRun { text, style, field: None }); }
}

pub fn apply_range(mut element: Element, start: usize, end: usize, style: RunStyle) -> Result<Element> {
    validate_element(&element)?; style.validate()?;
    let (text, format) = editable(&mut element)?;
    if start > end || end > text.chars().count() { return Err(Error::Invalid("invalid Unicode scalar range".into())); }
    if start == end || style == RunStyle::default() { return Ok(element); }
    let mut result = paragraphs(text, format); let mut offset = 0;
    for paragraph in &mut result {
        let mut runs = Vec::new();
        for run in &paragraph.runs {
            let chars: Vec<_> = run.text.chars().collect();
            let left = start.saturating_sub(offset).min(chars.len());
            let right = end.saturating_sub(offset).min(chars.len());
            if run.field.is_some() {
                if right > left && (left != 0 || right != chars.len()) { return Err(Error::Unsupported("style ranges cannot split a dynamic field".into())); }
                let mut preserved = run.clone();
                if right > left { preserved.style.overlay(&style); }
                runs.push(preserved); offset += chars.len(); continue;
            }
            append(&mut runs, chars[..left].iter().collect(), run.style.clone());
            if right > left { let mut merged = run.style.clone(); merged.overlay(&style); append(&mut runs, chars[left..right].iter().collect(), merged); }
            append(&mut runs, chars[right.max(left)..].iter().collect(), run.style.clone());
            offset += chars.len();
        }
        if !runs.is_empty() { paragraph.runs = runs; }
        offset += 1;
    }
    format.paragraphs = result; format.inherit_layout = false;
    validate_element(&element)?;
    Ok(element)
}

pub fn replace_text_content(mut element: Element, replacement: String) -> Result<Element> {
    validate_element(&element)?; valid_text(&replacement, 4000)?;
    if crate::fields::has_fields(&element) { return crate::fields::replace_body(element, replacement); }
    let (text, format) = editable(&mut element)?;
    if *text == replacement { return Ok(element); }
    if format.paragraphs.is_empty() { *text = replacement; return Ok(element); }
    if replacement.contains('\r') { return Err(Error::Invalid("rich text uses LF paragraph separators".into())); }
    let before: Vec<_> = text.chars().collect(); let after: Vec<_> = replacement.chars().collect();
    let prefix = before.iter().zip(&after).take_while(|(left, right)| left == right).count();
    let suffix = before[prefix..].iter().rev().zip(after[prefix..].iter().rev()).take_while(|(left, right)| left == right).count();
    let mut tokens = Vec::new();
    for (index, paragraph) in format.paragraphs.iter().enumerate() {
        if index > 0 { tokens.push(('\n', RunStyle::default(), index - 1)); }
        for run in &paragraph.runs { for character in run.text.chars() { tokens.push((character, run.style.clone(), index)); } }
    }
    let insertion_paragraph = before[..prefix].iter().filter(|character| **character == '\n').count();
    let inherited = prefix.checked_sub(1).and_then(|index| tokens.get(index)).filter(|token| token.0 != '\n').or_else(|| tokens.get(prefix).filter(|token| token.0 != '\n'));
    let (style, paragraph_index) = inherited.map(|token| (token.1.clone(), token.2)).unwrap_or_else(|| (format.paragraphs[insertion_paragraph].runs.first().map(|run| run.style.clone()).unwrap_or_default(), insertion_paragraph));
    let inserted = after[prefix..after.len() - suffix].iter().map(|character| (*character, style.clone(), paragraph_index));
    let combined: Vec<_> = tokens[..prefix].iter().cloned().chain(inserted).chain(tokens[tokens.len() - suffix..].iter().cloned()).collect();
    let first_index = combined.first().map(|token| token.2).unwrap_or(paragraph_index);
    let mut paragraph = format.paragraphs[first_index].clone();
    let mut empty_style = paragraph.runs.first().map(|run| run.style.clone()).unwrap_or_else(|| style.clone()); paragraph.runs.clear();
    let mut result = Vec::new();
    for (index, (character, style, source)) in combined.iter().enumerate() {
        if *character == '\n' {
            if paragraph.runs.is_empty() { paragraph.runs.push(RichRun { text: String::new(), style: empty_style.clone(), field: None }); }
            result.push(paragraph);
            let next_source = combined.get(index + 1).map(|token| token.2).unwrap_or((*source + 1).min(format.paragraphs.len() - 1));
            paragraph = format.paragraphs[next_source].clone(); empty_style = paragraph.runs.first().map(|run| run.style.clone()).unwrap_or_else(|| style.clone()); paragraph.runs.clear();
        } else { append(&mut paragraph.runs, character.to_string(), style.clone()); }
    }
    if paragraph.runs.is_empty() { paragraph.runs.push(RichRun { text: String::new(), style: empty_style, field: None }); }
    result.push(paragraph); format.paragraphs = result; *text = replacement;
    validate_element(&element)?;
    Ok(element)
}