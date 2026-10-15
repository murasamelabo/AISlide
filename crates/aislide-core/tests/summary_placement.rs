use aislide_core::{editing::{create_with_setup, PresentationSetup}, execute_request};
use serde_json::{json, Value};

fn document() -> Value {
    let setup = PresentationSetup { design_preset: Some("trust".into()), ..Default::default() };
    serde_json::to_value(create_with_setup("summary".into(), "Synthetic summary placement".into(), &setup).unwrap()).unwrap()
}

fn compose(document: &Value, spec: Value) -> Result<Value, String> {
    execute_request(json!({"op":"apply_operations","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],
        "operations":[{"op":"compose_slide","slide_id":"slide-1","id":"c","spec":spec}]})).map_err(|error| error.to_string())
}

fn elements(result: &Value) -> &Vec<Value> { result["document"]["deck"]["slides"][0]["elements"].as_array().unwrap() }

fn find<'a>(result: &'a Value, id: &str) -> &'a Value {
    elements(result).iter().find(|element| element["id"] == id).unwrap_or_else(|| panic!("missing element {id}"))
}

fn position(result: &Value, id: &str) -> usize {
    elements(result).iter().position(|element| element["id"] == id).unwrap_or_else(|| panic!("missing element {id}"))
}

/// The text frame of a part row; a managed part is one group whose children are the drawn elements.
fn row_text<'a>(result: &'a Value, label: &str) -> &'a Value {
    find(result, "c-primary")["children"].as_array().unwrap().iter()
        .find(|child| child["text"].as_str().is_some_and(|text| text.starts_with(label))).unwrap_or_else(|| panic!("missing row {label}"))
}

fn frame(element: &Value) -> [f64; 4] { ["x", "y", "width", "height"].map(|key| element[key].as_f64().unwrap()) }

fn sentence(phrase_style: Value) -> Value {
    json!([{"runs": [{"text": "Scope the "}, {"text": "limits", "style": phrase_style}, {"text": " before the rollout."}]}])
}

fn banded(style: &str, bold_only: bool) -> Result<Value, String> {
    let phrase = if bold_only { json!({"bold": true}) } else { json!({"bold": true, "color": "@accent2"}) };
    compose(&document(), json!({
        "title": "Supplied evidence", "component_profile": "technical_briefing", "pattern": {"id": "focus/full", "message_band": true},
        "slots": {
            "primary": {"kind": "table", "rows": [["Subject", "Scope"], ["Known subject", "Known scope"]]},
            "message": {"kind": "emphasis", "emphasis": {"style": style}, "paragraphs": sentence(phrase)}
        }
    }))
}

fn preflight_errors(result: &Value) -> Vec<Value> {
    let report = execute_request(json!({"op":"preflight_presentation","document":result["document"]})).unwrap();
    report["findings"].as_array().unwrap().iter().filter(|finding| finding["severity"] == "error").cloned().collect()
}

#[test]
fn closing_is_a_ruled_single_line_without_a_surface() {
    let result = banded("closing", false).unwrap();
    let message = find(&result, "c-message");
    let separator = find(&result, "c-message-separator");
    assert!(elements(&result).iter().all(|element| element["id"] != "c-message-surface"), "a closing line is never boxed");
    assert_eq!(message["font_size"].as_f64(), Some(28.0));
    assert_eq!(separator["height"].as_f64(), Some(2.0));
    assert_eq!(separator["fill"], "@accent2");
    assert!(frame(message)[1] >= frame(separator)[1] + 8.0, "the sentence starts below its rule");
    assert!(position(&result, "c-message-separator") < position(&result, "c-message"), "the rule must stay behind the text");
    assert_eq!(message["text"], "Scope the limits before the rollout.");
    assert!(preflight_errors(&result).is_empty());
}

