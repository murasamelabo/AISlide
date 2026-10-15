use aislide_core::execute_request;
use serde_json::{json, Value};

const FRAMES: [[f64; 4]; 3] = [[48.0, 120.0, 947.0, 540.0], [48.0, 120.0, 1184.0, 300.0], [48.0, 120.0, 1184.0, 540.0]];

fn children(element: &Value) -> &Vec<Value> { element["children"].as_array().unwrap() }
fn size(element: &Value) -> (f64, f64) { (element["width"].as_f64().unwrap(), element["height"].as_f64().unwrap()) }
fn ratio(element: &Value) -> f64 { let (width, height) = size(element); width / height }

fn marker(element: &Value) -> bool {
    let (width, height) = size(element);
    ["picture", "rect", "shape"].contains(&element["type"].as_str().unwrap())
        && width >= 8.0 && (width - height).abs() <= width.max(height) * 0.05 && width * height <= 1152.0 * 512.0 * 0.03
}

fn with_layout(spec: &Value, frame: [f64; 4], fit: &str) -> Value {
    let mut spec = spec.clone();
    spec["layout"] = json!({"x": frame[0], "y": frame[1], "width": frame[2], "height": frame[3], "show_title": true, "fit": fit});
    spec
}

#[test]
fn every_catalog_part_profiles_its_geometry_keeps_markers_square_and_contains_uniformly() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    for preset in catalog["presets"].as_array().unwrap() {
        let id = preset["id"].as_str().unwrap();
        let example = &preset["example"];
        let profile = execute_request(json!({"op":"part_aspect","spec":example})).unwrap_or_else(|error| panic!("{id}: {error}"));
        assert!(["free", "tolerant", "strict"].contains(&profile["sensitivity"].as_str().unwrap()), "{id}");
        assert_eq!(profile["tolerance"].is_null(), profile["sensitivity"] == "free", "{id}");
        if profile["layout"] == "native" { continue; }
        let natural = execute_request(json!({"op":"create_part","id":"probe","spec":example})).unwrap();
        let mut rendered = 0;
        for frame in FRAMES {
            let Ok(stretched) = execute_request(json!({"op":"create_part","id":"probe","spec":with_layout(example, frame, "stretch")})) else { continue };
            let Ok(contained) = execute_request(json!({"op":"create_part","id":"probe","spec":with_layout(example, frame, "contain")})) else { continue };
            rendered += 1;
            for ((original, stretched), contained) in children(&natural).iter().zip(children(&stretched)).zip(children(&contained)) {
                if marker(original) { assert!((ratio(stretched) / ratio(original) - 1.0).abs() < 0.02, "{id}: stretched marker {} lost its aspect", original["id"]); }
                let (width, height) = size(original);
                if original["type"] != "text" && width >= 2.0 && height >= 2.0 {
                    assert!((ratio(contained) / ratio(original) - 1.0).abs() < 0.01, "{id}: contained {} changed aspect", original["id"]);
                }
            }
        }
        assert!(rendered > 0, "{id} rendered in none of the representative frames");
    }
}

#[test]
fn preflight_flags_stretching_beyond_each_parts_measured_tolerance() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    let mut flagged = 0;
    for preset in catalog["presets"].as_array().unwrap() {
        let id = preset["id"].as_str().unwrap();
        let profile = execute_request(json!({"op":"part_aspect","spec":preset["example"]})).unwrap();
        for fit in ["stretch", "contain"] {
            let spec = with_layout(&preset["example"], FRAMES[0], fit);
            let deck = json!({"version":1,"title":"Aspect","width":1280,"height":720,"slides":[{"id":"slide","title":"Aspect","background":"FFFFFF","notes":"","elements":[]}]});
            let document = execute_request(json!({"op":"new_document","id":"aspect","deck":deck})).unwrap();
            let Ok(inserted) = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"probe","spec":spec})) else { continue };
            let report = execute_request(json!({"op":"preflight_presentation","document":inserted["document"],"options":{"page_indices":[0]}})).unwrap();
            let warned = report["findings"].as_array().unwrap().iter().any(|finding| finding["code"] == "PART_ASPECT_DISTORTED");
            let distortion = (947.0 / 1152.0_f64).max(540.0 / 512.0) / (947.0 / 1152.0_f64).min(540.0 / 512.0);
            let expected = fit == "stretch" && profile["tolerance"].as_f64().is_some_and(|tolerance| distortion > tolerance);
            assert_eq!(warned, expected, "{id} {fit}: {}", profile);
            flagged += usize::from(warned);
        }
    }
    assert!(flagged >= 10, "only {flagged} stretched parts were flagged");
}

