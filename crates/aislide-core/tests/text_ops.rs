use aislide_core::{design::Design, model::{Deck, Element, validate_deck},
    text_ops::{replace, replace_font, search, ReplaceOptions, SearchOptions}, Error};
use serde_json::{json, Value};

fn text(id: &str, value: &str) -> Value {
    json!({"type":"text", "id":id, "x":10, "y":10, "width":400, "height":100,
        "text":value, "font_size":24, "color":"202525", "bold":false})
}

fn deck(elements: Vec<Value>) -> Deck {
    let deck = serde_json::from_value(json!({"version":1, "title":"Metadata", "width":1280, "height":720,
        "slides":[{"id":"slide-1", "title":"Slide metadata", "background":"FFFFFF", "notes":"",
            "elements":elements}]})).unwrap();
    validate_deck(&deck).unwrap();
    deck
}

fn options(query: &str) -> SearchOptions {
    serde_json::from_value(json!({"query":query})).unwrap()
}

fn replace_all(query: &str, replacement: &str) -> ReplaceOptions {
    serde_json::from_value(json!({"search":{"query":query}, "replacement":replacement, "replace_all":true})).unwrap()
}

fn group(id: &str, children: Vec<Value>) -> Value {
    json!({"type":"group", "id":id, "x":0, "y":0, "width":800, "height":400,
        "view_width":800, "view_height":400, "children":children})
}

fn table(value: &str) -> Value {
    json!({"type":"table", "id":"table", "x":0, "y":0, "width":400, "height":100,
        "rows":[[value, "untouched"]], "font_size":20})
}

#[test]
fn japanese_literal_matches_use_scalar_offsets_and_real_json_pointers() {
    let deck = deck(vec![text("text/~id", "\u{1f600}日本語 日本語 .*")]);
    let matches = search(&deck, &options("日本語")).unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!((matches[0].start, matches[0].end), (1, 4));
    assert_eq!((matches[1].start, matches[1].end), (5, 8));
    let serialized = serde_json::to_value(&deck).unwrap();
    for found in &matches {
        assert_eq!(found.path, "/slides/0/elements/0/text");
        assert_eq!(serialized.pointer(&found.path).unwrap().as_str().unwrap(), found.expected);
        assert!(found.snippet.contains("日本語"));
    }
    assert_eq!(search(&deck, &options(".*")).unwrap().len(), 1);
    assert!(search(&deck, &options(".+")).unwrap().is_empty());
    let mut long = deck;
    if let Element::Text { text, .. } = &mut long.slides[0].elements[0] { *text = "日".repeat(4000); }
    let found = search(&long, &options(&"日".repeat(2000))).unwrap();
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|hit| hit.snippet.chars().count() <= 160));
}

#[test]
fn case_and_whole_word_are_unicode_aware_without_partial_case_expansions() {
    let deck = deck(vec![text("body", "Cat cat scatter cat_ 猫 猫語 ÉCOLE école İ i")]);
    let mut query = options("cat");
    assert_eq!(search(&deck, &query).unwrap().len(), 3);
    query.case_sensitive = false;
    assert_eq!(search(&deck, &query).unwrap().len(), 4);
    query.whole_word = true;
    assert_eq!(search(&deck, &query).unwrap().len(), 2);
    query.query = "猫".into();
    assert_eq!(search(&deck, &query).unwrap().len(), 1);
    query.query = "école".into();
    assert_eq!(search(&deck, &query).unwrap().len(), 2);
    query.query = "i".into();
    assert_eq!(search(&deck, &query).unwrap().len(), 1);
    query.query = "i\u{307}".into();
    let found = search(&deck, &query).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].end - found[0].start, 1);
    let mut update = replace_all("i\u{307}", "X");
    update.search.case_sensitive = false;
    let result = serde_json::to_value(replace(deck, &update).unwrap()).unwrap();
    assert!(result["slides"][0]["elements"][0]["text"].as_str().unwrap().ends_with("X i"));
}

