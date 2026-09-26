use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn column_spec() -> Value {
    json!({"version":1,"preset":"vertical-bar-graph/balanced","title":"Quarterly volume","subtitle":"Synthetic example","data":{"kind":"chart","categories":["Q1","Q2","Q3"],"series":[{"name":"Volume","values":[12,24,18]}],"x_axis":"Quarter","y_axis":"Units"}})
}

fn open_list_spec(preset: &str) -> Value {
    json!({"version":1,"preset":preset,"title":"Operating capabilities","subtitle":"Synthetic example","data":{"kind":"items","items":[
        {"label":"Collection","detail":"Bring the relevant signals together."},
        {"label":"Coordination","detail":"Connect decisions across teams."},
        {"label":"Judgment","detail":"Keep priorities and outcomes explicit."},
        {"label":"Learning","detail":"Use feedback to improve the next response."}
    ]}})
}

#[test]
fn open_list_presets_keep_content_without_decorative_boxes_rails_or_numbers() {
    for preset in ["list/rows", "list-horizontal/columns", "list-enumeration/grid"] {
        let spec = open_list_spec(preset);
        let rendered = execute_request(json!({"op":"create_part","id":"open-list","spec":spec})).unwrap();
        let children = rendered["children"].as_array().unwrap();
        assert!(children.iter().all(|element| element["type"] == "text"), "{preset}: decorative geometry");
        assert!(children.iter().all(|element| [json!("@dk1"), json!("@dk2")].contains(&element["color"])), "{preset}: automatic category color");
        let mut expected = vec![spec["title"].clone(), spec["subtitle"].clone()];
        for item in spec["data"]["items"].as_array().unwrap() { expected.extend([item["label"].clone(), item["detail"].clone()]); }
        assert_eq!(children.iter().map(|element| element["text"].clone()).collect::<Vec<_>>(), expected, "{preset}: content or automatic numbering");
        let labels: Vec<_> = children.iter().skip(2).step_by(2).collect();
        if preset == "list/rows" {
            assert!(labels.iter().all(|label| label["x"] == labels[0]["x"]));
            assert!(labels.windows(2).all(|pair| pair[0]["y"].as_f64().unwrap() < pair[1]["y"].as_f64().unwrap()));
        } else if preset == "list-horizontal/columns" {
            assert!(labels.iter().all(|label| label["y"] == labels[0]["y"]));
            assert!(labels.windows(2).all(|pair| pair[0]["x"].as_f64().unwrap() < pair[1]["x"].as_f64().unwrap()));
        } else {
            assert_eq!(labels[0]["y"], labels[1]["y"]);
            assert_eq!(labels[0]["x"], labels[2]["x"]);
            assert!(labels[0]["y"].as_f64().unwrap() < labels[2]["y"].as_f64().unwrap());
        }
    }
}

#[test]
fn open_list_catalog_recommends_new_compositions_and_retains_legacy_ids() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    let presets = catalog["presets"].as_array().unwrap();
    assert_eq!(presets.len(), 111);
    for (category, variant) in [("list", "rows"), ("list-horizontal", "columns"), ("list-enumeration", "grid")] {
        let id = format!("{category}/{variant}");
        let entry = presets.iter().find(|entry| entry["id"] == id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(entry["recommended"], true);
        assert!(!entry["use_when"].as_str().unwrap().is_empty());
        assert!(!entry["avoid_when"].as_str().unwrap().is_empty());
        assert_eq!(entry["example"]["preset"], id);
        for legacy in ["balanced", "focus", "labeled"] {
            assert!(presets.iter().any(|entry| entry["id"] == format!("{category}/{legacy}")));
        }
    }
}

#[test]
fn open_list_additions_preserve_all_nine_legacy_native_exports() {
    use sha2::{Digest, Sha256};
    for (preset, expected) in [
        ("list/balanced", "e38ff079fe057ceda156936d1e069bb34cdafad379d1a568573d7985e9bb47f0"),
        ("list/focus", "58d7ab7756459714054fff9e982c17d0802a4f66c4fb0f671055b44681e82c41"),
        ("list/labeled", "00a8b100ed2bab08451eb8963a10c7f3b0405aff1f218694e46b6d7bc2b0b84b"),
        ("list-horizontal/balanced", "a7d172e038d7d7e27c721418e9d0e427086a4f31a12e9b9cdf9eaeb590489ac8"),
        ("list-horizontal/focus", "7fd322fdde880490b5463b90f312baa7a37d789ebf5097df1aaa27cc8865824f"),
        ("list-horizontal/labeled", "6aadeea6cda76e61facec77eacd2eff2e804af42cc7bf976835dc71963f4634e"),
        ("list-enumeration/balanced", "0b92562809471a59b9b9669604744613f74acc4d8ff0990b455f8028c3a4655a"),
        ("list-enumeration/focus", "db82082f14462d5a685fd08737722daa4f5a6b50f231942c5d20f0bdb715f478"),
        ("list-enumeration/labeled", "f0309f4c451c887f7918fd04c73401a60e36eb4e5e6bc0847a2de65f9337888d"),
    ] {
        let rendered = execute_request(json!({"op":"create_part","id":"legacy-list","spec":open_list_spec(preset)})).unwrap();
        let saved = execute_request(json!({"op":"export","deck":deck(rendered)})).unwrap();
        let bytes = STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected, "{preset}");
    }
}

#[test]
fn open_list_native_layout_update_reopen_and_undo_preserve_editability() {
    for preset in ["list/rows", "list-horizontal/columns", "list-enumeration/grid"] {
        let mut spec = open_list_spec(preset);
        spec["layout"] = json!({"x":24,"y":36,"width":1152,"height":424,"show_title":false});
        let mut scene = deck(json!({})); scene["slides"][0]["elements"] = json!([]);
        let document = execute_request(json!({"op":"new_document","id":"open-list-native","deck":scene})).unwrap();
        let inserted = execute_request(json!({"op":"apply_operations","document":document,"expected_revision":0,"expected_hash":document["hash"],"operations":[
            {"op":"add_part","slide_id":"slide","id":"open-list","spec":spec}
        ]})).unwrap();
        let group = &inserted["document"]["deck"]["slides"][0]["elements"][0];
        assert_eq!(group["x"], 24.0); assert_eq!(group["y"], 36.0);
        let children = group["children"].as_array().unwrap();
        assert_eq!(children.len(), 8);
        assert!(children.iter().all(|child| child["type"] == "text"));
        let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
        let opened = execute_request(json!({"op":"open_presentation","id":"open-list-reopened","base64":saved["base64"]})).unwrap();
        assert_eq!(opened["document"]["parts"][0]["stale"], false);
        assert_eq!(opened["document"]["parts"][0]["spec"], inserted["document"]["parts"][0]["spec"]);
        assert!(opened["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().iter().all(|child| child["type"] == "text"));
        let unchanged = execute_request(json!({"op":"update_part","document":opened["document"],"expected_revision":0,"slide_id":"slide","id":"open-list","spec":spec})).unwrap();
        assert!(unchanged["receipt"].is_null(), "{preset}");
        assert_eq!(execute_request(json!({"op":"export_presentation","document":unchanged["document"]})).unwrap()["base64"], saved["base64"]);
        spec["data"]["items"][0]["detail"] = json!("Updated synthetic explanation.");
        let updated = execute_request(json!({"op":"update_part","document":opened["document"],"expected_revision":0,"slide_id":"slide","id":"open-list","spec":spec})).unwrap();
        assert_eq!(updated["document"]["parts"][0]["stale"], false);
        let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":1,"receipt":updated["receipt"]})).unwrap();
        assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], saved["base64"]);
        spec["layout"]["width"] = json!(1080);
        let fractional = execute_request(json!({"op":"create_part","id":"fractional-list","spec":spec})).unwrap();
        let exported = execute_request(json!({"op":"export","deck":deck(fractional.clone())})).unwrap();
        let reopened = execute_request(json!({"op":"open_presentation","id":"fractional-list","base64":exported["base64"]})).unwrap();
        let restored = &reopened["document"]["deck"]["slides"][0]["elements"][0];
        for (source, target) in fractional["children"].as_array().unwrap().iter().zip(restored["children"].as_array().unwrap()) {
            assert_eq!(source["text"], target["text"]);
            for field in ["x", "y", "width", "height"] { assert!((source[field].as_f64().unwrap() - target[field].as_f64().unwrap()).abs() <= 1.0 / 9525.0, "{preset}: {field}"); }
        }
    }
}

#[test]
fn open_list_japanese_text_and_titleless_frames_remain_measurable() {
    for preset in ["list/rows", "list-horizontal/columns", "list-enumeration/grid"] {
        for titleless in [false, true] {
            let mut spec = open_list_spec(preset);
            for item in spec["data"]["items"].as_array_mut().unwrap() {
                item["label"] = json!("人とAIの役割分担");
                item["detail"] = json!("人が優先度と成果を定義し、AIエージェントが調査と調整を担う。判断の根拠を共有し、継続的に改善する。");
            }
            if titleless { spec["layout"] = json!({"x":24,"y":36,"width":1152,"height":424,"show_title":false}); }
            let rendered = execute_request(json!({"op":"create_part","id":"open-japanese","spec":spec})).unwrap();
            let measured = execute_request(json!({"op":"measure_layout","deck":deck(rendered)})).unwrap();
            assert!(measured["measurements"].as_array().unwrap().iter().all(|entry| entry["overflow"] == false && entry["missing_glyphs"] == 0), "{preset}: {measured}");
        }
    }
}

