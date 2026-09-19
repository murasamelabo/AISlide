use aislide_core::execute_request;
use serde_json::{Value, json};

fn text_deck(slides: usize) -> Value {
    let report = execute_request(json!({"op":"sample"})).unwrap();
    let compiled = execute_request(json!({"op":"compile","report":report})).unwrap();
    let mut deck = compiled["deck"].clone();
    let slide = deck["slides"][0].clone();
    deck["slides"] = Value::Array((0..slides).map(|index| {
        let mut copy = slide.clone();
        copy["id"] = json!(format!("synthetic-{index}"));
        copy
    }).collect());
    deck
}

#[test]
fn large_profile_roundtrips_256_custom_identities_with_request_local_limits() {
    let mut deck = text_deck(256);
    let mut design = execute_request(json!({"op":"design_defaults"})).unwrap();
    let master = design["masters"][0].clone();
    let layout = design["layouts"][0].clone();
    design["masters"] = json!((0..8).map(|index| { let mut next = master.clone(); next["id"] = json!(format!("custom-master-{index}")); next }).collect::<Vec<_>>());
    design["layouts"] = json!((0..32).map(|index| { let mut next = layout.clone(); next["id"] = json!(format!("custom-layout-{index}")); next["master_id"] = json!(format!("custom-master-{}", index / 4)); next }).collect::<Vec<_>>());
    deck["design"] = design;
    let document = execute_request(json!({"op":"new_document","capacity_profile":"large","id":"large-256","deck":deck})).unwrap();
    assert!(execute_request(json!({"op":"verify_recovery","capacity_profile":"standard","document":document})).is_err());
    let changed = execute_request(json!({"op":"transaction","capacity_profile":"large","document":document,"transaction":{
        "expected_revision":0,"expected_hash":document["hash"],
        "operations":[{"op":"replace","path":"/deck/slides/255/title","value":"Last slide changed"}]
    }})).unwrap();
    let restored = execute_request(json!({"op":"undo_transaction","capacity_profile":"large","document":changed["document"],"expected_revision":1,"receipt":changed["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], document["hash"]);
    let exported = execute_request(json!({"op":"export_presentation","capacity_profile":"large","document":restored["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","capacity_profile":"large","id":"large-native","base64":exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][255]["id"], "synthetic-255");
    assert_eq!(reopened["document"]["deck"]["design"]["masters"][7]["id"], "custom-master-7");
    assert_eq!(reopened["document"]["deck"]["design"]["layouts"][31]["id"], "custom-layout-31");
    assert!(execute_request(json!({"op":"validate","capacity_profile":"large","deck":text_deck(257)})).is_err());
    assert!(execute_request(json!({"op":"validate","capacity_profile":"legacy","deck":text_deck(33)})).is_err());
    assert!(execute_request(json!({"op":"validate","capacity_profile":"large","deck":deck})).is_ok());
}

#[test]
fn standard_accepts_64_text_slides_without_changing_the_deck_schema() {
    let result = execute_request(json!({"op":"validate","deck":text_deck(64)})).unwrap();
    assert_eq!(result["slides"], 64);
    assert!(result["deck"].get("capacity_profile").is_none());
}

#[test]
fn profiles_are_explicit_bounded_and_request_local() {
    assert!(execute_request(json!({"op":"validate","capacity_profile":"legacy","deck":text_deck(32)})).is_ok());
    assert!(execute_request(json!({"op":"validate","capacity_profile":"legacy","deck":text_deck(33)})).is_err());
    assert!(execute_request(json!({"op":"validate","capacity_profile":"standard","deck":text_deck(128)})).is_ok());
    assert!(execute_request(json!({"op":"validate","capacity_profile":"standard","deck":text_deck(129)})).is_err());
    assert!(execute_request(json!({"op":"validate","deck":text_deck(256)})).is_ok());
    for profile in [json!("unlimited"), json!(null), json!({"slides":100000})] {
        assert!(execute_request(json!({"op":"sample","capacity_profile":profile})).is_err());
    }
    assert!(execute_request(json!({"op":"validate","deck":text_deck(64)})).is_ok());
}

#[test]
fn oversized_scene_is_rejected_before_typed_deserialization() {
    let mut deck = text_deck(257);
    deck["slides"][0]["elements"][0]["unknown_field"] = json!(true);
    let error = execute_request(json!({"op":"validate","deck":deck})).unwrap_err();
    assert!(matches!(error, aislide_core::Error::Limit(_)), "{error}");
}

#[test]
fn medium_document_edits_undoes_exports_and_reopens() {
    let started = std::time::Instant::now();
    let document = execute_request(json!({"op":"new_document","id":"g38-synthetic","deck":text_deck(64)})).unwrap();
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{
        "expected_revision":0,"expected_hash":document["hash"],
        "operations":[{"op":"replace","path":"/deck/title","value":"Synthetic G38 changed"}]
    }})).unwrap();
    let restored = execute_request(json!({"op":"undo_transaction","document":changed["document"],
        "expected_revision":1,"receipt":changed["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], document["hash"]);
    assert!(serde_json::to_vec(&changed["receipt"]).unwrap().len() < 1024);
    let exported = execute_request(json!({"op":"export_presentation","document":restored["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"reopened","base64":exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"].as_array().unwrap().len(), 64);
    println!("G38 synthetic64 document_bytes={} reopened_bytes={} export_encoded_bytes={} elapsed_ms={}",
        serde_json::to_vec(&document).unwrap().len(), serde_json::to_vec(&reopened["document"]).unwrap().len(),
        exported["base64"].as_str().unwrap().len(), started.elapsed().as_millis());
}

#[test]
fn repeated_images_share_media_but_charts_keep_independent_workbooks() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut buffer = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(2, 2).write_to(&mut buffer, image::ImageFormat::Png).unwrap();
    let picture = execute_request(json!({"op":"create_picture","id":"shared-picture","base64":STANDARD.encode(buffer.into_inner()),"mime_type":"image/png","alt":"Synthetic"})).unwrap();
    let chart = execute_request(json!({"op":"create_object","id":"independent-chart","kind":"chart"})).unwrap();
    let mut deck = text_deck(4);
    for slide in deck["slides"].as_array_mut().unwrap() {
        slide["elements"].as_array_mut().unwrap().extend([picture.clone(), chart.clone()]);
    }
    let exported = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let package = aislide_core::package::Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(package.parts().keys().filter(|name| name.starts_with("ppt/media/")).count(), 1);
    assert_eq!(package.parts().keys().filter(|name| name.starts_with("ppt/embeddings/")).count(), 4);
    for number in 1..=4 {
        assert!(package.text(&format!("ppt/slides/_rels/slide{number}.xml.rels")).unwrap().contains("../media/image1.png"));
    }
}

#[test]
fn counts_depth_and_image_payload_remain_fail_closed() {
    let rectangle = json!({"type":"rect","id":"r","x":1,"y":1,"width":10,"height":10,"fill":"FFFFFF"});
    let mut deck = text_deck(32);
    for slide in deck["slides"].as_array_mut().unwrap() {
        slide["elements"] = json!((0..256).map(|index| { let mut item = rectangle.clone(); item["id"] = json!(format!("r{index}")); item }).collect::<Vec<_>>());
    }
    assert!(execute_request(json!({"op":"validate","deck":deck})).is_ok());
    let mut extra = deck["slides"][0].clone(); extra["id"] = json!("overflow"); extra["elements"] = json!([rectangle]);
    deck["slides"].as_array_mut().unwrap().push(extra);
    assert!(execute_request(json!({"op":"validate","deck":deck})).unwrap_err().to_string().contains("scene elements"));
    let mut nested = rectangle.clone();
    for depth in 0..=8 {
        nested = json!({"type":"group","id":format!("g{depth}"),"x":0,"y":0,"width":20,"height":20,"view_width":20,"view_height":20,"children":[nested]});
        let mut deck = text_deck(1); deck["slides"][0]["elements"] = json!([nested]);
        assert_eq!(execute_request(json!({"op":"validate","deck":deck})).is_ok(), depth < 8);
    }
    let mut deck = text_deck(1);
    deck["slides"][0]["elements"] = json!([{"type":"picture","id":"large","x":0,"y":0,"width":10,"height":10,"base64":"!".repeat(3*1024*1024+1),"mime_type":"image/png","alt":"Synthetic","crop":{}}]);
    assert!(execute_request(json!({"op":"validate","deck":deck})).unwrap_err().to_string().contains("image payload"));
}

#[test]
fn document_and_raw_json_allocation_budgets_are_bounded() {
    let mut deck = text_deck(128);
    for slide in deck["slides"].as_array_mut().unwrap() {
        slide["elements"] = json!((0..8).map(|index| json!({"type":"text","id":format!("t{index}"),"x":0,"y":0,"width":100,"height":100,"font_size":12,"text":"a".repeat(3000),"color":"000000","bold":false})).collect::<Vec<_>>());
    }
    let document = execute_request(json!({"op":"new_document","id":"medium-bytes","deck":deck})).unwrap();
    assert!(serde_json::to_vec(&document).unwrap().len() > 2*1024*1024);
    assert!(execute_request(json!({"op":"verify_recovery","document":document})).is_ok());
    assert!(execute_request(json!({"op":"verify_recovery","capacity_profile":"legacy","document":document})).is_err());
    for slide in deck["slides"].as_array_mut().unwrap() {
        let elements = slide["elements"].as_array().unwrap().clone();
        slide["elements"].as_array_mut().unwrap().extend(elements.clone());
        slide["elements"].as_array_mut().unwrap().extend(elements);
    }
    assert!(execute_request(json!({"op":"new_document","capacity_profile":"standard","id":"too-large","deck":deck})).unwrap_err().to_string().contains("document"));
    assert!(aislide_core::preflight::json(&vec![b' '; 96*1024*1024+1]).is_err());
    let many = format!("[{}null]", "null,".repeat(250_000));
    assert!(aislide_core::preflight::json(many.as_bytes()).unwrap_err().to_string().contains("node budget"));
    let deep = format!("{}0{}", "[".repeat(65), "]".repeat(65));
    assert!(aislide_core::preflight::json(deep.as_bytes()).unwrap_err().to_string().contains("depth"));
    assert!(aislide_core::preflight::json(br#"{"escaped":"a\"b","number":1.2,"null":null,"bool":true}"#).is_ok());
}

#[test]
fn image_resource_count_is_bounded() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut deck = text_deck(1);
    let mut pictures = Vec::new();
    for index in 0..129 {
        let image = image::RgbImage::from_pixel(1,1,image::Rgb([index as u8,0,0]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        pictures.push(json!({"type":"picture","id":format!("p{index}"),"x":0,"y":0,"width":10,"height":10,"base64":STANDARD.encode(bytes.into_inner()),"mime_type":"image/png","alt":"Synthetic","crop":{}}));
    }
    deck["slides"][0]["elements"] = json!(&pictures[..128]);
    assert!(execute_request(json!({"op":"validate","deck":deck})).is_ok());
    deck["slides"][0]["elements"] = json!(pictures);
    assert!(execute_request(json!({"op":"validate","deck":deck})).unwrap_err().to_string().contains("unique scene images"));
}

#[tokio::test]
async fn cancelled_work_is_rejected_before_deserializing() {
    let token = aislide_core::generation::CancellationToken::new(); token.cancel();
    let error = aislide_core::protocol::execute_request_async(json!({"op":"unknown"}), token).await.unwrap_err();
    assert!(error.to_string().contains("cancelled"));
}

#[test]
fn raster_work_and_chart_part_counts_are_preflighted() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut deck = text_deck(1);
    let mut pictures = Vec::new();
    for color in [0, 1] {
        let image = image::RgbImage::from_pixel(2049,4096,image::Rgb([color,0,0]));
        let mut bytes = std::io::Cursor::new(Vec::new()); image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        pictures.push(json!({"type":"picture","id":format!("p{color}"),"x":0,"y":0,"width":10,"height":10,"base64":STANDARD.encode(bytes.into_inner()),"mime_type":"image/png","alt":"Synthetic","crop":{}}));
    }
    deck["slides"][0]["elements"] = json!(pictures);
    let typed = serde_json::from_value(deck).unwrap();
    assert!(aislide_core::preflight::standard_deck(&typed).unwrap_err().to_string().contains("raster work"));
    let mut deck = text_deck(32);
    let chart = execute_request(json!({"op":"create_object","id":"chart","kind":"chart"})).unwrap();
    for slide in deck["slides"].as_array_mut().unwrap() {
        slide["elements"] = json!((0..48).map(|index| { let mut copy=chart.clone(); copy["id"]=json!(format!("c{index}")); copy }).collect::<Vec<_>>());
    }
    let typed = serde_json::from_value(deck).unwrap();
    assert!(aislide_core::preflight::standard_deck(&typed).unwrap_err().to_string().contains("resource part"));
}

#[test]
fn changing_one_shared_picture_does_not_change_another_native_picture() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let asset = |id: &str, color: &str| execute_request(json!({"op":"create_asset","id":id,"mime_type":"image/svg+xml","alt":"Synthetic","size":32,
        "base64":STANDARD.encode(format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"2\" height=\"2\"><rect width=\"2\" height=\"2\" fill=\"{color}\"/></svg>"))})).unwrap();
    let picture = asset("picture", "red");
    let mut deck = text_deck(2);
    for slide in deck["slides"].as_array_mut().unwrap() { slide["elements"] = json!([picture]); }
    let document = execute_request(json!({"op":"new_document","id":"shared","deck":deck})).unwrap();
    let exported = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    let document = execute_request(json!({"op":"open_presentation","id":"native-shared","base64":exported["base64"]})).unwrap()["document"].clone();
    let replacement = asset("picture", "blue");
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],
        "operations":[{"op":"replace","path":"/deck/slides/0/elements/0/base64","value":replacement["base64"]},{"op":"replace","path":"/deck/slides/0/elements/0/svg","value":replacement["svg"]}]}})).unwrap();
    let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"changed","base64":output["base64"]})).unwrap();
    assert_eq!(opened["document"]["deck"]["slides"][0]["elements"][0]["base64"], replacement["base64"]);
    assert_eq!(opened["document"]["deck"]["slides"][1]["elements"][0]["base64"], picture["base64"]);
}

