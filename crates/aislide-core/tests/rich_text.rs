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