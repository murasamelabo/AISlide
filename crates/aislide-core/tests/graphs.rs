use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn graph() -> Value {
    json!({"version":1,"title":"Service topology","subtitle":"Synthetic example","nodes":[
        {"id":"client","label":"Client","kind":"rectangle","x":48,"y":160,"width":200,"height":96},
        {"id":"api","label":"API","kind":"rounded_rectangle","x":560,"y":256,"width":220,"height":112}
    ],"edges":[{"id":"request","source":"client","target":"api","label":"HTTPS","source_port":"right","target_port":"left","route":"straight"}],"groups":[]})
}

fn feedback_graph() -> Value {
    json!({"version":1,"title":"ISOC とデータの流れ（グラフ機能の検証）","subtitle":"","show_title":false,
        "groups":[
            {"id":"portal","label":"Microsoft Defender ポータル","x":24,"y":16,"width":620,"height":300,"padding":16,"header_height":44,"header_font_size":18,"fill":"EAF3FB","stroke":"0B6FC9"},
            {"id":"azure","label":"Azure（サブスクリプション）","x":700,"y":16,"width":428,"height":300,"padding":16,"header_height":44,"header_font_size":18,"fill":"E6F5F2","stroke":"0E8C7F"}
        ],"nodes":[
            {"id":"xdr","label":"Defender XDR","detail":"インシデント・アラート","kind":"rounded_rectangle","x":48,"y":84,"width":220,"height":84,"group":"portal"},
            {"id":"isoc","label":"ISOC","detail":"ケース・自動化・ワークブック","kind":"rounded_rectangle","x":380,"y":84,"width":240,"height":84,"group":"portal","fill":"FFFFFF","stroke":"0F2A44"},
            {"id":"pb","label":"プレイブック生成","x":380,"y":216,"width":240,"height":72,"group":"portal"},
            {"id":"ws","label":"ワークスペース","detail":"UEBA・TI・コネクタ","kind":"cylinder","x":724,"y":84,"width":180,"height":96,"group":"azure"},
            {"id":"lake","label":"データレイク","detail":"未確認","kind":"cylinder","x":930,"y":84,"width":180,"height":96,"group":"azure","stroke":"D9730D"},
            {"id":"src","label":"サードパーティ / Azure ログ","x":724,"y":224,"width":386,"height":64,"group":"azure"},
            {"id":"analyst","label":"SOC アナリスト","kind":"ellipse","x":48,"y":400,"width":220,"height":72}
        ],"edges":[
            {"id":"e1","source":"xdr","target":"isoc","label":"相関済みインシデント","label_placement":{"position":0.5,"side":"above","offset":8},"badge":{"number":1}},
            {"id":"e2","source":"isoc","target":"pb","source_port":"bottom","target_port":"top","source_offset":0.25,"target_offset":0.25,"label":"対応手順","label_placement":{"position":0.5,"side":"below","offset":6},"badge":{"number":2,"position":0.4}},
            {"id":"e3","source":"src","target":"ws","source_port":"top","target_port":"bottom","route":"elbow","label":"取り込み（課金）","color":"0E8C7F","badge":{"number":3,"fill":"FFFFFF"}},
            {"id":"e4","source":"ws","target":"isoc","source_port":"left","target_port":"right","route":"manual","waypoints":[[670,132],[670,126]],"label":"エンリッチ","label_placement":{"position":0.85,"side":"above","offset":8},"badge":{"number":4,"position":0.3}},
            {"id":"e5","source":"ws","target":"lake","source_port":"right","target_port":"left","label":"未確認","dashed":true,"color":"D9730D","label_color":"D9730D","label_font_size":13},
            {"id":"e6","source":"pb","target":"analyst","source_port":"left","target_port":"right","route":"manual","waypoints":[[320,252],[320,436]],"label":"承認して実行","stroke_width":3,"label_placement":{"position":0.8,"side":"above","offset":8},"badge":{"number":5,"position":0.35}}
        ]})
}

fn graph_child<'a>(rendered: &'a Value, suffix: &str) -> &'a Value {
    rendered["children"].as_array().unwrap().iter().find(|child| child["id"].as_str().unwrap().ends_with(suffix)).unwrap()
}

#[test]
fn feedback_explicit_overlap_policy_preserves_default_and_rejects_strict_collision() {
    let mut spec = feedback_graph();
    let original = execute_request(json!({"op":"create_graph","id":"feedback","spec":spec})).unwrap();
    let label = graph_child(&original, "-et-e1");
    for (field, expected) in [("x",240.0),("y",90.0),("width",168.0),("height",28.0)] { assert_eq!(label[field], expected); }
    spec["edges"][0]["label_placement"]["on_overlap"] = json!("warn");
    assert_eq!(execute_request(json!({"op":"create_graph","id":"feedback","spec":spec})).unwrap(), original);
    let typed: aislide_core::graphs::GraphSpec = serde_json::from_value(spec.clone()).unwrap();
    assert_eq!(typed.edges[0].label_placement.as_ref().unwrap().on_overlap, aislide_core::graphs::GraphLabelOverlap::Warn);
    assert!(serde_json::to_value(typed).unwrap()["edges"][0]["label_placement"].get("on_overlap").is_none());
    for value in [json!("ignore"), Value::Null, json!(true), json!({})] {
        let mut invalid = spec.clone(); invalid["edges"][0]["label_placement"]["on_overlap"] = value;
        assert!(serde_json::from_value::<aislide_core::graphs::GraphSpec>(invalid).is_err());
    }
    spec["edges"][0]["label_placement"]["on_overlap"] = json!("error");
    aislide_core::graphs::validate(&serde_json::from_value(spec.clone()).unwrap()).unwrap();
    let error = execute_request(json!({"op":"create_graph","id":"feedback","spec":spec})).unwrap_err().to_string();
    for expected in ["e1", "-n-xdr", "-n-isoc", "-eb-e1", "offset"] { assert!(error.contains(expected), "missing {expected}: {error}"); }
}