#[test]
fn ranking_prefers_slots_near_the_part_canvas_for_shape_sensitive_parts() {
    let ranked = execute_request(json!({"op":"rank_layout_patterns","part":"cycle/balanced"})).unwrap();
    assert_eq!(ranked["part"]["sensitivity"], "strict");
    let patterns = ranked["patterns"].as_array().unwrap();
    assert_eq!(patterns[0]["fit"], "stretch", "{}", patterns[0]);
    let position = |id: &str| patterns.iter().position(|entry| entry["pattern_id"] == id).unwrap();
    assert!(position("focus/full-band") < position("sequence/cycle"));
    let cycle = patterns.iter().find(|entry| entry["pattern_id"] == "sequence/cycle").unwrap();
    assert_eq!(cycle["fit"], "contain");
    let resolved = execute_request(json!({"op":"resolve_layout_pattern","pattern_id":"sequence/cycle","options":{"part":"cycle/balanced"}})).unwrap();
    assert_eq!(resolved["slots"][0]["part_fit"]["fit"], "contain");
    assert!(resolved["slots"][1]["part_fit"].is_null());
    let free = execute_request(json!({"op":"rank_layout_patterns","part":"list/rows"})).unwrap();
    assert_eq!(free["part"]["sensitivity"], "free");
    assert_eq!(free["patterns"][0]["pattern_id"], "focus/full");
    assert!(execute_request(json!({"op":"rank_layout_patterns","part":"missing/preset"})).is_err());
}

/// A graph that shows its title keeps its nodes below the 88px title band; one without a title starts at the top.
fn titleless_graph(show_title: bool) -> Value {
    let top = if show_title { 88.0 } else { 0.0 };
    json!({"version":1,"title":"Aspect","show_title":show_title,
        "nodes":[{"id":"a","label":"A","kind":"rounded_rectangle","x":0,"y":top,"width":240,"height":120},
                 {"id":"b","label":"B","kind":"rounded_rectangle","x":900,"y":380,"width":240,"height":120}],
        "edges":[{"id":"e1","source":"a","target":"b"}]})
}

fn graph_part_findings(graph_title: bool, layout_title: bool, height: f64) -> (Value, Vec<Value>) {
    let spec = json!({"version":1,"preset":"diagram/custom","title":"Aspect","data":{"kind":"diagram","graph":titleless_graph(graph_title)},
        "layout":{"x":64,"y":144,"width":1152,"height":height,"show_title":layout_title}});
    let document = execute_request(json!({"op":"create_presentation","id":"aspect-graph","title":"Aspect"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide-1","id":"g1","spec":spec.clone()})).unwrap();
    let report = execute_request(json!({"op":"preflight_presentation","document":inserted["document"],"options":{"page_indices":[0]}})).unwrap();
    let findings = report["findings"].as_array().unwrap().iter().filter(|finding| finding["code"] == "PART_ASPECT_DISTORTED").cloned().collect();
    (execute_request(json!({"op":"part_aspect","spec":spec})).unwrap(), findings)
}

#[test]
fn issue15_graph_without_its_own_title_maps_the_full_canvas_height() {
    // Neither the graph nor the layout shows a title: the renderer keeps all 512px of content height.
    let (profile, equal_scale) = graph_part_findings(false, false, 512.0);
    assert_eq!(profile["canvas"][1], 512.0, "{profile}");
    assert!(equal_scale.is_empty(), "a 1152x512 frame is drawn at equal scale and must not warn: {equal_scale:?}");
    let (profile, squashed) = graph_part_findings(false, false, 424.0);
    assert_eq!(profile["canvas"][1], 512.0, "{profile}");
    assert_eq!(squashed.len(), 1, "a 1152x424 frame squashes the figure to 0.83x and must warn: {squashed:?}");
}

#[test]
fn issue15_graph_with_a_title_still_loses_the_removed_title_band() {
    // The graph owns a title that the layout removes: the renderer shifts the figure up by 88px and maps 424px.
    let (profile, equal_scale) = graph_part_findings(true, false, 424.0);
    assert_eq!(profile["canvas"][1], 424.0, "{profile}");
    assert!(equal_scale.is_empty(), "{equal_scale:?}");
    let (_, stretched) = graph_part_findings(true, false, 512.0);
    assert_eq!(stretched.len(), 1, "{stretched:?}");
    // A layout that keeps its title always maps 512px, whatever the graph says.
    let (profile, kept) = graph_part_findings(true, true, 512.0);
    assert_eq!(profile["canvas"][1], 512.0, "{profile}");
    assert!(kept.is_empty(), "{kept:?}");
}

#[test]
fn issue15_rendered_scale_agrees_with_the_preflight_profile() {
    // The warning is only meaningful if the rendered node height follows the same canvas height as the profile.
    for (graph_title, layout_title, height, canvas) in [(false, false, 424.0, 512.0), (true, false, 424.0, 424.0), (false, true, 424.0, 512.0)] {
        let spec = json!({"version":1,"preset":"diagram/custom","title":"Aspect","data":{"kind":"diagram","graph":titleless_graph(graph_title)},
            "layout":{"x":64,"y":144,"width":1152,"height":height,"show_title":layout_title}});
        let profile = execute_request(json!({"op":"part_aspect","spec":spec.clone()})).unwrap();
        assert_eq!(profile["canvas"][1], canvas, "graph title {graph_title}, layout title {layout_title}");
        let part = execute_request(json!({"op":"create_part","id":"probe","spec":spec})).unwrap();
        let node = part["children"].as_array().unwrap().iter().find(|child| child["id"].as_str().is_some_and(|id| id.ends_with("-n-a"))).unwrap();
        let expected = 120.0 * height / canvas;
        assert!((node["height"].as_f64().unwrap() - expected).abs() < 0.5, "node height {} should be {expected}", node["height"]);
    }
}
