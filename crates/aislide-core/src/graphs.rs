use crate::{design::Theme, model::{Connection, Element, TextAlign, TextFormat, VerticalAlign, valid_color, valid_text, validate_elements}, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use geo::Intersects;
use std::collections::{BTreeMap, BTreeSet};

pub const WIDTH: f64 = 1152.0;
pub const HEIGHT: f64 = 512.0;
pub const CONTENT_TOP: f64 = 88.0;
pub const MAX_GROUPS: usize = 16;
pub const MAX_GROUP_DEPTH: usize = 4;
const RENDER_LAYOUT_VERSION: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphSpec {
    pub version: u32,
    pub title: String,
    #[serde(default)] pub subtitle: String,
    #[serde(default = "enabled", skip_serializing_if = "is_true")] pub show_title: bool,
    pub nodes: Vec<GraphNode>,
    #[serde(default)] pub edges: Vec<GraphEdge>,
    #[serde(default)] pub groups: Vec<GraphGroup>,
}

impl GraphSpec {
    fn content_top(&self) -> f64 { if self.show_title { CONTENT_TOP } else { 0.0 } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GraphTextAlign { Left, Center, Right }

impl GraphTextAlign {
    fn alignment(self) -> TextAlign {
        match self { Self::Left => TextAlign::Left, Self::Center => TextAlign::Center, Self::Right => TextAlign::Right }
    }
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
pub enum GraphPresentation { #[default] Card, Icon }
impl GraphPresentation {
    fn is_card(&self) -> bool { *self == Self::Card }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Port { #[default] Auto, Top, Left, Bottom, Right }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Route { #[default] Straight, Elbow, Manual }

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
fn enabled() -> bool { true }
fn is_true(value: &bool) -> bool { *value }
fn midpoint() -> f64 { 0.5 }
fn label_gap() -> f64 { 8.0 }
fn badge_size() -> f64 { 24.0 }
fn badge_font_size() -> f64 { 12.0 }

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GraphLabelSide { #[default] Above, Below }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphLabelPlacement {
    #[schemars(range(min = 0, max = 1))] pub position: f64,
    #[serde(default)] pub side: GraphLabelSide,
    #[serde(default = "label_gap")]
    #[schemars(range(min = 0, max = 128))] pub offset: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphBadge {
    #[schemars(range(min = 1, max = 99))] pub number: u16,
    #[serde(default = "midpoint")]
    #[schemars(range(min = 0, max = 1))] pub position: f64,
    #[serde(default = "badge_size")]
    #[schemars(range(min = 16, max = 64))] pub size: f64,
    #[serde(default = "badge_font_size")]
    #[schemars(range(min = 8, max = 32))] pub font_size: f64,
    #[serde(default = "paper")] pub fill: String,
    #[serde(default = "ink")] pub color: String,
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 240))] pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 12, max = 40))] pub detail_font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub text_align: Option<GraphTextAlign>,
    #[serde(default = "enabled", skip_serializing_if = "is_true")] pub heading_bold: bool,
    #[serde(default)] pub kind: NodeKind,
    #[serde(default, skip_serializing_if = "GraphPresentation::is_card")] pub presentation: GraphPresentation,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -0.5, max = 0.5))] pub source_offset: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -0.5, max = 0.5))] pub target_offset: Option<f64>,
    #[serde(default)] pub label: String,
    #[serde(default)] pub route: Route,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(length(max = 16))] pub waypoints: Vec<[f64; 2]>,
    #[serde(default = "muted")] pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.5, max = 12))] pub stroke_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub label_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 8, max = 40))] pub label_font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub label_placement: Option<GraphLabelPlacement>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub badge: Option<GraphBadge>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 64))] pub padding: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 20, max = 128))] pub header_height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 8, max = 32))] pub header_font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub icon: Option<GraphIcon>,
}

impl GraphGroup {
    fn padding(&self) -> f64 { self.padding.unwrap_or(8.0) }
    fn header_height(&self) -> f64 { self.header_height.unwrap_or(40.0) }
    fn header_font_size(&self) -> f64 { self.header_font_size.unwrap_or(18.0) }
}

fn identity(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 24 || !id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) {
        return Err(Error::Invalid("graph IDs require 1-24 ASCII letters, digits, hyphens or underscores".into()));
    }
    Ok(())
}

fn bounds(x: f64, y: f64, width: f64, height: f64, content_top: f64) -> Result<()> {
    if [x, y, width, height].iter().any(|value| !value.is_finite()) || x < 0.0 || y < content_top || width < 64.0 || height < 40.0 || x + width > WIDTH || y + height > HEIGHT {
        return Err(Error::Invalid(format!("graph bounds must fit 1152x512 at y >= {content_top}, with minimum size 64x40")));
    }
    Ok(())
}

fn group_ancestors<'a>(spec: &'a GraphSpec, mut parent: Option<&'a str>) -> Result<Vec<usize>> {
    let mut ancestors = Vec::new();
    while let Some(id) = parent {
        let index = spec.groups.iter().position(|group| group.id == id).ok_or_else(|| Error::Invalid("unknown graph parent group".into()))?;
        if ancestors.contains(&index) { return Err(Error::Invalid("graph group parent cycle".into())); }
        ancestors.push(index);
        if ancestors.len() > MAX_GROUP_DEPTH { return Err(Error::Limit("graph group depth exceeds 4".into())); }
        parent = spec.groups[index].parent.as_deref();
    }
    Ok(ancestors)
}

fn ordered_group_indices(spec: &GraphSpec) -> Result<Vec<usize>> {
    let mut indices = spec.groups.iter().enumerate().map(|(index, group)| Ok((index, group_ancestors(spec, Some(&group.id))?.len()))).collect::<Result<Vec<_>>>()?;
    indices.sort_by_key(|entry| entry.1);
    Ok(indices.into_iter().map(|entry| entry.0).collect())
}

