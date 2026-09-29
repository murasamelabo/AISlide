use aislide_core::{authoring_preflight::{preflight_presentation, PreflightOptions}, document};
use serde_json::{json, Value};

fn new_document() -> Value {
    serde_json::to_value(document::create("references-test".into(), serde_json::from_value(scene()).unwrap(), vec![], vec![], None).unwrap()).unwrap()
}

fn set_references(original: &Value, placement: &str, entries: Value) -> aislide_core::Result<Value> {
    aislide_core::execute_request(json!({"op":"set_references","document":original,"expected_revision":original["revision"],"expected_hash":original["hash"],"options":{"placement":placement,"entries":entries}}))
}

fn public_entry() -> Value {
    json!({"id":"learn","name":"Microsoft Learn","url":"https://learn.microsoft.com/azure/","slide_ids":["slide-1"],"publish":true})
}

fn scene() -> Value {
    json!({"version":1,"title":"Reference test","width":1280,"height":720,"slides":[{"id":"slide-1","title":"Example","background":"FFFFFF","notes":"Source: https://learn.microsoft.com/azure/","elements":[{"type":"text","id":"title","x":64,"y":80,"width":1152,"height":120,"text":"Synthetic reference example","font_size":42,"color":"202525","bold":true}]}]})
}

fn edit(original: &Value, operations: Value) -> aislide_core::Result<Value> {
    aislide_core::execute_request(json!({"op":"transaction","document":original,"transaction":{"expected_revision":original["revision"],"expected_hash":original["hash"],"operations":operations}}))
}

fn reference_element<'a>(document: &'a Value, id: &str) -> &'a Value {
    document["deck"]["slides"].as_array().unwrap().iter().flat_map(|slide| slide["elements"].as_array().unwrap()).find(|element| element["id"] == id).unwrap()
}

fn assert_reference_frames_unchanged(before: &Value, after: &Value) {
    for owned in after["references"]["elements"].as_array().unwrap() {
        let id = owned["id"].as_str().unwrap();
        let original = reference_element(before, id);
        let mut updated = reference_element(after, id).clone();
        if id.starts_with("aislide-ref-pages-") { updated["text"] = original["text"].clone(); }
        assert_eq!(&updated, original, "{id}");
    }
}

