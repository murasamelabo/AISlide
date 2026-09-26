use aislide_core::{editing, execute_request};
use serde_json::{json, Value};

fn canonical_element(value: Value) -> Value {
    serde_json::to_value(serde_json::from_value::<aislide_core::model::Element>(value).unwrap()).unwrap()
}

fn picture() -> Value {
    use base64::{Engine, engine::general_purpose::STANDARD};
    execute_request(json!({"op":"create_asset","id":"picture","base64":STANDARD.encode("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"2\" height=\"2\"><rect width=\"2\" height=\"2\" fill=\"red\"/></svg>"),"mime_type":"image/svg+xml","alt":"Synthetic square","size":40})).unwrap()
}

fn all_elements() -> Vec<Value> {
    let shape = json!({"type":"shape","id":"shape","x":40,"y":40,"width":200,"height":100,"preset":"roundRect","fill":"087F73","stroke":"202525","stroke_width":3,"rotation":15,"text":"Label","font_size":24,"color":"FFFFFF","bold":true,
        "format":{"alignment":"center","vertical":"middle"},"visual":{"flip_h":true,"opacity":0.7,"shadow":{"color":"000000","opacity":0.3,"blur":4,"distance":2,"angle":45},"adjustments":[{"name":"adj","value":10000}]}});
    let table = json!({"type":"table","id":"table","x":40,"y":40,"width":400,"height":160,"rows":[["Header",""],["One","Two"]],"font_size":20,
        "format":{"column_widths":{"unit":"absolute","values":[100,300]},"row_heights":{"unit":"absolute","values":[60,100]},"merges":[{"row":0,"column":0,"row_span":1,"col_span":2}],
        "cells":[{"row":1,"column":0,"style":{"fill":"F3F5F7","outline":{"color":"202525","width":2},"padding":{"left":4,"right":4,"top":2,"bottom":2},"text_style":{"font_size":18,"bold":true},"text_format":{"paragraphs":[{"alignment":"right","runs":[{"text":"One","style":{"font_size":16,"italic":true}}]}]}}}]}});
    let mut relative_table = table.clone();
    relative_table["id"] = json!("relative-table");
    relative_table["format"]["column_widths"] = json!({"unit":"relative","values":[1,3]});
    relative_table["format"]["row_heights"] = json!({"unit":"relative","values":[3,5]});
    let mut child = text();
    child["id"] = json!("nested-text"); child["width"] = json!(200);
    let mut child_table = table.clone();
    child_table["id"] = json!("nested-table"); child_table["x"] = json!(180); child_table["y"] = json!(130);
    vec![text(), json!({"type":"rect","id":"rect","x":40,"y":40,"width":200,"height":100,"fill":"087F73"}), shape,
        json!({"type":"polygon","id":"polygon","x":40,"y":40,"width":200,"height":100,"points":[[0,0],[1,0],[0.5,1]],"fill":"087F73","stroke":"202525","stroke_width":2}),
        table, relative_table,
        json!({"type":"chart","id":"chart","x":40,"y":40,"width":400,"height":200,"kind":"line","categories":["First","Second"],"series":[{"name":"Synthetic","values":[2,4],"color":"087F73"}],"options":{"legend":"top","primary_axis":{"min":0,"max":5},"data_labels":{"show_value":true}}}),
        picture(),
        json!({"type":"connector","id":"connector","x":40,"y":40,"width":200,"height":100,"color":"087F73","stroke_width":2,"arrow":true,"start":{"element_id":"rect","site":3},"end":{"element_id":"shape","site":1}}),
        json!({"type":"group","id":"group","x":40,"y":40,"width":600,"height":300,"view_width":600,"view_height":300,"children":[
            {"type":"group","id":"nested-group","x":20,"y":20,"width":200,"height":100,"view_width":400,"view_height":200,"children":[child]},child_table]})]
}

fn populated() -> Value {
    execute_request(request(&original(), json!([{"op":"add_elements","slide_id":"slide-1","elements":all_elements()}]))).unwrap()["document"].clone()
}

fn element<'a>(document: &'a Value, id: &str) -> &'a Value {
    document["deck"]["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["id"] == id).unwrap()
}

fn reject_atomic(before: &Value, operation: Value, message: &str) {
    let operations = json!([{"op":"set_slide_background","slide_id":"slide-1","color":"ABCDEF"},operation]);
    let parsed: Vec<aislide_core::authoring_batch::Operation> = serde_json::from_value(operations.clone()).expect("negative case must parse as real operations");
    let document: aislide_core::document::Document = serde_json::from_value(before.clone()).unwrap();
    let snapshot = serde_json::to_value(&document).unwrap();
    let error = aislide_core::authoring_batch::apply_operations(&document, document.revision, &document.hash, &parsed).err().expect("invalid batch must fail").to_string();
    assert!(error.contains(message), "expected {message:?}, got {error}");
    assert_eq!(serde_json::to_value(&document).unwrap(), snapshot);
    let error = execute_request(request(before, operations)).unwrap_err().to_string();
    assert!(error.contains(message), "expected {message:?}, got {error}");
    let valid = execute_request(request(before, json!([{"op":"set_slide_background","slide_id":"slide-1","color":"ABCDEF"}]))).unwrap();
    assert_eq!(valid["document"]["revision"].as_u64(), Some(document.revision + 1));
}

fn original() -> Value { serde_json::to_value(editing::create("batch".into(), "Synthetic batch".into()).unwrap()).unwrap() }

fn request(document: &Value, operations: Value) -> Value {
    json!({"op":"apply_operations","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":operations})
}

fn managed_fixture(count: usize) -> Value {
    let mut deck = original()["deck"].clone();
    let seed = deck["slides"][0].clone();
    deck["slides"] = json!((1..=count).map(|index| {
        let mut slide = seed.clone();
        slide["id"] = json!(format!("slide-{index}"));
        slide["title"] = json!(format!("Synthetic slide {index}"));
        slide
    }).collect::<Vec<_>>());
    execute_request(json!({"op":"new_document","id":"managed-fixture","deck":deck})).unwrap()
}

fn managed_part() -> Value {
    json!({"version":1,"preset":"list-horizontal/balanced","title":"Synthetic steps","subtitle":"Synthetic example","data":{"kind":"items","items":[{"label":"Check"},{"label":"Act"}]}})
}

fn managed_graph() -> Value {
    json!({"version":1,"title":"Synthetic path","subtitle":"Synthetic example","nodes":[
        {"id":"client","label":"Client","x":48,"y":160,"width":200,"height":96},
        {"id":"api","label":"API","x":560,"y":256,"width":220,"height":112}
    ],"edges":[{"id":"request","source":"client","target":"api","source_port":"right","target_port":"left","route":"straight"}]})
}

fn managed_layout() -> Value {
    json!({"x":40,"y":100,"width":1152,"height":424,"show_title":false})
}

fn sequential_managed(document: &Value, operation: &Value) -> Value {
    let mut single = operation.clone();
    single["document"] = document.clone();
    single["expected_revision"] = document["revision"].clone();
    match operation["op"].as_str().unwrap() {
        "add_part" => single["op"] = json!("insert_part"),
        "add_graph" if operation.get("layout").is_some() => {
            single["op"] = json!("insert_part");
            single.as_object_mut().unwrap().remove("layout");
            single["spec"] = json!({"version":1,"preset":"diagram/custom","title":operation["spec"]["title"],"subtitle":operation["spec"]["subtitle"],"data":{"kind":"diagram","graph":operation["spec"]},"layout":operation["layout"]});
        }
        "add_graph" => single["op"] = json!("insert_graph"),
        "update_part" | "update_graph" => {}
        _ => panic!("expected managed operation"),
    }
    execute_request(single).unwrap()["document"].clone()
}

fn assert_managed_history(before: &Value, changed: &Value) {
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
    for field in ["hash", "deck", "parts", "sources", "bindings", "origin", "report"] {
        assert_eq!(undone["document"][field], before[field], "undo {field}");
    }
    let redone = execute_request(json!({"op":"undo_transaction","document":undone["document"],"expected_revision":undone["document"]["revision"],"receipt":undone["receipt"]})).unwrap();
    for field in ["hash", "deck", "parts", "sources", "bindings", "origin", "report"] {
        assert_eq!(redone["document"][field], changed["document"][field], "redo {field}");
    }
}

#[test]
fn roundtrip_metadata_batch_matches_dedicated_tools_and_keeps_one_undo() {
    let authored = execute_request(request(&managed_fixture(2), json!([
        {"op":"add_elements","slide_id":"slide-1","elements":all_elements()},
        {"op":"add_elements","slide_id":"slide-2","elements":all_elements()}
    ]))).unwrap()["document"].clone();
    let exported = execute_request(json!({"op":"export_presentation","document":authored})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"metadata-native","base64":exported["base64"]})).unwrap()["document"].clone();
    let operations = json!([
        {"op":"update_notes","slide_id":"slide-1","notes":"Synthetic speaker notes\n\u{65e5}\u{672c}\u{8a9e}"},
        {"op":"set_table_headers","slide_id":"slide-1","element_id":"table","policy":"first_row"},
        {"op":"set_accessibility","slide_id":"slide-1","element_id":"picture","metadata":{"title":"Synthetic picture","description":"Descriptive alternative text","decorative":false}},
        {"op":"update_notes","slide_id":"slide-2","notes":"Second slide notes"},
        {"op":"set_table_headers","slide_id":"slide-2","element_id":"nested-table","policy":"both"},
        {"op":"set_accessibility","slide_id":"slide-2","element_id":"shape","metadata":{"description":"Synthetic shape"}}
    ]);
    for before in [authored, opened] {
        let changed = execute_request(request(&before, operations.clone())).unwrap();
        assert_eq!(changed["document"]["revision"].as_u64(), before["revision"].as_u64().map(|revision| revision + 1));
        let mut sequential = before.clone();
        for operation in operations.as_array().unwrap() {
            let mut input = operation.clone();
            if input["op"] == "update_notes" {
                let index = sequential["deck"]["slides"].as_array().unwrap().iter().position(|slide| slide["id"] == input["slide_id"]).unwrap();
                input = json!({"op":"transaction","document":sequential,"transaction":{"expected_revision":sequential["revision"],"expected_hash":sequential["hash"],"operations":[{"op":"replace","path":format!("/deck/slides/{index}/notes"),"value":operation["notes"]}]}});
            } else {
                input["document"] = sequential.clone();
                input["expected_revision"] = sequential["revision"].clone();
            }
            sequential = execute_request(input).unwrap()["document"].clone();
        }
        assert_eq!(changed["document"]["deck"], sequential["deck"]);
        assert_eq!(changed["document"]["hash"], sequential["hash"]);
        assert_eq!(element(&changed["document"], "picture")["alt"], "Descriptive alternative text");
        assert_eq!(changed["document"]["deck"]["slides"][1]["review"]["table_headers"]["nested-table"], "both");
        assert_managed_history(&before, &changed);
        let no_op = execute_request(request(&changed["document"], operations.clone())).unwrap();
        assert_eq!(no_op["document"], changed["document"]);
        assert!(no_op["receipt"].is_null());
        let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        let reopened = execute_request(json!({"op":"open_presentation","id":"metadata-reopened","base64":saved["base64"]})).unwrap();
        assert_eq!(reopened["document"]["deck"]["slides"][0]["notes"], "Synthetic speaker notes\n\u{65e5}\u{672c}\u{8a9e}");
        assert_eq!(element(&reopened["document"], "picture")["alt"], "Descriptive alternative text");
    }
}

