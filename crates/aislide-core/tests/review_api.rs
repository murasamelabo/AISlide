use aislide_core::protocol::execute_request;
use serde_json::{Value, json};

fn text_element() -> Value {
    let mut element = execute_request(json!({"op":"create_object","id":"text","kind":"text"})).unwrap();
    element["text"] = json!("Alpha Beta");
    element
}

#[test]
fn stateless_text_helpers_validate_and_return_only_an_element() {
    let element = text_element();
    let formatted = execute_request(json!({"op":"format_text_element","element":element,"start":0,"end":5,"style":{"bold":true}})).unwrap();
    assert_eq!(formatted["format"]["paragraphs"][0]["runs"][0]["style"]["bold"], true);
    assert!(formatted.get("document").is_none());
    assert!(element.get("revision").is_none());
    let replaced = execute_request(json!({"op":"replace_element_text","element":formatted,"text":"Alpha Gamma"})).unwrap();
    assert_eq!(replaced["text"], "Alpha Gamma");
    assert_eq!(replaced["format"]["paragraphs"][0]["runs"][0]["style"]["bold"], true);
    assert_eq!(element["text"], "Alpha Beta");
    for op in ["format_text_element", "replace_element_text"] {
        let mut request = if op == "format_text_element" { json!({"op":op,"element":element,"start":0,"end":5,"style":{}}) }
            else { json!({"op":op,"element":element,"text":"changed"}) };
        request["element"]["width"] = json!(-1);
        assert!(execute_request(request).is_err());
    }
    assert!(execute_request(json!({"op":"format_text_element","element":element,"start":0,"end":99,"style":{"bold":true}})).is_err());
    assert!(execute_request(json!({"op":"replace_element_text","element":element,"text":"changed","document":{}})).is_err());
    let mut inherited = element.clone();
    inherited["format"] = json!({"placeholder":{"kind":"body","index":99},"inherit_layout":true});
    let replaced = execute_request(json!({"op":"replace_element_text","element":inherited,"text":"Other layout"})).unwrap();
    assert_eq!(replaced["format"]["placeholder"]["index"], 99);
    assert_eq!(replaced["format"]["inherit_layout"], true);
}

fn document() -> Value {
    let mut document = execute_request(json!({"op":"create_presentation","id":"review-api","title":"Synthetic review"})).unwrap();
    document["deck"]["slides"][0]["elements"] = json!([text_element()]);
    document["deck"]["slides"][0]["notes"] = json!("PRIVATE_SYNTHETIC_NOTE");
    execute_request(json!({"op":"new_document","id":"review-api","deck":document["deck"]})).unwrap()
}

fn change(document: &Value, op: &str, mut input: Value) -> Value {
    input["op"] = json!(op); input["document"] = document.clone(); input["expected_revision"] = document["revision"].clone();
    execute_request(input).unwrap()
}

fn comment(id: &str) -> Value {
    json!({"id":id,"author":"Local synthetic reviewer","initials":"LS","timestamp":"2026-09-17T10:15:00Z","text":"PRIVATE_SYNTHETIC_COMMENT"})
}

