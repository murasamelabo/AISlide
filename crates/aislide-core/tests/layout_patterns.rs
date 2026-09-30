use aislide_core::execute_request;
use serde_json::{json, Value};

fn resolve(request: Value) -> Value {
    let mut request = request;
    request["op"] = json!("resolve_layout_pattern");
    execute_request(request).unwrap()
}

fn frames(resolved: &Value) -> Vec<(String, [f64; 4])> {
    resolved["slots"].as_array().unwrap().iter().map(|slot| {
        let frame = &slot["frame"];
        (slot["id"].as_str().unwrap().to_owned(), ["x", "y", "width", "height"].map(|key| frame[key].as_f64().unwrap()))
    }).collect()
}

fn frame_of(resolved: &Value, id: &str) -> [f64; 4] {
    frames(resolved).into_iter().find(|(slot, _)| slot == id).unwrap_or_else(|| panic!("missing slot {id}")).1
}

fn assert_geometry(resolved: &Value) {
    let body = &resolved["body"];
    let [bx, by, bw, bh] = ["x", "y", "width", "height"].map(|key| body[key].as_f64().unwrap());
    let slots = frames(resolved);
    let id = resolved["pattern"]["id"].as_str().unwrap();
    assert!(!slots.is_empty(), "{id} has no slots");
    let mut ids = std::collections::BTreeSet::new();
    for (slot, [x, y, width, height]) in &slots {
        assert!(ids.insert(slot.clone()), "{id} repeats slot {slot}");
        assert!([x, y, width, height].iter().all(|value| value.fract() == 0.0), "{id}/{slot} is not on integer pixels");
        assert!(*width > 0.0 && *height > 0.0, "{id}/{slot} is empty");
        assert!(*x >= bx && *y >= by && x + width <= bx + bw && y + height <= by + bh, "{id}/{slot} leaves the body");
    }
    for (index, (left, [lx, ly, lw, lh])) in slots.iter().enumerate() {
        for (right, [rx, ry, rw, rh]) in &slots[index + 1..] {
            let overlap = (lx + lw).min(rx + rw) - lx.max(*rx) > 0.0 && (ly + lh).min(ry + rh) - ly.max(*ry) > 0.0;
            assert!(!overlap, "{id}: {left} overlaps {right}");
        }
    }
}