#[test]
fn feedback_strict_labels_accept_clear_frames_and_report_headers_and_prior_labels() {
    let mut spec = graph();
    spec["nodes"][1]["y"] = json!(160); spec["nodes"][1]["height"] = json!(96);
    spec["edges"][0]["label_placement"] = json!({"position":0.5,"side":"above","offset":8,"on_overlap":"error"});
    let strict = execute_request(json!({"op":"create_graph","id":"clear-strict","spec":spec})).unwrap();
    assert_eq!(graph_rect(graph_child(&strict, "-et-request")), [377.5,172.0,53.0,28.0]);
    let document = execute_request(json!({"op":"create_presentation","id":"strict-history","title":"Strict labels"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide-1","id":"flow","spec":spec})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"strict-open","base64":saved["base64"]})).unwrap();
    assert_eq!(opened["document"]["parts"][0]["stale"], false);
    assert_eq!(opened["document"]["parts"][0]["spec"]["data"]["graph"]["edges"][0]["label_placement"]["on_overlap"], "error");
    let mut repeated = spec.clone();
    let mut second = spec["edges"][0].clone(); second["id"] = json!("second");
    repeated["edges"].as_array_mut().unwrap().push(second);
    let error = execute_request(json!({"op":"create_graph","id":"prior-label","spec":repeated})).unwrap_err().to_string();
    assert!(error.contains("edge 'request'") && error.contains("-et-second"), "{error}");
    spec["groups"] = json!([{"id":"boundary","label":"Boundary","x":0,"y":88,"width":1000,"height":400}]);
    spec["edges"][0]["label_placement"]["offset"] = json!(80);
    let error = execute_request(json!({"op":"create_graph","id":"header-collision","spec":spec})).unwrap_err().to_string();
    assert!(error.contains("request") && error.contains("-gt-boundary"), "{error}");
}

#[test]
fn feedback_strict_icon_labels_ignore_transparent_anchors_but_check_visible_text() {
    let mut spec = graph();
    spec["nodes"][0]["height"] = json!(160); spec["nodes"][0]["font_size"] = json!(12);
    spec["nodes"][0]["presentation"] = json!("icon"); spec["nodes"][0]["icon"] = node_icon("#007a4d");
    spec["nodes"][1]["height"] = json!(160); spec["nodes"][1]["y"] = json!(160);
    spec["edges"][0]["label_placement"] = json!({"position":0,"side":"above","offset":8,"on_overlap":"error"});
    let rendered = execute_request(json!({"op":"create_graph","id":"icon-clear","spec":spec})).unwrap();
    assert!(rects_overlap(graph_rect(graph_child(&rendered, "-et-request")), graph_rect(graph_child(&rendered, "-n-client"))));
    spec["edges"][0]["label_placement"] = json!({"position":0,"side":"below","offset":20,"on_overlap":"error"});
    let error = execute_request(json!({"op":"create_graph","id":"icon-text","spec":spec})).unwrap_err().to_string();
    assert!(error.contains("-nt-client") && !error.contains("-n-client"), "{error}");
}

#[test]
fn review_strict_collision_checks_later_warn_labels_in_both_orders_atomically() {
    let mut spec = graph();
    spec["nodes"][1]["y"] = json!(160); spec["nodes"][1]["height"] = json!(96);
    spec["edges"][0]["label_placement"] = json!({"position":0.5,"side":"above","offset":8,"on_overlap":"error"});
    let document = execute_request(json!({"op":"create_presentation","id":"review-strict","title":"Strict labels"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide-1","id":"flow","spec":spec})).unwrap();
    let before = inserted["document"].clone();
    let mut second = spec["edges"][0].clone(); second["id"] = json!("second");
    second["label_placement"]["on_overlap"] = json!("warn");
    spec["edges"].as_array_mut().unwrap().push(second);
    for reverse in [false, true] {
        let mut candidate = spec.clone();
        if reverse { candidate["edges"].as_array_mut().unwrap().reverse(); }
        let error = execute_request(json!({"op":"create_graph","id":"review-order","spec":candidate})).unwrap_err().to_string();
        assert!(error.contains("request") && error.contains("-et-second"), "{error}");
        let error = execute_request(json!({"op":"update_graph","document":before,"expected_revision":1,"slide_id":"slide-1","id":"flow","spec":candidate})).unwrap_err().to_string();
        assert!(error.contains("request") && error.contains("-et-second"), "{error}");
        let unchanged = execute_request(json!({"op":"apply_graph","document":before,"expected_revision":1,"slide_id":"slide-1","id":"flow","operations":[{"op":"move","ids":["api"],"dx":0,"dy":0}]})).unwrap();
        assert_eq!(unchanged["document"], before); assert!(unchanged["receipt"].is_null());
        let undone = execute_request(json!({"op":"undo_transaction","document":before,"expected_revision":1,"receipt":inserted["receipt"]})).unwrap();
        assert_eq!(undone["document"]["hash"], document["hash"]);
    }
}

#[test]
fn feedback_native_roundtrip_undo_and_strict_failure_are_atomic() {
    let spec = feedback_graph();
    let document = execute_request(json!({"op":"create_presentation","id":"feedback-history","title":"Feedback history"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide-1","id":"flow","spec":spec})).unwrap();
    let before = inserted["document"].clone();
    let preflight = execute_request(json!({"op":"preflight_presentation","document":before})).unwrap();
    assert!(preflight["findings"].as_array().unwrap().iter().any(|finding| finding["code"] == "TEXT_OVERLAP" && finding["severity"] == "warning" && finding["element_ids"].as_array().unwrap().iter().any(|id| id.as_str().unwrap().ends_with("-et-e1"))));
    let mut strict = spec.clone(); strict["edges"][0]["label_placement"]["on_overlap"] = json!("error");
    let error = execute_request(json!({"op":"update_graph","document":before,"expected_revision":1,"slide_id":"slide-1","id":"flow","spec":strict})).unwrap_err().to_string();
    assert!(error.contains("e1") && error.contains("-n-xdr"), "{error}");
    let unchanged = execute_request(json!({"op":"apply_graph","document":before,"expected_revision":1,"slide_id":"slide-1","id":"flow","operations":[{"op":"move","ids":["ws"],"dx":0,"dy":0}]})).unwrap();
    assert_eq!(unchanged["document"], before); assert!(unchanged["receipt"].is_null());
    let saved = execute_request(json!({"op":"export_presentation","document":before})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"feedback-open","base64":saved["base64"]})).unwrap();
    assert_eq!(opened["document"]["parts"][0]["stale"], false);
    assert_eq!(opened["document"]["parts"][0]["spec"], before["parts"][0]["spec"]);
    let original = &before["deck"]["slides"][0]["elements"][0];
    let restored = &opened["document"]["deck"]["slides"][0]["elements"][0];
    assert_eq!(original["children"].as_array().unwrap().iter().filter(|child| child["id"].as_str().unwrap().contains("-eb-")).count(), 5);
    for suffix in ["-nt-ws", "-nd-ws", "-nt-lake", "-nd-isoc", "-et-e1", "-et-e3", "-et-e5"] {
        let source = graph_child(original, suffix); let native = graph_child(restored, suffix);
        assert_eq!(source["text"], native["text"]);
        for (expected, actual) in graph_rect(source).iter().zip(graph_rect(native)) { assert!((expected - actual).abs() <= 1.0 / 9525.0); }
        assert!((source["font_size"].as_f64().unwrap() - native["font_size"].as_f64().unwrap()).abs() <= 0.02);
    }
    let mut updated_spec = spec.clone(); updated_spec["nodes"][4]["detail"] = json!("確認待ち");
    let updated = execute_request(json!({"op":"update_graph","document":opened["document"],"expected_revision":0,"slide_id":"slide-1","id":"flow","spec":updated_spec})).unwrap();
    let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":1,"receipt":updated["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], opened["document"]["hash"]);
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], saved["base64"]);
    let undone_insert = execute_request(json!({"op":"undo_transaction","document":before,"expected_revision":1,"receipt":inserted["receipt"]})).unwrap();
    assert_eq!(undone_insert["document"]["hash"], document["hash"]);
}

#[test]
fn review_strict_collision_checks_automatic_fallback_in_both_orders() {
    let mut spec = json!({"version":1,"title":"Strict fallback","show_title":false,
        "groups":[{"id":"boundary","label":"Boundary","x":0,"y":0,"width":420,"height":160}],
        "nodes":[
            {"id":"source","label":"A","x":8,"y":40,"width":120,"height":112,"group":"boundary"},
            {"id":"target","label":"B","x":292,"y":40,"width":120,"height":112,"group":"boundary"}
        ],"edges":[
            {"id":"above","source":"source","target":"target","source_port":"right","target_port":"left","label":"ABCDEFGHIJKLMNOPQ","label_placement":{"position":0.5,"side":"above","offset":8}},
            {"id":"below","source":"source","target":"target","source_port":"right","target_port":"left","label":"ABCDEFGHIJKLMNOPQ","label_placement":{"position":0.5,"side":"below","offset":8}},
            {"id":"automatic","source":"source","target":"target","source_port":"right","target_port":"left","label":"R"}
        ]});
    let warning = execute_request(json!({"op":"create_graph","id":"review-fallback","spec":spec})).unwrap();
    let automatic = graph_rect(graph_child(&warning, "-et-automatic"));
    assert!(["-et-above", "-et-below"].iter().any(|suffix| rects_overlap(automatic, graph_rect(graph_child(&warning, suffix)))));
    for index in [0, 1] { spec["edges"][index]["label_placement"]["on_overlap"] = json!("error"); }
    let document = execute_request(json!({"op":"create_presentation","id":"review-fallback-document","title":"Strict fallback"})).unwrap();
    for reverse in [false, true] {
        let mut candidate = spec.clone();
        if reverse { candidate["edges"].as_array_mut().unwrap().reverse(); }
        let error = execute_request(json!({"op":"create_graph","id":"review-fallback","spec":candidate})).unwrap_err().to_string();
        assert!(error.contains("-et-automatic"), "{error}");
        let error = execute_request(json!({"op":"apply_operations","document":document,"expected_revision":0,"expected_hash":document["hash"],"operations":[{
            "op":"add_graph","slide_id":"slide-1","id":"review-fallback","spec":candidate,"layout":{"x":24,"y":36,"width":1080,"height":512,"show_title":false}
        }]})).unwrap_err().to_string();
        assert!(error.contains("-et-automatic"), "{error}");
    }
}

#[test]
fn review_strict_collision_checks_badges_inside_nested_groups() {
    let mut spec = graph(); spec["show_title"] = json!(false);
    spec["groups"] = json!([
        {"id":"outer","label":"Outer","x":0,"y":0,"width":1000,"height":500},
        {"id":"inner","label":"Inner","x":16,"y":56,"width":960,"height":400,"parent":"outer"}
    ]);
    for node in spec["nodes"].as_array_mut().unwrap() { node["group"] = json!("inner"); node["y"] = json!(160); node["height"] = json!(96); }
    spec["edges"][0]["label_placement"] = json!({"position":0.5,"side":"above","offset":8});
    let mut second = spec["edges"][0].clone(); second["id"] = json!("second"); second["label"] = json!("");
    second.as_object_mut().unwrap().remove("label_placement"); second["badge"] = json!({"number":2,"position":0.5,"size":24});
    spec["edges"].as_array_mut().unwrap().push(second);
    let warning = execute_request(json!({"op":"create_graph","id":"review-nested","spec":spec})).unwrap();
    assert!(rects_overlap(graph_rect(graph_child(&warning, "-et-request")), graph_rect(graph_child(&warning, "-eb-second"))));
    spec["edges"][0]["label_placement"]["on_overlap"] = json!("error");
    for reverse in [false, true] {
        let mut candidate = spec.clone();
        if reverse { candidate["edges"].as_array_mut().unwrap().reverse(); }
        let error = execute_request(json!({"op":"create_graph","id":"review-nested","spec":candidate})).unwrap_err().to_string();
        assert!(error.contains("request") && error.contains("-eb-second"), "{error}");
    }
}

fn graph_rect(element: &Value) -> [f64; 4] {
    ["x", "y", "width", "height"].map(|field| element[field].as_f64().unwrap())
}

fn rects_overlap(left: [f64; 4], right: [f64; 4]) -> bool {
    left[0] < right[0] + right[2] && left[0] + left[2] > right[0]
        && left[1] < right[1] + right[3] && left[1] + left[3] > right[1]
}

#[test]
fn feedback_automatic_labels_stay_near_routes_inside_the_common_group() {
    let spec = feedback_graph();
    let rendered = execute_request(json!({"op":"create_graph","id":"feedback-auto","spec":spec})).unwrap();
    for suffix in ["-et-e3", "-et-e5"] {
        let label = graph_child(&rendered, suffix);
        let [left, top, width, height] = graph_rect(label);
        assert!(left >= 716.0 && left + width <= 1112.0 && top >= 60.0 && top + height <= 300.0, "outside common group body: {label}");
        assert!(label["font_size"].as_f64().unwrap() >= 12.0);
        for node in spec["nodes"].as_array().unwrap() { assert!(!rects_overlap(graph_rect(label), graph_rect(node)), "{suffix} overlaps {}: {label}", node["id"]); }
        for badge in rendered["children"].as_array().unwrap().iter().filter(|child| child["id"].as_str().unwrap().contains("-eb-")) {
            assert!(!rects_overlap(graph_rect(label), graph_rect(badge)), "{suffix} overlaps {}", badge["id"]);
        }
        assert!(top + height >= if suffix == "-et-e3" { 172.0 } else { 108.0 }, "label has drifted from its own route: {label}");
    }
    assert!(!rects_overlap(graph_rect(graph_child(&rendered, "-et-e3")), graph_rect(graph_child(&rendered, "-et-e5"))));
    let typed: aislide_core::model::Deck = serde_json::from_value(deck(rendered.clone())).unwrap();
    let report = aislide_core::layout::measure_layout(&typed).unwrap();
    assert!(report.measurements.iter().filter(|entry| entry.element_id.contains("-et-")).all(|entry| !entry.overflow && entry.missing_glyphs == 0));
    for suffix in ["-et-e3", "-et-e5"] { println!("{suffix}: {}", graph_child(&rendered, suffix)); }
}

#[test]
fn feedback_automatic_labels_without_clear_space_keep_a_bounded_fallback() {
    let spec = json!({"version":1,"title":"Crowded group","show_title":false,
        "groups":[{"id":"boundary","label":"Boundary","x":0,"y":0,"width":420,"height":160}],
        "nodes":[
            {"id":"source","label":"A","x":8,"y":40,"width":200,"height":112,"group":"boundary"},
            {"id":"target","label":"B","x":212,"y":40,"width":200,"height":112,"group":"boundary"}
        ],"edges":[{"id":"crowded","source":"source","target":"target","source_port":"right","target_port":"left","label":"R"}]});
    let rendered = execute_request(json!({"op":"create_graph","id":"fallback","spec":spec})).unwrap();
    let label = graph_child(&rendered, "-et-crowded");
    let [left, top, width, height] = graph_rect(label);
    assert!(left >= 8.0 && left + width <= 412.0 && top >= 40.0 && top + height <= 152.0);
    assert!(width >= 12.0 && height >= 12.0 && label["font_size"].as_f64().unwrap() >= 12.0);
    assert!(spec["nodes"].as_array().unwrap().iter().any(|node| rects_overlap(graph_rect(label), graph_rect(node))));
    assert!((top + height / 2.0 - 96.0).abs() <= height / 2.0 + 24.0);
}

#[test]
fn feedback_cylinder_text_excludes_the_cap_without_moving_nodes_or_connections() {
    let spec = feedback_graph();
    let rendered = execute_request(json!({"op":"create_graph","id":"feedback-cylinder","spec":spec})).unwrap();
    for node_id in ["ws", "lake"] {
        let node = graph_child(&rendered, &format!("-n-{node_id}"));
        let [left, top, width, height] = graph_rect(node);
        assert_eq!([top, width, height], [84.0, 180.0, 96.0]);
        for role in ["nt", "nd"] {
            let label = graph_child(&rendered, &format!("-{role}-{node_id}"));
            let [text_left, text_top, text_width, text_height] = graph_rect(label);
            assert!(text_top >= top + height * 0.3, "text overlaps cylinder cap: {label}");
            assert!(text_left >= left && text_left + text_width <= left + width && text_top + text_height <= top + height);
            assert!(label["font_size"].as_f64().unwrap() >= 12.0);
        }
    }
    let typed: aislide_core::graphs::GraphSpec = serde_json::from_value(spec).unwrap();
    assert_eq!(aislide_core::graphs::endpoint(&typed.nodes[3], aislide_core::graphs::Port::Bottom, &typed.nodes[5]), (814.0, 180.0, 3));
}

#[test]
fn feedback_cjk_soft_widow_is_fitted_without_changing_text_or_heading() {
    let rendered = execute_request(json!({"op":"create_graph","id":"feedback-widow","spec":feedback_graph()})).unwrap();
    let detail = graph_child(&rendered, "-nd-isoc");
    assert_eq!(detail["text"], "ケース・自動化・ワークブック");
    assert_eq!(graph_child(&rendered, "-nt-isoc")["font_size"], 18.0);
    assert!((12.0..14.4).contains(&detail["font_size"].as_f64().unwrap()), "single soft glyph should be reflowed: {detail}");
    let typed: aislide_core::model::Deck = serde_json::from_value(deck(rendered.clone())).unwrap();
    let report = aislide_core::layout::measure_layout(&typed).unwrap();
    let measured = report.measurements.iter().find(|entry| entry.element_id.ends_with("-nd-isoc")).unwrap();
    assert_eq!(measured.lines, 1); assert!(!measured.overflow);
    println!("ISOC detail: {detail}; lines={}", measured.lines);
}

#[test]
fn feedback_cjk_intentional_newline_short_details_and_floor_are_retained() {
    for (detail, width, size) in [("ケース・自動化・ワークブッ\nク",240,14.4), ("未確認",240,14.4), ("Short detail",240,14.4), ("ワークブッ",64,12.0)] {
        let spec = json!({"version":1,"title":"Detail boundary","nodes":[{"id":"detail","label":"A","detail":detail,"detail_font_size":size,"x":80,"y":100,"width":width,"height":120}]});
        let rendered = execute_request(json!({"op":"create_graph","id":"feedback-detail","spec":spec})).unwrap();
        let body = graph_child(&rendered, "-nd-detail");
        assert_eq!(body["text"], detail); assert_eq!(body["font_size"], size);
    }
}

#[test]
fn review_bounded_layout_refits_cjk_soft_widow_after_final_geometry() {
    let spec = json!({"version":1,"title":"Final detail fitting","show_title":false,
        "nodes":[{"id":"detail","label":"A","detail":"ケース・自動化・ワークブック","x":80,"y":100,"width":240,"height":120}]});
    let original = execute_request(json!({"op":"create_graph","id":"review-widow","spec":spec})).unwrap();
    let original_size = graph_child(&original, "-nd-detail")["font_size"].as_f64().unwrap();
    let layout = json!({"x":24,"y":36,"width":1080,"height":512,"show_title":false});
    let part = json!({"version":1,"preset":"diagram/custom","title":spec["title"],"data":{"kind":"diagram","graph":spec},"layout":layout});
    let bounded = execute_request(json!({"op":"create_part","id":"review-widow","spec":part})).unwrap();
    let document = execute_request(json!({"op":"create_presentation","id":"review-widow-document","title":"Final detail fitting"})).unwrap();
    let inserted = execute_request(json!({"op":"apply_operations","document":document,"expected_revision":0,"expected_hash":document["hash"],"operations":[{
        "op":"add_graph","slide_id":"slide-1","id":"review-widow","spec":spec,"layout":layout
    }]})).unwrap();
    for rendered in [&bounded, &inserted["document"]["deck"]["slides"][0]["elements"][0]] {
        let detail = graph_child(rendered, "-nd-detail");
        let heading = graph_child(rendered, "-nt-detail");
        assert_eq!(detail["text"], spec["nodes"][0]["detail"]);
        assert_eq!(heading["font_size"], graph_child(&original, "-nt-detail")["font_size"]);
        let size = detail["font_size"].as_f64().unwrap();
        assert!((12.0..original_size).contains(&size), "final soft widow was not refitted: {detail}");
        assert!(size <= heading["font_size"].as_f64().unwrap());
        for suffix in ["-n-detail", "-nt-detail", "-nd-detail"] {
            let [left, top, width, height] = graph_rect(graph_child(&original, suffix));
            assert_eq!(graph_rect(graph_child(rendered, suffix)), [left * 1080.0 / 1152.0, top, width * 1080.0 / 1152.0, height]);
        }
        let typed: aislide_core::model::Deck = serde_json::from_value(deck(rendered.clone())).unwrap();
        let report = aislide_core::layout::measure_layout(&typed).unwrap();
        let measured = report.measurements.iter().find(|entry| entry.element_id.ends_with("-nd-detail")).unwrap();
        assert_eq!(measured.lines, 1); assert!(!measured.overflow); assert_eq!(measured.missing_glyphs, 0);
    }
}

#[test]
fn review_cjk_early_paragraph_widow_is_fitted_and_hard_lines_keep_their_size() {
    for (detail, width, size, expected_lines, shrink) in [
        ("ケース・自動化・ワークブック\nOK",240,14.4,2,true),
        ("ケース・自動化・ワークブッ\nク",240,14.4,2,false),
        ("ワークブッ\nOK",64,12.0,3,false),
    ] {
        let spec = json!({"version":1,"title":"Paragraph tails","show_title":false,
            "nodes":[{"id":"detail","label":"A","detail":detail,"detail_font_size":size,"x":80,"y":100,"width":width,"height":160}]});
        let rendered = execute_request(json!({"op":"create_graph","id":"review-paragraph","spec":spec})).unwrap();
        let body = graph_child(&rendered, "-nd-detail");
        let fitted_size = body["font_size"].as_f64().unwrap();
        assert_eq!(body["text"], detail);
        assert_eq!(graph_child(&rendered, "-nt-detail")["font_size"], 18.0);
        if shrink { assert!((12.0..size).contains(&fitted_size), "early paragraph widow was lost: {body}"); }
        else { assert_eq!(fitted_size, size); }
        let typed: aislide_core::model::Deck = serde_json::from_value(deck(rendered)).unwrap();
        let report = aislide_core::layout::measure_layout(&typed).unwrap();
        let measured = report.measurements.iter().find(|entry| entry.element_id.ends_with("-nd-detail")).unwrap();
        assert_eq!(measured.lines, expected_lines); assert!(!measured.overflow); assert_eq!(measured.missing_glyphs, 0);
    }
}

#[test]
fn review_bounded_layout_preserves_hard_lines_font_floor_and_unaffected_details() {
    for (detail, width, size, layout_width) in [
        ("ワークブッ\nク",240,14.4,1080), ("未確認",240,14.4,1080),
        ("Short detail",240,14.4,1080), ("ワークブッ",64,12.0,576),
    ] {
        let spec = json!({"version":1,"title":"Bounded paragraph controls",
            "nodes":[{"id":"detail","label":"A","detail":detail,"detail_font_size":size,"x":80,"y":100,"width":width,"height":160}]});
        let document = execute_request(json!({"op":"create_presentation","id":"review-bounded-controls","title":"Bounded paragraph controls"})).unwrap();
        let inserted = execute_request(json!({"op":"apply_operations","document":document,"expected_revision":0,"expected_hash":document["hash"],"operations":[{
            "op":"add_graph","slide_id":"slide-1","id":"flow","spec":spec,"layout":{"x":24,"y":36,"width":layout_width,"height":424,"show_title":false}
        }]})).unwrap();
        let rendered = &inserted["document"]["deck"]["slides"][0]["elements"][0];
        let body = graph_child(rendered, "-nd-detail");
        assert_eq!(body["text"], detail); assert_eq!(body["font_size"], size);
        assert_eq!(graph_child(rendered, "-nt-detail")["font_size"], 18.0);
        let typed: aislide_core::model::Deck = serde_json::from_value(deck(rendered.clone())).unwrap();
        let report = aislide_core::layout::measure_layout(&typed).unwrap();
        assert!(report.measurements.iter().all(|entry| !entry.overflow && entry.missing_glyphs == 0));
    }
}

#[test]
fn graph_edge_styles_are_optional_bounded_and_rendered() {
    let original = graph();
    let legacy = execute_request(json!({"op":"create_graph","id":"edge-style","spec":original})).unwrap();
    let mut explicit = original.clone();
    for field in ["stroke_width", "label_color", "label_font_size"] { explicit["edges"][0][field] = Value::Null; }
    assert_eq!(execute_request(json!({"op":"create_graph","id":"edge-style","spec":explicit})).unwrap(), legacy);
    let typed: aislide_core::graphs::GraphSpec = serde_json::from_value(explicit).unwrap();
    let serialized = serde_json::to_value(typed).unwrap();
    for field in ["stroke_width", "label_color", "label_font_size"] { assert!(serialized["edges"][0].get(field).is_none()); }
    let mut styled = original.clone();
    styled["edges"][0]["stroke_width"] = json!(4.5);
    styled["edges"][0]["label_color"] = json!("C02040");
    styled["edges"][0]["label_font_size"] = json!(10);
    let rendered = execute_request(json!({"op":"create_graph","id":"edge-style","spec":styled})).unwrap();
    let children = rendered["children"].as_array().unwrap();
    let edge = children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-e-request")).unwrap();
    let label = children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-et-request")).unwrap();
    assert_eq!(edge["stroke_width"], 4.5);
    assert_eq!(label["color"], "C02040");
    assert_eq!(label["font_size"], 10.0);
    for (field, value) in [("stroke_width", json!(0.25)), ("stroke_width", json!(13)), ("label_font_size", json!(7)), ("label_font_size", json!(41)), ("label_color", json!("bad-color"))] {
        let mut invalid = original.clone(); invalid["edges"][0][field] = value;
        assert!(execute_request(json!({"op":"create_graph","id":"invalid-style","spec":invalid})).is_err(), "{field}");
    }
}

#[test]
fn graph_explicit_label_and_badge_positions_follow_the_route() {
    let mut spec = graph();
    spec["nodes"][1]["y"] = json!(160); spec["nodes"][1]["height"] = json!(96);
    spec["edges"][0]["label_font_size"] = json!(10);
    spec["edges"][0]["label_placement"] = json!({"position":0.25,"side":"above","offset":6});
    spec["edges"][0]["badge"] = json!({"number":8,"position":0.75,"size":24,"font_size":12,"fill":"FFFFFF","color":"C02040"});
    for side in ["above", "below"] {
        spec["edges"][0]["label_placement"]["side"] = json!(side);
        let rendered = execute_request(json!({"op":"create_graph","id":"positioned","spec":spec})).unwrap();
        let children = rendered["children"].as_array().unwrap();
        let label = children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-et-request")).unwrap();
        assert_eq!(label["x"].as_f64().unwrap() + label["width"].as_f64().unwrap() / 2.0, 326.0);
        let facing_edge = if side == "above" { label["y"].as_f64().unwrap() + label["height"].as_f64().unwrap() } else { label["y"].as_f64().unwrap() };
        assert_eq!(facing_edge, if side == "above" { 202.0 } else { 214.0 });
        let badge = children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-eb-request")).unwrap();
        assert_eq!(badge["type"], "shape"); assert_eq!(badge["preset"], "ellipse"); assert_eq!(badge["text"], "8");
        assert_eq!(badge["x"], 470.0); assert_eq!(badge["y"], 196.0); assert_eq!(badge["color"], "C02040");
        for (index, node) in spec["nodes"].as_array().unwrap().iter().enumerate() {
            let suffix = format!("-n-{}", node["id"].as_str().unwrap());
            let rendered_node = children.iter().find(|child| child["id"].as_str().unwrap().ends_with(&suffix)).unwrap();
            for field in ["x", "y", "width", "height"] { assert_eq!(rendered_node[field].as_f64(), spec["nodes"][index][field].as_f64()); }
        }
    }
    for (field, value) in [("label_placement", json!({"position":1.1,"side":"above"})), ("label_placement", json!({"position":0.5,"side":"above","offset":-1})), ("badge", json!({"number":0})), ("badge", json!({"number":100})), ("badge", json!({"number":1,"size":8}))] {
        let mut invalid = spec.clone(); invalid["edges"][0][field] = value;
        assert!(execute_request(json!({"op":"create_graph","id":"invalid-position","spec":invalid})).is_err());
    }
    spec["edges"][0]["label_placement"] = json!({"position":0.5,"side":"above","offset":128});
    spec["nodes"][0]["y"] = json!(88); spec["nodes"][1]["y"] = json!(88);
    assert!(execute_request(json!({"op":"create_graph","id":"outside-position","spec":spec})).is_err());
}

#[test]
fn graph_bounded_annotations_honor_declared_font_minimum_and_reopen() {
    for size in [8, 11] {
        let mut spec = graph();
        spec["edges"][0]["label_font_size"] = json!(size);
        spec["edges"][0]["badge"] = json!({"number":2,"position":0.75,"size":24,"font_size":size});
        spec["groups"] = json!([{"id":"boundary","label":"Boundary","x":0,"y":88,"width":1000,"height":400,"padding":8,"header_height":32,"header_font_size":size}]);
        for node in spec["nodes"].as_array_mut().unwrap() { node["group"] = json!("boundary"); }
        let document = execute_request(json!({"op":"create_presentation","id":"bounded-annotations","title":"Annotation size"})).unwrap();
        let inserted = execute_request(json!({"op":"apply_operations","document":document,"expected_revision":0,"expected_hash":document["hash"],"operations":[{
            "op":"add_graph","slide_id":"slide-1","id":"architecture","spec":spec,"layout":{"x":64,"y":144,"width":1152,"height":512,"show_title":true}
        }]})).unwrap();
        let children = inserted["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap();
        for suffix in ["-et-request", "-eb-request", "-gt-boundary"] {
            let annotation = children.iter().find(|child| child["id"].as_str().unwrap().ends_with(suffix)).unwrap();
            assert_eq!(annotation["font_size"].as_f64(), Some(f64::from(size)), "{suffix}");
        }
        let mut bounded = json!({"version":1,"preset":"diagram/custom","title":spec["title"],"subtitle":spec["subtitle"],"data":{"kind":"diagram","graph":spec},"layout":{"x":64,"y":144,"width":576,"height":512,"show_title":true}});
        bounded["data"]["graph"]["edges"][0]["badge"] = Value::Null;
        let compact = execute_request(json!({"op":"create_part","id":"compact-annotations","spec":bounded})).unwrap();
        let compact_children = compact["children"].as_array().unwrap();
        let compact_label = compact_children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-et-request")).unwrap();
        let compact_size = compact_label["font_size"].as_f64().unwrap();
        assert!((8.0..=f64::from(size)).contains(&compact_size));
        if size == 11 { assert!(compact_size < 11.0, "fixture must exercise fitting below 12px"); }
        for node in compact_children.iter().filter(|child| child["id"].as_str().unwrap().contains("-nt-")) { assert!(node["font_size"].as_f64().unwrap() >= 12.0); }
        bounded["data"]["graph"]["nodes"][0]["label"] = json!("M".repeat(60));
        let error = execute_request(json!({"op":"create_part","id":"body-floor","spec":bounded})).unwrap_err().to_string();
        assert!(error.contains("12px"), "{error}");
        let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
        let reopened = execute_request(json!({"op":"open_presentation","id":"bounded-annotations-open","base64":saved["base64"]})).unwrap();
        assert_eq!(reopened["document"]["parts"][0]["stale"], false);
        assert_eq!(reopened["document"]["parts"][0]["spec"], inserted["document"]["parts"][0]["spec"]);
        let undone = execute_request(json!({"op":"undo_transaction","document":inserted["document"],"expected_revision":1,"receipt":inserted["receipt"]})).unwrap();
        assert_eq!(undone["document"]["hash"], document["hash"]);
    }
}

#[test]
fn graph_group_spacing_controls_containment_header_and_layout() {
    let mut spec = graph();
    spec["groups"] = json!([{"id":"boundary","label":"Boundary","x":24,"y":120,"width":1000,"height":368,"padding":16,"header_height":24,"header_font_size":8}]);
    spec["nodes"][0]["y"] = json!(144);
    for node in spec["nodes"].as_array_mut().unwrap() { node["group"] = json!("boundary"); }
    let rendered = execute_request(json!({"op":"create_graph","id":"spacing","spec":spec})).unwrap();
    let header = rendered["children"].as_array().unwrap().iter().find(|child| child["id"].as_str().unwrap().ends_with("-gt-boundary")).unwrap();
    assert_eq!(header["x"], 44.0); assert_eq!(header["y"], 128.0); assert_eq!(header["height"], 12.0); assert_eq!(header["font_size"], 8.0);
    let layout = execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"layout","columns":2}]})).unwrap();
    assert_eq!(layout["nodes"][0]["x"], 182.0);
    assert_eq!(layout["nodes"][0]["y"], 260.0);
    let mut outside = spec.clone(); outside["nodes"][0]["y"] = json!(143);
    assert!(execute_request(json!({"op":"create_graph","id":"spacing-invalid","spec":outside})).is_err());
    for (field, value) in [("padding", -1), ("padding", 65), ("header_height", 10), ("header_font_size", 7), ("header_font_size", 40)] {
        let mut invalid = spec.clone(); invalid["groups"][0][field] = json!(value);
        assert!(execute_request(json!({"op":"create_graph","id":"spacing-invalid","spec":invalid})).is_err(), "{field}");
    }
}

