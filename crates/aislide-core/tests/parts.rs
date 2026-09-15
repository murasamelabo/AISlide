use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn column_spec() -> Value {
    json!({"version":1,"preset":"vertical-bar-graph/balanced","title":"Quarterly volume","subtitle":"Synthetic example","data":{"kind":"chart","categories":["Q1","Q2","Q3"],"series":[{"name":"Volume","values":[12,24,18]}],"x_axis":"Quarter","y_axis":"Units"}})
}

fn deck(element: Value) -> Value {
    json!({"version":1,"title":"Parts test","width":1280,"height":720,"slides":[{"id":"slide","title":"Parts","background":"FFFFFF","notes":"Synthetic examples","elements":[element]}]})
}

#[test]
fn parts_create_deterministic_native_column_charts_from_metadata() {
    let request = json!({"op":"create_part","id":"part-probe","spec":column_spec()});
    let first = execute_request(request.clone()).unwrap();
    assert_eq!(first, execute_request(request).unwrap());
    assert_eq!(first["type"], "group");
    let chart = first["children"].as_array().unwrap().iter().find(|element| element["type"] == "chart").unwrap();
    assert_eq!(chart["kind"], "column");
    assert_eq!(chart["series"][0]["values"], json!([12.0,24.0,18.0]));
    let exported = execute_request(json!({"op":"export","deck":deck(first)})).unwrap();
    let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert!(package.text("ppt/charts/chart1.xml").unwrap().contains("<c:barChart"));
    assert!(package.part("ppt/embeddings/chart1.xlsx").is_ok());
    assert!(!package.parts().keys().any(|path| path.starts_with("ppt/media/")));
}

#[test]
fn editable_polygons_and_transparent_shapes_round_trip_as_native_geometry() {
    let polygon = json!({"id":"region","type":"polygon","x":120,"y":150,"width":400,"height":300,"points":[[0,0.5],[0.4,0],[1,0.3],[0.8,1]],"fill":"@accent2","stroke":"@dk1","stroke_width":1});
    let mut scene = deck(polygon.clone());
    let mut outline = execute_request(json!({"op":"create_object","id":"outline","kind":"shape","preset":"ellipse"})).unwrap();
    outline["fill"] = json!("none");
    scene["slides"][0]["elements"].as_array_mut().unwrap().push(outline);
    let exported = execute_request(json!({"op":"export","deck":scene})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"polygon","base64":exported["base64"]})).unwrap();
    assert_eq!(opened["document"]["deck"]["slides"][0]["elements"][0]["points"], json!([[0.0,0.5],[0.4,0.0],[1.0,0.3],[0.8,1.0]]));
    assert_eq!(opened["document"]["deck"]["slides"][0]["elements"][1]["fill"], "none");
    let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert!(package.text("ppt/slides/slide1.xml").unwrap().contains("<a:custGeom>"));
    let mut invalid = polygon;
    invalid["points"][0][0] = json!(-1);
    assert!(execute_request(json!({"op":"export","deck":deck(invalid)})).is_err());
}

