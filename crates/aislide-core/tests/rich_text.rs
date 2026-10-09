use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};
use aislide_core::{model::Element, rich_text::{apply_range, replace_text_content, validate_element, RunStyle}};

fn deck(element: Value) -> Value {
    json!({"version":1,"title":"Rich text","width":1280,"height":720,"slides":[{"id":"slide-1","title":"Rich text","background":"FFFFFF","notes":"","elements":[element]}]})
}

fn text_element() -> Value {
    json!({"type":"text","id":"text-1","x":40,"y":40,"width":1000,"height":600,"text":"日本😀 English","font_size":24,"color":"@dk1","bold":false})
}

#[test]
fn explicit_text_padding_is_native_and_survives_reopening() {
    let padding = json!({"left":24.0,"right":24.0,"top":16.0,"bottom":16.0});
    for shape in [false, true] {
        let mut input = text_element();
        if shape {
            input["type"] = json!("shape"); input["preset"] = json!("roundRect");
            input["fill"] = json!("EEF1FB"); input["stroke"] = json!("087F73"); input["stroke_width"] = json!(1);
        }
        input["format"] = json!({"padding":padding});
        let bytes = export_deck(deck(input));
        let package = Package::open(bytes.clone()).unwrap();
        let xml = package.text("ppt/slides/slide1.xml").unwrap();
        // Shape insets are measured from the roundRect text rectangle (16.667% corner, 29.289% of it inset).
        let inset = if shape { (600.0_f64 * 16667.0 / 100000.0 * 29289.0 / 100000.0 * 9525.0).round() as i64 } else { 0 };
        for (name, value) in [("lIns", 228600), ("rIns", 228600), ("tIns", 152400), ("bIns", 152400)] { let attribute = format!("{name}=\"{}\"", value - inset); assert!(xml.contains(&attribute), "{attribute}"); }
        let document = open(&bytes);
        assert_eq!(document["deck"]["slides"][0]["elements"][0]["format"]["padding"], padding);
        assert_eq!(export_document(&document), bytes);
    }
}

#[test]
fn zero_padding_and_combined_native_text_edits_preserve_insets_and_unknown_properties() {
    let mut shape = text_element();
    shape["type"] = json!("shape"); shape["preset"] = json!("rect"); shape["fill"] = json!("EEEEEE"); shape["stroke"] = json!("000000"); shape["stroke_width"] = json!(1);
    let zero = json!({"left":0.0,"right":0.0,"top":0.0,"bottom":0.0});
    shape["format"] = json!({"padding":zero});
    let bytes = export_deck(deck(shape));
    assert_eq!(open(&bytes)["deck"]["slides"][0]["elements"][0]["format"]["padding"], zero);
    let bytes = body_fixture("<p:txBody><a:bodyPr lIns='152400' rIns='152400' tIns='95250' bIns='95250' rot='0'/><a:lstStyle/><a:p><a:r><a:t>Before</a:t></a:r></a:p></p:txBody>");
    let document = open(&bytes);
    let mut next = document["deck"]["slides"][0]["elements"][0].clone();
    next["text"] = json!("After");
    next["format"]["padding"] = json!({"left":24.0,"right":24.0,"top":12.0,"bottom":12.0});
    let changed = transact(&document, next.clone()).unwrap();
    let saved = export_document(&changed["document"]);
    let reopened = open(&saved);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["text"], "After");
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["format"]["padding"], next["format"]["padding"]);
    assert!(Package::open(saved).unwrap().text("ppt/slides/slide1.xml").unwrap().contains("rot='0'"));
}

#[test]
fn inherited_native_text_and_padding_are_saved_together() {
    let original = execute_request(json!({"op":"create_presentation","id":"inherited-padding","title":"Synthetic inheritance"})).unwrap();
    let layout = original["deck"]["design"]["layouts"].as_array().unwrap().iter().find(|layout| !layout["elements"].as_array().unwrap().is_empty()).unwrap();
    let assigned = execute_request(json!({"op":"assign_layout","deck":original["deck"],"slide_id":"slide-1","layout_id":layout["id"]})).unwrap();
    let document = open(&export_deck(assigned));
    let mut next = document["deck"]["slides"][0]["elements"][0].clone();
    assert_eq!(next["format"]["inherit_layout"], true);
    next["text"] = json!("Synthetic changed title");
    next["format"]["padding"] = json!({"left":12.0,"right":12.0,"top":4.0,"bottom":4.0});
    let changed = execute_request(json!({"op":"apply_operations","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
        {"op":"set_text_padding","slide_id":"slide-1","ids":[next["id"]],"padding":next["format"]["padding"]},
        {"op":"set_rich_text","slide_id":"slide-1","id":next["id"],"paragraphs":[{"runs":[{"text":next["text"]}]}]}
    ]})).unwrap();
    assert_eq!(changed["document"]["deck"]["slides"][0]["elements"][0]["format"]["inherit_layout"], false);
    let reopened = open(&export_document(&changed["document"]));
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["text"], next["text"]);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["format"]["padding"], next["format"]["padding"]);
}

