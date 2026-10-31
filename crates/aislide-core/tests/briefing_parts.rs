use aislide_core::execute_request;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

const BRIEFING: [&str; 6] = ["list-horizontal/icon-cards", "list/icon-rows", "before-after/shift", "flow/cards", "list/agenda", "list-enumeration/screenshot-callouts"];
const EDITORIAL: [&str; 6] = ["flow/open-steps", "vertical-flow/rail", "flow/roadmap", "list-horizontal/icon-columns", "list-horizontal/fact-columns", "list-horizontal/image-columns"];
const FRAME: [f64; 4] = [48.0, 120.0, 1184.0, 540.0];

fn icon(width: u32, height: u32) -> Value {
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"><rect x="2" y="2" width="{}" height="{}" fill="teal"/></svg>"#, width - 4, height - 4);
    execute_request(json!({"op":"create_graph_icon","base64":STANDARD.encode(svg),"mime_type":"image/svg+xml","alt":"Synthetic indicator"})).unwrap()
}

fn deck(elements: Vec<Value>) -> Value {
    json!({"version":1,"title":"Briefing parts","width":1280,"height":720,"slides":[{"id":"slide","title":"Briefing","background":"FFFFFF","notes":"Synthetic examples","elements":elements}]})
}

fn example(id: &str) -> Value {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    catalog["presets"].as_array().unwrap().iter().find(|entry| entry["id"] == id).unwrap_or_else(|| panic!("missing {id}"))["example"].clone()
}

fn framed(mut spec: Value) -> Value {
    spec["layout"] = json!({"x":FRAME[0],"y":FRAME[1],"width":FRAME[2],"height":FRAME[3],"show_title":false});
    spec
}

fn children(element: &Value) -> &Vec<Value> { element["children"].as_array().unwrap() }

fn number(value: &Value, field: &str) -> f64 { value[field].as_f64().unwrap() }

fn run_sizes(child: &Value) -> Vec<f64> {
    child["format"]["paragraphs"].as_array().map(|paragraphs| paragraphs.iter().flat_map(|paragraph| paragraph["runs"].as_array().unwrap().iter().map(|run| run["style"]["font_size"].as_f64().unwrap())).collect()).unwrap_or_default()
}

fn create(spec: &Value) -> Result<Value, String> { execute_request(json!({"op":"create_part","id":"briefing","spec":spec})).map_err(|error| error.to_string()) }

fn screenshot_spec(callouts: Value) -> Value {
    let image = icon(256, 160);
    framed(json!({"version":1,"preset":"list-enumeration/screenshot-callouts","title":"","data":{"kind":"screenshot_callouts","image":{"base64":image["base64"],"mime_type":image["mime_type"],"alt":"Synthetic settings screen"},"callouts":callouts}}))
}

fn numbered(id: &str, x: f64, y: f64, text: &str) -> Value {
    json!({"type":"shape","id":id,"x":x,"y":y,"width":30,"height":30,"preset":"ellipse","fill":"1F4E9A","stroke":"1F4E9A","stroke_width":1,"text":text,"font_size":14,"color":"FFFFFF","bold":true})
}

fn sequence_findings(elements: Vec<Value>) -> Vec<Value> {
    let document = execute_request(json!({"op":"new_document","id":"sequence","deck":deck(elements)})).unwrap();
    let report = execute_request(json!({"op":"preflight_presentation","document":document,"options":{"page_indices":[0]}})).unwrap();
    report["findings"].as_array().unwrap().iter().filter(|finding| finding["code"] == "NUMBERED_SEQUENCE_UNEVEN").cloned().collect()
}

#[test]
fn editorial_open_steps_draw_a_native_axis_without_cards() {
    let spec = framed(json!({"version":1,"preset":"flow/open-steps","title":"","data":{"kind":"open_steps","steps":[
        {"label":"Collect","detail":"Gather the approved evidence."},
        {"label":"Assess","detail":"Choose the next action."},
        {"label":"Respond","detail":"Record the outcome."}
    ]}}));
    let element = create(&spec).unwrap();
    let items = children(&element);
    assert!(items.iter().any(|child| child["type"] == "connector"), "the common axis is native");
    assert_eq!(items.iter().filter(|child| child["preset"] == "ellipse").count(), 3);
    assert!(!items.iter().any(|child| child["preset"] == "rect" && number(child, "height") > 60.0), "no enclosing cards");
    for child in items { for size in run_sizes(child) { assert!(size >= 14.0, "fixed readable typography"); } }
    let exported = execute_request(json!({"op":"export","deck":deck(vec![element.clone()])})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"open-flow-native","base64":exported["base64"]})).unwrap();
    assert_eq!(children(&opened["document"]["deck"]["slides"][0]["elements"][0]).len(), items.len());
}

#[test]
fn editorial_presets_are_documented_fixed_size_and_reopenable() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    for id in EDITORIAL {
        let entry = catalog["presets"].as_array().unwrap().iter().find(|entry| entry["id"] == id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(entry["recommended"], true, "{id}");
        assert!(!entry["use_when"].as_str().unwrap().is_empty() && !entry["avoid_when"].as_str().unwrap().is_empty(), "{id}");
        for spec in [entry["example"].clone(), framed(entry["example"].clone())] {
            let element = create(&spec).unwrap_or_else(|error| panic!("{id}: {error}"));
            assert_eq!(element, create(&spec).unwrap(), "{id}: deterministic");
            assert_eq!(number(&element, "width"), number(&element, "view_width"), "{id}: native width");
            assert_eq!(number(&element, "height"), number(&element, "view_height"), "{id}: native height");
            for child in children(&element) {
                let [x, y, width, height] = ["x", "y", "width", "height"].map(|field| number(child, field));
                assert!(x >= 0.0 && y >= 0.0 && x + width <= number(&element, "width") + 0.5 && y + height <= number(&element, "height") + 0.5, "{id}: outside frame");
                for size in run_sizes(child) { assert!(size >= 14.0, "{id}: typography is not shrunk"); }
            }
            let scene = deck(vec![element.clone()]);
            let measured = execute_request(json!({"op":"measure_layout","deck":scene})).unwrap();
            assert!(measured["measurements"].as_array().unwrap().iter().all(|item| item["overflow"] == false && item["missing_glyphs"] == 0), "{id}: {measured}");
            let document = execute_request(json!({"op":"new_document","id":"editorial-check","deck":scene})).unwrap();
            let report = execute_request(json!({"op":"preflight_presentation","document":document,"options":{"page_indices":[0],"min_font_size":14}})).unwrap();
            assert!(!report["findings"].as_array().unwrap().iter().any(|finding| ["TEXT_OVERLAP", "CONNECTOR_LABEL_INTERFERENCE", "OFF_SLIDE", "TEXT_OVERFLOW", "CONTAINER_PADDING"].contains(&finding["code"].as_str().unwrap())), "{id}: {report}");
            let exported = execute_request(json!({"op":"export","deck":scene})).unwrap();
            let opened = execute_request(json!({"op":"open_presentation","id":"editorial-native","base64":exported["base64"]})).unwrap();
            assert_eq!(children(&opened["document"]["deck"]["slides"][0]["elements"][0]).len(), children(&element).len(), "{id}");
        }
    }
}

