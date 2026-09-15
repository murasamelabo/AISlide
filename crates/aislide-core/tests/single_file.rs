use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn authored() -> Value {
    let report = execute_request(json!({"op":"sample"})).unwrap();
    let compiled = execute_request(json!({"op":"compile","report":report})).unwrap();
    let design = execute_request(json!({"op":"design_defaults"})).unwrap();
    let mut deck = compiled["deck"].clone();
    deck["design"] = design;
    deck["design"]["theme"]["colors"]["accent1"] = json!("B53055");
    deck["design"]["theme"]["fonts"]["major"] = json!("Arial");
    let shape = execute_request(json!({"op":"create_object","id":"native-ellipse","kind":"shape","preset":"ellipse"})).unwrap();
    deck["slides"][1]["elements"].as_array_mut().unwrap().push(shape);
    execute_request(json!({"op":"assign_layout","deck":deck,"slide_id":"slide-3","layout_id":"title-content"})).unwrap()
}

fn exported(deck: Value) -> Vec<u8> {
    let result = execute_request(json!({"op":"export","deck":deck})).unwrap();
    STANDARD.decode(result["base64"].as_str().unwrap()).unwrap()
}

#[test]
fn slide_commands_create_blank_insert_duplicate_reorder_and_undo() {
    let document = execute_request(json!({"op":"create_presentation","id":"blank","title":"Untitled presentation"})).unwrap();
    assert_eq!(document["deck"]["slides"].as_array().unwrap().len(), 1);
    assert!(document["deck"]["slides"][0]["elements"].as_array().unwrap().is_empty());
    let changed = execute_request(json!({"op":"edit_slides","document":document,"expected_revision":0,"operations":[
        {"op":"insert","id":"next","after":"slide-1","title":"Next slide"},
        {"op":"duplicate","slide_id":"next","id":"copy"},
        {"op":"move","slide_id":"copy","index":0}
    ]})).unwrap();
    assert_eq!(changed["document"]["deck"]["slides"][0]["id"], "copy");
    assert_eq!(changed["document"]["deck"]["slides"].as_array().unwrap().len(), 3);
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":1,"receipt":changed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], document["hash"]);
    assert!(execute_request(json!({"op":"edit_slides","document":document,"expected_revision":0,"operations":[{"op":"remove","slide_id":"slide-1"}]})).is_err());
}