#[test]
fn appendix_only_preserves_dense_slides_and_publishes_only_approved_entries() {
    let mut deck = scene();
    let mut source = deck["slides"][0].clone();
    source["title"] = json!("Private source title");
    source["notes"] = json!("Private speaker note. https://learn.microsoft.com/azure/");
    for (key, value) in [("x", 0), ("y", 0), ("width", 1280), ("height", 720)] { source["elements"][0][key] = json!(value); }
    deck["slides"] = json!((1..=22).map(|number| { let mut slide = source.clone(); slide["id"] = json!(format!("slide-{number}")); slide }).collect::<Vec<_>>());
    let original = serde_json::to_value(document::create("dense-references".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let mut entry = public_entry();
    entry["slide_ids"] = json!((1..=22).rev().map(|number| format!("slide-{number}")).collect::<Vec<_>>());
    let entries = json!([entry, {"id":"private","name":"Unapproved source","url":"file:///secret/DO_NOT_PUBLISH","slide_ids":["slide-1"]}]);
    let changed = set_references(&original, "appendix_only", entries.clone()).unwrap();
    let updated = &changed["document"];
    assert_eq!(changed["publication"], json!({"supplied":2,"published":1,"excluded":1}));
    assert_eq!(&updated["deck"]["slides"].as_array().unwrap()[..22], original["deck"]["slides"].as_array().unwrap());
    assert_eq!(updated["sources"], original["sources"]);
    assert_eq!(updated["deck"]["slides"].as_array().unwrap().len(), 23);
    assert_eq!(reference_element(updated, "aislide-ref-pages-learn")["text"], format!("Slides: {}", (1..=22).map(|number| number.to_string()).collect::<Vec<_>>().join(", ")));
    assert_eq!(reference_element(updated, "aislide-ref-entry-learn")["text"], "[1] Microsoft Learn");
    assert_eq!(reference_element(updated, "aislide-ref-url-learn")["text"], public_entry()["url"]);
    let appendix = &updated["deck"]["slides"][22];
    for private in ["Private source title", "Private speaker note", "DO_NOT_PUBLISH", "Unapproved source"] { assert!(!appendix.to_string().contains(private)); }
    assert!(!updated["references"].to_string().contains("DO_NOT_PUBLISH"));
    for element in appendix["elements"].as_array().unwrap() {
        assert!(element["font_size"].as_f64().unwrap() >= 16.0);
        if !element["format"]["hyperlink"].is_null() { assert_eq!(element["text"], public_entry()["url"]); }
    }
    let repeated = set_references(updated, "appendix_only", entries).unwrap();
    assert_eq!(repeated["document"]["hash"], updated["hash"]);
    assert!(repeated["receipt"].is_null());
    let checked = aislide_core::execute_request(json!({"op":"preflight_presentation","document":updated,"options":{"page_indices":[0,21,22]}})).unwrap();
    assert!(!checked["findings"].as_array().unwrap().iter().any(|finding| ["SOURCE_URL_NOT_VISIBLE", "TEXT_OVERFLOW", "REFERENCE_COLLISION"].contains(&finding["code"].as_str().unwrap())), "{}", checked["findings"]);
}

#[test]
fn appendix_only_on_imported_dense_deck_preserves_existing_slide_parts() {
    use base64::Engine;
    let mut deck = scene();
    for (key, value) in [("x", 0), ("y", 0), ("width", 1280), ("height", 720)] { deck["slides"][0]["elements"][0][key] = json!(value); }
    let original = serde_json::to_value(document::create("dense-import-source".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let exported = aislide_core::execute_request(json!({"op":"export_presentation","document":original})).unwrap();
    let source_bytes = base64::engine::general_purpose::STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap();
    let source_package = aislide_core::package::Package::open(source_bytes).unwrap();
    let opened = aislide_core::execute_request(json!({"op":"open_presentation","id":"dense-imported","base64":exported["base64"]})).unwrap();
    let before = opened["document"].clone();
    let changed = set_references(&before, "appendix_only", json!([public_entry()])).unwrap();
    assert_eq!(changed["document"]["deck"]["slides"][0], before["deck"]["slides"][0]);
    let output = aislide_core::execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let output_bytes = base64::engine::general_purpose::STANDARD.decode(output["base64"].as_str().unwrap()).unwrap();
    let output_package = aislide_core::package::Package::open(output_bytes).unwrap();
    for path in ["ppt/slides/slide1.xml", "ppt/slides/_rels/slide1.xml.rels", "ppt/notesSlides/notesSlide1.xml"] {
        assert_eq!(output_package.text(path).unwrap(), source_package.text(path).unwrap(), "{path}");
    }
    let reopened = aislide_core::execute_request(json!({"op":"open_presentation","id":"dense-imported-result","base64":output["base64"]})).unwrap();
    assert_eq!(reference_element(&reopened["document"], "aislide-ref-pages-learn")["text"], "Slides: 1");
    let restored = aislide_core::execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], before["hash"]);
}

#[test]
fn appendix_only_updates_captions_after_reorder_delete_and_undo_without_reflow() {
    let mut deck = scene();
    let source = deck["slides"][0].clone();
    deck["slides"] = json!((1..=3).map(|number| { let mut slide = source.clone(); slide["id"] = json!(format!("slide-{number}")); slide }).collect::<Vec<_>>());
    let original = serde_json::to_value(document::create("reference-order".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let mut entry = public_entry(); entry["slide_ids"] = json!(["slide-3", "slide-1"]);
    let mut second = public_entry(); second["id"] = json!("second"); second["slide_ids"] = json!(["slide-2"]);
    let changed = set_references(&original, "appendix_only", json!([entry, second])).unwrap();
    let updated = &changed["document"];
    assert_eq!(reference_element(updated, "aislide-ref-pages-learn")["text"], "Slides: 1, 3");
    let moved = edit(updated, json!([{"op":"move","from":"/deck/slides/3","path":"/deck/slides/0"}])).unwrap();
    assert_eq!(moved["document"]["deck"]["slides"][0]["id"], "aislide-ref-page-1");
    assert_eq!(reference_element(&moved["document"], "aislide-ref-pages-learn")["text"], "Slides: 2, 4");
    assert_reference_frames_unchanged(updated, &moved["document"]);
    let reordered = edit(&moved["document"], json!([{"op":"move","from":"/deck/slides/3","path":"/deck/slides/1"}])).unwrap();
    assert_eq!(reference_element(&reordered["document"], "aislide-ref-pages-learn")["text"], "Slides: 2, 3");
    assert_reference_frames_unchanged(updated, &reordered["document"]);
    let deleted = edit(&reordered["document"], json!([{"op":"remove","path":"/deck/slides/2"}])).unwrap();
    assert_eq!(reference_element(&deleted["document"], "aislide-ref-pages-learn")["text"], "Slides: 2");
    assert_eq!(reference_element(&deleted["document"], "aislide-ref-pages-second")["text"], "Slides: 3");
    assert_reference_frames_unchanged(updated, &deleted["document"]);
    let restored = aislide_core::execute_request(json!({"op":"undo_transaction","document":deleted["document"],"expected_revision":deleted["document"]["revision"],"receipt":deleted["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], reordered["document"]["hash"]);
    let removed = edit(&restored["document"], json!([{"op":"remove","path":"/deck/slides/3"}])).unwrap();
    assert!(!removed["document"]["references"]["elements"].as_array().unwrap().iter().any(|owned| owned["id"].as_str().unwrap().ends_with("-second")));
    assert!(!removed["document"]["deck"]["slides"][0]["elements"].to_string().contains("aislide-ref-pages-second"));
    assert_eq!(reference_element(&removed["document"], "aislide-ref-pages-learn")["text"], "Slides: 2, 3");
    let caption_index = removed["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().position(|element| element["id"] == "aislide-ref-pages-learn").unwrap();
    let before = removed["document"].clone();
    assert!(edit(&before, json!([{"op":"replace","path":format!("/deck/slides/0/elements/{caption_index}/text"),"value":"Slides: 999"}])).is_err());
    assert_eq!(before, removed["document"]);
    let cleared = set_references(&before, "appendix_only", json!([])).unwrap();
    assert!(cleared["document"]["references"].is_null());
    assert_eq!(cleared["document"]["deck"]["slides"].as_array().unwrap().len(), 2);
}

#[test]
fn appendix_only_reserves_digit_width_through_maximum_page_capacity() {
    let mut deck = scene();
    deck["width"] = json!(320);
    deck["slides"] = json!((1..=9).map(|number| json!({"id":format!("slide-{number}"),"title":"Private","background":"FFFFFF","notes":"","elements":[]})).collect::<Vec<_>>());
    let original = serde_json::to_value(document::create("caption-capacity".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let mut entry = public_entry(); entry["name"] = json!("Source"); entry["url"] = json!("https://a.example/"); entry["slide_ids"] = json!((1..=9).map(|number| format!("slide-{number}")).collect::<Vec<_>>());
    let changed = set_references(&original, "appendix_only", json!([entry])).unwrap();
    let updated = &changed["document"];
    let capacity = aislide_core::limits::LARGE.slides;
    let mut expanded: Vec<_> = (0..capacity - 10).map(|number| json!({"id":format!("filler-{number}"),"title":"Private filler","background":"FFFFFF","notes":"","elements":[]})).collect();
    expanded.extend(updated["deck"]["slides"].as_array().unwrap().iter().cloned());
    let grown = edit(updated, json!([{"op":"replace","path":"/deck/slides","value":expanded}])).unwrap();
    let moved = edit(&grown["document"], json!([{"op":"move","from":format!("/deck/slides/{}", capacity - 1),"path":"/deck/slides/0"}])).unwrap();
    let caption = reference_element(&moved["document"], "aislide-ref-pages-learn")["text"].as_str().unwrap().split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(caption, format!("Slides: {}", (capacity - 8..=capacity).map(|number| number.to_string()).collect::<Vec<_>>().join(", ")));
    assert_reference_frames_unchanged(updated, &moved["document"]);
    let measured = aislide_core::execute_request(json!({"op":"measure_layout","deck":moved["document"]["deck"]})).unwrap();
    assert!(!measured["measurements"].as_array().unwrap().iter().any(|measurement| measurement["overflow"] == true));
    let beyond_capacity = json!({"id":"too-many","title":"Extra","background":"FFFFFF","notes":"","elements":[]});
    assert!(edit(&moved["document"], json!([{"op":"add","path":"/deck/slides/-","value":beyond_capacity}])).is_err());
    assert_eq!(moved["document"]["deck"]["slides"].as_array().unwrap().len(), capacity);
}

#[test]
fn appendix_only_rejects_unfittable_captions_and_invalid_targets_atomically() {
    let mut deck = scene(); deck["width"] = json!(320); deck["height"] = json!(320);
    deck["slides"] = json!((1..=40).map(|number| json!({"id":format!("slide-{number}"),"title":"Private","background":"FFFFFF","notes":"","elements":[]})).collect::<Vec<_>>());
    let original = serde_json::to_value(document::create("caption-limit".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let before = original.clone();
    let mut entry = public_entry(); entry["name"] = json!("Source"); entry["url"] = json!("https://a.example/"); entry["slide_ids"] = json!((1..=40).map(|number| format!("slide-{number}")).collect::<Vec<_>>());
    let error = set_references(&original, "appendix_only", json!([entry])).unwrap_err().to_string();
    assert!(error.contains("caption") && error.contains("fit"), "{error}");
    assert_eq!(original, before);
    for (field, value) in [("url", json!("file:///DO_NOT_PUBLISH")), ("url", json!("https://user:password@example.com/")), ("slide_ids", json!(["missing"]))] {
        let mut invalid = public_entry(); invalid[field] = value;
        assert!(set_references(&original, "appendix_only", json!([invalid])).is_err());
        assert_eq!(original, before);
    }
    let mut private = public_entry(); private["publish"] = json!(false); private["url"] = json!("file:///DO_NOT_PUBLISH");
    let excluded = set_references(&original, "appendix_only", json!([private])).unwrap();
    assert_eq!(excluded["publication"], json!({"supplied":1,"published":0,"excluded":1}));
    assert_eq!(excluded["document"]["hash"], original["hash"]);
    assert!(excluded["document"]["references"].is_null());
}

#[test]
fn appendix_only_reopens_updates_native_captions_and_exports_readable_pdf() {
    use base64::Engine;
    let mut deck = scene();
    let mut second = deck["slides"][0].clone(); second["id"] = json!("slide-2");
    deck["slides"].as_array_mut().unwrap().push(second);
    let original = serde_json::to_value(document::create("native-captions".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let changed = set_references(&original, "appendix_only", json!([public_entry()])).unwrap();
    let moved = edit(&changed["document"], json!([{"op":"move","from":"/deck/slides/2","path":"/deck/slides/0"}])).unwrap();
    let exported = aislide_core::execute_request(json!({"op":"export_presentation","document":moved["document"]})).unwrap();
    let bytes = base64::engine::general_purpose::STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap();
    let package = aislide_core::package::Package::open(bytes).unwrap();
    assert!(package.text("ppt/slides/slide1.xml").unwrap().contains("Slides: 2"));
    let reopened = aislide_core::execute_request(json!({"op":"open_presentation","id":"reopened-captions","base64":exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["references"]["options"]["entries"][0]["slide_ids"], json!(["slide-1"]));
    let reordered = edit(&reopened["document"], json!([{"op":"move","from":"/deck/slides/1","path":"/deck/slides/2"}])).unwrap();
    let caption = reference_element(&reordered["document"], "aislide-ref-pages-learn");
    assert_eq!(caption["text"], "Slides: 3");
    for key in ["x", "y", "width", "height", "font_size"] { assert_eq!(caption[key], reference_element(&reopened["document"], "aislide-ref-pages-learn")[key]); }
    for id in ["aislide-ref-entry-learn", "aislide-ref-url-learn"] { assert_eq!(reference_element(&reordered["document"], id), reference_element(&reopened["document"], id)); }
    let reexported = aislide_core::execute_request(json!({"op":"export_presentation","document":reordered["document"]})).unwrap();
    let reopened_again = aislide_core::execute_request(json!({"op":"open_presentation","id":"roundtrip-captions","base64":reexported["base64"]})).unwrap();
    assert_eq!(reference_element(&reopened_again["document"], "aislide-ref-pages-learn")["text"], "Slides: 3");
    let document: document::Document = serde_json::from_value(reopened_again["document"].clone()).unwrap();
    let pdf_output = aislide_core::export_static::export_static(&document.deck, &serde_json::from_value(json!({"format":"pdf"})).unwrap()).unwrap();
    let pdf = lopdf::Document::load_mem(&pdf_output.artifacts[0].bytes).unwrap();
    let visible = pdf.extract_text(&[1]).unwrap();
    assert!(visible.contains("Slides: 3") && visible.contains("Microsoft Learn") && visible.contains("https://learn.microsoft.com/azure/"), "{visible}");
    let page_id = pdf.get_pages()[&1];
    let annotations = pdf.get_object(page_id).unwrap().as_dict().unwrap().get(b"Annots").unwrap().as_array().unwrap();
    assert_eq!(annotations.len(), 1);
    let annotation = pdf.get_object(annotations[0].as_reference().unwrap()).unwrap().as_dict().unwrap();
    assert_eq!(annotation.get(b"A").unwrap().as_dict().unwrap().get(b"URI").unwrap().as_str().unwrap(), b"https://learn.microsoft.com/azure/");
    let restored = aislide_core::execute_request(json!({"op":"undo_transaction","document":reordered["document"],"expected_revision":reordered["document"]["revision"],"receipt":reordered["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], reopened["document"]["hash"]);
}

#[test]
fn appendix_only_prunes_empty_appendix_pages_before_updating_remaining_numbers() {
    let mut deck = scene(); deck["width"] = json!(320); deck["height"] = json!(320);
    deck["slides"] = json!((1..=2).map(|number| json!({"id":format!("slide-{number}"),"title":"Private","background":"FFFFFF","notes":"","elements":[]})).collect::<Vec<_>>());
    let original = serde_json::to_value(document::create("appendix-pruning".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let mut first = public_entry(); first["name"] = json!("Source"); first["url"] = json!("https://a.example/");
    let mut second = first.clone(); second["id"] = json!("second"); second["slide_ids"] = json!(["slide-2"]);
    let changed = set_references(&original, "appendix_only", json!([first, second])).unwrap();
    assert_eq!(changed["document"]["deck"]["slides"].as_array().unwrap().len(), 4);
    let moved = edit(&changed["document"], json!([{"op":"move","from":"/deck/slides/2","path":"/deck/slides/0"},{"op":"move","from":"/deck/slides/3","path":"/deck/slides/1"}])).unwrap();
    assert_eq!(reference_element(&moved["document"], "aislide-ref-pages-second")["text"], "Slides: 4");
    let deleted = edit(&moved["document"], json!([{"op":"remove","path":"/deck/slides/2"}])).unwrap();
    assert_eq!(deleted["document"]["deck"]["slides"].as_array().unwrap().len(), 2);
    assert_eq!(deleted["document"]["references"]["slides"], json!(["aislide-ref-page-2"]));
    assert_eq!(reference_element(&deleted["document"], "aislide-ref-pages-second")["text"], "Slides: 2");
    assert_reference_frames_unchanged(&moved["document"], &deleted["document"]);
    let restored = aislide_core::execute_request(json!({"op":"undo_transaction","document":deleted["document"],"expected_revision":deleted["document"]["revision"],"receipt":deleted["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], moved["document"]["hash"]);
}

#[test]
fn appendix_only_multiline_caption_survives_native_reopen_and_refresh() {
    let mut deck = scene(); deck["width"] = json!(320);
    deck["slides"] = json!((1..=9).map(|number| json!({"id":format!("slide-{number}"),"title":"Private","background":"FFFFFF","notes":"","elements":[]})).collect::<Vec<_>>());
    let original = serde_json::to_value(document::create("multiline-caption".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let mut entry = public_entry(); entry["name"] = json!("Source"); entry["url"] = json!("https://a.example/"); entry["slide_ids"] = json!((1..=9).map(|number| format!("slide-{number}")).collect::<Vec<_>>());
    let changed = set_references(&original, "appendix_only", json!([entry])).unwrap();
    assert!(reference_element(&changed["document"], "aislide-ref-pages-learn")["text"].as_str().unwrap().contains('\n'));
    let exported = aislide_core::execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = aislide_core::execute_request(json!({"op":"open_presentation","id":"multiline-reopened","base64":exported["base64"]})).unwrap();
    let moved = edit(&reopened["document"], json!([{"op":"move","from":"/deck/slides/9","path":"/deck/slides/0"}])).unwrap();
    let caption = reference_element(&moved["document"], "aislide-ref-pages-learn");
    assert_eq!(caption["text"].as_str().unwrap().split_whitespace().collect::<Vec<_>>().join(" "), "Slides: 2, 3, 4, 5, 6, 7, 8, 9, 10");
    for key in ["x", "y", "width", "height", "font_size"] { assert_eq!(caption[key], reference_element(&reopened["document"], "aislide-ref-pages-learn")[key]); }
    let reexported = aislide_core::execute_request(json!({"op":"export_presentation","document":moved["document"]})).unwrap();
    let reopened_again = aislide_core::execute_request(json!({"op":"open_presentation","id":"multiline-again","base64":reexported["base64"]})).unwrap();
    assert_eq!(reference_element(&reopened_again["document"], "aislide-ref-pages-learn")["text"], caption["text"]);
    let restored = aislide_core::execute_request(json!({"op":"undo_transaction","document":moved["document"],"expected_revision":moved["document"]["revision"],"receipt":moved["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], reopened["document"]["hash"]);
}

#[test]
fn marker_stays_in_bottom_quarter_and_reports_only_candidate_obstacles() {
    let free = set_references(&new_document(), "appendix", json!([public_entry()])).unwrap();
    let marker = reference_element(&free["document"], "aislide-ref-marker");
    let expected_frame = format!("x=32, y={}, width=1216, height={}", marker["y"].as_f64().unwrap(), marker["height"].as_f64().unwrap());
    for placement in ["auto", "footnotes", "appendix"] {
        let mut deck = scene();
        deck["slides"][0]["elements"].as_array_mut().unwrap().push(json!({"type":"text","id":"lower-body","x":32,"y":540,"width":1216,"height":180,"text":"Dense lower body","font_size":24,"color":"202525","bold":false}));
        let original = serde_json::to_value(document::create("bounded-marker".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
        let before = original.clone();
        let error = set_references(&original, placement, json!([public_entry()])).unwrap_err().to_string();
        assert!(error.contains("slide-1") && error.contains("candidate frame") && error.contains(&expected_frame), "{error}");
        let blockers = error.split("blocking elements: ").nth(1).unwrap().split(';').next().unwrap();
        assert!(blockers.contains("lower-body") && !blockers.contains("title"), "{error}");
        assert!(error.contains("appendix_only") && error.contains("set_references"), "{error}");
        assert_eq!(original, before);
    }
}

#[test]
fn marker_requires_eight_pixels_clearance_even_without_candidate_intersection() {
    let free = set_references(&new_document(), "appendix", json!([public_entry()])).unwrap();
    let top = reference_element(&free["document"], "aislide-ref-marker")["y"].as_f64().unwrap();
    let mut deck = scene();
    deck["slides"][0]["elements"].as_array_mut().unwrap().push(json!({"type":"text","id":"nearby-body","x":32,"y":540,"width":1216,"height":top - 544.0,"text":"Only four pixels of clearance","font_size":24,"color":"202525","bold":false}));
    let original = serde_json::to_value(document::create("marker-clearance".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let error = set_references(&original, "appendix", json!([public_entry()])).unwrap_err().to_string();
    assert!(error.contains("8px clearance") && error.contains("blocking elements: none"), "{error}");
    assert!(!error.contains("nearby-body") && !error.contains("title"), "{error}");
}

#[test]
fn review_existing_footer_uses_free_band_above_without_moving_content() {
    for placement in ["auto", "appendix"] {
        let mut deck = scene();
        deck["slides"][0]["elements"].as_array_mut().unwrap().extend([
            json!({"type":"text","id":"existing-source","x":64,"y":680,"width":1050,"height":24,"text":"Microsoft Learn","font_size":16,"color":"202525","bold":false}),
            json!({"type":"text","id":"page-number","x":1160,"y":680,"width":56,"height":24,"text":"1","font_size":16,"color":"202525","bold":false}),
        ]);
        let original = serde_json::to_value(document::create("occupied-footer".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
        let changed = set_references(&original, placement, json!([public_entry()])).unwrap();
        let elements = changed["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
        assert_eq!(&elements[..3], original["deck"]["slides"][0]["elements"].as_array().unwrap());
        assert!(elements[3]["y"].as_f64().unwrap() >= 616.0);
        assert!(elements[3]["y"].as_f64().unwrap() + elements[3]["height"].as_f64().unwrap() <= 672.0);
    }
}

#[test]
fn review_unrelated_edits_keep_reference_geometry_and_page_count() {
    let changed = set_references(&new_document(), "auto", json!([public_entry()])).unwrap();
    let original = &changed["document"];
    for top in [600, 668] {
        let edited = edit(original, json!([{"op":"add","path":"/deck/slides/0/elements/-","value":{"type":"text","id":"new-label","x":64,"y":top,"width":600,"height":24,"text":"New body label","font_size":16,"color":"202525","bold":false}}])).unwrap();
        assert_eq!(edited["document"]["deck"]["slides"].as_array().unwrap().len(), 1);
        assert_eq!(edited["document"]["deck"]["slides"][0]["elements"][1], original["deck"]["slides"][0]["elements"][1]);
        assert_eq!(edited["document"]["references"], original["references"]);
    }
}

#[test]
fn review_appendix_reordering_is_preserved() {
    let changed = set_references(&new_document(), "appendix", json!([public_entry()])).unwrap();
    let moved = edit(&changed["document"], json!([{"op":"move","from":"/deck/slides/1","path":"/deck/slides/0"}])).unwrap();
    assert_eq!(moved["document"]["deck"]["slides"][0]["id"], "aislide-ref-page-1");
}

#[test]
fn review_japanese_note_urls_are_normalized_without_leaking_them() {
    for note in ["（https://aka.ms/mysecurityinfo）から行う", "https://aka.ms/mysecurityinfo。参照", "https://aka.ms/mysecurityinfoから行う", "https://aka.ms/mysecurityinfoから行う。", "https://LEARN.microsoft.com"] {
        let mut deck = scene(); deck["slides"][0]["notes"] = json!(note);
        let original = serde_json::to_value(document::create("cjk-url".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
        let mut entry = public_entry(); entry["url"] = json!(if note.contains("LEARN") { "https://learn.microsoft.com/" } else { "https://aka.ms/mysecurityinfo" });
        let changed = set_references(&original, "appendix", json!([entry])).unwrap();
        let report = aislide_core::execute_request(json!({"op":"preflight_presentation","document":changed["document"],"options":{"page_indices":[0]}})).unwrap();
        assert!(!report["findings"].as_array().unwrap().iter().any(|finding| finding["code"] == "SOURCE_URL_NOT_VISIBLE"), "{note}");
    }
}

#[test]
fn review_explicitly_delimited_unicode_urls_are_not_truncated() {
    for (note, url) in [
        ("参照：<https://example.com/資料>。", "https://example.com/%E8%B3%87%E6%96%99"),
        ("「https://example.com/資料」から行う", "https://example.com/%E8%B3%87%E6%96%99"),
        ("（https://例え.テスト/資料）から行う", "https://xn--r8jz45g.xn--zckzah/%E8%B3%87%E6%96%99"),
        ("<https://example.com/a(b)> を参照", "https://example.com/a(b)"),
    ] {
        let mut deck = scene(); deck["slides"][0]["notes"] = json!(note);
        let original = serde_json::to_value(document::create("unicode-url".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
        let mut entry = public_entry(); entry["url"] = json!(url);
        let changed = set_references(&original, "appendix", json!([entry])).unwrap();
        let report = aislide_core::execute_request(json!({"op":"preflight_presentation","document":changed["document"],"options":{"page_indices":[0]}})).unwrap();
        assert!(!report["findings"].as_array().unwrap().iter().any(|finding| finding["code"] == "SOURCE_URL_NOT_VISIBLE"), "{note}");
    }
}

#[test]
fn review_publication_reports_ignored_count_without_private_values() {
    let result = set_references(&new_document(), "auto", json!([public_entry(), {"id":"secret","name":"Private","url":"file:///secret","slide_ids":["slide-1"]}, {"id":"unapproved","name":"Private","url":"https://private.example/","slide_ids":["slide-1"],"publish":false}])).unwrap();
    assert_eq!(result["publication"], json!({"supplied":3,"published":1,"excluded":2}));
    assert!(!result.to_string().contains("file:///secret"));
}

#[test]
fn review_links_cover_only_url_rows_and_appendix_uses_theme_text() {
    let mut deck = scene();
    let mut design = aislide_core::execute_request(json!({"op":"design_defaults"})).unwrap();
    design["theme"]["colors"]["dk1"] = json!("123456");
    deck["design"] = design;
    let original = serde_json::to_value(document::create("theme-refs".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    for placement in ["auto", "appendix"] {
        let changed = set_references(&original, placement, json!([public_entry()])).unwrap();
        for slide in changed["document"]["deck"]["slides"].as_array().unwrap() {
            for element in slide["elements"].as_array().unwrap() {
                if !element["format"]["hyperlink"].is_null() { assert_eq!(element["text"], public_entry()["url"]); }
                if slide["id"].as_str().unwrap().starts_with("aislide-ref-page-") { assert_eq!(element["color"], "123456"); }
            }
        }
    }
}

#[test]
fn review_collision_warning_identifies_elements_and_explicit_repair() {
    let changed = set_references(&new_document(), "auto", json!([public_entry()])).unwrap();
    let original = &changed["document"];
    let mut overlap = original["deck"]["slides"][0]["elements"][1].clone();
    overlap["id"] = json!("overlapping-body"); overlap["text"] = json!("Body"); overlap["format"] = json!({});
    let edited = edit(original, json!([{"op":"add","path":"/deck/slides/0/elements/-","value":overlap}])).unwrap();
    let checked = aislide_core::execute_request(json!({"op":"preflight_presentation","document":edited["document"]})).unwrap();
    let warning = checked["findings"].as_array().unwrap().iter().find(|finding| finding["code"] == "REFERENCE_COLLISION").unwrap();
    assert_eq!(warning["slide_id"], "slide-1");
    assert!(warning["element_ids"].as_array().unwrap().contains(&json!("overlapping-body")));
    assert!(warning["suggestions"].to_string().contains("set_references"));
    let restored = aislide_core::execute_request(json!({"op":"undo_transaction","document":edited["document"],"expected_revision":edited["document"]["revision"],"receipt":edited["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], original["hash"]);
}

#[test]
fn review_twenty_six_slide_briefing_footer_layout_remains_usable() {
    for placement in ["auto", "appendix"] {
        let mut deck = scene();
        let source_slide = deck["slides"][0].clone();
        let slides: Vec<_> = (1..=26).map(|number| {
            let mut slide = source_slide.clone(); slide["id"] = json!(format!("slide-{number}"));
            slide["elements"].as_array_mut().unwrap().extend([
                json!({"type":"text","id":"body","x":64,"y":240,"width":1152,"height":360,"text":"Synthetic technical briefing content","font_size":24,"color":"202525","bold":false}),
                json!({"type":"text","id":"source","x":64,"y":680,"width":1050,"height":24,"text":"Microsoft Learn","font_size":16,"color":"202525","bold":false}),
                json!({"type":"text","id":"page","x":1160,"y":680,"width":56,"height":24,"text":number.to_string(),"font_size":16,"color":"202525","bold":false}),
            ]); slide
        }).collect();
        deck["slides"] = json!(slides);
        let original = serde_json::to_value(document::create_with_profile("briefing-26".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None, aislide_core::limits::CapacityProfile::Large).unwrap()).unwrap();
        let mut entry = public_entry(); entry["slide_ids"] = json!((1..=26).map(|number| format!("slide-{number}")).collect::<Vec<_>>());
        let changed = set_references(&original, placement, json!([entry])).unwrap();
        assert_eq!(changed["document"]["deck"]["slides"].as_array().unwrap().len(), 27);
        for page in 0..26 {
            assert_eq!(&changed["document"]["deck"]["slides"][page]["elements"].as_array().unwrap()[..4], original["deck"]["slides"][page]["elements"].as_array().unwrap());
        }
        for first in (0..27).step_by(8) {
            let checked = aislide_core::execute_request(json!({"op":"preflight_presentation","document":changed["document"],"options":{"page_indices":(first..(first+8).min(27)).collect::<Vec<_>>()}})).unwrap();
            assert!(!checked["findings"].as_array().unwrap().iter().any(|finding| ["REFERENCE_COLLISION","TEXT_OVERLAP","OFF_SLIDE","SOURCE_URL_NOT_VISIBLE","TEXT_OVERFLOW"].contains(&finding["code"].as_str().unwrap())));
        }
    }
}

#[test]
fn notes_only_reference_urls_warn_without_publishing_them() {
    let document = document::create("references-test".into(), serde_json::from_value(scene()).unwrap(), vec![], vec![], None).unwrap();
    let before = serde_json::to_value(&document).unwrap();
    let report = preflight_presentation(&document, &PreflightOptions::default()).unwrap();
    assert!(report.findings.iter().any(|finding| finding.code == "SOURCE_URL_NOT_VISIBLE" && finding.slide_id == "slide-1"));
    assert_eq!(serde_json::to_value(&document).unwrap(), before);
}

#[test]
fn references_require_explicit_publication_and_preserve_notes_and_body() {
    let original = new_document();
    let changed = set_references(&original, "auto", json!([public_entry(), {"id":"private","name":"Private","url":"file:///private/secret.pdf","slide_ids":["slide-1"]}])).unwrap();
    let updated = &changed["document"];
    assert_eq!(updated["deck"]["slides"][0]["notes"], original["deck"]["slides"][0]["notes"]);
    assert_eq!(updated["deck"]["slides"][0]["elements"][0], original["deck"]["slides"][0]["elements"][0]);
    assert!(!updated.to_string().contains("secret.pdf"));
    let elements = updated["deck"]["slides"][0]["elements"].as_array().unwrap();
    assert!(elements.iter().any(|element| element["text"].as_str().is_some_and(|text| text.contains("https://learn.microsoft.com/azure/")) && element["format"]["hyperlink"] == "https://learn.microsoft.com/azure/"));
    let restored = aislide_core::execute_request(json!({"op":"undo_transaction","document":updated,"expected_revision":updated["revision"],"receipt":changed["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], original["hash"]);
}

#[test]
fn reference_appendix_is_linked_visible_and_idempotent() {
    let changed = set_references(&new_document(), "appendix", json!([public_entry()])).unwrap();
    let updated = &changed["document"];
    assert_eq!(updated["deck"]["slides"].as_array().unwrap().len(), 2);
    assert!(updated["deck"]["slides"][0]["elements"].to_string().contains("[1]"));
    assert!(updated["deck"]["slides"][1]["elements"].to_string().contains("https://learn.microsoft.com/azure/"));
    let repeated = set_references(updated, "appendix", json!([public_entry()])).unwrap();
    assert_eq!(repeated["document"]["hash"], updated["hash"]);
    assert!(repeated["receipt"].is_null());
    let report = aislide_core::execute_request(json!({"op":"preflight_presentation","document":updated,"options":{"page_indices":[0]}})).unwrap();
    assert!(!report["findings"].as_array().unwrap().iter().any(|finding| finding["code"] == "SOURCE_URL_NOT_VISIBLE"));
}

#[test]
fn references_follow_slide_reordering_and_deletion_in_one_undo() {
    let mut deck = scene();
    let mut second = deck["slides"][0].clone();
    second["id"] = json!("slide-2");
    deck["slides"].as_array_mut().unwrap().push(second);
    let original = serde_json::to_value(document::create("references-test".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let changed = set_references(&original, "appendix", json!([public_entry()])).unwrap();
    let updated = &changed["document"];
    let reordered = aislide_core::execute_request(json!({"op":"transaction","document":updated,"transaction":{"expected_revision":updated["revision"],"expected_hash":updated["hash"],"operations":[{"op":"move","from":"/deck/slides/0","path":"/deck/slides/1"}]}})).unwrap();
    let reordered_doc = &reordered["document"];
    assert_eq!(reordered_doc["deck"]["slides"][1]["id"], "slide-1");
    assert!(reordered_doc["deck"]["slides"][1]["elements"].to_string().contains("[1]"));
    let deleted = aislide_core::execute_request(json!({"op":"transaction","document":reordered_doc,"transaction":{"expected_revision":reordered_doc["revision"],"expected_hash":reordered_doc["hash"],"operations":[{"op":"remove","path":"/deck/slides/1"}]}})).unwrap();
    assert_eq!(deleted["document"]["deck"]["slides"].as_array().unwrap().len(), 1);
    let restored = aislide_core::execute_request(json!({"op":"undo_transaction","document":deleted["document"],"expected_revision":deleted["document"]["revision"],"receipt":deleted["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], reordered_doc["hash"]);
}

#[test]
fn deleting_an_earlier_source_slide_does_not_renumber_remaining_references() {
    let mut deck = scene();
    let mut second = deck["slides"][0].clone(); second["id"] = json!("slide-2");
    second["elements"][0]["text"] = json!("Second source [2]");
    deck["slides"].as_array_mut().unwrap().push(second);
    let original = serde_json::to_value(document::create("references-test".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let mut second_entry = public_entry(); second_entry["id"] = json!("second"); second_entry["slide_ids"] = json!(["slide-2"]);
    let changed = set_references(&original, "appendix", json!([public_entry(), second_entry])).unwrap();
    let document = &changed["document"];
    let deleted = aislide_core::execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"remove","path":"/deck/slides/0"}]}})).unwrap();
    assert!(deleted["document"]["deck"]["slides"][0]["elements"][1]["text"].as_str().unwrap().contains("[2]"));
    assert!(deleted["document"]["deck"]["slides"][1]["elements"][1]["text"].as_str().unwrap().starts_with("[2]"));
}

#[test]
fn unsafe_public_reference_targets_are_rejected_atomically() {
    for url in ["file:///private/secret.pdf", "https://user:password@example.com/", "javascript:alert(1)", "http://localhost/private", "http://localhost./private", "http://internal.localhost./private", "https://127.0.0.1/private"] {
        let original = new_document();
        let mut entry = public_entry();
        entry["url"] = json!(url);
        assert!(set_references(&original, "auto", json!([entry])).is_err());
        assert_eq!(original, new_document());
    }
}

#[test]
fn references_survive_pptx_reopen_and_further_edits() {
    use base64::Engine;
    let changed = set_references(&new_document(), "appendix", json!([public_entry()])).unwrap();
    let exported = aislide_core::execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let bytes = base64::engine::general_purpose::STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap();
    let package = aislide_core::package::Package::open(bytes).unwrap();
    assert!(package.text("ppt/slides/slide2.xml").unwrap().contains("https://learn.microsoft.com/azure/"));
    assert!(package.text("ppt/slides/_rels/slide2.xml.rels").unwrap().contains("https://learn.microsoft.com/azure/"));
    let reopened = aislide_core::execute_request(json!({"op":"open_presentation","id":"reopened","base64":exported["base64"]})).unwrap();
    let document = &reopened["document"];
    assert!(document["references"].is_object());
    let edited = aislide_core::execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/slides/0/title","value":"Edited title"}]}})).unwrap();
    assert_eq!(edited["document"]["deck"]["slides"].as_array().unwrap().len(), 2);
}

#[test]
fn reference_pdf_has_readable_url_and_uri_annotation() {
    use base64::Engine;
    let changed = set_references(&new_document(), "appendix", json!([public_entry()])).unwrap();
    let document: document::Document = serde_json::from_value(changed["document"].clone()).unwrap();
    let options = serde_json::from_value(json!({"format":"pdf"})).unwrap();
    let exported = aislide_core::export_static::export_static(&document.deck, &options).unwrap();
    let pdf = lopdf::Document::load_mem(&exported.artifacts[0].bytes).unwrap();
    assert!(pdf.extract_text(&[2]).unwrap().contains("https://learn.microsoft.com/azure/"));
    let page_id = pdf.get_pages()[&2];
    let annotations = pdf.get_object(page_id).unwrap().as_dict().unwrap().get(b"Annots").unwrap().as_array().unwrap();
    assert!(annotations.iter().any(|value| {
        let annotation = pdf.get_object(value.as_reference().unwrap()).unwrap().as_dict().unwrap();
        annotation.get(b"A").unwrap().as_dict().unwrap().get(b"URI").unwrap().as_str().unwrap() == b"https://learn.microsoft.com/azure/"
    }));
    let encoded = base64::engine::general_purpose::STANDARD.encode(&exported.artifacts[0].bytes);
    assert!(!encoded.is_empty());
}

#[test]
fn many_long_references_paginate_without_body_or_font_shrinking() {
    let entries: Vec<_> = (0..12).map(|index| json!({"id":format!("learn-{index}"),"name":format!("Microsoft Learn {index}"),"url":format!("https://learn.microsoft.com/azure/{}", "long-path/".repeat(40)),"slide_ids":["slide-1"],"publish":true})).collect();
    let original = new_document();
    let changed = set_references(&original, "auto", json!(entries)).unwrap();
    let deck = &changed["document"]["deck"];
    assert!(deck["slides"].as_array().unwrap().len() > 2);
    assert_eq!(deck["slides"][0]["elements"][0], original["deck"]["slides"][0]["elements"][0]);
    let measured = aislide_core::execute_request(json!({"op":"measure_layout","deck":deck})).unwrap();
    assert!(!measured["measurements"].as_array().unwrap().iter().any(|measurement| measurement["overflow"] == true));
    for slide in deck["slides"].as_array().unwrap() {
        for element in slide["elements"].as_array().unwrap() { assert!(element["font_size"].as_f64().unwrap() >= 16.0); }
    }
}

#[test]
fn full_slide_body_must_not_be_mistaken_for_a_background() {
    let mut deck = scene();
    for (key, value) in [("x", 0), ("y", 0), ("width", 1280), ("height", 720)] { deck["slides"][0]["elements"][0][key] = json!(value); }
    let original = serde_json::to_value(document::create("references-test".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let error = set_references(&original, "auto", json!([public_entry()])).unwrap_err().to_string();
    assert!(error.contains("slide-1") && error.contains("title") && error.contains("set_references"));
}

#[test]
fn hidden_off_slide_and_link_only_urls_do_not_satisfy_visibility() {
    for mode in ["hidden", "off_slide", "link_only"] {
        let mut deck = scene();
        let mut text = deck["slides"][0]["elements"][0].clone();
        text["id"] = json!("source"); text["y"] = json!(300);
        text["text"] = json!("https://learn.microsoft.com/azure/");
        if mode == "hidden" { text["visual"] = json!({"hidden":true}); }
        if mode == "off_slide" { text["x"] = json!(1200); text["width"] = json!(70); text["visual"] = json!({"rotation":90}); }
        if mode == "link_only" { text["text"] = json!("Microsoft Learn"); text["format"] = json!({"hyperlink":"https://learn.microsoft.com/azure/"}); }
        deck["slides"][0]["elements"].as_array_mut().unwrap().push(text);
        let document = document::create("visibility".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap();
        assert!(preflight_presentation(&document, &PreflightOptions::default()).unwrap().findings.iter().any(|finding| finding.code == "SOURCE_URL_NOT_VISIBLE"), "{mode}");
    }
}

#[test]
fn direct_managed_edits_reject_and_clearing_references_restores_body() {
    let original = new_document();
    let changed = set_references(&original, "appendix", json!([public_entry()])).unwrap();
    let document = &changed["document"];
    assert!(aislide_core::execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/slides/0/elements/1/text","value":"Changed manually"}]}})).is_err());
    assert_eq!(set_references(document, "auto", json!([])).unwrap()["document"]["hash"], original["hash"]);
    assert!(aislide_core::execute_request(json!({"op":"set_references","document":document,"expected_revision":0,"expected_hash":original["hash"],"options":{"entries":[public_entry()]}})).is_err());
}

#[test]
fn managed_appendix_page_edits_are_not_silently_discarded() {
    let changed = set_references(&new_document(), "appendix", json!([public_entry()])).unwrap();
    let document = &changed["document"];
    assert!(aislide_core::execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/slides/1/background","value":"000000"}]}})).is_err());
}

#[test]
fn multiline_appendix_title_reserves_measured_height() {
    let document = new_document();
    let changed = aislide_core::execute_request(json!({"op":"set_references","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"options":{"placement":"appendix","title":"References\nDocumentation\nDistribution\nSources","entries":[public_entry()]}})).unwrap();
    let measured = aislide_core::execute_request(json!({"op":"measure_layout","deck":changed["document"]["deck"]})).unwrap();
    assert!(!measured["measurements"].as_array().unwrap().iter().any(|measurement| measurement["overflow"] == true));
    let checked = aislide_core::execute_request(json!({"op":"preflight_presentation","document":changed["document"],"options":{"page_indices":[1]}})).unwrap();
    assert!(!checked["findings"].as_array().unwrap().iter().any(|finding| finding["code"] == "TEXT_OVERLAP"));
}

#[test]
fn reference_footnotes_have_contrast_on_dark_slides() {
    let mut deck = scene();
    deck["slides"][0]["background"] = json!("101010");
    deck["slides"][0]["elements"][0]["color"] = json!("FFFFFF");
    let original = serde_json::to_value(document::create("references-test".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let changed = set_references(&original, "auto", json!([public_entry()])).unwrap();
    let color = changed["document"]["deck"]["slides"][0]["elements"][1]["color"].as_str().unwrap();
    assert!(aislide_core::review::contrast_ratio(color, "101010").unwrap() >= 4.5);
    assert_eq!(changed["document"]["deck"]["slides"].as_array().unwrap().len(), 2, "PowerPoint uses theme hyperlink colors, so low-contrast links belong on a separate white page");
}

#[test]
fn appendix_uses_empty_layout_instead_of_first_decorated_layout() {
    let mut deck = scene();
    let mut design = aislide_core::execute_request(json!({"op":"design_defaults"})).unwrap();
    design["layouts"].as_array_mut().unwrap().swap(0, 1);
    design["layouts"][0]["elements"].as_array_mut().unwrap().push(json!({"type":"text","id":"layout-label","x":64,"y":104,"width":1152,"height":100,"text":"Decorated layout label","font_size":32,"color":"202525","bold":false}));
    deck["design"] = design;
    let original = serde_json::to_value(document::create("references-test".into(), serde_json::from_value(deck).unwrap(), vec![], vec![], None).unwrap()).unwrap();
    let changed = set_references(&original, "appendix", json!([public_entry()])).unwrap();
    assert_eq!(changed["document"]["deck"]["slides"][1]["layout_id"], "blank");
    let capabilities = aislide_core::execute_request(json!({"op":"authoring_capabilities"})).unwrap();
    assert!(capabilities["operations"].as_array().unwrap().contains(&json!("set_references")));
}

#[test]
#[ignore = "requires a PDF explicitly produced by PowerPoint from the reference fixture"]
fn powerpoint_pdf_preserves_reference_text_and_link_targets() {
    let path = std::env::var("AISLIDE_POWERPOINT_REFERENCE_PDF").expect("set the generated PowerPoint PDF path");
    let pdf = lopdf::Document::load(path).unwrap();
    assert_eq!(pdf.get_pages().len(), 2);
    let text = pdf.extract_text(&[2]).unwrap();
    let mut targets = Vec::new();
    for page_id in pdf.get_pages().into_values() {
        let page = pdf.get_object(page_id).unwrap().as_dict().unwrap();
        let Ok(annotations) = page.get(b"Annots") else { continue; };
        for annotation in pdf.dereference(annotations).unwrap().1.as_array().unwrap() {
            let annotation = pdf.dereference(annotation).unwrap().1.as_dict().unwrap();
            let Ok(action) = annotation.get(b"A") else { continue; };
            let action = pdf.dereference(action).unwrap().1.as_dict().unwrap();
            let Ok(uri) = action.get(b"URI") else { continue; };
            let bytes = uri.as_str().unwrap();
            let target = if bytes.starts_with(&[0xfe, 0xff]) {
                String::from_utf16(&bytes[2..].chunks_exact(2).map(|pair| u16::from_be_bytes([pair[0], pair[1]])).collect::<Vec<_>>()).unwrap()
            } else { String::from_utf8(bytes.to_vec()).unwrap() };
            targets.push(target);
        }
    }
    for url in ["https://learn.microsoft.com/azure/", "https://learn.microsoft.com/azure/architecture/"] {
        assert!(text.contains(url), "readable URL absent: {url}");
        assert!(targets.iter().any(|target| target == url), "PDF link target absent: {url}");
    }
}