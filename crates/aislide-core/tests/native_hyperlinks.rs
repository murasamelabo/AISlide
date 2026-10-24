use aislide_core::{execute_request, package::Package};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::collections::BTreeSet;

const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const STRICT_R: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

fn source() -> Vec<u8> {
    let elements: Vec<_> = ["one-old", "two", "three-removed"].into_iter().enumerate().map(|(index, name)| json!({
        "type":"text","id":format!("link-{index}"),"x":40,"y":40+index*100,"width":400,"height":80,
        "text":name,"font_size":24,"color":"@accent1","bold":false,
        "format":{"underline":true,"hyperlink":format!("https://example.invalid/{name}")}
    })).collect();
    let deck = json!({"version":1,"title":"Synthetic native links","width":1280,"height":720,
        "slides":[{"id":"slide-1","title":"Links","background":"FFFFFF","notes":"","elements":elements}]});
    STANDARD.decode(execute_request(json!({"op":"export","deck":deck})).unwrap()["base64"].as_str().unwrap()).unwrap()
}

fn open(bytes: &[u8]) -> Value {
    execute_request(json!({"op":"open_presentation","id":"native-links","base64":STANDARD.encode(bytes)})).unwrap()["document"].clone()
}

fn save(document: &Value) -> Vec<u8> {
    STANDARD.decode(execute_request(json!({"op":"export_presentation","document":document})).unwrap()["base64"].as_str().unwrap()).unwrap()
}

fn links(bytes: &[u8]) -> Vec<(String, String)> {
    let package = Package::open(bytes.to_vec()).unwrap();
    let slide = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    let referenced: BTreeSet<_> = slide.descendants().flat_map(|node| node.attributes())
        .filter(|attribute| matches!(attribute.namespace(), Some(R | STRICT_R))).map(|attribute| attribute.value()).collect();
    let relationships = roxmltree::Document::parse(package.text("ppt/slides/_rels/slide1.xml.rels").unwrap()).unwrap();
    relationships.root_element().children().filter(|node| node.has_tag_name((REL, "Relationship"))
        && node.attribute("Type") == Some(format!("{R}/hyperlink").as_str())).map(|node| {
        let id = node.attribute("Id").unwrap();
        assert!(referenced.contains(id), "unused hyperlink relationship {id}: {}", node.attribute("Target").unwrap());
        (id.into(), node.attribute("Target").unwrap().into())
    }).collect()
}

#[test]
fn changing_moving_and_removing_native_links_does_not_retain_old_urls() {
    let bytes = source();
    let document = open(&bytes);
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{
        "expected_revision":0,"expected_hash":document["hash"],"operations":[
            {"op":"replace","path":"/deck/slides/0/elements/0/format/hyperlink","value":"https://example.invalid/one-new"},
            {"op":"replace","path":"/deck/slides/0/elements/1/y","value":150},
            {"op":"remove","path":"/deck/slides/0/elements/2"}
        ]
    }})).unwrap();
    let saved = save(&changed["document"]);
    let targets: BTreeSet<_> = links(&saved).into_iter().map(|(_, target)| target).collect();
    assert_eq!(targets, BTreeSet::from(["https://example.invalid/one-new".into(),"https://example.invalid/two".into()]));
    let package = Package::open(saved).unwrap();
    assert!(!package.text("ppt/slides/_rels/slide1.xml.rels").unwrap().contains("one-old"));
    assert!(!package.text("ppt/slides/_rels/slide1.xml.rels").unwrap().contains("three-removed"));
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":1,"receipt":changed["receipt"]})).unwrap();
    assert_eq!(save(&undone["document"]), bytes);
}

#[test]
fn native_link_destinations_remain_editable_after_repeated_reopening() {
    let original = source();
    let mut document = open(&original);
    for link in [Some("https://example.invalid/first-edit"),Some("https://example.invalid/second-edit"),None] {
        let result = execute_request(json!({"op":"apply_operations","document":document,
            "expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
                {"op":"set_hyperlink","slide_id":"slide-1","id":"link-0","link":link}
            ]})).unwrap();
        let bytes = save(&result["document"]);
        let mut expected = BTreeSet::from(["https://example.invalid/two".to_owned(),"https://example.invalid/three-removed".to_owned()]);
        if let Some(link) = link { expected.insert(link.into()); }
        assert_eq!(links(&bytes).into_iter().map(|(_, target)| target).collect::<BTreeSet<_>>(), expected);
        document = open(&bytes);
        assert_eq!(document["deck"]["slides"][0]["elements"][0]["format"]["hyperlink"], json!(link));
        assert_eq!(document["deck"]["slides"][0]["elements"][0]["format"]["underline"], true);
    }
}