fn contains(region: &GraphGroup, rect: [f64; 4]) -> bool {
    let [x, y, width, height] = rect;
    x >= region.x + region.padding() && y >= region.y + region.header_height() && x + width <= region.x + region.width - region.padding() && y + height <= region.y + region.height - region.padding()
}

fn icon_height(node: &GraphNode) -> f64 {
    let text_height = node.detail.as_ref().map_or(node.font_size * 2.5, |detail| {
        node.font_size * 1.25 * node.label.lines().count().max(1) as f64
            + 8.0 + detail_size(node) * 1.25 * detail.lines().count().max(1) as f64
    });
    (node.height * 0.55).min(node.height - 16.0 - text_height).min(96.0)
}

fn detail_size(node: &GraphNode) -> f64 { node.detail_font_size.unwrap_or((node.font_size * 0.8).max(12.0)) }

pub fn validate(spec: &GraphSpec) -> Result<()> {
    if spec.version != 1 { return Err(Error::Unsupported("graph version".into())); }
    valid_text(&spec.title, 80)?; valid_text(&spec.subtitle, 120)?;
    if spec.nodes.is_empty() || spec.nodes.len() > 48 || spec.edges.len() > 64 || spec.groups.len() > MAX_GROUPS { return Err(Error::Limit("graph requires 1-48 nodes, at most 64 edges and 16 groups".into())); }
    if spec.nodes.iter().filter_map(|node| node.icon.as_ref()).chain(spec.groups.iter().filter_map(|group| group.icon.as_ref())).fold(0usize, |total, icon| total.saturating_add(icon.base64.len())) > 3 * 1024 * 1024 {
        return Err(Error::Limit("graph icon data exceeds the 3 MiB encoded image budget".into()));
    }
    let mut ids = BTreeSet::new();
    for region in &spec.groups {
        identity(&region.id)?; valid_text(&region.label, 64)?;
        if !ids.insert(&region.id) { return Err(Error::Invalid("duplicate graph ID".into())); }
        bounds(region.x, region.y, region.width, region.height, spec.content_top())?;
        valid_color(&region.fill)?; valid_color(&region.stroke)?;
        if !region.padding().is_finite() || !(0.0..=64.0).contains(&region.padding())
            || !region.header_height().is_finite() || !(20.0..=128.0).contains(&region.header_height())
            || !region.header_font_size().is_finite() || !(8.0..=32.0).contains(&region.header_font_size())
            || region.padding() * 2.0 + 8.0 >= region.width
            || (region.padding.is_some() || region.header_height.is_some()) && region.header_height() + region.padding() >= region.height
            || region.header_font_size() * 1.25 > region.header_height() - 12.0 {
            return Err(Error::Invalid("graph group padding/header must be finite, in range, and leave room for its label and content".into()));
        }
    }
    ordered_group_indices(spec)?;
    for region in &spec.groups {
        if let Some(parent) = &region.parent {
            let ancestor = spec.groups.iter().find(|group| &group.id == parent).ok_or_else(|| Error::Invalid("unknown graph parent group".into()))?;
            if !contains(ancestor, [region.x, region.y, region.width, region.height]) { return Err(Error::Invalid("graph group must fit inside its parent below the group label".into())); }
        }
        if let Some(icon) = &region.icon {
            valid_text(&icon.alt, 500)?;
            crate::media::inspect_raster(&icon.base64, &icon.mime_type)?;
        }
    }
    for node in &spec.nodes {
        identity(&node.id)?; valid_text(&node.label, 160)?;
        if !ids.insert(&node.id) { return Err(Error::Invalid("duplicate graph ID".into())); }
        bounds(node.x, node.y, node.width, node.height, spec.content_top())?;
        valid_color(&node.fill)?; valid_color(&node.stroke)?; valid_color(&node.color)?;
        if !node.font_size.is_finite() || !(12.0..=40.0).contains(&node.font_size) { return Err(Error::Invalid("graph font size must be 12-40".into())); }
        if let Some(detail) = &node.detail {
            valid_text(detail, 240)?;
            if detail.trim().is_empty() { return Err(Error::Invalid("graph detail must be nonempty when supplied".into())); }
        }
        if let Some(size) = node.detail_font_size {
            if node.detail.is_none() || !size.is_finite() || !(12.0..=40.0).contains(&size) || size > node.font_size {
                return Err(Error::Invalid("graph detail_font_size requires detail and must be finite, 12-40 and no larger than font_size".into()));
            }
        }
        if node.presentation == GraphPresentation::Icon && (node.icon.is_none() || icon_height(node) <= 0.0) { return Err(Error::Invalid("icon presentation requires an icon and room for an image, gap and two-line label".into())); }
        if let Some(icon) = &node.icon {
            valid_text(&icon.alt, 500)?;
            crate::media::inspect_raster(&icon.base64, &icon.mime_type)?;
        }
        if let Some(parent) = &node.group {
            let region = spec.groups.iter().find(|region| &region.id == parent).ok_or_else(|| Error::Invalid("unknown graph group".into()))?;
            if !contains(region, [node.x, node.y, node.width, node.height]) { return Err(Error::Invalid("graph node must fit inside its group below the group label".into())); }
        }
    }
    for edge in &spec.edges {
        identity(&edge.id)?; valid_text(&edge.label, 64)?; valid_color(&edge.color)?;
        if let Some(color) = &edge.label_color { valid_color(color)?; }
        if edge.stroke_width.is_some_and(|value| !value.is_finite() || !(0.5..=12.0).contains(&value)) {
            return Err(Error::Invalid("graph edge stroke_width must be finite and 0.5-12".into()));
        }
        if edge.label_font_size.is_some_and(|value| !value.is_finite() || !(8.0..=40.0).contains(&value)) {
            return Err(Error::Invalid("graph edge label_font_size must be finite and 8-40".into()));
        }
        if let Some(placement) = &edge.label_placement {
            if edge.label.is_empty() || !placement.position.is_finite() || !(0.0..=1.0).contains(&placement.position)
                || !placement.offset.is_finite() || !(0.0..=128.0).contains(&placement.offset) {
                return Err(Error::Invalid("graph label_placement requires a label, position 0-1 and offset 0-128".into()));
            }
        }
        if let Some(badge) = &edge.badge {
            valid_color(&badge.fill)?; valid_color(&badge.color)?;
            if !(1..=99).contains(&badge.number) || !badge.position.is_finite() || !(0.0..=1.0).contains(&badge.position)
                || !badge.size.is_finite() || !(16.0..=64.0).contains(&badge.size)
                || !badge.font_size.is_finite() || !(8.0..=32.0).contains(&badge.font_size)
                || badge.font_size * 1.25 + 4.0 > badge.size
                || badge.number.to_string().len() as f64 * badge.font_size * 0.65 + 8.0 > badge.size {
                return Err(Error::Invalid("graph badge requires number 1-99, position 0-1, size 16-64 and fitting font 8-32".into()));
            }
        }
        if !ids.insert(&edge.id) { return Err(Error::Invalid("duplicate graph ID".into())); }
        if edge.source == edge.target || !spec.nodes.iter().any(|node| node.id == edge.source) || !spec.nodes.iter().any(|node| node.id == edge.target) { return Err(Error::Invalid("graph edges require distinct existing node endpoints".into())); }
        for (node_id, offset) in [(&edge.source, edge.source_offset), (&edge.target, edge.target_offset)] {
            if let Some(offset) = offset {
                if !offset.is_finite() || !(-0.5..=0.5).contains(&offset) { return Err(Error::Invalid("graph port offset must be finite and -0.5 to 0.5 of the side length".into())); }
                if offset != 0.0 && spec.nodes.iter().any(|node| &node.id == node_id && matches!(node.kind, NodeKind::Cylinder | NodeKind::Cloud)) {
                    return Err(Error::Unsupported("graph port offsets require rectangle, rounded_rectangle, ellipse or diamond nodes".into()));
                }
            }
        }
        if edge.waypoints.len() > 16 || (edge.route == Route::Manual) == edge.waypoints.is_empty()
            || edge.waypoints.iter().any(|point| !point[0].is_finite() || !point[1].is_finite() || !(0.0..=WIDTH).contains(&point[0]) || !(spec.content_top()..=HEIGHT).contains(&point[1])) {
            return Err(Error::Invalid("graph manual route requires 1-16 finite waypoints inside the content area; other routes cannot have waypoints".into()));
        }
        if edge.route == Route::Manual {
            let source = spec.nodes.iter().find(|node| node.id == edge.source).ok_or_else(|| Error::Invalid("unknown source node".into()))?;
            let target = spec.nodes.iter().find(|node| node.id == edge.target).ok_or_else(|| Error::Invalid("unknown target node".into()))?;
            if edge_points(source, target, edge).windows(2).any(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]) < 1e-6) {
                return Err(Error::Invalid("graph manual route cannot have collapsed consecutive segments".into()));
            }
        }
    }
    Ok(())
}