#[test]
fn open_list_limits_reject_wrong_categories_and_excess_items() {
    for preset in ["list/columns", "list-horizontal/rows", "list-enumeration/rows", "flow/rows", "pie-chart/grid"] {
        let error = execute_request(json!({"op":"create_part","id":"invalid-open-list","spec":open_list_spec(preset)})).unwrap_err().to_string();
        assert!(error.contains("unknown part variant"), "{error}");
    }
    for (preset, maximum) in [("list/rows",8), ("list-horizontal/columns",4), ("list-enumeration/grid",8)] {
        for length in [2, maximum, maximum + 1] {
            let mut spec = open_list_spec(preset);
            spec["data"]["items"] = json!((0..length).map(|index| json!({"label":format!("Topic {}",index+1),"detail":"Short explanation."})).collect::<Vec<_>>());
            let result = execute_request(json!({"op":"create_part","id":"open-list-limit","spec":spec}));
            if length <= maximum { assert!(result.is_ok(), "{preset}: {result:?}"); }
            else { assert!(result.unwrap_err().to_string().contains(&format!("2-{maximum} items"))); }
        }
    }
}

#[test]
fn open_list_selection_policy_is_in_every_core_authoring_guide() {
    for profile_id in ["consulting-decision", "technical-explainer", "event-talk", "status-report"] {
        let guide = execute_request(json!({"op":"best_practice_guide","profile_id":profile_id})).unwrap();
        let markdown = guide["markdown"].as_str().unwrap();
        for phrase in ["Choose the information relationship before the layout", "list/rows", "list-horizontal/columns", "list-enumeration/grid", "use_when", "avoid_when", "do not randomize layouts", "Existing preset IDs retain their rendering"] {
            assert!(markdown.contains(phrase), "{profile_id}: missing {phrase}");
        }
    }
}

fn deck(element: Value) -> Value {
    json!({"version":1,"title":"Parts test","width":1280,"height":720,"slides":[{"id":"slide","title":"Parts","background":"FFFFFF","notes":"Synthetic examples","elements":[element]}]})
}

fn matrix_spec(category: &str, variant: &str) -> Value {
    json!({"version":1,"preset":format!("{category}/{variant}"),"title":"Alternatives","subtitle":"Synthetic example","data":{"kind":"matrix","rows":["Speed","Control"],"columns":["Option A","Option B"],"cells":[["High","Medium"],["Shared","Dedicated"]]}})
}

fn managed_chart_with_theme_override() -> Value {
    let mut scene = deck(json!({})); scene["slides"][0]["elements"] = json!([]);
    let document = execute_request(json!({"op":"new_document","id":"chart-dependency","deck":scene})).unwrap();
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"managed","spec":column_spec()})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    let mut entries = package.parts().clone();
    entries.insert("ppt/theme/managedOverride.xml".into(), br#"<a:themeOverride xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:fontScheme name="Synthetic override"><a:majorFont><a:latin typeface="Courier New"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Courier New"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme></a:themeOverride>"#.to_vec());
    let mut package = Package::from_parts(entries).unwrap();
    let types = package.text("[Content_Types].xml").unwrap().replace("</Types>", "<Override PartName=\"/ppt/theme/managedOverride.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.themeOverride+xml\"/></Types>");
    package.replace_part("[Content_Types].xml", types.into_bytes()).unwrap();
    let relationships = package.text("ppt/charts/_rels/chart1.xml.rels").unwrap().replace("</Relationships>", "<Relationship Id=\"rIdOverride\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/themeOverride\" Target=\"../theme/managedOverride.xml\"/></Relationships>");
    package.replace_part("ppt/charts/_rels/chart1.xml.rels", relationships.into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let package = Package::open(bytes.clone()).unwrap();
    assert!(package.text("ppt/charts/_rels/chart1.xml.rels").unwrap().contains("themeOverride"));
    let reopened = execute_request(json!({"op":"open_presentation","id":"custom-managed","base64":STANDARD.encode(&bytes)})).unwrap();
    assert_eq!(reopened["document"]["parts"][0]["stale"], false);
    let unchanged = execute_request(json!({"op":"export_presentation","document":reopened["document"]})).unwrap();
    assert_eq!(STANDARD.decode(unchanged["base64"].as_str().unwrap()).unwrap(), bytes);
    reopened["document"].clone()
}

fn assert_managed_chart_dependency_rejected(batch: bool) {
    let document = managed_chart_with_theme_override();
    let snapshot = document.clone();
    let mut spec = column_spec(); spec["title"] = json!("Changed child identities");
    let operation = json!({"op":"update_part","slide_id":"slide","id":"managed","spec":spec});
    let request = if batch {
        json!({"op":"apply_operations","document":document,"expected_revision":0,"expected_hash":document["hash"],"operations":[
            {"op":"set_slide_background","slide_id":"slide","color":"ABCDEF"}, operation
        ]})
    } else {
        json!({"op":"update_part","document":document,"expected_revision":0,"slide_id":"slide","id":"managed","spec":spec})
    };
    let result = execute_request(request);
    if let Ok(changed) = &result {
        let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        let package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
        panic!("unsafe managed regeneration accepted (batch={batch}); new chart={}, old override retained but not attached to new chart={}",
            package.part("ppt/charts/chart2.xml").is_ok(),
            package.part("ppt/theme/managedOverride.xml").is_ok() && !package.text("ppt/charts/_rels/chart2.xml.rels").unwrap().contains("themeOverride"));
    }
    let error = result.unwrap_err().to_string();
    assert!(error.contains("Unsupported: custom native chart"), "{error}");
    assert_eq!(document, snapshot);
    let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    assert_eq!(saved["base64"], document["origin"]["base64"]);
}

#[test]
fn managed_chart_dependency_individual_update_rejects_regeneration() {
    assert_managed_chart_dependency_rejected(false);
}

#[test]
fn managed_chart_dependency_batch_update_rejects_regeneration() {
    assert_managed_chart_dependency_rejected(true);
}

#[test]
fn managed_chart_dependency_safe_regeneration_preserves_sources_opaque_parts_and_undo() {
    let source = execute_request(json!({"op":"ingest","input":{"format":"csv","name":"synthetic.csv","base64":STANDARD.encode(b"Label,Value\nSelected,12\n")}})).unwrap();
    let marker = json!({"id":"source-marker","type":"text","x":10,"y":10,"width":120,"height":30,"text":"12","font_size":18,"color":"202426","bold":false});
    let binding = json!({"slide_id":"slide","element_id":"source-marker","field":"/text","source_id":source["id"],"source_sha256":source["sha256"],"locator":"record:2/column:2","raw_value":"12","value":"12","transform":"display_scalar","stale":false});
    let mut scene = deck(marker);
    let mut untouched = scene["slides"][0].clone(); untouched["id"] = json!("untouched"); untouched["elements"] = json!([]);
    scene["slides"].as_array_mut().unwrap().push(untouched);
    let document = execute_request(json!({"op":"new_document","id":"safe-chart","deck":scene,"sources":[source],"bindings":[binding]})).unwrap();
    let mut spec = column_spec(); spec["layout"] = json!({"x":40,"y":100,"width":576,"height":512});
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"managed","spec":spec})).unwrap();
    let exported = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let mut package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide2.xml").unwrap().replace("</p:sld>", "<p:extLst><p:ext uri=\"urn:test:chart-control\"><v:opaque xmlns:v=\"urn:test:chart-control\" value=\"keep\"/></p:ext></p:extLst></p:sld>");
    package.replace_part("ppt/slides/slide2.xml", xml.into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let original_parts = Package::open(bytes.clone()).unwrap().parts().clone();
    let opened = execute_request(json!({"op":"open_presentation","id":"safe-reopened","base64":STANDARD.encode(&bytes)})).unwrap();
    let before = &opened["document"];
    assert_eq!(before["parts"][0]["stale"], false);
    for batch in [false, true] {
        let noop = if batch {
            json!({"op":"apply_operations","document":before,"expected_revision":0,"expected_hash":before["hash"],"operations":[{"op":"update_part","slide_id":"slide","id":"managed","spec":spec}]})
        } else { json!({"op":"update_part","document":before,"expected_revision":0,"slide_id":"slide","id":"managed","spec":spec}) };
        let noop = execute_request(noop).unwrap();
        assert_eq!(noop["document"], *before);
        assert!(noop["receipt"].is_null());
        assert_eq!(execute_request(json!({"op":"export_presentation","document":noop["document"]})).unwrap()["base64"], STANDARD.encode(&bytes));
    }
    let moved = execute_request(json!({"op":"apply_operations","document":before,"expected_revision":0,"expected_hash":before["hash"],"operations":[
        {"op":"set_frame","slide_id":"slide","id":"managed","frame":{"x":90,"y":120,"width":576,"height":512}}
    ]})).unwrap();
    let mut first = spec.clone(); first["data"]["series"][0]["values"][0] = json!(44);
    let mut second = first.clone(); second["title"] = json!("Second regeneration");
    second["layout"]["x"] = json!(90); second["layout"]["y"] = json!(120);
    let updated = execute_request(json!({"op":"apply_operations","document":moved["document"],"expected_revision":1,"expected_hash":moved["document"]["hash"],"operations":[
        {"op":"update_part","slide_id":"slide","id":"managed","spec":first},
        {"op":"update_part","slide_id":"slide","id":"managed","spec":second}
    ]})).unwrap();
    for field in ["sources", "bindings", "origin"] { assert_eq!(updated["document"][field], before[field], "{field}"); }
    assert_eq!(updated["document"]["deck"]["slides"][0]["elements"][0], before["deck"]["slides"][0]["elements"][0]);
    assert_eq!(updated["document"]["parts"][0]["spec"]["layout"]["x"], 90.0);
    assert_eq!(updated["document"]["parts"][0]["spec"]["layout"]["y"], 120.0);
    assert_eq!(updated["document"]["parts"][0]["stale"], false);
    let saved = execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
    let output = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    for (path, content) in &original_parts {
        if ["ppt/slides/slide1.xml", "ppt/slides/_rels/slide1.xml.rels", "[Content_Types].xml"].contains(&path.as_str()) || path.starts_with("customXml/") { continue; }
        assert_eq!(output.part(path).unwrap(), content, "untouched native part {path}");
    }
    let reopened = execute_request(json!({"op":"open_presentation","id":"safe-updated","base64":saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["parts"][0]["stale"], false);
    assert_eq!(reopened["document"]["parts"][0]["spec"], updated["document"]["parts"][0]["spec"]);
    for field in ["sources", "bindings"] { assert_eq!(reopened["document"][field], before[field], "reopened {field}"); }
    let undo_update = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":2,"receipt":updated["receipt"]})).unwrap();
    let undo_move = execute_request(json!({"op":"undo_transaction","document":undo_update["document"],"expected_revision":3,"receipt":moved["receipt"]})).unwrap();
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undo_move["document"]})).unwrap()["base64"], STANDARD.encode(bytes));
}