#[test]
fn editorial_limits_reject_invalid_data_and_tiny_frames() {
    let cases: Vec<(&str, Box<dyn Fn(&mut Value)>, &str)> = vec![
        ("flow/open-steps", Box::new(|data| data["steps"] = json!([{ "label":"Only" }])), "2-5 items"),
        ("vertical-flow/rail", Box::new(|data| data["steps"][0]["label"] = json!(" ")), "label is required"),
        ("flow/roadmap", Box::new(|data| data["phases"][0]["points"] = json!(["a","b","c","d","e"])), "four points"),
        ("flow/roadmap", Box::new(|data| data["phases"][0]["period"] = json!("Now\nLater")), "one line"),
        ("list-horizontal/icon-columns", Box::new(|data| data["items"][0]["accent"] = json!("blue")), "color"),
        ("list-horizontal/fact-columns", Box::new(|data| data["columns"] = json!(1)), "2-3 columns"),
        ("list-horizontal/fact-columns", Box::new(|data| data["items"][0]["value"] = json!("1\n2")), "one line"),
        ("list-horizontal/image-columns", Box::new(|data| { data["items"][0].as_object_mut().unwrap().remove("image"); }), "image"),
    ];
    for (id, edit, expected) in cases {
        let mut spec = example(id);
        edit(&mut spec["data"]);
        let error = create(&spec).unwrap_err();
        assert!(error.contains(expected), "{id}: {error}");
    }
    for id in EDITORIAL {
        let mut spec = example(id);
        spec["layout"] = json!({"x":40,"y":120,"width":360,"height":150,"show_title":false});
        assert!(create(&spec).is_err(), "{id} accepts a tiny frame");
    }
    let mut wrong = example("flow/open-steps");
    wrong["preset"] = json!("vertical-flow/rail");
    assert!(create(&wrong).unwrap_err().contains("requires rail_steps"));
    let mut long = example("flow/open-steps");
    long["layout"] = json!({"x":40,"y":120,"width":1152,"height":164,"show_title":false});
    long["data"]["steps"] = json!((0..5).map(|index| json!({"label":format!("Stage {index}"),"detail":"A long phrase ".repeat(11)})).collect::<Vec<_>>());
    assert!(create(&long).unwrap_err().contains("fixed font size"));
}

#[test]
fn editorial_documented_maximum_counts_render_without_shrinking() {
    for (id, key, count) in [
        ("flow/open-steps","steps",5), ("vertical-flow/rail","steps",5), ("flow/roadmap","phases",4),
        ("list-horizontal/icon-columns","items",4), ("list-horizontal/fact-columns","items",6), ("list-horizontal/image-columns","items",4),
    ] {
        let mut spec = example(id);
        let item = spec["data"][key][0].clone();
        spec["data"][key] = json!(vec![item; count]);
        for spec in [spec.clone(), framed(spec)] {
            let element = create(&spec).unwrap_or_else(|error| panic!("{id}: {error}"));
            for child in children(&element) { for size in run_sizes(child) { assert!(size >= 14.0, "{id}"); } }
        }
    }
}

#[test]
fn editorial_images_are_contained_and_facts_do_not_encode_values_as_area() {
    let mut images = framed(example("list-horizontal/image-columns"));
    let wide = icon(240, 120);
    let tall = icon(120, 240);
    images["data"]["items"] = json!([
        {"image":wide,"label":"Wide evidence","detail":"Every edge stays visible."},
        {"image":tall,"label":"Tall evidence","detail":"No crop is introduced."}
    ]);
    let rendered = create(&images).unwrap();
    let pictures: Vec<&Value> = children(&rendered).iter().filter(|child| child["type"] == "picture").collect();
    assert_eq!(pictures.len(), 2);
    for (picture, ratio) in pictures.iter().zip([2.0, 0.5]) {
        assert!((number(picture,"width") / number(picture,"height") - ratio).abs() < 1e-6);
        for side in ["left","right","top","bottom"] { assert_eq!(picture["crop"][side].as_f64().unwrap_or(0.0), 0.0); }
    }
    let mut facts = framed(example("list-horizontal/fact-columns"));
    facts["data"]["items"] = json!([
        {"value":"0.04","unit":"%","label":"First fact","qualifier":"Its own denominator."},
        {"value":"85","unit":"%","label":"Second fact","qualifier":"A different denominator."}
    ]);
    let rendered = create(&facts).unwrap();
    assert!(!children(&rendered).iter().any(|child| child["type"] == "chart" || child["type"] == "connector"));
    let rules: Vec<&Value> = children(&rendered).iter().filter(|child| child["preset"] == "rect").collect();
    assert_eq!(rules.len(), 2);
    assert_eq!([number(rules[0],"width"), number(rules[0],"height")], [number(rules[1],"width"), number(rules[1],"height")]);
    assert!(children(&rendered).iter().any(|child| child["text"].as_str().is_some_and(|text| text.contains("0.04 %") && text.contains("Its own denominator."))));
}

