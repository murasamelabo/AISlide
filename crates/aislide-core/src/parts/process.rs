use super::briefing::{colors, field, label, message, mix, shape, text, too_small, Drawn, Fill, Rich, SMALL};
use super::business::{darken, figure, finite, items, path_polygon, single};
use super::{accent, Drawing, PartData, PartSpec};
use crate::{design::Theme, graphs::GraphSpec, model::{Element, TextAlign, VerticalAlign}, vector::PathCommand, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const CANVAS_TOP: f64 = 88.0;
const CANVAS_HEIGHT: f64 = 424.0;

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FishboneCategory { pub label: String, pub causes: Vec<FishboneCause> }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FishboneCause { pub text: String, #[serde(default, skip_serializing_if = "std::ops::Not::not")] pub focus: bool }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepShape { #[default] Task, Decision, Event }

fn task(shape: &StepShape) -> bool { *shape == StepShape::Task }

/// `lane` indexes `lanes`; `column` pins the step to a 0-7 column, otherwise the longest forward path decides.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SwimlaneStep {
    pub id: String,
    pub label: String,
    pub lane: usize,
    #[serde(default, skip_serializing_if = "task")] pub shape: StepShape,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub column: Option<usize>,
}

/// `exception` marks returns and rework; they are dashed, routed below the steps and excluded from column ordering.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SwimlaneFlow {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub label: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")] pub exception: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SankeyNode { pub id: String, pub label: String, #[serde(default, skip_serializing_if = "Option::is_none")] pub color: Option<String> }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SankeyLink { pub from: String, pub to: String, pub value: f64 }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JourneyRow { pub label: String, pub cells: Vec<String>, #[serde(default, skip_serializing_if = "std::ops::Not::not")] pub boxed: bool }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArchitectureKind { Person, Container, Database, External }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureElement {
    pub id: String,
    pub label: String,
    pub kind: ArchitectureKind,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureRelation { pub from: String, pub to: String, #[serde(default, skip_serializing_if = "String::is_empty")] pub label: String }

fn identifier(value: &str, name: &str) -> Result<()> {
    if value.is_empty() || value.len() > 24 || !value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_') {
        return Err(Error::Invalid(format!("{name} must be 1-24 ASCII letters, digits, '-' or '_'")));
    }
    Ok(())
}

fn unique<'a>(ids: impl IntoIterator<Item = &'a str>, name: &str) -> Result<BTreeMap<&'a str, usize>> {
    let mut index = BTreeMap::new();
    for (position, id) in ids.into_iter().enumerate() {
        identifier(id, name)?;
        if index.insert(id, position).is_some() { return Err(Error::Invalid(format!("duplicate {name} {id}"))); }
    }
    Ok(index)
}

pub(super) fn validate(data: &PartData) -> Result<()> {
    match data {
        PartData::Fishbone { effect, categories, message: band } => {
            field(effect, 60, "fishbone effect", true)?;
            items(categories.len(), 2, 6, "fishbone categories")?;
            for category in categories {
                single(&category.label, 20, "fishbone category", true)?;
                items(category.causes.len(), 1, 3, "fishbone causes")?;
                for cause in &category.causes { single(&cause.text, 28, "fishbone cause", true)?; }
            }
            message(band)?;
        }
        PartData::Sankey { nodes, links, unit, message: band } => {
            sankey_stages(nodes, links)?;
            single(unit, 8, "sankey unit", false)?;
            message(band)?;
        }
        PartData::Journey { stages, rows, emotions, emotion_label, emotion_notes, highlight, message: band } => {
            items(stages.len(), 2, 6, "journey stages")?; items(rows.len(), 1, 4, "journey rows")?;
            for stage in stages { single(stage, 20, "journey stage", true)?; }
            for row in rows {
                single(&row.label, 16, "journey row", true)?;
                if row.cells.len() != stages.len() { return Err(Error::Invalid("journey rows require one cell per stage".into())); }
                for cell in &row.cells { field(cell, 60, "journey cell", false)?; }
            }
            if !emotions.is_empty() && emotions.len() != stages.len() { return Err(Error::Invalid("journey emotions require one -2..2 value per stage when present".into())); }
            if emotions.iter().any(|value| !(-2..=2).contains(value)) { return Err(Error::Invalid("journey emotions must be -2..2".into())); }
            if !emotion_notes.is_empty() && (emotions.is_empty() || emotion_notes.len() != stages.len()) { return Err(Error::Invalid("journey emotion_notes require emotions and one note per stage".into())); }
            for note in emotion_notes { field(note, 32, "journey emotion note", false)?; }
            single(emotion_label, 16, "emotion_label", false)?;
            if highlight.is_some_and(|stage| stage >= stages.len()) { return Err(Error::Invalid("journey highlight must index a stage".into())); }
            message(band)?;
        }
        PartData::Swimlane { .. } | PartData::Architecture { .. } => { graph_data(&data_spec(data), &Theme::default())?; }
        _ => return Err(Error::Invalid("briefing part data required".into())),
    }
    Ok(())
}

fn data_spec(data: &PartData) -> PartSpec { PartSpec { version: 1, preset: String::new(), title: String::new(), subtitle: String::new(), data: data.clone(), layout: None } }

pub(super) fn render(drawing: &mut Drawing, data: &PartData, frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    match data {
        PartData::Fishbone { effect, categories, .. } => fishbone(drawing, effect, categories, frame, theme),
        PartData::Sankey { nodes, links, unit, .. } => sankey(drawing, nodes, links, unit, frame),
        PartData::Journey { stages, rows, emotions, emotion_label, emotion_notes, highlight, .. } =>
            journey(drawing, stages, rows, emotions, emotion_notes, if emotion_label.is_empty() { "Emotion" } else { emotion_label }, *highlight, frame, theme),
        _ => Err(Error::Invalid("briefing part data required".into())),
    }
}

/// Straight native line drawn left to right; `arrow` marks the right-hand end.
fn connector(drawing: &mut Drawing, start: [f64; 2], end: [f64; 2], color: &str, width: f64, arrow: bool) {
    let id = drawing.id();
    let (start, end) = if start[0] <= end[0] { (start, end) } else { (end, start) };
    drawing.elements.push(Element::Connector { visual: None, id, x: start[0], y: start[1].min(end[1]), width: (end[0] - start[0]).max(0.01), height: (end[1] - start[1]).abs().max(0.01), color: color.into(), stroke_width: width, arrow, flip_v: end[1] < start[1], start: None, end: None, routing: None });
}

