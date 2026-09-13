use aislide_core::execute_request;
use serde_json::json;

#[test]
fn layout_measurement_detects_overflow_without_claiming_office_parity() {
    let deck = json!({"version":1,"title":"Layout","width":1280,"height":720,"slides":[{"id":"slide","title":"Layout","background":"FFFFFF","notes":"","elements":[
        {"type":"text","id":"fits","x":50,"y":50,"width":1100,"height":100,"text":"Short readable title","font_size":32,"color":"202525","bold":true},
        {"type":"text","id":"overflows","x":50,"y":200,"width":100,"height":20,"text":"This text intentionally cannot fit inside such a narrow and short box.","font_size":40,"color":"202525","bold":false}
    ]}]});
    let result = execute_request(json!({"op":"measure_layout","deck":deck})).unwrap();
    assert_eq!(result["office_parity_verified"], false);
    assert!(result["measurements"].as_array().unwrap().iter().any(|measurement| measurement["element_id"] == "overflows" && measurement["overflow"] == true));
    assert!(result["measurements"].as_array().unwrap().iter().any(|measurement| measurement["element_id"] == "fits" && measurement["overflow"] == false));
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| issue["code"] == "TEXT_OVERFLOW"));
}

#[test]
fn native_report_text_and_tables_have_measured_font_evidence() {
    let report = execute_request(json!({"op":"sample"})).unwrap();
    let compiled = execute_request(json!({"op":"compile","report":report})).unwrap();
    let result = execute_request(json!({"op":"measure_layout","deck":compiled["deck"]})).unwrap();
    assert!(result["measurements"].as_array().unwrap().len() > 50);
    assert!(!result["fonts"].as_array().unwrap().is_empty());
    assert_eq!(result["engine"], "cosmic-text");
}

#[test]
fn maximum_mapped_data_rows_fit_the_generated_table_cells() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut csv = String::from("Category,Value\n");
    for index in 0..32 { csv.push_str(&format!("Item {},{}\n", index + 1, index * 100)); }
    let source = execute_request(json!({"op":"ingest","input":{"name":"provided.csv","format":"csv","base64":STANDARD.encode(csv.as_bytes())}})).unwrap();
    let report = execute_request(json!({"op":"data_report","source":source,"mapping":{"title":"Provided rows","period":"Input","table_index":0,"category_column":0,"value_columns":[1],"row_start":0,"row_count":32,"chart_kind":"column"}})).unwrap();
    let measured = execute_request(json!({"op":"measure_layout","deck":report["compiled"]["deck"]})).unwrap();
    let overflowing_tables: Vec<_> = measured["measurements"].as_array().unwrap().iter().filter(|measurement| measurement["element_id"].as_str().unwrap().starts_with("data-table") && measurement["overflow"] == true).collect();
    assert!(overflowing_tables.is_empty(), "{} table cells overflowed", overflowing_tables.len());
}

#[test]
fn long_metric_values_are_measured_into_their_fixed_frames() {
    let mut report = execute_request(json!({"op":"sample"})).unwrap();
    report["sections"][2]["metrics"] = json!([
        {"label":"Source","value":"SYNTHETIC TEST DATA"},
        {"label":"Scope","value":"Q1-Q4"},
        {"label":"Boundary","value":"12345678901234567890"},
        {"label":"Units","value":"arbitrary units"}
    ]);
    let compiled = execute_request(json!({"op":"compile","report":report})).unwrap();
    let measured = execute_request(json!({"op":"measure_layout","deck":compiled["deck"]})).unwrap();
    assert!(!measured["measurements"].as_array().unwrap().iter().any(|entry| entry["element_id"].as_str().unwrap().starts_with("metric-value") && entry["overflow"] == true));
}