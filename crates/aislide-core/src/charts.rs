use crate::{model::{ChartKind, ChartSeries}, package::Package, pptx::{color, empty, xml}, Result};
use std::collections::BTreeMap;
use xmlwriter::XmlWriter;

pub(crate) const CHART_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const SHEET_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";

fn text(writer: &mut XmlWriter, name: &str, value: &str) {
    writer.start_element(name);
    writer.write_text(&quick_xml::escape::escape(value));
    writer.end_element();
}

fn string_reference(writer: &mut XmlWriter, formula: &str, values: &[String]) {
    writer.start_element("c:strRef");
    text(writer, "c:f", formula);
    writer.start_element("c:strCache");
    empty(writer, "c:ptCount", &[("val", &values.len().to_string())]);
    for (index, value) in values.iter().enumerate() {
        writer.start_element("c:pt"); writer.write_attribute("idx", &index.to_string());
        text(writer, "c:v", value); writer.end_element();
    }
    writer.end_element(); writer.end_element();
}

pub(crate) fn chart(kind: ChartKind, categories: &[String], series: &[ChartSeries]) -> Vec<u8> {
    xml("c:chartSpace", |writer| {
        writer.write_attribute("xmlns:c", CHART_NS);
        empty(writer, "c:lang", &[("val", "ja-JP")]);
        writer.start_element("c:chart");
        empty(writer, "c:autoTitleDeleted", &[("val", "1")]);
        writer.start_element("c:plotArea"); empty(writer, "c:layout", &[]);
        writer.start_element(match kind { ChartKind::Line => "c:lineChart", ChartKind::Area => "c:areaChart", ChartKind::Pie => "c:pieChart", ChartKind::Doughnut => "c:doughnutChart", ChartKind::Scatter => "c:scatterChart", _ => "c:barChart" });
        if kind.is_bar() { empty(writer, "c:barDir", &[("val", if kind.is_horizontal() { "bar" } else { "col" })]); }
        if !kind.is_polar() && kind != ChartKind::Scatter { empty(writer, "c:grouping", &[("val", if kind==ChartKind::PercentStackedColumn { "percentStacked" } else if matches!(kind, ChartKind::StackedColumn | ChartKind::StackedBar) { "stacked" } else if kind.is_bar() { "clustered" } else { "standard" })]); }
        if kind == ChartKind::Scatter { empty(writer, "c:scatterStyle", &[("val", "marker")]); }
        empty(writer, "c:varyColors", &[("val", if kind.is_polar() { "1" } else { "0" })]);
        for (index, entry) in series.iter().enumerate() {
            let column = char::from(b'B' + index as u8);
            writer.start_element("c:ser");
            empty(writer, "c:idx", &[("val", &index.to_string())]);
            empty(writer, "c:order", &[("val", &index.to_string())]);
            writer.start_element("c:tx"); string_reference(writer, &format!("Sheet1!${column}$1"), std::slice::from_ref(&entry.name)); writer.end_element();
            writer.start_element("c:spPr");
            if matches!(kind, ChartKind::Line | ChartKind::Scatter) {
                writer.start_element("a:ln"); writer.write_attribute("w", "28575"); color(writer, &entry.color); writer.end_element();
            } else { color(writer, &entry.color); }
            writer.end_element();
            if matches!(kind, ChartKind::Line | ChartKind::Scatter) { writer.start_element("c:marker"); empty(writer, "c:symbol", &[("val", "circle")]); empty(writer, "c:size", &[("val", "5")]); writer.end_element(); }
            else if kind.is_bar() { empty(writer, "c:invertIfNegative", &[("val", "0")]); }
            if kind.is_polar() {
                for position in 0..categories.len() { writer.start_element("c:dPt"); empty(writer, "c:idx", &[("val", &position.to_string())]); writer.start_element("c:spPr"); color(writer, &format!("@accent{}", position % 6 + 1)); writer.end_element(); writer.end_element(); }
            }
            if kind == ChartKind::Scatter {
                writer.start_element("c:xVal"); writer.start_element("c:numRef"); text(writer, "c:f", &format!("Sheet1!$A$2:$A${}", categories.len() + 1)); writer.start_element("c:numCache"); text(writer, "c:formatCode", "General");
                empty(writer, "c:ptCount", &[("val", &categories.len().to_string())]);
                for (position, category) in categories.iter().enumerate() { writer.start_element("c:pt"); writer.write_attribute("idx", &position.to_string()); text(writer, "c:v", category); writer.end_element(); }
                writer.end_element(); writer.end_element(); writer.end_element();
            } else { writer.start_element("c:cat"); string_reference(writer, &format!("Sheet1!$A$2:$A${}", categories.len() + 1), categories); writer.end_element(); }
            writer.start_element(if kind == ChartKind::Scatter { "c:yVal" } else { "c:val" }); writer.start_element("c:numRef");
            text(writer, "c:f", &format!("Sheet1!${column}$2:${column}${}", categories.len() + 1));
            writer.start_element("c:numCache"); text(writer, "c:formatCode", "General");
            empty(writer, "c:ptCount", &[("val", &entry.values.len().to_string())]);
            for (position, value) in entry.values.iter().enumerate() {
                writer.start_element("c:pt"); writer.write_attribute("idx", &position.to_string()); text(writer, "c:v", &value.to_string()); writer.end_element();
            }
            writer.end_element(); writer.end_element(); writer.end_element();
            if kind == ChartKind::Line { empty(writer, "c:smooth", &[("val", "0")]); }
            writer.end_element();
        }
        if kind.is_bar() { empty(writer, "c:gapWidth", &[("val", "90")]); empty(writer, "c:overlap", &[("val", if matches!(kind, ChartKind::StackedColumn | ChartKind::StackedBar | ChartKind::PercentStackedColumn) { "100" } else { "0" })]); }
        if kind.is_polar() { empty(writer, "c:firstSliceAng", &[("val", "0")]); if kind == ChartKind::Doughnut { empty(writer, "c:holeSize", &[("val", "50")]); } }
        else { empty(writer, "c:axId", &[("val", "1")]); empty(writer, "c:axId", &[("val", "2")]); }
        writer.end_element();
        for (tag, id, cross, position) in [(if kind == ChartKind::Scatter { "c:valAx" } else { "c:catAx" }, "1", "2", if kind.is_horizontal() { "l" } else { "b" }), ("c:valAx", "2", "1", if kind.is_horizontal() { "b" } else { "l" })] {
            if kind.is_polar() { break; }
            writer.start_element(tag); empty(writer, "c:axId", &[("val", id)]);
            writer.start_element("c:scaling"); empty(writer, "c:orientation", &[("val", "minMax")]);
            if id == "2" && kind == ChartKind::PercentStackedColumn { empty(writer,"c:max",&[("val","1")]); }
            if id == "2" && (kind.is_bar() || kind == ChartKind::Area) {
                if series.iter().flat_map(|entry| &entry.values).all(|value| *value >= 0.0) { empty(writer, "c:min", &[("val", "0")]); }
                else if series.iter().flat_map(|entry| &entry.values).all(|value| *value <= 0.0) { empty(writer, "c:max", &[("val", "0")]); }
            }
            writer.end_element();
            empty(writer, "c:delete", &[("val", "0")]); empty(writer, "c:axPos", &[("val", position)]);
            if id == "2" {
                writer.start_element("c:majorGridlines"); writer.start_element("c:spPr"); writer.start_element("a:ln"); writer.write_attribute("w", "6350"); color(writer, "DDE4E1"); writer.end_element(); writer.end_element(); writer.end_element();
                empty(writer, "c:numFmt", &[("formatCode", if kind==ChartKind::PercentStackedColumn {"0%"} else {"General"}), ("sourceLinked", if kind==ChartKind::PercentStackedColumn {"0"} else {"1"})]);
            }
            empty(writer, "c:majorTickMark", &[("val", "none")]); empty(writer, "c:minorTickMark", &[("val", "none")]);
            empty(writer, "c:tickLblPos", &[("val", "nextTo")]); empty(writer, "c:crossAx", &[("val", cross)]); empty(writer, "c:crosses", &[("val", "autoZero")]);
            if tag == "c:catAx" { empty(writer, "c:auto", &[("val", "1")]); empty(writer, "c:lblAlgn", &[("val", "ctr")]); empty(writer, "c:lblOffset", &[("val", "100")]); }
            else { empty(writer, "c:crossBetween", &[("val", "between")]); }
            writer.end_element();
        }
        writer.end_element();
        writer.start_element("c:legend"); empty(writer, "c:legendPos", &[("val", "b")]); empty(writer, "c:overlay", &[("val", "0")]); writer.end_element();
        empty(writer, "c:plotVisOnly", &[("val", "1")]); empty(writer, "c:dispBlanksAs", &[("val", "gap")]); writer.end_element();
        writer.start_element("c:externalData"); writer.write_attribute("r:id", "rIdWorkbook"); empty(writer, "c:autoUpdate", &[("val", "0")]); writer.end_element();
    })
}

