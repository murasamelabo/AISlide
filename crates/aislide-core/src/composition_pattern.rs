//! Pattern composition: places typed blocks in layout-pattern slots resolved inside design-token
//! frames, so composed slides share one spacing, type and color system. Text is measured at token
//! sizes and never shrunk to fit; undersized slots and overflow fail with the pattern fallback.
use crate::{
    composition::{CompositionBlock, CompositionSpec, CompositionStyle, ImageFit, PatternChoice},
    design::Theme,
    design_tokens::DesignTokens,
    layout_patterns::{Frame, PlacedSlot, ResolveOptions, Traits},
    model::{Crop, Deck, Element, TextAlign, TextFormat, TextPadding, VerticalAlign, valid_text, validate_rows},
    parts::{PartData, PartFit, PartItem, PartLayout, PartSpec},
    rich_text::{RichParagraph, RichRun, RunStyle, Spacing},
    Error, Result,
};
use std::collections::{BTreeMap, BTreeSet};

/// Elements and managed parts of one composed slide, before parts are registered.
pub(crate) struct Built { pub elements: Vec<Element>, pub parts: Vec<(String, PartSpec)> }

/// Slide furniture drawn around the pattern body.
pub(crate) struct Chrome<'a> { pub title: &'a str, pub footer: &'a str, pub page: &'a str }

#[derive(Clone, Copy)]
struct Look { size: f64, color: &'static str, bold: bool, major: bool, align: TextAlign, vertical: VerticalAlign }

/// Language tag for proofing and line breaking, chosen from the script of the text.
pub(crate) fn language(text: &str) -> &'static str {
    if text.chars().any(|character| matches!(character, '\u{1100}'..='\u{11ff}' | '\u{3130}'..='\u{318f}' | '\u{ac00}'..='\u{d7af}')) { return "ko-KR"; }
    if text.chars().any(|character| matches!(character, '\u{3000}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{ff00}'..='\u{ffef}' | '\u{20000}'..='\u{323af}')) { return "ja-JP"; }
    "en-US"
}

fn run(text: &str, bold: Option<bool>) -> RichRun {
    RichRun { text: text.into(), style: RunStyle { bold, language: Some(language(text).into()), ..Default::default() }, field: None }
}

/// One paragraph per line of plain text, each run tagged with its language.
pub(crate) fn lines(text: &str) -> Vec<RichParagraph> {
    text.split('\n').map(|line| RichParagraph { runs: vec![run(line.trim_end_matches('\r'), None)], ..Default::default() }).collect()
}

fn tagged(paragraphs: &[RichParagraph]) -> Result<Vec<RichParagraph>> {
    crate::rich_text::validate_paragraphs(paragraphs)?;
    let mut paragraphs = paragraphs.to_vec();
    for run in paragraphs.iter_mut().flat_map(|paragraph| paragraph.runs.iter_mut()) {
        if run.field.is_some() { return Err(Error::Unsupported("composition text cannot contain dynamic fields; use dedicated field operations after composing".into())); }
        if run.style.language.is_none() { run.style.language = Some(language(&run.text).into()); }
    }
    Ok(paragraphs)
}

fn text_format(paragraphs: Vec<RichParagraph>, look: Look) -> TextFormat {
    TextFormat { alignment: look.align, vertical: look.vertical, font_family: Some(if look.major { "@major" } else { "@minor" }.into()), paragraphs, ..Default::default() }
}

fn text(id: String, frame: [f64; 4], paragraphs: Vec<RichParagraph>, look: Look) -> Result<Element> {
    crate::rich_text::validate_paragraphs(&paragraphs)?;
    let [x, y, width, height] = frame;
    Ok(Element::Text { id, x, y, width, height, text: crate::rich_text::plain_text(&paragraphs), font_size: look.size, color: look.color.into(), bold: look.bold, format: text_format(paragraphs, look), visual: None })
}