fn assert_managed_part_master_theme(batch: bool, update: bool) {
    use aislide_core::{design::{Design, Master, SlideLayout}, parts::{PartSpec, create_with_theme}};
    let mut design = Design::default();
    design.theme.fonts.minor = "Arial".into();
    let mut master_theme = design.theme.clone();
    master_theme.fonts.major = "Courier New".into();
    master_theme.fonts.minor = "Courier New".into();
    design.masters.push(Master { id: "second".into(), name: "Second".into(), background: "@lt1".into(), elements: vec![], theme: Some(master_theme.clone()) });
    design.layouts.push(SlideLayout { id: "second-layout".into(), name: "Second".into(), master_id: "second".into(), background: None, elements: vec![] });
    for bounded in [false, true] {
        let mut spec = column_spec();
        spec["title"] = json!("i".repeat(if bounded { 40 } else { 80 }));
        if bounded { spec["layout"] = json!({"x":40,"y":100,"width":576,"height":512}); }
        let parsed: PartSpec = serde_json::from_value(spec.clone()).unwrap();
        let expected = serde_json::to_value(create_with_theme("fitted", &parsed, &master_theme).unwrap()).unwrap();
        let default = serde_json::to_value(create_with_theme("fitted", &parsed, &design.theme).unwrap()).unwrap();
        assert_ne!(expected, default, "font fixture must discriminate the effective theme (bounded={bounded})");
        let mut scene = deck(json!({})); scene["slides"][0]["elements"] = json!([]);
        scene["design"] = serde_json::to_value(&design).unwrap();
        for (layout, expected) in [(json!("second-layout"), &expected), (json!("blank"), &default), (Value::Null, &default)] {
            scene["slides"][0]["layout_id"] = layout;
            let mut document = execute_request(json!({"op":"new_document","id":"master-fit","deck":scene})).unwrap();
            if update {
                let mut seed = spec.clone(); seed["title"] = json!("Seed");
                document = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"fitted","spec":seed})).unwrap()["document"].clone();
            }
            let request = if batch {
                json!({"op":"apply_operations","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
                    {"op":if update {"update_part"} else {"add_part"},"slide_id":"slide","id":"fitted","spec":spec}
                ]})
            } else {
                json!({"op":if update {"update_part"} else {"insert_part"},"document":document,"expected_revision":document["revision"],"slide_id":"slide","id":"fitted","spec":spec})
            };
            let changed = execute_request(request).unwrap();
            assert_eq!(&changed["document"]["deck"]["slides"][0]["elements"][0], expected, "effective master fitting: batch={batch}, update={update}, bounded={bounded}");
            assert_eq!(changed["document"]["parts"][0]["stale"], false);
        }
    }
}

#[test]
fn managed_part_master_theme_individual_insert_matches_factory() {
    assert_managed_part_master_theme(false, false);
}

#[test]
fn managed_part_master_theme_individual_update_matches_factory() {
    assert_managed_part_master_theme(false, true);
}

#[test]
fn managed_part_master_theme_batch_insert_matches_factory() {
    assert_managed_part_master_theme(true, false);
}

#[test]
fn managed_part_master_theme_batch_update_matches_factory() {
    assert_managed_part_master_theme(true, true);
}

#[test]
fn matrix_corner_label_empty_preserves_legacy_serialization_ids_and_native_bytes() {
    use sha2::{Digest, Sha256};
    let mut failures = Vec::new();
    for category in ["matrix", "contrast"] {
        for variant in ["balanced", "focus", "labeled"] {
            let legacy = matrix_spec(category, variant);
            let parsed: aislide_core::parts::PartSpec = serde_json::from_value(legacy.clone()).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap(), legacy);
            let original = execute_request(json!({"op":"create_part","id":"legacy-matrix","spec":legacy})).unwrap();
            let identity = format!("{:x}", Sha256::digest(serde_json::to_vec(&legacy).unwrap()));
            assert_eq!(original["children"][0]["id"], format!("legacy-matrix-{}-0", &identity[..10]));
            let saved = execute_request(json!({"op":"export","deck":deck(original.clone())})).unwrap();
            let legacy_sha256 = match (category, variant) {
                ("matrix", "balanced") => "734a91f424f425cc19bf845d15fda9930d2576c4077bd1e877da50a78fb23c46",
                ("matrix", "focus") => "53cb61dc15108b14e67f452bc38b3f3000fb9d33cd45f43ad2fb3a5983d9609b",
                ("matrix", "labeled") => "a561fe30c26fb8cc7500d9f9d6cf3ba53af67b056c88edd3ebffb67ffc6c6361",
                ("contrast", "balanced") => "aa80268e7fdf1b467538b94da817b7591a4ddf38ea8f486cc48d0ba742925d60",
                ("contrast", "focus") => "896027bf450670dcaec293d31ade9289628ed4fe828a5f194034f9d943736325",
                ("contrast", "labeled") => "942150c2faa44e1c3e4c7a7ef675dfc80bb1ae449f6df5aaee337fe2bdebfc34",
                _ => unreachable!(),
            };
            assert_eq!(format!("{:x}", Sha256::digest(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap())), legacy_sha256, "{category}/{variant}");
            let mut explicit = legacy.clone(); explicit["data"]["corner_label"] = json!("");
            let parsed = match serde_json::from_value::<aislide_core::parts::PartSpec>(explicit.clone()) {
                Ok(parsed) => parsed,
                Err(error) => { failures.push(format!("{category}/{variant}: {error}")); continue; }
            };
            assert_eq!(serde_json::to_value(parsed).unwrap(), legacy);
            let blank = execute_request(json!({"op":"create_part","id":"legacy-matrix","spec":explicit})).unwrap();
            assert_eq!(blank, original);
            assert_eq!(execute_request(json!({"op":"export","deck":deck(blank)})).unwrap()["base64"], saved["base64"]);
            let mut scene = deck(json!({})); scene["slides"][0]["elements"] = json!([]);
            let document = execute_request(json!({"op":"new_document","id":"legacy-matrix","deck":scene})).unwrap();
            let omitted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"legacy-matrix","spec":legacy})).unwrap();
            let explicit = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"legacy-matrix","spec":explicit})).unwrap();
            assert_eq!(omitted, explicit);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn matrix_corner_labels_are_editable_native_headers_without_overlap() {
    for category in ["matrix", "contrast"] {
        for variant in ["balanced", "focus", "labeled"] {
            let mut spec = matrix_spec(category, variant);
            let blank = execute_request(json!({"op":"create_part","id":"heading","spec":spec})).unwrap();
            for label in ["Criterion", "\u{89b3}\u{70b9}"] {
                spec["data"]["corner_label"] = json!(label);
                let element = execute_request(json!({"op":"create_part","id":"heading","spec":spec})).unwrap();
                let children = element["children"].as_array().unwrap();
                if variant == "labeled" {
                    assert_eq!(children.len(), blank["children"].as_array().unwrap().len());
                    let table = children.iter().find(|child| child["type"] == "table").unwrap();
                    assert_eq!(table["rows"], json!([[label,"Option A","Option B"],["Speed","High","Medium"],["Control","Shared","Dedicated"]]));
                } else {
                    assert_eq!(children.len(), blank["children"].as_array().unwrap().len() + 1);
                    let heading = children.iter().find(|child| child["type"] == "text" && child["text"] == label).unwrap();
                    for (field, expected) in [("x",24.0),("y",90.0),("width",180.0),("height",48.0)] { assert_eq!(heading[field], expected, "{field}"); }
                    assert_eq!(heading["bold"], true);
                    assert_eq!(heading["format"]["alignment"], "left");
                    for other in children.iter().filter(|child| child["id"] != heading["id"]) {
                        let left = other["x"].as_f64().unwrap(); let top = other["y"].as_f64().unwrap();
                        let right = left + other["width"].as_f64().unwrap(); let bottom = top + other["height"].as_f64().unwrap();
                        assert!(!(left < 204.0 && right > 24.0 && top < 138.0 && bottom > 90.0), "{category}/{variant}: {} overlaps corner", other["id"]);
                    }
                }
                let measured = execute_request(json!({"op":"measure_layout","deck":deck(element.clone())})).unwrap();
                assert!(measured["issues"].as_array().unwrap().iter().all(|issue| issue["severity"] != "error"), "{}", measured["issues"]);
                let exported = execute_request(json!({"op":"export","deck":deck(element)})).unwrap();
                let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
                assert!(package.text("ppt/slides/slide1.xml").unwrap().contains(&format!("<a:t>{label}</a:t>")));
                assert!(!package.parts().keys().any(|path| path.starts_with("ppt/media/")));
            }
        }
    }
}

