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
fn issue76_a_turned_shape_keeps_its_own_fill_and_a_group_has_no_opacity_to_fade_its_children() {
    let turned = |id: &str, fill: &str, color: &str, y: f64| {
        let mut element = shape(id, "rect", [60.0, y, 300.0, 100.0], fill);
        element["text"] = json!("Synthetic");
        element["color"] = json!(color);
        element["rotation"] = json!(20);
        element
    };
    let report = report("FFFFFF", vec![turned("dark-turned", DARK, "FFFFFF", 60.0), turned("pale-turned", "F4F4F4", "FFFFFF", 300.0),
        // Text with no fill of its own, inside a turned group, still needs a background it cannot read from the geometry.
        group("turned", [500.0, 300.0, 300.0, 100.0], [300.0, 100.0], Some(json!({"rotation":20})), vec![text("in-turned", [10.0, 10.0, 100.0, 30.0], "000000")])]);
    // The outline is read in the shape's own coordinates, so a turned shape holds its text as an upright one does, without the
    // page geometry the other backgrounds need.
    assert!(outcome(&report, "dark-turned").is_empty());
    assert_eq!(outcome(&report, "pale-turned"), ["text_contrast_low"]);
    assert_eq!(outcome(&report, "in-turned"), ["contrast_background_unverified:transformed_text"]);
    // Only a soft edge can make what a group holds translucent, and that is read as not solid (see the unsupported backgrounds above).
    let faded = deck(slide("FFFFFF", vec![group("g", [0.0, 0.0, 400.0, 200.0], [400.0, 200.0], Some(json!({"opacity":0.5})), vec![rect("r", [0.0, 0.0, 400.0, 200.0], DARK)])]), None);
    assert!(aislide_core::model::validate_deck(&faded).is_err(), "a group cannot carry an opacity");
}

fn labelled(id: &str, preset: &str, bounds: [f64; 4], text: &str, format: Value) -> Value {
    let mut element = shape(id, preset, bounds, DARK);
    element["text"] = json!(text);
    // Navy on navy: when the fill is the backdrop this is a low-contrast finding, and when it is not, an unsupported one.
    element["color"] = json!("223A5E");
    element["format"] = format;
    element
}

#[test]
fn issue76_a_shapes_own_fill_is_the_backdrop_only_where_its_text_is_drawn_inside_the_outline() {
    let padding = |left: f64, top: f64| json!({"left":left,"right":left,"top":top,"bottom":top});
    let centered = |left: f64, top: f64| json!({"alignment":"center","vertical":"middle","padding":padding(left, top)});
    let pill = |id: &str, y: f64, format: Value, visual: Value| {
        let mut element = labelled(id, "roundRect", [60.0, y, 240.0, 36.0], "Label", format);
        let mut style = json!({"adjustments":[{"name":"adj","value":50000}]});
        for (key, value) in visual.as_object().into_iter().flatten() { style[key] = value.clone(); }
        element["visual"] = style;
        element
    };
    let turned = |id: &str, x: f64, text: &str, format: Value| {
        let mut element = labelled(id, "ellipse", [x, 600.0, 200.0, 200.0], text, format);
        element["rotation"] = json!(30);
        element
    };
    let mut custom = labelled("custom", "rect", [600.0, 600.0, 200.0, 200.0], "12", centered(0.0, 0.0));
    custom["visual"] = json!({"path":{"commands":[{"op":"move","point":[0,0]},{"op":"line","point":[1,0]},{"op":"line","point":[0,1]},{"op":"close"}]}});
    let report = report("FFFFFF", vec![
        // Centered text is small and sits in the middle of a circle, though the frame it is drawn in reaches past the outline.
        labelled("badge", "ellipse", [60.0, 60.0, 200.0, 200.0], "12", centered(0.0, 0.0)),
        // Text that starts at the frame's corner, or fills the frame, is drawn partly outside the circle.
        labelled("starts-at-corner", "ellipse", [300.0, 60.0, 200.0, 200.0], "Synthetic", json!({})),
        labelled("fills-the-frame", "ellipse", [540.0, 60.0, 200.0, 200.0], &"Synthetic ".repeat(60), centered(0.0, 0.0)),
        pill("tag", 300.0, centered(2.0, 0.0), json!({})),
        pill("tag-at-the-edge", 360.0, json!({"padding":padding(2.0, 0.0)}), json!({})),
        pill("tag-mirrored", 420.0, centered(2.0, 0.0), json!({"flip_h":true})),
        // A rounded rectangle at its default radius holds its default text frame, so no measuring is needed.
        labelled("card", "roundRect", [60.0, 480.0, 240.0, 36.0], "Label", json!({})),
        turned("turned-badge", 60.0, "12", centered(0.0, 0.0)),
        turned("turned-corner", 300.0, "Synthetic", json!({})),
        custom,
        labelled("arrow", "chevron", [900.0, 600.0, 200.0, 100.0], "Next", centered(24.0, 2.0)),
    ]);
    for id in ["badge", "tag", "card", "turned-badge"] { assert_eq!(outcome(&report, id), ["text_contrast_low"], "{id}: judged against its own fill"); }
    for id in ["starts-at-corner", "fills-the-frame", "tag-at-the-edge", "tag-mirrored", "turned-corner"] {
        assert_eq!(outcome(&report, id), ["contrast_background_unverified:partial_cover"], "{id}: its text leaves the outline");
    }
    for id in ["custom", "arrow"] { assert_eq!(outcome(&report, id), ["contrast_background_unverified:unsupported_backdrop"], "{id}: an outline that is not measured"); }
}

