use aislide_core::{document, editing, protocol::execute_request};
use serde_json::json;

fn fixture() -> serde_json::Value {
    let mut deck = serde_json::to_value(editing::create("seed".into(), "Synthetic API test".into()).unwrap().deck).unwrap();
    let mut text = execute_request(json!({"op":"create_object","id":"text","kind":"text"})).unwrap();
    text["text"] = json!("Alpha Beta");
    let table = execute_request(json!({"op":"create_object","id":"table","kind":"table","rows":2,"columns":2})).unwrap();
    deck["slides"][0]["elements"] = json!([text, table]);
    execute_request(json!({"op":"new_document","id":"authoring","deck":deck})).unwrap()
}

fn change(document: &serde_json::Value, op: &str, input: serde_json::Value) -> serde_json::Value {
    let mut request = input;
    request["op"] = json!(op);
    request["document"] = document.clone();
    request["expected_revision"] = document["revision"].clone();
    execute_request(request).unwrap()
}

#[test]
fn rich_and_table_operations_preserve_content_and_reject_invalid_candidates() {
    let original = fixture();
    let formatted = change(&original,"format_text",json!({"slide_id":"slide-1","id":"text","start":0,"end":5,"style":{"bold":true}}));
    assert_eq!(formatted["document"]["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"][0]["style"]["bold"],true);
    let replaced = change(&formatted["document"],"replace_text_content",json!({"slide_id":"slide-1","id":"text","text":"Alpha Gamma"}));
    assert_eq!(replaced["document"]["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"][0]["style"]["bold"],true);
    let paragraphs = change(&replaced["document"],"update_paragraphs",json!({"slide_id":"slide-1","id":"text","paragraphs":[{"runs":[{"text":"Synced","style":{"italic":true}}],"level":1}]}));
    assert_eq!(paragraphs["document"]["deck"]["slides"][0]["elements"][0]["text"],"Synced");
    let rows = change(&paragraphs["document"],"edit_table",json!({"slide_id":"slide-1","id":"table","operations":[{"op":"insert_row","index":1,"values":["",""]},{"op":"merge","region":{"row":1,"column":0,"row_span":1,"col_span":2}}]}));
    assert_eq!(rows["document"]["deck"]["slides"][0]["elements"][1]["rows"].as_array().unwrap().len(),3);
    assert!(execute_request(json!({"op":"edit_table","document":rows["document"],"expected_revision":4,"slide_id":"slide-1","id":"table","operations":[{"op":"remove_column","index":1}]})).is_err());
    let split = change(&rows["document"],"edit_table",json!({"slide_id":"slide-1","id":"table","operations":[{"op":"split","row":1,"column":1},{"op":"remove_row","index":1}]}));
    assert_eq!(split["document"]["deck"]["slides"][0]["elements"][1]["rows"],original["deck"]["slides"][0]["elements"][1]["rows"]);
}

