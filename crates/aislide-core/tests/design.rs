use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn sample() -> Value {
    let report = execute_request(json!({"op":"sample"})).unwrap();
    execute_request(json!({"op":"compile","report":report})).unwrap()["deck"].clone()
}

#[test]
fn native_masters_layouts_and_theme_are_editable_package_parts() {
    let mut design = execute_request(json!({"op":"design_defaults"})).unwrap();
    design["theme"]["name"] = json!("Company theme");
    design["theme"]["colors"]["accent1"] = json!("B53055");
    design["theme"]["fonts"]["major"] = json!("Arial");
    design["masters"][0]["elements"] = json!([{"type":"text","id":"company-footer","x":64,"y":665,"width":700,"height":30,"text":"Company master","font_size":14,"color":"@accent1","bold":false}]);
    let deck = execute_request(json!({"op":"update_design","deck":sample(),"design":design})).unwrap();
    let deck = execute_request(json!({"op":"assign_layout","deck":deck,"slide_id":"slide-1","layout_id":"title-content"})).unwrap();
    let exported = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert!(package.text("ppt/slideMasters/slideMaster1.xml").unwrap().contains("Company master"));
    assert!(package.parts().keys().filter(|name| name.starts_with("ppt/slideLayouts/slideLayout") && name.ends_with(".xml")).count() >= 4);
    let theme = package.text("ppt/theme/theme1.xml").unwrap();
    assert!(theme.contains("B53055")); assert!(theme.contains("Arial"));
    assert!(package.text("ppt/slides/slide1.xml").unwrap().contains("<p:ph"));
    assert!(package.text("ppt/slides/_rels/slide1.xml.rels").unwrap().contains("../slideLayouts/slideLayout2.xml"));
}

#[test]
fn layout_geometry_and_theme_binding_propagate_without_changing_text() {
    let design = execute_request(json!({"op":"design_defaults"})).unwrap();
    let deck = execute_request(json!({"op":"update_design","deck":sample(),"design":design})).unwrap();
    let mut deck = execute_request(json!({"op":"assign_layout","deck":deck,"slide_id":"slide-1","layout_id":"title-content"})).unwrap();
    let text = deck["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["id"] == "title").unwrap()["text"].clone();
    let mut design = deck["design"].clone();
    design["layouts"][1]["elements"][0]["x"] = json!(120);
    design["theme"]["colors"]["dk1"] = json!("112233");
    deck = execute_request(json!({"op":"update_design","deck":deck,"design":design})).unwrap();
    let title = deck["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["id"] == "title").unwrap();
    assert_eq!(title["x"], 120.0); assert_eq!(title["text"], text);
    assert_eq!(title["color"], "@dk1");
    assert_eq!(title["format"]["inherit_layout"], true);
}

#[test]
fn invalid_designs_fail_before_publication() {
    let mut design = execute_request(json!({"op":"design_defaults"})).unwrap();
    design["theme"]["colors"]["accent1"] = json!("red");
    assert!(execute_request(json!({"op":"update_design","deck":sample(),"design":design})).is_err());
    let mut design = execute_request(json!({"op":"design_defaults"})).unwrap();
    design["layouts"][0]["master_id"] = json!("missing");
    assert!(execute_request(json!({"op":"update_design","deck":sample(),"design":design})).is_err());
}

#[test]
fn applying_a_theme_binds_existing_palette_colors_and_preserves_unrelated_colors() {
    let mut deck = sample();
    deck["slides"][0]["elements"][0]["fill"] = json!("123456");
    let mut theme = execute_request(json!({"op":"design_defaults"})).unwrap()["theme"].clone();
    theme["colors"]["accent1"] = json!("B53055");
    let updated = execute_request(json!({"op":"apply_theme","deck":deck,"theme":theme})).unwrap();
    assert_eq!(updated["slides"][0]["elements"][0]["fill"], "123456");
    assert!(updated["slides"][0]["elements"].as_array().unwrap().iter().any(|element| element["fill"] == "@accent1"));
    assert_eq!(updated["design"]["theme"]["colors"]["accent1"], "B53055");
}