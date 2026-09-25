use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn deck(element: Value) -> Value {
    json!({"version":1,"title":"Visual test","width":1280,"height":720,"slides":[{"id":"slide","title":"Visual test","background":"FFFFFF","elements":[element],"notes":""}]})
}

fn shape() -> Value {
    json!({"type":"shape","id":"shape","x":100,"y":100,"width":300,"height":180,"preset":"roundRect","fill":"123456","stroke":"000000","stroke_width":2,"rotation":0,"text":"","font_size":24,"color":"000000","bold":false})
}

fn export(deck: Value) -> Vec<u8> {
    let output = execute_request(json!({"op":"export","deck":deck})).unwrap();
    STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()
}

#[test]
fn visual_transform_fill_effects_are_native_and_reopen() {
    let mut element = shape();
    element["visual"] = json!({"flip_h":true,"opacity":0.5,"shadow":{"color":"112233","opacity":0.4,"blur":6,"distance":8,"angle":45}});
    let bytes = export(deck(element));
    let package = Package::open(bytes.clone()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap();
    assert!(xml.contains("flipH=\"1\""));
    assert!(xml.contains("<a:alpha val=\"50000\""));
    assert!(xml.contains("<a:outerShdw"));
    let opened = execute_request(json!({"op":"open_presentation","id":"visual","base64":STANDARD.encode(bytes)})).unwrap();
    let visual = &opened["document"]["deck"]["slides"][0]["elements"][0]["visual"];
    assert_eq!(visual["flip_h"], true);
    assert_eq!(visual["opacity"], 0.5);
    assert_eq!(visual["shadow"]["blur"], 6.0);
}

#[test]
fn svg_asset_retains_vector_and_native_relationship() {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="2" y="2" width="20" height="20" fill="#ff0000"/></svg>"##;
    let asset = execute_request(json!({"op":"create_asset","id":"svg","base64":STANDARD.encode(source),"mime_type":"image/svg+xml","alt":"Vector","size":96})).unwrap();
    assert_eq!(asset["svg"], STANDARD.encode(source));
    let bytes = export(deck(asset));
    let package = Package::open(bytes.clone()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap();
    assert!(xml.contains("svgBlip"));
    assert!(package.parts().iter().any(|(path, data)| path.ends_with(".svg") && data == source.as_bytes()));
    let opened = execute_request(json!({"op":"open_presentation","id":"vector","base64":STANDARD.encode(bytes)})).unwrap();
    assert_eq!(opened["document"]["deck"]["slides"][0]["elements"][0]["svg"], STANDARD.encode(source));
}

fn open(bytes: &[u8]) -> Value {
    execute_request(json!({"op":"open_presentation","id":"visual-test","base64":STANDARD.encode(bytes)})).unwrap()["document"].clone()
}

fn transact(document: &Value, operations: Value) -> aislide_core::Result<Value> {
    execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":operations}}))
}

fn saved(document: &Value) -> Vec<u8> {
    let result = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    STANDARD.decode(result["base64"].as_str().unwrap()).unwrap()
}

fn assert_undo(document: &Value, changed: &Value, source: &[u8]) {
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], document["hash"]);
    assert_eq!(saved(&undone["document"]), source);
    assert_eq!(saved(document), source);
}

#[test]
fn custom_polyline_retains_native_route_references_and_styles() {
    let points = json!([[0.1,0.2],[0.3,0.8],[0.5,0.4],[0.6,0.9],[0.8,0.1],[0.9,0.7]]);
    let connector = json!({"type":"connector","id":"route","x":400,"y":180,"width":400,"height":240,"color":"123456","stroke_width":2,"arrow":true,"start":{"element_id":"shape","site":3},"end":{"element_id":"target","site":1},"routing":{"custom":true,"points":points,"start_arrow":true,"dashed":true}});
    let mut target = shape(); target["id"] = json!("target"); target["x"] = json!(850);
    let mut scene = deck(shape());
    scene["slides"][0]["elements"].as_array_mut().unwrap().extend([target, connector]);
    let source = export(scene);
    let package = Package::open(source.clone()).unwrap();
    let parsed = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    let route = parsed.descendants().find(|node| node.tag_name().name() == "cxnSp").unwrap();
    let native_id = |name| parsed.descendants().find(|node| node.tag_name().name() == "cNvPr" && node.attribute("name") == Some(name)).unwrap().attribute("id").unwrap();
    for (tag, name, site) in [("stCxn", "shape", "3"), ("endCxn", "target", "1")] {
        let connection = route.descendants().find(|node| node.tag_name().name() == tag).unwrap();
        assert_eq!(connection.attribute("id"), Some(native_id(name)));
        assert_eq!(connection.attribute("idx"), Some(site));
    }
    assert!(route.descendants().any(|node| node.tag_name().name() == "custGeom"));
    assert!(!route.descendants().any(|node| node.tag_name().name() == "prstGeom"));
    assert_eq!(route.descendants().filter(|node| node.tag_name().name() == "lnTo").count(), 5);
    for (tag, attribute, value) in [("headEnd", "type", "triangle"), ("tailEnd", "type", "triangle"), ("prstDash", "val", "dash")] {
        assert!(route.descendants().any(|node| node.tag_name().name() == tag && node.attribute(attribute) == Some(value)));
    }
    let document = open(&source);
    let reopened = &document["deck"]["slides"][0]["elements"][2];
    assert_eq!(reopened["routing"], json!({"custom":true,"points":points,"start_arrow":true,"dashed":true}));
    assert_eq!(reopened["start"], json!({"element_id":"shape","site":3}));
    assert_eq!(reopened["end"], json!({"element_id":"target","site":1}));
    assert_eq!(saved(&document), source);
}

