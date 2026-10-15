use aislide_core::{execute_request, report::{compile_report, compile_report_with, sample_report, CompileOptions, ReportInput}};
use serde_json::{json, Value};

const PRESETS: [&str; 7] = ["public", "minimal", "stylish", "pop", "dynamic", "trust", "luxury"];

fn japanese_report() -> ReportInput {
    serde_json::from_value(json!({
        "title": "\u{9867}\u{5ba2}\u{4f53}\u{9a13}\u{6539}\u{5584}\u{30d7}\u{30ed}\u{30b0}\u{30e9}\u{30e0} \u{4e2d}\u{9593}\u{5831}\u{544a}",
        "subtitle": "\u{5408}\u{6210}\u{30c7}\u{30fc}\u{30bf}\u{306b}\u{3088}\u{308b}\u{8a2d}\u{8a08}\u{691c}\u{8a3c}\u{7528}\u{306e}\u{30b5}\u{30f3}\u{30d7}\u{30eb}\u{8cc7}\u{6599}\u{3067}\u{3059}\u{3002}",
        "period": "2026\u{5e74} \u{7b2c}3\u{56db}\u{534a}\u{671f} / \u{30c7}\u{30e2}",
        "source": "AISlide \u{306e}\u{5408}\u{6210}\u{30c7}\u{30fc}\u{30bf}\u{3002}\u{5b9f}\u{5728}\u{306e}\u{4e8b}\u{5b9f}\u{3067}\u{306f}\u{3042}\u{308a}\u{307e}\u{305b}\u{3093}\u{3002}",
        "sections": [
            {"title": "\u{9867}\u{5ba2}\u{4f53}\u{9a13}\u{6539}\u{5584}\n\u{4e2d}\u{9593}\u{5831}\u{544a}", "layout": "cover", "body": ["\u{5408}\u{6210}\u{30c7}\u{30fc}\u{30bf} / \u{30c7}\u{30b6}\u{30a4}\u{30f3}\u{691c}\u{8a3c}"]},
            {"title": "\u{6539}\u{5584}\u{306e}\u{4e09}\u{3064}\u{306e}\u{67f1}", "layout": "columns", "body": [
                "\u{901f}\u{3055}\n\u{554f}\u{3044}\u{5408}\u{308f}\u{305b}\u{3078}\u{306e}\u{521d}\u{56de}\u{56de}\u{7b54}\u{3092}\u{65e9}\u{3081}\u{308b}\u{3002}",
                "\u{6b63}\u{78ba}\u{3055}\n\u{56de}\u{7b54}\u{5185}\u{5bb9}\u{3092}\u{4e00}\u{5ea6}\u{3067}\u{6b63}\u{3057}\u{304f}\u{4f1d}\u{3048}\u{308b}\u{3002}",
                "\u{7d99}\u{7d9a}\n\u{6539}\u{5584}\u{306e}\u{52b9}\u{679c}\u{3092}\u{6bce}\u{6708}\u{78ba}\u{8a8d}\u{3059}\u{308b}\u{3002}"]},
            {"title": "\u{4e3b}\u{8981}\u{6307}\u{6a19}\u{ff08}\u{5408}\u{6210}\u{5024}\u{ff09}", "layout": "metrics",
                "metrics": [{"label": "\u{521d}\u{56de}\u{56de}\u{7b54}\u{6642}\u{9593}", "value": "4.2\u{6642}\u{9593}"}, {"label": "\u{89e3}\u{6c7a}\u{7387}", "value": "87%"}, {"label": "\u{6e80}\u{8db3}\u{5ea6}", "value": "4.1/5"}],
                "body": ["\u{6570}\u{5024}\u{306f}\u{3059}\u{3079}\u{3066}\u{8aac}\u{660e}\u{7528}\u{306e}\u{5408}\u{6210}\u{5024}\u{3067}\u{3059}\u{3002}"]},
            {"title": "\u{30c1}\u{30e3}\u{30cd}\u{30eb}\u{5225}\u{306e}\u{72b6}\u{6cc1}", "layout": "table", "body": ["\u{4ef6}\u{6570}\u{306f}\u{5408}\u{6210}\u{30c7}\u{30fc}\u{30bf}\u{3067}\u{3059}\u{3002}"],
                "rows": [["\u{30c1}\u{30e3}\u{30cd}\u{30eb}", "\u{4ef6}\u{6570}", "\u{5bfe}\u{5fdc}\u{65b9}\u{91dd}"], ["\u{96fb}\u{8a71}", "120", "\u{6298}\u{308a}\u{8fd4}\u{3057}\u{3092}\u{77ed}\u{7e2e}"], ["\u{30e1}\u{30fc}\u{30eb}", "340", "\u{30c6}\u{30f3}\u{30d7}\u{30ec}\u{30fc}\u{30c8}\u{6574}\u{5099}"], ["\u{30c1}\u{30e3}\u{30c3}\u{30c8}", "510", "\u{81ea}\u{52d5}\u{5fdc}\u{7b54}\u{3092}\u{62e1}\u{5145}"]]},
            {"title": "\u{56db}\u{534a}\u{671f}\u{3054}\u{3068}\u{306e}\u{554f}\u{3044}\u{5408}\u{308f}\u{305b}\u{4ef6}\u{6570}", "layout": "chart", "body": ["\u{5408}\u{6210}\u{5024}\u{3067}\u{306f}\u{7b2c}2\u{56db}\u{534a}\u{671f}\u{306b}\u{5897}\u{52a0}\u{3057}\u{3066}\u{3044}\u{307e}\u{3059}\u{3002}"],
                "chart": {"kind": "column", "categories": ["Q1", "Q2", "Q3", "Q4"], "series": [{"name": "\u{4ef6}\u{6570}\u{ff08}\u{5408}\u{6210}\u{ff09}", "values": [820, 910, 870, 960], "color": "087F73"}]}},
            {"title": "\u{6539}\u{5584}\u{306e}\u{9032}\u{3081}\u{65b9}", "layout": "process", "body": ["\u{73fe}\u{72b6}\u{3092}\u{6e2c}\u{308b}", "\u{539f}\u{56e0}\u{3092}\u{7279}\u{5b9a}\u{3059}\u{308b}", "\u{5bfe}\u{7b56}\u{3092}\u{8a66}\u{3059}", "\u{52b9}\u{679c}\u{3092}\u{78ba}\u{8a8d}\u{3059}\u{308b}"]},
            {"title": "\u{7d50}\u{8ad6}", "layout": "statement", "body": ["\u{521d}\u{56de}\u{56de}\u{7b54}\u{306e}\u{901f}\u{3055}\u{304c}\u{6e80}\u{8db3}\u{5ea6}\u{3092}\u{5de6}\u{53f3}\u{3059}\u{308b}\u{3002}\n\u{6b21}\u{56db}\u{534a}\u{671f}\u{306f}\u{5fdc}\u{7b54}\u{6642}\u{9593}\u{306e}\u{77ed}\u{7e2e}\u{306b}\u{96c6}\u{4e2d}\u{3059}\u{308b}\u{3002}"]},
            {"title": "\u{4e8c}\u{3064}\u{306e}\u{9078}\u{629e}\u{80a2}", "layout": "columns", "body": ["A\u{6848}\n\u{65e2}\u{5b58}\u{4f53}\u{5236}\u{3067}\u{624b}\u{9806}\u{3092}\u{898b}\u{76f4}\u{3059}\u{3002}", "B\u{6848}\n\u{5c02}\u{4efb}\u{30c1}\u{30fc}\u{30e0}\u{3092}\u{65b0}\u{8a2d}\u{3059}\u{308b}\u{3002}"]},
            {"title": "\u{6700}\u{91cd}\u{8981}\u{6307}\u{6a19}", "layout": "metrics", "metrics": [{"label": "\u{4e00}\u{6b21}\u{89e3}\u{6c7a}\u{7387}", "value": "87%"}],
                "body": ["\u{4e00}\u{5ea6}\u{306e}\u{5bfe}\u{5fdc}\u{3067}\u{89e3}\u{6c7a}\u{3057}\u{305f}\u{5272}\u{5408}\u{3002}", "\u{51fa}\u{5178}\u{ff1a}\u{5408}\u{6210}\u{30c7}\u{30fc}\u{30bf}\u{3002}"]},
            {"title": "\u{6b21}\u{306e}\u{56db}\u{3064}\u{306e}\u{884c}\u{52d5}", "layout": "columns", "body": [
                "\u{6e2c}\u{5b9a}\n\u{57fa}\u{6e96}\u{5024}\u{3092}\u{78ba}\u{5b9a}\u{3059}\u{308b}\u{3002}", "\u{8a66}\u{884c}\n\u{5c0f}\u{898f}\u{6a21}\u{306b}\u{8a66}\u{3059}\u{3002}",
                "\u{8a55}\u{4fa1}\n\u{7d50}\u{679c}\u{3092}\u{6bd4}\u{8f03}\u{3059}\u{308b}\u{3002}", "\u{5c55}\u{958b}\n\u{52b9}\u{679c}\u{306e}\u{3042}\u{308b}\u{65bd}\u{7b56}\u{3092}\u{5e83}\u{3052}\u{308b}\u{3002}"]}
        ]
    })).unwrap()
}