#[test]
fn review_legacy_small_groups_accept_omitted_spacing_controls() {
    for height in [40, 48] {
        let mut spec = graph();
        spec["groups"] = json!([{"id":"empty","label":"Legacy","x":900,"y":88,"width":200,"height":height}]);
        let original = execute_request(json!({"op":"create_graph","id":"legacy-small","spec":spec})).unwrap();
        for field in ["padding", "header_height", "header_font_size"] { spec["groups"][0][field] = Value::Null; }
        assert_eq!(execute_request(json!({"op":"create_graph","id":"legacy-small","spec":spec})).unwrap(), original);
        let exported = execute_request(json!({"op":"export","deck":deck(original)})).unwrap();
        execute_request(json!({"op":"open_presentation","id":"legacy-small-open","base64":exported["base64"]})).unwrap();
    }
}

#[test]
fn graph_containment_errors_report_actual_and_required_bounds() {
    let mut spec = graph(); spec["show_title"] = json!(false);
    spec["groups"] = json!([{"id":"boundary","label":"Boundary","x":0,"y":10,"width":1000,"height":450,"padding":12,"header_height":30,"header_font_size":13}]);
    spec["nodes"][0]["group"] = json!("boundary"); spec["nodes"][0]["y"] = json!(40);
    execute_request(json!({"op":"create_graph","id":"contained","spec":spec})).unwrap();
    for (horizontal, vertical) in [(8,40), (48,39), (900,40), (48,390)] {
        let mut invalid = spec.clone(); invalid["nodes"][0]["x"] = json!(horizontal); invalid["nodes"][0]["y"] = json!(vertical);
        let message = execute_request(json!({"op":"create_graph","id":"outside-group","spec":invalid})).unwrap_err().to_string();
        for expected in ["graph node", "client", "boundary", "x >= 12", "y >= 40", "right <= 988", "bottom <= 448", "padding=12", "header_height=30"] {
            assert!(message.contains(expected), "missing {expected}: {message}");
        }
        assert!(message.contains(&format!("actual x={horizontal}, y={vertical}, width=200, height=96")), "{message}");
    }
    spec["groups"].as_array_mut().unwrap().push(json!({"id":"inner","label":"Inner","x":12,"y":39,"width":200,"height":100,"parent":"boundary"}));
    let message = execute_request(json!({"op":"create_graph","id":"outside-parent","spec":spec})).unwrap_err().to_string();
    for expected in ["graph group", "inner", "boundary", "y >= 40", "actual x=12, y=39"] { assert!(message.contains(expected), "missing {expected}: {message}"); }
}

fn deck(element: Value) -> Value {
    json!({"version":1,"title":"Graph fixture","width":1280,"height":720,"slides":[{"id":"slide","title":"Graph","background":"FFFFFF","notes":"Synthetic fixture","elements":[element]}]})
}

#[test]
fn graph_manual_routes_offsets_preserve_nodes_native_metadata_and_undo() {
    let mut spec = graph();
    spec["edges"][0]["route"] = json!("manual");
    spec["edges"][0]["waypoints"] = json!([[360,176],[440,248],[360,352],[480,400]]);
    spec["edges"][0]["source_offset"] = json!(-0.25);
    spec["edges"][0]["target_offset"] = json!(0.25);
    spec["edges"][0]["start_arrow"] = json!(true); spec["edges"][0]["dashed"] = json!(true);
    spec["edges"].as_array_mut().unwrap().push(graph()["edges"][0].clone());
    spec["edges"][1]["id"] = json!("parallel");
    let document = execute_request(json!({"op":"create_presentation","id":"manual-graph","title":"Manual graph"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide-1","id":"architecture","spec":spec})).unwrap();
    let original = &inserted["document"]["deck"]["slides"][0]["elements"][0];
    let children = original["children"].as_array().unwrap();
    let source = children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-n-client")).unwrap();
    assert_eq!(source["x"], 48.0); assert_eq!(source["y"], 160.0);
    assert_eq!(source["visual"]["connection_sites"], json!([{"x":1.0,"y":0.25,"angle":0.0},{"x":1.0,"y":0.5,"angle":0.0}]));
    let edge = children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-e-request")).unwrap();
    assert_eq!(edge["routing"]["custom"], true); assert_eq!(edge["routing"]["points"].as_array().unwrap().len(), 6);
    assert_eq!(edge["start"]["element_id"], source["id"]); assert_eq!(edge["start"]["site"], 0);
    let parallel = children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-e-parallel")).unwrap();
    assert_eq!(parallel["start"]["element_id"], source["id"]); assert_eq!(parallel["start"]["site"], 1);
    assert_eq!(parallel["routing"].get("custom"), None);
    let typed: aislide_core::graphs::GraphSpec = serde_json::from_value(spec.clone()).unwrap();
    assert_eq!(aislide_core::graphs::edge_points(&typed.nodes[0], &typed.nodes[1], &typed.edges[0]), vec![[248.0,184.0],[360.0,176.0],[440.0,248.0],[360.0,352.0],[480.0,400.0],[560.0,340.0]]);
    let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap();
    assert!(xml.contains("<a:cxn ")); assert!(xml.contains("<a:custGeom>")); assert!(xml.contains("<a:lnTo>"));
    let opened = execute_request(json!({"op":"open_presentation","id":"manual-open","base64":saved["base64"]})).unwrap();
    let before = &opened["document"];
    assert_eq!(before["parts"][0]["stale"], false);
    assert_eq!(before["parts"][0]["spec"], inserted["document"]["parts"][0]["spec"]);
    let restored = before["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap();
    assert_eq!(restored.len(), children.len());
    for child in children {
        let native = restored.iter().find(|native| native["id"] == child["id"]).unwrap();
        for field in ["type", "preset", "visual", "routing", "start", "end", "arrow", "flip_v"] { assert_eq!(native[field], child[field], "{field}"); }
        for field in ["x", "y", "width", "height"] {
            assert!((native[field].as_f64().unwrap() - child[field].as_f64().unwrap()).abs() <= 1.0 / 9525.0, "{field}");
        }
    }
    let updated = execute_request(json!({"op":"apply_graph","document":before,"expected_revision":0,"slide_id":"slide-1","id":"architecture","operations":[{"op":"move","ids":["client","api"],"dx":16,"dy":8}]})).unwrap();
    assert_eq!(updated["document"]["parts"][0]["spec"]["data"]["graph"]["edges"][0]["waypoints"], json!([[376.0,184.0],[456.0,256.0],[376.0,360.0],[496.0,408.0]]));
    let updated_saved = execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"manual-updated","base64":updated_saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["parts"][0]["stale"], false);
    let unchanged = execute_request(json!({"op":"apply_graph","document":before,"expected_revision":0,"slide_id":"slide-1","id":"architecture","operations":[{"op":"move","ids":["client"],"dx":0,"dy":0}]})).unwrap();
    assert_eq!(unchanged["document"]["hash"], before["hash"]); assert!(unchanged["receipt"].is_null());
    let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":1,"receipt":updated["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], before["hash"]);
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], saved["base64"]);
    let moved = execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"move","ids":["client"],"dx":16,"dy":0}]})).unwrap();
    assert_eq!(moved["edges"][0]["waypoints"], json!([[360.0,176.0],[440.0,248.0],[360.0,352.0],[480.0,400.0]]));
}

