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
fn master_import_appends_selected_potx_design_without_replacing_slides() {
    let capabilities = execute_request(json!({"op":"authoring_capabilities"})).unwrap();
    assert_eq!(capabilities["master_import"]["formats"], json!(["pptx", "potx"]));
    assert_eq!(capabilities["master_import"]["append_only"], true);
    let source = editing::create("source".into(), "Synthetic source master".into()).unwrap();
    let exported = execute_request(json!({"op":"export_template","document":source,"kind":"potx"})).unwrap();
    let catalog = execute_request(json!({"op":"inspect_master_source","kind":"potx","base64":exported["base64"]})).unwrap();
    assert_eq!(catalog["width"], 1280);
    assert_eq!(catalog["masters"].as_array().unwrap().len(), 1);
    let original = fixture();
    let input = json!({"kind":"potx","base64":exported["base64"],"source_sha256":catalog["source_sha256"],
        "mode":"masters","ids":[catalog["masters"][0]["id"]],"prefix":"brand","name":"Imported brand"});
    let preview = execute_request(json!({"op":"preview_master_import","document":original,"expected_revision":0,"expected_hash":original["hash"],"input":input})).unwrap();
    assert_eq!(preview["design"]["masters"].as_array().unwrap().len(), 2);
    assert_eq!(preview["design"]["masters"][0], original["deck"]["design"]["masters"][0]);
    let applied = execute_request(json!({"op":"import_masters","document":original,"expected_revision":0,"expected_hash":original["hash"],"input":input,"expected_candidate_hash":preview["candidate_hash"]})).unwrap();
    assert_eq!(applied["document"]["revision"], 1);
    assert_eq!(applied["document"]["deck"]["slides"], original["deck"]["slides"]);
    assert_eq!(applied["document"]["sources"], original["sources"]);
    assert_eq!(applied["document"]["origin"], original["origin"]);
    assert_eq!(applied["document"]["deck"]["design"], preview["design"]);
    let restored = execute_request(json!({"op":"undo_transaction","document":applied["document"],"expected_revision":1,"receipt":applied["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], original["hash"]);
}

#[test]
fn master_import_sample_slides_keep_fixed_content_theme_and_existing_native_parts() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut source = fixture()["deck"].clone();
    source["design"]["theme"]["colors"]["accent1"] = json!("AA1155");
    source["design"]["masters"][0]["elements"] = json!([{"type":"rect","id":"brand","x":0,"y":0,"width":1280,"height":18,"fill":"@accent1"}]);
    source["slides"][0]["title"] = json!("Example layout");
    source["slides"][0]["notes"] = json!("Private source notes are not layout content");
    let exported = execute_request(json!({"op":"export","deck":source})).unwrap();
    let catalog = execute_request(json!({"op":"inspect_master_source","kind":"pptx","base64":exported["base64"]})).unwrap();
    assert_eq!(catalog["slides"][0]["importable"], true, "{catalog}");
    let original_bytes = execute_request(json!({"op":"export_presentation","document":fixture()})).unwrap();
    let mut package = aislide_core::package::Package::open(STANDARD.decode(original_bytes["base64"].as_str().unwrap()).unwrap()).unwrap();
    let original_slide = package.text("ppt/slides/slide1.xml").unwrap().replace("</p:sld>", "<p:extLst><p:ext uri=\"urn:test:keep\"><v:opaque xmlns:v=\"urn:test:keep\" value=\"unchanged\"/></p:ext></p:extLst></p:sld>");
    package.replace_part("ppt/slides/slide1.xml", original_slide.clone().into_bytes()).unwrap();
    let original_bytes = STANDARD.encode(package.save().unwrap());
    let original = execute_request(json!({"op":"open_presentation","id":"native-target","base64":original_bytes})).unwrap()["document"].clone();
    let input = json!({"kind":"pptx","base64":exported["base64"],"source_sha256":catalog["source_sha256"],"mode":"slides","ids":[catalog["slides"][0]["id"]],"prefix":"sample","name":"Sample master"});
    let preview = execute_request(json!({"op":"preview_master_import","document":original,"expected_revision":0,"expected_hash":original["hash"],"input":input})).unwrap();
    let applied = execute_request(json!({"op":"import_masters","document":original,"expected_revision":0,"expected_hash":original["hash"],"input":input,"expected_candidate_hash":preview["candidate_hash"]})).unwrap();
    assert_eq!(applied["document"]["deck"]["slides"], original["deck"]["slides"]);
    assert_eq!(applied["document"]["origin"], original["origin"]);
    assert_eq!(preview["design"]["masters"][1]["theme"]["colors"]["accent1"], "AA1155");
    let layout = preview["design"]["layouts"].as_array().unwrap().last().unwrap();
    assert!(layout["elements"].as_array().unwrap().iter().any(|element| element["text"] == "Alpha Beta"));
    assert!(layout["elements"].as_array().unwrap().iter().any(|element| element["fill"] == "@accent1"));
    assert!(!preview["preview_slides"].to_string().contains("Private source notes"));
    let saved = execute_request(json!({"op":"export_presentation","document":applied["document"]})).unwrap();
    let saved_package = aislide_core::package::Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(saved_package.text("ppt/slides/slide1.xml").unwrap(), original_slide);
    let reopened = execute_request(json!({"op":"open_presentation","id":"reopened","base64":saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["design"]["masters"].as_array().unwrap().len(), 2);
    let restored = execute_request(json!({"op":"undo_transaction","document":applied["document"],"expected_revision":1,"receipt":applied["receipt"]})).unwrap();
    assert_eq!(execute_request(json!({"op":"export_presentation","document":restored["document"]})).unwrap()["base64"], original_bytes);
}

#[test]
fn master_import_rejects_unrepresented_backgrounds_extensions_and_theme_effects() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let original = execute_request(json!({"op":"export_presentation","document":editing::create("source".into(), "Source".into()).unwrap()})).unwrap();
    let bytes = STANDARD.decode(original["base64"].as_str().unwrap()).unwrap();
    for (path, before, after) in [
        ("ppt/slideMasters/slideMaster1.xml", "<a:effectLst/>", "<a:effectLst><a:blur rad=\"50000\"/></a:effectLst>"),
        ("ppt/slideMasters/slideMaster1.xml", "</p:sldMaster>", "<p:extLst><p:ext uri=\"urn:test:unsupported\"><v:unknown xmlns:v=\"urn:test:unsupported\"/></p:ext></p:extLst></p:sldMaster>"),
        ("ppt/theme/theme1.xml", "</a:theme>", "<a:extLst><a:ext uri=\"urn:test:theme\"><v:custom xmlns:v=\"urn:test:theme\"/></a:ext></a:extLst></a:theme>"),
    ] {
        let mut package = aislide_core::package::Package::open(bytes.clone()).unwrap();
        let xml = package.text(path).unwrap();
        assert!(xml.contains(before), "fixture token missing: {before}");
        package.replace_part(path, xml.replace(before, after).into_bytes()).unwrap();
        let catalog = execute_request(json!({"op":"inspect_master_source","kind":"pptx","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
        assert_eq!(catalog["masters"][0]["importable"], false, "{path}: {catalog}");
        assert!(catalog["masters"][0]["reason"].as_str().is_some_and(|value| !value.is_empty()));
    }
}

#[test]
fn master_import_rejects_stale_candidates_collisions_and_wrong_dimensions() {
    let source = execute_request(json!({"op":"export_presentation","document":editing::create("source".into(), "Source".into()).unwrap()})).unwrap();
    let catalog = execute_request(json!({"op":"inspect_master_source","kind":"pptx","base64":source["base64"]})).unwrap();
    let original = fixture();
    let input = json!({"kind":"pptx","base64":source["base64"],"source_sha256":catalog["source_sha256"],"mode":"masters","ids":[catalog["masters"][0]["id"]],"prefix":"brand","name":"Brand"});
    let request = json!({"op":"preview_master_import","document":original,"expected_revision":0,"expected_hash":original["hash"],"input":input});
    let preview = execute_request(request.clone()).unwrap();
    for (pointer, value) in [("/expected_revision",json!(1)),("/expected_hash",json!("0".repeat(64))),("/input/source_sha256",json!("0".repeat(64))),("/input/prefix",json!("../brand")),("/input/ids",json!([])),("/input/ids",json!(["missing"]))] {
        let mut invalid = request.clone(); *invalid.pointer_mut(pointer).unwrap() = value;
        assert!(execute_request(invalid).is_err(), "{pointer}");
    }
    assert!(execute_request(json!({"op":"import_masters","document":original,"expected_revision":0,"expected_hash":original["hash"],"input":input,"expected_candidate_hash":"0".repeat(64)})).is_err());
    let applied = execute_request(json!({"op":"import_masters","document":original,"expected_revision":0,"expected_hash":original["hash"],"input":input,"expected_candidate_hash":preview["candidate_hash"]})).unwrap();
    assert!(execute_request(json!({"op":"preview_master_import","document":applied["document"],"expected_revision":1,"expected_hash":applied["document"]["hash"],"input":input})).is_err());
    let resized = change(&original,"resize_canvas",json!({"width":960,"height":720,"mode":"scale"}));
    assert!(execute_request(json!({"op":"preview_master_import","document":resized["document"],"expected_revision":1,"expected_hash":resized["document"]["hash"],"input":input})).is_err());
    let mut without_design = original["deck"].clone();
    without_design["design"] = serde_json::Value::Null;
    for slide in without_design["slides"].as_array_mut().unwrap() { slide["layout_id"] = serde_json::Value::Null; }
    let target = execute_request(json!({"op":"new_document","id":"no-master","deck":without_design})).unwrap();
    let preview = execute_request(json!({"op":"preview_master_import","document":target,"expected_revision":0,"expected_hash":target["hash"],"input":input})).unwrap();
    let applied = execute_request(json!({"op":"import_masters","document":target,"expected_revision":0,"expected_hash":target["hash"],"input":input,"expected_candidate_hash":preview["candidate_hash"]})).unwrap();
    assert_eq!(applied["document"]["deck"]["slides"], target["deck"]["slides"]);
    let restored = execute_request(json!({"op":"undo_transaction","document":applied["document"],"expected_revision":1,"receipt":applied["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], target["hash"]);
    let compact = change(&serde_json::to_value(editing::create("compact".into(), "Compact".into()).unwrap()).unwrap(), "resize_canvas", json!({"width":960,"height":540,"mode":"scale"}));
    let compact_source = execute_request(json!({"op":"export_presentation","document":compact["document"]})).unwrap();
    let compact_catalog = execute_request(json!({"op":"inspect_master_source","kind":"pptx","base64":compact_source["base64"]})).unwrap();
    let mut compact_deck = compact["document"]["deck"].clone(); compact_deck["design"] = serde_json::Value::Null; compact_deck["slides"][0]["layout_id"] = serde_json::Value::Null;
    let compact_target = execute_request(json!({"op":"new_document","id":"compact-target","deck":compact_deck})).unwrap();
    let mut compact_input = input; compact_input["base64"] = compact_source["base64"].clone(); compact_input["source_sha256"] = compact_catalog["source_sha256"].clone();
    assert!(execute_request(json!({"op":"preview_master_import","document":compact_target,"expected_revision":0,"expected_hash":compact_target["hash"],"input":compact_input})).is_ok());
}

#[test]
fn master_import_sample_hides_master_but_keeps_layout_graphics() {
    let mut source = editing::create("source".into(), "Source".into()).unwrap().deck;
    let design = source.design.as_mut().unwrap();
    design.masters[0].elements.push(serde_json::from_value(json!({"type":"rect","id":"hidden-master","x":0,"y":0,"width":80,"height":80,"fill":"AA0000"})).unwrap());
    design.layouts[0].elements.push(serde_json::from_value(json!({"type":"rect","id":"visible-layout","x":100,"y":0,"width":80,"height":80,"fill":"00AA00"})).unwrap());
    source.slides[0].hide_master_graphics = true;
    let exported = execute_request(json!({"op":"export","deck":source})).unwrap();
    let catalog = execute_request(json!({"op":"inspect_master_source","kind":"pptx","base64":exported["base64"]})).unwrap();
    let original = fixture();
    let input = json!({"kind":"pptx","base64":exported["base64"],"source_sha256":catalog["source_sha256"],"mode":"slides","ids":[catalog["slides"][0]["id"]],"prefix":"visible","name":"Visible layout"});
    let preview = execute_request(json!({"op":"preview_master_import","document":original,"expected_revision":0,"expected_hash":original["hash"],"input":input})).unwrap();
    let layout = preview["design"]["layouts"].as_array().unwrap().last().unwrap();
    assert!(layout["elements"].as_array().unwrap().iter().any(|element| element["fill"] == "00AA00"));
    assert!(!layout["elements"].as_array().unwrap().iter().any(|element| element["fill"] == "AA0000"));
}

#[test]
fn master_import_rejects_distinct_unrepresented_hyperlink_destinations() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut deck = editing::create("links".into(), "Links".into()).unwrap().deck;
    deck.slides[0].elements.push(serde_json::from_value(json!({"type":"text","id":"links","x":64,"y":80,"width":600,"height":160,"text":"One\nTwo","font_size":24,"color":"@dk1","bold":false,"format":{"hyperlink":"https://example.test/first"}})).unwrap());
    let exported = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let mut package = aislide_core::package::Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let path = "ppt/slides/slide1.xml";
    let xml = package.text(path).unwrap();
    let start = xml.rfind("<a:hlinkClick").unwrap();
    let mut patched = xml.to_owned();
    let end = start + xml[start..].find("/>").unwrap() + 2;
    patched.replace_range(start..end, "<a:hlinkClick r:id=\"second-link\"/>");
    package.replace_part(path, patched.into_bytes()).unwrap();
    let rels = "ppt/slides/_rels/slide1.xml.rels";
    let xml = package.text(rels).unwrap().replace("</Relationships>", "<Relationship Id=\"second-link\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"https://example.test/second\" TargetMode=\"External\"/></Relationships>");
    package.replace_part(rels, xml.into_bytes()).unwrap();
    let catalog = execute_request(json!({"op":"inspect_master_source","kind":"pptx","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    assert_eq!(catalog["slides"][0]["importable"], false, "{catalog}");
}

#[test]
fn master_import_refuses_protected_sources_and_reports_unique_preview_slides() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let exported = execute_request(json!({"op":"export_presentation","document":editing::create("source".into(), "Source".into()).unwrap()})).unwrap();
    let catalog = execute_request(json!({"op":"inspect_master_source","kind":"pptx","base64":exported["base64"]})).unwrap();
    let target = fixture();
    let input = json!({"kind":"pptx","base64":exported["base64"],"source_sha256":catalog["source_sha256"],"mode":"masters","ids":[catalog["masters"][0]["id"]],"prefix":"preview","name":"Preview"});
    let preview = execute_request(json!({"op":"preview_master_import","document":target,"expected_revision":0,"expected_hash":target["hash"],"input":input})).unwrap();
    let slides = preview["preview_slides"].as_array().unwrap();
    assert_eq!(slides.iter().map(|slide| slide["id"].as_str().unwrap()).collect::<std::collections::BTreeSet<_>>().len(), slides.len());
    let mut package = aislide_core::package::Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/presentation.xml").unwrap().replace("</p:presentation>", "<p:modifyVerifier cryptProviderType=\"rsaAES\"/></p:presentation>");
    package.replace_part("ppt/presentation.xml", xml.into_bytes()).unwrap();
    let error = execute_request(json!({"op":"inspect_master_source","kind":"pptx","base64":STANDARD.encode(package.save().unwrap())})).unwrap_err();
    assert!(error.to_string().contains("protected"), "{error}");
    for path in ["docProps/custom.XML", "docProps/custom.data"] {
        let mut parts = aislide_core::package::Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap().parts().clone();
        parts.insert(path.into(), b"<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/custom-properties\"><property name=\"MSIP_Label_test\"/></Properties>".to_vec());
        let mut package = aislide_core::package::Package::from_parts(parts).unwrap();
        let types = package.text("[Content_Types].xml").unwrap().replace("</Types>", &format!("<Override PartName=\"/{path}\" ContentType=\"application/vnd.openxmlformats-officedocument.custom-properties+xml\"/></Types>"));
        package.replace_part("[Content_Types].xml", types.into_bytes()).unwrap();
        let error = execute_request(json!({"op":"inspect_master_source","kind":"pptx","base64":STANDARD.encode(package.save().unwrap())})).unwrap_err();
        assert!(error.to_string().contains("protected"), "{error}");
    }
}

#[test]
fn master_import_sample_rejects_placeholder_stacking_changes() {
    let source = editing::create("stacking".into(), "Stacking".into()).unwrap().deck;
    let mut source = aislide_core::design::assign_layout(source, "slide-1", "title-content").unwrap();
    source.slides[0].elements.push(serde_json::from_value(json!({"type":"rect","id":"cover-title","x":64,"y":80,"width":1152,"height":120,"fill":"FFFFFF"})).unwrap());
    let exported = execute_request(json!({"op":"export","deck":source})).unwrap();
    let catalog = execute_request(json!({"op":"inspect_master_source","kind":"pptx","base64":exported["base64"]})).unwrap();
    assert_eq!(catalog["masters"][0]["importable"], true);
    assert_eq!(catalog["slides"][0]["importable"], false, "{catalog}");
}

#[test]
fn master_import_sample_rejects_connectors_to_excluded_placeholders() {
    let mut source = editing::create("connections".into(), "Connections".into()).unwrap().deck;
    source.design.as_mut().unwrap().masters[0].elements = serde_json::from_value(json!([
        {"type":"text","id":"sample-0","x":64,"y":80,"width":160,"height":80,"text":"Excluded placeholder","font_size":24,"color":"@dk1","bold":false,"format":{"placeholder":{"kind":"body","index":0}}},
        {"type":"connector","id":"route","x":80,"y":80,"width":200,"height":1,"color":"@dk1","stroke_width":2,"arrow":true,"start":{"element_id":"sample-0","site":0}}
    ])).unwrap();
    source.slides[0].elements.push(serde_json::from_value(json!({"type":"rect","id":"unrelated","x":300,"y":80,"width":160,"height":80,"fill":"FFFFFF"})).unwrap());
    let exported = execute_request(json!({"op":"export","deck":source})).unwrap();
    let catalog = execute_request(json!({"op":"inspect_master_source","kind":"pptx","base64":exported["base64"]})).unwrap();
    assert_eq!(catalog["masters"][0]["importable"], true);
    assert_eq!(catalog["slides"][0]["importable"], false, "{catalog}");
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

fn check_insert_title(native: bool) {
    let mut deck = fixture()["deck"].clone();
    let template = &mut deck["design"]["layouts"][1]["elements"][0];
    template["id"] = json!("heading-not-title");
    template["font_size"] = json!(36);
    template["color"] = json!("@accent2");
    template["format"]["alignment"] = json!("right");
    template["format"]["paragraphs"] = json!([{"alignment":"right","space_after":{"kind":"points","value":400},"runs":[{"text":"Title","style":{"italic":true,"font_size":38}}]}]);
    let mut original = execute_request(json!({"op":"new_document","id":"insert-title","deck":deck})).unwrap();
    let bytes = execute_request(json!({"op":"export_presentation","document":original})).unwrap();
    if native {
        original = execute_request(json!({"op":"open_presentation","id":"insert-native","base64":bytes["base64"]})).unwrap()["document"].clone();
    }
    let layout = original["deck"]["design"]["layouts"].as_array().unwrap().iter().find(|layout| {
        layout["elements"].as_array().unwrap().iter().any(|element| element["format"]["placeholder"]["kind"] == "title")
    }).unwrap();
    let title_template = layout["elements"].as_array().unwrap().iter().find(|element| element["format"]["placeholder"]["kind"] == "title").unwrap();
    for title in ["Requested <&> title", ""] {
        let inserted = change(&original, "edit_slides", json!({"operations":[{"op":"insert","id":"inserted","after":"slide-1","title":title,"layout_id":layout["id"]}]}));
        let slide = &inserted["document"]["deck"]["slides"][1];
        assert_eq!(slide["title"], title);
        let heading = slide["elements"].as_array().unwrap().iter().find(|element| element["format"]["placeholder"]["kind"] == "title").unwrap();
        assert_eq!(heading["text"], title, "native={native}");
        let expected = aislide_core::rich_text::replace_text_content(serde_json::from_value(title_template.clone()).unwrap(), title.into()).unwrap();
        let mut expected = serde_json::to_value(expected).unwrap();
        expected["format"]["inherit_layout"] = json!(true);
        assert_eq!(heading, &expected);
        for body in slide["elements"].as_array().unwrap().iter().filter(|element| element["format"]["placeholder"]["kind"] == "body") {
            let mut expected = layout["elements"].as_array().unwrap().iter().find(|element| element["id"] == body["id"]).unwrap().clone();
            expected["format"]["inherit_layout"] = json!(true);
            assert_eq!(body, &expected);
        }
        assert_eq!(inserted["document"]["deck"]["slides"][0], original["deck"]["slides"][0]);
        assert_eq!(inserted["document"]["origin"], original["origin"]);
        assert_eq!(inserted["document"]["revision"], 1);
        let renamed = change(&inserted["document"], "edit_slides", json!({"operations":[{"op":"rename","slide_id":"inserted","title":"Metadata only"}]}));
        assert_eq!(renamed["document"]["deck"]["slides"][1]["elements"], slide["elements"]);
        let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
        let reopened = execute_request(json!({"op":"open_presentation","id":"insert-reopened","base64":saved["base64"]})).unwrap();
        let reopened_heading = reopened["document"]["deck"]["slides"][1]["elements"].as_array().unwrap().iter().find(|element| element["format"]["placeholder"]["kind"] == "title").unwrap();
        assert_eq!(reopened_heading["text"], title);
        assert_eq!(reopened_heading["format"]["paragraphs"][0]["runs"][0]["style"]["font_size"], heading["format"]["paragraphs"][0]["runs"][0]["style"]["font_size"]);
        assert_eq!(reopened_heading["format"]["paragraphs"][0]["runs"][0]["style"]["italic"], true);
        let undone = execute_request(json!({"op":"undo_transaction","document":inserted["document"],"expected_revision":1,"receipt":inserted["receipt"]})).unwrap();
        assert_eq!(undone["document"]["hash"], original["hash"]);
        if native {
            assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], bytes["base64"]);
        }
    }
    let blank = change(&original, "edit_slides", json!({"operations":[{"op":"insert","id":"blank-insert","title":"Metadata title"}]}));
    assert_eq!(blank["document"]["deck"]["slides"][1]["title"], "Metadata title");
    assert!(blank["document"]["deck"]["slides"][1]["elements"].as_array().unwrap().is_empty());
}

#[test]
fn insert_title_fills_authored_placeholder_and_preserves_layout() {
    check_insert_title(false);
}

#[test]
fn insert_title_fills_native_placeholder_and_preserves_origin() {
    check_insert_title(true);
}