#[test]
fn slide_commands_on_reopened_pptx_preserve_native_parts_and_original_bytes() {
    let document = execute_request(json!({"op":"create_presentation","id":"native-slides","title":"Native slide operations"})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    let mut parts = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap().parts().clone();
    parts.insert("ppt/vendor/preserved.bin".into(), vec![2, 4, 6, 8]);
    let original = Package::from_parts(parts).unwrap().save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"native-slides-open","base64":STANDARD.encode(&original)})).unwrap();
    let changed = execute_request(json!({"op":"edit_slides","document":opened["document"],"expected_revision":0,"operations":[
        {"op":"insert","id":"inserted","after":"slide-1","title":"Inserted slide"},
        {"op":"move","slide_id":"inserted","index":0}
    ]})).unwrap();
    let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let result = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(result.part("ppt/vendor/preserved.bin").unwrap(), &[2, 4, 6, 8]);
    let reopened = execute_request(json!({"op":"open_presentation","id":"native-slides-reopen","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][0]["title"], "Inserted slide");
    assert_eq!(reopened["document"]["deck"]["slides"].as_array().unwrap().len(), 2);
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":1,"receipt":changed["receipt"]})).unwrap();
    let restored = execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap();
    assert_eq!(STANDARD.decode(restored["base64"].as_str().unwrap()).unwrap(), original);
}

#[test]
fn native_slide_duplicate_preserves_opaque_xml_graph_metadata_and_identity() {
    let document = execute_request(json!({"op":"create_presentation","id":"copy-native","title":"Copy test"})).unwrap();
    let catalog = execute_request(json!({"op":"graph_catalog"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide-1","id":"diagram","spec":catalog["examples"][0]["spec"]})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let mut package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().replace("</p:sld>", "<p:extLst><p:ext uri=\"urn:vendor:slide\"><v:setting xmlns:v=\"urn:vendor\" value=\"preserve\"/></p:ext></p:extLst></p:sld>");
    package.replace_part("ppt/slides/slide1.xml", xml.clone().into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"copy-open","base64":STANDARD.encode(&bytes)})).unwrap();
    let duplicated = execute_request(json!({"op":"edit_slides","document":opened["document"],"expected_revision":0,"operations":[{"op":"duplicate","slide_id":"slide-1","id":"copy-slide"}]})).unwrap();
    assert_eq!(duplicated["document"]["parts"].as_array().unwrap().len(), 2);
    assert!(duplicated["document"]["parts"].as_array().unwrap().iter().all(|part| part["stale"] == false));
    let output = execute_request(json!({"op":"export_presentation","document":duplicated["document"]})).unwrap();
    let result = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(result.text("ppt/slides/slide1.xml").unwrap(), xml);
    assert!(result.text("ppt/slides/aislide-added-1.xml").unwrap().contains("urn:vendor:slide"));
    let reopened = execute_request(json!({"op":"open_presentation","id":"copy-reopen","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][1]["id"], "copy-slide");
    assert!(reopened["document"]["parts"].as_array().unwrap().iter().all(|part| part["stale"] == false));
    let moved = execute_request(json!({"op":"apply_graph","document":reopened["document"],"expected_revision":0,"slide_id":"copy-slide","id":"diagram","operations":[{"op":"move","ids":["user"],"dx":16,"dy":0}]})).unwrap();
    execute_request(json!({"op":"export_presentation","document":moved["document"]})).unwrap();
    let removed = execute_request(json!({"op":"edit_slides","document":reopened["document"],"expected_revision":0,"operations":[{"op":"remove","slide_id":"slide-1"}]})).unwrap();
    assert_eq!(removed["document"]["parts"].as_array().unwrap().len(), 1);
    let undone = execute_request(json!({"op":"undo_transaction","document":removed["document"],"expected_revision":1,"receipt":removed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], reopened["document"]["hash"]);
}

#[test]
fn native_slide_chart_copies_have_independent_embedded_workbooks() {
    let blank = execute_request(json!({"op":"create_presentation","id":"chart-copy","title":"Chart copy"})).unwrap();
    let mut deck = blank["deck"].clone();
    deck["slides"][0]["elements"] = json!([execute_request(json!({"op":"create_object","id":"chart","kind":"chart","preset":"column"})).unwrap()]);
    let document = execute_request(json!({"op":"new_document","id":"chart-copy-source","deck":deck})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"chart-copy-open","base64":saved["base64"]})).unwrap();
    let copied = execute_request(json!({"op":"edit_slides","document":opened["document"],"expected_revision":0,"operations":[{"op":"duplicate","slide_id":"slide-1","id":"chart-copy-slide"}]})).unwrap();
    let output = execute_request(json!({"op":"export_presentation","document":copied["document"]})).unwrap();
    let package = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    let relations = roxmltree::Document::parse(package.text("ppt/slides/_rels/aislide-added-1.xml.rels").unwrap()).unwrap();
    let copied_chart = relations.root_element().children().find(|node| node.attribute("Type").is_some_and(|kind| kind.ends_with("/chart"))).unwrap().attribute("Target").unwrap().trim_start_matches('/');
    assert_ne!(copied_chart, "ppt/charts/chart1.xml", "copy must not share its mutable native chart");
    assert_eq!(package.part(copied_chart).unwrap(), package.part("ppt/charts/chart1.xml").unwrap());
    let workbooks: Vec<_> = package.parts().iter().filter(|(name, _)| name.ends_with(".xlsx")).collect();
    assert_eq!(workbooks.len(), 2, "both slides need an independent workbook");
    assert_eq!(workbooks[0].1, workbooks[1].1);
    let reopened = execute_request(json!({"op":"open_presentation","id":"chart-copy-reopen","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"].as_array().unwrap().len(), 2);
}

#[test]
fn removing_native_slides_removes_their_slide_and_notes_parts() {
    let blank = execute_request(json!({"op":"create_presentation","id":"remove-native","title":"Remove slide"})).unwrap();
    let inserted = execute_request(json!({"op":"edit_slides","document":blank,"expected_revision":0,"operations":[{"op":"insert","id":"remove-me","title":"Deleted slide sentinel"}]})).unwrap();
    let mut document = inserted["document"].clone();
    let updated = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/slides/1/notes","value":"Deleted notes sentinel"}]}})).unwrap();
    document = updated["document"].clone();
    let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"remove-native-open","base64":saved["base64"]})).unwrap();
    let removed = execute_request(json!({"op":"edit_slides","document":opened["document"],"expected_revision":0,"operations":[{"op":"remove","slide_id":"remove-me"}]})).unwrap();
    let output = execute_request(json!({"op":"export_presentation","document":removed["document"]})).unwrap();
    let package = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert!(!package.parts().contains_key("ppt/slides/slide2.xml"));
    assert!(!package.parts().contains_key("ppt/notesSlides/notesSlide2.xml"));
    assert!(!package.parts().values().any(|bytes| String::from_utf8_lossy(bytes).contains("Deleted notes sentinel")));
    let reopened = execute_request(json!({"op":"open_presentation","id":"remove-native-final","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"].as_array().unwrap().len(), 1);
}

#[test]
fn native_presentation_title_can_be_renamed_without_replacing_slides() {
    let bytes = exported(authored());
    let opened = execute_request(json!({"op":"open_presentation","id":"rename-native","base64":STANDARD.encode(&bytes)})).unwrap();
    let changed = execute_request(json!({"op":"transaction","document":opened["document"],"transaction":{"expected_revision":0,"expected_hash":opened["document"]["hash"],"operations":[{"op":"replace","path":"/deck/title","value":"Renamed presentation"}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"renamed","base64":saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["title"], "Renamed presentation");
    let result = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(result.part("ppt/slides/slide1.xml").unwrap(), Package::open(bytes).unwrap().part("ppt/slides/slide1.xml").unwrap());
}

#[test]
fn pptx_alone_restores_native_theme_layout_text_shapes_and_notes() {
    let deck = authored();
    let bytes = exported(deck.clone());
    let opened = execute_request(json!({"op":"open_presentation","id":"standalone","base64":STANDARD.encode(&bytes)})).unwrap();
    let restored = &opened["document"]["deck"];
    assert_eq!(restored["title"], deck["title"]);
    assert_eq!(restored["slides"].as_array().unwrap().len(), 12);
    assert_eq!(restored["design"]["theme"]["colors"]["accent1"], "B53055");
    assert_eq!(restored["design"]["theme"]["fonts"]["major"], "Arial");
    assert_eq!(restored["design"]["layouts"].as_array().unwrap().len(), 4);
    assert!(restored["slides"][1]["elements"].as_array().unwrap().iter().any(|element| element["type"] == "shape" && element["preset"] == "ellipse"));
    assert!(restored["slides"][2]["elements"].as_array().unwrap().iter().any(|element| element["text"] == "Three signals.\nOne direction." || element["format"]["placeholder"]["kind"] == "title"));
    assert_eq!(restored["slides"][0]["notes"], deck["slides"][0]["notes"]);
    let saved = execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap();
    assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), bytes);
    assert!(saved.get("checkpoint").is_none());
}

#[test]
fn actual_open_xml_is_authoritative_without_an_external_snapshot() {
    let mut package = Package::open(exported(authored())).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().replace("Quarterly performance", "Changed in PowerPoint");
    package.replace_part("ppt/slides/slide1.xml", xml.into_bytes()).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"external-edit","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    assert!(opened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().any(|element| element["text"].as_str().is_some_and(|text| text.contains("Changed in PowerPoint"))));
}

#[test]
fn unknown_package_parts_survive_supported_single_file_edits() {
    let mut parts = Package::open(exported(authored())).unwrap().parts().clone();
    parts.insert("ppt/opaque/vendor.bin".into(), vec![0, 17, 254, 255]);
    let bytes = Package::from_parts(parts).unwrap().save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"opaque","base64":STANDARD.encode(&bytes)})).unwrap();
    let document = &opened["document"];
    let elements = document["deck"]["slides"][0]["elements"].as_array().unwrap();
    let index = elements.iter().position(|element| element["type"] == "text" && element["text"].as_str().is_some_and(|text| text.contains("Quarterly"))).unwrap();
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":format!("/deck/slides/0/elements/{index}/text"),"value":"Single file editing\nNative XML"}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(package.part("ppt/opaque/vendor.bin").unwrap(), &[0, 17, 254, 255]);
    let reopened = execute_request(json!({"op":"open_presentation","id":"reopened","base64":saved["base64"]})).unwrap();
    assert!(reopened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().any(|element| element["text"] == "Single file editing\nNative XML"));
}

#[test]
fn protected_or_legacy_containers_are_identified_before_zip_or_checkpoint_errors() {
    let bytes = [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1, 0, 0, 0, 0];
    let error = execute_request(json!({"op":"open_presentation","id":"protected","base64":STANDARD.encode(bytes)})).unwrap_err().to_string();
    assert!(error.contains("encrypted") && error.contains("legacy"), "{error}");
    assert!(!error.contains("checkpoint"), "{error}");
}

#[test]
fn reopened_pptx_supports_text_geometry_table_and_theme_edits() {
    let bytes = exported(authored());
    let opened = execute_request(json!({"op":"open_presentation","id":"native-edit","base64":STANDARD.encode(bytes)})).unwrap();
    let document = &opened["document"];
    let mut deck = document["deck"].clone();
    let title = deck["slides"][0]["elements"].as_array_mut().unwrap().iter_mut().find(|element| element["id"] == "title").unwrap();
    title["text"] = json!("Native edit\nThree lines\nNo sidecar");
    title["font_size"] = json!(36);
    title["x"] = json!(75);
    title["width"] = json!(1000);
    title["format"]["italic"] = json!(true);
    let table = deck["slides"][3]["elements"].as_array_mut().unwrap().iter_mut().find(|element| element["type"] == "table").unwrap();
    table["rows"][1][0] = json!("Edited native cell");
    deck["design"]["theme"]["colors"]["accent1"] = json!("2468AC");
    deck["design"]["theme"]["fonts"]["minor"] = json!("Arial");
    let result = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck","value":deck}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":result["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"edited","base64":saved["base64"]})).unwrap();
    let deck = &reopened["document"]["deck"];
    let title = deck["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["id"] == "title").unwrap();
    assert_eq!(title["text"], "Native edit\nThree lines\nNo sidecar");
    assert_eq!(title["x"], 75.0);
    assert_eq!(title["font_size"], 36.0);
    assert_eq!(title["format"]["italic"], true);
    assert_eq!(deck["design"]["theme"]["colors"]["accent1"], "2468AC");
    assert!(deck["slides"][3]["elements"].as_array().unwrap().iter().any(|element| element["rows"][1][0] == "Edited native cell"));
}

#[test]
fn native_master_layout_edits_and_new_shapes_survive_reopen() {
    let opened = execute_request(json!({"op":"open_presentation","id":"design-edit","base64":STANDARD.encode(exported(authored()))})).unwrap();
    let document = &opened["document"];
    let mut design = document["deck"]["design"].clone();
    design["layouts"][1]["elements"][0]["x"] = json!(80);
    design["layouts"][1]["elements"][0]["width"] = json!(1080);
    design["masters"][0]["elements"].as_array_mut().unwrap().push(json!({"id":"master-text","type":"text","x":64,"y":668,"width":400,"height":28,"text":"New common footer","font_size":14,"color":"@dk2","bold":false}));
    let mut deck = execute_request(json!({"op":"update_design","deck":document["deck"],"design":design})).unwrap();
    deck["slides"][1]["elements"].as_array_mut().unwrap().push(execute_request(json!({"op":"create_object","id":"new-shape","kind":"shape","preset":"diamond"})).unwrap());
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck","value":deck}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"design-reopen","base64":saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["design"]["layouts"][1]["elements"][0]["x"], 80.0);
    assert!(reopened["document"]["deck"]["slides"][2]["elements"].as_array().unwrap().iter().any(|element| element["format"]["inherit_layout"] == true && element["x"] == 80.0));
    assert!(reopened["document"]["deck"]["design"]["masters"][0]["elements"].as_array().unwrap().iter().any(|element| element["text"] == "New common footer"));
    assert!(reopened["document"]["deck"]["slides"][1]["elements"].as_array().unwrap().iter().any(|element| element["id"] == "new-shape"));
}

#[test]
fn new_embedded_images_charts_and_links_get_unique_native_relationships() {
    let opened = execute_request(json!({"op":"open_presentation","id":"resources","base64":STANDARD.encode(exported(authored()))})).unwrap();
    let document = &opened["document"];
    let mut deck = document["deck"].clone();
    for kind in ["pie", "doughnut", "area", "scatter", "stacked_column", "stacked_bar", "line", "column", "bar"] {
        let chart = execute_request(json!({"op":"create_object","id":format!("chart-{kind}"),"kind":"chart","preset":kind})).unwrap();
        deck["slides"][1]["elements"].as_array_mut().unwrap().push(chart);
    }
    let mut image_bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(8, 8, image::Rgb([20, 90, 140]))).write_to(&mut image_bytes, image::ImageFormat::Png).unwrap();
    let picture = execute_request(json!({"op":"create_picture","id":"new-picture","base64":STANDARD.encode(image_bytes.into_inner()),"mime_type":"image/png","alt":"Synthetic pixels"})).unwrap();
    deck["slides"][1]["elements"].as_array_mut().unwrap().push(picture);
    let mut label = execute_request(json!({"op":"create_object","id":"linked-label","kind":"text"})).unwrap();
    label["format"] = json!({"hyperlink":"https://example.invalid/?left=1&right=2"});
    deck["slides"][1]["elements"].as_array_mut().unwrap().push(label);
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck","value":deck}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"resources-reopened","base64":saved["base64"]})).unwrap();
    let elements = reopened["document"]["deck"]["slides"][1]["elements"].as_array().unwrap();
    assert_eq!(elements.iter().filter(|element| element["type"] == "chart").count(), 9);
    assert!(elements.iter().any(|element| element["type"] == "picture" && element["alt"] == "Synthetic pixels"));
    assert!(elements.iter().any(|element| element["format"]["hyperlink"] == "https://example.invalid/?left=1&right=2"));
}

#[test]
fn sources_and_bindings_travel_inside_pptx_without_an_embedded_deck_snapshot() {
    let source = execute_request(json!({"op":"ingest","input":{"name":"fixture.csv","format":"csv","base64":STANDARD.encode(b"Quarter,Value\nQ1,10\nQ2,12\n")}})).unwrap();
    let report = execute_request(json!({"op":"data_report","source":source,"mapping":{"title":"Source-bound single file","period":"Synthetic fixture","table_index":0,"category_column":0,"value_columns":[1],"row_start":0,"row_count":2,"chart_kind":"column"}})).unwrap();
    let document = execute_request(json!({"op":"new_document","id":"sources","deck":report["compiled"]["deck"],"sources":[source],"bindings":report["bindings"]})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    assert!(saved.get("checkpoint").is_none());
    let package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    let metadata: Vec<_> = package.parts().iter().filter(|(name, _)| name.starts_with("customXml/") && name.ends_with(".xml")).collect();
    assert!(!metadata.is_empty());
    for (_, bytes) in metadata { let xml = std::str::from_utf8(bytes).unwrap(); assert!(!xml.contains("\"deck\"")); assert!(!xml.contains("name=\"deck\"")); }
    let opened = execute_request(json!({"op":"open_presentation","id":"sources-reopened","base64":saved["base64"]})).unwrap();
    assert_eq!(opened["document"]["sources"][0]["sha256"], source["sha256"]);
    assert_eq!(opened["document"]["bindings"].as_array().unwrap().len(), report["bindings"].as_array().unwrap().len());
    assert!(opened["document"]["bindings"].as_array().unwrap().iter().all(|binding| binding["stale"] == false));
    let reexported = execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap();
    assert!(reexported["base64"] == saved["base64"], "unmodified single-file project must remain byte-identical");
}

#[test]
fn explicit_local_placeholder_styles_override_inherited_template_styles() {
    let mut package = Package::open(exported(authored())).unwrap();
    let xml = package.text("ppt/slides/slide3.xml").unwrap().replace("<a:rPr lang=\"ja-JP\"/>", "<a:rPr lang=\"ja-JP\" sz=\"1800\" b=\"0\"/>");
    package.replace_part("ppt/slides/slide3.xml", xml.into_bytes()).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"local-style","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    let title = opened["document"]["deck"]["slides"][2]["elements"].as_array().unwrap().iter().find(|element| element["format"]["placeholder"]["kind"] == "title").unwrap();
    assert_eq!(title["font_size"], 24.0);
    assert_eq!(title["bold"], false);
    assert_eq!(title["format"]["inherit_layout"], false);
}

#[test]
fn unrepresented_text_and_chart_formatting_is_preserved_or_edit_is_rejected() {
    let mut deck = authored();
    deck["slides"][1]["elements"].as_array_mut().unwrap().push(execute_request(json!({"op":"create_object","id":"native-chart","kind":"chart","preset":"column"})).unwrap());
    let mut package = Package::open(exported(deck)).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().replace("<a:bodyPr ", "<a:bodyPr vert=\"vert270\" ");
    package.replace_part("ppt/slides/slide1.xml", xml.into_bytes()).unwrap();
    let chart = package.text("ppt/charts/chart1.xml").unwrap().replace("<c:gapWidth", "<c:dLbls><c:showVal val=\"1\"/></c:dLbls><c:gapWidth");
    package.replace_part("ppt/charts/chart1.xml", chart.into_bytes()).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"complex","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    let document = &opened["document"];
    let title = document["deck"]["slides"][0]["elements"].as_array().unwrap().iter().position(|element| element["id"] == "title").unwrap();
    let chart = document["deck"]["slides"][1]["elements"].as_array().unwrap().iter().position(|element| element["id"] == "native-chart").unwrap();
    for operation in [json!({"op":"replace","path":format!("/deck/slides/0/elements/{title}/font_size"),"value":36}), json!({"op":"replace","path":format!("/deck/slides/1/elements/{chart}/series/0/values/0"),"value":25})] {
        let result = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[operation]}}));
        assert!(result.is_err(), "unrepresented formatting must not be silently dropped");
    }
}

