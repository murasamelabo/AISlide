use aislide_core::{model::{Deck, validate_deck}, package::Package, pptx::export_pptx};
use serde_json::{json, Value};

fn scene(kind: &str) -> Value {
    json!({"version":1,"title":"Synthetic chart fixture","width":1280,"height":720,"slides":[{
        "id":"slide-1","title":"Synthetic trend","background":"FFFFFF","notes":"Synthetic test data, not factual measurements.",
        "elements":[{"type":"chart","id":"trend","x":64,"y":140,"width":1152,"height":480,
            "kind":kind,"categories":["Q1","Q2","Q3"],"series":[
                {"name":"Actual & test","values":[-5.5,0,12],"color":"087F73"},
                {"name":"Plan","values":[2,4,8],"color":"CF5847"}
            ]}]
    }]})
}

#[test]
fn charts_are_native_objects_with_internal_editable_workbooks() {
    for kind in ["column", "bar", "line"] {
        let deck: Deck = serde_json::from_value(scene(kind)).expect("chart must be a supported scene element");
        validate_deck(&deck).unwrap();
        let bytes = export_pptx(&deck).unwrap();
        assert_eq!(bytes, export_pptx(&deck).unwrap());
        let package = Package::open(bytes).unwrap();
        let slide = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
        assert_eq!(slide.descendants().filter(|node| node.tag_name().name() == "chart").count(), 1);
        assert!(!package.parts().keys().any(|name| name.starts_with("ppt/media/")));
        let chart = roxmltree::Document::parse(package.text("ppt/charts/chart1.xml").unwrap()).unwrap();
        assert_eq!(chart.descendants().filter(|node| node.tag_name().name() == "ser").count(), 2);
        if kind != "line" { assert_eq!(chart.descendants().filter(|node| node.tag_name().name() == "invertIfNegative" && node.attribute("val") == Some("0")).count(), 2); }
        assert!(chart.descendants().any(|node| node.tag_name().name() == "f" && node.text() == Some("Sheet1!$B$2:$B$4")));
        let relation = package.text("ppt/charts/_rels/chart1.xml.rels").unwrap();
        assert!(relation.contains("../embeddings/chart1.xlsx"));
        assert!(!relation.contains("External"));
        let workbook = Package::open(package.part("ppt/embeddings/chart1.xlsx").unwrap().to_vec()).unwrap();
        let sheet = workbook.text("xl/worksheets/sheet1.xml").unwrap();
        assert!(sheet.contains("-5.5"));
        let types = package.text("[Content_Types].xml").unwrap();
        assert!(types.contains("drawingml.chart+xml"));
        assert!(types.contains("spreadsheetml.sheet"));
    }
}

#[test]
fn chart_data_must_be_finite_rectangular_and_bounded() {
    for invalid in [
        json!({"categories":[]}),
        json!({"series":[]}),
        json!({"series":[{"name":"Wrong count","values":[1],"color":"087F73"}]}),
        json!({"series":[{"name":"Wrong color","values":[1,2,3],"color":"red"}]}),
        json!({"kind":"pie"}),
        json!({"categories":vec!["category"; 33]}),
        json!({"series":[{"name":"Too large","values":[1e30,2,3],"color":"087F73"}]})
    ] {
        let mut value = scene("column");
        for (key, entry) in invalid.as_object().unwrap() { value["slides"][0]["elements"][0][key] = entry.clone(); }
        match serde_json::from_value::<Deck>(value) {
            Ok(deck) => assert!(validate_deck(&deck).is_err()),
            Err(_) => {}
        }
    }
}

#[test]
fn report_chart_layout_uses_the_shared_scene_compiler() {
    let chart = scene("line")["slides"][0]["elements"][0].clone();
    let mut report = serde_json::to_value(aislide_core::report::sample_report()).unwrap();
    report["sections"][1] = json!({"title":"Synthetic chart report","layout":"chart","body":["Synthetic values"],"chart":{"kind":"line","categories":chart["categories"],"series":chart["series"]}});
    let compiled = aislide_core::report::compile_report(&serde_json::from_value(report).unwrap()).unwrap();
    let bytes = export_pptx(&compiled.deck).unwrap();
    let package = Package::open(bytes).unwrap();
    assert!(package.part("ppt/charts/chart1.xml").is_ok());
}

#[test]
fn native_bar_axes_include_zero_for_one_sided_values() {
    for (values, bound) in [(vec![4.0, 7.0, 5.0], "min"), (vec![-4.0, -7.0, -5.0], "max")] {
        let mut input = scene("column");
        for series in input["slides"][0]["elements"][0]["series"].as_array_mut().unwrap() { series["values"] = json!(values); }
        let deck: Deck = serde_json::from_value(input).unwrap();
        let package = Package::open(export_pptx(&deck).unwrap()).unwrap();
        let chart = roxmltree::Document::parse(package.text("ppt/charts/chart1.xml").unwrap()).unwrap();
        let axis = chart.descendants().find(|node| node.tag_name().name() == "valAx").unwrap();
        assert!(axis.descendants().any(|node| node.tag_name().name() == bound && node.attribute("val") == Some("0")));
    }
}

#[test]
fn extended_funnel_uses_native_chartex_not_a_renamed_bar_chart() {
    let mut input = scene("funnel");
    input["slides"][0]["elements"][0]["series"] = json!([
        {"name":"Synthetic funnel & stages","values":[120,80,40],"color":"087F73"}
    ]);
    let deck: Deck = serde_json::from_value(input).expect("native funnel kind");
    validate_deck(&deck).unwrap();
    let bytes = export_pptx(&deck).unwrap();
    assert_eq!(bytes, export_pptx(&deck).unwrap());
    let package = Package::open(bytes).unwrap();
    let chart = roxmltree::Document::parse(package.text("ppt/charts/chart1.xml").unwrap()).unwrap();
    let namespace = "http://schemas.microsoft.com/office/drawing/2014/chartex";
    assert!(chart.root_element().has_tag_name((namespace, "chartSpace")));
    assert!(chart.descendants().any(|node| node.has_tag_name((namespace, "series")) && node.attribute("layoutId") == Some("funnel")));
    assert!(!chart.descendants().any(|node| node.tag_name().name() == "barChart"));
    assert!(!package.parts().keys().any(|name| name.starts_with("ppt/media/")));
    assert!(package.text("[Content_Types].xml").unwrap().contains("application/vnd.ms-office.chartex+xml"));
    assert!(package.text("ppt/slides/_rels/slide1.xml.rels").unwrap().contains("http://schemas.microsoft.com/office/2014/relationships/chartEx"));
    let workbook = Package::open(package.part("ppt/embeddings/chart1.xlsx").unwrap().to_vec()).unwrap();
    assert!(workbook.text("xl/worksheets/sheet1.xml").unwrap().contains("120"));
}