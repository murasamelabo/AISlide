use aislide_core::execute_request;
use serde_json::{json, Value};

fn brief(profile: &str) -> Value {
    json!({"version":1,"profile_id":profile,"title":"Source-bound briefing","audience":"Decision owners","purpose":"Choose the first remediation step","governing_message":"We should prioritize the highest measured exposure.","language":"en","evidence":[{"id":"source-a","kind":"source","reference":"Provided synthetic test fixture, not factual business data","statement":"The fixture values are 12 and 24."}],"slides":[{"id":"evidence-slide","section":"Exposure","headline":"We should prioritize the larger exposure because it doubles the baseline.","sentence_form":"causal","pattern_id":"native-part","question":"Which exposure should be addressed first?","parent_message":"governing","transition":"because","parallel_basis":"exposure","part":{"version":1,"preset":"horizontal-bar-graph/labeled","title":"Exposure by area","subtitle":"Synthetic test fixture / units","data":{"kind":"chart","categories":["Baseline","Priority"],"series":[{"name":"Units","values":[12,24]}],"x_axis":"Units","y_axis":"Area"}},"support":[{"clause":"We should prioritize the larger exposure because it doubles the baseline.","body_paths":["/data/series/0/values"],"evidence_ids":["source-a"]}],"numbers":[{"path":"/data/series/0/values/0","value":12,"evidence_id":"source-a"},{"path":"/data/series/0/values/1","value":24,"evidence_id":"source-a"}]}]})
}

fn authoring_brief(profile: &str) -> Value {
    let mut input = brief(profile);
    input["slides"][0]["headline"] = json!("Review the evidence before proceeding.");
    input["slides"][0]["support"][0]["clause"] = input["slides"][0]["headline"].clone();
    input["slides"][0]["support"][0]["body_paths"] = json!(["/data/items"]);
    input["slides"][0]["numbers"] = json!([]);
    input["slides"][0]["part"] = json!({"version":1,"preset":"list-horizontal/balanced","title":"Stages","subtitle":"Review sequence","data":{"kind":"items","items":[{"label":"Check","detail":"Review"},{"label":"Act","detail":"Proceed"}]}});
    input
}

fn assert_body_floor(element: &Value, scale: f64, floor: f64) {
    if let Some(size) = element["font_size"].as_f64() {
        assert!(size * scale + 0.001 >= floor, "effective size {} below {floor}: {element}", size * scale);
    }
    fn styles(value: &Value, scale: f64, floor: f64) {
        match value {
            Value::Object(fields) => {for (name,value) in fields {
                if name=="font_size" {if let Some(size)=value.as_f64() {assert!(size*scale+0.001>=floor,"rich/cell size {size} below effective floor {floor}");}}
                styles(value,scale,floor);
            }}
            Value::Array(values) => for value in values {styles(value,scale,floor);},
            _ => (),
        }
    }
    styles(&element["format"],scale,floor);
    if let Some(children) = element["children"].as_array() {
        let child_scale = scale * (element["width"].as_f64().unwrap() / element["view_width"].as_f64().unwrap())
            .min(element["height"].as_f64().unwrap() / element["view_height"].as_f64().unwrap());
        for child in children { assert_body_floor(child, child_scale, floor); }
    }
}