#[test]
fn custom_connection_sites_retain_semantic_shapes_and_native_indices() {
    let sites = json!([{"x":0.5,"y":0.0,"angle":270.0},{"x":0.0,"y":0.5,"angle":180.0},{"x":0.5,"y":1.0,"angle":90.0},{"x":1.0,"y":0.5,"angle":0.0},{"x":0.75,"y":0.25,"angle":360.0}]);
    for preset in ["rect", "roundRect", "ellipse", "diamond"] {
        let mut node = shape(); node["preset"] = json!(preset);
        node["visual"] = json!({"connection_sites":sites});
        let mut target = node.clone(); target["id"] = json!("target"); target["x"] = json!(850);
        let connector = json!({"type":"connector","id":"route","x":400,"y":180,"width":400,"height":240,"color":"123456","stroke_width":2,"arrow":true,"start":{"element_id":"shape","site":4},"end":{"element_id":"target","site":4},"routing":{"custom":true,"points":[[0.25,0.25],[0.5,0.75],[1.0,1.0]],"start_arrow":true,"dashed":true}});
        let mut scene = deck(node);
        scene["slides"][0]["elements"].as_array_mut().unwrap().extend([connector, target]);
        let source = export(scene);
        let package = Package::open(source.clone()).unwrap();
        let parsed = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
        let native_shape = parsed.descendants().find(|node| node.tag_name().name() == "sp").unwrap();
        let geometry = native_shape.descendants().find(|node| node.tag_name().name() == "custGeom").unwrap();
        assert_eq!(geometry.descendants().filter(|node| node.tag_name().name() == "cxn").count(), 5, "{preset}");
        let native_id = native_shape.descendants().find(|node| node.tag_name().name() == "cNvPr").unwrap().attribute("id").unwrap();
        let connection = parsed.descendants().find(|node| node.tag_name().name() == "stCxn").unwrap();
        assert_eq!(connection.attribute("id"), Some(native_id));
        assert_eq!(connection.attribute("idx"), Some("4"));
        let target_id = parsed.descendants().find(|node| node.tag_name().name() == "cNvPr" && node.attribute("name") == Some("target")).unwrap().attribute("id").unwrap();
        let end = parsed.descendants().find(|node| node.tag_name().name() == "endCxn").unwrap();
        assert_eq!(end.attribute("id"), Some(target_id));
        assert_eq!(end.attribute("idx"), Some("4"));
        let document = open(&source);
        let reopened = &document["deck"]["slides"][0]["elements"][0];
        assert_eq!(reopened["type"], "shape", "{preset}");
        assert_eq!(reopened["preset"], preset);
        assert_eq!(reopened["visual"]["connection_sites"], sites);
        assert_eq!(document["deck"]["slides"][0]["elements"].as_array().unwrap().len(), 3);
        assert_eq!(document["deck"]["slides"][0]["elements"][2]["visual"]["connection_sites"], sites);
        assert_eq!(document["deck"]["slides"][0]["elements"][1]["end"], json!({"element_id":"target","site":4}));
        assert_eq!(saved(&document), source);
        let changed = transact(&document, json!([
            {"op":"replace","path":"/deck/slides/0/elements/1/start/site","value":1},
            {"op":"remove","path":"/deck/slides/0/elements/0/visual"}
        ])).unwrap();
        let cleared = open(&saved(&changed["document"]));
        assert_eq!(cleared["deck"]["slides"][0]["elements"][0]["preset"], preset);
        assert!(cleared["deck"]["slides"][0]["elements"][0]["visual"].is_null());
        assert_undo(&document, &changed, &source);
    }
    for preset in ["can", "cloud"] {
        let mut node = shape(); node["preset"] = json!(preset); node["visual"] = json!({"connection_sites":sites});
        let error = execute_request(json!({"op":"export","deck":deck(node)})).unwrap_err().to_string();
        assert!(error.contains("faithful custom connection-site geometry"), "{error}");
    }
}

#[test]
fn custom_polyline_native_edit_is_guarded_and_undoable() {
    let route = json!({"type":"connector","id":"route","x":400,"y":180,"width":400,"height":240,"color":"123456","stroke_width":2,"arrow":true,"start":{"element_id":"shape","site":3},"routing":{"custom":true,"points":[[0.1,0.2],[0.5,0.8],[0.9,0.7]],"start_arrow":true,"dashed":true}});
    let mut scene = deck(shape()); scene["slides"][0]["elements"].as_array_mut().unwrap().push(route);
    let source = export(scene); let document = open(&source);
    let points = json!([[0.0,0.2],[0.3,0.8],[0.8,0.7],[1.0,0.5]]);
    let operations = json!([
        {"op":"replace","path":"/deck/slides/0/elements/1/routing/points","value":points},
        {"op":"replace","path":"/deck/slides/0/elements/1/routing/dashed","value":false},
        {"op":"replace","path":"/deck/slides/0/elements/1/routing/start_arrow","value":false},
        {"op":"replace","path":"/deck/slides/0/elements/1/x","value":420}
    ]);
    let changed = transact(&document, operations.clone()).unwrap();
    let reopened = open(&saved(&changed["document"]));
    let route = &reopened["deck"]["slides"][0]["elements"][1];
    assert_eq!(route["routing"], json!({"custom":true,"points":points,"start_arrow":false,"dashed":false}));
    assert_eq!(route["start"], json!({"element_id":"shape","site":3}));
    assert_eq!(route["arrow"], true);
    assert_eq!(route["x"], 420.0);
    assert_undo(&document, &changed, &source);
    let mut package = Package::open(source).unwrap();
    let path = "ppt/slides/slide1.xml";
    let xml = package.text(path).unwrap().replace("fill=\"none\"", "fill=\"none\" extrusionOk=\"0\"");
    package.replace_part(path, xml.into_bytes()).unwrap();
    let source = package.save().unwrap(); let document = open(&source);
    assert!(transact(&document, operations).is_err());
    assert_eq!(saved(&document), source);
}