#[test]
fn catalog_lists_unique_documented_patterns_with_slots() {
    let catalog = execute_request(json!({"op":"layout_patterns"})).unwrap();
    let patterns = catalog["patterns"].as_array().unwrap();
    assert_eq!(patterns.len(), 54);
    let ids: std::collections::BTreeSet<_> = patterns.iter().map(|pattern| pattern["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), patterns.len());
    for pattern in patterns {
        let id = pattern["id"].as_str().unwrap();
        assert_eq!(pattern["family"].as_str().unwrap(), id.split('/').next().unwrap());
        assert!(!pattern["use_when"].as_str().unwrap().is_empty() && !pattern["avoid_when"].as_str().unwrap().is_empty(), "{id}");
        assert!(!pattern["relationships"].as_array().unwrap().is_empty(), "{id}");
        assert!(!pattern["slots"].as_array().unwrap().is_empty(), "{id}");
        if let Some(fallback) = pattern["fallback"].as_str() { assert!(ids.contains(fallback), "{id} falls back to unknown {fallback}"); }
    }
    assert_eq!(catalog["guidance"].as_array().unwrap().len(), 8);
    assert_eq!(catalog["tokens"]["gaps"]["contrast"]["wide"], json!(48));
}

#[test]
fn every_pattern_fits_the_default_wide_body_and_non_fitting_standard_patterns_fall_back() {
    let catalog = execute_request(json!({"op":"layout_patterns"})).unwrap();
    for pattern in catalog["patterns"].as_array().unwrap() {
        let id = pattern["id"].as_str().unwrap();
        let wide = resolve(json!({"pattern_id":id}));
        assert_geometry(&wide);
        assert_eq!(wide["fits"], json!(true), "{id}: {}", wide["issues"]);
        let standard = resolve(json!({"pattern_id":id,"canvas":{"width":960,"height":720}}));
        assert_geometry(&standard);
        if standard["fits"] == json!(false) {
            let fallback = standard["fallback"].as_str().unwrap_or_else(|| panic!("{id} does not fit 4:3 and has no fallback"));
            let replacement = resolve(json!({"pattern_id":fallback,"canvas":{"width":960,"height":720}}));
            assert_eq!(replacement["fits"], json!(true), "{id} falls back to non-fitting {fallback}");
        }
        if let Some([minimum, maximum]) = pattern["count"].as_object().map(|count| [count["min"].as_u64().unwrap(), count["max"].as_u64().unwrap()]) {
            for count in minimum..=maximum { assert_geometry(&resolve(json!({"pattern_id":id,"options":{"count":count}}))); }
        }
        if pattern["message_band"] == json!(true) {
            let banded = resolve(json!({"pattern_id":id,"options":{"message_band":true,"mirror":true}}));
            assert_geometry(&banded);
            assert_eq!(frame_of(&banded, "message"), [48.0, 588.0, 1184.0, 72.0], "{id}");
        }
    }
}

#[test]
fn ratios_and_gutters_follow_the_canvas_tokens() {
    let halves = resolve(json!({"pattern_id":"split/1-1"}));
    assert_eq!(frame_of(&halves, "left"), [48.0, 120.0, 568.0, 540.0]);
    assert_eq!(frame_of(&halves, "right"), [664.0, 120.0, 568.0, 540.0]);
    let primary = resolve(json!({"pattern_id":"split/2-1"}));
    assert_eq!(frame_of(&primary, "primary"), [48.0, 120.0, 768.0, 540.0]);
    assert_eq!(frame_of(&primary, "support"), [848.0, 120.0, 384.0, 540.0]);
    assert!(primary["slots"][1]["capacity"]["cjk_chars_per_line"].as_u64().unwrap() >= 20);
    let grid = resolve(json!({"pattern_id":"grid/2x2"}));
    assert_eq!(frame_of(&grid, "cell-2-2"), [652.0, 402.0, 580.0, 258.0]);
    let standard = resolve(json!({"pattern_id":"split/1-1","canvas":{"width":960,"height":720}}));
    assert_eq!(frame_of(&standard, "left"), [40.0, 120.0, 420.0, 540.0]);
    assert_eq!(frame_of(&standard, "right"), [500.0, 120.0, 420.0, 540.0]);
    let columns = resolve(json!({"pattern_id":"columns/4","canvas":{"width":960,"height":720}}));
    assert_eq!(columns["fits"], json!(false));
    assert_eq!(columns["fallback"], json!("grid/2x2"));
    let three = resolve(json!({"pattern_id":"columns/3"}));
    let widths: Vec<f64> = frames(&three).iter().map(|(_, frame)| frame[2]).collect();
    assert!(widths.iter().all(|width| (373.0..=374.0).contains(width)));
    let edges: Vec<[f64; 2]> = frames(&three).iter().map(|(_, frame)| [frame[0], frame[0] + frame[2]]).collect();
    assert!(edges.windows(2).all(|pair| pair[1][0] - pair[0][1] == 32.0));
    assert_eq!(edges.last().unwrap()[1], 1232.0);
}

#[test]
fn options_mirror_band_reference_body_and_count() {
    let mirrored = resolve(json!({"pattern_id":"split/2-1","options":{"mirror":true}}));
    assert_eq!(frame_of(&mirrored, "primary"), [464.0, 120.0, 768.0, 540.0]);
    assert_eq!(frame_of(&mirrored, "support"), [48.0, 120.0, 384.0, 540.0]);
    let banded = resolve(json!({"pattern_id":"focus/full","options":{"message_band":true}}));
    assert_eq!(frame_of(&banded, "primary"), [48.0, 120.0, 1184.0, 444.0]);
    let referenced = resolve(json!({"pattern_id":"focus/full","options":{"reference_band":true}}));
    assert_eq!(referenced["body"]["height"], json!(488.0));
    let explicit = resolve(json!({"pattern_id":"focus/full","body":{"x":100.4,"y":150,"width":600,"height":400.2}}));
    assert_eq!(frame_of(&explicit, "primary"), [100.0, 150.0, 600.0, 400.0]);
    let cover = resolve(json!({"pattern_id":"structure/cover"}));
    assert_eq!(cover["body"]["y"], json!(48.0));
    let steps = resolve(json!({"pattern_id":"sequence/steps-h","options":{"count":3}}));
    assert_eq!(frames(&steps).iter().filter(|(id, _)| id.starts_with("step-")).count(), 3);
    assert_eq!(frames(&steps).iter().filter(|(id, _)| id.starts_with("connector-")).count(), 2);
    let crowded = resolve(json!({"pattern_id":"sequence/steps-h","options":{"count":5}}));
    assert_eq!(crowded["fits"], json!(false));
    assert_eq!(crowded["fallback"], json!("sequence/steps-v"));
    assert!(crowded["issues"][0].as_str().unwrap().contains("card slot"));
}

#[test]
fn contain_fit_keeps_round_parts_round_and_preflight_flags_stretching() {
    let resolved = resolve(json!({"pattern_id":"focus/centered-diagram"}));
    let frame = resolved["slots"][0]["frame"].clone();
    let spec = |fit: &str| json!({"version":1,"preset":"cycle/balanced","title":"","data":{"kind":"items","center":"Loop","items":[{"label":"Plan"},{"label":"Run"},{"label":"Check"},{"label":"Improve"}]},
        "layout":{"x":frame["x"],"y":frame["y"],"width":frame["width"],"height":frame["height"],"show_title":false,"fit":fit}});
    let ellipse_ratio = |element: &Value| element["children"].as_array().unwrap().iter()
        .find(|child| child["preset"] == "ellipse").map(|child| child["width"].as_f64().unwrap() / child["height"].as_f64().unwrap()).unwrap();
    let original = execute_request(json!({"op":"create_part","id":"loop","spec":{"version":1,"preset":"cycle/balanced","title":"","data":spec("stretch")["data"]}})).unwrap();
    let contained = execute_request(json!({"op":"create_part","id":"loop","spec":spec("contain")})).unwrap();
    let stretched = execute_request(json!({"op":"create_part","id":"loop","spec":spec("stretch")})).unwrap();
    assert!((ellipse_ratio(&contained) - ellipse_ratio(&original)).abs() < 0.01);
    assert!((ellipse_ratio(&stretched) - ellipse_ratio(&original)).abs() > 0.2);
    assert_eq!([contained["x"].clone(), contained["width"].clone()], [frame["x"].clone(), frame["width"].clone()]);
    let children = contained["children"].as_array().unwrap();
    let top = children.iter().map(|child| child["y"].as_f64().unwrap()).fold(f64::INFINITY, f64::min);
    let bottom = children.iter().map(|child| child["y"].as_f64().unwrap() + child["height"].as_f64().unwrap()).fold(0.0, f64::max);
    let (width, height) = (frame["width"].as_f64().unwrap(), frame["height"].as_f64().unwrap());
    let offset = (height - 424.0 * (width / 1152.0).min(height / 424.0)) / 2.0;
    assert!(offset > 50.0 && top >= offset - 0.5 && bottom <= height - offset + 0.5, "contain centers the uniformly scaled canvas: {top} {bottom} {offset}");
    assert!(execute_request(json!({"op":"create_part","id":"loop","spec":{"version":1,"preset":"cycle/balanced","title":"","data":spec("stretch")["data"],"layout":{"x":0,"y":0,"width":400,"height":300,"fit":"cover"}}})).is_err());

    let findings = |fit: &str| {
        let deck = json!({"version":1,"title":"Aspect","width":1280,"height":720,"slides":[{"id":"slide","title":"Aspect","background":"FFFFFF","notes":"","elements":[]}]});
        let document = execute_request(json!({"op":"new_document","id":"aspect","deck":deck})).unwrap();
        let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"loop","spec":spec(fit)})).unwrap();
        let report = execute_request(json!({"op":"preflight_presentation","document":inserted["document"],"options":{"page_indices":[0]}})).unwrap();
        report["findings"].as_array().unwrap().iter().filter(|finding| finding["code"] == "PART_ASPECT_DISTORTED").cloned().collect::<Vec<_>>()
    };
    let stretched = findings("stretch");
    assert_eq!(stretched.len(), 1);
    assert_eq!(stretched[0]["element_ids"][0], "loop");
    assert!(findings("contain").is_empty());
}