/// Language-tagged text box in the theme's minor font, for furniture outside pattern slots.
pub(crate) fn caption_text(id: String, frame: [f64; 4], value: &str, size: f64, color: &str, bold: bool, align: TextAlign) -> Result<Element> {
    let paragraphs = lines(value);
    crate::rich_text::validate_paragraphs(&paragraphs)?;
    let [x, y, width, height] = frame;
    let format = TextFormat { alignment: align, vertical: VerticalAlign::Middle, font_family: Some("@minor".into()), paragraphs, ..Default::default() };
    Ok(Element::Text { id, x, y, width, height, text: crate::rich_text::plain_text(&format.paragraphs), font_size: size, color: color.into(), bold, format, visual: None })
}

fn required(value: &str, limit: usize, what: &str) -> Result<()> {
    valid_text(value, limit)?;
    if value.trim().is_empty() { return Err(Error::Invalid(format!("{what} requires text"))); }
    Ok(())
}

fn rectangle(id: String, frame: [f64; 4], fill: &str) -> Element {
    Element::Rect { id, x: frame[0], y: frame[1], width: frame[2], height: frame[3], fill: fill.into(), visual: None }
}

fn container(id: String, frame: [f64; 4], paragraphs: Vec<RichParagraph>, padding: TextPadding, vertical: VerticalAlign, tokens: &DesignTokens) -> Result<Element> {
    crate::rich_text::validate_paragraphs(&paragraphs)?;
    padding.validate(frame[2], frame[3]).map_err(|_| Error::Invalid(format!("{id} is too small for its padding; use a larger slot or another pattern")))?;
    let [x, y, width, height] = frame;
    let look = Look { size: tokens.type_scale.body, color: tokens.colors.ink, bold: false, major: false, align: TextAlign::Left, vertical };
    let mut format = text_format(paragraphs, look);
    format.padding = Some(padding);
    Ok(Element::Shape { id, x, y, width, height, preset: "rect".into(), fill: tokens.card.fill.into(), stroke: tokens.card.fill.into(), stroke_width: 0.0, rotation: 0.0,
        text: crate::rich_text::plain_text(&format.paragraphs), font_size: look.size, color: look.color.into(), bold: false, format, visual: None })
}

/// Surface card with an accent rule along its top edge.
fn card(id: String, frame: [f64; 4], paragraphs: Vec<RichParagraph>, padding: f64, tokens: &DesignTokens) -> Result<Vec<Element>> {
    let rule = tokens.card.rule;
    let shape = container(id.clone(), frame, paragraphs, TextPadding { left: padding, right: padding, top: padding + rule, bottom: padding }, VerticalAlign::Top, tokens)?;
    Ok(vec![shape, rectangle(format!("{id}-rule"), [frame[0], frame[1], frame[2], rule], tokens.colors.accent)])
}

/// Takeaway band: surface strip with an accent bar on its leading edge.
fn band(id: String, frame: [f64; 4], paragraphs: Vec<RichParagraph>, tokens: &DesignTokens) -> Result<Vec<Element>> {
    let bar = 6.0;
    let padding = tokens.card.band_padding;
    let shape = container(id.clone(), [frame[0] + bar, frame[1], frame[2] - bar, frame[3]], paragraphs, TextPadding { left: padding, right: padding, top: padding / 2.0, bottom: padding / 2.0 }, VerticalAlign::Middle, tokens)?;
    Ok(vec![shape, rectangle(format!("{id}-bar"), [frame[0], frame[1], bar, frame[3]], tokens.colors.accent)])
}

fn item_paragraphs(item: &PartItem) -> Result<Vec<RichParagraph>> {
    valid_text(&item.label, 80)?; valid_text(&item.detail, 400)?;
    if item.value.is_some() { return Err(Error::Invalid("composition cards require text; use a metric block for values".into())); }
    if item.label.trim().is_empty() && item.detail.trim().is_empty() { return Err(Error::Invalid("composition cards require a label or detail".into())); }
    let mut paragraphs: Vec<RichParagraph> = if item.label.is_empty() { Vec::new() } else { item.label.split('\n').map(|line| RichParagraph { runs: vec![run(line, Some(true))], ..Default::default() }).collect() };
    if let Some(last) = paragraphs.last_mut().filter(|_| !item.detail.is_empty()) { last.space_after = Some(Spacing::Points(800)); }
    if !item.detail.is_empty() { paragraphs.extend(lines(&item.detail)); }
    Ok(paragraphs)
}