#[test]
fn solid_reverses_the_text_on_the_ink_surface_and_stays_behind_it() {
    let result = banded("solid", true).unwrap();
    let surface = find(&result, "c-message-surface");
    let message = find(&result, "c-message");
    assert_eq!(surface["fill"], "@dk1");
    assert_eq!(message["color"], "@lt1");
    assert_eq!(message["font_size"].as_f64(), Some(24.0));
    assert!(position(&result, "c-message-surface") < position(&result, "c-message"), "an opaque surface added after its text would hide the text");
    let [sx, sy, sw, sh] = frame(surface);
    let [mx, my, mw, mh] = frame(message);
    assert!(mx >= sx && my >= sy && mx + mw <= sx + sw && my + mh <= sy + sh, "the text frame stays inside its surface");
    let runs = &message["format"]["paragraphs"][0]["runs"];
    assert_eq!(runs[1]["style"]["bold"], true, "a supplied bold phrase is preserved");
    assert!(runs[1]["style"]["color"].is_null() || runs[1]["style"]["color"] != "@accent2", "bold, not the accent green, marks a phrase on the ink surface");
    assert!(preflight_errors(&result).is_empty());
}

#[test]
fn solid_default_emphasis_uses_reversed_text_not_the_accent_green() {
    let result = compose(&document(), json!({
        "title": "Supplied evidence", "component_profile": "technical_briefing", "pattern": {"id": "focus/full", "message_band": true},
        "slots": {
            "primary": {"kind": "table", "rows": [["Subject", "Scope"], ["Known subject", "Known scope"]]},
            "message": {"kind": "emphasis", "emphasis": {"style": "solid"}, "paragraphs": [{"runs": [{"text": "Decide the owner first."}]}]}
        }
    })).unwrap();
    let run = &find(&result, "c-message")["format"]["paragraphs"][0]["runs"][0]["style"];
    assert_eq!(run["bold"], true);
    assert_eq!(run["color"], "@lt1");
}

#[test]
fn closing_and_solid_are_never_chosen_automatically() {
    for (strength, relationship) in [("subtle", "supporting"), ("normal", "supporting"), ("strong", "supporting"), ("normal", "standalone"), ("strong", "standalone"), ("normal", "contrast"), ("normal", "conditional"), ("strong", "conditional")] {
        let result = compose(&document(), json!({
            "title": "Supplied evidence", "component_profile": "technical_briefing", "pattern": {"id": "focus/full", "message_band": true},
            "slots": {
                "primary": {"kind": "table", "rows": [["Subject", "Scope"], ["Known subject", "Known scope"]]},
                "message": {"kind": "emphasis", "emphasis": {"strength": strength, "relationship": relationship}, "paragraphs": [{"runs": [{"text": "Keep the limit visible."}]}]}
            }
        })).unwrap();
        let message = find(&result, "c-message");
        assert_eq!(message["font_size"].as_f64(), Some(20.0), "{strength} {relationship} keeps the ordinary band size");
        if let Some(surface) = elements(&result).iter().find(|element| element["id"] == "c-message-surface") {
            assert_ne!(surface["fill"], "@dk1", "{strength} {relationship} must not become a dark band");
        }
    }
}

#[test]
fn a_conditional_qualifier_no_longer_receives_the_tinted_surface() {
    let result = compose(&document(), json!({
        "title": "Supplied evidence", "component_profile": "technical_briefing", "pattern": {"id": "focus/full", "message_band": true},
        "slots": {
            "primary": {"kind": "table", "rows": [["Subject", "Scope"], ["Known subject", "Known scope"]]},
            "message": {"kind": "emphasis", "emphasis": {"relationship": "conditional"}, "paragraphs": [{"runs": [{"text": "Applies to supplied subjects only."}]}]}
        }
    })).unwrap();
    assert!(elements(&result).iter().all(|element| element["id"] != "c-message-surface"), "a qualifier is a ruled line, not a surface");
    assert!(elements(&result).iter().any(|element| element["id"] == "c-message-separator"));
}

fn lead_slide(emphasis: Value, with_band: bool) -> Result<Value, String> {
    let mut slots = json!({
        "primary": {"kind": "table", "rows": [["Subject", "Scope"], ["Known subject", "Known scope"]]},
        "lead-message": {"kind": "emphasis", "emphasis": emphasis, "paragraphs": sentence(json!({"bold": true, "color": "@accent2"}))}
    });
    if with_band { slots["message"] = json!({"kind": "callout", "text": "Supplied closing note."}); }
    compose(&document(), json!({"title": "Supplied evidence", "component_profile": "technical_briefing", "pattern": {"id": "focus/full", "lead": true, "message_band": with_band}, "slots": slots}))
}

