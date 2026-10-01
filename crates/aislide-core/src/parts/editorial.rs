use super::briefing::{badge, colors, field, label, mix, shape, text, too_small, Drawn, Fill, Rich};
use super::business::single;
use super::{Drawing, PartData};
use crate::{design::Theme, graphs::GraphIcon, model::{valid_text, Element, TextAlign, VerticalAlign}, Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditorialStep {
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoadmapPhase {
    pub period: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub detail: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub points: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub outcome: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FactColumn {
    pub value: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub unit: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub detail: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub qualifier: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImageColumn {
    pub image: GraphIcon,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub detail: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub caption: String,
}

pub(super) fn validate(data: &PartData) -> Result<()> {
    match data {
        PartData::OpenSteps { steps, accent } | PartData::RailSteps { steps, accent } => {
            if !(2..=5).contains(&steps.len()) { return Err(Error::Invalid("open steps require 2-5 items".into())); }
            for step in steps {
                field(&step.label, 48, "open step label", true)?;
                field(&step.detail, 160, "open step detail", false)?;
            }
            colors(&[accent])?;
        }
        PartData::Roadmap { phases, accent } => {
            if !(2..=4).contains(&phases.len()) { return Err(Error::Invalid("roadmap requires 2-4 phases".into())); }
            for phase in phases {
                single(&phase.period, 24, "roadmap period", true)?;
                field(&phase.label, 48, "roadmap label", true)?;
                field(&phase.detail, 160, "roadmap detail", false)?;
                field(&phase.outcome, 64, "roadmap outcome", false)?;
                if phase.points.len() > 4 { return Err(Error::Invalid("roadmap allows at most four points per phase".into())); }
                for point in &phase.points { field(point, 80, "roadmap point", true)?; }
            }
            colors(&[accent])?;
        }
        PartData::IconColumns { items } => {
            if !(2..=4).contains(&items.len()) { return Err(Error::Invalid("icon columns require 2-4 items".into())); }
            for item in items {
                field(&item.label, 48, "icon column label", true)?;
                field(&item.detail, 200, "icon column detail", false)?;
                if let Some(icon) = &item.icon { valid_text(&icon.alt, 500)?; }
                colors(&[&item.accent])?;
            }
        }
        PartData::FactColumns { items, columns, accent } => {
            if !(2..=6).contains(&items.len()) { return Err(Error::Invalid("fact columns require 2-6 items".into())); }
            let columns = columns.unwrap_or_else(|| fact_columns_count(items.len()));
            if !(2..=3).contains(&columns) || columns > items.len() || items.len().div_ceil(columns) > 2 {
                return Err(Error::Invalid("fact columns require 2-3 columns and at most two rows".into()));
            }
            for item in items {
                single(&item.value, 24, "fact value", true)?;
                single(&item.unit, 12, "fact unit", false)?;
                field(&item.label, 48, "fact label", true)?;
                field(&item.detail, 160, "fact detail", false)?;
                field(&item.qualifier, 100, "fact qualifier", false)?;
            }
            colors(&[accent])?;
        }
        PartData::ImageColumns { items } => {
            if !(2..=4).contains(&items.len()) { return Err(Error::Invalid("image columns require 2-4 items".into())); }
            for item in items {
                valid_text(&item.image.alt, 500)?;
                field(&item.label, 48, "image column label", true)?;
                field(&item.detail, 160, "image column detail", false)?;
                field(&item.caption, 80, "image caption", false)?;
            }
        }
        _ => return Err(Error::Invalid("editorial part data required".into())),
    }
    Ok(())
}

pub(super) fn render(drawing: &mut Drawing, data: &PartData, frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    match data {
        PartData::OpenSteps { steps, accent } => open_steps(drawing, steps, accent.as_deref().unwrap_or("@accent1"), frame),
        PartData::RailSteps { steps, accent } => rail_steps(drawing, steps, accent.as_deref().unwrap_or("@accent1"), frame),
        PartData::Roadmap { phases, accent } => roadmap(drawing, phases, accent.as_deref().unwrap_or("@accent1"), frame),
        PartData::IconColumns { items } => icon_columns(drawing, items, frame, theme),
        PartData::FactColumns { items, columns, accent } => fact_columns(drawing, items, *columns, accent.as_deref().unwrap_or("@accent1"), frame),
        PartData::ImageColumns { items } => image_columns(drawing, items, frame),
        _ => Err(Error::Invalid("editorial part data required".into())),
    }
}

fn line(drawing: &mut Drawing, start: [f64; 2], end: [f64; 2], color: &str, arrow: bool) {
    let id = drawing.id();
    drawing.elements.push(Element::Connector {
        visual: None, id, x: start[0].min(end[0]), y: start[1].min(end[1]),
        width: (end[0] - start[0]).abs().max(0.01), height: (end[1] - start[1]).abs().max(0.01),
        color: color.into(), stroke_width: 2.0, arrow, flip_v: end[1] < start[1], start: None, end: None, routing: None,
    });
}

fn marker(drawing: &mut Drawing, center: [f64; 2], number: usize, accent: &str) {
    shape(drawing, Fill::solid("ellipse", accent), [center[0] - 16.0, center[1] - 16.0, 32.0, 32.0],
        Some((label(&number.to_string(), 14.0, "@lt1", true), TextAlign::Center, [0.0, 0.0, 0.0, 0.0])));
}

fn open_steps(drawing: &mut Drawing, steps: &[EditorialStep], accent: &str, frame: [f64; 4]) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let gap = 32.0;
    let column = (width - gap * (steps.len() - 1) as f64) / steps.len() as f64;
    if column < 180.0 || height < 160.0 { return Err(too_small("open step labels")); }
    let axis = top + 24.0;
    let center = |index: usize| left + index as f64 * (column + gap) + column / 2.0;
    for index in 0..steps.len() - 1 { line(drawing, [center(index) + 22.0, axis], [center(index + 1) - 22.0, axis], accent, true); }
    for (index, step) in steps.iter().enumerate() {
        marker(drawing, [center(index), axis], index + 1, accent);
        let content = Rich::default().add(&step.label, 18.0, "@dk1", true, 500).add(&step.detail, 16.0, "@dk2", false, 0).finish();
        text(drawing, [left + index as f64 * (column + gap), top + 66.0, column, height - 66.0], content, TextAlign::Left, VerticalAlign::Top);
    }
    Ok(Drawn { bottom: top + height, flexible: Vec::new() })
}

fn rail_steps(drawing: &mut Drawing, steps: &[EditorialStep], accent: &str, frame: [f64; 4]) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let pitch = (height / steps.len() as f64).min(112.0);
    if width < 420.0 || pitch < 72.0 { return Err(too_small("vertical rail steps")); }
    let center = |index: usize| top + index as f64 * pitch + 22.0;
    for index in 0..steps.len() - 1 { line(drawing, [left + 16.0, center(index) + 22.0], [left + 16.0, center(index + 1) - 22.0], accent, true); }
    for (index, step) in steps.iter().enumerate() {
        marker(drawing, [left + 16.0, center(index)], index + 1, accent);
        let content = Rich::default().add(&step.label, 18.0, "@dk1", true, 300).add(&step.detail, 16.0, "@dk2", false, 0).finish();
        text(drawing, [left + 64.0, top + index as f64 * pitch + 3.0, width - 64.0, pitch - 9.0], content, TextAlign::Left, VerticalAlign::Top);
    }
    Ok(Drawn { bottom: top + pitch * steps.len() as f64, flexible: Vec::new() })
}

fn roadmap(drawing: &mut Drawing, phases: &[RoadmapPhase], accent: &str, frame: [f64; 4]) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let gap = 32.0;
    let column = (width - gap * (phases.len() - 1) as f64) / phases.len() as f64;
    if column < 220.0 || height < 240.0 { return Err(too_small("roadmap phases")); }
    let center = |index: usize| left + index as f64 * (column + gap) + column / 2.0;
    let axis = top + 42.0;
    for index in 0..phases.len() - 1 { line(drawing, [center(index) + 22.0, axis], [center(index + 1) - 22.0, axis], accent, true); }
    let outcomes = phases.iter().any(|phase| !phase.outcome.is_empty());
    let reserved = if outcomes { 66.0 } else { 0.0 };
    for (index, phase) in phases.iter().enumerate() {
        let x = left + index as f64 * (column + gap);
        text(drawing, [x, top, column, 22.0], label(&phase.period, 16.0, accent, true), TextAlign::Center, VerticalAlign::Top);
        marker(drawing, [center(index), axis], index + 1, accent);
        let mut content = Rich::default();
        content.add(&phase.label, 18.0, "@dk1", true, 400).add(&phase.detail, 16.0, "@dk2", false, 400);
        for point in &phase.points { content.bullet(point, 16.0, "@dk1", 300); }
        text(drawing, [x, top + 78.0, column, height - 78.0 - reserved], content.finish(), TextAlign::Left, VerticalAlign::Top);
        if !phase.outcome.is_empty() {
            line(drawing, [x, top + height - 58.0], [x + column, top + height - 58.0], accent, false);
            text(drawing, [x, top + height - 44.0, column, 40.0], label(&phase.outcome, 16.0, accent, true), TextAlign::Left, VerticalAlign::Top);
        }
    }
    Ok(Drawn { bottom: top + height, flexible: Vec::new() })
}

fn icon_columns(drawing: &mut Drawing, items: &[super::IconRow], frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let gap = 32.0;
    let column = (width - gap * (items.len() - 1) as f64) / items.len() as f64;
    let icons = items.iter().any(|item| item.icon.is_some());
    let text_top = if icons { 70.0 } else { 8.0 };
    if column < 200.0 || height - text_top < 120.0 { return Err(too_small("icon columns")); }
    for (index, item) in items.iter().enumerate() {
        let x = left + index as f64 * (column + gap);
        let accent = item.accent.as_deref().unwrap_or("@accent1");
        if let Some(icon) = &item.icon { badge(drawing, icon, [x, top], 48.0, &mix(accent, theme, 0.9))?; }
        let content = Rich::default().add(&item.label, 18.0, "@dk1", true, 600).add(&item.detail, 16.0, "@dk2", false, 0).finish();
        text(drawing, [x, top + text_top, column, height - text_top], content, TextAlign::Left, VerticalAlign::Top);
    }
    Ok(Drawn { bottom: top + height, flexible: Vec::new() })
}

fn fact_columns_count(count: usize) -> usize { if count <= 3 { count } else if count == 4 { 2 } else { 3 } }

fn fact_columns(drawing: &mut Drawing, items: &[FactColumn], columns: Option<usize>, accent: &str, frame: [f64; 4]) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let columns = columns.unwrap_or_else(|| fact_columns_count(items.len()));
    let rows = items.len().div_ceil(columns);
    let gap = 32.0;
    let column = (width - gap * (columns - 1) as f64) / columns as f64;
    let row = (height - gap * (rows - 1) as f64) / rows as f64;
    if column < 200.0 || row < 164.0 { return Err(too_small("fact columns")); }
    for (index, item) in items.iter().enumerate() {
        let x = left + (index % columns) as f64 * (column + gap);
        let y = top + (index / columns) as f64 * (row + gap);
        shape(drawing, Fill::solid("rect", accent), [x, y, 44.0, 3.0], None);
        let unit = if item.unit.is_empty() { String::new() } else { format!(" {}", item.unit) };
        let content = Rich::default().runs(&[(&item.value, 44.0, accent, true), (&unit, 22.0, accent, true)], 500)
            .add(&item.label, 18.0, "@dk1", true, 300).add(&item.detail, 16.0, "@dk1", false, 400).add(&item.qualifier, 14.0, "@dk2", false, 0).finish();
        text(drawing, [x, y + 14.0, column, row - 14.0], content, TextAlign::Left, VerticalAlign::Top);
    }
    Ok(Drawn { bottom: top + height, flexible: Vec::new() })
}