fn text(id: String, rect: [f64; 4], label: &str, size: f64, color: &str, alignment: TextAlign, bold: bool) -> Element {
    let [x, y, width, height] = rect;
    Element::Text { visual: None, id, x, y, width, height, text: label.into(), font_size: size, color: color.into(), bold, format: TextFormat { alignment, vertical: VerticalAlign::Middle, font_family: Some("@minor".into()), ..Default::default() } }
}

fn node_text(children: &mut Vec<Element>, prefix: &str, node: &GraphNode, content: [f64; 4]) -> Result<()> {
    let alignment = node.text_align.unwrap_or(if node.detail.is_some() { GraphTextAlign::Left } else { GraphTextAlign::Center });
    let heading_id = format!("{prefix}-nt-{}", node.id);
    let Some(detail) = &node.detail else {
        children.push(text(heading_id, content, &node.label, node.font_size, &node.color, alignment.alignment(), node.heading_bold));
        return Ok(());
    };
    let [left, top, width, height] = content;
    let heading_height = (node.font_size * 1.25 * node.label.lines().count().max(1) as f64).min((height - 8.0) / 2.0);
    let detail_height = height - heading_height - 8.0;
    if heading_height < 15.0 || detail_height < 15.0 { return Err(Error::Invalid("graph detail requires room for a heading, 8px gap and body at 12px".into())); }
    let mut heading = text(heading_id, [left, top, width, heading_height], &node.label, node.font_size, &node.color, alignment.alignment(), node.heading_bold);
    let mut body = text(format!("{prefix}-nd-{}", node.id), [left, top + heading_height + 8.0, width, detail_height], detail, detail_size(node), &node.color, alignment.alignment(), false);
    for element in [&mut heading, &mut body] {
        if let Element::Text { format, .. } = element { format.vertical = VerticalAlign::Top; }
    }
    children.push(heading); children.push(body);
    Ok(())
}

fn shape(id: String, rect: [f64; 4], preset: &str, fill: &str, stroke: &str) -> Element {
    let [x, y, width, height] = rect;
    Element::Shape { visual: None, id, x, y, width, height, preset: preset.into(), fill: fill.into(), stroke: stroke.into(), stroke_width: 1.5, rotation: 0.0, text: String::new(), font_size: 18.0, color: "@dk1".into(), bold: false, format: Default::default() }
}