#[test]
fn switching_a_reopened_slide_to_blank_removes_its_placeholder_marker() {
    let opened = execute_request(json!({"op":"open_presentation","id":"switch","base64":STANDARD.encode(exported(authored()))})).unwrap();
    let document = &opened["document"];
    let blank = document["deck"]["design"]["layouts"][0]["id"].as_str().unwrap();
    let slide_id = document["deck"]["slides"][2]["id"].as_str().unwrap();
    let deck = execute_request(json!({"op":"assign_layout","deck":document["deck"],"slide_id":slide_id,"layout_id":blank})).unwrap();
    let result = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck","value":deck}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":result["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"blank","base64":saved["base64"]})).unwrap();
    assert!(reopened["document"]["deck"]["slides"][2]["elements"].as_array().unwrap().iter().all(|element| element["format"]["placeholder"].is_null()));
}

#[test]
fn native_insertions_use_the_original_presentations_coordinate_scale() {
    let mut package = Package::open(exported(authored())).unwrap();
    let xml = package.text("ppt/presentation.xml").unwrap().replace("cx=\"12192000\" cy=\"6858000\"", "cx=\"24384000\" cy=\"13716000\"");
    package.replace_part("ppt/presentation.xml", xml.into_bytes()).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"other-dimensions","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    let document = &opened["document"];
    let shape = execute_request(json!({"op":"create_object","id":"coordinate-probe","kind":"shape","preset":"rect"})).unwrap();
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"add","path":"/deck/slides/0/elements/-","value":shape}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"coordinate-reopen","base64":saved["base64"]})).unwrap();
    let inserted = reopened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["id"] == "coordinate-probe").unwrap();
    for field in ["x", "y", "width", "height"] { assert_eq!(inserted[field], shape[field], "{field}"); }
}

