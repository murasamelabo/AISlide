use aislide_core::protocol::execute_request;
use serde_json::json;
use sha2::{Digest, Sha256};

#[test]
fn local_text_assist_request_is_explicit_and_strict() {
    let request = json!({"op":"text_assist","input":{"task":"proofread","text":"This are a test.","language":"en","target_language":null}});
    let result = execute_request(request.clone());
    assert!(!result.err().is_some_and(|error| error.to_string().contains("unknown variant `text_assist`")), "text_assist must be a supported, stateless operation");
    for (key, value) in [("kind", json!("html")), ("allow_remote", json!(true)), ("path", json!("model.gguf"))] {
        let mut invalid = request.clone();
        invalid["input"][key] = value;
        assert!(execute_request(invalid).unwrap_err().to_string().contains("unknown field"));
    }
    let mut invalid = request;
    invalid["input"]["task"] = json!("execute");
    assert!(execute_request(invalid).unwrap_err().to_string().contains("unknown variant `execute`"));
}

#[test]
fn local_segmentation_status_is_sanitized_and_requests_cannot_select_paths() {
    let status = execute_request(json!({"op":"segmentation_status"})).unwrap();
    assert_eq!(status["model"], "u2netp");
    assert_eq!(status["remote"], false);
    assert!(status.get("path").is_none());
    for key in ["path", "model", "url", "runtime"] {
        let mut request = json!({"op":"segment_image","base64":"","mime_type":"image/png"});
        request[key] = json!("untrusted");
        assert!(execute_request(request).unwrap_err().to_string().contains("unknown field"));
    }
}

fn text_document() -> serde_json::Value {
    execute_request(json!({"op":"new_document","id":"ai-edit","deck":{"version":1,"title":"AI fixture","width":1280,"height":720,"slides":[{"id":"slide","title":"Test","background":"FFFFFF","notes":"","elements":[
        {"type":"text","id":"text","x":40,"y":40,"width":900,"height":400,"font_size":24,"color":"222222","bold":false,"text":"This are a test.\nSecond paragraph.","format":{"paragraphs":[
            {"alignment":"center","runs":[{"text":"This are a test.","style":{"bold":true}}]},
            {"space_after":{"kind":"points","value":800},"runs":[{"text":"Second paragraph.","style":{"italic":true}}]}]}}
    ]}]}})).unwrap()
}

#[test]
fn text_assist_apply_is_guarded_atomic_and_preserves_paragraph_styles() {
    let document = text_document();
    let before = &document["deck"]["slides"][0]["elements"][0];
    let expected = before["text"].as_str().unwrap();
    let candidate = json!({"text":"This is a test.\nSecond revised paragraph.","source_sha256":format!("{:x}",Sha256::digest(expected.as_bytes()))});
    let request = json!({"op":"apply_text_assist","document":document,"expected_revision":0,"slide_id":"slide","id":"text","expected_text":expected,"candidate":candidate});
    let result = execute_request(request.clone()).unwrap();
    let after = &result["document"]["deck"]["slides"][0]["elements"][0];
    assert_eq!(after["text"], candidate["text"]);
    assert_eq!(after["format"]["paragraphs"][0]["alignment"],"center");
    assert_eq!(after["format"]["paragraphs"][1]["space_after"],before["format"]["paragraphs"][1]["space_after"]);
    assert_eq!(after["format"]["paragraphs"][1]["runs"][0]["style"]["italic"],true);
    let undo = execute_request(json!({"op":"undo_transaction","document":result["document"],"expected_revision":1,"receipt":result["receipt"]})).unwrap();
    assert_eq!(undo["document"]["hash"], document["hash"]);
    for (key, value) in [("expected_revision",json!(1)),("expected_text",json!("stale"))] {
        let mut stale = request.clone(); stale[key] = value; assert!(execute_request(stale).is_err());
    }
    let mut stale = request.clone(); stale["candidate"]["source_sha256"] = json!("0".repeat(64)); assert!(execute_request(stale).is_err());
    let mut injected = request; injected["candidate"]["kind"] = json!("html"); assert!(execute_request(injected).is_err());
}

#[test]
fn text_assist_never_overwrites_fields_or_locked_text() {
    for variant in ["field", "locked"] {
        let mut deck = text_document()["deck"].clone();
        if variant == "field" { deck["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"][0]["field"] = json!({"id":"9a65ddcd-8d65-48c2-9d25-a6e59e50dc67","kind":"vendor"}); }
        else { deck["slides"][0]["elements"][0]["visual"] = json!({"locked":true}); }
        let document = execute_request(json!({"op":"new_document","id":"guard","deck":deck})).unwrap();
        let expected = document["deck"]["slides"][0]["elements"][0]["text"].as_str().unwrap();
        let candidate = json!({"text":"Corrected text.\nParagraph.","source_sha256":format!("{:x}",Sha256::digest(expected.as_bytes()))});
        assert!(execute_request(json!({"op":"apply_text_assist","document":document,"expected_revision":0,"slide_id":"slide","id":"text","expected_text":expected,"candidate":candidate})).is_err());
    }
}

#[test]
#[ignore = "requires explicit local U2NetP setup and dataset terms acknowledgement"]
fn real_u2netp_segments_a_synthetic_still_life() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use aislide_core::{generation::CancellationToken, segmentation};
    let mut pixels = image::RgbaImage::new(320,320);
    for (column,row,pixel) in pixels.enumerate_pixels_mut() {
        let background = 210 + ((column + row) % 29) as u8;
        let center_distance = (column as i32 - 160).abs();
        let vase = (65..265).contains(&row) && center_distance < (44 + (row as i32 - 65) / 5);
        let rim = (row as i32 - 66).pow(2) * 8 + (column as i32 - 160).pow(2) < 45 * 45;
        *pixel = if vase || rim { image::Rgba([180 + (column % 35) as u8,45 + (row % 15) as u8,35,255]) }
            else { image::Rgba([background,background,background.saturating_sub(10),255]) };
    }
    let mut source = std::io::Cursor::new(Vec::new());
    pixels.write_to(&mut source,image::ImageFormat::Png).unwrap();
    let result = segmentation::segment(&STANDARD.encode(source.get_ref()),"image/png",CancellationToken::new()).unwrap();
    let output = image::load_from_memory(&STANDARD.decode(&result.image.base64).unwrap()).unwrap().into_rgba8();
    let foreground = output.get_pixel(160,160)[3];
    let background = output.get_pixel(10,10)[3];
    println!("REAL_U2NETP {}",json!({"foreground_alpha":foreground,"background_alpha":background,"provenance":result.provenance}));
    assert!(foreground > 200, "foreground alpha {foreground}");
    assert!(background < 64,"background alpha {background}");
    assert!(output.pixels().any(|pixel| pixel[3] > 0 && pixel[3] < 255));
    assert_eq!(result.image.width,320);
    assert_eq!(result.image.mime_type,"image/png");
}