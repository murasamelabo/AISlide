use aislide_core::execute_request;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

fn document() -> Value {
    execute_request(json!({"op":"create_presentation","id":"static-test","title":"Output test"})).unwrap()
}

#[test]
fn delivery_preparation_is_read_only_and_hashes_requested_artifacts() {
    use sha2::{Digest, Sha256};
    let capabilities=execute_request(json!({"op":"authoring_capabilities"})).unwrap();
    assert!(capabilities["operations"].as_array().unwrap().contains(&json!("prepare_delivery")));
    assert_eq!(capabilities["delivery"]["output_bytes"],32*1024*1024);
    assert_eq!(capabilities["delivery"]["notes_opt_in"],true);
    let mut deck=document()["deck"].clone();
    deck["slides"][0]["notes"]=json!("Explicit synthetic speaker notes.");
    let document=execute_request(json!({"op":"new_document","id":"delivery-core","deck":deck})).unwrap();
    let result=execute_request(json!({"op":"prepare_delivery","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"options":{"pdf":true,"preview":"contact_sheet","notes":true,"source_report":true,"preflight":true,"max_dimension":320}})).unwrap();
    assert_eq!(result["manifest"]["format"],"aislide.delivery");
    assert_eq!(result["manifest"]["document"]["hash"],document["hash"]);
    assert_eq!(result["manifest"]["checks"]["office_visual_parity"],false);
    assert_eq!(result["manifest"]["checks"]["source_authenticity_verified"],false);
    assert_eq!(result["manifest"]["checks"]["preflight_page_indices"],json!([0]));
    let files=result["files"].as_array().unwrap();assert_eq!(files.len(),5);
    for file in files {
        let bytes=STANDARD.decode(file["base64"].as_str().unwrap()).unwrap();
        assert_eq!(file["byte_length"],bytes.len());
        assert_eq!(file["sha256"],format!("{:x}",Sha256::digest(&bytes)));
        assert_eq!(file["page_indices"],json!([0]));
        match file["kind"].as_str().unwrap() {
            "pptx"=>assert_eq!(file["base64"],execute_request(json!({"op":"export_presentation","document":document})).unwrap()["base64"]),
            "pdf"=>assert_eq!(lopdf::Document::load_mem(&bytes).unwrap().get_pages().len(),1),
            "preview"=>assert!(image::load_from_memory(&bytes).is_ok()),
            "notes"=>{let text=std::str::from_utf8(&bytes).unwrap();assert!(text.starts_with('\u{feff}'));assert!(text.contains("Explicit synthetic speaker notes."));},
            "source_report"=>{let report:Value=serde_json::from_slice(&bytes).unwrap();assert_eq!(report["sources"],json!([]));assert_eq!(report["source_authenticity_verified"],false);},
            other=>panic!("unexpected artifact {other}"),
        }
    }
    assert_eq!(execute_request(json!({"op":"verify_recovery","document":document})).unwrap(),document);
}

#[test]
fn delivery_preparation_bounds_selection_and_rejects_stale_or_unknown_options() {
    let mut deck=document()["deck"].clone();
    let first=deck["slides"][0].clone();
    deck["slides"]=json!((0..9).map(|index|{let mut slide=first.clone();slide["id"]=json!(format!("page-{index}"));slide}).collect::<Vec<_>>());
    let document=execute_request(json!({"op":"new_document","id":"delivery-selection","deck":deck})).unwrap();
    let request=json!({"op":"prepare_delivery","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"options":{"page_indices":[8,0],"preview":"pages","max_dimension":320}});
    let result=execute_request(request.clone()).unwrap();
    assert_eq!(result["files"][0]["page_indices"],json!((0..9).collect::<Vec<_>>()));
    assert_eq!(result["files"][1]["page_indices"],json!([8]));
    assert_eq!(result["files"][2]["page_indices"],json!([0]));
    assert!(!result["files"].as_array().unwrap().iter().any(|file|file["kind"]=="notes" || file["kind"]=="source_report"));
    for options in [json!({}),json!({"page_indices":[]}),json!({"page_indices":[0,0]}),json!({"page_indices":[9]}),json!({"page_indices":[0],"max_output_bytes":1}),json!({"page_indices":[0],"max_output_bytes":33554433}),json!({"page_indices":[0],"path":"outside"})] {
        let mut invalid=request.clone();invalid["options"]=options;
        assert!(execute_request(invalid).is_err());
    }
    for (field,value) in [("expected_revision",json!(1)),("expected_hash",json!("0".repeat(64)))] {
        let mut invalid=request.clone();invalid[field]=value;assert!(execute_request(invalid).is_err());
    }
    let mut pptx_only=request;
    pptx_only["options"]=json!({"preview":"none","preflight":false});
    let output=execute_request(pptx_only).unwrap();
    assert_eq!(output["files"].as_array().unwrap().len(),1);
    assert_eq!(output["manifest"]["checks"]["preflight_page_indices"],json!([]));
}

#[test]
fn delivery_preparation_reports_source_metadata_without_raw_data_and_rejects_stale_bindings() {
    let sentinel="private-fixture-cell-not-for-source-report";
    let source=execute_request(json!({"op":"ingest","input":{"name":"supplied.csv","format":"csv","base64":STANDARD.encode(format!("Label\n{sentinel}\n"))}})).unwrap();
    let mut deck=document()["deck"].clone();
    deck["slides"][0]["elements"]=json!([{"type":"text","id":"bound-text","x":40,"y":50,"width":1000,"height":80,"text":sentinel,"font_size":24,"color":"000000","bold":false}]);
    let binding=json!({"slide_id":"slide-1","element_id":"bound-text","field":"/text","source_id":source["id"],"source_sha256":source["sha256"],"locator":source["tables"][0]["locators"][0][0],"value":sentinel,"raw_value":sentinel,"transform":"display_scalar"});
    let document=execute_request(json!({"op":"new_document","id":"delivery-private-source","deck":deck,"sources":[source],"bindings":[binding]})).unwrap();
    let options=json!({"preview":"none","preflight":false,"source_report":true});
    let prepared=execute_request(json!({"op":"prepare_delivery","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"options":options})).unwrap();
    let report=prepared["files"].as_array().unwrap().iter().find(|file|file["kind"]=="source_report").unwrap();
    let bytes=STANDARD.decode(report["base64"].as_str().unwrap()).unwrap();
    assert!(!std::str::from_utf8(&bytes).unwrap().contains(sentinel));
    let data:Value=serde_json::from_slice(&bytes).unwrap();
    assert_eq!(data["sources"][0]["sha256"],source["sha256"]);
    assert!(data["sources"][0].get("tables").is_none());
    assert!(data["bindings"][0].get("value").is_none());
    assert!(data["bindings"][0].get("raw_value").is_none());
    assert_eq!(prepared["manifest"]["checks"]["source_authenticity_verified"],false);
    let stale=execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/slides/0/elements/0/text","value":"manually changed"}]}})).unwrap()["document"].clone();
    let error=execute_request(json!({"op":"prepare_delivery","document":stale,"expected_revision":stale["revision"],"expected_hash":stale["hash"],"options":options})).unwrap_err();
    assert!(error.to_string().contains("stale"),"{error}");
}

#[test]
fn visual_authoring_capabilities_publish_bounded_shared_contracts() {
    let capabilities=execute_request(json!({"op":"authoring_capabilities"})).unwrap();
    for operation in ["preview_presentation","preflight_presentation","preview_slide_revision","apply_slide_revision"] {
        assert!(capabilities["operations"].as_array().unwrap().contains(&json!(operation)));
    }
    assert_eq!(capabilities["visual_authoring"]["preview_pages"],8);
    assert_eq!(capabilities["visual_authoring"]["preview_max_dimension"],1600);
    assert_eq!(capabilities["visual_authoring"]["revision_edits"],16);
    assert!(capabilities["visual_authoring"]["revision_edit_schema"].is_object());
}

#[test]
fn chart_parity_notices_are_summarized_without_hiding_specific_preview_limits() {
    let mut scene = document()["deck"].clone();
    scene["slides"][0]["elements"] = json!((0..3).map(|index| json!({
        "type":"chart","id":format!("chart-{index}"),"x":24+index*400,"y":100,"width":380,"height":280,"kind":"line",
        "categories":["One","Two","Three"],"series":[{"name":"Synthetic","values":[1,2,3],"color":"1976D2"}],
        "options":{"primary_axis":{"number_format":"invalid-format"}}
    })).collect::<Vec<_>>());
    let state = execute_request(json!({"op":"new_document","id":"chart-notice-summary","deck":scene})).unwrap();
    let report = execute_request(json!({"op":"preflight_presentation","document":state})).unwrap();
    let findings = report["findings"].as_array().unwrap();
    let notices: Vec<_> = findings.iter().filter(|finding| finding["code"] == "CHART_PREVIEW").collect();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0]["severity"], "info");
    assert_eq!(notices[0]["element_ids"].as_array().unwrap().len(), 3);
    assert!(notices[0]["message"].as_str().unwrap().contains("not Office"));
    assert!(findings.iter().any(|finding| finding["code"] == "CHART_PRESENTATION" && finding["severity"] == "warning" && finding["message"].as_str().unwrap().contains("Unsupported number format")));
    assert_eq!(report["office_visual_parity"], false);
}

#[test]
fn authoring_preflight_reports_renderer_clipping_and_connector_label_interference() {
    let elements = json!([
        {"type":"rect","id":"background","x":0,"y":0,"width":640,"height":360,"fill":"FFFFFF"},
        {"type":"connector","id":"route","x":100,"y":180,"width":300,"height":1,"color":"000000","stroke_width":2,"arrow":true},
        {"type":"shape","id":"label","preset":"rect","x":180,"y":166,"width":90,"height":28,"fill":"FFFFFF","stroke":"FFFFFF","stroke_width":0,"text":"route","font_size":16,"color":"000000","bold":false},
        {"type":"shape","id":"clipped","preset":"rect","x":30,"y":30,"width":180,"height":20,"fill":"FFFFFF","stroke":"FFFFFF","stroke_width":0,"text":"Clipped","font_size":16,"color":"000000","bold":false},
        {"type":"text","id":"small","x":30,"y":260,"width":200,"height":40,"text":"Too small for projection","font_size":12,"color":"000000","bold":false}
    ]);
    let deck = json!({"version":1,"title":"Diagnostic fixture","width":640,"height":360,"slides":[{"id":"slide-1","title":"Fixture","background":"FFFFFF","notes":"","elements":elements}]});
    let document = execute_request(json!({"op":"new_document","id":"preflight-test","deck":deck})).unwrap();
    let result = execute_request(json!({"op":"preflight_presentation","document":document,"options":{"min_font_size":24}})).unwrap();
    let findings = result["findings"].as_array().unwrap();
    for (code, id) in [("TEXT_OVERFLOW","clipped"),("CONNECTOR_LABEL_INTERFERENCE","label"),("SMALL_TEXT","small")] {
        let finding = findings.iter().find(|finding| finding["code"] == code && finding["element_ids"].as_array().unwrap().contains(&json!(id))).expect(code);
        assert_eq!(finding["slide_id"], "slide-1");
        assert!(finding["bounds"]["width"].as_f64().unwrap() > 0.0);
        assert!(!finding["suggestions"].as_array().unwrap().is_empty());
    }
    assert!(!findings.iter().any(|finding| finding["code"] == "TEXT_OVERLAP" && finding["element_ids"].as_array().unwrap().contains(&json!("background"))));
    assert_eq!(result["office_visual_parity"], false);
    assert_eq!(result["semantic_truth_verified"], false);
    assert_eq!(execute_request(json!({"op":"verify_recovery","document":document})).unwrap(), document);
}

#[test]
fn authoring_preflight_checks_rounded_container_corners_and_safe_padding() {
    let mut deck = document()["deck"].clone();
    deck["slides"][0]["elements"] = json!([
        {"type":"shape","id":"card","preset":"roundRect","x":40,"y":40,"width":400,"height":240,"fill":"FFFFFF","stroke":"087F73","stroke_width":2,"text":"","font_size":16,"color":"000000","bold":false},
        {"type":"rect","id":"accent","x":40,"y":40,"width":8,"height":240,"fill":"087F73"},
        {"type":"text","id":"corner-title","x":46,"y":46,"width":220,"height":32,"text":"Corner label","font_size":16,"color":"000000","bold":false},
        {"type":"text","id":"tight-curve","x":52,"y":52,"width":220,"height":32,"text":"Too close to the curve","font_size":16,"color":"000000","bold":false},
        {"type":"text","id":"tight-padding","x":42,"y":140,"width":200,"height":32,"text":"Too close to the edge","font_size":16,"color":"000000","bold":false},
        {"type":"text","id":"safe-title","x":92,"y":90,"width":260,"height":32,"text":"Safe title","font_size":16,"color":"000000","bold":false},
        {"type":"polygon","id":"outline-accent","x":40,"y":40,"width":8,"height":240,"fill":"none","stroke":"087F73","stroke_width":2,"points":[[0,0],[1,0],[1,1],[0,1]]},
        {"type":"text","id":"outside","x":500,"y":46,"width":160,"height":32,"text":"Separate label","font_size":16,"color":"000000","bold":false}
    ]);
    let document = execute_request(json!({"op":"new_document","id":"container-clearance","deck":deck})).unwrap();
    let result = execute_request(json!({"op":"preflight_presentation","document":document})).unwrap();
    let findings = result["findings"].as_array().unwrap();
    for (code, id) in [("CONTAINER_CORNER_OVERFLOW", "accent"), ("CONTAINER_CORNER_OVERFLOW", "outline-accent"), ("CONTAINER_CORNER_OVERFLOW", "corner-title"), ("CONTAINER_PADDING", "tight-padding"), ("CONTAINER_PADDING", "tight-curve")] {
        let finding = findings.iter().find(|finding| finding["code"] == code && finding["element_ids"] == json!([id, "card"])).expect(id);
        assert_eq!(finding["severity"], "warning");
        assert_eq!(finding["evidence"], "heuristic");
        assert!(!finding["suggestions"].as_array().unwrap().is_empty());
    }
    assert!(!findings.iter().any(|finding| finding["code"].as_str().unwrap().starts_with("CONTAINER_") && finding["element_ids"].as_array().unwrap().iter().any(|id| id == "safe-title" || id == "outside")));
    assert_eq!(execute_request(json!({"op":"verify_recovery","document":document})).unwrap(), document);
}

#[test]
fn authoring_preflight_container_clearance_respects_transforms_visibility_and_nearest_frame() {
    for (case, scale, rotation, gap, hidden, transparent, expected) in [
        ("scaled-clear", 2.0, 0.0, 6.0, false, false, false),
        ("scaled-tight", 2.0, 0.0, 2.0, false, false, true),
        ("rotated-tight", 1.0, 30.0, 2.0, false, false, true),
        ("hidden", 1.0, 0.0, 2.0, true, false, false),
        ("transparent", 1.0, 0.0, 2.0, false, true, false),
    ] {
        let mut card = json!({"type":"shape","id":"card","preset":"roundRect","x":0,"y":0,"width":300,"height":200,"fill":"FFFFFF","stroke":"000000","stroke_width":0,"text":"","font_size":16,"color":"000000","bold":false});
        if hidden { card["visual"] = json!({"hidden":true}); }
        if transparent { card["visual"] = json!({"gradient":{"kind":"linear","angle":0,"stops":[{"offset":0,"color":"FFFFFF","opacity":0},{"offset":1,"color":"FFFFFF","opacity":0}]}}); }
        let label = json!({"type":"text","id":"label","x":gap,"y":80,"width":180,"height":32,"text":"Readable label","font_size":16,"color":"000000","bold":false});
        let mut deck = document()["deck"].clone();
        deck["slides"][0]["elements"] = json!([{"type":"group","id":"group","x":100,"y":100,"width":300.0 * scale,"height":200.0 * scale,"view_width":300,"view_height":200,"visual":{"rotation":rotation,"flip_h":true},"children":[card,label]}]);
        let document = execute_request(json!({"op":"new_document","id":format!("container-{case}"),"deck":deck})).unwrap();
        let result = execute_request(json!({"op":"preflight_presentation","document":document})).unwrap();
        let findings: Vec<_> = result["findings"].as_array().unwrap().iter().filter(|finding| finding["code"].as_str().unwrap().starts_with("CONTAINER_")).collect();
        assert_eq!(findings.len(), usize::from(expected), "{case}: {result}");
        if expected { assert_eq!(findings[0]["code"], "CONTAINER_PADDING"); }
    }
    let mut deck = document()["deck"].clone();
    let outer = json!({"type":"shape","id":"outer","preset":"roundRect","x":40,"y":40,"width":500,"height":400,"fill":"FFFFFF","stroke":"000000","stroke_width":1,"text":"","font_size":16,"color":"000000","bold":false});
    let inner = json!({"type":"shape","id":"inner","preset":"roundRect","x":100,"y":100,"width":300,"height":200,"fill":"FFFFFF","stroke":"000000","stroke_width":1,"text":"","font_size":16,"color":"000000","bold":false});
    deck["slides"][0]["elements"] = json!([outer,inner,{"type":"text","id":"label","x":102,"y":180,"width":180,"height":32,"text":"Inner label","font_size":16,"color":"000000","bold":false}]);
    let document = execute_request(json!({"op":"new_document","id":"nearest-container","deck":deck})).unwrap();
    let result = execute_request(json!({"op":"preflight_presentation","document":document})).unwrap();
    let finding = result["findings"].as_array().unwrap().iter().find(|finding| finding["code"] == "CONTAINER_PADDING").unwrap();
    assert_eq!(finding["element_ids"], json!(["label","inner"]));
}

#[test]
fn authoring_preflight_measures_perpendicular_clearance_after_nested_anisotropic_scaling() {
    let mut deck = document()["deck"].clone();
    deck["slides"][0]["elements"] = json!([{
        "type":"group","id":"scaled","x":100,"y":100,"width":800,"height":400,"view_width":400,"view_height":400,"children":[{
            "type":"group","id":"rotated","x":50,"y":50,"width":300,"height":200,"view_width":300,"view_height":200,"visual":{"rotation":45},"children":[
                {"type":"shape","id":"card","preset":"roundRect","x":0,"y":0,"width":300,"height":200,"fill":"FFFFFF","stroke":"000000","stroke_width":1,"text":"","font_size":16,"color":"000000","bold":false},
                {"type":"text","id":"label","x":6,"y":80,"width":180,"height":32,"text":"Perpendicular clearance","font_size":16,"color":"000000","bold":false}
            ]
        }]
    }]);
    let document = execute_request(json!({"op":"new_document","id":"skew-clearance","deck":deck})).unwrap();
    let result = execute_request(json!({"op":"preflight_presentation","document":document})).unwrap();
    assert!(result["findings"].as_array().unwrap().iter().any(|finding| finding["code"] == "CONTAINER_PADDING" && finding["element_ids"] == json!(["label","card"])), "{result}");
}

#[test]
fn authoring_preflight_distinguishes_numbered_badges_without_hiding_real_label_interference() {
    for (case, text, fill, opacity, foreground_route, vertical_offset, expected) in [
        ("badge", "2", "087F73", 1.0, false, 0.0, "CONNECTOR_BADGE_OVERLAP"),
        ("transparent", "2", "087F73", 0.5, false, 0.0, "CONNECTOR_LABEL_INTERFERENCE"),
        ("unfilled", "2", "none", 1.0, false, 0.0, "CONNECTOR_LABEL_INTERFERENCE"),
        ("label", "X", "087F73", 1.0, false, 0.0, "CONNECTOR_LABEL_INTERFERENCE"),
        ("foreground", "2", "087F73", 1.0, true, 0.0, "CONNECTOR_LABEL_INTERFERENCE"),
        ("grazing", "2", "087F73", 1.0, false, 12.0, "CONNECTOR_LABEL_INTERFERENCE"),
        ("soft-edge", "2", "087F73", 1.0, false, 0.0, "CONNECTOR_LABEL_INTERFERENCE"),
        ("ancestor-soft-edge", "2", "087F73", 1.0, false, 0.0, "CONNECTOR_LABEL_INTERFERENCE"),
    ] {
        let route = json!({"type":"connector","id":"route","x":100,"y":216.0 + vertical_offset,"width":220,"height":0.01,"color":"000000","stroke_width":2,"arrow":true});
        let mut badge = json!({"type":"shape","id":"badge","preset":"ellipse","x":200,"y":200,"width":32,"height":32,"fill":fill,"stroke":"087F73","stroke_width":1,"text":text,"font_size":12,"color":"FFFFFF","bold":true});
        if opacity < 1.0 { badge["visual"] = json!({"opacity":opacity}); }
        if case == "soft-edge" { badge["visual"] = json!({"soft_edge":10}); }
        let mut deck = document()["deck"].clone();
        deck["slides"][0]["elements"] = if foreground_route { json!([badge, route]) } else { json!([route, badge]) };
        if case == "ancestor-soft-edge" {
            let badge = deck["slides"][0]["elements"].as_array_mut().unwrap().pop().unwrap();
            deck["slides"][0]["elements"].as_array_mut().unwrap().push(json!({"type":"group","id":"blurred","x":0,"y":0,"width":400,"height":300,"view_width":400,"view_height":300,"visual":{"soft_edge":10},"children":[badge]}));
        }
        let document = execute_request(json!({"op":"new_document","id":format!("badge-{case}"),"deck":deck})).unwrap();
        let result = execute_request(json!({"op":"preflight_presentation","document":document})).unwrap();
        let findings: Vec<_> = result["findings"].as_array().unwrap().iter().filter(|finding| finding["code"].as_str().unwrap().starts_with("CONNECTOR_")).collect();
        assert_eq!(findings.len(), 1, "{case}: {result}");
        assert_eq!(findings[0]["code"], expected, "{case}");
        assert_eq!(findings[0]["severity"], if case == "badge" { "info" } else { "warning" });
        assert_eq!(findings[0]["element_ids"], json!(["badge", "route"]));
        assert_eq!(execute_request(json!({"op":"verify_recovery","document":document})).unwrap(), document);
    }
}

#[test]
fn authoring_preflight_uses_group_transforms_and_rejects_unbounded_requests() {
    let label = json!({"type":"text","id":"nested","x":20,"y":20,"width":100,"height":40,"text":"Label","font_size":12,"color":"000000","bold":false});
    let deck = json!({"version":1,"title":"Groups","width":640,"height":360,"slides":[{"id":"slide-1","title":"Groups","background":"FFFFFF","notes":"","elements":[
        {"type":"group","id":"group","x":100,"y":100,"width":300,"height":200,"view_width":150,"view_height":100,"children":[label]},
        {"type":"text","id":"peer","x":160,"y":150,"width":140,"height":50,"text":"Overlap","font_size":24,"color":"000000","bold":false}
    ]}]});
    let document = execute_request(json!({"op":"new_document","id":"group-test","deck":deck})).unwrap();
    let result = execute_request(json!({"op":"preflight_presentation","document":document,"options":{"min_font_size":20}})).unwrap();
    let findings = result["findings"].as_array().unwrap();
    let overlap = findings.iter().find(|finding| finding["code"] == "TEXT_OVERLAP").unwrap();
    assert!(overlap["element_ids"].as_array().unwrap().contains(&json!("nested")));
    assert!(!findings.iter().any(|finding| finding["code"] == "SMALL_TEXT" && finding["element_ids"].as_array().unwrap().contains(&json!("nested"))));
    for options in [json!({"page_indices":[]}),json!({"page_indices":[0,0]}),json!({"page_indices":[1]}),json!({"min_font_size":0}),json!({"fetch_fonts":true})] {
        assert!(execute_request(json!({"op":"preflight_presentation","document":document,"options":options})).is_err());
    }
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
fn authoring_preview_handles_minimum_contact_sheets_and_strict_output_budgets() {
    let slides: Vec<Value> = (0..8).map(|index| json!({"id":format!("page-{index}"),"title":"Preview","background":"FFFFFF","notes":"","elements":[]})).collect();
    let document = execute_request(json!({"op":"new_document","id":"preview-bounds","deck":{"version":1,"title":"Preview","width":4096,"height":4096,"slides":slides}})).unwrap();
    for (layout, count) in [("contact_sheet", 1), ("pages", 8)] {
        let result = execute_request(json!({"op":"preview_presentation","document":document,"options":{"layout":layout,"max_dimension":160}})).unwrap();
        assert_eq!(result["images"].as_array().unwrap().len(), count);
        for image in result["images"].as_array().unwrap() {
            let bytes = STANDARD.decode(image["base64"].as_str().unwrap()).unwrap();
            let pixels = image::load_from_memory(&bytes).unwrap();
            assert!(pixels.width() <= 160 && pixels.height() <= 160);
            assert_eq!(image["byte_length"], bytes.len());
        }
    }
    for options in [json!({"max_output_bytes":1}),json!({"max_output_bytes":2097153}),json!({"max_dimension":159}),json!({"max_dimension":1601}),json!({"page_indices":[0,0]}),json!({"path":"outside"})] {
        assert!(execute_request(json!({"op":"preview_presentation","document":document,"options":options})).is_err());
    }
}

#[test]
fn authoring_preflight_matches_fill_opacity_visibility_and_connected_nodes() {
    let deck = json!({"version":1,"title":"Visibility","width":360,"height":640,"slides":[{"id":"portrait","title":"Visibility","background":"FFFFFF","notes":"","elements":[
        {"type":"shape","id":"transparent-text","preset":"rect","x":30,"y":30,"width":180,"height":20,"fill":"FFFFFF","stroke":"FFFFFF","stroke_width":0,"text":"Visible text","font_size":16,"color":"000000","bold":false,"visual":{"opacity":0}},
        {"type":"text","id":"rotated","x":0,"y":10,"width":200,"height":50,"text":"Rotated","font_size":24,"color":"000000","bold":false,"visual":{"rotation":45}},
        {"type":"shape","id":"endpoint","preset":"rect","x":50,"y":300,"width":100,"height":80,"fill":"FFFFFF","stroke":"000000","stroke_width":1,"text":"Node","font_size":20,"color":"000000","bold":false},
        {"type":"connector","id":"attached","x":100,"y":330,"width":180,"height":1,"color":"000000","stroke_width":2,"arrow":true,"start":{"element_id":"endpoint","site":1}},
        {"type":"text","id":"hidden","x":10,"y":440,"width":60,"height":8,"text":"Hidden overflow","font_size":24,"color":"000000","bold":false,"visual":{"hidden":true}}
    ]}]});
    let document = execute_request(json!({"op":"new_document","id":"visibility","deck":deck})).unwrap();
    let result = execute_request(json!({"op":"preflight_presentation","document":document})).unwrap();
    let findings = result["findings"].as_array().unwrap();
    let clipped = findings.iter().find(|finding| finding["code"] == "TEXT_OVERFLOW" && finding["element_ids"].as_array().unwrap().contains(&json!("transparent-text"))).unwrap();
    assert!(clipped["bounds"]["width"].as_f64().unwrap() > 0.0);
    assert!(findings.iter().any(|finding| finding["code"] == "OFF_SLIDE" && finding["element_ids"].as_array().unwrap().contains(&json!("rotated"))));
    assert!(!findings.iter().any(|finding| finding["code"] == "CONNECTOR_LABEL_INTERFERENCE" && finding["element_ids"].as_array().unwrap().contains(&json!("endpoint"))));
    assert!(!findings.iter().any(|finding| finding["element_ids"].as_array().unwrap().contains(&json!("hidden"))));
}

#[test]
fn authoring_preflight_ignores_unrendered_master_and_layout_placeholders() {
    let mut deck=document()["deck"].clone();
    let placeholder=json!({"type":"text","id":"phantom","x":20,"y":20,"width":200,"height":40,"text":"Not rendered","font_size":8,"color":"000000","bold":false,"format":{"placeholder":{"kind":"title","index":0}}});
    deck["design"]["masters"][0]["elements"]=json!([placeholder]);
    deck["slides"][0]["elements"]=json!([{"type":"text","id":"visible","x":20,"y":20,"width":200,"height":40,"text":"Visible","font_size":24,"color":"000000","bold":false}]);
    let document=execute_request(json!({"op":"new_document","id":"placeholder-preflight","deck":deck})).unwrap();
    let result=execute_request(json!({"op":"preflight_presentation","document":document})).unwrap();
    assert!(!result["findings"].as_array().unwrap().iter().any(|finding|finding["element_ids"].as_array().unwrap().contains(&json!("phantom"))),"{result}");
}

#[test]
fn slide_revision_preview_is_scoped_and_native_apply_is_one_exact_undo() {
    let mut deck = document()["deck"].clone();
    deck["slides"][0]["elements"] = json!([{ "type":"text","id":"label","x":50,"y":50,"width":280,"height":60,"text":"Before","font_size":24,"color":"000000","bold":false }]);
    let mut second = deck["slides"][0].clone();
    second["id"] = json!("untouched");
    second["elements"][0]["id"] = json!("other-label");
    deck["slides"].as_array_mut().unwrap().push(second);
    let authored = execute_request(json!({"op":"new_document","id":"revision-author","deck":deck})).unwrap();
    let source = execute_request(json!({"op":"export_presentation","document":authored})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"revision-native","base64":source["base64"]})).unwrap();
    let document = opened["document"].clone();
    let slide_id = document["deck"]["slides"][0]["id"].clone();
    let element_id = document["deck"]["slides"][0]["elements"][0]["id"].clone();
    let edits = json!([
        {"op":"set_text_frame","id":element_id,"x":60,"y":80,"width":400,"height":80},
        {"op":"replace_text","id":element_id,"text":"After"}
    ]);
    let request = json!({"op":"preview_slide_revision","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"slide_id":slide_id,"edits":edits,"max_dimension":640});
    let preview = execute_request(request.clone()).unwrap();
    assert_eq!(preview["base_hash"], document["hash"]);
    assert_ne!(preview["candidate_hash"], document["hash"]);
    assert_ne!(preview["before"]["images"][0]["sha256"], preview["after"]["images"][0]["sha256"]);
    assert_eq!(preview["before"]["pages"][0]["slide_id"], slide_id);
    assert_eq!(preview["affected_ids"], json!([element_id]));
    assert!(preview.get("document").is_none());
    let mut apply = request.clone();
    apply["op"] = json!("apply_slide_revision");
    apply.as_object_mut().unwrap().remove("max_dimension");
    apply["candidate_hash"] = preview["candidate_hash"].clone();
    let changed = execute_request(apply.clone()).unwrap();
    assert_eq!(changed["document"]["revision"], document["revision"].as_u64().unwrap() + 1);
    assert_eq!(changed["document"]["deck"]["slides"][1], document["deck"]["slides"][1]);
    assert_eq!(changed["document"]["origin"], document["origin"]);
    assert_eq!(changed["document"]["deck"]["slides"][0]["elements"][0]["font_size"], document["deck"]["slides"][0]["elements"][0]["font_size"]);
    let restored = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
    assert_eq!(execute_request(json!({"op":"export_presentation","document":restored["document"]})).unwrap()["base64"], source["base64"]);
    for field in ["expected_hash", "candidate_hash"] {
        let mut invalid = apply.clone(); invalid[field] = json!("0".repeat(64));
        assert!(execute_request(invalid).is_err());
    }
    apply["document"] = changed["document"].clone();
    assert!(execute_request(apply).is_err());
    assert_eq!(execute_request(json!({"op":"verify_recovery","document":document})).unwrap(), document);
}

#[test]
fn slide_revision_rejects_unknown_locked_or_out_of_scope_edits() {
    let mut deck = document()["deck"].clone();
    deck["slides"][0]["elements"] = json!([{ "type":"text","id":"locked","x":50,"y":50,"width":280,"height":60,"text":"Before","font_size":24,"color":"000000","bold":false,"visual":{"locked":true} }]);
    let document = execute_request(json!({"op":"new_document","id":"revision-locked","deck":deck})).unwrap();
    for edits in [json!([]), json!([{"op":"replace_text","id":"missing","text":"No"}]),json!([{"op":"replace_text","id":"locked","text":"No"}]),json!([{"op":"remove","id":"locked"}]),json!([{"op":"set_text_frame","id":"locked","x":0,"y":0,"width":300,"height":100}])] {
        assert!(execute_request(json!({"op":"preview_slide_revision","document":document,"expected_revision":0,"expected_hash":document["hash"],"slide_id":"slide-1","edits":edits})).is_err());
    }
}

#[test]
fn slide_revision_managed_updates_keep_metadata_and_reject_manual_changes() {
    let part=json!({"version":1,"preset":"list-horizontal/balanced","title":"Steps","data":{"kind":"items","items":[{"label":"Check","detail":"Review"},{"label":"Act","detail":"Proceed"}]}});
    let graph=json!({"version":1,"title":"Route","nodes":[{"id":"first","label":"Start","x":100,"y":140},{"id":"second","label":"End","x":620,"y":140}],"edges":[{"id":"link","source":"first","target":"second","label":"Next"}]});
    for (insert,update,spec) in [("insert_part","update_part",part),("insert_graph","update_graph",graph)] {
        let base=document();
        let document=execute_request(json!({"op":insert,"document":base,"expected_revision":0,"slide_id":"slide-1","id":"managed","spec":spec})).unwrap()["document"].clone();
        let mut revised=spec.clone();revised["title"]=json!("Revised");
        let edits=json!([{"op":update,"id":"managed","spec":revised}]);
        let preview=execute_request(json!({"op":"preview_slide_revision","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"slide_id":"slide-1","edits":edits,"max_dimension":320})).unwrap();
        assert_eq!(preview["stale_part_ids"],json!([]));
        let changed=execute_request(json!({"op":"apply_slide_revision","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"slide_id":"slide-1","edits":edits,"candidate_hash":preview["candidate_hash"]})).unwrap();
        assert_eq!(changed["document"]["parts"][0]["stale"],false);
        let undo=execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
        assert_eq!(undo["document"]["hash"],document["hash"]);
        let root=&document["deck"]["slides"][0]["elements"][0];
        let (child_index,child)=root["children"].as_array().unwrap().iter().enumerate().find(|(_,element)|element.get("text").is_some()).unwrap();
        let manual=execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":format!("/deck/slides/0/elements/0/children/{child_index}/text"),"value":format!("{} changed",child["text"].as_str().unwrap())}]}})).unwrap()["document"].clone();
        assert_eq!(manual["parts"][0]["stale"],true);
        let error=execute_request(json!({"op":"preview_slide_revision","document":manual,"expected_revision":manual["revision"],"expected_hash":manual["hash"],"slide_id":"slide-1","edits":edits})).unwrap_err();
        assert!(error.to_string().contains("stale"),"{error}");
    }
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