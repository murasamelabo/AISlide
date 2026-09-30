use super::briefing::{field, label, message, mix, shape, text, too_small, Drawn, Fill, Rich, GAP, SMALL};
use super::{display, Drawing, PartData, PartMessage};
use crate::{design::Theme, model::{chart_format::{AxisOptions, ChartAxis, ChartOptions, LegendPosition}, ChartKind, ChartSeries, Element, TextAlign, VerticalAlign}, vector::{PathCommand, VectorPath}, visual::VisualStyle, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const KPI_HEIGHT: f64 = 200.0;
const KAPPA: f64 = 0.552_284_75;
const RACI_CODES: [&str; 6] = ["", "R", "A", "C", "I", "A/R"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KpiStatus { #[default] Neutral, Good, Bad }

fn neutral(status: &KpiStatus) -> bool { *status == KpiStatus::Neutral }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KpiCard {
    pub label: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub unit: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub delta: String,
    #[serde(default, skip_serializing_if = "neutral")] pub status: KpiStatus,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub comparison: String,
}

/// `ranges` are ascending qualitative band limits; the last one is the axis maximum.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BulletRow {
    pub label: String,
    pub actual: f64,
    pub target: f64,
    pub ranges: Vec<f64>,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub unit: String,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub note: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")] pub lower_is_better: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VarianceRow { pub label: String, pub plan: f64, pub actual: f64 }

/// `levels` holds one 0-4 quarter count per column.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HarveyRow { pub label: String, pub levels: Vec<u8> }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HeatmapRow { pub label: String, pub values: Vec<f64> }

/// `assignments` holds one of "", "R", "A", "C", "I" or "A/R" per role.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RaciTask { pub label: String, pub assignments: Vec<String> }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Risk {
    pub id: String,
    pub label: String,
    pub likelihood: u8,
    pub impact: u8,
    #[serde(default, skip_serializing_if = "String::is_empty")] pub action: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ParetoItem { pub label: String, pub value: f64 }

pub(super) fn single(value: &str, maximum: usize, name: &str, required: bool) -> Result<()> {
    field(value, maximum, name, required)?;
    if value.contains('\n') { return Err(Error::Invalid(format!("{name} must be one line"))); }
    Ok(())
}

pub(super) fn items(length: usize, minimum: usize, maximum: usize, name: &str) -> Result<()> {
    if !(minimum..=maximum).contains(&length) { return Err(Error::Invalid(format!("{name} require {minimum}-{maximum} items"))); }
    Ok(())
}

pub(super) fn finite(value: f64, name: &str) -> Result<()> {
    if !value.is_finite() || value.abs() > 1e15 { return Err(Error::Invalid(format!("{name} must be finite and within +/-1e15"))); }
    Ok(())
}

fn labels(values: &[String], expected: usize, maximum: usize, name: &str) -> Result<()> {
    if !values.is_empty() && values.len() != expected { return Err(Error::Invalid(format!("{name} requires exactly {expected} entries when present"))); }
    for value in values { single(value, maximum, name, true)?; }
    Ok(())
}

fn kpi_columns(count: usize) -> usize { match count { 0..=4 => count.max(1), 5 | 6 => 3, _ => 4 } }

fn score(risk: &Risk) -> u8 { risk.likelihood * risk.impact }

pub(super) fn validate(data: &PartData) -> Result<()> {
    match data {
        PartData::KpiCards { cards, columns, message: band } => {
            items(cards.len(), 1, 8, "kpi cards")?;
            let columns = columns.unwrap_or_else(|| kpi_columns(cards.len()));
            if !(1..=4).contains(&columns) || columns > cards.len() || cards.len().div_ceil(columns) > 2 {
                return Err(Error::Invalid("kpi cards require 1-4 columns, at most the card count and at most two rows".into()));
            }
            for card in cards {
                field(&card.label, 48, "kpi label", true)?; single(&card.value, 16, "kpi value", true)?; single(&card.unit, 12, "kpi unit", false)?;
                field(&card.delta, 40, "kpi delta", false)?; field(&card.comparison, 60, "kpi comparison", false)?;
            }
            message(band)?;
        }
        PartData::BulletGraphs { rows, message: band } => {
            items(rows.len(), 1, 6, "bullet graphs")?;
            for row in rows {
                field(&row.label, 40, "bullet label", true)?; single(&row.unit, 8, "bullet unit", false)?; field(&row.note, 80, "bullet note", false)?;
                if !(1..=3).contains(&row.ranges.len()) { return Err(Error::Invalid("bullet ranges require 1-3 ascending limits".into())); }
                for value in row.ranges.iter().chain([&row.actual, &row.target]) { finite(*value, "bullet values")?; }
                if row.actual < 0.0 || row.target < 0.0 || row.ranges[0] <= 0.0 || row.ranges.windows(2).any(|pair| pair[1] <= pair[0]) {
                    return Err(Error::Invalid("bullet values must be nonnegative and ranges strictly ascending from above zero".into()));
                }
                let maximum = row.ranges[row.ranges.len() - 1];
                if row.actual > maximum || row.target > maximum { return Err(Error::Invalid("bullet actual and target must not exceed the last range".into())); }
            }
            message(band)?;
        }
        PartData::Variance { rows, unit, plan_label, actual_label, variance_label, total_label, lower_is_better: _, message: band } => {
            items(rows.len(), 2, 10, "variance rows")?;
            single(unit, 8, "variance unit", false)?; single(plan_label, 16, "plan_label", false)?; single(actual_label, 16, "actual_label", false)?;
            single(variance_label, 16, "variance_label", false)?; single(total_label, 24, "total_label", false)?;
            for row in rows { single(&row.label, 32, "variance label", true)?; finite(row.plan, "variance plan")?; finite(row.actual, "variance actual")?; }
            message(band)?;
        }
        PartData::HarveyMatrix { columns, rows, legend, message: band } => {
            items(columns.len(), 2, 6, "harvey columns")?; items(rows.len(), 2, 8, "harvey rows")?;
            for column in columns { field(column, 24, "harvey column", true)?; }
            for row in rows {
                single(&row.label, 40, "harvey row", true)?;
                if row.levels.len() != columns.len() || row.levels.iter().any(|level| *level > 4) { return Err(Error::Invalid("harvey levels require one 0-4 value per column".into())); }
            }
            labels(legend, 5, 20, "harvey legend")?; message(band)?;
        }
        PartData::Heatmap { columns, rows, unit, midpoint, lower_is_better: _, message: band } => {
            items(columns.len(), 2, 12, "heatmap columns")?; items(rows.len(), 2, 10, "heatmap rows")?;
            for column in columns { single(column, 16, "heatmap column", true)?; }
            single(unit, 4, "heatmap unit", false)?;
            for row in rows {
                single(&row.label, 32, "heatmap row", true)?;
                if row.values.len() != columns.len() { return Err(Error::Invalid("heatmap values require one value per column".into())); }
                for value in &row.values { finite(*value, "heatmap values")?; }
            }
            if let Some(value) = midpoint { finite(*value, "heatmap midpoint")?; }
            message(band)?;
        }
        PartData::Raci { roles, tasks, legend, message: band } => {
            items(roles.len(), 2, 8, "raci roles")?; items(tasks.len(), 2, 10, "raci tasks")?;
            for role in roles { field(role, 20, "raci role", true)?; }
            for task in tasks {
                single(&task.label, 48, "raci task", true)?;
                if task.assignments.len() != roles.len() || task.assignments.iter().any(|code| !RACI_CODES.contains(&code.as_str())) {
                    return Err(Error::Invalid("raci assignments require one of \"\", R, A, C, I or A/R per role".into()));
                }
                let accountable = task.assignments.iter().filter(|code| code.contains('A')).count();
                if accountable != 1 { return Err(Error::Invalid(format!("raci task \"{}\" needs exactly one A (found {accountable})", task.label))); }
                if !task.assignments.iter().any(|code| code.contains('R')) { return Err(Error::Invalid(format!("raci task \"{}\" needs at least one R", task.label))); }
            }
            labels(legend, 4, 24, "raci legend")?; message(band)?;
        }
        PartData::RiskMatrix { risks, likelihood_label, impact_label, zone_labels, message: band } => {
            items(risks.len(), 1, 10, "risks")?;
            single(likelihood_label, 24, "likelihood_label", false)?; single(impact_label, 24, "impact_label", false)?;
            let mut ids = std::collections::BTreeSet::new();
            let mut cells = BTreeMap::<(u8, u8), usize>::new();
            for risk in risks {
                single(&risk.id, 3, "risk id", true)?; single(&risk.label, 48, "risk label", true)?; field(&risk.action, 80, "risk action", false)?;
                if !ids.insert(risk.id.as_str()) { return Err(Error::Invalid(format!("duplicate risk id {}", risk.id))); }
                if !(1..=5).contains(&risk.likelihood) || !(1..=5).contains(&risk.impact) { return Err(Error::Invalid("risk likelihood and impact must be 1-5".into())); }
                let members = cells.entry((risk.likelihood, risk.impact)).or_default();
                *members += 1;
                if *members > 4 { return Err(Error::Invalid("a risk matrix cell holds at most four risks".into())); }
            }
            labels(zone_labels, 4, 16, "zone_labels")?; message(band)?;
        }
        PartData::Pareto { items: entries, value_label, cumulative_label, threshold, message: band } => {
            items(entries.len(), 3, 12, "pareto items")?;
            single(value_label, 24, "value_label", false)?; single(cumulative_label, 24, "cumulative_label", false)?;
            for entry in entries {
                single(&entry.label, 24, "pareto label", true)?; finite(entry.value, "pareto values")?;
                if entry.value < 0.0 { return Err(Error::Invalid("pareto values must be nonnegative".into())); }
            }
            if entries.iter().map(|entry| entry.value).sum::<f64>() <= 0.0 { return Err(Error::Invalid("pareto values need a positive total".into())); }
            if threshold.is_some_and(|value| !value.is_finite() || value <= 0.0 || value >= 1.0) { return Err(Error::Invalid("pareto threshold must be between 0 and 1".into())); }
            message(band)?;
        }
        PartData::ControlChart { labels: names, values, series_label, center, upper, lower, message: band } => {
            items(values.len(), 5, 60, "control chart values")?;
            if names.len() != values.len() { return Err(Error::Invalid("control chart labels must match values".into())); }
            for name in names { single(name, 12, "control chart label", true)?; }
            single(series_label, 24, "series_label", false)?;
            for value in values.iter().chain(center.iter()).chain(upper.iter()).chain(lower.iter()) { finite(*value, "control chart values")?; }
            if let (Some(upper), Some(lower)) = (upper, lower) { if upper <= lower { return Err(Error::Invalid("control chart upper limit must exceed the lower limit".into())); } }
            message(band)?;
        }
        _ => super::process::validate(data)?,
    }
    Ok(())
}

pub(super) fn message_of(data: &PartData) -> Option<&PartMessage> {
    match data {
        PartData::KpiCards { message, .. } | PartData::BulletGraphs { message, .. } | PartData::Variance { message, .. } | PartData::HarveyMatrix { message, .. }
        | PartData::Heatmap { message, .. } | PartData::Raci { message, .. } | PartData::RiskMatrix { message, .. } | PartData::Pareto { message, .. }
        | PartData::ControlChart { message, .. } | PartData::Fishbone { message, .. } | PartData::Sankey { message, .. } | PartData::Journey { message, .. } => message.as_ref(),
        _ => None,
    }
}

pub(super) fn render(drawing: &mut Drawing, data: &PartData, frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    match data {
        PartData::KpiCards { cards, columns, .. } => kpi_cards(drawing, cards, *columns, frame, theme),
        PartData::BulletGraphs { rows, .. } => bullet_graphs(drawing, rows, frame, theme),
        PartData::Variance { rows, unit, plan_label, actual_label, variance_label, total_label, lower_is_better, .. } =>
            variance(drawing, rows, [or(plan_label, "Plan"), or(actual_label, "Actual"), or(variance_label, "Variance")], unit, total_label, *lower_is_better, frame, theme),
        PartData::HarveyMatrix { columns, rows, legend, .. } => harvey_matrix(drawing, columns, rows, legend, frame, theme),
        PartData::Heatmap { columns, rows, unit, midpoint, lower_is_better, .. } => heatmap(drawing, columns, rows, unit, *midpoint, *lower_is_better, frame, theme),
        PartData::Raci { roles, tasks, legend, .. } => raci(drawing, roles, tasks, legend, frame, theme),
        PartData::RiskMatrix { risks, likelihood_label, impact_label, zone_labels, .. } => risk_matrix(drawing, risks, [likelihood_label, impact_label], zone_labels, frame, theme),
        PartData::Pareto { items, value_label, cumulative_label, threshold, .. } => pareto(drawing, items, [or(value_label, "Value"), or(cumulative_label, "Cumulative")], threshold.unwrap_or(0.8), frame),
        PartData::ControlChart { labels, values, series_label, center, upper, lower, .. } => control_chart(drawing, labels, values, or(series_label, "Value"), [*center, *upper, *lower], frame),
        _ => super::process::render(drawing, data, frame, theme),
    }
}

fn or<'a>(value: &'a str, fallback: &'a str) -> &'a str { if value.is_empty() { fallback } else { value } }

/// Short, locale-neutral number text: at most two decimals, k/M/B above 10,000.
pub(super) fn figure(value: f64) -> String {
    if value.abs() >= 10000.0 { return display(value); }
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".into() } else { text.into() }
}

fn signed(value: f64) -> String {
    if value > 0.0 { format!("+{}", figure(value)) } else if value < 0.0 { format!("-{}", figure(-value)) } else { "0".into() }
}

pub(super) fn darken(color: &str, theme: &Theme, black: f64) -> String {
    let resolved = crate::design::resolve_color(color, Some(theme));
    let value = u32::from_str_radix(&resolved, 16).unwrap_or(0x80_80_80);
    let channel = |shift: u32| (f64::from((value >> shift) & 0xFF) * (1.0 - black)).round() as u8;
    format!("{:02X}{:02X}{:02X}", channel(16), channel(8), channel(0))
}

pub(super) fn estimate(value: &str, size: f64) -> f64 { value.chars().map(|character| if character.is_ascii() { size * 0.62 } else { size * 1.05 }).sum() }

pub(super) fn path_polygon(drawing: &mut Drawing, frame: [f64; 4], commands: Vec<PathCommand>, fill: &str, opacity: Option<f64>) {
    let [x, y, width, height] = frame;
    let points = VectorPath { commands: commands.clone() }.points();
    let id = drawing.id();
    let visual = VisualStyle { path: Some(VectorPath { commands }), opacity, ..Default::default() };
    drawing.elements.push(Element::Polygon { visual: Some(visual), id, x, y, width, height, points, fill: fill.into(), stroke: fill.into(), stroke_width: 0.0 });
}

fn kpi_cards(drawing: &mut Drawing, cards: &[KpiCard], columns: Option<usize>, frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let columns = columns.unwrap_or_else(|| kpi_columns(cards.len()));
    let rows = cards.len().div_ceil(columns);
    let card_width = (width - GAP * (columns - 1) as f64) / columns as f64;
    let card_height = ((height - GAP * (rows - 1) as f64) / rows as f64).min(KPI_HEIGHT);
    if card_width < 180.0 || card_height < 128.0 { return Err(too_small("kpi cards")); }
    let panel = mix("@dk2", theme, 0.93);
    for (index, card) in cards.iter().enumerate() {
        let x = left + (index % columns) as f64 * (card_width + GAP);
        let y = top + (index / columns) as f64 * (card_height + GAP);
        let accent = match card.status { KpiStatus::Good => "@accent1", KpiStatus::Bad => "@accent2", KpiStatus::Neutral => "@dk2" };
        shape(drawing, Fill::solid("rect", &panel), [x, y, card_width, card_height], None);
        shape(drawing, Fill::solid("rect", accent), [x, y, 6.0, card_height], None);
        let unit = if card.unit.is_empty() { String::new() } else { format!(" {}", card.unit) };
        let value_size = ((card_width - 52.0 - estimate(&unit, 18.0)) / estimate(&card.value, 1.0).max(0.6)).clamp(24.0, 44.0).floor();
        let emphasis = if card.status == KpiStatus::Neutral { "@dk2".to_string() } else { darken(accent, theme, 0.2) };
        let mut content = Rich::default();
        content.add(&card.label, 15.0, "@dk2", true, 300)
            .runs(&[(card.value.as_str(), value_size, "@dk1", true), (unit.as_str(), 18.0, "@dk2", false)], 200)
            .add(&card.delta, 16.0, &emphasis, true, 100)
            .add(&card.comparison, SMALL, "@dk2", false, 0);
        text(drawing, [x + 26.0, y + 12.0, card_width - 44.0, card_height - 24.0], content.finish(), TextAlign::Left, VerticalAlign::Middle);
    }
    Ok(Drawn { bottom: top + rows as f64 * card_height + GAP * (rows - 1) as f64, flexible: Vec::new() })
}

fn tick(drawing: &mut Drawing, value: f64, center: f64, span: [f64; 2], y: f64, last: &mut f64) {
    let width = 56.0;
    let (x, alignment) = if center - width / 2.0 < span[0] { (span[0], TextAlign::Left) } else if center + width / 2.0 > span[1] { (span[1] - width, TextAlign::Right) } else { (center - width / 2.0, TextAlign::Center) };
    if x < *last + 4.0 { return; }
    text(drawing, [x, y, width, 18.0], label(&figure(value), 12.0, "@dk2", false), alignment, VerticalAlign::Top);
    *last = x + width;
}

fn bullet_graphs(drawing: &mut Drawing, rows: &[BulletRow], frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let pitch = (height / rows.len() as f64).min(92.0);
    if pitch < 60.0 { return Err(too_small("bullet rows")); }
    let label_width = (width * 0.24).clamp(150.0, 280.0);
    let value_width = 132.0;
    let bar_left = left + label_width + 24.0;
    let bar_width = left + width - value_width - 20.0 - bar_left;
    if bar_width < 180.0 { return Err(too_small("bullet graphs")); }
    let shades = [mix("@dk2", theme, 0.62), mix("@dk2", theme, 0.76), mix("@dk2", theme, 0.89)];
    let band = (pitch - 44.0).clamp(18.0, 28.0);
    for (index, row) in rows.iter().enumerate() {
        let y = top + index as f64 * pitch;
        let band_top = y + 8.0;
        let maximum = row.ranges[row.ranges.len() - 1];
        let at = |value: f64| bar_left + value / maximum * bar_width;
        let mut start = 0.0;
        for (range, end) in row.ranges.iter().enumerate() {
            let shade = if row.lower_is_better { 2 - range } else { 3 - row.ranges.len() + range };
            shape(drawing, Fill::solid("rect", &shades[shade]), [at(start), band_top, at(*end) - at(start), band], None);
            start = *end;
        }
        let met = if row.lower_is_better { row.actual <= row.target } else { row.actual >= row.target };
        let accent = if met { "@accent1" } else { "@accent2" };
        let bar = (band / 3.0).round().max(6.0);
        if at(row.actual) - bar_left >= 0.5 { shape(drawing, Fill::solid("rect", accent), [bar_left, band_top + (band - bar) / 2.0, at(row.actual) - bar_left, bar], None); }
        shape(drawing, Fill::solid("rect", "@dk1"), [at(row.target) - 1.5, band_top - 5.0, 3.0, band + 10.0], None);
        let mut last = f64::NEG_INFINITY;
        for value in std::iter::once(0.0).chain(row.ranges.iter().copied()) { tick(drawing, value, at(value), [bar_left, bar_left + bar_width], band_top + band + 7.0, &mut last); }
        let mut name = Rich::default();
        name.add(&row.label, 16.0, "@dk1", true, 200).add(&row.note, SMALL, "@dk2", false, 0);
        text(drawing, [left, y + 2.0, label_width, pitch - 10.0], name.finish(), TextAlign::Left, VerticalAlign::Top);
        let unit = row.unit.as_str();
        let (actual, target, color) = (format!("{}{unit}", figure(row.actual)), format!(" / {}{unit}", figure(row.target)), darken(accent, theme, 0.2));
        let value = Rich::default().runs(&[(actual.as_str(), 18.0, color.as_str(), true), (target.as_str(), SMALL, "@dk2", false)], 0).finish();
        text(drawing, [left + width - value_width, band_top - 4.0, value_width, band + 8.0], value, TextAlign::Right, VerticalAlign::Middle);
    }
    Ok(Drawn { bottom: top + pitch * rows.len() as f64, flexible: Vec::new() })
}

#[allow(clippy::too_many_arguments)]
fn variance(drawing: &mut Drawing, rows: &[VarianceRow], headings: [&str; 3], unit: &str, total: &str, lower_is_better: bool, frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let mut entries: Vec<(&str, f64, f64, bool)> = rows.iter().map(|row| (row.label.as_str(), row.plan, row.actual, false)).collect();
    if !total.is_empty() { entries.push((total, rows.iter().map(|row| row.plan).sum(), rows.iter().map(|row| row.actual).sum(), true)); }
    let header = 30.0;
    let pitch = ((height - header - 8.0) / entries.len() as f64).min(44.0);
    if pitch < 26.0 { return Err(too_small("variance rows")); }
    let label_width = (width * 0.24).clamp(130.0, 260.0);
    let number = (width * 0.11).clamp(76.0, 132.0);
    let plan_x = left + label_width + 8.0;
    let actual_x = plan_x + number + 8.0;
    let bar_left = actual_x + number + 24.0;
    let bar_width = left + width - bar_left;
    if bar_width < 260.0 { return Err(too_small("variance bars")); }
    let center = bar_left + bar_width / 2.0;
    let reach = bar_width / 2.0 - 96.0;
    let largest = entries.iter().map(|entry| (entry.2 - entry.1).abs()).fold(0.0, f64::max).max(f64::MIN_POSITIVE);
    let rule = mix("@dk2", theme, 0.82);
    for (heading, x, column, alignment) in [(headings[0], plan_x, number, TextAlign::Right), (headings[1], actual_x, number, TextAlign::Right), (headings[2], bar_left, bar_width, TextAlign::Center)] {
        text(drawing, [x, top, column, header - 6.0], label(heading, SMALL, "@dk2", true), alignment, VerticalAlign::Bottom);
    }
    shape(drawing, Fill::solid("rect", &rule), [left, top + header - 1.0, width, 1.0], None);
    let body = top + header + 4.0;
    shape(drawing, Fill::solid("rect", "@dk2"), [center - 0.5, body, 1.0, pitch * entries.len() as f64], None);
    for (index, (name, plan, actual, emphasized)) in entries.into_iter().enumerate() {
        let y = body + index as f64 * pitch;
        if emphasized { shape(drawing, Fill::solid("rect", "@dk1"), [left, y - 2.0, width, 1.5], None); }
        text(drawing, [left, y, label_width, pitch], label(name, 15.0, "@dk1", emphasized), TextAlign::Left, VerticalAlign::Middle);
        text(drawing, [plan_x, y, number, pitch], label(&figure(plan), 15.0, "@dk2", emphasized), TextAlign::Right, VerticalAlign::Middle);
        text(drawing, [actual_x, y, number, pitch], label(&figure(actual), 15.0, "@dk1", emphasized), TextAlign::Right, VerticalAlign::Middle);
        let difference = actual - plan;
        let color = if difference == 0.0 { "@dk2" } else if (difference > 0.0) != lower_is_better { "@accent1" } else { "@accent2" };
        let length = difference.abs() / largest * reach;
        let thickness = (pitch * 0.5).clamp(10.0, 20.0);
        if length >= 0.5 { shape(drawing, Fill::solid("rect", color), [if difference > 0.0 { center } else { center - length }, y + (pitch - thickness) / 2.0, length, thickness], None); }
        let figure_width = 88.0;
        let (x, alignment) = if difference >= 0.0 { (center + length + 6.0, TextAlign::Left) } else { (center - length - 6.0 - figure_width, TextAlign::Right) };
        text(drawing, [x, y, figure_width, pitch], label(&format!("{}{unit}", signed(difference)), 14.0, &darken(color, theme, 0.2), true), alignment, VerticalAlign::Middle);
        if !emphasized && index + 1 < rows.len() { shape(drawing, Fill::solid("rect", &mix("@dk2", theme, 0.92)), [left, y + pitch - 0.5, label_width, 0.75], None); }
    }
    Ok(Drawn { bottom: body + pitch * (rows.len() + usize::from(!total.is_empty())) as f64, flexible: Vec::new() })
}

pub(super) fn harvey(drawing: &mut Drawing, center: [f64; 2], diameter: f64, level: u8, color: &str) {
    let bounds = [center[0] - diameter / 2.0, center[1] - diameter / 2.0, diameter, diameter];
    shape(drawing, Fill { preset: "ellipse", fill: if level >= 4 { color } else { "@lt1" }, stroke: color, stroke_width: 1.5, radius: None }, bounds, None);
    if !(1..4).contains(&level) { return; }
    let k = KAPPA / 2.0;
    let quarters = [[[0.5 + k, 0.0], [1.0, 0.5 - k], [1.0, 0.5]], [[1.0, 0.5 + k], [0.5 + k, 1.0], [0.5, 1.0]], [[0.5 - k, 1.0], [0.0, 0.5 + k], [0.0, 0.5]]];
    let mut commands = vec![PathCommand::Move { point: [0.5, 0.5] }, PathCommand::Line { point: [0.5, 0.0] }];
    commands.extend(quarters.iter().take(usize::from(level)).map(|[control1, control2, point]| PathCommand::Cubic { control1: *control1, control2: *control2, point: *point }));
    commands.push(PathCommand::Close);
    path_polygon(drawing, bounds, commands, color, None);
    shape(drawing, Fill { preset: "ellipse", fill: "none", stroke: color, stroke_width: 1.5, radius: None }, bounds, None);
}

fn harvey_matrix(drawing: &mut Drawing, columns: &[String], rows: &[HarveyRow], legend: &[String], frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let legend_height = if legend.is_empty() { 0.0 } else { 40.0 };
    let header = 48.0;
    let label_width = (width * 0.28).clamp(140.0, 320.0);
    let column_width = (width - label_width) / columns.len() as f64;
    let pitch = ((height - header - legend_height) / rows.len() as f64).min(60.0);
    if column_width < 72.0 || pitch < 34.0 { return Err(too_small("harvey ball rows")); }
    let diameter = (pitch - 14.0).min(30.0);
    for (index, column) in columns.iter().enumerate() {
        text(drawing, [left + label_width + index as f64 * column_width + 4.0, top, column_width - 8.0, header - 8.0], label(column, 14.0, "@dk1", true), TextAlign::Center, VerticalAlign::Bottom);
    }
    shape(drawing, Fill::solid("rect", "@dk2"), [left, top + header - 1.5, width, 1.5], None);
    let rule = mix("@dk2", theme, 0.86);
    for (row_index, row) in rows.iter().enumerate() {
        let y = top + header + row_index as f64 * pitch;
        text(drawing, [left, y, label_width - 12.0, pitch], label(&row.label, 15.0, "@dk1", false), TextAlign::Left, VerticalAlign::Middle);
        for (index, level) in row.levels.iter().enumerate() { harvey(drawing, [left + label_width + (index as f64 + 0.5) * column_width, y + pitch / 2.0], diameter, *level, "@dk1"); }
        if row_index + 1 < rows.len() { shape(drawing, Fill::solid("rect", &rule), [left, y + pitch - 0.5, width, 0.75], None); }
    }
    let mut bottom = top + header + pitch * rows.len() as f64;
    if !legend.is_empty() {
        let y = bottom + 16.0;
        let slot = width / 5.0;
        for (level, name) in legend.iter().enumerate() {
            let x = left + level as f64 * slot;
            harvey(drawing, [x + 9.0, y + 10.0], 18.0, level as u8, "@dk1");
            text(drawing, [x + 26.0, y, slot - 32.0, 20.0], label(name, 13.0, "@dk2", false), TextAlign::Left, VerticalAlign::Middle);
        }
        bottom = y + 20.0;
    }
    Ok(Drawn { bottom, flexible: Vec::new() })
}

#[allow(clippy::too_many_arguments)]
fn heatmap(drawing: &mut Drawing, columns: &[String], rows: &[HeatmapRow], unit: &str, midpoint: Option<f64>, lower_is_better: bool, frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let header = 34.0;
    let label_width = (width * 0.2).clamp(120.0, 240.0);
    let cell_width = (width - label_width) / columns.len() as f64;
    let pitch = ((height - header) / rows.len() as f64).min(48.0);
    if cell_width < 56.0 || pitch < 28.0 { return Err(too_small("heatmap cells")); }
    let values: Vec<f64> = rows.iter().flat_map(|row| row.values.iter().copied()).collect();
    let middle = midpoint.unwrap_or_else(|| values.iter().sum::<f64>() / values.len() as f64);
    let spread = values.iter().map(|value| (value - middle).abs()).fold(0.0, f64::max);
    for (index, column) in columns.iter().enumerate() {
        text(drawing, [left + label_width + index as f64 * cell_width, top, cell_width, header - 6.0], label(column, SMALL, "@dk2", true), TextAlign::Center, VerticalAlign::Bottom);
    }
    let neutral = mix("@dk2", theme, 0.94);
    for (row_index, row) in rows.iter().enumerate() {
        let y = top + header + row_index as f64 * pitch;
        text(drawing, [left, y, label_width - 10.0, pitch], label(&row.label, 14.0, "@dk1", true), TextAlign::Left, VerticalAlign::Middle);
        for (index, value) in row.values.iter().enumerate() {
            let signal = if spread > 0.0 { (value - middle) / spread } else { 0.0 } * if lower_is_better { -1.0 } else { 1.0 };
            let step = (signal.abs() * 4.0).round() / 4.0;
            let fill = if step == 0.0 { neutral.clone() } else { mix(if signal > 0.0 { "@accent1" } else { "@accent2" }, theme, 1.0 - 0.6 * step) };
            let style = Fill { preset: "rect", fill: &fill, stroke: "@lt1", stroke_width: 2.0, radius: None };
            shape(drawing, style, [left + label_width + index as f64 * cell_width, y, cell_width, pitch], Some((label(&format!("{}{unit}", figure(*value)), 14.0, "@dk1", step >= 0.75), TextAlign::Center, [4.0, 2.0, 4.0, 2.0])));
        }
    }
    Ok(Drawn { bottom: top + header + pitch * rows.len() as f64, flexible: Vec::new() })
}

fn raci_chip(drawing: &mut Drawing, code: &str, center: [f64; 2], theme: &Theme) {
    let (fill, color) = match code { "R" => ("@accent1".to_string(), "@lt1"), "A" | "A/R" => ("@dk1".to_string(), "@lt1"), "C" => (mix("@accent3", theme, 0.72), "@dk1"), _ => (mix("@dk2", theme, 0.86), "@dk1") };
    let width = if code.len() > 1 { 52.0 } else { 34.0 };
    shape(drawing, Fill::solid("roundRect", &fill).rounded(30000), [center[0] - width / 2.0, center[1] - 13.0, width, 26.0], Some((label(code, 14.0, color, true), TextAlign::Center, [2.0, 0.0, 2.0, 0.0])));
}

fn raci(drawing: &mut Drawing, roles: &[String], tasks: &[RaciTask], legend: &[String], frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let header = 48.0;
    let legend_height = if legend.is_empty() { 0.0 } else { 44.0 };
    let label_width = (width * 0.3).clamp(160.0, 360.0);
    let column_width = (width - label_width) / roles.len() as f64;
    let pitch = ((height - header - legend_height) / tasks.len() as f64).min(52.0);
    if column_width < 64.0 || pitch < 34.0 { return Err(too_small("RACI rows")); }
    for (index, role) in roles.iter().enumerate() {
        text(drawing, [left + label_width + index as f64 * column_width + 4.0, top, column_width - 8.0, header - 8.0], label(role, 14.0, "@dk1", true), TextAlign::Center, VerticalAlign::Bottom);
    }
    shape(drawing, Fill::solid("rect", "@dk2"), [left, top + header - 1.5, width, 1.5], None);
    let rule = mix("@dk2", theme, 0.86);
    for (row_index, task) in tasks.iter().enumerate() {
        let y = top + header + row_index as f64 * pitch;
        text(drawing, [left, y, label_width - 12.0, pitch], label(&task.label, 15.0, "@dk1", false), TextAlign::Left, VerticalAlign::Middle);
        for (index, code) in task.assignments.iter().enumerate() {
            if !code.is_empty() { raci_chip(drawing, code, [left + label_width + (index as f64 + 0.5) * column_width, y + pitch / 2.0], theme); }
        }
        if row_index + 1 < tasks.len() { shape(drawing, Fill::solid("rect", &rule), [left, y + pitch - 0.5, width, 0.75], None); }
    }
    let mut bottom = top + header + pitch * tasks.len() as f64;
    if !legend.is_empty() {
        let y = bottom + 18.0;
        let slot = width / 4.0;
        for (index, (code, name)) in ["R", "A", "C", "I"].iter().zip(legend).enumerate() {
            let x = left + index as f64 * slot;
            raci_chip(drawing, code, [x + 17.0, y + 13.0], theme);
            text(drawing, [x + 42.0, y + 1.0, slot - 48.0, 24.0], label(name, 13.0, "@dk2", false), TextAlign::Left, VerticalAlign::Middle);
        }
        bottom = y + 26.0;
    }
    Ok(Drawn { bottom, flexible: Vec::new() })
}

fn zone(score: u8) -> usize { match score { 15.. => 0, 10..=14 => 1, 5..=9 => 2, _ => 3 } }

fn zone_fill(zone: usize, theme: &Theme) -> String {
    match zone { 0 => mix("@accent2", theme, 0.35), 1 => mix("@accent2", theme, 0.65), 2 => mix("@accent4", theme, 0.62), _ => mix("@accent1", theme, 0.80) }
}

fn risk_marker(drawing: &mut Drawing, id: &str, bounds: [f64; 4]) {
    shape(drawing, Fill { preset: "ellipse", fill: "@dk1", stroke: "@lt1", stroke_width: 1.5, radius: None }, bounds, Some((label(id, 12.0, "@lt1", true), TextAlign::Center, [0.0, 0.0, 0.0, 0.0])));
}

fn risk_matrix(drawing: &mut Drawing, risks: &[Risk], axis: [&String; 2], zone_labels: &[String], frame: [f64; 4], theme: &Theme) -> Result<Drawn> {
    let [left, top, width, height] = frame;
    let (axis_width, caption, numbers) = (26.0, 20.0, 20.0);
    let cell = ((height - caption * 2.0 - numbers) / 5.0).min((width * 0.46 - axis_width) / 5.0).floor();
    if cell < 56.0 { return Err(too_small("the risk matrix")); }
    let (grid_left, grid_top, grid) = (left + axis_width, top + caption, cell * 5.0);
    if !axis[0].is_empty() { text(drawing, [left, top, grid + axis_width, caption - 2.0], label(&format!("↑ {}", axis[0]), SMALL, "@dk2", true), TextAlign::Left, VerticalAlign::Top); }
    for likelihood in 1..=5u8 {
        let y = grid_top + f64::from(5 - likelihood) * cell;
        text(drawing, [left, y, axis_width - 6.0, cell], label(&likelihood.to_string(), SMALL, "@dk2", false), TextAlign::Right, VerticalAlign::Middle);
        for impact in 1..=5u8 {
            let fill = zone_fill(zone(likelihood * impact), theme);
            shape(drawing, Fill { preset: "rect", fill: &fill, stroke: "@lt1", stroke_width: 2.0, radius: None }, [grid_left + f64::from(impact - 1) * cell, y, cell, cell], None);
        }
    }
    for impact in 1..=5u8 { text(drawing, [grid_left + f64::from(impact - 1) * cell, grid_top + grid + 2.0, cell, numbers - 2.0], label(&impact.to_string(), SMALL, "@dk2", false), TextAlign::Center, VerticalAlign::Top); }
    if !axis[1].is_empty() { text(drawing, [grid_left, grid_top + grid + numbers, grid, caption], label(&format!("{} →", axis[1]), SMALL, "@dk2", true), TextAlign::Center, VerticalAlign::Top); }
    let marker = (cell * 0.4).min(30.0).floor();
    let mut cells = BTreeMap::<(u8, u8), Vec<&Risk>>::new();
    for risk in risks { cells.entry((risk.likelihood, risk.impact)).or_default().push(risk); }
    for ((likelihood, impact), members) in cells {
        let (x, y) = (grid_left + f64::from(impact - 1) * cell, grid_top + f64::from(5 - likelihood) * cell);
        let offsets: &[[f64; 2]] = match members.len() { 1 => &[[0.5, 0.5]], 2 => &[[0.27, 0.5], [0.73, 0.5]], _ => &[[0.27, 0.27], [0.73, 0.27], [0.27, 0.73], [0.73, 0.73]] };
        for (risk, offset) in members.iter().zip(offsets) { risk_marker(drawing, &risk.id, [x + offset[0] * cell - marker / 2.0, y + offset[1] * cell - marker / 2.0, marker, marker]); }
    }
    let list_left = grid_left + grid + 32.0;
    let list_width = left + width - list_left;
    let pitch = (height / risks.len() as f64).min(64.0);
    if list_width < 260.0 || pitch < 40.0 { return Err(too_small("the risk list")); }
    let mut ordered: Vec<&Risk> = risks.iter().collect();
    ordered.sort_by_key(|risk| std::cmp::Reverse(score(risk)));
    for (index, risk) in ordered.into_iter().enumerate() {
        let y = top + index as f64 * pitch;
        risk_marker(drawing, &risk.id, [list_left, y + 4.0, 26.0, 26.0]);
        let zone = zone(score(risk));
        let chip = match zone_labels.get(zone) { Some(name) => format!("{}×{} {name}", risk.likelihood, risk.impact), None => format!("{}×{}", risk.likelihood, risk.impact) };
        let chip_width = (estimate(&chip, 12.0) + 20.0).ceil();
        let fill = zone_fill(zone, theme);
        shape(drawing, Fill::solid("roundRect", &fill).rounded(30000), [list_left + list_width - chip_width, y + 4.0, chip_width, 26.0], Some((label(&chip, 12.0, "@dk1", true), TextAlign::Center, [4.0, 0.0, 4.0, 0.0])));
        let mut content = Rich::default();
        content.add(&risk.label, 15.0, "@dk1", true, 100).add(&risk.action, SMALL, "@dk2", false, 0);
        text(drawing, [list_left + 38.0, y + 2.0, list_width - 48.0 - chip_width, pitch - 6.0], content.finish(), TextAlign::Left, VerticalAlign::Top);
    }
    Ok(Drawn { bottom: (grid_top + grid + numbers + caption).max(top + pitch * risks.len() as f64), flexible: Vec::new() })
}

fn series(name: &str, values: Vec<f64>, color: &str, kind: Option<ChartKind>, secondary: bool) -> ChartSeries {
    ChartSeries { name: name.into(), values, color: color.into(), kind, axis: secondary.then_some(ChartAxis::Secondary), bubble_sizes: None, trendline: None, error_bars: None }
}

fn chart(drawing: &mut Drawing, kind: ChartKind, categories: Vec<String>, series: Vec<ChartSeries>, options: ChartOptions, frame: [f64; 4]) {
    let [x, y, width, height] = frame;
    let id = drawing.id();
    drawing.elements.push(Element::Chart { id, x, y, width, height, kind, categories, series, options });
}

fn pareto(drawing: &mut Drawing, entries: &[ParetoItem], names: [&str; 2], threshold: f64, frame: [f64; 4]) -> Result<Drawn> {
    if frame[2] < 360.0 || frame[3] < 200.0 { return Err(too_small("the pareto chart")); }
    let mut sorted: Vec<&ParetoItem> = entries.iter().collect();
    sorted.sort_by(|left, right| right.value.total_cmp(&left.value));
    let total: f64 = sorted.iter().map(|entry| entry.value).sum();
    let mut running = 0.0;
    let cumulative = sorted.iter().map(|entry| { running += entry.value; (running / total * 10000.0).round() / 10000.0 }).collect();
    let values = vec![
        series(names[0], sorted.iter().map(|entry| entry.value).collect(), "@accent1", Some(ChartKind::Column), false),
        series(names[1], cumulative, "@accent2", Some(ChartKind::Line), true),
        series(&format!("{}%", figure(threshold * 100.0)), vec![threshold; sorted.len()], "@dk2", Some(ChartKind::Line), true),
    ];
    let options = ChartOptions {
        primary_axis: AxisOptions { min: Some(0.0), ..Default::default() },
        secondary_axis: AxisOptions { min: Some(0.0), max: Some(1.0), major_unit: Some(0.2), number_format: Some("0%".into()), ..Default::default() },
        legend: Some(LegendPosition::Bottom),
        ..Default::default()
    };
    chart(drawing, ChartKind::Combo, sorted.iter().map(|entry| entry.label.clone()).collect(), values, options, frame);
    Ok(Drawn { bottom: frame[1] + frame[3], flexible: Vec::new() })
}

/// Individuals chart: omitted limits use the moving-range estimate sigma = mean(|x[i] - x[i-1]|) / 1.128.
pub(super) fn control_limits(values: &[f64], center: Option<f64>, upper: Option<f64>, lower: Option<f64>) -> [f64; 3] {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let moving = values.windows(2).map(|pair| (pair[1] - pair[0]).abs()).sum::<f64>() / (values.len() - 1) as f64;
    let center = center.unwrap_or(mean);
    let sigma = moving / 1.128;
    [center, upper.unwrap_or(center + 3.0 * sigma), lower.unwrap_or(center - 3.0 * sigma)]
}

fn control_chart(drawing: &mut Drawing, names: &[String], values: &[f64], name: &str, limits: [Option<f64>; 3], frame: [f64; 4]) -> Result<Drawn> {
    if frame[2] < 360.0 || frame[3] < 200.0 { return Err(too_small("the control chart")); }
    let [center, upper, lower] = control_limits(values, limits[0], limits[1], limits[2]);
    let round = |value: f64| (value * 10000.0).round() / 10000.0;
    let count = values.len();
    let lines = vec![
        series(name, values.to_vec(), "@accent1", None, false),
        series(&format!("UCL {}", figure(upper)), vec![round(upper); count], "@accent2", None, false),
        series(&format!("CL {}", figure(center)), vec![round(center); count], "@dk2", None, false),
        series(&format!("LCL {}", figure(lower)), vec![round(lower); count], "@accent2", None, false),
    ];
    // Scale the value axis to the data and limits instead of zero, so variation stays visible.
    let low = values.iter().copied().fold(lower.min(upper), f64::min);
    let high = values.iter().copied().fold(upper.max(lower), f64::max);
    let raw = ((high - low) / 5.0).max(1e-9);
    let magnitude = 10f64.powf(raw.log10().floor());
    let step = magnitude * [1.0, 2.0, 5.0, 10.0].into_iter().find(|factor| raw / magnitude <= *factor).unwrap_or(10.0);
    let snap = |value: f64| (value * 1e6).round() / 1e6;
    let axis = AxisOptions { min: Some(snap(((low - step / 2.0) / step).floor() * step)), max: Some(snap(((high + step / 2.0) / step).ceil() * step)), major_unit: Some(snap(step)), ..Default::default() };
    chart(drawing, ChartKind::Line, names.to_vec(), lines, ChartOptions { primary_axis: axis, legend: Some(LegendPosition::Bottom), ..Default::default() }, frame);
    Ok(Drawn { bottom: frame[1] + frame[3], flexible: Vec::new() })
}