#[test]
fn every_category_has_three_distinct_bounded_native_presets() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    let presets = catalog["presets"].as_array().unwrap();
    assert_eq!(presets.len(), 108);
    let mut categories = std::collections::BTreeMap::<String, Vec<Value>>::new();
    let mut failures = Vec::new();
    for preset in presets {
        let element = match execute_request(json!({"op":"create_part","id":"example","spec":preset["example"]})) { Ok(element) => element, Err(error) => { failures.push(format!("{}: {error}",preset["id"])); continue; } };
        let scene = deck(element.clone());
        execute_request(json!({"op":"validate","deck":scene})).unwrap();
        let measured = execute_request(json!({"op":"measure_layout","deck":scene})).unwrap();
        let errors: Vec<_> = measured["issues"].as_array().unwrap().iter().filter(|issue| issue["severity"] == "error").collect();
        if !errors.is_empty() { failures.push(format!("{}: {errors:?}",preset["id"])); }
        let exported = execute_request(json!({"op":"export","deck":scene})).unwrap();
        let opened = execute_request(json!({"op":"open_presentation","id":"preset","base64":exported["base64"]})).unwrap();
        assert_eq!(opened["document"]["deck"]["slides"][0]["elements"][0]["type"], "group", "{}", preset["id"]);
        assert_eq!(opened["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().len(), element["children"].as_array().unwrap().len(), "{}", preset["id"]);
        let layouts = categories.entry(preset["category"].as_str().unwrap().into()).or_default();
        if layouts.contains(&element) { failures.push(format!("duplicate recipe: {}",preset["id"])); }
        layouts.push(element);
    }
    assert!(failures.is_empty(), "{}",failures.join("\n"));
    assert_eq!(categories.len(), 36);
    assert!(categories.values().all(|presets| presets.len() == 3));
}

#[test]
fn part_data_is_validated_before_geometry_is_generated() {
    let mut invalid = column_spec();
    invalid["data"]["series"][0]["values"] = json!([1]);
    assert!(execute_request(json!({"op":"create_part","id":"bad","spec":invalid})).is_err());
    for category in ["100-add-vertical-bar-graph", "pie-chart"] {
        let mut invalid = column_spec(); invalid["preset"] = json!(format!("{category}/balanced")); invalid["data"]["series"][0]["values"] = json!([0,-1,1]);
        assert!(execute_request(json!({"op":"create_part","id":"bad","spec":invalid})).is_err());
    }
    for input in [
        json!({"kind":"items","items":[{"label":"TAM","value":10},{"label":"SAM","value":20},{"label":"SOM","value":5}]}),
        json!({"kind":"items","items":[{"label":"TAM","value":0},{"label":"SAM","value":0},{"label":"SOM","value":0}]})
    ] {
        assert!(execute_request(json!({"op":"create_part","id":"bad","spec":{"version":1,"preset":"grow/balanced","title":"Markets","data":input}})).is_err());
    }
}

#[test]
fn metadata_parts_are_revisioned_undoable_and_do_not_override_external_xml() {
    let mut scene=deck(json!({"id":"label","type":"text","x":64,"y":20,"width":900,"height":60,"text":"Parts","font_size":28,"color":"202426","bold":true}));
    scene["slides"][0]["elements"]=json!([]);
    let document=execute_request(json!({"op":"new_document","id":"parts","deck":scene})).unwrap();
    let inserted=execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"metric-part","spec":column_spec()})).unwrap();
    assert_eq!(inserted["document"]["revision"],1);
    assert_eq!(inserted["document"]["parts"][0]["stale"],false);
    let mut spec=column_spec(); spec["title"]=json!("Updated volume");spec["data"]["series"][0]["values"][0]=json!(44);
    let updated=execute_request(json!({"op":"update_part","document":inserted["document"],"expected_revision":1,"slide_id":"slide","id":"metric-part","spec":spec})).unwrap();
    assert_eq!(updated["document"]["parts"][0]["spec"]["title"],"Updated volume");
    let undone=execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":2,"receipt":updated["receipt"]})).unwrap();
    assert_eq!(undone["document"]["deck"],inserted["document"]["deck"]);
    let saved=execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"reopen","base64":saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["parts"][0]["stale"],false);
    assert_eq!(execute_request(json!({"op":"export_presentation","document":reopened["document"]})).unwrap()["base64"],saved["base64"]);
    let mut next_spec=spec.clone();next_spec["title"]=json!("Reopened edit");
    execute_request(json!({"op":"update_part","document":reopened["document"],"expected_revision":0,"slide_id":"slide","id":"metric-part","spec":next_spec})).unwrap();
    let mut package=Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml=package.text("ppt/slides/slide1.xml").unwrap().replace("Updated volume","Edited outside AISlide");
    package.replace_part("ppt/slides/slide1.xml",xml.into_bytes()).unwrap();
    let external=execute_request(json!({"op":"open_presentation","id":"external","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    assert_eq!(external["document"]["parts"][0]["stale"],true);
    let result=execute_request(json!({"op":"update_part","document":external["document"],"expected_revision":0,"slide_id":"slide","id":"metric-part","spec":spec}));
    assert!(result.unwrap_err().to_string().contains("stale"));
    assert!(external["document"]["deck"].to_string().contains("Edited outside AISlide"));
}

#[test]
fn long_part_labels_are_fitted_or_rejected_before_insertion() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    let mut spec=catalog["presets"].as_array().unwrap().iter().find(|preset|preset["id"]=="flow/balanced").unwrap()["example"].clone();
    spec["data"]["items"]=json!((0..6).map(|_|json!({"label":"A long label that must fit within each stage","detail":""})).collect::<Vec<_>>());
    let element=execute_request(json!({"op":"create_part","id":"long","spec":spec})).unwrap();
    let measured=execute_request(json!({"op":"measure_layout","deck":deck(element)})).unwrap();
    assert!(measured["issues"].as_array().unwrap().iter().all(|issue|issue["severity"]!="error"),"{}",measured["issues"]);
    let mut invalid=column_spec();
    invalid["title"]=json!("W".repeat(80));
    let result=execute_request(json!({"op":"create_part","id":"fit","spec":invalid}));
    if let Ok(element)=result {let measured=execute_request(json!({"op":"measure_layout","deck":deck(element)})).unwrap();assert!(measured["issues"].as_array().unwrap().iter().all(|issue|issue["severity"]!="error"));}
}

#[test]
fn deleting_parts_removes_metadata_and_undo_restores_both() {
    let mut scene=deck(json!({}));scene["slides"][0]["elements"]=json!([]);
    let document=execute_request(json!({"op":"new_document","id":"delete-part","deck":scene})).unwrap();
    let inserted=execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"part","spec":column_spec()})).unwrap();
    let saved=execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"delete-reopened","base64":saved["base64"]})).unwrap();
    for document in [inserted["document"].clone(),reopened["document"].clone()] {
        let removed=execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"remove","path":"/deck/slides/0/elements/0"}]}})).unwrap();
        assert!(removed["document"]["parts"].as_array().is_none_or(|parts|parts.is_empty()));
        let undone=execute_request(json!({"op":"undo_transaction","document":removed["document"],"expected_revision":removed["document"]["revision"],"receipt":removed["receipt"]})).unwrap();
        assert_eq!(undone["document"]["parts"],document["parts"]);
        assert_eq!(undone["document"]["deck"],document["deck"]);
        let exported=execute_request(json!({"op":"export_presentation","document":removed["document"]})).unwrap();
        let empty=execute_request(json!({"op":"open_presentation","id":"empty","base64":exported["base64"]})).unwrap();
        assert!(empty["document"]["parts"].as_array().is_none_or(|parts|parts.is_empty()));
        if document["origin"].is_null() {
            let replaced=execute_request(json!({"op":"insert_part","document":removed["document"],"expected_revision":removed["document"]["revision"],"slide_id":"slide","id":"part","spec":column_spec()})).unwrap();
            assert_eq!(replaced["document"]["parts"][0]["stale"],false);
        }
    }
}