#[test]
fn envelope_bytes_and_legacy_template_counts_cannot_escape_the_profile() {
    let mut request = json!({"op":"sample","capacity_profile":"legacy","extra":""});
    let overhead = serde_json::to_vec(&request).unwrap().len();
    request["extra"] = json!("a".repeat(aislide_core::limits::LEGACY.request_bytes + 1 - overhead));
    assert!(matches!(execute_request(request).unwrap_err(), aislide_core::Error::Limit(_)));
    let mut deck = text_deck(8);
    let rectangle = json!({"type":"rect","id":"rect","x":0,"y":0,"width":10,"height":10,"fill":"000000"});
    for slide in deck["slides"].as_array_mut().unwrap() {
        slide["elements"] = json!((0..256).map(|index| { let mut item=rectangle.clone(); item["id"]=json!(format!("r{index}")); item }).collect::<Vec<_>>());
    }
    deck["design"] = execute_request(json!({"op":"design_defaults"})).unwrap();
    deck["design"]["masters"][0]["elements"] = json!([rectangle]);
    assert!(execute_request(json!({"op":"validate","capacity_profile":"legacy","deck":deck})).unwrap_err().to_string().contains("scene elements"));
    assert!(execute_request(json!({"op":"validate","deck":deck})).is_ok());
    assert!(aislide_core::preflight::archive_admission("YWI=", 1).is_err());
    assert!(aislide_core::preflight::archive_admission("YQ==", 1).is_ok());
}