#[test]
fn guided_authoring_defaults_distinguish_reading_and_projection() {
    for (profile, floor, headline_size) in [("status-report",16.0,34.0),("technical-explainer",16.0,34.0),("consulting-decision",16.0,34.0),("event-talk",24.0,40.0)] {
        let mut input = authoring_brief(profile);
        input["authoring"] = json!({});
        let result = execute_request(json!({"op":"create_guided_presentation","id":"authoring-defaults","input":input}));
        assert!(result.is_ok(), "authoring settings should compile: {result:?}");
        let created = result.unwrap();
        let elements = created["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
        assert_eq!(elements.iter().find(|element|element["id"]=="headline").unwrap()["font_size"], headline_size);
        assert_body_floor(elements.iter().find(|element|element["id"]=="guided-part-1").unwrap(), 1.0, floor);
        assert_eq!(created["document"]["parts"][0]["stale"], false);
    }
}

#[test]
fn guided_authoring_omitted_preserves_legacy_rendering_and_notes() {
    for profile in ["status-report","event-talk"] {for double in [false,true] {
        let mut input=authoring_brief(profile);
        if double {
            input["slides"][0]["headline"]=json!("The measured baseline needs another review because current evidence does not establish the expected operational benefit.");
            input["slides"][0]["support"][0]["clause"]=input["slides"][0]["headline"].clone();
        }
        let created=execute_request(json!({"op":"create_guided_presentation","id":"legacy","input":input})).unwrap();
        let slide=&created["document"]["deck"]["slides"][0];
        let elements=slide["elements"].as_array().unwrap();
        let headline=elements.iter().find(|element|element["id"]=="headline").unwrap();
        assert_eq!(headline["font_size"],if double {29.0} else {32.0});
        assert_eq!(headline["height"],if double {86.0} else {52.0});
        assert_eq!(elements[0]["font_size"],18.0);
        assert_eq!(elements[2]["font_size"],12.0);
        let mut expected=execute_request(json!({"op":"create_part","id":"guided-part-1","spec":input["slides"][0]["part"],"theme":created["document"]["deck"]["design"]["theme"]})).unwrap();
        if expected.get("element").is_some() {expected=expected["element"].clone();}
        let height=if double {474.0} else {498.0};
        fn scale(value:&mut Value,factor:f64) {
            for field in ["y","height","view_height"] {if let Some(number)=value[field].as_f64() {value[field]=json!(number*factor);}}
            if let Some(size)=value["font_size"].as_f64() {value["font_size"]=json!((size*factor).clamp(8.0,120.0).max(12.0));}
            if let Some(size)=value["stroke_width"].as_f64().filter(|size|*size>0.0) {value["stroke_width"]=json!((size*factor).max(0.5));}
            if let Some(children)=value.get_mut("children").and_then(Value::as_array_mut) {for child in children {scale(child,factor);}}
        }
        for child in expected["children"].as_array_mut().unwrap() {
            scale(child,height/512.0);
            if profile=="event-talk" && child["type"]=="text" && child["y"].as_f64().unwrap()>70.0 && child["height"].as_f64().unwrap()>=40.0 {child["font_size"]=json!(child["font_size"].as_f64().unwrap().max(22.0));}
        }
        expected["height"]=json!(height);expected["view_height"]=json!(height);expected["y"]=json!(if double {168.0} else {144.0});
        assert_eq!(elements.iter().find(|element|element["id"]=="guided-part-1").unwrap(),&expected);
        let notes=slide["notes"].as_str().unwrap();
        assert!(!notes.contains("speaker_notes"));
        assert!(notes.ends_with("Semantic truth and Office parity require human review."));
        input["authoring"]=Value::Null;
        let null_settings=execute_request(json!({"op":"create_guided_presentation","id":"legacy","input":input})).unwrap();
        assert_eq!(created,null_settings);
    }}
}

#[test]
fn guided_authoring_overrides_are_visible_and_native_metadata_roundtrips() {
    for (profile,context,floor,headline_size) in [("status-report","projection",24.0,40.0),("event-talk","reading",16.0,34.0)] {
        let mut input=authoring_brief(profile);
        input["authoring"]=json!({"context":context});
        let created=execute_request(json!({"op":"create_guided_presentation","id":"context-override","input":input})).unwrap();
        let elements=created["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
        assert_eq!(elements[1]["font_size"],headline_size);
        assert_body_floor(&elements[4],1.0,floor);
    }
    let mut input=authoring_brief("status-report");
    input["authoring"]=json!({"body_font_min":24,"headline_font_size":48,"font_family":"Arial","density":"compact","spacing":"relaxed"});
    input["brand_color"]=json!("A12233");
    input["slides"][0]["part"]["data"]["items"][0]["detail"]=json!("Review\nThen act");
    let created=execute_request(json!({"op":"create_guided_presentation","id":"explicit-authoring","input":input})).unwrap();
    let deck=&created["document"]["deck"];
    assert_eq!(deck["design"]["theme"]["colors"]["accent1"],"A12233");
    for role in ["major","minor","east_asian","complex_script"] {assert_eq!(deck["design"]["theme"]["fonts"][role],"Arial");}
    let elements=deck["slides"][0]["elements"].as_array().unwrap();
    assert_eq!(elements[1]["font_size"],48.0);
    assert_eq!(elements[4]["x"],48.0);assert_eq!(elements[4]["width"],1184.0);
    assert_body_floor(&elements[4],1.0,24.0);
    let detail=elements[4]["children"].as_array().unwrap().iter().find(|child|child["text"]=="Review\nThen act").unwrap();
    assert_eq!(detail["format"]["font_family"],"Arial");
    assert_eq!(detail["format"]["paragraphs"][0]["line_spacing"],json!({"kind":"percent","value":130000}));
    assert_eq!(detail["format"]["paragraphs"][0]["space_after"],json!({"kind":"percent","value":0}));
    let mut comfortable=input.clone();comfortable["authoring"]["density"]=json!("comfortable");comfortable["authoring"]["spacing"]=json!("standard");
    let alternative=execute_request(json!({"op":"create_guided_presentation","id":"comfortable-authoring","input":comfortable})).unwrap();
    assert_ne!(deck["slides"][0]["elements"],alternative["document"]["deck"]["slides"][0]["elements"]);
    let exported=execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"authoring-reopened","base64":exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["parts"][0]["stale"],false);
    assert_eq!(reopened["document"]["parts"][0]["spec"],created["document"]["parts"][0]["spec"]);
    let part=reopened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().find(|element|element["id"]=="guided-part-1").unwrap();
    assert_body_floor(part,1.0,24.0);
    assert!(part["children"].as_array().unwrap().iter().any(|child|child["type"]=="text"));
}

#[test]
fn guided_authoring_speaker_notes_append_once_and_preserve_evidence() {
    let mut input=authoring_brief("status-report");
    let original=execute_request(json!({"op":"create_guided_presentation","id":"notes","input":input})).unwrap();
    let speaker="\u{1f4ac}".repeat(4000);
    input["slides"][0]["speaker_notes"]=json!(speaker);
    let created=execute_request(json!({"op":"create_guided_presentation","id":"notes","input":input})).unwrap();
    let expected=format!("{}\nSpeaker notes:\n{speaker}",original["document"]["deck"]["slides"][0]["notes"].as_str().unwrap());
    assert_eq!(created["document"]["deck"]["slides"][0]["notes"],expected);
    assert_eq!(created["document"]["parts"],original["document"]["parts"]);
    let exported=execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"notes-reopened","base64":exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][0]["notes"],expected);
    assert!(expected.contains("Ledger:") && expected.contains("Evidence:") && expected.contains("source-a"));
    input["slides"][0]["speaker_notes"]=json!(format!("{speaker}x"));
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
    input["slides"][0]["speaker_notes"]=json!(speaker);
    input["evidence"]=json!((0..4).map(|index|json!({"id":format!("source-{index}"),"kind":"source","reference":"Synthetic fixture","statement":"s".repeat(1200)})).collect::<Vec<_>>());
    input["slides"][0]["support"][0]["evidence_ids"]=json!(["source-0","source-1","source-2","source-3"]);
    let checked=execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
    assert_eq!(checked["ready"],false);
    assert!(checked["issues"].to_string().contains("8000"),"{checked}");
}