#[test]
fn review_protocol_mutations_have_verified_inverses_and_bounded_inspection() {
    let original = document();
    let added = change(&original, "add_comment", json!({"slide_id":"slide-1","comment":comment("first")}));
    let replied = change(&added["document"], "reply_comment", json!({"slide_id":"slide-1","parent_id":"first","comment":comment("reply")}));
    let resolved = change(&replied["document"], "resolve_comment", json!({"slide_id":"slide-1","comment_id":"first","resolved":true}));
    assert_eq!(resolved["document"]["revision"], 3);
    let accessible = change(&resolved["document"], "set_accessibility", json!({"slide_id":"slide-1","element_id":"text","metadata":{"title":"Synthetic text","description":"Synthetic description"}}));
    let ordered = change(&accessible["document"], "set_reading_order", json!({"slide_id":"slide-1","order":["text"]}));
    assert!(ordered["warnings"].as_array().unwrap().iter().any(|warning| warning.as_str().unwrap().contains("z-order")));
    let report = execute_request(json!({"op":"check_accessibility","document":ordered["document"]})).unwrap();
    assert_eq!(report["wcag_certified"], false);
    let inspection = execute_request(json!({"op":"inspect_document","document":ordered["document"]})).unwrap();
    assert_eq!(inspection["complete_personal_data_detection"], false);
    assert!(!inspection.to_string().contains("PRIVATE_SYNTHETIC"));
    let removed = change(&ordered["document"], "remove_comment", json!({"slide_id":"slide-1","comment_id":"first"}));
    assert!(removed["document"]["deck"]["slides"][0]["review"].get("comments").is_none());
    let undone = execute_request(json!({"op":"undo_transaction","document":removed["document"],"expected_revision":removed["document"]["revision"],"receipt":removed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], ordered["document"]["hash"]);
    let reopened_state = change(&ordered["document"], "resolve_comment", json!({"slide_id":"slide-1","comment_id":"first","resolved":false}));
    assert_eq!(reopened_state["document"]["deck"]["slides"][0]["review"]["comments"][0]["resolved"], false);
    assert!(execute_request(json!({"op":"add_comment","document":original,"expected_revision":9,"slide_id":"slide-1","comment":comment("stale")})).is_err());
    let mut tampered = original.clone(); tampered["deck"]["title"] = json!("forged");
    assert!(execute_request(json!({"op":"inspect_document","document":tampered})).is_err());
}

#[test]
fn field_refresh_and_clean_copy_are_explicit_and_never_replace_the_source() {
    let original = document();
    let field = json!({"id":"{00112233-4455-6677-8899-aabbccddeeff}","kind":"slidenum"});
    let with_field = change(&original, "update_paragraphs", json!({"slide_id":"slide-1","id":"text","paragraphs":[{"runs":[{"text":"Page "},{"text":"9","field":field}]}]}));
    let refreshed = change(&with_field["document"], "refresh_fields", json!({"reference_date":"2026-09-17"}));
    let element = &refreshed["document"]["deck"]["slides"][0]["elements"][0];
    assert_eq!(element["text"], "Page 1");
    assert_eq!(element["format"]["paragraphs"][0]["runs"][1]["field"], field);
    let undone = execute_request(json!({"op":"undo_transaction","document":refreshed["document"],"expected_revision":2,"receipt":refreshed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], with_field["document"]["hash"]);
    for options in [json!({"new_document_id":"clean","categories":["notes"],"confirmed":false}), json!({"new_document_id":"review-api","categories":["notes"],"confirmed":true}), json!({"new_document_id":"clean","categories":[],"confirmed":true})] {
        assert!(execute_request(json!({"op":"export_clean_copy","document":original,"options":options})).is_err());
    }
    let clean = execute_request(json!({"op":"export_clean_copy","document":original,"options":{"new_document_id":"clean","categories":["notes"],"confirmed":true}})).unwrap();
    assert_eq!(clean["document"]["id"], "clean"); assert_eq!(clean["document"]["revision"], 0);
    assert_eq!(clean["document"]["deck"]["slides"][0]["notes"], "");
    assert_eq!(original["deck"]["slides"][0]["notes"], "PRIVATE_SYNTHETIC_NOTE");
    assert!(clean["base64"].is_string()); assert!(clean.get("bytes").is_none());
}

#[test]
fn protocol_refuses_cleaning_protected_namespaces_without_touching_the_source() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use aislide_core::package::Package;
    let original = document();
    let exported = execute_request(json!({"op":"export","deck":original["deck"]})).unwrap();
    let bytes = STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap();
    for namespace in ["http://schemas.microsoft.com/office/2020/mipLabelMetadata", "urn:synthetic:sensitivitylabel"] {
        let mut package = Package::open(bytes.clone()).unwrap();
        let properties = format!("<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\"><label:protected xmlns:label=\"{namespace}\"/></cp:coreProperties>");
        package.replace_part("docProps/core.xml", properties.into_bytes()).unwrap();
        let encoded = STANDARD.encode(package.save().unwrap());
        let opened = execute_request(json!({"op":"open_presentation","id":"protected-fixture","base64":encoded})).unwrap();
        let document = &opened["document"];
        let before = document["hash"].clone();
        assert!(execute_request(json!({"op":"export_clean_copy","document":document,"options":{"new_document_id":"clean","categories":["custom_xml","notes","comments","sources"],"confirmed":true}})).is_err());
        assert!(execute_request(json!({"op":"refresh_fields","document":document,"expected_revision":0,"reference_date":"2026-09-17"})).is_err());
        assert_eq!(document["hash"], before);
        assert_eq!(execute_request(json!({"op":"export_presentation","document":document})).unwrap()["base64"], encoded);
    }
}

#[test]
fn native_field_refresh_preserves_origin_and_undo_restores_original_archive() {
    let original = document();
    let field = json!({"id":"{00112233-4455-6677-8899-aabbccddeeff}","kind":"slidenum"});
    let with_field = change(&original, "update_paragraphs", json!({"slide_id":"slide-1","id":"text","paragraphs":[{"runs":[{"text":"9","field":field}]}]}));
    let exported = execute_request(json!({"op":"export_presentation","document":with_field["document"]})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"native-fields","base64":exported["base64"]})).unwrap();
    let refreshed = change(&opened["document"], "refresh_fields", json!({"reference_date":"2026-09-17"}));
    assert_eq!(refreshed["document"]["deck"]["slides"][0]["elements"][0]["text"], "1");
    assert_eq!(refreshed["document"]["origin"], opened["document"]["origin"]);
    let saved = execute_request(json!({"op":"export_presentation","document":refreshed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"refreshed","base64":saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][0]["elements"][0]["text"], "1");
    let noop = change(&refreshed["document"], "refresh_fields", json!({"reference_date":"2026-09-17"}));
    assert!(noop["receipt"].is_null());
    assert_eq!(noop["document"], refreshed["document"]);
    let undone = execute_request(json!({"op":"undo_transaction","document":refreshed["document"],"expected_revision":1,"receipt":refreshed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], opened["document"]["hash"]);
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], exported["base64"]);
    assert_eq!(execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap()["base64"], exported["base64"]);
}