fn fishbone(drawing: &mut Drawing, effect: &str, categories: &[FishboneCategory], frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let head_width = (width * 0.19).clamp(150.0, 230.0);
    let head_height = (height * 0.34).clamp(72.0, 112.0);
    let spine_y = top + height / 2.0;
    let head_x = left + width - head_width;
    let spine_left = left + 4.0;
    let upper = categories.len().div_ceil(2);
    let span = head_x - 24.0 - spine_left;
    let slots = [span / upper as f64, span / (categories.len() - upper) as f64];
    let reach = height / 2.0 - 44.0;
    let most = categories.iter().map(|category| category.causes.len()).max().unwrap_or(1) as f64;
    if slots[0] < 170.0 || reach < 84.0 || reach * 0.8 / (most + 1.0) < 22.0 { return Err(too_small("fishbone causes")); }
    connector(drawing, [spine_left, spine_y], [head_x, spine_y], "@dk1", 3.0, true);
    shape(drawing, Fill::solid("rect", "@dk1"), [head_x, spine_y - head_height / 2.0, head_width, head_height], Some((label(effect, 17.0, "@lt1", true), TextAlign::Center, [12.0, 8.0, 12.0, 8.0])));
    let slant = reach * 0.5;
    let tint = mix("@accent1", theme, 0.84);
    let focus = darken("@accent2", theme, 0.2);
    for (index, category) in categories.iter().enumerate() {
        let above = index < upper;
        let (position, slot) = if above { (index, slots[0]) } else { (index - upper, slots[1]) };
        // Lower bones sit 30% of a slot earlier so they do not meet the spine where upper bones do.
        let join_x = spine_left + slot * (position as f64 + 1.0) - 8.0 - if above { 0.0 } else { slot * 0.3 };
        let direction = if above { -1.0 } else { 1.0 };
        let tip = [join_x - slant, spine_y + direction * reach];
        connector(drawing, tip, [join_x, spine_y], "@dk2", 2.0, false);
        let label_width = (slot * 0.7).min(170.0);
        let label_y = if above { tip[1] - 34.0 } else { tip[1] + 4.0 };
        shape(drawing, Fill::solid("rect", &tint), [(tip[0] - label_width / 2.0).max(left), label_y, label_width, 30.0], Some((label(&category.label, 15.0, "@dk1", true), TextAlign::Center, [6.0, 0.0, 6.0, 0.0])));
        let count = category.causes.len() as f64;
        for (cause_index, cause) in category.causes.iter().enumerate() {
            let fraction = (cause_index as f64 + 1.0) / (count + 1.0) * 0.8 + 0.1;
            let point = [tip[0] + (join_x - tip[0]) * fraction, tip[1] + (spine_y - tip[1]) * fraction];
            let start = (point[0] - (slot * 0.62).min(190.0)).max(left);
            connector(drawing, [start, point[1]], point, "@dk2", 1.0, false);
            let color = if cause.focus { focus.as_str() } else { "@dk1" };
            // Upper bones lean left above their rib, so the label stops short of where the bone crosses its top edge.
            let clearance = if above { 8.0 + 22.0 * slant / reach } else { 6.0 };
            text(drawing, [start, point[1] - 22.0, point[0] - clearance - start, 20.0], label(&cause.text, SMALL, color, cause.focus), TextAlign::Right, VerticalAlign::Bottom);
        }
    }
    Ok(Drawn { bottom: top + height, flexible: Vec::new() })
}

fn sankey_stages(nodes: &[SankeyNode], links: &[SankeyLink]) -> Result<Vec<usize>> {
    items(nodes.len(), 2, 16, "sankey nodes")?; items(links.len(), 1, 32, "sankey links")?;
    let index = unique(nodes.iter().map(|node| node.id.as_str()), "sankey node id")?;
    for node in nodes { single(&node.label, 24, "sankey label", true)?; colors(&[&node.color])?; }
    let mut pairs = BTreeSet::new();
    let mut inflow = vec![0.0; nodes.len()];
    let mut outflow = vec![0.0; nodes.len()];
    let mut edges = Vec::new();
    for link in links {
        let (Some(&from), Some(&to)) = (index.get(link.from.as_str()), index.get(link.to.as_str())) else { return Err(Error::Invalid("sankey links must reference node ids".into())); };
        finite(link.value, "sankey values")?;
        if link.value <= 0.0 || from == to || !pairs.insert((from, to)) { return Err(Error::Invalid("sankey links need a positive value between two different nodes, once per pair".into())); }
        outflow[from] += link.value; inflow[to] += link.value; edges.push((from, to));
    }
    for (position, node) in nodes.iter().enumerate() {
        if inflow[position] == 0.0 && outflow[position] == 0.0 { return Err(Error::Invalid(format!("sankey node {} has no links", node.id))); }
    }
    let mut stage = vec![0usize; nodes.len()];
    for _ in 0..nodes.len() {
        let mut changed = false;
        for &(from, to) in &edges { if stage[to] < stage[from] + 1 { stage[to] = stage[from] + 1; changed = true; } }
        if !changed { break; }
    }
    if edges.iter().any(|&(from, to)| stage[to] <= stage[from]) { return Err(Error::Invalid("sankey links must not form a cycle".into())); }
    if stage.iter().max().copied().unwrap_or(0) > 4 { return Err(Error::Invalid("sankey diagrams allow at most five stages".into())); }
    for (position, node) in nodes.iter().enumerate() {
        if inflow[position] > 0.0 && outflow[position] > 0.0 && (inflow[position] - outflow[position]).abs() > 1e-6 * inflow[position].max(outflow[position]) {
            return Err(Error::Invalid(format!("sankey node {} receives {} but sends {}; add an explicit loss or gain node", node.id, figure(inflow[position]), figure(outflow[position]))));
        }
    }
    Ok(stage)
}

