use aislide_core::{design::Design, model::Deck, review::{check_accessibility, AccessibilityReport}};
use serde_json::{json, Value};

fn text(id: &str, bounds: [f64; 4], color: &str) -> Value {
    json!({"type":"text","id":id,"x":bounds[0],"y":bounds[1],"width":bounds[2],"height":bounds[3],"text":"Synthetic","font_size":18,"color":color,"bold":false})
}

fn rect(id: &str, bounds: [f64; 4], fill: &str) -> Value {
    json!({"type":"rect","id":id,"x":bounds[0],"y":bounds[1],"width":bounds[2],"height":bounds[3],"fill":fill})
}

fn styled_rect(id: &str, bounds: [f64; 4], fill: &str, visual: Value) -> Value {
    let mut element = rect(id, bounds, fill);
    element["visual"] = visual;
    element
}

fn shape(id: &str, preset: &str, bounds: [f64; 4], fill: &str) -> Value {
    json!({"type":"shape","id":id,"x":bounds[0],"y":bounds[1],"width":bounds[2],"height":bounds[3],"preset":preset,"fill":fill,"stroke":"none","stroke_width":0,
        "text":"","font_size":18,"color":"000000","bold":false})
}

fn group(id: &str, bounds: [f64; 4], view: [f64; 2], visual: Option<Value>, children: Vec<Value>) -> Value {
    let mut element = json!({"type":"group","id":id,"x":bounds[0],"y":bounds[1],"width":bounds[2],"height":bounds[3],"view_width":view[0],"view_height":view[1],"children":children});
    if let Some(visual) = visual { element["visual"] = visual; }
    element
}

fn picture(id: &str, bounds: [f64; 4]) -> Value {
    json!({"type":"picture","id":id,"x":bounds[0],"y":bounds[1],"width":bounds[2],"height":bounds[3],"base64":"AA==","mime_type":"image/png","alt":"Synthetic picture"})
}

fn deck(slide: Value, design: Option<Value>) -> Deck {
    let mut value = json!({"version":1,"title":"Synthetic","width":1280,"height":720,"slides":[slide]});
    if let Some(design) = design { value["design"] = design; }
    serde_json::from_value(value).unwrap()
}

fn slide(background: &str, elements: Vec<Value>) -> Value {
    json!({"id":"s1","title":"Synthetic","background":background,"notes":"","elements":elements})
}

fn report(background: &str, elements: Vec<Value>) -> AccessibilityReport { check_accessibility(&deck(slide(background, elements), None)) }

/// A slide that inherits its background and graphics from the "blank" layout of the default design.
fn inheriting(master: (&str, Vec<Value>), layout: (Option<&str>, Vec<Value>), hide_master_graphics: bool, elements: Vec<Value>) -> AccessibilityReport {
    let mut design = serde_json::to_value(Design::default()).unwrap();
    design["masters"][0]["background"] = json!(master.0);
    design["masters"][0]["elements"] = json!(master.1);
    design["layouts"][0]["background"] = layout.0.map_or(Value::Null, |background| json!(background));
    design["layouts"][0]["elements"] = json!(layout.1);
    // The stored slide background is white on purpose: only the inherited one may decide.
    let mut slide = slide("FFFFFF", elements);
    slide["layout_id"] = json!("blank");
    slide["inherit_background"] = json!(true);
    slide["hide_master_graphics"] = json!(hide_master_graphics);
    check_accessibility(&deck(slide, Some(design)))
}

/// The contrast findings of one element: its issue code, with `:reason` when the check is unsupported. Empty when the text passes.
fn outcome(report: &AccessibilityReport, id: &str) -> Vec<String> {
    report.issues.iter()
        .filter(|issue| issue.element_id.as_deref() == Some(id) && ["text_contrast_low", "contrast_background_unverified", "contrast_color_unverified"].contains(&issue.code.as_str()))
        .map(|issue| issue.reason.as_ref().map_or_else(|| issue.code.clone(), |reason| format!("{}:{reason}", issue.code))).collect()
}

const DARK: &str = "0B1F3A";
const TEXT_AT: [f64; 4] = [60.0, 60.0, 200.0, 40.0];