#[test]
fn text_padding_controls_measurement_and_preview_content_frame() {
    for shape in [false, true] {
        let mut input = text_element();
        input["text"] = json!("Padding"); input["width"] = json!(300); input["height"] = json!(100);
        if shape {
            input["type"] = json!("shape"); input["preset"] = json!("roundRect");
            input["fill"] = json!("EEF1FB"); input["stroke"] = json!("087F73"); input["stroke_width"] = json!(1);
        }
        input["format"] = json!({"padding":{"left":24,"right":24,"top":16,"bottom":16}});
        let measured = execute_request(json!({"op":"measure_layout","deck":deck(input.clone())})).unwrap();
        assert_eq!(measured["measurements"][0]["width"].as_f64(), Some(252.0));
        assert_eq!(measured["measurements"][0]["height"].as_f64(), Some(68.0));
        let preview = execute_request(json!({"op":"render_element_preview","element":input})).unwrap();
        assert!(preview["svg"].as_str().unwrap().contains("translate(24 16)"));
    }
    for padding in [json!({"left":-1}), json!({"left":1000}), json!({"top":500,"bottom":100}), json!({"left":1,"unknown":true})] {
        let mut input = text_element(); input["format"] = json!({"padding":padding});
        assert!(execute_request(json!({"op":"export","deck":deck(input)})).is_err());
    }
}

#[test]
fn native_padding_only_edits_preserve_rich_runs_and_undo_exactly() {
    for rich in [false, true] {
        let mut input = text_element();
        input["format"] = json!({"padding":{"left":24,"right":24,"top":16,"bottom":16}});
        if rich { input["format"]["paragraphs"] = json!([{"runs":[{"text":"日本😀 ","style":{"bold":true}},{"text":"English","style":{"italic":true}}]}]); }
        let bytes = export_deck(deck(input));
        let document = open(&bytes);
        let mut next = document["deck"]["slides"][0]["elements"][0].clone();
        next["format"]["padding"] = json!({"left":32.0,"right":32.0,"top":20.0,"bottom":20.0});
        let changed = transact(&document, next.clone()).unwrap();
        let saved = export_document(&changed["document"]);
        let reopened = open(&saved);
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["format"]["padding"], next["format"]["padding"]);
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"], document["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"]);
        let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
        assert_eq!(undone["document"]["hash"], document["hash"]);
        assert_eq!(export_document(&undone["document"]), bytes);
    }
}

