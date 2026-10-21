use aislide_core::execute_request;
use serde_json::json;
use base64::{Engine, engine::general_purpose::STANDARD};
use aislide_core::package::Package;

#[test]
fn interactive_titles_are_required_even_when_the_visible_title_is_hidden() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    for entry in catalog["presets"].as_array().unwrap().iter().filter(|entry|entry["category"]=="interactive") {
        for appearance in ["plain","showcase"] {
            for show_title in [true,false] {
                for title in [""," \t\n ","\u{3000}"] {
                    let mut spec=entry["example"].clone();
                    spec["title"]=json!(title);
                    spec["data"]["spec"]["appearance"]=json!(appearance);
                    spec["layout"]=json!({"x":0,"y":0,"width":1280,"height":720,"show_title":show_title});
                    for op in ["create_part","preview_interactive","compile_interactive"] {
                        let error=execute_request(json!({"op":op,"id":"blank-title","spec":spec})).expect_err("blank interactive title must not produce an unusable runtime");
                        assert!(error.to_string().contains("interactive.title"),"{op}: {error}");
                    }
                }
            }
        }
    }
}

#[test]
fn showcase_dashboard_heading_does_not_invent_a_count_unit() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    let original=&catalog["presets"].as_array().unwrap().iter().find(|entry|entry["id"]=="interactive/dashboard").unwrap()["example"];
    for unit in ["件","%","円","USD",""] {
        for scale in ["linear","logarithmic"] {
            let mut spec=original.clone();
            spec["data"]["spec"]["appearance"]=json!("showcase");
            spec["data"]["spec"]["initial_state"]["scale"]=json!(scale);
            spec["layout"]=json!({"x":0,"y":0,"width":1280,"height":720,"show_title":true});
            for item in spec["data"]["spec"]["items"].as_array_mut().unwrap() {item["unit"]=json!(unit);}
            let view=execute_request(json!({"op":"create_part","id":"units","spec":spec})).unwrap();
            let texts:Vec<_>=view["children"].as_array().unwrap().iter().filter_map(|child|child["text"].as_str()).collect();
            assert!(!texts.iter().any(|text|text.starts_with("件数 /")),"unit {unit}: {texts:?}");
            assert!(texts.contains(&if scale=="linear" {"線形スケール"} else {"対数スケール log(1 + 値)"}));
            if !unit.is_empty() {assert!(texts.iter().any(|text|text.ends_with(&format!(" {unit}"))));}
        }
    }
}

