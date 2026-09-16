use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn sample() -> Value {
    let report = execute_request(json!({"op":"sample"})).unwrap();
    execute_request(json!({"op":"compile","report":report})).unwrap()["deck"].clone()
}

#[test]
fn design_presets_define_native_layouts_typography_and_spacing() {
    let presets = execute_request(json!({"op":"design_presets"})).unwrap();
    let presets = presets.as_array().unwrap();
    assert_eq!(presets.len(), 7);
    let mut signatures = std::collections::BTreeSet::new();
    for preset in presets {
        let design = preset["design"].clone();
        assert_eq!(design["masters"].as_array().unwrap().len(), 1);
        assert_eq!(design["layouts"].as_array().unwrap().len(), 7);
        assert!(preset["rules"]["margin"].as_f64().unwrap() >= 48.0);
        assert!(preset["rules"]["gutter"].as_f64().unwrap() >= 32.0);
        assert!(!design["theme"]["fonts"]["major"].as_str().unwrap().is_empty());
        let deck = execute_request(json!({"op":"update_design","deck":sample(),"design":design})).unwrap();
        execute_request(json!({"op":"validate","deck":deck})).unwrap();
        signatures.insert(preset["design"].to_string());
        let frames = preset["rules"]["regions"].as_array().unwrap();
        assert!(frames.iter().any(|frame| frame["layout_id"] == "preset-visual-content"));
        for frame in frames {
            assert!(frame["width"].as_f64().unwrap() > 0.0);
            assert!(frame["height"].as_f64().unwrap() > 0.0);
        }
    }
    assert_eq!(signatures.len(), 7);
}

#[test]
fn design_presets_preserve_content_and_replace_only_the_preset_templates() {
    let original = sample();
    let mut current = original.clone();
    for id in ["public", "minimal", "stylish", "pop", "dynamic", "trust", "luxury"] {
        current = execute_request(json!({"op":"apply_design_preset","deck":current,"preset_id":id})).unwrap();
        assert_eq!(current["slides"].as_array().unwrap().len(), original["slides"].as_array().unwrap().len());
        for (before, after) in original["slides"].as_array().unwrap().iter().zip(current["slides"].as_array().unwrap()) {
            assert_eq!(before["title"], after["title"]);
            assert_eq!(before["notes"], after["notes"]);
            let text = |slide: &Value| slide["elements"].as_array().unwrap().iter().map(|entry| (&entry["id"], &entry["text"])).map(|(id, text)| json!([id, text])).collect::<Vec<_>>();
            assert_eq!(text(before), text(after));
        }
        assert_eq!(current["design"]["masters"].as_array().unwrap().len(), 2);
        assert_eq!(current["design"]["layouts"].as_array().unwrap().len(), 11);
        assert_eq!(current["design"]["masters"][0]["id"], "master-1");
    }
    assert!(execute_request(json!({"op":"apply_design_preset","deck":original,"preset_id":"unknown"})).is_err());
}

#[test]
fn design_presets_retain_user_added_master_elements_and_layouts() {
    let mut deck = execute_request(json!({"op":"apply_design_preset","deck":sample(),"preset_id":"public"})).unwrap();
    let custom = json!({"type":"text","id":"company-note","x":64,"y":650,"width":700,"height":30,"text":"Retained company text","font_size":14,"color":"@dk1","bold":false});
    deck["design"]["masters"][1]["elements"].as_array_mut().unwrap().push(custom.clone());
    deck["design"]["layouts"].as_array_mut().unwrap().push(json!({"id":"company-layout","name":"Company layout","master_id":"preset-master","background":null,"elements":[custom.clone()]}));
    let updated = execute_request(json!({"op":"apply_design_preset","deck":deck,"preset_id":"minimal"})).unwrap();
    assert!(updated["design"]["masters"][1]["elements"].as_array().unwrap().iter().any(|element| element["id"] == custom["id"] && element["text"] == custom["text"]));
    assert!(updated["design"]["layouts"].as_array().unwrap().iter().any(|layout| layout["id"] == "company-layout"));
}

#[test]
fn design_presets_reject_foreign_ids_and_modified_templates() {
    let mut deck = sample();
    let mut design = execute_request(json!({"op":"design_defaults"})).unwrap();
    design["masters"][0]["id"] = json!("preset-master");
    for layout in design["layouts"].as_array_mut().unwrap() { layout["master_id"] = json!("preset-master"); }
    deck["design"] = design;
    assert!(execute_request(json!({"op":"apply_design_preset","deck":deck,"preset_id":"public"})).is_err());
    let mut deck = execute_request(json!({"op":"apply_design_preset","deck":sample(),"preset_id":"public"})).unwrap();
    let cover = deck["design"]["layouts"].as_array_mut().unwrap().iter_mut().find(|layout| layout["id"] == "preset-cover").unwrap();
    cover["elements"][0]["text"] = json!("Custom master title");
    assert!(execute_request(json!({"op":"apply_design_preset","deck":deck,"preset_id":"minimal"})).is_err());
}

#[test]
fn design_presets_remove_previous_style_artwork_on_every_switch() {
    let ids = ["public", "minimal", "stylish", "pop", "dynamic", "trust", "luxury"];
    for source in ids {
        let previous = execute_request(json!({"op":"apply_design_preset","deck":sample(),"preset_id":source})).unwrap();
        for target in ids {
            let actual = execute_request(json!({"op":"apply_design_preset","deck":previous,"preset_id":target})).unwrap();
            let expected = execute_request(json!({"op":"apply_design_preset","deck":sample(),"preset_id":target})).unwrap();
            assert_eq!(actual["design"], expected["design"], "{source} -> {target}");
        }
    }
}

#[test]
fn design_presets_reject_capacity_before_appending_templates() {
    let mut deck = sample();
    let mut design = execute_request(json!({"op":"design_defaults"})).unwrap();
    for index in 1..8 {
        design["masters"].as_array_mut().unwrap().push(json!({"id":format!("company-{index}"),"name":"Company master","background":"@lt1","elements":[]}));
        design["layouts"].as_array_mut().unwrap().push(json!({"id":format!("company-layout-{index}"),"name":"Company layout","master_id":format!("company-{index}"),"background":null,"elements":[]}));
    }
    deck["design"] = design;
    let error = execute_request(json!({"op":"apply_design_preset","deck":deck,"preset_id":"public"})).unwrap_err();
    assert!(error.to_string().contains("capacity"));
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