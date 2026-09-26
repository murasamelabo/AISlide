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
    #[serde(default, skip_serializing_if = "Option::is_none")] pub layout: Option<PartLayout>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default = "show_part_title")] pub show_title: bool,
}

fn show_part_title() -> bool { true }

impl PartLayout {
    fn validate(&self) -> Result<()> {
        if [self.x, self.y, self.width, self.height].iter().any(|value| !value.is_finite() || !(0.0..=4096.0).contains(value))
            || self.width <= 0.0 || self.height <= 0.0 || self.x + self.width > 4096.0 || self.y + self.height > 4096.0 {
            return Err(Error::Invalid("part layout requires a positive frame within 0..4096".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PartData {
    Chart { categories: Vec<String>, series: Vec<PartSeries>, #[serde(default)] x_axis: String, #[serde(default)] y_axis: String },
    Items { items: Vec<PartItem>, #[serde(default)] center: String },
    Tree { nodes: Vec<TreeNode> },
    Network { nodes: Vec<PartItem>, edges: Vec<PartEdge> },
    Matrix { rows: Vec<String>, columns: Vec<String>, cells: Vec<Vec<String>>, #[serde(default, skip_serializing_if = "String::is_empty")] #[schemars(length(max = 48))] corner_label: String },
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
        PartData::Matrix { rows, columns, cells, corner_label } => {
            count(rows.len(), 2, 4)?; count(columns.len(), 2, 4)?;
            bounded_text(corner_label, 48)?;
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
    if let Some(layout) = &spec.layout { layout.validate()?; }
    if spec.version != 1 { return Err(Error::Invalid("part version".into())); }
    if spec.preset == "diagram/custom" {
        let PartData::Diagram { graph } = &spec.data else { return Err(Error::Invalid("diagram requires graph data".into())); };
        if graph.title != spec.title || graph.subtitle != spec.subtitle { return Err(Error::Invalid("graph titles must match part metadata".into())); }
        crate::graphs::validate(graph)?;
        return Ok(("diagram", 0));
    }
    let (category, variant) = spec.preset.split_once('/').ok_or_else(|| Error::Invalid("part preset must include category/variant".into()))?;
    let variant = ["balanced", "focus", "labeled"].iter().position(|candidate| *candidate == variant)
        .or_else(|| catalog::OPEN_LISTS.iter().any(|entry| entry.0 == category && entry.1 == variant).then_some(3))
        .ok_or_else(|| Error::Unsupported("unknown part variant".into()))?;
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
        let Some(layout) = &spec.layout else { return crate::graphs::create(id, graph, theme); };
        let mut rendered_graph = graph.clone();
        rendered_graph.show_title = layout.show_title;
        let content_height = if graph.show_title && !layout.show_title {
            for node in &mut rendered_graph.nodes { node.y -= 88.0; }
            for group in &mut rendered_graph.groups { group.y -= 88.0; }
            424.0
        } else { 512.0 };
        use sha2::{Digest, Sha256};
        let identity = format!("{:x}", Sha256::digest(crate::canonical::bytes(&(id, spec))?));
        let graph_id = format!("part-{}", &identity[..24]);
        let mut result = crate::graphs::create(&graph_id, &rendered_graph, theme)?;
        if let Element::Group { id: root_id, .. } = &mut result { *root_id = id.into(); }
        adopt_layout(&mut result, layout, 0.0, content_height, theme, &crate::graphs::small_annotation_ids(&graph_id, &rendered_graph)?)?;
        if let Element::Group { children, .. } = &mut result {
            crate::graphs::cap_detail_fonts(&graph_id, &rendered_graph, children)?;
            crate::graphs::fit_detail_widows(&graph_id, &rendered_graph, children, theme)?;
            crate::graphs::validate_label_overlaps(&graph_id, &rendered_graph, children)?;
        }
        return Ok(result);
    }
    let identity = crate::canonical::bytes(spec)?;
    use sha2::{Digest, Sha256};
    let mut drawing = Drawing::new(&format!("{id}-{}", &format!("{:x}",Sha256::digest(identity))[..10]));
    drawing.text(&spec.title, [16.0, 0.0, 1120.0, 40.0], 28.0, "@dk1", true, TextAlign::Left);
    drawing.text(&spec.subtitle, [16.0, 44.0, 1120.0, 26.0], 16.0, "@dk2", false, TextAlign::Left);
    if catalog::CATEGORIES.iter().take(10).any(|entry| entry.0 == category) { charts::render(&mut drawing, category, variant, &spec.data)?; }
    else { diagrams::render(&mut drawing, category, variant, &spec.data)?; }
    if spec.layout.is_none() { crate::layout::fit_part_text(&mut drawing.elements,theme)?; }
    let mut result = Element::Group { visual: None, id: id.into(), x: 64.0, y: 144.0, width: 1152.0, height: 512.0, view_width: 1152.0, view_height: 512.0, children: drawing.elements };
    validate_elements(std::slice::from_ref(&result), (1280.0, 720.0), 0, &mut BTreeSet::new(), &mut 0, &mut 0)?;
    if let Some(layout) = &spec.layout {
        if !layout.show_title {
            if let Element::Group { children, .. } = &mut result { drop(children.drain(..2)); }
        }
        let content_top = if layout.show_title { 0.0 } else if category == "venn" && variant == 1 { 80.0 } else { 88.0 };
        adopt_layout(&mut result, layout, content_top, 512.0 - content_top, theme, &BTreeSet::new())?;
    }
    Ok(result)
}

fn transform_layout_element(element: &mut Element, horizontal: f64, vertical: f64, content_top: f64) -> Result<()> {
    match element {
        Element::Text { x, y, width, height, .. } | Element::Rect { x, y, width, height, .. }
        | Element::Polygon { x, y, width, height, .. } | Element::Shape { x, y, width, height, .. }
        | Element::Table { x, y, width, height, .. } | Element::Chart { x, y, width, height, .. }
        | Element::Picture { x, y, width, height, .. } | Element::Connector { x, y, width, height, .. }
        | Element::Group { x, y, width, height, .. } => {
            if *y < content_top { return Err(Error::Invalid("part body crosses the removed title band".into())); }
            *x *= horizontal; *y = (*y - content_top) * vertical;
            *width *= horizontal; *height *= vertical;
        }
    }
    match element {
        Element::Group { width, height, view_width, view_height, children, .. } => {
            for child in children { transform_layout_element(child, *width / *view_width, *height / *view_height, 0.0)?; }
            *view_width = *width; *view_height = *height;
        }
        Element::Table { format, .. } => {
            for (tracks, factor) in [(&mut format.column_widths, horizontal), (&mut format.row_heights, vertical)] {
                if let Some(tracks) = tracks {
                    if tracks.unit == crate::table_format::DimensionUnit::Absolute { for value in &mut tracks.values { *value *= factor; } }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn minimum_layout_text(text: &str, size: f64, format: &TextFormat, minimum: f64) -> Result<()> {
    if !text.is_empty() && (size < minimum || format.paragraphs.iter().flat_map(|paragraph| &paragraph.runs)
        .any(|run| !run.text.is_empty() && run.style.font_size.unwrap_or(size) < minimum)) {
        return Err(Error::Invalid(format!("bounded part text requires at least {minimum}px")));
    }
    Ok(())
}

fn adopt_layout(element: &mut Element, layout: &PartLayout, content_top: f64, content_height: f64, theme: &crate::design::Theme, small_annotations: &BTreeSet<String>) -> Result<()> {
    layout.validate()?;
    let Element::Group { x, y, width, height, view_width, view_height, children, .. } = element else {
        return Err(Error::Invalid("part layout requires a group".into()));
    };
    for child in children.iter_mut() { transform_layout_element(child, layout.width / *view_width, layout.height / content_height, content_top)?; }
    *x = layout.x; *y = layout.y; *width = layout.width; *height = layout.height;
    *view_width = layout.width; *view_height = layout.height;
    crate::layout::fit_part_text_with_small_annotations(children, theme, small_annotations)?;
    for child in crate::model::element_list(children) {
        match child {
            Element::Text { id, text, font_size, format, .. } | Element::Shape { id, text, font_size, format, .. } => minimum_layout_text(text, *font_size, format, if small_annotations.contains(id) { 8.0 } else { 12.0 })?,
            Element::Table { rows, font_size, format, .. } => {
                for (row_index, row) in rows.iter().enumerate() {
                    for (column_index, text) in row.iter().enumerate() { minimum_layout_text(text, *font_size, &format.cell_style(row_index, column_index).text(text), 12.0)?; }
                }
            }
            _ => {}
        }
    }
    let mut design = crate::design::Design { theme: theme.clone(), ..Default::default() };
    design.layouts.truncate(1);
    let deck = crate::model::Deck {
        version: 1, title: "Part layout".into(), width: 4096, height: 4096, design: Some(design), embedded_fonts: Vec::new(), auxiliary_design: None,
        slides: vec![crate::model::Slide {
            id: "part-layout".into(), title: "Part layout".into(), background: "@lt1".into(), elements: vec![element.clone()], notes: String::new(),
            notes_paragraphs: Vec::new(), layout_id: None, inherit_background: false, hide_master_graphics: false, native_source_id: None, review: None,
        }],
    };
    let measured = crate::layout::measure_layout(&deck)?;
    if measured.measurements.iter().any(|measurement| measurement.overflow || measurement.missing_glyphs > 0) {
        return Err(Error::Invalid("part content does not fit the requested frame without clipping".into()));
    }
    Ok(())
}

pub(super) struct Drawing { prefix: String, pub elements: Vec<Element> }
impl Drawing {
    pub fn new(prefix: &str) -> Self { Self { prefix: prefix.into(), elements: Vec::new() } }
    pub fn id(&self) -> String { format!("{}-{}", self.prefix, self.elements.len()) }
    pub fn text(&mut self, text: &str, bounds: [f64; 4], size: f64, color: &str, bold: bool, alignment: TextAlign) {
        let [x, y, width, height] = bounds;
        self.elements.push(Element::Text { visual: None, id: self.id(), x, y, width, height, text: text.into(), font_size: size, color: color.into(), bold, format: TextFormat { alignment, font_family: Some("@minor".into()), ..TextFormat::default() } });
    }
    pub fn chart(&mut self, kind: ChartKind, categories: Vec<String>, series: Vec<ChartSeries>, bounds: [f64; 4]) {
        let [x, y, width, height] = bounds;
        self.elements.push(Element::Chart { id: self.id(), x, y, width, height, kind, categories, series, options: Default::default() });
    }
    pub fn rect(&mut self, bounds: [f64; 4], fill: &str) { let [x,y,width,height] = bounds; if width > 0.0 && height > 0.0 { self.elements.push(Element::Rect { visual: None, id: self.id(), x,y,width,height,fill:fill.into() }); } }
    pub fn shape(&mut self, preset: &str, bounds: [f64; 4], fill: &str, stroke: &str) {
        let [x,y,width,height] = bounds;
        self.elements.push(Element::Shape { visual: None, id:self.id(),x,y,width,height,preset:preset.into(),fill:fill.into(),stroke:stroke.into(),stroke_width:1.5,rotation:0.0,text:String::new(),font_size:18.0,color:"@dk1".into(),bold:false,format:TextFormat::default() });
    }
    pub fn polygon(&mut self, points: Vec<[f64; 2]>, fill: &str, stroke: &str) {
        let left = points.iter().map(|point| point[0]).fold(f64::INFINITY, f64::min); let top = points.iter().map(|point| point[1]).fold(f64::INFINITY, f64::min);
        let width = points.iter().map(|point| point[0]).fold(f64::NEG_INFINITY, f64::max) - left; let height = points.iter().map(|point| point[1]).fold(f64::NEG_INFINITY, f64::max) - top;
        if width <= 0.0 || height <= 0.0 { return; }
        let points = points.into_iter().map(|point| [((point[0]-left)/width*1e6).round()/1e6,((point[1]-top)/height*1e6).round()/1e6]).collect();
        self.elements.push(Element::Polygon { visual: None, id:self.id(),x:left,y:top,width,height,points,fill:fill.into(),stroke:stroke.into(),stroke_width:1.0 });
    }
    pub fn line(&mut self, start: [f64; 2], end: [f64; 2], arrow: bool, color: &str) {
        let target=end;let distance=((end[0]-start[0]).powi(2)+(end[1]-start[1]).powi(2)).sqrt();let unit=if distance>0.0 {[(end[0]-start[0])/distance,(end[1]-start[1])/distance]} else {[1.0,0.0]};
        let (start,end) = if start[0] <= end[0] { (start,end) } else { (end,start) };
        self.elements.push(Element::Connector { visual: None, id:self.id(),x:start[0],y:start[1].min(end[1]),width:(end[0]-start[0]).max(0.01),height:(end[1]-start[1]).abs().max(0.01),color:color.into(),stroke_width:1.5,arrow:false,flip_v:end[1]<start[1],start:None,end:None,routing:None });
        if arrow {self.polygon(vec![target,[target[0]-unit[0]*9.0-unit[1]*4.0,target[1]-unit[1]*9.0+unit[0]*4.0],[target[0]-unit[0]*9.0+unit[1]*4.0,target[1]-unit[1]*9.0-unit[0]*4.0]],color,color);}
    }
}

pub(super) fn accent(index: usize) -> String { format!("@accent{}", index % 6 + 1) }
pub(super) fn display(value: f64) -> String { if value.abs() >= 1e9 { format!("{:.2}B",value/1e9) } else if value.abs() >= 1e6 { format!("{:.2}M",value/1e6) } else if value.abs() >= 10000.0 { format!("{:.1}k",value/1000.0) } else if value.fract().abs() < 1e-9 { format!("{value:.0}") } else { format!("{value:.2}") } }

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn titleless_venn_layout_preserves_body_content_for_all_variants() {
        use sha2::{Digest, Sha256};
        let fingerprint = |element: &Element| format!("{:x}", Sha256::digest(serde_json::to_vec(&serde_json::to_value(element).unwrap()).unwrap()));
        for (variant, expected_default, expected_visible) in [
            ("balanced", "d75e70f5a945d7b4a8cea42060789663ceff760ced1b5e3fe95720b159cfa80c", "b81ec53ac088970ee9e9db9e224e1347b7a451676991f1c37698238e3647789f"),
            ("focus", "f3306940ce484b971e1b741d476643634cdb3d6a82e0471c7b02d052411fe6f7", "7c876f58ed924df909737c0d55628297ea15eee439b22e3e4384c92a2f7fd3ee"),
            ("labeled", "6d027d1ee9e09482b1732e1437edf9c2d9db155f50b2d749a5a05baa6f4a8f22", "62622c972e0a67136556639839b7878196a5027fb2f3696da30b5fa965c8359b"),
        ] {
            let spec = PartSpec {
                version: 1, preset: format!("venn/{variant}"), title: "Synthetic title".into(), subtitle: "Synthetic subtitle".into(),
                data: PartData::Items {
                    items: ["A", "B", "C"].into_iter().take(if variant == "focus" { 3 } else { 2 })
                        .map(|label| PartItem { label: label.into(), detail: String::new(), value: None }).collect(),
                    center: "Both".into(),
                },
                layout: None,
            };
            let original = create("venn", &spec).unwrap();
            assert_eq!(fingerprint(&original), expected_default, "{variant}: default compatibility");
            let mut visible = spec.clone();
            visible.layout = Some(PartLayout { x: 40.0, y: 160.0, width: 1152.0, height: 512.0, show_title: true });
            assert_eq!(fingerprint(&create("venn", &visible).unwrap()), expected_visible, "{variant}: title-visible compatibility");
            let Element::Group { children: original_children, .. } = &original else { panic!("expected a group") };
            let mut positioned = spec.clone();
            positioned.layout = Some(PartLayout { x: 40.0, y: 160.0, width: 1152.0, height: 424.0, show_title: false });
            let Element::Group { x, y, width, height, children, .. } = create("venn", &positioned).unwrap() else { panic!("expected a group") };
            assert_eq!([x, y, width, height], [40.0, 160.0, 1152.0, 424.0]);
            assert_eq!(children.len(), original_children.len() - 2);
            if variant == "focus" {
                assert_eq!(children[0].bounds().2, 0.0);
                assert!((children[0].bounds().4 - 296.0 * 424.0 / 432.0).abs() < 0.000001);
            }
            for (child, before) in children.iter().zip(&original_children[2..]) {
                let (_, left, top, width, height) = child.bounds();
                assert!(left >= 0.0 && top >= 0.0 && left + width <= 1152.0 && top + height <= 424.001, "{variant}: {:?}", child.bounds());
                if let (Element::Text { text, .. }, Element::Text { text: original_text, .. }) = (child, before) { assert_eq!(text, original_text); }
            }
        }
    }

    #[test]
    fn bounded_layout_normalizes_nested_geometry_and_absolute_tracks_without_scaling_styles() {
        let rich = json!({"type":"text","id":"rich","x":0,"y":0,"width":200,"height":80,"text":"X","font_size":20,"color":"@dk1","bold":false,
            "format":{"paragraphs":[{"runs":[{"text":"X","style":{"font_size":28,"bold":true,"italic":true,"color":"@accent1"}}]}]}});
        let table = json!({"type":"table","id":"table","x":200,"y":0,"width":600,"height":300,"rows":[["A","B"],["C","D"]],"font_size":18,
            "format":{"column_widths":{"unit":"absolute","values":[200,400]},"row_heights":{"unit":"absolute","values":[150,150]},
                "cells":[{"row":0,"column":0,"style":{"text_style":{"font_size":32,"bold":true}}}]}});
        let source = json!({"type":"rect","id":"source","x":0,"y":300,"width":80,"height":80,"fill":"@accent1"});
        let target = json!({"type":"rect","id":"target","x":400,"y":300,"width":80,"height":80,"fill":"@accent2"});
        let edge = json!({"type":"connector","id":"edge","x":80,"y":340,"width":320,"height":0.01,"color":"@dk1","stroke_width":2,"arrow":true,
            "start":{"element_id":"source","site":3},"end":{"element_id":"target","site":1},"routing":{"points":[[0,0],[1,0]],"dashed":true}});
        let nested = json!({"type":"group","id":"nested","x":100,"y":100,"width":400,"height":200,"view_width":800,"view_height":400,"children":[rich,table,source,target,edge]});
        let mut element: Element = serde_json::from_value(json!({"type":"group","id":"root","x":64,"y":144,"width":1152,"height":512,"view_width":1152,"view_height":512,"children":[nested]})).unwrap();
        let original = serde_json::to_value(&element).unwrap();
        let layout = PartLayout { x: 20.0, y: 30.0, width: 576.0, height: 512.0, show_title: true };
        adopt_layout(&mut element, &layout, 0.0, 512.0, &crate::design::Theme::default(), &BTreeSet::new()).unwrap();
        let result = serde_json::to_value(element).unwrap();
        let nested = &result["children"][0];
        assert_eq!(nested["x"], 50.0);
        assert_eq!(nested["y"], 100.0);
        assert_eq!(nested["width"], 200.0);
        assert_eq!(nested["height"], 200.0);
        assert_eq!(nested["view_width"], 200.0);
        assert_eq!(nested["view_height"], 200.0);
        let before = &original["children"][0]["children"];
        let children = &nested["children"];
        assert_eq!(children[0]["width"], 50.0);
        assert_eq!(children[0]["height"], 40.0);
        assert_eq!(children[0]["font_size"], before[0]["font_size"]);
        assert_eq!(children[0]["format"], before[0]["format"]);
        assert_eq!(children[1]["x"], 50.0);
        assert_eq!(children[1]["width"], 150.0);
        assert_eq!(children[1]["height"], 150.0);
        assert_eq!(children[1]["format"]["column_widths"]["values"], json!([50.0,100.0]));
        assert_eq!(children[1]["format"]["row_heights"]["values"], json!([75.0,75.0]));
        assert_eq!(children[1]["font_size"], before[1]["font_size"]);
        assert_eq!(children[1]["format"]["cells"], before[1]["format"]["cells"]);
        assert_eq!(children[4]["x"], 20.0);
        assert_eq!(children[4]["y"], 170.0);
        assert_eq!(children[4]["width"], 80.0);
        assert_eq!(children[4]["height"], 0.005);
        for field in ["routing", "start", "end", "stroke_width"] { assert_eq!(children[4][field], before[4][field], "{field}"); }
    }

    #[test]
    fn bounded_layout_rejects_rich_table_and_shape_clipping_and_subminimum_text() {
        for child in [
            json!({"type":"text","id":"rich","x":0,"y":88,"width":600,"height":20,"text":"X","font_size":18,"color":"@dk1","bold":false,
                "format":{"paragraphs":[{"runs":[{"text":"X","style":{"font_size":32}}]}]}}),
            json!({"type":"table","id":"table","x":0,"y":88,"width":600,"height":20,"rows":[["Table"]],"font_size":18}),
            json!({"type":"shape","id":"shape","x":0,"y":88,"width":600,"height":20,"text":"X","font_size":18,"color":"@dk1","bold":false,"preset":"rect","fill":"@lt1","stroke":"@dk1","stroke_width":1}),
            json!({"type":"text","id":"small","x":0,"y":88,"width":600,"height":20,"text":"X","font_size":11,"color":"@dk1","bold":false}),
        ] {
            let mut element: Element = serde_json::from_value(json!({"type":"group","id":"root","x":64,"y":144,"width":1152,"height":512,"view_width":1152,"view_height":512,"children":[child]})).unwrap();
            let layout = PartLayout { x: 0.0, y: 0.0, width: 1152.0, height: 424.0, show_title: false };
            assert!(adopt_layout(&mut element, &layout, 88.0, 424.0, &crate::design::Theme::default(), &BTreeSet::new()).is_err(), "{child}");
        }
    }
}