#[test]
fn screenshot_callouts_keep_badges_on_the_image_and_a_uniform_legend() {
    let long = "Detail text that wraps onto a second line in the legend column.";
    let callouts = json!([{"x":0.25,"y":0.2,"label":"Enable","detail":"Short."},{"x":0.1,"y":0.5,"label":"Include and exclude","detail":long},
        {"x":0.9,"y":0.95,"label":"Add targets","detail":"Short."},{"x":0.0,"y":0.0,"label":"Choose profiles","detail":format!("{long}\n{long}")}]);
    let element = create(&screenshot_spec(callouts.clone())).unwrap();
    let items = children(&element);
    let picture = items.iter().find(|child| child["type"] == "picture").unwrap();
    let [px, py, pw, ph] = ["x", "y", "width", "height"].map(|field| number(picture, field));
    assert!((pw / ph - 256.0 / 160.0).abs() < 0.01, "screenshot keeps its aspect ratio");
    let badges: Vec<&Value> = items.iter().filter(|child| child["preset"] == "ellipse").collect();
    let (on_image, legend): (Vec<&Value>, Vec<&Value>) = badges.into_iter().partition(|badge| number(badge, "x") < px + pw);
    assert_eq!((on_image.len(), legend.len()), (4, 4));
    for (index, badge) in on_image.iter().enumerate() {
        let [x, y, size] = [number(badge, "x"), number(badge, "y"), number(badge, "width")];
        let expected = [(px + callouts[index]["x"].as_f64().unwrap() * pw).clamp(px + size / 2.0, px + pw - size / 2.0), (py + callouts[index]["y"].as_f64().unwrap() * ph).clamp(py + size / 2.0, py + ph - size / 2.0)];
        assert!((x + size / 2.0 - expected[0]).abs() < 0.01 && (y + size / 2.0 - expected[1]).abs() < 0.01, "badge {index} follows its image fraction");
        assert!(x >= px && y >= py && x + size <= px + pw + 0.01 && y + size <= py + ph + 0.01, "badge {index} stays on the image");
        assert_eq!(badge["text"], (index + 1).to_string());
        assert_eq!(legend[index]["text"], (index + 1).to_string());
        assert_eq!(badge["fill"], legend[index]["fill"]);
    }
    let tops: Vec<f64> = legend.iter().map(|badge| number(badge, "y")).collect();
    let pitches: Vec<f64> = tops.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert!(pitches.iter().all(|pitch| (pitch - pitches[0]).abs() < 0.01 && *pitch > 0.0), "legend rows share one pitch: {pitches:?}");
    assert!(legend.iter().all(|badge| (number(badge, "x") - number(legend[0], "x")).abs() < 0.01), "legend badges share one left edge");
    assert!(sequence_findings(vec![element]).is_empty());
}

#[test]
fn screenshot_callouts_reject_unbounded_positions_and_counts() {
    let item = |x: f64| json!({"x":x,"y":0.5,"label":"Item"});
    assert!(create(&screenshot_spec(json!([item(1.2)]))).unwrap_err().contains("0-1 fractions"));
    assert!(create(&screenshot_spec(json!([item(-0.1)]))).unwrap_err().contains("0-1 fractions"));
    assert!(create(&screenshot_spec(json!((0..7).map(|_| item(0.5)).collect::<Vec<_>>()))).unwrap_err().contains("1-6 items"));
    for region in [
        json!({"x": 0.9, "y": 0.4, "width": 0.2, "height": 0.2}),
        json!({"x": 0.4, "y": 0.4, "width": 0, "height": 0.2}),
        json!({"x": 0.8, "y": 0.4, "width": 0.1, "height": 0.2}),
    ] {
        let mut callout = item(0.5);
        callout["target_region"] = region;
        assert!(create(&screenshot_spec(json!([callout]))).unwrap_err().contains("target_region"));
    }
    let mut wrong = screenshot_spec(json!([item(0.5)]));
    wrong["preset"] = json!("list/icon-rows");
    assert!(create(&wrong).unwrap_err().contains("requires icon_rows data"));
}

#[test]
fn preflight_flags_unevenly_spaced_numbered_columns_but_not_screenshot_callouts() {
    let uneven = sequence_findings(vec![numbered("n1", 880.0, 124.0, "1"), numbered("n2", 880.0, 214.0, "2"), numbered("n3", 880.0, 322.0, "3"), numbered("n4", 880.0, 412.0, "4")]);
    assert_eq!(uneven.len(), 1, "{uneven:?}");
    assert_eq!(uneven[0]["element_ids"], json!(["n1", "n2", "n3", "n4"]));
    assert!(uneven[0]["message"].as_str().unwrap().contains("90, 108, 90"));
    let misaligned = sequence_findings(vec![numbered("a", 880.0, 124.0, "1"), numbered("b", 884.0, 214.0, "2"), numbered("c", 880.0, 304.0, "3")]);
    assert_eq!(misaligned.len(), 1, "a 4px left-edge offset is reported");
    assert!(sequence_findings(vec![numbered("e1", 880.0, 124.0, "1"), numbered("e2", 880.0, 214.0, "2"), numbered("e3", 880.0, 304.0, "3")]).is_empty());
    let image = icon(256, 160);
    let mut picture = execute_request(json!({"op":"create_picture","id":"shot","base64":image["base64"],"mime_type":"image/png","alt":"Synthetic screen"})).unwrap();
    for (field, value) in [("x", 48.0), ("y", 120.0), ("width", 800.0), ("height", 500.0)] { picture[field] = json!(value); }
    let annotated = sequence_findings(vec![picture, numbered("c1", 138.0, 293.0, "1"), numbered("c2", 138.0, 326.0, "2"), numbered("c3", 138.0, 420.0, "3")]);
    assert!(annotated.is_empty(), "badges over a screenshot mark screen positions, not a list");
}

