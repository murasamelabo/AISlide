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