fn string_cell(writer: &mut XmlWriter, address: &str, value: &str) {
    writer.start_element("c"); writer.write_attribute("r", address); writer.write_attribute("t", "inlineStr");
    writer.start_element("is"); writer.start_element("t"); writer.write_attribute("xml:space", "preserve");
    writer.write_text(&quick_xml::escape::escape(value)); writer.end_element(); writer.end_element(); writer.end_element();
}

pub(crate) fn workbook(kind: ChartKind, categories: &[String], series: &[ChartSeries]) -> Result<Vec<u8>> {
    let mut parts = BTreeMap::new();
    parts.insert("[Content_Types].xml".into(), br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#.to_vec());
    parts.insert("_rels/.rels".into(), br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_vec());
    parts.insert("xl/workbook.xml".into(), xml("workbook", |writer| {
        writer.write_attribute("xmlns", SHEET_NS); writer.start_element("sheets"); empty(writer, "sheet", &[("name", "Sheet1"), ("sheetId", "1"), ("r:id", "rId1")]); writer.end_element();
    }));
    parts.insert("xl/_rels/workbook.xml.rels".into(), br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#.to_vec());
    parts.insert("xl/worksheets/sheet1.xml".into(), xml("worksheet", |writer| {
        writer.write_attribute("xmlns", SHEET_NS); writer.start_element("sheetData");
        writer.start_element("row"); writer.write_attribute("r", "1"); string_cell(writer, "A1", "Category");
        for (index, entry) in series.iter().enumerate() { string_cell(writer, &format!("{}1", char::from(b'B' + index as u8)), &entry.name); }
        writer.end_element();
        for (index, category) in categories.iter().enumerate() {
            let row = index + 2;
            writer.start_element("row"); writer.write_attribute("r", &row.to_string());
            if kind == ChartKind::Scatter { writer.start_element("c"); writer.write_attribute("r", &format!("A{row}")); text(writer, "v", category); writer.end_element(); }
            else { string_cell(writer, &format!("A{row}"), category); }
            for (column, entry) in series.iter().enumerate() {
                writer.start_element("c"); writer.write_attribute("r", &format!("{}{row}", char::from(b'B' + column as u8))); text(writer, "v", &entry.values[index].to_string()); writer.end_element();
            }
            writer.end_element();
        }
        writer.end_element();
    }));
    Package::from_parts(parts)?.save()
}