#[test]
fn traversal_covers_nested_shapes_tables_and_opt_in_notes_masters_and_layouts() {
    let shape = json!({"type":"shape", "id":"shape", "x":0, "y":0, "width":400, "height":100,
        "preset":"rect", "fill":"FFFFFF", "stroke":"202525", "stroke_width":1,
        "text":"needle", "font_size":24, "color":"202525", "bold":false});
    let mut deck = deck(vec![group("outer", vec![group("inner", vec![shape, table("needle")])])]);
    deck.slides[0].notes = "needle notes".into();
    let mut design = Design::default();
    design.masters[0].elements.push(serde_json::from_value(text("master", "needle")).unwrap());
    design.layouts[0].elements.push(serde_json::from_value(text("layout", "needle")).unwrap());
    deck.design = Some(design);
    let mut query = options("needle");
    assert_eq!(search(&deck, &query).unwrap().len(), 2);
    query.include_notes = true;
    assert_eq!(search(&deck, &query).unwrap().len(), 3);
    query.include_masters = true;
    let found = search(&deck, &query).unwrap();
    assert_eq!(found.len(), 5);
    let before = serde_json::to_value(&deck).unwrap();
    for hit in &found { assert_eq!(before.pointer(&hit.path).unwrap().as_str().unwrap(), hit.expected); }
    assert!(found.iter().any(|hit| hit.path == "/slides/0/elements/0/children/0/children/1/rows/0/0"));
    assert!(found.iter().any(|hit| hit.path == "/slides/0/notes"));
    assert!(found.iter().any(|hit| hit.path == "/design/masters/0/elements/0/text"));
    assert!(found.iter().any(|hit| hit.path == "/design/layouts/0/elements/0/text"));
    let default_result = serde_json::to_value(replace(deck.clone(), &replace_all("needle", "next")).unwrap()).unwrap();
    assert_eq!(default_result["slides"][0]["notes"], "needle notes");
    assert_eq!(default_result["design"], before["design"]);
    let mut update = replace_all("needle", "next");
    update.search = query;
    let result = serde_json::to_value(replace(deck, &update).unwrap()).unwrap();
    for hit in found { assert!(result.pointer(&hit.path).unwrap().as_str().unwrap().starts_with("next")); }
}

#[test]
fn selected_replacement_checks_expected_field_ranges_scope_and_nontext_paths() {
    let mut value = text("needle", "needle needle");
    value["format"] = json!({"font_family":"needle", "hyperlink":"https://example.invalid/needle"});
    let mut deck = deck(vec![value]);
    deck.title = "needle".into();
    deck.slides[0].notes = "needle needle".into();
    let before = serde_json::to_value(&deck).unwrap();
    let found = search(&deck, &options("needle")).unwrap();
    assert_eq!(found.len(), 2);
    let mut update: ReplaceOptions = serde_json::from_value(json!({"search":{"query":"needle"},
        "replacement":"日本", "selected":[found[1]]})).unwrap();
    let result = serde_json::to_value(replace(deck.clone(), &update).unwrap()).unwrap();
    assert_eq!(result["slides"][0]["elements"][0]["text"], "needle 日本");
    assert_eq!(result["slides"][0]["elements"][0]["format"], before["slides"][0]["elements"][0]["format"]);
    assert_eq!(result["title"], before["title"]);
    let mut stale = deck.clone();
    if let Element::Text { text, .. } = &mut stale.slides[0].elements[0] { *text = "needle needle!".into(); }
    assert!(matches!(replace(stale, &update), Err(Error::Conflict(_))));
    for path in ["/title", "/slides/0/elements/0/id", "/slides/0/elements/0/format/font_family",
        "/slides/0/elements/0/format/hyperlink", "/slides/0/notes", "/slides/00/elements/0/text", "/missing"] {
        update.selected.as_mut().unwrap()[0].path = path.into();
        assert!(matches!(replace(deck.clone(), &update), Err(Error::Conflict(_))), "{path}");
    }
    update.selected = Some(vec![found[0].clone()]);
    update.selected.as_mut().unwrap()[0].end = usize::MAX;
    assert!(replace(deck.clone(), &update).is_err());
    update.selected = Some(vec![found[0].clone(), found[0].clone()]);
    assert!(matches!(replace(deck.clone(), &update), Err(Error::Conflict(_))));
    update.selected = Some(found.into_iter().rev().collect());
    let result = serde_json::to_value(replace(deck, &update).unwrap()).unwrap();
    assert_eq!(result["slides"][0]["elements"][0]["text"], "日本 日本");
}