#[test]
fn issue76_inherited_background_is_read_from_the_layout_and_then_the_master() {
    let elements = || vec![text("dark", TEXT_AT, "202525"), text("light", [60.0, 160.0, 200.0, 40.0], "FFFFFF")];
    let on_master = inheriting((DARK, vec![]), (None, vec![]), false, elements());
    assert_eq!(outcome(&on_master, "dark"), ["text_contrast_low"], "dark text on the master's dark background");
    assert!(outcome(&on_master, "light").is_empty(), "{:?}", on_master.issues);
    assert_eq!((on_master.contrast_checked, on_master.contrast_unsupported), (2, 0));
    let on_layout = inheriting((DARK, vec![]), (Some("FFFFFF"), vec![]), false, elements());
    assert!(outcome(&on_layout, "dark").is_empty(), "the layout's own background wins over the master's");
    assert_eq!(outcome(&on_layout, "light"), ["text_contrast_low"]);
}

#[test]
fn issue76_inherited_graphics_are_painted_below_slide_text_unless_hidden() {
    let cover = || rect("cover", [0.0, 0.0, 1280.0, 720.0], DARK);
    let elements = || vec![text("dark", TEXT_AT, "202525"), text("light", [60.0, 160.0, 200.0, 40.0], "FFFFFF")];
    let covered = inheriting(("FFFFFF", vec![cover()]), (None, vec![]), false, elements());
    assert_eq!(outcome(&covered, "dark"), ["text_contrast_low"], "a master rectangle is the background of the text above it");
    assert!(outcome(&covered, "light").is_empty());
    let hidden = inheriting(("FFFFFF", vec![cover()]), (None, vec![]), true, elements());
    assert!(outcome(&hidden, "dark").is_empty(), "hide_master_graphics removes the master rectangle, as the renderer does");
    assert_eq!(outcome(&hidden, "light"), ["text_contrast_low"]);
    let layout_picture = inheriting(("FFFFFF", vec![]), (None, vec![picture("photo", [0.0, 0.0, 1280.0, 720.0])]), true, elements());
    assert_eq!(outcome(&layout_picture, "dark"), ["contrast_background_unverified:unsupported_backdrop"], "a layout picture is never replaced by white");
}

#[test]
fn issue76_group_text_is_judged_at_its_page_position_and_size() {
    let backdrop = rect("panel", [0.0, 0.0, 200.0, 100.0], DARK);
    let inside = report("FFFFFF", vec![group("g", [100.0, 100.0, 400.0, 200.0], [200.0, 100.0], None, vec![backdrop, text("in-dark", [10.0, 10.0, 100.0, 30.0], "202525"), text("in-light", [10.0, 50.0, 100.0, 30.0], "FFFFFF")])]);
    assert_eq!(outcome(&inside, "in-dark"), ["text_contrast_low"], "the panel is the background of the text in the same group");
    assert!(outcome(&inside, "in-light").is_empty());
    assert_eq!((inside.contrast_checked, inside.contrast_unsupported), (2, 0));

    // 777777 on white is 4.48:1: low at 14px, but 28px once the group doubles it, which only needs 3:1.
    let mut small = text("small", TEXT_AT, "777777");
    small["font_size"] = json!(14);
    let loose = report("FFFFFF", vec![small.clone()]);
    assert_eq!(outcome(&loose, "small"), ["text_contrast_low"]);
    assert_eq!(loose.issues.iter().find(|issue| issue.code == "text_contrast_low").unwrap().required_ratio, Some(4.5));
    let doubled = report("FFFFFF", vec![group("g", [0.0, 0.0, 800.0, 400.0], [400.0, 200.0], None, vec![small])]);
    assert!(outcome(&doubled, "small").is_empty(), "{:?}", doubled.issues);
}

#[test]
fn issue76_only_the_nearest_covering_shape_below_the_text_decides() {
    let report = report("FFFFFF", vec![
        rect("outer", [0.0, 0.0, 600.0, 400.0], DARK),
        rect("inner", [50.0, 50.0, 300.0, 100.0], "FFFFFF"),
        text("on-inner", [60.0, 60.0, 100.0, 30.0], "FFFFFF"),
        text("on-outer", [60.0, 250.0, 100.0, 30.0], "FFFFFF"),
        rect("above", [40.0, 240.0, 300.0, 100.0], "FFFFFF"),
    ]);
    assert_eq!(outcome(&report, "on-inner"), ["text_contrast_low"], "the white rectangle below the text is its background, not the dark one beneath that");
    assert!(outcome(&report, "on-outer").is_empty(), "a rectangle painted after the text is not its background");
}

