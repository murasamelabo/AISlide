use crate::{design::Theme, model::{Connection, Element, TextAlign, TextFormat, VerticalAlign, valid_color, valid_text, validate_elements}, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const WIDTH: f64 = 1152.0;
pub const HEIGHT: f64 = 512.0;
pub const CONTENT_TOP: f64 = 88.0;

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphSpec {
    pub version: u32,
    pub title: String,
    #[serde(default)] pub subtitle: String,
    pub nodes: Vec<GraphNode>,
    #[serde(default)] pub edges: Vec<GraphEdge>,
    #[serde(default)] pub groups: Vec<GraphGroup>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind { #[default] Rectangle, RoundedRectangle, Ellipse, Diamond, Cylinder, Cloud }
impl NodeKind {
    pub fn preset(self) -> &'static str {
        match self { Self::Rectangle => "rect", Self::RoundedRectangle => "roundRect", Self::Ellipse => "ellipse", Self::Diamond => "diamond", Self::Cylinder => "can", Self::Cloud => "cloud" }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Port { #[default] Auto, Top, Left, Bottom, Right }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Route { #[default] Straight, Elbow }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Alignment { Left, Center, Right, Top, Middle, Bottom }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum GraphOperation {
    PutNode { node: GraphNode },
    PutEdge { edge: GraphEdge },
    PutGroup { group: GraphGroup },
    Move { ids: Vec<String>, dx: f64, dy: f64 },
    Remove { ids: Vec<String> },
    Align { ids: Vec<String>, alignment: Alignment },
    Layout { columns: usize },
}

fn node_width() -> f64 { 176.0 }
fn node_height() -> f64 { 80.0 }
fn font_size() -> f64 { 18.0 }
fn paper() -> String { "@lt1".into() }
fn ink() -> String { "@dk1".into() }
fn accent() -> String { "@accent1".into() }
fn muted() -> String { "@dk2".into() }
fn surface() -> String { "@lt2".into() }
fn arrow() -> bool { true }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphIcon {
    pub base64: String,
    pub mime_type: String,
    #[serde(default)] pub alt: String,
}

pub fn create_icon(base64: String, mime_type: &str, alt: &str) -> Result<GraphIcon> {
    valid_text(alt, 500)?;
    let (base64, mime_type) = crate::media::icon_raster(base64, mime_type)?;
    Ok(GraphIcon { base64, mime_type, alt: alt.into() })
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphNode {
    pub id: String, pub label: String,
    #[serde(default)] pub kind: NodeKind,
    pub x: f64, pub y: f64,
    #[serde(default = "node_width")] pub width: f64,
    #[serde(default = "node_height")] pub height: f64,
    #[serde(default = "paper")] pub fill: String,
    #[serde(default = "accent")] pub stroke: String,
    #[serde(default = "ink")] pub color: String,
    #[serde(default = "font_size")] pub font_size: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub icon: Option<GraphIcon>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphEdge {
    pub id: String, pub source: String, pub target: String,
    #[serde(default)] pub source_port: Port,
    #[serde(default)] pub target_port: Port,
    #[serde(default)] pub label: String,
    #[serde(default)] pub route: Route,
    #[serde(default = "muted")] pub color: String,
    #[serde(default = "arrow")] pub arrow: bool,
    #[serde(default)] pub start_arrow: bool,
    #[serde(default)] pub dashed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphGroup {
    pub id: String, pub label: String,
    pub x: f64, pub y: f64, pub width: f64, pub height: f64,
    #[serde(default = "surface")] pub fill: String,
    #[serde(default = "muted")] pub stroke: String,
}

fn identity(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 24 || !id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) {
        return Err(Error::Invalid("graph IDs require 1-24 ASCII letters, digits, hyphens or underscores".into()));
    }
    Ok(())
}

fn bounds(x: f64, y: f64, width: f64, height: f64) -> Result<()> {
    if [x, y, width, height].iter().any(|value| !value.is_finite()) || x < 0.0 || y < CONTENT_TOP || width < 64.0 || height < 40.0 || x + width > WIDTH || y + height > HEIGHT {
        return Err(Error::Invalid("graph bounds must fit 1152x512, below the 88px title band, with minimum size 64x40".into()));
    }
    Ok(())
}

pub fn validate(spec: &GraphSpec) -> Result<()> {
    if spec.version != 1 { return Err(Error::Unsupported("graph version".into())); }
    valid_text(&spec.title, 80)?; valid_text(&spec.subtitle, 120)?;
    if spec.nodes.is_empty() || spec.nodes.len() > 48 || spec.edges.len() > 64 || spec.groups.len() > 8 { return Err(Error::Limit("graph requires 1-48 nodes, at most 64 edges and 8 groups".into())); }
    if spec.nodes.iter().filter_map(|node| node.icon.as_ref()).fold(0usize, |total, icon| total.saturating_add(icon.base64.len())) > 3 * 1024 * 1024 {
        return Err(Error::Limit("graph icon data exceeds the 3 MiB encoded image budget".into()));
    }
    let mut ids = BTreeSet::new();
    for region in &spec.groups {
        identity(&region.id)?; valid_text(&region.label, 64)?;
        if !ids.insert(&region.id) { return Err(Error::Invalid("duplicate graph ID".into())); }
        bounds(region.x, region.y, region.width, region.height)?;
        valid_color(&region.fill)?; valid_color(&region.stroke)?;
    }
    for node in &spec.nodes {
        identity(&node.id)?; valid_text(&node.label, 160)?;
        if !ids.insert(&node.id) { return Err(Error::Invalid("duplicate graph ID".into())); }
        bounds(node.x, node.y, node.width, node.height)?;
        valid_color(&node.fill)?; valid_color(&node.stroke)?; valid_color(&node.color)?;
        if !node.font_size.is_finite() || !(12.0..=40.0).contains(&node.font_size) { return Err(Error::Invalid("graph font size must be 12-40".into())); }
        if let Some(icon) = &node.icon {
            valid_text(&icon.alt, 500)?;
            crate::media::inspect_raster(&icon.base64, &icon.mime_type)?;
        }
        if let Some(parent) = &node.group {
            let region = spec.groups.iter().find(|region| &region.id == parent).ok_or_else(|| Error::Invalid("unknown graph group".into()))?;
            if node.x < region.x + 8.0 || node.y < region.y + 40.0 || node.x + node.width > region.x + region.width - 8.0 || node.y + node.height > region.y + region.height - 8.0 { return Err(Error::Invalid("graph node must fit inside its group below the group label".into())); }
        }
    }
    for edge in &spec.edges {
        identity(&edge.id)?; valid_text(&edge.label, 64)?; valid_color(&edge.color)?;
        if !ids.insert(&edge.id) { return Err(Error::Invalid("duplicate graph ID".into())); }
        if edge.source == edge.target || !spec.nodes.iter().any(|node| node.id == edge.source) || !spec.nodes.iter().any(|node| node.id == edge.target) { return Err(Error::Invalid("graph edges require distinct existing node endpoints".into())); }
    }
    Ok(())
}

fn text(id: String, rect: [f64; 4], label: &str, size: f64, color: &str, alignment: TextAlign, bold: bool) -> Element {
    let [x, y, width, height] = rect;
    Element::Text { id, x, y, width, height, text: label.into(), font_size: size, color: color.into(), bold, format: TextFormat { alignment, vertical: VerticalAlign::Middle, font_family: Some("@minor".into()), ..Default::default() } }
}

fn shape(id: String, rect: [f64; 4], preset: &str, fill: &str, stroke: &str) -> Element {
    let [x, y, width, height] = rect;
    Element::Shape { id, x, y, width, height, preset: preset.into(), fill: fill.into(), stroke: stroke.into(), stroke_width: 1.5, rotation: 0.0, text: String::new(), font_size: 18.0, color: "@dk1".into(), bold: false, format: Default::default() }
}

fn resolved_port(node: &GraphNode, port: Port, other: &GraphNode) -> Port {
    if port == Port::Auto {
        let horizontal = other.x + other.width / 2.0 - node.x - node.width / 2.0;
        let vertical = other.y + other.height / 2.0 - node.y - node.height / 2.0;
        if horizontal.abs() >= vertical.abs() { if horizontal >= 0.0 { Port::Right } else { Port::Left } }
        else if vertical >= 0.0 { Port::Bottom } else { Port::Top }
    } else { port }
}

pub fn endpoint(node: &GraphNode, port: Port, other: &GraphNode) -> (f64, f64, u32) {
    let index = match resolved_port(node, port, other) { Port::Top => 0, Port::Left => 1, Port::Bottom => 2, Port::Right | Port::Auto => 3 };
    let sites = match node.kind { NodeKind::Ellipse => [0,2,4,6], NodeKind::Cylinder => [1,2,3,4], NodeKind::Cloud => [3,2,1,0], _ => [0,1,2,3] };
    let positions = if node.kind == NodeKind::Cloud { [[0.5,1235.0/21600.0],[67.0/21600.0,0.5],[0.5,21577.0/21600.0],[21582.0/21600.0,0.5]] } else { [[0.5,0.0],[0.0,0.5],[0.5,1.0],[1.0,0.5]] };
    (node.x + positions[index][0] * node.width, node.y + positions[index][1] * node.height, sites[index])
}

pub fn edge_points(source: &GraphNode, target: &GraphNode, edge: &GraphEdge) -> Vec<[f64; 2]> {
    let start = endpoint(source, edge.source_port, target);
    let end = endpoint(target, edge.target_port, source);
    let mut points = vec![[start.0, start.1]];
    if edge.route == Route::Elbow && start.0 != end.0 && start.1 != end.1 {
        let start_horizontal = matches!(resolved_port(source, edge.source_port, target), Port::Left | Port::Right);
        let end_horizontal = matches!(resolved_port(target, edge.target_port, source), Port::Left | Port::Right);
        if start_horizontal && end_horizontal { let middle = (start.0 + end.0) / 2.0; points.extend([[middle, start.1], [middle, end.1]]); }
        else if !start_horizontal && !end_horizontal { let middle = (start.1 + end.1) / 2.0; points.extend([[start.0, middle], [end.0, middle]]); }
        else if start_horizontal { points.push([end.0, start.1]); }
        else { points.push([start.0, end.1]); }
    }
    points.push([end.0, end.1]);
    points
}

pub fn create(id: &str, spec: &GraphSpec, theme: &Theme) -> Result<Element> {
    valid_text(id, 40)?;
    if id.is_empty() { return Err(Error::Invalid("graph root ID is required".into())); }
    validate(spec)?; crate::design::validate_theme(theme)?;
    let prefix = format!("{id}-{}", &format!("{:x}", Sha256::digest(crate::canonical::bytes(spec)?))[..10]);
    let mut children = vec![text(format!("{prefix}-title"), [16.0, 0.0, 1120.0, 40.0], &spec.title, 28.0, "@dk1", TextAlign::Left, true), text(format!("{prefix}-subtitle"), [16.0, 44.0, 1120.0, 26.0], &spec.subtitle, 16.0, "@dk2", TextAlign::Left, false)];
    for region in &spec.groups {
        children.push(shape(format!("{prefix}-g-{}", region.id), [region.x, region.y, region.width, region.height], "rect", &region.fill, &region.stroke));
        children.push(text(format!("{prefix}-gt-{}", region.id), [region.x + 12.0, region.y + 8.0, region.width - 24.0, 28.0], &region.label, 18.0, "@dk1", TextAlign::Left, true));
    }
    for edge in &spec.edges {
        let source = spec.nodes.iter().find(|node| node.id == edge.source).ok_or_else(|| Error::Invalid("unknown source node".into()))?;
        let target = spec.nodes.iter().find(|node| node.id == edge.target).ok_or_else(|| Error::Invalid("unknown target node".into()))?;
        let start = endpoint(source, edge.source_port, target);
        let end = endpoint(target, edge.target_port, source);
        let points = edge_points(source, target, edge);
        let left = points.iter().map(|point| point[0]).fold(f64::INFINITY, f64::min); let top = points.iter().map(|point| point[1]).fold(f64::INFINITY, f64::min);
        let width = (points.iter().map(|point| point[0]).fold(f64::NEG_INFINITY, f64::max) - left).max(0.01); let height = (points.iter().map(|point| point[1]).fold(f64::NEG_INFINITY, f64::max) - top).max(0.01);
        let points = points.into_iter().map(|point| [((point[0] - left) / width * 1e6).round() / 1e6, ((point[1] - top) / height * 1e6).round() / 1e6]).collect();
        children.push(Element::Connector { id: format!("{prefix}-e-{}", edge.id), x: left, y: top, width, height, color: edge.color.clone(), stroke_width: 2.0, arrow: edge.arrow, flip_v: false,
            start: Some(Connection { element_id: format!("{prefix}-n-{}", source.id), site: start.2 }), end: Some(Connection { element_id: format!("{prefix}-n-{}", target.id), site: end.2 }), routing: Some(crate::model::ConnectorRouting { points, start_arrow: edge.start_arrow, dashed: edge.dashed }) });
    }
    for node in &spec.nodes {
        children.push(shape(format!("{prefix}-n-{}", node.id), [node.x, node.y, node.width, node.height], node.kind.preset(), &node.fill, &node.stroke));
        let inset = match node.kind { NodeKind::Diamond => 0.24, NodeKind::Ellipse | NodeKind::Cloud => 0.18, _ => 0.1 };
        let mut content = [node.x + node.width * inset, node.y + node.height * inset, node.width * (1.0 - 2.0 * inset), node.height * (1.0 - 2.0 * inset)];
        if let Some(icon) = &node.icon {
            let mut picture = crate::media::create_picture(&format!("{prefix}-ni-{}", node.id), icon.base64.clone(), &icon.mime_type, &icon.alt)?;
            let size = (content[2] / 4.0).min(content[3]).min(48.0);
            if let Element::Picture { x, y, width, height, .. } = &mut picture {
                let scale = size / width.max(*height);
                *width *= scale; *height *= scale;
                *x = content[0] + (size - *width) / 2.0;
                *y = content[1] + (content[3] - *height) / 2.0;
            }
            children.push(picture);
            let offset = size + 8.0_f64.min(content[2] / 8.0);
            content[0] += offset; content[2] -= offset;
        }
        children.push(text(format!("{prefix}-nt-{}", node.id), content, &node.label, node.font_size, &node.color, TextAlign::Center, true));
    }
    for edge in &spec.edges {
        if edge.label.is_empty() { continue; }
        let source = spec.nodes.iter().find(|node| node.id == edge.source).ok_or_else(|| Error::Invalid("unknown source node".into()))?;
        let target = spec.nodes.iter().find(|node| node.id == edge.target).ok_or_else(|| Error::Invalid("unknown target node".into()))?;
        let start = endpoint(source, edge.source_port, target); let end = endpoint(target, edge.target_port, source);
        let width = (edge.label.chars().count() as f64 * 12.0 + 24.0).clamp(64.0, 280.0);
        let mut label = shape(format!("{prefix}-et-{}", edge.id), [((start.0 + end.0 - width) / 2.0).clamp(0.0, WIDTH - width), ((start.1 + end.1) / 2.0 - 16.0).clamp(CONTENT_TOP, HEIGHT - 32.0), width, 32.0], "rect", "@lt1", "@lt1");
        if let Element::Shape { text, font_size, format, stroke_width, .. } = &mut label { *text = edge.label.clone(); *font_size = 16.0; *stroke_width = 0.0; *format = TextFormat { alignment: TextAlign::Center, vertical: VerticalAlign::Middle, font_family: Some("@minor".into()), ..Default::default() }; }
        children.push(label);
    }
    crate::layout::fit_part_text(&mut children, theme)?;
    let result = Element::Group { id: id.into(), x: 64.0, y: 144.0, width: WIDTH, height: HEIGHT, view_width: WIDTH, view_height: HEIGHT, children };
    validate_elements(std::slice::from_ref(&result), (1280.0, 720.0), 0, &mut BTreeSet::new(), &mut 0, &mut 0)?;
    Ok(result)
}

pub fn catalog() -> Value {
    let examples = [
        json!({"id":"service","name":"Service architecture","spec":{"version":1,"title":"Service architecture","subtitle":"Editable example","nodes":[{"id":"user","label":"Client","kind":"rounded_rectangle","x":40,"y":220},{"id":"api","label":"API service","kind":"rectangle","x":390,"y":220},{"id":"data","label":"Database","kind":"cylinder","x":780,"y":220}],"edges":[{"id":"request","source":"user","target":"api","label":"HTTPS"},{"id":"query","source":"api","target":"data","label":"Query"}],"groups":[]}}),
        json!({"id":"approval","name":"Approval flow","spec":{"version":1,"title":"Approval flow","subtitle":"Editable example","nodes":[{"id":"input","label":"Request","x":40,"y":220},{"id":"review","label":"Review","kind":"diamond","x":390,"y":190,"width":200,"height":140},{"id":"approve","label":"Approved","kind":"rounded_rectangle","x":830,"y":120},{"id":"revise","label":"Revise","x":830,"y":340}],"edges":[{"id":"submit","source":"input","target":"review"},{"id":"yes","source":"review","target":"approve","label":"Yes"},{"id":"no","source":"review","target":"revise","label":"No"}],"groups":[]}}),
        json!({"id":"boundary","name":"Network boundary","spec":{"version":1,"title":"Network boundary","subtitle":"Editable example","nodes":[{"id":"client","label":"Client","x":40,"y":210},{"id":"app","label":"Application","x":430,"y":210,"group":"private"},{"id":"store","label":"Storage","kind":"cylinder","x":800,"y":210,"group":"private"}],"edges":[{"id":"access","source":"client","target":"app","label":"Authorized"},{"id":"data","source":"app","target":"store"}],"groups":[{"id":"private","label":"Private network","x":370,"y":120,"width":680,"height":300}]}}),
    ];
    json!({"version":1,"shapes":["rectangle","rounded_rectangle","ellipse","diamond","cylinder","cloud"],"ports":["auto","top","left","bottom","right"],"routes":["straight","elbow"],"limits":{"nodes":48,"edges":64,"groups":8,"rendered_elements":256},"canvas":{"width":WIDTH,"height":HEIGHT,"content_top":CONTENT_TOP},"schema":schemars::schema_for!(GraphSpec),"operation_schema":schemars::schema_for!(GraphOperation),"examples":examples})
}

pub fn change(document: &crate::document::Document, expected_revision: u64, slide_id: &str, id: &str, spec: &GraphSpec, update: bool) -> Result<crate::document::TransactionResult> {
    if update && !document.parts.iter().any(|part| part.slide_id == slide_id && part.element_id == id && part.spec.preset == "diagram/custom") {
        return Err(Error::Invalid("managed graph metadata not found".into()));
    }
    let part = crate::parts::PartSpec { version: 1, preset: "diagram/custom".into(), title: spec.title.clone(), subtitle: spec.subtitle.clone(), data: crate::parts::PartData::Diagram { graph: spec.clone() } };
    crate::parts::state::change(document, expected_revision, slide_id, id, &part, update)
}

fn selected<'a>(spec: &GraphSpec, ids: &'a [String]) -> Result<BTreeSet<&'a str>> {
    if ids.is_empty() || ids.len() > 120 { return Err(Error::Limit("select 1-120 graph entities".into())); }
    let selection: BTreeSet<_> = ids.iter().map(String::as_str).collect();
    if selection.len() != ids.len() || selection.iter().any(|id| !spec.nodes.iter().any(|node| node.id == *id) && !spec.groups.iter().any(|group| group.id == *id) && !spec.edges.iter().any(|edge| edge.id == *id)) { return Err(Error::Invalid("selection contains duplicate or unknown graph IDs".into())); }
    Ok(selection)
}

pub fn transform(spec: &GraphSpec, operations: &[GraphOperation]) -> Result<GraphSpec> {
    validate(spec)?;
    if operations.is_empty() || operations.len() > 128 { return Err(Error::Limit("graph edit requires 1-128 operations".into())); }
    let mut next = spec.clone();
    for operation in operations {
        match operation {
            GraphOperation::PutNode { node } => {
                if let Some(index) = next.nodes.iter().position(|entry| entry.id == node.id) { next.nodes[index] = node.clone(); }
                else { next.nodes.push(node.clone()); }
            }
            GraphOperation::PutEdge { edge } => {
                if let Some(index) = next.edges.iter().position(|entry| entry.id == edge.id) { next.edges[index] = edge.clone(); }
                else { next.edges.push(edge.clone()); }
            }
            GraphOperation::PutGroup { group } => {
                if let Some(index) = next.groups.iter().position(|entry| entry.id == group.id) {
                    let old = &next.groups[index]; let dx = group.x - old.x; let dy = group.y - old.y;
                    for node in next.nodes.iter_mut().filter(|node| node.group.as_deref() == Some(&group.id)) { node.x += dx; node.y += dy; }
                    next.groups[index] = group.clone();
                } else { next.groups.push(group.clone()); }
            }
            GraphOperation::Move { ids, dx, dy } => {
                if !dx.is_finite() || !dy.is_finite() { return Err(Error::Invalid("graph movement must be finite".into())); }
                let selection = selected(&next, ids)?;
                for node in &mut next.nodes { if selection.contains(node.id.as_str()) || node.group.as_deref().is_some_and(|id| selection.contains(id)) { node.x += dx; node.y += dy; } }
                for group in &mut next.groups { if selection.contains(group.id.as_str()) { group.x += dx; group.y += dy; } }
            }
            GraphOperation::Remove { ids } => {
                let selection = selected(&next, ids)?;
                next.nodes.retain(|node| !selection.contains(node.id.as_str()));
                next.groups.retain(|group| !selection.contains(group.id.as_str()));
                for node in &mut next.nodes { if node.group.as_deref().is_some_and(|id| selection.contains(id)) { node.group = None; } }
                next.edges.retain(|edge| !selection.contains(edge.id.as_str()) && next.nodes.iter().any(|node| node.id == edge.source) && next.nodes.iter().any(|node| node.id == edge.target));
            }
            GraphOperation::Align { ids, alignment } => {
                let selection = selected(&next, ids)?;
                let nodes: Vec<_> = next.nodes.iter().filter(|node| selection.contains(node.id.as_str())).collect();
                if nodes.len() < 2 || nodes.len() != ids.len() { return Err(Error::Invalid("alignment requires at least two nodes".into())); }
                let left = nodes.iter().map(|node| node.x).fold(f64::INFINITY, f64::min); let right = nodes.iter().map(|node| node.x + node.width).fold(f64::NEG_INFINITY, f64::max);
                let top = nodes.iter().map(|node| node.y).fold(f64::INFINITY, f64::min); let bottom = nodes.iter().map(|node| node.y + node.height).fold(f64::NEG_INFINITY, f64::max);
                for node in next.nodes.iter_mut().filter(|node| selection.contains(node.id.as_str())) {
                    match alignment { Alignment::Left => node.x = left, Alignment::Center => node.x = (left + right - node.width) / 2.0, Alignment::Right => node.x = right - node.width, Alignment::Top => node.y = top, Alignment::Middle => node.y = (top + bottom - node.height) / 2.0, Alignment::Bottom => node.y = bottom - node.height }
                }
            }
            GraphOperation::Layout { columns } => {
                if !(1..=8).contains(columns) { return Err(Error::Invalid("grid layout requires 1-8 columns".into())); }
                let place = |nodes: &mut Vec<&mut GraphNode>, area: [f64; 4]| -> Result<()> {
                    if nodes.is_empty() { return Ok(()); }
                    let columns = (*columns).min(nodes.len()); let rows = nodes.len().div_ceil(columns);
                    let cell_width = area[2] / columns as f64; let cell_height = area[3] / rows as f64;
                    for (index, node) in nodes.iter_mut().enumerate() {
                        if node.width + 16.0 > cell_width || node.height + 16.0 > cell_height { return Err(Error::Invalid("grid does not fit existing node sizes; use fewer nodes, more space or smaller nodes".into())); }
                        node.x = area[0] + (index % columns) as f64 * cell_width + (cell_width - node.width) / 2.0;
                        node.y = area[1] + (index / columns) as f64 * cell_height + (cell_height - node.height) / 2.0;
                    }
                    Ok(())
                };
                if next.groups.is_empty() { place(&mut next.nodes.iter_mut().collect(), [0.0, CONTENT_TOP, WIDTH, HEIGHT - CONTENT_TOP])?; }
                else {
                    if next.nodes.iter().any(|node| node.group.is_none()) { return Err(Error::Invalid("grid with groups requires all nodes to belong to a group".into())); }
                    for group in &next.groups { place(&mut next.nodes.iter_mut().filter(|node| node.group.as_deref() == Some(&group.id)).collect(), [group.x + 8.0, group.y + 40.0, group.width - 16.0, group.height - 48.0])?; }
                }
            }
        }
        if next.nodes.len() > 48 || next.edges.len() > 64 || next.groups.len() > 8 { return Err(Error::Limit("graph entity count exceeded".into())); }
    }
    validate(&next)?;
    Ok(next)
}

pub fn apply(document: &crate::document::Document, expected_revision: u64, slide_id: &str, id: &str, operations: &[GraphOperation]) -> Result<crate::document::TransactionResult> {
    crate::document::verify(document)?;
    let part = document.parts.iter().find(|part| part.slide_id == slide_id && part.element_id == id && part.spec.preset == "diagram/custom").ok_or_else(|| Error::Invalid("managed graph metadata not found".into()))?;
    let crate::parts::PartData::Diagram { graph } = &part.spec.data else { return Err(Error::Invalid("graph data missing".into())); };
    change(document, expected_revision, slide_id, id, &transform(graph, operations)?, true)
}