#[test]
fn matrix_corner_label_is_a_bounded_xml_safe_string() {
    for category in ["matrix", "contrast"] {
        for variant in ["balanced", "focus", "labeled"] {
            let mut spec = matrix_spec(category, variant);
            for label in ["i".repeat(48), "\u{e9}".repeat(48), "A <&> B".into()] {
                spec["data"]["corner_label"] = json!(label);
                execute_request(json!({"op":"create_part","id":"valid-corner","spec":spec})).unwrap();
            }
            for label in ["i".repeat(49), "\u{89b3}".repeat(49)] {
                spec["data"]["corner_label"] = json!(label);
                let error = execute_request(json!({"op":"create_part","id":"long-corner","spec":spec})).unwrap_err();
                assert!(error.to_string().contains("text exceeds 48 characters"), "{error}");
            }
            for label in [json!("bad\u{0}text"), json!("bad\u{8}text"), json!("bad\u{fffe}text"), Value::Null, json!(42), json!({})] {
                spec["data"]["corner_label"] = label;
                assert!(execute_request(json!({"op":"create_part","id":"invalid-corner","spec":spec})).is_err(), "{spec}");
            }
        }
    }
    let mut invalid = column_spec(); invalid["data"]["corner_label"] = json!("Criterion");
    assert!(execute_request(json!({"op":"create_part","id":"wrong-kind","spec":invalid})).is_err());
}

#[test]
fn matrix_corner_labels_survive_bounded_native_update_reopen_and_undo() {
    for category in ["matrix", "contrast"] {
        for variant in ["balanced", "focus", "labeled"] {
            let mut scene = deck(json!({})); scene["slides"][0]["elements"] = json!([]);
            let document = execute_request(json!({"op":"new_document","id":"corner-state","deck":scene})).unwrap();
            let mut spec = matrix_spec(category, variant);
            spec["layout"] = json!({"x":40.0,"y":100.0,"width":1152.0,"height":424.0,"show_title":false});
            spec["data"]["corner_label"] = json!("Criterion");
            let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"corner","spec":spec})).unwrap();
            let factory = execute_request(json!({"op":"create_part","id":"corner","spec":spec})).unwrap();
            assert_eq!(inserted["document"]["deck"]["slides"][0]["elements"][0], factory);
            let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
            let reopened = execute_request(json!({"op":"open_presentation","id":"corner-native","base64":saved["base64"]})).unwrap();
            assert_eq!(reopened["document"]["parts"][0]["spec"], inserted["document"]["parts"][0]["spec"]);
            assert_eq!(reopened["document"]["parts"][0]["stale"], false);
            assert_eq!(execute_request(json!({"op":"export_presentation","document":reopened["document"]})).unwrap()["base64"], saved["base64"]);
            spec["data"]["corner_label"] = json!("\u{89b3}\u{70b9}");
            let updated = execute_request(json!({"op":"update_part","document":reopened["document"],"expected_revision":0,"slide_id":"slide","id":"corner","spec":spec})).unwrap();
            assert_eq!(updated["document"]["parts"][0]["spec"], spec);
            assert_eq!(updated["document"]["parts"][0]["stale"], false);
            let updated_saved = execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
            let updated_opened = execute_request(json!({"op":"open_presentation","id":"corner-updated","base64":updated_saved["base64"]})).unwrap();
            assert_eq!(updated_opened["document"]["parts"][0]["spec"], spec);
            assert_eq!(updated_opened["document"]["parts"][0]["stale"], false);
            let package = Package::open(STANDARD.decode(updated_saved["base64"].as_str().unwrap()).unwrap()).unwrap();
            assert!(package.text("ppt/slides/slide1.xml").unwrap().contains("<a:t>\u{89b3}\u{70b9}</a:t>"));
            let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":1,"receipt":updated["receipt"]})).unwrap();
            for field in ["deck", "parts"] { assert_eq!(undone["document"][field], reopened["document"][field], "{field}"); }
            assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], saved["base64"]);
            spec["data"]["corner_label"] = json!("");
            let cleared = execute_request(json!({"op":"update_part","document":updated_opened["document"],"expected_revision":0,"slide_id":"slide","id":"corner","spec":spec})).unwrap();
            assert!(cleared["document"]["parts"][0]["spec"]["data"].get("corner_label").is_none());
            assert_eq!(cleared["document"]["parts"][0]["stale"], false);
            let factory = execute_request(json!({"op":"create_part","id":"corner","spec":spec})).unwrap();
            assert_eq!(cleared["document"]["deck"]["slides"][0]["elements"][0], factory);
            let restored = execute_request(json!({"op":"undo_transaction","document":inserted["document"],"expected_revision":1,"receipt":inserted["receipt"]})).unwrap();
            for field in ["deck", "parts"] { assert_eq!(restored["document"][field], document[field], "{field}"); }
        }
    }
}

#[test]
fn absent_part_layout_preserves_legacy_serialization_and_native_bytes() {
    let legacy = column_spec();
    let spec: aislide_core::parts::PartSpec = serde_json::from_value(legacy.clone()).unwrap();
    #[derive(serde::Serialize)]
    struct LegacyPartSpec<'a> {
        version: u32, preset: &'a str, title: &'a str, subtitle: &'a str, data: &'a aislide_core::parts::PartData,
    }
    let old = LegacyPartSpec { version: spec.version, preset: &spec.preset, title: &spec.title, subtitle: &spec.subtitle, data: &spec.data };
    assert_eq!(serde_json::to_vec(&spec).unwrap(), serde_json::to_vec(&old).unwrap());
    let mut explicit = legacy.clone();
    explicit["layout"] = Value::Null;
    let first = execute_request(json!({"op":"create_part","id":"legacy","spec":legacy})).unwrap();
    let second = execute_request(json!({"op":"create_part","id":"legacy","spec":explicit})).unwrap();
    assert_eq!(first, second);
    use sha2::{Digest, Sha256};
    let identity = format!("{:x}", Sha256::digest(serde_json::to_vec(&legacy).unwrap()));
    assert_eq!(first["children"][0]["id"], format!("legacy-{}-0", &identity[..10]));
    let first = execute_request(json!({"op":"export","deck":deck(first)})).unwrap();
    let second = execute_request(json!({"op":"export","deck":deck(second)})).unwrap();
    assert_eq!(first["base64"], second["base64"]);
}

#[test]
fn bounded_part_layout_removes_only_headers_and_preserves_body_font_sizes() {
    let mut spec = column_spec();
    spec["preset"] = json!("pie-chart/focus");
    let legacy = execute_request(json!({"op":"create_part","id":"frame","spec":spec})).unwrap();
    spec["layout"] = json!({"x":24,"y":36,"width":1152,"height":424,"show_title":false});
    let bounded = execute_request(json!({"op":"create_part","id":"frame","spec":spec})).unwrap();
    for (field, expected) in [("x",24.0),("y",36.0),("width",1152.0),("height",424.0),("view_width",1152.0),("view_height",424.0)] {
        assert_eq!(bounded[field].as_f64().unwrap(), expected, "{field}");
    }
    let original = legacy["children"].as_array().unwrap();
    let children = bounded["children"].as_array().unwrap();
    assert_eq!(children.len(), original.len() - 2);
    for (before, after) in original[2..].iter().zip(children) {
        assert_eq!(after["type"], before["type"]);
        assert_eq!(after["x"], before["x"]);
        assert_eq!(after["y"].as_f64().unwrap(), before["y"].as_f64().unwrap() - 88.0);
        assert_eq!(after["width"], before["width"]);
        assert_eq!(after["height"], before["height"]);
        assert_eq!(after["font_size"], before["font_size"]);
        assert_ne!(after["id"], before["id"]);
    }
}

#[test]
fn parts_create_deterministic_native_column_charts_from_metadata() {
    let request = json!({"op":"create_part","id":"part-probe","spec":column_spec()});
    let first = execute_request(request.clone()).unwrap();
    assert_eq!(first, execute_request(request).unwrap());
    assert_eq!(first["type"], "group");
    let chart = first["children"].as_array().unwrap().iter().find(|element| element["type"] == "chart").unwrap();
    assert_eq!(chart["kind"], "column");
    assert_eq!(chart["series"][0]["values"], json!([12.0,24.0,18.0]));
    let exported = execute_request(json!({"op":"export","deck":deck(first)})).unwrap();
    let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert!(package.text("ppt/charts/chart1.xml").unwrap().contains("<c:barChart"));
    assert!(package.part("ppt/embeddings/chart1.xlsx").is_ok());
    assert!(!package.parts().keys().any(|path| path.starts_with("ppt/media/")));
}