#[test]
fn guided_authoring_invalid_settings_reject_without_fallback() {
    for settings in [json!({"context":"screen"}),json!({"density":"dense"}),json!({"spacing":"wide"}),json!({"extra":true}),json!({"body_font_min":11.99}),json!({"body_font_min":40.01}),json!({"headline_font_size":27.99}),json!({"headline_font_size":64.01}),json!({"font_family":""}),json!({"font_family":" "}),json!({"font_family":"a".repeat(101)}),json!({"font_family":"bad\nfont"}),json!({"body_font_min":"24"})] {
        let mut input=authoring_brief("status-report");input["authoring"]=settings.clone();
        let checked=execute_request(json!({"op":"validate_guided_presentation","input":input}));
        assert!(checked.as_ref().map_or(true,|value|value["ready"]==false),"{settings}: {checked:?}");
        assert!(execute_request(json!({"op":"create_guided_presentation","id":"invalid-settings","input":input})).is_err(),"{settings}");
    }
}

#[test]
fn guided_authoring_table_cells_keep_effective_floor_and_native_editability() {
    let mut input=authoring_brief("status-report");
    input["authoring"]=json!({"context":"projection","body_font_min":28,"font_family":"Arial","spacing":"relaxed"});
    input["slides"][0]["part"]=json!({"version":1,"preset":"matrix/labeled","title":"Review","subtitle":"Decisions","data":{"kind":"matrix","rows":["Now","Next"],"columns":["A","B"],"cells":[["Yes","No"],["No","Yes"]]}});
    input["slides"][0]["support"][0]["body_paths"]=json!(["/data/cells"]);
    let result=execute_request(json!({"op":"create_guided_presentation","id":"table-authoring","input":input})).unwrap();
    let part=&result["document"]["deck"]["slides"][0]["elements"][4];
    assert_body_floor(part,1.0,28.0);
    let table=part["children"].as_array().unwrap().iter().find(|child|child["type"]=="table").unwrap();
    let cells=table["format"]["cells"].as_array().unwrap();assert_eq!(cells.len(),9);
    for cell in cells {assert_eq!(cell["style"]["text_style"]["font_family"],"Arial");assert_eq!(cell["style"]["text_format"]["paragraphs"][0]["line_spacing"]["value"],130000);}
    let exported=execute_request(json!({"op":"export_presentation","document":result["document"]})).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"table-authoring-reopened","base64":exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["parts"][0]["stale"],false);
    let part=&reopened["document"]["deck"]["slides"][0]["elements"][4];
    assert_body_floor(part,1.0,28.0);
    assert!(part["children"].as_array().unwrap().iter().any(|child|child["type"]=="table"));
}