#[test]
fn replace_all_is_nonrecursive_noops_are_exact_and_match_limits_never_truncate() {
    let deck = deck(vec![text("body", "aaaa")]);
    let result = serde_json::to_value(replace(deck.clone(), &replace_all("aa", "aaa")).unwrap()).unwrap();
    assert_eq!(result["slides"][0]["elements"][0]["text"], "aaaaaa");
    for update in [replace_all("aa", "aa"), replace_all("absent", "replacement")] {
        assert_eq!(serde_json::to_value(replace(deck.clone(), &update).unwrap()).unwrap(), serde_json::to_value(&deck).unwrap());
    }
    let mut update = replace_all("aa", "");
    assert_eq!(serde_json::to_value(replace(deck.clone(), &update).unwrap()).unwrap()["slides"][0]["elements"][0]["text"], "");
    update.search.max_matches = 1;
    assert!(matches!(search(&deck, &update.search), Err(Error::Limit(_))));
    assert!(matches!(replace(deck.clone(), &update), Err(Error::Limit(_))));
    update.search.max_matches = 2;
    assert_eq!(search(&deck, &update.search).unwrap().len(), 2);
    for limit in [0, usize::MAX] {
        update.search.max_matches = limit;
        assert!(search(&deck, &update.search).is_err());
    }
    assert!(search(&deck, &options("")).is_err());
    assert!(search(&deck, &options("\u{0}")).is_err());
    assert!(search(&deck, &options(&"x".repeat(8001))).is_err());
    assert!(serde_json::from_value::<SearchOptions>(json!({"query":"a", "regex":true})).is_err());
    update = replace_all("a", "x");
    update.replace_all = false;
    assert!(replace(deck.clone(), &update).is_err());
    update.selected = Some(Vec::new());
    assert_eq!(serde_json::to_value(replace(deck.clone(), &update).unwrap()).unwrap(), serde_json::to_value(&deck).unwrap());
    update.replace_all = true;
    assert!(replace(deck, &update).is_err());
}

#[test]
fn replacement_preserves_per_field_limits_and_numeric_validation() {
    for (elements, accepted, rejected) in [(vec![text("body", "x")], 4000, 4001), (vec![table("x")], 200, 201)] {
        let deck = deck(elements);
        assert!(replace(deck.clone(), &replace_all("x", &"日".repeat(accepted))).is_ok());
        assert!(matches!(replace(deck.clone(), &replace_all("x", &"日".repeat(rejected))), Err(Error::Limit(_))));
        assert!(matches!(replace(deck, &replace_all("x", "\u{0}")), Err(Error::Invalid(_))));
    }
    let mut deck = deck(vec![text("body", &format!("{}x", "a".repeat(3999)))]);
    let mut update = replace_all("x", "xx");
    let error = replace(deck.clone(), &update).unwrap_err();
    assert!(matches!(&error, Error::Limit(_)));
    assert!(error.to_string().contains("/slides/0/elements/0/text exceeds 4000"));
    deck.slides[0].notes = "needle".into();
    update = replace_all("needle", &"日".repeat(8000));
    update.search.include_notes = true;
    assert!(replace(deck.clone(), &update).is_ok());
    update.replacement.push('日');
    assert!(matches!(replace(deck.clone(), &update), Err(Error::Limit(_))));
    if let Element::Text { font_size, .. } = &mut deck.slides[0].elements[0] { *font_size = f64::NAN; }
    assert!(search(&deck, &options("needle")).is_err());
    assert!(replace_font(deck, "Arial", "Aptos").is_err());
}