#[test]
fn a_lead_sits_directly_under_the_title_and_pushes_the_body_down() {
    let plain = compose(&document(), json!({"title": "Supplied evidence", "component_profile": "technical_briefing", "pattern": {"id": "focus/full"},
        "slots": {"primary": {"kind": "table", "rows": [["Subject", "Scope"], ["Known subject", "Known scope"]]}}})).unwrap();
    let result = lead_slide(json!({}), false).unwrap();
    let title = frame(find(&result, "c-title"));
    let lead = find(&result, "c-lead-message");
    let lead_frame = frame(lead);
    assert!(lead_frame[1] >= title[1] + title[3], "the lead starts below the title");
    assert!(lead_frame[1] - (title[1] + title[3]) <= 24.0, "the lead stays attached to the title");
    assert_eq!(lead["font_size"].as_f64(), Some(24.0));
    assert!(elements(&result).iter().all(|element| element["id"] != "c-lead-message-surface" && element["id"] != "c-lead-message-separator"), "an automatic lead is open text");
    let (plain_primary, led_primary) = (frame(find(&plain, "c-primary")), frame(find(&result, "c-primary")));
    assert!(led_primary[1] >= lead_frame[1] + lead_frame[3], "the body starts below the lead");
    assert!(led_primary[1] > plain_primary[1], "the lead takes its height from the body");
    let runs = &lead["format"]["paragraphs"][0]["runs"];
    assert_eq!(runs[1]["style"]["color"], "@accent2");
    assert_ne!(runs[0]["style"]["bold"], true, "a lead keeps its weight and lets supplied runs carry the emphasis");
    assert!(preflight_errors(&result).is_empty());
}

#[test]
fn a_ruled_lead_draws_its_rule_below_the_sentence() {
    let result = lead_slide(json!({"style": "rule"}), false).unwrap();
    let lead = frame(find(&result, "c-lead-message"));
    let separator = frame(find(&result, "c-lead-message-separator"));
    assert!(separator[1] >= lead[1] + lead[3] - 1.0, "the rule closes the lead instead of opening it");
    assert!(position(&result, "c-lead-message-separator") < position(&result, "c-lead-message"));
}

#[test]
fn a_lead_and_a_bottom_message_band_leave_the_body_between_them() {
    let result = lead_slide(json!({}), true).unwrap();
    let lead = frame(find(&result, "c-lead-message"));
    let primary = frame(find(&result, "c-primary"));
    let message = frame(find(&result, "c-message"));
    assert!(lead[1] + lead[3] <= primary[1]);
    assert!(primary[1] + primary[3] <= message[1]);
    assert!(preflight_errors(&result).is_empty());
}

#[test]
fn a_lead_accepts_only_open_or_ruled_emphasis() {
    for style in ["outline", "tint", "closing", "solid"] {
        let error = lead_slide(json!({"style": style}), false).unwrap_err();
        assert!(error.contains("inline or rule"), "{style}: {error}");
    }
}

#[test]
fn a_lead_is_unavailable_on_full_page_layouts_and_for_unknown_slots() {
    let error = compose(&document(), json!({"title": "Supplied title", "pattern": {"id": "structure/section", "lead": true}, "slots": {"number": {"kind": "label", "text": "01"}}})).unwrap_err();
    assert!(error.contains("lead is not available"), "{error}");
    let error = compose(&document(), json!({"title": "Supplied title", "component_profile": "technical_briefing", "pattern": {"id": "focus/full"},
        "slots": {"primary": {"kind": "table", "rows": [["A", "B"], ["C", "D"]]}, "lead-message": {"kind": "label", "text": "No lead requested."}}})).unwrap_err();
    assert!(error.contains("no slot lead-message"), "{error}");
}

