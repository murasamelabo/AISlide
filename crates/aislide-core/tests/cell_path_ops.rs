use aislide_core::execute_request;
use serde_json::{json, Value};

fn table() -> Value {
    json!({"type":"table","id":"table","x":40,"y":100,"width":600,"height":180,"font_size":20,
        "rows":[["Header", ""],["A", "B"]],"format":{"column_widths":{"unit":"relative","values":[1,2]},
        "merges":[{"row":0,"column":0,"row_span":1,"col_span":2}],
        "cells":[{"row":0,"column":0,"style":{"fill":"@accent2","vertical":"middle",
        "text_style":{"bold":true},"text_format":{"paragraphs":[{"alignment":"center","runs":[
        {"text":"Head","style":{"italic":true}},{"text":"er","style":{"color":"@accent3"}}]}]}}}]}})
}

fn polygon() -> Value {
    json!({"type":"polygon","id":"polygon","x":100,"y":100,"width":300,"height":180,
        "points":[[0,0],[1,0],[0,1]],"fill":"@accent1","stroke":"@dk1","stroke_width":2,
        "visual":{"opacity":0.5,"flip_h":true,"shadow":{"color":"112233","opacity":0.4,"blur":6,"distance":8,"angle":45}}})
}

fn curve() -> Value {
    json!({"commands":[{"op":"move","point":[0.0,0.0]},
        {"op":"quadratic","control":[0.5,0.0],"point":[1.0,1.0]},
        {"op":"cubic","control1":[0.8,1.0],"control2":[0.0,0.8],"point":[0.0,1.0]}, {"op":"close"}]})
}

fn canonical(value: Value) -> Value {
    serde_json::to_value(serde_json::from_value::<aislide_core::model::Element>(value).unwrap()).unwrap()
}

#[test]
fn cell_text_helper_preserves_rich_styles_and_synchronizes_rows() {
    let original = canonical(table());
    let updated = execute_request(json!({"op":"set_table_cell_text","element":original,"row":0,"column":0,"text":"Heading\nNext"})).unwrap();
    assert_eq!(updated["rows"][0][0], "Heading\nNext");
    assert_eq!(updated["format"]["column_widths"], original["format"]["column_widths"]);
    assert_eq!(updated["format"]["merges"], original["format"]["merges"]);
    let style = &updated["format"]["cells"][0]["style"];
    assert_eq!(style["fill"], "@accent2");
    assert_eq!(style["vertical"], "middle");
    assert_eq!(style["text_style"], original["format"]["cells"][0]["style"]["text_style"]);
    let paragraphs: Vec<aislide_core::rich_text::RichParagraph> = serde_json::from_value(style["text_format"]["paragraphs"].clone()).unwrap();
    assert_eq!(aislide_core::rich_text::plain_text(&paragraphs), "Heading\nNext");
    assert_eq!(style["text_format"]["paragraphs"][0]["alignment"], "center");
    assert_eq!(style["text_format"]["paragraphs"][0]["runs"][0]["style"]["italic"], true);
}

#[test]
fn vector_helper_preserves_full_element_and_canonicalizes_both_path_kinds() {
    let original = canonical(polygon());
    let edited = execute_request(json!({"op":"edit_vector","element":original,"path":curve()})).unwrap();
    assert_eq!(edited["visual"]["path"], curve());
    assert_eq!(edited["points"], json!([[0.0,0.0],[0.5,0.0],[1.0,1.0],[0.8,1.0],[0.0,0.8],[0.0,1.0]]));
    let straight = json!({"commands":[{"op":"move","point":[0,0]},{"op":"line","point":[1,0]},{"op":"line","point":[0,1]},{"op":"close"}]});
    let restored = execute_request(json!({"op":"edit_vector","element":edited,"path":straight})).unwrap();
    assert_eq!(restored, original);
}

fn deck(elements: Value) -> Value {
    json!({"version":1,"title":"Cell path fixture","width":1280,"height":720,
        "slides":[{"id":"slide","title":"Fixture","background":"FFFFFF","notes":"","elements":elements}]})
}

fn open(base64: &Value) -> Value {
    execute_request(json!({"op":"open_presentation","id":"cell-path-native","base64":base64})).unwrap()["document"].clone()
}

fn save(document: &Value) -> Value {
    execute_request(json!({"op":"export_presentation","document":document})).unwrap()["base64"].clone()
}