#[test]
fn font_replacement_updates_explicit_and_all_theme_families_preserving_tokens_and_inheritance() {
    let mut explicit = text("explicit", "Arial content");
    explicit["format"] = json!({"font_family":"Arial"});
    let mut linked = text("linked", "Keep token");
    linked["format"] = json!({"font_family":"@major"});
    let shape = json!({"type":"shape", "id":"shape", "x":0, "y":0, "width":400, "height":100,
        "preset":"rect", "fill":"FFFFFF", "stroke":"202525", "stroke_width":1,
        "text":"Arial shape", "font_size":24, "color":"202525", "bold":false, "format":{"font_family":"Arial"}});
    let mut deck = deck(vec![group("group", vec![explicit.clone(), linked, text("implicit", "Default"), shape])]);
    let mut design = Design::default();
    design.theme.fonts.major = "Arial".into();
    design.theme.fonts.minor = "Arial".into();
    design.theme.fonts.east_asian = "Arial".into();
    design.theme.fonts.complex_script = "Arial".into();
    design.masters[0].elements.push(serde_json::from_value(explicit.clone()).unwrap());
    design.layouts[0].elements.push(serde_json::from_value(explicit).unwrap());
    for element in &mut design.layouts[1].elements {
        if let Element::Text { format, .. } = element { format.font_family = Some("Arial".into()); }
    }
    deck.design = Some(design);
    deck = aislide_core::design::assign_layout(deck, "slide-1", "title-content").unwrap();
    let replaced = replace_font(deck, "Arial", "Noto Sans JP").unwrap();
    validate_deck(&replaced).unwrap();
    let value = serde_json::to_value(&replaced).unwrap();
    for field in ["major", "minor", "east_asian", "complex_script"] {
        assert_eq!(value["design"]["theme"]["fonts"][field], "Noto Sans JP");
    }
    assert_eq!(value["slides"][0]["elements"][0]["children"][0]["format"]["font_family"], "Noto Sans JP");
    assert_eq!(value["slides"][0]["elements"][0]["children"][0]["text"], "Arial content");
    assert_eq!(value["slides"][0]["elements"][0]["children"][1]["format"]["font_family"], "@major");
    assert!(value["slides"][0]["elements"][0]["children"][2].get("format").is_none());
    assert_eq!(value["slides"][0]["elements"][0]["children"][3]["format"]["font_family"], "Noto Sans JP");
    assert_eq!(value["slides"][0]["elements"][0]["children"][3]["text"], "Arial shape");
    assert_eq!(value["design"]["masters"][0]["elements"][0]["format"]["font_family"], "Noto Sans JP");
    assert_eq!(value["design"]["layouts"][0]["elements"][0]["format"]["font_family"], "Noto Sans JP");
    assert!(value["slides"][0]["elements"][1]["format"]["inherit_layout"].as_bool().unwrap());
    for (from, to) in [("Absent", "Other"), ("Noto Sans JP", "Noto Sans JP")] {
        assert_eq!(serde_json::to_value(replace_font(replaced.clone(), from, to).unwrap()).unwrap(), value);
    }
    for bad in ["", " ", "bad\nfont", "@major", "@minor"] {
        assert!(replace_font(replaced.clone(), "Noto Sans JP", bad).is_err());
        assert!(replace_font(replaced.clone(), bad, "Arial").is_err());
    }
    assert!(replace_font(replaced, "Noto Sans JP", &"a".repeat(101)).is_err());
}