#[test]
fn large_waterfall_totals_cannot_hide_material_reconciliation_errors() {
    let spec=json!({"version":1,"preset":"water-fall/balanced","title":"Totals","data":{"kind":"waterfall","steps":[{"label":"Start","value":1e15,"total":true},{"label":"Finish","value":1e15-100.0,"total":true}]}});
    assert!(execute_request(json!({"op":"create_part","id":"bad-total","spec":spec})).is_err());
}

#[test]
fn external_part_extensions_and_workbook_changes_remain_stale_after_save() {
    let mut scene=deck(json!({"id":"unused","type":"rect","x":0,"y":0,"width":1,"height":1,"fill":"FFFFFF"}));scene["slides"][0]["elements"]=json!([]);
    let document=execute_request(json!({"op":"new_document","id":"native-guards","deck":scene})).unwrap();
    let inserted=execute_request(json!({"op":"insert_part","document":document,"expected_revision":0,"slide_id":"slide","id":"part","spec":column_spec()})).unwrap();
    let exported=execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let bytes=STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap();
    for case in ["extension","workbook","missing-fingerprint"] {
        let mut package=Package::open(bytes.clone()).unwrap();
        if case=="workbook" {
            let mut workbook=Package::open(package.part("ppt/embeddings/chart1.xlsx").unwrap().to_vec()).unwrap();
            let sheet=workbook.text("xl/worksheets/sheet1.xml").unwrap().replace("<v>12</v>","<v>999</v>");
            workbook.replace_part("xl/worksheets/sheet1.xml",sheet.into_bytes()).unwrap();package.replace_part("ppt/embeddings/chart1.xlsx",workbook.save().unwrap()).unwrap();
        } else {
            let xml=package.text("ppt/slides/slide1.xml").unwrap().replace("</p:grpSp>","<p:extLst><p:ext uri=\"urn:foreign\"><v:foreign xmlns:v=\"urn:vendor\"/></p:ext></p:extLst></p:grpSp>");
            package.replace_part("ppt/slides/slide1.xml",xml.into_bytes()).unwrap();
            if case=="missing-fingerprint" {
                let path=package.parts().keys().find(|path|path.starts_with("customXml/aislide-provenance")).unwrap().clone();let mut xml=package.text(&path).unwrap().to_owned();let parsed=roxmltree::Document::parse(&xml).unwrap();let range=parsed.descendants().find(|node|node.attribute("name")==Some("native_sha256")).unwrap().range();xml.replace_range(range,"");package.replace_part(&path,xml.into_bytes()).unwrap();
            }
        }
        let external_bytes=package.save().unwrap();
        let opened=execute_request(json!({"op":"open_presentation","id":"outside","base64":STANDARD.encode(&external_bytes)})).unwrap();
        assert_eq!(opened["document"]["parts"][0]["stale"],true,"{case}");
        let saved=execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap();
        assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(),external_bytes,"{case}");
        let reopened=execute_request(json!({"op":"open_presentation","id":"again","base64":saved["base64"]})).unwrap();
        assert_eq!(reopened["document"]["parts"][0]["stale"],true,"{case}");
        assert!(execute_request(json!({"op":"update_part","document":reopened["document"],"expected_revision":0,"slide_id":"slide","id":"part","spec":column_spec()})).is_err());
    }
}

