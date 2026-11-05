use aislide_core::execute_request;
use serde_json::{json, Value};

fn catalog() -> Value { execute_request(json!({"op":"part_catalog"})).unwrap() }

fn example(preset: &str) -> Value {
    let catalog = catalog();
    catalog["presets"].as_array().unwrap().iter().chain(catalog["related"].as_array().unwrap()).find(|entry| entry["id"] == preset).unwrap_or_else(|| panic!("{preset}"))["example"].clone()
}

fn guided(parts: &[&str]) -> Value {
    let slides: Vec<Value> = parts.iter().enumerate().map(|(index, preset)| json!({
        "id": format!("s{}", index + 1), "section": "Mix", "headline": format!("Slide {} shows the relation.", index + 1), "sentence_form": "outcome", "pattern_id": "native-part",
        "question": "What relates?", "parent_message": "governing", "transition": "because", "parallel_basis": "relation",
        "part": {"version": 1, "preset": preset, "title": "Part", "data": {"kind": "items", "items": [{"label": "A", "detail": "First"}, {"label": "B", "detail": "Second"}]}},
        "support": [{"clause": format!("Slide {} shows the relation.", index + 1), "body_paths": ["/data/items"], "evidence_ids": ["e"]}],
    })).collect();
    json!({"version":1,"profile_id":"status-report","title":"Mix","audience":"Reviewers","purpose":"Show the part mix","governing_message":"The mix is information.","language":"en",
        "evidence":[{"id":"e","kind":"source","reference":"Synthetic test fixture","statement":"Synthetic."}],"slides":slides})
}

#[test]
fn issue85_part_catalog_lists_diagram_custom_as_a_related_part_with_an_example_add_part_accepts() {
    let catalog = catalog();
    // The 143 presets are unchanged; the managed graph is listed beside them.
    assert_eq!(catalog["presets"].as_array().unwrap().len(), 143);
    let related = catalog["related"].as_array().unwrap();
    assert_eq!(related.len(), 1);
    let entry = &related[0];
    assert_eq!(entry["id"], "diagram/custom");
    assert_eq!(entry["recommended"], true);
    let keywords: Vec<&str> = entry["keywords"].as_array().unwrap().iter().map(|keyword| keyword.as_str().unwrap()).collect();
    for keyword in ["architecture", "concept", "relationship", "boundary", "handoff"] { assert!(keywords.contains(&keyword), "{keyword}"); }
    assert!(entry["use_when"].as_str().unwrap().contains("Relationships, boundaries or handoffs"));
    // Everything it points to exists.
    let ids: Vec<&str> = catalog["presets"].as_array().unwrap().iter().map(|preset| preset["id"].as_str().unwrap()).collect();
    for target in ["flow/swimlane", "correlation/c4-container"] { assert!(ids.contains(&target), "{target}"); }
    // The example is a PartSpec that previews, profiles and inserts as a managed part.
    let spec = &entry["example"];
    assert_eq!((spec["preset"].as_str(), spec["data"]["kind"].as_str()), (Some("diagram/custom"), Some("diagram")));
    execute_request(json!({"op":"create_part","id":"probe","spec":spec})).unwrap();
    execute_request(json!({"op":"part_aspect","spec":spec})).unwrap();
    let deck = json!({"version":1,"title":"Diagram","width":1280,"height":720,"slides":[{"id":"s1","title":"One","background":"FFFFFF","notes":"","elements":[]}]});
    let document = execute_request(json!({"op":"new_document","id":"diagram","deck":deck})).unwrap();
    let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"s1","id":"architecture","spec":spec})).unwrap();
    assert_eq!(inserted["document"]["parts"][0]["spec"]["preset"], "diagram/custom");
    // Existing consumers of the catalog are unaffected by the extra field.
    let ids: Vec<&str> = catalog["presets"].as_array().unwrap().iter().map(|preset| preset["id"].as_str().unwrap()).collect();
    assert!(!ids.contains(&"diagram/custom"));
}

#[test]
fn issue85_authoring_capabilities_state_the_visual_default_and_its_relationship_parts() {
    let capabilities = execute_request(json!({"op":"authoring_capabilities"})).unwrap();
    let default = &capabilities["visual_default"];
    assert!(default["policy"].as_str().unwrap().contains("Explicit user exclusions"));
    assert_eq!(default["relationship_parts"], json!(["diagram/custom", "flow/swimlane", "correlation/c4-container"]));
    assert_eq!(default["icons_replace_relationship_diagrams"], false);
    assert!(default["deck_mix"].as_str().unwrap().contains("never a quota"));
}