#[test]
fn custom_polyline_limits_and_legacy_defaults_are_preserved() {
    let base = json!({"type":"connector","id":"route","x":100,"y":100,"width":400,"height":240,"color":"123456","stroke_width":2,"arrow":true,"routing":{"points":[[0.0,0.0],[1.0,1.0]],"start_arrow":false,"dashed":false}});
    let legacy = export(deck(base.clone()));
    let mut explicit_default = base.clone(); explicit_default["routing"]["custom"] = json!(false);
    assert_eq!(export(deck(explicit_default.clone())), legacy);
    let legacy_model: aislide_core::model::Deck = serde_json::from_value(deck(base.clone())).unwrap();
    let explicit_model: aislide_core::model::Deck = serde_json::from_value(deck(explicit_default)).unwrap();
    assert_eq!(serde_json::to_value(legacy_model).unwrap(), serde_json::to_value(explicit_model).unwrap());
    let mut custom = base.clone(); custom["routing"]["custom"] = json!(true);
    for points in [json!([[0.0,0.0],[1.0,1.0]]), json!([[0.0,0.0],[0.5,0.8],[1.0,1.0]]), json!((0..18).map(|index| [index as f64 / 17.0, (index % 2) as f64]).collect::<Vec<_>>())] {
        custom["routing"]["points"] = points;
        let bytes = export(deck(custom.clone()));
        assert_eq!(open(&bytes)["deck"]["slides"][0]["elements"][0]["routing"]["custom"], true);
        assert!(Package::open(bytes).unwrap().text("ppt/slides/slide1.xml").unwrap().contains("a:custGeom"));
    }
    for points in [json!([]),json!([[0,0]]),json!([[0,0],[0,0],[1,1]]),json!([[0,0],[0.00000001,0.00000001],[1,1]]),json!([[0,0],[1.1,1]]),json!([[0,0],[-0.1,1]]),json!((0..19).map(|index| [index as f64 / 18.0, (index % 2) as f64]).collect::<Vec<_>>())] {
        custom["routing"]["points"] = points;
        assert!(execute_request(json!({"op":"export","deck":deck(custom.clone())})).is_err());
    }
    for points in [json!([[0.1,0.2],[0.9,0.7]]),json!([[0,0],[0.5,0.8],[1,1]]),json!([[0,0],[0.2,0],[0.2,0.5],[0.8,0.5],[1,1]])] {
        let mut legacy = base.clone(); legacy["routing"]["points"] = points;
        assert!(execute_request(json!({"op":"export","deck":deck(legacy)})).is_err());
    }
}

#[test]
fn custom_connection_site_validation_and_default_serialization() {
    let source = export(deck(shape()));
    let mut empty = shape(); empty["visual"] = json!({"connection_sites":[]});
    let mut default = shape(); default["visual"] = json!({});
    assert_eq!(export(deck(empty.clone())), export(deck(default)));
    assert_eq!(serde_json::to_value(serde_json::from_value::<aislide_core::visual::VisualStyle>(json!({"connection_sites":[]})).unwrap()).unwrap(), json!({}));
    for sites in [json!([{"x":-0.1,"y":0,"angle":0}]),json!([{"x":0,"y":1.1,"angle":0}]),json!([{"x":0,"y":0,"angle":360.1}]),json!([{"x":0,"y":0,"angle":-1}]),json!([{"x":0,"y":0}]),json!(vec![json!({"x":0,"y":0,"angle":0});129])] {
        let mut node = shape(); node["visual"] = json!({"connection_sites":sites});
        assert!(execute_request(json!({"op":"export","deck":deck(node)})).is_err());
    }
    let sites = json!([{"x":0.2,"y":0.5,"angle":180.0}]);
    let mut node = shape(); node["visual"] = json!({"connection_sites":sites});
    let mut scene = deck(node.clone());
    scene["slides"][0]["elements"].as_array_mut().unwrap().push(json!({"type":"connector","id":"route","x":400,"y":180,"width":400,"height":240,"color":"123456","stroke_width":2,"arrow":false,"start":{"element_id":"shape","site":1}}));
    assert!(execute_request(json!({"op":"export","deck":scene})).is_err());
    for kind in ["rect", "text", "connector", "polygon", "group", "picture"] {
        let mut invalid = node.clone(); invalid["type"] = json!(kind);
        assert!(execute_request(json!({"op":"export","deck":deck(invalid)})).is_err());
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for site in [aislide_core::visual::ConnectionSite {x:value,y:0.5,angle:0.0},aislide_core::visual::ConnectionSite {x:0.5,y:value,angle:0.0},aislide_core::visual::ConnectionSite {x:0.5,y:0.5,angle:value}] {
            assert!(aislide_core::vector::connection_geometry_xml("rect", &[], &[site]).is_err());
        }
    }
    assert_eq!(saved(&open(&source)), source);
}

#[test]
fn custom_connection_sites_resize_edit_and_reject_tampered_geometry() {
    let sites = json!([{"x":0.25,"y":0.75,"angle":45.0},{"x":1.0,"y":0.5,"angle":0.0}]);
    let source = export(deck(shape())); let document = open(&source);
    let style = json!({"connection_sites":sites,"adjustments":[{"name":"adj","value":12000}],"flip_h":true});
    let added = transact(&document, json!([{"op":"add","path":"/deck/slides/0/elements/0/visual","value":style}])).unwrap();
    let source = saved(&added["document"]); let document = open(&source);
    assert_eq!(document["deck"]["slides"][0]["elements"][0]["visual"], style);
    let package = Package::open(source.clone()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap();
    let parsed = roxmltree::Document::parse(xml).unwrap();
    let geometry = parsed.descendants().find(|node| node.tag_name().name() == "custGeom").unwrap();
    let connections: Vec<_> = geometry.descendants().filter(|node| node.tag_name().name() == "cxn").collect();
    assert_eq!(connections[0].attribute("ang"), Some("2700000"));
    let position = connections[0].children().find(|node| node.tag_name().name() == "pos").unwrap();
    assert_eq!(position.attribute("x"), Some("site0x"));
    assert_eq!(position.attribute("y"), Some("site0y"));
    for (name, formula) in [("site0x", "*/ w 250000 1000000"),("site0y", "*/ h 750000 1000000"),("radius", "*/ ss adj 100000")] {
        assert!(geometry.descendants().any(|node| node.tag_name().name() == "gd" && node.attribute("name") == Some(name) && node.attribute("fmla") == Some(formula)));
    }
    assert_eq!(geometry.descendants().filter(|node| node.tag_name().name() == "arcTo").count(), 4);
    let changed = transact(&document, json!([
        {"op":"replace","path":"/deck/slides/0/elements/0/visual/connection_sites/0/x","value":0.5},
        {"op":"replace","path":"/deck/slides/0/elements/0/visual/adjustments/0/value","value":24000},
        {"op":"replace","path":"/deck/slides/0/elements/0/width","value":500}
    ])).unwrap();
    let reopened = open(&saved(&changed["document"]));
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["width"], 500.0);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["visual"]["connection_sites"][0]["x"], 0.5);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["visual"]["adjustments"][0]["value"], 24000);
    assert_undo(&document, &changed, &source);
    let mut package = Package::open(source).unwrap();
    let path = "ppt/slides/slide1.xml";
    let xml = package.text(path).unwrap().replace("swAng=\"5400000\"", "swAng=\"2700000\"");
    package.replace_part(path, xml.into_bytes()).unwrap();
    let source = package.save().unwrap(); let document = open(&source);
    assert!(document["deck"]["slides"][0]["elements"].as_array().unwrap().is_empty());
    assert_eq!(saved(&document), source);
    let mut node = shape(); node["visual"] = json!({"connection_sites":sites});
    assert!(transact(&document, json!([{"op":"add","path":"/deck/slides/0/elements/-","value":node}])).is_err());
}

