use crate::{model::{valid_text, Deck, Element}, rich_text::{RichParagraph, RunStyle}, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_DICTIONARY_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FormatSnapshot { Text(TextSnapshot), Object(ObjectSnapshot) }

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PaintEffects {
    pub shadow: Option<crate::visual::Shadow>,
    pub glow: Option<crate::visual::Glow>,
    pub soft_edge: Option<f64>,
    pub reflection: Option<crate::visual::Reflection>,
}

impl PaintEffects {
    fn copy(element: &Element) -> Self {
        let visual = element.visual().cloned().unwrap_or_default();
        Self { shadow: visual.shadow, glow: visual.glow, soft_edge: visual.soft_edge, reflection: visual.reflection }
    }
    fn apply(&self, element: &mut Element) -> Result<()> {
        let visual = element.visual_mut().ok_or_else(|| Error::Unsupported("target has no visual effects".into()))?.get_or_insert_with(Default::default);
        visual.shadow = self.shadow.clone(); visual.glow = self.glow.clone(); visual.soft_edge = self.soft_edge; visual.reflection = self.reflection.clone();
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceStyle {
    pub fill: String, pub stroke: Option<String>, pub stroke_width: Option<f64>,
    pub opacity: Option<f64>, pub gradient: Option<crate::visual::Gradient>,
}

impl SurfaceStyle {
    fn copy(element: &Element) -> Option<Self> {
        let (fill,stroke,stroke_width) = match element {
            Element::Rect { fill, .. } => (fill.clone(),None,None),
            Element::Shape { fill, stroke, stroke_width, .. } | Element::Polygon { fill, stroke, stroke_width, .. } => (fill.clone(),Some(stroke.clone()),Some(*stroke_width)),
            _ => return None,
        };
        let visual = element.visual().cloned().unwrap_or_default();
        Some(Self { fill, stroke, stroke_width, opacity: visual.opacity, gradient: visual.gradient })
    }
    fn apply(&self, element: &mut Element) -> Result<()> {
        match element {
            Element::Rect { fill, .. } if self.stroke_width.is_none_or(|width| width == 0.0) => *fill = self.fill.clone(),
            Element::Polygon { fill, stroke, stroke_width, .. } | Element::Shape { fill, stroke, stroke_width, .. } => {
                *fill = self.fill.clone(); *stroke = self.stroke.clone().unwrap_or_else(|| "@dk1".into()); *stroke_width = self.stroke_width.unwrap_or(0.0);
            },
            _ => return Err(Error::Unsupported("surface format needs a compatible filled shape; rectangle cannot represent an outline".into())),
        }
        let visual = element.visual_mut().ok_or_else(|| Error::Unsupported("target has no surface".into()))?.get_or_insert_with(Default::default);
        visual.opacity = self.opacity; visual.gradient = self.gradient.clone();
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObjectSnapshot {
    Filled { surface: SurfaceStyle, effects: PaintEffects },
    Picture { opacity: Option<f64>, effects: PaintEffects },
    Table { font_size: f64, rows: usize, columns: usize, cells: Vec<crate::table_format::CellFormat> },
    Chart { colors: Vec<String>, legend: Option<crate::model::chart_format::LegendPosition>, data_labels: Option<crate::model::chart_format::DataLabels>, axis_formats: [Option<String>;3] },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextSnapshot {
    pub run: RunStyle,
    pub paragraph: RichParagraph,
    pub vertical: crate::model::VerticalAlign,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub surface: Option<SurfaceStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub effects: Option<PaintEffects>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub text_warp: Option<crate::visual::TextWarp>,
}

pub fn copy_format(element: &Element, paragraph_index: usize, run_index: usize) -> Result<FormatSnapshot> {
    if matches!(element,Element::Text { .. } | Element::Shape { .. }) { return copy_text(element,paragraph_index,run_index).map(FormatSnapshot::Text); }
    if paragraph_index != 0 || run_index != 0 { return Err(Error::Invalid("non-text formats require zero paragraph/run indexes".into())); }
    let style = match element {
        Element::Rect { .. } | Element::Polygon { .. } => ObjectSnapshot::Filled { surface: SurfaceStyle::copy(element).ok_or_else(|| Error::Invalid("missing surface".into()))?, effects: PaintEffects::copy(element) },
        Element::Picture { .. } => ObjectSnapshot::Picture { opacity: element.visual().and_then(|visual| visual.opacity), effects: PaintEffects::copy(element) },
        Element::Table { rows, font_size, format, .. } => ObjectSnapshot::Table { font_size:*font_size, rows:rows.len(), columns:rows[0].len(), cells:format.cells.iter().cloned().map(|mut cell| { sanitize_cell(&mut cell.style); cell }).collect() },
        Element::Chart { series, options, .. } => ObjectSnapshot::Chart { colors:series.iter().map(|series| series.color.clone()).collect(), legend:options.legend, data_labels:options.data_labels.clone(), axis_formats:[options.primary_axis.number_format.clone(),options.secondary_axis.number_format.clone(),options.category_axis.number_format.clone()] },
        _ => return Err(Error::Unsupported("format painter supports text, rect, polygon, shape, picture, table and chart".into())),
    };
    Ok(FormatSnapshot::Object(style))
}

fn sanitize_cell(style: &mut crate::table_format::CellStyle) {
    if let Some(format) = &mut style.text_format {
        format.paragraphs.truncate(1);
        if let Some(paragraph) = format.paragraphs.first_mut() {
            if let Some(run) = paragraph.runs.first() { style.text_style.get_or_insert_with(Default::default).overlay(&run.style); }
            paragraph.runs.clear();
        }
        format.hyperlink = None; format.placeholder = None; format.inherit_layout = false;
    }
    if let Some(run) = &mut style.text_style { run.highlight = None; run.language = None; }
}

fn copy_text(element: &Element, paragraph_index: usize, run_index: usize) -> Result<TextSnapshot> {
    let (text, format, font_size, color, bold) = match element {
        Element::Text { text, format, font_size, color, bold, .. } | Element::Shape { text, format, font_size, color, bold, .. } => (text, format, *font_size, color, *bold),
        _ => return Err(Error::Unsupported("format painter supports top-level text boxes and shape text only".into())),
    };
    crate::rich_text::validate_element(element)?;
    let mut run = RunStyle::frame(font_size, color, bold, format);
    let mut paragraph = if format.paragraphs.is_empty() {
        if paragraph_index >= text.split('\n').count() || run_index != 0 { return Err(Error::Invalid("format source paragraph or run not found".into())); }
        RichParagraph::default()
    } else {
        let paragraph = format.paragraphs.get(paragraph_index).ok_or_else(|| Error::Invalid("format source paragraph not found".into()))?;
        if let Some(source) = paragraph.runs.get(run_index) { run.overlay(&source.style); }
        else if run_index != 0 || !paragraph.runs.is_empty() { return Err(Error::Invalid("format source run not found".into())); }
        paragraph.clone()
    };
    run.highlight = None; run.language = None; run.baseline = Some(run.baseline.unwrap_or(0));
    paragraph.runs.clear();
    paragraph.alignment = Some(paragraph.alignment.unwrap_or(format.alignment));
    paragraph.bullet = Some(paragraph.bullet.unwrap_or(format.bullet));
    Ok(TextSnapshot { run, paragraph, vertical: format.vertical, surface: SurfaceStyle::copy(element), effects:Some(PaintEffects::copy(element)), text_warp:element.visual().and_then(|visual| visual.text_warp) })
}

pub fn apply_format(mut deck: Deck, slide_id: &str, ids: &[String], style: &FormatSnapshot) -> Result<Deck> {
    if let FormatSnapshot::Text(style) = style { return apply_text(deck,slide_id,ids,style); }
    let FormatSnapshot::Object(style) = style else { unreachable!() };
    if ids.is_empty() || ids.len() > 128 || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() { return Err(Error::Limit("format painter requires 1-128 unique targets".into())); }
    let slide = deck.slides.iter_mut().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("format target slide not found".into()))?;
    for id in ids {
        let element = slide.elements.iter_mut().find(|element| element.bounds().0 == id).ok_or_else(|| Error::Invalid("format target not found".into()))?;
        if element.visual().is_some_and(|visual| visual.locked || visual.hidden) { return Err(Error::Unsupported("unlock and show format targets".into())); }
        match (style,&mut *element) {
            (ObjectSnapshot::Filled { surface,effects },target) => { surface.apply(target)?; effects.apply(target)?; }
            (ObjectSnapshot::Picture { opacity,effects },target @ Element::Picture { .. }) => { effects.apply(target)?; target.visual_mut().unwrap().get_or_insert_with(Default::default).opacity = *opacity; }
            (ObjectSnapshot::Chart { colors,legend,data_labels,axis_formats },Element::Chart { series,options,.. }) => {
                if colors.len() != series.len() { return Err(Error::Unsupported("chart format requires matching series counts".into())); }
                for (series,color) in series.iter_mut().zip(colors) { series.color = color.clone(); }
                options.legend = *legend; options.data_labels = data_labels.clone();
                options.primary_axis.number_format = axis_formats[0].clone(); options.secondary_axis.number_format = axis_formats[1].clone(); options.category_axis.number_format = axis_formats[2].clone();
            }
            (ObjectSnapshot::Table { font_size,rows,columns,cells },Element::Table { rows:target_rows,font_size:target_size,format,.. }) => {
                if *rows != target_rows.len() || *columns != target_rows[0].len() || cells.len()>96 { return Err(Error::Unsupported("table format requires matching grid dimensions".into())); }
                let mut seen = BTreeSet::new();
                for cell in cells {
                    let mut safe = cell.style.clone(); sanitize_cell(&mut safe);
                    if safe != cell.style || cell.row>=*rows || cell.column>=*columns || !seen.insert((cell.row,cell.column)) { return Err(Error::Invalid("table snapshot contains content, semantics or invalid cell indices".into())); }
                }
                let mut next = Vec::new();
                for row in 0..*rows { for column in 0..*columns {
                    let old = format.cell_style(row,column);
                    let mut cell = cells.iter().find(|cell| cell.row==row && cell.column==column).map(|cell| cell.style.clone()).unwrap_or_default();
                    if let Some(old_run) = &old.text_style {
                        let run = cell.text_style.get_or_insert_with(Default::default); run.highlight = old_run.highlight.clone(); run.language = old_run.language.clone();
                    }
                    if cell.text_format.is_some() || old.text_format.is_some() {
                        let text_format = cell.text_format.get_or_insert_with(Default::default);
                        let mut template = text_format.paragraphs.first().cloned().unwrap_or_default();
                        template.alignment = Some(template.alignment.unwrap_or(text_format.alignment));
                        template.bullet = Some(template.bullet.unwrap_or(text_format.bullet));
                        let paragraphs = old.text_format.as_ref().map(|format| format.paragraphs.clone()).filter(|paragraphs| !paragraphs.is_empty()).unwrap_or_else(|| target_rows[row][column].split('\n').map(|text| RichParagraph { runs:vec![crate::rich_text::RichRun { text:text.into(),style:Default::default(),field:None }],..Default::default() }).collect());
                        let mut paint = RunStyle::frame(*font_size,"@dk1",false,text_format);
                        if let Some(run) = &cell.text_style { paint.overlay(run); }
                        paint.highlight = None; paint.language = None;
                        text_format.paragraphs = paragraphs.into_iter().map(|paragraph| {
                            let mut result = template.clone(); result.runs = paragraph.runs;
                            for run in &mut result.runs { run.style.overlay(&paint); }
                            result
                        }).collect();
                    }
                    if cell != crate::table_format::CellStyle::default() { next.push(crate::table_format::CellFormat { row,column,style:cell }); }
                } }
                *target_size = *font_size; format.cells = next;
            }
            _ => return Err(Error::Unsupported("incompatible object format target".into())),
        }
    }
    crate::model::validate_deck(&deck)?;
    Ok(deck)
}

fn apply_text(mut deck: Deck, slide_id: &str, ids: &[String], style: &TextSnapshot) -> Result<Deck> {
    style.run.validate()?;
    crate::rich_text::validate_paragraphs(std::slice::from_ref(&style.paragraph))?;
    if !style.paragraph.runs.is_empty() || style.run.highlight.is_some() || style.run.language.is_some() {
        return Err(Error::Invalid("format snapshot cannot contain text, fields, highlight or proofing language".into()));
    }
    if ids.is_empty() || ids.len() > 128 || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err(Error::Limit("format painter requires 1-128 unique target IDs".into()));
    }
    let slide = deck.slides.iter_mut().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("format target slide not found".into()))?;
    for id in ids {
        let element = slide.elements.iter_mut().find(|element| element.bounds().0 == id).ok_or_else(|| Error::Invalid("format target not found".into()))?;
        let length = match element {
            Element::Text { text, visual, .. } | Element::Shape { text, visual, .. } => {
                if visual.as_ref().is_some_and(|visual| visual.locked || visual.hidden) { return Err(Error::Unsupported("unlock and show all format targets before editing".into())); }
                text.chars().count()
            },
            _ => return Err(Error::Unsupported("format painter supports text boxes and shape text only".into())),
        };
        *element = crate::rich_text::apply_range(element.clone(), 0, length, style.run.clone())?;
        if let Element::Text { text, format, .. } | Element::Shape { text, format, .. } = element {
            format.vertical = style.vertical; format.inherit_layout = false;
            if format.paragraphs.is_empty() {
                format.paragraphs = text.split('\n').map(|line| RichParagraph { runs: vec![crate::rich_text::RichRun { text: line.into(), style: style.run.clone(), field: None }], ..Default::default() }).collect();
            }
            for paragraph in &mut format.paragraphs {
                let mut runs = std::mem::take(&mut paragraph.runs);
                if runs.is_empty() { runs.push(crate::rich_text::RichRun { text: String::new(), style: style.run.clone(), field: None }); }
                for run in &mut runs { run.style.overlay(&style.run); }
                *paragraph = style.paragraph.clone(); paragraph.runs = runs;
            }
        }
        if let Some(surface) = &style.surface { surface.apply(element)?; }
        if let Some(effects) = &style.effects {
            effects.apply(element)?;
            element.visual_mut().unwrap().get_or_insert_with(Default::default).text_warp = style.text_warp;
        }
    }
    crate::model::validate_deck(&deck)?;
    Ok(deck)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dictionary {
    pub language: String,
    pub words: Vec<String>,
    #[serde(default)] pub synonyms: BTreeMap<String, Vec<String>>,
    #[serde(default)] pub translations: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DictionaryFormat { Wordlist, Json }

#[derive(Debug, Serialize)]
pub struct Issue {
    pub word: String,
    pub start: usize,
    pub end: usize,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ProofResult {
    pub language: String,
    pub sample_dictionary: bool,
    pub complete_dictionary: bool,
    pub issues: Vec<Issue>,
    pub synonyms: Vec<String>,
    pub translation: Option<String>,
}

pub fn validate_language(language: &str) -> Result<()> {
    let mut segments = language.split('-');
    let first = segments.next().unwrap_or_default();
    if language.len() > 64 || !(2..=8).contains(&first.len()) || !first.bytes().all(|byte| byte.is_ascii_alphabetic())
        || segments.any(|segment| segment.is_empty() || segment.len() > 8 || !segment.bytes().all(|byte| byte.is_ascii_alphanumeric())) {
        return Err(Error::Invalid("select a language tag such as en-US or ja-JP".into()));
    }
    Ok(())
}

fn term(value: &str) -> Result<()> {
    valid_text(value, 128)?;
    if value.trim() != value || value.is_empty() || value.chars().any(char::is_control) {
        return Err(Error::Invalid("dictionary terms must be nonempty text without controls or outer whitespace".into()));
    }
    Ok(())
}

fn word_character(character: char) -> bool { character.is_alphabetic() || character == '\'' || character == '\u{2019}' }

impl Dictionary {
    pub fn validate(&self) -> Result<()> {
        validate_language(&self.language)?;
        if serde_json::to_vec(self)?.len() > MAX_DICTIONARY_BYTES || self.words.len() > 10000
            || self.synonyms.len() > 2000 || self.translations.len() > 16 {
            return Err(Error::Limit("dictionary exceeds 512 KiB, 10000 words, 2000 synonym keys or 16 target languages".into()));
        }
        for word in &self.words {
            term(word)?;
            if word.chars().count() > 64 || !word.chars().all(word_character) || !word.chars().any(char::is_alphabetic) {
                return Err(Error::Invalid("word list entries must be alphabetic words of at most 64 scalars (apostrophes allowed)".into()));
            }
        }
        let mut keys = BTreeSet::new();
        for (key, values) in &self.synonyms {
            term(key)?;
            if !keys.insert(key.to_lowercase()) || values.len() > 16 { return Err(Error::Invalid("duplicate case-folded synonym key or more than 16 synonyms".into())); }
            for value in values { term(value)?; }
        }
        let mut languages = BTreeSet::new();
        for (language, values) in &self.translations {
            validate_language(language)?;
            if !languages.insert(language.to_lowercase()) || values.len() > 2000 { return Err(Error::Limit("duplicate target language or more than 2000 translations".into())); }
            let mut keys = BTreeSet::new();
            for (key, value) in values {
                term(key)?; term(value)?;
                if !keys.insert(key.to_lowercase()) { return Err(Error::Invalid("duplicate case-folded translation key".into())); }
            }
        }
        Ok(())
    }
}

pub fn import(language: &str, format: DictionaryFormat, content: &str) -> Result<Dictionary> {
    validate_language(language)?;
    if content.len() > MAX_DICTIONARY_BYTES { return Err(Error::Limit("dictionary file exceeds 512 KiB".into())); }
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let dictionary = match format {
        DictionaryFormat::Wordlist => Dictionary { language: language.into(), words: content.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_owned).collect(), synonyms: BTreeMap::new(), translations: BTreeMap::new() },
        DictionaryFormat::Json => serde_json::from_str(content)?,
    };
    dictionary.validate()?;
    if !dictionary.language.eq_ignore_ascii_case(language) { return Err(Error::Invalid("dictionary language differs from selected language".into())); }
    Ok(dictionary)
}

fn sample() -> Dictionary {
    Dictionary { language: "en-US".into(),
        words: "a an and are as at be by can data for from has have hello in is it local of on or report slide slides test text the this to use we with word world you".split_whitespace().map(str::to_owned).collect(),
        synonyms: BTreeMap::from([("hello".into(), vec!["greeting".into()]), ("report".into(), vec!["account".into(), "summary".into()])]),
        translations: BTreeMap::new() }
}

fn one_edit(left: &[char], right: &[char]) -> bool {
    if left.len().abs_diff(right.len()) > 1 { return false; }
    let prefix = left.iter().zip(right).take_while(|(left, right)| left == right).count();
    if prefix == left.len().min(right.len()) { return true; }
    if left.len() == right.len() {
        left[prefix + 1..] == right[prefix + 1..]
            || (prefix + 1 < left.len() && left[prefix] == right[prefix + 1] && left[prefix + 1] == right[prefix] && left[prefix + 2..] == right[prefix + 2..])
    } else if left.len() > right.len() { left[prefix + 1..] == right[prefix..] }
    else { left[prefix..] == right[prefix + 1..] }
}

pub fn proof(text: &str, language: &str, dictionary: Option<&Dictionary>, lookup: Option<&str>, target_language: Option<&str>) -> Result<ProofResult> {
    valid_text(text, 4000)?; validate_language(language)?;
    if let Some(lookup) = lookup { term(lookup)?; }
    if let Some(target) = target_language { validate_language(target)?; }
    let fallback = sample();
    let sample_dictionary = dictionary.is_none();
    let dictionary = dictionary.unwrap_or(&fallback);
    dictionary.validate()?;
    if !language.eq_ignore_ascii_case(&dictionary.language) { return Err(Error::Unsupported("no local dictionary for the selected language; choose a matching file".into())); }
    let words: BTreeSet<_> = dictionary.words.iter().map(|word| word.to_lowercase()).collect();
    let candidates: Vec<_> = words.iter().map(|word| (word, word.chars().collect::<Vec<_>>())).collect();
    let characters: Vec<_> = text.chars().collect();
    let mut issues = Vec::new();
    let mut start = 0;
    while start < characters.len() {
        if !characters[start].is_alphabetic() { start += 1; continue; }
        let mut end = start + 1;
        while end < characters.len() && word_character(characters[end]) { end += 1; }
        while end > start && !characters[end - 1].is_alphabetic() { end -= 1; }
        let word: String = characters[start..end].iter().collect();
        let folded = word.to_lowercase();
        if !words.contains(&folded) {
            if issues.len() == 100 { return Err(Error::Limit("more than 100 unknown words; check a shorter selection".into())); }
            let letters: Vec<_> = folded.chars().collect();
            let suggestions = if letters.len() > 64 { Vec::new() } else {
                candidates.iter().filter(|(_, candidate)| one_edit(&letters, candidate)).take(5).map(|(word, _)| (*word).clone()).collect()
            };
            issues.push(Issue { word, start, end, suggestions });
        }
        start = end;
    }
    let synonyms = lookup.and_then(|lookup| dictionary.synonyms.iter().find(|(key, _)| key.to_lowercase() == lookup.to_lowercase()).map(|(_, values)| values.clone())).unwrap_or_default();
    let translation = match (lookup, target_language) {
        (Some(lookup), Some(target)) => dictionary.translations.iter().find(|(language, _)| language.eq_ignore_ascii_case(target))
            .and_then(|(_, values)| values.iter().find(|(key, _)| key.to_lowercase() == lookup.to_lowercase()).map(|(_, value)| value.clone())),
        _ => None,
    };
    Ok(ProofResult { language: language.into(), sample_dictionary, complete_dictionary: false, issues, synonyms, translation })
}