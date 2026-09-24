use aislide_core::execute_request;
use serde_json::json;

#[test]
fn explicit_rich_run_sizes_do_not_inherit_a_larger_unused_frame_size() {
    let mut deck = json!({"version":1,"title":"Run metrics","width":1280,"height":720,"slides":[{"id":"slide","title":"Run metrics","background":"FFFFFF","notes":"","elements":[
        {"type":"text","id":"small-runs","x":50,"y":50,"width":600,"height":20,"text":"Small explicit text","font_size":28,"color":"202525","bold":false,"format":{"paragraphs":[{"runs":[{"text":"Small explicit text","style":{"font_size":12}}]}]}}
    ]}]});
    let measured = execute_request(json!({"op":"measure_layout","deck":deck})).unwrap();
    assert_eq!(measured["measurements"][0]["overflow"], false, "{measured}");
    assert!(measured["measurements"][0]["measured_height"].as_f64().unwrap() <= 20.0);
    deck["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"][0]["style"]["font_size"] = json!(48);
    let measured = execute_request(json!({"op":"measure_layout","deck":deck})).unwrap();
    assert_eq!(measured["measurements"][0]["overflow"], true);
}

#[test]
fn layout_measurement_detects_overflow_without_claiming_office_parity() {
    let deck = json!({"version":1,"title":"Layout","width":1280,"height":720,"slides":[{"id":"slide","title":"Layout","background":"FFFFFF","notes":"","elements":[
        {"type":"text","id":"fits","x":50,"y":50,"width":1100,"height":100,"text":"Short readable title","font_size":32,"color":"202525","bold":true},
        {"type":"text","id":"overflows","x":50,"y":200,"width":100,"height":20,"text":"This text intentionally cannot fit inside such a narrow and short box.","font_size":40,"color":"202525","bold":false}
    ]}]});
    let result = execute_request(json!({"op":"measure_layout","deck":deck})).unwrap();
    assert_eq!(result["office_parity_verified"], false);
    assert!(result["measurements"].as_array().unwrap().iter().any(|measurement| measurement["element_id"] == "overflows" && measurement["overflow"] == true));
    assert!(result["measurements"].as_array().unwrap().iter().any(|measurement| measurement["element_id"] == "fits" && measurement["overflow"] == false));
    assert!(result["issues"].as_array().unwrap().iter().any(|issue| issue["code"] == "TEXT_OVERFLOW"));
}

#[test]
fn native_report_text_and_tables_have_measured_font_evidence() {
    let report = execute_request(json!({"op":"sample"})).unwrap();
    let compiled = execute_request(json!({"op":"compile","report":report})).unwrap();
    let result = execute_request(json!({"op":"measure_layout","deck":compiled["deck"]})).unwrap();
    assert!(result["measurements"].as_array().unwrap().len() > 50);
    assert!(!result["fonts"].as_array().unwrap().is_empty());
    assert_eq!(result["engine"], "cosmic-text");
}

#[test]
fn maximum_mapped_data_rows_fit_the_generated_table_cells() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut csv = String::from("Category,Value\n");
    for index in 0..32 { csv.push_str(&format!("Item {},{}\n", index + 1, index * 100)); }
    let source = execute_request(json!({"op":"ingest","input":{"name":"provided.csv","format":"csv","base64":STANDARD.encode(csv.as_bytes())}})).unwrap();
    let report = execute_request(json!({"op":"data_report","source":source,"mapping":{"title":"Provided rows","period":"Input","table_index":0,"category_column":0,"value_columns":[1],"row_start":0,"row_count":32,"chart_kind":"column"}})).unwrap();
    let measured = execute_request(json!({"op":"measure_layout","deck":report["compiled"]["deck"]})).unwrap();
    let overflowing_tables: Vec<_> = measured["measurements"].as_array().unwrap().iter().filter(|measurement| measurement["element_id"].as_str().unwrap().starts_with("data-table") && measurement["overflow"] == true).collect();
    assert!(overflowing_tables.is_empty(), "{} table cells overflowed", overflowing_tables.len());
}

