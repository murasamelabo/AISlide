use aislide_core::execute_request;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn document() -> Value {
    let report = execute_request(json!({"op":"sample"})).unwrap();
    let compiled = execute_request(json!({"op":"compile","report":report})).unwrap();
    execute_request(json!({"op":"new_document","id":"test-document","deck":compiled["deck"]})).unwrap()
}

fn title_path(document: &Value) -> String {
    let index = document["deck"]["slides"][0]["elements"].as_array().unwrap().iter().position(|element| element["id"] == "title").unwrap();
    format!("/deck/slides/0/elements/{index}/text")
}

fn transact(document: &Value, operations: Value) -> aislide_core::Result<Value> {
    execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":operations}}))
}

fn undo(document: &Value, receipt: &Value) -> aislide_core::Result<Value> {
    execute_request(json!({"op":"undo_transaction","document":document,"expected_revision":document["revision"],"receipt":receipt}))
}

#[test]
fn batch_is_atomic_revision_checked_and_undoable() {
    let original = document();
    let path = title_path(&original);
    let changed = transact(&original, json!([
        {"op":"test","path":path,"value":original.pointer(&path).unwrap()},
        {"op":"replace","path":path,"value":"Edited as one transaction"},
        {"op":"replace","path":"/deck/slides/0/background","value":"EDF3F0"}
    ])).unwrap();
    assert_eq!(changed["document"]["revision"], 1);
    assert_eq!(changed["document"].pointer(&path).unwrap(), "Edited as one transaction");
    assert_eq!(undo(&changed["document"], &changed["receipt"]).unwrap()["document"]["deck"], original["deck"]);
    let stale = execute_request(json!({"op":"transaction","document":changed["document"],"transaction":{"expected_revision":0,"expected_hash":original["hash"],"operations":[{"op":"replace","path":path,"value":"stale"}]}}));
    assert!(stale.is_err());
    assert!(transact(&original, json!([
        {"op":"replace","path":path,"value":"Must not escape"},
        {"op":"replace","path":"/deck/slides/0/elements/0/width","value":-10}
    ])).is_err());
}

#[test]
fn consecutive_undo_uses_content_preconditions_and_monotonic_revisions() {
    let original = document();
    let path = title_path(&original);
    let first = transact(&original, json!([{"op":"replace","path":path,"value":"First"}])).unwrap();
    let second = transact(&first["document"], json!([{"op":"replace","path":path,"value":"Second"}])).unwrap();
    assert!(undo(&second["document"], &first["receipt"]).is_err());
    let undo_second = undo(&second["document"], &second["receipt"]).unwrap();
    let undo_first = undo(&undo_second["document"], &first["receipt"]).unwrap();
    assert_eq!(undo_first["document"]["deck"], original["deck"]);
    assert_eq!(undo_first["document"]["revision"], 4);
    let redo = undo(&undo_first["document"], &undo_first["receipt"]).unwrap();
    assert_eq!(redo["document"].pointer(&path).unwrap(), "First");
}

#[test]
fn malformed_patches_and_direct_document_tampering_are_rejected() {
    let original = document();
    for operations in [json!([]), json!([{"op":"replace","path":"/revision","value":0}]), json!([{"op":"remove","path":"/deck"}]), json!([{"op":"replace","path":"/deck/slides/0/background","value":"<script>"}])] {
        assert!(transact(&original, operations).is_err());
    }
    let mut tampered = original.clone();
    tampered["deck"]["title"] = json!("tampered outside transactions");
    assert!(transact(&tampered, json!([{"op":"replace","path":title_path(&original),"value":"New"}])).is_err());
}

#[test]
fn checkpoints_require_the_exact_pptx_and_matching_scene() {
    let original = document();
    let exported = execute_request(json!({"op":"export_project","document":original})).unwrap();
    let restored = execute_request(json!({"op":"open_project","base64":exported["base64"],"checkpoint":exported["checkpoint"]})).unwrap();
    assert_eq!(restored, original);
    let mut bytes = STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap(); bytes.push(0);
    assert!(execute_request(json!({"op":"open_project","base64":STANDARD.encode(bytes),"checkpoint":exported["checkpoint"]})).is_err());
    let mut checkpoint = exported["checkpoint"].clone(); checkpoint["document"]["deck"]["title"] = json!("changed");
    assert!(execute_request(json!({"op":"open_project","base64":exported["base64"],"checkpoint":checkpoint})).is_err());
}

#[test]
fn source_bindings_become_stale_on_data_edit_and_recover_on_undo() {
    let source = execute_request(json!({"op":"ingest","input":{"format":"csv","name":"provided.csv","base64":STANDARD.encode(b"Quarter,Value\nQ1,-2\nQ2,0\nQ3,12\n")}})).unwrap();
    let report = execute_request(json!({"op":"data_report","source":source,"mapping":{"title":"Provided values","period":"Input period","table_index":0,"category_column":0,"value_columns":[1],"row_start":0,"row_count":3,"chart_kind":"column"}})).unwrap();
    let original = execute_request(json!({"op":"new_document","id":"bound-document","deck":report["compiled"]["deck"],"sources":[source],"bindings":report["bindings"]})).unwrap();
    let chart_index = original["deck"]["slides"][3]["elements"].as_array().unwrap().iter().position(|element| element["type"] == "chart").unwrap();
    let changed = transact(&original, json!([{"op":"replace","path":format!("/deck/slides/3/elements/{chart_index}/series/0/values/0"),"value":999}])).unwrap();
    assert!(changed["document"]["bindings"].as_array().unwrap().iter().any(|binding| binding["stale"] == true));
    assert!(execute_request(json!({"op":"export_project","document":changed["document"]})).is_err());
    let restored = undo(&changed["document"], &changed["receipt"]).unwrap();
    assert!(execute_request(json!({"op":"export_project","document":restored["document"]})).is_ok());
}

#[test]
fn new_project_export_rejects_measured_text_overflow() {
    let original = document();
    let path = title_path(&original);
    let changed = transact(&original, json!([{"op":"replace","path":path,"value":"Long words cannot safely fit into the slide title. ".repeat(50)}])).unwrap();
    let exported = execute_request(json!({"op":"export_project","document":changed["document"]}));
    assert!(exported.is_err(), "project publication must block measured overflow rather than silently clipping content");
}