#[test]
fn issue76_a_line_or_an_outline_across_the_text_leaves_its_backdrop_unknown() {
    let line = |id: &str, bounds: [f64; 4], width: f64| json!({"type":"connector","id":id,"x":bounds[0],"y":bounds[1],"width":bounds[2],"height":bounds[3],"color":"000000","stroke_width":width,"arrow":false});
    let outlined = |id: &str, preset: &str, bounds: [f64; 4]| {
        let mut element = shape(id, preset, bounds, "none");
        element["stroke"] = json!("000000");
        element["stroke_width"] = json!(4);
        element
    };
    let mut bordered = shape("bordered", "rect", [40.0, 560.0, 300.0, 60.0], DARK);
    bordered["stroke"] = json!("FFFFFF");
    bordered["stroke_width"] = json!(6);
    let report = report("FFFFFF", vec![
        line("line", [0.0, 120.0, 400.0, 1.0], 2.0),
        text("crossed", [60.0, 100.0, 200.0, 40.0], "000000"),
        text("beside", [60.0, 200.0, 200.0, 40.0], "000000"),
        // A line under a covering shape is hidden by it; a line over the shape still crosses the text.
        line("under", [0.0, 320.0, 400.0, 1.0], 2.0),
        rect("panel", [40.0, 290.0, 300.0, 60.0], DARK),
        text("on-panel", [60.0, 300.0, 200.0, 40.0], "FFFFFF"),
        rect("panel-2", [40.0, 390.0, 300.0, 60.0], DARK),
        line("over", [0.0, 420.0, 400.0, 1.0], 2.0),
        text("through-panel", [60.0, 400.0, 200.0, 40.0], "FFFFFF"),
        // A shape with no fill draws only its outline: text inside it is on the slide, text on the outline is not.
        outlined("ring", "ellipse", [500.0, 60.0, 300.0, 300.0]),
        text("in-ring", [590.0, 170.0, 120.0, 60.0], "000000"),
        text("on-ring", [490.0, 190.0, 100.0, 40.0], "000000"),
        outlined("card", "roundRect", [500.0, 400.0, 400.0, 200.0]),
        text("in-card", [520.0, 420.0, 360.0, 60.0], "000000"),
        text("on-card-edge", [520.0, 390.0, 200.0, 40.0], "000000"),
        // An outline that is not measured covers its whole frame with something that is not one color.
        outlined("arrow", "chevron", [940.0, 60.0, 300.0, 100.0]),
        text("in-chevron", [990.0, 80.0, 100.0, 40.0], "000000"),
        // The border of a filled shape is an edge of its fill, not a backdrop.
        bordered,
        text("in-bordered", [40.0, 560.0, 300.0, 60.0], "FFFFFF"),
        // The stroke is as wide as it is drawn: its center line lies outside the frame, but its edge does not.
        line("thick", [900.0, 250.0, 340.0, 1.0], 20.0),
        text("near-thick", [960.0, 210.0, 200.0, 35.0], "000000"),
        text("clear-of-thick", [960.0, 280.0, 200.0, 35.0], "000000"),
        // A shadow or a reflection reaches past the shape that casts it; the shape itself still covers the text on it.
        styled_rect("shadowed", [940.0, 360.0, 200.0, 80.0], DARK, json!({"shadow":{"color":"000000","opacity":0.4,"blur":6,"distance":4,"angle":45}})),
        text("on-shadowed", [960.0, 380.0, 100.0, 30.0], "FFFFFF"),
        text("in-halo", [1146.0, 400.0, 60.0, 30.0], "000000"),
        text("clear-of-halo", [1180.0, 400.0, 60.0, 30.0], "000000"),
        styled_rect("mirrored", [940.0, 480.0, 200.0, 40.0], DARK, json!({"reflection":{"blur":2,"distance":2,"start_opacity":0.5,"end_opacity":0.0,"end_position":0.5}})),
        text("in-reflection", [960.0, 530.0, 100.0, 25.0], "000000"),
        text("below-reflection", [960.0, 620.0, 100.0, 25.0], "000000"),
    ]);
    for id in ["beside", "on-panel", "in-ring", "in-card", "in-bordered", "clear-of-thick", "on-shadowed", "clear-of-halo", "below-reflection"] { assert!(outcome(&report, id).is_empty(), "{id}: {:?}", report.issues); }
    for id in ["crossed", "through-panel", "on-ring", "on-card-edge", "in-chevron", "near-thick"] {
        assert_eq!(outcome(&report, id), ["contrast_background_unverified:unsupported_backdrop"], "{id}");
    }
    for id in ["in-halo", "in-reflection"] { assert_eq!(outcome(&report, id), ["contrast_background_unverified:background_not_solid"], "{id}"); }
}