#[test]
fn bounded_part_geometry_scales_without_blindly_scaling_text_or_strokes() {
    let mut spec = column_spec();
    spec["preset"] = json!("pie-chart/focus");
    let original = execute_request(json!({"op":"create_part","id":"scaled","spec":spec})).unwrap();
    spec["layout"] = json!({"x":0,"y":0,"width":576,"height":512});
    let parsed: aislide_core::parts::PartSpec = serde_json::from_value(spec.clone()).unwrap();
    assert!(parsed.layout.as_ref().unwrap().show_title);
    let scaled = execute_request(json!({"op":"create_part","id":"scaled","spec":spec})).unwrap();
    assert_eq!(scaled["view_width"], 576.0);
    for (before, after) in original["children"].as_array().unwrap().iter().zip(scaled["children"].as_array().unwrap()) {
        assert_eq!(after["x"].as_f64().unwrap(), before["x"].as_f64().unwrap() / 2.0);
        assert_eq!(after["width"].as_f64().unwrap(), before["width"].as_f64().unwrap() / 2.0);
        for field in ["y", "height", "font_size", "stroke_width", "format"] { assert_eq!(after[field], before[field], "{field}"); }
    }
    spec["layout"] = json!({"x":2000,"y":2000,"width":1152,"height":512});
    let large = execute_request(json!({"op":"create_part","id":"large","spec":spec})).unwrap();
    assert_eq!(large["x"], 2000.0);
    assert!(execute_request(json!({"op":"validate","deck":deck(large)})).is_err());
    spec["layout"] = json!({"x":0,"y":0,"width":4096,"height":4096});
    assert!(execute_request(json!({"op":"create_part","id":"maximum","spec":spec})).is_ok());
}

#[test]
fn bounded_part_layout_rejects_invalid_frames_and_body_clipping() {
    for layout in [
        json!({"x":-1,"y":0,"width":1152,"height":512}),
        json!({"x":0,"y":-1,"width":1152,"height":512}),
        json!({"x":0,"y":0,"width":0,"height":512}),
        json!({"x":0,"y":0,"width":1152,"height":0}),
        json!({"x":0,"y":0,"width":4097,"height":512}),
        json!({"x":3000,"y":0,"width":1152,"height":512}),
        json!({"x":0,"y":4000,"width":1152,"height":512}),
        json!({"x":0,"y":0,"width":1152,"height":512,"clip":true}),
        json!({"x":0,"y":0,"width":20,"height":20}),
    ] {
        let mut spec = column_spec(); spec["layout"] = layout;
        assert!(execute_request(json!({"op":"create_part","id":"invalid","spec":spec})).is_err(), "{spec}");
    }
    let mut spec = column_spec();
    spec["layout"] = json!({"x":0,"y":0,"width":1152,"height":424,"show_title":false});
    let error = execute_request(json!({"op":"create_part","id":"clipped","spec":spec})).unwrap_err();
    assert!(error.to_string().contains("title band"));
    let mut parsed: aislide_core::parts::PartSpec = serde_json::from_value(spec).unwrap();
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        parsed.layout.as_mut().unwrap().width = invalid;
        assert!(aislide_core::parts::create("nonfinite", &parsed).is_err());
    }
}

#[test]
fn bounded_custom_graph_layout_handles_both_original_title_modes() {
    let mut graph = json!({"version":1,"title":"Services","subtitle":"Synthetic","nodes":[
        {"id":"api","label":"API","detail":"Receives requests","font_size":24,"detail_font_size":16,"text_align":"left","heading_bold":false,"x":40,"y":120,"width":320,"height":160},
        {"id":"data","label":"Data","x":600,"y":120,"width":320,"height":160}
    ],"edges":[{"id":"call","source":"api","target":"data"}],"groups":[]});
    let mut spec = json!({"version":1,"preset":"diagram/custom","title":"Services","subtitle":"Synthetic","data":{"kind":"diagram","graph":graph},"layout":{"x":10,"y":20,"width":1152,"height":424,"show_title":false}});
    let element = execute_request(json!({"op":"create_part","id":"graph-frame","spec":spec})).unwrap();
    let children = element["children"].as_array().unwrap();
    assert!(!children.iter().any(|child| child["text"] == "Services" || child["text"] == "Synthetic"));
    let node = children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-n-api")).unwrap();
    assert_eq!(node["y"], 32.0);
    let heading = children.iter().find(|child| child["text"] == "API").unwrap();
    assert_eq!(heading["font_size"], 24.0);
    assert_eq!(heading["bold"], false);
    assert_eq!(heading["format"]["alignment"], "left");
    let detail = children.iter().find(|child| child["text"] == "Receives requests").unwrap();
    assert_eq!(detail["font_size"], 16.0);
    let connector = children.iter().find(|child| child["type"] == "connector").unwrap();
    assert_eq!(connector["start"]["element_id"], node["id"]);
    let mut shifted = spec.clone(); shifted["layout"]["x"] = json!(30);
    let shifted = execute_request(json!({"op":"create_part","id":"graph-frame","spec":shifted})).unwrap();
    assert_ne!(shifted["children"][0]["id"], element["children"][0]["id"]);
    graph["show_title"] = json!(false);
    graph["nodes"][0]["y"] = json!(0);
    graph["nodes"][1]["y"] = json!(400);
    graph["nodes"][1]["height"] = json!(80);
    graph["edges"] = json!([]);
    spec["data"]["graph"] = graph.clone();
    spec["layout"]["height"] = json!(512);
    let untitled = execute_request(json!({"op":"create_part","id":"untitled","spec":spec})).unwrap();
    for (id, expected) in [("api",0.0),("data",400.0)] {
        let node = untitled["children"].as_array().unwrap().iter().find(|child| child["id"].as_str().unwrap().ends_with(&format!("-n-{id}"))).unwrap();
        assert_eq!(node["y"], expected);
    }
    let mut scene = deck(json!({})); scene["slides"][0]["elements"] = json!([]);
    let document = execute_request(json!({"op":"new_document","id":"graph-metadata","deck":scene})).unwrap();
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"untitled","spec":spec})).unwrap();
    assert_eq!(inserted["document"]["parts"][0]["spec"]["data"]["graph"]["show_title"], false);
    assert_eq!(inserted["document"]["parts"][0]["spec"]["data"]["graph"]["nodes"][1]["y"], 400.0);
    assert_eq!(inserted["document"]["parts"][0]["stale"], false);
    spec["layout"]["show_title"] = json!(true);
    let still_untitled = execute_request(json!({"op":"create_part","id":"reserved-band","spec":spec})).unwrap();
    assert!(!still_untitled["children"].as_array().unwrap().iter().any(|child| child["text"] == "Services" || child["text"] == "Synthetic"));
    spec["data"]["graph"]["show_title"] = json!(true);
    assert!(execute_request(json!({"op":"create_part","id":"reserved-band","spec":spec})).is_err());
}