#[test]
fn issue76_a_shape_that_does_not_cover_the_whole_text_is_partial_not_a_guess() {
    let report = report("FFFFFF", vec![
        rect("half", [0.0, 0.0, 100.0, 100.0], DARK),
        text("crossing", [60.0, 20.0, 100.0, 30.0], "000000"),
        rect("touching", [300.0, 0.0, 100.0, 100.0], DARK),
        text("inside", [300.0, 20.0, 100.0, 30.0], "FFFFFF"),
    ]);
    assert_eq!(outcome(&report, "crossing"), ["contrast_background_unverified:partial_cover"]);
    assert!(outcome(&report, "inside").is_empty(), "a frame that fits exactly is covered");
    assert_eq!((report.contrast_checked, report.contrast_unsupported), (1, 1));
}

#[test]
fn issue76_unsupported_backgrounds_name_their_reason_and_are_never_treated_as_white() {
    let gradient = json!({"gradient":{"kind":"linear","angle":90,"stops":[{"offset":0,"color":"FFFFFF","opacity":1},{"offset":1,"color":"000000","opacity":1}]}});
    let mut highlighted = text("highlight", [800.0, 200.0, 200.0, 40.0], "000000");
    highlighted["format"] = json!({"paragraphs":[{"runs":[{"text":"Synthetic","style":{"highlight":"FFFF00"}}]}]});
    let report = report(DARK, vec![
        picture("photo", [0.0, 0.0, 300.0, 100.0]), text("on-picture", [10.0, 10.0, 100.0, 30.0], "000000"),
        styled_rect("fade", [400.0, 0.0, 300.0, 100.0], "FFFFFF", gradient), text("on-gradient", [410.0, 10.0, 100.0, 30.0], "000000"),
        styled_rect("glass", [800.0, 0.0, 300.0, 100.0], "FFFFFF", json!({"opacity":0.5})), text("on-glass", [810.0, 10.0, 100.0, 30.0], "000000"),
        group("turned", [400.0, 200.0, 300.0, 100.0], [300.0, 100.0], Some(json!({"rotation":10})), vec![text("in-turned", [10.0, 10.0, 100.0, 30.0], "000000")]),
        highlighted,
        shape("arrow", "rightArrow", [0.0, 400.0, 300.0, 100.0], "FFFFFF"), text("on-arrow", [10.0, 410.0, 100.0, 30.0], "000000"),
        group("blur", [400.0, 400.0, 300.0, 100.0], [300.0, 100.0], Some(json!({"soft_edge":4})), vec![rect("blurred", [0.0, 0.0, 300.0, 100.0], "FFFFFF"), text("in-blur", [10.0, 10.0, 100.0, 30.0], "000000")]),
    ]);
    for (id, reason) in [("on-picture", "unsupported_backdrop"), ("on-gradient", "background_not_solid"), ("on-glass", "background_not_solid"), ("in-turned", "transformed_text"),
        ("highlight", "text_highlight_or_baseline"), ("on-arrow", "unsupported_backdrop"), ("in-blur", "background_not_solid")] {
        assert_eq!(outcome(&report, id), [format!("contrast_background_unverified:{reason}")], "{id}");
    }
    assert!(report.issues.iter().filter(|issue| issue.code == "contrast_background_unverified").all(|issue| issue.status == "unsupported" && issue.reason.is_some()));
    assert_eq!((report.contrast_checked, report.contrast_unsupported), (0, 7), "dark text on the slide's dark background would have failed had it been used");
}

#[test]
fn issue76_rounded_and_elliptical_shapes_and_convex_polygons_cover_only_the_text_inside_them() {
    let triangle = json!({"type":"polygon","id":"triangle","x":0,"y":600,"width":400,"height":400,"points":[[0.5,0.0],[1.0,1.0],[0.0,1.0]],"fill":DARK,"stroke":"none","stroke_width":0});
    let report = report("FFFFFF", vec![
        shape("card", "roundRect", [0.0, 0.0, 400.0, 200.0], DARK),
        text("in-card", [20.0, 20.0, 360.0, 160.0], "FFFFFF"),
        text("at-card-corner", [0.0, 0.0, 100.0, 40.0], "FFFFFF"),
        shape("pill", "ellipse", [500.0, 0.0, 400.0, 200.0], DARK),
        text("in-pill", [600.0, 50.0, 200.0, 100.0], "FFFFFF"),
        text("at-pill-edge", [520.0, 20.0, 360.0, 160.0], "FFFFFF"),
        triangle,
        text("in-triangle", [150.0, 850.0, 100.0, 40.0], "FFFFFF"),
        text("at-apex-side", [0.0, 600.0, 100.0, 40.0], "FFFFFF"),
    ]);
    assert!(outcome(&report, "in-card").is_empty() && outcome(&report, "in-pill").is_empty() && outcome(&report, "in-triangle").is_empty(), "{:?}", report.issues);
    for id in ["at-card-corner", "at-pill-edge", "at-apex-side"] {
        assert_eq!(outcome(&report, id), ["contrast_background_unverified:partial_cover"], "{id}");
    }
}