fn metric(id: &str, frame: [f64; 4], padding: f64, value: &str, label: &str, detail: &str, tokens: &DesignTokens, theme: &Theme) -> Result<Vec<Element>> {
    required(value, 40, "metric value")?; required(label, 120, "metric label")?; valid_text(detail, 240)?;
    let scale = tokens.type_scale;
    let rule = tokens.card.rule;
    let inner = [frame[0] + padding, frame[1] + padding + rule, frame[2] - padding * 2.0, frame[3] - padding * 2.0 - rule];
    if inner[2] < scale.body * 2.0 || inner[3] < scale.body { return Err(Error::Invalid(format!("{id} is too small for a metric; use a larger slot or another pattern"))); }
    let label_size = if inner[3] < 160.0 { scale.caption } else { scale.body };
    let label_look = Look { size: label_size, color: tokens.colors.ink, bold: false, major: false, align: TextAlign::Left, vertical: VerticalAlign::Top };
    let detail_look = Look { size: scale.caption, color: tokens.colors.muted, ..label_look };
    let value_look = Look { size: scale.metric, color: tokens.colors.accent, bold: true, major: true, align: TextAlign::Left, vertical: VerticalAlign::Bottom };
    let height = |content: &str, look: Look| -> Result<f64> { Ok((crate::layout::text_height(content, inner[2], look.size, look.bold, &text_format(lines(content), look), theme)? + 2.0).ceil()) };
    let label_height = height(label, label_look)?;
    let detail_height = if detail.is_empty() { 0.0 } else { height(detail, detail_look)? + 4.0 };
    let gap = 8.0;
    let value_height = (inner[3] - label_height - detail_height - gap).min((scale.metric * 1.25).ceil());
    let fitted = if value_height >= scale.body * 1.15 { crate::layout::fit_size(value, inner[2], value_height, [scale.metric, scale.body], true, &text_format(lines(value), value_look), theme)? } else { None };
    let size = fitted.ok_or_else(|| Error::Invalid(format!("metric value in {id} does not fit at {}px with its label; use a larger slot, fewer metrics or a shorter label", scale.body)))?;
    let top = inner[1] + ((inner[3] - value_height - gap - label_height - detail_height) / 2.0).max(0.0);
    let mut elements = vec![rectangle(id.into(), frame, tokens.card.fill), rectangle(format!("{id}-rule"), [frame[0], frame[1], frame[2], rule], tokens.colors.accent)];
    elements.push(text(format!("{id}-value"), [inner[0], top, inner[2], value_height], lines(value), Look { size, ..value_look })?);
    elements.push(text(format!("{id}-label"), [inner[0], top + value_height + gap, inner[2], label_height], lines(label), label_look)?);
    if !detail.is_empty() { elements.push(text(format!("{id}-detail"), [inner[0], top + value_height + gap + label_height + 4.0, inner[2], detail_height - 4.0], lines(detail), detail_look)?); }
    Ok(elements)
}

fn picture(id: String, frame: [f64; 4], base64: &str, mime_type: &str, alt: &str, fit: ImageFit) -> Result<Element> {
    required(alt, 500, "image alt text")?;
    let info = crate::media::inspect_raster(base64, mime_type)?;
    let (image_width, image_height) = (info.width as f64, info.height as f64);
    let [x, y, width, height] = frame;
    let (bounds, crop) = match fit {
        ImageFit::Contain => {
            let scale = (width / image_width).min(height / image_height);
            let (fitted_width, fitted_height) = (image_width * scale, image_height * scale);
            ([x + (width - fitted_width) / 2.0, y + (height - fitted_height) / 2.0, fitted_width, fitted_height], Crop::default())
        }
        ImageFit::Cover => {
            let (image_aspect, frame_aspect) = (image_width / image_height, width / height);
            let crop = if image_aspect > frame_aspect {
                let trim = (1.0 - frame_aspect / image_aspect) / 2.0;
                Crop { left: trim, right: trim, ..Default::default() }
            } else {
                let trim = (1.0 - image_aspect / frame_aspect) / 2.0;
                Crop { top: trim, bottom: trim, ..Default::default() }
            };
            (frame, crop)
        }
    };
    Ok(Element::Picture { id, x: bounds[0], y: bounds[1], width: bounds[2], height: bounds[3], base64: base64.into(), mime_type: mime_type.into(), alt: alt.into(), crop, visual: None, svg: None })
}