#[test]
fn large_inverse_receipts_do_not_copy_unchanged_origin() {
    let document = execute_request(json!({"op":"new_document","id":"receipts","deck":text_deck(128)})).unwrap();
    let exported = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    let document = execute_request(json!({"op":"open_presentation","id":"receipts-native","base64":exported["base64"]})).unwrap()["document"].clone();
    let mut deck = document["deck"].clone();
    for slide in deck["slides"].as_array_mut().unwrap() { slide["title"] = json!("Synthetic changed"); slide["notes"] = json!("Synthetic changed notes"); }
    let result = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck","value":deck}]}})).unwrap();
    assert_eq!(result["receipt"]["inverse"].as_array().unwrap().len(), 1);
    assert_eq!(result["receipt"]["inverse"][0]["path"], "/deck");
    let restored = execute_request(json!({"op":"undo_transaction","document":result["document"],"expected_revision":1,"receipt":result["receipt"]})).unwrap();
    assert_eq!(restored["document"]["hash"], document["hash"]);
    println!("G38 receipt_bytes={} original_document_bytes={} origin_encoded_bytes={}", serde_json::to_vec(&result["receipt"]).unwrap().len(), serde_json::to_vec(&document).unwrap().len(), document["origin"]["base64"].as_str().unwrap().len());
}