fn sankey(drawing: &mut Drawing, nodes: &[SankeyNode], links: &[SankeyLink], unit: &str, frame: [f64; 4]) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let stage = sankey_stages(nodes, links)?;
    let stages = stage.iter().max().copied().unwrap_or(0) + 1;
    let index: BTreeMap<&str, usize> = nodes.iter().enumerate().map(|(position, node)| (node.id.as_str(), position)).collect();
    let mut inflow = vec![0.0; nodes.len()];
    let mut outflow = vec![0.0; nodes.len()];
    for link in links { outflow[index[link.from.as_str()]] += link.value; inflow[index[link.to.as_str()]] += link.value; }
    let value: Vec<f64> = inflow.iter().zip(&outflow).map(|(input, output)| input.max(*output)).collect();
    let bar = 14.0;
    let label_width = (width * 0.17).clamp(120.0, 200.0);
    let first = left + label_width + 8.0;
    let last = left + width - label_width - 8.0 - bar;
    let spacing = (last - first) / (stages - 1) as f64;
    if spacing < 150.0 { return Err(too_small("sankey stages")); }
    let middle_label = (spacing - bar - 20.0).min(label_width);
    let gap = 14.0;
    let members: Vec<Vec<usize>> = (0..stages).map(|current| (0..nodes.len()).filter(|position| stage[*position] == current).collect()).collect();
    let scale = members.iter().map(|group| (height - gap * (group.len() - 1) as f64) / group.iter().map(|position| value[*position]).sum::<f64>()).fold(f64::INFINITY, f64::min);
    if !scale.is_finite() || scale <= 0.0 { return Err(too_small("sankey nodes")); }
    let mut y = vec![0.0; nodes.len()];
    for group in &members {
        let total = group.iter().map(|position| value[*position] * scale).sum::<f64>() + gap * (group.len() - 1) as f64;
        let mut cursor = top + (height - total) / 2.0;
        let mut previous = f64::NEG_INFINITY;
        for &position in group {
            y[position] = cursor;
            let middle = cursor + value[position] * scale / 2.0;
            if middle - previous < 22.0 { return Err(too_small("sankey labels")); }
            previous = middle;
            cursor += value[position] * scale + gap;
        }
    }
    let x = |position: usize| first + spacing * stage[position] as f64;
    let color: Vec<String> = nodes.iter().enumerate().map(|(position, node)| node.color.clone().unwrap_or_else(|| accent(position))).collect();
    let mut order: Vec<usize> = (0..links.len()).collect();
    let target = |link: usize| index[links[link].to.as_str()];
    let source = |link: usize| index[links[link].from.as_str()];
    order.sort_by(|a, b| (stage[source(*a)], y[source(*a)], y[target(*a)]).partial_cmp(&(stage[source(*b)], y[source(*b)], y[target(*b)])).unwrap_or(std::cmp::Ordering::Equal));
    let mut incoming: Vec<usize> = (0..links.len()).collect();
    incoming.sort_by(|a, b| (y[target(*a)], y[source(*a)]).partial_cmp(&(y[target(*b)], y[source(*b)])).unwrap_or(std::cmp::Ordering::Equal));
    let mut entry = vec![0.0; links.len()];
    let mut filled = vec![0.0; nodes.len()];
    for link in incoming { let to = target(link); entry[link] = y[to] + filled[to]; filled[to] += links[link].value * scale; }
    let mut sent = vec![0.0; nodes.len()];
    for link in order {
        let (from, to) = (source(link), target(link));
        let thickness = links[link].value * scale;
        let (x0, x1) = (x(from) + bar, x(to));
        let (y0, y1) = (y[from] + sent[from], entry[link]);
        sent[from] += thickness;
        let frame = [x0, y0.min(y1), x1 - x0, (y0.max(y1) + thickness) - y0.min(y1)];
        let normal = |point: [f64; 2]| [((point[0] - frame[0]) / frame[2] * 1e6).round() / 1e6, ((point[1] - frame[1]) / frame[3] * 1e6).round() / 1e6];
        let middle = (x0 + x1) / 2.0;
        let commands = vec![
            PathCommand::Move { point: normal([x0, y0]) },
            PathCommand::Cubic { control1: normal([middle, y0]), control2: normal([middle, y1]), point: normal([x1, y1]) },
            PathCommand::Line { point: normal([x1, y1 + thickness]) },
            PathCommand::Cubic { control1: normal([middle, y1 + thickness]), control2: normal([middle, y0 + thickness]), point: normal([x0, y0 + thickness]) },
            PathCommand::Close,
        ];
        path_polygon(drawing, frame, commands, &color[from], Some(0.35));
    }
    for (position, node) in nodes.iter().enumerate() {
        let (bar_x, bar_y, bar_height) = (x(position), y[position], value[position] * scale);
        shape(drawing, Fill::solid("rect", &color[position]), [bar_x, bar_y, bar, bar_height.max(1.0)], None);
        let amount = format!(" {}{unit}", figure(value[position]));
        let content = Rich::default().runs(&[(node.label.as_str(), 14.0, "@dk1", true), (amount.as_str(), SMALL, "@dk2", false)], 0).finish();
        let middle = bar_y + bar_height / 2.0;
        if stage[position] == 0 { text(drawing, [bar_x - 8.0 - label_width, middle - 11.0, label_width, 22.0], content, TextAlign::Right, VerticalAlign::Middle); }
        else {
            let span = if stage[position] + 1 == stages { label_width } else { middle_label };
            text(drawing, [bar_x + bar + 8.0, middle - 11.0, span, 22.0], content, TextAlign::Left, VerticalAlign::Middle);
        }
    }
    Ok(Drawn { bottom: top + height, flexible: Vec::new() })
}

