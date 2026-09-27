use crate::{model::{Deck, Element, valid_color, valid_text}, parts::{PartData, PartItem, PartLayout, PartSpec}, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionSpec {
    pub title: String,
    #[serde(default)] pub subtitle: String,
    #[serde(default)] pub footer: String,
    #[serde(default)] pub style: CompositionStyle,
    pub blocks: Vec<CompositionBlock>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CompositionStyle {
    pub title_size: f64,
    pub body_size: f64,
    pub footer_size: f64,
    pub padding: f64,
    pub card_fill: String,
    pub accent: String,
}

impl Default for CompositionStyle {
    fn default() -> Self { Self { title_size: 40.0, body_size: 24.0, footer_size: 14.0, padding: 24.0, card_fill: "@lt2".into(), accent: "@accent1".into() } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CompositionBlock {
    Cards { items: Vec<PartItem>, #[serde(default)] columns: Option<usize> },
    Callout { text: String },
    Text { paragraphs: Vec<crate::rich_text::RichParagraph> },
    Steps { items: Vec<PartItem> },
    Comparison { rows: Vec<String>, columns: Vec<String>, cells: Vec<Vec<String>>, #[serde(default)] corner_label: String },
    Graph { input: crate::graphs::GraphLayoutInput },
    Part { spec: PartSpec },
}

fn text(id: &str, value: &str, frame: [f64; 4], size: f64, font: &str) -> Result<Element> {
    Ok(serde_json::from_value(json!({"type":"text","id":id,"x":frame[0],"y":frame[1],"width":frame[2],"height":frame[3],"text":value,"font_size":size,"bold":false,"color":"@dk1","format":{"font_family":font}}))?)
}

fn card(id: &str, item: &PartItem, frame: [f64; 4], style: &CompositionStyle) -> Result<Element> {
    valid_text(&item.label, 80)?; valid_text(&item.detail, 400)?;
    if item.value.is_some() { return Err(Error::Invalid("composition cards require text; use a numeric part for values".into())); }
    let mut paragraphs = Vec::new();
    if !item.label.is_empty() { paragraphs.push(json!({"runs":[{"text":item.label,"style":{"bold":true}}],"space_after":{"kind":"points","value":800}})); }
    if !item.detail.is_empty() { paragraphs.push(json!({"runs":[{"text":item.detail}]})); }
    let paragraphs = serde_json::from_value::<Vec<crate::rich_text::RichParagraph>>(json!(paragraphs))?;
    crate::rich_text::validate_paragraphs(&paragraphs)?;
    let padding = style.padding;
    Ok(serde_json::from_value(json!({"type":"shape","id":id,"x":frame[0],"y":frame[1],"width":frame[2],"height":frame[3],"preset":"rect","fill":style.card_fill,"stroke":style.accent,"stroke_width":1,"text":crate::rich_text::plain_text(&paragraphs),"font_size":style.body_size,"bold":false,"color":"@dk1","format":{"font_family":"@minor","padding":{"left":padding,"right":padding,"top":padding,"bottom":padding},"paragraphs":paragraphs}}))?)
}

pub(crate) fn compose(deck: &mut Deck, parts: &mut Vec<crate::parts::state::PartInstance>, slide_id: &str, id: &str, spec: &CompositionSpec, native_guard: &mut crate::parts::state::NativeRegenerationGuard<'_>) -> Result<()> {
    if id.is_empty() || id.len() > 24 || !id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')) { return Err(Error::Invalid("composition ID requires 1-24 ASCII letters, digits, underscores or hyphens".into())); }
    if spec.blocks.is_empty() || spec.blocks.len() > 3 { return Err(Error::Limit("composition requires 1-3 blocks".into())); }
    valid_text(&spec.title, 120)?; valid_text(&spec.subtitle, 160)?; valid_text(&spec.footer, 200)?;
    let style = &spec.style;
    for (name, value, minimum, maximum) in [("title_size", style.title_size, 24.0, 64.0), ("body_size", style.body_size, 16.0, 40.0), ("footer_size", style.footer_size, 10.0, 24.0), ("padding", style.padding, 8.0, 64.0)] {
        if !value.is_finite() || !(minimum..=maximum).contains(&value) { return Err(Error::Invalid(format!("composition {name} must be in {minimum}..{maximum}"))); }
    }
    valid_color(&style.card_fill)?; valid_color(&style.accent)?;
    let index = deck.slides.iter().position(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("composition slide not found".into()))?;
    let slide = &deck.slides[index];
    if !slide.elements.is_empty() || slide.native_source_id.is_some() || parts.iter().any(|part| part.slide_id == slide_id) { return Err(Error::Conflict("compose_slide requires a new empty slide; existing or native content is never replaced".into())); }
    let width = deck.width as f64; let height = deck.height as f64;
    let margin = 48.0; let gutter = 24.0; let body_width = width - margin * 2.0;
    let title_height = style.title_size * 1.5;
    let mut top = margin + title_height + 16.0;
    let mut elements = vec![text(&format!("{id}-title"), &spec.title, [margin, margin, body_width, title_height], style.title_size, "@major")?];
    if !spec.subtitle.is_empty() {
        let subtitle_height = style.body_size * 1.5;
        elements.push(text(&format!("{id}-subtitle"), &spec.subtitle, [margin, top, body_width, subtitle_height], style.body_size, "@minor")?);
        top += subtitle_height + 16.0;
    }
    let footer_height = if spec.footer.is_empty() { 0.0 } else { style.footer_size * 1.6 + 16.0 };
    let bottom = height - margin - footer_height;
    let block_height = (bottom - top - gutter * (spec.blocks.len() - 1) as f64) / spec.blocks.len() as f64;
    if body_width < 200.0 || block_height < 80.0 { return Err(Error::Invalid("composition content area is too small; use fewer blocks or a larger canvas".into())); }
    if !spec.footer.is_empty() { elements.push(text(&format!("{id}-footer"), &spec.footer, [margin, height - margin - style.footer_size * 1.6, body_width, style.footer_size * 1.6], style.footer_size, "@minor")?); }
    let mut managed = Vec::new();
    for (block_index, block) in spec.blocks.iter().enumerate() {
        let block_id = format!("{id}-b{block_index}");
        let frame = [margin, top + block_index as f64 * (block_height + gutter), body_width, block_height];
        let part = match block {
            CompositionBlock::Cards { items, columns } => {
                if items.is_empty() || items.len() > 6 { return Err(Error::Limit("composition cards require 1-6 items".into())); }
                let columns = columns.unwrap_or(items.len().min(3));
                if !(1..=3).contains(&columns) || columns > items.len() { return Err(Error::Invalid("composition cards require 1-3 columns, not more than the item count".into())); }
                let rows = items.len().div_ceil(columns);
                let card_width = (frame[2] - gutter * (columns - 1) as f64) / columns as f64;
                let card_height = (frame[3] - gutter * (rows - 1) as f64) / rows as f64;
                for (item_index, item) in items.iter().enumerate() {
                    elements.push(card(&format!("{block_id}-c{item_index}"), item, [frame[0] + (item_index % columns) as f64 * (card_width + gutter), frame[1] + (item_index / columns) as f64 * (card_height + gutter), card_width, card_height], style)?);
                }
                None
            }
            CompositionBlock::Callout { text: value } => {
                elements.push(card(&block_id, &PartItem { label: String::new(), detail: value.clone(), value: None }, frame, style)?);
                None
            }
            CompositionBlock::Text { paragraphs } => {
                elements.push(crate::rich_text::replace_paragraphs(text(&block_id, "", frame, style.body_size, "@minor")?, paragraphs.clone())?);
                None
            }
            CompositionBlock::Steps { items } => Some(PartSpec { version: 1, preset: "list-horizontal/labeled".into(), title: String::new(), subtitle: String::new(), data: PartData::Items { items: items.clone(), center: String::new() }, layout: None }),
            CompositionBlock::Comparison { rows, columns, cells, corner_label } => Some(PartSpec { version: 1, preset: "matrix/balanced".into(), title: String::new(), subtitle: String::new(), data: PartData::Matrix { rows: rows.clone(), columns: columns.clone(), cells: cells.clone(), corner_label: corner_label.clone() }, layout: None }),
            CompositionBlock::Graph { input } => {
                let graph = crate::graphs::layout_graph(input)?;
                Some(PartSpec { version: 1, preset: "diagram/custom".into(), title: graph.title.clone(), subtitle: graph.subtitle.clone(), data: PartData::Diagram { graph }, layout: None })
            }
            CompositionBlock::Part { spec: part } => {
                if part.layout.is_some() { return Err(Error::Invalid("composition supplies part layout; omit explicit part coordinates".into())); }
                Some(part.clone())
            }
        };
        if let Some(mut part) = part {
            part.layout = Some(PartLayout { x: frame[0], y: frame[1], width: frame[2], height: frame[3], show_title: !part.title.is_empty() });
            managed.push((block_id, part));
        }
    }
    deck.slides[index].title = spec.title.clone();
    deck.slides[index].layout_id = None;
    deck.slides[index].hide_master_graphics = true;
    deck.slides[index].elements = elements;
    for (part_id, part) in managed { crate::parts::state::change_in_deck(deck, parts, slide_id, &part_id, &part, false, native_guard)?; }
    let mut measured = deck.clone(); measured.slides = vec![deck.slides[index].clone()];
    let report = crate::layout::measure_layout(&measured)?;
    if let Some(measurement) = report.measurements.iter().find(|measurement| measurement.slide_id == slide_id && measurement.overflow) {
        return Err(Error::Invalid(format!("composition text overflows {}: reduce content or split the slide; font sizes are not reduced", measurement.element_id)));
    }
    Ok(())
}