#[test]
fn graph_manual_routes_and_offsets_reject_ambiguous_or_unsupported_input() {
    for edge in [
        json!({"route":"manual"}), json!({"waypoints":[[320,200]]}),
        json!({"route":"manual","waypoints":[[320,200],[320,200]]}),
        json!({"route":"manual","waypoints":[[320,80]]}),
        json!({"route":"manual","waypoints":[[1200,200]]}),
        json!({"route":"manual","waypoints":vec![[320,200];17]}),
        json!({"source_offset":0.51}), json!({"target_offset":-0.51}),
    ] {
        let mut spec = graph(); spec["edges"][0].as_object_mut().unwrap().extend(edge.as_object().unwrap().clone());
        assert!(execute_request(json!({"op":"create_graph","id":"invalid-manual","spec":spec})).is_err(), "{edge}");
    }
    for kind in ["cylinder", "cloud"] {
        let mut spec = graph(); spec["nodes"][0]["kind"] = json!(kind); spec["edges"][0]["source_offset"] = json!(0.2);
        assert!(execute_request(json!({"op":"create_graph","id":"unsupported-offset","spec":spec})).unwrap_err().to_string().contains("offset"));
    }
}

fn node_icon(color: &str) -> Value {
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16"><rect width="32" height="16" fill="{color}"/></svg>"#);
    let picture = execute_request(json!({"op":"create_asset","id":"icon","base64":STANDARD.encode(svg),"mime_type":"image/svg+xml","alt":"Client icon","size":48})).unwrap();
    json!({"base64":picture["base64"],"mime_type":picture["mime_type"],"alt":picture["alt"]})
}

#[test]
fn graph_noop_update_preserves_other_parts_order_and_receipt() {
    let mut document = execute_request(json!({"op":"create_presentation","id":"graph-order","title":"Synthetic graph order"})).unwrap();
    for id in ["first", "second"] {
        let added = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":id,"spec":graph()})).unwrap();
        document = added["document"].clone();
    }
    for id in ["first", "second"] {
        for operation in ["update_graph", "apply_graph"] {
            let mut request = json!({"op":operation,"document":document,"expected_revision":document["revision"],"slide_id":"slide-1","id":id});
            if operation == "update_graph" { request["spec"] = graph(); }
            else { request["operations"] = json!([{"op":"move","ids":["client"],"dx":0,"dy":0}]); }
            let unchanged = execute_request(request).unwrap();
            assert_eq!(unchanged["document"], document, "{operation} on {id}");
            assert!(unchanged["receipt"].is_null(), "no-op must not clear session Redo");
        }
    }
}

#[test]
fn graph_updates_preserve_inserted_part_layout_and_noop_hash() {
    let spec = graph();
    let layout = json!({"x":24,"y":36,"width":1152,"height":424,"show_title":false});
    let part = json!({"version":1,"preset":"diagram/custom","title":spec["title"],"subtitle":spec["subtitle"],"data":{"kind":"diagram","graph":spec},"layout":layout});
    let document = execute_request(json!({"op":"create_presentation","id":"graph-part-layout","title":"Layout"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide-1","id":"architecture","spec":part})).unwrap();
    let before = &inserted["document"];
    let original = &before["deck"]["slides"][0]["elements"][0];
    assert!(original["children"].as_array().unwrap().iter().all(|child| !child["id"].as_str().unwrap().ends_with("-title")));
    for operation in ["apply_graph", "update_graph"] {
        let mut request = json!({"op":operation,"document":before,"expected_revision":1,"slide_id":"slide-1","id":"architecture"});
        if operation == "apply_graph" {
            request["operations"] = json!([{"op":"move","ids":["client"],"dx":0,"dy":0}]);
        } else { request["spec"] = spec.clone(); }
        let unchanged = execute_request(request).unwrap();
        assert_eq!(unchanged["document"]["hash"], before["hash"], "{operation} must preserve content identity");
        assert_eq!(unchanged["document"]["parts"], before["parts"], "{operation}");
        assert_eq!(unchanged["document"]["parts"][0]["stale"], false);
        assert_eq!(unchanged["document"]["deck"]["slides"][0]["elements"][0], *original, "{operation} must preserve all child fonts and frames");
        let mut moved_spec = spec.clone(); moved_spec["nodes"][0]["x"] = json!(64);
        let mut request = json!({"op":operation,"document":before,"expected_revision":1,"slide_id":"slide-1","id":"architecture"});
        if operation == "apply_graph" {
            request["operations"] = json!([{"op":"move","ids":["client"],"dx":16,"dy":0}]);
        } else { request["spec"] = moved_spec; }
        let moved = execute_request(request).unwrap();
        assert_eq!(moved["document"]["parts"][0]["spec"]["layout"], before["parts"][0]["spec"]["layout"]);
        assert_eq!(moved["document"]["parts"][0]["stale"], false);
        assert_eq!(moved["document"]["parts"][0]["spec"]["data"]["graph"]["nodes"][0]["x"], 64.0);
        let group = &moved["document"]["deck"]["slides"][0]["elements"][0];
        for field in ["x","y","width","height","view_width","view_height"] { assert_eq!(group[field], original[field], "{field}"); }
        let children = group["children"].as_array().unwrap();
        assert!(children.iter().all(|child| !child["id"].as_str().unwrap().ends_with("-title")));
        assert_eq!(children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-n-client")).unwrap()["x"], 64.0);
        let undone = execute_request(json!({"op":"undo_transaction","document":moved["document"],"expected_revision":2,"receipt":moved["receipt"]})).unwrap();
        for field in ["hash","deck","parts"] { assert_eq!(undone["document"][field], before[field], "{operation}: {field}"); }
    }
}

#[test]
fn graph_layout_updates_reject_unrelated_stale_and_invalid_metadata() {
    let spec = graph();
    let mut scene = deck(json!({})); scene["slides"][0]["elements"] = json!([]);
    let mut other_slide = scene["slides"][0].clone(); other_slide["id"] = json!("other-slide");
    scene["slides"].as_array_mut().unwrap().push(other_slide);
    let document = execute_request(json!({"op":"new_document","id":"layout-guards","deck":scene})).unwrap();
    let chart = json!({"version":1,"preset":"pie-chart/focus","title":"Other part","data":{"kind":"chart","categories":["One","Two"],"series":[{"name":"Synthetic","values":[1,2]}]},"layout":{"x":0,"y":0,"width":1152,"height":512,"show_title":true}});
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"other","spec":chart})).unwrap();
    let mut part = json!({"version":1,"preset":"diagram/custom","title":spec["title"],"subtitle":spec["subtitle"],"data":{"kind":"diagram","graph":spec},"layout":{"x":40,"y":100,"width":1152,"height":512,"show_title":true}});
    let inserted = execute_request(json!({"op":"insert_part","document":inserted["document"],"expected_revision":1,"slide_id":"other-slide","id":"architecture","spec":part})).unwrap();
    part["layout"] = json!({"x":24,"y":36,"width":1152,"height":424,"show_title":false});
    let inserted = execute_request(json!({"op":"insert_part","document":inserted["document"],"expected_revision":2,"slide_id":"slide","id":"architecture","spec":part})).unwrap();
    let before = &inserted["document"];
    let children = before["deck"]["slides"][0]["elements"][1]["children"].as_array().unwrap();
    let heading = children.iter().position(|child| child["id"].as_str().unwrap().ends_with("-nt-client")).unwrap();
    let stale = execute_request(json!({"op":"transaction","document":before,"transaction":{"expected_revision":3,"expected_hash":before["hash"],"operations":[
        {"op":"replace","path":format!("/deck/slides/0/elements/1/children/{heading}/text"),"value":"Manual edit"}
    ]}})).unwrap();
    assert_eq!(stale["document"]["parts"][2]["stale"], true);
    let mut invalid = before.clone(); invalid["parts"][2]["spec"]["layout"]["width"] = json!(-1);
    let mut tampered = before.clone(); tampered["hash"] = json!("0".repeat(64));
    for operation in ["update_graph", "apply_graph"] {
        let request = |document: &Value, id: &str| {
            let mut request = json!({"op":operation,"document":document,"expected_revision":document["revision"],"slide_id":"slide","id":id});
            if operation == "update_graph" { request["spec"] = spec.clone(); }
            else { request["operations"] = json!([{"op":"move","ids":["client"],"dx":0,"dy":0}]); }
            request
        };
        let unchanged = execute_request(request(before, "architecture")).unwrap();
        assert_eq!(unchanged["document"]["hash"], before["hash"]);
        assert_eq!(unchanged["document"]["parts"], before["parts"]);
        assert!(execute_request(request(before, "other")).unwrap_err().to_string().contains("managed graph metadata not found"));
        assert!(execute_request(request(before, "missing")).is_err());
        assert!(execute_request(request(&stale["document"], "architecture")).unwrap_err().to_string().contains("stale"));
        assert!(execute_request(request(&invalid, "architecture")).is_err());
        assert!(execute_request(request(&tampered, "architecture")).is_err());
        assert_eq!(execute_request(request(before, "architecture")).unwrap()["document"]["hash"], before["hash"]);
    }
}

#[test]
fn graph_title_band_opt_out_preserves_defaults_and_uses_full_canvas() {
    let original=graph();
    let legacy=execute_request(json!({"op":"create_graph","id":"title-mode","spec":original})).unwrap();
    let mut explicit=original.clone();explicit["show_title"]=json!(true);
    for node in explicit["nodes"].as_array_mut().unwrap() {
        node["detail"]=Value::Null;node["detail_font_size"]=Value::Null;node["text_align"]=Value::Null;node["heading_bold"]=json!(true);
    }
    assert_eq!(legacy,execute_request(json!({"op":"create_graph","id":"title-mode","spec":explicit})).unwrap());
    let typed:aislide_core::graphs::GraphSpec=serde_json::from_value(explicit).unwrap();
    let serialized=serde_json::to_value(typed).unwrap();
    assert!(serialized.get("show_title").is_none());
    for node in serialized["nodes"].as_array().unwrap() {
        for field in ["detail","detail_font_size","text_align","heading_bold"] {assert!(node.get(field).is_none());}
    }
    let mut spec=original;spec["nodes"][0]["y"]=json!(0);
    assert!(execute_request(json!({"op":"create_graph","id":"title-mode","spec":spec})).is_err());
    spec["show_title"]=json!(false);
    let element=execute_request(json!({"op":"create_graph","id":"title-mode","spec":spec})).unwrap();
    let children=element["children"].as_array().unwrap();
    assert!(children.iter().all(|child|!child["id"].as_str().unwrap().ends_with("-title") && !child["id"].as_str().unwrap().ends_with("-subtitle")));
    for field in ["x","y","width","height","view_width","view_height"] {assert_eq!(element[field],legacy[field]);}
    assert!(execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"move","ids":["client"],"dx":0,"dy":-1}]})).is_err());
    let grid=json!({"version":1,"title":"Full canvas","show_title":false,"nodes":[{"id":"tall","label":"Tall","x":0,"y":0,"width":200,"height":450}]});
    let placed=execute_request(json!({"op":"transform_graph","spec":grid,"operations":[{"op":"layout","columns":1}]})).unwrap();
    assert_eq!(placed["nodes"][0]["y"],31.0);
    execute_request(json!({"op":"create_graph","id":"full-grid","spec":placed})).unwrap();
    let mut grouped=spec.clone();
    grouped["groups"]=json!([{"id":"boundary","label":"Boundary","x":0,"y":0,"width":1100,"height":480}]);
    grouped["nodes"][0]["y"]=json!(48);grouped["nodes"][0]["group"]=json!("boundary");
    execute_request(json!({"op":"create_graph","id":"top-group","spec":grouped})).unwrap();
    grouped["nodes"][0]["y"]=json!(39);
    assert!(execute_request(json!({"op":"create_graph","id":"group-header","spec":grouped})).is_err());
}

fn detailed_graph(presentation: &str) -> Value {
    let mut spec=graph();spec["show_title"]=json!(false);
    let node=&mut spec["nodes"][0];
    node["y"]=json!(0);node["width"]=json!(360);node["height"]=json!(240);node["font_size"]=json!(24);
    node["detail"]=json!("Validate access\nRecord outcome");node["detail_font_size"]=json!(16);
    node["presentation"]=json!(presentation);node["icon"]=node_icon("#007a4d");
    spec
}

#[test]
fn graph_hidden_title_relationship_labels_can_use_the_top_band() {
    let spec=json!({"version":1,"title":"Hidden","show_title":false,"nodes":[
        {"id":"source","label":"Source","x":0,"y":0,"width":200,"height":40},
        {"id":"target","label":"Target","x":500,"y":0,"width":200,"height":40}
    ],"edges":[{"id":"request","source":"source","target":"target","source_port":"right","target_port":"left","label":"HTTPS"}]});
    let element=execute_request(json!({"op":"create_graph","id":"top-label","spec":spec})).unwrap();
    let label=element["children"].as_array().unwrap().iter().find(|child|child["id"].as_str().unwrap().ends_with("-et-request")).unwrap();
    assert!((0.0..88.0).contains(&label["y"].as_f64().unwrap()));
    assert!(label["y"].as_f64().unwrap()>20.0 || label["y"].as_f64().unwrap()+label["height"].as_f64().unwrap()<20.0);
}

