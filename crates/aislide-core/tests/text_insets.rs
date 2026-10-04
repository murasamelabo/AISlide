use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

// Expected preset text-rectangle insets for a 200x100 frame, from ECMA-376 presetShapeDefinitions.
const PRESETS: &[(&str, [f64; 4])] = &[
    ("rect", [0.0, 0.0, 0.0, 0.0]),
    ("roundRect", [100.0 * 0.16667 * 0.29289; 4]),
    ("ellipse", [100.0 * (1.0 - std::f64::consts::FRAC_1_SQRT_2), 50.0 * (1.0 - std::f64::consts::FRAC_1_SQRT_2), 100.0 * (1.0 - std::f64::consts::FRAC_1_SQRT_2), 50.0 * (1.0 - std::f64::consts::FRAC_1_SQRT_2)]),
    ("diamond", [50.0, 25.0, 50.0, 25.0]),
    ("can", [0.0, 25.0, 0.0, 12.5]),
    ("cloud", [200.0 * 2977.0 / 21600.0, 100.0 * 3262.0 / 21600.0, 200.0 * 4513.0 / 21600.0, 100.0 * 4263.0 / 21600.0]),
];

fn shape(id: &str, preset: &str, y: f64) -> Value {
    json!({"type":"shape","id":id,"x":40,"y":y,"width":200,"height":100,"preset":preset,"fill":"FFFFFF","stroke":"333333","stroke_width":1,"rotation":0,"text":"Synthetic","font_size":12,"color":"@dk1","bold":false,
        "format":{"padding":{"left":12,"top":12,"right":12,"bottom":12}}})
}

fn deck(elements: Vec<Value>) -> Value {
    json!({"version":1,"title":"Preset insets","width":1280,"height":720,"slides":[{"id":"slide","title":"Insets","background":"FFFFFF","notes":"","elements":elements}]})
}

fn insets(xml: &str, name: &str) -> [f64; 4] {
    let document = roxmltree::Document::parse(xml).unwrap();
    let shape = document.descendants().find(|node| node.tag_name().name() == "cNvPr" && node.attribute("name") == Some(name)).unwrap().ancestors().find(|node| node.tag_name().name() == "sp").unwrap();
    let body = shape.descendants().find(|node| node.tag_name().name() == "bodyPr").unwrap();
    ["lIns", "tIns", "rIns", "bIns"].map(|key| body.attribute(key).unwrap().parse::<f64>().unwrap() / 9525.0)
}

#[test]
fn explicit_shape_padding_is_written_relative_to_the_preset_text_rectangle_and_read_back() {
    let elements: Vec<_> = PRESETS.iter().enumerate().map(|(index, (preset, _))| shape(preset, preset, 20.0 + index as f64 * 110.0)).chain([shape("triangle", "triangle", 20.0)].into_iter().map(|mut value| { value["x"] = json!(400); value })).collect();
    let exported = execute_request(json!({"op":"export","deck":deck(elements)})).unwrap();
    let bytes = STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap();
    let xml = Package::open(bytes).unwrap().text("ppt/slides/slide1.xml").unwrap().to_owned();
    for (preset, [left, top, right, bottom]) in PRESETS {
        let written = insets(&xml, preset);
        for (actual, expected) in written.iter().zip([12.0 - left, 12.0 - top, 12.0 - right, 12.0 - bottom]) {
            assert!((actual - expected).abs() <= 1.0 / 9525.0 + 0.01, "{preset}: {written:?}");
        }
    }
    assert_eq!(insets(&xml, "triangle"), [12.0; 4], "unmodelled presets keep frame-relative insets");
    let opened = execute_request(json!({"op":"open_presentation","id":"inset-open","base64":exported["base64"]})).unwrap();
    for element in opened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap() {
        for side in ["left", "top", "right", "bottom"] {
            assert!((element["format"]["padding"][side].as_f64().unwrap() - 12.0).abs() <= 2.0 / 9525.0, "{element}");
        }
    }
}

#[test]
fn imported_insets_that_fill_a_small_preset_keep_the_shape_editable() {
    let mut pill = shape("pill", "roundRect", 40.0);
    pill["width"] = json!(30); pill["height"] = json!(20); pill["text"] = json!("1"); pill["format"]["padding"] = json!({"left":4,"top":4,"right":4,"bottom":4});
    let exported = execute_request(json!({"op":"export","deck":deck(vec![pill])})).unwrap();
    let mut parts = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap().parts().clone();
    let xml = String::from_utf8(parts["ppt/slides/slide1.xml"].clone()).unwrap();
    let start = xml.find("<a:bodyPr").unwrap(); let end = start + xml[start..].find('>').unwrap();
    // Google Slides and PowerPoint commonly write explicit 0.1in insets on every shape.
    let patched = format!("{}<a:bodyPr lIns=\"91425\" tIns=\"91425\" rIns=\"91425\" bIns=\"91425\" wrap=\"square\" anchor=\"ctr\"{}", &xml[..start], &xml[end..]);
    parts.insert("ppt/slides/slide1.xml".into(), patched.into_bytes());
    let bytes = Package::from_parts(parts).unwrap().save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"inset-small","base64":STANDARD.encode(bytes)})).unwrap();
    let element = &opened["document"]["deck"]["slides"][0]["elements"][0];
    assert_eq!(element["id"], "pill", "{opened}");
    assert!((element["format"]["padding"]["top"].as_f64().unwrap() - 91425.0 / 9525.0).abs() < 1e-9);
}