#[test]
fn mixed_runs_are_native_and_reopen_with_styles() {
    let mut element = text_element();
    element["format"] = json!({"paragraphs":[{"runs":[{"text":"日本😀 ","style":{"bold":true,"highlight":"FFFF00"}},{"text":"English","style":{"italic":true,"baseline":30000,"language":"en-US"}}]}]});
    let exported = execute_request(json!({"op":"export","deck":deck(element)})).unwrap();
    let bytes = STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap();
    let package = Package::open(bytes.clone()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap();
    assert!(xml.contains("<a:highlight>"));
    assert!(xml.contains("baseline=\"30000\""));
    let opened = execute_request(json!({"op":"open_presentation","id":"rich","base64":STANDARD.encode(bytes)})).unwrap();
    let actual = &opened["document"]["deck"]["slides"][0]["elements"][0];
    assert_eq!(actual["text"], "日本😀 English");
    assert_eq!(actual["format"]["paragraphs"][0]["runs"][0]["style"]["highlight"], "FFFF00");
    assert_eq!(actual["format"]["paragraphs"][0]["runs"][1]["style"]["baseline"], 30000);
}

fn value(element: Element) -> Value { serde_json::to_value(element).unwrap() }
fn element(input: Value) -> Element { serde_json::from_value(input).unwrap() }

#[test]
fn scalar_ranges_cross_paragraphs_preserving_partial_styles() {
    let mut original = text_element(); original["text"] = json!("日😀A\nB語Z");
    let highlighted = apply_range(element(original), 1, 6, RunStyle { highlight: Some("FFFF00".into()), baseline: Some(-25000), ..Default::default() }).unwrap();
    let result = value(apply_range(highlighted, 2, 5, RunStyle { bold: Some(true), ..Default::default() }).unwrap());
    assert_eq!(result["text"], "日😀A\nB語Z");
    let paragraphs = &result["format"]["paragraphs"];
    assert_eq!(paragraphs[0]["runs"][0]["text"], "日");
    assert_eq!(paragraphs[0]["runs"][1]["text"], "😀");
    assert_eq!(paragraphs[0]["runs"][1]["style"]["highlight"], "FFFF00");
    assert_eq!(paragraphs[0]["runs"][2]["style"]["bold"], true);
    assert_eq!(paragraphs[1]["runs"][0]["text"], "B");
    assert_eq!(paragraphs[1]["runs"][0]["style"]["baseline"], -25000);
    assert_eq!(paragraphs[1]["runs"][1]["text"], "語");
    assert_eq!(paragraphs[1]["runs"][2]["style"], json!({}));
}

#[test]
fn replacement_preserves_prefix_suffix_and_inherits_inserted_style() {
    let rich = apply_range(element(text_element()), 0, 3, RunStyle { bold: Some(true), ..Default::default() }).unwrap();
    let rich = apply_range(rich, 4, 11, RunStyle { italic: Some(true), ..Default::default() }).unwrap();
    let replaced = replace_text_content(rich, "日本X😀 English".into()).unwrap();
    let result = value(replaced.clone());
    assert_eq!(result["format"]["paragraphs"][0]["runs"][0]["text"], "日本X😀");
    assert_eq!(result["format"]["paragraphs"][0]["runs"][0]["style"]["bold"], true);
    assert_eq!(result["format"]["paragraphs"][0]["runs"][2]["text"], "English");
    assert_eq!(result["format"]["paragraphs"][0]["runs"][2]["style"]["italic"], true);
    let cleared = replace_text_content(replaced, String::new()).unwrap();
    assert_eq!(value(cleared.clone())["text"], "");
    assert!(validate_element(&cleared).is_ok());
    let inserted = replace_text_content(cleared, "新\n文\n".into()).unwrap();
    assert!(validate_element(&inserted).is_ok());
    assert_eq!(value(inserted)["format"]["paragraphs"].as_array().unwrap().len(), 3);
}

#[test]
fn empty_and_legacy_noops_do_not_add_paragraphs() {
    let original = element(text_element());
    assert_eq!(value(apply_range(original.clone(), 0, 0, RunStyle { bold: Some(true), ..Default::default() }).unwrap()), value(original.clone()));
    assert_eq!(value(replace_text_content(original.clone(), "日本😀 English".into()).unwrap()), value(original));
    let mut empty = text_element(); empty["text"] = json!("");
    assert_eq!(value(replace_text_content(element(empty.clone()), String::new()).unwrap()), value(element(empty)));
}

#[test]
fn invalid_styles_ranges_and_plain_rich_mismatches_reject() {
    let original = element(text_element());
    assert!(apply_range(original.clone(), 3, 2, RunStyle::default()).is_err());
    assert!(apply_range(original.clone(), 0, usize::MAX, RunStyle::default()).is_err());
    for style in [RunStyle { font_size: Some(f64::NAN), ..Default::default() }, RunStyle { font_size: Some(-1.0), ..Default::default() }, RunStyle { highlight: Some("bad".into()), ..Default::default() }, RunStyle { baseline: Some(100001), ..Default::default() }, RunStyle { font_family: Some("@unknown".into()), ..Default::default() }, RunStyle { language: Some("en\nUS".into()), ..Default::default() }] {
        assert!(apply_range(original.clone(), 0, 1, style).is_err());
    }
    let mut mismatched = text_element(); mismatched["format"] = json!({"paragraphs":[{"runs":[{"text":"different"}]}]});
    assert!(validate_element(&element(mismatched.clone())).is_err());
    assert!(replace_text_content(element(mismatched), "replacement".into()).is_err());
    for properties in [json!({"number_start":0,"bullet":"numbered"}), json!({"number_start":1}), json!({"margin_left":-1}), json!({"level":9}), json!({"line_spacing":{"kind":"percent","value":0}}), json!({"tabs":[{"position":10},{"position":5}]})] {
        let mut input = text_element(); input["format"] = json!({"paragraphs":[properties]});
        assert!(serde_json::from_value::<Element>(input).is_err());
    }
}

fn export_deck(input: Value) -> Vec<u8> {
    let exported = execute_request(json!({"op":"export","deck":input})).unwrap();
    STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()
}

fn open(bytes: &[u8]) -> Value {
    execute_request(json!({"op":"open_presentation","id":"rich-native","base64":STANDARD.encode(bytes)})).unwrap()["document"].clone()
}

fn transact(document: &Value, replacement: Value) -> aislide_core::Result<Value> {
    execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/slides/0/elements/0","value":replacement}]}}))
}

fn export_document(document: &Value) -> Vec<u8> {
    let exported = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()
}

#[test]
fn native_range_edit_replacement_and_undo_are_real_roundtrips() {
    let rich = apply_range(element(text_element()), 0, 3, RunStyle { bold: Some(true), ..Default::default() }).unwrap();
    let bytes = export_deck(deck(value(rich)));
    let document = open(&bytes);
    assert_eq!(export_document(&document), bytes);
    let current = element(document["deck"]["slides"][0]["elements"][0].clone());
    let next = apply_range(current, 1, 6, RunStyle { highlight: Some("@accent2".into()), baseline: Some(30000), ..Default::default() }).unwrap();
    let next = replace_text_content(next, "日本😀 Excellent".into()).unwrap();
    let changed = transact(&document, value(next)).unwrap();
    let saved = export_document(&changed["document"]);
    let reopened = open(&saved);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["text"], "日本😀 Excellent");
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"][1]["style"]["highlight"], "@accent2");
    let old_package = Package::open(bytes.clone()).unwrap(); let new_package = Package::open(saved).unwrap();
    for (path, content) in old_package.parts().iter().filter(|(path, _)| path.as_str() != "ppt/slides/slide1.xml" && !path.starts_with("customXml/")) { assert_eq!(new_package.part(path).unwrap(), content, "unrelated part {path}"); }
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], document["hash"]);
    assert_eq!(export_document(&undone["document"]), bytes);
}

#[test]
fn paragraph_properties_roundtrip_and_edit_without_flattening() {
    let mut input = text_element(); input["text"] = json!("One\n二\n");
    input["format"] = json!({"paragraphs":[
        {"alignment":"right","bullet":"numbered","numbering":"romanUcPeriod","number_start":4,"level":2,"margin_left":400000,"indent":-200000,"line_spacing":{"kind":"percent","value":125000},"space_before":{"kind":"points","value":600},"space_after":{"kind":"percent","value":50000},"tabs":[{"position":800000,"alignment":"decimal"}],"runs":[{"text":"One","style":{"language":"en-US"}}]},
        {"alignment":"center","bullet":"bullet","bullet_character":"◆","line_spacing":{"kind":"points","value":2400},"runs":[{"text":"二","style":{"font_size":32,"font_family":"Yu Gothic","color":"@accent1","underline":true}}]},
        {"runs":[{"text":"","style":{"italic":true}}]}
    ]});
    let document = open(&export_deck(deck(input)));
    let current = &document["deck"]["slides"][0]["elements"][0];
    let properties = &current["format"]["paragraphs"][0];
    assert_eq!(properties["number_start"], 4); assert_eq!(properties["level"], 2);
    assert_eq!(properties["indent"], -200000); assert_eq!(properties["tabs"][0]["alignment"], "decimal");
    let replaced = replace_text_content(element(current.clone()), "One\n日本\n".into()).unwrap();
    let changed = transact(&document, value(replaced)).unwrap();
    let reopened = open(&export_document(&changed["document"]));
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"][0], *properties);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["text"], "One\n日本\n");
}