fn elements(deck: &Value) -> impl Iterator<Item = (&Value, &Value)> {
    deck["slides"].as_array().unwrap().iter().flat_map(|slide| slide["elements"].as_array().unwrap().iter().map(move |element| (slide, element)))
}

fn run_languages(deck: &Value) -> Vec<String> {
    elements(deck).flat_map(|(_, element)| element["format"]["paragraphs"].as_array().into_iter().flatten())
        .flat_map(|paragraph| paragraph["runs"].as_array().unwrap().iter()).filter_map(|run| run["style"]["language"].as_str().map(String::from)).collect()
}

fn compiled(report: &ReportInput, preset: &str) -> Value {
    let compiled = compile_report_with(report, &CompileOptions { design_preset: Some(preset.into()) }).unwrap_or_else(|error| panic!("{preset}: {error}"));
    serde_json::to_value(&compiled).unwrap()
}

fn assert_designed(deck: &Value, preset: &str, slides: usize) {
    assert_eq!(deck["slides"].as_array().unwrap().len(), slides);
    let tokens = execute_request(json!({"op":"design_tokens","deck":deck})).unwrap();
    assert_eq!(tokens["source"], format!("preset:{preset}"), "compiled decks keep the unmodified preset design");
    for slide in deck["slides"].as_array().unwrap() {
        assert!(matches!(slide["layout_id"].as_str(), Some("preset-blank" | "preset-cover")), "{preset} {}", slide["id"]);
        assert!(slide.get("hide_master_graphics").is_none(), "{preset} keeps master decorations");
    }
    let report = execute_request(json!({"op":"measure_layout","deck":deck})).unwrap();
    let overflow: Vec<&Value> = report["issues"].as_array().unwrap().iter().filter(|issue| issue["code"] == "TEXT_OVERFLOW" && !issue["message"].as_str().unwrap().starts_with("layout:") && !issue["message"].as_str().unwrap().starts_with("master:")).collect();
    assert!(overflow.is_empty(), "{preset} overflow {overflow:?}");
    let bytes = execute_request(json!({"op":"export","deck":deck})).unwrap();
    assert!(bytes["base64"].as_str().is_some_and(|value| !value.is_empty()));
}

