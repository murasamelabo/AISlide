use aislide_core::{execute_request, model::Deck, package::Package, pptx::export_pptx};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn ingest(format: &str, name: &str, bytes: &[u8]) -> aislide_core::Result<Value> {
    execute_request(json!({"op":"ingest","input":{"format":format,"name":name,"base64":STANDARD.encode(bytes)}}))
}

fn mapping() -> Value {
    json!({"title":"Provided quarterly values","period":"Provided period","table_index":0,"category_column":0,"value_columns":[1],"row_start":0,"row_count":3,"chart_kind":"column"})
}

fn workbook() -> Vec<u8> {
    let deck: Deck = serde_json::from_value(json!({"version":1,"title":"Fixture","width":1280,"height":720,"slides":[{"id":"slide-1","title":"Fixture","background":"FFFFFF","notes":"Synthetic","elements":[{"type":"chart","id":"chart","x":50,"y":50,"width":1000,"height":500,"kind":"column","categories":["Q1","Q2","Q3"],"series":[{"name":"Value","values":[-2,0,12],"color":"087F73"}]}]}]})).unwrap();
    let package = Package::open(export_pptx(&deck).unwrap()).unwrap();
    package.part("ppt/embeddings/chart1.xlsx").unwrap().to_vec()
}

#[test]
fn csv_values_hash_and_record_locators_are_preserved() {
    let bytes = b"Quarter,Value\r\nQ1,-2\r\nQ2,0\r\nQ3,12\r\n";
    let source = ingest("csv", "provided.csv", bytes).unwrap();
    assert_eq!(source, ingest("csv", "provided.csv", bytes).unwrap());
    assert_eq!(source["tables"][0]["rows"][0][1], "-2");
    assert_eq!(source["tables"][0]["locators"][0][1], "record:2/column:2");
    assert_eq!(source["sha256"].as_str().unwrap().len(), 64);
    assert_ne!(source["sha256"], ingest("csv", "provided.csv", b"Quarter,Value\nQ1,999\n").unwrap()["sha256"]);
}

#[test]
fn json_types_and_json_pointer_escaping_are_preserved() {
    let source = ingest("json", "provided.json", br#"[{"Net/revenue":12.5,"active":true},{"Net/revenue":null,"active":false}]"#).unwrap();
    assert_eq!(source["tables"][0]["rows"][0][0], 12.5);
    assert_eq!(source["tables"][0]["rows"][0][1], true);
    assert_eq!(source["tables"][0]["rows"][1][0], Value::Null);
    assert_eq!(source["tables"][0]["locators"][0][0], "/0/Net~1revenue");
}

#[test]
fn xlsx_cells_are_read_without_executing_formulas() {
    let source = ingest("xlsx", "provided.xlsx", &workbook()).unwrap();
    assert_eq!(source["tables"][0]["name"], "Sheet1");
    assert_eq!(source["tables"][0]["rows"][0][1], -2.0);
    assert_eq!(source["tables"][0]["locators"][0][1], "Sheet1!B2");
    let mut package = Package::open(workbook()).unwrap();
    let sheet = package.text("xl/worksheets/sheet1.xml").unwrap().replace("<v>-2</v>", "<f>WEBSERVICE(&quot;https://example.invalid&quot;)</f><v>-2</v>");
    package.replace_part("xl/worksheets/sheet1.xml", sheet.into_bytes()).unwrap();
    let formula = ingest("xlsx", "cached.xlsx", &package.save().unwrap()).unwrap();
    assert!(formula["warnings"].as_array().unwrap().iter().any(|warning| warning.as_str().unwrap().contains("cached")));
    assert_eq!(formula["tables"][0]["rows"][0][1], -2.0);
}

#[test]
fn dangerous_or_unbounded_spreadsheets_are_rejected_before_reading_ranges() {
    let mut package = Package::open(workbook()).unwrap();
    let sheet = package.text("xl/worksheets/sheet1.xml").unwrap().replace("r=\"B4\"", "r=\"XFD1048576\"");
    package.replace_part("xl/worksheets/sheet1.xml", sheet.into_bytes()).unwrap();
    assert!(ingest("xlsx", "sparse-bomb.xlsx", &package.save().unwrap()).is_err());
    assert!(ingest("csv", "duplicate.csv", b"Same,Same\n1,2\n").is_err());
    assert!(ingest("json", "nested.json", br#"[{"value":{"nested":1}}]"#).is_err());
    assert!(ingest("csv", "large.csv", format!("Label,Value\n{},1\n", "X".repeat(2001)).as_bytes()).is_err());
    assert!(ingest("csv", "oversized.csv", &vec![b'x'; 2 * 1024 * 1024 + 1]).is_err());
}

#[test]
fn mapped_data_report_binds_chart_values_to_exact_source_cells() {
    let source = ingest("csv", "provided.csv", b"Quarter,Value\nQ1,-2\nQ2,0\nQ3,12\n").unwrap();
    let result = execute_request(json!({"op":"data_report","source":source,"mapping":mapping()})).unwrap();
    assert_eq!(result["compiled"]["deck"]["slides"].as_array().unwrap().len(), 12);
    let bindings = result["bindings"].as_array().unwrap();
    assert!(bindings.iter().any(|binding| binding["locator"] == "record:4/column:2" && binding["value"] == 12.0 && binding["source_id"] == source["id"]));
    assert!(bindings.iter().any(|binding| binding["element_id"] == "data-table" && binding["field"] == "/rows/3/1" && binding["locator"] == "record:4/column:2" && binding["value"] == "12"));
    let mut changed = source.clone();
    changed["tables"][0]["rows"][0][1] = json!("999");
    assert!(execute_request(json!({"op":"data_report","source":changed,"mapping":mapping()})).is_err());
}

#[test]
fn mapping_never_coerces_missing_or_formula_values_to_numbers() {
    for value in ["", "=1+1", "not-a-number", "NaN"] {
        let source = ingest("csv", "provided.csv", format!("Quarter,Value\nQ1,{value}\nQ2,0\nQ3,12\n").as_bytes()).unwrap();
        assert!(execute_request(json!({"op":"data_report","source":source,"mapping":mapping()})).is_err());
    }
}

#[test]
fn markdown_is_retained_as_plain_evidence_not_executed() {
    let source = ingest("markdown", "notes.md", b"# Notes\n<script>alert(1)</script>\n").unwrap();
    assert_eq!(source["text"], "# Notes\n<script>alert(1)</script>\n");
    assert!(source["tables"].as_array().unwrap().is_empty());
}

#[test]
fn attribution_is_retained_in_the_source_and_generated_notes_without_network_access() {
    let source = execute_request(json!({"op":"ingest","input":{"format":"csv","name":"attributed.csv","base64":STANDARD.encode(b"Quarter,Value\nQ1,1\nQ2,2\nQ3,3\n"),"attribution":{"citation":"Fixture source","url":"https://example.invalid/data","license":"test-only","transformation":"No transform"}}})).unwrap();
    let report = execute_request(json!({"op":"data_report","source":source,"mapping":mapping()})).unwrap();
    assert_eq!(source["attribution"]["citation"], "Fixture source");
    assert!(report["compiled"]["deck"]["slides"][0]["notes"].as_str().unwrap().contains("https://example.invalid/data"));
}