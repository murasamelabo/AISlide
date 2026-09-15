use crate::{model::{ChartKind, ChartSeries, Element, TextAlign, TextFormat, valid_text, validate_elements}, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

mod catalog;
mod charts;
mod diagrams;
pub mod state;
pub use catalog::catalog;

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartSpec {
    pub version: u32,
    pub preset: String,
    pub title: String,
    #[serde(default)] pub subtitle: String,
    pub data: PartData,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PartData {
    Chart { categories: Vec<String>, series: Vec<PartSeries>, #[serde(default)] x_axis: String, #[serde(default)] y_axis: String },
    Items { items: Vec<PartItem>, #[serde(default)] center: String },
    Tree { nodes: Vec<TreeNode> },
    Network { nodes: Vec<PartItem>, edges: Vec<PartEdge> },
    Matrix { rows: Vec<String>, columns: Vec<String>, cells: Vec<Vec<String>> },
    Groups { groups: Vec<PartGroup> },
    Timeline { periods: Vec<String>, tasks: Vec<PartTask> },
    Waterfall { steps: Vec<WaterfallStep>, #[serde(default)] unit: String },
    Map { points: Vec<MapPoint> },
    Diagram { graph: crate::graphs::GraphSpec },
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartSeries { pub name: String, pub values: Vec<f64> }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartItem { pub label: String, #[serde(default)] pub detail: String, #[serde(default, skip_serializing_if = "Option::is_none")] pub value: Option<f64> }
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TreeNode { pub id: String, pub label: String, #[serde(default)] pub parent: Option<String> }
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartEdge { pub from: usize, pub to: usize, #[serde(default)] pub label: String }
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartGroup { pub label: String, pub items: Vec<String> }
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartTask { pub label: String, pub start: usize, pub end: usize, #[serde(default)] pub progress: f64 }
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaterfallStep { pub label: String, pub value: f64, #[serde(default)] pub total: bool }
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MapPoint { pub label: String, pub longitude: f64, pub latitude: f64, #[serde(default)] pub value: Option<f64> }

fn bounded_text(value: &str, maximum: usize) -> Result<()> { valid_text(value, maximum) }
fn finite(value: f64) -> Result<()> { if !value.is_finite() || value.abs() > 1e15 { Err(Error::Invalid("part values must be finite and within +/-1e15".into())) } else { Ok(()) } }
fn count(length: usize, minimum: usize, maximum: usize) -> Result<()> { if !(minimum..=maximum).contains(&length) { Err(Error::Invalid(format!("part requires {minimum}-{maximum} items"))) } else { Ok(()) } }

fn validate_data(data: &PartData) -> Result<()> {
    match data {
        PartData::Diagram { graph } => crate::graphs::validate(graph)?,
        PartData::Chart { categories, series, x_axis, y_axis } => {
            count(categories.len(), 1, 12)?; count(series.len(), 1, 4)?;
            bounded_text(x_axis, 60)?; bounded_text(y_axis, 60)?;
            for category in categories { bounded_text(category, 32)?; }
            for entry in series { bounded_text(&entry.name, 32)?; if entry.values.len() != categories.len() { return Err(Error::Invalid("series must match categories".into())); } for value in &entry.values { finite(*value)?; } }
        }
        PartData::Items { items, center } => {
            count(items.len(), 2, 12)?; bounded_text(center, 48)?;
            for item in items { bounded_text(&item.label, 48)?; bounded_text(&item.detail, 120)?; if let Some(value) = item.value { finite(value)?; } }
        }
        PartData::Tree { nodes } => {
            count(nodes.len(), 2, 12)?;
            let mut ids = BTreeSet::new();
            for node in nodes { bounded_text(&node.id, 32)?; bounded_text(&node.label, 40)?; if node.id.is_empty() || !ids.insert(&node.id) { return Err(Error::Invalid("duplicate tree node".into())); } }
            if nodes.iter().filter(|node| node.parent.is_none()).count() != 1 { return Err(Error::Invalid("tree requires one root".into())); }
            for node in nodes { let mut seen = BTreeSet::new(); let mut current = Some(node); while let Some(entry) = current { if !seen.insert(&entry.id) { return Err(Error::Invalid("tree contains a cycle".into())); } current = match &entry.parent { Some(parent) => Some(nodes.iter().find(|node| &node.id == parent).ok_or_else(|| Error::Invalid("unknown tree parent".into()))?), None => None }; } if seen.len() > 4 { return Err(Error::Limit("tree depth > 4".into())); } }
        }
        PartData::Network { nodes, edges } => {
            validate_data(&PartData::Items { items: nodes.clone(), center: String::new() })?; count(nodes.len(), 2, 6)?; count(edges.len(), 1, 12)?;
            for edge in edges { bounded_text(&edge.label, 24)?; if edge.from >= nodes.len() || edge.to >= nodes.len() || edge.from == edge.to { return Err(Error::Invalid("network edge endpoint".into())); } }
        }
        PartData::Matrix { rows, columns, cells } => {
            count(rows.len(), 2, 4)?; count(columns.len(), 2, 4)?;
            for text in rows.iter().chain(columns).chain(cells.iter().flatten()) { bounded_text(text, 48)?; }
            if cells.len() != rows.len() || cells.iter().any(|row| row.len() != columns.len()) { return Err(Error::Invalid("matrix cells must match row and column labels".into())); }
        }
        PartData::Groups { groups } => {
            count(groups.len(), 2, 6)?;
            for group in groups { bounded_text(&group.label, 32)?; count(group.items.len(), 1, 5)?; for item in &group.items { bounded_text(item, 40)?; } }
        }
        PartData::Timeline { periods, tasks } => {
            count(periods.len(), 2, 12)?; count(tasks.len(), 1, 8)?;
            for period in periods { bounded_text(period, 16)?; }
            for task in tasks { bounded_text(&task.label, 32)?; if task.start >= task.end || task.end > periods.len() || !task.progress.is_finite() || !(0.0..=1.0).contains(&task.progress) { return Err(Error::Invalid("timeline requires start < end within periods and progress 0-1".into())); } }
        }
        PartData::Waterfall { steps, unit } => {
            count(steps.len(), 2, 10)?; bounded_text(unit, 24)?; let mut running = 0.0;
            for (index, step) in steps.iter().enumerate() { bounded_text(&step.label, 32)?; finite(step.value)?; if step.total { if index > 0 && (step.value - running).abs() > f64::EPSILON * 16.0 * running.abs().max(1.0) { return Err(Error::Invalid("waterfall total does not reconcile".into())); } running = step.value; } else { running += step.value; } finite(running)?; }
        }
        PartData::Map { points } => {
            count(points.len(), 1, 8)?;
            for point in points { bounded_text(&point.label, 32)?; if !point.longitude.is_finite() || !point.latitude.is_finite() || !(-180.0..=180.0).contains(&point.longitude) || !(-85.0..=85.0).contains(&point.latitude) { return Err(Error::Invalid("map coordinates outside supported range".into())); } if let Some(value) = point.value { finite(value)?; if value < 0.0 { return Err(Error::Invalid("map sizes must be nonnegative".into())); } } }
        }
    }
    Ok(())
}

pub(super) fn validate_spec(spec: &PartSpec) -> Result<(&str, usize)> {
    valid_text(&spec.title, 80)?;
    valid_text(&spec.subtitle, 120)?;
    if spec.version != 1 { return Err(Error::Invalid("part version".into())); }
    if spec.preset == "diagram/custom" {
        let PartData::Diagram { graph } = &spec.data else { return Err(Error::Invalid("diagram requires graph data".into())); };
        if graph.title != spec.title || graph.subtitle != spec.subtitle { return Err(Error::Invalid("graph titles must match part metadata".into())); }
        crate::graphs::validate(graph)?;
        return Ok(("diagram", 0));
    }
    let (category, variant) = spec.preset.split_once('/').ok_or_else(|| Error::Invalid("part preset must include category/variant".into()))?;
    let variant = ["balanced", "focus", "labeled"].iter().position(|candidate| *candidate == variant).ok_or_else(|| Error::Unsupported("unknown part variant".into()))?;
    if !catalog::CATEGORIES.iter().any(|entry| entry.0 == category) { return Err(Error::Unsupported("unknown part category".into())); }
    validate_data(&spec.data)?;
    Ok((category, variant))
}

pub fn create(id: &str, spec: &PartSpec) -> Result<Element> { create_with_theme(id, spec, &crate::design::Theme::default()) }

pub fn create_with_theme(id: &str, spec: &PartSpec, theme: &crate::design::Theme) -> Result<Element> {
    valid_text(id,40)?; if id.is_empty() {return Err(Error::Invalid("part identity".into()));}
    crate::design::validate_theme(theme)?;
    let (category,variant)=validate_spec(spec)?;
    if let PartData::Diagram { graph } = &spec.data {
        if category != "diagram" { return Err(Error::Invalid("graph data requires diagram/custom".into())); }
        return crate::graphs::create(id, graph, theme);
    }
    let identity = crate::canonical::bytes(spec)?;
    use sha2::{Digest, Sha256};
    let mut drawing = Drawing::new(&format!("{id}-{}", &format!("{:x}",Sha256::digest(identity))[..10]));
    drawing.text(&spec.title, [16.0, 0.0, 1120.0, 40.0], 28.0, "@dk1", true, TextAlign::Left);
    drawing.text(&spec.subtitle, [16.0, 44.0, 1120.0, 26.0], 16.0, "@dk2", false, TextAlign::Left);
    if catalog::CATEGORIES.iter().take(10).any(|entry| entry.0 == category) { charts::render(&mut drawing, category, variant, &spec.data)?; }
    else { diagrams::render(&mut drawing, category, variant, &spec.data)?; }
    crate::layout::fit_part_text(&mut drawing.elements,theme)?;
    let result = Element::Group { id: id.into(), x: 64.0, y: 144.0, width: 1152.0, height: 512.0, view_width: 1152.0, view_height: 512.0, children: drawing.elements };
    validate_elements(std::slice::from_ref(&result), (1280.0, 720.0), 0, &mut BTreeSet::new(), &mut 0, &mut 0)?;
    Ok(result)
}

pub(super) struct Drawing { prefix: String, pub elements: Vec<Element> }
impl Drawing {
    pub fn new(prefix: &str) -> Self { Self { prefix: prefix.into(), elements: Vec::new() } }
    pub fn id(&self) -> String { format!("{}-{}", self.prefix, self.elements.len()) }
    pub fn text(&mut self, text: &str, bounds: [f64; 4], size: f64, color: &str, bold: bool, alignment: TextAlign) {
        let [x, y, width, height] = bounds;
        self.elements.push(Element::Text { id: self.id(), x, y, width, height, text: text.into(), font_size: size, color: color.into(), bold, format: TextFormat { alignment, font_family: Some("@minor".into()), ..TextFormat::default() } });
    }
    pub fn chart(&mut self, kind: ChartKind, categories: Vec<String>, series: Vec<ChartSeries>, bounds: [f64; 4]) {
        let [x, y, width, height] = bounds;
        self.elements.push(Element::Chart { id: self.id(), x, y, width, height, kind, categories, series });
    }
    pub fn rect(&mut self, bounds: [f64; 4], fill: &str) { let [x,y,width,height] = bounds; if width > 0.0 && height > 0.0 { self.elements.push(Element::Rect { id: self.id(), x,y,width,height,fill:fill.into() }); } }
    pub fn shape(&mut self, preset: &str, bounds: [f64; 4], fill: &str, stroke: &str) {
        let [x,y,width,height] = bounds;
        self.elements.push(Element::Shape { id:self.id(),x,y,width,height,preset:preset.into(),fill:fill.into(),stroke:stroke.into(),stroke_width:1.5,rotation:0.0,text:String::new(),font_size:18.0,color:"@dk1".into(),bold:false,format:TextFormat::default() });
    }
    pub fn polygon(&mut self, points: Vec<[f64; 2]>, fill: &str, stroke: &str) {
        let left = points.iter().map(|point| point[0]).fold(f64::INFINITY, f64::min); let top = points.iter().map(|point| point[1]).fold(f64::INFINITY, f64::min);
        let width = points.iter().map(|point| point[0]).fold(f64::NEG_INFINITY, f64::max) - left; let height = points.iter().map(|point| point[1]).fold(f64::NEG_INFINITY, f64::max) - top;
        if width <= 0.0 || height <= 0.0 { return; }
        let points = points.into_iter().map(|point| [((point[0]-left)/width*1e6).round()/1e6,((point[1]-top)/height*1e6).round()/1e6]).collect();
        self.elements.push(Element::Polygon { id:self.id(),x:left,y:top,width,height,points,fill:fill.into(),stroke:stroke.into(),stroke_width:1.0 });
    }
    pub fn line(&mut self, start: [f64; 2], end: [f64; 2], arrow: bool, color: &str) {
        let target=end;let distance=((end[0]-start[0]).powi(2)+(end[1]-start[1]).powi(2)).sqrt();let unit=if distance>0.0 {[(end[0]-start[0])/distance,(end[1]-start[1])/distance]} else {[1.0,0.0]};
        let (start,end) = if start[0] <= end[0] { (start,end) } else { (end,start) };
        self.elements.push(Element::Connector { id:self.id(),x:start[0],y:start[1].min(end[1]),width:(end[0]-start[0]).max(0.01),height:(end[1]-start[1]).abs().max(0.01),color:color.into(),stroke_width:1.5,arrow:false,flip_v:end[1]<start[1],start:None,end:None,routing:None });
        if arrow {self.polygon(vec![target,[target[0]-unit[0]*9.0-unit[1]*4.0,target[1]-unit[1]*9.0+unit[0]*4.0],[target[0]-unit[0]*9.0+unit[1]*4.0,target[1]-unit[1]*9.0-unit[0]*4.0]],color,color);}
    }
}

pub(super) fn accent(index: usize) -> String { format!("@accent{}", index % 6 + 1) }
pub(super) fn display(value: f64) -> String { if value.abs() >= 1e9 { format!("{:.2}B",value/1e9) } else if value.abs() >= 1e6 { format!("{:.2}M",value/1e6) } else if value.abs() >= 10000.0 { format!("{:.1}k",value/1000.0) } else if value.fract().abs() < 1e-9 { format!("{value:.0}") } else { format!("{value:.2}") } }