#[test]
fn issue85_guided_validation_returns_the_deck_mix_for_review() {
    let presets = ["list/icon-rows", "list/icon-rows", "list/icon-rows", "diagram/custom", "list/icon-rows", "list-horizontal/icon-cards"];
    let checked = execute_request(json!({"op":"validate_guided_presentation","input":guided(&presets)})).unwrap();
    let expression = &checked["expression"];
    assert_eq!(expression["slides"], 6);
    assert_eq!(expression["parts"][0], json!({"preset":"list/icon-rows","slides":4,"instances":4}));
    assert_eq!(expression["parts"].as_array().unwrap().len(), 3);
    assert_eq!(expression["longest_runs"], json!([{"preset":"list/icon-rows","slides":3,"first_slide_id":"s1","last_slide_id":"s3"}]), "only runs of three or more are listed");
    assert_eq!(expression["relationship_diagrams"], 1);
    assert_eq!(expression["without_parts"], 0);
    assert!(expression["note"].as_str().unwrap().contains("not a target or a quota"));
    assert!(checked["review_required"].as_array().unwrap().iter().any(|item| item.as_str().unwrap().contains("part mix in expression") && item.as_str().unwrap().contains("no-diagram request wins")));

    // A deck of one repeated list preset and no relationship diagram reports exactly that, as information and not as an issue.
    let repeated = execute_request(json!({"op":"validate_guided_presentation","input":guided(&["list/icon-rows"; 5])})).unwrap();
    assert_eq!(repeated["expression"]["relationship_diagrams"], 0);
    assert_eq!(repeated["expression"]["longest_runs"][0]["slides"], 5);
    assert!(!repeated["issues"].as_array().unwrap().iter().any(|issue| issue.as_str().unwrap().contains("relationship")), "the mix never becomes an issue");
    let without_quota = execute_request(json!({"op":"validate_guided_presentation","input":guided(&["list/icon-rows", "diagram/custom"])})).unwrap();
    assert_eq!(without_quota["expression"]["longest_runs"], json!([]));
}

#[test]
fn issue85_preflight_reports_the_whole_decks_mix_whatever_pages_it_checks() {
    let slides: Vec<Value> = (1..=5).map(|index| json!({"id":format!("s{index}"),"title":format!("Slide {index}"),"background":"FFFFFF","notes":"","elements":[]})).collect();
    let deck = json!({"version":1,"title":"Mix","width":1280,"height":720,"slides":slides});
    let mut document = execute_request(json!({"op":"new_document","id":"mix","deck":deck})).unwrap();
    // Slides 1-3 hold the same list preset, slide 4 a swimlane (a relationship diagram), slide 5 holds no managed part.
    for (slide, preset) in [("s1", "list/icon-rows"), ("s2", "list/icon-rows"), ("s3", "list/icon-rows"), ("s4", "flow/swimlane")] {
        let revision = document["revision"].as_u64().unwrap();
        let inserted = execute_request(json!({"op":"insert_part","document":document,"expected_revision":revision,"slide_id":slide,"id":format!("part-{slide}"),"spec":example(preset)})).unwrap();
        document = inserted["document"].clone();
    }
    let report = execute_request(json!({"op":"preflight_presentation","document":document,"options":{"page_indices":[4]}})).unwrap();
    let composition = &report["deck_composition"];
    assert_eq!(composition["slides"], 5);
    assert_eq!(composition["parts"], json!([{"preset":"list/icon-rows","slides":3,"instances":3},{"preset":"flow/swimlane","slides":1,"instances":1}]));
    assert_eq!(composition["longest_runs"], json!([{"preset":"list/icon-rows","slides":3,"first_slide_id":"s1","last_slide_id":"s3"}]));
    assert_eq!(composition["relationship_diagrams"], 1);
    assert_eq!(composition["without_parts"], 1);
    // It is not a finding: the selected page has none about it.
    assert!(report["findings"].as_array().unwrap().iter().all(|finding| !finding["code"].as_str().unwrap().contains("COMPOSITION") && !finding["code"].as_str().unwrap().contains("MIX")));
    let other = execute_request(json!({"op":"preflight_presentation","document":document,"options":{"page_indices":[0]}})).unwrap();
    assert_eq!(other["deck_composition"], *composition);
}