fn contained_image(drawing: &mut Drawing, image: &GraphIcon, bounds: [f64; 4]) -> Result<()> {
    let mut picture = crate::media::create_picture(&drawing.id(), image.base64.clone(), &image.mime_type, &image.alt)?;
    if let Element::Picture { x, y, width, height, .. } = &mut picture {
        let scale = (bounds[2] / *width).min(bounds[3] / *height);
        *width *= scale; *height *= scale;
        *x = bounds[0] + (bounds[2] - *width) / 2.0;
        *y = bounds[1] + (bounds[3] - *height) / 2.0;
    }
    drawing.elements.push(picture);
    Ok(())
}

fn image_columns(drawing: &mut Drawing, items: &[ImageColumn], frame: [f64; 4]) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let gap = 32.0;
    let column = (width - gap * (items.len() - 1) as f64) / items.len() as f64;
    let image_height = (column * 9.0 / 16.0).min(height - 140.0);
    if column < 200.0 || image_height < 100.0 { return Err(too_small("image columns")); }
    let captions = items.iter().any(|item| !item.caption.is_empty());
    let text_top = image_height + if captions { 50.0 } else { 20.0 };
    for (index, item) in items.iter().enumerate() {
        let x = left + index as f64 * (column + gap);
        contained_image(drawing, &item.image, [x, top, column, image_height])?;
        if !item.caption.is_empty() { text(drawing, [x, top + image_height + 8.0, column, 32.0], label(&item.caption, 14.0, "@dk2", false), TextAlign::Left, VerticalAlign::Top); }
        let content = Rich::default().add(&item.label, 18.0, "@dk1", true, 500).add(&item.detail, 16.0, "@dk1", false, 0).finish();
        text(drawing, [x, top + text_top, column, height - text_top], content, TextAlign::Left, VerticalAlign::Top);
    }
    Ok(Drawn { bottom: top + height, flexible: Vec::new() })
}