fn native_field_fixture() -> Vec<u8> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use aislide_core::package::Package;
    let mut deck = document()["deck"].clone();
    let body = &mut deck["slides"][0]["elements"][0];
    body["text"] = json!("\u{1f600} Page 9 | old | vendor | 13\nStable text");
    body["format"] = json!({"paragraphs":[{"alignment":"center", "space_after":{"kind":"points","value":120}, "runs":[
        {"text":"\u{1f600} Page ","style":{"bold":true}},
        {"text":"9","style":{"italic":true,"highlight":"FFFF00"},"field":{"id":"{00112233-4455-6677-8899-AABBCCDDEE01}","kind":"slidenum"}},
        {"text":" | "},
        {"text":"old","style":{"baseline":1200,"language":"en-US"},"field":{"id":"{00112233-4455-6677-8899-AABBCCDDEE02}","kind":"datetime1"}},
        {"text":" | "},
        {"text":"vendor","field":{"id":"{00112233-4455-6677-8899-AABBCCDDEE03}","kind":"vendor-kind"}},
        {"text":" | "},
        {"text":"13","field":{"id":"{00112233-4455-6677-8899-AABBCCDDEE04}","kind":"datetime13"}}
    ]},{"alignment":"right","runs":[{"text":"Stable text","style":{"underline":true}}]}]});
    let mut late = text_element();
    late["id"] = json!("late");
    deck["slides"][0]["elements"].as_array_mut().unwrap().push(late);
    let mut second = deck["slides"][0].clone(); second["id"] = json!("slide-2");
    deck["slides"].as_array_mut().unwrap().push(second);
    let exported = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let mut package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    for path in ["ppt/slides/slide1.xml", "ppt/slides/slide2.xml"] {
        let mut xml = package.text(path).unwrap().replace("<a:fld ", "<a:fld xmlns:v=\"urn:field-test\" v:unknown=\"keep\" ")
            .replace("</a:fld>", "<a:extLst><a:ext uri=\"urn:field-test\"><v:keep value=\"exact\"/></a:ext></a:extLst></a:fld>")
            .replace("</a:rPr>", "<a:effectLst><a:glow rad=\"12700\"><a:srgbClr val=\"123456\"/></a:glow></a:effectLst></a:rPr>");
        let parsed = roxmltree::Document::parse(&xml).unwrap();
        let mut positions: Vec<_> = parsed.descendants().filter(|node| node.tag_name().name() == "t" && node.parent().is_some_and(|parent| parent.tag_name().name() == "fld"))
            .map(|node| node.range().start + xml[node.range()].find('>').unwrap()).collect();
        assert_eq!(positions.len(), 4);
        positions.sort_unstable();
        for position in positions.into_iter().rev() { xml.insert_str(position, " xml:space=\"preserve\" v:cache=\"keep\""); }
        package.replace_part(path, xml.into_bytes()).unwrap();
    }
    package.save().unwrap()
}