#[test]
fn issue76_hidden_shapes_paint_nothing_but_hidden_text_is_still_checked() {
    let mut hidden_text = text("hidden-text", [60.0, 200.0, 200.0, 40.0], "FFFFFF");
    hidden_text["visual"] = json!({"hidden":true});
    let report = report("FFFFFF", vec![
        styled_rect("ghost", [0.0, 0.0, 400.0, 150.0], DARK, json!({"hidden":true})),
        text("on-white", TEXT_AT, "000000"),
        hidden_text,
    ]);
    assert!(outcome(&report, "on-white").is_empty(), "a hidden rectangle is not the background");
    assert_eq!(outcome(&report, "hidden-text"), ["text_contrast_low"]);
}

#[test]
fn issue76_text_in_a_shape_uses_the_shapes_own_solid_fill() {
    let with_text = |id: &str, fill: &str, color: &str, visual: Option<Value>| {
        let mut element = shape(id, "rect", [60.0, 60.0, 300.0, 100.0], fill);
        element["text"] = json!("Synthetic");
        element["color"] = json!(color);
        if let Some(visual) = visual { element["visual"] = visual; }
        element
    };
    let mut clear = with_text("clear", "none", "FFFFFF", None);
    clear["x"] = json!(500);
    let report = report("FFFFFF", vec![
        with_text("dark-card", DARK, "FFFFFF", None),
        { let mut element = with_text("pale-card", "F4F4F4", "FFFFFF", None); element["y"] = json!(300); element },
        { let mut element = with_text("fade-card", DARK, "FFFFFF", Some(json!({"opacity":0.4}))); element["y"] = json!(500); element },
        clear,
    ]);
    assert!(outcome(&report, "dark-card").is_empty());
    assert_eq!(outcome(&report, "pale-card"), ["text_contrast_low"]);
    assert_eq!(outcome(&report, "fade-card"), ["contrast_background_unverified:background_not_solid"]);
    assert_eq!(outcome(&report, "clear"), ["text_contrast_low"], "a shape without a fill lets the slide background show");
}

#[test]
fn issue76_counts_cover_every_text_element_and_low_contrast_counts_as_checked() {
    let mut decorative = text("decoration", [60.0, 300.0, 200.0, 40.0], "FFFFFF");
    decorative["text"] = json!("Ornament");
    let mut value = slide("FFFFFF", vec![
        text("fine", TEXT_AT, "000000"),
        text("pale", [60.0, 120.0, 200.0, 40.0], "DDDDDD"),
        picture("photo", [0.0, 400.0, 300.0, 100.0]), text("on-photo", [10.0, 410.0, 100.0, 30.0], "000000"),
        decorative,
    ]);
    value["review"] = json!({"accessibility":{"decoration":{"decorative":true}}});
    let report = check_accessibility(&deck(value, None));
    assert_eq!((report.contrast_checked, report.contrast_unsupported), (2, 1), "{:?}", report.issues);
    assert!(outcome(&report, "decoration").is_empty());
    assert!(report.checked_features.iter().any(|feature| feature == "solid_rgb_text_contrast"));
    let serialized = serde_json::to_value(&report).unwrap();
    assert_eq!((serialized["contrast_checked"].as_u64(), serialized["contrast_unsupported"].as_u64()), (Some(2), Some(1)));
    assert!(serialized["issues"].as_array().unwrap().iter().filter(|issue| issue["code"] != "contrast_background_unverified").all(|issue| issue.get("reason").is_none()));
    let none = report_without_text();
    assert_eq!((none.contrast_checked, none.contrast_unsupported), (0, 0));
}

fn report_without_text() -> AccessibilityReport { report("FFFFFF", vec![rect("only", [0.0, 0.0, 100.0, 100.0], DARK)]) }