#[test]
fn rich_text_stays_synchronized_and_contracts_have_schemas_and_single_boms() {
    let original = deck(vec![text("body", "plain")]);
    let mut rich = serde_json::to_value(&original).unwrap();
    rich["slides"][0]["elements"][0]["format"] = json!({"paragraphs":[{"runs":[{"text":"plain"}]}]});
    let rich_deck = serde_json::from_value::<Deck>(rich).unwrap();
    assert_eq!(search(&rich_deck, &options("plain")).unwrap().len(), 1);
    let replaced = replace(rich_deck.clone(), &replace_all("plain", "next")).unwrap();
    validate_deck(&replaced).unwrap();
    let value = serde_json::to_value(replaced).unwrap();
    assert_eq!(value["slides"][0]["elements"][0]["text"], "next");
    assert_eq!(value["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"][0]["text"], "next");
    assert_eq!(serde_json::to_value(replace_font(rich_deck.clone(), "Arial", "Aptos").unwrap()).unwrap(), serde_json::to_value(rich_deck).unwrap());
    let schema = schemars::schema_for!(ReplaceOptions);
    assert!(serde_json::to_value(schema).unwrap().is_object());
    for bytes in [include_bytes!("text_ops.rs").as_slice(), include_bytes!("../src/text_ops.rs").as_slice(),
        include_bytes!("../src/lib.rs").as_slice()] {
        assert!(bytes.starts_with(&[0xef, 0xbb, 0xbf]));
        assert!(!bytes[3..].starts_with(&[0xef, 0xbb, 0xbf]));
        assert!(std::str::from_utf8(bytes).is_ok());
    }
}

#[test]
fn rich_replacements_preserve_separated_styles_and_unicode_scalar_ranges() {
    let mut body = text("body", "\u{1f600}hit middle hit\nuntouched");
    body["format"] = json!({"paragraphs":[
        {"alignment":"center", "space_after":{"kind":"points", "value":120}, "runs":[
            {"text":"\u{1f600}hit", "style":{"bold":true}},
            {"text":" middle ", "style":{"italic":true, "highlight":"FFFF00", "baseline":1200}},
            {"text":"hit", "style":{"underline":true}}]},
        {"alignment":"right", "runs":[{"text":"untouched", "style":{"font_family":"Yu Gothic", "language":"ja-JP"}}]}
    ]});
    let original = deck(vec![body]);
    let matches = search(&original, &options("hit")).unwrap();
    assert_eq!(matches.iter().map(|found| (found.start, found.end)).collect::<Vec<_>>(), vec![(1, 4), (12, 15)]);
    let mut update = replace_all("hit", "NEW");
    update.replace_all = false;
    update.selected = Some(matches.into_iter().rev().collect());
    let result = replace(original.clone(), &update).unwrap();
    validate_deck(&result).unwrap();
    assert_eq!(serde_json::to_value(&result).unwrap(), serde_json::to_value(replace(original.clone(), &replace_all("hit", "NEW")).unwrap()).unwrap());
    let Element::Text { text, format, .. } = &result.slides[0].elements[0] else { panic!() };
    assert_eq!(text, "\u{1f600}NEW middle NEW\nuntouched");
    assert_eq!(aislide_core::rich_text::plain_text(&format.paragraphs), *text);
    let Element::Text { format: before, .. } = &original.slides[0].elements[0] else { panic!() };
    assert_eq!(format.paragraphs[1], before.paragraphs[1]);
    let middle = format.paragraphs[0].runs.iter().find(|run| run.text.contains(" middle ")).unwrap();
    assert_eq!(middle.style, before.paragraphs[0].runs[1].style);
    assert_eq!(format.paragraphs[0].alignment, before.paragraphs[0].alignment);
    assert_eq!(format.paragraphs[0].space_after, before.paragraphs[0].space_after);
}

#[test]
fn rich_fields_are_searchable_but_selected_cache_or_straddling_replacements_are_atomic_errors() {
    for kind in ["slidenum", "vendor-math-token"] {
        let mut body = text("body", "\u{1f600}hit 99 hit");
        body["format"] = json!({"paragraphs":[{"runs":[
            {"text":"\u{1f600}hit ", "style":{"bold":true}},
            {"text":"99", "style":{"italic":true}, "field":{"id":"{00112233-4455-6677-8899-aabbccddeeff}", "kind":kind}},
            {"text":" hit", "style":{"underline":true}}
        ]}]});
        let original = deck(vec![text("earlier", "99"), body]);
        let before = serde_json::to_value(&original).unwrap();
        for query in ["99", " 99 ", "9 h", "hit 99 hit"] {
            let matches = search(&original, &options(query)).unwrap();
            assert!(!matches.is_empty());
            let mut update = replace_all(query, "changed");
            assert!(matches!(replace(original.clone(), &update), Err(Error::Unsupported(_))));
            update.replace_all = false;
            update.selected = Some(matches);
            assert!(matches!(replace(original.clone(), &update), Err(Error::Unsupported(_))));
            assert_eq!(serde_json::to_value(&original).unwrap(), before);
        }
        assert!(matches!(replace(original.clone(), &replace_all("99", "99")), Err(Error::Unsupported(_))));
        let next = replace(original.clone(), &replace_all("hit", "\u{1f680}done")).unwrap();
        validate_deck(&next).unwrap();
        let Element::Text { text, format, .. } = &next.slides[0].elements[1] else { panic!() };
        assert_eq!(text, "\u{1f600}\u{1f680}done 99 \u{1f680}done");
        let cache = format.paragraphs[0].runs.iter().find(|run| run.field.is_some()).unwrap();
        assert_eq!(serde_json::to_value(cache).unwrap(), before["slides"][0]["elements"][1]["format"]["paragraphs"][0]["runs"][1]);
    }
}