fn open_field_fixture(bytes: &[u8]) -> aislide_core::document::Document {
    serde_json::from_value(aislide_core::document::open_presentation("native-field-fixture".into(), bytes.to_vec()).unwrap()["document"].clone()).unwrap()
}

fn save_field_fixture(document: &aislide_core::document::Document) -> Vec<u8> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD.decode(aislide_core::document::export_presentation(document).unwrap()["base64"].as_str().unwrap()).unwrap()
}

fn without_supported_caches(xml: &str) -> String {
    let parsed = roxmltree::Document::parse(xml).unwrap();
    let mut ranges: Vec<_> = parsed.descendants().filter(|node| node.tag_name().name() == "fld" && matches!(node.attribute("type"), Some("slidenum" | "datetime1")))
        .flat_map(|node| node.children().filter(|node| node.tag_name().name() == "t"))
        .flat_map(|node| node.children().filter(|node| node.is_text()).map(|node| node.range())).collect();
    ranges.sort_by_key(|range| range.start);
    let mut masked = xml.to_owned();
    for range in ranges.into_iter().rev() { masked.replace_range(range, "CACHE"); }
    masked
}

#[test]
fn native_fields_refresh_only_caches_preserving_ids_effects_extensions_paragraphs_and_notes() {
    use aislide_core::{authoring_ops, document as documents, fields, model, package::Package};
    let bytes = native_field_fixture();
    let original = open_field_fixture(&bytes);
    let refreshed = authoring_ops::refresh_fields(&original, 0, "2024-02-29").unwrap();
    documents::verify(&refreshed.document).unwrap();
    model::validate_deck(&refreshed.document.deck).unwrap();
    let saved = save_field_fixture(&refreshed.document);
    let reopened = open_field_fixture(&saved);
    for (index, slide) in reopened.deck.slides.iter().enumerate() {
        assert_eq!(serde_json::to_value(&slide.elements[0]).unwrap()["text"], format!("\u{1f600} Page {} | 2/29/2024 | vendor | 13\nStable text", index + 1));
        assert_eq!(slide.notes, original.deck.slides[index].notes);
    }
    let before = Package::open(bytes.clone()).unwrap(); let after = Package::open(saved.clone()).unwrap();
    assert_eq!(before.parts().keys().collect::<Vec<_>>(), after.parts().keys().collect::<Vec<_>>());
    for (path, data) in before.parts() {
        if ["ppt/slides/slide1.xml", "ppt/slides/slide2.xml"].contains(&path.as_str()) {
            assert_eq!(without_supported_caches(before.text(path).unwrap()), without_supported_caches(after.text(path).unwrap()), "{path}");
        } else { assert_eq!(after.part(path).unwrap(), data, "{path}"); }
    }
    assert_eq!(fields::refresh_native(bytes.clone(), "2024-02-29").unwrap(), saved);
    assert_eq!(fields::refresh_native(saved.clone(), "2024-02-29").unwrap(), saved);
    let noop = authoring_ops::refresh_fields(&refreshed.document, 1, "2024-02-29").unwrap();
    assert!(noop.receipt.is_none()); assert!(noop.changes.is_empty());
    assert_eq!(noop.document.hash, refreshed.document.hash);
    let undone = documents::undo(&refreshed.document, 1, refreshed.receipt.unwrap()).unwrap();
    assert_eq!(save_field_fixture(&undone.document), bytes);
    assert_eq!(save_field_fixture(&original), bytes);
}