fn body_fixture(body: &str) -> Vec<u8> {
    let mut package = Package::open(export_deck(deck(text_element()))).unwrap();
    let path = "ppt/slides/slide1.xml"; let mut xml = package.text(path).unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let range = parsed.descendants().find(|node| node.tag_name().name() == "txBody").unwrap().range();
    xml.replace_range(range, body); package.replace_part(path, xml.into_bytes()).unwrap(); package.save().unwrap()
}

#[test]
fn native_office_proofing_flags_do_not_block_rich_replacement() {
    let bytes = body_fixture("<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang='en-US' err='1' dirty='1'/><a:t>Lorem ipsum dolor</a:t></a:r></a:p></p:txBody>");
    let document = open(&bytes);
    assert_eq!(export_document(&document), bytes);
    let changed = execute_request(json!({"op":"apply_operations","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
        {"op":"set_rich_text","slide_id":"slide-1","id":"text-1","paragraphs":[{"runs":[{"text":"Updated wording","style":{"bold":true,"language":"en-US"}}]}]}
    ]})).expect("Office proofing flags must not reject a supported rich-text replacement");
    let saved = export_document(&changed["document"]);
    let package = Package::open(saved.clone()).unwrap();
    let xml = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    assert!(xml.descendants().filter(|node| node.tag_name().name() == "rPr").all(|node| node.attribute("err").is_none()));
    assert_eq!(open(&saved)["deck"]["slides"][0]["elements"][0]["text"], "Updated wording");
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
    assert_eq!(export_document(&undone["document"]), bytes);
}

#[test]
fn native_office_alternative_language_survives_rich_edits() {
    let bytes = body_fixture("<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang='en-US' altLang='ja-JP'/><a:t>Lorem ipsum</a:t></a:r></a:p></p:txBody>");
    let document = open(&bytes);
    assert_eq!(export_document(&document), bytes);
    let current = element(document["deck"]["slides"][0]["elements"][0].clone());
    let next = apply_range(current, 0, 5, RunStyle { bold: Some(true), ..Default::default() }).unwrap();
    let changed = transact(&document, value(next)).expect("Office alternative language must not reject a supported rich-text edit");
    let saved = export_document(&changed["document"]);
    let package = Package::open(saved.clone()).unwrap();
    let xml = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    let runs: Vec<_> = xml.descendants().filter(|node| node.tag_name().name() == "rPr").collect();
    assert!(!runs.is_empty());
    assert!(runs.iter().all(|node| node.attribute("altLang") == Some("ja-JP")));
    let reopened = open(&saved);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["text"], "Lorem ipsum");
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
    assert_eq!(export_document(&undone["document"]), bytes);
}

#[test]
fn native_office_alternative_language_and_end_properties_survive_two_edit_cycles() {
    let bytes = body_fixture("<p:txBody><a:bodyPr lIns='100'/><a:lstStyle/><a:p><a:r><a:rPr lang='en-US' altLang='ja-JP'/><a:t>Lorem ipsum</a:t></a:r><a:endParaRPr lang='fr-FR' altLang='ja-JP' sz='3300' baseline='-10000'/></a:p></p:txBody>");
    let original = open(&bytes);
    assert_eq!(export_document(&original), bytes);
    let mut document = original.clone();
    let mut source = bytes.clone();
    for style in [RunStyle { bold: Some(true), ..Default::default() }, RunStyle { italic: Some(true), ..Default::default() }] {
        let current = element(document["deck"]["slides"][0]["elements"][0].clone());
        let next = apply_range(current, 0, 5, style).unwrap();
        let changed = transact(&document, value(next)).expect("rich editing must remain valid after export and reopen");
        let saved = export_document(&changed["document"]);
        let package = Package::open(saved.clone()).unwrap();
        let xml = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
        let runs: Vec<_> = xml.descendants().filter(|node| node.tag_name().name() == "rPr").collect();
        assert!(!runs.is_empty());
        assert!(runs.iter().all(|node| node.attribute("altLang") == Some("ja-JP")));
        let ends: Vec<_> = xml.descendants().filter(|node| node.tag_name().name() == "endParaRPr").collect();
        assert_eq!(ends.len(), 1);
        for (name, expected) in [("lang", "fr-FR"), ("altLang", "ja-JP"), ("sz", "3300"), ("baseline", "-10000")] {
            assert_eq!(ends[0].attribute(name), Some(expected), "{name}");
        }
        let old_package = Package::open(source.clone()).unwrap();
        for (path, content) in old_package.parts().iter().filter(|(path, _)| path.as_str() != "ppt/slides/slide1.xml" && !path.starts_with("customXml/")) {
            assert_eq!(package.part(path).unwrap(), content, "unrelated part {path}");
        }
        let reopened = open(&saved);
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["text"], "Lorem ipsum");
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"][0]["style"]["bold"], true);
        assert_eq!(export_document(&reopened), saved);
        assert_eq!(export_document(&document), source);
        let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
        assert_eq!(undone["document"]["hash"], document["hash"]);
        assert_eq!(export_document(&undone["document"]), source);
        document = reopened; source = saved;
    }
    assert_eq!(export_document(&original), bytes);
}