#[test]
fn graph_node_detail_defaults_and_fitting_preserve_font_hierarchy() {
    for presentation in ["card","icon"] {
        let mut spec=detailed_graph(presentation);
        spec["nodes"][0].as_object_mut().unwrap().remove("detail_font_size");
        let element=execute_request(json!({"op":"create_graph","id":"detail-defaults","spec":spec})).unwrap();
        let children=element["children"].as_array().unwrap();
        let heading=children.iter().find(|child|child["id"].as_str().unwrap().ends_with("-nt-client")).unwrap();
        let detail=children.iter().find(|child|child["id"].as_str().unwrap().ends_with("-nd-client")).unwrap();
        assert_eq!(heading["bold"],true);
        assert!((detail["font_size"].as_f64().unwrap()-19.2).abs()<0.001);
        spec["nodes"][0]["label"]=json!("A longer heading needs to fit");
        spec["nodes"][0]["width"]=json!(200);
        spec["nodes"][0]["detail"]=json!("Detail");
        let fitted=execute_request(json!({"op":"create_graph","id":"detail-fitting","spec":spec})).unwrap();
        let children=fitted["children"].as_array().unwrap();
        let heading=children.iter().find(|child|child["id"].as_str().unwrap().ends_with("-nt-client")).unwrap();
        let detail=children.iter().find(|child|child["id"].as_str().unwrap().ends_with("-nd-client")).unwrap();
        assert!(heading["font_size"].as_f64().unwrap()<24.0);
        assert!(detail["font_size"].as_f64().unwrap()<=heading["font_size"].as_f64().unwrap());
    }
}

#[test]
fn bounded_graph_part_card_keeps_detail_below_final_heading() {
    bounded_graph_part_font_hierarchy("card");
}

#[test]
fn bounded_graph_part_icon_keeps_detail_below_final_heading() {
    bounded_graph_part_font_hierarchy("icon");
}

fn bounded_graph_part_font_hierarchy(presentation: &str) {
    let mut spec = detailed_graph(presentation);
    spec["nodes"].as_array_mut().unwrap().truncate(1);
    spec["edges"] = json!([]);
    spec["nodes"][0]["width"] = json!(320);
    spec["nodes"][0]["height"] = json!(300);
    spec["nodes"][0]["detail"] = json!("Body");
    let original = execute_request(json!({"op":"create_graph","id":"hierarchy","spec":spec})).unwrap();
    let find = |group: &Value, suffix: &str| -> Value {
        group["children"].as_array().unwrap().iter().find(|child| child["id"].as_str().unwrap().ends_with(suffix)).unwrap().clone()
    };
    assert_eq!(find(&original, "-nt-client")["font_size"], 24.0);
    assert_eq!(find(&original, "-nd-client")["font_size"], 16.0);
    let layout = json!({"x":24,"y":36,"width":1152,"height":256,"show_title":false});
    let part = json!({"version":1,"preset":"diagram/custom","title":spec["title"],"subtitle":spec["subtitle"],"data":{"kind":"diagram","graph":spec},"layout":layout});
    let bounded = execute_request(json!({"op":"create_part","id":"hierarchy","spec":part})).unwrap();
    let assert_hierarchy = |group: &Value| {
        let heading = find(group, "-nt-client");
        let detail = find(group, "-nd-client");
        let heading_size = heading["font_size"].as_f64().unwrap();
        let detail_size = detail["font_size"].as_f64().unwrap();
        assert!((12.0..16.0).contains(&heading_size), "expected final heading to shrink: {heading_size}");
        assert!(detail_size >= 12.0 && detail_size <= heading_size, "{presentation}: detail {detail_size} exceeds final heading {heading_size}");
        for child in [heading, detail] {
            let preview = execute_request(json!({"op":"render_element_preview","element":child})).unwrap();
            assert!(preview["warnings"].as_array().unwrap().iter().all(|warning| !warning["code"].as_str().unwrap().contains("OVERFLOW")));
        }
    };
    assert_hierarchy(&bounded);
    assert_eq!(find(&bounded, "-nt-client")["height"].as_f64().unwrap(), find(&original, "-nt-client")["height"].as_f64().unwrap() / 2.0);
    assert_eq!(find(&bounded, "-n-client")["stroke_width"], find(&original, "-n-client")["stroke_width"]);
    let document = execute_request(json!({"op":"create_presentation","id":"bounded-hierarchy","title":"Hierarchy"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide-1","id":"hierarchy","spec":part})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"hierarchy-open","base64":saved["base64"]})).unwrap();
    assert_hierarchy(&opened["document"]["deck"]["slides"][0]["elements"][0]);
    assert_eq!(opened["document"]["parts"][0]["stale"], false);
    assert_eq!(opened["document"]["parts"][0]["spec"], inserted["document"]["parts"][0]["spec"]);
    let mut updated_part = opened["document"]["parts"][0]["spec"].clone();
    updated_part["data"]["graph"]["nodes"][0]["detail"] = json!("Updated body");
    let updated = execute_request(json!({"op":"update_part","document":opened["document"],"expected_revision":0,"slide_id":"slide-1","id":"hierarchy","spec":updated_part})).unwrap();
    assert_hierarchy(&updated["document"]["deck"]["slides"][0]["elements"][0]);
    assert_eq!(updated["document"]["parts"][0]["spec"], updated_part);
    assert_eq!(updated["document"]["parts"][0]["stale"], false);
    let saved_update = execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"hierarchy-reopen","base64":saved_update["base64"]})).unwrap();
    assert_hierarchy(&reopened["document"]["deck"]["slides"][0]["elements"][0]);
    assert_eq!(reopened["document"]["parts"][0]["stale"], false);
    let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":1,"receipt":updated["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], opened["document"]["hash"]);
    let mut too_small = part.clone(); too_small["layout"]["height"] = json!(128);
    assert!(execute_request(json!({"op":"create_part","id":"below-floor","spec":too_small})).is_err());
}

#[test]
fn graph_node_details_are_native_bounded_and_aligned_for_cards_and_icons() {
    for presentation in ["card","icon"] {
        for alignment in [None,Some("left"),Some("center"),Some("right")] {
            let mut spec=detailed_graph(presentation);
            if let Some(alignment)=alignment {spec["nodes"][0]["text_align"]=json!(alignment);}
            spec["nodes"][0]["heading_bold"]=json!(false);
            let element=execute_request(json!({"op":"create_graph","id":"details","spec":spec})).unwrap();
            let children=element["children"].as_array().unwrap();
            let find=|suffix:&str|children.iter().find(|child|child["id"].as_str().unwrap().ends_with(suffix)).unwrap();
            let heading=find("-nt-client");let detail=find("-nd-client");
            assert_eq!(heading["type"],"text");assert_eq!(detail["type"],"text");
            assert_eq!(heading["text"],"Client");assert_eq!(detail["text"],spec["nodes"][0]["detail"]);
            assert_eq!(heading["bold"],false);assert_eq!(detail["bold"],false);
            assert_eq!(heading["format"]["alignment"],alignment.unwrap_or("left"));
            assert_eq!(detail["format"]["alignment"],alignment.unwrap_or("left"));
            assert_eq!(heading["format"]["vertical"],"top");assert_eq!(detail["format"]["vertical"],"top");
            assert!(detail["font_size"].as_f64().unwrap()>=12.0);
            assert!(detail["font_size"].as_f64().unwrap()<=heading["font_size"].as_f64().unwrap());
            assert_eq!(heading["x"],detail["x"]);assert_eq!(heading["width"],detail["width"]);
            assert!(heading["y"].as_f64().unwrap()+heading["height"].as_f64().unwrap()+8.0<=detail["y"].as_f64().unwrap());
            assert!(detail["y"].as_f64().unwrap()+detail["height"].as_f64().unwrap()<=240.0);
            let picture=find("-ni-client");
            if presentation=="icon" {assert!(picture["y"].as_f64().unwrap()+picture["height"].as_f64().unwrap()+8.0<=heading["y"].as_f64().unwrap());}
            else {assert!(picture["x"].as_f64().unwrap()+picture["width"].as_f64().unwrap()<heading["x"].as_f64().unwrap());}
            let edge=find("-e-request");assert_eq!(edge["start"]["element_id"],find("-n-client")["id"]);
            for label in [heading,detail] {
                let preview=execute_request(json!({"op":"render_element_preview","element":label})).unwrap();
                assert!(preview["warnings"].as_array().unwrap().iter().all(|warning|!warning["code"].as_str().unwrap().contains("OVERFLOW")));
            }
        }
    }
}

#[test]
fn graph_node_detail_validation_and_schema_are_bounded() {
    for (field,value) in [("detail",json!("x".repeat(241))),("detail",json!(" ")),("detail",json!("bad\u{0001}text")),("detail_font_size",json!(11.9)),("detail_font_size",json!(40.1)),("detail_font_size",json!(25)),("text_align",json!("justify")),("heading_bold",json!("yes"))] {
        let mut spec=detailed_graph("card");spec["nodes"][0][field]=value;
        assert!(execute_request(json!({"op":"create_graph","id":"invalid-detail","spec":spec})).is_err(),"{field}");
        assert!(execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"move","ids":["client"],"dx":0,"dy":0}]})).is_err(),"{field}");
    }
    let mut missing=graph();missing["nodes"][0]["detail_font_size"]=json!(12);
    assert!(execute_request(json!({"op":"create_graph","id":"missing-detail","spec":missing})).is_err());
    let mut small=detailed_graph("icon");small["nodes"][0]["height"]=json!(40);
    assert!(execute_request(json!({"op":"create_graph","id":"small-detail","spec":small})).is_err());
    let mut spec=detailed_graph("card");spec["nodes"][0]["detail"]=json!("\u{1f4ac}".repeat(240));
    let mut typed:aislide_core::graphs::GraphSpec=serde_json::from_value(spec).unwrap();
    aislide_core::graphs::validate(&typed).unwrap();
    for invalid in [f64::NAN,f64::INFINITY,f64::NEG_INFINITY] {
        typed.nodes[0].detail_font_size=Some(invalid);
        assert!(aislide_core::graphs::validate(&typed).is_err());
    }
    let catalog=execute_request(json!({"op":"graph_catalog"})).unwrap();
    let definitions=catalog["schema"].get("$defs").or_else(||catalog["schema"].get("definitions")).unwrap();
    assert_eq!(definitions["GraphNode"]["properties"]["detail"]["maxLength"],240);
    assert_eq!(definitions["GraphNode"]["properties"]["detail_font_size"]["minimum"],12);
    assert_eq!(definitions["GraphNode"]["properties"]["detail_font_size"]["maximum"],40);
    assert_eq!(definitions["GraphTextAlign"]["enum"],json!(["left","center","right"]));
    assert_eq!(catalog["canvas"]["content_top_without_title"],0.0);
}