#[test]
fn custom_polyline_zero_axis_native_bounds_do_not_introduce_diagonals() {
    for (axis, attribute, points) in [(0, "cx", json!([[0.2,0.1],[0.7,0.5],[0.4,0.9]])),(1, "cy", json!([[0.1,0.2],[0.5,0.7],[0.9,0.4]]))] {
        let route = json!({"type":"connector","id":"route","x":100,"y":100,"width":400,"height":240,"color":"123456","stroke_width":2,"arrow":true,"routing":{"custom":true,"points":points,"start_arrow":true,"dashed":true}});
        let mut package = Package::open(export(deck(route))).unwrap();
        let path = "ppt/slides/slide1.xml";
        let mut xml = package.text(path).unwrap().to_owned();
        let parsed = roxmltree::Document::parse(&xml).unwrap();
        let extent = parsed.descendants().find(|node| node.tag_name().name() == "cxnSp").unwrap().descendants().find(|node| node.tag_name().name() == "ext").unwrap();
        let range = extent.attributes().find(|candidate| candidate.name() == attribute).unwrap().range_value();
        xml.replace_range(range, "0");
        package.replace_part(path, xml.into_bytes()).unwrap();
        let source = package.save().unwrap(); let document = open(&source);
        let route = &document["deck"]["slides"][0]["elements"][0];
        assert!(route["routing"]["points"].as_array().unwrap().iter().all(|point| point[axis] == 0.0));
        assert_eq!(saved(&document), source);
        let changed = transact(&document, json!([{"op":"replace","path":"/deck/slides/0/elements/0/x","value":120}])).unwrap();
        let reopened = open(&saved(&changed["document"]));
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["routing"], route["routing"]);
        let fresh = open(&export(reopened["deck"].clone()));
        assert_eq!(fresh["deck"]["slides"][0]["elements"][0]["routing"], route["routing"]);
    }
}

#[test]
fn review_flipped_custom_connector_frame_edits_preserve_route() {
    for flips in ["flipV=\"1\"", "flipH=\"1\"", "flipH=\"1\" flipV=\"1\""] {
        let route = json!({"type":"connector","id":"route","x":100,"y":100,"width":400,"height":240,"color":"123456","stroke_width":2,"arrow":true,"routing":{"custom":true,"points":[[0.2,0.1],[0.4,0.8],[1.0,0.3]],"start_arrow":true,"dashed":true}});
        let mut package = Package::open(export(deck(route))).unwrap();
        let path = "ppt/slides/slide1.xml";
        let mut xml = package.text(path).unwrap().to_owned();
        let parsed = roxmltree::Document::parse(&xml).unwrap();
        let transform = parsed.descendants().find(|node| node.tag_name().name() == "cxnSp").unwrap().descendants().find(|node| node.tag_name().name() == "xfrm").unwrap();
        xml.insert_str(transform.range().start + "<a:xfrm".len(), &format!(" {flips}"));
        package.replace_part(path, xml.into_bytes()).unwrap();
        let source = package.save().unwrap(); let document = open(&source);
        let changed = transact(&document, json!([{"op":"replace","path":"/deck/slides/0/elements/0/x","value":120}])).unwrap();
        let reopened = open(&saved(&changed["document"]));
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["routing"], document["deck"]["slides"][0]["elements"][0]["routing"], "{flips}");
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["x"], 120.0);
        assert_undo(&document, &changed, &source);
    }
}

#[test]
fn custom_polyline_rejects_malformed_points_and_unrepresented_line_edits() {
    let route = json!({"type":"connector","id":"route","x":100,"y":100,"width":400,"height":240,"color":"123456","stroke_width":2,"arrow":true,"routing":{"custom":true,"points":[[0.1,0.2],[0.5,0.8],[0.9,0.7]],"start_arrow":true,"dashed":true}});
    let source = export(deck(route.clone()));
    let mut package = Package::open(source.clone()).unwrap();
    let path = "ppt/slides/slide1.xml";
    let xml = package.text(path).unwrap().replace("</a:moveTo>", "<a:pt x=\"1\" y=\"1\"/></a:moveTo>");
    package.replace_part(path, xml.into_bytes()).unwrap();
    let malformed = package.save().unwrap(); let document = open(&malformed);
    assert!(document["deck"]["slides"][0]["elements"].as_array().unwrap().is_empty());
    assert_eq!(saved(&document), malformed);
    let mut package = Package::open(source).unwrap();
    let xml = package.text(path).unwrap().replace("<a:ln w=", "<a:ln cap=\"rnd\" w=");
    package.replace_part(path, xml.into_bytes()).unwrap();
    let source = package.save().unwrap(); let document = open(&source);
    for (field, value) in [("color",json!("654321")),("stroke_width",json!(3)),("arrow",json!(false))] {
        assert!(transact(&document, json!([{"op":"replace","path":format!("/deck/slides/0/elements/0/{field}"),"value":value}])).is_err());
    }
    assert_eq!(saved(&document), source);
    let mut collapsed = route;
    collapsed["width"] = json!(0.00001);
    collapsed["routing"]["points"] = json!([[0.0,0.5],[1.0,0.5]]);
    assert!(execute_request(json!({"op":"export","deck":deck(collapsed)})).is_err());
}