#[test]
fn issue76_the_corners_and_arrowheads_of_a_line_are_part_of_its_stroke_and_its_flat_ends_are_not() {
    let connector = |id: &str, bounds: [f64; 4], width: f64, arrow: bool, routing: Option<Value>| {
        let mut element = json!({"type":"connector","id":id,"x":bounds[0],"y":bounds[1],"width":bounds[2],"height":bounds[3],"color":"000000","stroke_width":width,"arrow":arrow});
        if let Some(routing) = routing { element["routing"] = routing; }
        element
    };
    let mut ghost = connector("ghost", [100.0, 600.0, 200.0, 0.01], 10.0, true, None);
    ghost["visual"] = json!({"hidden":true});
    let report = report("FFFFFF", vec![
        // A turn of 30 degrees: the miter at the corner (300, 100) reaches 19 px past it, beyond the bands of both segments.
        connector("sharp", [100.0, 100.0, 200.0, 100.0], 10.0, false, Some(json!({"points":[[0,0],[1,0],[0.134,1]]}))),
        // A 10 px line ends in an arrowhead 40 px long and 32 px wide, much wider than the line itself.
        connector("pointing", [100.0, 300.0, 200.0, 0.01], 10.0, true, None),
        connector("plain", [100.0, 500.0, 200.0, 0.01], 10.0, false, None),
        connector("flat", [500.0, 300.0, 200.0, 0.01], 10.0, false, None),
        ghost,
        text("in-the-miter", [308.0, 96.0, 4.0, 3.0], "000000"),
        text("beyond-the-miter", [340.0, 96.0, 4.0, 3.0], "000000"),
        text("beside-the-arrowhead", [270.0, 286.0, 20.0, 6.0], "000000"),
        text("beside-the-plain-line", [270.0, 486.0, 20.0, 6.0], "000000"),
        text("on-the-flat-end", [680.0, 295.0, 15.0, 10.0], "000000"),
        text("past-the-flat-end", [702.0, 295.0, 30.0, 10.0], "000000"),
        text("under-the-hidden-line", [150.0, 595.0, 50.0, 10.0], "000000"),
    ]);
    for id in ["in-the-miter", "beside-the-arrowhead", "on-the-flat-end"] {
        assert_eq!(outcome(&report, id), ["contrast_background_unverified:unsupported_backdrop"], "{id}");
    }
    for id in ["beyond-the-miter", "beside-the-plain-line", "past-the-flat-end", "under-the-hidden-line"] { assert!(outcome(&report, id).is_empty(), "{id}: {:?}", report.issues); }
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
