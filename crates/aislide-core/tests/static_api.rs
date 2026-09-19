use aislide_core::execute_request;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

fn document() -> Value {
    execute_request(json!({"op":"create_presentation","id":"static-test","title":"Output test"})).unwrap()
}

#[test]
fn static_api_exports_files_and_metadata_without_mutation() {
    let mut document = document();
    let first = document["deck"]["slides"][0].clone();
    let mut second = first.clone();
    second["id"] = json!("second");
    second["background"] = json!("FF0000");
    second["inherit_background"] = json!(false);
    let mut deck = document["deck"].clone();
    deck["slides"] = json!([first,second]);
    document = execute_request(json!({"op":"new_document","id":"static-test","deck":deck})).unwrap();
    let before = document.clone();
    for (format, mime, magic, count) in [("png", "image/png", &b"\x89PNG"[..], 2),
        ("jpeg", "image/jpeg", &b"\xff\xd8\xff"[..], 2), ("pdf", "application/pdf", &b"%PDF-"[..], 1)] {
        let result = execute_request(json!({"op":"export_static","document":document,
            "options":{"format":format,"page_indices":[1,0]}})).unwrap();
        let files = result["files"].as_array().unwrap();
        assert_eq!(files.len(), count);
        for file in files {
            let bytes = STANDARD.decode(file["base64"].as_str().unwrap()).unwrap();
            assert!(bytes.starts_with(magic));
            assert_eq!(file["mime_type"], mime);
            assert_eq!(file["byte_length"], bytes.len());
            assert_eq!(file["width"], document["deck"]["width"]);
            assert_eq!(file["height"], document["deck"]["height"]);
            if format == "pdf" {
                assert_eq!(lopdf::Document::load_mem(&bytes).unwrap().get_pages().len(), 2);
                assert_eq!(file["page_indices"], json!([1,0]));
                assert_eq!(file["filename"], "report.pdf");
            }
        }
        if format != "pdf" {
            assert_eq!(files[0]["page_indices"], json!([1]));
            assert_eq!(files[1]["page_indices"], json!([0]));
            assert!(files[0]["filename"].as_str().unwrap().contains("002"));
            let bytes = STANDARD.decode(files[0]["base64"].as_str().unwrap()).unwrap();
            let pixels = image::load_from_memory(&bytes).unwrap().into_rgb8();
            assert!(pixels.get_pixel(0, 0)[0] > 250);
            assert!(pixels.get_pixel(0, 0)[1] < 5);
        }
        assert_eq!(result["office_parity_verified"], false);
        assert_eq!(result["pdf_rasterized"], false);
        assert_eq!(result["pdf_text_outlined"], format == "pdf");
        assert_eq!(result["pdf_tagged"], format == "pdf");
        assert_eq!(result["pdf_searchable_text"], format == "pdf");
        assert_eq!(result["pdf_selectable_text"], format == "pdf");
        assert_eq!(result["pdf_semantic_overlay"], format == "pdf");
        assert_eq!(result["pdf_editable_text"], false);
        assert_eq!(result["pdf_ua_certified"], false);
        assert!(result["warnings"].is_array());
    }
    assert_eq!(document, before);
}