#[test]
fn every_preset_compiles_the_english_sample_into_pattern_slides() {
    for preset in PRESETS {
        let mut report = sample_report();
        if preset == "dynamic" {
            let error = compile_report_with(&report, &CompileOptions { design_preset: Some(preset.into()) }).unwrap_err().to_string();
            assert!(error.contains("report.sections[0].title does not fit the dynamic cover title"), "an 84px cover title is never shrunk: {error}");
            report.sections[0].title = "Quarterly\nperformance".into();
        }
        let deck = compiled(&report, preset)["deck"].clone();
        assert_designed(&deck, preset, 12);
        let languages = run_languages(&deck);
        assert!(languages.iter().filter(|language| *language == "en-US").count() > 40, "{preset} English runs are tagged en-US");
        let cover = &deck["slides"][0];
        assert_eq!(cover["layout_id"], "preset-cover");
        let placeholders: Vec<&str> = cover["elements"].as_array().unwrap().iter().filter_map(|element| element["format"]["placeholder"]["kind"].as_str()).collect();
        assert_eq!(placeholders, ["title", "subtitle"], "{preset} cover fills the preset cover placeholders");
        let columns = &deck["slides"][1];
        assert!(columns["elements"].as_array().unwrap().iter().any(|element| element["id"] == "s2-column-3"), "{preset} three columns use columns/3");
        let metrics = &deck["slides"][2];
        assert!(metrics["elements"].as_array().unwrap().iter().any(|element| element["id"] == "s3-column-1-value"));
        assert!(metrics["elements"].as_array().unwrap().iter().any(|element| element["id"] == "s3-message"), "{preset} metric commentary sits in the message band");
        let table = &deck["slides"][3];
        assert!(table["elements"].as_array().unwrap().iter().any(|element| element["id"] == "s4-table" && element["type"] == "table"));
    }
}

