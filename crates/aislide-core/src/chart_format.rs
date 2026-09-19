use crate::{model::{ChartKind, ChartSeries, validate_chart, valid_text}, Error, Result};
use serde::{Deserialize, Serialize};

#[path = "chart_presentation.rs"]
mod presentation;
pub use presentation::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChartAxis { #[default] Primary, Secondary }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AxisOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub major_unit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minor_unit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_base: Option<f64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reverse: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
}

impl AxisOptions { pub fn is_default(&self) -> bool { *self == Self::default() } }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LegendPosition { Bottom, Top, Left, Right, TopRight, Hidden }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LabelPosition { Center, InsideEnd, OutsideEnd, BestFit }

pub(crate) const LABEL_POSITION_NS: &str = "urn:aislide:chart-label-position:v1";

pub(crate) fn automatic_label_position(kind: ChartKind) -> bool {
    matches!(kind, ChartKind::Doughnut | ChartKind::Area | ChartKind::Radar | ChartKind::RadarFilled | ChartKind::Column3d | ChartKind::Bar3d)
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DataLabels {
    #[serde(default)] pub show_value: bool,
    #[serde(default)] pub show_category_name: bool,
    #[serde(default)] pub show_series_name: bool,
    #[serde(default)] pub show_percent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<LabelPosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChartOptions {
    #[serde(default, skip_serializing_if = "AxisOptions::is_default")]
    pub primary_axis: AxisOptions,
    #[serde(default, skip_serializing_if = "AxisOptions::is_default")]
    pub secondary_axis: AxisOptions,
    #[serde(default, skip_serializing_if = "AxisOptions::is_default")]
    pub category_axis: AxisOptions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legend: Option<LegendPosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_labels: Option<DataLabels>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waterfall_totals: Option<Vec<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub histogram: Option<HistogramOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub box_whisker: Option<BoxWhiskerOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hierarchy: Option<HierarchyOptions>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "rule", rename_all = "snake_case", deny_unknown_fields)]