fn icon_picture(id: &str, icon: &GraphIcon, rect: [f64; 4]) -> Result<Element> {
    let [left, top, box_width, box_height] = rect;
    let mut picture = crate::media::create_picture(id, icon.base64.clone(), &icon.mime_type, &icon.alt)?;
    if let Element::Picture { x, y, width, height, .. } = &mut picture {
        let scale = (box_width / *width).min(box_height / *height);
        *width *= scale; *height *= scale;
        *x = left + (box_width - *width) / 2.0;
        *y = top + (box_height - *height) / 2.0;
    }
    Ok(picture)
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

fn offset_endpoint(node: &GraphNode, port: Port, other: &GraphNode, offset: Option<f64>) -> (f64, f64, u32) {
    let original = endpoint(node, port, other);
    let offset = offset.unwrap_or(0.0);
    if offset == 0.0 { return original; }
    let port = resolved_port(node, port, other);
    let along_horizontal = matches!(port, Port::Top | Port::Bottom);
    let inset = match node.kind {
        NodeKind::Ellipse => (1.0 - (1.0 - 4.0 * offset * offset).max(0.0).sqrt()) / 2.0,
        NodeKind::Diamond => offset.abs(),
        NodeKind::RoundedRectangle => {
            let radius = node.width.min(node.height) * 16667.0 / 100000.0;
            let length = if along_horizontal { node.width } else { node.height };
            let perpendicular = if along_horizontal { node.height } else { node.width };
            let coordinate = (0.5 + offset) * length;
            let corner_distance = (radius - coordinate).max(coordinate - length + radius).max(0.0);
            (radius - (radius * radius - corner_distance * corner_distance).max(0.0).sqrt()) / perpendicular
        }
        _ => 0.0,
    };
    let point = match port {
        Port::Top => [0.5 + offset, inset], Port::Bottom => [0.5 + offset, 1.0 - inset],
        Port::Left => [inset, 0.5 + offset], Port::Right | Port::Auto => [1.0 - inset, 0.5 + offset],
    };
    (node.x + (point[0] * 1e6).round() / 1e6 * node.width, node.y + (point[1] * 1e6).round() / 1e6 * node.height, original.2)
}

pub fn edge_points(source: &GraphNode, target: &GraphNode, edge: &GraphEdge) -> Vec<[f64; 2]> {
    let start = offset_endpoint(source, edge.source_port, target, edge.source_offset);
    let end = offset_endpoint(target, edge.target_port, source, edge.target_offset);
    let mut points = vec![[start.0, start.1]];
    if edge.route == Route::Manual { points.extend(edge.waypoints.iter().copied()); }
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

fn route_anchor(points: &[[f64; 2]], position: f64) -> Result<([f64; 2], [f64; 2])> {
    let length = |segment: &[[f64; 2]]| (segment[1][0] - segment[0][0]).hypot(segment[1][1] - segment[0][1]);
    let total: f64 = points.windows(2).map(length).sum();
    if !total.is_finite() || total <= 0.0 { return Err(Error::Invalid("graph edge annotations require a nonempty path".into())); }
    let mut distance = total * position;
    for segment in points.windows(2) {
        let segment_length = length(segment);
        if segment_length <= 0.0 { continue; }
        if distance <= segment_length + 1e-9 {
            let ratio = (distance / segment_length).min(1.0);
            let horizontal = segment[1][0] - segment[0][0]; let vertical = segment[1][1] - segment[0][1];
            let mut normal = [-vertical / segment_length, horizontal / segment_length];
            if normal[1] > 0.0 || normal[1].abs() < 1e-9 && normal[0] < 0.0 { normal = [-normal[0], -normal[1]]; }
            return Ok(([segment[0][0] + horizontal * ratio, segment[0][1] + vertical * ratio], normal));
        }
        distance -= segment_length;
    }
    Err(Error::Invalid("graph annotation position does not lie on the route".into()))
}

fn annotation_bounds(spec: &GraphSpec, center: [f64; 2], width: f64, height: f64) -> Result<[f64; 4]> {
    let rect = [center[0] - width / 2.0, center[1] - height / 2.0, width, height];
    if rect[0] < 0.0 || rect[1] < spec.content_top() || rect[0] + width > WIDTH || rect[1] + height > HEIGHT {
        return Err(Error::Invalid("explicit graph edge annotation must fit inside the graph content area".into()));
    }
    Ok(rect)
}

fn relationship_label_bounds(spec: &GraphSpec, elements: &[Element], points: &[[f64; 2]], width: f64, height: f64, placed: &[[f64; 4]]) -> [f64; 4] {
    let mut obstacles = Vec::new();
    for node in &spec.nodes {
        if node.presentation == GraphPresentation::Icon {
            let suffixes = [format!("-ni-{}", node.id), format!("-nt-{}", node.id), format!("-nd-{}", node.id)];
            for element in elements {
                let (id, x, y, width, height) = element.bounds();
                if suffixes.iter().any(|suffix| id.ends_with(suffix)) { obstacles.push([x, y, width, height]); }
            }
        } else { obstacles.push([node.x, node.y, node.width, node.height]); }
    }
    obstacles.extend(spec.groups.iter().map(|group| [group.x, group.y, group.width, group.header_height()]));
    obstacles.extend(placed.iter().copied());
    let rectangle = |bounds: [f64; 4], padding: f64| geo::Rect::new(
        (bounds[0] - padding, bounds[1] - padding), (bounds[0] + bounds[2] + padding, bounds[1] + bounds[3] + padding));
    let routes: Vec<_> = spec.edges.iter().flat_map(|edge| {
        let source = spec.nodes.iter().find(|node| node.id == edge.source).expect("validated graph source");
        let target = spec.nodes.iter().find(|node| node.id == edge.target).expect("validated graph target");
        let route = edge_points(source, target, edge);
        route.windows(2).map(|segment| geo::Line::new((segment[0][0], segment[0][1]), (segment[1][0], segment[1][1]))).collect::<Vec<_>>()
    }).collect();
    let mut segments: Vec<_> = points.windows(2).collect();
    segments.sort_by(|left, right| {
        let length = |segment: &[[f64; 2]]| (segment[1][0] - segment[0][0]).hypot(segment[1][1] - segment[0][1]);
        length(right).total_cmp(&length(left))
    });
    let mut best = [points[0][0].clamp(0.0, WIDTH - width), points[0][1].clamp(spec.content_top(), HEIGHT - height), width, height];
    let mut best_score = f64::INFINITY;
    for segment in segments {
        let horizontal = segment[1][0] - segment[0][0]; let vertical = segment[1][1] - segment[0][1];
        let length = horizontal.hypot(vertical);
        let mut normal = if length < 0.01 { [0.0, -1.0] } else { [-vertical / length, horizontal / length] };
        if normal[1] > 0.0 || normal[1].abs() < 0.000001 && normal[0] < 0.0 { normal = [-normal[0], -normal[1]]; }
        let clearance = (normal[0].abs() * width + normal[1].abs() * height) / 2.0 + 8.0;
        for extra in [0.0, 16.0, 32.0, 48.0, 64.0, 96.0] {
            for side in [1.0, -1.0] {
                let candidate = [
                    ((segment[0][0] + segment[1][0]) / 2.0 + normal[0] * (clearance + extra) * side - width / 2.0).clamp(0.0, WIDTH - width),
                    ((segment[0][1] + segment[1][1]) / 2.0 + normal[1] * (clearance + extra) * side - height / 2.0).clamp(spec.content_top(), HEIGHT - height), width, height];
                let area = rectangle(candidate, 3.0);
                let route_crossings = routes.iter().filter(|line| line.intersects(&area)).count();
                let overlaps = obstacles.iter().filter(|bounds| rectangle(**bounds, 0.0).intersects(&area)).count();
                let score = route_crossings as f64 * 1000000.0 + overlaps as f64 * 1000.0 + extra + if side < 0.0 { 0.5 } else { 0.0 };
                if score < best_score { best = candidate; best_score = score; }
            }
        }
    }
    best
}

fn render_prefix(id: &str, spec: &GraphSpec) -> Result<String> {
    Ok(format!("{id}-{}", &format!("{:x}", Sha256::digest(crate::canonical::bytes(&(RENDER_LAYOUT_VERSION, spec))?))[..10]))
}

pub(crate) fn cap_detail_fonts(id: &str, spec: &GraphSpec, children: &mut [Element]) -> Result<()> {
    let prefix = render_prefix(id, spec)?;
    for node in spec.nodes.iter().filter(|node| node.detail.is_some()) {
        let heading_id = format!("{prefix}-nt-{}", node.id);
        let heading_size = children.iter().find_map(|element| match element {
            Element::Text { id, font_size, .. } if id == &heading_id => Some(*font_size), _ => None,
        }).ok_or_else(|| Error::Invalid("graph detail heading is missing".into()))?;
        let detail_id = format!("{prefix}-nd-{}", node.id);
        for element in children.iter_mut() {
            if let Element::Text { id, font_size, .. } = element {
                if id == &detail_id { *font_size = font_size.min(heading_size); }
            }
        }
    }
    Ok(())
}

pub fn create(id: &str, spec: &GraphSpec, theme: &Theme) -> Result<Element> {
    valid_text(id, 40)?;
    if id.is_empty() { return Err(Error::Invalid("graph root ID is required".into())); }
    validate(spec)?; crate::design::validate_theme(theme)?;
    let prefix = render_prefix(id, spec)?;
    let mut sites_by_node = BTreeMap::new();
    let mut site_indices = BTreeMap::new();
    for node in &spec.nodes {
        if !spec.edges.iter().any(|edge| edge.source == node.id && edge.source_offset.is_some_and(|offset| offset != 0.0)
            || edge.target == node.id && edge.target_offset.is_some_and(|offset| offset != 0.0)) { continue; }
        let mut sites = Vec::new();
        for edge in &spec.edges {
            let (other_id, port, offset) = if edge.source == node.id { (&edge.target, edge.source_port, edge.source_offset) }
                else if edge.target == node.id { (&edge.source, edge.target_port, edge.target_offset) } else { continue; };
            let other = spec.nodes.iter().find(|other| &other.id == other_id).ok_or_else(|| Error::Invalid("unknown connected node".into()))?;
            let point = offset_endpoint(node, port, other, offset);
            let angle = match resolved_port(node, port, other) { Port::Top => 270.0, Port::Left => 180.0, Port::Bottom => 90.0, _ => 0.0 };
            site_indices.insert((node.id.as_str(), edge.id.as_str()), sites.len() as u32);
            sites.push(crate::visual::ConnectionSite { x: ((point.0 - node.x) / node.width * 1e6).round() / 1e6, y: ((point.1 - node.y) / node.height * 1e6).round() / 1e6, angle });
        }
        sites_by_node.insert(node.id.as_str(), sites);
    }
    let mut children = if spec.show_title { vec![text(format!("{prefix}-title"), [16.0, 0.0, 1120.0, 40.0], &spec.title, 28.0, "@dk1", TextAlign::Left, true), text(format!("{prefix}-subtitle"), [16.0, 44.0, 1120.0, 26.0], &spec.subtitle, 16.0, "@dk2", TextAlign::Left, false)] } else { Vec::new() };
    for index in ordered_group_indices(spec)? {
        let region = &spec.groups[index];
        children.push(shape(format!("{prefix}-g-{}", region.id), [region.x, region.y, region.width, region.height], "rect", &region.fill, &region.stroke));
        let inset = region.padding() + 4.0;
        let offset = if let Some(icon) = &region.icon {
            let size = ((region.width - inset * 2.0) / 3.0).min(32.0).min(region.header_height() - 8.0);
            children.push(icon_picture(&format!("{prefix}-gi-{}", region.id), icon, [region.x + inset, region.y + 4.0, size, region.header_height() - 8.0])?);
            size + 8.0
        } else { 0.0 };
        children.push(text(format!("{prefix}-gt-{}", region.id), [region.x + inset + offset, region.y + 8.0, region.width - inset * 2.0 - offset, region.header_height() - 12.0], &region.label, region.header_font_size(), "@dk1", TextAlign::Left, true));
    }
    for edge in &spec.edges {
        let source = spec.nodes.iter().find(|node| node.id == edge.source).ok_or_else(|| Error::Invalid("unknown source node".into()))?;
        let target = spec.nodes.iter().find(|node| node.id == edge.target).ok_or_else(|| Error::Invalid("unknown target node".into()))?;
        let start = offset_endpoint(source, edge.source_port, target, edge.source_offset);
        let end = offset_endpoint(target, edge.target_port, source, edge.target_offset);
        let points = edge_points(source, target, edge);
        let left = points.iter().map(|point| point[0]).fold(f64::INFINITY, f64::min); let top = points.iter().map(|point| point[1]).fold(f64::INFINITY, f64::min);
        let width = (points.iter().map(|point| point[0]).fold(f64::NEG_INFINITY, f64::max) - left).max(0.01); let height = (points.iter().map(|point| point[1]).fold(f64::NEG_INFINITY, f64::max) - top).max(0.01);
        let points = points.into_iter().map(|point| [((point[0] - left) / width * 1e6).round() / 1e6, ((point[1] - top) / height * 1e6).round() / 1e6]).collect();
        children.push(Element::Connector { visual: None, id: format!("{prefix}-e-{}", edge.id), x: left, y: top, width, height, color: edge.color.clone(), stroke_width: edge.stroke_width.unwrap_or(2.0), arrow: edge.arrow, flip_v: false,
            start: Some(Connection { element_id: format!("{prefix}-n-{}", source.id), site: site_indices.get(&(source.id.as_str(), edge.id.as_str())).copied().unwrap_or(start.2) }), end: Some(Connection { element_id: format!("{prefix}-n-{}", target.id), site: site_indices.get(&(target.id.as_str(), edge.id.as_str())).copied().unwrap_or(end.2) }), routing: Some(crate::model::ConnectorRouting { points, custom: edge.route == Route::Manual, start_arrow: edge.start_arrow, dashed: edge.dashed }) });
    }
    for node in &spec.nodes {
        if node.presentation == GraphPresentation::Icon {
            let mut anchor = shape(format!("{prefix}-n-{}", node.id), [node.x, node.y, node.width, node.height], node.kind.preset(), "none", "@dk1");
            if let Element::Shape { stroke_width, .. } = &mut anchor { *stroke_width = 0.0; }
            children.push(anchor);
            let height = icon_height(node);
            let width = (node.width - 8.0).min(96.0);
            let icon = node.icon.as_ref().ok_or_else(|| Error::Invalid("icon presentation requires an icon".into()))?;
            children.push(icon_picture(&format!("{prefix}-ni-{}", node.id), icon, [node.x + (node.width - width) / 2.0, node.y + 4.0, width, height])?);
            node_text(&mut children, &prefix, node, [node.x + 4.0, node.y + 12.0 + height, node.width - 8.0, node.height - 16.0 - height])?;
            continue;
        }
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
        node_text(&mut children, &prefix, node, content)?;
    }
    for (node_id, connection_sites) in sites_by_node {
        let node_id = format!("{prefix}-n-{node_id}");
        for child in &mut children {
            if let Element::Shape { id, visual, .. } = child {
                if id == &node_id { *visual = Some(crate::visual::VisualStyle { connection_sites: connection_sites.clone(), ..Default::default() }); }
            }
        }
    }
    let mut label_bounds = Vec::new();
    for edge in &spec.edges {
        let Some(badge) = &edge.badge else { continue; };
        let source = spec.nodes.iter().find(|node| node.id == edge.source).ok_or_else(|| Error::Invalid("unknown source node".into()))?;
        let target = spec.nodes.iter().find(|node| node.id == edge.target).ok_or_else(|| Error::Invalid("unknown target node".into()))?;
        let (center, _) = route_anchor(&edge_points(source, target, edge), badge.position)?;
        let bounds = annotation_bounds(spec, center, badge.size, badge.size)?;
        let mut element = shape(format!("{prefix}-eb-{}", edge.id), bounds, "ellipse", &badge.fill, &edge.color);
        if let Element::Shape { text, font_size, color, bold, format, .. } = &mut element {
            *text = badge.number.to_string(); *font_size = badge.font_size; *color = badge.color.clone(); *bold = true;
            *format = TextFormat { alignment: TextAlign::Center, vertical: VerticalAlign::Middle, font_family: Some("@minor".into()), ..Default::default() };
        }
        children.push(element); label_bounds.push(bounds);
    }
    for edge in &spec.edges {
        if edge.label.is_empty() { continue; }
        let source = spec.nodes.iter().find(|node| node.id == edge.source).ok_or_else(|| Error::Invalid("unknown source node".into()))?;
        let target = spec.nodes.iter().find(|node| node.id == edge.target).ok_or_else(|| Error::Invalid("unknown target node".into()))?;
        let size = edge.label_font_size.unwrap_or(16.0);
        let width = (edge.label.chars().map(|character| if character.is_ascii() { 9.0 } else { 16.0 }).sum::<f64>() * size / 16.0 + 8.0).clamp(48.0, 280.0);
        let points = edge_points(source, target, edge);
        let bounds = if let Some(placement) = &edge.label_placement {
            let (center, normal) = route_anchor(&points, placement.position)?;
            let direction = match placement.side { GraphLabelSide::Above => 1.0, GraphLabelSide::Below => -1.0 };
            let distance = (normal[0].abs() * width + normal[1].abs() * size * 1.75) / 2.0 + placement.offset;
            annotation_bounds(spec, [center[0] + normal[0] * distance * direction, center[1] + normal[1] * distance * direction], width, size * 1.75)?
        } else { relationship_label_bounds(spec, &children, &points, width, size * 1.75, &label_bounds) };
        children.push(text(format!("{prefix}-et-{}", edge.id), bounds, &edge.label, size, edge.label_color.as_deref().unwrap_or("@dk1"), TextAlign::Center, false));
        label_bounds.push(bounds);
    }
    crate::layout::fit_part_text(&mut children, theme)?;
    cap_detail_fonts(id, spec, &mut children)?;
    let result = Element::Group { visual: None, id: id.into(), x: 64.0, y: 144.0, width: WIDTH, height: HEIGHT, view_width: WIDTH, view_height: HEIGHT, children };
    validate_elements(std::slice::from_ref(&result), (1280.0, 720.0), 0, &mut BTreeSet::new(), &mut 0, &mut 0)?;
    Ok(result)
}

pub fn catalog() -> Value {
    let examples = [
        json!({"id":"service","name":"Service architecture","spec":{"version":1,"title":"Service architecture","subtitle":"Editable example","nodes":[{"id":"user","label":"Client","kind":"rounded_rectangle","x":40,"y":220},{"id":"api","label":"API service","kind":"rectangle","x":390,"y":220},{"id":"data","label":"Database","kind":"cylinder","x":780,"y":220}],"edges":[{"id":"request","source":"user","target":"api","label":"HTTPS"},{"id":"query","source":"api","target":"data","label":"Query"}],"groups":[]}}),
        json!({"id":"approval","name":"Approval flow","spec":{"version":1,"title":"Approval flow","subtitle":"Editable example","nodes":[{"id":"input","label":"Request","x":40,"y":220},{"id":"review","label":"Review","kind":"diamond","x":390,"y":190,"width":200,"height":140},{"id":"approve","label":"Approved","kind":"rounded_rectangle","x":830,"y":120},{"id":"revise","label":"Revise","x":830,"y":340}],"edges":[{"id":"submit","source":"input","target":"review"},{"id":"yes","source":"review","target":"approve","label":"Yes"},{"id":"no","source":"review","target":"revise","label":"No"}],"groups":[]}}),
        json!({"id":"boundary","name":"Network boundary","spec":{"version":1,"title":"Network boundary","subtitle":"Editable example","nodes":[{"id":"client","label":"Client","x":40,"y":210},{"id":"app","label":"Application","x":430,"y":210,"group":"private"},{"id":"store","label":"Storage","kind":"cylinder","x":800,"y":210,"group":"private"}],"edges":[{"id":"access","source":"client","target":"app","label":"Authorized"},{"id":"data","source":"app","target":"store"}],"groups":[{"id":"private","label":"Private network","x":370,"y":120,"width":680,"height":300}]}}),
    ];
    json!({"version":1,"shapes":["rectangle","rounded_rectangle","ellipse","diamond","cylinder","cloud"],"ports":["auto","top","left","bottom","right"],"routes":["straight","elbow","manual"],"limits":{"nodes":48,"edges":64,"groups":MAX_GROUPS,"group_depth":MAX_GROUP_DEPTH,"rendered_elements":256,"waypoints_per_edge":16},"canvas":{"width":WIDTH,"height":HEIGHT,"content_top":CONTENT_TOP,"content_top_without_title":0.0},"node_text":{"detail_max_length":240,"detail_font_size_min":12,"detail_font_size_max":40,"detail_font_size_default":"max(12, font_size * 0.8)","detail_font_size_ceiling":"font_size","text_align":["left","center","right"],"default_alignment":"left with detail, center without detail","heading_bold_default":true},"edge_layout":{"position":"fraction of path length from semantic source to target","label_side":"above/below the segment; right/left for vertical segments","offset":"source_offset/target_offset are -0.5..0.5 of side length from center, increasing right/down","offset_shapes":["rectangle","rounded_rectangle","ellipse","diamond"],"manual":"1-16 graph-coordinate waypoints, including diagonal segments; nodes are not moved","move":"waypoints move with both endpoints, or when the edge itself is selected; otherwise intermediate points stay fixed"},"schema":schemars::schema_for!(GraphSpec),"operation_schema":schemars::schema_for!(GraphOperation),"examples":examples})
}

pub fn change(document: &crate::document::Document, expected_revision: u64, slide_id: &str, id: &str, spec: &GraphSpec, update: bool) -> Result<crate::document::TransactionResult> {
    let layout = if update {
        document.parts.iter().find(|part| part.slide_id == slide_id && part.element_id == id && part.spec.preset == "diagram/custom")
            .ok_or_else(|| Error::Invalid("managed graph metadata not found".into()))?.spec.layout.clone()
    } else { None };
    let part = crate::parts::PartSpec { version: 1, preset: "diagram/custom".into(), title: spec.title.clone(), subtitle: spec.subtitle.clone(), data: crate::parts::PartData::Diagram { graph: spec.clone() }, layout };
    crate::parts::state::change(document, expected_revision, slide_id, id, &part, update)
}

fn selected<'a>(spec: &GraphSpec, ids: &'a [String]) -> Result<BTreeSet<&'a str>> {
    if ids.is_empty() || ids.len() > 120 { return Err(Error::Limit("select 1-120 graph entities".into())); }
    let selection: BTreeSet<_> = ids.iter().map(String::as_str).collect();
    if selection.len() != ids.len() || selection.iter().any(|id| !spec.nodes.iter().any(|node| node.id == *id) && !spec.groups.iter().any(|group| group.id == *id) && !spec.edges.iter().any(|edge| edge.id == *id)) { return Err(Error::Invalid("selection contains duplicate or unknown graph IDs".into())); }
    Ok(selection)
}