#[test]
fn every_preset_compiles_a_japanese_report_with_every_layout() {
    let report = japanese_report();
    for preset in PRESETS {
        let output = compiled(&report, preset);
        let deck = &output["deck"];
        assert_designed(deck, preset, 10);
        let languages = run_languages(deck);
        assert!(languages.iter().filter(|language| *language == "ja-JP").count() > 30, "{preset} Japanese runs are tagged ja-JP");
        let chart = elements(deck).find(|(_, element)| element["type"] == "chart").unwrap().1;
        assert_eq!(chart["series"][0]["color"], "@accent1", "{preset} charts follow theme accents");
        let process = &deck["slides"][5];
        let ids: Vec<&str> = process["elements"].as_array().unwrap().iter().map(|element| element["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"s6-step-4"), "{preset} process keeps four steps");
        assert!(ids.contains(&"s6-connector-1") || ids.contains(&"s6-marker-1"), "{preset} process draws arrows or numbers");
        assert!(output["issues"].as_array().unwrap().iter().any(|issue| issue["code"] == "FONT_PARITY_UNVERIFIED"));
    }
}

#[test]
fn legacy_compilation_is_unchanged_without_a_preset_and_preset_errors_are_actionable() {
    let report = sample_report();
    let legacy = serde_json::to_value(compile_report(&report).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(compile_report_with(&report, &CompileOptions::default()).unwrap()).unwrap(), legacy);
    assert_eq!(execute_request(json!({"op":"compile","report":report})).unwrap(), legacy);
    assert!(legacy["deck"].get("design").is_none());
    let designed = execute_request(json!({"op":"compile","report":report,"options":{"design_preset":"trust"}})).unwrap();
    assert!(designed["deck"]["design"].is_object());
    assert!(execute_request(json!({"op":"compile","report":report,"options":{"design_preset":"unknown"}})).unwrap_err().to_string().contains("design preset unknown not found"));
    assert!(execute_request(json!({"op":"compile","report":report,"options":{"preset":"trust"}})).is_err(), "unknown option fields are rejected");

    let mut single = japanese_report();
    single.sections[8].body.clear();
    let error = compile_report_with(&single, &CompileOptions { design_preset: Some("trust".into()) }).unwrap_err().to_string();
    assert!(error.contains("report.sections[8].body") && error.contains("single metric"), "{error}");
    assert!(compile_report(&single).is_ok(), "the legacy layout still accepts it");

    let mut long = japanese_report();
    long.sections[6].body = vec!["\u{9577}\u{3044}\u{6587}\u{7ae0}\u{3002}".repeat(48); 4];
    let error = compile_report_with(&long, &CompileOptions { design_preset: Some("luxury".into()) }).unwrap_err().to_string();
    assert!(error.contains("report.sections[6] does not fit the luxury design") && error.contains("text/reading"), "{error}");
}