#[test]
fn graph_node_details_and_hidden_title_roundtrip_update_and_undo() {
    for presentation in ["card","icon"] {
        let spec=detailed_graph(presentation);
        let document=execute_request(json!({"op":"create_presentation","id":"detail-history","title":"Details"})).unwrap();
        let inserted=execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide-1","id":"architecture","spec":spec})).unwrap();
        let saved=execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
        let opened=execute_request(json!({"op":"open_presentation","id":"detail-open","base64":saved["base64"]})).unwrap();
        assert_eq!(opened["document"]["parts"][0]["stale"],false);
        assert_eq!(opened["document"]["parts"][0]["spec"],inserted["document"]["parts"][0]["spec"]);
        let children=opened["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap();
        assert!(children.iter().any(|child|child["type"]=="text" && child["text"]=="Validate access\nRecord outcome"));
        assert!(children.iter().all(|child|!child["id"].as_str().unwrap().ends_with("-title")));
        for (suffix,bold) in [("-nt-client",true),("-nd-client",false)] {
            let restored=children.iter().find(|child|child["id"].as_str().unwrap().ends_with(suffix)).unwrap();
            assert_eq!(restored["bold"],bold);assert_eq!(restored["format"]["alignment"],"left");
        }
        let mut node=spec["nodes"][0].clone();node["detail"]=json!("Updated detail");node["text_align"]=json!("right");
        let updated=execute_request(json!({"op":"apply_graph","document":opened["document"],"expected_revision":0,"slide_id":"slide-1","id":"architecture","operations":[{"op":"put_node","node":node}]})).unwrap();
        assert_eq!(updated["document"]["parts"][0]["stale"],false);
        let updated_saved=execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
        let reopened=execute_request(json!({"op":"open_presentation","id":"detail-updated","base64":updated_saved["base64"]})).unwrap();
        assert_eq!(reopened["document"]["parts"][0]["stale"],false);
        assert_eq!(reopened["document"]["parts"][0]["spec"]["data"]["graph"]["nodes"][0]["detail"],"Updated detail");
        let undone=execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":1,"receipt":updated["receipt"]})).unwrap();
        assert_eq!(undone["document"]["hash"],opened["document"]["hash"]);
        assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"],saved["base64"]);
    }
}

#[test]
fn graph_relationship_labels_are_transparent_and_clear_of_routes() {
    for (source, target, source_port, target_port, route) in [
        ([80, 180], [500, 180], "right", "left", "straight"),
        ([500, 180], [80, 180], "left", "right", "straight"),
        ([280, 100], [280, 360], "bottom", "top", "straight"),
        ([280, 360], [280, 100], "top", "bottom", "straight"),
        ([80, 140], [720, 360], "right", "left", "elbow"),
        ([80, 140], [720, 360], "right", "left", "straight"),
        ([80, 88], [270, 88], "right", "left", "straight"),
        ([80, 180], [256, 180], "right", "left", "straight"),
    ] {
        let spec = json!({"version":1,"title":"Clear relationship","nodes":[
            {"id":"source","label":"Source","x":source[0],"y":source[1],"width":176,"height":80},
            {"id":"target","label":"Target","x":target[0],"y":target[1],"width":176,"height":80}
        ],"edges":[{"id":"traffic","source":"source","target":"target","label":"1 HTTPS","source_port":source_port,"target_port":target_port,"route":route}]});
        let result = execute_request(json!({"op":"create_graph","id":"clear-label","spec":spec}));
        if source == [80, 180] && target == [256, 180] {
            assert!(result.unwrap_err().to_string().contains("connector path must have length"));
            continue;
        }
        let rendered = result.unwrap();
        let children = rendered["children"].as_array().unwrap();
        let label = children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-et-traffic")).unwrap();
        assert_eq!(label["type"], "text", "Relationship labels must not have a shape background or fixed shape padding");
        assert!(label.get("fill").is_none());
        let left = label["x"].as_f64().unwrap(); let top = label["y"].as_f64().unwrap();
        let right = left + label["width"].as_f64().unwrap(); let bottom = top + label["height"].as_f64().unwrap();
        assert!(left >= 0.0 && right <= 1152.0 && top >= 88.0 && bottom <= 512.0);
        let typed: aislide_core::graphs::GraphSpec = serde_json::from_value(spec).unwrap();
        let points = aislide_core::graphs::edge_points(&typed.nodes[0], &typed.nodes[1], &typed.edges[0]);
        for segment in points.windows(2) {
            for step in 0..=200 {
                let fraction = f64::from(step) / 200.0;
                let horizontal = segment[0][0] + (segment[1][0] - segment[0][0]) * fraction;
                let vertical = segment[0][1] + (segment[1][1] - segment[0][1]) * fraction;
                assert!(horizontal < left - 2.0 || horizontal > right + 2.0 || vertical < top - 2.0 || vertical > bottom + 2.0,
                    "Relationship label overlaps its {route} connector: {label}");
            }
        }
        let connector = children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-e-traffic")).unwrap();
        assert_eq!(connector["arrow"], true);
        assert!(connector["start"]["element_id"].as_str().unwrap().ends_with("-n-source"));
        assert!(connector["end"]["element_id"].as_str().unwrap().ends_with("-n-target"));
    }
}

#[test]
fn graph_relationship_labels_use_visible_icon_bounds_for_short_elbows() {
    let icon = node_icon("#007a4d");
    let spec = json!({"version":1,"title":"Short service route","nodes":[
        {"id":"endpoint","label":"Private endpoint","presentation":"icon","icon":icon,"x":352,"y":360,"width":176,"height":128,"font_size":12},
        {"id":"sql","label":"SQL Database","presentation":"icon","icon":icon,"x":620,"y":352,"width":152,"height":128,"font_size":12}
    ],"edges":[{"id":"data","source":"endpoint","target":"sql","label":"3 SQL","source_port":"right","target_port":"left","route":"elbow"}]});
    let rendered = execute_request(json!({"op":"create_graph","id":"short-label","spec":spec})).unwrap();
    let label = rendered["children"].as_array().unwrap().iter().find(|child| child["id"].as_str().unwrap().ends_with("-et-data")).unwrap();
    assert_eq!(label["type"], "text");
    let center = label["y"].as_f64().unwrap() + label["height"].as_f64().unwrap() / 2.0;
    assert!((center - 420.0).abs() <= 48.0, "Keep the label next to the short route, not below the service frames: {label}");
    let preview = execute_request(json!({"op":"render_element_preview","element":label})).unwrap();
    assert!(preview["warnings"].as_array().unwrap().iter().all(|warning| !warning["code"].as_str().unwrap().contains("OVERFLOW")), "The complete short label must remain visible: {}", preview["warnings"]);
}

fn cloud_graph() -> Value {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="#007a4d"/></svg>"##;
    let icon = execute_request(json!({"op":"create_graph_icon","base64":STANDARD.encode(svg),"mime_type":"image/svg+xml","alt":"Synthetic service"})).unwrap();
    json!({"version":1,"title":"Synthetic cloud","nodes":[
        {"id":"service","label":"Service\ninstance","presentation":"icon","kind":"rectangle","x":88,"y":296,"width":160,"height":120,"font_size":16,"group":"subnet","icon":icon},
        {"id":"peer","label":"Peer","x":560,"y":320,"width":176,"height":80,"group":"vpc"}
    ],"edges":[{"id":"traffic","source":"service","target":"peer","source_port":"right","target_port":"left"}],"groups":[
        {"id":"subnet","label":"Subnet","parent":"vpc","x":64,"y":240,"width":400,"height":224},
        {"id":"vpc","label":"VPC","parent":"region","x":48,"y":192,"width":920,"height":288},
        {"id":"region","label":"Region","parent":"cloud","x":32,"y":144,"width":960,"height":344},
        {"id":"cloud","label":"Cloud","x":16,"y":96,"width":1000,"height":408,"icon":node_icon("#0017c1")}
    ]})
}

#[test]
fn cloud_graph_icon_presentation_and_nested_boundaries() {
    use sha2::{Digest, Sha256};
    let legacy = execute_request(json!({"op":"create_graph","id":"legacy","spec":graph()})).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(serde_json::to_vec(&legacy).unwrap())), "256f078a12873dfc67e884507ea071a9ab1f8dcb7ffee599aa4bd8f5806e02ea");
    let spec = cloud_graph();
    let element = execute_request(json!({"op":"create_graph","id":"cloud-test","spec":spec})).unwrap();
    let children = element["children"].as_array().unwrap();
    let find = |suffix: &str| children.iter().find(|child| child["id"].as_str().unwrap().ends_with(suffix)).unwrap();
    let picture = find("-ni-service");
    let label = find("-nt-service");
    let anchor = find("-n-service");
    assert_eq!(picture["base64"], spec["nodes"][0]["icon"]["base64"]);
    assert_eq!(picture["width"], picture["height"]);
    assert!((56.0..=96.0).contains(&picture["height"].as_f64().unwrap()));
    assert!(picture["y"].as_f64().unwrap() + picture["height"].as_f64().unwrap() + 8.0 <= label["y"].as_f64().unwrap());
    assert!(label["height"].as_f64().unwrap() >= 38.4);
    assert_eq!(anchor["fill"], "none");
    assert_eq!(anchor["stroke_width"], 0.0);
    let edge = find("-e-traffic");
    assert_eq!(edge["start"]["element_id"], anchor["id"]);
    let order: Vec<_> = children.iter().filter_map(|child| child["id"].as_str()).filter(|id| id.contains("-g-")).map(|id| id.rsplit("-g-").next().unwrap()).collect();
    assert_eq!(order, ["cloud", "region", "vpc", "subnet"]);
    let header = find("-gi-cloud");
    assert_eq!(header["width"].as_f64().unwrap() / header["height"].as_f64().unwrap(), 2.0);
    assert!(header["x"].as_f64().unwrap() + header["width"].as_f64().unwrap() < find("-gt-cloud")["x"].as_f64().unwrap());
}

#[test]
fn cloud_graph_defaults_preserve_legacy_serialization_and_flat_order() {
    use aislide_core::graphs::{GraphPresentation, GraphSpec};
    let mut legacy = graph();
    legacy["groups"] = json!([
        {"id":"second","label":"Second","x":16,"y":104,"width":300,"height":240},
        {"id":"first","label":"First","x":500,"y":120,"width":500,"height":300}
    ]);
    let typed: GraphSpec = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(typed.nodes[0].presentation, GraphPresentation::Card);
    let serialized = serde_json::to_value(&typed).unwrap();
    assert!(serialized["nodes"].as_array().unwrap().iter().all(|node| node.get("presentation").is_none()));
    assert!(serialized["groups"].as_array().unwrap().iter().all(|group| group.get("parent").is_none() && group.get("icon").is_none()));
    let original = execute_request(json!({"op":"create_graph","id":"legacy-flat","spec":legacy})).unwrap();
    for node in legacy["nodes"].as_array_mut().unwrap() { node["presentation"] = json!("card"); }
    for group in legacy["groups"].as_array_mut().unwrap() { group["parent"] = Value::Null; group["icon"] = Value::Null; }
    let explicit: GraphSpec = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(serde_json::to_vec(&typed).unwrap(), serde_json::to_vec(&explicit).unwrap());
    assert_eq!(original, execute_request(json!({"op":"create_graph","id":"legacy-flat","spec":legacy})).unwrap());
    let boundaries: Vec<_> = original["children"].as_array().unwrap().iter().filter(|child| child["id"].as_str().unwrap().contains("-g-")).collect();
    assert!(boundaries[0]["id"].as_str().unwrap().ends_with("-g-second"));
    assert!(boundaries[1]["id"].as_str().unwrap().ends_with("-g-first"));
}

#[test]
fn cloud_graph_rejects_invalid_hierarchies_namespace_collisions_and_icons() {
    let original = cloud_graph();
    for case in ["self", "missing", "node-parent", "cycle", "depth", "containment", "header", "node-group-id", "edge-group-id", "node-edge-id", "duplicate-group", "missing-icon", "small-icon-frame", "unknown-presentation", "group-icon", "group-icon-extra", "dashed-group"] {
        let mut spec = original.clone();
        match case {
            "self" => spec["groups"][0]["parent"] = json!("subnet"),
            "missing" => spec["groups"][0]["parent"] = json!("absent"),
            "node-parent" => spec["groups"][0]["parent"] = json!("service"),
            "cycle" => spec["groups"][3]["parent"] = json!("vpc"),
            "depth" => spec["groups"].as_array_mut().unwrap().push(json!({"id":"fifth","label":"Fifth","parent":"subnet","x":80,"y":288,"width":360,"height":160})),
            "containment" => spec["groups"][0]["x"] = json!(48),
            "header" => spec["groups"][0]["y"] = json!(220),
            "node-group-id" => spec["nodes"][0]["id"] = json!("subnet"),
            "edge-group-id" => spec["edges"][0]["id"] = json!("cloud"),
            "node-edge-id" => spec["edges"][0]["id"] = json!("peer"),
            "duplicate-group" => spec["groups"][0]["id"] = json!("vpc"),
            "missing-icon" => spec["nodes"][0]["icon"] = Value::Null,
            "small-icon-frame" => spec["nodes"][0]["height"] = json!(40),
            "unknown-presentation" => spec["nodes"][0]["presentation"] = json!("image"),
            "group-icon" => spec["groups"][3]["icon"]["mime_type"] = json!("image/jpeg"),
            "group-icon-extra" => spec["groups"][3]["icon"]["url"] = json!("https://example.invalid/icon.png"),
            "dashed-group" => spec["groups"][0]["dashed"] = json!(true),
            _ => unreachable!(),
        }
        assert!(execute_request(json!({"op":"create_graph","id":"invalid-cloud","spec":spec})).is_err(), "{case}");
        assert!(execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"move","ids":["peer"],"dx":0,"dy":0}]})).is_err(), "{case}");
    }
    let mut excessive = original.clone();
    for index in 0..2 { excessive["nodes"][index]["icon"] = json!({"base64":"A".repeat(800000),"mime_type":"image/png"}); }
    for index in 0..2 { excessive["groups"][index]["icon"] = json!({"base64":"A".repeat(800000),"mime_type":"image/png"}); }
    assert!(execute_request(json!({"op":"create_graph","id":"budget","spec":excessive})).unwrap_err().to_string().contains("3 MiB"));
    let typed: aislide_core::graphs::GraphSpec = serde_json::from_value(original).unwrap();
    for invalid in [f64::NAN, f64::INFINITY] {
        assert!(aislide_core::graphs::transform(&typed, &[aislide_core::graphs::GraphOperation::Move { ids: vec!["cloud".into()], dx: invalid, dy: 0.0 }]).is_err());
    }
}

#[test]
fn cloud_graph_moves_descendants_once_and_removes_boundaries_without_resources() {
    let spec = cloud_graph();
    let moved = execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"move","ids":["cloud","region","subnet","service","peer","traffic"],"dx":8,"dy":4}]})).unwrap();
    for collection in ["nodes", "groups"] {
        for (before, after) in spec[collection].as_array().unwrap().iter().zip(moved[collection].as_array().unwrap()) {
            assert_eq!(before["id"], after["id"]);
            assert_eq!(after["x"].as_f64().unwrap(), before["x"].as_f64().unwrap() + 8.0);
            assert_eq!(after["y"].as_f64().unwrap(), before["y"].as_f64().unwrap() + 4.0);
        }
    }
    let mut replacement = spec["groups"][3].clone(); replacement["x"] = json!(24); replacement["y"] = json!(100);
    let put = execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"put_group","group":replacement}]})).unwrap();
    assert_eq!(put, moved);
    replacement["width"] = json!(100);
    assert!(execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"put_group","group":replacement}]})).is_err());
    let mut reparent = spec["groups"][0].clone(); reparent["parent"] = json!("cloud");
    let changed = execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"put_group","group":reparent}]})).unwrap();
    assert_eq!(changed["groups"][0]["parent"], "cloud");
    assert_eq!(changed["nodes"][0]["x"], 88.0);
    assert_eq!(changed["groups"][0]["width"], 400.0);
    for selection in [json!(["region"]), json!(["region","vpc"]), json!(["cloud"]), json!(["cloud","subnet"])] {
        let removed = execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"remove","ids":selection}]})).unwrap();
        assert_eq!(removed["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(removed["edges"].as_array().unwrap().len(), 1);
        for collection in ["nodes", "groups"] {
            for after in removed[collection].as_array().unwrap() {
                let before = spec[collection].as_array().unwrap().iter().find(|entry| entry["id"] == after["id"]).unwrap();
                for field in ["x", "y", "width", "height"] { assert_eq!(before[field].as_f64(), after[field].as_f64()); }
            }
        }
        if selection == json!(["region","vpc"]) { assert_eq!(removed["groups"][0]["parent"], "cloud"); assert_eq!(removed["nodes"][1]["group"], "cloud"); }
        if selection == json!(["cloud"]) { assert!(removed["groups"][2].get("parent").is_none()); }
    }
    let deleted = execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"remove","ids":["region","service"]}]})).unwrap();
    assert_eq!(deleted["nodes"].as_array().unwrap().len(), 1);
    assert!(deleted["edges"].as_array().unwrap().is_empty());
    for operation in [json!({"op":"move","ids":["cloud"],"dx":0,"dy":20}), json!({"op":"move","ids":["unknown"],"dx":0,"dy":0}), json!({"op":"move","ids":["service","service"],"dx":0,"dy":0})] {
        assert!(execute_request(json!({"op":"transform_graph","spec":spec,"operations":[operation]})).is_err());
    }
    assert!(execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"layout","columns":2}]})).unwrap_err().to_string().contains("nested"));
}

#[test]
fn cloud_graph_limits_include_sixteen_groups_and_rendered_children() {
    let catalog = execute_request(json!({"op":"graph_catalog"})).unwrap();
    assert_eq!(catalog["limits"], json!({"nodes":48,"edges":64,"groups":16,"group_depth":4,"rendered_elements":256,"waypoints_per_edge":16}));
    let mut spec = graph();
    spec["groups"] = json!((0..16).map(|index| json!({"id":format!("boundary-{index}"),"label":"Boundary","x":16,"y":104,"width":300,"height":240})).collect::<Vec<_>>());
    execute_request(json!({"op":"create_graph","id":"sixteen","spec":spec})).unwrap();
    spec["groups"].as_array_mut().unwrap().push(json!({"id":"excess","label":"Excess","x":16,"y":104,"width":300,"height":240}));
    assert!(execute_request(json!({"op":"create_graph","id":"seventeen","spec":spec})).is_err());
    spec["groups"].as_array_mut().unwrap().pop();
    let icon = node_icon("#007a4d");
    spec["nodes"] = json!((0..48).map(|index| json!({"id":format!("node-{index}"),"label":"Node","x":if index % 2 == 0 { 48 } else { 560 },"y":160,"width":160,"height":120,"font_size":16,"presentation":"icon","icon":icon})).collect::<Vec<_>>());
    spec["edges"] = json!((0..64).map(|index| json!({"id":format!("edge-{index}"),"source":"node-0","target":"node-1"})).collect::<Vec<_>>());
    execute_request(json!({"op":"create_graph","id":"within-rendered-limit","spec":spec})).unwrap();
    for group in spec["groups"].as_array_mut().unwrap() { group["icon"] = icon.clone(); }
    assert!(execute_request(json!({"op":"create_graph","id":"rendered-limit","spec":spec})).unwrap_err().to_string().contains("too many scene elements"));
}