#[allow(clippy::too_many_arguments)]
fn journey(drawing: &mut Drawing, stages: &[String], rows: &[JourneyRow], emotions: &[i8], notes: &[String], emotion_label: &str, highlight: Option<usize>, frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let label_width = (width * 0.11).clamp(96.0, 140.0);
    let column = (width - label_width) / stages.len() as f64;
    let header = 40.0;
    let curve = 64.0;
    let emotion_height = if emotions.is_empty() { 0.0 } else if notes.iter().all(String::is_empty) { curve + 10.0 } else { curve + 46.0 };
    let gap = 6.0;
    let body_top = top + header + 10.0 + emotion_height;
    let row_height = ((top + height - body_top - gap * (rows.len() - 1) as f64) / rows.len() as f64).min(120.0);
    if column < 120.0 || row_height < 52.0 { return Err(too_small("journey stages and rows")); }
    let bottom = body_top + row_height * rows.len() as f64 + gap * (rows.len() - 1) as f64;
    let x = |stage: usize| left + label_width + stage as f64 * column;
    if let Some(stage) = highlight { shape(drawing, Fill::solid("rect", &mix("@accent2", theme, 0.9)), [x(stage), top + header + 4.0, column, bottom - top - header - 4.0], None); }
    for (index, stage) in stages.iter().enumerate() {
        let fill = if highlight == Some(index) { "@dk1" } else { "@accent1" };
        let preset = if index == 0 { "homePlate" } else { "chevron" };
        let padding = if index == 0 { [12.0, 2.0, 22.0, 2.0] } else { [24.0, 2.0, 22.0, 2.0] };
        shape(drawing, Fill::solid(preset, fill), [x(index) + 2.0, top, column - 4.0, header - 4.0], Some((label(stage, 14.0, "@lt1", true), TextAlign::Center, padding)));
    }
    if !emotions.is_empty() {
        let region = top + header + 10.0;
        let middle = region + curve / 2.0;
        text(drawing, [left, region, label_width - 10.0, curve], label(emotion_label, 14.0, "@dk1", true), TextAlign::Left, VerticalAlign::Middle);
        shape(drawing, Fill::solid("rect", &mix("@dk2", theme, 0.8)), [x(0) + 8.0, middle - 0.5, column * stages.len() as f64 - 16.0, 1.0], None);
        let points: Vec<[f64; 2]> = emotions.iter().enumerate().map(|(index, value)| [x(index) + column / 2.0, middle - f64::from(*value) * 13.0]).collect();
        for pair in points.windows(2) { connector(drawing, pair[0], pair[1], "@dk2", 2.0, false); }
        for (point, value) in points.iter().zip(emotions) {
            let fill = match value.signum() { 1 => "@accent1", -1 => "@accent2", _ => "@dk2" };
            shape(drawing, Fill { preset: "ellipse", fill, stroke: "@lt1", stroke_width: 2.0, radius: None }, [point[0] - 8.0, point[1] - 8.0, 16.0, 16.0], None);
        }
        for (index, note) in notes.iter().enumerate() {
            if !note.is_empty() { text(drawing, [x(index) + 8.0, region + curve + 4.0, column - 16.0, 36.0], label(note, 12.0, "@dk2", false), TextAlign::Center, VerticalAlign::Top); }
        }
    }
    let tint = mix("@dk2", theme, 0.92);
    let boxed = mix("@accent1", theme, 0.9);
    for (row_index, row) in rows.iter().enumerate() {
        let y = body_top + row_index as f64 * (row_height + gap);
        shape(drawing, Fill::solid("rect", &tint), [left, y, label_width - 8.0, row_height], Some((label(&row.label, 14.0, "@dk1", true), TextAlign::Left, [10.0, 8.0, 8.0, 8.0])));
        for (index, cell) in row.cells.iter().enumerate() {
            if cell.is_empty() { continue; }
            let bounds = [x(index) + 6.0, y, column - 12.0, row_height];
            if row.boxed { shape(drawing, Fill::solid("rect", &boxed), bounds, Some((label(cell, 13.0, "@dk1", false), TextAlign::Left, [10.0, 8.0, 10.0, 8.0]))); }
            else { text(drawing, [bounds[0] + 4.0, y + 4.0, bounds[2] - 8.0, row_height - 8.0], label(cell, 13.0, "@dk1", false), TextAlign::Left, VerticalAlign::Top); }
        }
    }
    Ok(Drawn { bottom, flexible: Vec::new() })
}

pub(super) fn graph(spec: &PartSpec, theme: &Theme) -> Result<Option<GraphSpec>> {
    if !matches!(spec.data, PartData::Swimlane { .. } | PartData::Architecture { .. }) { return Ok(None); }
    graph_data(spec, theme).map(Some)
}

fn graph_data(spec: &PartSpec, theme: &Theme) -> Result<GraphSpec> {
    let (nodes, edges, groups) = match &spec.data {
        PartData::Swimlane { lanes, steps, flows } => swimlane(lanes, steps, flows, theme)?,
        PartData::Architecture { system, elements, relations } => architecture(system, elements, relations, theme)?,
        _ => return Err(Error::Invalid("graph part data required".into())),
    };
    let graph: GraphSpec = serde_json::from_value(json!({"version":1,"title":spec.title,"subtitle":spec.subtitle,"nodes":nodes,"edges":edges,"groups":groups}))
        .map_err(|error| Error::Invalid(format!("graph part conversion failed: {error}")))?;
    crate::graphs::validate(&graph)?;
    // The engine draws routes as given without obstacle avoidance, so refuse any route that would run through another box.
    let (what, hint) = match &spec.data {
        PartData::Swimlane { .. } => ("swimlane flow", "set explicit step columns so the flow has a free gutter"),
        _ => ("architecture relation", "relate boxes on the edge of a full row, move the relation to a neighboring box or split the view"),
    };
    let node = |id: &str| graph.nodes.iter().find(|node| node.id == id).ok_or_else(|| Error::Invalid("unknown graph node".into()));
    for edge in &graph.edges {
        let points = crate::graphs::edge_points(node(&edge.source)?, node(&edge.target)?, edge);
        for other in graph.nodes.iter().filter(|other| other.id != edge.source && other.id != edge.target) {
            let bounds = [other.x + 1.0, other.y + 1.0, other.width - 2.0, other.height - 2.0];
            if points.windows(2).any(|segment| crosses_box(segment[0], segment[1], bounds)) {
                return Err(Error::Invalid(format!("{what} {} -> {} would run through {}; {hint}", edge.source, edge.target, other.id)));
            }
        }
    }
    Ok(graph)
}

type GraphParts = (Vec<Value>, Vec<Value>, Vec<Value>);

fn round(value: f64) -> f64 { (value * 100.0).round() / 100.0 }

fn swimlane_columns(steps: &[SwimlaneStep], flows: &[SwimlaneFlow], index: &BTreeMap<&str, usize>) -> Result<Vec<usize>> {
    let forward: Vec<(usize, usize)> = flows.iter().filter(|flow| !flow.exception).map(|flow| (index[flow.from.as_str()], index[flow.to.as_str()])).collect();
    let mut indegree = vec![0usize; steps.len()];
    for &(_, to) in &forward { indegree[to] += 1; }
    let mut ready: std::collections::VecDeque<usize> = (0..steps.len()).filter(|position| indegree[*position] == 0).collect();
    let mut column: Vec<usize> = steps.iter().map(|step| step.column.unwrap_or(0)).collect();
    let mut visited = 0;
    while let Some(position) = ready.pop_front() {
        visited += 1;
        for &(from, to) in forward.iter().filter(|(from, _)| *from == position) {
            if steps[to].column.is_none() { column[to] = column[to].max(column[from] + 1); }
            indegree[to] -= 1;
            if indegree[to] == 0 { ready.push_back(to); }
        }
    }
    if visited != steps.len() { return Err(Error::Invalid("swimlane forward flows form a cycle; mark return or rework flows with exception: true".into())); }
    if column.iter().any(|value| *value >= 7) { return Err(Error::Invalid("swimlane layouts allow at most seven columns (0-6); split longer processes across slides".into())); }
    let mut occupied = BTreeSet::new();
    for (position, step) in steps.iter().enumerate() {
        if !occupied.insert((step.lane, column[position])) { return Err(Error::Invalid(format!("swimlane steps share lane {} column {}; set column explicitly", step.lane, column[position]))); }
    }
    Ok(column)
}