#[test]
fn long_metric_values_are_measured_into_their_fixed_frames() {
    let mut report = execute_request(json!({"op":"sample"})).unwrap();
    report["sections"][2]["metrics"] = json!([
        {"label":"Source","value":"SYNTHETIC TEST DATA"},
        {"label":"Scope","value":"Q1-Q4"},
        {"label":"Boundary","value":"12345678901234567890"},
        {"label":"Units","value":"arbitrary units"}
    ]);
    let compiled = execute_request(json!({"op":"compile","report":report})).unwrap();
    let measured = execute_request(json!({"op":"measure_layout","deck":compiled["deck"]})).unwrap();
    assert!(!measured["measurements"].as_array().unwrap().iter().any(|entry| entry["element_id"].as_str().unwrap().starts_with("metric-value") && entry["overflow"] == true));
}

fn measure_elements(elements: serde_json::Value) -> serde_json::Value {
    execute_request(json!({"op":"measure_layout","deck":{"version":1,"title":"Cell metrics","width":1280,"height":720,"slides":[
        {"id":"slide","title":"Cell metrics","background":"FFFFFF","notes":"","elements":elements}
    ]}})).unwrap()
}

fn assert_same_text_measurement(actual: &serde_json::Value, expected: &serde_json::Value) {
    for field in ["width", "height", "measured_width", "measured_height", "lines", "overflow", "missing_glyphs", "requested_family", "fonts"] {
        assert_eq!(actual[field], expected[field], "{field}: {actual} != {expected}");
    }
}

#[test]
fn table_column_tracks_use_relative_and_absolute_widths_for_wrapping() {
    for dimensions in [json!({"unit":"relative","values":[1,5]}), json!({"unit":"absolute","values":[100,500]})] {
        let text = "Measured column widths must control wrapping.";
        let measured = measure_elements(json!([
            {"type":"table","id":"table","x":0,"y":0,"width":600,"height":50,"rows":[[text,text]],"font_size":20,"format":{"column_widths":dimensions}}
        ]));
        let cells = measured["measurements"].as_array().unwrap();
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0]["width"], 88.0, "{measured}");
        assert_eq!(cells[1]["width"], 488.0);
        assert_eq!(cells[0]["height"], 38.0);
        assert_eq!(cells[0]["overflow"], true);
        assert_eq!(cells[1]["overflow"], false, "{measured}");
        assert!(cells[0]["lines"].as_u64().unwrap() > cells[1]["lines"].as_u64().unwrap());
    }
}

#[test]
fn table_row_tracks_use_relative_and_absolute_heights_for_overflow() {
    for dimensions in [json!({"unit":"relative","values":[1,3]}), json!({"unit":"absolute","values":[40,120]})] {
        let text = "First line\nSecond line\nThird line";
        let measured = measure_elements(json!([
            {"type":"table","id":"table","x":0,"y":0,"width":600,"height":160,"rows":[[text],[text]],"font_size":20,"format":{"row_heights":dimensions}}
        ]));
        let cells = measured["measurements"].as_array().unwrap();
        assert_eq!(cells[0]["height"], 28.0, "{measured}");
        assert_eq!(cells[1]["height"], 108.0);
        assert_eq!(cells[0]["width"], 588.0);
        assert_eq!(cells[0]["overflow"], true);
        assert_eq!(cells[1]["overflow"], false);
        assert_eq!(cells[0]["lines"], 3);
        assert_eq!(cells[1]["lines"], 3);
    }
}