#[test]
fn guided_authoring_rejects_rich_table_overflow_without_shrinking() {
    let mut input=authoring_brief("status-report");
    input["authoring"]=json!({"body_font_min":28,"spacing":"relaxed","font_family":"Arial"});
    input["slides"][0]["part"]=json!({"version":1,"preset":"matrix/labeled","title":"Review","subtitle":"Decisions","data":{"kind":"matrix","rows":["Now","Next"],"columns":["A","B"],"cells":[["A\nB\nC","No"],["No","Yes"]]}});
    input["slides"][0]["support"][0]["body_paths"]=json!(["/data/cells"]);
    let checked=execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
    assert_eq!(checked["ready"],false,"relaxed cell typography must be measured: {checked}");
    assert!(checked["issues"].to_string().contains("TEXT_OVERFLOW"),"{checked}");
    assert!(execute_request(json!({"op":"create_guided_presentation","id":"overflow","input":input})).is_err());
}

#[test]
fn guided_authoring_schema_exposes_only_the_locked_settings() {
    let guide=execute_request(json!({"op":"best_practice_guide","profile_id":"status-report"})).unwrap();
    let schema=&guide["input_schema"];
    let definitions=schema.get("$defs").or_else(||schema.get("definitions")).unwrap();
    let settings=&definitions["Authoring"];
    assert_eq!(settings["additionalProperties"],false);
    assert_eq!(settings["properties"].as_object().unwrap().len(),6);
    assert_eq!(settings["properties"]["body_font_min"]["minimum"],12);
    assert_eq!(settings["properties"]["body_font_min"]["maximum"],40);
    assert_eq!(settings["properties"]["headline_font_size"]["minimum"],28);
    assert_eq!(settings["properties"]["headline_font_size"]["maximum"],64);
    assert_eq!(settings["properties"]["font_family"]["maxLength"],100);
    assert_eq!(definitions["GuidedSlide"]["properties"]["speaker_notes"]["maxLength"],4000);
}