fn styled_table() -> Value {
    let mut value = table("\u{1f600}hit middle hit");
    value["format"] = json!({"cells":[{"row":0, "column":0, "style":{
        "fill":"ABCDEF", "text_style":{"font_family":"Arial", "bold":true},
        "text_format":{"font_family":"Arial", "paragraphs":[{"runs":[
            {"text":"\u{1f600}hi", "style":{"font_family":"Arial"}},
            {"text":"t middle ", "style":{"italic":true, "highlight":"FFFF00"}},
            {"text":"hit", "style":{"font_family":"@minor", "underline":true}}
        ]}]}
    }}]});
    value
}

#[test]
fn rich_cell_cross_run_matches_sync_rows_without_discarding_cell_styles() {
    let original = deck(vec![styled_table()]);
    let found = search(&original, &options("hit")).unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].start, 1);
    assert_eq!(found[0].path, "/slides/0/elements/0/rows/0/0");
    let replaced = replace(original.clone(), &replace_all("hit", "NEXT")).unwrap();
    validate_deck(&replaced).unwrap();
    let Element::Table { rows, format, .. } = &replaced.slides[0].elements[0] else { panic!() };
    assert_eq!(rows[0][0], "\u{1f600}NEXT middle NEXT");
    assert_eq!(rows[0][1], "untouched");
    let style = &format.cells[0].style;
    assert_eq!(style.fill.as_deref(), Some("ABCDEF"));
    assert_eq!(style.text_style.as_ref().unwrap().font_family.as_deref(), Some("Arial"));
    let paragraphs = &style.text_format.as_ref().unwrap().paragraphs;
    assert_eq!(aislide_core::rich_text::plain_text(paragraphs), rows[0][0]);
    assert!(paragraphs[0].runs.iter().any(|run| run.text.contains(" middle ") && run.style.highlight.as_deref() == Some("FFFF00")));
    let mut selected = replace_all("hit", "X");
    selected.replace_all = false;
    selected.selected = Some(found);
    assert!(matches!(replace(replaced, &selected), Err(Error::Conflict(_))));
    assert!(matches!(replace(original, &replace_all("hit", &"x".repeat(200))), Err(Error::Limit(_))));
}