fn swimlane(lanes: &[String], steps: &[SwimlaneStep], flows: &[SwimlaneFlow], theme: &Theme) -> Result<GraphParts> {
    // Five lanes keep 50px steps and seven columns keep 104px steps on the fixed canvas.
    items(lanes.len(), 2, 5, "swimlane lanes")?; items(steps.len(), 2, 16, "swimlane steps")?; items(flows.len(), 1, 24, "swimlane flows")?;
    for lane in lanes { single(lane, 24, "swimlane lane", true)?; }
    let index = unique(steps.iter().map(|step| step.id.as_str()), "swimlane step id")?;
    for step in steps {
        field(&step.label, 40, "swimlane step", true)?;
        if step.lane >= lanes.len() { return Err(Error::Invalid("swimlane step lane must index lanes".into())); }
        if step.id.starts_with("lane-") || step.id.starts_with("flow-") { return Err(Error::Invalid("swimlane step ids must not start with the reserved lane- or flow- prefixes".into())); }
    }
    for flow in flows {
        single(&flow.label, 16, "swimlane flow label", false)?;
        if !index.contains_key(flow.from.as_str()) || !index.contains_key(flow.to.as_str()) || flow.from == flow.to { return Err(Error::Invalid("swimlane flows must connect two different step ids".into())); }
    }
    let column = swimlane_columns(steps, flows, &index)?;
    let columns = column.iter().max().copied().unwrap_or(0) + 1;
    // Lane names sit in a left label column, so flows never cross a full-width header band.
    // Unlabeled groups still reserve the smallest header the engine accepts (8px font needs 22px).
    let (header, gap, label_width) = (22.0, 6.0, 112.0);
    let lane_height = (CANVAS_HEIGHT - gap * (lanes.len() - 1) as f64) / lanes.len() as f64;
    let lane_top = |lane: usize| CANVAS_TOP + lane as f64 * (lane_height + gap);
    let columns_left = 16.0 + label_width;
    let column_width = (1152.0 - columns_left - 16.0) / columns as f64;
    let node_width = (column_width - 40.0).min(190.0);
    let usable = lane_height - header - 8.0;
    if node_width < 96.0 || usable < 40.0 { return Err(too_small("swimlane steps")); }
    let groups = lanes.iter().enumerate().map(|(lane, _)| json!({
        "id": format!("lane-{lane}"), "label": "", "x": 0.0, "y": round(lane_top(lane)), "width": 1152.0, "height": round(lane_height),
        "fill": mix("@dk2", theme, if lane % 2 == 0 { 0.95 } else { 0.975 }), "stroke": mix("@dk2", theme, 0.78), "header_height": header, "header_font_size": 8, "padding": 6,
    })).collect();
    let mut nodes: Vec<Value> = lanes.iter().enumerate().map(|(lane, name)| json!({
        "id": format!("lane-label-{lane}"), "label": name, "kind": "rectangle", "x": 6.0, "y": round(lane_top(lane) + header), "width": label_width, "height": round(lane_height - header - 6.0),
        "fill": mix("@dk2", theme, 0.86), "stroke": mix("@dk2", theme, 0.86), "color": "@dk1", "font_size": 14, "group": format!("lane-{lane}"),
    })).collect();
    let mut bounds = Vec::new();
    nodes.extend(steps.iter().enumerate().map(|(position, step)| {
        let (kind, fill, outline, width, height) = match step.shape {
            StepShape::Task => ("rectangle", mix("@accent1", theme, 0.9), "@accent1".to_string(), node_width, usable.min(58.0)),
            StepShape::Decision => ("diamond", mix("@accent4", theme, 0.82), darken("@accent4", theme, 0.1), node_width, usable.min(76.0)),
            StepShape::Event => ("ellipse", "@lt1".to_string(), "@dk2".to_string(), (node_width * 0.75).max(96.0), usable.min(48.0)),
        };
        let x = columns_left + column[position] as f64 * column_width + (column_width - width) / 2.0;
        let y = lane_top(step.lane) + header + (lane_height - header - 6.0 - height) / 2.0;
        bounds.push([x, y, width, height]);
        json!({"id": step.id, "label": step.label, "kind": kind, "x": round(x), "y": round(y), "width": round(width), "height": round(height), "fill": fill, "stroke": outline, "color": "@dk1", "font_size": 14, "group": format!("lane-{}", step.lane)})
    }));
    // Steps keep at least 20px from every column boundary and sit between the lane header and a bottom band,
    // so returns, exceptions and flows past occupied cells run through those free gutters and channels instead of across steps.
    let occupied: BTreeSet<(usize, usize)> = steps.iter().zip(&column).map(|(step, column)| (step.lane, *column)).collect();
    let blocked = |lanes: std::ops::Range<usize>, columns: std::ops::Range<usize>| lanes.into_iter().any(|lane| columns.clone().any(|column| occupied.contains(&(lane, column))));
    let half_gap = (column_width - node_width) / 2.0;
    let lane_bottom = |lane: usize| lane_top(lane) + lane_height;
    let floor = |lane: usize| steps.iter().zip(&bounds).filter(|(step, _)| step.lane == lane).map(|(_, bounds)| bounds[1] + bounds[3]).fold(lane_top(lane) + header, f64::max);
    // Routes sharing a lane channel take successive slots while space allows; later ones reuse the last slot.
    let mut uses: BTreeMap<(char, usize), usize> = BTreeMap::new();
    let mut slot = |channel: char, index: usize| { let count = uses.entry((channel, index)).or_default(); *count += 1; *count - 1 };
    let below = |lane: usize, slot: usize| { let space = lane_bottom(lane) - 3.0 - floor(lane); (floor(lane) + (space / 2.0).min(12.0) + 8.0 * slot as f64).min(lane_bottom(lane) - 3.0) };
    let above = |lane: usize, slot: usize| lane_top(lane) + header / 2.0 + [0.0, 5.0, -5.0][slot.min(2)];
    // Gutter `index` is the boundary left of that column. Each route takes a free offset into the gap, preferring `side`
    // (toward the step it enters or leaves) and staying 6px from every other vertical, including the turn of each forward
    // elbow (midway between its boxes, so a narrow event node moves it off the boundary). Outer gutters end at the
    // lane-label column and at the lane edge.
    let direct = |flow: &SwimlaneFlow| {
        let (from, to) = (index[flow.from.as_str()], index[flow.to.as_str()]);
        let (low, high) = (steps[from].lane.min(steps[to].lane), steps[from].lane.max(steps[to].lane));
        !flow.exception && column[to] > column[from] && !blocked(low..high + 1, column[from] + 1..column[to])
    };
    let mut taken: Vec<f64> = flows.iter().filter(|flow| direct(flow)).filter_map(|flow| {
        let (from, to) = (index[flow.from.as_str()], index[flow.to.as_str()]);
        (steps[from].lane != steps[to].lane).then(|| (bounds[from][0] + bounds[from][2] + bounds[to][0]) / 2.0)
    }).collect();
    let mut gutter = |index: usize, side: i32| {
        let boundary = columns_left + index as f64 * column_width;
        let low = if index == 0 { 6.0 + label_width } else { boundary - half_gap };
        let high = if index == columns { 1150.0 } else { boundary + half_gap };
        let inside: Vec<f64> = [50, 75, 25, -50, -75, -25].into_iter().map(|share| boundary + half_gap * f64::from(share * side) / 100.0)
            .filter(|x| (low + 4.0..high - 4.0).contains(x)).collect();
        let x = inside.iter().copied().find(|x| taken.iter().all(|other| (other - x).abs() >= 6.0)).unwrap_or(inside.first().copied().unwrap_or(boundary));
        taken.push(x);
        x
    };
    let center = |position: usize| [bounds[position][0] + bounds[position][2] / 2.0, bounds[position][1] + bounds[position][3] / 2.0];
    // Pass 1: ports plus either an engine route or manual waypoints. Manual routes leaving or entering a side port use its
    // upper or lower part (toward the band they run in), so their stubs do not overlap straight flows at the side center.
    let mut offsets: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    let mut plans: Vec<([&str; 2], Option<&str>, Vec<[f64; 2]>)> = Vec::new();
    for (position, flow) in flows.iter().enumerate() {
        let (from, to) = (index[flow.from.as_str()], index[flow.to.as_str()]);
        let (source_lane, target_lane, source_column, target_column) = (steps[from].lane, steps[to].lane, column[from], column[to]);
        let (low, high) = (source_lane.min(target_lane), source_lane.max(target_lane));
        let (source, target) = (center(from), center(to));
        plans.push(if direct(flow) {
            (["right", "left"], Some(if source_lane == target_lane { "straight" } else { "elbow" }), Vec::new())
        } else if !flow.exception && target_column == source_column && !blocked(low + 1..high, source_column..source_column + 1) {
            (if target_lane > source_lane { ["bottom", "top"] } else { ["top", "bottom"] }, None, Vec::new())
        } else if !flow.exception && target_column > source_column {
            // Pass occupied cells through the target lane's empty header band.
            let (exit, enter, y) = (gutter(source_column + 1, -1), gutter(target_column, 1), above(target_lane, slot('t', target_lane)));
            offsets.insert((position, 0), if target_lane > source_lane { 0.3 } else { -0.3 });
            offsets.insert((position, 1), -0.3);
            (["right", "left"], Some("manual"), vec![[exit, source[1]], [exit, y], [enter, y], [enter, target[1]]])
        } else if !flow.exception && target_column == source_column {
            let x = gutter(source_column + 1, -1);
            let down = target_lane > source_lane;
            offsets.insert((position, 0), if down { 0.3 } else { -0.3 });
            offsets.insert((position, 1), if down { -0.3 } else { 0.3 });
            (["right", "right"], Some("manual"), vec![[x, source[1]], [x, target[1]]])
        } else {
            // Returns and exceptions drop into the source lane's bottom channel, cross lanes in the gutter beside the
            // target column and enter the target from its own lane's bottom channel or header band.
            let start = below(source_lane, slot('b', source_lane));
            if source_lane == target_lane {
                (["bottom", "bottom"], Some("manual"), vec![[source[0], start], [target[0], start]])
            } else {
                let x = if source_column < target_column { gutter(target_column, 1) } else { gutter(target_column + 1, -1) };
                let (end, port) = if target_lane < source_lane { (below(target_lane, slot('b', target_lane)), "bottom") } else { (above(target_lane, slot('t', target_lane)), "top") };
                (["bottom", port], Some("manual"), vec![[source[0], start], [x, start], [x, end], [target[0], end]])
            }
        });
    }
    // Pass 2: manual ends sharing a top or bottom side spread apart. Ends heading left sit left of center and ends heading
    // right sit right of it; on each side the end whose channel lies nearest the step sits outermost, so no stub crosses
    // another end's channel segment. A straight vertical flow on that side keeps the center.
    let mut ends: BTreeMap<(usize, &str), (bool, Vec<(bool, f64, usize, usize)>)> = BTreeMap::new();
    for (position, (flow, (ports, route, waypoints))) in flows.iter().zip(&plans).enumerate() {
        for (end, node) in [(0, index[flow.from.as_str()]), (1, index[flow.to.as_str()])] {
            if !matches!(ports[end], "top" | "bottom") { continue; }
            let group = ends.entry((node, ports[end])).or_default();
            if *route != Some("manual") { group.0 = true; continue; }
            let (stub, next) = if end == 0 { (waypoints[0], waypoints[1]) } else { (waypoints[waypoints.len() - 1], waypoints[waypoints.len() - 2]) };
            let edge = if ports[end] == "top" { bounds[node][1] } else { bounds[node][1] + bounds[node][3] };
            group.1.push((next[0] < bounds[node][0] + bounds[node][2] / 2.0, (stub[1] - edge).abs(), position, end));
        }
    }
    for (straight, mut manual) in ends.into_values() {
        if manual.is_empty() || manual.len() == 1 && !straight { continue; }
        manual.sort_by(|left, right| left.1.total_cmp(&right.1).then(left.2.cmp(&right.2)));
        for leftward in [true, false] {
            let side: Vec<_> = manual.iter().filter(|entry| entry.0 == leftward).collect();
            let count = side.len();
            for (order, (_, _, position, end)) in side.into_iter().enumerate() {
                let magnitude = (0.15 + 0.1 * (count - 1 - order) as f64).min(0.45);
                offsets.insert((*position, *end), if leftward { -magnitude } else { magnitude });
            }
        }
    }
    // Pass 3: the stub waypoint moves with its port, keeping both the stub and the next segment axis-aligned.
    let mut edges = Vec::new();
    for (position, (flow, (ports, route, mut waypoints))) in flows.iter().zip(plans).enumerate() {
        let mut edge = json!({"id": format!("flow-{position}"), "source": flow.from, "target": flow.to, "label": flow.label, "label_font_size": 12,
            "color": if flow.exception { "@accent2" } else { "@dk2" }, "dashed": flow.exception, "source_port": ports[0], "target_port": ports[1]});
        if let Some(route) = route { edge["route"] = json!(route); }
        for (end, field, node) in [(0, "source_offset", index[flow.from.as_str()]), (1, "target_offset", index[flow.to.as_str()])] {
            let Some(share) = offsets.get(&(position, end)).map(|share| round(*share)) else { continue };
            edge[field] = json!(share);
            let stub = if end == 0 { 0 } else { waypoints.len() - 1 };
            if matches!(ports[end], "top" | "bottom") { waypoints[stub][0] = round(bounds[node][0]) + (0.5 + share) * round(bounds[node][2]); }
            else { waypoints[stub][1] = round(bounds[node][1]) + (0.5 + share) * round(bounds[node][3]); }
        }
        if !waypoints.is_empty() { edge["waypoints"] = json!(waypoints.iter().map(|point| [round(point[0]), round(point[1])]).collect::<Vec<_>>()); }
        edges.push(edge);
    }
    Ok((nodes, edges, groups))
}