#[test]
fn native_office_alternative_language_survives_paragraph_replacement() {
    let bytes = body_fixture("<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang='en-US' altLang='ja-JP'/><a:t>Lorem </a:t></a:r><a:r><a:rPr lang='fr-FR' altLang='ja-JP'/><a:t>ipsum</a:t></a:r></a:p></p:txBody>");
    let document = open(&bytes);
    for (replacement, expected) in [
        (json!({"runs":[{"text":"New ","style":{"bold":true}},{"text":"title","style":{"italic":true}}]}), ["ja-JP", "ja-JP"]),
        (json!({"runs":[{"text":"New ","style":{"alternative_language":"ko-KR"}},{"text":"title"}]}), ["ko-KR", "ja-JP"]),
        (json!({"runs":[{"text":"New ","style":{"alternative_language":"ko-KR"}},{"text":"title","style":{"alternative_language":"de-DE"}}]}), ["ko-KR", "de-DE"]),
    ] {
        let changed = execute_request(json!({"op":"update_paragraphs","document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":"text-1","paragraphs":[replacement]})).unwrap();
        let saved = export_document(&changed["document"]);
        let package = Package::open(saved.clone()).unwrap();
        let xml = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
        let languages: Vec<_> = xml.descendants().filter(|node| node.tag_name().name() == "rPr").map(|node| node.attribute("altLang").unwrap()).collect();
        assert_eq!(languages, expected);
        let reopened = open(&saved);
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["text"], "New title");
        for (index, language) in expected.iter().enumerate() {
            assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"][index]["style"]["alternative_language"], *language);
        }
        let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
        assert_eq!(undone["document"]["hash"], document["hash"]);
        assert_eq!(export_document(&undone["document"]), bytes);
    }
    assert_eq!(export_document(&document), bytes);
}