fn table(id: String, frame: [f64; 4], rows: &[Vec<String>], tokens: &DesignTokens, theme: &Theme) -> Result<Element> {
    validate_rows(rows)?;
    let scale = tokens.type_scale;
    let dense = rows.len() > 7 || rows[0].len() > 5;
    let sizes: Vec<f64> = if dense || scale.body <= scale.caption { vec![scale.caption] } else { vec![scale.body, scale.caption] };
    for size in sizes {
        let compact = (rows.len() as f64 * (size * 2.4).max(40.0)).min(frame[3]).floor();
        for height in [compact, frame[3]] {
            let element = Element::Table { id: id.clone(), x: frame[0], y: frame[1], width: frame[2], height, rows: rows.to_vec(), font_size: size, format: Default::default() };
            if crate::layout::first_overflow(std::slice::from_ref(&element), theme)?.is_none() { return Ok(element); }
        }
    }
    Err(Error::Invalid(format!("table {id} does not fit at {}px; shorten cells, reduce rows or split the table", scale.caption)))
}

fn part(block: &CompositionBlock, frame: [f64; 4]) -> Result<PartSpec> {
    let mut spec = match block {
        CompositionBlock::Steps { items } => PartSpec { version: 1, preset: "list-horizontal/labeled".into(), title: String::new(), subtitle: String::new(), data: PartData::Items { items: items.clone(), center: String::new() }, layout: None },
        CompositionBlock::Comparison { rows, columns, cells, corner_label } => PartSpec { version: 1, preset: "matrix/balanced".into(), title: String::new(), subtitle: String::new(), data: PartData::Matrix { rows: rows.clone(), columns: columns.clone(), cells: cells.clone(), corner_label: corner_label.clone() }, layout: None },
        CompositionBlock::Graph { input } => {
            let graph = crate::graphs::layout_graph(input)?;
            PartSpec { version: 1, preset: "diagram/custom".into(), title: graph.title.clone(), subtitle: graph.subtitle.clone(), data: PartData::Diagram { graph }, layout: None }
        }
        CompositionBlock::Part { spec } => {
            if spec.layout.is_some() { return Err(Error::Invalid("composition supplies part layout; omit explicit part coordinates".into())); }
            spec.clone()
        }
        other => return Err(Error::Invalid(format!("{} is not a managed part block", other.kind_name()))),
    };
    let [x, y, width, height] = frame;
    spec.layout = Some(PartLayout { x, y, width, height, show_title: !spec.title.is_empty(), fit: PartFit::Stretch });
    let advice = crate::parts::aspect::fit_in(&crate::parts::aspect::profile(&spec)?, width, height);
    if advice["fit"] == "contain" { if let Some(layout) = spec.layout.as_mut() { layout.fit = PartFit::Contain; } }
    Ok(spec)
}