fn move_entities(spec: &mut GraphSpec, selection: &BTreeSet<&str>, dx: f64, dy: f64) -> Result<()> {
    let mut moved_groups = BTreeSet::new();
    for group in &spec.groups {
        if group_ancestors(spec, Some(&group.id))?.iter().any(|index| selection.contains(spec.groups[*index].id.as_str())) { moved_groups.insert(group.id.clone()); }
    }
    let mut moved_nodes = BTreeSet::new();
    for node in &mut spec.nodes {
        if selection.contains(node.id.as_str()) || node.group.as_ref().is_some_and(|id| moved_groups.contains(id)) { node.x += dx; node.y += dy; moved_nodes.insert(node.id.clone()); }
    }
    for group in &mut spec.groups { if moved_groups.contains(&group.id) { group.x += dx; group.y += dy; } }
    for edge in &mut spec.edges {
        if selection.contains(edge.id.as_str()) || moved_nodes.contains(&edge.source) && moved_nodes.contains(&edge.target) {
            for point in &mut edge.waypoints { point[0] += dx; point[1] += dy; }
        }
    }
    Ok(())
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
                    move_entities(&mut next, &BTreeSet::from([group.id.as_str()]), dx, dy)?;
                    next.groups[index] = group.clone();
                } else { next.groups.push(group.clone()); }
            }
            GraphOperation::Move { ids, dx, dy } => {
                if !dx.is_finite() || !dy.is_finite() { return Err(Error::Invalid("graph movement must be finite".into())); }
                let selection = selected(&next, ids)?;
                move_entities(&mut next, &selection, *dx, *dy)?;
            }
            GraphOperation::Remove { ids } => {
                let selection = selected(&next, ids)?;
                let parents = next.groups.iter().map(|group| {
                    let parent = group_ancestors(&next, Some(&group.id))?.into_iter().find(|index| !selection.contains(next.groups[*index].id.as_str())).map(|index| next.groups[index].id.clone());
                    Ok((group.id.clone(), parent))
                }).collect::<Result<std::collections::BTreeMap<_, _>>>()?;
                for node in &mut next.nodes { if let Some(parent) = &node.group { node.group = parents.get(parent).cloned().flatten(); } }
                for group in &mut next.groups { if let Some(parent) = &group.parent { group.parent = parents.get(parent).cloned().flatten(); } }
                next.nodes.retain(|node| !selection.contains(node.id.as_str()));
                next.groups.retain(|group| !selection.contains(group.id.as_str()));
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
                if next.groups.iter().any(|group| group.parent.is_some()) { return Err(Error::Unsupported("automatic grid layout for nested graph groups; use explicit coordinates or move operations".into())); }
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
                if next.groups.is_empty() {
                    let top = next.content_top();
                    place(&mut next.nodes.iter_mut().collect(), [0.0, top, WIDTH, HEIGHT - top])?;
                }
                else {
                    if next.nodes.iter().any(|node| node.group.is_none()) { return Err(Error::Invalid("grid with groups requires all nodes to belong to a group".into())); }
                    for group in &next.groups { place(&mut next.nodes.iter_mut().filter(|node| node.group.as_deref() == Some(&group.id)).collect(), [group.x + group.padding(), group.y + group.header_height(), group.width - group.padding() * 2.0, group.height - group.header_height() - group.padding()])?; }
                }
            }
        }
        if next.nodes.len() > 48 || next.edges.len() > 64 || next.groups.len() > MAX_GROUPS { return Err(Error::Limit("graph entity count exceeded".into())); }
    }
    validate(&next)?;
    Ok(next)
}