#[test]
fn round_rect_adjustment_and_custom_connection_sites_set_the_text_rectangle() {
    let open_padding = |elements: Vec<Value>| {
        let exported = execute_request(json!({"op":"export","deck":deck(elements)})).unwrap();
        let xml = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap().text("ppt/slides/slide1.xml").unwrap().to_owned();
        let opened = execute_request(json!({"op":"open_presentation","id":"inset-variants","base64":exported["base64"]})).unwrap();
        (xml, opened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().clone())
    };
    let assert_padding = |element: &Value| for side in ["left", "top", "right", "bottom"] {
        assert!((element["format"]["padding"][side].as_f64().unwrap() - 12.0).abs() <= 2.0 / 9525.0, "{element}");
    };
    let rounded = |id: &str, y: f64, value: i32| { let mut element = shape(id, "roundRect", y); element["visual"] = json!({"adjustments":[{"name":"adj","value":value}]}); element };
    let (xml, opened) = open_padding(vec![rounded("square-corner", 20.0, 0), rounded("pill", 140.0, 50000)]);
    // adj 0 has no corner inset; adj 50000 insets the text rectangle by 29.289% of half the short side.
    for (name, inset) in [("square-corner", 0.0), ("pill", 100.0 * 0.5 * 0.29289)] {
        for value in insets(&xml, name) { assert!((value - (12.0 - inset)).abs() <= 0.01, "{name}: {value}"); }
    }
    for (element, value) in opened.iter().zip([0, 50000]) {
        assert_eq!(element["visual"]["adjustments"][0]["value"], value, "{element}");
        assert_padding(element);
    }
    let mut sited = shape("sited", "roundRect", 20.0);
    sited["visual"] = json!({"connection_sites":[{"x":0.5,"y":0.0,"angle":270.0},{"x":0.25,"y":1.0,"angle":90.0}]});
    let (xml, opened) = open_padding(vec![sited.clone()]);
    // Custom connection sites are written as custGeom with a full-frame text rectangle, so insets equal the padding.
    assert!(xml.contains("<a:custGeom>") && xml.contains("<a:rect l=\"l\" t=\"t\" r=\"r\" b=\"b\"/>"));
    assert_eq!(insets(&xml, "sited"), [12.0; 4]);
    assert_eq!(opened[0]["preset"], "roundRect");
    assert_eq!(opened[0]["visual"]["connection_sites"], sited["visual"]["connection_sites"]);
    assert_padding(&opened[0]);
}

#[test]
fn non_uniform_native_ungroup_writes_insets_that_reopen_as_the_document_padding() {
    // Exact 2 x 3 factors keep the transformed element identical, so ungroup takes the scaled-style native path.
    let child = shape("rounded", "roundRect", 10.0);
    let group = json!({"type":"group","id":"group","x":100,"y":100,"width":600,"height":360,"view_width":300,"view_height":120,"children":[child]});
    let exported = execute_request(json!({"op":"export","deck":deck(vec![group])})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"inset-ungroup","base64":exported["base64"]})).unwrap();
    let document = &opened["document"];
    let changed = execute_request(json!({"op":"edit_selection","document":document,"expected_revision":document["revision"],
        "slide_id":document["deck"]["slides"][0]["id"],"operation":{"op":"ungroup","ids":["group"]}})).unwrap();
    let ungrouped = &changed["transaction"]["document"];
    let element = &ungrouped["deck"]["slides"][0]["elements"][0];
    for (actual, expected) in [(element["width"].as_f64().unwrap(), 400.0), (element["height"].as_f64().unwrap(), 300.0)] { assert!((actual - expected).abs() < 1e-9, "non-uniform 2 x 3 scale: {actual}"); }
    let saved = execute_request(json!({"op":"export_presentation","document":ungrouped})).unwrap();
    let xml = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap().text("ppt/slides/slide1.xml").unwrap().to_owned();
    let corner = element["width"].as_f64().unwrap().min(element["height"].as_f64().unwrap()) * 0.16667 * 0.29289;
    let padding = &element["format"]["padding"];
    for (value, side) in insets(&xml, "rounded").iter().zip(["left", "top", "right", "bottom"]) {
        assert!((value - (padding[side].as_f64().unwrap() - corner)).abs() <= 0.01, "{side}: {value} for {padding}");
    }
    let reopened = execute_request(json!({"op":"open_presentation","id":"inset-ungroup-reopen","base64":saved["base64"]})).unwrap();
    let restored = &reopened["document"]["deck"]["slides"][0]["elements"][0]["format"]["padding"];
    for side in ["left", "top", "right", "bottom"] { assert!((restored[side].as_f64().unwrap() - padding[side].as_f64().unwrap()).abs() <= 2.0 / 9525.0, "{side}: {restored} vs {padding}"); }
}

#[test]
fn resizing_a_native_preset_rewrites_its_insets_to_keep_frame_relative_padding() {
    let exported = execute_request(json!({"op":"export","deck":deck(vec![shape("rounded", "roundRect", 40.0)])})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"inset-resize","base64":exported["base64"]})).unwrap();
    let document = &opened["document"];
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
        {"op":"replace","path":"/deck/slides/0/elements/0/height","value":300}
    ]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let xml = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap().text("ppt/slides/slide1.xml").unwrap().to_owned();
    let expected = 12.0 - 200.0 * 0.16667 * 0.29289;
    for value in insets(&xml, "rounded") { assert!((value - expected).abs() <= 0.01, "{value} != {expected}"); }
    let reopened = execute_request(json!({"op":"open_presentation","id":"inset-resize-reopen","base64":saved["base64"]})).unwrap();
    let padding = &reopened["document"]["deck"]["slides"][0]["elements"][0]["format"]["padding"];
    for side in ["left", "top", "right", "bottom"] { assert!((padding[side].as_f64().unwrap() - 12.0).abs() <= 2.0 / 9525.0, "{padding}"); }
}
