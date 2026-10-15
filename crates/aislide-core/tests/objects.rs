use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::json;

#[test]
fn catalog_shapes_export_as_native_presets_with_text_and_links() {
    let catalog = execute_request(json!({"op":"object_catalog"})).unwrap();
    assert!(catalog["shapes"].as_array().unwrap().len() >= 30);
    let mut elements = Vec::new();
    for preset in ["ellipse", "roundRect", "diamond", "rightArrow", "star5", "flowChartDecision", "wedgeRoundRectCallout", "heart"] {
        let mut element = execute_request(json!({"op":"create_object","id":format!("shape-{preset}"),"kind":"shape","preset":preset})).unwrap();
        element["text"] = json!("Native text");
        element["format"] = json!({"italic":true,"underline":true,"alignment":"center","hyperlink":"https://example.invalid/info"});
        elements.push(element);
    }
    let deck = json!({"version":1,"title":"Native objects","width":1280,"height":720,"slides":[{"id":"slide-1","title":"Shapes","background":"FFFFFF","notes":"Own test shapes","elements":elements}]});
    let exported = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let slide = package.text("ppt/slides/slide1.xml").unwrap();
    for preset in ["ellipse", "roundRect", "diamond", "rightArrow", "star5", "flowChartDecision", "wedgeRoundRectCallout", "heart"] { assert!(slide.contains(&format!("prst=\"{preset}\""))); }
    assert_eq!(slide.matches("<a:hlinkClick").count(), 8);
    assert!(package.text("ppt/slides/_rels/slide1.xml.rels").unwrap().contains("TargetMode=\"External\""));
    assert!(slide.contains("Native text"));
}

#[test]
fn additional_chart_types_have_native_chart_parts_and_workbooks() {
    for kind in ["pie", "doughnut", "area", "scatter", "stacked_column", "stacked_bar"] {
        let element = execute_request(json!({"op":"create_object","id":"chart","kind":"chart","preset":kind})).unwrap();
        let deck = json!({"version":1,"title":"Native chart","width":1280,"height":720,"slides":[{"id":"slide","title":"Chart","background":"FFFFFF","notes":"Synthetic chart values","elements":[element]}]});
        let result = execute_request(json!({"op":"export","deck":deck})).unwrap();
        let package = Package::open(STANDARD.decode(result["base64"].as_str().unwrap()).unwrap()).unwrap();
        let chart = package.text("ppt/charts/chart1.xml").unwrap();
        let tag = match kind { "pie" => "pieChart", "doughnut" => "doughnutChart", "area" => "areaChart", "scatter" => "scatterChart", _ => "barChart" };
        assert!(chart.contains(&format!("<c:{tag}")));
        assert!(package.part("ppt/embeddings/chart1.xlsx").is_ok());
    }
}

#[test]
fn native_table_and_line_factories_validate_their_parameters() {
    let table = execute_request(json!({"op":"create_object","id":"table","kind":"table","rows":4,"columns":3})).unwrap();
    assert_eq!(table["rows"].as_array().unwrap().len(), 4);
    assert_eq!(table["rows"][0].as_array().unwrap().len(), 3);
    assert_eq!(execute_request(json!({"op":"create_object","id":"line","kind":"line"})).unwrap()["type"], "connector");
    assert!(execute_request(json!({"op":"create_object","id":"table","kind":"table","rows":999,"columns":3})).is_err());
    assert!(execute_request(json!({"op":"create_object","id":"bad","kind":"shape","preset":"arbitrary-script"})).is_err());
}