#[test]
fn bounded_parts_preserve_metadata_sources_markers_and_frames_through_update_undo_and_native_reopen() {
    let source = execute_request(json!({"op":"ingest","input":{"format":"csv","name":"selected.csv","base64":STANDARD.encode(b"Label,Value\nSelected,12\n")}})).unwrap();
    let marker = json!({"id":"source-marker","type":"text","x":10,"y":10,"width":120,"height":30,"text":"12","font_size":18,"color":"202426","bold":false});
    let binding = json!({"slide_id":"slide","element_id":"source-marker","field":"/text","source_id":source["id"],"source_sha256":source["sha256"],"locator":"record:2/column:2","raw_value":"12","value":"12","transform":"display_scalar","stale":false});
    let document = aislide_core::document::create("bounded-state".into(), serde_json::from_value(deck(marker)).unwrap(), vec![serde_json::from_value(source).unwrap()], vec![serde_json::from_value(binding).unwrap()], None).unwrap();
    let document = serde_json::to_value(document).unwrap();
    let mut spec = column_spec(); spec["layout"] = json!({"x":40,"y":100,"width":576,"height":512});
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"bounded","spec":spec})).unwrap();
    let factory = execute_request(json!({"op":"create_part","id":"bounded","spec":spec})).unwrap();
    assert_eq!(inserted["document"]["deck"]["slides"][0]["elements"][1], factory);
    let moved = execute_request(json!({"op":"transaction","document":inserted["document"],"transaction":{"expected_revision":1,"expected_hash":inserted["document"]["hash"],"operations":[
        {"op":"replace","path":"/deck/slides/0/elements/1/x","value":90},
        {"op":"replace","path":"/deck/slides/0/elements/1/y","value":120},
        {"op":"replace","path":"/deck/slides/0/elements/1/width","value":768}
    ]}})).unwrap();
    assert_eq!(moved["document"]["parts"][0]["stale"], false);
    spec["title"] = json!("Updated"); spec["data"]["series"][0]["values"][0] = json!(44);
    let updated = execute_request(json!({"op":"update_part","document":moved["document"],"expected_revision":2,"slide_id":"slide","id":"bounded","spec":spec})).unwrap();
    let stored = &updated["document"]["parts"][0]["spec"];
    assert_eq!(stored["layout"]["x"], 90.0);
    assert_eq!(stored["layout"]["y"], 120.0);
    assert_eq!(stored["layout"]["width"], 768.0);
    assert_eq!(updated["document"]["parts"][0]["stale"], false);
    let regenerated = execute_request(json!({"op":"create_part","id":"bounded","spec":stored})).unwrap();
    assert_eq!(updated["document"]["deck"]["slides"][0]["elements"][1], regenerated);
    for field in ["sources", "bindings"] { assert_eq!(updated["document"][field], document[field], "{field}"); }
    assert_eq!(updated["document"]["deck"]["slides"][0]["elements"][0], document["deck"]["slides"][0]["elements"][0]);
    let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":3,"receipt":updated["receipt"]})).unwrap();
    for field in ["deck", "parts", "sources", "bindings"] { assert_eq!(undone["document"][field], moved["document"][field], "{field}"); }
    spec = stored.clone(); spec["layout"]["x"] = json!(250); spec["layout"]["width"] = json!(576);
    let reframed = execute_request(json!({"op":"update_part","document":updated["document"],"expected_revision":3,"slide_id":"slide","id":"bounded","spec":spec})).unwrap();
    let factory = execute_request(json!({"op":"create_part","id":"bounded","spec":spec})).unwrap();
    assert_eq!(reframed["document"]["deck"]["slides"][0]["elements"][1], factory);
    let mut invalid = spec.clone(); invalid["layout"]["height"] = json!(1);
    assert!(execute_request(json!({"op":"update_part","document":reframed["document"],"expected_revision":4,"slide_id":"slide","id":"bounded","spec":invalid})).is_err());
    let saved = execute_request(json!({"op":"export_presentation","document":reframed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"bounded-native","base64":saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["parts"][0]["stale"], false);
    assert_eq!(reopened["document"]["parts"][0]["spec"], reframed["document"]["parts"][0]["spec"]);
    for field in ["sources", "bindings"] { assert_eq!(reopened["document"][field], document[field], "{field}"); }
    assert_eq!(reopened["document"]["deck"]["slides"][0]["elements"][0]["text"], "12");
    assert_eq!(execute_request(json!({"op":"export_presentation","document":reopened["document"]})).unwrap()["base64"], saved["base64"]);
    spec["title"] = json!("Native edit");
    let native_update = execute_request(json!({"op":"update_part","document":reopened["document"],"expected_revision":0,"slide_id":"slide","id":"bounded","spec":spec})).unwrap();
    assert_eq!(native_update["document"]["parts"][0]["stale"], false);
    assert_eq!(native_update["document"]["bindings"], document["bindings"]);
    let restored = execute_request(json!({"op":"undo_transaction","document":native_update["document"],"expected_revision":1,"receipt":native_update["receipt"]})).unwrap();
    assert_eq!(restored["document"]["deck"], reopened["document"]["deck"]);
}

#[test]
fn explicit_part_layout_bypasses_automatic_visual_region_resizing() {
    let mut design = aislide_core::design::Design::default();
    design.layouts[0].id = "preset-visual-content".into();
    design.layouts[0].elements.push(serde_json::from_value(json!({"id":"preset-visual-region","type":"rect","x":100,"y":100,"width":700,"height":400,"fill":"FFFFFF"})).unwrap());
    let mut scene = deck(json!({})); scene["slides"][0]["elements"] = json!([]);
    scene["design"] = serde_json::to_value(design).unwrap();
    scene["slides"][0]["layout_id"] = json!("preset-visual-content");
    let document = execute_request(json!({"op":"new_document","id":"explicit-region","deck":scene})).unwrap();
    let mut spec = column_spec(); spec["preset"] = json!("pie-chart/focus");
    spec["layout"] = json!({"x":24,"y":36,"width":1152,"height":424,"show_title":false});
    let factory = execute_request(json!({"op":"create_part","id":"explicit","spec":spec})).unwrap();
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"explicit","spec":spec})).unwrap();
    assert_eq!(inserted["document"]["deck"]["slides"][0]["elements"][0], factory);
    assert_eq!(inserted["document"]["parts"][0]["stale"], false);
    let updated = execute_request(json!({"op":"update_part","document":inserted["document"],"expected_revision":1,"slide_id":"slide","id":"explicit","spec":spec})).unwrap();
    assert_eq!(updated["document"]["deck"], inserted["document"]["deck"]);
}

#[test]
fn editable_polygons_and_transparent_shapes_round_trip_as_native_geometry() {
    let polygon = json!({"id":"region","type":"polygon","x":120,"y":150,"width":400,"height":300,"points":[[0,0.5],[0.4,0],[1,0.3],[0.8,1]],"fill":"@accent2","stroke":"@dk1","stroke_width":1});
    let mut scene = deck(polygon.clone());
    let mut outline = execute_request(json!({"op":"create_object","id":"outline","kind":"shape","preset":"ellipse"})).unwrap();
    outline["fill"] = json!("none");
    scene["slides"][0]["elements"].as_array_mut().unwrap().push(outline);
    let exported = execute_request(json!({"op":"export","deck":scene})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"polygon","base64":exported["base64"]})).unwrap();
    assert_eq!(opened["document"]["deck"]["slides"][0]["elements"][0]["points"], json!([[0.0,0.5],[0.4,0.0],[1.0,0.3],[0.8,1.0]]));
    assert_eq!(opened["document"]["deck"]["slides"][0]["elements"][1]["fill"], "none");
    let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert!(package.text("ppt/slides/slide1.xml").unwrap().contains("<a:custGeom>"));
    let mut invalid = polygon;
    invalid["points"][0][0] = json!(-1);
    assert!(execute_request(json!({"op":"export","deck":deck(invalid)})).is_err());
}

