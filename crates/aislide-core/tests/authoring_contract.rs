use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn scene() -> Value {
    json!({"version":1,"title":"Authoring contract","width":1280,"height":720,"slides":[{"id":"slide-1","title":"Example","background":"FFFFFF","notes":"Synthetic example","elements":[{"type":"text","id":"title","x":64,"y":80,"width":1152,"height":120,"text":"Original title","font_size":42,"color":"202525","bold":true}]}]})
}

#[test]
fn each_native_master_owns_a_distinct_theme_part() {
    let mut deck = scene();
    let mut design = execute_request(json!({"op":"design_defaults"})).unwrap();
    design["masters"].as_array_mut().unwrap().push(json!({"id":"master-2","name":"Second master","background":"@lt1","elements":[]}));
    design["layouts"].as_array_mut().unwrap().push(json!({"id":"second-blank","name":"Second blank","master_id":"master-2","background":null,"elements":[]}));
    deck["design"] = design;
    let exported = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let first = package.text("ppt/slideMasters/_rels/slideMaster1.xml.rels").unwrap();
    let second = package.text("ppt/slideMasters/_rels/slideMaster2.xml.rels").unwrap();
    assert!(first.contains("../theme/theme1.xml"));
    assert!(second.contains("../theme/theme3.xml"));
    assert!(package.part("ppt/theme/theme3.xml").is_ok());
    let mut ids = std::collections::BTreeSet::new();
    for part in ["ppt/presentation.xml", "ppt/slideMasters/slideMaster1.xml", "ppt/slideMasters/slideMaster2.xml"] {
        let document = roxmltree::Document::parse(package.text(part).unwrap()).unwrap();
        for node in document.descendants().filter(|node| ["sldMasterId", "sldLayoutId"].contains(&node.tag_name().name())) { assert!(ids.insert(node.attribute("id").unwrap().to_owned()), "master and layout IDs must not collide"); }
    }
}

#[test]
fn inherited_placeholders_do_not_freeze_native_geometry_or_font() {
    let mut deck = execute_request(json!({"op":"assign_layout","deck":scene(),"slide_id":"slide-1","layout_id":"title-content"})).unwrap();
    deck["design"]["layouts"][1]["elements"][0]["format"]["italic"] = json!(true);
    deck = execute_request(json!({"op":"update_design","deck":deck,"design":deck["design"]})).unwrap();
    let result = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let package = Package::open(STANDARD.decode(result["base64"].as_str().unwrap()).unwrap()).unwrap();
    let parsed = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    let shape = parsed.descendants().find(|node| node.tag_name().name() == "sp" && node.descendants().any(|entry| entry.tag_name().name() == "ph" && entry.attribute("idx") == Some("0"))).unwrap();
    assert!(!shape.descendants().any(|node| node.tag_name().name() == "xfrm"));
    assert!(!shape.descendants().any(|node| node.tag_name().name() == "rPr" && node.attribute("sz").is_some()));
    let layout = package.text("ppt/slideLayouts/slideLayout2.xml").unwrap();
    assert!(layout.contains("<a:lvl1pPr"));
    assert!(layout.contains("<a:defRPr"));
}

#[test]
fn inherited_overrides_require_detachment_and_removed_templates_preserve_text() {
    let deck = execute_request(json!({"op":"assign_layout","deck":scene(),"slide_id":"slide-1","layout_id":"title-content"})).unwrap();
    let mut moved = deck.clone(); moved["slides"][0]["elements"][0]["x"] = json!(100);
    assert!(execute_request(json!({"op":"validate","deck":moved})).is_err());
    moved["slides"][0]["elements"][0]["format"]["inherit_layout"] = json!(false);
    assert!(execute_request(json!({"op":"validate","deck":moved})).is_ok());
    let mut design = deck["design"].clone(); design["layouts"][1]["elements"].as_array_mut().unwrap().remove(0);
    let updated = execute_request(json!({"op":"update_design","deck":deck,"design":design})).unwrap();
    let title = &updated["slides"][0]["elements"][0];
    assert_eq!(title["text"], "Original title");
    assert_eq!(title["format"]["inherit_layout"], false);
    assert!(title["format"]["placeholder"].is_null());
}