#[test]
fn javascript_lab_export_accepts_only_explicit_trusted_kinds_and_preserves_source() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    let mut spec=catalog["presets"].as_array().unwrap().iter().find(|entry|entry["id"]=="interactive/summary").unwrap()["example"].clone();
    spec["data"]["spec"]["appearance"]=json!("showcase");
    spec["title"]=json!("JavaScript実験 </script>");
    spec["layout"]=json!({"x":0,"y":0,"width":1280,"height":720,"show_title":true});
    let document=execute_request(json!({"op":"create_presentation","id":"js-lab","title":"Synthetic JavaScript lab"})).unwrap();
    let inserted=execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide-1","id":"lab","spec":spec})).unwrap();
    let document=&inserted["document"];
    let before=execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    for kind in ["simulator","network","priority","playback","comparison","quiz","explorer","canvas"] {
        let request=json!({"op":"export_copilot_interactive","document":document,"acknowledged_experimental":true,
            "javascript_labs":[{"slide_id":"slide-1","element_id":"lab","kind":kind}]});
        let result=execute_request(request).unwrap();
        assert_eq!(result["javascript_lab_components"],1);
        let package=Package::open(STANDARD.decode(result["base64"].as_str().unwrap()).unwrap()).unwrap();
        let xml=roxmltree::Document::parse(package.text("ppt/webextensions/webextension1.xml").unwrap()).unwrap();
        let html:String=serde_json::from_str(xml.descendants().find(|node|node.tag_name().name()=="property").unwrap().attribute("value").unwrap()).unwrap();
        assert!(html.contains("function mountJavascriptLab"));
        assert!(html.contains(&format!("\"kind\":\"{kind}\"")));
        assert!(html.contains("\\u003c/script\\u003e"));
        assert_eq!(html.matches("</script>").count(),1);
        assert!(html.contains("connect-src 'none'"));
        assert!(!html.contains("export function mountJavascriptLab"));
        assert!(!html.contains("appsforoffice.microsoft.com"));
        assert!(html.contains("\"canvas\":{\"height\":720.0,\"width\":1280.0}"));
    }
    for selections in [
        json!([{"slide_id":"slide-1","element_id":"lab","kind":"arbitrary_script"}]),
        json!([{"slide_id":"slide-1","element_id":"missing","kind":"simulator"}]),
        json!([{"slide_id":"slide-1","element_id":"lab","kind":"simulator","javascript":"alert(1)"}]),
        json!([{"slide_id":"slide-1","element_id":"lab","kind":"simulator"},{"slide_id":"slide-1","element_id":"lab","kind":"canvas"}]),
    ] {
        assert!(execute_request(json!({"op":"export_copilot_interactive","document":document,"acknowledged_experimental":true,"javascript_labs":selections})).is_err());
    }
    assert_eq!(execute_request(json!({"op":"export_presentation","document":document})).unwrap()["base64"],before["base64"]);
    let mut four_three=spec.clone();
    four_three["layout"]=json!({"x":0,"y":0,"width":1200,"height":900,"show_title":true});
    let target=execute_request(json!({"op":"new_document","id":"four-three-lab","deck":{"version":1,"title":"4:3 lab","width":1200,"height":900,
        "slides":[{"id":"slide-1","title":"4:3 lab","background":"FFFFFF","notes":"","elements":[]}]}})).unwrap();
    let inserted=execute_request(json!({"op":"insert_part","document":target,"expected_revision":0,"slide_id":"slide-1","id":"lab","spec":four_three})).unwrap();
    let exported=execute_request(json!({"op":"export_copilot_interactive","document":inserted["document"],"acknowledged_experimental":true,
        "javascript_labs":[{"slide_id":"slide-1","element_id":"lab","kind":"simulator"}]})).unwrap();
    let package=Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml=roxmltree::Document::parse(package.text("ppt/webextensions/webextension1.xml").unwrap()).unwrap();
    let html:String=serde_json::from_str(xml.descendants().find(|node|node.tag_name().name()=="property").unwrap().attribute("value").unwrap()).unwrap();
    assert!(html.contains("\"canvas\":{\"height\":900.0,\"width\":1200.0}"));
}

#[test]
fn showcase_appearance_compiles_native_views_without_changing_plain_defaults() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    for entry in catalog["presets"].as_array().unwrap().iter().filter(|entry|entry["category"]=="interactive") {
        let plain=execute_request(json!({"op":"compile_interactive","id":"legacy","spec":entry["example"]})).unwrap();
        let mut spec=entry["example"].clone();
        spec["data"]["spec"]["appearance"]=json!("showcase");
        spec["layout"]=json!({"x":0,"y":0,"width":1280,"height":720,"show_title":true});
        let compiled=execute_request(json!({"op":"compile_interactive","id":"showcase","spec":spec})).unwrap();
        assert_eq!(compiled["views"].as_array().unwrap().len(),plain["views"].as_array().unwrap().len());
        for view in compiled["views"].as_array().unwrap() {
            let children=view["element"]["children"].as_array().unwrap();
            assert!(children.len()<=128);
            assert!(children.iter().any(|child|child["type"]=="rect" && child["fill"]=="@dk1"));
            assert!(children.iter().all(|child|["text","rect"].contains(&child["type"].as_str().unwrap())));
            let targets=view["hotspots"].as_array().unwrap();
            assert!(!targets.is_empty() && targets.len()<=8);
            for target in targets {
                let n=|key:&str|target[key].as_f64().unwrap();
                assert!(n("x")>=0.0 && n("y")>=0.0 && n("width")>0.0 && n("height")>0.0);
                assert!(n("x")+n("width")<=1280.0 && n("y")+n("height")<=720.0);
            }
            assert_eq!(view["controls_frame"],json!({"x":32.0,"y":88.0,"width":1216.0,"height":40.0}));
        }
        assert!(entry["example"]["data"]["spec"].get("appearance").is_none());
        spec["layout"]["height"]=json!(512);
        assert!(execute_request(json!({"op":"create_part","id":"small","spec":spec})).is_err());
    }
}