#[test]
fn briefing_presets_are_recommended_deterministic_native_size_parts() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    for id in BRIEFING {
        let entry = catalog["presets"].as_array().unwrap().iter().find(|entry| entry["id"] == id).unwrap();
        assert_eq!(entry["recommended"], true, "{id}");
        assert!(!entry["use_when"].as_str().unwrap().is_empty() && !entry["avoid_when"].as_str().unwrap().is_empty(), "{id}");
        let spec = entry["example"].clone();
        let default = create(&spec).unwrap();
        assert_eq!(default, create(&spec).unwrap(), "{id}: deterministic");
        assert_eq!([number(&default, "x"), number(&default, "y"), number(&default, "width"), number(&default, "height"), number(&default, "view_width"), number(&default, "view_height")], [64.0, 144.0, 1152.0, 512.0, 1152.0, 512.0], "{id}");
        let element = create(&framed(spec)).unwrap();
        assert_eq!([number(&element, "view_width"), number(&element, "view_height")], [FRAME[2], FRAME[3]], "{id}: rendered at the frame size, not scaled");
        for child in children(&element) {
            let [x, y, width, height] = ["x", "y", "width", "height"].map(|field| number(child, field));
            assert!(x >= 0.0 && y >= 0.0 && x + width <= FRAME[2] + 0.5 && y + height <= FRAME[3] + 0.5, "{id}: child outside frame {child}");
            for size in run_sizes(child) { assert!((12.0..=22.0).contains(&size), "{id}: unexpected font size {size}"); }
        }
        let scene = deck(vec![element.clone()]);
        let measured = execute_request(json!({"op":"measure_layout","deck":scene})).unwrap();
        assert!(measured["measurements"].as_array().unwrap().iter().all(|entry| entry["overflow"] == false && entry["missing_glyphs"] == 0), "{id}: {measured}");
        let document = execute_request(json!({"op":"new_document","id":"briefing-preflight","deck":scene})).unwrap();
        let preflight = execute_request(json!({"op":"preflight_presentation","document":document,"options":{"page_indices":[0],"min_font_size":12}})).unwrap();
        assert!(!preflight["findings"].as_array().unwrap().iter().any(|finding| ["CONTAINER_PADDING", "CONTAINER_CORNER_OVERFLOW", "TEXT_OVERLAP", "SMALL_TEXT", "OFF_SLIDE"].contains(&finding["code"].as_str().unwrap())), "{id}: {preflight}");
        let exported = execute_request(json!({"op":"export","deck":scene})).unwrap();
        let opened = execute_request(json!({"op":"open_presentation","id":"briefing-native","base64":exported["base64"]})).unwrap();
        assert_eq!(children(&opened["document"]["deck"]["slides"][0]["elements"][0]).len(), children(&element).len(), "{id}");
    }
}

#[test]
fn briefing_parts_reject_overflow_instead_of_shrinking_text() {
    let words = "Repeated synthetic words for overflow. ";
    let cases = [
        ("list-horizontal/icon-cards", json!({"kind":"icon_cards","cards":[{"label":"Alpha","detail":words.repeat(6)},{"label":"Beta","detail":"Short."}]}), [700.0, 180.0]),
        ("list/icon-rows", json!({"kind":"icon_rows","rows":[{"label":"Alpha","detail":words.repeat(5)},{"label":"Beta","detail":"Short."}]}), [520.0, 130.0]),
        ("before-after/shift", json!({"kind":"shift_rows","rows":[{"from":"Before","to":"After","detail":words.repeat(5)},{"from":"Old","to":"New"}]}), [900.0, 140.0]),
        ("flow/cards", json!({"kind":"step_cards","steps":[{"label":"Alpha","detail":words.repeat(6)},{"label":"Beta"}]}), [600.0, 150.0]),
        ("list/agenda", json!({"kind":"agenda","items":[{"label":"Alpha","detail":"Short"},{"label":"Beta"},{"label":"Gamma"},{"label":"Delta"}]}), [800.0, 150.0]),
    ];
    for (preset, data, [width, height]) in cases {
        let spec = json!({"version":1,"preset":preset,"title":"","data":data,"layout":{"x":40,"y":120,"width":width,"height":height,"show_title":false}});
        let error = create(&spec).unwrap_err();
        assert!(error.contains("fit"), "{preset}: {error}");
    }
}

#[test]
fn briefing_presets_bind_to_their_data_kinds_and_bounded_fields() {
    let cards = example("list-horizontal/icon-cards");
    let mut wrong = cards.clone(); wrong["preset"] = json!("list/icon-rows");
    assert!(create(&wrong).unwrap_err().contains("requires icon_rows data"));
    let mut legacy = cards.clone(); legacy["preset"] = json!("list/rows");
    assert!(create(&legacy).unwrap_err().contains("matching briefing preset"));
    let mut unknown = cards.clone(); unknown["data"]["cards"][0]["unexpected"] = json!(true);
    assert!(create(&unknown).is_err());
    let card = json!({"label":"Topic","detail":"Short explanation."});
    for (field, value, expected) in [
        ("cards", json!(vec![card.clone(); 7]), "2-6 items"),
        ("columns", json!(5), "2-4 columns"),
        ("body_size", json!(30), "14-22px"),
    ] {
        let mut invalid = cards.clone(); invalid["data"][field] = value;
        let error = create(&invalid).unwrap_err();
        assert!(error.contains(expected), "{field}: {error}");
    }
    let mut points = cards.clone(); points["data"]["cards"][0]["points"] = json!(["a", "b", "c", "d", "e"]);
    assert!(create(&points).unwrap_err().contains("four points"));
    let mut blank = cards.clone(); blank["data"]["cards"][0]["label"] = json!("  ");
    assert!(create(&blank).unwrap_err().contains("card label is required"));
    let mut long = cards.clone(); long["data"]["message"]["text"] = json!("x".repeat(121));
    assert!(create(&long).unwrap_err().contains("message text exceeds 120"));
    let mut color = cards; color["data"]["cards"][0]["accent"] = json!("blue");
    assert!(create(&color).is_err());
}

#[test]
fn icon_cards_fit_content_and_place_the_message_band_below() {
    let element = create(&framed(example("list-horizontal/icon-cards"))).unwrap();
    let cards: Vec<_> = children(&element).iter().filter(|child| child["preset"] == "rect" && child["fill"] == "@lt1").collect();
    assert_eq!(cards.len(), 3);
    let natural = FRAME[3] - 52.0 - 16.0;
    let card_height = number(cards[0], "height");
    assert!(card_height < natural - 40.0 && card_height >= 120.0, "cards keep empty space: {card_height}");
    assert!(cards.iter().all(|card| number(card, "height") == card_height && number(card, "y") == 0.0));
    let band = children(&element).iter().find(|child| child["preset"] == "rect" && child["fill"] == "@dk1").unwrap();
    assert_eq!(number(band, "y"), card_height + 16.0);
    assert_eq!(number(band, "width"), FRAME[2]);
    let message = children(&element).iter().find(|child| child["text"] == "People set direction; automation extends reach.").unwrap();
    assert!(number(message, "y") > number(band, "y"));
}