#[test]
fn rich_link_only_changes_update_each_paragraph_without_losing_run_styles() {
    let mut deck = open(&source())["deck"].clone();
    deck["slides"][0]["elements"][0]["text"] = json!("one-old\ncontinued");
    deck["slides"][0]["elements"][0]["format"]["paragraphs"] = json!([
        {"runs":[{"text":"one","style":{"bold":true}},{"text":"-old","style":{"italic":true}}]},
        {"runs":[{"text":"continued","style":{"underline":false}}]}
    ]);
    let original = STANDARD.decode(execute_request(json!({"op":"export","deck":deck})).unwrap()["base64"].as_str().unwrap()).unwrap();
    let mut document = open(&original);
    let paragraphs = document["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"].clone();
    for link in [Some("https://example.invalid/rich-first"), Some("https://example.invalid/rich-second"), None] {
        let changed = execute_request(json!({"op":"apply_operations","document":document,
            "expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
                {"op":"set_hyperlink","slide_id":"slide-1","id":"link-0","link":link}
            ]})).unwrap();
        let bytes = save(&changed["document"]);
        let mut expected = BTreeSet::from(["https://example.invalid/two".to_owned(),"https://example.invalid/three-removed".to_owned()]);
        if let Some(link) = link { expected.insert(link.into()); }
        assert_eq!(links(&bytes).into_iter().map(|(_, target)| target).collect::<BTreeSet<_>>(), expected);
        document = open(&bytes);
        let element = &document["deck"]["slides"][0]["elements"][0];
        assert_eq!(element["format"]["hyperlink"], json!(link));
        assert_eq!(element["format"]["paragraphs"], paragraphs);
    }
}

#[test]
fn native_link_geometry_changes_reuse_existing_relationship_identity() {
    let original = source();
    let original_links = links(&original);
    let document = open(&original);
    let changed = execute_request(json!({"op":"apply_operations","document":document,
        "expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
            {"op":"set_frame","slide_id":"slide-1","id":"link-1","frame":{"x":40,"y":150,"width":400,"height":80}}
        ]})).unwrap();
    assert_eq!(links(&save(&changed["document"])), original_links);
}

#[test]
fn clean_copy_removes_unused_external_hyperlink_relationships_only() {
    let mut package = Package::open(source()).unwrap();
    let path = "ppt/slides/_rels/slide1.xml.rels";
    let xml = package.text(path).unwrap().replace("</Relationships>", &format!(
        "<Relationship Id=\"unused-link\" Type=\"{R}/hyperlink\" Target=\"https://example.invalid/private-stale\" TargetMode=\"External\"/></Relationships>"
    ));
    package.replace_part(path, xml.into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let document = open(&bytes);
    assert_eq!(save(&document), bytes);
    let clean = execute_request(json!({"op":"export_clean_copy","document":document,
        "options":{"new_document_id":"clean-native-links","categories":["notes"],"confirmed":true}})).unwrap();
    let cleaned = STANDARD.decode(clean["base64"].as_str().unwrap()).unwrap();
    assert_eq!(links(&cleaned).len(), 3);
    assert!(!Package::open(cleaned).unwrap().text(path).unwrap().contains("private-stale"));
    assert_eq!(save(&document), bytes);
}

#[test]
fn cleanup_keeps_hover_extension_and_non_hyperlink_relationships() {
    let mut package = Package::open(source()).unwrap();
    let path = "ppt/slides/_rels/slide1.xml.rels";
    let additions = [
        ("keep-hover", format!("{R}/hyperlink"), "https://example.invalid/hover"),
        ("keep-extension", format!("{R}/hyperlink"), "https://example.invalid/extension"),
        ("keep-strict-extension", format!("{R}/hyperlink"), "https://example.invalid/strict-extension"),
        ("remove-unused", format!("{R}/hyperlink"), "https://example.invalid/stale"),
        ("keep-other-type", "urn:synthetic:relationship".into(), "https://example.invalid/other-kind"),
    ].into_iter().map(|(id, kind, target)| format!("<Relationship Id=\"{id}\" Type=\"{kind}\" Target=\"{target}\" TargetMode=\"External\"/>")).collect::<String>();
    let relationships = package.text(path).unwrap().replace("</Relationships>", &format!("{additions}</Relationships>"));
    package.replace_part(path, relationships.into_bytes()).unwrap();
    let slide = package.text("ppt/slides/slide1.xml").unwrap().replacen("</a:rPr>",
        &format!("<a:hlinkMouseOver xmlns:r=\"{R}\" r:id=\"keep-hover\"/></a:rPr>"), 1)
        .replace("</p:sld>", &format!("<p:extLst><p:ext uri=\"urn:synthetic:extension\"><x:reference xmlns:x=\"urn:synthetic\" xmlns:r=\"{R}\" xmlns:s=\"{STRICT_R}\" r:id=\"keep-extension\" s:id=\"keep-strict-extension\"/></p:ext></p:extLst></p:sld>"));
    package.replace_part("ppt/slides/slide1.xml", slide.into_bytes()).unwrap();
    let document = open(&package.save().unwrap());
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{
        "expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
            {"op":"replace","path":"/deck/slides/0/notes","value":"An unrelated supported note edit"}
        ]
    }})).unwrap();
    let output = save(&changed["document"]);
    assert_eq!(links(&output).len(), 6);
    let output = Package::open(output).unwrap();
    let relationships = output.text(path).unwrap();
    for retained in ["keep-hover","keep-extension","keep-strict-extension","keep-other-type"] { assert!(relationships.contains(retained)); }
    assert!(!relationships.contains("remove-unused"));
}