#[test]
fn svg_identity_is_part_of_the_immutable_media_key() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let asset = |id: &str, color: &str| execute_request(json!({"op":"create_asset","id":id,"mime_type":"image/svg+xml","alt":"Synthetic","size":32,
        "base64":STANDARD.encode(format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"2\" height=\"2\"><rect width=\"2\" height=\"2\" fill=\"{color}\"/></svg>"))})).unwrap();
    let first = asset("first", "red");
    let mut second = asset("second", "#ff0000");
    second["base64"] = first["base64"].clone();
    let mut third = first.clone(); third["id"] = json!("third"); third["alt"] = json!("Different description");
    let mut deck = text_deck(1); deck["slides"][0]["elements"] = json!([first, second, third]);
    let output = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let package = aislide_core::package::Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(package.parts().keys().filter(|name| name.starts_with("ppt/media/")).count(), 4);
    let relationships = package.text("ppt/slides/_rels/slide1.xml.rels").unwrap();
    assert_eq!(relationships.matches("../media/image1.svg").count(), 2);
    assert_eq!(relationships.matches("../media/image2.svg").count(), 1);
}

#[test]
fn zip64_remains_rejected_before_archive_expansion() {
    let mut archive = vec![0u8; 22];
    archive[..4].copy_from_slice(b"PK\x05\x06");
    archive[8..12].fill(255);
    let error = aislide_core::package::Package::open(archive).unwrap_err();
    assert!(error.to_string().contains("ZIP64"), "{error}");
}