#[test]
fn briefing_icons_and_images_keep_their_aspect_ratio() {
    let square = icon(24, 24);
    let wide = icon(200, 100);
    let mut cards = framed(example("list-horizontal/icon-cards"));
    for card in cards["data"]["cards"].as_array_mut().unwrap() { card["icon"] = square.clone(); }
    let mut steps = framed(example("flow/cards"));
    for step in steps["data"]["steps"].as_array_mut().unwrap() { step["image"] = wide.clone(); step["icon"] = square.clone(); }
    let mut rows = framed(example("list/icon-rows"));
    for row in rows["data"]["rows"].as_array_mut().unwrap() { row["icon"] = square.clone(); }
    for spec in [cards, rows, steps] {
        let element = create(&spec).unwrap();
        let mut images = 0;
        for child in children(&element) {
            if child["preset"] == "ellipse" { assert_eq!(number(child, "width"), number(child, "height")); }
            if child["type"] != "picture" { continue; }
            images += 1;
            let crop = &child["crop"];
            let horizontal = number(child, "width") / (1.0 - crop["left"].as_f64().unwrap_or(0.0) - crop["right"].as_f64().unwrap_or(0.0));
            let vertical = number(child, "height") / (1.0 - crop["top"].as_f64().unwrap_or(0.0) - crop["bottom"].as_f64().unwrap_or(0.0));
            let expected = if number(child, "width") > 60.0 { 2.0 } else { 1.0 };
            assert!((horizontal / vertical - expected).abs() < 0.01, "{}: distorted image {child}", spec["preset"]);
        }
        assert!(images >= 3, "{}", spec["preset"]);
        let document = execute_request(json!({"op":"new_document","id":"briefing-aspect","deck":deck(vec![element])})).unwrap();
        let preflight = execute_request(json!({"op":"preflight_presentation","document":document,"options":{"page_indices":[0],"min_font_size":12}})).unwrap();
        assert!(!preflight["findings"].as_array().unwrap().iter().any(|finding| ["CONTAINER_PADDING", "IMAGE_ASPECT_DISTORTED", "TEXT_OVERFLOW", "TEXT_CLIPPED", "MISSING_GLYPHS", "SMALL_TEXT"].contains(&finding["code"].as_str().unwrap())), "{}: {preflight}", spec["preset"]);
    }
}

#[test]
fn briefing_parts_are_managed_through_native_reopen_update_and_undo() {
    let square = icon(24, 24);
    let spec = json!({"version":1,"preset":"list-horizontal/icon-cards","title":"","data":{"kind":"icon_cards","numbered":true,"message":{"text":"人が方向を定め、自動化が範囲を広げる。"},"cards":[
        {"label":"統合型の運用基盤","caption":"Unified operations","detail":"シグナル、コンテキスト、制御を一つの基盤で扱う。","tag":"第 2 部","icon":square},
        {"label":"継続的な保護","caption":"Continuous protection","detail":"検知、予測、阻止を切れ目なく回す。","tag":"第 3 部","icon":square},
        {"label":"エージェント型の運用","caption":"Agent operations","detail":"調査はエージェント、判断と優先度は人が担う。","points":["承認の範囲を決める","成果を測る"],"icon":square}
    ]},"layout":{"x":48,"y":120,"width":1184,"height":540,"show_title":false}});
    let document = execute_request(json!({"op":"new_document","id":"briefing-managed","deck":deck(Vec::new())})).unwrap();
    let inserted = execute_request(json!({"op":"apply_operations","document":document,"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"add_part","slide_id":"slide","id":"briefing-cards","spec":spec}]})).unwrap();
    let group = &inserted["document"]["deck"]["slides"][0]["elements"][0];
    assert_eq!([number(group, "x"), number(group, "y"), number(group, "width"), number(group, "height")], FRAME);
    let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"briefing-reopened","base64":saved["base64"]})).unwrap();
    assert_eq!(opened["document"]["parts"][0]["stale"], false);
    assert_eq!(opened["document"]["parts"][0]["spec"], inserted["document"]["parts"][0]["spec"]);
    let mut changed = spec.clone();
    changed["data"]["cards"][1]["detail"] = json!("検知、予測、阻止と事後の知見を切れ目なく回す。");
    let updated = execute_request(json!({"op":"update_part","document":opened["document"],"expected_revision":0,"slide_id":"slide","id":"briefing-cards","spec":changed})).unwrap();
    assert_eq!(updated["document"]["parts"][0]["stale"], false);
    assert!(updated["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().iter().any(|child| child["text"].as_str().is_some_and(|text| text.contains("事後の知見"))));
    let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":1,"receipt":updated["receipt"]})).unwrap();
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], saved["base64"]);
}

