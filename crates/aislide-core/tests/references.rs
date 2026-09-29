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
    assert!(set_references(&original, "auto", json!([public_entry()])).is_err());
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