#[test]
fn native_fields_follow_reordered_slide_numbers_with_independent_undo() {
    use aislide_core::{authoring_ops, document as documents, editing::{self, SlideOperation}};
    let bytes = native_field_fixture(); let original = open_field_fixture(&bytes);
    let reordered = editing::slides(&original, 0, &[SlideOperation::Move { slide_id:original.deck.slides[1].id.clone(), index:0 }]).unwrap();
    let reordered_bytes = save_field_fixture(&reordered.document);
    let refreshed = authoring_ops::refresh_fields(&reordered.document, 1, "2026-09-17").unwrap();
    let reopened = open_field_fixture(&save_field_fixture(&refreshed.document));
    assert_eq!(reopened.deck.slides[0].id, original.deck.slides[1].id);
    for (index, slide) in reopened.deck.slides.iter().enumerate() {
        assert_eq!(serde_json::to_value(&slide.elements[0]).unwrap()["text"], format!("\u{1f600} Page {} | 9/17/2026 | vendor | 13\nStable text", index + 1));
    }
    let undone_refresh = documents::undo(&refreshed.document, 2, refreshed.receipt.unwrap()).unwrap();
    assert_eq!(save_field_fixture(&undone_refresh.document), reordered_bytes);
    let undone_reorder = documents::undo(&undone_refresh.document, 3, reordered.receipt.unwrap()).unwrap();
    assert_eq!(save_field_fixture(&undone_reorder.document), bytes);
}

#[test]
fn native_fields_reject_invalid_dates_stale_requests_and_partial_candidates_atomically() {
    use aislide_core::{authoring_ops, document as documents, fields, model::Element};
    let bytes = native_field_fixture(); let original = open_field_fixture(&bytes);
    for date in ["2023-02-29", "1900-02-29", "0000-01-01", "2026-13-01", "2026-04-31", "2026-09-00", "2026-9-17", "２０２６-09-17"] {
        assert!(authoring_ops::refresh_fields(&original, 0, date).is_err());
        assert!(fields::refresh_native(bytes.clone(), date).is_err());
    }
    assert!(authoring_ops::refresh_fields(&original, 9, "2026-09-17").is_err());
    let mut forged = original.clone(); forged.deck.title = "tampered".into();
    assert!(authoring_ops::refresh_fields(&forged, 0, "2026-09-17").is_err());
    for mutation in ["identity", "kind", "style", "unknown_cache", "late_type", "invalid_model"] {
        let mut candidate = fields::refresh(&original.deck, "2026-09-17").unwrap();
        let Element::Text { text, format, .. } = &mut candidate.slides[0].elements[0] else { panic!() };
        match mutation {
            "identity" => format.paragraphs[0].runs[1].field.as_mut().unwrap().id = "{00112233-4455-6677-8899-AABBCCDDEE99}".into(),
            "kind" => format.paragraphs[0].runs[1].field.as_mut().unwrap().kind = "datetime1".into(),
            "style" => format.paragraphs[0].runs[1].style.bold = Some(true),
            "unknown_cache" => format.paragraphs[0].runs[5].text = "overwritten".into(),
            "invalid_model" => format.paragraphs[0].runs[1].style.font_size = Some(-1.0),
            _ => {}
        }
        *text = aislide_core::rich_text::plain_text(&format.paragraphs);
        if mutation == "late_type" {
            let id = candidate.slides[1].elements[1].bounds().0.to_owned();
            candidate.slides[1].elements[1] = Element::Rect { id, x:0.0, y:0.0, width:100.0, height:100.0, fill:"FFFFFF".into(), visual:None };
        }
        let transaction = documents::Transaction { expected_revision:0, expected_hash:original.hash.clone(), operations:serde_json::from_value(json!([{"op":"replace","path":"/deck","value":candidate}])).unwrap() };
        assert!(documents::transact(&original, transaction).is_err(), "{mutation}");
        assert_eq!(save_field_fixture(&original), bytes, "{mutation}");
    }
}

