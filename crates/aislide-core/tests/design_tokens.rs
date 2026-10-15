use aislide_core::{design_tokens, editing::{create_with_setup, PresentationSetup}, execute_request};
use serde_json::json;

const PRESETS: [&str; 7] = ["public", "minimal", "stylish", "pop", "dynamic", "trust", "luxury"];

fn preset_deck(preset: &str) -> aislide_core::model::Deck {
    create_with_setup("tokens".into(), "Synthetic tokens".into(), &PresentationSetup { design_preset: Some(preset.into()), ..Default::default() }).unwrap().deck
}

fn inside(frame: aislide_core::layout_patterns::Frame, width: f64, height: f64) -> bool {
    frame.x >= 0.0 && frame.y >= 0.0 && frame.width > 0.0 && frame.height > 0.0 && frame.x + frame.width <= width && frame.y + frame.height <= height
}

#[test]
fn every_preset_resolves_its_own_frames_spacing_and_type_scale() {
    let catalog = execute_request(json!({"op":"design_presets"})).unwrap();
    for preset in PRESETS {
        let tokens = design_tokens::resolve(&preset_deck(preset)).unwrap();
        let rules = &catalog.as_array().unwrap().iter().find(|entry| entry["id"] == preset).unwrap()["rules"];
        assert_eq!(tokens.source, format!("preset:{preset}"));
        assert_eq!(tokens.margin, rules["margin"].as_f64().unwrap(), "{preset} margin");
        assert_eq!(tokens.type_scale.title, rules["heading_size"].as_f64().unwrap(), "{preset} heading");
        assert_eq!(tokens.type_scale.body, rules["body_size"].as_f64().unwrap(), "{preset} body");
        assert_eq!(tokens.layout_id.as_deref(), Some("preset-blank"));
        assert!(tokens.master_graphics);
        let frames = tokens.frames;
        for frame in [frames.title, frames.body, frames.footer, frames.full_page] { assert!(inside(frame, 1280.0, 720.0), "{preset} frame {frame:?}"); }
        assert_eq!(frames.body.x, tokens.margin);
        assert_eq!(frames.body.y, rules["regions"][0]["y"].as_f64().unwrap(), "{preset} body top follows the preset regions");
        assert!(frames.title.y + frames.title.height <= frames.body.y, "{preset} title above body");
        assert!(frames.body.y + frames.body.height <= frames.footer.y, "{preset} footer below body");
        let scale = tokens.type_scale;
        assert!(scale.metric > scale.title && scale.title > scale.statement && scale.statement > scale.body && scale.body >= scale.caption && scale.caption >= scale.minimum, "{preset} scale {scale:?}");
        assert_eq!(scale.minimum, 16.0);
        assert_eq!(scale.line_spacing, 115);
        let gaps = tokens.gaps;
        assert!(gaps.tight <= gaps.peer && gaps.peer <= gaps.contrast && gaps.support <= gaps.contrast, "{preset} gaps {gaps:?}");
        for gap in [gaps.tight, gaps.peer, gaps.support, gaps.contrast] { assert_eq!(gap % 8.0, 0.0, "{preset} gaps stay on the 8px grid"); }
        let scripts = &tokens.scripts;
        assert_eq!((scripts.latin.language, scripts.east_asian.language), ("en-US", "ja-JP"));
        assert!(scripts.latin.title_line > scripts.east_asian.title_line, "{preset} Latin titles fit more characters per line");
        assert_eq!(scripts.latin.title_lines, scripts.east_asian.title_lines);
        assert!(scripts.east_asian.title_lines >= 1);
    }
    let dynamic = design_tokens::resolve(&preset_deck("dynamic")).unwrap();
    assert_eq!(dynamic.scripts.east_asian.title_lines, 1, "60px headings leave one title line");
    let minimal = design_tokens::resolve(&preset_deck("minimal")).unwrap();
    let pop = design_tokens::resolve(&preset_deck("pop")).unwrap();
    assert!(minimal.gaps.contrast > pop.gaps.contrast, "spacious presets separate regions further");
    assert_eq!(pop.card.rule, 8.0);
}

#[test]
fn other_designs_and_edited_presets_use_neutral_defaults() {
    let plain = aislide_core::editing::create("plain".into(), "Synthetic".into()).unwrap().deck;
    let tokens = design_tokens::resolve(&plain).unwrap();
    assert_eq!(tokens.source, "default");
    assert_eq!(tokens.layout_id.as_deref(), Some("blank"));
    assert!(!tokens.master_graphics);
    assert_eq!([tokens.frames.body.x, tokens.frames.body.y, tokens.frames.body.width, tokens.frames.body.height], [48.0, 120.0, 1184.0, 540.0], "matches the layout-pattern default body");
    assert_eq!(tokens.type_scale.body, 20.0);
    let mut edited = preset_deck("trust");
    let design = edited.design.as_mut().unwrap();
    let master = design.masters.iter_mut().find(|master| master.id == "preset-master").unwrap();
    master.background = "@lt2".into();
    assert_eq!(design_tokens::resolve(&edited).unwrap().source, "default", "an edited preset master is no longer the preset");
    let mut wide = plain.clone();
    wide.width = 1920; wide.height = 1080;
    let scaled = design_tokens::resolve(&wide).unwrap();
    assert_eq!(scaled.type_scale.body, 30.0, "default sizes scale with the canvas height");
    assert!(inside(scaled.frames.body, 1920.0, 1080.0));
}

#[test]
fn design_tokens_operation_reports_json_tokens() {
    let deck = preset_deck("luxury");
    let tokens = execute_request(json!({"op":"design_tokens","deck":deck})).unwrap();
    assert_eq!(tokens["version"], 1);
    assert_eq!(tokens["source"], "preset:luxury");
    assert_eq!(tokens["frames"]["body"]["x"], 104.0);
    assert_eq!(tokens["colors"]["accent"], "@accent1");
    assert_eq!(tokens["scripts"]["east_asian"]["language"], "ja-JP");
    assert!(execute_request(json!({"op":"design_tokens","deck":{"version":1}})).is_err());
}