#[test]
fn roundtrip_metadata_batch_invalid_targets_and_notes_are_atomic() {
    let before = populated();
    for operation in [
        json!({"op":"set_table_headers","slide_id":"slide-1","element_id":"text","policy":"first_row"}),
        json!({"op":"set_accessibility","slide_id":"slide-1","element_id":"missing","metadata":{"description":"No target"}}),
        json!({"op":"update_notes","slide_id":"slide-1","notes":"x".repeat(8001)}),
        json!({"op":"update_notes","slide_id":"slide-1","notes":"bad\0notes"}),
    ] { reject_atomic(&before, operation, "operation 2"); }
    let with_rich_notes = execute_request(json!({"op":"update_rich_notes","document":before,"expected_revision":before["revision"],"slide_id":"slide-1","paragraphs":[{"runs":[{"text":"Rich notes","style":{"bold":true}}]}]})).unwrap()["document"].clone();
    assert!(execute_request(request(&with_rich_notes, json!([{"op":"update_notes","slide_id":"slide-1","notes":"Different plain notes"}]))).is_err());
}

#[test]
fn retest_graph_title_visibility_cannot_be_reenabled_by_default_layout() {
    let mut graph = managed_graph();
    graph["title"] = json!("Hidden graph heading");
    graph["show_title"] = json!(false);
    for layout in [None, Some(json!({"x":64,"y":144,"width":1152,"height":512})), Some(json!({"x":64,"y":144,"width":1152,"height":512,"show_title":true})), Some(json!({"x":64,"y":144,"width":1152,"height":512,"show_title":false}))] {
        let mut operation = json!({"op":"add_graph","slide_id":"slide-1","id":"flow","spec":graph});
        if let Some(layout) = layout { operation["layout"] = layout; }
        let result = execute_request(request(&original(), json!([operation]))).unwrap();
        let children = result["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap();
        assert!(children.iter().all(|element| element["text"] != "Hidden graph heading"), "A hidden graph title must remain hidden");
    }
    let before = original();
    let result = execute_request(json!({"op":"insert_graph","document":before,"expected_revision":before["revision"],"slide_id":"slide-1","id":"flow","spec":graph})).unwrap();
    assert!(result["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().iter().all(|element| element["text"] != "Hidden graph heading"));
}

#[test]
fn managed_batch_twenty_one_roots_match_sequential_inserts_on_thirty_nine_slides() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut deck = managed_fixture(39)["deck"].clone();
    let source = execute_request(json!({"op":"ingest","input":{"name":"synthetic.csv","format":"csv","base64":STANDARD.encode("label,value\nAlpha,28\n")}})).unwrap();
    let mut source_text = text(); source_text["text"] = json!("Alpha"); source_text["format"] = json!({});
    deck["slides"][38]["elements"] = json!([source_text]);
    let binding = json!({"slide_id":"slide-39","element_id":"text","field":"/font_size","source_id":source["id"],"source_sha256":source["sha256"],"locator":source["tables"][0]["locators"][0][1],"raw_value":source["tables"][0]["rows"][0][1],"value":28,"transform":"strict_numeric","stale":false});
    let before = execute_request(json!({"op":"new_document","id":"managed-equality","deck":deck,"sources":[source],"bindings":[binding]})).unwrap();
    let operations: Vec<_> = (0..21).map(|index| {
        let mut operation = json!({"op":if index < 17 {"add_part"} else {"add_graph"},"slide_id":format!("slide-{}",index+1),"id":format!("root-{index}"),"spec":if index < 17 {managed_part()} else {managed_graph()}});
        if index % 2 == 0 {
            if index < 17 { operation["spec"]["layout"] = managed_layout(); }
            else { operation["layout"] = managed_layout(); }
        }
        operation
    }).collect();
    let changed = execute_request(request(&before, json!(operations))).unwrap();
    let sequential = operations.iter().fold(before.clone(), |document, operation| sequential_managed(&document, operation));
    assert_eq!(changed["document"]["revision"], 1);
    assert_eq!(sequential["revision"], 21);
    for field in ["hash", "deck", "parts", "sources", "bindings"] {
        assert_eq!(changed["document"][field], sequential[field], "sequential {field}");
    }
    let parts = changed["document"]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 21);
    assert_eq!(parts.iter().filter(|part| part["spec"]["preset"] == "diagram/custom").count(), 4);
    assert!(parts.iter().all(|part| part["stale"] == false));
    let slides = changed["document"]["deck"]["slides"].as_array().unwrap();
    assert_eq!(slides.len(), 39);
    for (index, operation) in operations.iter().enumerate() {
        assert_eq!(slides[index]["elements"].as_array().unwrap().len(), 1);
        let factory = execute_request(json!({"op":"create_part","id":operation["id"],"spec":parts[index]["spec"]})).unwrap();
        assert_eq!(slides[index]["elements"][0], factory, "root frame, style and content {index}");
    }
    assert_eq!(&slides[21..], &before["deck"]["slides"].as_array().unwrap()[21..]);
    assert_managed_history(&before, &changed);
    let mut updated_part = parts[0]["spec"].clone(); updated_part["data"]["items"][0]["label"] = json!("Inspect");
    let stale = execute_request(request(&changed["document"], json!([
        {"op":"update_part","slide_id":"slide-1","id":"root-0","spec":updated_part},
        {"op":"set_text_style","slide_id":"slide-39","ids":["text"],"style":{"font_size":30}}
    ]))).unwrap();
    assert_eq!(stale["document"]["sources"], before["sources"]);
    assert_eq!(stale["document"]["bindings"][0]["stale"], true);
    assert_eq!(stale["document"]["parts"][0]["stale"], false);
    assert!(execute_request(json!({"op":"export_presentation","document":stale["document"]})).unwrap_err().to_string().contains("source bindings are stale"));
    assert_managed_history(&changed["document"], &stale);
}

#[test]
fn managed_batch_add_then_update_preserves_default_and_explicit_layouts() {
    for explicit in [false, true] {
        let before = managed_fixture(2);
        let mut part = managed_part();
        if explicit { part["layout"] = managed_layout(); }
        let mut graph_add = json!({"op":"add_graph","slide_id":"slide-2","id":"graph","spec":managed_graph()});
        if explicit { graph_add["layout"] = managed_layout(); }
        let mut updated_part = part.clone(); updated_part["data"]["items"][0]["label"] = json!("Inspect");
        let mut updated_graph = managed_graph(); updated_graph["nodes"][0]["label"] = json!("Caller");
        let operations = vec![
            json!({"op":"add_part","slide_id":"slide-1","id":"steps","spec":part}), graph_add,
            json!({"op":"update_part","slide_id":"slide-1","id":"steps","spec":updated_part}),
            json!({"op":"update_graph","slide_id":"slide-2","id":"graph","spec":updated_graph}),
        ];
        let inserted = execute_request(request(&before, json!(&operations[..2]))).unwrap();
        let changed = execute_request(request(&before, json!(operations))).unwrap();
        let sequential = operations.iter().fold(before.clone(), |document, operation| sequential_managed(&document, operation));
        for field in ["hash", "deck", "parts"] { assert_eq!(changed["document"][field], sequential[field], "{field}"); }
        assert_eq!(changed["document"]["revision"], 1);
        for index in 0..2 {
            assert_eq!(changed["document"]["parts"][index]["stale"], false);
            assert_eq!(changed["document"]["parts"][index]["spec"]["layout"], inserted["document"]["parts"][index]["spec"]["layout"]);
            for field in ["x", "y", "width", "height", "view_width", "view_height"] {
                assert_eq!(changed["document"]["deck"]["slides"][index]["elements"][0][field], inserted["document"]["deck"]["slides"][index]["elements"][0][field], "{field}");
            }
        }
        assert_eq!(changed["document"]["parts"][0]["spec"]["data"]["items"][0]["label"], "Inspect");
        assert_eq!(changed["document"]["parts"][1]["spec"]["data"]["graph"]["nodes"][0]["label"], "Caller");
        assert_managed_history(&before, &changed);
    }
}

#[test]
fn managed_batch_noop_nonlast_updates_keep_root_and_metadata_order() {
    let before = managed_fixture(4);
    let operations: Vec<_> = (0..4).map(|index| json!({"op":if index % 2 == 0 {"add_part"} else {"add_graph"},"slide_id":format!("slide-{}",index+1),"id":format!("root-{index}"),"spec":if index % 2 == 0 {managed_part()} else {managed_graph()}})).collect();
    let inserted = execute_request(request(&before, json!(operations))).unwrap();
    let mut marker = text(); marker["id"] = json!("marker"); marker["y"] = json!(650); marker["height"] = json!(60);
    let marked = execute_request(request(&inserted["document"], json!([
        {"op":"add_elements","slide_id":"slide-1","elements":[marker]},
        {"op":"add_elements","slide_id":"slide-2","elements":[marker]}
    ]))).unwrap();
    let before = &marked["document"];
    let unchanged = execute_request(request(before, json!([
        {"op":"update_part","slide_id":"slide-1","id":"root-0","spec":managed_part()},
        {"op":"update_graph","slide_id":"slide-2","id":"root-1","spec":managed_graph()}
    ]))).unwrap();
    assert_eq!(unchanged["document"], *before);
    assert!(unchanged["receipt"].is_null());
    assert_eq!(unchanged["changes"], json!([]));
}

#[test]
fn managed_batch_late_invalid_specs_and_root_or_nested_collisions_are_atomic() {
    let before = managed_fixture(2);
    let mut invalid_part = managed_part(); invalid_part["data"]["items"] = json!([]);
    let mut invalid_graph = managed_graph(); invalid_graph["edges"][0]["target"] = json!("missing");
    let mut off_slide = managed_part(); off_slide["layout"] = managed_layout(); off_slide["layout"]["x"] = json!(1280);
    let mut zero_width = managed_part(); zero_width["layout"] = managed_layout(); zero_width["layout"]["width"] = json!(0);
    let mut invalid_graph_layout = managed_layout(); invalid_graph_layout["y"] = json!(720);
    for invalid in [
        json!({"op":"add_part","slide_id":"slide-2","id":"invalid","spec":invalid_part}),
        json!({"op":"add_graph","slide_id":"slide-2","id":"invalid","spec":invalid_graph}),
        json!({"op":"add_part","slide_id":"slide-2","id":"invalid","spec":off_slide}),
        json!({"op":"add_part","slide_id":"slide-2","id":"invalid","spec":zero_width}),
        json!({"op":"add_graph","slide_id":"slide-2","id":"invalid","spec":managed_graph(),"layout":invalid_graph_layout}),
        json!({"op":"add_part","slide_id":"slide-1","id":"steps","spec":managed_part()}),
        json!({"op":"update_graph","slide_id":"slide-1","id":"steps","spec":managed_graph()}),
    ] {
        let operations = json!([{"op":"add_part","slide_id":"slide-1","id":"steps","spec":managed_part()},invalid]);
        let parsed: Vec<aislide_core::authoring_batch::Operation> = serde_json::from_value(operations.clone()).unwrap();
        let document: aislide_core::document::Document = serde_json::from_value(before.clone()).unwrap();
        let snapshot = serde_json::to_value(&document).unwrap();
        assert!(aislide_core::authoring_batch::apply_operations(&document, document.revision, &document.hash, &parsed).is_err());
        assert_eq!(serde_json::to_value(&document).unwrap(), snapshot);
        assert!(execute_request(request(&before, operations)).is_err());
    }
    let seeded = execute_request(request(&before, json!([
        {"op":"add_part","slide_id":"slide-1","id":"steps","spec":managed_part()},
        {"op":"add_graph","slide_id":"slide-2","id":"graph","spec":managed_graph()}
    ]))).unwrap();
    for invalid in [
        json!({"op":"update_part","slide_id":"slide-1","id":"steps","spec":invalid_part}),
        json!({"op":"update_part","slide_id":"slide-1","id":"steps","spec":off_slide}),
        json!({"op":"update_part","slide_id":"slide-1","id":"steps","spec":zero_width}),
        json!({"op":"update_graph","slide_id":"slide-2","id":"graph","spec":invalid_graph}),
    ] {
        reject_atomic(&seeded["document"], invalid, "");
    }
    for (kind, spec) in [("add_part", managed_part()), ("add_graph", managed_graph())] {
        let added = execute_request(request(&before, json!([{"op":kind,"slide_id":"slide-1","id":"managed","spec":spec}]))).unwrap();
        let root = element(&added["document"], "managed");
        for id in [root["id"].clone(), root["children"][0]["id"].clone()] {
            let mut collision = text(); collision["id"] = id;
            reject_atomic(&added["document"], json!({"op":"add_elements","slide_id":"slide-1","elements":[collision]}), "geometry or ID");
            let container = json!({"type":"group","id":"container","x":0,"y":0,"width":1280,"height":720,"view_width":1280,"view_height":720,"children":[collision]});
            let occupied = execute_request(request(&before, json!([{"op":"add_elements","slide_id":"slide-1","elements":[container]}]))).unwrap();
            reject_atomic(&occupied["document"], json!({"op":kind,"slide_id":"slide-1","id":"managed","spec":spec}), "geometry or ID");
        }
        reject_atomic(&added["document"], json!({"op":kind,"slide_id":"slide-1","id":"managed","spec":spec}), "identity already exists");
    }
}

#[test]
fn managed_batch_unknown_fields_and_malformed_layouts_reject_during_parsing() {
    let before = original();
    for (kind, spec) in [("add_part", managed_part()), ("update_part", managed_part()), ("add_graph", managed_graph()), ("update_graph", managed_graph())] {
        let valid = json!({"op":kind,"slide_id":"slide-1","id":"managed","spec":spec});
        serde_json::from_value::<aislide_core::authoring_batch::Operation>(valid.clone()).unwrap();
        for pointer in ["", "/spec"] {
            let mut invalid = valid.clone(); invalid.pointer_mut(pointer).unwrap()["unexpected"] = json!(true);
            assert!(serde_json::from_value::<aislide_core::authoring_batch::Operation>(invalid.clone()).unwrap_err().to_string().contains("unknown field"));
            assert!(execute_request(request(&before, json!([invalid]))).unwrap_err().to_string().contains("unknown field"));
        }
        let layout_pointer = if kind.ends_with("part") { "/spec" } else { "" };
        for layout in [json!({"x":0,"y":0,"width":1152,"height":512,"unexpected":true}), json!({"x":0,"y":0,"width":"wide","height":512}), json!({"x":0,"y":0,"height":512})] {
            let mut invalid = valid.clone(); invalid.pointer_mut(layout_pointer).unwrap()["layout"] = layout;
            assert!(serde_json::from_value::<aislide_core::authoring_batch::Operation>(invalid.clone()).is_err(), "{kind}");
            assert!(execute_request(request(&before, json!([invalid]))).is_err(), "{kind}");
        }
    }
}

#[test]
fn managed_batch_generic_child_edits_cannot_be_overwritten_by_managed_updates() {
    for (add, update, spec) in [("add_part", "update_part", managed_part()), ("add_graph", "update_graph", managed_graph())] {
        let before = execute_request(request(&original(), json!([{"op":add,"slide_id":"slide-1","id":"managed","spec":spec}]))).unwrap()["document"].clone();
        let child = element(&before, "managed")["children"].as_array().unwrap().iter().find(|child| child["type"] == "text").unwrap();
        let edit = json!({"op":"set_text_style","slide_id":"slide-1","ids":[child["id"]],"style":{"color":"CC0000"}});
        let operation = json!({"op":update,"slide_id":"slide-1","id":"managed","spec":spec});
        let error = execute_request(request(&before, json!([edit,operation]))).unwrap_err().to_string();
        assert!(error.contains("operation 2") && error.contains("stale after an earlier edit"), "{error}");
        let edited = execute_request(request(&before, json!([edit]))).unwrap();
        assert_eq!(edited["document"]["parts"][0]["stale"], true);
        reject_atomic(&edited["document"], operation, "metadata is stale");
        let unchanged = execute_request(request(&before, json!([{"op":update,"slide_id":"slide-1","id":"managed","spec":spec}]))).unwrap();
        assert_eq!(unchanged["document"], before);
        assert!(unchanged["receipt"].is_null());
    }
}

#[test]
fn managed_batch_locked_or_hidden_roots_and_descendants_fail_closed() {
    for (add, update, spec) in [("add_part", "update_part", managed_part()), ("add_graph", "update_graph", managed_graph())] {
        let added = execute_request(request(&original(), json!([{"op":add,"slide_id":"slide-1","id":"managed","spec":spec}]))).unwrap();
        for flag in ["locked", "hidden"] {
            for suffix in ["", "/children/0"] {
                let protected = execute_request(json!({"op":"transaction","document":added["document"],"transaction":{"expected_revision":1,"expected_hash":added["document"]["hash"],"operations":[
                    {"op":"add","path":format!("/deck/slides/0/elements/0{suffix}/visual"),"value":{flag:true}}
                ]}})).unwrap();
                let before = &protected["document"];
                assert_eq!(before["deck"]["slides"][0]["elements"][0].pointer(&format!("{suffix}/visual/{flag}")), Some(&json!(true)));
                reject_atomic(before, json!({"op":update,"slide_id":"slide-1","id":"managed","spec":spec}), "stale");
                let child_id = element(before, "managed")["children"][0]["id"].clone();
                reject_atomic(before, json!({"op":"set_text_style","slide_id":"slide-1","ids":[child_id],"style":{"color":"CC0000"}}), "unlock and show");
            }
        }
    }
}

#[test]
fn managed_batch_128_slides_coalesce_patches_and_129_parts_reject_atomically() {
    let before = managed_fixture(129);
    let operations: Vec<_> = (1..=128).map(|index| json!({"op":"add_part","slide_id":format!("slide-{index}"),"id":format!("part-{index}"),"spec":managed_part()})).collect();
    let document: aislide_core::document::Document = serde_json::from_value(before.clone()).unwrap();
    let parsed: Vec<aislide_core::authoring_batch::Operation> = serde_json::from_value(json!(operations)).unwrap();
    let typed = aislide_core::authoring_batch::apply_operations(&document, document.revision, &document.hash, &parsed).unwrap();
    let changed = execute_request(request(&before, json!(operations))).unwrap();
    assert_eq!(changed, serde_json::to_value(typed).unwrap());
    assert_eq!(changed["document"]["revision"], 1);
    assert_eq!(changed["document"]["parts"].as_array().unwrap().len(), 128);
    assert!(changed["document"]["parts"].as_array().unwrap().iter().all(|part| part["stale"] == false));
    for index in 0..128 { assert_eq!(changed["document"]["deck"]["slides"][index]["elements"].as_array().unwrap().len(), 1); }
    assert_eq!(changed["document"]["deck"]["slides"][128], before["deck"]["slides"][128]);
    assert_eq!(serde_json::to_value(&document).unwrap(), before);
    assert_managed_history(&before, &changed);
    let full = &changed["document"];
    reject_atomic(full, json!({"op":"add_part","slide_id":"slide-129","id":"part-129","spec":managed_part()}), "more than 128 metadata parts");
    let almost = execute_request(request(&before, json!(&operations[..127]))).unwrap();
    let over_budget = json!([
        operations[127], {"op":"add_graph","slide_id":"slide-129","id":"graph-129","spec":managed_graph()}
    ]);
    let almost_document: aislide_core::document::Document = serde_json::from_value(almost["document"].clone()).unwrap();
    let parsed: Vec<aislide_core::authoring_batch::Operation> = serde_json::from_value(over_budget.clone()).unwrap();
    let error = aislide_core::authoring_batch::apply_operations(&almost_document, almost_document.revision, &almost_document.hash, &parsed).err().expect("129th metadata part must fail").to_string();
    assert!(error.contains("operation 2") && error.contains("more than 128 metadata parts"), "{error}");
    assert_eq!(serde_json::to_value(&almost_document).unwrap(), almost["document"]);
    let error = execute_request(request(&almost["document"], over_budget)).unwrap_err().to_string();
    assert!(error.contains("operation 2") && error.contains("more than 128 metadata parts"), "{error}");
    let completed = execute_request(request(&almost["document"], json!([operations[127]]))).unwrap();
    for field in ["hash", "deck", "parts"] { assert_eq!(completed["document"][field], full[field], "{field}"); }
}

#[test]
fn managed_batch_native_updates_preserve_untouched_package_parts_and_exact_undo() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use aislide_core::package::Package;
    let before = managed_fixture(3);
    let mut part = managed_part(); part["layout"] = managed_layout();
    let inserted = execute_request(request(&before, json!([
        {"op":"add_part","slide_id":"slide-1","id":"steps","spec":part},
        {"op":"add_elements","slide_id":"slide-3","elements":[text()]}
    ]))).unwrap();
    let exported = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let mut package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide3.xml").unwrap().replace("</p:sld>", "<p:extLst><p:ext uri=\"urn:test:managed-batch\"><b:opaque xmlns:b=\"urn:test:managed-batch\" value=\"keep\"/></p:ext></p:extLst></p:sld>");
    package.replace_part("ppt/slides/slide3.xml", xml.into_bytes()).unwrap();
    let input_bytes = package.save().unwrap();
    let original_parts = Package::open(input_bytes.clone()).unwrap().parts().clone();
    let bytes = STANDARD.encode(input_bytes);
    let opened = execute_request(json!({"op":"open_presentation","id":"native-managed-batch","base64":bytes})).unwrap();
    let before = &opened["document"];
    assert_eq!(before["parts"][0]["stale"], false);
    assert_eq!(execute_request(json!({"op":"export_presentation","document":before})).unwrap()["base64"], bytes);
    part["data"]["items"][0]["label"] = json!("Inspect");
    let changed = execute_request(request(before, json!([
        {"op":"update_part","slide_id":"slide-1","id":"steps","spec":part},
        {"op":"add_graph","slide_id":"slide-2","id":"graph","spec":managed_graph(),"layout":managed_layout()}
    ]))).unwrap();
    assert_eq!(changed["document"]["origin"], before["origin"]);
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let saved_package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    for (path, content) in &original_parts {
        if path == "ppt/slides/slide1.xml" || path == "ppt/slides/slide2.xml" || path.starts_with("customXml/") { continue; }
        assert!(saved_package.parts().get(path) == Some(content), "untouched native part {path}");
    }
    let reopened = execute_request(json!({"op":"open_presentation","id":"native-managed-reopened","base64":saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["parts"].as_array().unwrap().len(), 2);
    for index in 0..2 {
        assert_eq!(reopened["document"]["parts"][index]["stale"], false);
        assert_eq!(reopened["document"]["parts"][index]["spec"], changed["document"]["parts"][index]["spec"]);
    }
    let mut graph = managed_graph(); graph["nodes"][0]["x"] = json!(64);
    let updated = execute_request(request(&reopened["document"], json!([{"op":"update_graph","slide_id":"slide-2","id":"graph","spec":graph}]))).unwrap();
    assert_eq!(updated["document"]["parts"][1]["spec"]["layout"], reopened["document"]["parts"][1]["spec"]["layout"]);
    assert_eq!(updated["document"]["parts"][1]["stale"], false);
    execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
    assert_managed_history(before, &changed);
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], bytes);
}