#[test]
fn cloud_graph_icon_images_preserve_source_aspect_and_all_kind_ports() {
    use sha2::{Digest, Sha256};
    for (format, mime) in [(image::ImageFormat::Png, "image/png"), (image::ImageFormat::Jpeg, "image/jpeg")] {
        for (width, height) in [(512, 256), (256, 512), (256, 256)] {
            let raster = image::RgbImage::from_pixel(width, height, image::Rgb([0, 96, 192]));
            let mut output = std::io::Cursor::new(Vec::new()); raster.write_to(&mut output, format).unwrap();
            let bytes = output.into_inner(); let encoded = STANDARD.encode(&bytes);
            let mut spec = cloud_graph();
            spec["nodes"][0]["icon"] = json!({"base64":encoded,"mime_type":mime,"alt":"Original raster"});
            spec["groups"][3]["icon"] = spec["nodes"][0]["icon"].clone();
            let element = execute_request(json!({"op":"create_graph","id":"aspect","spec":spec})).unwrap();
            for picture in element["children"].as_array().unwrap().iter().filter(|child| child["type"] == "picture") {
                assert_eq!(Sha256::digest(STANDARD.decode(picture["base64"].as_str().unwrap()).unwrap()), Sha256::digest(&bytes));
                assert_eq!(picture["width"].as_f64().unwrap() / picture["height"].as_f64().unwrap(), width as f64 / height as f64);
                assert!(picture["width"].as_f64().unwrap() <= 96.0 && picture["height"].as_f64().unwrap() <= 96.0);
            }
        }
    }
    for kind in ["rectangle", "rounded_rectangle", "ellipse", "diamond", "cylinder", "cloud"] {
        let mut spec = cloud_graph(); spec["nodes"][0]["kind"] = json!(kind);
        let icon = execute_request(json!({"op":"create_graph","id":"kind","spec":spec})).unwrap();
        spec["nodes"][0]["presentation"] = json!("card"); spec["nodes"][0]["label"] = json!("Node");
        let card = execute_request(json!({"op":"create_graph","id":"kind","spec":spec})).unwrap();
        let edge = |element: &Value| element["children"].as_array().unwrap().iter().find(|child| child["type"] == "connector").unwrap().clone();
        for field in ["x", "y", "width", "height", "routing"] { assert_eq!(edge(&icon)[field], edge(&card)[field], "{kind}"); }
        assert_eq!(edge(&icon)["start"]["site"], edge(&card)["start"]["site"], "{kind}");
    }
}

#[test]
fn cloud_graph_native_roundtrip_child_update_ungroup_and_exact_undo() {
    let spec = cloud_graph();
    let document = execute_request(json!({"op":"create_presentation","id":"cloud-history","title":"Synthetic cloud"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide-1","id":"architecture","spec":spec})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"cloud-open","base64":saved["base64"]})).unwrap();
    assert_eq!(opened["document"]["parts"][0]["stale"], false);
    assert_eq!(opened["document"]["parts"][0]["spec"]["data"]["graph"], inserted["document"]["parts"][0]["spec"]["data"]["graph"]);
    assert_eq!(execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap()["base64"], saved["base64"]);
    let updated = execute_request(json!({"op":"apply_graph","document":opened["document"],"expected_revision":0,"slide_id":"slide-1","id":"architecture","operations":[{"op":"move","ids":["cloud","region","subnet","service"],"dx":8,"dy":4},{"op":"move","ids":["service"],"dx":8,"dy":0}]})).unwrap();
    assert_eq!(updated["document"]["revision"], 1);
    let updated_saved = execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"cloud-updated","base64":updated_saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["parts"][0]["stale"], false);
    assert_eq!(reopened["document"]["parts"][0]["spec"]["data"]["graph"]["nodes"][0]["x"], 104.0);
    let removed = execute_request(json!({"op":"apply_graph","document":reopened["document"],"expected_revision":0,"slide_id":"slide-1","id":"architecture","operations":[{"op":"remove","ids":["region","vpc"]}]})).unwrap();
    let removed_saved = execute_request(json!({"op":"export_presentation","document":removed["document"]})).unwrap();
    let removed_open = execute_request(json!({"op":"open_presentation","id":"cloud-ungrouped","base64":removed_saved["base64"]})).unwrap();
    assert_eq!(removed_open["document"]["parts"][0]["stale"], false);
    let children = removed_open["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap();
    let edge = children.iter().find(|child| child["type"] == "connector").unwrap();
    for (endpoint, suffix, frame) in [("start", "-n-service", [104.0, 300.0, 160.0, 120.0]), ("end", "-n-peer", [568.0, 324.0, 176.0, 80.0])] {
        let target = children.iter().find(|child| child["id"] == edge[endpoint]["element_id"]).unwrap();
        assert!(target["id"].as_str().unwrap().ends_with(suffix));
        for (field, expected) in ["x", "y", "width", "height"].into_iter().zip(frame) { assert_eq!(target[field].as_f64().unwrap(), expected); }
    }
    let package = Package::open(STANDARD.decode(removed_saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap();
    let native = roxmltree::Document::parse(xml).unwrap();
    let connector = native.descendants().find(|node| node.tag_name().name() == "cxnSp").unwrap();
    for connection in ["stCxn", "endCxn"] {
        let target_id = connector.descendants().find(|node| node.tag_name().name() == connection).unwrap().attribute("id").unwrap();
        let target = native.descendants().find(|node| node.tag_name().name() == "sp" && node.descendants().any(|property| property.tag_name().name() == "cNvPr" && property.attribute("id") == Some(target_id))).unwrap();
        assert!(target.descendants().any(|node| node.tag_name().name() == "prstGeom" && node.attribute("prst") == Some("rect")));
        if connection == "stCxn" { assert!(target.descendants().any(|node| node.tag_name().name() == "noFill")); }
    }
    let restored_graph = &removed_open["document"]["parts"][0]["spec"]["data"]["graph"];
    assert_eq!(restored_graph["groups"][0]["parent"], "cloud");
    let mut expected_nodes = reopened["document"]["parts"][0]["spec"]["data"]["graph"]["nodes"].clone();
    expected_nodes[1]["group"] = json!("cloud");
    assert_eq!(restored_graph["nodes"], expected_nodes);
    let undone_remove = execute_request(json!({"op":"undo_transaction","document":removed["document"],"expected_revision":1,"receipt":removed["receipt"]})).unwrap();
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undone_remove["document"]})).unwrap()["base64"], updated_saved["base64"]);
    let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":1,"receipt":updated["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], opened["document"]["hash"]);
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], saved["base64"]);
}

#[test]
fn graph_icon_preparation_bounds_svg_png_and_jpeg_without_upscaling_rasters() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16"><path d="M0 0h32v16H0z" fill="#007a4d"/></svg>"##;
    let prepared = execute_request(json!({"op":"create_graph_icon","base64":STANDARD.encode(svg),"mime_type":"image/svg+xml","alt":"Prepared SVG"})).unwrap();
    assert_eq!(prepared["mime_type"], "image/png");
    let info = aislide_core::media::inspect_raster(prepared["base64"].as_str().unwrap(), "image/png").unwrap();
    assert_eq!((info.width, info.height), (256, 128));
    for (format, mime) in [(image::ImageFormat::Png, "image/png"), (image::ImageFormat::Jpeg, "image/jpeg")] {
        for width in [64, 512] {
            let raster = image::RgbImage::from_pixel(width, width / 2, image::Rgb([0, 96, 192]));
            let mut output = std::io::Cursor::new(Vec::new());
            raster.write_to(&mut output, format).unwrap();
            let encoded = STANDARD.encode(output.into_inner());
            let icon = execute_request(json!({"op":"create_graph_icon","base64":encoded,"mime_type":mime,"alt":"Raster icon"})).unwrap();
            assert_eq!(icon["mime_type"], mime);
            assert_eq!(icon["alt"], "Raster icon");
            let info = aislide_core::media::inspect_raster(icon["base64"].as_str().unwrap(), mime).unwrap();
            assert_eq!((info.width, info.height), (width.min(256), width.min(256) / 2));
            if width < 256 { assert_eq!(icon["base64"], encoded); }
        }
    }
    assert!(execute_request(json!({"op":"create_graph_icon","base64":STANDARD.encode("<svg xmlns='http://www.w3.org/2000/svg'><script/></svg>"),"mime_type":"image/svg+xml"})).is_err());
}

#[test]
fn graph_node_icons_render_as_native_pictures_with_separate_labels_and_connections() {
    let mut spec = graph();
    spec["nodes"][0]["icon"] = node_icon("#0017c1");
    let element = execute_request(json!({"op":"create_graph","id":"icons","spec":spec})).unwrap();
    let children = element["children"].as_array().unwrap();
    let picture = children.iter().find(|child| child["type"] == "picture").unwrap();
    let label = children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-nt-client")).unwrap();
    let connector = children.iter().find(|child| child["type"] == "connector").unwrap();
    assert_eq!(picture["base64"], spec["nodes"][0]["icon"]["base64"]);
    assert_eq!(picture["alt"], "Client icon");
    assert_eq!(picture["width"].as_f64().unwrap() / picture["height"].as_f64().unwrap(), 2.0);
    assert!(picture["x"].as_f64().unwrap() >= 48.0);
    assert!(picture["y"].as_f64().unwrap() >= 160.0);
    assert!(picture["y"].as_f64().unwrap() + picture["height"].as_f64().unwrap() <= 256.0);
    assert!(picture["x"].as_f64().unwrap() + picture["width"].as_f64().unwrap() < label["x"].as_f64().unwrap());
    assert!(label["x"].as_f64().unwrap() + label["width"].as_f64().unwrap() <= 248.0);
    assert!(connector["start"]["element_id"].as_str().unwrap().ends_with("-n-client"));
    let saved = execute_request(json!({"op":"export","deck":deck(element)})).unwrap();
    let bytes = STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap();
    let package = Package::open(bytes).unwrap();
    assert!(package.text("ppt/slides/slide1.xml").unwrap().contains("<p:pic>"));
    assert_eq!(package.parts().keys().filter(|name| name.starts_with("ppt/media/")).count(), 1);
    let opened = execute_request(json!({"op":"open_presentation","id":"icon-open","base64":saved["base64"]})).unwrap();
    let restored = opened["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().iter().find(|child| child["type"] == "picture").unwrap();
    assert_eq!(restored["base64"], spec["nodes"][0]["icon"]["base64"]);
    let mut default_width = graph();
    default_width["nodes"][0]["width"] = json!(176);
    default_width["nodes"][0]["label"] = json!("Application");
    default_width["nodes"][0]["icon"] = node_icon("#0017c1");
    let compact = execute_request(json!({"op":"create_graph","id":"compact","spec":default_width})).unwrap();
    let compact_label = compact["children"].as_array().unwrap().iter().find(|child| child["id"].as_str().unwrap().ends_with("-nt-client")).unwrap();
    assert!(compact_label["width"].as_f64().unwrap() >= 97.0);
}

#[test]
fn graph_icons_reject_invalid_images_and_preserve_icon_free_output() {
    let icon = node_icon("#007a4d");
    for replacement in [
        json!({"base64":"https://example.invalid/icon.png","mime_type":"image/png","alt":"URL"}),
        json!({"base64":STANDARD.encode("<svg/>"),"mime_type":"image/svg+xml","alt":"Raw SVG"}),
        json!({"base64":icon["base64"],"mime_type":"image/jpeg","alt":"Wrong MIME"}),
        json!({"base64":icon["base64"],"mime_type":"image/png","alt":"x".repeat(501)}),
        json!({"base64":icon["base64"],"mime_type":"image/png","alt":"External","url":"https://example.invalid/icon.png"}),
    ] {
        let mut spec = graph(); spec["nodes"][0]["icon"] = replacement;
        assert!(execute_request(json!({"op":"create_graph","id":"bad-icon","spec":spec})).is_err());
        assert!(execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"move","ids":["client"],"dx":8,"dy":0}]})).is_err());
    }
    let mut excessive = graph();
    excessive["nodes"] = json!((0..4).map(|index| json!({"id":format!("node-{index}"),"label":"Node","x":48,"y":160,"icon":{"base64":"A".repeat(800000),"mime_type":"image/png"}})).collect::<Vec<_>>());
    excessive["edges"] = json!([]);
    assert!(execute_request(json!({"op":"create_graph","id":"budget","spec":excessive})).unwrap_err().to_string().contains("3 MiB"));
    let original = execute_request(json!({"op":"create_graph","id":"unchanged","spec":graph()})).unwrap();
    let mut without_icon = graph(); without_icon["nodes"][0]["icon"] = Value::Null;
    assert_eq!(original, execute_request(json!({"op":"create_graph","id":"unchanged","spec":without_icon})).unwrap());
}

#[test]
fn graph_icons_follow_nodes_for_all_shapes_without_changing_connection_sites() {
    let icon = node_icon("#0017c1");
    for kind in ["rectangle", "rounded_rectangle", "ellipse", "diamond", "cylinder", "cloud"] {
        let mut spec = graph(); spec["nodes"][0]["kind"] = json!(kind);
        let plain = execute_request(json!({"op":"create_graph","id":"plain","spec":spec})).unwrap();
        spec["nodes"][0]["icon"] = icon.clone();
        let original = execute_request(json!({"op":"create_graph","id":"icon","spec":spec})).unwrap();
        let children = original["children"].as_array().unwrap();
        let picture = children.iter().find(|child| child["type"] == "picture").unwrap();
        let plain_edge = plain["children"].as_array().unwrap().iter().find(|child| child["type"] == "connector").unwrap();
        let icon_edge = children.iter().find(|child| child["type"] == "connector").unwrap();
        assert_eq!(plain_edge["start"]["site"], icon_edge["start"]["site"], "{kind}");
        assert_eq!(plain_edge["x"], icon_edge["x"], "{kind}");
        let moved = execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"move","ids":["client"],"dx":24,"dy":16}]})).unwrap();
        assert_eq!(moved["nodes"][0]["icon"], icon);
        let rendered = execute_request(json!({"op":"create_graph","id":"moved","spec":moved})).unwrap();
        let shifted = rendered["children"].as_array().unwrap().iter().find(|child| child["type"] == "picture").unwrap();
        assert_eq!(shifted["x"].as_f64().unwrap(), picture["x"].as_f64().unwrap() + 24.0);
        assert_eq!(shifted["y"].as_f64().unwrap(), picture["y"].as_f64().unwrap() + 16.0);
    }
}