#[test]
fn custom_connection_geometry_render_keeps_curves_and_degenerate_round_rect() {
    for (preset, adjustment) in [("rect",None),("diamond",None),("ellipse",None),("roundRect",None),("roundRect",Some(0)),("roundRect",Some(50000))] {
        let mut node = shape(); node["preset"] = json!(preset);
        node["visual"] = json!({"connection_sites":[{"x":0.75,"y":0.25,"angle":45.0}]});
        if let Some(value) = adjustment { node["visual"]["adjustments"] = json!([{"name":"adj","value":value}]); }
        let scene: aislide_core::model::Deck = serde_json::from_value(deck(node.clone())).unwrap();
        let rendered = aislide_core::export_static::export_static(&scene, &Default::default()).unwrap();
        let image = image::load_from_memory(&rendered.artifacts[0].bytes).unwrap().to_rgba8();
        assert_eq!(image.get_pixel(250,190).0, [0x12,0x34,0x56,255]);
        let bytes = export(deck(node));
        let reopened = open(&bytes);
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["preset"], preset);
        if preset == "roundRect" && adjustment == Some(0) {
            let package = Package::open(bytes).unwrap();
            assert!(!package.text("ppt/slides/slide1.xml").unwrap().contains("a:arcTo"));
        }
    }
}

#[test]
fn native_visual_edits_reopen_remove_and_undo_each_family() {
    let source = export(deck(shape()));
    let document = open(&source);
    for style in [
        json!({"flip_h":true,"flip_v":true,"hidden":true,"locked":true}),
        json!({"opacity":0.2}),
        json!({"shadow":{"color":"112233","opacity":0.4,"blur":6,"distance":8,"angle":45}}),
        json!({"glow":{"color":"112233","opacity":0.4,"radius":6}}),
        json!({"soft_edge":5}),
        json!({"reflection":{"blur":3,"distance":6,"start_opacity":0.5,"end_opacity":0,"end_position":0.7}}),
        json!({"gradient":{"kind":"linear","angle":60,"stops":[{"offset":0,"color":"123456","opacity":1},{"offset":1,"color":"ABCDEF","opacity":0.4}]}}),
        json!({"gradient":{"kind":"radial","center":[0.25,0.75],"stops":[{"offset":0,"color":"123456","opacity":1},{"offset":1,"color":"ABCDEF","opacity":0.4}]}}),
        json!({"text_warp":"arch_up"}),
        json!({"text_warp":"wave1"}),
        json!({"text_warp":"inflate"}),
        json!({"text_warp":"arch_down"}),
        json!({"text_warp":"wave2"}),
        json!({"text_warp":"deflate"}),
        json!({"text_warp":"slant_up"}),
        json!({"text_warp":"slant_down"}),
        json!({"adjustments":[{"name":"adj","value":12000}]}),
    ] {
        let style = serde_json::to_value(serde_json::from_value::<aislide_core::visual::VisualStyle>(style).unwrap()).unwrap();
        let changed = transact(&document, json!([{"op":"add","path":"/deck/slides/0/elements/0/visual","value":style}])).unwrap();
        let bytes = saved(&changed["document"]);
        let reopened = open(&bytes);
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["visual"], style, "{style}");
        let cleared = transact(&reopened, json!([{"op":"remove","path":"/deck/slides/0/elements/0/visual"}])).unwrap();
        let cleared = open(&saved(&cleared["document"]));
        assert!(cleared["deck"]["slides"][0]["elements"][0]["visual"].is_null(), "{style}");
        assert_undo(&document, &changed, &source);
    }
}

#[test]
fn polygon_point_and_style_edit_is_native_and_undoable() {
    let polygon = json!({"type":"polygon","id":"polygon","x":100,"y":100,"width":300,"height":180,"points":[[0,0],[1,0],[0,1]],"fill":"123456","stroke":"000000","stroke_width":2});
    let source = export(deck(polygon)); let document = open(&source);
    let points = json!([[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]]);
    let changed = transact(&document, json!([{"op":"replace","path":"/deck/slides/0/elements/0/points","value":points},{"op":"replace","path":"/deck/slides/0/elements/0/fill","value":"ABCDEF"}])).unwrap();
    let reopened = open(&saved(&changed["document"]));
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["points"], points);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["fill"], "ABCDEF");
    assert_undo(&document, &changed, &source);
}

#[test]
fn svg_replacement_mask_and_transform_are_native_and_rendered() {
    let svg = |color: &str| STANDARD.encode(format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><rect width='24' height='24' fill='{color}'/></svg>"));
    let asset = execute_request(json!({"op":"create_asset","id":"svg","base64":svg("red"),"mime_type":"image/svg+xml","alt":"Vector","size":96})).unwrap();
    let source = export(deck(asset)); let document = open(&source);
    let next_svg = svg("blue"); let fallback = aislide_core::vector::prepare_svg(&next_svg).unwrap();
    let style = json!({"rotation":30.0,"flip_h":true,"opacity":0.5,"picture_mask":"ellipse"});
    let changed = transact(&document, json!([{"op":"replace","path":"/deck/slides/0/elements/0/svg","value":next_svg},{"op":"replace","path":"/deck/slides/0/elements/0/base64","value":fallback},{"op":"add","path":"/deck/slides/0/elements/0/visual","value":style}])).unwrap();
    let bytes = saved(&changed["document"]); let reopened = open(&bytes);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["svg"], next_svg);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["visual"], style);
    let package = Package::open(bytes).unwrap();
    let output_svg = package.parts().values().find(|bytes| **bytes == STANDARD.decode(&next_svg).unwrap()).unwrap();
    let rendered = aislide_core::vector::prepare_svg(&STANDARD.encode(output_svg)).unwrap();
    assert_eq!(rendered, fallback);
    let raster = image::load_from_memory(&STANDARD.decode(rendered).unwrap()).unwrap().into_rgba8();
    assert_eq!(raster.get_pixel(512,512).0, [0,0,255,255]);
    assert_undo(&document, &changed, &source);
}