#[test]
fn selection_copy_and_resize_have_honest_effects_and_one_receipt() {
    let original = fixture();
    let copied = change(&original,"edit_selection",json!({"slide_id":"slide-1","operation":{"op":"copy","ids":["text"],"format":"keep_source_formatting"}}));
    assert_eq!(copied["transaction"]["document"],original);
    assert!(copied["transaction"]["receipt"].is_null());
    assert_eq!(copied["effects"]["raw_native_preserved"],false);
    let pasted = change(&original,"edit_selection",json!({"slide_id":"slide-1","operation":{"op":"paste","id_prefix":"copy","dx":0,"dy":0},"clipboard":copied["clipboard"]}));
    assert_eq!(pasted["transaction"]["document"]["revision"],1);
    assert_eq!(pasted["transaction"]["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().len(),3);
    let resized = change(&original,"resize_canvas",json!({"width":1600,"height":900,"mode":"scale"}));
    assert_eq!(resized["document"]["deck"]["width"],1600);
    let restored = execute_request(json!({"op":"undo_transaction","document":resized["document"],"expected_revision":1,"receipt":resized["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"],original["hash"]);
    assert!(execute_request(json!({"op":"resize_canvas","document":original,"expected_revision":0,"width":319,"height":900,"mode":"keep"})).is_err());
}

#[test]
fn templates_are_new_documents_with_explicit_types() {
    let document = serde_json::to_value(editing::create("template".into(),"Template test".into()).unwrap()).unwrap();
    for kind in ["potx","thmx"] {
        let exported = execute_request(json!({"op":"export_template","document":document,"kind":kind})).unwrap();
        assert_eq!(exported["filename"],format!("template.{kind}"));
        let opened = execute_request(json!({"op":"import_template","id":format!("new-{kind}"),"kind":kind,"base64":exported["base64"]})).unwrap();
        assert_eq!(opened["revision"],0);
        assert_eq!(opened["id"],format!("new-{kind}"));
        let other = if kind == "potx" {"thmx"} else {"potx"};
        assert!(execute_request(json!({"op":"import_template","id":"wrong","kind":other,"base64":exported["base64"]})).is_err());
    }
}

#[test]
fn image_preparation_is_explicit_and_apply_preserves_frame() {
    use base64::{Engine,engine::general_purpose::STANDARD};
    let picture = execute_request(json!({"op":"create_asset","id":"picture","base64":STANDARD.encode("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"2\" height=\"2\"><rect width=\"2\" height=\"2\" fill=\"red\"/></svg>"),"mime_type":"image/svg+xml","alt":"Synthetic red square","size":40})).unwrap();
    let mut deck = editing::create("image".into(),"Image".into()).unwrap().deck;
    deck.slides[0].elements.push(serde_json::from_value(picture.clone()).unwrap());
    let document = serde_json::to_value(document::create("image".into(),deck,vec![],vec![],None).unwrap()).unwrap();
    let image = execute_request(json!({"op":"edit_image","base64":picture["base64"],"mime_type":"image/png","params":{"grayscale":true,"resize_longest_side":8}})).unwrap();
    assert_eq!(image["width"],8);
    let applied = change(&document,"apply_image_edit",json!({"slide_id":"slide-1","id":"picture","image":image}));
    let updated = &applied["document"]["deck"]["slides"][0]["elements"][0];
    assert_eq!(updated["width"],picture["width"]);
    assert_eq!(updated["alt"],picture["alt"]);
    let mut invalid = image; invalid["width"] = json!(9);
    assert!(execute_request(json!({"op":"apply_image_edit","document":document,"expected_revision":0,"slide_id":"slide-1","id":"picture","image":invalid})).is_err());
}

#[test]
fn selection_maps_current_metadata_and_detaches_reparented_roots_atomically() {
    let original = fixture();
    let spec = json!({"version":1,"preset":"flow/balanced","title":"Synthetic stages","data":{"kind":"items","items":[{"label":"One"},{"label":"Two"}]}});
    let inserted = change(&original,"insert_part",json!({"slide_id":"slide-1","id":"managed","spec":spec}));
    let document = &inserted["document"];
    let copied = change(document,"edit_selection",json!({"slide_id":"slide-1","operation":{"op":"copy","ids":["managed"],"format":"keep_source_formatting"}}));
    let pasted = change(document,"edit_selection",json!({"slide_id":"slide-1","operation":{"op":"paste","id_prefix":"copy","dx":0,"dy":0},"clipboard":copied["clipboard"]}));
    assert_eq!(pasted["transaction"]["document"]["parts"].as_array().unwrap().len(),2);
    assert_eq!(pasted["transaction"]["document"]["parts"][1]["stale"],false);
    let grouped = change(document,"edit_selection",json!({"slide_id":"slide-1","operation":{"op":"group","ids":["managed","text"],"group_id":"container"}}));
    assert!(grouped["transaction"]["document"].get("parts").is_none());
    assert_eq!(grouped["effects"]["metadata_review_required"],true);
    assert!(grouped["effects"]["warnings"].as_array().unwrap().iter().any(|value| value.as_str().unwrap().contains("Detached 1")));
    let restored = execute_request(json!({"op":"undo_transaction","document":grouped["transaction"]["document"],"expected_revision":2,"receipt":grouped["transaction"]["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"],document["hash"]);
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":1,"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/slides/0/elements/2/children/0/text","value":"Manual edit"}]}})).unwrap();
    assert_eq!(changed["document"]["parts"][0]["stale"],true);
    assert!(execute_request(json!({"op":"edit_selection","document":changed["document"],"expected_revision":2,"slide_id":"slide-1","operation":{"op":"copy","ids":["managed"],"format":"keep_source_formatting"}})).is_err());
}

#[test]
fn selection_maps_source_binding_ids_and_cut_removes_only_affected_bindings() {
    use base64::{Engine,engine::general_purpose::STANDARD};
    let source = execute_request(json!({"op":"ingest","input":{"name":"synthetic.csv","format":"csv","base64":STANDARD.encode("label,value\nAlpha,1\n")}})).unwrap();
    let mut deck = fixture()["deck"].clone(); deck["slides"][0]["elements"][0]["text"] = json!("Alpha");
    let binding = json!({"slide_id":"slide-1","element_id":"text","field":"/text","source_id":source["id"],"source_sha256":source["sha256"],"locator":source["tables"][0]["locators"][0][0],"raw_value":"Alpha","value":"Alpha","transform":"display_scalar","stale":false});
    let document = execute_request(json!({"op":"new_document","id":"bound","deck":deck,"sources":[source],"bindings":[binding]})).unwrap();
    let copied = change(&document,"edit_selection",json!({"slide_id":"slide-1","operation":{"op":"copy","ids":["text"],"format":"keep_source_formatting"}}));
    let pasted = change(&document,"edit_selection",json!({"slide_id":"slide-1","operation":{"op":"paste","id_prefix":"boundcopy","dx":0,"dy":0},"clipboard":copied["clipboard"]}));
    let candidate = &pasted["transaction"]["document"];
    assert_eq!(candidate["bindings"].as_array().unwrap().len(),2);
    assert_eq!(candidate["bindings"][1]["element_id"],pasted["effects"]["id_map"]["text"]);
    assert_eq!(candidate["bindings"][1]["stale"],false);
    let cut = change(candidate,"edit_selection",json!({"slide_id":"slide-1","operation":{"op":"cut","ids":["text"],"format":"keep_source_formatting"}}));
    assert_eq!(cut["transaction"]["document"]["bindings"].as_array().unwrap().len(),1);
    assert_eq!(cut["transaction"]["document"]["sources"],document["sources"]);
}

#[test]
fn native_clipboard_checks_each_source_and_rejects_unrepresented_raw_formatting() {
    use base64::{Engine,engine::general_purpose::STANDARD};
    let mut deck = fixture()["deck"].clone();
    deck["slides"][0]["elements"].as_array_mut().unwrap().truncate(1);
    let bytes = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let mut package = aislide_core::package::Package::open(STANDARD.decode(bytes["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().replace("<a:bodyPr", "<a:bodyPr compatLnSpc=\"1\"");
    package.replace_part("ppt/slides/slide1.xml",xml.into_bytes()).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"native","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    let document = &opened["document"];
    let request = json!({"op":"edit_selection","document":document,"expected_revision":0,"slide_id":"slide-1","operation":{"op":"copy","ids":["text"],"format":"keep_source_formatting"}});
    let error = execute_request(request).unwrap_err().to_string();
    assert!(error.contains("individual copy"),"{error}");
    let duplicated = change(document,"edit_slides",json!({"operations":[{"op":"duplicate","slide_id":"slide-1","id":"duplicated"}]}));
    let error = execute_request(json!({"op":"edit_selection","document":duplicated["document"],"expected_revision":1,"slide_id":"duplicated","operation":{"op":"copy","ids":["text"],"format":"keep_source_formatting"}})).unwrap_err().to_string();
    assert!(error.contains("individual copy"),"{error}");
}

#[test]
fn public_text_operations_are_revision_checked_candidates() {
    let mut deck = editing::create("seed".into(), "Synthetic API test".into()).unwrap().deck;
    deck.slides[0].notes = "Alpha Alpha".into();
    let original = document::create("authoring".into(), deck, vec![], vec![], None).unwrap();
    let before = serde_json::to_value(&original).unwrap();
    let search = json!({"query":"Alpha", "include_notes":true});
    let found = execute_request(json!({"op":"search_text", "deck":original.deck, "options":search})).unwrap();
    assert_eq!(found.as_array().unwrap().len(), 2);
    let request = json!({"op":"replace_text", "document":original, "expected_revision":0,
        "options":{"search":search,"replacement":"Beta","replace_all":true}});
    let result = execute_request(request.clone()).unwrap();
    assert_eq!(result["document"]["deck"]["slides"][0]["notes"], "Beta Beta");
    assert_eq!(result["document"]["revision"], 1);
    assert_eq!(serde_json::to_value(&original).unwrap(), before);
    let restored = execute_request(json!({"op":"undo_transaction", "document":result["document"],
        "expected_revision":1,"receipt":result["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], original.hash);
    let mut stale = request.clone();
    stale["expected_revision"] = json!(7);
    assert!(execute_request(stale).is_err());
    let mut forged = request;
    forged["document"]["deck"]["slides"][0]["notes"] = json!("tampered");
    assert!(execute_request(forged).is_err());
}