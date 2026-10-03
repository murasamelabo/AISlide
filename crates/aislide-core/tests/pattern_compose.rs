use aislide_core::{editing::{create_with_setup, PresentationSetup}, execute_request};
use serde_json::{json, Value};

fn document(preset: Option<&str>) -> Value {
    let setup = PresentationSetup { design_preset: preset.map(String::from), ..Default::default() };
    serde_json::to_value(create_with_setup("pattern".into(), "Synthetic patterns".into(), &setup).unwrap()).unwrap()
}

fn compose(document: &Value, spec: Value) -> Result<Value, String> {
    execute_request(json!({"op":"apply_operations","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],
        "operations":[{"op":"compose_slide","slide_id":"slide-1","id":"c","spec":spec}]})).map_err(|error| error.to_string())
}

fn tokens(document: &Value) -> Value { execute_request(json!({"op":"design_tokens","deck":document["deck"]})).unwrap() }

fn slide(result: &Value) -> &Value { &result["document"]["deck"]["slides"][0] }

fn element<'a>(slide: &'a Value, id: &str) -> &'a Value {
    slide["elements"].as_array().unwrap().iter().find(|element| element["id"] == id).unwrap_or_else(|| panic!("missing element {id} in {}", slide["elements"]))
}

fn frame(element: &Value) -> [f64; 4] { ["x", "y", "width", "height"].map(|key| element[key].as_f64().unwrap()) }

fn within(inner: [f64; 4], outer: [f64; 4]) -> bool {
    inner[0] >= outer[0] - 0.01 && inner[1] >= outer[1] - 0.01 && inner[0] + inner[2] <= outer[0] + outer[2] + 0.01 && inner[1] + inner[3] <= outer[1] + outer[3] + 0.01
}

fn languages(element: &Value) -> Vec<String> {
    element["format"]["paragraphs"].as_array().unwrap().iter().flat_map(|paragraph| paragraph["runs"].as_array().unwrap().iter()).filter(|run| !run["text"].as_str().unwrap().is_empty())
        .map(|run| run["style"]["language"].as_str().unwrap().to_owned()).collect()
}

fn token_frame(tokens: &Value, name: &str) -> [f64; 4] { ["x", "y", "width", "height"].map(|key| tokens["frames"][name][key].as_f64().unwrap()) }

fn no_preflight_errors(result: &Value) {
    let report = execute_request(json!({"op":"preflight_presentation","document":result["document"]})).unwrap();
    let errors: Vec<&Value> = report["findings"].as_array().unwrap().iter().filter(|finding| finding["severity"] == "error").collect();
    assert!(errors.is_empty(), "preflight errors: {errors:?}");
}

