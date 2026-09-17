use aislide_core::execute_request;
use serde_json::{json, Value};

fn brief(profile: &str) -> Value {
    json!({"version":1,"profile_id":profile,"title":"Source-bound briefing","audience":"Decision owners","purpose":"Choose the first remediation step","governing_message":"We should prioritize the highest measured exposure.","language":"en","evidence":[{"id":"source-a","kind":"source","reference":"Provided synthetic test fixture, not factual business data","statement":"The fixture values are 12 and 24."}],"slides":[{"id":"evidence-slide","section":"Exposure","headline":"We should prioritize the larger exposure because it doubles the baseline.","sentence_form":"causal","pattern_id":"native-part","question":"Which exposure should be addressed first?","parent_message":"governing","transition":"because","parallel_basis":"exposure","part":{"version":1,"preset":"horizontal-bar-graph/labeled","title":"Exposure by area","subtitle":"Synthetic test fixture / units","data":{"kind":"chart","categories":["Baseline","Priority"],"series":[{"name":"Units","values":[12,24]}],"x_axis":"Units","y_axis":"Area"}},"support":[{"clause":"We should prioritize the larger exposure because it doubles the baseline.","body_paths":["/data/series/0/values"],"evidence_ids":["source-a"]}],"numbers":[{"path":"/data/series/0/values/0","value":12,"evidence_id":"source-a"},{"path":"/data/series/0/values/1","value":24,"evidence_id":"source-a"}]}]})
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