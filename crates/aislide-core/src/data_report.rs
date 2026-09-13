use crate::{model::{ChartKind, ChartSeries, valid_text}, report::{CompiledReport, Layout, Metric, ReportChart, ReportInput, Section, compile_report}, sources::{SourceDocument, validate_source}, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataMapping {
    pub title: String, pub period: String, pub table_index: usize, pub category_column: usize,
    pub value_columns: Vec<usize>, pub row_start: usize, pub row_count: usize, pub chart_kind: ChartKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBinding {
    pub slide_id: String, pub element_id: String, pub field: String,
    pub source_id: String, pub source_sha256: String, pub locator: String,
    pub value: Value, pub raw_value: Value, pub transform: String,
    #[serde(default)]
    pub stale: bool,
}

#[derive(Serialize)]
pub struct DataReport { pub report: ReportInput, pub compiled: CompiledReport, pub bindings: Vec<SourceBinding> }

fn section(title: &str, layout: Layout, body: Vec<String>) -> Section {
    Section { title: title.into(), layout, body, metrics: Vec::new(), rows: Vec::new(), chart: None }
}

pub(crate) fn display(value: &Value) -> String { match value { Value::String(text) => text.clone(), Value::Null => String::new(), value => value.to_string() } }

pub(crate) fn number(value: &Value) -> Result<f64> {
    let parsed = match value { Value::Number(value) => value.as_f64(), Value::String(value) => value.trim().parse::<f64>().ok(), _ => None };
    parsed.filter(|number| number.is_finite() && number.abs() <= 1e15).ok_or_else(|| Error::Invalid("selected numeric cell is missing, nonnumeric, a formula, or outside +/-1e15; no imputation was applied".into()))
}

pub fn data_report(source: &SourceDocument, mapping: &DataMapping) -> Result<DataReport> {
    validate_source(source)?; valid_text(&mapping.title, 120)?; valid_text(&mapping.period, 80)?;
    let table = source.tables.get(mapping.table_index).ok_or_else(|| Error::Invalid("selected source table does not exist".into()))?;
    if mapping.category_column >= table.columns.len() || mapping.value_columns.is_empty() || mapping.value_columns.len() > 6 || !(1..=32).contains(&mapping.row_count) { return Err(Error::Invalid("select a category, 1-6 numeric columns, and 1-32 rows".into())); }
    let mut columns = BTreeSet::new();
    if mapping.value_columns.iter().any(|column| *column >= table.columns.len() || !columns.insert(*column)) { return Err(Error::Invalid("numeric columns must exist and be distinct".into())); }
    let end = mapping.row_start.checked_add(mapping.row_count).filter(|end| *end <= table.rows.len()).ok_or_else(|| Error::Invalid("selected row range is outside the source".into()))?;
    let selected = &table.rows[mapping.row_start..end];
    let categories: Vec<String> = selected.iter().map(|row| display(&row[mapping.category_column])).collect();
    for category in &categories { valid_text(category, 80)?; if category.is_empty() { return Err(Error::Invalid("category cells must not be empty".into())); } }
    let palette = ["087F73", "CF5847", "416CA5", "C68B19", "677C54", "9A506B"];
    let series = mapping.value_columns.iter().enumerate().map(|(index, column)| Ok(ChartSeries { name: table.columns[*column].clone(), values: selected.iter().map(|row| number(&row[*column])).collect::<Result<Vec<_>>>()?, color: palette[index].into() })).collect::<Result<Vec<_>>>()?;
    let mut sections = vec![
        section(&mapping.title, Layout::Cover, vec!["SOURCE-BOUND DATA REPORT".into()]),
        section("Source and selection", Layout::Statement, vec![format!("File: {}", source.name), format!("Table: {}", table.name), format!("Rows {}-{}, selected explicitly. Source data is not independently verified.", mapping.row_start + 1, end)]),
        section("Selected data scope", Layout::Metrics, vec!["Counts describe the selected input rectangle, not business outcomes.".into()]),
    ];
    sections[2].metrics = [Metric { label: "Rows selected".into(), value: mapping.row_count.to_string() }, Metric { label: "Numeric series".into(), value: series.len().to_string() }, Metric { label: "Provided values".into(), value: (mapping.row_count * series.len()).to_string() }].into();
    let mut bindings = Vec::new();
    for (position, (title, kind)) in [("Provided values by category", mapping.chart_kind), ("Ordered values", ChartKind::Line), ("Category comparison", ChartKind::Bar)].into_iter().enumerate() {
        let mut slide = section(title, Layout::Chart, vec![format!("{} / {}. Same source values; no causal inference.", source.name, table.name)]);
        slide.chart = Some(ReportChart { kind, categories: categories.clone(), series: series.clone() });
        for (row_index, row) in selected.iter().enumerate() {
            for (series_index, column) in mapping.value_columns.iter().enumerate() {
                bindings.push(SourceBinding { slide_id: format!("slide-{}", position + 4), element_id: "data-chart".into(), field: format!("/series/{series_index}/values/{row_index}"), source_id: source.id.clone(), source_sha256: source.sha256.clone(), locator: table.locators[mapping.row_start + row_index][*column].clone(), value: json!(series[series_index].values[row_index]), raw_value: row[*column].clone(), transform: "strict_numeric".into(), stale: false });
            }
            bindings.push(SourceBinding { slide_id: format!("slide-{}", position + 4), element_id: "data-chart".into(), field: format!("/categories/{row_index}"), source_id: source.id.clone(), source_sha256: source.sha256.clone(), locator: table.locators[mapping.row_start + row_index][mapping.category_column].clone(), value: json!(categories[row_index]), raw_value: row[mapping.category_column].clone(), transform: "display_scalar".into(), stale: false });
        }
        sections.push(slide);
    }
    let selected_columns: Vec<usize> = std::iter::once(mapping.category_column).chain(mapping.value_columns.iter().copied()).collect();
    for page in 0..3 {
        let start = page * 11;
        if start < selected.len() {
            let mut slide = section(&format!("Source rows / {}", page + 1), Layout::Table, vec!["Values retain source order; spreadsheet formulas are not recalculated.".into()]);
            slide.rows.push(selected_columns.iter().map(|column| table.columns[*column].clone()).collect());
            slide.rows.extend(selected.iter().skip(start).take(11).map(|row| selected_columns.iter().map(|column| display(&row[*column])).collect()));
            for (row_index, row) in selected.iter().enumerate().skip(start).take(11) {
                for (output_column, column) in selected_columns.iter().enumerate() {
                    bindings.push(SourceBinding { slide_id: format!("slide-{}", sections.len() + 1), element_id: "data-table".into(), field: format!("/rows/{}/{}", row_index - start + 1, output_column),
                        source_id: source.id.clone(), source_sha256: source.sha256.clone(), locator: table.locators[mapping.row_start + row_index][*column].clone(), value: json!(display(&row[*column])), raw_value: row[*column].clone(), transform: "display_scalar".into(), stale: false });
                }
            }
            sections.push(slide);
        } else {
            sections.push(section(if page == 1 { "Reading the evidence" } else { "Validation boundaries" }, Layout::Statement, vec!["Source values and exact cell locations are retained. Missing values are not replaced with zero.".into(), "Data parsing establishes structure, not factual correctness or statistical significance.".into()]));
        }
    }
    sections.push(section("From source to presentation", Layout::Process, vec!["Read source".into(), "Map fields".into(), "Validate values".into(), "Export a copy".into()]));
    sections.push(section("Interpretation limits", Layout::Statement, vec!["These slides show provided input values. Patterns, causes and recommendations require domain review.".into(), "A connecting line follows source row order; it does not establish a time series.".into()]));
    sections.push(section("Source identity", Layout::Statement, vec![source.name.clone(), format!("SHA-256:\n{}", source.sha256), "Source-byte and extracted-content hashes support integrity checks, not authentication or fact verification.".into()]));
    let report = ReportInput { title: mapping.title.clone(), subtitle: source.name.clone(), period: mapping.period.clone(), source: format!("{} / SHA-256 {} / input data not independently verified", source.name, source.sha256), sections };
    let mut compiled = compile_report(&report)?;
    for slide in &mut compiled.deck.slides {
        slide.notes.push_str(&format!("\n\nSource ID: {}\nSelection: table {}, rows {}-{}, category column {}, numeric columns {:?}", source.id, table.name, mapping.row_start + 1, end, mapping.category_column + 1, mapping.value_columns.iter().map(|column| column + 1).collect::<Vec<_>>()));
        if let Some(attribution) = &source.attribution { slide.notes.push_str(&format!("\nCitation: {}\nURL (not fetched by the core): {}\nLicense: {}\nTransformation: {}", attribution.citation, attribution.url, attribution.license, attribution.transformation)); }
    }
    crate::model::validate_deck(&compiled.deck)?;
    Ok(DataReport { report, compiled, bindings })
}