#[test]
fn rich_font_replacement_visits_runs_cell_defaults_and_master_layout_content() {
    let mut body = text("body", "Arial unchanged");
    body["format"] = json!({"paragraphs":[{"runs":[
        {"text":"Arial", "style":{"font_family":"Arial", "bold":true, "baseline":1000}},
        {"text":" unchanged", "style":{"font_family":"@major", "language":"en-US"}}
    ]}]});
    let mut original = deck(vec![body.clone(), styled_table()]);
    let mut design = Design::default();
    design.masters[0].elements = vec![serde_json::from_value(group("master-group", vec![body.clone(), styled_table()])).unwrap()];
    design.layouts[0].elements = vec![serde_json::from_value(body).unwrap()];
    original.design = Some(design);
    let target = "TextOps Fixture Font";
    let result = replace_font(original.clone(), "Arial", target).unwrap();
    validate_deck(&result).unwrap();
    let value = serde_json::to_value(&result).unwrap();
    for path in ["/slides/0/elements/0", "/design/masters/0/elements/0/children/0", "/design/layouts/0/elements/0"] {
        let element = value.pointer(path).unwrap();
        assert_eq!(element["text"], "Arial unchanged");
        assert_eq!(element["format"]["paragraphs"][0]["runs"][0]["style"]["font_family"], target);
        assert_eq!(element["format"]["paragraphs"][0]["runs"][0]["style"]["baseline"], 1000);
        assert_eq!(element["format"]["paragraphs"][0]["runs"][1]["style"]["font_family"], "@major");
    }
    for path in ["/slides/0/elements/1", "/design/masters/0/elements/0/children/1"] {
        let style = &value.pointer(path).unwrap()["format"]["cells"][0]["style"];
        assert_eq!(style["text_style"]["font_family"], target);
        assert_eq!(style["text_format"]["font_family"], target);
        assert_eq!(style["text_format"]["paragraphs"][0]["runs"][0]["style"]["font_family"], target);
        assert_eq!(style["text_format"]["paragraphs"][0]["runs"][2]["style"]["font_family"], "@minor");
    }
    assert_eq!(serde_json::to_value(replace_font(result, target, "Arial").unwrap()).unwrap(), serde_json::to_value(original).unwrap());
}

#[test]
fn native_rich_text_and_cells_replace_through_validated_transactions_and_exact_undo() {
    use aislide_core::{authoring_ops, document, pptx};
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut body = text("body", "\u{1f600}hit middle hit");
    body["format"] = json!({"paragraphs":[{"runs":[
        {"text":"\u{1f600}hit", "style":{"bold":true}},
        {"text":" middle ", "style":{"italic":true, "highlight":"FFFF00"}},
        {"text":"hit", "style":{"underline":true}}
    ]}]});
    let bytes = pptx::export_pptx(&deck(vec![body, styled_table()])).unwrap();
    let original: document::Document = serde_json::from_value(document::open_presentation("rich-text-ops".into(), bytes.clone()).unwrap()["document"].clone()).unwrap();
    assert_eq!(search(&original.deck, &options("hit")).unwrap().len(), 4);
    let changed = authoring_ops::replace_text(&original, 0, &replace_all("hit", "NEXT")).unwrap();
    document::verify(&changed.document).unwrap();
    validate_deck(&changed.document.deck).unwrap();
    let exported = document::export_presentation(&changed.document).unwrap();
    let saved = STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap();
    let reopened: document::Document = serde_json::from_value(document::open_presentation("rich-reopened".into(), saved).unwrap()["document"].clone()).unwrap();
    assert_eq!(search(&reopened.deck, &options("NEXT")).unwrap().len(), 4);
    assert_eq!(search(&reopened.deck, &options("middle")).unwrap().len(), 2);
    let Element::Text { format: before, .. } = &original.deck.slides[0].elements[0] else { panic!() };
    let Element::Text { format: after, .. } = &reopened.deck.slides[0].elements[0] else { panic!() };
    let middle_style = |format: &aislide_core::model::TextFormat| format.paragraphs[0].runs.iter().find(|run| run.text.contains(" middle ")).unwrap().style.clone();
    assert_eq!(middle_style(before), middle_style(after));
    let undone = document::undo(&changed.document, 1, changed.receipt.unwrap()).unwrap();
    assert_eq!(undone.document.hash, original.hash);
    assert_eq!(STANDARD.decode(document::export_presentation(&undone.document).unwrap()["base64"].as_str().unwrap()).unwrap(), bytes);
}

#[test]
fn unknown_inline_math_and_token_payloads_are_not_silently_treated_as_editable_runs() {
    for property in ["math", "token", "field_cache"] {
        let mut run = json!({"text":"opaque"});
        run[property] = json!({"content":"not an ordinary text run"});
        let error = serde_json::from_value::<aislide_core::rich_text::RichRun>(run).unwrap_err();
        assert!(error.to_string().contains("unknown field"));
        assert!(error.to_string().contains(property));
    }
}