#[test]
fn managed_batch_inserts_parts_and_graphs_with_metadata_and_one_undo() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let before = original();
    let spec = json!({"version":1,"preset":"list-horizontal/balanced","title":"Synthetic steps","data":{"kind":"items","items":[{"label":"Check"},{"label":"Act"}]},"layout":{"x":40,"y":160,"width":1152,"height":424,"show_title":false}});
    let graph = json!({"version":1,"title":"Synthetic path","show_title":false,"nodes":[{"id":"node","label":"Service","x":40,"y":40,"width":200,"height":100}],"edges":[]});
    let added = execute_request(request(&before, json!([
        {"op":"add_part","slide_id":"slide-1","id":"steps","spec":spec},
        {"op":"add_graph","slide_id":"slide-1","id":"graph","spec":graph,"layout":{"x":40,"y":20,"width":1152,"height":512,"show_title":false}}
    ]))).unwrap();
    assert_eq!(added["document"]["revision"], 1);
    let parts = added["document"]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 2);
    assert!(parts.iter().all(|part| part["stale"] == false));
    assert_eq!(parts[0]["element_id"], "steps");
    assert_eq!(parts[1]["spec"]["preset"], "diagram/custom");
    assert_eq!(parts[0]["spec"]["layout"]["y"].as_f64(), Some(160.0));
    let exported = execute_request(json!({"op":"export_presentation","document":added["document"]})).unwrap();
    assert_eq!(&STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()[..2], b"PK");
    let reopened = execute_request(json!({"op":"open_presentation","id":"managed-reopened","base64":exported["base64"]})).unwrap();
    assert!(reopened["document"]["parts"].as_array().unwrap().iter().all(|part| part["stale"] == false));
    let mut changed_spec = spec.clone(); changed_spec["data"]["items"][0]["label"] = json!("Inspect");
    let updated = execute_request(json!({"op":"update_part","document":reopened["document"],"expected_revision":reopened["document"]["revision"],"slide_id":"slide-1","id":"steps","spec":changed_spec})).unwrap();
    assert_eq!(updated["document"]["parts"][0]["spec"]["data"]["items"][0]["label"], "Inspect");
    assert_eq!(updated["document"]["parts"][0]["stale"], false);
    let undone = execute_request(json!({"op":"undo_transaction","document":added["document"],"expected_revision":1,"receipt":added["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], before["hash"]);
}

#[test]
fn typed_authoring_and_slide_import_are_discoverable_without_claiming_office_parity() {
    let capabilities = execute_request(json!({"op":"authoring_capabilities"})).unwrap();
    for operation in ["apply_operations", "import_slides"] { assert!(capabilities["operations"].as_array().unwrap().contains(&json!(operation))); }
    assert_eq!(capabilities["typed_authoring"]["batch_limit"], 128);
    assert_eq!(capabilities["typed_authoring"]["scale_fonts"], false);
    assert_eq!(capabilities["typed_authoring"]["one_undo"], true);
    let operations = capabilities["typed_authoring"]["operations"].as_array().unwrap();
    assert_eq!(operations.len(), 16);
    for operation in ["add_elements", "set_frame", "set_text_style", "set_slide_background", "set_connector", "set_picture_crop", "set_hyperlink", "set_shape_adjustment", "add_picture", "add_part", "update_part", "add_graph", "update_graph", "update_notes", "set_table_headers", "set_accessibility"] {
        assert!(operations.contains(&json!(operation)), "missing {operation}");
    }
    assert_eq!(capabilities["typed_authoring"]["managed_parts_limit"], 128);
    assert_eq!(capabilities["typed_authoring"]["managed_metadata_persisted"], true);
    assert_eq!(capabilities["slide_import"]["source"], "authored_document_only");
    assert_eq!(capabilities["slide_import"]["office_visual_parity"], false);
}

fn text() -> Value {
    json!({"type":"text","id":"text","x":20,"y":20,"width":600,"height":100,"text":"One\nTwo","font_size":28,"color":"202525","bold":false,
        "format":{"paragraphs":[{"alignment":"right","space_after":{"kind":"points","value":600},"runs":[{"text":"One","style":{"italic":true}}]},
        {"bullet":"bullet","bullet_character":"-","runs":[{"text":"Two","style":{"color":"AA0000"}}]}]}})
}

#[test]
fn typed_batch_adds_complete_elements_and_changes_frames_without_scaling_styles() {
    let before = original();
    let result = execute_request(request(&before, json!([
        {"op":"add_elements","slide_id":"slide-1","elements":[text(),
            {"type":"connector","id":"line","x":20,"y":160,"width":400,"height":1,"color":"087F73","stroke_width":2,"arrow":true},
            {"type":"polygon","id":"custom","x":600,"y":100,"width":100,"height":100,"points":[[0,0],[1,0],[0.5,1]],"fill":"087F73","stroke":"202525","stroke_width":1}]},
        {"op":"set_frame","slide_id":"slide-1","id":"text","frame":{"x":80,"y":80,"width":300,"height":60}},
        {"op":"set_frame","slide_id":"slide-1","id":"line","frame":{"x":80,"y":180,"width":700,"height":90}},
        {"op":"set_slide_background","slide_id":"slide-1","color":"F3F5F7"}
    ]))).unwrap();
    assert_eq!(result["document"]["revision"], 1);
    let slide = &result["document"]["deck"]["slides"][0];
    assert_eq!(slide["elements"][0]["font_size"].as_f64(), Some(28.0));
    let expected: aislide_core::model::Element = serde_json::from_value(text()).unwrap();
    assert_eq!(slide["elements"][0]["format"], serde_json::to_value(expected).unwrap()["format"]);
    assert_eq!(slide["elements"][1]["stroke_width"].as_f64(), Some(2.0));
    assert_eq!(slide["background"], "F3F5F7");
    let undone = execute_request(json!({"op":"undo_transaction","document":result["document"],"expected_revision":1,"receipt":result["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], before["hash"]);
}

#[test]
fn typed_batch_partial_text_style_keeps_paragraphs_and_updates_frame_defaults() {
    let before = original();
    let result = execute_request(request(&before, json!([
        {"op":"add_elements","slide_id":"slide-1","elements":[text()]},
        {"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":{"font_size":12,"bold":true}}
    ]))).unwrap();
    let element = &result["document"]["deck"]["slides"][0]["elements"][0];
    assert_eq!(element["font_size"].as_f64(), Some(12.0));
    assert_eq!(element["bold"], true);
    assert_eq!(element["format"]["paragraphs"][0]["alignment"], "right");
    assert_eq!(element["format"]["paragraphs"][0]["space_after"], text()["format"]["paragraphs"][0]["space_after"]);
    assert_eq!(element["format"]["paragraphs"][0]["runs"][0]["style"]["italic"], true);
    assert_eq!(element["format"]["paragraphs"][1]["runs"][0]["style"]["color"], "AA0000");
}

#[test]
fn typed_batch_rejects_late_invalid_changes_and_stale_or_unknown_input() {
    let before = original();
    let operations = json!([
        {"op":"add_elements","slide_id":"slide-1","elements":[text()]},
        {"op":"set_frame","slide_id":"slide-1","id":"text","frame":{"x":0,"y":0,"width":-1,"height":40}}
    ]);
    assert!(execute_request(request(&before, operations)).is_err());
    let mut stale = request(&before, json!([{"op":"set_slide_background","slide_id":"slide-1","color":"000000"}]));
    stale["expected_hash"] = json!("0".repeat(64));
    assert!(execute_request(stale).is_err());
    assert!(execute_request(request(&before, json!([{"op":"execute_shell","command":"ignored"}]))).is_err());
    assert_eq!(before, original());
}

#[test]
fn typed_batch_materialized_paragraphs_preserve_plain_text_format() {
    let before = original();
    let mut plain = text();
    plain["format"] = json!({"alignment":"right","bullet":"numbered","italic":true,"underline":true,"font_family":"Arial","vertical":"middle"});
    let result = execute_request(request(&before, json!([
        {"op":"add_elements","slide_id":"slide-1","elements":[plain]},
        {"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":{"language":"ja-JP","baseline":12000}}
    ]))).unwrap();
    let element = &result["document"]["deck"]["slides"][0]["elements"][0];
    assert_eq!(element["text"], "One\nTwo");
    assert_eq!(element["format"]["vertical"], "middle");
    for paragraph in element["format"]["paragraphs"].as_array().unwrap() {
        assert_eq!(paragraph["alignment"], "right");
        assert_eq!(paragraph["bullet"], "numbered");
        assert_eq!(paragraph["runs"][0]["style"]["language"], "ja-JP");
        assert_eq!(paragraph["runs"][0]["style"]["baseline"], 12000);
    }
}

#[test]
fn typed_batch_full_elements_and_geometry_only_preserve_all_payloads() {
    let before = populated();
    let expected: Vec<_> = all_elements().into_iter().map(canonical_element).collect();
    assert_eq!(before["deck"]["slides"][0]["elements"], json!(expected));
    let operations: Vec<_> = expected.iter().map(|entry| json!({"op":"set_frame","slide_id":"slide-1","id":entry["id"],"frame":{"x":80,"y":90,"width":300,"height":150}})).collect();
    let changed = execute_request(request(&before, json!(operations))).unwrap();
    assert_eq!(changed["document"]["revision"], 2);
    for mut expected in expected {
        let id = expected["id"].as_str().unwrap().to_owned();
        expected["x"] = json!(80.0); expected["y"] = json!(90.0); expected["width"] = json!(300.0); expected["height"] = json!(150.0);
        if id == "table" {
            expected["format"]["column_widths"]["values"] = json!([75.0,225.0]);
            expected["format"]["row_heights"]["values"] = json!([56.25,93.75]);
        }
        if id == "group" {
            expected["view_width"] = json!(300.0); expected["view_height"] = json!(150.0);
            let nested = &mut expected["children"][0];
            nested["x"] = json!(10.0); nested["y"] = json!(10.0); nested["width"] = json!(100.0); nested["height"] = json!(50.0);
            nested["view_width"] = json!(100.0); nested["view_height"] = json!(50.0);
            let text = &mut nested["children"][0];
            text["x"] = json!(5.0); text["y"] = json!(5.0); text["width"] = json!(50.0); text["height"] = json!(25.0);
            let table = &mut expected["children"][1];
            table["x"] = json!(90.0); table["y"] = json!(65.0); table["width"] = json!(200.0); table["height"] = json!(80.0);
            table["format"]["column_widths"]["values"] = json!([50.0,150.0]);
            table["format"]["row_heights"]["values"] = json!([30.0,50.0]);
        }
        assert_eq!(element(&changed["document"], &id), &expected, "{id}");
    }
    let moved = execute_request(request(&before, json!([{"op":"set_frame","slide_id":"slide-1","id":"group","frame":{"x":100,"y":100,"width":600,"height":300}}]))).unwrap();
    assert_eq!(element(&moved["document"], "group")["children"], element(&before, "group")["children"]);
}

#[test]
fn typed_batch_group_resize_respects_descendant_locks_and_visibility() {
    for flag in ["locked", "hidden"] {
        let mut deck = populated()["deck"].clone();
        deck["slides"][0]["elements"][9]["children"][0]["children"][0]["visual"] = json!({flag:true});
        let before = execute_request(json!({"op":"new_document","id":"locked-child","deck":deck})).unwrap();
        reject_atomic(&before, json!({"op":"set_frame","slide_id":"slide-1","id":"group","frame":{"x":40,"y":40,"width":300,"height":150}}), "unlock and show");
    }
}

#[test]
fn typed_batch_frame_and_link_detach_inheritance_only_on_change() {
    let deck = aislide_core::design::assign_layout(editing::create("seed".into(), "Frame".into()).unwrap().deck, "slide-1", "title-content").unwrap();
    let before = execute_request(json!({"op":"new_document","id":"inherited-frame","deck":deck})).unwrap();
    let same = execute_request(request(&before, json!([{"op":"set_frame","slide_id":"slide-1","id":"title","frame":{"x":64,"y":80,"width":1152,"height":120}}]))).unwrap();
    assert_eq!(same["document"], before);
    assert!(same["receipt"].is_null());
    let changed = execute_request(request(&before, json!([{"op":"set_frame","slide_id":"slide-1","id":"title","frame":{"x":80,"y":90,"width":1000,"height":120}}]))).unwrap();
    assert_eq!(element(&changed["document"], "title")["format"]["inherit_layout"], false);
    assert_eq!(element(&changed["document"], "title")["text"], element(&before, "title")["text"]);
    assert_eq!(element(&changed["document"], "title")["font_size"], element(&before, "title")["font_size"]);
    assert_eq!(element(&changed["document"], "body"), element(&before, "body"));
    let same = execute_request(request(&before, json!([{"op":"set_hyperlink","slide_id":"slide-1","id":"title","link":null}]))).unwrap();
    assert_eq!(same["document"], before); assert!(same["receipt"].is_null());
    let linked = execute_request(request(&before, json!([{"op":"set_hyperlink","slide_id":"slide-1","id":"title","link":"https://example.test/"}]))).unwrap();
    let mut expected = element(&before, "title").clone();
    expected["format"]["hyperlink"] = json!("https://example.test/"); expected["format"]["inherit_layout"] = json!(false);
    assert_eq!(element(&linked["document"], "title"), &expected);
    assert_eq!(element(&linked["document"], "body"), element(&before, "body"));
}

#[test]
fn typed_batch_hyperlink_requires_key_but_accepts_explicit_null() {
    let unlinked = execute_request(request(&original(), json!([{"op":"add_elements","slide_id":"slide-1","elements":[text()]}]))).unwrap();
    let linked = execute_request(request(&unlinked["document"], json!([{"op":"set_hyperlink","slide_id":"slide-1","id":"text","link":"https://example.test/"}]))).unwrap();
    let before = &linked["document"];
    assert_eq!(element(before, "text")["format"]["hyperlink"], "https://example.test/");
    let snapshot = before.clone();
    let error = execute_request(request(before, json!([
        {"op":"set_slide_background","slide_id":"slide-1","color":"ABCDEF"},
        {"op":"set_hyperlink","slide_id":"slide-1","id":"text"}
    ]))).unwrap_err().to_string();
    assert!(error.contains("missing field `link`"), "{error}");
    assert_eq!(*before, snapshot);
    let changed = execute_request(request(before, json!([{"op":"set_slide_background","slide_id":"slide-1","color":"ABCDEF"}]))).unwrap();
    assert_eq!(changed["document"]["revision"].as_u64(), Some(before["revision"].as_u64().unwrap() + 1));
    assert_eq!(changed["document"]["deck"]["slides"][0]["background"], "ABCDEF");
    assert_eq!(element(&changed["document"], "text"), element(before, "text"));
    let clear = json!({"op":"set_hyperlink","slide_id":"slide-1","id":"text","link":null});
    let typed: aislide_core::authoring_batch::Operation = serde_json::from_value(clear.clone()).unwrap();
    assert_eq!(serde_json::to_value(typed).unwrap(), clear);
    let cleared = execute_request(request(before, json!([clear]))).unwrap();
    assert!(element(&cleared["document"], "text")["format"]["hyperlink"].is_null());
    assert_eq!(cleared["document"]["hash"], unlinked["document"]["hash"]);
}

#[test]
fn typed_batch_links_clear_and_reject_unsafe_destinations_atomically() {
    let before = populated();
    for link in ["https://example.test/path?q=one&next=two", "http://example.test/", "mailto:reader@example.test"] {
        let linked = execute_request(request(&before, json!([
            {"op":"set_hyperlink","slide_id":"slide-1","id":"text","link":link},
            {"op":"set_hyperlink","slide_id":"slide-1","id":"shape","link":link}
        ]))).unwrap();
        for id in ["text", "shape"] {
            let mut expected = element(&before, id).clone(); expected["format"]["hyperlink"] = json!(link);
            assert_eq!(element(&linked["document"], id), &expected);
        }
        let cleared = execute_request(request(&linked["document"], json!([
            {"op":"set_hyperlink","slide_id":"slide-1","id":"text","link":null},
            {"op":"set_hyperlink","slide_id":"slide-1","id":"shape","link":null}
        ]))).unwrap();
        assert_eq!(cleared["document"]["hash"], before["hash"]);
    }
    for link in ["javascript:alert(1)", "file:///C:/private", "data:text/html,ignored", "https://user:secret@example.test/", "https://example.test/\npath"] {
        reject_atomic(&before, json!({"op":"set_hyperlink","slide_id":"slide-1","id":"text","link":link}), "hyperlink");
    }
    reject_atomic(&before, json!({"op":"set_hyperlink","slide_id":"slide-1","id":"rect","link":null}), "requires a text box or shape");
}

#[test]
fn typed_batch_rounding_preserves_other_visual_properties() {
    let before = populated();
    for value in [0, 25000, 50000] {
        let changed = execute_request(request(&before, json!([{"op":"set_shape_adjustment","slide_id":"slide-1","id":"shape","adjustment":{"name":"adj","value":value}}]))).unwrap();
        let mut expected = element(&before, "shape").clone();
        expected["visual"]["adjustments"] = json!([{"name":"adj","value":value}]);
        assert_eq!(element(&changed["document"], "shape"), &expected);
        let same = execute_request(request(&changed["document"], json!([{"op":"set_shape_adjustment","slide_id":"slide-1","id":"shape","adjustment":{"name":"adj","value":value}}]))).unwrap();
        assert_eq!(same["document"], changed["document"]); assert!(same["receipt"].is_null());
    }
    for adjustment in [json!({"name":"adj","value":-1}), json!({"name":"adj","value":50001}), json!({"name":"other","value":100})] {
        reject_atomic(&before, json!({"op":"set_shape_adjustment","slide_id":"slide-1","id":"shape","adjustment":adjustment}), "adjustment");
    }
    reject_atomic(&before, json!({"op":"set_shape_adjustment","slide_id":"slide-1","id":"rect","adjustment":{"name":"adj","value":100}}), "requires a shape");
}

#[test]
fn typed_batch_connector_settings_routes_and_references_are_atomic() {
    let before = populated();
    let settings = json!({"color":"CF5847","stroke_width":4,"arrow":false,"flip_v":false,"start":{"element_id":"shape","site":2},"end":{"element_id":"picture","site":0},"routing":{"points":[[0,0],[0.25,0],[0.25,1],[1,1]],"start_arrow":true,"dashed":true}});
    let operation = json!({"op":"set_connector","slide_id":"slide-1","id":"connector","connector":settings,"frame":{"x":100,"y":120,"width":300,"height":160}});
    let changed = execute_request(request(&before, json!([operation]))).unwrap();
    let mut expected = element(&before, "connector").clone();
    for (key, value) in settings.as_object().unwrap() { expected[key] = value.clone(); }
    expected["x"] = json!(100.0); expected["y"] = json!(120.0); expected["width"] = json!(300.0); expected["height"] = json!(160.0);
    assert_eq!(element(&changed["document"], "connector"), &canonical_element(expected));
    let cleared = execute_request(request(&changed["document"], json!([{"op":"set_connector","slide_id":"slide-1","id":"connector","connector":{"color":"087F73","stroke_width":2,"arrow":true,"flip_v":true}}]))).unwrap();
    let line = element(&cleared["document"], "connector");
    assert!(line["start"].is_null()); assert!(line["end"].is_null()); assert!(line["routing"].is_null()); assert_eq!(line["flip_v"], true);
    assert_eq!(line["width"].as_f64(), Some(300.0));
    for target in ["missing", "connector", "table", "nested-text", "group"] {
        let mut invalid = operation.clone(); invalid["connector"]["start"]["element_id"] = json!(target);
        reject_atomic(&before, invalid, "connector target");
    }
    for (pointer, value, message) in [
        ("/connector/end/site", json!(4), "connector target"),
        ("/connector/flip_v", json!(true), "without flip_v"),
        ("/connector/stroke_width", json!(0), "connector stroke"),
        ("/connector/routing/points", json!([[0,0],[0.4,0.6],[1,1]]), "connector route"),
        ("/id", json!("text"), "requires a connector")
    ] {
        let mut invalid = operation.clone(); *invalid.pointer_mut(pointer).unwrap() = value;
        reject_atomic(&before, invalid, message);
    }
}

#[test]
fn typed_batch_picture_placement_crop_and_add_are_one_transaction() {
    let before = original();
    let asset = picture();
    let operation = json!({"op":"add_picture","slide_id":"slide-1","id":"photo","base64":asset["base64"],"mime_type":"image/png","alt":"Synthetic placement","frame":{"x":200,"y":100,"width":300,"height":150},"crop":{"left":0.1,"right":0.2,"top":0.05,"bottom":0.15}});
    let added = execute_request(request(&before, json!([operation]))).unwrap();
    assert_eq!(added["document"]["revision"], 1);
    let photo = element(&added["document"], "photo");
    assert_eq!(photo["base64"], asset["base64"]); assert_eq!(photo["alt"], "Synthetic placement");
    for (key, value) in operation["frame"].as_object().unwrap() { assert_eq!(photo[key].as_f64(), value.as_f64()); }
    assert_eq!(photo["crop"], operation["crop"]);
    let changed = execute_request(request(&added["document"], json!([
        {"op":"set_picture_crop","slide_id":"slide-1","id":"photo","crop":{"left":0.2}},
        {"op":"set_frame","slide_id":"slide-1","id":"photo","frame":{"x":20,"y":30,"width":200,"height":80}}
    ]))).unwrap();
    assert_eq!(changed["document"]["revision"], 2);
    let photo = element(&changed["document"], "photo");
    assert_eq!(photo["crop"], json!({"left":0.2,"top":0.0,"right":0.0,"bottom":0.0}));
    assert_eq!(photo["base64"], asset["base64"]); assert_eq!(photo["alt"], "Synthetic placement");
    let undone = execute_request(json!({"op":"undo_transaction","document":added["document"],"expected_revision":1,"receipt":added["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], before["hash"]);
    for crop in [json!({"left":0.5,"right":0.5}), json!({"top":-0.1}), json!({"bottom":1.0})] {
        let mut invalid = operation.clone(); invalid["crop"] = crop.clone();
        reject_atomic(&before, invalid, "crop");
        reject_atomic(&added["document"], json!({"op":"set_picture_crop","slide_id":"slide-1","id":"photo","crop":crop}), "crop");
    }
    let mut invalid = operation.clone(); invalid["base64"] = json!("not-an-image");
    reject_atomic(&before, invalid, "image");
    reject_atomic(&added["document"], operation, "geometry or ID");
    let populated = populated();
    reject_atomic(&populated, json!({"op":"set_picture_crop","slide_id":"slide-1","id":"text","crop":{}}), "requires a picture");
}

#[test]
fn retest_picture_fit_preserves_aspect_crop_and_source_bytes() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    for (width, height) in [(200, 100), (100, 200)] {
        let image = image::RgbImage::from_pixel(width, height, image::Rgb([30, 140, 100]));
        let mut output = std::io::Cursor::new(Vec::new());
        image.write_to(&mut output, image::ImageFormat::Png).unwrap();
        let base64 = STANDARD.encode(output.into_inner());
        for crop in [json!({"left":0.0,"top":0.0,"right":0.0,"bottom":0.0}), json!({"left":0.1,"top":0.05,"right":0.2,"bottom":0.15})] {
            for fit in ["contain", "cover", "stretch"] {
                let before = original();
                let operation = json!({"op":"add_picture","slide_id":"slide-1","id":"fitted","base64":base64,"mime_type":"image/png","alt":"Synthetic image","frame":{"x":100,"y":100,"width":300,"height":200},"crop":crop,"fit":fit});
                let result = execute_request(request(&before, json!([operation]))).unwrap();
                let fitted = element(&result["document"], "fitted");
                assert_eq!(fitted["base64"], base64);
                let visible_ratio = f64::from(width) * (1.0 - fitted["crop"]["left"].as_f64().unwrap() - fitted["crop"]["right"].as_f64().unwrap())
                    / (f64::from(height) * (1.0 - fitted["crop"]["top"].as_f64().unwrap() - fitted["crop"]["bottom"].as_f64().unwrap()));
                let frame_width = fitted["width"].as_f64().unwrap();
                let frame_height = fitted["height"].as_f64().unwrap();
                if fit == "contain" {
                    assert_eq!(fitted["crop"], crop);
                    assert!(frame_width <= 300.0 + 1e-9 && frame_height <= 200.0 + 1e-9);
                    assert!((fitted["x"].as_f64().unwrap() - (100.0 + (300.0 - frame_width) / 2.0)).abs() < 1e-9);
                    assert!((fitted["y"].as_f64().unwrap() - (100.0 + (200.0 - frame_height) / 2.0)).abs() < 1e-9);
                } else {
                    assert_eq!(frame_width, 300.0);
                    assert_eq!(frame_height, 200.0);
                }
                if fit != "stretch" { assert!((visible_ratio - frame_width / frame_height).abs() < 1e-9); }
                else { assert_eq!(fitted["crop"], crop); }
                assert_managed_history(&before, &result);
                let saved = execute_request(json!({"op":"export_presentation","document":result["document"]})).unwrap();
                let reopened = execute_request(json!({"op":"open_presentation","id":"fitted-native","base64":saved["base64"]})).unwrap();
                let native = element(&reopened["document"], "fitted");
                assert_eq!(native["base64"], base64);
                for key in ["x", "y", "width", "height"] { assert!((native[key].as_f64().unwrap() - fitted[key].as_f64().unwrap()).abs() < 0.001); }
                for key in ["left", "top", "right", "bottom"] { assert!((native["crop"][key].as_f64().unwrap() - fitted["crop"][key].as_f64().unwrap()).abs() < 0.00002); }
            }
        }
    }
}

#[test]
fn typed_batch_bounds_types_and_unknown_fields_fail_without_mutation() {
    let before = populated();
    let valid = json!({"op":"set_frame","slide_id":"slide-1","id":"text","frame":{"x":0,"y":0,"width":300,"height":100}});
    assert!(execute_request(request(&before, json!([valid]))).is_ok());
    for (pointer, value, message) in [
        ("/frame/width", json!(0), "frame requires"), ("/frame/height", json!(-1), "frame requires"),
        ("/frame/x", json!(-1), "frame requires"), ("/frame/width", json!(4097), "frame requires"),
        ("/frame/x", json!(1280), "geometry or ID"), ("/frame/y", json!(720), "geometry or ID"),
        ("/id", json!("missing"), "element not found"), ("/slide_id", json!("missing"), "slide not found")
    ] {
        let mut invalid = valid.clone(); *invalid.pointer_mut(pointer).unwrap() = value;
        reject_atomic(&before, invalid, message);
    }
    reject_atomic(&before, json!({"op":"set_frame","slide_id":"slide-1","id":"nested-text","frame":{"x":350,"y":0,"width":100,"height":100}}), "geometry or ID");
    reject_atomic(&before, json!({"op":"set_slide_background","slide_id":"slide-1","color":"red"}), "color");
    for size in [1, 7, 121, 400] {
        reject_atomic(&before, json!({"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":{"font_size":size}}), "font size");
    }
    for size in [8, 120] {
        let changed = execute_request(request(&before, json!([{"op":"set_text_style","slide_id":"slide-1","ids":["text","shape"],"style":{"font_size":size}}]))).unwrap();
        assert_eq!(element(&changed["document"], "text")["font_size"].as_f64(), Some(f64::from(size)));
    }
    reject_atomic(&before, json!({"op":"set_text_style","slide_id":"slide-1","ids":["text","table"],"style":{"bold":true}}), "requires a text box or shape");
    reject_atomic(&before, json!({"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":{}}), "at least one");
    let unknowns = [
        json!({"op":"set_frame","slide_id":"slide-1","id":"text","frame":{"x":0,"y":0,"width":300,"height":100},"typo":true}),
        json!({"op":"set_frame","slide_id":"slide-1","id":"text","frame":{"x":0,"y":0,"width":300,"height":100,"typo":true}}),
        json!({"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":{"bold":true,"typo":true}}),
        json!({"op":"set_connector","slide_id":"slide-1","id":"connector","connector":{"color":"087F73","stroke_width":2,"arrow":true,"typo":true}}),
        json!({"op":"set_picture_crop","slide_id":"slide-1","id":"picture","crop":{"typo":true}})
    ];
    for invalid in unknowns {
        let mut valid = invalid.clone();
        valid.as_object_mut().unwrap().remove("typo");
        for key in ["frame", "style", "connector", "crop"] { if let Some(value) = valid.get_mut(key).and_then(Value::as_object_mut) { value.remove("typo"); } }
        assert!(execute_request(request(&before, json!([valid]))).is_ok());
        let error = execute_request(request(&before, json!([invalid]))).unwrap_err().to_string();
        assert!(error.contains("unknown field"), "{error}");
    }
}

#[test]
fn typed_batch_target_and_parent_locks_and_hidden_are_enforced() {
    for flag in ["locked", "hidden"] {
        for index in [0, 2, 7, 8, 9] {
            let mut deck = populated()["deck"].clone();
            deck["slides"][0]["elements"][index]["visual"][flag] = json!(true);
            let before = execute_request(json!({"op":"new_document","id":"protected-target","deck":deck})).unwrap();
            let id = before["deck"]["slides"][0]["elements"][index]["id"].as_str().unwrap();
            reject_atomic(&before, json!({"op":"set_frame","slide_id":"slide-1","id":id,"frame":{"x":80,"y":90,"width":300,"height":150}}), "unlock and show");
            if index == 0 || index == 2 || index == 9 {
                let target = if index == 9 { "nested-text" } else { id };
                reject_atomic(&before, json!({"op":"set_text_style","slide_id":"slide-1","ids":[target],"style":{"bold":true}}), "unlock and show");
                reject_atomic(&before, json!({"op":"set_hyperlink","slide_id":"slide-1","id":target,"link":null}), "unlock and show");
            }
            if index == 2 { reject_atomic(&before, json!({"op":"set_shape_adjustment","slide_id":"slide-1","id":id,"adjustment":{"name":"adj","value":20000}}), "unlock and show"); }
            if index == 7 { reject_atomic(&before, json!({"op":"set_picture_crop","slide_id":"slide-1","id":id,"crop":{}}), "unlock and show"); }
            if index == 8 { reject_atomic(&before, json!({"op":"set_connector","slide_id":"slide-1","id":id,"connector":{"color":"087F73","stroke_width":2,"arrow":true}}), "unlock and show"); }
            if index == 9 { reject_atomic(&before, json!({"op":"set_frame","slide_id":"slide-1","id":"nested-text","frame":{"x":0,"y":0,"width":100,"height":50}}), "unlock and show"); }
        }
    }
}

#[test]
fn typed_batch_operation_root_and_style_counts_are_bounded() {
    let before = original();
    for count in [0, 1, 128, 129] {
        let operations = vec![json!({"op":"set_slide_background","slide_id":"slide-1","color":"ABCDEF"}); count];
        let result = execute_request(request(&before, json!(operations)));
        if count == 1 || count == 128 {
            let result = result.unwrap(); assert_eq!(result["document"]["revision"], 1); assert!(!result["receipt"].is_null());
        } else { assert!(result.unwrap_err().to_string().contains("1-128 operations")); }
        let elements: Vec<_> = (0..count).map(|index| json!({"type":"text","id":format!("text-{index}"),"x":0,"y":0,"width":100,"height":50,"text":"Synthetic","font_size":20,"color":"202525","bold":false})).collect();
        let result = execute_request(request(&before, json!([{"op":"add_elements","slide_id":"slide-1","elements":elements}])));
        if count == 1 || count == 128 {
            let added = result.unwrap();
            assert_eq!(added["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().len(), count);
            let ids: Vec<_> = (0..count).map(|index| format!("text-{index}")).collect();
            let styled = execute_request(request(&added["document"], json!([{"op":"set_text_style","slide_id":"slide-1","ids":ids,"style":{"bold":true}}]))).unwrap();
            assert!(styled["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().all(|element| element["bold"] == true));
        } else { assert!(result.unwrap_err().to_string().contains("1-128 roots")); }
    }
    let before = populated();
    for ids in [json!([]), json!(["text","text"]), json!(vec!["text";129])] {
        reject_atomic(&before, json!({"op":"set_text_style","slide_id":"slide-1","ids":ids,"style":{"bold":true}}), "1-128 distinct IDs");
    }
}

#[test]
fn typed_batch_partial_style_preserves_rich_fields_and_paragraph_metadata() {
    let mut rich = text();
    rich["format"]["hyperlink"] = json!("https://example.test/reference");
    rich["format"]["paragraphs"][0]["runs"][0]["field"] = json!({"id":"11111111-1111-4111-8111-111111111111","kind":"slidenum"});
    rich["format"]["paragraphs"][0]["runs"][0]["style"]["font_size"] = json!(1);
    rich["format"]["paragraphs"][1]["runs"][0]["style"]["font_size"] = json!(400);
    rich["format"]["paragraphs"][1]["level"] = json!(2);
    rich["format"]["paragraphs"][1]["margin_left"] = json!(24000);
    rich["format"]["paragraphs"][1]["indent"] = json!(-12000);
    rich["format"]["paragraphs"][1]["line_spacing"] = json!({"kind":"percent","value":120000});
    rich["format"]["paragraphs"][1]["tabs"] = json!([{"position":30000,"alignment":"decimal"}]);
    let before = execute_request(request(&original(), json!([{"op":"add_elements","slide_id":"slide-1","elements":[rich]}]))).unwrap()["document"].clone();
    let style = json!({"bold":true,"italic":false,"underline":true,"color":"@accent2","font_family":"Arial","baseline":10000,"highlight":"FFFF00","language":"en-US"});
    let changed = execute_request(request(&before, json!([{"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":style}]))).unwrap();
    let mut expected = element(&before, "text").clone();
    expected["bold"] = json!(true); expected["color"] = json!("@accent2");
    expected["format"]["italic"] = json!(false); expected["format"]["underline"] = json!(true); expected["format"]["font_family"] = json!("Arial");
    for paragraph in expected["format"]["paragraphs"].as_array_mut().unwrap() {
        for run in paragraph["runs"].as_array_mut().unwrap() { for (key, value) in style.as_object().unwrap() { run["style"][key] = value.clone(); } }
    }
    assert_eq!(element(&changed["document"], "text"), &expected);
    let same = execute_request(request(&changed["document"], json!([{"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":style}]))).unwrap();
    assert_eq!(same["document"], changed["document"]); assert!(same["receipt"].is_null());
    for (style, message) in [(json!({"font_family":""}), "font family"), (json!({"baseline":100001}), "baseline"), (json!({"highlight":"red"}), "color"), (json!({"language":"en_US"}), "language")] {
        reject_atomic(&before, json!({"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":style}), message);
    }
}

#[test]
fn typed_batch_multislide_undo_redo_and_noop_have_exact_content_hashes() {
    let added = execute_request(json!({"op":"edit_slides","document":populated(),"expected_revision":1,"operations":[{"op":"insert","id":"second","title":"Second"}]})).unwrap();
    let before = &added["document"];
    let operations = json!([
        {"op":"set_slide_background","slide_id":"slide-1","color":"F3F5F7"},
        {"op":"set_slide_background","slide_id":"second","color":"202525"},
        {"op":"set_text_style","slide_id":"slide-1","ids":["text","shape"],"style":{"bold":true,"font_size":24}},
        {"op":"set_frame","slide_id":"slide-1","id":"text","frame":{"x":60,"y":80,"width":600,"height":100}},
        {"op":"set_hyperlink","slide_id":"slide-1","id":"text","link":"https://example.test/"},
        {"op":"set_shape_adjustment","slide_id":"slide-1","id":"shape","adjustment":{"name":"adj","value":25000}},
        {"op":"set_connector","slide_id":"slide-1","id":"connector","connector":{"color":"CF5847","stroke_width":3,"arrow":false}},
        {"op":"set_picture_crop","slide_id":"slide-1","id":"picture","crop":{"left":0.1}}
    ]);
    let changed = execute_request(request(before, operations.clone())).unwrap();
    assert_eq!(changed["document"]["revision"], 3);
    assert_eq!(changed["changes"].as_array().unwrap().len(), 2);
    let same = execute_request(request(&changed["document"], operations.clone())).unwrap();
    assert_eq!(same["document"], changed["document"]); assert!(same["receipt"].is_null()); assert_eq!(same["changes"], json!([]));
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":3,"receipt":changed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], before["hash"]); assert_eq!(undone["document"]["deck"], before["deck"]);
    let redone = execute_request(json!({"op":"undo_transaction","document":undone["document"],"expected_revision":4,"receipt":undone["receipt"]})).unwrap();
    assert_eq!(redone["document"]["hash"], changed["document"]["hash"]); assert_eq!(redone["document"]["revision"], 5);
    let mut stale = request(&redone["document"], operations.clone()); stale["expected_revision"] = json!(3);
    assert!(execute_request(stale).unwrap_err().to_string().contains("stale"));
    let mut invalid = operations.as_array().unwrap().clone();
    invalid.push(json!({"op":"set_frame","slide_id":"second","id":"missing","frame":{"x":0,"y":0,"width":100,"height":50}}));
    let error = execute_request(request(before, json!(invalid))).unwrap_err().to_string();
    assert!(error.contains("operation 9") && error.contains("element not found"), "{error}");
    assert_eq!(execute_request(request(before, operations)).unwrap()["document"], changed["document"]);
}

#[test]
fn typed_batch_preserves_source_hashes_bindings_and_rejects_forgery() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let source = execute_request(json!({"op":"ingest","input":{"name":"synthetic.csv","format":"csv","base64":STANDARD.encode("label,value\nAlpha,28\n")}})).unwrap();
    let mut text = text(); text["text"] = json!("Alpha"); text["format"] = json!({});
    let mut deck = original()["deck"].clone(); deck["slides"][0]["elements"] = json!([text]);
    let binding = json!({"slide_id":"slide-1","element_id":"text","field":"/font_size","source_id":source["id"],"source_sha256":source["sha256"],"locator":source["tables"][0]["locators"][0][1],"raw_value":source["tables"][0]["rows"][0][1],"value":28,"transform":"strict_numeric","stale":false});
    let before = execute_request(json!({"op":"new_document","id":"source-batch","deck":deck,"sources":[source],"bindings":[binding]})).unwrap();
    let changed = execute_request(request(&before, json!([
        {"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":{"bold":true}},
        {"op":"set_frame","slide_id":"slide-1","id":"text","frame":{"x":80,"y":100,"width":600,"height":100}}
    ]))).unwrap();
    assert_eq!(changed["document"]["sources"], before["sources"]); assert_eq!(changed["document"]["bindings"], before["bindings"]);
    let exported = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"source-reopened","base64":exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["sources"], before["sources"]); assert_eq!(reopened["document"]["bindings"], before["bindings"]);
    let stale = execute_request(request(&before, json!([{"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":{"font_size":30}}]))).unwrap();
    assert_eq!(stale["document"]["bindings"][0]["stale"], true); assert_eq!(stale["document"]["sources"], before["sources"]);
    assert!(execute_request(json!({"op":"export_presentation","document":stale["document"]})).unwrap_err().to_string().contains("source bindings are stale"));
    for pointer in ["/sources/0/sha256", "/bindings/0/source_sha256"] {
        let mut forged = before.clone(); *forged.pointer_mut(pointer).unwrap() = json!("0".repeat(64));
        let error = execute_request(request(&forged, json!([{"op":"set_slide_background","slide_id":"slide-1","color":"ABCDEF"}]))).unwrap_err().to_string();
        assert!(error.contains("source"), "{error}");
    }
    let mut forged = before.clone(); forged["deck"]["slides"][0]["background"] = json!("ABCDEF");
    assert!(execute_request(request(&forged, json!([{"op":"set_slide_background","slide_id":"slide-1","color":"000000"}]))).unwrap_err().to_string().contains("outside a transaction"));
}

#[test]
fn typed_batch_native_unsupported_text_fails_closed_and_original_parts_survive() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use aislide_core::package::Package;
    let mut deck = original()["deck"].clone(); deck["slides"][0]["elements"] = json!([text()]);
    let exported = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let mut package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().to_owned();
    assert!(xml.contains("<a:bodyPr"));
    let xml = xml.replace("<a:bodyPr", "<a:bodyPr compatLnSpc=\"1\"").replace("</p:sld>", "<p:extLst><p:ext uri=\"urn:test:batch\"><b:opaque xmlns:b=\"urn:test:batch\" value=\"keep\"/></p:ext></p:extLst></p:sld>");
    package.replace_part("ppt/slides/slide1.xml", xml.into_bytes()).unwrap();
    let original_parts = package.parts().clone();
    let bytes = STANDARD.encode(package.save().unwrap());
    let opened = execute_request(json!({"op":"open_presentation","id":"opaque-batch","base64":bytes})).unwrap();
    let before = &opened["document"];
    assert_eq!(execute_request(json!({"op":"export_presentation","document":before})).unwrap()["base64"], bytes);
    reject_atomic(before, json!({"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":{"highlight":"FFFF00"}}), "unrepresentable native text");
    let changed = execute_request(request(before, json!([
        {"op":"set_frame","slide_id":"slide-1","id":"text","frame":{"x":80,"y":80,"width":600,"height":100}},
        {"op":"set_slide_background","slide_id":"slide-1","color":"F3F5F7"}
    ]))).unwrap();
    assert_eq!(changed["document"]["origin"], before["origin"]);
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let saved_package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    for (path, content) in &original_parts {
        if path == "ppt/slides/slide1.xml" || path.starts_with("customXml/") { continue; }
        assert_eq!(saved_package.parts().get(path), Some(content), "original part {path}");
    }
    let saved_xml = saved_package.text("ppt/slides/slide1.xml").unwrap();
    assert!(saved_xml.contains("compatLnSpc=\"1\"")); assert!(saved_xml.contains("<b:opaque xmlns:b=\"urn:test:batch\" value=\"keep\"/>"));
    let reopened = execute_request(json!({"op":"open_presentation","id":"opaque-reopened","base64":saved["base64"]})).unwrap();
    assert_eq!(element(&reopened["document"], "text")["x"].as_f64(), Some(80.0));
    assert_eq!(element(&reopened["document"], "text")["format"], element(before, "text")["format"]);
    reject_atomic(&reopened["document"], json!({"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":{"highlight":"FFFF00"}}), "unrepresentable native text");
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":1,"receipt":changed["receipt"]})).unwrap();
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], bytes);
}

#[test]
fn typed_batch_native_supported_operations_survive_export_reopen() {
    let mut deck = populated()["deck"].clone();
    deck["slides"][0]["elements"].as_array_mut().unwrap().retain(|entry| entry["type"] != "group");
    let exported = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let before = execute_request(json!({"op":"open_presentation","id":"native-batch","base64":exported["base64"]})).unwrap()["document"].clone();
    reject_atomic(&before, json!({"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":{"font_size":24,"bold":true}}), "instead of frame-style replacement");
    reject_atomic(&before, json!({"op":"set_connector","slide_id":"slide-1","id":"connector","connector":{"color":"CF5847","stroke_width":4,"arrow":false,"routing":{"points":[[0,0],[0.25,0],[0.25,1],[1,1]],"start_arrow":true,"dashed":true}}}), "requires graph metadata editing");
    let operations = json!([
        {"op":"set_text_style","slide_id":"slide-1","ids":["text"],"style":{"highlight":"FFFF00","baseline":1000,"language":"en-US"}},
        {"op":"set_hyperlink","slide_id":"slide-1","id":"text","link":"https://example.test/"},
        {"op":"set_shape_adjustment","slide_id":"slide-1","id":"shape","adjustment":{"name":"adj","value":25000}},
        {"op":"set_picture_crop","slide_id":"slide-1","id":"picture","crop":{"left":0.1}},
        {"op":"set_connector","slide_id":"slide-1","id":"connector","connector":{"color":"CF5847","stroke_width":4,"arrow":false,"start":{"element_id":"shape","site":2},"end":{"element_id":"picture","site":0},"routing":element(&before,"connector")["routing"]}},
        {"op":"set_frame","slide_id":"slide-1","id":"table","frame":{"x":80,"y":100,"width":800,"height":320}}
    ]);
    let changed = execute_request(request(&before, operations)).unwrap();
    assert_eq!(changed["document"]["origin"], before["origin"]); assert_eq!(changed["document"]["revision"], 1);
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"native-batch-reopened","base64":saved["base64"]})).unwrap();
    let after = &reopened["document"];
    assert_eq!(element(after, "text")["format"]["hyperlink"], "https://example.test/");
    assert_eq!(element(after, "text")["format"]["paragraphs"][0]["alignment"], "right");
    assert_eq!(element(after, "text")["format"]["paragraphs"][0]["runs"][0]["style"]["highlight"], "FFFF00");
    assert_eq!(element(after, "text")["format"]["paragraphs"][0]["runs"][0]["style"]["baseline"], 1000);
    assert_eq!(element(after, "text")["format"]["paragraphs"][0]["runs"][0]["style"]["language"], "en-US");
    assert_eq!(element(after, "shape")["visual"]["adjustments"], json!([{"name":"adj","value":25000}]));
    assert_eq!(element(after, "shape")["visual"]["shadow"], element(&before, "shape")["visual"]["shadow"]);
    assert_eq!(element(after, "picture")["crop"]["left"].as_f64(), Some(0.1));
    assert_eq!(element(after, "picture")["base64"], element(&before, "picture")["base64"]);
    for field in ["color", "stroke_width", "arrow", "start", "end", "routing"] { assert_eq!(element(after, "connector")[field], element(&changed["document"], "connector")[field], "{field}"); }
    assert_eq!(element(after, "table")["width"].as_f64(), Some(800.0));
    assert_eq!(element(after, "table")["rows"], element(&before, "table")["rows"]);
    assert_eq!(element(after, "chart"), element(&before, "chart"));
}