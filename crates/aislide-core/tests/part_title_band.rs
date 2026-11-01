use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

const MESSAGE: &str = "part show_title:true requires a nonempty part title";
const BLANKS: [&str; 3] = ["", "   ", "\u{3000}"];

fn call(request: Value) -> Result<Value, String> { execute_request(request).map_err(|error| error.to_string()) }

fn example(preset: &str) -> Value {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    catalog["presets"].as_array().unwrap().iter().find(|entry| entry["id"] == preset).unwrap()["example"].clone()
}

fn rows(title: &str, show_title: Option<bool>) -> Value {
    let mut spec = example("list/icon-rows");
    spec["title"] = json!(title);
    let mut layout = json!({"x":48,"y":120,"width":1184,"height":540});
    if let Some(show_title) = show_title { layout["show_title"] = json!(show_title); }
    spec["layout"] = layout;
    spec
}

fn document() -> Value { execute_request(json!({"op":"create_presentation","id":"title-band","title":"Synthetic title band"})).unwrap() }

fn insert(document: &Value, id: &str, spec: &Value) -> Result<Value, String> {
    call(json!({"op":"insert_part","document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":id,"spec":spec}))
}

fn batch(document: &Value, operation: Value) -> Result<Value, String> {
    call(json!({"op":"apply_operations","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[operation]}))
}

fn compose(document: &Value, spec: Value, show_title: Option<bool>) -> Result<Value, String> {
    let mut block = json!({"kind":"part","spec":spec});
    if let Some(show_title) = show_title { block["show_title"] = json!(show_title); }
    batch(document, json!({"op":"compose_slide","slide_id":"slide-1","id":"c","spec":{"title":"Slide title","blocks":[block]}}))
}

fn findings(report: &Value, code: &str) -> Vec<Value> {
    report["findings"].as_array().unwrap().iter().filter(|finding| finding["code"] == code).cloned().collect()
}

#[test]
fn every_part_entry_point_rejects_a_visible_empty_title_band_like_compose_slide() {
    let base = document();
    let mut untitled = rows("", None);
    untitled.as_object_mut().unwrap().remove("layout");
    for title in BLANKS {
        // `layout.show_title` defaults to true, so leaving it out reserves the band too.
        for show_title in [Some(true), None] {
            let spec = rows(title, show_title);
            let context = format!("title {title:?}, show_title {show_title:?}");
            let errors = [
                call(json!({"op":"create_part","id":"p","spec":spec})).map(|_| ()),
                insert(&base, "p", &spec).map(|_| ()),
                batch(&base, json!({"op":"add_part","slide_id":"slide-1","id":"p","spec":spec})).map(|_| ()),
            ];
            for (entry, result) in ["create_part", "insert_part", "add_part"].iter().zip(errors) {
                let error = result.expect_err(&format!("{entry} accepted an empty visible band ({context})"));
                assert!(error.contains(MESSAGE) && error.contains("show_title:false"), "{entry} ({context}): {error}");
            }
        }
        // The same input reaches compose_slide as an explicit show_title:true.
        let mut spec = untitled.clone();
        spec["title"] = json!(title);
        let error = compose(&base, spec, Some(true)).unwrap_err();
        assert!(error.contains(MESSAGE), "compose_slide, title {title:?}: {error}");
    }
}

#[test]
fn an_empty_title_is_accepted_wherever_no_band_is_drawn() {
    let base = document();
    for title in BLANKS {
        let hidden = rows(title, Some(false));
        call(json!({"op":"create_part","id":"p","spec":hidden})).unwrap();
        insert(&base, "p", &hidden).unwrap();
        batch(&base, json!({"op":"add_part","slide_id":"slide-1","id":"p","spec":hidden})).unwrap();
        // Without a layout there is no show_title to disagree with, so existing flows keep their behavior.
        let mut unplaced = rows(title, None);
        unplaced.as_object_mut().unwrap().remove("layout");
        call(json!({"op":"create_part","id":"p","spec":unplaced})).unwrap();
        insert(&base, "p", &unplaced).unwrap();
    }
    for show_title in [Some(true), None] {
        insert(&base, "p", &rows("Operating capabilities", show_title)).unwrap();
    }
    // A diagram draws its band only while its own show_title allows it.
    let diagram = |title: &str, graph_title: bool| json!({"version":1,"preset":"diagram/custom","title":title,"subtitle":"","layout":{"x":48,"y":120,"width":1184,"height":540,"show_title":true},
        "data":{"kind":"diagram","graph":{"version":1,"title":title,"subtitle":"","show_title":graph_title,
            "nodes":[{"id":"n1","label":"Source","x":64.0,"y":120.0,"width":200.0,"height":88.0},{"id":"n2","label":"Target","x":360.0,"y":120.0,"width":200.0,"height":88.0}],
            "edges":[{"id":"e1","source":"n1","target":"n2"}]}}});
    insert(&base, "g", &diagram("", false)).unwrap();
    let error = insert(&base, "g", &diagram("", true)).unwrap_err();
    assert!(error.contains(MESSAGE), "{error}");
}

#[test]
fn an_update_may_keep_an_empty_band_but_cannot_introduce_one() {
    let base = document();
    let titled = insert(&base, "p", &rows("Operating capabilities", Some(true))).unwrap();
    let document = &titled["document"];
    let mut blanked = rows("", Some(true));
    blanked["data"] = document["parts"][0]["spec"]["data"].clone();
    for result in [
        call(json!({"op":"update_part","document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":"p","spec":blanked})),
        batch(document, json!({"op":"update_part","slide_id":"slide-1","id":"p","spec":blanked})),
    ] {
        let error = result.unwrap_err();
        assert!(error.contains(MESSAGE), "{error}");
    }
    // Hiding the band, or keeping a title, is always allowed.
    for spec in [rows("", Some(false)), rows("Renamed heading", Some(true))] {
        call(json!({"op":"update_part","document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":"p","spec":spec})).unwrap();
    }
}

/// A document saved before the rule: its stored spec has a blank title and a visible band.
fn saved_with_empty_band() -> (Vec<u8>, Value) {
    let base = document();
    let titled = insert(&base, "p", &rows("Synthetic heading", Some(true))).unwrap();
    let output = execute_request(json!({"op":"export_presentation","document":titled["document"]})).unwrap();
    let pristine = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    let metadata = pristine.parts().keys().find(|path| path.starts_with("customXml/aislide-provenance") && path.ends_with(".xml")).unwrap().clone();
    let xml = pristine.text(&metadata).unwrap();
    assert_eq!(xml.matches("Synthetic heading").count(), 1, "the title is stored once");
    let mut entries = pristine.parts().clone();
    entries.insert(metadata, xml.replace("Synthetic heading", "").into_bytes());
    let source = Package::from_parts(entries).unwrap().save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"saved-empty-band","base64":STANDARD.encode(&source)})).unwrap();
    (source, opened)
}