#[test]
fn unknown_effects_are_preserved_and_cannot_be_replaced() {
    let mut package = Package::open(export(deck(shape()))).unwrap();
    let path = "ppt/slides/slide1.xml";
    let xml = package.text(path).unwrap().replace("</p:spPr>", "<a:effectLst><a:blur rad=\"9525\" grow=\"1\"/></a:effectLst><a:extLst><a:ext uri=\"urn:vendor\"><v:keep xmlns:v=\"urn:vendor\"/></a:ext></a:extLst></p:spPr>");
    package.replace_part(path, xml.into_bytes()).unwrap();
    let source = package.save().unwrap(); let document = open(&source);
    let changed = transact(&document, json!([{"op":"add","path":"/deck/slides/0/elements/0/visual","value":{"soft_edge":6}}]));
    assert!(changed.is_err());
    assert_eq!(saved(&document), source);
    let moved = transact(&document, json!([{"op":"replace","path":"/deck/slides/0/elements/0/x","value":120}])).unwrap();
    let bytes = saved(&moved["document"]); let package = Package::open(bytes).unwrap();
    assert!(package.text(path).unwrap().contains("urn:vendor"));
    assert!(package.text(path).unwrap().contains("grow=\"1\""));
    assert_undo(&document, &moved, &source);
}

#[test]
fn invalid_visual_values_and_unknown_fields_reject() {
    for style in [json!({"opacity":1.1}),json!({"rotation":1}),json!({"flip_h":"true"}),json!({"soft_edge":-1}),json!({"soft_edge":101}),json!({"shadow":{"color":"x","opacity":1,"blur":0,"distance":0,"angle":0}}),json!({"text_warp":"arbitraryXml"}),json!({"adjustments":[{"name":"formula","value":1}]}),json!({"picture_mask":"ellipse"}),json!({"unknown":true})] {
        let mut element = shape(); element["visual"] = style.clone();
        assert!(execute_request(json!({"op":"export","deck":deck(element)})).is_err(), "{style}");
    }
}

#[test]
fn bezier_path_is_native_bounded_and_editable() {
    let path = json!({"commands":[{"op":"move","point":[0.0,0.0]},{"op":"cubic","control1":[0.5,0.0],"control2":[1.0,0.5],"point":[1.0,1.0]},{"op":"quadratic","control":[0.0,1.0],"point":[0.0,0.0]},{"op":"close"}]});
    let mut element = json!({"type":"polygon","id":"curve","x":100,"y":100,"width":300,"height":180,"points":[[0.0,0.0],[0.5,0.0],[1.0,0.5],[1.0,1.0],[0.0,1.0],[0.0,0.0]],"fill":"123456","stroke":"000000","stroke_width":2,"visual":{"path":path}});
    let source = export(deck(element.clone())); let document = open(&source);
    let package = Package::open(source.clone()).unwrap();
    assert!(package.text("ppt/slides/slide1.xml").unwrap().contains("a:cubicBezTo"));
    assert!(package.text("ppt/slides/slide1.xml").unwrap().contains("a:quadBezTo"));
    assert_eq!(document["deck"]["slides"][0]["elements"][0]["visual"]["path"], path);
    element["points"][1][0] = json!(0.75); element["visual"]["path"]["commands"][1]["control1"][0] = json!(0.75);
    let changed = transact(&document, json!([{"op":"replace","path":"/deck/slides/0/elements/0","value":element}])).unwrap();
    assert_eq!(open(&saved(&changed["document"]))["deck"]["slides"][0]["elements"][0]["visual"]["path"]["commands"][1]["control1"][0], 0.75);
    assert_undo(&document, &changed, &source);
    for commands in [json!([]),json!([{"op":"close"}]),json!([{"op":"move","point":[-1,0]},{"op":"close"}]),json!([{"op":"move","point":[0,0]},{"op":"arc","radius":1},{"op":"close"}])] {
        element["visual"]["path"]["commands"] = commands;
        assert!(execute_request(json!({"op":"export","deck":deck(element.clone())})).is_err());
    }
}

#[test]
fn svg_processing_instructions_and_external_native_references_are_rejected() {
    let svg = STANDARD.encode("<?xml-stylesheet href='https://example.invalid/style.css'?><svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><rect width='24' height='24'/></svg>");
    assert!(aislide_core::vector::prepare_svg(&svg).is_err());
}

#[test]
fn nonzero_group_origin_normalizes_children_without_losing_native_origin() {
    let group = json!({"type":"group","id":"group","x":100,"y":100,"width":300,"height":200,"view_width":300,"view_height":200,"visual":{"rotation":30.0,"flip_h":true},"children":[{"type":"rect","id":"child","x":20,"y":20,"width":100,"height":80,"fill":"ABCDEF"}]});
    let mut package = Package::open(export(deck(group))).unwrap();
    let path = "ppt/slides/slide1.xml"; let mut xml = package.text(path).unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let group = parsed.descendants().find(|node| node.tag_name().name() == "grpSp").unwrap();
    let group_properties = group.children().find(|node| node.tag_name().name() == "grpSpPr").unwrap();
    let offset = group_properties.descendants().find(|node| node.tag_name().name() == "chOff").unwrap();
    let child_offset = group.children().find(|node| node.tag_name().name() == "sp").unwrap().descendants().find(|node| node.tag_name().name() == "off").unwrap();
    let mut edits = Vec::new();
    for (node, x, y) in [(offset,"952500","476250"),(child_offset,"1143000","666750")] {
        for (name,value) in [("x",x),("y",y)] { edits.push((node.attributes().find(|attribute| attribute.name() == name).unwrap().range_value(),value)); }
    }
    edits.sort_by_key(|(range,_)| range.start); for (range,value) in edits.into_iter().rev() { xml.replace_range(range,value); }
    package.replace_part(path,xml.into_bytes()).unwrap();
    let source = package.save().unwrap(); let document = open(&source);
    assert_eq!(document["deck"]["slides"][0]["elements"][0]["children"][0]["x"], 20.0);
    let changed = transact(&document,json!([{"op":"replace","path":"/deck/slides/0/elements/0/children/0/x","value":30}])).unwrap();
    let bytes = saved(&changed["document"]); let reopened = open(&bytes);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["children"][0]["x"],30.0);
    let package = Package::open(bytes).unwrap();
    assert!(package.text(path).unwrap().contains("1238250"));
    assert_undo(&document,&changed,&source);
}