fn assert_round_trip(result: &Value) {
    let undone = execute_request(json!({"op":"undo_transaction","document":result["document"],"expected_revision":result["document"]["revision"],"receipt":result["receipt"]})).unwrap();
    assert!(undone["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().is_empty());
    let exported = execute_request(json!({"op":"export_presentation","document":result["document"]})).unwrap();
    execute_request(json!({"op":"open_presentation","id":"pattern-reopened","base64":exported["base64"]})).unwrap();
}

fn png(width: u32, height: u32) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let image = image::RgbImage::from_fn(width, height, |x, y| image::Rgb([(x * 255 / width) as u8, (y * 255 / height) as u8, 160]));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgb8(image).write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    STANDARD.encode(bytes)
}

fn card(label: &str, detail: &str) -> Value { json!({"kind":"cards","items":[{"label":label,"detail":detail}]}) }

#[test]
fn columns_keep_master_decorations_token_sizes_and_language_tags_in_english_and_japanese() {
    for (language, title, items) in [
        ("en-US", "Three pillars of the synthetic plan", [("Clarity", "One message per slide."), ("Evidence", "Every number names its source."), ("Action", "Close with a decision and owner.")]),
        ("ja-JP", "\u{8a08}\u{753b}\u{3092}\u{652f}\u{3048}\u{308b}\u{4e09}\u{3064}\u{306e}\u{67f1}", [("\u{660e}\u{78ba}\u{3055}", "1\u{679a}\u{306b}1\u{30e1}\u{30c3}\u{30bb}\u{30fc}\u{30b8}\u{3002}"), ("\u{6839}\u{62e0}", "\u{6570}\u{5024}\u{306b}\u{306f}\u{51fa}\u{5178}\u{3092}\u{793a}\u{3059}\u{3002}"), ("\u{884c}\u{52d5}", "\u{6c7a}\u{5b9a}\u{3068}\u{62c5}\u{5f53}\u{3067}\u{7d50}\u{3076}\u{3002}")]),
    ] {
        let before = document(Some("trust"));
        let tokens = tokens(&before);
        let spec = json!({"title":title,"footer":"Synthetic example","pattern":{"id":"columns/3"},"slots":{
            "column-1":card(items[0].0, items[0].1),"column-2":card(items[1].0, items[1].1),"column-3":card(items[2].0, items[2].1)}});
        let result = compose(&before, spec).unwrap();
        let composed = slide(&result);
        assert_eq!(composed["layout_id"], "preset-blank");
        assert!(composed.get("hide_master_graphics").is_none_or(|value| value == false), "preset decorations stay visible");
        assert_eq!(composed["title"], title);
        let heading = element(composed, "c-title");
        assert_eq!(heading["font_size"], tokens["type_scale"]["title"]);
        assert_eq!(frame(heading), token_frame(&tokens, "title"));
        let body = token_frame(&tokens, "body");
        for index in 1..=3 {
            let column = element(composed, &format!("c-column-{index}"));
            assert_eq!(column["type"], "shape");
            assert_eq!(column["font_size"], tokens["type_scale"]["body"]);
            assert_eq!(column["fill"], "@lt2");
            assert!(within(frame(column), body), "column {index} stays in the token body");
            assert_eq!(column["format"]["padding"]["left"], tokens["card"]["padding"]);
            assert_eq!(languages(column), vec![language; 2], "card runs carry {language}");
            assert_eq!(element(composed, &format!("c-column-{index}-rule"))["fill"], "@accent1");
        }
        let gap = frame(element(composed, "c-column-2"))[0] - (frame(element(composed, "c-column-1"))[0] + frame(element(composed, "c-column-1"))[2]);
        assert_eq!(gap, tokens["gaps"]["support"].as_f64().unwrap(), "columns use the preset support gap");
        assert_eq!(languages(heading), vec![language]);
        assert_eq!(element(composed, "c-footer")["font_size"], tokens["type_scale"]["caption"]);
        no_preflight_errors(&result);
        assert_round_trip(&result);
    }
}

#[test]
fn visual_metric_quote_and_flow_patterns_fill_slots_from_tokens() {
    let chart = json!({"kind":"chart","chart":{"kind":"column","categories":["Q1","Q2","Q3"],"series":[{"name":"\u{5408}\u{6210}\u{30c7}\u{30fc}\u{30bf}","values":[3,5,4],"color":"@accent1"}]}});
    let support = json!({"kind":"text","paragraphs":[{"runs":[{"text":"\u{5408}\u{6210}\u{30c7}\u{30fc}\u{30bf}\u{306e}\u{4f8b}\u{3067}\u{3059}\u{3002}"}]}]});
    let result = compose(&document(Some("minimal")), json!({"title":"\u{56db}\u{534a}\u{671f}\u{3054}\u{3068}\u{306e}\u{63a8}\u{79fb}","pattern":{"id":"split/2-1"},"slots":{"primary":chart,"support":support}})).unwrap();
    let composed = slide(&result);
    assert_eq!(element(composed, "c-primary")["type"], "chart");
    assert_eq!(languages(element(composed, "c-support")), vec!["ja-JP"]);
    assert!(frame(element(composed, "c-primary"))[2] > frame(element(composed, "c-support"))[2] * 1.8, "2:1 split");
    no_preflight_errors(&result);

    let before = document(Some("pop"));
    let tokens = tokens(&before);
    let result = compose(&before, json!({"title":"One synthetic number","pattern":{"id":"focus/kpi-hero"},"slots":{
        "metric":{"kind":"metric","value":"42%","label":"Synthetic adoption"},
        "context":{"kind":"text","paragraphs":[{"runs":[{"text":"Share of sample teams using the template."}]}]},
        "evidence":{"kind":"text","paragraphs":[{"runs":[{"text":"Illustrative value; not a measured result."}]}]}}})).unwrap();
    let composed = slide(&result);
    let value = element(composed, "c-metric-value");
    assert_eq!(value["color"], "@accent1");
    assert!(value["font_size"].as_f64().unwrap() > tokens["type_scale"]["body"].as_f64().unwrap());
    assert!(within(frame(value), frame(element(composed, "c-metric"))));
    assert_eq!(languages(element(composed, "c-metric-label")), vec!["en-US"]);
    no_preflight_errors(&result);

    let result = compose(&document(Some("luxury")), json!({"title":"\u{5f15}\u{7528}","pattern":{"id":"text/quote"},"slots":{
        "quote":{"kind":"quote","text":"\u{826f}\u{3044}\u{8cc7}\u{6599}\u{306f}\u{3001}\u{7d50}\u{8ad6}\u{304b}\u{3089}\u{59cb}\u{307e}\u{308b}\u{3002}","attribution":"Synthetic interview, 2026"}}})).unwrap();
    let composed = slide(&result);
    assert_eq!(element(composed, "c-attribution")["text"], "Synthetic interview, 2026", "attribution fills the pattern's attribution slot");
    assert_eq!(languages(element(composed, "c-quote")), vec!["ja-JP"]);
    assert_eq!(element(composed, "c-quote-rule")["fill"], "@accent1");
    assert!(compose(&document(Some("luxury")), json!({"title":"Quote","pattern":{"id":"text/quote"},"slots":{"quote":{"kind":"quote","text":"Unattributed words."}}})).unwrap_err().contains("attribution"));

    let result = compose(&document(Some("stylish")), json!({"title":"Synthetic flow","pattern":{"id":"sequence/steps-h"},"slots":{
        "step-1":card("Collect", "Record the source."),"step-2":card("Check", "Reconcile totals."),"step-3":card("Share", "State uncertainty.")}})).unwrap();
    let composed = slide(&result);
    for index in 1..=2 {
        let arrow = element(composed, &format!("c-connector-{index}"));
        assert_eq!(arrow["preset"], "rightArrow");
        assert!(frame(arrow)[0] > frame(element(composed, &format!("c-step-{index}")))[0]);
    }
    assert!(composed["elements"].as_array().unwrap().iter().all(|element| element["id"] != "c-connector-3"), "count is inferred from the supplied steps");
    no_preflight_errors(&result);

    let result = compose(&document(Some("trust")), json!({"title":"\u{624b}\u{9806}","pattern":{"id":"sequence/steps-v","count":3},"slots":{
        "step-1":{"kind":"text","paragraphs":[{"runs":[{"text":"\u{53ce}\u{96c6}\u{3059}\u{308b}"}]}]},"step-2":{"kind":"text","paragraphs":[{"runs":[{"text":"\u{78ba}\u{8a8d}\u{3059}\u{308b}"}]}]},"step-3":{"kind":"text","paragraphs":[{"runs":[{"text":"\u{5171}\u{6709}\u{3059}\u{308b}"}]}]}}})).unwrap();
    let composed = slide(&result);
    for index in 1..=3 {
        let badge = element(composed, &format!("c-marker-{index}"));
        assert_eq!((badge["preset"].as_str(), badge["text"].as_str()), (Some("ellipse"), Some(index.to_string().as_str())));
    }
    no_preflight_errors(&result);
}

#[test]
fn tables_images_bands_and_full_page_patterns_compose_natively() {
    let rows = json!([["\u{9805}\u{76ee}","\u{73fe}\u{72b6}","\u{5bfe}\u{5fdc}"],["\u{54c1}\u{8cea}","\u{672a}\u{6e2c}\u{5b9a}","\u{8a08}\u{6e2c}\u{3059}\u{308b}"],["\u{901f}\u{5ea6}","\u{4e0d}\u{660e}","\u{6bd4}\u{8f03}\u{3059}\u{308b}"]]);
    let result = compose(&document(Some("dynamic")), json!({"title":"\u{6bd4}\u{8f03}","pattern":{"id":"compare/table-full"},"slots":{
        "table":{"kind":"table","rows":rows},"decision":{"kind":"callout","text":"\u{307e}\u{305a}\u{54c1}\u{8cea}\u{3092}\u{8a08}\u{6e2c}\u{3059}\u{308b}\u{3002}"}}})).unwrap();
    let composed = slide(&result);
    assert_eq!(element(composed, "c-table")["type"], "table");
    assert_eq!(element(composed, "c-decision-bar")["fill"], "@accent1");
    no_preflight_errors(&result);

    let images = [png(64, 48), png(48, 64), png(64, 64)];
    let slots = json!({
        "image-1":{"kind":"image","base64":images[0],"mime_type":"image/png","alt":"Synthetic landscape gradient"},
        "image-2":{"kind":"image","base64":images[1],"mime_type":"image/png","alt":"Synthetic portrait gradient","fit":"cover"},
        "image-3":{"kind":"image","base64":images[2],"mime_type":"image/png","alt":"Synthetic square gradient"},
        "caption-1":{"kind":"label","text":"Landscape"},"caption-2":{"kind":"label","text":"Portrait, cropped"},"caption-3":{"kind":"label","text":"\u{6b63}\u{65b9}\u{5f62}"}});
    let result = compose(&document(Some("public")), json!({"title":"Synthetic gallery","pattern":{"id":"media/gallery-3"},"slots":slots})).unwrap();
    let composed = slide(&result);
    let contained = frame(element(composed, "c-image-1"));
    assert!((contained[2] / contained[3] - 64.0 / 48.0).abs() < 0.01, "contain keeps the image aspect");
    let covered = element(composed, "c-image-2");
    assert!(covered["crop"]["top"].as_f64().unwrap() > 0.0 && covered["crop"]["bottom"].as_f64().unwrap() > 0.0, "cover crops the long axis");
    assert_eq!(languages(element(composed, "c-caption-3")), vec!["ja-JP"]);
    no_preflight_errors(&result);
    assert!(compose(&document(Some("public")), json!({"title":"Gallery","pattern":{"id":"focus/image-caption"},"slots":{"image":{"kind":"image","base64":images[0],"mime_type":"image/png","alt":" "},"caption":{"kind":"label","text":"Caption"}}})).unwrap_err().contains("alt"));

    let result = compose(&document(Some("trust")), json!({"title":"Synthetic alternatives","pattern":{"id":"split/1-1","message_band":true},"slots":{
        "left":card("Option A", "Lower cost, slower."),"right":card("Option B", "Higher cost, faster."),"message":{"kind":"callout","text":"Choose after measuring the baseline."}}})).unwrap();
    assert!(frame(element(slide(&result), "c-message"))[1] > frame(element(slide(&result), "c-left"))[1]);

    let before = document(Some("trust"));
    let tokens = tokens(&before);
    let result = compose(&before, json!({"title":"\u{7b2c}2\u{7ae0} \u{691c}\u{8a3c}","pattern":{"id":"structure/section"},"slots":{"number":{"kind":"label","text":"02"}}})).unwrap();
    let composed = slide(&result);
    let title = element(composed, "c-title");
    assert_eq!(title["font_size"], tokens["type_scale"]["statement"], "full-page patterns place the title in their own slot");
    assert!(within(frame(title), token_frame(&tokens, "full_page")));
    assert_eq!(composed["elements"].as_array().unwrap().iter().filter(|element| element["id"] == "c-title").count(), 1);

    let plain = document(None);
    let result = compose(&plain, json!({"title":"Neutral design","pattern":{"id":"focus/statement"},"slots":{"statement":{"kind":"statement","text":"Defaults apply without a preset."}}})).unwrap();
    let composed = slide(&result);
    assert_eq!(composed["layout_id"], "blank");
    assert_eq!(composed["hide_master_graphics"], true, "unknown masters stay hidden as in block mode");
    assert_eq!(element(composed, "c-title")["font_size"], 36.0);
    assert_eq!(element(composed, "c-statement")["format"]["alignment"], "center", "centered slots center the statement");
    assert_round_trip(&result);
}

#[test]
fn managed_parts_and_graphs_fill_pattern_slots_with_advised_fit() {
    let graph = json!({"kind":"graph","input":{"version":1,"title":"","show_title":false,"columns":2,"nodes":[{"id":"source","label":"Source"},{"id":"target","label":"Target"}],"edges":[{"id":"flow","source":"source","target":"target"}]}});
    let result = compose(&document(Some("trust")), json!({"title":"Synthetic architecture","pattern":{"id":"focus/full-band"},"slots":{
        "primary":graph,"message":{"kind":"callout","text":"Synthetic topology for layout testing."}}})).unwrap();
    assert!(result["diagnostics"]["findings"].is_array(), "composed diagrams report graph diagnostics");
    let parts = result["document"]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0]["element_id"], "c-primary");
    let layout = &parts[0]["spec"]["layout"];
    let tokens = tokens(&document(Some("trust")));
    assert_eq!(layout["x"], tokens["frames"]["body"]["x"]);
    let result = compose(&document(Some("trust")), json!({"title":"Synthetic steps","pattern":{"id":"focus/full"},"slots":{"primary":{"kind":"steps","items":[{"label":"Check"},{"label":"Act"}]}}})).unwrap();
    assert_eq!(result["document"]["parts"][0]["spec"]["preset"], "list-horizontal/labeled");
    assert_round_trip(&result);
}