#[test]
fn static_api_and_recovery_reject_untrusted_content_and_options() {
    let document = document();
    assert_eq!(execute_request(json!({"op":"verify_recovery","document":document})).unwrap(), document);
    for options in [json!({"format":"svg"}), json!({"path":"outside"}), json!({"page_indices":[]}),
        json!({"page_indices":[0,0]}), json!({"scale":16}), json!({"max_output_bytes":1}),
        json!({"max_output_bytes":33554433})] {
        assert!(execute_request(json!({"op":"export_static","document":document,"options":options})).is_err());
    }
    for field in ["hash", "shape", "origin"] {
        let mut tampered = document.clone();
        match field {
            "hash" => tampered["hash"] = json!("0".repeat(64)),
            "shape" => tampered["deck"]["slides"][0]["elements"] = json!([{"type":"script"}]),
            _ => tampered["origin"] = json!({"base64":"0M8R4KGxGuE=","sha256":"0".repeat(64),"native":true}),
        }
        for op in ["verify_recovery", "export_static"] {
            let mut request = json!({"op":op,"document":tampered});
            if op == "export_static" { request["options"] = json!({}); }
            assert!(execute_request(request).is_err());
        }
    }
    assert!(execute_request(json!({"op":"verify_recovery","document":document,"trust_hash":true})).is_err());
    assert!(execute_request(json!({"op":"export_static","document":document,"options":{},"path":"outside"})).is_err());
    assert!(execute_request(json!({"op":"verify_recovery","document":"x".repeat(4*1024*1024)})).is_err());
    let schema = serde_json::to_value(schemars::schema_for!(aislide_core::export_static::ExportOptions)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
}

#[test]
fn static_api_rejects_a_valid_bundle_exceeding_the_json_response_budget() {
    let mut seed = 17u32;
    let pixels = image::RgbImage::from_fn(512, 512, |_column, _row| {
        let mut channels = [0u8; 3];
        for channel in &mut channels {
            seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5;
            *channel = seed as u8;
        }
        image::Rgb(channels)
    });
    let mut image_bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut image_bytes, 45).encode_image(&pixels).unwrap();
    let base64 = STANDARD.encode(image_bytes);
    let slides: Vec<Value> = (0..5).map(|index| json!({"id":format!("page-{index}"),"title":"Noise","background":"FFFFFF","notes":"",
        "elements":[{"type":"picture","id":format!("image-{index}"),"x":0,"y":0,"width":512,"height":512,"base64":base64,"mime_type":"image/jpeg","alt":"Synthetic noise","crop":{}}]})).collect();
    let document = execute_request(json!({"op":"new_document","id":"large-bundle","deck":{"version":1,"title":"Noise","width":512,"height":512,"slides":slides}})).unwrap();
    let deck = serde_json::from_value(document["deck"].clone()).unwrap();
    let raw = aislide_core::export_static::export_static(&deck, &Default::default()).unwrap();
    assert!(raw.artifacts.iter().map(|file| file.bytes.len()).sum::<usize>() > aislide_core::limits::LEGACY.request_bytes * 3 / 4);
    let failure = execute_request(json!({"op":"export_static","document":document,"options":{"format":"png"}})).unwrap_err();
    assert!(failure.to_string().contains("4 MiB JSON protocol"), "{failure}");
    assert_eq!(execute_request(json!({"op":"verify_recovery","document":document})).unwrap(), document);
}

#[test]
fn static_api_selects_any_32_pages_from_128_without_modifying_document() {
    let mut deck = json!({"version":1,"title":"Static selection","width":320,"height":320,"slides":[]});
    deck["width"] = json!(320);
    deck["height"] = json!(320);
    deck["slides"] = Value::Array((0..128).map(|index| json!({"id":format!("page-{index}"),"title":"Selected","background":"FFFFFF","notes":"","elements":[]})).collect());
    let document = execute_request(json!({"op":"new_document","id":"static-128","deck":deck})).unwrap();
    for pages in [vec![127,64,0], (96..128).rev().collect()] {
        let output = execute_request(json!({"op":"export_static","document":document,"options":{"format":"pdf","page_indices":pages}})).unwrap();
        assert_eq!(output["files"][0]["page_indices"], json!(pages));
        let bytes = STANDARD.decode(output["files"][0]["base64"].as_str().unwrap()).unwrap();
        assert_eq!(lopdf::Document::load_mem(&bytes).unwrap().get_pages().len(), pages.len());
    }
    for options in [json!({"format":"pdf"}), json!({"format":"pdf","page_indices":(0..33).collect::<Vec<_>>()})] {
        let error = execute_request(json!({"op":"export_static","document":document,"options":options})).unwrap_err();
        assert!(error.to_string().contains("1-32 pages"), "{error}");
    }
    assert_eq!(execute_request(json!({"op":"verify_recovery","document":document})).unwrap(), document);
}