fn architecture(system: &str, elements: &[ArchitectureElement], relations: &[ArchitectureRelation], theme: &Theme) -> Result<GraphParts> {
    single(system, 40, "architecture system", true)?;
    items(elements.len(), 2, 12, "architecture elements")?; items(relations.len(), 1, 16, "architecture relations")?;
    let index = unique(elements.iter().map(|element| element.id.as_str()), "architecture element id")?;
    for element in elements {
        single(&element.label, 32, "architecture label", true)?; single(&element.detail, 48, "architecture detail", false)?;
        if element.id == "system" || element.id.starts_with("relation-") { return Err(Error::Invalid("architecture element ids must not be system or start with relation-".into())); }
    }
    let mut pairs = BTreeSet::new();
    for relation in relations {
        single(&relation.label, 24, "architecture relation label", false)?;
        if !index.contains_key(relation.from.as_str()) || !index.contains_key(relation.to.as_str()) || relation.from == relation.to || !pairs.insert((&relation.from, &relation.to)) {
            return Err(Error::Invalid("architecture relations must connect two different element ids, once per direction".into()));
        }
    }
    let of = |kind: ArchitectureKind| elements.iter().enumerate().filter(|(_, element)| element.kind == kind).map(|(position, _)| position).collect::<Vec<_>>();
    let (people, externals, containers, databases) = (of(ArchitectureKind::Person), of(ArchitectureKind::External), of(ArchitectureKind::Container), of(ArchitectureKind::Database));
    if containers.is_empty() && databases.is_empty() { return Err(Error::Invalid("architecture requires at least one container or database inside the system boundary".into())); }
    if people.len() > 3 || externals.len() > 3 { return Err(Error::Invalid("architecture allows at most three people and three external systems".into())); }
    // Containers fill rows of up to three and databases start a new row; three rows fit the boundary on the fixed canvas.
    let inner_rows = containers.len().div_ceil(3) + databases.len().div_ceil(3);
    if inner_rows > 3 {
        return Err(Error::Invalid(format!("architecture boundary holds at most three rows of up to three boxes, and databases start a new row; {} containers and {} databases need {inner_rows} rows (for example, six containers and three databases fit)", containers.len(), databases.len())));
    }
    let (side, gap, header) = (196.0, 44.0, 34.0);
    let boundary_left = if people.is_empty() { 16.0 } else { 16.0 + side + gap };
    let boundary_right = if externals.is_empty() { 1136.0 } else { 1136.0 - side - gap };
    let boundary_width = boundary_right - boundary_left;
    let groups = vec![json!({"id": "system", "label": system, "x": boundary_left, "y": CANVAS_TOP, "width": boundary_width, "height": CANVAS_HEIGHT,
        "fill": mix("@accent3", theme, 0.95), "stroke": "@accent3", "header_height": header, "header_font_size": 15, "header_color": darken("@accent3", theme, 0.2), "padding": 12})];
    let links: Vec<(usize, usize)> = relations.iter().map(|relation| (index[relation.from.as_str()], index[relation.to.as_str()])).collect();
    let mut best: Option<(f64, Vec<[f64; 4]>, Vec<usize>)> = None;
    // Try one to three inner columns; widest wins ties, so the default reads left to right.
    for grid in (1..=3usize).rev() {
        let rows = containers.len().div_ceil(grid) + databases.len().div_ceil(grid);
        let pitch = (CANVAS_HEIGHT - header - 20.0) / rows as f64;
        // Rounded boxes inset text by 10% of their height, so at least 80px keeps the 8px container padding.
        let height = (pitch - 36.0).min(96.0);
        let column = (boundary_width - 24.0) / grid as f64;
        let width = (column - 56.0).min(230.0);
        if rows > 3 || height < 80.0 || width < 110.0 { continue; }
        let mut slots: Vec<[f64; 4]> = Vec::new();
        let mut occupants: Vec<usize> = Vec::new();
        for (members, x) in [(&people, 16.0), (&externals, 1136.0 - side)] {
            let side_pitch = CANVAS_HEIGHT / members.len().max(1) as f64;
            let side_height = (side_pitch - 40.0).min(96.0);
            for (position, element) in members.iter().enumerate() { slots.push([x, CANVAS_TOP + position as f64 * side_pitch + (side_pitch - side_height) / 2.0, side, side_height]); occupants.push(*element); }
        }
        let mut row = 0;
        for members in [&containers, &databases] {
            for chunk in members.chunks(grid) {
                // Rows share one column grid, so shorter rows center under full ones and vertical relations stay exactly aligned.
                let offset = (grid - chunk.len()) as f64 / 2.0;
                for (position, element) in chunk.iter().enumerate() {
                    slots.push([boundary_left + 12.0 + (offset + position as f64) * column + (column - width) / 2.0, CANVAS_TOP + header + 8.0 + row as f64 * pitch + (pitch - height) / 2.0, width, height]);
                    occupants.push(*element);
                }
                row += 1;
            }
        }
        let (cost, occupants) = arrange(&slots, occupants, elements, &links);
        if best.as_ref().is_none_or(|(current, ..)| cost + 1e-9 < *current) { best = Some((cost, slots, occupants)); }
    }
    // Three columns always fit once inner_rows <= 3 (87px boxes at least 149px wide), so a candidate always exists.
    let (_, slots, occupants) = best.ok_or_else(|| Error::Invalid("architecture layout found no arrangement".into()))?;
    let style = |kind: ArchitectureKind| -> (&'static str, String, String, &'static str) {
        match kind {
            ArchitectureKind::Person => ("rounded_rectangle", darken("@accent1", theme, 0.1), darken("@accent1", theme, 0.3), "@lt1"),
            ArchitectureKind::Container => ("rounded_rectangle", mix("@accent3", theme, 0.84), "@accent3".into(), "@dk1"),
            ArchitectureKind::Database => ("cylinder", mix("@accent3", theme, 0.84), "@accent3".into(), "@dk1"),
            ArchitectureKind::External => ("rectangle", mix("@dk2", theme, 0.86), "@dk2".into(), "@dk1"),
        }
    };
    let nodes = slots.iter().zip(&occupants).map(|(bounds, position)| {
        let element = &elements[*position];
        let (kind, fill, stroke, color) = style(element.kind);
        let mut value = json!({"id": element.id, "label": element.label, "kind": kind, "x": round(bounds[0]), "y": round(bounds[1]), "width": round(bounds[2]), "height": round(bounds[3]),
            "fill": fill, "stroke": stroke, "color": color, "font_size": 15});
        if !element.detail.is_empty() { value["detail"] = json!(element.detail); value["detail_font_size"] = json!(12); }
        if matches!(element.kind, ArchitectureKind::Container | ArchitectureKind::Database) { value["group"] = json!("system"); }
        value
    }).collect();
    let edges: Vec<Value> = relations.iter().enumerate().map(|(position, relation)| json!({"id": format!("relation-{position}"), "source": relation.from, "target": relation.to, "label": relation.label, "label_font_size": 12, "color": "@dk2"})).collect();
    let mut slot_of = vec![0; elements.len()];
    for (slot, element) in occupants.iter().enumerate() { slot_of[*element] = slot; }
    let boxes: Vec<[f64; 4]> = slot_of.iter().map(|slot| slots[*slot]).collect();
    let edges = relation_ports(edges, &links, &boxes);
    Ok((nodes, edges, groups))
}