#[test]
fn pattern_mode_rejects_ambiguous_or_unfitting_input_atomically() {
    let before = document(Some("trust"));
    let base = |pattern: Value, slots: Value| json!({"title":"Synthetic","pattern":pattern,"slots":slots});
    let three = json!({"column-1":card("A","a"),"column-2":card("B","b"),"column-3":card("C","c")});
    let mut extra = three.clone(); extra["column-4"] = card("D", "d");
    let mut missing = three.clone(); missing.as_object_mut().unwrap().remove("column-3");
    let long = "Synthetic words that keep going without stopping. ".repeat(8);
    for (spec, message) in [
        (base(json!({"id":"columns/3"}), extra), "has no slot column-4"),
        (base(json!({"id":"columns/3"}), missing), "requires slots column-3"),
        (base(json!({"id":"sequence/steps-h"}), json!({"step-1":card("A","a"),"step-2":card("B","b"),"connector-1":card("X","x")})), "marker slot"),
        (base(json!({"id":"focus/full"}), json!({"primary":card("A","a")})), "accepting"),
        (base(json!({"id":"no/such-pattern"}), three.clone()), "unknown layout pattern"),
        (base(json!({"id":"focus/statement"}), json!({"statement":{"kind":"statement","text":long}})), "overflows"),
        (json!({"title":"Synthetic","subtitle":"No","pattern":{"id":"columns/3"},"slots":three}), "no subtitle"),
        (json!({"title":"Synthetic","style":{"body_size":30},"pattern":{"id":"columns/3"},"slots":three}), "omit style"),
        (json!({"title":"Synthetic","pattern":{"id":"columns/3"},"slots":three,"blocks":[{"kind":"callout","text":"x"}]}), "omit blocks"),
        (json!({"title":"Synthetic","slots":three}), "require a pattern"),
        (json!({"title":"Synthetic","blocks":[{"kind":"statement","text":"Legacy blocks stack only"}]}), "require a pattern"),
        (base(json!({"id":"columns/3"}), json!({"column-1":{"kind":"cards","items":[{"label":"A"},{"label":"B"}]},"column-2":card("B","b"),"column-3":card("C","c")})), "holds one card"),
        (json!({"title":"","pattern":{"id":"columns/3"},"slots":three}), "title requires text"),
    ] {
        let error = compose(&before, spec.clone()).expect_err(&format!("expected failure for {spec}"));
        assert!(error.contains(message), "expected {message:?}, got {error}");
    }
    let luxury = document(Some("luxury"));
    let steps: serde_json::Map<String, Value> = (1..=5).map(|index| (format!("step-{index}"), card("Step", "Synthetic"))).collect();
    let error = compose(&luxury, base(json!({"id":"sequence/steps-h"}), Value::Object(steps))).unwrap_err();
    assert!(error.contains("does not fit the design body") && error.contains("sequence/steps-v"), "undersized slots name the fallback: {error}");
    let error = compose(&before, base(json!({"id":"focus/statement"}), json!({"statement":{"kind":"statement","text":long}}))).unwrap_err();
    assert!(error.contains("font sizes are not reduced"), "{error}");
}
