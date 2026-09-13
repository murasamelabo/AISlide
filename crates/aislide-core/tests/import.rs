use aislide_core::{execute_request, model::Deck, package::Package, pptx::export_pptx};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::json;

fn presentation() -> Vec<u8> {
    let report = execute_request(json!({"op":"sample"})).unwrap();
    let compiled = execute_request(json!({"op":"compile","report":report})).unwrap();
    let mut deck: Deck = serde_json::from_value(compiled["deck"].clone()).unwrap();
    deck.slides[1].elements.push(aislide_core::graphics::create_diagram("process", &["Input".into(), "Output".into()]).unwrap());
    let mut parts = Package::open(export_pptx(&deck).unwrap()).unwrap().parts().clone();
    parts.insert("ppt/opaque/preserved.bin".into(), vec![0, 1, 254, 255]);
    Package::from_parts(parts).unwrap().save().unwrap()
}

#[test]
fn supported_import_has_visual_scene_and_lossless_noop_export() {
    let bytes = presentation();
    let imported = execute_request(json!({"op":"import_pptx","base64":STANDARD.encode(&bytes)})).unwrap();
    assert_eq!(imported["deck"]["slides"].as_array().unwrap().len(), 12);
    assert!(imported["deck"]["slides"][0]["elements"].as_array().unwrap().iter().any(|element| element["type"] == "text"));
    assert!(imported["deck"]["slides"][1]["elements"].as_array().unwrap().iter().any(|element| element["type"] == "group"));
    let saved = execute_request(json!({"op":"save_import","base64":STANDARD.encode(&bytes),"deck":imported["deck"]})).unwrap();
    assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), bytes);
}

#[test]
fn imported_text_geometry_edits_preserve_every_other_part() {
    let bytes = presentation();
    let imported = execute_request(json!({"op":"import_pptx","base64":STANDARD.encode(&bytes)})).unwrap();
    let mut deck = imported["deck"].clone();
    let target = deck["slides"][2]["elements"].as_array_mut().unwrap().iter_mut().find(|element| element["type"] == "text" && element["text"].as_str().unwrap().contains("Three signals")).unwrap();
    target["text"] = json!("Edited imported title"); target["x"] = json!(70);
    let saved = execute_request(json!({"op":"save_import","base64":STANDARD.encode(&bytes),"deck":deck})).unwrap();
    let original = Package::open(bytes).unwrap();
    let updated = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    for (name, value) in original.parts() { if name != "ppt/slides/slide3.xml" { assert_eq!(updated.part(name).unwrap(), value); } }
    let reloaded = execute_request(json!({"op":"import_pptx","base64":saved["base64"]})).unwrap();
    assert!(reloaded["deck"]["slides"][2]["elements"].as_array().unwrap().iter().any(|element| element["text"] == "Edited imported title" && element["x"] == 70.0));
}

#[test]
fn unsupported_import_changes_and_signed_edits_fail_closed() {
    let bytes = presentation();
    let imported = execute_request(json!({"op":"import_pptx","base64":STANDARD.encode(&bytes)})).unwrap();
    let mut changed = imported["deck"].clone(); changed["slides"][0]["elements"].as_array_mut().unwrap().pop();
    assert!(execute_request(json!({"op":"save_import","base64":STANDARD.encode(&bytes),"deck":changed})).is_err());
    let mut changed = imported["deck"].clone(); changed["slides"][0]["background"] = json!("FF0000");
    assert!(execute_request(json!({"op":"save_import","base64":STANDARD.encode(&bytes),"deck":changed})).is_err());
    let mut parts = Package::open(bytes).unwrap().parts().clone(); parts.insert("_xmlsignatures/signature.xml".into(), b"<signature/>".to_vec());
    let signed = Package::from_parts(parts).unwrap().save().unwrap();
    assert!(execute_request(json!({"op":"save_import","base64":STANDARD.encode(signed),"deck":changed})).is_err());
}

#[test]
fn imported_document_transactions_keep_origin_and_block_unsupported_edits() {
    let bytes = presentation();
    let imported = execute_request(json!({"op":"import_document","id":"imported","base64":STANDARD.encode(&bytes)})).unwrap();
    let document = &imported["document"];
    let exported = execute_request(json!({"op":"export_project","document":document})).unwrap();
    assert_eq!(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap(), bytes);
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/slides/0/background","value":"FF0000"}]}}));
    assert!(changed.is_err());
    let detached = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"remove","path":"/origin"}]}}));
    assert!(detached.is_err());
    let restored = execute_request(json!({"op":"open_project","base64":exported["base64"],"checkpoint":exported["checkpoint"]})).unwrap();
    assert_eq!(restored, *document);
}

#[test]
fn package_manifest_accounts_for_opaque_payloads() {
    let bytes = presentation();
    let manifest = execute_request(json!({"op":"package_manifest","base64":STANDARD.encode(&bytes)})).unwrap();
    let opaque = manifest["parts"].as_array().unwrap().iter().find(|part| part["path"] == "ppt/opaque/preserved.bin").unwrap();
    assert_eq!(opaque["bytes"], 4);
    assert_eq!(opaque["sha256"].as_str().unwrap().len(), 64);
    let package = Package::open(bytes).unwrap();
    assert_eq!(manifest["parts"].as_array().unwrap().len(), package.parts().len());
}