#[test]
fn composition_part_blocks_accept_briefing_presets() {
    let document = execute_request(json!({"op":"new_document","id":"briefing-composition","deck":deck(Vec::new())})).unwrap();
    let mut spec = example("before-after/shift");
    spec["title"] = json!("");
    spec["subtitle"] = json!("");
    let composed = execute_request(json!({"op":"apply_operations","document":document,"expected_revision":0,"expected_hash":document["hash"],"operations":[
        {"op":"compose_slide","slide_id":"slide","id":"shift","spec":{"title":"Role changes","blocks":[{"kind":"part","spec":spec}]}}
    ]})).unwrap();
    assert_eq!(composed["document"]["parts"][0]["spec"]["preset"], "before-after/shift");
    let group = composed["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["type"] == "group").unwrap();
    assert_eq!(number(group, "view_width"), number(group, "width"));
    assert_eq!(number(group, "view_height"), number(group, "height"));
}

fn preflight_part(preset: &str, id: &str) -> Value {
    let document = execute_request(json!({"op":"create_presentation","id":format!("small-{id}"),"title":"Small text"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide-1","id":id,"spec":example(preset)})).unwrap();
    execute_request(json!({"op":"preflight_presentation","document":inserted["document"],"options":{"page_indices":[0]}})).unwrap()
}

fn small_text(report: &Value) -> Vec<&Value> {
    report["findings"].as_array().unwrap().iter().filter(|finding| finding["code"] == "SMALL_TEXT").collect()
}

#[test]
fn issue18_fixed_part_typography_is_information_not_a_warning() {
    let mut reported = 0;
    for preset in BRIEFING.iter().chain(EDITORIAL.iter()) {
        let report = preflight_part(preset, "part");
        for finding in small_text(&report) {
            reported += 1;
            assert_eq!(finding["severity"], "info", "{preset}: a size the preset fixes is not an actionable warning: {finding}");
            let message = finding["message"].as_str().unwrap();
            assert!(message.contains(preset), "{preset}: the message names the preset: {message}");
            assert!(message.to_lowercase().contains("fixed"), "{preset}: the message says the size is fixed: {message}");
            let suggestions = finding["suggestions"].to_string();
            assert!(suggestions.contains("preset") || suggestions.contains("split"), "{preset}: an executable suggestion: {suggestions}");
        }
    }
    assert!(reported >= 2, "the reported presets list/agenda and list-horizontal/icon-cards must still be reported, as information");
}

#[test]
fn issue18_reported_presets_keep_their_finding_as_information() {
    for (preset, minimum) in [("list/agenda", 6), ("list-horizontal/icon-cards", 3)] {
        let report = preflight_part(preset, "part");
        let findings = small_text(&report);
        assert!(findings.len() >= minimum, "{preset}: {report}");
        assert!(findings.iter().all(|finding| finding["severity"] == "info"), "{preset}: {findings:?}");
    }
}

#[test]
fn issue18_ordinary_small_text_is_still_a_warning() {
    let small = json!({"type":"text","id":"small","x":40,"y":40,"width":300,"height":30,"text":"Small body text","font_size":12,"color":"000000","bold":false});
    let document = execute_request(json!({"op":"new_document","id":"ordinary-small","deck":deck(vec![small])})).unwrap();
    let report = execute_request(json!({"op":"preflight_presentation","document":document,"options":{"page_indices":[0]}})).unwrap();
    let findings = small_text(&report);
    assert_eq!(findings.len(), 1, "{report}");
    assert_eq!(findings[0]["severity"], "warning");
    assert!(!findings[0]["message"].as_str().unwrap().to_lowercase().contains("fixed"));
}

#[test]
fn pr21_manually_shrunken_stale_part_text_stays_a_warning() {
    let document = execute_request(json!({"op":"create_presentation","id":"stale-small","title":"Stale text"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,
        "slide_id":"slide-1","id":"part","spec":example("list-horizontal/icon-cards")})).unwrap();
    let children = inserted["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap();
    let index = children.iter().position(|child| child["type"] == "text" && child["font_size"].as_f64() == Some(16.0)).unwrap();
    let id = children[index]["id"].clone();
    let updated = execute_request(json!({"op":"transaction","document":inserted["document"],"transaction":{
        "expected_revision":inserted["document"]["revision"],"expected_hash":inserted["document"]["hash"],
        "operations":[{"op":"replace","path":format!("/deck/slides/0/elements/0/children/{index}/font_size"),"value":9}]
    }})).unwrap();
    assert_eq!(updated["document"]["parts"][0]["stale"], true);
    let report = execute_request(json!({"op":"preflight_presentation","document":updated["document"]})).unwrap();
    let finding = small_text(&report).into_iter().find(|finding| finding["element_ids"].as_array().unwrap().contains(&id)).unwrap();
    assert_eq!(finding["severity"], "warning", "{finding}");
    assert!(!finding["message"].as_str().unwrap().contains("fixed"), "{finding}");
    assert!(!finding["message"].as_str().unwrap().contains("preset does not let"), "{finding}");
    let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],
        "expected_revision":updated["document"]["revision"],"receipt":updated["receipt"]})).unwrap();
    assert_ne!(undone["document"]["parts"][0]["stale"], true);
    let restored = execute_request(json!({"op":"preflight_presentation","document":undone["document"]})).unwrap();
    assert!(small_text(&restored).iter().all(|finding| finding["severity"] == "info"));
}

#[test]
fn issue18_part_text_the_author_can_resize_is_still_a_warning() {
    // Comparison panels have a body_size lever (16-28px), so their text is not fixed typography and must keep warning.
    let spec = json!({"version":1,"preset":"contrast/panels","title":"panels","data":{"kind":"comparison_panels","panels":[
        {"label":"Before","items":[{"text":"Row"}]},{"label":"After","items":[{"text":"Row"}]}]}});
    let document = execute_request(json!({"op":"create_presentation","id":"controlled","title":"Controlled"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide-1","id":"part","spec":spec})).unwrap();
    let report = execute_request(json!({"op":"preflight_presentation","document":inserted["document"],"options":{"page_indices":[0],"min_font_size":20}})).unwrap();
    let findings = small_text(&report);
    assert!(!findings.is_empty(), "18px panel text is below a 20px floor: {report}");
    assert!(findings.iter().all(|finding| finding["severity"] == "warning"), "{findings:?}");
}

fn preflight_spec(spec: Value, id: &str, floor: f64) -> Value {
    let document = execute_request(json!({"op":"create_presentation","id":format!("sized-{id}"),"title":"Body size"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide-1","id":"part","spec":spec})).unwrap();
    execute_request(json!({"op":"preflight_presentation","document":inserted["document"],"options":{"page_indices":[0],"min_font_size":floor}})).unwrap()
}

const SIZED: [&str; 5] = ["list-horizontal/icon-cards", "list/icon-rows", "before-after/shift", "flow/cards", "list-enumeration/screenshot-callouts"];

fn severities(report: &Value) -> Vec<&str> { small_text(report).iter().map(|finding| finding["severity"].as_str().unwrap()).collect() }

#[test]
fn issue18_author_chosen_body_size_below_the_floor_stays_a_warning() {
    // body_size (14-22px) is a lever the author controls, so a body they set under the floor is theirs to raise.
    for preset in SIZED {
        let mut spec = example(preset);
        spec["data"]["body_size"] = json!(14);
        let report = preflight_spec(spec, "low", 16.0);
        let findings = small_text(&report);
        assert!(!findings.is_empty(), "{preset}: a 14px body is below the 16px floor: {report}");
        assert!(findings.iter().all(|finding| finding["severity"] == "warning"), "{preset}: a body the author lowered is actionable: {findings:?}");
        assert!(findings.iter().all(|finding| !finding["message"].as_str().unwrap().to_lowercase().contains("fixed")),
            "{preset}: the warning must not claim the size is fixed: {findings:?}");
    }
}

#[test]
fn issue18_body_size_at_or_above_the_floor_leaves_only_fixed_sizes_as_information() {
    for preset in SIZED {
        for body in [None, Some(16), Some(20)] {
            let mut spec = example(preset);
            if let Some(size) = body { spec["data"]["body_size"] = json!(size); }
            let report = preflight_spec(spec, "ok", 16.0);
            assert!(severities(&report).iter().all(|severity| *severity == "info"), "{preset} body {body:?}: what remains is the preset's fixed text: {report}");
        }
    }
}

#[test]
fn issue18_a_floor_above_the_default_body_makes_it_an_author_choice() {
    // The default body is 16px: under an 18px floor the author can raise body_size, so those findings are warnings.
    let report = preflight_spec(example("list-horizontal/icon-cards"), "floor", 18.0);
    let found = severities(&report);
    assert!(!found.is_empty() && found.iter().all(|severity| *severity == "warning"), "{report}");
    let mut spec = example("list-horizontal/icon-cards");
    spec["data"]["body_size"] = json!(18);
    let raised = preflight_spec(spec, "raised", 18.0);
    assert!(severities(&raised).iter().all(|severity| *severity == "info"), "raising body_size to the floor clears the warning: {raised}");
}

#[test]
fn issue61_part_catalog_schema_matches_icon_column_graph_icon_and_body_size_limits() {
    // Compile-time check: IconColumn is public and constructible, including its legacy field, from outside the crate.
    let _column = aislide_core::parts::IconColumn {
        label: "Item".into(),
        detail: String::new(),
        detail_emphasis: Vec::new(),
        icon: None,
        accent: None,
    };
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    let defs = catalog["schema"].get("$defs").or_else(|| catalog["schema"].get("definitions")).unwrap();

    // 1. IconColumn is distinct from IconRow, accepts only the legacy empty detail_emphasis, and bounds label to 1..=48 scalars.
    assert!(defs.get("IconColumn").is_some(), "missing IconColumn in part_catalog schema");
    assert_eq!(defs["IconColumn"]["additionalProperties"], false);
    assert_eq!(defs["IconColumn"]["properties"]["detail_emphasis"]["maxItems"], 0);
    assert_eq!(defs["IconColumn"]["properties"]["detail_emphasis"]["deprecated"], true);
    assert_eq!(defs["IconColumn"]["properties"]["label"]["minLength"], 1);
    assert_eq!(defs["IconColumn"]["properties"]["label"]["maxLength"], 48);
    assert_eq!(defs["IconColumn"]["properties"]["detail"]["maxLength"], 200);
    let icon_columns_variant = defs["PartData"]["oneOf"].as_array().unwrap().iter()
        .find(|variant| variant["properties"]["kind"]["const"] == "icon_columns").unwrap();
    assert_eq!(icon_columns_variant["properties"]["items"]["items"]["$ref"], "#/$defs/IconColumn");
    assert_eq!(icon_columns_variant["properties"]["items"]["minItems"], 2);
    assert_eq!(icon_columns_variant["properties"]["items"]["maxItems"], 4);

    let mut valid_col = example("list-horizontal/icon-columns");
    valid_col["data"]["items"][0]["label"] = json!("a".repeat(48));
    assert!(create(&valid_col).is_ok());
    valid_col["data"]["items"][0]["label"] = json!("a".repeat(49));
    assert!(create(&valid_col).unwrap_err().contains("48"));
    let mut with_empty_emphasis = example("list-horizontal/icon-columns");
    with_empty_emphasis["data"]["items"][0]["detail_emphasis"] = json!([]);
    assert!(create(&with_empty_emphasis).is_ok(), "legacy empty detail_emphasis remains readable");
    let parsed: aislide_core::parts::PartSpec = serde_json::from_value(with_empty_emphasis.clone()).unwrap();
    let normalized = serde_json::to_value(parsed).unwrap();
    assert!(normalized["data"]["items"][0].get("detail_emphasis").is_none(), "legacy detail_emphasis is removed when serialized");
    let mut with_emphasis = example("list-horizontal/icon-columns");
    with_emphasis["data"]["items"][0]["detail_emphasis"] = json!(["phrase"]);
    assert!(create(&with_emphasis).unwrap_err().contains("Unsupported: icon columns do not draw detail_emphasis"), "the rejection keeps its Unsupported category");

    // 2. GraphIcon schema in part_catalog, graph_catalog, and best_practice_guide documents both asset_id and base64.
    let graph_catalog = execute_request(json!({"op":"graph_catalog"})).unwrap();
    let guide = execute_request(json!({"op":"best_practice_guide","profile_id":"technical-explainer"})).unwrap();
    for schema in [&catalog["schema"], &graph_catalog["schema"], &guide["input_schema"]] {
        let schema_defs = schema.get("$defs").or_else(|| schema.get("definitions")).unwrap();
        let graph_icon = &schema_defs["GraphIcon"];
        assert_eq!(graph_icon["additionalProperties"], false);
        assert_eq!(graph_icon["properties"]["asset_id"]["format"], "uuid");
        assert_eq!(graph_icon["properties"]["base64"]["minLength"], 1);
        assert_eq!(graph_icon["properties"]["base64"]["maxLength"], 1398104);
        assert_eq!(graph_icon["properties"]["mime_type"]["enum"], json!(["image/png", "image/jpeg"]));
        assert_eq!(graph_icon["properties"]["alt"]["maxLength"], 500);
        assert_eq!(graph_icon["oneOf"].as_array().unwrap().len(), 2);
    }

    // 3. body_size bounds in schema match core validation constants and boundary checks.
    assert_eq!(defs["ComparisonPanel"]["properties"]["body_size"]["minimum"], aislide_core::parts::PANEL_BODY_SIZE_MIN);
    assert_eq!(defs["ComparisonPanel"]["properties"]["body_size"]["maximum"], aislide_core::parts::PANEL_BODY_SIZE_MAX);
    for kind in ["icon_cards", "icon_rows", "shift_rows", "step_cards", "screenshot_callouts"] {
        let variant = defs["PartData"]["oneOf"].as_array().unwrap().iter()
            .find(|variant| variant["properties"]["kind"]["const"] == kind).unwrap();
        assert_eq!(variant["properties"]["body_size"]["minimum"], aislide_core::parts::BODY_SIZE_MIN, "{kind}");
        assert_eq!(variant["properties"]["body_size"]["maximum"], aislide_core::parts::BODY_SIZE_MAX, "{kind}");
    }
    for (size, ok) in [(13.9, false), (14.0, true), (22.0, true), (22.1, false)] {
        let mut cards = example("list-horizontal/icon-cards");
        cards["data"]["body_size"] = json!(size);
        assert_eq!(create(&cards).is_ok(), ok, "icon_cards body_size={size}");
    }
    for (size, ok) in [(15.9, false), (16.0, true), (28.0, true), (28.1, false)] {
        let mut panels = example("contrast/panels");
        panels["data"]["panels"][0]["body_size"] = json!(size);
        assert_eq!(create(&panels).is_ok(), ok, "contrast/panels body_size={size}");
    }

    // 4. Each bullet point is bounded per item as well as per list, and the validators enforce the advertised bounds.
    for definition in ["IconCard", "StepCard", "RoadmapPhase"] {
        let points = &defs[definition]["properties"]["points"];
        assert_eq!(points["maxItems"], 4, "{definition}");
        assert_eq!(points["items"]["minLength"], 1, "{definition}");
        assert_eq!(points["items"]["maxLength"], 80, "{definition}");
    }
    let mut long_point = example("list-horizontal/icon-cards");
    long_point["data"]["cards"][0]["points"] = json!(["p".repeat(81)]);
    assert!(create(&long_point).unwrap_err().contains("exceeds 80"));
    let mut blank_point = example("list-horizontal/icon-cards");
    blank_point["data"]["cards"][0]["points"] = json!([" "]);
    assert!(create(&blank_point).unwrap_err().contains("card point is required"));
}

#[test]
fn issue61_graph_icon_schema_tracks_the_struct_and_core_rejects_mcp_asset_handles() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    let schema = &catalog["schema"]["$defs"]["GraphIcon"];

    // Every field core serializes is documented; `asset_id` is the only MCP-only addition.
    let serialized = icon(32, 32);
    let mut core_fields: Vec<&str> = serialized.as_object().unwrap().keys().map(String::as_str).collect();
    let mut documented: Vec<&str> = schema["properties"].as_object().unwrap().keys().map(String::as_str).filter(|name| *name != "asset_id").collect();
    core_fields.sort_unstable();
    documented.sort_unstable();
    assert_eq!(core_fields, documented);
    assert!(schema.get("required").is_none(), "required fields live in the oneOf alternatives");
    assert_eq!(schema["properties"]["asset_id"]["x-mcp-only"], true, "direct callers can tell the handle is MCP-only");
    assert!(schema["description"].as_str().is_some_and(|text| text.contains("asset_id") && text.contains("base64")), "the schema says which form MCP and direct callers use");

    // MCP resolves `asset_id` to base64 before calling core, which has no asset registry and must not accept the handle.
    let mut spec = example("list-horizontal/icon-cards");
    spec["data"]["cards"][0]["icon"] = json!({"asset_id":"123e4567-e89b-42d3-a456-426614174000","mime_type":"image/png","alt":"Synthetic indicator"});
    let error = create(&spec).unwrap_err();
    assert!(error.contains("asset_id"), "{error}");
}

#[test]
fn issue61_schema_text_limits_match_what_the_validators_enforce() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    let defs = &catalog["schema"]["$defs"];
    let variant = |kind: &str| defs["PartData"]["oneOf"].as_array().unwrap().iter()
        .find(|variant| variant["properties"]["kind"]["const"] == kind).unwrap_or_else(|| panic!("missing {kind}"));
    let example_of = |preset: &str| catalog["presets"].as_array().unwrap().iter().find(|entry| entry["id"] == preset).unwrap_or_else(|| panic!("missing {preset}"))["example"].clone();
    // The schema object that owns the limits, a preset, and the JSON pointer of one such object in that preset's example.
    let probes = [
        (&defs["IconCard"], "list-horizontal/icon-cards", "/data/cards/0"),
        (&defs["PartMessage"], "list-horizontal/icon-cards", "/data/message"),
        (&defs["IconRow"], "list/icon-rows", "/data/rows/0"),
        (&defs["ShiftRow"], "before-after/shift", "/data/rows/0"),
        (variant("shift_rows"), "before-after/shift", "/data"),
        (&defs["StepCard"], "flow/cards", "/data/steps/0"),
        (variant("step_cards"), "flow/cards", "/data"),
        (&defs["AgendaItem"], "list/agenda", "/data/items/0"),
        (&defs["Callout"], "list-enumeration/screenshot-callouts", "/data/callouts/0"),
        (&defs["EditorialStep"], "flow/open-steps", "/data/steps/0"),
        (&defs["RoadmapPhase"], "flow/roadmap", "/data/phases/0"),
        (&defs["IconColumn"], "list-horizontal/icon-columns", "/data/items/0"),
        (&defs["FactColumn"], "list-horizontal/fact-columns", "/data/items/0"),
        (&defs["ImageColumn"], "list-horizontal/image-columns", "/data/items/0"),
        (&defs["ComparisonPanel"], "contrast/panels", "/data/panels/0"),
        (&defs["ComparisonPanelItem"], "contrast/panels", "/data/panels/0/items/0"),
    ];
    // JSON Schema and the validators both count Unicode scalar values, whatever their UTF-8 width.
    for unit in ["x", "あ", "😀"] {
        for (owner, preset, pointer) in probes {
            for (property, schema) in owner["properties"].as_object().unwrap() {
                let Some(maximum) = schema["maxLength"].as_u64() else { continue };
                let probe = |length: u64| {
                    let mut spec = example_of(preset);
                    if spec.pointer(pointer).is_none() {
                        // Optional objects such as `message` are absent from the example; seed the smallest valid one.
                        let (parent, key) = pointer.rsplit_once('/').unwrap();
                        spec.pointer_mut(parent).unwrap()[key] = json!({"text":"Message"});
                    }
                    spec.pointer_mut(pointer).unwrap()[property] = json!(unit.repeat(length as usize));
                    create(&spec)
                };
                let over = probe(maximum + 1).unwrap_err();
                assert!(over.contains(&format!("exceeds {maximum} ")), "{preset} {pointer}.{property} advertises {maximum} for {unit}: {over}");
                // Fitting the frame is a separate rule, so only the length rule is asserted at the advertised limit.
                if let Err(error) = probe(maximum) { assert!(!error.contains("exceeds"), "{preset} {pointer}.{property} at {maximum} for {unit}: {error}"); }
            }
        }
    }
}
