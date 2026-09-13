use aislide_core::{model::{Deck, validate_deck}, package::Package, pptx::export_pptx};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageEncoder, codecs::png::PngEncoder, ExtendedColorType};
use serde_json::{json, Value};

fn raster(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes).write_image(&vec![100; (width * height * 3) as usize], width, height, ExtendedColorType::Rgb8).unwrap();
    bytes
}

fn scene() -> Value {
    json!({"version":1,"title":"Graphic fixtures","width":1280,"height":720,"slides":[{"id":"slide-1","title":"Native graphics","background":"FFFFFF","notes":"Synthetic fixture","elements":[
        {"type":"picture","id":"photo","x":700,"y":100,"width":400,"height":300,"base64":STANDARD.encode(raster(4, 3)),"mime_type":"image/png","alt":"Synthetic raster & crop","crop":{"left":0.1,"right":0,"top":0,"bottom":0}},
        {"type":"group","id":"flow","x":50,"y":100,"width":500,"height":100,"view_width":500,"view_height":100,"children":[
            {"type":"rect","id":"start","x":0,"y":0,"width":180,"height":100,"fill":"087F73"},
            {"type":"rect","id":"end","x":320,"y":0,"width":180,"height":100,"fill":"CF5847"},
            {"type":"connector","id":"link","x":180,"y":50,"width":140,"height":1,"color":"586563","stroke_width":2,"arrow":true,"start":{"element_id":"start","site":3},"end":{"element_id":"end","site":1}}
        ]}
    ]}]})
}

#[test]
fn pictures_groups_and_connected_shapes_are_native() {
    let deck: Deck = serde_json::from_value(scene()).expect("native graphics scene contract");
    let bytes = export_pptx(&deck).unwrap();
    let package = Package::open(bytes).unwrap();
    let slide = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    for (tag, count) in [("pic", 1), ("grpSp", 1), ("cxnSp", 1), ("stCxn", 1), ("endCxn", 1)] {
        assert_eq!(slide.descendants().filter(|node| node.tag_name().name() == tag).count(), count);
    }
    let mut ids = std::collections::BTreeSet::new();
    for node in slide.descendants().filter(|node| node.tag_name().name() == "cNvPr") { assert!(ids.insert(node.attribute("id").unwrap())); }
    for node in slide.descendants().filter(|node| ["stCxn", "endCxn"].contains(&node.tag_name().name())) { assert!(ids.contains(node.attribute("id").unwrap())); }
    assert!(slide.descendants().any(|node| node.tag_name().name() == "srcRect" && node.attribute("l") == Some("10000")));
    assert!(package.parts().iter().any(|(name, data)| name.starts_with("ppt/media/") && data == &raster(4, 3)));
}

#[test]
fn malformed_or_unbounded_images_and_crops_are_rejected() {
    for patch in [
        json!({"base64":STANDARD.encode(b"<svg onload='alert(1)'/>"),"mime_type":"image/svg+xml"}),
        json!({"base64":STANDARD.encode(&raster(4, 3)[..24])}),
        json!({"base64":STANDARD.encode(raster(5000, 1))}),
        json!({"base64":"A".repeat(2 * 1024 * 1024)}),
        json!({"mime_type":"image/jpeg"}),
        json!({"crop":{"left":0.8,"right":0.3,"top":0,"bottom":0}})
    ] {
        let mut value = scene();
        for (key, entry) in patch.as_object().unwrap() { value["slides"][0]["elements"][0][key] = entry.clone(); }
        if let Ok(deck) = serde_json::from_value::<Deck>(value) { assert!(validate_deck(&deck).is_err()); }
    }
}

#[test]
fn groups_reject_duplicate_ids_missing_targets_and_excessive_depth() {
    let mut invalid = scene();
    invalid["slides"][0]["elements"][1]["children"][2]["start"]["element_id"] = json!("missing");
    if let Ok(deck) = serde_json::from_value::<Deck>(invalid) { assert!(validate_deck(&deck).is_err()); }
    let mut invalid = scene();
    invalid["slides"][0]["elements"][1]["children"][0]["id"] = json!("photo");
    if let Ok(deck) = serde_json::from_value::<Deck>(invalid) { assert!(validate_deck(&deck).is_err()); }
    let mut nested = json!({"type":"rect","id":"leaf","x":0,"y":0,"width":10,"height":10,"fill":"FFFFFF"});
    for index in 0..12 { nested = json!({"type":"group","id":format!("group-{index}"),"x":0,"y":0,"width":100,"height":100,"view_width":100,"view_height":100,"children":[nested]}); }
    let mut invalid = scene();
    invalid["slides"][0]["elements"] = json!([nested]);
    if let Ok(deck) = serde_json::from_value::<Deck>(invalid) { assert!(validate_deck(&deck).is_err()); }
}

#[test]
fn graphic_factories_are_shared_by_the_json_api() {
    let mut value = scene();
    value["slides"][0]["elements"] = json!([
        aislide_core::execute_request(json!({"op":"create_picture","id":"image","base64":STANDARD.encode(raster(4,3)),"mime_type":"image/png","alt":"Synthetic pixels"})).unwrap(),
        aislide_core::execute_request(json!({"op":"create_diagram","id":"diagram","steps":["Collect","Verify","Publish"]})).unwrap()
    ]);
    let deck: Deck = serde_json::from_value(value).unwrap();
    validate_deck(&deck).unwrap();
    assert!(export_pptx(&deck).is_ok());
    assert!(aislide_core::execute_request(json!({"op":"create_picture","id":"bad","base64":"","mime_type":"image/svg+xml","alt":"bad"})).is_err());
}

#[test]
fn validation_returns_canonical_defaults_for_scene_consumers() {
    let mut value = scene();
    value["slides"][0]["elements"][0]["crop"] = json!({"left":0.1});
    let result = aislide_core::execute_request(json!({"op":"validate","deck":value})).unwrap();
    assert_eq!(result["deck"]["slides"][0]["elements"][0]["crop"]["right"], json!(0.0));
    assert_eq!(result["deck"]["slides"][0]["elements"][1]["children"][2]["flip_v"], json!(false));
}