#[test]
fn native_fields_in_master_or_layout_keep_shared_caches_while_slide_caches_refresh() {
    use aislide_core::{authoring_ops, fields, package::Package};
    let bytes = native_field_fixture();
    for path in ["ppt/slideMasters/slideMaster1.xml", "ppt/slideLayouts/slideLayout1.xml"] {
        let mut package = Package::open(bytes.clone()).unwrap();
        let slide = package.text("ppt/slides/slide1.xml").unwrap();
        let parsed = roxmltree::Document::parse(slide).unwrap();
        let shape = parsed.descendants().find(|node| node.tag_name().name() == "sp").unwrap();
        let xml = package.text(path).unwrap().replace("</p:spTree>", &format!("{}</p:spTree>", &slide[shape.range()]));
        package.replace_part(path, xml.into_bytes()).unwrap();
        let bytes = package.save().unwrap(); let original = open_field_fixture(&bytes);
        let changed = authoring_ops::refresh_fields(&original, 0, "2026-09-17").unwrap();
        let saved = Package::open(save_field_fixture(&changed.document)).unwrap();
        assert_eq!(saved.part(path).unwrap(), package.part(path).unwrap());
        let refreshed = Package::open(fields::refresh_native(bytes.clone(), "2026-09-17").unwrap()).unwrap();
        assert_eq!(refreshed.part(path).unwrap(), package.part(path).unwrap());
        assert_eq!(save_field_fixture(&original), bytes);
    }
}

#[test]
fn rich_notes_and_auxiliary_design_operations_are_atomic_bounded_and_revision_checked() {
    let original = document();
    let paragraphs = json!([{"runs":[{"text":"N".repeat(8000),"style":{"bold":true}}]}]);
    let changed = change(&original, "update_rich_notes", json!({"slide_id":"slide-1","paragraphs":paragraphs}));
    assert_eq!(changed["document"]["deck"]["slides"][0]["notes"].as_str().unwrap().len(), 8000);
    assert_eq!(changed["document"]["revision"], 1);
    for request in [
        json!({"op":"update_rich_notes","document":changed["document"],"expected_revision":0,"slide_id":"slide-1","paragraphs":paragraphs}),
        json!({"op":"update_rich_notes","document":original,"expected_revision":0,"slide_id":"slide-1","paragraphs":[{"runs":[{"text":"N".repeat(8001)}]}]}),
        json!({"op":"update_paragraphs","document":original,"expected_revision":0,"slide_id":"slide-1","id":"text","paragraphs":paragraphs}),
    ] { assert!(execute_request(request).is_err()); }
    let auxiliary = json!({"width":720,"height":960,"notes_master":{"name":"Notes","background":"@lt1","theme":original["deck"]["design"]["theme"],"elements":[]}});
    let updated = change(&changed["document"], "update_auxiliary_design", json!({"design":auxiliary}));
    assert_eq!(updated["document"]["deck"]["auxiliary_design"]["width"], 720);
    let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":2,"receipt":updated["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], changed["document"]["hash"]);
}

#[test]
fn reserved_date_fields_use_explicit_date_and_never_invent_time() {
    for (kind, expected) in [("datetime2", "Friday, September 18, 2026"), ("datetime3", "18 September 2026"), ("datetime4", "September 18, 2026"), ("datetime5", "18-Sep-26"), ("datetime6", "September 26"), ("datetime7", "Sep-26")] {
        let field = aislide_core::fields::Field { id: "00112233-4455-6677-8899-aabbccddeeff".into(), kind: kind.into() };
        assert_eq!(field.cached_text(2, "2026-09-18").unwrap().as_deref(), Some(expected));
    }
    let original = document();
    let paragraphs = (8..=13).map(|number| json!({"runs":[{"text":"retained","field":{"id":format!("00112233-4455-6677-8899-aabbccddee{number:02}"),"kind":format!("datetime{number}")}}]})).collect::<Vec<_>>();
    let with_fields = change(&original, "update_paragraphs", json!({"slide_id":"slide-1","id":"text","paragraphs":paragraphs}));
    let unchanged = change(&with_fields["document"], "refresh_fields", json!({"reference_date":"2026-09-18"}));
    assert_eq!(unchanged["document"]["hash"], with_fields["document"]["hash"]);
    let refreshed = change(&with_fields["document"], "refresh_fields", json!({"reference_date":"2026-09-18","reference_time":"16:28:34","locale":"en-US"}));
    let runs = refreshed["document"]["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"].as_array().unwrap();
    for (paragraph, expected) in runs.iter().zip(["9/18/2026 4:28 PM", "9/18/2026 4:28:34 PM", "16:28", "16:28:34", "4:28 PM", "4:28:34 PM"]) { assert_eq!(paragraph["runs"][0]["text"], expected); }
}