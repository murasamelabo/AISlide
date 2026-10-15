use aislide_core::{authoring_batch, document::Document, editing::{create_with_setup, PresentationSetup}, execute_request, recipes::{self, RecipeInput, RecipeIntent, RecipeRelationship}};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn layer_input() -> RecipeInput {
    serde_json::from_value(json!({
        "version": 1, "title": "Synthetic layer responsibilities", "source_caption": "Synthetic layout fixture; not production evidence.",
        "takeaway": "Interpret signals before acting.", "notes": "Speaker notes kept verbatim.",
        "data": {"kind": "layer_role_map", "layer_heading": "Layer", "role_heading": "Responsibility", "layers": [
            {"id": "signals", "label": "Signals", "role": "Events and telemetry"},
            {"id": "context", "label": "Context", "role": "Shared investigation context"},
            {"id": "actions", "label": "Actions", "role": "Approved response actions"}
        ]}
    })).unwrap()
}

#[test]
fn catalog_exposes_initial_and_specialized_information_relationships() {
    let catalog = recipes::catalog();
    let entries = catalog["recipes"].as_array().unwrap();
    assert_eq!(entries.len(), 5, "all implemented recipe families must be discoverable");
    let ids: Vec<_> = entries.iter().map(|entry| entry["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["layer-role-map", "source-reading", "activity-outcome", "entity-instance-map", "screenshot-reading"]);
    for entry in entries {
        assert!(!entry["use_when"].as_str().unwrap().is_empty());
        assert!(!entry["avoid_when"].as_str().unwrap().is_empty());
        assert!(!entry["variants"].as_array().unwrap().is_empty());
    }
}

#[test]
fn recommendations_select_by_relationship_and_expose_missing_inputs() {
    let intent = RecipeIntent { relationship: RecipeRelationship::EvidenceReading, item_count: 2, has_source_image: false, has_measurement_conditions: false, ..Default::default() };
    let candidates = recipes::recommend(&intent).unwrap();
    assert_eq!(candidates.len(), 2);
    assert!(candidates.iter().all(|candidate| candidate.recipe_id == "source-reading"));
    assert!(candidates.iter().all(|candidate| candidate.missing_inputs == ["image"]));
    assert!(candidates.iter().all(|candidate| !candidate.reason.is_empty() && !candidate.fallback.is_empty()));
    let layers = RecipeIntent { relationship: RecipeRelationship::Layers, item_count: 6, ..intent.clone() };
    assert!(recipes::recommend(&layers).is_err());
    let outcome = RecipeIntent { relationship: RecipeRelationship::ActivityOutcome, item_count: 2, ..intent };
    let candidates = recipes::recommend(&outcome).unwrap();
    assert!(candidates[0].missing_inputs.contains(&"measurement_conditions".to_owned()));
}

#[test]
fn recipe_recommendations_discriminate_reader_goals_and_reject_forced_relations() {
    let wide = execute_request(json!({"op": "recommend_recipes", "intent": {
        "relationship": "evidence_reading", "item_count": 2, "has_source_image": true,
        "source_dimensions": {"width": 825, "height": 290}, "reader_goal": "trace_flow"
    }})).unwrap();
    assert_eq!(wide["candidates"][0]["variant"], "visual-top");
    assert!(wide["candidates"][0]["reason"].as_str().unwrap().contains("wide"));
    let height_limited = execute_request(json!({"op": "recommend_recipes", "intent": {
        "relationship": "evidence_reading", "item_count": 2, "has_source_image": true,
        "source_dimensions": {"width": 825, "height": 500}, "reader_goal": "trace_flow"
    }})).unwrap();
    assert!(height_limited["candidates"].as_array().unwrap().iter().all(|candidate| candidate["variant"] != "visual-top"), "a nominally wide source must not rank above a larger contained alternative");
    assert!(height_limited["rejected"].as_array().unwrap().iter().any(|entry| entry["code"] == "upper_source_not_larger"));
    let screenshot = execute_request(json!({"op": "recommend_recipes", "intent": {
        "relationship": "evidence_reading", "item_count": 3, "has_source_image": true,
        "needs_exact_positions": true, "reader_goal": "locate_controls"
    }})).unwrap();
    assert_eq!(screenshot["candidates"][0]["recipe_id"], "screenshot-reading");
    assert!(!screenshot["rejected"].as_array().unwrap().is_empty());
    let forced = execute_request(json!({"op": "recommend_recipes", "intent": {
        "relationship": "layers", "actual_relationship": "entity_instance", "item_count": 2
    }})).unwrap();
    assert_eq!(forced["status"], "no_suitable_recipe");
    assert!(forced["candidates"].as_array().unwrap().is_empty());
    assert_eq!(forced["rejected"][0]["code"], "not_ordered_layers");
    let entities = execute_request(json!({"op": "recommend_recipes", "intent": {
        "relationship": "entity_instance", "item_count": 3, "reader_goal": "understand_relationship"
    }})).unwrap();
    assert_eq!(entities["candidates"][0]["recipe_id"], "entity-instance-map");
    assert!(!entities["candidates"][0]["decision_checks"].as_array().unwrap().is_empty());
}

#[test]
fn layer_recipe_builds_a_diagram_and_table_from_the_same_ordered_data() {
    let document = create_with_setup("recipe".into(), "Synthetic recipes".into(), &PresentationSetup::default()).unwrap();
    let input = layer_input();
    let prepared = recipes::prepare(&document.deck, &input, "diagram-table").unwrap();
    let composition = serde_json::to_value(prepared.composition()).unwrap();
    assert_eq!(composition["pattern"]["id"], "split/1-1");
    assert_eq!(composition["slots"]["left"]["spec"]["preset"], "layer/balanced");
    assert_eq!(composition["slots"]["right"]["rows"][1], json!(["Signals", "Events and telemetry"]));
    assert_eq!(composition["slots"]["right"]["rows"][2], json!(["Context", "Shared investigation context"]));
    assert_eq!(composition["slots"]["right"]["rows"][3], json!(["Actions", "Approved response actions"]));
    assert_eq!(composition["slots"]["message"]["text"], input.takeaway);
    assert_eq!(composition["footer"], input.source_caption);
    assert_eq!(prepared.notes(), input.notes);
    assert!(prepared.layout_checked);
    assert!(!prepared.semantic_truth_verified && !prepared.office_visual_parity_verified);
}

#[test]
fn layer_recipe_table_uses_compact_header_and_matches_diagram_typography() {
    let setup = serde_json::from_value(json!({"design_preset": "trust", "font_family": "BIZ UDPGothic"})).unwrap();
    let document = create_with_setup("typography".into(), "Synthetic table typography".into(), &setup).unwrap();
    let mut input = layer_input();
    if let recipes::RecipeData::LayerRoleMap { layers, .. } = &mut input.data {
        layers.truncate(2);
        layers[0].role = "Common settings,\ntoken issuance and\nrequired resources.".into();
        layers[1].role = "Tenant permissions\nand operating state.".into();
    }
    let result = apply(&document, &input, "diagram-table");
    let elements = result["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
    let table = elements.iter().find(|element| element["type"] == "table").unwrap();
    let heights = table["format"]["row_heights"]["values"].as_array().expect("a header must not inherit equal full-slot row heights");
    assert_eq!(heights.len(), 3);
    assert!(heights[0].as_f64().unwrap() <= 56.0, "a one-line header must stay compact");
    fn diagram_label_size(element: &Value) -> Option<f64> {
        if element["text"] == "Signals" { return element["font_size"].as_f64(); }
        element["children"].as_array().and_then(|children| children.iter().find_map(diagram_label_size))
    }
    let label_size = elements.iter().find_map(diagram_label_size).unwrap();
    assert_eq!(table["font_size"].as_f64().unwrap(), label_size, "paired diagram and table must share the same deliberate body scale");
    for cell in table["format"]["cells"].as_array().unwrap() {
        assert_eq!(cell["style"]["text_style"]["font_family"], "@minor", "table cells must inherit the same theme family as diagram labels");
    }
    let report = execute_request(json!({"op": "measure_layout", "deck": result["document"]["deck"]})).unwrap();
    assert!(!report["measurements"].as_array().unwrap().iter().any(|measurement| measurement["overflow"] == true));
}

fn image() -> String {
    let image = image::RgbImage::from_fn(180, 60, |x, y| image::Rgb([(x * 255 / 180) as u8, (y * 255 / 60) as u8, 128]));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgb8(image).write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    STANDARD.encode(bytes)
}

fn source_input() -> RecipeInput {
    serde_json::from_value(json!({
        "version": 1, "title": "\u{539f}\u{56f3}\u{3068}\u{8aac}\u{660e}", "source_caption": "\u{67b6}\u{7a7a}\u{4f8b}\u{3002}\u{753b}\u{50cf}\u{306f}\u{914d}\u{7f6e}\u{691c}\u{67fb}\u{7528}\u{3002}", "takeaway": "\u{539f}\u{56f3}\u{3092}\u{4fdd}\u{6301}\u{3057}\u{3066}\u{8aac}\u{660e}\u{3059}\u{308b}", "notes": "\u{767a}\u{8868}\u{8005}\u{30ce}\u{30fc}\u{30c8}\u{3092}\u{4fdd}\u{6301}\u{3059}\u{308b}\u{3002}",
        "data": {"kind": "source_reading", "image": {"base64": image(), "mime_type": "image/png", "alt": "Synthetic wide image"}, "readings": [
            {"label": "\u{4e3b}\u{4f53}", "detail": "\u{8ab0}\u{304c}\u{51e6}\u{7406}\u{3059}\u{308b}\u{304b}"},
            {"label": "\u{6761}\u{4ef6}", "detail": "\u{9069}\u{7528}\u{7bc4}\u{56f2}\u{3092}\u{78ba}\u{8a8d}\u{3059}\u{308b}"}
        ]}
    })).unwrap()
}

fn outcome_input() -> RecipeInput {
    serde_json::from_value(json!({
        "version": 1, "title": "Synthetic measure definitions", "source_caption": "Synthetic operational example; not measured.", "takeaway": "Define outcomes before comparing results.", "notes": "No effectiveness claim is made.",
        "data": {"kind": "activity_outcome", "from_label": "Activity", "to_label": "Outcome", "measurement_conditions": "Example only; period and population must be supplied.", "changes": [
            {"activity": "Alerts handled", "outcome": "Exposure addressed", "definition": "Specify the population and resolution rule."},
            {"activity": "Reviews performed", "outcome": "Access corrected", "definition": "Record the scope and completion criteria."}
        ]}
    })).unwrap()
}

fn apply(document: &Document, input: &RecipeInput, variant: &str) -> Value {
    let prepared = recipes::prepare(&document.deck, input, variant).unwrap();
    let operations = prepared.operations(&document.deck, "slide-1", "recipe").unwrap();
    serde_json::to_value(authoring_batch::apply_operations(document, document.revision, &document.hash, &operations).unwrap()).unwrap()
}

fn text(element: &Value) -> String {
    let mut content = element["text"].as_str().unwrap_or_default().to_owned();
    if let Some(children) = element["children"].as_array() { for child in children { content.push_str(&text(child)); } }
    if let Some(rows) = element["rows"].as_array() { for row in rows { for cell in row.as_array().unwrap() { content.push_str(cell.as_str().unwrap()); } } }
    content
}

#[test]
fn source_reading_keeps_image_bytes_aspect_attribution_notes_and_label_emphasis() {
    for variant in ["visual-left", "visual-right"] {
        let document = create_with_setup("source".into(), "Synthetic recipe".into(), &PresentationSetup::default()).unwrap();
        let input = source_input();
        let result = apply(&document, &input, variant);
        let slide = &result["document"]["deck"]["slides"][0];
        let elements = slide["elements"].as_array().unwrap();
        let picture = elements.iter().find(|element| element["type"] == "picture").unwrap();
        let recipes::RecipeData::SourceReading { image, .. } = &input.data else { panic!("source reading fixture required") };
        assert_eq!(picture["base64"], image.base64);
        assert!((picture["width"].as_f64().unwrap() / picture["height"].as_f64().unwrap() - 3.0).abs() < 0.001);
        assert_eq!(picture["alt"], "Synthetic wide image");
        assert_eq!(slide["notes"], input.notes);
        assert!(elements.iter().any(|element| element["text"] == input.source_caption));
        assert!(elements.iter().any(|element| element["text"] == input.takeaway));
        let explanation = elements.iter().find(|element| element["id"] == if variant == "visual-left" { "recipe-support" } else { "recipe-lead" }).unwrap();
        assert_eq!(explanation["format"]["paragraphs"][0]["runs"][0]["style"]["bold"], true);
        assert_eq!(explanation["format"]["paragraphs"][0]["runs"][0]["style"]["color"], "@accent1");
        assert_eq!(explanation["format"]["paragraphs"][0]["runs"][0]["style"]["language"], "ja-JP");
    }
}

#[test]
fn visual_top_enlarges_the_actual_contained_source_without_distortion() {
    let setup = serde_json::from_value(json!({"design_preset": "trust"})).unwrap();
    let document = create_with_setup("source-size".into(), "Synthetic source size".into(), &setup).unwrap();
    let mut input = source_input();
    input.component_profile = Some(aislide_core::design_tokens::ComponentProfile::TechnicalBriefing);
    let raster = image::DynamicImage::ImageRgb8(image::RgbImage::new(825, 290));
    let mut bytes = Vec::new();
    raster.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    let encoded = STANDARD.encode(bytes);
    if let recipes::RecipeData::SourceReading { image, .. } = &mut input.data { image.base64 = encoded.clone(); }
    let side = apply(&document, &input, "visual-left");
    let top = apply(&document, &input, "visual-top");
    let picture = |result: &Value| result["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["type"] == "picture").unwrap().clone();
    let side = picture(&side);
    let top = picture(&top);
    let width = top["width"].as_f64().unwrap();
    let height = top["height"].as_f64().unwrap();
    assert!(width >= side["width"].as_f64().unwrap() * 1.1, "the recommended upper source must actually be larger: side={}, top={width}", side["width"]);
    assert!((width / height - 825.0 / 290.0).abs() < 0.001);
    assert_eq!(top["base64"], encoded);
}

#[test]
fn screenshot_details_enlarge_source_labels_and_keep_markers_off_the_original() {
    let document = create_with_setup("screen-details".into(), "Synthetic screen details".into(), &PresentationSetup::default()).unwrap();
    let raster = image::DynamicImage::ImageRgb8(image::RgbImage::new(1200, 1131));
    let mut bytes = Vec::new();
    raster.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    let encoded = STANDARD.encode(bytes);
    let input: RecipeInput = serde_json::from_value(json!({
        "version": 1, "title": "Synthetic exact controls", "source_caption": "Synthetic fixture; not a service screen.",
        "takeaway": "Inspect the supplied controls.", "component_profile": "technical_briefing",
        "data": {"kind": "screenshot_reading", "image": {"base64": encoded, "mime_type": "image/png", "alt": "Synthetic settings screen"}, "callouts": [
            {"x": 0.47, "y": 0.56, "label": "Conditions", "detail": "Inspect the supplied conditions."},
            {"x": 0.37, "y": 0.697, "label": "Block access", "detail": "Inspect the supplied action."}
        ]}
    })).unwrap();
    let result = apply(&document, &input, "position-callouts");
    let group = result["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["id"] == "recipe-primary").unwrap();
    let children = group["children"].as_array().unwrap();
    let pictures: Vec<_> = children.iter().filter(|element| element["type"] == "picture").collect();
    assert_eq!(pictures.len(), 3, "a complete original plus two readable source detail views are required");
    for picture in &pictures { assert_eq!(picture["base64"], encoded); }
    let full = pictures[0];
    let full_right = full["x"].as_f64().unwrap() + full["width"].as_f64().unwrap();
    for picture in pictures.iter().skip(1) {
        let fraction = 1.0 - picture["crop"]["left"].as_f64().unwrap() - picture["crop"]["right"].as_f64().unwrap();
        let scale = picture["width"].as_f64().unwrap() / (1200.0 * fraction);
        assert!(scale >= 1.2, "source UI labels must be enlarged, not merely fit: {scale}");
    }
    for badge in children.iter().filter(|element| element["preset"] == "ellipse" && matches!(element["text"].as_str(), Some("1" | "2"))) {
        assert!(badge["x"].as_f64().unwrap() >= full_right, "numbered markers must not cover source labels");
    }
    let regions: Vec<_> = children.iter().filter(|element| element["preset"] == "rect" && element["fill"] == "none" && element["x"].as_f64().is_some_and(|x| x < full_right)).collect();
    let leaders: Vec<_> = children.iter().filter(|element| element["type"] == "connector").collect();
    assert_eq!(regions.len(), leaders.len());
    for (region, leader) in regions.iter().zip(leaders) {
        let right = region["x"].as_f64().unwrap() + region["width"].as_f64().unwrap();
        assert!((leader["x"].as_f64().unwrap() - right).abs() < 0.001, "the leader must start at its highlighted region, not at a detached image-edge position");
    }
}

#[test]
fn supplied_screen_regions_align_in_original_and_details_and_survive_native_reopen() {
    let document = create_with_setup("screen-regions".into(), "Synthetic screen regions".into(), &PresentationSetup::default()).unwrap();
    let raster = image::DynamicImage::ImageRgb8(image::RgbImage::new(1200, 1131));
    let mut bytes = Vec::new();
    raster.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    let regions = [json!({"x": 0.32, "y": 0.378, "width": 0.30, "height": 0.0725}), json!({"x": 0.32, "y": 0.648, "width": 0.30, "height": 0.0725})];
    let input: RecipeInput = serde_json::from_value(json!({
        "version": 1, "title": "Synthetic supplied target regions", "source_caption": "Synthetic fixture.", "takeaway": "Keep exact locations distinct.", "component_profile": "technical_briefing",
        "data": {"kind": "screenshot_reading", "image": {"base64": STANDARD.encode(bytes), "mime_type": "image/png", "alt": "Synthetic screen"}, "callouts": [
            {"x": 0.47, "y": 0.40, "target_region": regions[0], "label": "Subject", "detail": "Inspect the supplied subject."},
            {"x": 0.47, "y": 0.69, "target_region": regions[1], "label": "Grant", "detail": "Inspect the supplied grant."}
        ]}
    })).unwrap();
    let result = apply(&document, &input, "position-callouts");
    let group = result["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["id"] == "recipe-primary").unwrap();
    let children = group["children"].as_array().unwrap();
    let pictures: Vec<_> = children.iter().filter(|element| element["type"] == "picture").collect();
    let full = pictures[0];
    let original_right = full["x"].as_f64().unwrap() + full["width"].as_f64().unwrap();
    let original_regions: Vec<_> = children.iter().filter(|element| element["preset"] == "rect" && element["fill"] == "none" && element["x"].as_f64().is_some_and(|x| x < original_right)).collect();
    let detail_regions: Vec<_> = children.iter().filter(|element| element["preset"] == "rect" && element["fill"] == "none" && element["x"].as_f64().is_some_and(|x| x >= original_right)).collect();
    assert_eq!((original_regions.len(), detail_regions.len()), (2, 2));
    for index in 0..2 {
        let region = &regions[index];
        let original = original_regions[index];
        for field in ["x", "y", "width", "height"] {
            let dimension = if matches!(field, "x" | "width") { "width" } else { "height" };
            assert!((original[field].as_f64().unwrap() - region[field].as_f64().unwrap() * full[dimension].as_f64().unwrap()).abs() < 0.001, "original region field {field} shifted");
        }
        let picture = pictures[index + 1];
        let zoom = picture["width"].as_f64().unwrap() / (1200.0 * (1.0 - picture["crop"]["left"].as_f64().unwrap() - picture["crop"]["right"].as_f64().unwrap()));
        let expected_x = picture["x"].as_f64().unwrap() + (region["x"].as_f64().unwrap() - picture["crop"]["left"].as_f64().unwrap()) * 1200.0 * zoom;
        let expected_y = picture["y"].as_f64().unwrap() + (region["y"].as_f64().unwrap() - picture["crop"]["top"].as_f64().unwrap()) * 1131.0 * zoom;
        assert!((detail_regions[index]["x"].as_f64().unwrap() - expected_x).abs() < 0.001);
        assert!((detail_regions[index]["y"].as_f64().unwrap() - expected_y).abs() < 0.001);
    }
    let exported = execute_request(json!({"op": "export_presentation", "document": result["document"]})).unwrap();
    let reopened = execute_request(json!({"op": "open_presentation", "id": "region-reopen", "base64": exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["parts"][0]["spec"]["data"]["callouts"][0]["target_region"], regions[0]);
}

#[test]
fn new_recipe_variants_preserve_source_images_positions_and_relationships() {
    let document = create_with_setup("visual-decisions".into(), "Synthetic visual decisions".into(), &PresentationSetup::default()).unwrap();
    let wide = apply(&document, &source_input(), "visual-top");
    let elements = wide["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
    let picture = elements.iter().find(|element| element["type"] == "picture").unwrap();
    let explanation = elements.iter().find(|element| element["id"] == "recipe-insight-1").unwrap();
    assert!(picture["y"].as_f64().unwrap() < explanation["y"].as_f64().unwrap());
    let entity: RecipeInput = serde_json::from_value(json!({
        "version": 1, "title": "Synthetic definition and instances", "source_caption": "Synthetic fixture, not an actual tenant.",
        "takeaway": "Definition and instance are different roles.", "notes": "Exact entity notes.",
        "data": {"kind": "entity_instance_map", "definition": {"label": "Definition", "detail": "Common configuration"}, "relation_label": "Creates", "instances": [
            {"label": "Tenant A", "detail": "Local permissions"}, {"label": "Tenant B", "detail": "Independent permissions"}
        ]}
    })).unwrap();
    let entities = apply(&document, &entity, "definition-instances");
    assert_eq!(entities["document"]["parts"][0]["spec"]["preset"], "diagram/custom");
    let content: String = entities["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().map(text).collect();
    for value in ["Definition", "Tenant A", "Tenant B", "Creates"] { assert!(content.contains(value)); }
    let screenshot: RecipeInput = serde_json::from_value(json!({
        "version": 1, "title": "Synthetic controls", "source_caption": "Synthetic screenshot fixture.", "takeaway": "Use supplied positions.", "notes": "Exact screenshot notes.",
        "data": {"kind": "screenshot_reading", "image": {"base64": image(), "mime_type": "image/png", "alt": "Synthetic screen"}, "callouts": [
            {"x": 0.2, "y": 0.4, "label": "Subject", "detail": "Inspect the subject."},
            {"x": 0.7, "y": 0.6, "label": "Condition", "detail": "Inspect the condition."}
        ]}
    })).unwrap();
    let screens = apply(&document, &screenshot, "position-callouts");
    assert_eq!(screens["document"]["parts"][0]["spec"]["data"]["callouts"][0]["x"], 0.2);
    let mut elements = screens["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().clone();
    let mut pictures = Vec::new();
    while let Some(element) = elements.pop() {
        if element["type"] == "picture" { pictures.push(element); }
        else if let Some(children) = element["children"].as_array() { elements.extend(children.clone()); }
    }
    assert_eq!(pictures[0]["base64"], image());
    assert_eq!(screens["document"]["deck"]["slides"][0]["notes"], screenshot.notes);
}

#[test]
fn component_profile_is_shared_by_recipes_and_ordinary_pattern_composition() {
    let setup = serde_json::from_value(json!({"design_preset": "trust"})).unwrap();
    let document = create_with_setup("shared-components".into(), "Synthetic shared components".into(), &setup).unwrap();
    let mut source = serde_json::to_value(source_input()).unwrap();
    source["component_profile"] = json!("technical_briefing");
    let input: RecipeInput = serde_json::from_value(source).unwrap();
    let recipe = apply(&document, &input, "visual-left");
    let elements = recipe["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
    let title = elements.iter().find(|element| element["id"] == "recipe-title").unwrap();
    let footer = elements.iter().find(|element| element["id"] == "recipe-footer").unwrap();
    let message = elements.iter().find(|element| element["id"] == "recipe-message").unwrap();
    assert_eq!(title["font_size"].as_f64(), Some(40.0));
    assert_eq!(title["height"].as_f64(), Some(60.0));
    assert_eq!(footer["font_size"].as_f64(), Some(14.0));
    assert_eq!(footer["y"].as_f64(), Some(650.0));
    assert_eq!(message["font_size"].as_f64(), Some(20.0));
    assert_eq!(message["y"].as_f64(), Some(580.0));
    assert_eq!(message["format"]["vertical"], "middle");
    let ordinary = execute_request(json!({"op": "apply_operations", "document": document, "expected_revision": document.revision, "expected_hash": document.hash, "operations": [{
        "op": "compose_slide", "slide_id": "slide-1", "id": "ordinary", "spec": {
            "title": input.title, "footer": input.source_caption, "component_profile": "technical_briefing",
            "pattern": {"id": "focus/full", "message_band": true},
            "slots": {"primary": {"kind": "table", "rows": [["Work", "Purpose"], ["Supplied content", "Supplied role"]]}, "message": {"kind": "callout", "text": input.takeaway}}
        }
    }]})).unwrap();
    let ordinary = ordinary["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
    let ordinary_title = ordinary.iter().find(|element| element["id"] == "ordinary-title").unwrap();
    let ordinary_footer = ordinary.iter().find(|element| element["id"] == "ordinary-footer").unwrap();
    let ordinary_message = ordinary.iter().find(|element| element["id"] == "ordinary-message").unwrap();
    for field in ["x", "y", "width", "height", "font_size"] {
        assert_eq!(ordinary_title[field], title[field]);
        assert_eq!(ordinary_footer[field], footer[field]);
        assert_eq!(ordinary_message[field], message[field]);
    }
    let styles = execute_request(json!({"op": "component_styles", "deck": document.deck, "profile": "technical_briefing"})).unwrap();
    assert_eq!(styles["roles"]["title"]["font_size"].as_f64(), Some(40.0));
    assert_eq!(styles["roles"]["source"]["frame"]["y"].as_f64(), Some(650.0));
}

#[test]
fn normal_supporting_takeaway_is_visibly_separated_from_body_and_source() {
    let document = create_with_setup("takeaway-boundary".into(), "Synthetic takeaway boundary".into(), &PresentationSetup::default()).unwrap();
    let mut input = source_input();
    input.component_profile = Some(aislide_core::design_tokens::ComponentProfile::TechnicalBriefing);
    let result = apply(&document, &input, "visual-left");
    let elements = result["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
    let separator = elements.iter().find(|element| element["id"] == "recipe-message-separator").expect("a normal supporting takeaway needs a distinct boundary, not unframed footer-like text");
    let message = elements.iter().find(|element| element["id"] == "recipe-message").unwrap();
    let footer = elements.iter().find(|element| element["id"] == "recipe-footer").unwrap();
    assert!(separator["height"].as_f64().unwrap() <= 2.0);
    assert!(message["y"].as_f64().unwrap() >= separator["y"].as_f64().unwrap() + 8.0);
    assert!(message["y"].as_f64().unwrap() + message["height"].as_f64().unwrap() < footer["y"].as_f64().unwrap());
    assert_eq!(message["font_size"].as_f64(), Some(20.0));
}

#[test]
fn emphasis_methods_are_selected_by_strength_and_relationship_without_losing_text() {
    let document = create_with_setup("emphasis-methods".into(), "Synthetic emphasis methods".into(), &PresentationSetup::default()).unwrap();
    let styles = execute_request(json!({"op": "component_styles", "deck": document.deck, "profile": "technical_briefing"})).unwrap();
    assert_eq!(styles["emphasis_variants"].as_array().expect("emphasis methods must be discoverable").len(), 6);
    for (strength, relationship, expected) in [
        ("subtle", "supporting", "inline"), ("normal", "supporting", "rule"),
        ("normal", "standalone", "outline"), ("normal", "contrast", "outline"),
        ("subtle", "conditional", "inline"), ("normal", "conditional", "rule"), ("strong", "conditional", "outline"),
        ("strong", "supporting", "tint"),
    ] {
        let result = execute_request(json!({"op": "apply_operations", "document": document, "expected_revision": document.revision, "expected_hash": document.hash, "operations": [{
            "op": "compose_slide", "slide_id": "slide-1", "id": "choice", "spec": {
                "title": "Supplied evidence", "component_profile": "technical_briefing", "pattern": {"id": "focus/full", "message_band": true},
                "slots": {
                    "primary": {"kind": "table", "rows": [["Subject", "Scope"], ["Known subject", "Known scope"]]},
                    "message": {"kind": "emphasis", "emphasis": {"strength": strength, "relationship": relationship}, "paragraphs": [{"runs": [{"text": "Keep the "}, {"text": "condition", "style": {"bold": true, "color": "@accent2"}}, {"text": " visible."}]}]}
                }
            }
        }]})).unwrap();
        let elements = result["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
        let message = elements.iter().find(|element| element["id"] == "choice-message").unwrap();
        let surface = elements.iter().find(|element| element["id"] == "choice-message-surface");
        let separator = elements.iter().any(|element| element["id"] == "choice-message-separator");
        let text_index = elements.iter().position(|element| element["id"] == "choice-message").unwrap();
        for (index, element) in elements.iter().enumerate().filter(|(_, element)| matches!(element["id"].as_str(), Some("choice-message-surface" | "choice-message-separator"))) {
            assert!(index < text_index, "{} must stay behind the emphasis text", element["id"]);
        }
        assert_eq!(message["text"], "Keep the condition visible.");
        assert_eq!(message["format"]["paragraphs"][0]["runs"][1]["style"]["color"], "@accent2");
        assert_ne!(message["format"]["paragraphs"][0]["runs"][0]["style"]["color"], "@accent2");
        assert_eq!(separator, expected == "rule");
        match expected {
            "outline" => assert_eq!(surface.unwrap()["fill"], "none"),
            "tint" => assert_eq!(surface.unwrap()["fill"], "@lt2"),
            _ => assert!(surface.is_none()),
        }
    }
}

#[test]
fn visual_quality_profile_uses_open_components_without_changing_legacy_cards() {
    let document = create_with_setup("open-components".into(), "Synthetic open components".into(), &PresentationSetup::default()).unwrap();
    let compose = |profile: Value| execute_request(json!({"op": "apply_operations", "document": document, "expected_revision": document.revision, "expected_hash": document.hash, "operations": [{
        "op": "compose_slide", "slide_id": "slide-1", "id": "open", "spec": {
            "title": "Supplied peer concepts", "component_profile": profile,
            "pattern": {"id": "columns/3", "message_band": true},
            "slots": {
                "column-1": {"kind": "cards", "items": [{"label": "Discover", "detail": "Inventory the supplied subjects."}]},
                "column-2": {"kind": "cards", "items": [{"label": "Protect", "detail": "Apply the supplied conditions."}]},
                "column-3": {"kind": "cards", "items": [{"label": "Review", "detail": "Keep the supplied evidence."}]},
                "message": {"kind": "callout", "text": "Start with the known subjects."}
            }
        }
    }]})).unwrap();
    let open = compose(json!("technical_briefing"));
    let legacy = compose(Value::Null);
    for suffix in ["column-1", "column-2", "column-3", "message"] {
        let id = format!("open-{suffix}");
        let open = open["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["id"] == id).unwrap();
        let legacy = legacy["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["id"] == id).unwrap();
        assert_eq!(open["type"], "text", "the technical profile must not enclose {suffix} in a ruled rectangle");
        assert_eq!(legacy["type"], "shape", "omitting the profile preserves explicit legacy cards and bands");
        assert!(open["format"]["paragraphs"][0]["runs"][0]["style"]["color"].is_string());
    }
    let elements = open["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
    assert!(!elements.iter().any(|element| element["id"].as_str().is_some_and(|id| id.ends_with("-bar") || id.ends_with("-rule"))));
}

#[test]
fn visual_quality_takeaway_retains_explicit_rich_text_emphasis() {
    let document = create_with_setup("rich-takeaway".into(), "Synthetic emphasis".into(), &PresentationSetup::default()).unwrap();
    let result = execute_request(json!({"op": "apply_operations", "document": document, "expected_revision": document.revision, "expected_hash": document.hash, "operations": [{
        "op": "compose_slide", "slide_id": "slide-1", "id": "emphasis", "spec": {
            "title": "Supplied evidence", "component_profile": "technical_briefing",
            "pattern": {"id": "focus/full", "message_band": true},
            "slots": {
                "primary": {"kind": "table", "rows": [["Subject", "Scope"], ["Known subject", "Known scope"]]},
                "message": {"kind": "text", "paragraphs": [{"runs": [{"text": "Start with "}, {"text": "ownership", "style": {"bold": true, "color": "@accent2"}}, {"text": ", not decoration."}]}]}
            }
        }
    }]})).unwrap();
    let elements = result["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
    let message = elements.iter().find(|element| element["id"] == "emphasis-message").unwrap();
    assert_eq!(message["type"], "text");
    assert_eq!(message["text"], "Start with ownership, not decoration.");
    assert_eq!(message["format"]["paragraphs"][0]["runs"][1]["style"]["color"], "@accent2");
    assert_eq!(message["format"]["paragraphs"][0]["runs"][1]["style"]["bold"], true);
    for index in [0, 2] {
        assert_ne!(message["format"]["paragraphs"][0]["runs"][index]["style"]["color"], "@accent2", "normal text around explicit emphasis must keep the body color");
        assert_ne!(message["format"]["paragraphs"][0]["runs"][index]["style"]["bold"], true);
    }
}

#[test]
fn visual_quality_entity_nodes_share_label_and_detail_styles() {
    let document = create_with_setup("node-roles".into(), "Synthetic node roles".into(), &PresentationSetup::default()).unwrap();
    let input: RecipeInput = serde_json::from_value(json!({
        "version": 1, "title": "Synthetic definition and instance", "source_caption": "Synthetic fixture.", "takeaway": "Keep equivalent labels consistent.",
        "data": {"kind": "entity_instance_map", "definition": {"label": "Definition", "detail": "Common configuration"}, "relation_label": "Instantiates", "instances": [{"label": "Instance", "detail": "Local permissions"}]}
    })).unwrap();
    let result = apply(&document, &input, "definition-instances");
    let nodes = result["document"]["parts"][0]["spec"]["data"]["graph"]["nodes"].as_array().unwrap();
    for field in ["font_size", "detail_font_size", "text_align"] {
        assert_eq!(nodes[0][field], nodes[1][field], "same-purpose node field {field} must be consistent");
    }
}

#[test]
fn shared_profile_keeps_embedded_takeaways_and_conditions_in_the_same_band() {
    let document = create_with_setup("conditional-components".into(), "Synthetic conditional components".into(), &PresentationSetup::default()).unwrap();
    for (input, variant) in [(outcome_input(), "shift-rows"), (layer_input(), "annotated-layers")] {
        let mut value = serde_json::to_value(input).unwrap();
        value["component_profile"] = json!("technical_briefing");
        let input: RecipeInput = serde_json::from_value(value).unwrap();
        let result = apply(&document, &input, variant);
        let elements = result["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
        let message = elements.iter().find(|element| element["id"] == "recipe-message").expect("all profile takeaways must use the shared band");
        assert_eq!(message["y"].as_f64(), Some(580.0));
        assert_eq!(message["font_size"].as_f64(), Some(20.0));
        assert_eq!(message["format"]["vertical"], "middle");
        assert!(message["text"].as_str().unwrap().contains(&input.takeaway));
        if let recipes::RecipeData::ActivityOutcome { measurement_conditions, .. } = &input.data {
            assert!(message["text"].as_str().unwrap().contains(measurement_conditions));
            assert!(result["document"]["parts"][0]["spec"]["data"]["message"].is_null());
        }
        if variant == "annotated-layers" {
            let body = elements.iter().find(|element| element["id"] == "recipe-primary").unwrap();
            assert!(!body["children"].as_array().unwrap().iter().any(|element| {
                matches!(element["type"].as_str(), Some("shape" | "rect")) && element["fill"] != "none" && element["height"].as_f64().is_some_and(|height| height > 10.0)
            }), "short role labels must not expand into empty filled rectangles under the technical profile");
        }
    }
}

#[test]
fn component_profile_rejects_unsupported_block_mode_without_silent_ignoring() {
    let document = create_with_setup("profile-guard".into(), "Synthetic profile guard".into(), &PresentationSetup::default()).unwrap();
    let result = execute_request(json!({"op": "apply_operations", "document": document, "expected_revision": document.revision, "expected_hash": document.hash, "operations": [{
        "op": "compose_slide", "slide_id": "slide-1", "id": "unsupported", "spec": {
            "title": "Supplied title", "component_profile": "technical_briefing", "blocks": [{"kind": "callout", "text": "Supplied content"}]
        }
    }]}));
    assert!(result.unwrap_err().to_string().contains("component_profile requires pattern mode"));
}

#[test]
fn activity_outcome_keeps_definitions_and_measurement_conditions_visible() {
    let document = create_with_setup("outcome".into(), "Synthetic recipe".into(), &PresentationSetup::default()).unwrap();
    let input = outcome_input();
    let result = apply(&document, &input, "shift-rows");
    let slide = &result["document"]["deck"]["slides"][0];
    let content: String = slide["elements"].as_array().unwrap().iter().map(text).collect();
    for expected in ["Alerts handled", "Exposure addressed", "Specify the population and resolution rule.", "Reviews performed", "Access corrected", "Record the scope and completion criteria.", "Example only; period and population must be supplied."] {
        assert!(content.contains(expected), "missing visible recipe content: {expected}");
    }
    assert!(content.contains(&input.takeaway) && content.contains(&input.source_caption));
    assert_eq!(slide["notes"], input.notes);
    assert_eq!(result["document"]["parts"][0]["spec"]["preset"], "before-after/shift");
}

#[test]
fn every_variant_is_atomic_undoable_and_can_reopen_as_pptx() {
    for (input, variants) in [(layer_input(), vec!["diagram-table", "annotated-layers"]), (source_input(), vec!["visual-left", "visual-right"]), (outcome_input(), vec!["shift-rows"])] {
        for variant in variants {
            let document = create_with_setup("roundtrip".into(), "Synthetic recipes".into(), &PresentationSetup::default()).unwrap();
            let before = serde_json::to_value(&document).unwrap();
            let result = apply(&document, &input, variant);
            assert_eq!(serde_json::to_value(&document).unwrap(), before, "recipe preparation must not mutate its input");
            let undo = execute_request(json!({"op": "undo_transaction", "document": result["document"], "expected_revision": result["document"]["revision"], "receipt": result["receipt"]})).unwrap();
            assert!(undo["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().is_empty());
            assert_eq!(undo["document"]["deck"]["slides"][0]["notes"], "");
            let undone: Document = serde_json::from_value(undo["document"].clone()).unwrap();
            assert!(undone.parts.is_empty());
            let exported = execute_request(json!({"op": "export_presentation", "document": result["document"]})).unwrap();
            let opened = execute_request(json!({"op": "open_presentation", "id": "recipe-reopened", "base64": exported["base64"]})).unwrap();
            assert_eq!(opened["document"]["deck"]["slides"].as_array().unwrap().len(), 1);
            assert_eq!(opened["document"]["deck"]["slides"][0]["notes"], input.notes);
            if variant == "diagram-table" { assert_eq!(result["document"]["deck"]["slides"][0]["review"]["table_headers"]["recipe-right"], "first_row"); }
            let preview = execute_request(json!({"op": "preview_presentation", "document": result["document"], "options": {"page_indices": [0], "format": "png", "layout": "pages", "max_dimension": 640, "max_output_bytes": 2097152}})).unwrap();
            assert_eq!(preview["images"].as_array().unwrap().len(), 1);
            assert!(STANDARD.decode(preview["images"][0]["base64"].as_str().unwrap()).unwrap().starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
        }
    }
}

#[test]
fn recipes_reject_missing_conditions_duplicate_ids_stale_design_and_nonempty_targets() {
    let document = create_with_setup("guard".into(), "Synthetic recipes".into(), &PresentationSetup::default()).unwrap();
    let mut layer = layer_input();
    if let recipes::RecipeData::LayerRoleMap { layers, .. } = &mut layer.data { layers[1].id = layers[0].id.clone(); }
    assert!(recipes::prepare(&document.deck, &layer, "diagram-table").unwrap_err().to_string().contains("distinct"));
    let mut outcome = outcome_input();
    if let recipes::RecipeData::ActivityOutcome { measurement_conditions, .. } = &mut outcome.data { measurement_conditions.clear(); }
    assert!(recipes::prepare(&document.deck, &outcome, "shift-rows").unwrap_err().to_string().contains("measurement_conditions"));
    let input = source_input();
    let prepared = recipes::prepare(&document.deck, &input, "visual-left").unwrap();
    let mut changed = document.deck.clone(); changed.width += 8;
    assert!(prepared.operations(&changed, "slide-1", "recipe").unwrap_err().to_string().contains("design changed"));
    let result = apply(&document, &input, "visual-left");
    let populated: Document = serde_json::from_value(result["document"].clone()).unwrap();
    assert!(prepared.operations(&populated.deck, "slide-1", "recipe").unwrap_err().to_string().contains("new empty slide"));
    let mut too_long = input.clone(); too_long.takeaway = "long ".repeat(100);
    assert!(recipes::prepare(&document.deck, &too_long, "visual-left").is_err());
    assert!(serde_json::from_value::<RecipeInput>(json!({"version": 1, "unexpected": true})).is_err());
}

#[test]
fn recipes_keep_identical_text_in_distinct_visual_roles() {
    let document = create_with_setup("roles".into(), "Synthetic recipes".into(), &PresentationSetup::default()).unwrap();
    let mut layer = layer_input();
    if let recipes::RecipeData::LayerRoleMap { layers, .. } = &mut layer.data {
        for item in layers { item.label = "Operations".into(); item.role = "Operations".into(); }
    }
    let result = apply(&document, &layer, "annotated-layers");
    let content: String = result["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().map(text).collect();
    assert_eq!(content.matches("Operations").count(), 6);
    let mut outcome = outcome_input();
    if let recipes::RecipeData::ActivityOutcome { changes, .. } = &mut outcome.data {
        for change in changes { change.activity = "Retention".into(); change.outcome = "Retention".into(); change.definition = "Retention".into(); }
    }
    let result = apply(&document, &outcome, "shift-rows");
    let content: String = result["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().map(text).collect();
    assert_eq!(content.matches("Retention").count(), 6);
}

#[test]
fn recipes_reject_notes_only_targets_without_erasing_them() {
    let document = create_with_setup("notes".into(), "Synthetic recipes".into(), &PresentationSetup::default()).unwrap();
    let prepared = recipes::prepare(&document.deck, &layer_input(), "diagram-table").unwrap();
    let mut deck = document.deck.clone();
    deck.slides[0].notes = "Existing speaker notes must not be replaced.".into();
    let before = serde_json::to_value(&deck).unwrap();
    assert!(matches!(prepared.operations(&deck, "slide-1", "recipe"), Err(aislide_core::Error::Conflict(_))));
    assert_eq!(serde_json::to_value(&deck).unwrap(), before);
}

#[test]
fn recipe_protocol_previews_without_mutation_and_applies_the_exact_candidate_once() {
    let document = create_with_setup("protocol-recipes".into(), "Synthetic recipes".into(), &PresentationSetup::default()).unwrap();
    let input = layer_input();
    let before = serde_json::to_value(&document).unwrap();
    let catalog = execute_request(json!({"op": "recipe_catalog"})).unwrap();
    assert_eq!(catalog["recipes"].as_array().unwrap().len(), 5);
    let recommendation = execute_request(json!({"op": "recommend_recipes", "intent": {"relationship": "layers", "item_count": 3}})).unwrap();
    assert_eq!(recommendation["candidates"].as_array().unwrap().len(), 2);
    let preview = execute_request(json!({"op": "preview_recipe", "document": document, "slide_id": "slide-1", "id": "recipe", "input": input, "variant": "diagram-table", "max_dimension": 640})).unwrap();
    assert_eq!(preview["base_revision"], document.revision);
    assert_eq!(preview["base_hash"], document.hash);
    assert_eq!(preview["recipe_id"], "layer-role-map");
    assert_eq!(preview["variant"], "diagram-table");
    assert_eq!(preview["semantic_truth_verified"], false);
    assert_eq!(preview["office_visual_parity_verified"], false);
    assert_eq!(preview["preview"]["images"].as_array().unwrap().len(), 1);
    assert!(STANDARD.decode(preview["preview"]["images"][0]["base64"].as_str().unwrap()).unwrap().starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
    assert_eq!(serde_json::to_value(&document).unwrap(), before);
    let request = json!({"op": "apply_recipe", "document": document, "expected_revision": document.revision, "expected_hash": document.hash, "slide_id": "slide-1", "id": "recipe", "input": input, "variant": "diagram-table", "candidate_hash": preview["candidate_hash"]});
    for (field, value) in [("expected_revision", json!(99)), ("expected_hash", json!("0".repeat(64))), ("candidate_hash", json!("0".repeat(64)))] {
        let mut invalid = request.clone(); invalid[field] = value;
        assert!(matches!(execute_request(invalid), Err(aislide_core::Error::Conflict(_))), "guard must reject {field}");
    }
    let mut changed_input = request.clone(); changed_input["input"]["title"] = json!("Changed after preview");
    assert!(matches!(execute_request(changed_input), Err(aislide_core::Error::Conflict(_))));
    let result = execute_request(request).unwrap();
    assert_eq!(result["document"]["hash"], preview["candidate_hash"]);
    assert_eq!(result["document"]["revision"], document.revision + 1);
    let undo = execute_request(json!({"op": "undo_transaction", "document": result["document"], "expected_revision": result["document"]["revision"], "receipt": result["receipt"]})).unwrap();
    assert_eq!(undo["document"]["hash"], document.hash);
    assert_eq!(serde_json::to_value(&document).unwrap(), before);
}

#[test]
fn recipes_keep_supplied_content_across_existing_design_presets() {
    for preset in ["public", "minimal", "stylish", "pop", "dynamic", "trust", "luxury"] {
        let document = create_with_setup("preset-recipes".into(), "Synthetic recipes".into(), &PresentationSetup { design_preset: Some(preset.into()), font_family: Some("Yu Gothic".into()), ..Default::default() }).unwrap();
        for (input, variants) in [(layer_input(), vec!["diagram-table", "annotated-layers"]), (source_input(), vec!["visual-left", "visual-right"]), (outcome_input(), vec!["shift-rows"])] {
            for variant in variants {
                recipes::prepare(&document.deck, &input, variant).unwrap_or_else(|error| panic!("{preset}/{variant}: {error}"));
                let result = apply(&document, &input, variant);
                let slide = &result["document"]["deck"]["slides"][0];
                let content: String = slide["elements"].as_array().unwrap().iter().map(text).collect();
                assert!(content.contains(&input.title), "{preset}/{variant} lost title");
                assert!(content.contains(&input.source_caption), "{preset}/{variant} lost source caption");
                assert!(content.contains(&input.takeaway), "{preset}/{variant} lost takeaway");
                assert_eq!(slide["notes"], input.notes, "{preset}/{variant} lost notes");
            }
        }
    }
}

#[test]
fn recipes_reject_body_that_cannot_fit_even_within_character_limits() {
    let document = create_with_setup("fit-recipes".into(), "Synthetic recipes".into(), &PresentationSetup::default()).unwrap();
    let mut input = layer_input();
    if let recipes::RecipeData::LayerRoleMap { layers, .. } = &mut input.data {
        *layers = (0..5).map(|index| recipes::LayerRole { id: format!("layer-{index}"), label: "WW".repeat(24), role: "WW".repeat(60) }).collect();
    }
    assert!(recipes::prepare(&document.deck, &input, "annotated-layers").is_err(), "bounded but overflowing body must not shrink or lose text");
}

#[test]
fn recipe_review_keeps_both_layer_headings_visible_and_bound_to_the_candidate() {
    let document = create_with_setup("heading-recipes".into(), "Synthetic recipes".into(), &PresentationSetup::default()).unwrap();
    let mut input = layer_input();
    if let recipes::RecipeData::LayerRoleMap { layer_heading, role_heading, .. } = &mut input.data {
        *layer_heading = "Supplied layers".into(); *role_heading = "Assigned work".into();
    }
    for variant in ["diagram-table", "annotated-layers"] {
        let result = apply(&document, &input, variant);
        let content: String = result["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().map(text).collect();
        assert!(content.contains("Supplied layers") && content.contains("Assigned work"), "{variant} must display both supplied headings");
        let exported = execute_request(json!({"op": "export_presentation", "document": result["document"]})).unwrap();
        let reopened = execute_request(json!({"op": "open_presentation", "id": "headings-reopened", "base64": exported["base64"]})).unwrap();
        let content: String = reopened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().map(text).collect();
        assert!(content.contains("Supplied layers") && content.contains("Assigned work"));
        let mut changed = input.clone();
        if let recipes::RecipeData::LayerRoleMap { role_heading, .. } = &mut changed.data { *role_heading = "Changed heading".into(); }
        let candidate = apply(&document, &changed, variant);
        assert_ne!(result["document"]["hash"], candidate["document"]["hash"], "supplied headings must affect the candidate");
    }
}

#[test]
fn recipe_review_rejects_table_content_that_requires_caption_size() {
    let document = create_with_setup("table-size-recipes".into(), "Synthetic recipes".into(), &PresentationSetup::default()).unwrap();
    let mut input = layer_input();
    if let recipes::RecipeData::LayerRoleMap { layers, .. } = &mut input.data { layers[0].role = ["Measure"; 13].join("\n"); }
    let error = recipes::prepare(&document.deck, &input, "diagram-table").unwrap_err();
    assert!(error.to_string().contains("recipe table would shrink"), "recipe tables must not silently fall back from the paired body scale to caption size: {error}");
}