#[test]
fn a_saved_empty_band_stays_managed_saves_unchanged_and_is_flagged_by_preflight() {
    let (source, opened) = saved_with_empty_band();
    let document = &opened["document"];
    let warnings = opened["warnings"].as_array().unwrap().iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n");
    assert!(!warnings.contains("PART_METADATA_UNMANAGED"), "{warnings}");
    assert_eq!(document["parts"].as_array().unwrap().len(), 1);
    assert_eq!(document["parts"][0]["stale"], false);
    assert_eq!(document["parts"][0]["spec"]["title"], "");
    assert_eq!(document["parts"][0]["spec"]["layout"]["show_title"], true);

    // An unchanged save keeps the source archive, and so does Undo after an unrelated edit.
    let unchanged = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    assert_eq!(STANDARD.decode(unchanged["base64"].as_str().unwrap()).unwrap(), source);
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],
        "operations":[{"op":"replace","path":"/deck/slides/0/notes","value":"Reviewed synthetic notes"}]}})).unwrap();
    let restored = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":1,"receipt":changed["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], document["hash"]);
    let restored_bytes = execute_request(json!({"op":"export_presentation","document":restored["document"]})).unwrap();
    assert_eq!(STANDARD.decode(restored_bytes["base64"].as_str().unwrap()).unwrap(), source);

    // Preflight names the part, so the saved band can be noticed and removed.
    let report = execute_request(json!({"op":"preflight_presentation","document":document})).unwrap();
    assert!(report["checks"].as_array().unwrap().iter().any(|check| check == "part_title_band"), "{report}");
    let found = findings(&report, "PART_TITLE_BAND_EMPTY");
    assert_eq!(found.len(), 1, "{report}");
    assert_eq!(found[0]["severity"], "warning");
    assert_eq!(found[0]["element_ids"], json!(["p"]));
    assert!(found[0]["message"].as_str().unwrap().contains("layout.show_title"), "{}", found[0]);

    // The stored part can still be edited without being forced to change its title, and hiding the band clears the finding.
    let slide_id = document["deck"]["slides"][0]["id"].clone();
    let mut edited = document["parts"][0]["spec"].clone();
    edited["data"]["rows"][0]["label"] = json!("Edited label");
    let kept = call(json!({"op":"update_part","document":document,"expected_revision":document["revision"],"slide_id":slide_id,"id":"p","spec":edited})).unwrap();
    assert_eq!(kept["document"]["parts"][0]["spec"]["title"], "");
    assert_eq!(kept["document"]["parts"][0]["spec"]["layout"]["show_title"], true);
    let mut hidden = edited.clone();
    hidden["layout"]["show_title"] = json!(false);
    let fixed = call(json!({"op":"update_part","document":kept["document"],"expected_revision":kept["document"]["revision"],"slide_id":slide_id,"id":"p","spec":hidden})).unwrap();
    let report = execute_request(json!({"op":"preflight_presentation","document":fixed["document"]})).unwrap();
    assert!(findings(&report, "PART_TITLE_BAND_EMPTY").is_empty(), "{report}");
}

#[test]
fn managed_parts_with_a_title_or_a_hidden_band_never_raise_the_finding() {
    let base = document();
    let mut document = base;
    for (id, spec) in [("titled", rows("Operating capabilities", Some(true))), ("hidden", rows("", Some(false)))] {
        let inserted = insert(&document, id, &spec).unwrap();
        document = inserted["document"].clone();
    }
    let report = execute_request(json!({"op":"preflight_presentation","document":document})).unwrap();
    assert!(findings(&report, "PART_TITLE_BAND_EMPTY").is_empty(), "{report}");
}