#[test]
fn positive_fixed_angles_must_not_round_to_360_degrees() {
    for angle in [360.0,359.999999, -0.1] {
        let mut element = shape();
        element["visual"] = json!({"shadow":{"color":"123456","opacity":0.5,"blur":3,"distance":6,"angle":angle}});
        assert!(execute_request(json!({"op":"export","deck":deck(element.clone())})).is_err());
        element["visual"] = json!({"gradient":{"kind":"linear","angle":angle,"stops":[{"offset":0,"color":"123456","opacity":1},{"offset":1,"color":"ABCDEF","opacity":1}]}});
        assert!(execute_request(json!({"op":"export","deck":deck(element)})).is_err());
    }
}

#[test]
fn raster_edits_cannot_silently_leave_the_old_vector_active() {
    let svg = STANDARD.encode("<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><rect width='24' height='24' fill='red'/></svg>");
    let asset = execute_request(json!({"op":"create_asset","id":"svg","base64":svg,"mime_type":"image/svg+xml","alt":"Vector","size":96})).unwrap();
    let source = export(deck(asset)); let document = open(&source);
    let blue = aislide_core::vector::prepare_svg(&STANDARD.encode("<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><rect width='24' height='24' fill='blue'/></svg>")).unwrap();
    assert!(transact(&document,json!([{"op":"replace","path":"/deck/slides/0/elements/0/base64","value":blue}])).is_err());
    let mut replacement = document["deck"]["slides"][0]["elements"][0].clone();
    replacement["base64"] = json!(blue); replacement.as_object_mut().unwrap().remove("svg");
    let changed = transact(&document,json!([{"op":"replace","path":"/deck/slides/0/elements/0","value":replacement}])).unwrap();
    assert!(open(&saved(&changed["document"]))["deck"]["slides"][0]["elements"][0]["svg"].is_null());
    assert_undo(&document,&changed,&source);
}

#[test]
fn straight_paths_use_polygon_points_without_an_ignored_path_override() {
    let element = json!({"type":"polygon","id":"curve","x":100,"y":100,"width":300,"height":180,"points":[[0,0],[1,0],[0,1]],"fill":"123456","stroke":"000000","stroke_width":2,"visual":{"path":{"commands":[{"op":"move","point":[0,0]},{"op":"line","point":[1,0]},{"op":"line","point":[0,1]},{"op":"close"}]}}});
    assert!(execute_request(json!({"op":"export","deck":deck(element)})).is_err());
}

#[test]
fn transformed_text_group_connector_and_picture_masks_roundtrip_and_undo() {
    for element in [
        json!({"type":"text","id":"text","x":100,"y":100,"width":300,"height":180,"text":"Visual","font_size":24,"color":"000000","bold":false}),
        json!({"type":"group","id":"group","x":100,"y":100,"width":300,"height":200,"view_width":300,"view_height":200,"children":[{"type":"rect","id":"child","x":20,"y":20,"width":100,"height":80,"fill":"ABCDEF"}]}),
        json!({"type":"connector","id":"line","x":100,"y":100,"width":300,"height":1,"color":"000000","stroke_width":2,"arrow":false}),
    ] {
        let style = if element["type"] == "connector" { json!({"hidden":true,"locked":true,"soft_edge":2.0}) } else { json!({"rotation":35.0,"flip_v":true,"hidden":true,"locked":true,"soft_edge":2.0}) };
        let source = export(deck(element)); let document = open(&source);
        let changed = transact(&document,json!([{"op":"add","path":"/deck/slides/0/elements/0/visual","value":style}])).unwrap();
        assert_eq!(open(&saved(&changed["document"]))["deck"]["slides"][0]["elements"][0]["visual"],style);
        assert_undo(&document,&changed,&source);
    }
    for mask in ["ellipse","round_rect","diamond","hexagon"] {
        let svg = STANDARD.encode("<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><rect width='24' height='24'/></svg>");
        let asset = execute_request(json!({"op":"create_asset","id":"svg","base64":svg,"mime_type":"image/svg+xml","alt":"Vector","size":96})).unwrap();
        let source = export(deck(asset)); let document = open(&source);
        let changed = transact(&document,json!([{"op":"add","path":"/deck/slides/0/elements/0/visual","value":{"picture_mask":mask,"opacity":0.4,"rotation":20.0}},{"op":"replace","path":"/deck/slides/0/elements/0/crop/left","value":0.1}])).unwrap();
        let reopened = open(&saved(&changed["document"]));
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["visual"]["picture_mask"],mask);
        assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["crop"]["left"],0.1);
        assert_undo(&document,&changed,&source);
    }
}