#[test]
fn every_category_has_three_distinct_bounded_native_presets() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    let presets: Vec<_> = catalog["presets"].as_array().unwrap().iter().filter(|preset| preset["recommended"] != true).collect();
    assert_eq!(presets.len(), 108);
    let mut categories = std::collections::BTreeMap::<String, Vec<Value>>::new();
    let mut failures = Vec::new();
    for preset in presets {
        let element = match execute_request(json!({"op":"create_part","id":"example","spec":preset["example"]})) { Ok(element) => element, Err(error) => { failures.push(format!("{}: {error}",preset["id"])); continue; } };
        let scene = deck(element.clone());
        execute_request(json!({"op":"validate","deck":scene})).unwrap();
        let measured = execute_request(json!({"op":"measure_layout","deck":scene})).unwrap();
        let errors: Vec<_> = measured["issues"].as_array().unwrap().iter().filter(|issue| issue["severity"] == "error").collect();
        if !errors.is_empty() { failures.push(format!("{}: {errors:?}",preset["id"])); }
        let exported = execute_request(json!({"op":"export","deck":scene})).unwrap();
        let opened = execute_request(json!({"op":"open_presentation","id":"preset","base64":exported["base64"]})).unwrap();
        assert_eq!(opened["document"]["deck"]["slides"][0]["elements"][0]["type"], "group", "{}", preset["id"]);
        assert_eq!(opened["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().len(), element["children"].as_array().unwrap().len(), "{}", preset["id"]);
        let layouts = categories.entry(preset["category"].as_str().unwrap().into()).or_default();
        if layouts.contains(&element) { failures.push(format!("duplicate recipe: {}",preset["id"])); }
        layouts.push(element);
    }
    assert!(failures.is_empty(), "{}",failures.join("\n"));
    assert_eq!(categories.len(), 36);
    assert!(categories.values().all(|presets| presets.len() == 3));
}

#[test]
fn part_data_is_validated_before_geometry_is_generated() {
    let mut invalid = column_spec();
    invalid["data"]["series"][0]["values"] = json!([1]);
    assert!(execute_request(json!({"op":"create_part","id":"bad","spec":invalid})).is_err());
    for category in ["100-add-vertical-bar-graph", "pie-chart"] {
        let mut invalid = column_spec(); invalid["preset"] = json!(format!("{category}/balanced")); invalid["data"]["series"][0]["values"] = json!([0,-1,1]);
        assert!(execute_request(json!({"op":"create_part","id":"bad","spec":invalid})).is_err());
    }
    for input in [
        json!({"kind":"items","items":[{"label":"TAM","value":10},{"label":"SAM","value":20},{"label":"SOM","value":5}]}),
        json!({"kind":"items","items":[{"label":"TAM","value":0},{"label":"SAM","value":0},{"label":"SOM","value":0}]})
    ] {
        assert!(execute_request(json!({"op":"create_part","id":"bad","spec":{"version":1,"preset":"grow/balanced","title":"Markets","data":input}})).is_err());
    }
}

#[test]
fn all_catalog_parts_keep_text_frames_separate_and_inside_their_canvas() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    let mut failures = Vec::new();
    for preset in catalog["presets"].as_array().unwrap() {
        let element = execute_request(json!({"op":"create_part","id":"geometry","spec":preset["example"]})).unwrap();
        let text: Vec<_> = element["children"].as_array().unwrap().iter().filter(|child| child["type"] == "text" && !child["text"].as_str().unwrap().is_empty()).collect();
        for (index, first) in text.iter().enumerate() {
            let rectangle = |value:&Value| [value["x"].as_f64().unwrap(),value["y"].as_f64().unwrap(),value["width"].as_f64().unwrap(),value["height"].as_f64().unwrap()];
            let [left, top, width, height] = rectangle(first);
            if left < 0.0 || top < 0.0 || left + width > 1152.5 || top + height > 512.5 { failures.push(format!("{}: text outside canvas: {}",preset["id"],first["text"])); }
            for second in &text[index + 1..] {
                let [other_left, other_top, other_width, other_height] = rectangle(second);
                if left < other_left + other_width - 0.5 && other_left < left + width - 0.5 && top < other_top + other_height - 0.5 && other_top < top + height - 0.5 {
                    failures.push(format!("{}: {} overlaps {}",preset["id"],first["text"],second["text"]));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn metadata_parts_are_revisioned_undoable_and_do_not_override_external_xml() {
    let mut scene=deck(json!({"id":"label","type":"text","x":64,"y":20,"width":900,"height":60,"text":"Parts","font_size":28,"color":"202426","bold":true}));
    scene["slides"][0]["elements"]=json!([]);
    let document=execute_request(json!({"op":"new_document","id":"parts","deck":scene})).unwrap();
    let inserted=execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"metric-part","spec":column_spec()})).unwrap();
    assert_eq!(inserted["document"]["revision"],1);
    assert_eq!(inserted["document"]["parts"][0]["stale"],false);
    let mut spec=column_spec(); spec["title"]=json!("Updated volume");spec["data"]["series"][0]["values"][0]=json!(44);
    let updated=execute_request(json!({"op":"update_part","document":inserted["document"],"expected_revision":1,"slide_id":"slide","id":"metric-part","spec":spec})).unwrap();
    assert_eq!(updated["document"]["parts"][0]["spec"]["title"],"Updated volume");
    let undone=execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":2,"receipt":updated["receipt"]})).unwrap();
    assert_eq!(undone["document"]["deck"],inserted["document"]["deck"]);
    let saved=execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"reopen","base64":saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["parts"][0]["stale"],false);
    assert_eq!(execute_request(json!({"op":"export_presentation","document":reopened["document"]})).unwrap()["base64"],saved["base64"]);
    let mut next_spec=spec.clone();next_spec["title"]=json!("Reopened edit");
    execute_request(json!({"op":"update_part","document":reopened["document"],"expected_revision":0,"slide_id":"slide","id":"metric-part","spec":next_spec})).unwrap();
    let mut package=Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml=package.text("ppt/slides/slide1.xml").unwrap().replace("Updated volume","Edited outside AISlide");
    package.replace_part("ppt/slides/slide1.xml",xml.into_bytes()).unwrap();
    let external=execute_request(json!({"op":"open_presentation","id":"external","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    assert_eq!(external["document"]["parts"][0]["stale"],true);
    let result=execute_request(json!({"op":"update_part","document":external["document"],"expected_revision":0,"slide_id":"slide","id":"metric-part","spec":spec}));
    assert!(result.unwrap_err().to_string().contains("stale"));
    assert!(external["document"]["deck"].to_string().contains("Edited outside AISlide"));
}

#[test]
fn long_part_labels_are_fitted_or_rejected_before_insertion() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    let mut spec=catalog["presets"].as_array().unwrap().iter().find(|preset|preset["id"]=="flow/balanced").unwrap()["example"].clone();
    spec["data"]["items"]=json!((0..6).map(|_|json!({"label":"A long label that must fit within each stage","detail":""})).collect::<Vec<_>>());
    let element=execute_request(json!({"op":"create_part","id":"long","spec":spec})).unwrap();
    let measured=execute_request(json!({"op":"measure_layout","deck":deck(element)})).unwrap();
    assert!(measured["issues"].as_array().unwrap().iter().all(|issue|issue["severity"]!="error"),"{}",measured["issues"]);
    let mut invalid=column_spec();
    invalid["title"]=json!("W".repeat(80));
    let result=execute_request(json!({"op":"create_part","id":"fit","spec":invalid}));
    if let Ok(element)=result {let measured=execute_request(json!({"op":"measure_layout","deck":deck(element)})).unwrap();assert!(measured["issues"].as_array().unwrap().iter().all(|issue|issue["severity"]!="error"));}
}

#[test]
fn process_diagrams_use_filled_geometry_with_safe_separate_labels() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    for preset in catalog["presets"].as_array().unwrap().iter().filter(|preset| matches!(preset["category"].as_str(), Some("flow" | "vertical-flow" | "tree" | "cycle"))) {
        let element = execute_request(json!({"op":"create_part","id":"visual","spec":preset["example"]})).unwrap();
        let children = element["children"].as_array().unwrap();
        let labels = preset["example"]["data"]["items"].as_array().or_else(|| preset["example"]["data"]["nodes"].as_array()).unwrap();
        for item in labels {
            let label = children.iter().find(|child| child["type"] == "text" && child["text"] == item["label"]).unwrap();
            let left = label["x"].as_f64().unwrap(); let top = label["y"].as_f64().unwrap();
            let right = left + label["width"].as_f64().unwrap(); let bottom = top + label["height"].as_f64().unwrap();
            let contained = children.iter().any(|shape| {
                if !matches!(shape["type"].as_str(), Some("rect" | "polygon" | "shape")) || matches!(shape["fill"].as_str(), None | Some("none" | "@lt1" | "FFFFFF")) { return false; }
                let shape_left = shape["x"].as_f64().unwrap(); let shape_top = shape["y"].as_f64().unwrap();
                let width = shape["width"].as_f64().unwrap(); let height = shape["height"].as_f64().unwrap();
                let corners = [[left, top], [right, top], [right, bottom], [left, bottom]];
                if !corners.iter().all(|point| point[0] > shape_left && point[0] < shape_left + width && point[1] > shape_top && point[1] < shape_top + height) { return false; }
                if shape["preset"] == "chevron" { return left > shape_left + width.min(height) / 2.0 && right < shape_left + width - width.min(height) / 2.0; }
                if shape["type"] != "polygon" { return true; }
                let points = shape["points"].as_array().unwrap();
                corners.iter().all(|point| {
                    let horizontal = (point[0] - shape_left) / width; let vertical = (point[1] - shape_top) / height;
                    let mut inside = false;
                    for index in 0..points.len() {
                        let start = &points[index]; let end = &points[(index + 1) % points.len()];
                        let start_x = start[0].as_f64().unwrap(); let start_y = start[1].as_f64().unwrap();
                        let end_x = end[0].as_f64().unwrap(); let end_y = end[1].as_f64().unwrap();
                        if (start_y > vertical) != (end_y > vertical) && horizontal < (end_x - start_x) * (vertical - start_y) / (end_y - start_y) + start_x { inside = !inside; }
                    }
                    inside
                })
            });
            assert!(contained, "{}: label {} must fit inside its filled node, away from arrow notches", preset["id"], item["label"]);
        }
        let text: Vec<_> = children.iter().filter(|child| child["type"] == "text" && !child["text"].as_str().unwrap().is_empty()).collect();
        for (index, first) in text.iter().enumerate() {
            for second in &text[index + 1..] {
                let overlaps = first["x"].as_f64().unwrap() < second["x"].as_f64().unwrap() + second["width"].as_f64().unwrap() - 0.5
                    && second["x"].as_f64().unwrap() < first["x"].as_f64().unwrap() + first["width"].as_f64().unwrap() - 0.5
                    && first["y"].as_f64().unwrap() < second["y"].as_f64().unwrap() + second["height"].as_f64().unwrap() - 0.5
                    && second["y"].as_f64().unwrap() < first["y"].as_f64().unwrap() + first["height"].as_f64().unwrap() - 0.5;
                assert!(!overlaps, "{}: text frames {} and {} overlap", preset["id"], first["text"], second["text"]);
            }
        }
    }
}

#[test]
fn deleting_parts_removes_metadata_and_undo_restores_both() {
    let mut scene=deck(json!({}));scene["slides"][0]["elements"]=json!([]);
    let document=execute_request(json!({"op":"new_document","id":"delete-part","deck":scene})).unwrap();
    let inserted=execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"part","spec":column_spec()})).unwrap();
    let saved=execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"delete-reopened","base64":saved["base64"]})).unwrap();
    for document in [inserted["document"].clone(),reopened["document"].clone()] {
        let removed=execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"remove","path":"/deck/slides/0/elements/0"}]}})).unwrap();
        assert!(removed["document"]["parts"].as_array().is_none_or(|parts|parts.is_empty()));
        let undone=execute_request(json!({"op":"undo_transaction","document":removed["document"],"expected_revision":removed["document"]["revision"],"receipt":removed["receipt"]})).unwrap();
        assert_eq!(undone["document"]["parts"],document["parts"]);
        assert_eq!(undone["document"]["deck"],document["deck"]);
        let exported=execute_request(json!({"op":"export_presentation","document":removed["document"]})).unwrap();
        let empty=execute_request(json!({"op":"open_presentation","id":"empty","base64":exported["base64"]})).unwrap();
        assert!(empty["document"]["parts"].as_array().is_none_or(|parts|parts.is_empty()));
        if document["origin"].is_null() {
            let replaced=execute_request(json!({"op":"insert_part","document":removed["document"],"expected_revision":removed["document"]["revision"],"slide_id":"slide","id":"part","spec":column_spec()})).unwrap();
            assert_eq!(replaced["document"]["parts"][0]["stale"],false);
        }
    }
}