#[test]
fn invalid_requests_are_rejected() {
    for (request, message) in [
        (json!({"pattern_id":"missing/pattern"}), "unknown layout pattern"),
        (json!({"pattern_id":"focus/full","canvas":{"width":100,"height":720}}), "320..4096"),
        (json!({"pattern_id":"focus/full","body":{"x":1200,"y":120,"width":400,"height":400}}), "inside the canvas"),
        (json!({"pattern_id":"columns/3","options":{"count":3}}), "fixed item count"),
        (json!({"pattern_id":"sequence/steps-h","options":{"count":9}}), "count must be 2..5"),
        (json!({"pattern_id":"focus/full-band","options":{"message_band":true}}), "message_band is not available"),
        (json!({"pattern_id":"structure/cover","options":{"reference_band":true}}), "reference_band"),
        (json!({"pattern_id":"focus/full","options":{"body_size":8}}), "body_size"),
        (json!({"pattern_id":"columns/4","body":{"x":0,"y":0,"width":60,"height":400}}), "does not fit"),
        (json!({"pattern_id":"focus/full","options":{"unknown":true}}), "unknown field"),
    ] {
        let mut request = request;
        request["op"] = json!("resolve_layout_pattern");
        let error = execute_request(request).unwrap_err().to_string();
        assert!(error.contains(message), "{error} should contain {message}");
    }
}