#[test]
fn svg_resources_are_shared_authoritative_and_never_fetch_external_targets() {
    let svg = STANDARD.encode("<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><rect width='24' height='24' fill='red'/></svg>");
    let asset = execute_request(json!({"op":"create_asset","id":"svg","base64":svg,"mime_type":"image/svg+xml","alt":"Vector","size":96})).unwrap();
    let mut input = deck(asset.clone()); let mut second = asset; second["id"] = json!("second"); input["slides"][0]["elements"].as_array_mut().unwrap().push(second);
    let source = export(input); let package = Package::open(source.clone()).unwrap();
    assert_eq!(package.parts().keys().filter(|path| path.ends_with(".svg")).count(),1);
    let relationships = roxmltree::Document::parse(package.text("ppt/slides/_rels/slide1.xml.rels").unwrap()).unwrap();
    let ids: Vec<_> = relationships.descendants().filter_map(|node| node.attribute("Id")).collect();
    assert_eq!(ids.len(),ids.iter().collect::<std::collections::BTreeSet<_>>().len());
    let original = open(&source);
    let mut changed = package.clone();
    let path = changed.parts().keys().find(|path| path.ends_with(".svg")).unwrap().clone();
    changed.replace_part(&path,b"<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><rect width='24' height='24' fill='blue'/></svg>".to_vec()).unwrap();
    let current = open(&changed.save().unwrap());
    assert_ne!(original["hash"],current["hash"]);
    assert_ne!(original["deck"]["slides"][0]["elements"][0]["svg"],current["deck"]["slides"][0]["elements"][0]["svg"]);
    assert_eq!(current["deck"]["slides"][0]["elements"][0]["svg"],current["deck"]["slides"][0]["elements"][1]["svg"]);
    for bad in ["<svg xmlns='http://www.w3.org/2000/svg'><script>alert(1)</script></svg>","<svg xmlns='http://www.w3.org/2000/svg'><image href='https://example.invalid/image.png'/></svg>"] {
        changed.replace_part(&path,bad.as_bytes().to_vec()).unwrap();
        let bytes = changed.save().unwrap(); let document = open(&bytes);
        assert!(document["deck"]["slides"][0]["elements"].as_array().unwrap().is_empty());
        assert_eq!(saved(&document),bytes);
    }
    let mut changed = package.clone(); let path = "ppt/slides/_rels/slide1.xml.rels";
    let xml = changed.text(path).unwrap().replace("Target=\"../media/image1.svg\"", "Target=\"https://example.invalid/image.svg\" TargetMode=\"External\"");
    changed.replace_part(path,xml.into_bytes()).unwrap();
    let bytes = changed.save().unwrap(); let document = open(&bytes);
    assert!(document["deck"]["slides"][0]["elements"].as_array().unwrap().is_empty());
    assert_eq!(saved(&document),bytes);
}

#[test]
fn absent_visual_defaults_do_not_change_native_xml() {
    let source = export(deck(shape())); let document = open(&source);
    assert!(document["deck"]["slides"][0]["elements"][0]["visual"].is_null());
    let mut with_defaults = shape(); with_defaults["visual"] = json!({});
    assert_eq!(export(deck(with_defaults)),source);
    assert_eq!(saved(&document),source);
}

#[test]
#[ignore = "explicit local-only schema fixture output"]
fn write_visual_schema_fixture() {
    use std::io::Write;
    let path = std::env::var("AISLIDE_VISUAL_SCHEMA_OUTPUT").expect("explicit new fixture path");
    let mut element = shape();
    element["text"] = json!("Visual schema fixture");
    element["visual"] = json!({"flip_h":true,"hidden":true,"locked":true,"gradient":{"kind":"radial","center":[0.3,0.7],"stops":[{"offset":0,"color":"123456","opacity":1},{"offset":1,"color":"ABCDEF","opacity":0.5}]},"shadow":{"color":"112233","opacity":0.4,"blur":6,"distance":8,"angle":45},"glow":{"color":"445566","opacity":0.4,"radius":6},"soft_edge":2,"reflection":{"blur":3,"distance":6,"start_opacity":0.5,"end_opacity":0,"end_position":0.7},"text_warp":"arch_up","adjustments":[{"name":"adj","value":12000}]});
    let mut input = deck(element);
    let svg = STANDARD.encode("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><rect width='24' height='24' fill='blue'/></svg>");
    let mut asset = execute_request(json!({"op":"create_asset","id":"svg","base64":svg,"mime_type":"image/svg+xml","alt":"Vector","size":96})).unwrap();
    asset["visual"] = json!({"rotation":30,"picture_mask":"ellipse","opacity":0.4,"locked":true});
    input["slides"][0]["elements"].as_array_mut().unwrap().push(asset);
    let bytes = export(input);
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap(); file.write_all(&bytes).unwrap();
}

#[test]
fn phase2_slide_pixel_sampler_reads_composited_pixels_without_mutation() {
    let source = export(deck(json!({"type":"rect","id":"paint","x":100,"y":100,"width":200,"height":200,"fill":"FF0000","visual":{"opacity":0.5}})));
    let document = open(&source);
    let sample = |x: i32,y: i32| execute_request(json!({"op":"sample_slide_pixel","document":document,"slide_id":document["deck"]["slides"][0]["id"],"x":x,"y":y}));
    let pixel = sample(150,150).unwrap();
    assert_eq!(pixel["color"],"FF7F7F");
    assert_eq!(pixel["office_parity_verified"],false);
    assert_eq!(sample(10,10).unwrap()["color"],"FFFFFF");
    for (x,y) in [(-1,0),(1280,0),(0,720)] { assert!(sample(x,y).is_err()); }
    assert_eq!(saved(&document),source);
}

#[test]
fn phase2_svg_text_and_embedded_image_native_replacement_undo() {
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(2,2).write_to(&mut png,image::ImageFormat::Png).unwrap();
    let svg = STANDARD.encode(format!("<svg xmlns='http://www.w3.org/2000/svg' width='120' height='40'><text x='1' y='20' font-size='18'>Native SVG</text><image x='100' y='20' width='20' height='20' href='data:image/png;base64,{}'/></svg>",STANDARD.encode(png.into_inner())));
    let asset = execute_request(json!({"op":"create_asset","id":"svg","base64":svg,"mime_type":"image/svg+xml","alt":"Text and image","size":120})).unwrap();
    let source = export(deck(asset)); let document = open(&source);
    assert_eq!(document["deck"]["slides"][0]["elements"][0]["svg"],svg);
    let changed = transact(&document,json!([{"op":"replace","path":"/deck/slides/0/elements/0/x","value":200}])).unwrap();
    let reopened = open(&saved(&changed["document"]));
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["svg"],svg);
    assert_undo(&document,&changed,&source);
}