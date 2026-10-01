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