#[test]
fn table_batch_is_atomic_native_roundtrippable_and_undoable() {
    let source = execute_request(json!({"op":"export","deck":deck(json!([table()]))})).unwrap()["base64"].clone();
    let document = open(&source);
    let before = &document["deck"]["slides"][0]["elements"][0];
    let operations = json!([{"op":"set_cell_text","row":0,"column":0,"text":"Heading"},{"op":"set_cell_text","row":1,"column":1,"text":"Next"}]);
    let edit = |operations: Value, revision: Value| execute_request(json!({"op":"edit_table","document":document,"expected_revision":revision,"slide_id":document["deck"]["slides"][0]["id"],"id":before["id"],"operations":operations}));
    let updated = edit(operations.clone(), document["revision"].clone()).unwrap();
    assert_eq!(updated["document"]["revision"].as_u64(), Some(document["revision"].as_u64().unwrap() + 1));
    let expected = &updated["document"]["deck"]["slides"][0]["elements"][0];
    assert_eq!(expected["rows"], json!([["Heading", ""],["A", "Next"]]));
    assert_eq!(expected["format"]["column_widths"], before["format"]["column_widths"]);
    assert_eq!(expected["format"]["merges"], before["format"]["merges"]);
    assert_eq!(open(&save(&updated["document"]))["deck"]["slides"][0]["elements"][0], *expected);
    let mut invalid = operations.clone(); invalid[1]["row"] = json!(0); invalid[1]["column"] = json!(1);
    assert!(edit(invalid, document["revision"].clone()).is_err());
    assert!(edit(operations, json!(999)).is_err());
    assert_eq!(save(&document), source);
    let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":updated["document"]["revision"],"receipt":updated["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], document["hash"]);
    assert_eq!(save(&undone["document"]), source);
}

#[test]
fn vector_native_curves_and_straight_points_reopen_and_undo_without_source_changes() {
    let source = execute_request(json!({"op":"export","deck":deck(json!([polygon()]))})).unwrap()["base64"].clone();
    let document = open(&source);
    let original = document["deck"]["slides"][0]["elements"][0].clone();
    for path in [curve(), json!({"commands":[{"op":"move","point":[0,0]},{"op":"line","point":[1,0]},{"op":"line","point":[1,1]},{"op":"line","point":[0,1]},{"op":"close"}]})] {
        let element = execute_request(json!({"op":"edit_vector","element":original,"path":path})).unwrap();
        let updated = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/slides/0/elements/0","value":element}]}})).unwrap();
        assert_eq!(open(&save(&updated["document"]))["deck"]["slides"][0]["elements"][0], element);
        let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":updated["document"]["revision"],"receipt":updated["receipt"]})).unwrap();
        assert_eq!(save(&undone["document"]), source);
    }
    assert_eq!(save(&document), source);
}

#[test]
fn cell_helper_rejects_invalid_values_and_preserves_caps_and_noops() {
    let original = canonical(table());
    for (row, column, text) in [(0,1,"follower".into()),(12,0,"outside".into()),(0,8,"outside".into()),(0,0,"x".repeat(201)),(0,0,"bad\rtext".into()),(0,0,"\0".into())] {
        let parsed = serde_json::from_value(original.clone()).unwrap();
        assert!(aislide_core::table_format::replace_cell_text(parsed, row, column, text).is_err());
    }
    assert_eq!(execute_request(json!({"op":"set_table_cell_text","element":original,"row":0,"column":0,"text":"Header"})).unwrap(), original);
    let mut maximum = table(); maximum["rows"] = json!(vec![vec![""; 8]; 12]); maximum.as_object_mut().unwrap().remove("format");
    let changed = execute_request(json!({"op":"set_table_cell_text","element":maximum,"row":11,"column":7,"text":"\u{1f600}".repeat(200)})).unwrap();
    assert_eq!(changed["rows"][11][7].as_str().unwrap().chars().count(),200);
    maximum["rows"].as_array_mut().unwrap().push(json!(vec![""; 8]));
    assert!(execute_request(json!({"op":"set_table_cell_text","element":maximum,"row":0,"column":0,"text":""})).is_err());
}

#[test]
fn vector_helper_rejects_locked_malformed_nonfinite_and_unknown_inputs() {
    let original = polygon();
    let mut locked = original.clone(); locked["visual"]["locked"] = json!(true);
    let mut bad_bounds = original.clone(); bad_bounds["x"] = json!(-1);
    let mut outside = original.clone(); outside["x"] = json!(4096);
    for element in [locked, bad_bounds, outside, table()] {
        assert!(execute_request(json!({"op":"edit_vector","element":element,"path":curve()})).is_err());
    }
    for path in [json!({"commands":[]}), json!({"commands":[{"op":"move","point":[0,0]},{"op":"cubic","control1":[0,0],"point":[1,1]},{"op":"close"}]}), json!({"commands":[{"op":"move","point":[0,0]},{"op":"quadratic","control":[2,0],"point":[1,1]},{"op":"close"}]})] {
        assert!(execute_request(json!({"op":"edit_vector","element":original,"path":path})).is_err());
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut path: aislide_core::vector::VectorPath = serde_json::from_value(curve()).unwrap();
        path.commands[1] = aislide_core::vector::PathCommand::Quadratic { control: [value,0.0], point: [1.0,1.0] };
        assert!(aislide_core::vector::edit_path(serde_json::from_value(original.clone()).unwrap(), path).is_err());
    }
    let mut unknown = original.clone(); unknown["unknown"] = json!(true);
    for request in [json!({"op":"edit_vector","element":unknown,"path":curve()}), json!({"op":"edit_vector","element":original,"path":curve(),"unknown":true}), json!({"op":"set_table_cell_text","element":table(),"row":0,"column":0,"text":"ok","unknown":true})] {
        assert!(execute_request(request).is_err());
    }
}