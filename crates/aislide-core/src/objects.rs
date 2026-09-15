use crate::{model::{ChartKind, ChartSeries, Element, TextFormat, validate_elements}, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SHAPES: &[(&str, &str)] = &[
    ("rect", "Rectangle"), ("roundRect", "Rounded rectangle"), ("ellipse", "Ellipse"),
    ("triangle", "Triangle"), ("rtTriangle", "Right triangle"), ("diamond", "Diamond"),
    ("parallelogram", "Parallelogram"), ("trapezoid", "Trapezoid"), ("pentagon", "Pentagon"),
    ("hexagon", "Hexagon"), ("heptagon", "Heptagon"), ("octagon", "Octagon"), ("decagon", "Decagon"),
    ("star4", "Four-point star"), ("star5", "Five-point star"), ("star6", "Six-point star"),
    ("star8", "Eight-point star"), ("star12", "Twelve-point star"), ("plus", "Plus"),
    ("heart", "Heart"), ("chevron", "Chevron"), ("rightArrow", "Right arrow"), ("leftArrow", "Left arrow"),
    ("upArrow", "Up arrow"), ("downArrow", "Down arrow"), ("leftRightArrow", "Left-right arrow"),
    ("upDownArrow", "Up-down arrow"), ("homePlate", "Pentagon arrow"),
    ("flowChartProcess", "Process"), ("flowChartDecision", "Decision"),
    ("flowChartTerminator", "Terminator"), ("flowChartInputOutput", "Input / output"),
    ("flowChartDocument", "Document"), ("flowChartPredefinedProcess", "Predefined process"),
    ("flowChartInternalStorage", "Internal storage"), ("flowChartConnector", "Flowchart connector"),
    ("flowChartOfflineStorage", "Offline storage"), ("wedgeRectCallout", "Rectangle callout"),
    ("wedgeRoundRectCallout", "Rounded callout"), ("wedgeEllipseCallout", "Oval callout"),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind { Text, Shape, Table, Chart, Line, Arrow }

pub fn catalog() -> serde_json::Value {
    serde_json::json!({
        "shapes": SHAPES.iter().map(|(id, name)| serde_json::json!({"id":id,"name":name})).collect::<Vec<_>>(),
        "charts": ["column", "bar", "line", "pie", "doughnut", "area", "scatter", "stacked_column", "stacked_bar"],
        "table": {"max_rows":12,"max_columns":8}
    })
}

pub fn create(id: String, kind: ObjectKind, preset: Option<String>, rows: Option<usize>, columns: Option<usize>) -> Result<Element> {
    let element = match kind {
        ObjectKind::Text => Element::Text { id, x: 80.0, y: 160.0, width: 640.0, height: 100.0, text: "Text".into(), font_size: 28.0, color: "@dk1".into(), bold: false, format: TextFormat::default() },
        ObjectKind::Shape => Element::Shape { id, x: 160.0, y: 180.0, width: 280.0, height: 160.0, preset: preset.unwrap_or_else(|| "rect".into()), fill: "@lt2".into(), stroke: "@accent1".into(), stroke_width: 2.0, rotation: 0.0, text: String::new(), font_size: 24.0, color: "@dk1".into(), bold: false, format: TextFormat::default() },
        ObjectKind::Table => {
            let rows = rows.unwrap_or(4); let columns = columns.unwrap_or(3);
            if !(1..=12).contains(&rows) || !(1..=8).contains(&columns) { return Err(Error::Invalid("table requires 1-12 rows and 1-8 columns".into())); }
            Element::Table { id, x: 100.0, y: 200.0, width: 1080.0, height: (rows as f64 * 44.0).min(440.0), rows: (0..rows).map(|row| (0..columns).map(|column| if row == 0 { format!("Column {}", column + 1) } else { String::new() }).collect()).collect(), font_size: if rows > 6 { 16.0 } else { 22.0 } }
        }
        ObjectKind::Chart => {
            let kind: ChartKind = serde_json::from_value(serde_json::Value::String(preset.unwrap_or_else(|| "column".into())))?;
            Element::Chart { id, x: 100.0, y: 170.0, width: 1040.0, height: 440.0, kind,
                categories: if kind == ChartKind::Scatter { vec!["1", "2", "3", "4"] } else { vec!["A", "B", "C", "D"] }.into_iter().map(str::to_owned).collect(),
                series: vec![ChartSeries { name: "Synthetic example".into(), values: vec![10.0, 15.0, 12.0, 18.0], color: "@accent1".into() }] }
        }
        ObjectKind::Line | ObjectKind::Arrow => Element::Connector { id, x: 160.0, y: 340.0, width: 400.0, height: 1.0, color: "@accent1".into(), stroke_width: 3.0, arrow: matches!(kind, ObjectKind::Arrow), flip_v: false, start: None, end: None, routing: None },
    };
    validate_elements(std::slice::from_ref(&element), (1280.0, 720.0), 0, &mut BTreeSet::new(), &mut 0, &mut 0)?;
    Ok(element)
}