#[test]
fn table_merges_measure_only_anchors_with_summed_tracks_and_anchor_padding() {
    let measured = measure_elements(json!([
        {"type":"table","id":"table","x":0,"y":0,"width":600,"height":180,"font_size":20,
         "rows":[["","", ""],["First line\nSecond line\nThird line","", ""],["","", ""]],
         "format":{"column_widths":{"unit":"relative","values":[1,2,3]},"row_heights":{"unit":"absolute","values":[30,50,100]},
                   "merges":[{"row":0,"column":1,"row_span":1,"col_span":2},{"row":1,"column":0,"row_span":2,"col_span":2}],
                   "cells":[{"row":1,"column":0,"style":{"padding":{"left":10,"right":20,"top":4,"bottom":6}}},
                            {"row":2,"column":1,"style":{"text_style":{"font_size":120},"padding":{"left":4096,"right":4096,"top":4096,"bottom":4096}}}]}}
    ]));
    let cells = measured["measurements"].as_array().unwrap();
    let ids: Vec<_> = cells.iter().map(|cell| cell["element_id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["table[1,1]", "table[1,2]", "table[2,1]", "table[2,3]", "table[3,3]"]);
    assert_eq!(cells[1]["width"], 488.0);
    assert_eq!(cells[1]["height"], 18.0);
    assert_eq!(cells[2]["width"], 270.0);
    assert_eq!(cells[2]["height"], 140.0);
    assert_eq!(cells[2]["lines"], 3);
    assert!(cells.iter().all(|cell| cell["overflow"] == false), "{measured}");
}

#[test]
fn default_table_cells_keep_equal_tracks_padding_and_header_font_defaults() {
    let measured = measure_elements(json!([
        {"type":"table","id":"table","x":0,"y":0,"width":400,"height":100,"rows":[["Header", ""],["Body", ""]],"font_size":20},
        {"type":"text","id":"header","x":0,"y":0,"width":188,"height":38,"text":"Header","font_size":20,"color":"000000","bold":true},
        {"type":"text","id":"body","x":0,"y":0,"width":188,"height":38,"text":"Body","font_size":20,"color":"000000","bold":false}
    ]));
    let cells = measured["measurements"].as_array().unwrap();
    assert_eq!(cells.len(), 6);
    assert_same_text_measurement(&cells[0], &cells[4]);
    assert_same_text_measurement(&cells[2], &cells[5]);
    assert!(cells.iter().all(|cell| cell["overflow"] == false));
}

#[test]
fn table_cell_text_style_overrides_font_size_family_weight_and_italic() {
    let small_style = json!({"font_size":12,"font_family":"Consolas","bold":false,"italic":true});
    let large_style = json!({"font_size":48,"font_family":"Consolas","bold":true,"italic":false});
    let measured = measure_elements(json!([
        {"type":"table","id":"table","x":0,"y":0,"width":600,"height":80,"rows":[["Small"],["Large"]],"font_size":28,"format":{"cells":[
            {"row":0,"column":0,"style":{"text_style":small_style}},
            {"row":1,"column":0,"style":{"text_style":large_style}}
        ]}},
        {"type":"text","id":"small","x":0,"y":0,"width":588,"height":28,"text":"Small","font_size":28,"color":"000000","bold":true,"format":{"paragraphs":[{"runs":[{"text":"Small","style":small_style}]}]}},
        {"type":"text","id":"large","x":0,"y":0,"width":588,"height":28,"text":"Large","font_size":28,"color":"000000","bold":false,"format":{"paragraphs":[{"runs":[{"text":"Large","style":large_style}]}]}}
    ]));
    let cells = measured["measurements"].as_array().unwrap();
    assert_same_text_measurement(&cells[0], &cells[2]);
    assert_same_text_measurement(&cells[1], &cells[3]);
    assert_eq!(cells[0]["requested_family"], "Consolas");
    assert_eq!(cells[0]["overflow"], false);
    assert_eq!(cells[1]["overflow"], true);
}

#[test]
fn table_rich_cell_runs_override_cell_style_and_preserve_paragraph_metrics() {
    let mut elements = json!([
        {"type":"table","id":"table","x":0,"y":0,"width":600,"height":52,"rows":[["Small run\nSecond"]],"font_size":28,"format":{"cells":[
            {"row":0,"column":0,"style":{"text_style":{"font_size":48,"font_family":"Consolas","bold":true,"italic":true},"text_format":{"paragraphs":[
                {"runs":[{"text":"Small ","style":{"font_size":12}},{"text":"run","style":{"font_size":14,"bold":false,"italic":false,"font_family":"@minor"}}],"space_after":{"kind":"points","value":150}},
                {"runs":[{"text":"Second","style":{"font_size":12}}]}
            ]}}}
        ]}},
        {"type":"text","id":"rich","x":0,"y":0,"width":588,"height":40,"text":"Small run\nSecond","font_size":28,"color":"000000","bold":true,"format":{"paragraphs":[
            {"runs":[{"text":"Small ","style":{"font_size":12,"font_family":"Consolas","bold":true,"italic":true}},{"text":"run","style":{"font_size":14,"bold":false,"italic":false,"font_family":"@minor"}}],"space_after":{"kind":"points","value":150}},
            {"runs":[{"text":"Second","style":{"font_size":12,"font_family":"Consolas","bold":true,"italic":true}}]}
        ]}}
    ]);
    let measured = measure_elements(elements.clone());
    assert_same_text_measurement(&measured["measurements"][0], &measured["measurements"][1]);
    assert_eq!(measured["measurements"][0]["overflow"], false, "{measured}");
    assert_eq!(measured["measurements"][0]["lines"], 2);
    elements[0]["format"]["cells"][0]["style"]["text_format"]["paragraphs"][0]["runs"][1]["style"]["font_size"] = json!(48);
    let measured = measure_elements(elements);
    assert_eq!(measured["measurements"][0]["overflow"], true);
}

#[test]
fn table_padding_controls_the_text_frame_and_real_overflow() {
    for (padding, width, height, overflow) in [
        (json!({"left":0,"right":0,"top":0,"bottom":0}), 320.0, 60.0, false),
        (json!({"left":70,"right":30,"top":20,"bottom":10}), 220.0, 30.0, true),
    ] {
        let text = "Padding changes the text area";
        let measured = measure_elements(json!([
            {"type":"table","id":"table","x":0,"y":0,"width":320,"height":60,"rows":[[text]],"font_size":20,"format":{"cells":[{"row":0,"column":0,"style":{"padding":padding}}]}},
            {"type":"text","id":"inner","x":0,"y":0,"width":width,"height":height,"text":text,"font_size":20,"color":"000000","bold":true}
        ]));
        assert_same_text_measurement(&measured["measurements"][0], &measured["measurements"][1]);
        assert_eq!(measured["measurements"][0]["overflow"], overflow, "{measured}");
    }
}

#[test]
fn table_padding_without_positive_text_area_overflows_only_nonempty_cells() {
    for padding in [
        json!({"left":100,"right":0,"top":0,"bottom":0}),
        json!({"left":101,"right":0,"top":0,"bottom":0}),
        json!({"left":0,"right":0,"top":40,"bottom":0}),
        json!({"left":0,"right":0,"top":41,"bottom":0}),
    ] {
        for text in ["i", ""] {
            let measured = measure_elements(json!([
                {"type":"table","id":"table","x":0,"y":0,"width":100,"height":40,"rows":[[text]],"font_size":20,"format":{"cells":[
                    {"row":0,"column":0,"style":{"padding":padding,"text_style":{"font_size":1}}}
                ]}}
            ]));
            let cell = &measured["measurements"][0];
            assert_eq!(cell["width"], 100.0 - padding["left"].as_f64().unwrap());
            assert_eq!(cell["height"], 40.0 - padding["top"].as_f64().unwrap());
            assert_eq!(cell["overflow"], !text.is_empty(), "{measured}");
        }
    }
}

#[test]
fn shape_text_is_measured_inside_the_rendered_padding() {
    let measured = measure_elements(json!([
        {"type":"shape","id":"shape","x":0,"y":0,"width":200,"height":28,"text":"Short text","font_size":20,"color":"000000","bold":false,"preset":"rect","fill":"FFFFFF","stroke":"000000","stroke_width":1},
        {"type":"text","id":"inner","x":0,"y":0,"width":188,"height":20,"text":"Short text","font_size":20,"color":"000000","bold":false},
        {"type":"text","id":"outer","x":0,"y":0,"width":200,"height":28,"text":"Short text","font_size":20,"color":"000000","bold":false}
    ]));
    assert_same_text_measurement(&measured["measurements"][0], &measured["measurements"][1]);
    assert_eq!(measured["measurements"][0]["overflow"], true);
    assert_eq!(measured["measurements"][2]["overflow"], false);
}