#[test]
fn review19_mixed_office_languages_inherit_alternatives_from_unique_primary_language() {
    let bytes = body_fixture("<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang='en-US' altLang='ja-JP'/><a:t>Azure </a:t></a:r><a:r><a:rPr lang='ja-JP' altLang='en-US'/><a:t>\u{306e}\u{5c0e}\u{5165}</a:t></a:r></a:p></p:txBody>");
    let document = open(&bytes);
    for batch in [false, true] {
        let paragraphs = json!([{"runs":[{"text":"Azure ","style":{"language":"en-US","bold":true}},{"text":"\u{306e}\u{66f4}\u{65b0}","style":{"language":"ja-JP"}}]}]);
        let request = if batch { json!({"op":"apply_operations","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"set_rich_text","slide_id":"slide-1","id":"text-1","paragraphs":paragraphs}]}) }
            else { json!({"op":"update_paragraphs","document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":"text-1","paragraphs":paragraphs}) };
        let changed = execute_request(request).unwrap();
        let reopened = open(&export_document(&changed["document"]));
        let runs = &reopened["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"];
        assert_eq!(runs[0]["style"]["alternative_language"], "ja-JP");
        assert_eq!(runs[1]["style"]["alternative_language"], "en-US");
        assert_eq!(runs[0]["style"]["language"], "en-US");
        assert_eq!(runs[1]["style"]["language"], "ja-JP");
        let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
        assert_eq!(export_document(&undone["document"]), bytes);
    }
    for style in [json!({}), json!({"language":"de-DE"})] {
        let error = execute_request(json!({"op":"update_paragraphs","document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":"text-1","paragraphs":[{"runs":[{"text":"Changed","style":style}]}]})).unwrap_err().to_string();
        for pair in ["(en-US, ja-JP)", "(ja-JP, en-US)"] { assert!(error.contains(pair), "{error}"); }
        assert!(error.len() < 2048, "diagnostics must stay bounded");
    }
    let explicit = execute_request(json!({"op":"update_paragraphs","document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":"text-1","paragraphs":[{"runs":[{"text":"Changed","style":{"language":"en-US","alternative_language":"ko-KR"}}]}]})).unwrap();
    assert_eq!(open(&export_document(&explicit["document"]))["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"][0]["style"]["alternative_language"], "ko-KR");
    assert_eq!(export_document(&document), bytes);
}

#[test]
fn review19_alternative_language_mapping_ignores_tag_case_and_preserves_source_spelling() {
    for (body, style, expected) in [
        ("<a:r><a:rPr lang='en-US' altLang='ja-JP'/><a:t>First</a:t></a:r><a:r><a:rPr lang='EN-us' altLang='JA-jp'/><a:t>Second</a:t></a:r>", json!({}), "ja-JP"),
        ("<a:r><a:rPr lang='en-US' altLang='ja-JP'/><a:t>First</a:t></a:r><a:r><a:rPr lang='ja-JP' altLang='en-US'/><a:t>Second</a:t></a:r><a:r><a:rPr lang='JA-jp' altLang='EN-us'/><a:t>Third</a:t></a:r>", json!({"language":"ja-jp"}), "en-US"),
    ] {
        let bytes = body_fixture(&format!("<p:txBody><a:bodyPr/><a:lstStyle/><a:p>{body}</a:p></p:txBody>"));
        let document = open(&bytes);
        let changed = execute_request(json!({"op":"update_paragraphs","document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":"text-1","paragraphs":[{"runs":[{"text":"Changed","style":style}]}]})).unwrap();
        let reopened = open(&export_document(&changed["document"]));
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"][0]["style"]["alternative_language"], expected);
        assert_eq!(export_document(&document), bytes);
    }
}

#[test]
fn native_office_mixed_alternative_languages_require_every_replacement_run_to_be_explicit() {
    for second_language in ["altLang='ko-KR'", ""] {
        let bytes = body_fixture(&format!("<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang='en-US' altLang='ja-JP'/><a:t>First</a:t></a:r><a:r><a:rPr lang='en-US' {second_language}/><a:t>Second</a:t></a:r></a:p></p:txBody>"));
        let document = open(&bytes);
        let paragraphs = &document["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"];
        let unchanged = execute_request(json!({"op":"update_paragraphs","document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":"text-1","paragraphs":paragraphs})).unwrap();
        assert_eq!(export_document(&unchanged["document"]), bytes);
        for runs in [
            json!([{"text":"New title"}]),
            json!([{"text":"New title","style":{"language":"en-US"}}]),
            json!([{"text":"New ","style":{"alternative_language":"de-DE"}},{"text":"title"}]),
            json!([{"text":"New "},{"text":"title","style":{"alternative_language":"de-DE"}}]),
        ] {
            let error = execute_request(json!({"op":"update_paragraphs","document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":"text-1","paragraphs":[{"runs":runs}]})).unwrap_err();
            assert!(matches!(error, aislide_core::Error::Unsupported(_)), "{error}");
            assert!(error.to_string().contains("mixed native alternative languages require explicit alternative_language"), "{error}");
            assert_eq!(export_document(&document), bytes);
        }
        let changed = execute_request(json!({"op":"update_paragraphs","document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":"text-1","paragraphs":[{"runs":[
            {"text":"New ","style":{"alternative_language":"de-DE"}},
            {"text":"title","style":{"alternative_language":"es-ES"}}
        ]}]})).unwrap();
        let reopened = open(&export_document(&changed["document"]));
        let runs = &reopened["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"];
        assert_eq!(runs.as_array().unwrap().len(), 2);
        assert_eq!(runs[0]["style"]["alternative_language"], "de-DE");
        assert_eq!(runs[1]["style"]["alternative_language"], "es-ES");
        let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
        assert_eq!(export_document(&undone["document"]), bytes);
        assert_eq!(export_document(&document), bytes);
    }
}

#[test]
fn native_office_rich_edits_clear_only_modified_paragraph_end_proofing_flags() {
    let untouched = "<a:p><a:r><a:rPr lang='en-US' altLang='ja-JP' err='1' dirty='0'/><a:t>Untouched</a:t></a:r><a:endParaRPr lang='de-DE' altLang='ko-KR' sz='2400' err='0' dirty='1'/></a:p>";
    let bytes = body_fixture(&format!("<p:txBody><a:bodyPr lIns='100'/><a:lstStyle/><a:p><a:r><a:rPr lang='en-US' altLang='ja-JP' err='1' dirty='1'/><a:t>Lorem ipsum</a:t></a:r><a:endParaRPr lang='fr-FR' altLang='ja-JP' sz='3300' baseline='-10000' i='1' err='1' dirty='0'/></a:p>{untouched}</p:txBody>"));
    let document = open(&bytes);
    assert_eq!(export_document(&document), bytes);
    for text_changed in [false, true] {
        let mut paragraphs = document["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"].clone();
        if text_changed { paragraphs[0]["runs"][0]["text"] = json!("Edited wording"); }
        else { paragraphs[0]["runs"][0]["style"]["bold"] = json!(true); }
        let changed = execute_request(json!({"op":"update_paragraphs","document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":"text-1","paragraphs":paragraphs})).unwrap();
        let saved = export_document(&changed["document"]);
        let package = Package::open(saved.clone()).unwrap();
        let xml = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
        let ends: Vec<_> = xml.descendants().filter(|node| node.tag_name().name() == "endParaRPr").collect();
        assert_eq!(ends.len(), 2);
        for name in ["err", "dirty"] { assert_eq!(ends[0].attribute(name), None, "modified paragraph end {name}"); }
        for (name, expected) in [("lang", "fr-FR"), ("altLang", "ja-JP"), ("sz", "3300"), ("baseline", "-10000"), ("i", "1")] {
            assert_eq!(ends[0].attribute(name), Some(expected), "preserved paragraph end {name}");
        }
        assert_eq!(ends[1].attribute("err"), Some("0"));
        assert_eq!(ends[1].attribute("dirty"), Some("1"));
        assert!(package.text("ppt/slides/slide1.xml").unwrap().contains(untouched));
        assert!(package.text("ppt/slides/slide1.xml").unwrap().contains("lIns='100'"));
        let reopened = open(&saved);
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["text"], if text_changed { "Edited wording\nUntouched" } else { "Lorem ipsum\nUntouched" });
        let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
        assert_eq!(export_document(&undone["document"]), bytes);
        assert_eq!(export_document(&document), bytes);
    }
}

#[test]
fn native_office_batch_error_identifies_operation_element_and_attribute() {
    let mut source = deck(text_element());
    let mut second = text_element(); second["id"] = json!("blocked-text");
    source["slides"][0]["elements"].as_array_mut().unwrap().push(second);
    let mut package = Package::open(export_deck(source)).unwrap();
    let mut xml = package.text("ppt/slides/slide1.xml").unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let position = parsed.descendants().filter(|node| node.tag_name().name() == "rPr").last().unwrap().range().start + "<a:rPr".len();
    xml.insert_str(position, " kern='1200'");
    package.replace_part("ppt/slides/slide1.xml", xml.into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let document = open(&bytes);
    let error = execute_request(json!({"op":"apply_operations","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
        {"op":"set_rich_text","slide_id":"slide-1","id":"text-1","paragraphs":[{"runs":[{"text":"First update"}]}]},
        {"op":"set_rich_text","slide_id":"slide-1","id":"blocked-text","paragraphs":[{"runs":[{"text":"Blocked update"}]}]}
    ]})).unwrap_err().to_string();
    for expected in ["operation 2", "slide-1", "blocked-text", "a:rPr@kern"] { assert!(error.contains(expected), "{expected}: {error}"); }
    assert_eq!(export_document(&document), bytes);
}

#[test]
fn native_office_noop_keeps_unrepresented_styles_byte_identical() {
    let bytes = body_fixture("<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr b='1' kern='1200'/><a:t>First</a:t></a:r><a:r><a:rPr i='1'/><a:t>Second</a:t></a:r></a:p></p:txBody>");
    let document = open(&bytes);
    let paragraphs = &document["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"];
    assert!(paragraphs.is_array());
    let unchanged = execute_request(json!({"op":"apply_operations","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
        {"op":"set_rich_text","slide_id":"slide-1","id":"text-1","paragraphs":paragraphs}
    ]})).unwrap();
    assert_eq!(export_document(&unchanged["document"]), bytes);
}

#[test]
fn native_office_table_cell_retains_alternative_language_on_text_edit() {
    let table = json!({"type":"table","id":"table-1","x":40,"y":100,"width":600,"height":160,"rows":[["Before"]],"font_size":24});
    let mut package = Package::open(export_deck(deck(table))).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().replacen("<a:rPr ", "<a:rPr altLang='ja-JP' err='1' ", 1);
    package.replace_part("ppt/slides/slide1.xml", xml.into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let document = open(&bytes);
    let current = element(document["deck"]["slides"][0]["elements"][0].clone());
    let edited = aislide_core::table_format::replace_cell_text(current, 0, 0, "After".into()).unwrap();
    let changed = transact(&document, value(edited)).unwrap();
    let saved = export_document(&changed["document"]);
    let reopened = open(&saved);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["rows"][0][0], "After");
    let package = Package::open(saved).unwrap();
    let xml = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    assert!(xml.descendants().filter(|node| node.tag_name().name() == "rPr").all(|node| node.attribute("altLang") == Some("ja-JP") && node.attribute("err").is_none()));
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
    assert_eq!(export_document(&undone["document"]), bytes);
}

#[test]
fn fields_and_unknown_styles_preserve_original_and_reject_lossy_edits() {
    for content in [
        "<a:fld id='{00000000-0000-0000-0000-000000000001}' type='slidenum'><a:rPr/><a:t>7</a:t></a:fld>",
        "<a:r><a:rPr kumimoji='1'/><a:t>unknown</a:t></a:r>",
        "<a:r><a:rPr u='dbl'/><a:t>double underline</a:t></a:r>",
        "<a:r><a:rPr><a:latin typeface='Arial'/></a:rPr><a:t>partial font</a:t></a:r>",
        "<a:r><a:rPr><a:solidFill><a:srgbClr val='000000'><a:alpha val='50000'/></a:srgbClr></a:solidFill></a:rPr><a:t>transparent</a:t></a:r>",
    ] {
        let bytes = body_fixture(&format!("<p:txBody><a:bodyPr/><a:lstStyle/><a:p>{content}</a:p></p:txBody>"));
        let document = open(&bytes); assert_eq!(export_document(&document), bytes);
        let current = element(document["deck"]["slides"][0]["elements"][0].clone());
        let edited = apply_range(current, 0, 1, RunStyle { bold: Some(true), ..Default::default() }).unwrap();
        assert!(matches!(transact(&document, value(edited)), Err(aislide_core::Error::Unsupported(_))), "{content}");
        assert_eq!(export_document(&document), bytes);
    }
}

#[test]
fn simple_uniform_native_frame_stays_legacy_and_byte_identical() {
    let bytes = export_deck(deck(text_element())); let document = open(&bytes);
    assert!(document["deck"]["slides"][0]["elements"][0]["format"].get("paragraphs").is_none());
    assert_eq!(export_document(&document), bytes);
}

#[test]
fn rich_measurement_uses_run_sizes_and_reports_missing_glyphs() {
    let mut input = text_element(); input["text"] = json!("Small BIG"); input["height"] = json!(45);
    let rich = apply_range(element(input), 6, 9, RunStyle { font_size: Some(120.0), ..Default::default() }).unwrap();
    let measured = execute_request(json!({"op":"measure_layout","deck":deck(value(rich))})).unwrap();
    assert_eq!(measured["measurements"][0]["overflow"], true);
    assert!(measured["measurements"][0]["measured_height"].as_f64().unwrap() >= 120.0);
    let mut input = text_element(); input["text"] = json!("a\u{10ffff}");
    let rich = apply_range(element(input), 1, 2, RunStyle { font_family: Some("No Such AISlide Font".into()), ..Default::default() }).unwrap();
    let measured = execute_request(json!({"op":"measure_layout","deck":deck(value(rich))})).unwrap();
    assert!(measured["measurements"][0]["missing_glyphs"].as_u64().unwrap() > 0);
    assert_eq!(measured["office_parity_verified"], false);
}

#[test]
fn native_empty_paragraphs_accept_rich_insertions() {
    for paragraph in ["<a:p/>", "<a:p><a:pPr/><a:endParaRPr lang='en-US' sz='1800'/></a:p>"] {
        let bytes = body_fixture(&format!("<p:txBody><a:bodyPr/><a:lstStyle/>{paragraph}</p:txBody>"));
        let document = open(&bytes);
        let current = element(document["deck"]["slides"][0]["elements"][0].clone());
        let changed = replace_text_content(current, "new 日本".into()).unwrap();
        let changed = apply_range(changed, 0, 6, RunStyle { underline: Some(true), ..Default::default() }).unwrap();
        let result = transact(&document, value(changed)).unwrap();
        assert_eq!(open(&export_document(&result["document"]))["deck"]["slides"][0]["elements"][0]["text"], "new 日本");
    }
}

#[test]
fn inherited_run_defaults_do_not_bleed_from_first_run() {
    let bytes = body_fixture("<p:txBody><a:bodyPr/><a:lstStyle><a:lvl2pPr algn='r'><a:defRPr sz='2400' i='1'/></a:lvl2pPr></a:lstStyle><a:p><a:pPr lvl='1' marL='1000' indent='-500'/><a:r><a:rPr b='1'/><a:t>日</a:t></a:r><a:r><a:t>A</a:t></a:r></a:p></p:txBody>");
    let document = open(&bytes); let current = &document["deck"]["slides"][0]["elements"][0];
    let runs = &current["format"]["paragraphs"][0]["runs"];
    assert_eq!(runs[0]["style"]["bold"], true); assert_eq!(runs[1]["style"]["bold"], false);
    assert_eq!(runs[1]["style"]["font_size"].as_f64(), Some(32.0)); assert_eq!(runs[1]["style"]["italic"], true);
    let next = apply_range(element(current.clone()), 1, 2, RunStyle { highlight: Some("FF0000".into()), ..Default::default() }).unwrap();
    let changed = transact(&document, value(next)).unwrap();
    let reopened = open(&export_document(&changed["document"]));
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"][1]["style"]["bold"], false);
}

#[test]
fn inserting_into_empty_styled_paragraph_keeps_its_insertion_style() {
    let mut input = text_element(); input["text"] = json!("First\n");
    input["format"] = json!({"paragraphs":[{"runs":[{"text":"First","style":{"bold":true}}]},{"alignment":"right","runs":[{"text":"","style":{"italic":true,"highlight":"FFFF00"}}]}]});
    let result = value(replace_text_content(element(input), "First\nNew".into()).unwrap());
    assert_eq!(result["format"]["paragraphs"][1]["alignment"], "right");
    assert_eq!(result["format"]["paragraphs"][1]["runs"][0]["style"]["italic"], true);
    assert_eq!(result["format"]["paragraphs"][1]["runs"][0]["style"]["highlight"], "FFFF00");
}

#[test]
fn paragraph_end_properties_survive_rich_edits() {
    let bytes = body_fixture("<p:txBody><a:bodyPr lIns='100'/><a:lstStyle/><a:p><a:r><a:rPr b='1'/><a:t>A</a:t></a:r><a:r><a:rPr i='1'/><a:t>B</a:t></a:r><a:endParaRPr lang='fr-FR' sz='3300' baseline='-10000'/></a:p></p:txBody>");
    let document = open(&bytes); let current = element(document["deck"]["slides"][0]["elements"][0].clone());
    let next = apply_range(current, 0, 1, RunStyle { color: Some("@accent3".into()), ..Default::default() }).unwrap();
    let changed = transact(&document, value(next)).unwrap();
    let saved = Package::open(export_document(&changed["document"])).unwrap();
    let xml = roxmltree::Document::parse(saved.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    let end = xml.descendants().find(|node| node.tag_name().name() == "endParaRPr").unwrap();
    assert_eq!(end.attribute("lang"), Some("fr-FR")); assert_eq!(end.attribute("baseline"), Some("-10000"));
    let body = xml.descendants().find(|node| node.tag_name().name() == "bodyPr").unwrap(); assert_eq!(body.attribute("lIns"), Some("100"));
}

#[test]
fn rich_run_size_bounds_do_not_escape_legacy_frame_bounds() {
    for size in [1.0, 400.0] {
        let mut input = text_element(); input["text"] = json!("A");
        let rich = apply_range(element(input), 0, 1, RunStyle { font_size: Some(size), ..Default::default() }).unwrap();
        let document = open(&export_deck(deck(value(rich))));
        let current = &document["deck"]["slides"][0]["elements"][0];
        assert_eq!(current["text"], "A");
        assert_eq!(current["format"]["paragraphs"][0]["runs"][0]["style"]["font_size"].as_f64(), Some(size));
        let next = apply_range(element(current.clone()), 0, 1, RunStyle { bold: Some(true), ..Default::default() }).unwrap();
        let changed = transact(&document, value(next)).unwrap();
        assert_eq!(open(&export_document(&changed["document"]))["deck"]["slides"][0]["elements"][0]["text"], "A");
    }
}