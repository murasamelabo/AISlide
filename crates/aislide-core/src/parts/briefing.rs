use super::{count, Drawing, PartData, PartSpec};
use crate::{design::Theme, graphs::GraphIcon, model::{Bullet, Crop, Element, TextAlign, TextFormat, TextPadding, VerticalAlign, valid_color, valid_text}, rich_text::{RichParagraph, RichRun, RunStyle, Spacing}, visual::{ShapeAdjustment, VisualStyle}, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub(super) struct Preset { pub id: &'static str, pub kind: &'static str, pub name: &'static str, pub use_when: &'static str, pub avoid_when: &'static str }

pub(super) const PRESETS: &[Preset] = &[
    Preset { id: "list-horizontal/icon-cards", kind: "icon_cards", name: "Icon feature cards",
        use_when: "2-6 equal-status concepts that each need a meaningful icon, a heading and a short explanation, optional caption, check points or footer tag. Fixed typography; overflow rejects.",
        avoid_when: "Ordered stages, measured comparisons or two paired alternatives; use flow/cards, a chart or contrast/panels instead. Do not add icons that only decorate." },
    Preset { id: "list/icon-rows", kind: "icon_rows", name: "Icon explanation rows",
        use_when: "2-6 stacked statements, risks or capabilities read top to bottom, each with a meaningful icon, bold heading and supporting detail.",
        avoid_when: "Sequences with handoffs, dense tables, or topics without supporting detail; use flow/cards, a table or list/rows instead." },
    Preset { id: "before-after/shift", kind: "shift_rows", name: "Role and focus shift rows",
        use_when: "2-5 explicit from-to changes such as role, metric or operating-model shifts, optionally with column headings and one explanation per row.",
        avoid_when: "Two broad alternatives explained through paired statements (contrast/panels) or changes without a real from-to relationship." },
    Preset { id: "flow/cards", kind: "step_cards", name: "Step cards",
        use_when: "2-4 ordered steps with a numbered step pill, heading, explanation, optional supplied image or icon, check points and an outcome box.",
        avoid_when: "Unordered peers (list-horizontal/icon-cards) or more than four stages; split long procedures across slides." },
    Preset { id: "list/agenda", kind: "agenda", name: "Numbered agenda",
        use_when: "2-7 agenda or chapter entries with a number badge, heading, optional one-line detail and optional duration or page label.",
        avoid_when: "Content slides or process steps with relationships; use the matching list, flow or diagram preset." },
];

const GAP: f64 = 16.0;
const PAD: f64 = 20.0;
const LABEL: f64 = 18.0;
const BODY: f64 = 16.0;
const SMALL: f64 = 13.0;
const FIT_MARGIN: f64 = 28.0;
const ROW_PITCH: f64 = 112.0;

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartMessage {
    pub text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub color: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IconCard {
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub caption: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub detail: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub points: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub icon: Option<GraphIcon>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub accent: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IconRow {
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub icon: Option<GraphIcon>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub accent: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShiftRow {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub caption: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StepCard {
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub detail: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub points: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub image: Option<GraphIcon>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub icon: Option<GraphIcon>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgendaItem {
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub detail: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub meta: String,
}

pub(super) fn kind(data: &PartData) -> Option<&'static str> {
    Some(match data {
        PartData::IconCards { .. } => "icon_cards",
        PartData::IconRows { .. } => "icon_rows",
        PartData::ShiftRows { .. } => "shift_rows",
        PartData::StepCards { .. } => "step_cards",
        PartData::Agenda { .. } => "agenda",
        _ => return None,
    })
}

fn field(value: &str, maximum: usize, name: &str, required: bool) -> Result<()> {
    if value.chars().count() > maximum { return Err(Error::Limit(format!("{name} exceeds {maximum} Unicode scalars"))); }
    valid_text(value, maximum)?;
    if value.contains('\r') { return Err(Error::Invalid(format!("{name} must use LF line breaks"))); }
    if value.split('\n').count() > 3 { return Err(Error::Invalid(format!("{name} allows at most three lines"))); }
    if required && value.trim().is_empty() { return Err(Error::Invalid(format!("{name} is required"))); }
    Ok(())
}

fn colors(values: &[&Option<String>]) -> Result<()> {
    for value in values.iter().copied().flatten() { valid_color(value)?; }
    Ok(())
}

fn body_size(value: Option<f64>) -> Result<()> {
    if value.is_some_and(|size| !size.is_finite() || !(14.0..=22.0).contains(&size)) { return Err(Error::Invalid("body_size must be 14-22px".into())); }
    Ok(())
}

fn picture(value: &Option<GraphIcon>) -> Result<()> {
    if let Some(value) = value { valid_text(&value.alt, 500)?; }
    Ok(())
}

fn points(values: &[String], name: &str) -> Result<()> {
    if values.len() > 4 { return Err(Error::Limit(format!("{name} allows at most four points"))); }
    for value in values { field(value, 80, name, true)?; }
    Ok(())
}

fn message(value: &Option<PartMessage>) -> Result<()> {
    if let Some(value) = value {
        field(&value.text, 120, "message text", true)?;
        field(&value.detail, 160, "message detail", false)?;
        colors(&[&value.fill, &value.color])?;
    }
    Ok(())
}

pub(super) fn validate(data: &PartData) -> Result<()> {
    match data {
        PartData::IconCards { cards, columns, numbered: _, body_size: size, message: band } => {
            count(cards.len(), 2, 6)?;
            let columns = columns.unwrap_or(if cards.len() <= 4 { cards.len() } else { 3 });
            if !(2..=4).contains(&columns) || columns > cards.len() || cards.len().div_ceil(columns) > 2 {
                return Err(Error::Invalid("icon cards require 2-4 columns, at most the card count and at most two rows".into()));
            }
            for card in cards {
                field(&card.label, 48, "card label", true)?; field(&card.caption, 64, "card caption", false)?;
                field(&card.detail, 240, "card detail", false)?; field(&card.tag, 40, "card tag", false)?;
                points(&card.points, "card point")?; picture(&card.icon)?; colors(&[&card.accent])?;
            }
            body_size(*size)?; message(band)?;
        }
        PartData::IconRows { rows, boxed: _, body_size: size, message: band } => {
            count(rows.len(), 2, 6)?;
            for row in rows { field(&row.label, 60, "row label", true)?; field(&row.detail, 200, "row detail", false)?; picture(&row.icon)?; colors(&[&row.accent])?; }
            body_size(*size)?; message(band)?;
        }
        PartData::ShiftRows { from_label, to_label, rows, accent, body_size: size, message: band } => {
            count(rows.len(), 2, 5)?;
            field(from_label, 24, "from_label", false)?; field(to_label, 24, "to_label", false)?;
            for row in rows { field(&row.from, 48, "shift from", true)?; field(&row.to, 48, "shift to", true)?; field(&row.caption, 64, "shift caption", false)?; field(&row.detail, 200, "shift detail", false)?; }
            colors(&[accent])?; body_size(*size)?; message(band)?;
        }
        PartData::StepCards { steps, step_label, accent, body_size: size, message: band } => {
            count(steps.len(), 2, 4)?;
            field(step_label, 12, "step_label", false)?;
            if step_label.contains('\n') { return Err(Error::Invalid("step_label must be one line".into())); }
            for step in steps {
                field(&step.label, 48, "step label", true)?; field(&step.detail, 240, "step detail", false)?; field(&step.outcome, 64, "step outcome", false)?;
                points(&step.points, "step point")?; picture(&step.image)?; picture(&step.icon)?;
            }
            colors(&[accent])?; body_size(*size)?; message(band)?;
        }
        PartData::Agenda { items, accent } => {
            count(items.len(), 2, 7)?;
            for item in items {
                field(&item.label, 60, "agenda label", true)?; field(&item.detail, 120, "agenda detail", false)?; field(&item.meta, 16, "agenda meta", false)?;
                if item.meta.contains('\n') { return Err(Error::Invalid("agenda meta must be one line".into())); }
            }
            colors(&[accent])?;
        }
        _ => return Err(Error::Invalid("briefing part data required".into())),
    }
    Ok(())
}

pub(super) fn create(id: &str, spec: &PartSpec, theme: &Theme) -> Result<Element> {
    use sha2::{Digest, Sha256};
    let (x, y, width, height, show_title) = spec.layout.as_ref().map_or((64.0, 144.0, 1152.0, 512.0, true), |layout| (layout.x, layout.y, layout.width, layout.height, layout.show_title));
    let prefix = format!("{id}-{}", &format!("{:x}", Sha256::digest(crate::canonical::bytes(spec)?))[..10]);
    let top = if show_title { 88.0 } else { 0.0 };
    if width < 320.0 || height - top < 120.0 { return Err(Error::Invalid(format!("{} requires a body region of at least 320x120px", spec.preset))); }
    let build = |shrink: f64| -> Result<(Element, Vec<String>)> {
        let mut drawing = Drawing::new(&prefix);
        if show_title {
            drawing.text(&spec.title, [0.0, 0.0, width, 40.0], 28.0, "@dk1", true, TextAlign::Left);
            drawing.text(&spec.subtitle, [0.0, 44.0, width, 26.0], 16.0, "@dk2", false, TextAlign::Left);
        }
        let flexible = render(&mut drawing, &spec.data, [0.0, top, width, height - top], theme, shrink)?;
        Ok((Element::Group { visual: None, id: id.into(), x, y, width, height, view_width: width, view_height: height, children: drawing.elements }, flexible))
    };
    let (mut result, flexible) = build(0.0)?;
    if !flexible.is_empty() {
        let shrink = (super::measured_slack(&result, theme, &flexible)? - FIT_MARGIN).floor();
        if shrink >= 8.0 { result = build(shrink)?.0; }
    }
    crate::model::validate_elements(std::slice::from_ref(&result), (4096.0, 4096.0), 0, &mut BTreeSet::new(), &mut 0, &mut 0)?;
    if let Some((text, missing)) = super::layout_overflow(&result, theme)? {
        return Err(Error::Invalid(if missing { format!("{} text \"{text}\" uses glyphs missing from the selected font; choose an installed font that covers it", spec.preset) }
            else { format!("{} text \"{text}\" does not fit at its fixed font size; shorten the text, enlarge the part frame or split the slide", spec.preset) }));
    }
    Ok(result)
}

struct Drawn { bottom: f64, flexible: Vec<String> }

fn render(drawing: &mut Drawing, data: &PartData, body: [f64; 4], theme: &Theme, shrink: f64) -> Result<Vec<String>> {
    let band = match data {
        PartData::IconCards { message, .. } | PartData::IconRows { message, .. } | PartData::ShiftRows { message, .. } | PartData::StepCards { message, .. } => message.as_ref(),
        _ => None,
    };
    let band_height = band.map(message_height);
    let [left, top, width, height] = body;
    let content = [left, top, width, height - band_height.map_or(0.0, |value| value + GAP)];
    if content[3] < 96.0 { return Err(too_small("the message band and body")); }
    let drawn = match data {
        PartData::IconCards { cards, columns, numbered, body_size, .. } => icon_cards(drawing, cards, *columns, *numbered, body_size.unwrap_or(BODY), content, theme, shrink)?,
        PartData::IconRows { rows, boxed, body_size, .. } => icon_rows(drawing, rows, *boxed, body_size.unwrap_or(BODY), content, theme)?,
        PartData::ShiftRows { from_label, to_label, rows, accent, body_size, .. } => shift_rows(drawing, [from_label, to_label], rows, accent.as_deref().unwrap_or("@accent1"), body_size.unwrap_or(BODY), content, theme)?,
        PartData::StepCards { steps, step_label, accent, body_size, .. } => step_cards(drawing, steps, if step_label.is_empty() { "STEP" } else { step_label }, accent.as_deref().unwrap_or("@accent1"), body_size.unwrap_or(BODY), content, theme, shrink)?,
        PartData::Agenda { items, accent } => agenda(drawing, items, accent.as_deref().unwrap_or("@dk1"), content, theme)?,
        _ => return Err(Error::Invalid("briefing part data required".into())),
    };
    if let (Some(band), Some(band_height)) = (band, band_height) { message_band(drawing, band, [left, drawn.bottom + GAP, width, band_height]); }
    Ok(drawn.flexible)
}

fn mix(color: &str, theme: &Theme, white: f64) -> String {
    let resolved = crate::design::resolve_color(color, Some(theme));
    let value = u32::from_str_radix(&resolved, 16).unwrap_or(0x80_80_80);
    let channel = |shift: u32| { let current = f64::from((value >> shift) & 0xFF); (current + (255.0 - current) * white).round() as u8 };
    format!("{:02X}{:02X}{:02X}", channel(16), channel(8), channel(0))
}

fn pill_width(text: &str, size: f64) -> f64 {
    text.chars().map(|character| if character.is_ascii() { size * 0.64 } else { size * 1.1 }).sum::<f64>() + 32.0
}

#[derive(Default)]
struct Rich(Vec<RichParagraph>);

impl Rich {
    fn add(&mut self, value: &str, size: f64, color: &str, bold: bool, after: u32) -> &mut Self { self.lines(value, size, color, bold, after, false) }
    fn bullet(&mut self, value: &str, size: f64, color: &str, after: u32) -> &mut Self { self.lines(value, size, color, false, after, true) }
    fn lines(&mut self, value: &str, size: f64, color: &str, bold: bool, after: u32, bullet: bool) -> &mut Self {
        if value.is_empty() { return self; }
        let lines: Vec<_> = value.split('\n').collect();
        for (index, line) in lines.iter().enumerate() {
            self.0.push(RichParagraph {
                runs: vec![RichRun { text: (*line).into(), style: RunStyle { bold: Some(bold), font_size: Some(size), color: Some(color.into()), font_family: Some("@minor".into()), ..Default::default() }, field: None }],
                bullet: bullet.then_some(Bullet::Bullet),
                margin_left: bullet.then_some(171_450), indent: bullet.then_some(-171_450),
                space_after: (index + 1 == lines.len() && after > 0).then_some(Spacing::Points(after)),
                ..Default::default()
            });
        }
        self
    }
    fn finish(&mut self) -> Vec<RichParagraph> {
        let mut paragraphs = std::mem::take(&mut self.0);
        if let Some(last) = paragraphs.last_mut() { last.space_after = None; }
        paragraphs
    }
}

fn format(alignment: TextAlign, vertical: VerticalAlign, padding: Option<[f64; 4]>, paragraphs: Vec<RichParagraph>) -> TextFormat {
    TextFormat { alignment, vertical, font_family: Some("@minor".into()), padding: padding.map(|[left, top, right, bottom]| TextPadding { left, top, right, bottom }), paragraphs, ..TextFormat::default() }
}

fn text(drawing: &mut Drawing, bounds: [f64; 4], paragraphs: Vec<RichParagraph>, alignment: TextAlign, vertical: VerticalAlign) -> Option<String> {
    if paragraphs.is_empty() { return None; }
    let [x, y, width, height] = bounds;
    let id = drawing.id();
    let text = crate::rich_text::plain_text(&paragraphs);
    let (font_size, color, bold) = frame_style(&paragraphs);
    drawing.elements.push(Element::Text { visual: None, id: id.clone(), x, y, width, height, text, font_size, color, bold, format: format(alignment, vertical, None, paragraphs) });
    Some(id)
}

// Native reopen derives frame styles from the first run; matching them keeps later part updates writable.
fn frame_style(paragraphs: &[RichParagraph]) -> (f64, String, bool) {
    let style = paragraphs.first().and_then(|paragraph| paragraph.runs.first()).map(|run| &run.style);
    (style.and_then(|style| style.font_size).unwrap_or(BODY), style.and_then(|style| style.color.clone()).unwrap_or_else(|| "@dk1".into()), style.and_then(|style| style.bold).unwrap_or(false))
}

struct Fill<'a> { preset: &'a str, fill: &'a str, stroke: &'a str, stroke_width: f64, radius: Option<i32> }

impl<'a> Fill<'a> {
    fn solid(preset: &'a str, fill: &'a str) -> Self { Self { preset, fill, stroke: fill, stroke_width: 0.75, radius: None } }
    fn rounded(mut self, radius: i32) -> Self { self.radius = Some(radius); self }
}

fn shape(drawing: &mut Drawing, style: Fill<'_>, bounds: [f64; 4], content: Option<(Vec<RichParagraph>, TextAlign, [f64; 4])>) {
    let [x, y, width, height] = bounds;
    let id = drawing.id();
    let (text, font_size, color, bold, format) = match content {
        Some((paragraphs, alignment, padding)) => { let (size, color, bold) = frame_style(&paragraphs); (crate::rich_text::plain_text(&paragraphs), size, color, bold, format(alignment, VerticalAlign::Middle, Some(padding), paragraphs)) }
        None => (String::new(), BODY, "@dk1".into(), false, TextFormat::default()),
    };
    let visual = style.radius.map(|value| VisualStyle { adjustments: vec![ShapeAdjustment { name: "adj".into(), value }], ..Default::default() });
    drawing.elements.push(Element::Shape { visual, id, x, y, width, height, preset: style.preset.into(), fill: style.fill.into(), stroke: style.stroke.into(), stroke_width: style.stroke_width, rotation: 0.0, text, font_size, color, bold, format });
}

fn label(value: &str, size: f64, color: &str, bold: bool) -> Vec<RichParagraph> { Rich::default().add(value, size, color, bold, 0).finish() }

fn icon(drawing: &mut Drawing, icon: &GraphIcon, center: [f64; 2], size: f64) -> Result<()> {
    let mut picture = crate::media::create_picture(&drawing.id(), icon.base64.clone(), &icon.mime_type, &icon.alt)?;
    if let Element::Picture { x, y, width, height, .. } = &mut picture {
        let scale = size / width.max(*height);
        *width *= scale; *height *= scale;
        *x = center[0] - *width / 2.0; *y = center[1] - *height / 2.0;
    }
    drawing.elements.push(picture);
    Ok(())
}

fn badge(drawing: &mut Drawing, value: &GraphIcon, origin: [f64; 2], diameter: f64, fill: &str) -> Result<()> {
    shape(drawing, Fill::solid("ellipse", fill), [origin[0], origin[1], diameter, diameter], None);
    icon(drawing, value, [origin[0] + diameter / 2.0, origin[1] + diameter / 2.0], diameter * 0.58)
}

fn cover(drawing: &mut Drawing, image: &GraphIcon, bounds: [f64; 4]) -> Result<()> {
    let info = crate::media::inspect_raster(&image.base64, &image.mime_type)?;
    let mut picture = crate::media::create_picture(&drawing.id(), image.base64.clone(), &image.mime_type, &image.alt)?;
    if let Element::Picture { x, y, width, height, crop, .. } = &mut picture {
        [*x, *y, *width, *height] = bounds;
        let source = f64::from(info.width) / f64::from(info.height);
        let target = bounds[2] / bounds[3];
        *crop = if source > target { let excess = (1.0 - target / source) / 2.0; Crop { left: excess, right: excess, ..Crop::default() } }
            else { let excess = (1.0 - source / target) / 2.0; Crop { top: excess, bottom: excess, ..Crop::default() } };
    }
    drawing.elements.push(picture);
    Ok(())
}

fn too_small(what: &str) -> Error { Error::Invalid(format!("{what} do not fit the part frame at fixed font sizes; enlarge the frame, reduce items or split the slide")) }

fn message_height(band: &PartMessage) -> f64 {
    let lines = band.text.split('\n').count() as f64;
    let detail_lines = if band.detail.is_empty() { 0.0 } else { band.detail.split('\n').count() as f64 };
    (24.0 + 24.0 * lines + if detail_lines > 0.0 { 4.0 + 19.0 * detail_lines } else { 0.0 }).max(52.0).round()
}

fn message_band(drawing: &mut Drawing, band: &PartMessage, frame: [f64; 4]) {
    let [x, top, width, band_height] = frame;
    let fill = band.fill.as_deref().unwrap_or("@dk1");
    let color = band.color.as_deref().unwrap_or("@lt1");
    let detail = if band.fill.is_some() { color } else { "@lt1" };
    shape(drawing, Fill::solid("rect", fill), [x, top, width, band_height], None);
    text(drawing, [x + 24.0, top + 8.0, width - 48.0, band_height - 16.0], Rich::default().add(&band.text, 17.0, color, true, 200).add(&band.detail, SMALL, detail, false, 0).finish(), TextAlign::Left, VerticalAlign::Middle);
}

fn icon_cards(drawing: &mut Drawing, cards: &[IconCard], columns: Option<usize>, numbered: bool, size: f64, frame: [f64; 4], theme: &Theme, shrink: f64) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let columns = columns.unwrap_or(if cards.len() <= 4 { cards.len() } else { 3 });
    let rows = cards.len().div_ceil(columns);
    let card_width = (width - GAP * (columns - 1) as f64) / columns as f64;
    let natural = (height - GAP * (rows - 1) as f64) / rows as f64;
    if card_width < 180.0 || natural < 120.0 { return Err(too_small("icon cards")); }
    let card_height = (natural - shrink).max(120.0);
    let border = mix("@dk2", theme, 0.80);
    let number_color = mix("@dk2", theme, 0.45);
    let has_icon = cards.iter().any(|card| card.icon.is_some());
    let mut flexible = Vec::new();
    for (index, card) in cards.iter().enumerate() {
        let x = left + (index % columns) as f64 * (card_width + GAP);
        let y = top + (index / columns) as f64 * (card_height + GAP);
        let accent = card.accent.as_deref().unwrap_or("@accent1");
        let tint = mix(accent, theme, 0.88);
        shape(drawing, Fill { preset: "rect", fill: "@lt1", stroke: &border, stroke_width: 1.0, radius: None }, [x, y, card_width, card_height], None);
        shape(drawing, Fill::solid("rect", accent), [x, y, card_width, 4.0], None);
        let mut cursor = y + 22.0;
        if let Some(value) = &card.icon { badge(drawing, value, [x + PAD, cursor], 44.0, &tint)?; }
        if numbered { text(drawing, [x + card_width - PAD - 64.0, cursor + 6.0, 64.0, 30.0], label(&format!("{:02}", index + 1), LABEL, &number_color, true), TextAlign::Right, VerticalAlign::Top); }
        if has_icon { cursor += 58.0; } else if numbered { cursor += 40.0; }
        let mut bottom = y + card_height - PAD;
        if !card.tag.is_empty() {
            bottom -= 30.0;
            shape(drawing, Fill::solid("roundRect", &tint).rounded(50000), [x + PAD, bottom, card_width - PAD * 2.0, 30.0], Some((label(&card.tag, SMALL, accent, true), TextAlign::Center, [10.0, 2.0, 10.0, 2.0])));
            bottom -= 12.0;
        }
        if bottom - cursor < 40.0 { return Err(too_small("icon card texts")); }
        let mut content = Rich::default();
        content.add(&card.label, LABEL, "@dk1", true, 600).add(&card.caption, SMALL, "@dk2", false, 800).add(&card.detail, size, "@dk1", false, 800);
        for point in &card.points { content.bullet(point, size, "@dk1", 700); }
        flexible.extend(text(drawing, [x + PAD, cursor, card_width - PAD * 2.0, bottom - cursor], content.finish(), TextAlign::Left, VerticalAlign::Top));
    }
    Ok(Drawn { bottom: top + rows as f64 * card_height + GAP * (rows - 1) as f64, flexible })
}

fn icon_rows(drawing: &mut Drawing, rows: &[IconRow], boxed: bool, size: f64, frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let gap = if boxed { 12.0 } else { 0.0 };
    let row_height = ((height - gap * (rows.len() - 1) as f64) / rows.len() as f64).min(ROW_PITCH);
    if row_height < 52.0 { return Err(too_small("icon rows")); }
    let diameter = if row_height >= 64.0 { 44.0 } else { 36.0 };
    let inset = if boxed { 18.0 } else { 0.0 };
    let border = mix("@dk2", theme, 0.80);
    let pale = mix("@dk2", theme, 0.94);
    let has_icon = rows.iter().any(|row| row.icon.is_some());
    for (index, row) in rows.iter().enumerate() {
        let y = top + index as f64 * (row_height + gap);
        let accent = row.accent.as_deref().unwrap_or("@accent1");
        if boxed { shape(drawing, Fill::solid("roundRect", &pale).rounded(6000), [left, y, width, row_height], None); }
        let mut x = left + inset;
        if has_icon {
            if let Some(value) = &row.icon { badge(drawing, value, [x, y + (row_height - diameter) / 2.0], diameter, &mix(accent, theme, 0.88))?; }
            x += diameter + 16.0;
        }
        text(drawing, [x, y + 6.0, left + width - inset - x, row_height - 12.0], Rich::default().add(&row.label, LABEL, "@dk1", true, 300).add(&row.detail, size, "@dk2", false, 0).finish(), TextAlign::Left, VerticalAlign::Middle);
        if !boxed && index + 1 < rows.len() { shape(drawing, Fill::solid("rect", &border), [x, y + row_height - 0.5, left + width - x, 1.0], None); }
    }
    Ok(Drawn { bottom: top + rows.len() as f64 * row_height + gap * (rows.len() - 1) as f64, flexible: Vec::new() })
}

fn shift_rows(drawing: &mut Drawing, headings: [&String; 2], rows: &[ShiftRow], accent: &str, size: f64, frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let has_detail = rows.iter().any(|row| !row.detail.is_empty());
    let (from_width, to_width) = if has_detail { ((width * 0.22).max(150.0), (width * 0.28).max(190.0)) } else { let from = (width - 50.0) * 0.42; (from, width - 50.0 - from) };
    let chevron_x = left + from_width + 12.0;
    let to_x = chevron_x + 26.0 + 12.0;
    let detail_x = to_x + to_width + 24.0;
    let detail_width = left + width - detail_x;
    if has_detail && detail_width < 160.0 { return Err(too_small("shift explanations")); }
    let neutral = mix("@dk2", theme, 0.88);
    let mut y = top;
    if !headings[0].is_empty() || !headings[1].is_empty() {
        for (heading, x, column, fill, color) in [(headings[0], left, from_width, mix("@dk2", theme, 0.80), "@dk1".to_string()), (headings[1], to_x, to_width, mix(accent, theme, 0.84), accent.to_string())] {
            if heading.is_empty() { continue; }
            shape(drawing, Fill::solid("roundRect", &fill).rounded(50000), [x, y, pill_width(heading, SMALL).min(column), 28.0], Some((label(heading, SMALL, &color, true), TextAlign::Center, [10.0, 1.0, 10.0, 1.0])));
        }
        y += 40.0;
    }
    let gap = 10.0;
    let row_height = ((top + height - y - gap * (rows.len() - 1) as f64) / rows.len() as f64).min(ROW_PITCH);
    if row_height < 44.0 { return Err(too_small("shift rows")); }
    for (index, row) in rows.iter().enumerate() {
        let row_top = y + index as f64 * (row_height + gap);
        shape(drawing, Fill::solid("rect", &neutral), [left, row_top, from_width, row_height], Some((label(&row.from, BODY, "@dk1", false), TextAlign::Center, [12.0, 8.0, 12.0, 8.0])));
        shape(drawing, Fill::solid("chevron", accent), [chevron_x, row_top + row_height / 2.0 - 16.0, 26.0, 32.0], None);
        let destination = Rich::default().add(&row.to, 17.0, "@lt1", true, 200).add(&row.caption, 12.0, "@lt1", false, 0).finish();
        shape(drawing, Fill::solid("rect", accent), [to_x, row_top, to_width, row_height], Some((destination, TextAlign::Left, [16.0, 8.0, 12.0, 8.0])));
        if !row.detail.is_empty() { text(drawing, [detail_x, row_top, detail_width, row_height], label(&row.detail, size, "@dk1", false), TextAlign::Left, VerticalAlign::Middle); }
    }
    Ok(Drawn { bottom: y + rows.len() as f64 * row_height + gap * (rows.len() - 1) as f64, flexible: Vec::new() })
}

fn step_cards(drawing: &mut Drawing, steps: &[StepCard], step_label: &str, accent: &str, size: f64, frame: [f64; 4], theme: &Theme, shrink: f64) -> Result<Drawn> {
    let [left, top, width, natural] = frame;
    let arrow = 32.0;
    let column = (width - arrow * (steps.len() - 1) as f64) / steps.len() as f64;
    if column < 180.0 { return Err(too_small("step cards")); }
    let has_image = steps.iter().any(|step| step.image.is_some());
    let has_icon = steps.iter().any(|step| step.icon.is_some());
    let has_outcome = steps.iter().any(|step| !step.outcome.is_empty());
    let image_height = if has_image { (natural * 0.40).min(column * 0.62).round() } else { 0.0 };
    let height = (natural - shrink).max(image_height + 160.0).min(natural);
    let mut flexible = Vec::new();
    let tint = mix(accent, theme, 0.88);
    let placeholder = mix("@dk2", theme, 0.94);
    let chevron = mix("@dk2", theme, 0.55);
    let chevron_center = if has_image { top + image_height / 2.0 } else if has_icon { top + 18.0 } else { top + 13.0 };
    for (index, step) in steps.iter().enumerate() {
        let x = left + index as f64 * (column + arrow);
        let mut y = top;
        if has_image {
            match &step.image { Some(image) => cover(drawing, image, [x, y, column, image_height])?, None => shape(drawing, Fill::solid("rect", &placeholder), [x, y, column, image_height], None) }
            y += image_height + 14.0;
        }
        let mut pill_x = x;
        let mut pill_y = y;
        if has_icon {
            if let Some(value) = &step.icon { badge(drawing, value, [x, y], 36.0, &tint)?; }
            pill_x += 48.0;
            pill_y += 5.0;
        }
        let pill = format!("{step_label} {}", index + 1);
        shape(drawing, Fill::solid("roundRect", accent).rounded(50000), [pill_x, pill_y, pill_width(&pill, 12.0).max(72.0).min(x + column - pill_x), 26.0], Some((label(&pill, 12.0, "@lt1", true), TextAlign::Center, [8.0, 0.0, 8.0, 0.0])));
        y += if has_icon { 48.0 } else { 38.0 };
        let mut bottom = top + height;
        if has_outcome {
            bottom -= 52.0;
            if !step.outcome.is_empty() { shape(drawing, Fill::solid("rect", &tint), [x, bottom, column, 52.0], Some((label(&step.outcome, 14.0, accent, true), TextAlign::Center, [10.0, 8.0, 10.0, 8.0]))); }
            bottom -= 12.0;
        }
        if bottom - y < 48.0 { return Err(too_small("step card texts")); }
        let mut content = Rich::default();
        content.add(&step.label, LABEL, "@dk1", true, 600).add(&step.detail, size, "@dk1", false, 800);
        for point in &step.points { content.bullet(point, size, "@dk1", 600); }
        flexible.extend(text(drawing, [x, y, column, bottom - y], content.finish(), TextAlign::Left, VerticalAlign::Top));
        if index + 1 < steps.len() { shape(drawing, Fill::solid("chevron", &chevron), [x + column + (arrow - 14.0) / 2.0, chevron_center - 13.0, 14.0, 26.0], None); }
    }
    Ok(Drawn { bottom: top + height, flexible })
}

fn agenda(drawing: &mut Drawing, items: &[AgendaItem], fill: &str, frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let row_height = height / items.len() as f64;
    if row_height < 44.0 { return Err(too_small("agenda items")); }
    let diameter = (row_height - 12.0).min(44.0);
    let meta_width = items.iter().filter(|item| !item.meta.is_empty()).map(|item| pill_width(&item.meta, SMALL)).fold(0.0, f64::max);
    let meta_width = if meta_width > 0.0 { meta_width.max(96.0).min(width * 0.25) } else { 0.0 };
    let text_left = left + diameter + 24.0;
    let text_right = left + width - if meta_width > 0.0 { meta_width + 24.0 } else { 0.0 };
    let border = mix("@dk2", theme, 0.80);
    let pale = mix("@dk2", theme, 0.92);
    for (index, item) in items.iter().enumerate() {
        let y = top + index as f64 * row_height;
        let middle = y + row_height / 2.0;
        shape(drawing, Fill::solid("ellipse", fill), [left, middle - diameter / 2.0, diameter, diameter], Some((label(&format!("{:02}", index + 1), 15.0, "@lt1", true), TextAlign::Center, [0.0, 0.0, 0.0, 0.0])));
        text(drawing, [text_left, y + 4.0, text_right - text_left, row_height - 8.0], Rich::default().add(&item.label, LABEL, "@dk1", true, 200).add(&item.detail, 14.0, "@dk2", false, 0).finish(), TextAlign::Left, VerticalAlign::Middle);
        if !item.meta.is_empty() { shape(drawing, Fill::solid("roundRect", &pale).rounded(50000), [left + width - meta_width, middle - 14.0, meta_width, 28.0], Some((label(&item.meta, SMALL, "@dk2", false), TextAlign::Center, [10.0, 0.0, 10.0, 0.0]))); }
        if index + 1 < items.len() { shape(drawing, Fill::solid("rect", &border), [text_left, y + row_height - 0.5, left + width - text_left, 1.0], None); }
    }
    Ok(Drawn { bottom: top + height, flexible: Vec::new() })
}
