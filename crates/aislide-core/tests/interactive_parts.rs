use aislide_core::execute_request;
use serde_json::{Value, json};

fn spec(template: &str) -> Value {
    json!({"version":1,"preset":format!("interactive/{template}"),"title":"Interactive example","subtitle":"Synthetic data",
        "data":{"kind":"interactive","spec":{"version":1,"template":template,"source":"Synthetic example, not measured",
            "groups":[{"id":"before","label":"Before"},{"id":"after","label":"After"}],
            "dimensions":["Purpose","Control"],"items":[
                {"id":"identity","label":"Identity","detail":"Check the identity and its owner.","action":"Verify access.",
                    "group_id":"before","value":120,"unit":"events","values":["Authentication","Review"],"metrics":[{"label":"Coverage","value":"80%","qualifier":"Synthetic, denominator 10"}]},
                {"id":"resource","label":"Resource","detail":"Check resource permissions.","action":"Use least privilege.",
                    "group_id":"after","value":12,"unit":"events","values":["Authorization","Restrict"]}
            ],"initial_state":{"selected_id":"identity"}}}})
}

#[test]
fn interactive_catalog_and_projection_are_readonly_and_bounded() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    assert_eq!(catalog["presets"].as_array().unwrap().iter().filter(|item| item["category"]=="interactive").count(), 5);
    for template in ["summary","dashboard","timeline","comparison","map"] {
        let input=spec(template);
        let first=execute_request(json!({"op":"preview_interactive","id":"test","spec":input})).unwrap();
        let second=execute_request(json!({"op":"preview_interactive","id":"test","spec":input,
            "state":{"selected_id":"resource","dimension":1,"scale":"logarithmic"}})).unwrap();
        assert_eq!(first["powerpoint_interactive"], false);
        assert_eq!(first["executes_imported_html"], false);
        assert_eq!(first["state"]["selected_id"], "identity");
        assert_eq!(second["state"]["selected_id"], "resource");
        assert_ne!(first["element"],second["element"]);
        assert_eq!(execute_request(json!({"op":"create_part","id":"test","spec":input})).unwrap(),first["element"]);
    }
    for invalid in [
        json!({"selected_id":"missing"}),json!({"selected_id":"identity","group_id":"after"}),
        json!({"selected_id":"identity","dimension":2}),json!({"selected_id":"identity","html":"<script/>"})
    ] {
        assert!(execute_request(json!({"op":"preview_interactive","id":"test","spec":spec("dashboard"),"state":invalid})).is_err());
    }
}

#[test]
fn interactive_parts_persist_defaults_not_transient_state_and_undo_in_one_step() {
    let input=spec("dashboard");
    let original=execute_request(json!({"op":"create_presentation","id":"test","title":"Interactive test"})).unwrap();
    let inserted=execute_request(json!({"op":"insert_part","document":original,"expected_revision":0,
        "slide_id":"slide-1","id":"part","spec":input})).unwrap();
    let exported=execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let opened=execute_request(json!({"op":"open_presentation","id":"reopened","base64":exported["base64"]})).unwrap();
    let normalized = serde_json::to_value(serde_json::from_value::<aislide_core::parts::PartSpec>(input.clone()).unwrap()).unwrap();
    assert_eq!(opened["document"]["parts"][0]["spec"]["data"],normalized["data"]);
    let before=opened["document"].clone();
    execute_request(json!({"op":"preview_interactive","id":"part","spec":before["parts"][0]["spec"],
        "state":{"selected_id":"resource","group_id":"after"}})).unwrap();
    assert_eq!(execute_request(json!({"op":"export_presentation","document":before})).unwrap()["base64"],exported["base64"]);
    let mut next=input;
    next["data"]["spec"]["initial_state"]=json!({"selected_id":"resource","group_id":"after","scale":"logarithmic"});
    let updated=execute_request(json!({"op":"update_part","document":before,"expected_revision":before["revision"],
        "slide_id":"slide-1","id":"part","spec":next})).unwrap();
    let undone=execute_request(json!({"op":"undo_transaction","document":updated["document"],
        "expected_revision":updated["document"]["revision"],"receipt":updated["receipt"]})).unwrap();
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"],exported["base64"]);
}

#[test]
fn interactive_content_rejects_code_references_and_invalid_data() {
    for (pointer,value) in [
        ("/data/spec/html",json!("<script>alert(1)</script>")),
        ("/data/spec/items/0/url",json!("https://example.test/")),
        ("/data/spec/items/1/id",json!("identity")),
        ("/data/spec/items/0/group_id",json!("missing")),
        ("/data/spec/items/0/value",json!(-1)),
        ("/data/spec/items/0/values",json!(["Wrong length"])),
        ("/data/spec/items/0/detail",json!("x".repeat(500))),
    ] {
        let mut bad=spec("dashboard");
        if let Some(slot)=bad.pointer_mut(pointer) { *slot=value; }
        else {
            let (parent,key)=pointer.rsplit_once('/').unwrap();
            bad.pointer_mut(parent).unwrap().as_object_mut().unwrap().insert(key.into(),value);
        }
        assert!(execute_request(json!({"op":"create_part","id":"test","spec":bad})).is_err(),"{pointer}");
    }

}

