use crate::{document::{self, Document, Transaction, TransactionResult}, model::{Connection, ConnectorRouting, Crop, Deck, Element}, rich_text::{RichParagraph, RichRun, RunStyle}, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame { pub x: f64, pub y: f64, pub width: f64, pub height: f64 }

impl Frame {
    fn validate(&self) -> Result<()> {
        if [self.x, self.y, self.width, self.height].iter().any(|value| !value.is_finite() || !(0.0..=4096.0).contains(value)) || self.width <= 0.0 || self.height <= 0.0 {
            return Err(Error::Invalid("frame requires finite nonnegative coordinates and positive dimensions within 4096px".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorSettings {
    pub color: String,
    pub stroke_width: f64,
    pub arrow: bool,
    #[serde(default)] pub flip_v: bool,
    #[serde(default)] pub start: Option<Connection>,
    #[serde(default)] pub end: Option<Connection>,
    #[serde(default)] pub routing: Option<ConnectorRouting>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    AddElements { slide_id: String, elements: Vec<Element> },
    AddPart { slide_id: String, id: String, spec: crate::parts::PartSpec },
    UpdatePart { slide_id: String, id: String, spec: crate::parts::PartSpec },
    AddGraph { slide_id: String, id: String, spec: crate::graphs::GraphSpec, #[serde(default)] layout: Option<crate::parts::PartLayout> },
    UpdateGraph { slide_id: String, id: String, spec: crate::graphs::GraphSpec },
    SetFrame { slide_id: String, id: String, frame: Frame },
    SetTextStyle { slide_id: String, ids: Vec<String>, style: RunStyle },
    SetSlideBackground { slide_id: String, color: String },
    SetConnector { slide_id: String, id: String, connector: ConnectorSettings, #[serde(default)] frame: Option<Frame> },
    SetPictureCrop { slide_id: String, id: String, crop: Crop },
    SetHyperlink { slide_id: String, id: String, #[serde(deserialize_with = "Option::deserialize")] link: Option<String> },
    SetShapeAdjustment { slide_id: String, id: String, adjustment: crate::visual::ShapeAdjustment },
    AddPicture { slide_id: String, id: String, base64: String, mime_type: String, alt: String, #[serde(default)] frame: Option<Frame>, #[serde(default)] crop: Crop },
}

fn target<'a>(elements: &'a mut [Element], id: &str) -> Result<&'a mut Element> {
    for element in elements {
        let contains = element.bounds().0 == id || matches!(element, Element::Group { children, .. } if crate::model::element_list(children).iter().any(|child| child.bounds().0 == id));
        if !contains { continue; }
        if element.visual().is_some_and(|visual| visual.locked || visual.hidden) { return Err(Error::Unsupported("unlock and show the target and its parent groups before editing".into())); }
        if element.bounds().0 == id { return Ok(element); }
        if let Element::Group { children, .. } = element { return target(children, id); }
    }
    Err(Error::Invalid(format!("element not found: {id}")))
}

fn frame(element: &mut Element, destination: &Frame) -> Result<()> {
    destination.validate()?;
    if element.visual().is_some_and(|visual| visual.locked || visual.hidden) { return Err(Error::Unsupported("unlock and show the target and its parent groups before editing".into())); }
    let (_, previous_x, previous_y, previous_width, previous_height) = element.bounds();
    if [previous_x, previous_y, previous_width, previous_height] == [destination.x, destination.y, destination.width, destination.height] { return Ok(()); }
    match element {
        Element::Text { format, .. } => format.inherit_layout = false,
        Element::Group { width, height, view_width, view_height, children, .. } if *width != destination.width || *height != destination.height => {
            for child in children {
                let (_, x, y, width, height) = child.bounds();
                frame(child, &Frame { x: x * destination.width / *view_width, y: y * destination.height / *view_height, width: width * destination.width / *view_width, height: height * destination.height / *view_height })?;
            }
            *view_width = destination.width; *view_height = destination.height;
        }
        Element::Table { format, .. } => {
            for (tracks, scale) in [(&mut format.column_widths, destination.width / previous_width), (&mut format.row_heights, destination.height / previous_height)] {
                if let Some(tracks) = tracks {
                    if tracks.unit == crate::table_format::DimensionUnit::Absolute { for value in &mut tracks.values { *value *= scale; } }
                }
            }
        }
        _ => {}
    }
    match element {
        Element::Text { x, y, width, height, .. } | Element::Rect { x, y, width, height, .. }
        | Element::Polygon { x, y, width, height, .. } | Element::Shape { x, y, width, height, .. }
        | Element::Table { x, y, width, height, .. } | Element::Chart { x, y, width, height, .. }
        | Element::Picture { x, y, width, height, .. } | Element::Connector { x, y, width, height, .. }
        | Element::Group { x, y, width, height, .. } => {
            *x = destination.x; *y = destination.y; *width = destination.width; *height = destination.height;
        }
    }
    Ok(())
}

fn text_style(element: &mut Element, style: &RunStyle) -> Result<()> {
    style.validate()?;
    let (text, font_size, color, bold, format) = match element {
        Element::Text { text, font_size, color, bold, format, .. } | Element::Shape { text, font_size, color, bold, format, .. } => (text, font_size, color, bold, format),
        _ => return Err(Error::Unsupported("partial text style requires a text box or shape".into())),
    };
    if let Some(value) = style.font_size { *font_size = value; }
    if let Some(value) = &style.color { *color = value.clone(); }
    if let Some(value) = style.bold { *bold = value; }
    if let Some(value) = style.italic { format.italic = value; }
    if let Some(value) = style.underline { format.underline = value; }
    if let Some(value) = &style.font_family { format.font_family = Some(value.clone()); }
    if format.paragraphs.is_empty() && (style.baseline.is_some() || style.highlight.is_some() || style.language.is_some()) {
        format.paragraphs = text.split('\n').map(|line| RichParagraph { runs: vec![RichRun { text: line.into(), style: style.clone(), field: None }], alignment: Some(format.alignment), bullet: Some(format.bullet), ..Default::default() }).collect();
    } else {
        for paragraph in &mut format.paragraphs { for run in &mut paragraph.runs { run.style.overlay(style); } }
    }
    format.inherit_layout = false;
    Ok(())
}

fn apply(deck: &mut Deck, parts: &mut Vec<crate::parts::state::PartInstance>, operation: &Operation, native_guard: &mut crate::parts::state::NativeRegenerationGuard<'_>) -> Result<usize> {
    let slide_id = match operation {
        Operation::AddElements { slide_id, .. } | Operation::SetFrame { slide_id, .. } | Operation::SetTextStyle { slide_id, .. }
        | Operation::AddPart { slide_id, .. } | Operation::UpdatePart { slide_id, .. } | Operation::AddGraph { slide_id, .. } | Operation::UpdateGraph { slide_id, .. }
        | Operation::SetSlideBackground { slide_id, .. } | Operation::SetConnector { slide_id, .. } | Operation::SetPictureCrop { slide_id, .. }
        | Operation::SetHyperlink { slide_id, .. } | Operation::SetShapeAdjustment { slide_id, .. } | Operation::AddPicture { slide_id, .. } => slide_id,
    };
    let index = deck.slides.iter().position(|slide| slide.id == *slide_id).ok_or_else(|| Error::Invalid(format!("slide not found: {slide_id}")))?;
    let slide = &mut deck.slides[index];
    match operation {
        Operation::AddElements { elements, .. } => {
            if elements.is_empty() || elements.len() > 128 { return Err(Error::Limit("add_elements requires 1-128 roots".into())); }
            slide.elements.extend(elements.iter().cloned());
        }
        Operation::AddPart { id, spec, .. } | Operation::UpdatePart { id, spec, .. } => {
            crate::parts::state::change_in_deck(deck, parts, slide_id, id, spec, matches!(operation, Operation::UpdatePart { .. }), native_guard)?;
        }
        Operation::AddGraph { id, spec, layout, .. } => {
            let part = crate::parts::PartSpec { version: 1, preset: "diagram/custom".into(), title: spec.title.clone(), subtitle: spec.subtitle.clone(), data: crate::parts::PartData::Diagram { graph: spec.clone() }, layout: layout.clone() };
            crate::parts::state::change_in_deck(deck, parts, slide_id, id, &part, false, native_guard)?;
        }
        Operation::UpdateGraph { id, spec, .. } => {
            let layout = parts.iter().find(|part| part.slide_id == *slide_id && part.element_id == *id && part.spec.preset == "diagram/custom")
                .ok_or_else(|| Error::Invalid("managed graph metadata not found".into()))?.spec.layout.clone();
            let part = crate::parts::PartSpec { version: 1, preset: "diagram/custom".into(), title: spec.title.clone(), subtitle: spec.subtitle.clone(), data: crate::parts::PartData::Diagram { graph: spec.clone() }, layout };
            crate::parts::state::change_in_deck(deck, parts, slide_id, id, &part, true, native_guard)?;
        }
        Operation::SetFrame { id, frame: destination, .. } => frame(target(&mut slide.elements, id)?, destination)?,
        Operation::SetTextStyle { ids, style, .. } => {
            if ids.is_empty() || ids.len() > 128 || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() { return Err(Error::Limit("set_text_style requires 1-128 distinct IDs".into())); }
            if *style == RunStyle::default() { return Err(Error::Invalid("set_text_style requires at least one supplied property".into())); }
            for id in ids { text_style(target(&mut slide.elements, id)?, style)?; }
        }
        Operation::SetSlideBackground { color, .. } => {
            crate::model::valid_color(color)?;
            slide.background = color.clone(); slide.inherit_background = false;
        }
        Operation::SetConnector { id, connector, frame: destination, .. } => {
            let element = target(&mut slide.elements, id)?;
            let Element::Connector { color, stroke_width, arrow, flip_v, start, end, routing, .. } = element else { return Err(Error::Unsupported("set_connector requires a connector".into())); };
            *color = connector.color.clone(); *stroke_width = connector.stroke_width; *arrow = connector.arrow; *flip_v = connector.flip_v;
            *start = connector.start.clone(); *end = connector.end.clone(); *routing = connector.routing.clone();
            if let Some(destination) = destination { frame(element, destination)?; }
        }
        Operation::SetPictureCrop { id, crop, .. } => {
            let Element::Picture { crop: current, .. } = target(&mut slide.elements, id)? else { return Err(Error::Unsupported("set_picture_crop requires a picture".into())); };
            *current = crop.clone();
        }
        Operation::SetHyperlink { id, link, .. } => {
            if let Some(link) = link { crate::model::validate_hyperlink(link)?; }
            match target(&mut slide.elements, id)? {
                Element::Text { format, .. } | Element::Shape { format, .. } => {
                    if format.hyperlink != *link { format.hyperlink = link.clone(); format.inherit_layout = false; }
                }
                _ => return Err(Error::Unsupported("set_hyperlink requires a text box or shape".into())),
            }
        }
        Operation::SetShapeAdjustment { id, adjustment, .. } => {
            let element = target(&mut slide.elements, id)?;
            if !matches!(element, Element::Shape { .. }) { return Err(Error::Unsupported("set_shape_adjustment requires a shape".into())); }
            let visual = element.visual_mut().ok_or_else(|| Error::Unsupported("shape visual properties".into()))?.get_or_insert_with(Default::default);
            visual.adjustments.retain(|entry| entry.name != adjustment.name);
            visual.adjustments.push(adjustment.clone());
        }
        Operation::AddPicture { id, base64, mime_type, alt, frame: destination, crop, .. } => {
            let mut element = crate::media::create_picture(id, base64.clone(), mime_type, alt)?;
            if let Some(destination) = destination { frame(&mut element, destination)?; }
            if let Element::Picture { crop: current, .. } = &mut element { *current = crop.clone(); }
            slide.elements.push(element);
        }
    }
    Ok(index)
}

pub fn apply_operations(document: &Document, expected_revision: u64, expected_hash: &str, operations: &[Operation]) -> Result<TransactionResult> {
    if operations.is_empty() || operations.len() > 128 { return Err(Error::Limit("apply_operations requires 1-128 operations".into())); }
    if document.revision != expected_revision || document.hash != expected_hash { return Err(Error::Conflict("stale document revision or content hash".into())); }
    document::verify(document)?;
    let mut deck = document.deck.clone();
    let mut parts = document.parts.clone();
    let mut native_guard = crate::parts::state::NativeRegenerationGuard::new(document);
    let metadata_changed = operations.iter().any(|operation| matches!(operation, Operation::AddPart { .. } | Operation::UpdatePart { .. } | Operation::AddGraph { .. } | Operation::UpdateGraph { .. }));
    let mut changed = BTreeSet::new();
    for (index, operation) in operations.iter().enumerate() {
        changed.insert(apply(&mut deck, &mut parts, operation, &mut native_guard).map_err(|error| Error::Invalid(format!("operation {}: {error}", index + 1)))?);
        crate::preflight::deck(&deck, document.capacity_profile.limits())?;
    }
    let mut patches: Vec<_> = if metadata_changed && changed.len() == 128 {
        vec![json!({"op":"replace","path":"/deck/slides","value":deck.slides})]
    } else { changed.into_iter().map(|index| json!({"op":"replace","path":format!("/deck/slides/{index}"),"value":deck.slides[index]})).collect() };
    if metadata_changed { patches.push(json!({"op":"add","path":"/parts","value":parts})); }
    let result = document::transact(document, Transaction { expected_revision, expected_hash: expected_hash.into(), operations: serde_json::from_value(json!(patches))? })?;
    let targets = operations.iter().filter_map(|operation| match operation {
        Operation::AddGraph { slide_id, id, .. } | Operation::UpdateGraph { slide_id, id, .. } => Some((slide_id.clone(), id.clone())),
        Operation::AddPart { slide_id, id, spec } | Operation::UpdatePart { slide_id, id, spec } if matches!(spec.data, crate::parts::PartData::Diagram { .. }) => Some((slide_id.clone(), id.clone())),
        _ => None,
    }).collect();
    Ok(crate::graphs::annotate_transaction(result, &targets))
}