#[test]
fn recovery_session_validates_both_receipt_chains_and_immutable_origin() {
    let document = execute_request(json!({"op":"create_presentation","id":"recovery-chain","title":"Start"})).unwrap();
    let edit = |document: &Value, title: &str| execute_request(json!({"op":"transaction","document":document,"transaction":{
        "expected_revision":document["revision"],"expected_hash":document["hash"],
        "operations":[{"op":"replace","path":"/deck/title","value":title}]
    }})).unwrap();
    let first = edit(&document, "First");
    let second = edit(&first["document"], "Second");
    let undone = execute_request(json!({"op":"undo_transaction","document":second["document"],"expected_revision":2,"receipt":second["receipt"]})).unwrap();
    let envelope = json!({"format":"aislide.session","version":1,"capacity_profile":"large","document":undone["document"],
        "past":[first["receipt"]],"future":[undone["receipt"]],"history_boundary":null});
    assert_eq!(execute_request(json!({"op":"verify_session_recovery","envelope":envelope})).unwrap(), envelope);
    for stack in ["past", "future"] {
        let mut damaged = envelope.clone(); damaged[stack][0]["after_hash"] = json!("0".repeat(64));
        assert!(execute_request(json!({"op":"verify_session_recovery","envelope":damaged})).is_err());
        let mut damaged = envelope.clone(); damaged[stack][0]["inverse"] = json!([{"op":"add","path":"/origin","value":null}]);
        assert!(execute_request(json!({"op":"verify_session_recovery","envelope":damaged})).unwrap_err().to_string().contains("receipt path"));
    }
    let mut oversized = envelope.clone(); oversized["past"] = json!(vec![first["receipt"].clone();31]);
    assert!(execute_request(json!({"op":"verify_session_recovery","envelope":oversized})).unwrap_err().to_string().contains("30"));
    let mut oversized = envelope.clone(); oversized["past"][0]["inverse"][0]["value"] = json!("界".repeat(1_400_000));
    assert!(execute_request(json!({"op":"verify_session_recovery","envelope":oversized})).unwrap_err().to_string().contains("history"));
    let mut large = envelope.clone(); large["document"]["revision"] = json!(9_007_199_254_740_991u64);
    assert!(execute_request(json!({"op":"verify_session_recovery","envelope":large})).is_ok());
}