/// Explicit ports that face each other across the larger gap. Automatic ports can pick left/right for boxes
/// that overlap horizontally, which runs the line back through both boxes and hides its arrowhead.
/// When that straight line would cross another box (stacked people reaching one container), the other facing pair is used.
/// Several relations on one side are spread in the order of their far ends, so they neither share a point nor cross.
fn relation_ports(mut edges: Vec<Value>, links: &[(usize, usize)], boxes: &[[f64; 4]]) -> Vec<Value> {
    let mut sides: BTreeMap<(usize, &str), Vec<(f64, usize, &str)>> = BTreeMap::new();
    let port = |bounds: [f64; 4], side: &str| match side {
        "right" => [bounds[0] + bounds[2], bounds[1] + bounds[3] / 2.0], "left" => [bounds[0], bounds[1] + bounds[3] / 2.0],
        "bottom" => [bounds[0] + bounds[2] / 2.0, bounds[1] + bounds[3]], _ => [bounds[0] + bounds[2] / 2.0, bounds[1]],
    };
    for (position, &(from, to)) in links.iter().enumerate() {
        let (source, target) = (boxes[from], boxes[to]);
        let horizontal = (target[0] - source[0] - source[2]).max(source[0] - target[0] - target[2]).max(0.0);
        let vertical = (target[1] - source[1] - source[3]).max(source[1] - target[1] - target[3]).max(0.0);
        let across = if target[0] > source[0] { ("right", "left") } else { ("left", "right") };
        let down = if target[1] > source[1] { ("bottom", "top") } else { ("top", "bottom") };
        let crosses = |(start, end): (&str, &str)| boxes.iter().enumerate().any(|(element, bounds)| element != from && element != to
            && crosses_box(port(source, start), port(target, end), [bounds[0] + 1.0, bounds[1] + 1.0, bounds[2] - 2.0, bounds[3] - 2.0]));
        let (preferred, other) = if horizontal >= vertical { (across, down) } else { (down, across) };
        let (source_port, target_port) = if crosses(preferred) && !crosses(other) { other } else { preferred };
        edges[position]["source_port"] = json!(source_port);
        edges[position]["target_port"] = json!(target_port);
        let along = |far: [f64; 4], port: &str| if matches!(port, "top" | "bottom") { far[0] + far[2] / 2.0 } else { far[1] + far[3] / 2.0 };
        sides.entry((from, source_port)).or_default().push((along(target, source_port), position, "source_offset"));
        sides.entry((to, target_port)).or_default().push((along(source, target_port), position, "target_offset"));
    }
    for mut uses in sides.into_values().filter(|uses| uses.len() > 1) {
        uses.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)));
        let last = (uses.len() - 1) as f64;
        for (order, (_, position, field)) in uses.into_iter().enumerate() { edges[position][field] = json!(round(-0.3 + 0.6 * order as f64 / last)); }
    }
    edges
}