#[test]
fn copilot_export_matches_observed_reference_and_json_string_without_changing_source() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    let mut spec=catalog["presets"].as_array().unwrap().iter().find(|entry|entry["id"]=="interactive/dashboard").unwrap()["example"].clone();
    spec["data"]["spec"]["source"]=json!("</script><img src=x onerror=globalThis.injected=true>");
    let original=execute_request(json!({"op":"create_presentation","id":"copilot-export","title":"Synthetic Copilot format experiment"})).unwrap();
    let inserted=execute_request(json!({"op":"insert_part","document":original,"expected_revision":0,"slide_id":"slide-1","id":"experiment","spec":spec})).unwrap();
    let document=&inserted["document"];
    let before=execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    assert!(execute_request(json!({"op":"export_copilot_interactive","document":document,"acknowledged_experimental":false})).is_err());
    let result=execute_request(json!({"op":"export_copilot_interactive","document":document,"acknowledged_experimental":true})).unwrap();
    assert_eq!(result["catalog_id"],"fa000000210");
    assert_eq!(result["experimental"],true);
    assert_eq!(result["public_authoring_api_verified"],false);
    assert_eq!(result["powerpoint_playback_verified"],false);
    assert!(result.get("manifest").is_none());
    let package=Package::open(STANDARD.decode(result["base64"].as_str().unwrap()).unwrap()).unwrap();
    let extension=roxmltree::Document::parse(package.text("ppt/webextensions/webextension1.xml").unwrap()).unwrap();
    let reference=extension.descendants().find(|node|node.tag_name().name()=="reference").unwrap();
    assert_eq!(reference.attribute("id"),Some("fa000000210"));
    assert_eq!(reference.attribute("version"),Some("1.0.0.0"));
    assert_eq!(reference.attribute("store"),Some("en-US"));
    assert_eq!(reference.attribute("storeType"),Some("sdxcatalog"));
    let properties:Vec<_>=extension.descendants().filter(|node|node.tag_name().name()=="property").collect();
    assert_eq!(properties.len(),1);
    assert_eq!(properties[0].attribute("name"),Some("contentBlockHtml"));
    let html:String=serde_json::from_str(properties[0].attribute("value").unwrap()).unwrap();
    assert!(html.starts_with("<!doctype html>"));
    assert_eq!(html.matches("<script>").count(),1);
    assert_eq!(html.matches("</script>").count(),1);
    assert!(!html.contains("<img src=x"));
    assert!(html.contains("\\u003c/script\\u003e"));
    assert!(html.contains("connect-src 'none'"));
    assert!(!html.contains("export function mountInteractive"));
    assert!(!html.contains("office-boot.mjs"));
    assert!(!html.contains("appsforoffice.microsoft.com"));
    assert!(package.part("ppt/media/aislide-interactive-1.png").is_ok());
    assert_eq!(execute_request(json!({"op":"export_presentation","document":document})).unwrap()["base64"],before["base64"]);
    let reopened=execute_request(json!({"op":"open_presentation","id":"inert-copilot","base64":result["base64"]})).unwrap();
    assert_eq!(execute_request(json!({"op":"export_presentation","document":reopened["document"]})).unwrap()["base64"],result["base64"]);
}

#[test]
fn office_components_compile_every_allowed_control_state_using_the_shared_core() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    for entry in catalog["presets"].as_array().unwrap().iter().filter(|entry|entry["category"]=="interactive") {
        let spec=&entry["example"];
        let payload=execute_request(json!({"op":"compile_interactive","id":"office","spec":spec})).unwrap();
        let views=payload["views"].as_array().unwrap();
        assert!(!views.is_empty() && views.len()<=128);
        for view in views {
            let expected=execute_request(json!({"op":"preview_interactive","id":"office","spec":spec,"state":view["state"]})).unwrap();
            assert_eq!(expected["element"],view["element"]);
            assert_eq!(expected["state"],view["state"]);
        }
        assert!(views.iter().any(|view|view["state"]==payload["initial_state"]));
    }
}