#[test]
fn best_practice_profiles_expose_english_guidance_and_honest_pattern_capabilities() {
    let catalog = execute_request(json!({"op":"best_practice_profiles"})).unwrap();
    assert_eq!(catalog["profiles"].as_array().unwrap().len(), 4);
    for id in ["consulting-decision", "technical-explainer", "event-talk", "status-report"] {
        let guide = execute_request(json!({"op":"best_practice_guide","profile_id":id})).unwrap();
        assert_eq!(guide["language"], "en");
        assert!(guide["markdown"].as_str().unwrap().contains("Claim"));
        assert!(guide["markdown"].as_str().unwrap().contains("unknown"));
        assert!(guide["input_schema"].is_object());
        assert!(guide["semantic_truth_verified"] == false);
        if id == "consulting-decision" {
            assert_eq!(guide["patterns"].as_array().unwrap().len(), 48);
            assert!(guide["patterns"].as_array().unwrap().iter().any(|pattern|pattern["id"]=="C03" && pattern["capability"]=="native-template"));
            assert!(guide["patterns"].as_array().unwrap().iter().any(|pattern|pattern["capability"]=="guidance-only"));
            assert_eq!(guide["automatic_patterns"],json!(["native-part","C02","C03"]));
        } else {
            assert_eq!(guide["automatic_patterns"],json!(["native-part"]));
        }
    }
    assert!(execute_request(json!({"op":"best_practice_guide","profile_id":"../../secret"})).is_err());
}

#[test]
fn guided_creation_preserves_values_and_requires_numeric_and_clause_evidence() {
    for profile in ["consulting-decision", "technical-explainer", "event-talk", "status-report"] {
        let input = brief(profile);
        let checked = execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
        assert_eq!(checked["ready"], true, "{checked}");
        assert_eq!(checked["semantic_truth_verified"], false);
        let created = execute_request(json!({"op":"create_guided_presentation","id":"guided","input":input})).unwrap();
        assert_eq!(created["document"]["parts"].as_array().unwrap().len(), 1);
        assert_eq!(created["document"]["parts"][0]["spec"]["data"]["series"][0]["values"], json!([12.0,24.0]));
        assert!(created["document"]["deck"]["slides"][0]["notes"].as_str().unwrap().contains("source-a"));
        let exported = execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
        let opened = execute_request(json!({"op":"open_presentation","id":"reopened","base64":exported["base64"]})).unwrap();
        assert_eq!(opened["document"]["parts"][0]["stale"], false);
    }
    for case in ["missing-number", "wrong-number", "missing-support", "title-as-evidence", "empty-evidence", "unknown-reference", "unsupported-pattern", "invalid-geometry"] {
        let mut input = brief("status-report");
        match case {
            "missing-number" => input["slides"][0]["numbers"] = json!([]),
            "wrong-number" => input["slides"][0]["numbers"][0]["value"] = json!(999),
            "missing-support" => input["slides"][0]["support"] = json!([]),
            "title-as-evidence" => input["slides"][0]["support"][0]["body_paths"] = json!(["/title"]),
            "empty-evidence" => { input["slides"][0]["support"][0]["body_paths"] = json!(["/data/series"]); input["slides"][0]["part"]["data"]["series"] = json!([]); },
            "unknown-reference" => input["slides"][0]["numbers"][0]["evidence_id"] = json!("missing"),
            "unsupported-pattern" => input["slides"][0]["pattern_id"] = json!("unimplemented-special-chart"),
            _ => input["slides"][0]["part"]["data"]["series"][0]["values"] = json!([1]),
        }
        let checked = execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
        assert_eq!(checked["ready"], false, "{case}: {checked}");
        assert!(execute_request(json!({"op":"create_guided_presentation","id":"rejected","input":input})).is_err(), "{case}");
    }
}