#[test]
fn process_diagrams_handle_boundary_counts_and_reject_unreadable_trees() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    for category in ["flow","vertical-flow","cycle","tree"] {
        for variant in ["balanced","focus","labeled"] {
            let id=format!("{category}/{variant}");
            let example=&catalog["presets"].as_array().unwrap().iter().find(|preset|preset["id"]==id).unwrap()["example"];
            for count in if category=="tree" {vec![2,8]} else if category=="vertical-flow" {vec![2,5]} else if category=="cycle" {vec![3,6]} else {vec![2,6]} {
                let mut spec=example.clone();
                spec["title"]=json!("日本語の境界条件");
                if category=="tree" {spec["data"]=json!({"kind":"tree","nodes":(0..count).map(|index|json!({"id":format!("node-{index}"),"label":format!("項目{index}"),"parent":if index==0 {None} else {Some("node-0")}})).collect::<Vec<_>>()});}
                else {spec["data"]=json!({"kind":"items","center":"共通目的","items":(0..count).map(|index|json!({"label":format!("工程{index}"),"detail":"担当が確認"})).collect::<Vec<_>>()});}
                let element=execute_request(json!({"op":"create_part","id":"boundary","spec":spec})).unwrap_or_else(|error|panic!("{id} / {count}: {error}"));
                let measured=execute_request(json!({"op":"measure_layout","deck":deck(element.clone())})).unwrap();
                assert!(measured["issues"].as_array().unwrap().iter().all(|issue|issue["severity"]!="error"),"{id} / {count}: {measured}");
                let text:Vec<_>=element["children"].as_array().unwrap().iter().filter(|child|child["type"]=="text" && child["text"].as_str().is_some_and(|text|text.starts_with("項目") || text.starts_with("工程"))).collect();
                for (index,first) in text.iter().enumerate() {for second in &text[index+1..] {
                    let separated=first["x"].as_f64().unwrap()+first["width"].as_f64().unwrap()<=second["x"].as_f64().unwrap()
                        || second["x"].as_f64().unwrap()+second["width"].as_f64().unwrap()<=first["x"].as_f64().unwrap()
                        || first["y"].as_f64().unwrap()+first["height"].as_f64().unwrap()<=second["y"].as_f64().unwrap()
                        || second["y"].as_f64().unwrap()+second["height"].as_f64().unwrap()<=first["y"].as_f64().unwrap();
                    assert!(separated,"{id} / {count}: labels overlap");
                }}
            }
        }
    }
    let dense=json!({"version":1,"preset":"tree/focus","title":"Dense tree","data":{"kind":"tree","nodes":(0..12).map(|index|json!({"id":format!("node-{index}"),"label":"Review ownership","parent":if index==0 {None} else {Some("node-0")}})).collect::<Vec<_>>()}});
    assert!(execute_request(json!({"op":"create_part","id":"dense","spec":dense})).unwrap_err().to_string().contains("sibling branches"));
}

#[test]
fn large_waterfall_totals_cannot_hide_material_reconciliation_errors() {
    let spec=json!({"version":1,"preset":"water-fall/balanced","title":"Totals","data":{"kind":"waterfall","steps":[{"label":"Start","value":1e15,"total":true},{"label":"Finish","value":1e15-100.0,"total":true}]}});
    assert!(execute_request(json!({"op":"create_part","id":"bad-total","spec":spec})).is_err());
}

#[test]
fn external_part_extensions_and_workbook_changes_remain_stale_after_save() {
    let mut scene=deck(json!({"id":"unused","type":"rect","x":0,"y":0,"width":1,"height":1,"fill":"FFFFFF"}));scene["slides"][0]["elements"]=json!([]);
    let document=execute_request(json!({"op":"new_document","id":"native-guards","deck":scene})).unwrap();
    let inserted=execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"part","spec":column_spec()})).unwrap();
    let exported=execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let bytes=STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap();
    for case in ["extension","workbook","missing-fingerprint"] {
        let mut package=Package::open(bytes.clone()).unwrap();
        if case=="workbook" {
            let mut workbook=Package::open(package.part("ppt/embeddings/chart1.xlsx").unwrap().to_vec()).unwrap();
            let sheet=workbook.text("xl/worksheets/sheet1.xml").unwrap().replace("<v>12</v>","<v>999</v>");
            workbook.replace_part("xl/worksheets/sheet1.xml",sheet.into_bytes()).unwrap();package.replace_part("ppt/embeddings/chart1.xlsx",workbook.save().unwrap()).unwrap();
        } else {
            let xml=package.text("ppt/slides/slide1.xml").unwrap().replace("</p:grpSp>","<p:extLst><p:ext uri=\"urn:foreign\"><v:foreign xmlns:v=\"urn:vendor\"/></p:ext></p:extLst></p:grpSp>");
            package.replace_part("ppt/slides/slide1.xml",xml.into_bytes()).unwrap();
            if case=="missing-fingerprint" {
                let path=package.parts().keys().find(|path|path.starts_with("customXml/aislide-provenance")).unwrap().clone();let mut xml=package.text(&path).unwrap().to_owned();let parsed=roxmltree::Document::parse(&xml).unwrap();let range=parsed.descendants().find(|node|node.attribute("name")==Some("native_sha256")).unwrap().range();xml.replace_range(range,"");package.replace_part(&path,xml.into_bytes()).unwrap();
            }
        }
        let external_bytes=package.save().unwrap();
        let opened=execute_request(json!({"op":"open_presentation","id":"outside","base64":STANDARD.encode(&external_bytes)})).unwrap();
        assert_eq!(opened["document"]["parts"][0]["stale"],true,"{case}");
        let saved=execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap();
        assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(),external_bytes,"{case}");
        let reopened=execute_request(json!({"op":"open_presentation","id":"again","base64":saved["base64"]})).unwrap();
        assert_eq!(reopened["document"]["parts"][0]["stale"],true,"{case}");
        assert!(execute_request(json!({"op":"update_part","document":reopened["document"],"expected_revision":0,"slide_id":"slide","id":"part","spec":column_spec()})).is_err());
    }
}

#[test]
fn percentage_focus_uses_share_and_equal_ranking_values_share_rank() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    let presets=catalog["presets"].as_array().unwrap();
    let mut spec=presets.iter().find(|preset|preset["id"]=="100-add-vertical-bar-graph/focus").unwrap()["example"].clone();
    spec["data"]["categories"]=json!(["A","B"]);spec["data"]["series"]=json!([{"name":"Primary","values":[25,25]},{"name":"Secondary","values":[75,75]}]);
    let element=execute_request(json!({"op":"create_part","id":"share","spec":spec})).unwrap();
    assert!(element["children"].as_array().unwrap().iter().any(|element|element["text"]=="25%"));
    let mut rank=presets.iter().find(|preset|preset["id"]=="ranking/focus").unwrap()["example"].clone();
    rank["data"]["items"][1]["value"]=rank["data"]["items"][0]["value"].clone();
    let element=execute_request(json!({"op":"create_part","id":"rank","spec":rank})).unwrap();
    assert_eq!(element["children"].as_array().unwrap().iter().filter(|element|element["text"]=="#01").count(),2);
}

#[test]
fn three_set_venn_labels_do_not_cross_circle_outlines() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    let spec=&catalog["presets"].as_array().unwrap().iter().find(|preset|preset["id"]=="venn/focus").unwrap()["example"];
    let element=execute_request(json!({"op":"create_part","id":"venn","spec":spec})).unwrap();
    let children=element["children"].as_array().unwrap();
    for text in children.iter().filter(|child|child["type"]=="text" && child["y"].as_f64().unwrap()>=80.0) {
        let left=text["x"].as_f64().unwrap();let right=left+text["width"].as_f64().unwrap();
        let top=text["y"].as_f64().unwrap();let bottom=top+text["height"].as_f64().unwrap();
        for circle in children.iter().filter(|child|child["preset"]=="ellipse") {
            let radius_x=circle["width"].as_f64().unwrap()/2.0;let radius_y=circle["height"].as_f64().unwrap()/2.0;
            let center_x=circle["x"].as_f64().unwrap()+radius_x;let center_y=circle["y"].as_f64().unwrap()+radius_y;
            let distance=|horizontal:f64,vertical:f64|((horizontal-center_x)/radius_x).powi(2)+((vertical-center_y)/radius_y).powi(2);
            let nearest=distance(center_x.clamp(left,right),center_y.clamp(top,bottom));
            let furthest=[distance(left,top),distance(left,bottom),distance(right,top),distance(right,bottom)].into_iter().fold(0.0,f64::max);
            assert!(nearest>1.0 || furthest<1.0,"circle outline crosses {}",text["text"]);
        }
    }
    let mut japanese=spec.clone();japanese["data"]["center"]=json!("共通の目的");
    let element=execute_request(json!({"op":"create_part","id":"venn-ja","spec":japanese})).unwrap();
    let label=element["children"].as_array().unwrap().iter().find(|child|child["text"]=="共通の目的").unwrap();
    assert!(label["width"].as_f64().unwrap()>=5.0*label["font_size"].as_f64().unwrap()+8.0,"Japanese center label should not leave a one-character line");
}