#[test]
fn different_native_link_destinations_in_one_text_body_stay_guarded() {
    let mut deck = open(&source())["deck"].clone();
    deck["slides"][0]["elements"][0]["text"] = json!("one-old\ncontinued");
    let bytes = STANDARD.decode(execute_request(json!({"op":"export","deck":deck})).unwrap()["base64"].as_str().unwrap()).unwrap();
    let mut package = Package::open(bytes).unwrap();
    let mut xml = package.text("ppt/slides/slide1.xml").unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let relation = parsed.descendants().filter(|node| node.tag_name().name() == "hlinkClick").nth(1).unwrap().attribute_node((R,"id")).unwrap().range_value();
    xml.replace_range(relation, "rIdLink3");
    package.replace_part("ppt/slides/slide1.xml", xml.into_bytes()).unwrap();
    let document = open(&package.save().unwrap());
    let snapshot = document.clone();
    let error = execute_request(json!({"op":"apply_operations","document":document,
        "expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
            {"op":"set_hyperlink","slide_id":"slide-1","id":"link-0","link":"https://example.invalid/replacement"}
        ]})).unwrap_err();
    assert!(error.to_string().contains("complex native text formatting"), "{error}");
    assert_eq!(document, snapshot);
}

#[test]
fn review_equivalent_native_hyperlink_ids_remain_editable() {
    for rich in [false, true] {
        let mut deck = open(&source())["deck"].clone();
        deck["slides"][0]["elements"][0]["text"] = json!("one-old\ncontinued");
        if rich {
            deck["slides"][0]["elements"][0]["format"]["paragraphs"] = json!([
                {"runs":[{"text":"one-old","style":{"bold":true}}]},
                {"runs":[{"text":"continued","style":{"italic":true}}]}
            ]);
        }
        let bytes = STANDARD.decode(execute_request(json!({"op":"export","deck":deck})).unwrap()["base64"].as_str().unwrap()).unwrap();
        let mut package = Package::open(bytes).unwrap();
        let mut xml = package.text("ppt/slides/slide1.xml").unwrap().to_owned();
        let parsed = roxmltree::Document::parse(&xml).unwrap();
        let link = parsed.descendants().filter(|node| node.tag_name().name() == "hlinkClick").nth(1).unwrap().attribute_node((R,"id")).unwrap().range_value();
        xml.replace_range(link, "same-target");
        package.replace_part("ppt/slides/slide1.xml", xml.into_bytes()).unwrap();
        let path = "ppt/slides/_rels/slide1.xml.rels";
        let relationships = package.text(path).unwrap().replace("</Relationships>", &format!(
            "<Relationship Id=\"same-target\" Type=\"{R}/hyperlink\" Target=\"https://example.invalid/one-old\" TargetMode=\"External\"/></Relationships>"));
        package.replace_part(path, relationships.into_bytes()).unwrap();
        let document = open(&package.save().unwrap());
        let changed = execute_request(json!({"op":"apply_operations","document":document,
            "expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
                {"op":"set_hyperlink","slide_id":"slide-1","id":"link-0","link":"https://example.invalid/equivalent-edited"}
            ]})).unwrap();
        assert_eq!(links(&save(&changed["document"])).into_iter().map(|(_, target)| target).collect::<BTreeSet<_>>(),
            BTreeSet::from(["https://example.invalid/equivalent-edited".into(),"https://example.invalid/two".into(),"https://example.invalid/three-removed".into()]));
    }
}