fn marker(id: &str, slot: &PlacedSlot, tokens: &DesignTokens, mirror: bool) -> Result<Vec<Element>> {
    let [x, y, width, height] = slot.frame;
    let element_id = format!("{id}-{}", slot.id);
    let role = slot.id.split('-').next().unwrap_or_default();
    let shape = |preset: &str, bounds: [f64; 4], label: &str, size: f64| -> Element {
        let look = Look { size, color: tokens.colors.on_accent, bold: true, major: false, align: TextAlign::Center, vertical: VerticalAlign::Middle };
        let paragraphs = if label.is_empty() { Vec::new() } else { lines(label) };
        Element::Shape { id: element_id.clone(), x: bounds[0], y: bounds[1], width: bounds[2], height: bounds[3], preset: preset.into(), fill: tokens.colors.accent.into(), stroke: tokens.colors.accent.into(), stroke_width: 0.0, rotation: 0.0,
            text: label.into(), font_size: size, color: look.color.into(), bold: true, format: text_format(paragraphs, look), visual: None }
    };
    Ok(match role {
        "connector" | "arrow" => {
            let arrow_width = (width * 0.75).min(48.0).floor();
            let arrow_height = (arrow_width * 0.75).floor();
            // Mirroring reverses the horizontal slot order, so arrows must point right to left.
            let preset = if mirror { "leftArrow" } else { "rightArrow" };
            vec![shape(preset, [x + ((width - arrow_width) / 2.0).round(), y + ((height - arrow_height) / 2.0).round(), arrow_width, arrow_height], "", tokens.type_scale.caption)]
        }
        "marker" | "number" => {
            let diameter = width.min(height).min(48.0).floor();
            let number = slot.id.rsplit('-').next().filter(|value| value.bytes().all(|byte| byte.is_ascii_digit())).unwrap_or("1");
            vec![shape("ellipse", [x + ((width - diameter) / 2.0).round(), y, diameter, diameter], number, (diameter * 0.45 / 2.0).round() * 2.0)]
        }
        "axis" => vec![rectangle(element_id.clone(), [x, (y + height / 2.0 - 2.0).round(), width, 4.0], tokens.colors.accent)],
        _ => Vec::new(),
    })
}

fn advice(fallback: Option<&str>) -> String {
    fallback.map_or_else(|| "split the content or choose another pattern".into(), |pattern| format!("use fallback pattern {pattern}, split the content or choose another pattern"))
}

fn frame_of(frame: Frame) -> [f64; 4] { [frame.x, frame.y, frame.width, frame.height] }

fn infer_count(choice: &PatternChoice, traits: &Traits, slots: &BTreeMap<String, CompositionBlock>, body: Frame, tokens: &DesignTokens) -> Option<usize> {
    let Some([minimum, maximum, default]) = traits.count else { return choice.count };
    if choice.count.is_some() { return choice.count; }
    let provided: BTreeSet<&str> = slots.keys().map(String::as_str).collect();
    for count in minimum..=maximum {
        let options = ResolveOptions { mirror: choice.mirror, message_band: choice.message_band, count: Some(count), ..Default::default() };
        let Ok(placed) = crate::layout_patterns::placement(&choice.id, tokens.canvas, Some(body), Some(tokens.gaps), &options) else { continue };
        let content: BTreeSet<&str> = placed.iter().filter(|slot| slot.kind != "marker").map(|slot| slot.id.as_str()).collect();
        if content == provided { return Some(count); }
    }
    Some(default)
}