#[test]
fn decision_templates_recover_the_same_issues_with_timing_and_criteria() {
    for count in [3,6] {
        let mut input=brief("consulting-decision");
        input["issues"]=json!((1..=count).map(|index|json!({"id":format!("Q{index}"),"question":"Scope approval","requested_decision":"Approve the scoped pilot","criterion":"Evidence reviewed","owner":"Review owner","due":"After approval","evidence_ids":["source-a"],"analysis_slide_ids":["evidence-slide"]})).collect::<Vec<_>>());
        let template=|id:&str,pattern:&str,headline:&str,form:&str|json!({"id":id,"section":"Decision","headline":headline,"sentence_form":form,"pattern_id":pattern,"question":"What should be approved?","parent_message":"governing","transition":"therefore","parallel_basis":"decision issue","support":[{"clause":headline,"body_paths":["/issues"],"evidence_ids":["source-a"]}],"numbers":[]});
        input["slides"]=json!([template("summary","C02","We should approve the scoped pilot before committing to rollout.","proposal"),input["slides"][0].clone(),template("closing","C03","We can proceed if owners confirm the evidence and acceptance criteria.","conditional")]);
        let created=execute_request(json!({"op":"create_guided_presentation","id":"decisions","input":input})).unwrap();
        let slides=created["document"]["deck"]["slides"].as_array().unwrap();
        for index in [0,2] {
            let table=slides[index]["elements"].as_array().unwrap().iter().find(|element|element["id"]=="decision-table").unwrap();
            assert_eq!(table["type"],"group","Decision templates need purpose-sized columns, not equal-width cells");
            let children=table["children"].as_array().unwrap();
            let heading=children.iter().find(|element|element["text"]=="Issue").unwrap();
            assert!(heading["height"].as_f64().unwrap()<=40.0);
            assert_eq!(children.iter().filter(|element|element["type"]=="text").count(),(count+1)*4);
            for issue in 1..=count {assert!(children.iter().any(|element|element["text"].as_str().is_some_and(|text|text.starts_with(&format!("Q{issue}:")))));}
            if index==0 {
                let evidence=children.iter().find(|element|element["text"]=="Evidence").unwrap();
                let pages=children.iter().find(|element|element["text"]=="Analysis pages").unwrap();
                assert!(evidence["width"].as_f64().unwrap()>pages["width"].as_f64().unwrap()*2.0);
            }
        }
        assert!(slides[0]["elements"].to_string().contains("The fixture values are 12 and 24."), "The summary must show evidence, not only an opaque reference id");
        assert!(slides[2].to_string().contains("Immediate execution schedule"));
        assert!(slides[2].to_string().contains("Evidence reviewed"));
        let exported=execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
        let reopened=execute_request(json!({"op":"open_presentation","id":"decisions-reopened","base64":exported["base64"]})).unwrap();
        assert_eq!(reopened["document"]["deck"]["slides"].as_array().unwrap().len(),3);
        for target in ["missing", "summary", "closing"] {
            let mut invalid=input.clone();invalid["issues"][0]["analysis_slide_ids"]=json!([target]);
            assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":invalid})).unwrap()["ready"],false,"{target} is not analysis evidence");
        }
    }
}