#[test]
fn theme_and_shape_fonts_are_used_for_measurement() {
    let mut design = execute_request(json!({"op":"design_defaults"})).unwrap();
    design["theme"]["fonts"]["major"] = json!("Arial");
    design["theme"]["fonts"]["minor"] = json!("Arial");
    let mut deck = scene(); deck["design"] = design;
    deck["slides"][0]["elements"][0]["format"] = json!({"font_family":"@major","italic":true});
    let mut shape = execute_request(json!({"op":"create_object","id":"shape-text","kind":"shape","preset":"rect"})).unwrap();
    shape["text"] = json!("Shape text");
    deck["slides"][0]["elements"].as_array_mut().unwrap().push(shape);
    let measured = execute_request(json!({"op":"measure_layout","deck":deck})).unwrap();
    let measurements = measured["measurements"].as_array().unwrap();
    assert!(measurements.iter().any(|entry| entry["element_id"] == "title" && entry["requested_family"] == "Arial"));
    assert!(measurements.iter().any(|entry| entry["element_id"] == "shape-text" && entry["requested_family"] == "Arial"));
}

#[test]
fn switching_layouts_detaches_obsolete_placeholders_and_preserves_their_content() {
    let mut deck = execute_request(json!({"op":"assign_layout","deck":scene(),"slide_id":"slide-1","layout_id":"title-content"})).unwrap();
    deck["slides"][0]["elements"][1]["text"] = json!("Preserve this body");
    let changed = execute_request(json!({"op":"assign_layout","deck":deck,"slide_id":"slide-1","layout_id":"blank"})).unwrap();
    assert_eq!(changed["slides"][0]["layout_id"], "blank");
    assert_eq!(changed["slides"][0]["elements"][0]["text"], "Original title");
    assert_eq!(changed["slides"][0]["elements"][1]["text"], "Preserve this body");
    assert_eq!(changed["slides"][0]["elements"][1]["format"]["inherit_layout"], false);
    assert!(changed["slides"][0]["elements"][1]["format"]["placeholder"].is_null());
}

#[test]
fn imported_new_style_and_design_fields_are_never_silently_ignored() {
    let exported = execute_request(json!({"op":"export","deck":scene()})).unwrap();
    let imported = execute_request(json!({"op":"import_pptx","base64":exported["base64"]})).unwrap();
    let deck = imported["deck"].clone();
    let mut formatted = deck.clone(); formatted["slides"][0]["elements"][0]["format"] = json!({"italic":true});
    assert!(execute_request(json!({"op":"save_import","base64":exported["base64"],"deck":formatted})).is_err());
    let mut designed = deck.clone(); designed["design"] = execute_request(json!({"op":"design_defaults"})).unwrap();
    assert!(execute_request(json!({"op":"save_import","base64":exported["base64"],"deck":designed})).is_err());
    let mut hidden = deck; hidden["slides"][0]["hide_master_graphics"] = json!(true);
    assert!(execute_request(json!({"op":"save_import","base64":exported["base64"],"deck":hidden})).is_err());
}

#[test]
fn template_elements_cannot_claim_slide_only_inheritance() {
    for owner in ["masters", "layouts"] {
        let mut deck = scene();
        let mut design = execute_request(json!({"op":"design_defaults"})).unwrap();
        let mut placeholder = design["layouts"][1]["elements"][0].clone();
        placeholder["format"]["inherit_layout"] = json!(true);
        if owner == "masters" { design[owner][0]["elements"].as_array_mut().unwrap().push(placeholder); }
        else { design[owner][1]["elements"][0] = placeholder; }
        deck["design"] = design;
        assert!(execute_request(json!({"op":"validate","deck":deck})).is_err(), "template owner {owner} must reject inheritance");
    }
    let inherited = execute_request(json!({"op":"assign_layout","deck":scene(),"slide_id":"slide-1","layout_id":"title-content"})).unwrap();
    assert!(execute_request(json!({"op":"validate","deck":inherited})).is_ok());
}

#[test]
fn duplicate_slide_placeholder_indices_fail_but_freeform_text_remains_valid() {
    let mut deck = execute_request(json!({"op":"assign_layout","deck":scene(),"slide_id":"slide-1","layout_id":"title-content"})).unwrap();
    let mut duplicate = deck["slides"][0]["elements"][0].clone();
    duplicate["id"] = json!("another-title");
    deck["slides"][0]["elements"].as_array_mut().unwrap().push(duplicate);
    assert!(execute_request(json!({"op":"validate","deck":deck})).is_err());
    let last = deck["slides"][0]["elements"].as_array_mut().unwrap().last_mut().unwrap();
    last["format"]["placeholder"] = Value::Null;
    last["format"]["inherit_layout"] = json!(false);
    assert!(execute_request(json!({"op":"validate","deck":deck})).is_ok());
}