#[test]
fn every_interactive_catalog_example_projects_without_executable_content() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    for entry in catalog["presets"].as_array().unwrap().iter().filter(|entry|entry["category"]=="interactive") {
        let view=execute_request(json!({"op":"preview_interactive","id":"catalog","spec":entry["example"]})).unwrap();
        assert_eq!(view["element"]["type"],"group");
        assert!(view["element"]["children"].as_array().unwrap().iter().all(|child|["text","rect"].contains(&child["type"].as_str().unwrap())));
    }
}

#[test]
fn dashboard_fractional_and_zero_values_keep_the_declared_scale() {
    let mut input=spec("dashboard");
    input["data"]["spec"]["items"][0]["value"]=json!(0.5);
    input["data"]["spec"]["items"][1]["value"]=json!(0.25);
    let root="i".repeat(40);
    let view=execute_request(json!({"op":"preview_interactive","id":root,"spec":input})).unwrap();
    let widths:Vec<_>=view["element"]["children"].as_array().unwrap().iter()
        .filter(|element|element["type"]=="rect").map(|element|element["width"].as_f64().unwrap()).collect();
    assert_eq!(widths.len(),2);
    assert!((widths[0]-1152.0*0.58*0.26).abs()<0.001);
    assert!((widths[1]/widths[0]-0.5).abs()<0.001);
    input["data"]["spec"]["items"][0]["value"]=json!(0);
    input["data"]["spec"]["items"][1]["value"]=json!(0);
    let zero=execute_request(json!({"op":"preview_interactive","id":"zero","spec":input,
        "state":{"selected_id":"identity","scale":"logarithmic"}})).unwrap();
    assert!(zero["element"]["children"].as_array().unwrap().iter().all(|element|element["type"]!="rect"));
}

#[test]
fn map_group_headers_and_ungrouped_items_have_separate_tracks() {
    let mut input=spec("map");
    input["data"]["spec"]["items"].as_array_mut().unwrap().push(json!({
        "id":"ungrouped","label":"Standalone","detail":"An ungrouped item.",
        "values":["Independent","Review"]
    }));
    let view=execute_request(json!({"op":"preview_interactive","id":"map","spec":input})).unwrap();
    let children=view["element"]["children"].as_array().unwrap();
    for label in ["Before","After","Ungrouped"] {
        assert_eq!(children.iter().filter(|child|child["text"]==label).count(),1,"{label}");
    }
    let labels:Vec<_>=children.iter().filter(|child|child["type"]=="text" && child["x"].as_f64().unwrap()<500.0).collect();
    for (index,a) in labels.iter().enumerate() {
        for b in &labels[index+1..] {
            let bound=|item:&Value,key:&str|item[key].as_f64().unwrap();
            let overlap=bound(a,"x")<bound(b,"x")+bound(b,"width") && bound(a,"x")+bound(a,"width")>bound(b,"x")
                && bound(a,"y")<bound(b,"y")+bound(b,"height") && bound(a,"y")+bound(a,"height")>bound(b,"y");
            assert!(!overlap,"{} overlaps {}",a["text"],b["text"]);
        }
    }
}

#[test]
fn interactive_minimum_frame_follows_the_visible_title_band() {
    // Hiding the 80px title band lowers the required height by the same amount, so the body stays 340px at either minimum.
    for template in ["summary","dashboard","timeline","comparison","map"] {
        let at=|width:f64,height:f64,show_title:bool| {
            let mut input=spec(template);
            input["layout"]=json!({"x":0,"y":0,"width":width,"height":height,"show_title":show_title});
            execute_request(json!({"op":"create_part","id":"frame","spec":input})).map_err(|error|error.to_string())
        };
        assert!(at(960.0,480.0,true).is_ok(),"{template}: title shown at the minimum");
        assert!(at(960.0,400.0,false).is_ok(),"{template}: title hidden at the minimum");
        let titled=at(960.0,479.0,true).unwrap_err();
        assert!(titled.contains("at least 960x480") && titled.contains("960x400 when layout.show_title is false"),"{template}: {titled}");
        let hidden=at(960.0,399.0,false).unwrap_err();
        assert!(hidden.contains("at least 960x400") && !hidden.contains("960x480"),"{template}: {hidden}");
        for show_title in [true,false] {
            let narrow=at(959.0,720.0,show_title).unwrap_err();
            assert!(narrow.contains("at least 960x"),"{template}, show_title={show_title}: {narrow}");
        }
    }
}