#[test]
fn unknown_quantities_remain_text_and_cannot_justify_a_numeric_chart() {
    let mut input=brief("status-report");
    input["evidence"][0]["kind"]=json!("unknown");
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
    input["slides"][0]["part"]=json!({"version":1,"preset":"list-horizontal/balanced","title":"Outstanding measurements","subtitle":"Values are not yet supplied","data":{"kind":"items","items":[{"label":"Volume","detail":"xx"},{"label":"Timing","detail":"xx"}]}});
    input["slides"][0]["numbers"]=json!([]);
    input["slides"][0]["support"][0]["body_paths"]=json!(["/data/items"]);
    let result=execute_request(json!({"op":"create_guided_presentation","id":"unknown-values","input":input})).unwrap();
    assert!(result["document"]["deck"]["slides"][0].to_string().contains("xx"));
    assert_eq!(result["document"]["parts"][0]["spec"]["data"]["items"][0]["value"],Value::Null);
    input["slides"][0]["support"][0]["clause"]=json!("Only half of the headline");
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
}

#[test]
fn guided_authoring_decision_bodies_reserve_citations_with_large_headlines() {
    let mut input=authoring_brief("consulting-decision");
    input["authoring"]=json!({"headline_font_size":48,"body_font_min":12});
    input["issues"]=json!((1..=3).map(|index|json!({"id":format!("Q{index}"),"question":"Scope","requested_decision":"Approve pilot","criterion":"Review evidence","owner":"Owner","due":"After approval","evidence_ids":["source-a"],"analysis_slide_ids":["evidence-slide"]})).collect::<Vec<_>>());
    let template=|id:&str,pattern:&str,headline:&str,form:&str|json!({"id":id,"section":"Decision","headline":headline,"sentence_form":form,"pattern_id":pattern,"question":"What should be approved?","parent_message":"governing","transition":"therefore","parallel_basis":"decision issue","support":[{"clause":headline,"body_paths":["/issues"],"evidence_ids":["source-a"]}],"numbers":[]});
    input["slides"]=json!([template("summary","C02","Approve the pilot.","proposal"),input["slides"][0].clone(),template("closing","C03","Confirm the conditions.","conditional")]);
    let result=execute_request(json!({"op":"create_guided_presentation","id":"large-decisions","input":input})).unwrap();
    for index in [0,2] {for element in result["document"]["deck"]["slides"][index]["elements"].as_array().unwrap().iter().skip(4) {
        assert!(element["y"].as_f64().unwrap()+element["height"].as_f64().unwrap()<=642.01,"body overlaps citation region: {element}");
    }}
}

#[test]
fn guided_headline_numbers_are_bounded_and_small_labels_keep_the_font_floor() {
    let mut input=brief("status-report");
    let headline="The measured baseline needs another review because current evidence does not establish the expected operational benefit.";
    input["slides"][0]["headline"]=json!(headline);input["slides"][0]["support"][0]["clause"]=json!(headline);
    input["slides"][0]["part"]["preset"]=json!("cycle/labeled");
    input["slides"][0]["part"]["data"]=json!({"kind":"items","center":"Review","items":[{"label":"Measure","detail":"Check the result"},{"label":"Assess","detail":"Confirm the context"},{"label":"Change","detail":"Agree the next step"}]});
    input["slides"][0]["support"][0]["body_paths"]=json!(["/data/items"]);input["slides"][0]["numbers"]=json!([]);
    let created=execute_request(json!({"op":"create_guided_presentation","id":"font-floor","input":input})).unwrap();
    fn check(elements:&[Value]) {for element in elements {if let Some(size)=element["font_size"].as_f64() {assert!(size>=12.0,"{element}");}if let Some(children)=element["children"].as_array(){check(children);}}}
    check(created["document"]["deck"]["slides"][0]["elements"].as_array().unwrap());
    input["profile_id"]=json!("consulting-decision");
    input["slides"][0]["headline"]=json!("We should address 12 delays across 4 teams within 30 days.");
    input["slides"][0]["support"][0]["clause"]=input["slides"][0]["headline"].clone();
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
}