/// Builds the slide content for a pattern composition and verifies that every text frame fits.
pub(crate) fn build(tokens: &DesignTokens, theme: &Theme, id: &str, chrome: &Chrome<'_>, choice: &PatternChoice, slots: &BTreeMap<String, CompositionBlock>) -> Result<Built> {
    required(chrome.title, 120, "a pattern composition title")?;
    valid_text(chrome.footer, 200)?; valid_text(chrome.page, 16)?;
    if slots.len() > 32 { return Err(Error::Limit("pattern composition accepts at most 32 slots".into())); }
    let traits = crate::layout_patterns::traits(&choice.id)?;
    let body = if traits.full_page { tokens.frames.full_page } else { tokens.frames.body };
    let options = ResolveOptions { mirror: choice.mirror, message_band: choice.message_band, count: infer_count(choice, &traits, slots, body, tokens), ..Default::default() };
    let placed = crate::layout_patterns::placement(&choice.id, tokens.canvas, Some(body), Some(tokens.gaps), &options)?;
    let mut blocks = slots.clone();
    if traits.full_page && placed.iter().any(|slot| slot.id == "title") && !blocks.contains_key("title") {
        blocks.insert("title".into(), CompositionBlock::Statement { text: chrome.title.into() });
    }
    let names = || placed.iter().filter(|slot| slot.kind != "marker").map(|slot| slot.id.as_str()).collect::<Vec<_>>().join(", ");
    for key in blocks.keys() {
        match placed.iter().find(|slot| &slot.id == key) {
            None => return Err(Error::Invalid(format!("{} has no slot {key}; its slots are {}", choice.id, names()))),
            Some(slot) if slot.kind == "marker" => return Err(Error::Invalid(format!("{key} is a marker slot; arrows, numbers and axes are drawn automatically"))),
            _ => {}
        }
    }
    let quoted = blocks.values().find_map(|block| match block { CompositionBlock::Quote { attribution, .. } if !attribution.trim().is_empty() => Some(attribution.clone()), _ => None });
    let mut attribution_slot = false;
    if placed.iter().any(|slot| slot.id == "attribution") {
        match (blocks.contains_key("attribution"), quoted) {
            (true, Some(_)) => return Err(Error::Invalid("quote attribution is given twice; use the quote attribution or the attribution slot".into())),
            (false, Some(attribution)) => { blocks.insert("attribution".into(), CompositionBlock::Label { text: attribution }); attribution_slot = true; }
            _ => {}
        }
    }
    let missing: Vec<&str> = placed.iter().filter(|slot| slot.kind != "marker" && !blocks.contains_key(&slot.id)).map(|slot| slot.id.as_str()).collect();
    if !missing.is_empty() { return Err(Error::Invalid(format!("{} requires slots {}; markers are drawn automatically", choice.id, missing.join(", ")))); }
    for slot in &placed {
        let Some(block) = blocks.get(&slot.id) else { continue };
        if !block.content().iter().any(|content| slot.accepts.contains(content)) {
            return Err(Error::Invalid(format!("slot {} is a {} slot accepting {}; a {} block provides {}", slot.id, slot.kind, slot.accepts.join(", "), block.kind_name(), block.content().join(", "))));
        }
    }
    let issues: Vec<String> = placed.iter().filter(|slot| !slot.fits()).map(|slot| format!("{} is {}x{}px but a {} slot needs {}x{}px", slot.id, slot.frame[2], slot.frame[3], slot.kind, slot.min[0], slot.min[1])).collect();
    if !issues.is_empty() { return Err(Error::Invalid(format!("{} does not fit the design body ({}); {}", choice.id, issues.join("; "), advice(traits.fallback)))); }
    let scale = tokens.type_scale;
    let colors = tokens.colors;
    let mut elements = Vec::new();
    let mut parts = Vec::new();
    if !traits.full_page {
        elements.push(text(format!("{id}-title"), frame_of(tokens.frames.title), lines(chrome.title), Look { size: scale.title, color: colors.ink, bold: true, major: true, align: TextAlign::Left, vertical: VerticalAlign::Top })?);
    }
    let body_frame = frame_of(body);
    for slot in &placed {
        if slot.kind == "marker" { elements.extend(marker(id, slot, tokens, choice.mirror)?); continue; }
        let element_id = format!("{id}-{}", slot.id);
        let frame = slot.frame;
        let padding = if slot.kind == "tile" { tokens.card.band_padding } else { tokens.card.padding };
        let plain = Look { size: scale.body, color: colors.ink, bold: false, major: false, align: TextAlign::Left, vertical: VerticalAlign::Top };
        match &blocks[&slot.id] {
            CompositionBlock::Cards { items, columns } => {
                if items.is_empty() || items.len() > 6 { return Err(Error::Limit("composition cards require 1-6 items".into())); }
                if slot.kind == "card" && items.len() != 1 { return Err(Error::Invalid(format!("card slot {} holds one card; use a panel slot or one card per slot", slot.id))); }
                let columns = columns.unwrap_or(items.len().min(3));
                if !(1..=3).contains(&columns) || columns > items.len() { return Err(Error::Invalid("composition cards require 1-3 columns, not more than the item count".into())); }
                let rows = items.len().div_ceil(columns);
                let gap = tokens.gaps.peer;
                let card_width = ((frame[2] - gap * (columns - 1) as f64) / columns as f64).floor();
                let card_height = ((frame[3] - gap * (rows - 1) as f64) / rows as f64).floor();
                for (index, item) in items.iter().enumerate() {
                    let item_id = if items.len() == 1 { element_id.clone() } else { format!("{element_id}-c{}", index + 1) };
                    let bounds = [frame[0] + (index % columns) as f64 * (card_width + gap), frame[1] + (index / columns) as f64 * (card_height + gap), card_width, card_height];
                    elements.extend(card(item_id, bounds, item_paragraphs(item)?, padding, tokens)?);
                }
            }
            CompositionBlock::Callout { text: value } => { required(value, 400, "a callout")?; elements.extend(band(element_id, frame, lines(value), tokens)?); }
            CompositionBlock::Text { paragraphs } => {
                let paragraphs = tagged(paragraphs)?;
                if crate::rich_text::plain_text(&paragraphs).trim().is_empty() { return Err(Error::Invalid(format!("text slot {} requires text", slot.id))); }
                if matches!(slot.kind, "card" | "panel") { elements.extend(card(element_id, frame, paragraphs, padding, tokens)?); }
                else { elements.push(text(element_id, frame, paragraphs, plain)?); }
            }
            block @ (CompositionBlock::Steps { .. } | CompositionBlock::Comparison { .. } | CompositionBlock::Graph { .. } | CompositionBlock::Part { .. }) => parts.push((element_id, part(block, frame)?)),
            CompositionBlock::Statement { text: value } => {
                required(value, 400, "a statement")?;
                let centered = frame[2] < body_frame[2] - 1.0 && ((frame[0] + frame[2] / 2.0) - (body_frame[0] + body_frame[2] / 2.0)).abs() < 1.0;
                elements.push(text(element_id, frame, lines(value), Look { size: scale.statement, bold: true, major: true, align: if centered { TextAlign::Center } else { TextAlign::Left }, vertical: VerticalAlign::Middle, ..plain })?);
            }
            CompositionBlock::Quote { text: value, attribution } => {
                required(value, 400, "a quotation")?; valid_text(attribution, 200)?;
                let inline = !attribution_slot && !attribution.trim().is_empty();
                if !inline && !attribution_slot && !blocks.contains_key("attribution") { return Err(Error::Invalid("quotations require an attribution; set quote.attribution or fill the attribution slot".into())); }
                let indent = 24.0;
                let attribution_height = if inline { (scale.caption * 1.15 * 2.0 + 4.0).ceil() } else { 0.0 };
                elements.push(rectangle(format!("{element_id}-rule"), [frame[0], frame[1], 6.0, frame[3]], colors.accent));
                elements.push(text(element_id.clone(), [frame[0] + indent, frame[1], frame[2] - indent, frame[3] - attribution_height], lines(value), Look { size: scale.statement, major: true, vertical: VerticalAlign::Middle, ..plain })?);
                if inline { elements.push(text(format!("{element_id}-attribution"), [frame[0] + indent, frame[1] + frame[3] - attribution_height, frame[2] - indent, attribution_height], lines(attribution), Look { size: scale.caption, color: colors.muted, ..plain })?); }
            }
            CompositionBlock::Metric { value, label, detail } => elements.extend(metric(&element_id, frame, padding, value, label, detail, tokens, theme)?),
            CompositionBlock::Label { text: value } => {
                required(value, 200, "a label")?;
                match slot.kind {
                    "band" => elements.extend(band(element_id, frame, lines(value), tokens)?),
                    "tile" => {
                        elements.push(rectangle(format!("{element_id}-tile"), frame, tokens.card.fill));
                        elements.push(rectangle(format!("{element_id}-rule"), [frame[0], frame[1], frame[2], tokens.card.rule], colors.accent));
                        let inset = tokens.card.band_padding;
                        elements.push(text(element_id, [frame[0] + inset, frame[1] + inset + tokens.card.rule, frame[2] - inset * 2.0, frame[3] - inset * 2.0 - tokens.card.rule], lines(value), Look { bold: true, align: TextAlign::Center, vertical: VerticalAlign::Middle, ..plain })?);
                    }
                    _ => elements.push(text(element_id, frame, lines(value), Look { size: scale.caption, color: colors.muted, vertical: VerticalAlign::Middle, ..plain })?),
                }
            }
            CompositionBlock::Image { base64, mime_type, alt, fit } => elements.push(picture(element_id, frame, base64, mime_type, alt, *fit)?),
            CompositionBlock::Table { rows } => elements.push(table(element_id, frame, rows, tokens, theme)?),
            CompositionBlock::Chart { chart } => {
                crate::model::chart_format::validate(chart.kind, &chart.categories, &chart.series, &chart.options)?;
                elements.push(Element::Chart { id: element_id, x: frame[0], y: frame[1], width: frame[2], height: frame[3], kind: chart.kind, categories: chart.categories.clone(), series: chart.series.clone(), options: chart.options.clone() });
            }
        }
    }
    let footer = frame_of(tokens.frames.footer);
    let page_width = if chrome.page.is_empty() { 0.0 } else { 96.0 };
    let caption = Look { size: scale.caption, color: colors.muted, bold: false, major: false, align: TextAlign::Left, vertical: VerticalAlign::Middle };
    if !chrome.footer.is_empty() { elements.push(text(format!("{id}-footer"), [footer[0], footer[1], footer[2] - page_width - if page_width > 0.0 { 16.0 } else { 0.0 }, footer[3]], lines(chrome.footer), caption)?); }
    if !chrome.page.is_empty() { elements.push(text(format!("{id}-page"), [footer[0] + footer[2] - page_width, footer[1], page_width, footer[3]], lines(chrome.page), Look { align: TextAlign::Right, ..caption })?); }
    if let Some(element) = crate::layout::first_overflow(&elements, theme)? {
        return Err(Error::Invalid(format!("composition text overflows {element} at design-token sizes; shorten the text, {}; font sizes are not reduced", advice(traits.fallback))));
    }
    Ok(Built { elements, parts })
}