#[test]
fn standard_profile_checks_intermediate_transactions_before_commit() {
    let document = execute_request(json!({"op":"new_document","capacity_profile":"standard","id":"local-profile","deck":text_deck(128)})).unwrap();
    let error = execute_request(json!({"op":"transaction","capacity_profile":"standard","document":document,"transaction":{
        "expected_revision":0,"expected_hash":document["hash"],"operations":[
            {"op":"add","path":"/deck/slides/-","value":text_deck(1)["slides"][0]},
            {"op":"remove","path":"/deck/slides/128"}
        ]
    }})).unwrap_err();
    assert!(error.to_string().contains("128"), "{error}");
    assert_eq!(execute_request(json!({"op":"verify_recovery","capacity_profile":"standard","document":document})).unwrap(), document);
}

#[test]
fn recovery_policy_requires_v2_consent_and_rejects_stale_generations() {
    let initial = json!({"version":2,"enabled":false,"generation":0,"consent_epoch":0,"entries":[]});
    let prepare = |state: &Value, generation: u64, action: Value, now: u64| execute_request(json!({
        "op":"prepare_recovery","state":state,"expected_generation":generation,"action":action,"now_ms":now
    }));
    let document = execute_request(json!({"op":"create_presentation","id":"stored","title":"Stored"})).unwrap();
    let envelope = json!({"format":"aislide.session","version":1,"capacity_profile":"large","document":document,"past":[],"future":[],"history_boundary":null});
    let save = json!({"op":"save","envelope":envelope,"filename":"Stored.pptx"});
    assert!(prepare(&initial, 0, save.clone(), 1000).is_err());
    let enabled = prepare(&initial, 0, json!({"op":"configure","enabled":true}), 1000).unwrap();
    let saved = prepare(&enabled["state"], 1, save.clone(), 1000).unwrap();
    assert_eq!(saved["state"]["entries"].as_array().unwrap().len(), 1);
    assert!(saved["write"]["payload"].as_str().unwrap().contains("aislide.session"));
    assert!(prepare(&saved["state"], 1, save.clone(), 1001).is_err());
    let removed = prepare(&saved["state"], 2, json!({"op":"remove","id":"stored"}), 1001).unwrap();
    assert!(prepare(&removed["state"], 2, save.clone(), 1002).is_err());
    let disabled = prepare(&saved["state"], 2, json!({"op":"configure","enabled":false}), 1001).unwrap();
    assert_eq!(disabled["state"]["entries"].as_array().unwrap().len(), 1);
    let retained = prepare(&disabled["state"], 3, json!({"op":"open"}), 1000 + 8*86400000).unwrap();
    assert_eq!(retained["state"]["entries"].as_array().unwrap().len(), 1);
    let expired = prepare(&saved["state"], 2, json!({"op":"open"}), 1000 + 8*86400000).unwrap();
    assert!(expired["state"]["entries"].as_array().unwrap().is_empty());
    assert_eq!(expired["deleted"].as_array().unwrap().len(), 1);
    let mut legacy = initial.clone(); legacy["version"] = json!(1);
    assert!(prepare(&legacy, 0, json!({"op":"open"}), 1000).is_err());
    let mut full = saved["state"].clone();
    full["entries"] = json!((0..5).map(|index| {
        let mut entry = saved["state"]["entries"][0].clone();
        entry["id"] = json!(format!("old-{index}"));
        entry["integrity"] = json!(format!("{index:064x}"));
        entry["byte_length"] = json!(20 * 1024 * 1024);
        entry["saved_at"] = json!(1000 + index);
        entry
    }).collect::<Vec<_>>());
    let bounded = prepare(&full, 2, save.clone(), 2000).unwrap();
    assert_eq!(bounded["state"]["entries"].as_array().unwrap().len(), 5);
    assert_eq!(bounded["deleted"][0], format!("{:064x}",0));
    assert!(!bounded["state"]["entries"].as_array().unwrap().iter().any(|entry|entry["id"] == "old-0"));
    full["entries"][0]["byte_length"] = json!(48 * 1024 * 1024 + 1);
    assert!(prepare(&full, 2, save, 2000).is_err());
}