pub enum HistogramBinning { Count { count: u32 }, Width { width: f64 } }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntervalClosed { Left, Right }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistogramOptions {
    pub samples: Vec<f64>,
    pub binning: HistogramBinning,
    pub interval_closed: IntervalClosed,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub underflow: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub overflow: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuartileMethod { Inclusive, Exclusive }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoxWhiskerOptions {
    pub samples: Vec<Vec<f64>>,
    pub quartile_method: QuartileMethod,
    pub mean_line: bool,
    pub mean_marker: bool,
    pub nonoutliers: bool,
    pub outliers: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParentLabelLayout { None, Banner, Overlapping }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HierarchyOptions {
    pub paths: Vec<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub parent_labels: Option<ParentLabelLayout>,
}

impl ChartOptions { pub fn is_default(&self) -> bool { *self == Self::default() } }

pub const KINDS: &[ChartKind] = &[
    ChartKind::Column, ChartKind::Bar, ChartKind::Line, ChartKind::Pie, ChartKind::Doughnut,
    ChartKind::Area, ChartKind::Scatter, ChartKind::StackedColumn, ChartKind::StackedBar,
    ChartKind::PercentStackedColumn, ChartKind::PercentStackedBar, ChartKind::Combo,
    ChartKind::Bubble, ChartKind::Radar, ChartKind::RadarFilled,
    ChartKind::Column3d, ChartKind::Bar3d, ChartKind::Pie3d,
    ChartKind::Funnel, ChartKind::Waterfall, ChartKind::Histogram, ChartKind::BoxWhisker, ChartKind::Treemap, ChartKind::Sunburst,
];

pub fn catalog() -> Vec<serde_json::Value> {
    let mut entries: Vec<_> = KINDS.iter().map(|kind| {
        if kind.is_extended() {
            let contract = match kind {
                ChartKind::Histogram => "Raw options.histogram.samples with empty categories/values; explicit count 1..128 or bounded positive bin width. Explicit bins use val attributes required by tested PowerPoint; Open XML SDK 3.5.1 reports two CT_Binning schema errors per histogram. Count, width and single-sample fixtures passed Office editing/render checks; not universal compatibility",
                ChartKind::BoxWhisker => "Raw sample groups with unique category labels and empty series values; at least four samples per group; explicit quartile method and visibility flags",
                ChartKind::Treemap | ChartKind::Sunburst => "Unique equal-depth leaf paths (2..4 labels), matching category leaf labels and strictly positive leaf values; parent-label options only for treemap",
                ChartKind::Funnel => "Funnel requires strictly positive values",
                _ => "Waterfall requires explicit sorted zero-based waterfall_totals indices",
            };
            let limits = if matches!(kind, ChartKind::Histogram | ChartKind::BoxWhisker) { serde_json::json!({"categories":if *kind == ChartKind::Histogram {0} else {32},"series":1,"samples":4096}) } else if matches!(kind, ChartKind::Treemap | ChartKind::Sunburst) { serde_json::json!({"categories":32,"series":1,"path_depth":4}) } else { serde_json::json!({"categories":32,"series":1}) };
            return serde_json::json!({"id":kind,"native":true,"create":true,"read":true,"edit":true,"axis_options":false,"secondary_axis":false,"data_labels":false,"trendlines":false,"error_bars":false,"bubble_sizes":false,"derived":false,"office_validated":false,"limits":limits,"limitations":["Native chartEx requires Office 2016+; no older-Office fallback is emitted",contract,"Only default axes and optional legend placement; static rendering uses AISlide approximations with explicit warnings","Imported edits require representable chart XML and an unchanged synchronized workbook; cross-namespace conversion requires a new chart","Fixture-specific Office results are documented in docs/authoring/chart-ex.md; no general visual parity"]});
        }
        let statistics = matches!(kind, ChartKind::Column | ChartKind::Bar | ChartKind::Line | ChartKind::Area | ChartKind::Scatter | ChartKind::Bubble | ChartKind::Combo);
        let mut limitations = vec!["Editing imported chart data/options requires representable chart XML and an unchanged synchronized embedded workbook", "Office visual validation is not implied by native OOXML support"];
        if automatic_label_position(*kind) || *kind == ChartKind::Combo { limitations.push("Doughnut, area, radar and 3D bar groups require Office automatic label placement; explicit center is retained as AISlide intent, not native position parity"); }
        serde_json::json!({"id":kind,"native":true,"create":true,"read":true,"edit":true,"axis_options":!kind.is_polar(),"secondary_axis":*kind==ChartKind::Combo,"data_labels":true,"trendlines":statistics,"error_bars":statistics,"bubble_sizes":*kind==ChartKind::Bubble,"derived":false,"office_validated":false,"limits":{"categories":32,"series":6},"limitations":limitations})
    }).collect();
    for id in ["line3d", "area3d", "surface3d"] {
        entries.push(serde_json::json!({"id":id,"native":false,"create":false,"read":false,"edit":false,"axis_options":false,"secondary_axis":false,"data_labels":false,"trendlines":false,"error_bars":false,"bubble_sizes":false,"derived":false,"office_validated":false,"limitations":["This native chart family is not implemented and is not an accepted ChartKind"]}));
    }
    entries
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct HistogramBin { pub lower: f64, pub upper: f64, pub upper_inclusive: bool, pub count: u32 }

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct HistogramData {
    pub kind: ChartKind,
    pub sample_count: usize,
    pub bins: Vec<HistogramBin>,
    pub categories: Vec<String>,
    pub series: Vec<ChartSeries>,
}

pub fn histogram(samples: &[f64], bin_edges: &[f64]) -> Result<HistogramData> {
    if samples.is_empty() || samples.len() > 100_000 || !(2..=33).contains(&bin_edges.len()) { return Err(invalid("histogram requires 1..100000 samples and 2..33 explicit bin edges")); }
    if samples.iter().chain(bin_edges).any(|value| !bounded(*value)) || bin_edges.windows(2).any(|edges| edges[0] >= edges[1]) { return Err(invalid("histogram data must be finite in +/-1e15 with strictly increasing edges")); }
    let last = bin_edges[bin_edges.len() - 1];
    if samples.iter().any(|value| *value < bin_edges[0] || *value > last) { return Err(invalid("histogram edges must include every sample; outliers are not silently discarded")); }
    let mut bins: Vec<_> = bin_edges.windows(2).enumerate().map(|(index, edges)| HistogramBin { lower: edges[0], upper: edges[1], upper_inclusive: index == bin_edges.len() - 2, count: 0 }).collect();
    for sample in samples {
        let index = (bin_edges.partition_point(|edge| edge <= sample) - 1).min(bins.len() - 1);
        bins[index].count += 1;
    }
    let categories = bins.iter().map(|bin| format!("[{}, {}{}", bin.lower, bin.upper, if bin.upper_inclusive { "]" } else { ")" })).collect();
    let series = vec![ChartSeries { name: "Count".into(), values: bins.iter().map(|bin| f64::from(bin.count)).collect(), color: "@accent1".into(), ..Default::default() }];
    Ok(HistogramData { kind: ChartKind::Column, sample_count: samples.len(), bins, categories, series })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrendlineKind { Linear, Exponential, Logarithmic, Polynomial, Power, MovingAverage }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Trendline {
    pub kind: TrendlineKind,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub order: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub period: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub intercept: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub forward: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub backward: Option<f64>,
    #[serde(default)] pub display_equation: bool,
    #[serde(default)] pub display_r_squared: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorBarKind { FixedValue, Percentage, StandardDeviation, StandardError, Custom }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorBarDirection { X, #[default] Y }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorBarType { #[default] Both, Plus, Minus }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ErrorBars {
    pub kind: ErrorBarKind,
    #[serde(default)] pub direction: ErrorBarDirection,
    #[serde(default)] pub bar_type: ErrorBarType,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub plus: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub minus: Option<Vec<f64>>,
}

fn invalid(message: &str) -> Error { Error::Invalid(message.into()) }
fn bounded(value: f64) -> bool { value.is_finite() && value.abs() <= 1e15 }

use crate::native::{C, child};
use roxmltree::Node;

fn attribute<'a>(node: Node<'a, '_>, tag: &str) -> Option<&'a str> { child(node, C, tag).and_then(|node| node.attribute("val")) }

fn number(node: Node<'_, '_>, tag: &str) -> Result<Option<f64>> {
    attribute(node, tag).map(|value| value.parse::<f64>().map_err(|_| Error::Unsupported(format!("invalid chart {tag}")))).transpose()
}

fn flag(node: Node<'_, '_>, tag: &str) -> Result<bool> {
    match attribute(node, tag) { None | Some("0" | "false") => Ok(false), Some("1" | "true") => Ok(true), _ => Err(Error::Unsupported(format!("invalid chart {tag}"))) }
}

pub(crate) fn read_axis(node: Node<'_, '_>, defaults: &AxisOptions, default_format: Option<(&str, &str)>) -> Result<AxisOptions> {
    let scaling = child(node, C, "scaling").ok_or_else(|| Error::Unsupported("chart axis scaling missing".into()))?;
    let mut options = AxisOptions {
        min: number(scaling, "min")?, max: number(scaling, "max")?, log_base: number(scaling, "logBase")?,
        major_unit: number(node, "majorUnit")?, minor_unit: number(node, "minorUnit")?,
        reverse: match attribute(scaling, "orientation") { None | Some("minMax") => false, Some("maxMin") => true, _ => return Err(Error::Unsupported("chart orientation".into())) },
        number_format: child(node, C, "numFmt").and_then(|format| {
            let code = format.attribute("formatCode")?;
            if default_format == Some((code, format.attribute("sourceLinked").unwrap_or("1"))) { None } else { Some(code.to_owned()) }
        }),
    };
    if options.log_base.is_none() {
        if options.min == defaults.min { options.min = None; }
        if options.max == defaults.max { options.max = None; }
    }
    Ok(options)
}

pub(crate) fn read_labels(node: Node<'_, '_>, kind: ChartKind) -> Result<Option<DataLabels>> {
    child(node, C, "dLbls").map(|labels| Ok(DataLabels {
        show_value: flag(labels, "showVal")?, show_category_name: flag(labels, "showCatName")?,
        show_series_name: flag(labels, "showSerName")?, show_percent: flag(labels, "showPercent")?,
        position: match attribute(labels, "dLblPos") {
            None => {
                let extension = child(labels, C, "extLst").and_then(|list| list.children().find(|entry| entry.has_tag_name((C, "ext")) && entry.attribute("uri") == Some(LABEL_POSITION_NS)));
                match extension {
                    Some(extension) if automatic_label_position(kind) && child(extension, LABEL_POSITION_NS, "position").and_then(|node| node.attribute("val")) == Some("center") => Some(LabelPosition::Center),
                    Some(_) => return Err(Error::Unsupported("chart fixed label position extension".into())),
                    None => None,
                }
            },
            Some("ctr") => Some(LabelPosition::Center), Some("inEnd") => Some(LabelPosition::InsideEnd), Some("outEnd") => Some(LabelPosition::OutsideEnd), Some("bestFit") => Some(LabelPosition::BestFit),
            _ => return Err(Error::Unsupported("chart label position".into())),
        },
        number_format: child(labels, C, "numFmt").and_then(|node| node.attribute("formatCode")).map(str::to_owned),
    })).transpose()
}

pub(crate) fn read_legend(node: Node<'_, '_>) -> Result<Option<LegendPosition>> {
    match child(node, C, "legend").map(|node| attribute(node, "legendPos")) {
        None => Ok(Some(LegendPosition::Hidden)), Some(None | Some("b")) => Ok(None),
        Some(Some("t")) => Ok(Some(LegendPosition::Top)), Some(Some("l")) => Ok(Some(LegendPosition::Left)),
        Some(Some("r")) => Ok(Some(LegendPosition::Right)), Some(Some("tr")) => Ok(Some(LegendPosition::TopRight)),
        _ => Err(Error::Unsupported("chart legend position".into())),
    }
}

pub(crate) fn read_trendline(node: Node<'_, '_>) -> Result<Option<Trendline>> {
    let trends: Vec<_> = node.children().filter(|node| node.has_tag_name((C, "trendline"))).collect();
    if trends.len() > 1 { return Err(Error::Unsupported("multiple trendlines per series".into())); }
    trends.first().map(|node| {
        let unsigned = |tag: &str| -> Result<Option<u8>> { attribute(*node, tag).map(|value| value.parse().map_err(|_| Error::Unsupported(format!("chart trendline {tag}")))).transpose() };
        Ok(Trendline {
            kind: match attribute(*node, "trendlineType") { Some("linear") => TrendlineKind::Linear, Some("exp") => TrendlineKind::Exponential, Some("log") => TrendlineKind::Logarithmic, Some("poly") => TrendlineKind::Polynomial, Some("power") => TrendlineKind::Power, Some("movingAvg") => TrendlineKind::MovingAverage, _ => return Err(Error::Unsupported("chart trendline type".into())) },
            order: unsigned("order")?, period: unsigned("period")?, intercept: number(*node, "intercept")?,
            forward: number(*node, "forward")?, backward: number(*node, "backward")?,
            display_equation: flag(*node, "dispEq")?, display_r_squared: flag(*node, "dispRSqr")?,
        })
    }).transpose()
}

pub(crate) fn read_errors(node: Node<'_, '_>, kind: ChartKind) -> Result<Option<ErrorBars>> {
    let errors: Vec<_> = node.children().filter(|node| node.has_tag_name((C, "errBars"))).collect();
    if errors.len() > 1 { return Err(Error::Unsupported("multiple error-bar directions per series".into())); }
    errors.first().map(|node| Ok(ErrorBars {
        kind: match attribute(*node, "errValType") { Some("fixedVal") => ErrorBarKind::FixedValue, Some("percentage") => ErrorBarKind::Percentage, Some("stdDev") => ErrorBarKind::StandardDeviation, Some("stdErr") => ErrorBarKind::StandardError, Some("cust") => ErrorBarKind::Custom, _ => return Err(Error::Unsupported("chart error-bar type".into())) },
        direction: match attribute(*node, "errDir") { Some("x") if kind.is_horizontal() => ErrorBarDirection::Y, Some("x") => ErrorBarDirection::X, None | Some("y") if !kind.is_horizontal() => ErrorBarDirection::Y, _ => return Err(Error::Unsupported("chart error-bar direction".into())) },
        bar_type: match attribute(*node, "errBarType") { Some("both") => ErrorBarType::Both, Some("plus") => ErrorBarType::Plus, Some("minus") => ErrorBarType::Minus, _ => return Err(Error::Unsupported("chart error-bar sign".into())) },
        value: number(*node, "val")?,
        plus: child(*node, C, "plus").map(crate::charts::numeric_cache).transpose()?,
        minus: child(*node, C, "minus").map(crate::charts::numeric_cache).transpose()?,
    })).transpose()
}

fn validate_axis(axis: &AxisOptions, numeric: bool) -> Result<()> {
    if [axis.min, axis.max, axis.major_unit, axis.minor_unit, axis.log_base].into_iter().flatten().any(|value| !bounded(value)) { return Err(invalid("chart axis values must be finite in +/-1e15")); }
    if !numeric && [axis.min, axis.max, axis.major_unit, axis.minor_unit, axis.log_base].iter().any(Option::is_some) { return Err(invalid("category axis supports reverse and number_format only")); }
    if axis.min.zip(axis.max).is_some_and(|(min, max)| min >= max) { return Err(invalid("chart axis min must be less than max")); }
    if [axis.major_unit, axis.minor_unit].into_iter().flatten().any(|value| value <= 0.0) { return Err(invalid("chart axis units must be positive")); }
    if let Some(base) = axis.log_base {
        if !(2.0..=1000.0).contains(&base) || [axis.min, axis.max].into_iter().flatten().any(|value| value <= 0.0) { return Err(invalid("log axis requires base 2..1000 and positive bounds")); }
        if axis.major_unit.is_some() || axis.minor_unit.is_some() { return Err(invalid("explicit units on logarithmic axes are not supported")); }
    }
    if let Some(format) = &axis.number_format { valid_text(format, 128)?; if format.is_empty() { return Err(invalid("empty chart number format")); } }
    Ok(())
}

pub fn validate(kind: ChartKind, categories: &[String], series: &[ChartSeries], options: &ChartOptions) -> Result<()> {
    if kind.is_extended() { return crate::chart_extended::validate(kind, categories, series, options); }
    validate_chart(categories, series)?;
    if options.histogram.is_some() || options.box_whisker.is_some() || options.hierarchy.is_some() { return Err(invalid("typed statistical/hierarchy data requires its chartEx family")); }
    if options.waterfall_totals.is_some() { return Err(invalid("waterfall_totals requires waterfall")); }
    if kind.is_percent() && (series.iter().flat_map(|entry| &entry.values).any(|value| *value < 0.0) || (0..categories.len()).any(|index| series.iter().map(|entry| entry.values[index]).sum::<f64>() <= 0.0)) { return Err(invalid("100% stacks require nonnegative values and positive category totals")); }
    if kind.is_polar() && (series.len() != 1 || series[0].values.iter().any(|value| *value < 0.0) || series[0].values.iter().all(|value| *value == 0.0)) { return Err(invalid("pie/doughnut requires one non-negative series with a positive total")); }
    if kind.is_xy() && categories.iter().any(|value| value.parse::<f64>().map_or(true, |number| !bounded(number))) { return Err(invalid("XY coordinates must be finite numbers in +/-1e15")); }
    if kind == ChartKind::Combo {
        if series.iter().any(|entry| !matches!(entry.kind, Some(ChartKind::Column | ChartKind::Line | ChartKind::Area))) { return Err(invalid("combo series require column, line or area kind")); }
        if !series.iter().any(|entry| entry.axis.unwrap_or_default() == ChartAxis::Primary) || series.iter().all(|entry| (entry.kind, entry.axis.unwrap_or_default()) == (series[0].kind, series[0].axis.unwrap_or_default())) { return Err(invalid("combo requires a primary series and at least two distinct kind/axis groups")); }
    } else if series.iter().any(|entry| entry.kind.is_some() || entry.axis.is_some()) { return Err(invalid("per-series kind/axis requires combo")); }
    let secondary = series.iter().any(|entry| entry.axis == Some(ChartAxis::Secondary));
    if !secondary && !options.secondary_axis.is_default() { return Err(invalid("secondary axis options require a secondary series")); }
    if kind.is_polar() && (!options.primary_axis.is_default() || !options.category_axis.is_default()) { return Err(invalid("pie/doughnut charts have no axes")); }
    validate_axis(&options.primary_axis, true)?;
    validate_axis(&options.secondary_axis, true)?;
    validate_axis(&options.category_axis, kind.is_xy())?;
    for (axis, custom) in [(ChartAxis::Primary, &options.primary_axis), (ChartAxis::Secondary, &options.secondary_axis)] {
        let defaults = if custom.log_base.is_none() { crate::charts::default_axis(kind, series, axis) } else { AxisOptions::default() };
        if custom.min.or(defaults.min).zip(custom.max.or(defaults.max)).is_some_and(|(min, max)| min >= max) { return Err(invalid("effective chart axis bounds must increase, including default zero bounds")); }
    }
    if options.category_axis.log_base.is_some() && categories.iter().any(|value| value.parse::<f64>().map_or(true, |value| value <= 0.0)) { return Err(invalid("log X axis requires positive coordinates")); }
    if kind.is_percent() && (options.primary_axis.log_base.is_some() || options.primary_axis.min.is_some_and(|value| value < 0.0) || options.primary_axis.max.is_some_and(|value| value > 1.0)) { return Err(invalid("percentage axis bounds must stay within 0..1 without logarithms")); }
    if let Some(labels) = &options.data_labels {
        if labels.position.is_some_and(|position| position != LabelPosition::Center) && (automatic_label_position(kind) || series.iter().any(|entry| entry.kind.is_some_and(automatic_label_position))) { return Err(invalid("this chart family requires automatic native labels; only center authoring intent can be retained")); }
        if labels.show_percent && !kind.is_polar() { return Err(invalid("native percentage labels require pie/doughnut; stacks use percentage axes")); }
        if let Some(format) = &labels.number_format { valid_text(format, 128)?; if format.is_empty() { return Err(invalid("empty label number format")); } }
        if matches!(labels.position, Some(LabelPosition::BestFit)) && !kind.is_polar() { return Err(invalid("best_fit label position requires pie/doughnut")); }
        if matches!(labels.position, Some(LabelPosition::InsideEnd | LabelPosition::OutsideEnd)) && !(kind.is_bar() || kind.is_polar()) { return Err(invalid("end labels require bars or pie/doughnut")); }
        if labels.position == Some(LabelPosition::OutsideEnd) && (kind.is_percent() || matches!(kind, ChartKind::StackedBar | ChartKind::StackedColumn) || kind.is_3d()) { return Err(invalid("outside_end labels are unsupported for stacked and 3D charts")); }
    }
    for entry in series {
        let actual = entry.kind.unwrap_or(kind);
        let axis = if entry.axis == Some(ChartAxis::Secondary) { &options.secondary_axis } else { &options.primary_axis };
        if axis.log_base.is_some() && entry.values.iter().any(|value| *value <= 0.0) { return Err(invalid("log value axis requires strictly positive series values")); }
        if kind == ChartKind::Bubble {
            if entry.bubble_sizes.as_ref().is_none_or(|sizes| sizes.len() != categories.len() || sizes.iter().any(|value| !bounded(*value) || *value < 0.0)) { return Err(invalid("bubble sizes must match categories and be finite nonnegative values <=1e15")); }
        } else if entry.bubble_sizes.is_some() { return Err(invalid("bubble_sizes requires bubble chart")); }
        if (entry.trendline.is_some() || entry.error_bars.is_some()) && !matches!(actual, ChartKind::Column | ChartKind::Bar | ChartKind::Line | ChartKind::Area | ChartKind::Scatter | ChartKind::Bubble) { return Err(invalid("trendlines/error bars require unstacked 2D cartesian series")); }
        if let Some(trend) = &entry.trendline {
            if entry.values.len() < 2 || [trend.intercept, trend.forward, trend.backward].into_iter().flatten().any(|value| !bounded(value)) || [trend.forward, trend.backward].into_iter().flatten().any(|value| value < 0.0) { return Err(invalid("trendline requires two points and finite bounded coefficients/nonnegative forecasts")); }
            if trend.kind == TrendlineKind::Polynomial {
                if trend.order.is_none_or(|order| !(2..=6).contains(&order) || usize::from(order) >= entry.values.len()) { return Err(invalid("polynomial order 2..6 must be less than point count")); }
            } else if trend.order.is_some() { return Err(invalid("order requires polynomial trendline")); }
            if trend.kind == TrendlineKind::MovingAverage {
                if trend.period.is_none_or(|period| period < 2 || usize::from(period) >= entry.values.len()) || trend.intercept.is_some() || trend.forward.is_some() || trend.backward.is_some() || trend.display_equation || trend.display_r_squared { return Err(invalid("moving average requires period 2..point_count-1 and no regression controls")); }
            } else if trend.period.is_some() { return Err(invalid("period requires moving average")); }
            if matches!(trend.kind, TrendlineKind::Logarithmic | TrendlineKind::Power) && trend.intercept.is_some() { return Err(invalid("intercept is unsupported for logarithmic/power trendlines")); }
            if matches!(trend.kind, TrendlineKind::Exponential | TrendlineKind::Power) && entry.values.iter().any(|value| *value <= 0.0) { return Err(invalid("exponential/power trendlines require positive Y values")); }
            if matches!(trend.kind, TrendlineKind::Logarithmic | TrendlineKind::Power) && kind.is_xy() && categories.iter().any(|value| value.parse::<f64>().map_or(true, |value| value <= 0.0)) { return Err(invalid("logarithmic/power trendlines require positive X values")); }
            if kind.is_xy() && trend.kind != TrendlineKind::MovingAverage {
                let mut coordinates = categories.iter().map(|value| value.parse::<f64>().unwrap_or(f64::NAN)).collect::<Vec<_>>();
                coordinates.sort_by(f64::total_cmp); coordinates.dedup();
                if coordinates.len() <= usize::from(trend.order.unwrap_or(1)) { return Err(invalid("trendline requires enough distinct X coordinates for its order")); }
            }
        }
        if let Some(errors) = &entry.error_bars {
            if matches!(errors.kind, ErrorBarKind::StandardDeviation | ErrorBarKind::StandardError) && entry.values.len() < 2 { return Err(invalid("statistical error bars require at least two data points")); }
            if errors.direction == ErrorBarDirection::X && !actual.is_xy() { return Err(invalid("X error bars require scatter/bubble")); }
            if errors.kind == ErrorBarKind::Custom {
                let needs_plus = errors.bar_type != ErrorBarType::Minus;
                let needs_minus = errors.bar_type != ErrorBarType::Plus;
                for (values, required) in [(&errors.plus, needs_plus), (&errors.minus, needs_minus)] {
                    if required != values.is_some() || values.as_ref().is_some_and(|values| values.len() != categories.len() || values.iter().any(|value| !bounded(*value) || *value < 0.0)) { return Err(invalid("custom error arrays must match direction, point count and nonnegative finite bounds")); }
                }
                if errors.value.is_some() { return Err(invalid("custom errors cannot include scalar value")); }
            } else {
                if errors.plus.is_some() || errors.minus.is_some() { return Err(invalid("error arrays require custom kind")); }
                if errors.kind == ErrorBarKind::StandardError {
                    if errors.value.is_some() { return Err(invalid("standard_error has no scalar value")); }
                } else if errors.value.is_none_or(|value| !bounded(value) || value < 0.0) { return Err(invalid("error bar value must be finite nonnegative and <=1e15")); }
            }
        }
    }
    Ok(())
}