#[test]
fn content_manifest_has_its_own_identity_and_rejects_unsafe_runtime_urls() {
    let manifest=execute_request(json!({"op":"powerpoint_addin_manifest","runtime_url":"https://localhost:4198/office.html"})).unwrap();
    let xml=manifest["manifest"].as_str().unwrap();
    assert!(xml.contains("ContentApp"));
    assert!(xml.contains("27b2fe44-fec3-4a21-b8bd-73781ec5b010"));
    assert!(!xml.contains("fa000000210"));
    for url in ["http://localhost/office.html","file:///office.html","https://a:b@host/office.html","https://host/office.html?token=private","https://host/office.html#code","https://host/evil.html"] {
        assert!(execute_request(json!({"op":"powerpoint_addin_manifest","runtime_url":url})).is_err(),"{url}");
    }
}

#[test]
fn content_addin_export_is_explicit_readonly_and_preserves_both_fallback_layers() {
        let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
        let spec=&catalog["presets"].as_array().unwrap().iter().find(|entry|entry["id"]=="interactive/dashboard").unwrap()["example"];
        let original=execute_request(json!({"op":"create_presentation","id":"office-export","title":"Synthetic example"})).unwrap();
        let inserted=execute_request(json!({"op":"insert_part","document":original,"expected_revision":0,"slide_id":"slide-1","id":"interactive","spec":spec})).unwrap();
        let document=&inserted["document"];
        let before=execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        let options=json!({"runtime_url":"https://localhost:4198/office.html","catalog":{"kind":"file_system","location":"\\\\localhost\\AISlideAddins"}});
        let result=execute_request(json!({"op":"export_powerpoint_interactive","document":document,"options":options})).unwrap();
        assert_eq!(result["components"],1);
        assert_eq!(result["powerpoint_playback_verified"],false);
        assert_eq!(result["requires_addin_installation"],true);
        let package=Package::open(STANDARD.decode(result["base64"].as_str().unwrap()).unwrap()).unwrap();
        let slide=roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
        let mc="http://schemas.openxmlformats.org/markup-compatibility/2006";
        assert_eq!(slide.descendants().filter(|node|node.has_tag_name((mc,"AlternateContent"))).count(),1);
        let fallback=slide.descendants().find(|node|node.has_tag_name((mc,"Fallback"))).unwrap();
        assert!(fallback.children().any(|node|node.tag_name().name()=="grpSp"));
        let extension=roxmltree::Document::parse(package.text("ppt/webextensions/webextension1.xml").unwrap()).unwrap();
        let reference=extension.descendants().find(|node|node.tag_name().name()=="reference").unwrap();
        assert_eq!(reference.attribute("storeType"),Some("FileSystem"));
        assert_eq!(reference.attribute("id"),Some("27b2fe44-fec3-4a21-b8bd-73781ec5b010"));
        let value=extension.descendants().find(|node|node.tag_name().name()=="property").unwrap().attribute("value").unwrap();
        let payload:serde_json::Value=serde_json::from_str(value).unwrap();
        assert_eq!(payload["template"],"dashboard");
        assert!(!payload["views"].as_array().unwrap().is_empty());
        assert!(extension.descendants().any(|node|node.tag_name().name()=="snapshot" && node.attribute(("http://schemas.openxmlformats.org/officeDocument/2006/relationships","embed"))==Some("rIdSnapshot")));
        assert!(package.part("ppt/media/aislide-interactive-1.png").unwrap().starts_with(b"\x89PNG"));
        assert!(package.text("[Content_Types].xml").unwrap().contains("application/vnd.ms-office.webextension+xml"));
        assert!(!result["manifest"].as_str().unwrap().contains("fa000000210"));
        assert_eq!(execute_request(json!({"op":"export_presentation","document":document})).unwrap()["base64"],before["base64"]);
        for location in ["C:\\not-a-catalog", "\\\\?\\C:\\device", "\\\\server\\share\\..\\outside", "\\\\server", "\\\\server\\share\nbad"] {
            let mut bad=options.clone();bad["catalog"]["location"]=json!(location);
            assert!(execute_request(json!({"op":"export_powerpoint_interactive","document":document,"options":bad})).is_err(),"{location}");
        }
        let opened=execute_request(json!({"op":"open_presentation","id":"office-readonly","base64":result["base64"]})).unwrap();
        assert_eq!(execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap()["base64"],result["base64"]);
}
