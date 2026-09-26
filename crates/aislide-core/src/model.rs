use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deck {
    pub version: u32,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub slides: Vec<Slide>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<crate::design::Design>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub embedded_fonts: Vec<crate::fonts::EmbeddedFont>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auxiliary_design: Option<AuxiliaryDesign>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuxiliaryMaster {
    pub name: String,
    pub background: String,
    pub theme: crate::design::Theme,
    pub elements: Vec<Element>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuxiliaryDesign {
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes_master: Option<AuxiliaryMaster>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handout_master: Option<AuxiliaryMaster>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slide {
    pub id: String,
    pub title: String,
    pub background: String,
    pub elements: Vec<Element>,
    pub notes: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes_paragraphs: Vec<crate::rich_text::RichParagraph>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_id: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub inherit_background: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hide_master_graphics: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<crate::review::SlideReview>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Element {
    Text { id: String, x: f64, y: f64, width: f64, height: f64, text: String, font_size: f64, color: String, bold: bool, #[serde(default, skip_serializing_if = "TextFormat::is_default")] format: TextFormat, #[serde(default, skip_serializing_if = "Option::is_none")] visual: Option<crate::visual::VisualStyle> },
    Rect { id: String, x: f64, y: f64, width: f64, height: f64, fill: String, #[serde(default, skip_serializing_if = "Option::is_none")] visual: Option<crate::visual::VisualStyle> },
    Polygon { id: String, x: f64, y: f64, width: f64, height: f64, points: Vec<[f64; 2]>, fill: String, stroke: String, stroke_width: f64, #[serde(default, skip_serializing_if = "Option::is_none")] visual: Option<crate::visual::VisualStyle> },
    Shape { id: String, x: f64, y: f64, width: f64, height: f64, preset: String, fill: String, stroke: String, stroke_width: f64, #[serde(default)] rotation: f64, text: String, font_size: f64, color: String, bold: bool, #[serde(default)] format: TextFormat, #[serde(default, skip_serializing_if = "Option::is_none")] visual: Option<crate::visual::VisualStyle> },
    Table { id: String, x: f64, y: f64, width: f64, height: f64, rows: Vec<Vec<String>>, font_size: f64, #[serde(default, skip_serializing_if = "crate::table_format::TableFormat::is_default")] format: crate::table_format::TableFormat },
    Chart { id: String, x: f64, y: f64, width: f64, height: f64, kind: ChartKind, categories: Vec<String>, series: Vec<ChartSeries>, #[serde(default, skip_serializing_if = "ChartOptions::is_default")] options: ChartOptions },
    Picture { id: String, x: f64, y: f64, width: f64, height: f64, base64: String, mime_type: String, alt: String, #[serde(default)] crop: Crop, #[serde(default, skip_serializing_if = "Option::is_none")] visual: Option<crate::visual::VisualStyle>, #[serde(default, skip_serializing_if = "Option::is_none")] svg: Option<String> },
    Connector { id: String, x: f64, y: f64, width: f64, height: f64, color: String, stroke_width: f64, arrow: bool, #[serde(default)] flip_v: bool, #[serde(default)] start: Option<Connection>, #[serde(default)] end: Option<Connection>, #[serde(default, skip_serializing_if = "Option::is_none")] routing: Option<ConnectorRouting>, #[serde(default, skip_serializing_if = "Option::is_none")] visual: Option<crate::visual::VisualStyle> },
    Group { id: String, x: f64, y: f64, width: f64, height: f64, view_width: f64, view_height: f64, children: Vec<Element>, #[serde(default, skip_serializing_if = "Option::is_none")] visual: Option<crate::visual::VisualStyle> },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign { #[default] Left, Center, Right, Justify }
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerticalAlign { #[default] Top, Middle, Bottom }
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bullet { #[default] None, Bullet, Numbered }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceholderKind { Title, Body, Subtitle, Footer, Date, SlideNumber }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placeholder { pub kind: PlaceholderKind, pub index: u32 }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TextFormat {
    pub italic: bool, pub underline: bool, pub alignment: TextAlign, pub vertical: VerticalAlign, pub bullet: Bullet,
    pub font_family: Option<String>, pub hyperlink: Option<String>, pub placeholder: Option<Placeholder>, pub inherit_layout: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "crate::rich_text::deserialize_paragraphs")]
    pub paragraphs: Vec<crate::rich_text::RichParagraph>,
}
impl TextFormat { pub fn is_default(&self) -> bool { self == &Self::default() } }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Crop { pub left: f64, pub top: f64, pub right: f64, pub bottom: f64 }

impl Crop {
    pub(crate) fn validate(&self) -> Result<()> {
        if [self.left, self.right, self.top, self.bottom].iter().any(|value| !value.is_finite() || !(0.0..1.0).contains(value)) || self.left + self.right >= 1.0 || self.top + self.bottom >= 1.0 {
            return Err(Error::Invalid("image crop must retain positive area".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection { pub element_id: String, pub site: u32 }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorRouting {
    pub points: Vec<[f64; 2]>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")] pub custom: bool,
    #[serde(default)] pub start_arrow: bool,
    #[serde(default)] pub dashed: bool,
}

pub(crate) struct ConnectorPreset {
    pub name: &'static str,
    pub adjustments: Vec<(&'static str, f64)>,
    pub flip_h: bool,
    pub flip_v: bool,
    pub zero_width: bool,
    pub zero_height: bool,
}

impl ConnectorRouting {
    pub(crate) fn native_preset(&self) -> Option<ConnectorPreset> {
        if self.custom { return None; }
        let start = self.points.first()?;
        let end = self.points.last()?;
        let flip_h = start[0] > end[0];
        let flip_v = start[1] > end[1];
        let points: Vec<_> = self.points.iter().map(|point| [if flip_h { 1.0 - point[0] } else { point[0] }, if flip_v { 1.0 - point[1] } else { point[1] }]).collect();
        let end = points.last()?;
        if points[0] != [0.0, 0.0] || *end == [0.0, 0.0] || end.iter().any(|value| *value != 0.0 && *value != 1.0) { return None; }
        let (name, adjustments) = match points.as_slice() {
            [_, _] => ("straightConnector1", vec![]),
            [_, corner, end] if *end == [1.0, 1.0] && *corner == [1.0, 0.0] => ("bentConnector2", vec![]),
            [_, corner, end] if *end == [1.0, 1.0] && *corner == [0.0, 1.0] => ("bentConnector3", vec![("adj1", 0.0)]),
            [_, first, second, end] if *end == [1.0, 1.0] && first[0] == second[0] && first[1] == 0.0 && second[1] == 1.0 => ("bentConnector3", vec![("adj1", first[0] * 100000.0)]),
            [_, first, second, end] if *end == [1.0, 1.0] && first[1] == second[1] && first[0] == 0.0 && second[0] == 1.0 => ("bentConnector4", vec![("adj1", 0.0), ("adj2", first[1] * 100000.0)]),
            _ => return None,
        };
        if adjustments.iter().any(|(_, value)| !value.is_finite() || !(0.0..=100000.0).contains(value) || (value.round() - value).abs() > 1e-6) { return None; }
        Some(ConnectorPreset { name, adjustments, flip_h, flip_v, zero_width: end[0] == 0.0, zero_height: end[1] == 0.0 })
    }
}

#[path = "chart_format.rs"]
pub mod chart_format;
pub use chart_format::{ChartOptions, ChartAxis, Trendline, ErrorBars};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind { Column, Bar, Line, Pie, Doughnut, Area, Scatter, StackedColumn, StackedBar, PercentStackedColumn, PercentStackedBar, Combo, Bubble, Radar, RadarFilled, Column3d, Bar3d, Pie3d, Funnel, Waterfall, Histogram, BoxWhisker, Treemap, Sunburst }
impl ChartKind {
    pub fn is_extended(self) -> bool { matches!(self, Self::Funnel | Self::Waterfall | Self::Histogram | Self::BoxWhisker | Self::Treemap | Self::Sunburst) }
    pub fn is_bar(self) -> bool { matches!(self, Self::Column | Self::Bar | Self::StackedColumn | Self::StackedBar | Self::PercentStackedColumn | Self::PercentStackedBar | Self::Column3d | Self::Bar3d) }
    pub fn is_horizontal(self) -> bool { matches!(self, Self::Bar | Self::StackedBar | Self::PercentStackedBar | Self::Bar3d) }
    pub fn is_percent(self) -> bool { matches!(self, Self::PercentStackedColumn | Self::PercentStackedBar) }
    pub fn is_polar(self) -> bool { matches!(self, Self::Pie | Self::Doughnut | Self::Pie3d) }
    pub fn is_xy(self) -> bool { matches!(self, Self::Scatter | Self::Bubble) }
    pub fn is_3d(self) -> bool { matches!(self, Self::Column3d | Self::Bar3d | Self::Pie3d) }
    pub fn is_radar(self) -> bool { matches!(self, Self::Radar | Self::RadarFilled) }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChartSeries {
    pub name: String, pub values: Vec<f64>, pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub kind: Option<ChartKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub axis: Option<ChartAxis>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub bubble_sizes: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub trendline: Option<Trendline>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub error_bars: Option<ErrorBars>,
}

impl Element {
    pub fn bounds(&self) -> (&str, f64, f64, f64, f64) {
        match self {
            Self::Text { id, x, y, width, height, .. }
            | Self::Rect { id, x, y, width, height, .. }
            | Self::Polygon { id, x, y, width, height, .. }
            | Self::Shape { id, x, y, width, height, .. }
            | Self::Table { id, x, y, width, height, .. }
            | Self::Chart { id, x, y, width, height, .. }
            | Self::Picture { id, x, y, width, height, .. }
            | Self::Connector { id, x, y, width, height, .. }
            | Self::Group { id, x, y, width, height, .. } => (id, *x, *y, *width, *height),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Issue {
    pub severity: String,
    pub code: String,
    pub message: String,
}

pub fn valid_text(value: &str, limit: usize) -> Result<()> {
    if value.chars().count() > limit {
        return Err(Error::Limit(format!("text exceeds {limit} characters")));
    }
    if value.chars().any(|character| !matches!(character, '\u{9}' | '\u{a}' | '\u{d}' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')) {
        return Err(Error::Invalid("character not permitted in XML 1.0".into()));
    }
    Ok(())
}

pub fn valid_color(value: &str) -> Result<()> {
    if value.strip_prefix('@').is_some_and(|key| crate::design::COLOR_KEYS.contains(&key)) { return Ok(()); }
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::Invalid("color must be six hexadecimal digits".into()));
    }
    Ok(())
}

pub fn validate_deck(deck: &Deck) -> Result<()> {
    crate::fonts::validate(&deck.embedded_fonts)?;
    crate::preflight::deck(deck, &crate::limits::LARGE)?;
    if deck.version != 1 {
        return Err(Error::Unsupported("expected scene version 1".into()));
    }
    crate::canvas::validate_size(deck.width, deck.height)?;
    valid_text(&deck.title, 120)?;
    if let Some(design) = &deck.design { crate::design::validate_design_on_canvas(design, deck.width, deck.height)?; }
    let mut slide_ids = BTreeSet::new();
    let mut total = 0;
    let mut image_bytes = 0;
    if let Some(auxiliary) = &deck.auxiliary_design {
        crate::canvas::validate_size(auxiliary.width, auxiliary.height)?;
        for master in [&auxiliary.notes_master, &auxiliary.handout_master].into_iter().flatten() {
            valid_text(&master.name, 100)?; valid_color(&master.background)?;
            crate::design::validate_theme(&master.theme)?;
            validate_elements(&master.elements, (f64::from(auxiliary.width), f64::from(auxiliary.height)), 0, &mut BTreeSet::new(), &mut total, &mut image_bytes)?;
            if element_list(&master.elements).iter().any(|element| matches!(element, Element::Text { format, .. } if format.inherit_layout)) { return Err(Error::Invalid("auxiliary masters cannot inherit slide layouts".into())); }
        }
    }
    for slide in &deck.slides {
        valid_text(&slide.id, 80)?;
        valid_text(&slide.title, 120)?;
        valid_text(&slide.notes, 8000)?;
        crate::rich_text::validate_paragraphs_with_limit(&slide.notes_paragraphs, 8000)?;
        if !slide.notes_paragraphs.is_empty() && crate::rich_text::plain_text(&slide.notes_paragraphs) != slide.notes { return Err(Error::Unsupported("rich notes require an atomic plain text and paragraph update".into())); }
        valid_color(&slide.background)?;
        if let Some(id) = &slide.layout_id { if !deck.design.as_ref().is_some_and(|design| design.layouts.iter().any(|layout| &layout.id == id)) { return Err(Error::Invalid("slide refers to an unknown layout".into())); } }
        if slide.id.is_empty() || !slide_ids.insert(&slide.id) {
            return Err(Error::Invalid("duplicate/empty slide ID or too many elements".into()));
        }
        let mut ids = BTreeSet::new();
        validate_elements(&slide.elements, (f64::from(deck.width), f64::from(deck.height)), 0, &mut ids, &mut total, &mut image_bytes)?;
        crate::design::validate_inheritance(slide, deck.design.as_ref())?;
        crate::review::validate_slide(slide)?;
    }
    Ok(())
}

pub(crate) fn validate_elements(elements: &[Element], canvas: (f64, f64), depth: usize, ids: &mut BTreeSet<String>, total: &mut usize, image_bytes: &mut usize) -> Result<()> {
    if depth > crate::limits::STANDARD.group_depth { return Err(Error::Limit("group nesting > 8".into())); }
    *total += elements.len();
    if elements.len() > crate::limits::STANDARD.elements_per_slide || *total > crate::limits::STANDARD.elements_total { return Err(Error::Limit("too many scene elements".into())); }
    for element in elements {
            let (id, x, y, width, height) = element.bounds();
            valid_text(id, 80)?;
            if id.is_empty() || !ids.insert(id.into()) || ids.len() > crate::limits::STANDARD.elements_per_slide || [x, y, width, height].iter().any(|value| !value.is_finite())
                || x < 0.0 || y < 0.0 || width <= 0.0 || height <= 0.0 || x + width > canvas.0 + 0.01 || y + height > canvas.1 + 0.01 {
                return Err(Error::Invalid(format!("invalid element geometry or ID: {id}")));
            }
            crate::visual::validate(element)?;
            match element {
                Element::Text { text, font_size, color, format, .. } | Element::Shape { text, font_size, color, format, .. } => {
                    valid_text(text, 4000)?;
                    crate::rich_text::validate_element(element)?;
                    valid_color(color)?;
                    font_size_check(*font_size)?;
                    if let Some(family) = &format.font_family { valid_text(family, 100)?; if family.trim().is_empty() || family.chars().any(char::is_control) { return Err(Error::Invalid("invalid font family".into())); } }
                    if let Some(link) = &format.hyperlink { validate_hyperlink(link)?; }
                    if format.placeholder.as_ref().is_some_and(|placeholder| placeholder.index > 255) { return Err(Error::Invalid("placeholder index > 255".into())); }
                    if (format.placeholder.is_some() || format.inherit_layout) && (depth > 0 || matches!(element, Element::Shape { .. })) { return Err(Error::Unsupported("placeholders must be top-level text boxes".into())); }
                    if let Element::Shape { preset, fill, stroke, stroke_width, rotation, .. } = element {
                        if !crate::objects::SHAPES.iter().any(|(id, _)| *id == preset) && !matches!(preset.as_str(), "can" | "cloud") { return Err(Error::Unsupported("unknown shape preset".into())); }
                        if fill != "none" { valid_color(fill)?; } valid_color(stroke)?;
                        if !stroke_width.is_finite() || !(0.0..=20.0).contains(stroke_width) || !rotation.is_finite() || !(-360.0..=360.0).contains(rotation) { return Err(Error::Invalid("shape stroke or rotation out of range".into())); }
                    }
                }
                Element::Rect { fill, .. } => valid_color(fill)?,
                Element::Polygon { points, fill, stroke, stroke_width, .. } => {
                    if !(3..=4096).contains(&points.len()) || points.iter().flatten().any(|value| !value.is_finite() || !(0.0..=1.0).contains(value)) { return Err(Error::Invalid("polygon requires 3-4096 normalized finite points".into())); }
                    if fill != "none" { valid_color(fill)?; } valid_color(stroke)?;
                    if !stroke_width.is_finite() || !(0.0..=20.0).contains(stroke_width) { return Err(Error::Invalid("polygon stroke out of range".into())); }
                }
                Element::Table { rows, font_size, .. } => {
                    font_size_check(*font_size)?;
                    validate_rows(rows)?;
                    crate::table_format::validate_element(element)?;
                }
                Element::Chart { kind, categories, series, options, .. } => chart_format::validate(*kind, categories, series, options)?,
                Element::Picture { base64, mime_type, alt, crop, svg, .. } => {
                    valid_text(alt, 500)?;
                    *image_bytes += base64.len();
                    if let Some(svg) = svg {
                        *image_bytes += svg.len();
                        if mime_type != "image/png" { return Err(Error::Invalid("SVG requires a PNG fallback".into())); }
                        crate::vector::prepare_svg(svg)?;
                    }
                    if *image_bytes > crate::limits::STANDARD.image_encoded_bytes { return Err(Error::Limit("scene image payload > 3 MiB encoded".into())); }
                    crop.validate()?;
                    crate::media::inspect_raster(base64, mime_type)?;
                }
                Element::Connector { color, stroke_width, start, end, routing, flip_v, .. } => {
                    valid_color(color)?;
                    if !stroke_width.is_finite() || !(0.5..=20.0).contains(stroke_width) { return Err(Error::Invalid("connector stroke must be 0.5-20 pixels".into())); }
                    if let Some(route) = routing {
                        let maximum = if route.custom { 18 } else { 4 };
                        if *flip_v || !(2..=maximum).contains(&route.points.len()) || route.points.iter().flatten().any(|value| !value.is_finite() || !(0.0..=1.0).contains(value)) { return Err(Error::Invalid(format!("routed connector requires 2-{maximum} normalized points without flip_v"))); }
                        if route.points.windows(2).all(|points| points[0] == points[1]) { return Err(Error::Invalid("connector path must have length".into())); }
                        if route.custom {
                            let extent = [(width * 9525.0).round(), (height * 9525.0).round()];
                            if route.points.windows(2).any(|points| (0..2).all(|axis| extent[axis] == 0.0 || (points[0][axis] * 1e6).round() == (points[1][axis] * 1e6).round())) { return Err(Error::Invalid("custom connector segments must remain nonzero at native coordinate precision".into())); }
                        } else if route.native_preset().is_none() { return Err(Error::Unsupported("connector route must fit a standard straight or one/two-bend DrawingML preset".into())); }
                    }
                    for connection in [start, end].into_iter().flatten() {
                        let target = elements.iter().find(|candidate| candidate.bounds().0 == connection.element_id);
                        let sites = match target {
                            Some(Element::Shape { visual: Some(style), .. }) if !style.connection_sites.is_empty() => style.connection_sites.len() as u32,
                            Some(Element::Shape { preset, .. }) if preset == "ellipse" => 8,
                            Some(Element::Shape { preset, .. }) if preset == "can" => 5,
                            _ => 4,
                        };
                        if connection.site >= sites || connection.element_id == id || !target.is_some_and(|candidate| matches!(candidate, Element::Rect { .. } | Element::Shape { .. } | Element::Text { .. } | Element::Picture { .. })) {
                            return Err(Error::Invalid("connector target must be a supported sibling with an existing connection site".into()));
                        }
                    }
                }
                Element::Group { view_width, view_height, children, .. } => {
                    if children.is_empty() || [*view_width, *view_height].iter().any(|value| !value.is_finite() || !(1.0..=4096.0).contains(value)) { return Err(Error::Invalid("group requires children and a 1-4096px coordinate space".into())); }
                    validate_elements(children, (*view_width, *view_height), depth + 1, ids, total, image_bytes)?;
                }
            }
    }
    Ok(())
}

pub fn validate_hyperlink(value: &str) -> Result<()> {
    valid_text(value, 2048)?;
    let parsed = reqwest::Url::parse(value).map_err(|_| Error::Invalid("hyperlink must be an absolute URL".into()))?;
    if !["https", "http", "mailto"].contains(&parsed.scheme()) || !parsed.username().is_empty() || parsed.password().is_some() || value.chars().any(char::is_control) { return Err(Error::Unsupported("hyperlink scheme or embedded credentials".into())); }
    Ok(())
}

pub(crate) fn element_list(elements: &[Element]) -> Vec<&Element> {
    let mut result = Vec::new();
    for element in elements {
        result.push(element);
        if let Element::Group { children, .. } = element { result.extend(element_list(children)); }
    }
    result
}

fn font_size_check(size: f64) -> Result<()> {
    if !size.is_finite() || !(8.0..=120.0).contains(&size) {
        return Err(Error::Invalid("font size must be 8-120 pixels".into()));
    }
    Ok(())
}

pub fn validate_chart(categories: &[String], series: &[ChartSeries]) -> Result<()> {
    if categories.is_empty() || categories.len() > 32 || series.is_empty() || series.len() > 6 {
        return Err(Error::Invalid("chart requires 1-32 categories and 1-6 series".into()));
    }
    for category in categories { valid_text(category, 80)?; }
    for entry in series {
        valid_text(&entry.name, 80)?;
        valid_color(&entry.color)?;
        if entry.values.len() != categories.len() || entry.values.iter().any(|value| !value.is_finite() || value.abs() > 1e15) {
            return Err(Error::Invalid("chart series must match categories with finite values in +/-1e15".into()));
        }
    }
    Ok(())
}

pub fn validate_chart_kind(kind: ChartKind, categories: &[String], series: &[ChartSeries]) -> Result<()> {
    chart_format::validate(kind, categories, series, &ChartOptions::default())
}

pub fn validate_rows(rows: &[Vec<String>]) -> Result<()> {
    let columns = rows.first().map_or(0, Vec::len);
    if rows.is_empty() || rows.len() > 12 || columns == 0 || columns > 8 || rows.iter().any(|row| row.len() != columns) {
        return Err(Error::Invalid("table must be rectangular, 1-12 rows, 1-8 columns".into()));
    }
    for cell in rows.iter().flatten() { valid_text(cell, 200)?; }
    Ok(())
}