fn layout_theme(deck: &Deck, layout_id: Option<&str>) -> Theme {
    let Some(design) = &deck.design else { return Theme::default() };
    let layout = layout_id.and_then(|id| design.layouts.iter().find(|layout| layout.id == id)).or_else(|| design.layouts.first());
    layout.map_or_else(|| design.theme.clone(), |layout| crate::design::master_theme(design, &layout.master_id).clone())
}

/// `compose_slide` in pattern mode: fills a new empty slide and registers its managed parts.
pub(crate) fn compose(deck: &mut Deck, parts: &mut Vec<crate::parts::state::PartInstance>, slide_id: &str, id: &str, spec: &CompositionSpec, choice: &PatternChoice, native_guard: &mut crate::parts::state::NativeRegenerationGuard<'_>) -> Result<()> {
    if !spec.blocks.is_empty() { return Err(Error::Invalid("pattern compositions place content in slots; omit blocks".into())); }
    if !spec.subtitle.is_empty() { return Err(Error::Invalid("pattern compositions have no subtitle; use a text slot or message_band".into())); }
    if spec.style != CompositionStyle::default() { return Err(Error::Invalid("pattern compositions take sizes and colors from design tokens; omit style".into())); }
    let index = deck.slides.iter().position(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("composition slide not found".into()))?;
    let slide = &deck.slides[index];
    if !slide.elements.is_empty() || slide.native_source_id.is_some() || parts.iter().any(|part| part.slide_id == slide_id) { return Err(Error::Conflict("compose_slide requires a new empty slide; existing or native content is never replaced".into())); }
    let tokens = crate::design_tokens::resolve(deck)?;
    let layout_id = tokens.layout_id.clone().or_else(|| slide.layout_id.clone());
    let theme = layout_theme(deck, layout_id.as_deref());
    let built = build(&tokens, &theme, id, &Chrome { title: &spec.title, footer: &spec.footer, page: "" }, choice, &spec.slots)?;
    let slide = &mut deck.slides[index];
    slide.title = spec.title.clone();
    slide.layout_id = layout_id;
    if tokens.master_graphics { slide.inherit_background = true; }
    slide.hide_master_graphics = !tokens.master_graphics;
    slide.elements = built.elements;
    for (part_id, part) in built.parts { crate::parts::state::change_in_deck(deck, parts, slide_id, &part_id, &part, false, native_guard)?; }
    let mut measured = deck.clone(); measured.slides = vec![deck.slides[index].clone()];
    let report = crate::layout::measure_layout(&measured)?;
    if let Some(measurement) = report.measurements.iter().find(|measurement| measurement.slide_id == slide_id && measurement.overflow) {
        return Err(Error::Invalid(format!("composition text overflows {}: reduce content or split the slide; font sizes are not reduced", measurement.element_id)));
    }
    Ok(())
}