#[test]
fn graph_icon_metadata_reopens_replaces_removes_and_undoes_with_stale_image_protection() {
    let mut spec = graph(); spec["nodes"][0]["icon"] = node_icon("#0017c1");
    let document = execute_request(json!({"op":"create_presentation","id":"graph-icons","title":"Icon history"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide-1","id":"architecture","spec":spec})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"icons-open","base64":saved["base64"]})).unwrap();
    assert_eq!(opened["document"]["parts"][0]["stale"], false);
    assert_eq!(opened["document"]["parts"][0]["spec"]["data"]["graph"]["nodes"][0]["icon"], spec["nodes"][0]["icon"]);
    let changed_icon = node_icon("#007a4d");
    let mut replacement = spec.clone(); replacement["nodes"][0]["icon"] = changed_icon.clone();
    let updated = execute_request(json!({"op":"update_graph","document":opened["document"],"expected_revision":0,"slide_id":"slide-1","id":"architecture","spec":replacement})).unwrap();
    let updated_saved = execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
    let updated_opened = execute_request(json!({"op":"open_presentation","id":"icons-updated","base64":updated_saved["base64"]})).unwrap();
    assert_eq!(updated_opened["document"]["parts"][0]["stale"], false);
    assert_eq!(updated_opened["document"]["parts"][0]["spec"]["data"]["graph"]["nodes"][0]["icon"], changed_icon);
    replacement["nodes"][0]["icon"] = Value::Null;
    let removed = execute_request(json!({"op":"update_graph","document":updated["document"],"expected_revision":1,"slide_id":"slide-1","id":"architecture","spec":replacement})).unwrap();
    assert!(removed["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().iter().all(|child| child["type"] != "picture"));
    let undone = execute_request(json!({"op":"undo_transaction","document":removed["document"],"expected_revision":2,"receipt":removed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], updated["document"]["hash"]);
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], updated_saved["base64"]);
    let mut package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    let image_path = package.parts().keys().find(|name| name.starts_with("ppt/media/")).unwrap().clone();
    package.replace_part(&image_path, STANDARD.decode(changed_icon["base64"].as_str().unwrap()).unwrap()).unwrap();
    let external = execute_request(json!({"op":"open_presentation","id":"external-icon","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    assert_eq!(external["document"]["parts"][0]["stale"], true);
    assert!(execute_request(json!({"op":"update_graph","document":external["document"],"expected_revision":0,"slide_id":"slide-1","id":"architecture","spec":spec})).unwrap_err().to_string().contains("stale"));
}

#[test]
fn graph_api_creates_native_attached_nodes_and_recomputes_moved_endpoints() {
    let spec = graph();
    let first = execute_request(json!({"op":"create_graph","id":"topology","spec":spec})).unwrap();
    assert_eq!(first["type"], "group");
    assert_eq!(first, execute_request(json!({"op":"create_graph","id":"topology","spec":spec})).unwrap());
    let edge = first["children"].as_array().unwrap().iter().find(|element| element["type"] == "connector").unwrap();
    assert!(edge["start"]["element_id"].is_string());
    assert!(edge["end"]["element_id"].is_string());
    assert_eq!(edge["x"], 248.0);
    assert_eq!(edge["width"], 312.0);
    let mut moved = spec;
    moved["nodes"][1]["x"] = json!(720);
    let result = execute_request(json!({"op":"create_graph","id":"topology","spec":moved})).unwrap();
    let moved_edge = result["children"].as_array().unwrap().iter().find(|element| element["type"] == "connector").unwrap();
    assert_eq!(moved_edge["width"], 472.0);
    let exported = execute_request(json!({"op":"export","deck":deck(first)})).unwrap();
    let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap();
    assert!(xml.contains("<p:cxnSp>"));
    assert!(xml.contains("<a:stCxn"));
    assert!(xml.contains("<a:endCxn"));
    assert!(!package.parts().keys().any(|path| path.starts_with("ppt/media/")));
}

#[test]
fn graph_contract_rejects_ambiguous_ids_missing_endpoints_and_excessive_counts() {
    for modify in ["duplicate", "endpoint", "bounds", "count", "group"] {
        let mut spec = graph();
        match modify {
            "duplicate" => spec["nodes"][1]["id"] = json!("client"),
            "endpoint" => spec["edges"][0]["target"] = json!("missing"),
            "bounds" => spec["nodes"][0]["x"] = json!(-1),
            "count" => spec["nodes"] = json!((0..49).map(|index| json!({"id":format!("node-{index}"),"label":"Node","kind":"rectangle","x":48,"y":160,"width":200,"height":96})).collect::<Vec<_>>()),
            "group" => spec["nodes"][0]["group"] = json!("missing"),
            _ => unreachable!(),
        }
        assert!(execute_request(json!({"op":"create_graph","id":"invalid","spec":spec})).is_err(), "{modify}");
    }
    let catalog = execute_request(json!({"op":"graph_catalog"})).unwrap();
    assert_eq!(catalog["limits"]["nodes"], 48);
    assert!(catalog["examples"].as_array().is_some_and(|examples| examples.len() >= 3));
    assert_eq!(execute_request(json!({"op":"part_catalog"})).unwrap()["presets"].as_array().unwrap().len(), 111);
}

#[test]
fn routed_reverse_edges_preserve_geometry_arrows_and_native_connections() {
    let mut spec = graph();
    spec["edges"][0] = json!({"id":"reply","source":"api","target":"client","source_port":"left","target_port":"right","route":"elbow","label":"Reply","start_arrow":true,"dashed":true});
    let element = execute_request(json!({"op":"create_graph","id":"reverse","spec":spec})).unwrap();
    let edge = element["children"].as_array().unwrap().iter().find(|element| element["type"] == "connector").unwrap();
    let points = edge["routing"]["points"].as_array().unwrap();
    assert!(points.len() >= 3);
    assert_eq!(points.first().unwrap()[0], 1.0);
    assert_eq!(points.last().unwrap()[0], 0.0);
    assert!(points.windows(2).all(|pair| pair[0][0] == pair[1][0] || pair[0][1] == pair[1][1]));
    let exported = execute_request(json!({"op":"export","deck":deck(element.clone())})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"reverse-open","base64":exported["base64"]})).unwrap();
    let restored = opened["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().iter().find(|element| element["type"] == "connector").unwrap();
    assert_eq!(restored["routing"], edge["routing"]);
    assert_eq!(restored["arrow"], true);
    let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap();
    assert!(xml.contains("<a:prstGeom prst=\"bentConnector3\""));
    assert!(xml.contains("<a:headEnd type=\"triangle\""));
    assert!(xml.contains("<a:prstDash val=\"dash\""));
}

#[test]
fn graph_routes_use_office_presets_in_every_port_direction() {
    for route in ["straight", "elbow"] {
        for source_port in ["top", "left", "bottom", "right"] {
            for target_port in ["top", "left", "bottom", "right"] {
                let mut spec = graph();
                spec["edges"][0] = json!({"id":"route","source":"api","target":"client","source_port":source_port,"target_port":target_port,"route":route,"start_arrow":true,"dashed":true});
                let element = execute_request(json!({"op":"create_graph","id":"ports","spec":spec})).unwrap();
                let edge = element["children"].as_array().unwrap().iter().find(|element| element["type"] == "connector").unwrap();
                let exported = execute_request(json!({"op":"export","deck":deck(element.clone())})).unwrap();
                let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
                let xml = package.text("ppt/slides/slide1.xml").unwrap();
                assert!(!xml.contains("<a:custGeom>"), "{route} {source_port} {target_port}");
                assert!(xml.contains("Connector"), "{route} {source_port} {target_port}");
                let opened = execute_request(json!({"op":"open_presentation","id":"ports-open","base64":exported["base64"]})).unwrap();
                let restored = opened["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().iter().find(|element| element["type"] == "connector").unwrap();
                assert_eq!(restored["routing"], edge["routing"], "{route} {source_port} {target_port}");
                assert_eq!(restored["start"], edge["start"]);
                assert_eq!(restored["end"], edge["end"]);
                assert_eq!(restored["arrow"], true);
                assert_eq!(restored["flip_v"], false);
            }
        }
    }
}

#[test]
fn routed_connectors_reject_unrepresentable_paths_before_export() {
    for points in [
        json!([[0,0],[0.25,0.25],[1,1]]),
        json!([[0.1,0],[1,1]]),
        json!([[0,0],[0.123456,0],[0.123456,1],[1,1]]),
        json!([[0,0],[0.25,0],[0.25,0.75],[1,0.75],[1,1]]),
    ] {
        let mut element = execute_request(json!({"op":"create_graph","id":"invalid-route","spec":graph()})).unwrap();
        let edge = element["children"].as_array_mut().unwrap().iter_mut().find(|element| element["type"] == "connector").unwrap();
        edge["routing"]["points"] = points;
        assert!(execute_request(json!({"op":"export","deck":deck(element)})).is_err());
    }
}

#[test]
fn graph_ports_match_native_shape_connection_sites() {
    use aislide_core::graphs::{GraphNode, Port, endpoint};
    let other: GraphNode = serde_json::from_value(graph()["nodes"][1].clone()).unwrap();
    for (kind, sites, positions) in [
        ("rectangle", [0,1,2,3], [[0.5,0.0],[0.0,0.5],[0.5,1.0],[1.0,0.5]]),
        ("rounded_rectangle", [0,1,2,3], [[0.5,0.0],[0.0,0.5],[0.5,1.0],[1.0,0.5]]),
        ("diamond", [0,1,2,3], [[0.5,0.0],[0.0,0.5],[0.5,1.0],[1.0,0.5]]),
        ("ellipse", [0,2,4,6], [[0.5,0.0],[0.0,0.5],[0.5,1.0],[1.0,0.5]]),
        ("cylinder", [1,2,3,4], [[0.5,0.0],[0.0,0.5],[0.5,1.0],[1.0,0.5]]),
        ("cloud", [3,2,1,0], [[0.5,1235.0/21600.0],[67.0/21600.0,0.5],[0.5,21577.0/21600.0],[21582.0/21600.0,0.5]]),
    ] {
        let node: GraphNode = serde_json::from_value(json!({"id":"node","label":"Node","kind":kind,"x":48,"y":160,"width":200,"height":96})).unwrap();
        for (index, port) in [Port::Top, Port::Left, Port::Bottom, Port::Right].into_iter().enumerate() {
            let actual = endpoint(&node, port, &other);
            assert_eq!(actual.2, sites[index], "{kind} {port:?}");
            assert!((actual.0 - node.x - positions[index][0] * node.width).abs() < 1e-9);
            assert!((actual.1 - node.y - positions[index][1] * node.height).abs() < 1e-9);
            let mut spec = graph(); spec["nodes"][0]["kind"] = json!(kind);
            spec["edges"][0]["source_port"] = serde_json::to_value(port).unwrap();
            let element = execute_request(json!({"op":"create_graph","id":"sites","spec":spec})).unwrap();
            let saved = execute_request(json!({"op":"export","deck":deck(element)})).unwrap();
            execute_request(json!({"op":"open_presentation","id":"sites-open","base64":saved["base64"]})).unwrap();
        }
    }
}

#[test]
fn element_commands_duplicate_managed_graphs_and_remove_attached_edges_atomically() {
    let document = execute_request(json!({"op":"create_presentation","id":"element-commands","title":"Objects"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide-1","id":"architecture","spec":graph()})).unwrap();
    let duplicated = execute_request(json!({"op":"edit_elements","document":inserted["document"],"expected_revision":1,"slide_id":"slide-1","operations":[{"op":"duplicate","id":"architecture","new_id":"copy"}]})).unwrap();
    assert_eq!(duplicated["document"]["parts"].as_array().unwrap().len(), 2);
    assert!(duplicated["document"]["parts"].as_array().unwrap().iter().all(|part| part["stale"] == false));
    let removed = execute_request(json!({"op":"edit_elements","document":duplicated["document"],"expected_revision":2,"slide_id":"slide-1","operations":[{"op":"remove","id":"copy"}]})).unwrap();
    assert_eq!(removed["document"]["parts"].as_array().unwrap().len(), 1);
    let undone = execute_request(json!({"op":"undo_transaction","document":removed["document"],"expected_revision":3,"receipt":removed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], duplicated["document"]["hash"]);
}

#[test]
fn managed_graphs_share_parts_history_and_preserve_native_edits() {
    let mut scene = deck(json!({})); scene["slides"][0]["elements"] = json!([]);
    let document = execute_request(json!({"op":"new_document","id":"managed","deck":scene})).unwrap();
    let inserted = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide","id":"architecture","spec":graph()})).unwrap();
    assert_eq!(inserted["document"]["parts"][0]["spec"]["preset"], "diagram/custom");
    let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"managed-open","base64":saved["base64"]})).unwrap();
    assert_eq!(opened["document"]["parts"][0]["stale"], false);
    let mut spec = graph(); spec["nodes"][1]["x"] = json!(720); spec["edges"][0]["route"] = json!("elbow");
    let updated = execute_request(json!({"op":"update_graph","document":opened["document"],"expected_revision":0,"slide_id":"slide","id":"architecture","spec":spec})).unwrap();
    let updated_saved = execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
    let updated_opened = execute_request(json!({"op":"open_presentation","id":"updated","base64":updated_saved["base64"]})).unwrap();
    assert_eq!(updated_opened["document"]["parts"][0]["stale"], false);
    assert_eq!(updated_opened["document"]["parts"][0]["spec"]["data"]["graph"]["nodes"][1]["x"], 720.0);
    let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":1,"receipt":updated["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], opened["document"]["hash"]);
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], saved["base64"]);
    let mut package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().replace(">API<", ">Changed externally<");
    package.replace_part("ppt/slides/slide1.xml", xml.into_bytes()).unwrap();
    let external = execute_request(json!({"op":"open_presentation","id":"external","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    assert_eq!(external["document"]["parts"][0]["stale"], true);
    assert!(execute_request(json!({"op":"update_graph","document":external["document"],"expected_revision":0,"slide_id":"slide","id":"architecture","spec":graph()})).unwrap_err().to_string().contains("stale"));
}

#[test]
fn graph_operations_move_groups_align_and_remove_incident_edges_atomically() {
    let mut spec = graph();
    spec["groups"] = json!([{"id":"zone","label":"Boundary","x":16,"y":104,"width":300,"height":240}]);
    spec["nodes"][0]["group"] = json!("zone");
    let moved = execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"move","ids":["zone","client"],"dx":32,"dy":8}]})).unwrap();
    assert_eq!(moved["nodes"][0]["x"], 80.0);
    assert_eq!(moved["groups"][0]["x"], 48.0);
    let removed = execute_request(json!({"op":"transform_graph","spec":moved,"operations":[{"op":"remove","ids":["client"]}]})).unwrap();
    assert_eq!(removed["nodes"].as_array().unwrap().len(), 1);
    assert!(removed["edges"].as_array().unwrap().is_empty());
    let aligned = execute_request(json!({"op":"transform_graph","spec":graph(),"operations":[{"op":"align","ids":["client","api"],"alignment":"top"}]})).unwrap();
    assert_eq!(aligned["nodes"][0]["y"], aligned["nodes"][1]["y"]);
    let arranged = execute_request(json!({"op":"transform_graph","spec":graph(),"operations":[{"op":"layout","columns":2}]})).unwrap();
    assert_ne!(arranged["nodes"][0]["x"], arranged["nodes"][1]["x"]);
    execute_request(json!({"op":"create_graph","id":"arranged","spec":arranged})).unwrap();
    assert!(execute_request(json!({"op":"transform_graph","spec":graph(),"operations":[{"op":"move","ids":["client"],"dx":10000,"dy":0}]})).is_err());
}