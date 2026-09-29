use aislide_core::execute_request;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

const BRIEFING: [&str; 5] = ["list-horizontal/icon-cards", "list/icon-rows", "before-after/shift", "flow/cards", "list/agenda"];
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
