use crate::{model::{Deck, Slide, Element, Issue, ChartKind, ChartSeries, ChartOptions, valid_text, validate_deck, validate_rows}, Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportInput {
    #[schemars(length(min = 1, max = 120))]
    pub title: String,
    #[schemars(length(max = 200))]
    pub subtitle: String,
    #[schemars(length(max = 80))]
    pub period: String,
    #[schemars(length(max = 1200))]
    pub source: String,
    #[schemars(length(min = 1, max = 128))]
    pub sections: Vec<Section>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Section {
    #[schemars(length(max = 100))]
    pub title: String,
    pub layout: Layout,
    #[serde(default)]
    #[schemars(length(max = 4))]
    pub body: Vec<String>,
    #[serde(default)]
    pub metrics: Vec<Metric>,
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart: Option<ReportChart>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportChart {
    pub kind: ChartKind, pub categories: Vec<String>, pub series: Vec<ChartSeries>,
    #[serde(default, skip_serializing_if = "ChartOptions::is_default")]
    pub options: ChartOptions,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Layout { Cover, Metrics, Table, Columns, Statement, Chart, Process }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Metric { pub label: String, pub value: String }

#[derive(Debug, Serialize)]
pub struct CompiledReport { pub deck: Deck, pub issues: Vec<Issue> }

const INK: &str = "202525";
const MUTED: &str = "586563";
const TEAL: &str = "087F73";
const CORAL: &str = "CF5847";

fn text(id: &str, bounds: [f64; 4], value: &str, size: f64, color: &str, bold: bool) -> Element {
    Element::Text { visual: None, id: id.into(), x: bounds[0], y: bounds[1], width: bounds[2], height: bounds[3], text: value.into(), font_size: size, color: color.into(), bold, format: Default::default() }
}

fn rect(id: &str, bounds: [f64; 4], fill: &str) -> Element {
    Element::Rect { visual: None, id: id.into(), x: bounds[0], y: bounds[1], width: bounds[2], height: bounds[3], fill: fill.into() }
}

pub fn compile_report(report: &ReportInput) -> Result<CompiledReport> {
    valid_text(&report.title, 120)?;
    valid_text(&report.subtitle, 200)?;
    valid_text(&report.period, 80)?;
    valid_text(&report.source, 1200)?;
    if report.title.trim().is_empty() || report.sections.is_empty() || report.sections.len() > crate::limits::LARGE.slides {
        return Err(Error::Invalid("a title and 1-128 sections are required".into()));
    }
    let mut slides = Vec::new();
    for (index, section) in report.sections.iter().enumerate() {
        valid_text(&section.title, 100)?;
        if section.body.len() > 4 || section.metrics.len() > 4 { return Err(Error::Limit("maximum four body blocks or metrics".into())); }
        for body in &section.body { valid_text(body, 240)?; }
        for metric in &section.metrics { valid_text(&metric.label, 48)?; valid_text(&metric.value, 20)?; }
        if !section.rows.is_empty() { validate_rows(&section.rows)?; }
        let mut elements = vec![
            rect("accent", [64.0, 52.0, 40.0, 5.0], TEAL),
            text("period", [122.0, 42.0, 1050.0, 30.0], &report.period, 16.0, MUTED, false),
            text("title", [64.0, 105.0, 1152.0, 104.0], &section.title, 42.0, INK, true),
            rect("footer-rule", [64.0, 657.0, 1152.0, 1.0], "DDE4E1"),
            text("footer", [64.0, 672.0, 1080.0, 25.0], &report.title, 13.0, MUTED, false),
            text("page", [1150.0, 672.0, 66.0, 25.0], &format!("{:02}", index + 1), 14.0, MUTED, true),
        ];
        match section.layout {
            Layout::Cover => {
                elements = vec![
                    rect("field", [0.0, 0.0, 1280.0, 720.0], "EAF3EF"),
                    rect("vertical-rule", [64.0, 76.0, 6.0, 560.0], TEAL),
                    text("period", [100.0, 78.0, 1060.0, 36.0], &report.period, 20.0, TEAL, true),
                    text("title", [100.0, 178.0, 1060.0, 214.0], &section.title, 62.0, INK, true),
                    text("subtitle", [100.0, 418.0, 1020.0, 130.0], &report.subtitle, 26.0, MUTED, false),
                    rect("coral-rule", [100.0, 584.0, 60.0, 5.0], CORAL),
                    text("label", [182.0, 574.0, 970.0, 60.0], section.body.first().map_or("", String::as_str), 18.0, MUTED, false),
                ];
            }
            Layout::Metrics => {
                if section.metrics.is_empty() { return Err(Error::Invalid("metrics layout requires metrics".into())); }
                let column = 1152.0 / section.metrics.len() as f64;
                for (metric_index, metric) in section.metrics.iter().enumerate() {
                    let left = 64.0 + metric_index as f64 * column;
                    elements.push(rect(&format!("metric-rule-{metric_index}"), [left, 258.0, column - 32.0, 3.0], TEAL));
                    let size = crate::layout::fit_metric_size(&metric.value, column - 32.0, 106.0)?;
                    elements.push(text(&format!("metric-value-{metric_index}"), [left, 288.0, column - 32.0, 106.0], &metric.value, size, INK, true));
                    elements.push(text(&format!("metric-label-{metric_index}"), [left, 412.0, column - 32.0, 94.0], &metric.label, 23.0, MUTED, false));
                }
                elements.push(text("body", [64.0, 548.0, 1152.0, 87.0], &section.body.join("\n"), 20.0, MUTED, false));
            }
            Layout::Table => {
                validate_rows(&section.rows)?;
                elements.push(Element::Table { id: "data-table".into(), x: 64.0, y: 230.0, width: 1152.0, height: 340.0, rows: section.rows.clone(), font_size: if section.rows.len() > 6 { 12.0 } else { 20.0 }, format: Default::default() });
                elements.push(text("body", [64.0, 590.0, 1152.0, 57.0], &section.body.join("\n"), 16.0, MUTED, false));
            }
            Layout::Chart => {
                let chart = section.chart.as_ref().ok_or_else(|| Error::Invalid("chart layout requires chart data".into()))?;
                crate::model::chart_format::validate(chart.kind, &chart.categories, &chart.series, &chart.options)?;
                elements.push(Element::Chart { id: "data-chart".into(), x: 64.0, y: 225.0, width: 1152.0, height: 352.0, kind: chart.kind, categories: chart.categories.clone(), series: chart.series.clone(), options: chart.options.clone() });
                elements.push(text("body", [64.0, 590.0, 1152.0, 57.0], &section.body.join("\n"), 16.0, MUTED, false));
            }
            Layout::Process => elements.push(crate::graphics::create_diagram("process", &section.body)?),
            Layout::Columns => {
                if section.body.is_empty() { return Err(Error::Invalid("columns layout requires body blocks".into())); }
                let column = 1152.0 / section.body.len() as f64;
                for (body_index, body) in section.body.iter().enumerate() {
                    let left = 64.0 + body_index as f64 * column;
                    elements.push(text(&format!("number-{body_index}"), [left, 246.0, column - 30.0, 72.0], &format!("{:02}", body_index + 1), 42.0, if body_index % 2 == 0 { TEAL } else { CORAL }, true));
                    elements.push(rect(&format!("rule-{body_index}"), [left, 337.0, column - 32.0, 2.0], "DDE4E1"));
                    elements.push(text(&format!("body-{body_index}"), [left, 362.0, column - 40.0, 250.0], body, 25.0, INK, false));
                }
            }
            Layout::Statement => {
                elements.push(rect("quote-accent", [64.0, 246.0, 6.0, 305.0], CORAL));
                elements.push(text("body", [104.0, 244.0, 1050.0, 358.0], &section.body.join("\n\n"), 32.0, INK, false));
            }
        }
        slides.push(Slide { id: format!("slide-{}", index + 1), title: section.title.clone(), background: "FFFFFF".into(), elements, notes: format!("{}\n\nSource: {}", section.body.join("\n"), report.source), notes_paragraphs: Vec::new(), layout_id: None, inherit_background: false, hide_master_graphics: false, native_source_id: None, review: None });
    }
    let deck = Deck { version: 1, title: report.title.clone(), width: 1280, height: 720, slides, design: None, embedded_fonts: Vec::new(), auxiliary_design: None };
    validate_deck(&deck)?;
    Ok(CompiledReport { deck, issues: vec![Issue { severity: "warning".into(), code: "FONT_PARITY_UNVERIFIED".into(), message: "PowerPoint text wrapping depends on installed fonts. Native Office parity has not been established.".into() }] })
}

pub fn sample_report() -> ReportInput {
    let make = |title: &str, layout, body: &[&str]| Section { title: title.into(), layout, body: body.iter().map(|text| (*text).into()).collect(), metrics: Vec::new(), rows: Vec::new(), chart: None };
    let mut sections = vec![
        make("Quarterly performance\n\u{56db}\u{534a}\u{671f}\u{30ec}\u{30dd}\u{30fc}\u{30c8}", Layout::Cover, &["SYNTHETIC DATA / DESIGN STUDY"]),
        make("The quarter at a glance", Layout::Columns, &["Growth\nRevenue increased in this synthetic scenario.", "Efficiency\nOperating costs stayed within the sample budget.", "Focus\nRetention is the next experiment, not a proven conclusion."]),
        make("Three signals. One clear direction.", Layout::Metrics, &["Illustrative values only. Not investment or business guidance."]),
        make("Revenue by region", Layout::Table, &["Source: built-in synthetic dataset. USD millions."]),
        make("What changed this quarter", Layout::Columns, &["Demand\nSample enterprise accounts expanded.", "Mix\nThe fictional product mix shifted toward services.", "Capacity\nHeadcount is held constant in this scenario."]),
        make("A healthier operating model", Layout::Metrics, &["All ratios are illustrative inputs, not audited results."]),
        make("Channel performance", Layout::Table, &["Illustrative attribution. These rows are editable PowerPoint table cells."]),
        make("From evidence to action", Layout::Columns, &["Collect\nRecord the source and period.", "Validate\nReconcile totals before drawing conclusions.", "Communicate\nState uncertainty alongside the result."]),
        make("Risks worth watching", Layout::Table, &["Fictional risks for demonstrating the report layout."]),
        make("The decision, in one sentence", Layout::Statement, &["Treat retention as a testable priority.\nMeasure the outcome before scaling the program."]),
        make("Next quarter: three commitments", Layout::Columns, &["Baseline\nConfirm a reproducible metric definition.", "Experiment\nRun a small, measurable retention test.", "Review\nCompare outcomes with the original baseline."]),
        make("Sources & methodology", Layout::Statement, &["Every number in this deck is synthetic.", "Generated locally by AISlide. No external source or AI model was called."]),
    ];
    sections[2].metrics = [("Sample revenue", "$12.4M"), ("Sample growth", "+18%"), ("Sample retention", "94%")].into_iter().map(|(label, value)| Metric { label: label.into(), value: value.into() }).collect();
    sections[5].metrics = [("Gross margin", "72%"), ("Operating ratio", "0.64"), ("Payback period", "11 mo")].into_iter().map(|(label, value)| Metric { label: label.into(), value: value.into() }).collect();
    for (index, rows) in [
        (3, vec![vec!["Region", "Revenue", "Share"], vec!["Americas", "6.2", "50%"], vec!["Europe", "3.7", "30%"], vec!["Asia Pacific", "2.5", "20%"], vec!["Total", "12.4", "100%"]]),
        (6, vec![vec!["Channel", "Accounts", "Conversion"], vec!["Direct", "240", "12%"], vec!["Partner", "180", "9%"], vec!["Self-service", "580", "5%"]]),
        (8, vec![vec!["Risk", "Signal", "Response"], vec!["Concentration", "Few large accounts", "Diversify pipeline"], vec!["Capacity", "Long lead times", "Measure bottlenecks"], vec!["Data quality", "Missing attribution", "Audit inputs"]]),
    ] {
        sections[index].rows = rows.into_iter().map(|row| row.into_iter().map(String::from).collect()).collect();
    }
    ReportInput { title: "Quarterly performance".into(), subtitle: "An evidence-led view of growth, efficiency and the next quarter.".into(), period: "Q3 2026 / DEMO REPORT".into(), source: "AISlide built-in synthetic demonstration data. No external factual claims.".into(), sections }
}