fn rows_slide(emphasis: Value) -> Result<Value, String> {
    compose(&document(), json!({"title": "Supplied conditions", "component_profile": "technical_briefing", "pattern": {"id": "focus/full"}, "slots": {
        "primary": {"kind": "part", "spec": {"version": 1, "preset": "list/icon-rows", "title": "", "data": {"kind": "icon_rows", "body_size": 20, "rows": [
            {"label": "Scope", "detail": "Other vendors and managed identities are out of scope. Cover them with another control.", "detail_emphasis": emphasis},
            {"label": "Mode", "detail": "Review the report-only results first."}
        ]}}}
    }}))
}

#[test]
fn row_emphasis_marks_only_the_supplied_phrase_and_keeps_the_text() {
    let result = rows_slide(json!(["another control"])).unwrap();
    let detail = row_text(&result, "Scope");
    assert_eq!(detail["text"], "Scope\nOther vendors and managed identities are out of scope. Cover them with another control.");
    let runs = detail["format"]["paragraphs"][1]["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 3, "text before, the phrase, and the closing period");
    assert_eq!(runs[0]["text"], "Other vendors and managed identities are out of scope. Cover them with ");
    assert_ne!(runs[0]["style"]["bold"], true);
    assert_eq!(runs[1]["text"], "another control");
    assert_eq!(runs[1]["style"]["bold"], true);
    assert_eq!(runs[1]["style"]["color"], "@accent2");
    assert_eq!(runs[2]["text"], ".");
    assert_ne!(runs[2]["style"]["bold"], true);
    assert!(preflight_errors(&result).is_empty());
}

#[test]
fn row_emphasis_without_phrases_keeps_the_single_run_row() {
    let result = rows_slide(json!([])).unwrap();
    let detail = row_text(&result, "Scope");
    assert_eq!(detail["format"]["paragraphs"][1]["runs"].as_array().unwrap().len(), 1);
}

#[test]
fn row_emphasis_rejects_phrases_that_are_missing_empty_overlapping_or_too_many() {
    for (phrase, expected) in [
        (json!(["not in the detail"]), "does not occur verbatim"),
        (json!([""]), "requires text"),
        (json!(["Cover them", "them with"]), "overlap"),
        (json!(["a", "b", "c", "d", "e"]), "at most four"),
    ] {
        let error = rows_slide(phrase.clone()).unwrap_err();
        assert!(error.contains(expected), "{phrase}: {error}");
    }
}

#[test]
fn icon_columns_refuse_row_emphasis_instead_of_ignoring_it() {
    let error = compose(&document(), json!({"title": "Supplied columns", "component_profile": "technical_briefing", "pattern": {"id": "focus/full"}, "slots": {
        "primary": {"kind": "part", "spec": {"version": 1, "preset": "list-horizontal/icon-columns", "title": "", "data": {"kind": "icon_columns", "items": [
            {"label": "Scope", "detail": "Apply the supplied limit.", "detail_emphasis": ["supplied limit"]},
            {"label": "Mode", "detail": "Review the report first."}
        ]}}}
    }})).unwrap_err();
    assert!(error.contains("detail_emphasis"), "{error}");
}

#[test]
fn component_styles_publish_the_conclusion_guide() {
    let styles = execute_request(json!({"op": "component_styles", "deck": document()["deck"], "profile": "technical_briefing"})).unwrap();
    let variants = styles["emphasis_variants"].as_array().unwrap();
    let names: Vec<&str> = variants.iter().map(|variant| variant["style"].as_str().unwrap()).collect();
    assert_eq!(names, ["inline", "rule", "outline", "tint", "closing", "solid"]);
    for variant in variants {
        assert!(!variant["use_when"].as_str().unwrap().is_empty());
        assert!(!variant["avoid_when"].as_str().unwrap().is_empty(), "{} needs a stated limit", variant["style"]);
    }
    let placements = styles["message_placements"].as_array().expect("placement guidance");
    let ids: Vec<&str> = placements.iter().map(|placement| placement["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["lead", "bottom", "fold_into_body", "none"]);
    for placement in placements {
        assert!(!placement["how"].as_str().unwrap().is_empty());
        assert!(!placement["use_when"].as_str().unwrap().is_empty());
        assert!(!placement["avoid_when"].as_str().unwrap().is_empty());
    }
    assert_eq!(placements[0]["frame"]["height"].as_f64(), Some(56.0));
}