pub fn apply(document: &crate::document::Document, expected_revision: u64, slide_id: &str, id: &str, operations: &[GraphOperation]) -> Result<crate::document::TransactionResult> {
    crate::document::verify(document)?;
    let part = document.parts.iter().find(|part| part.slide_id == slide_id && part.element_id == id && part.spec.preset == "diagram/custom").ok_or_else(|| Error::Invalid("managed graph metadata not found".into()))?;
    let crate::parts::PartData::Diagram { graph } = &part.spec.data else { return Err(Error::Invalid("graph data missing".into())); };
    let next = transform(graph, operations)?;
    if crate::canonical::bytes(graph)? == crate::canonical::bytes(&next)? {
        if part.stale { return Err(Error::Conflict("part metadata is stale; retain manual edits or insert a new part".into())); }
        let current = document.deck.slides.iter().find(|slide| slide.id == slide_id).and_then(|slide| slide.elements.iter().find(|element| element.bounds().0 == id)).ok_or_else(|| Error::Conflict("part element missing".into()))?;
        if crate::model::element_list(std::slice::from_ref(current)).iter().any(|element| element.visual().is_some_and(|visual| visual.locked || visual.hidden)) {
            return Err(Error::Unsupported("unlock and show the managed part before updating".into()));
        }
        return crate::document::transact(document, crate::document::Transaction { expected_revision, expected_hash: document.hash.clone(), operations: serde_json::from_value(json!([{"op":"replace","path":"/deck","value":document.deck}]))? });
    }
    change(document, expected_revision, slide_id, id, &next, true)
}