fn crosses_box(start: [f64; 2], end: [f64; 2], bounds: [f64; 4]) -> bool {
    let (dx, dy) = (end[0] - start[0], end[1] - start[1]);
    let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
    for (direction, distance) in [(-dx, start[0] - bounds[0]), (dx, bounds[0] + bounds[2] - start[0]), (-dy, start[1] - bounds[1]), (dy, bounds[1] + bounds[3] - start[1])] {
        if direction == 0.0 { if distance < 0.0 { return false; } continue; }
        let ratio = distance / direction;
        if direction < 0.0 { enter = enter.max(ratio); } else { leave = leave.min(ratio); }
        if enter > leave { return false; }
    }
    true
}

fn segments_cross(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let orient = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    orient(a, b, c) * orient(a, b, d) < 0.0 && orient(c, d, a) * orient(c, d, b) < 0.0
}

/// Deterministic local search: swap elements of the same kind between slots while relation lines pass through
/// fewer unrelated boxes (100 each), cross fewer other relations (10 each) or get shorter (0.01 per pixel).
fn arrange(slots: &[[f64; 4]], mut occupants: Vec<usize>, elements: &[ArchitectureElement], links: &[(usize, usize)]) -> (f64, Vec<usize>) {
    let cost = |occupants: &[usize]| -> f64 {
        let mut slot_of = vec![0; elements.len()];
        for (slot, element) in occupants.iter().enumerate() { slot_of[*element] = slot; }
        let center = |element: usize| { let bounds = slots[slot_of[element]]; [bounds[0] + bounds[2] / 2.0, bounds[1] + bounds[3] / 2.0] };
        let mut total = 0.0;
        for (position, &(from, to)) in links.iter().enumerate() {
            let (start, end) = (center(from), center(to));
            total += (end[0] - start[0]).hypot(end[1] - start[1]) * 0.01;
            for (slot, bounds) in slots.iter().enumerate() {
                if occupants[slot] != from && occupants[slot] != to && crosses_box(start, end, [bounds[0] - 6.0, bounds[1] - 6.0, bounds[2] + 12.0, bounds[3] + 12.0]) { total += 100.0; }
            }
            for &(other_from, other_to) in &links[position + 1..] {
                if ![other_from, other_to].iter().any(|element| *element == from || *element == to) && segments_cross(start, end, center(other_from), center(other_to)) { total += 10.0; }
            }
        }
        total
    };
    let mut best = cost(&occupants);
    for _ in 0..32 {
        let mut improved = false;
        for first in 0..slots.len() {
            for second in first + 1..slots.len() {
                if elements[occupants[first]].kind != elements[occupants[second]].kind { continue; }
                occupants.swap(first, second);
                let candidate = cost(&occupants);
                if candidate + 1e-9 < best { best = candidate; improved = true; } else { occupants.swap(first, second); }
            }
        }
        if !improved { break; }
    }
    (best, occupants)
}