#[test]
fn percentage_focus_uses_share_and_equal_ranking_values_share_rank() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    let presets=catalog["presets"].as_array().unwrap();
    let mut spec=presets.iter().find(|preset|preset["id"]=="100-add-vertical-bar-graph/focus").unwrap()["example"].clone();
    spec["data"]["categories"]=json!(["A","B"]);spec["data"]["series"]=json!([{"name":"Primary","values":[25,25]},{"name":"Secondary","values":[75,75]}]);
    let element=execute_request(json!({"op":"create_part","id":"share","spec":spec})).unwrap();
    assert!(element["children"].as_array().unwrap().iter().any(|element|element["text"]=="25%"));
    let mut rank=presets.iter().find(|preset|preset["id"]=="ranking/focus").unwrap()["example"].clone();
    rank["data"]["items"][1]["value"]=rank["data"]["items"][0]["value"].clone();
    let element=execute_request(json!({"op":"create_part","id":"rank","spec":rank})).unwrap();
    assert_eq!(element["children"].as_array().unwrap().iter().filter(|element|element["text"]=="#01").count(),2);
}

#[test]
fn three_set_venn_labels_do_not_cross_circle_outlines() {
    let catalog=execute_request(json!({"op":"part_catalog"})).unwrap();
    let spec=&catalog["presets"].as_array().unwrap().iter().find(|preset|preset["id"]=="venn/focus").unwrap()["example"];
    let element=execute_request(json!({"op":"create_part","id":"venn","spec":spec})).unwrap();
    let children=element["children"].as_array().unwrap();
    for text in children.iter().filter(|child|child["type"]=="text" && child["y"].as_f64().unwrap()>=80.0) {
        let left=text["x"].as_f64().unwrap();let right=left+text["width"].as_f64().unwrap();
        let top=text["y"].as_f64().unwrap();let bottom=top+text["height"].as_f64().unwrap();
        for circle in children.iter().filter(|child|child["preset"]=="ellipse") {
            let radius_x=circle["width"].as_f64().unwrap()/2.0;let radius_y=circle["height"].as_f64().unwrap()/2.0;
            let center_x=circle["x"].as_f64().unwrap()+radius_x;let center_y=circle["y"].as_f64().unwrap()+radius_y;
            let distance=|horizontal:f64,vertical:f64|((horizontal-center_x)/radius_x).powi(2)+((vertical-center_y)/radius_y).powi(2);
            let nearest=distance(center_x.clamp(left,right),center_y.clamp(top,bottom));
            let furthest=[distance(left,top),distance(left,bottom),distance(right,top),distance(right,bottom)].into_iter().fold(0.0,f64::max);
            assert!(nearest>1.0 || furthest<1.0,"circle outline crosses {}",text["text"]);
        }
    }
}