#[test]
fn native_replacements_reject_unrepresented_paragraph_and_table_styles() {
    for table_case in [false, true] {
        let mut deck = authored();
        let title = deck["slides"][0]["elements"].as_array_mut().unwrap().iter_mut().find(|element| element["id"] == "title").unwrap();
        title["text"] = json!("First paragraph\nSecond paragraph");
        let mut package = Package::open(exported(deck)).unwrap();
        let path = if table_case { "ppt/slides/slide4.xml" } else { "ppt/slides/slide1.xml" };
        let mut xml = package.text(path).unwrap().to_owned();
        let parsed = roxmltree::Document::parse(&xml).unwrap();
        let drawing = "http://schemas.openxmlformats.org/drawingml/2006/main";
        let target = if table_case {
            parsed.descendants().find(|node| node.has_tag_name((drawing, "gridCol"))).unwrap()
        } else {
            let title = parsed.descendants().find(|node| node.attribute("name") == Some("title")).unwrap().parent().unwrap().parent().unwrap();
            title.descendants().filter(|node| node.has_tag_name((drawing, "pPr"))).nth(1).unwrap()
        };
        let range = target.range();
        let replacement = if table_case { "<a:gridCol w=\"777777\"/>" } else { "<a:pPr algn=\"r\"><a:lnSpc><a:spcPct val=\"200000\"/></a:lnSpc></a:pPr>" };
        xml.replace_range(range, replacement);
        package.replace_part(path, xml.into_bytes()).unwrap();
        let bytes = package.save().unwrap();
        let opened = execute_request(json!({"op":"open_presentation","id":"styled","base64":STANDARD.encode(&bytes)})).unwrap();
        let document = &opened["document"];
        let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), bytes);
        let slide_index = if table_case { 3 } else { 0 };
        let element_index = document["deck"]["slides"][slide_index]["elements"].as_array().unwrap().iter().position(|element| if table_case { element["type"] == "table" } else { element["id"] == "title" }).unwrap();
        let operation = if table_case {
            let row = document["deck"]["slides"][slide_index]["elements"][element_index]["rows"][1].clone();
            json!({"op":"add","path":format!("/deck/slides/{slide_index}/elements/{element_index}/rows/-"),"value":row})
        } else { json!({"op":"replace","path":format!("/deck/slides/0/elements/{element_index}/font_size"),"value":36}) };
        let result = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[operation]}}));
        assert!(result.is_err(), "unrepresented {} formatting must survive or reject replacement", if table_case { "table" } else { "paragraph" });
    }
}