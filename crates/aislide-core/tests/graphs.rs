use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn graph() -> Value {
    json!({"version":1,"title":"Service topology","subtitle":"Synthetic example","nodes":[
        {"id":"client","label":"Client","kind":"rectangle","x":48,"y":160,"width":200,"height":96},
        {"id":"api","label":"API","kind":"rounded_rectangle","x":560,"y":256,"width":220,"height":112}
    ],"edges":[{"id":"request","source":"client","target":"api","label":"HTTPS","source_port":"right","target_port":"left","route":"straight"}],"groups":[]})
}

fn deck(element: Value) -> Value {
    json!({"version":1,"title":"Graph fixture","width":1280,"height":720,"slides":[{"id":"slide","title":"Graph","background":"FFFFFF","notes":"Synthetic fixture","elements":[element]}]})
}

fn node_icon(color: &str) -> Value {
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16"><rect width="32" height="16" fill="{color}"/></svg>"#);
    let picture = execute_request(json!({"op":"create_asset","id":"icon","base64":STANDARD.encode(svg),"mime_type":"image/svg+xml","alt":"Client icon","size":48})).unwrap();
    json!({"base64":picture["base64"],"mime_type":picture["mime_type"],"alt":picture["alt"]})
}

#[test]
fn graph_icon_preparation_bounds_svg_png_and_jpeg_without_upscaling_rasters() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16"><path d="M0 0h32v16H0z" fill="#007a4d"/></svg>"##;
    let prepared = execute_request(json!({"op":"create_graph_icon","base64":STANDARD.encode(svg),"mime_type":"image/svg+xml","alt":"Prepared SVG"})).unwrap();
    assert_eq!(prepared["mime_type"], "image/png");
    let info = aislide_core::media::inspect_raster(prepared["base64"].as_str().unwrap(), "image/png").unwrap();
    assert_eq!((info.width, info.height), (256, 128));
    for (format, mime) in [(image::ImageFormat::Png, "image/png"), (image::ImageFormat::Jpeg, "image/jpeg")] {
        for width in [64, 512] {
            let raster = image::RgbImage::from_pixel(width, width / 2, image::Rgb([0, 96, 192]));
            let mut output = std::io::Cursor::new(Vec::new());
            raster.write_to(&mut output, format).unwrap();
            let encoded = STANDARD.encode(output.into_inner());
            let icon = execute_request(json!({"op":"create_graph_icon","base64":encoded,"mime_type":mime,"alt":"Raster icon"})).unwrap();
            assert_eq!(icon["mime_type"], mime);
            assert_eq!(icon["alt"], "Raster icon");
            let info = aislide_core::media::inspect_raster(icon["base64"].as_str().unwrap(), mime).unwrap();
            assert_eq!((info.width, info.height), (width.min(256), width.min(256) / 2));
            if width < 256 { assert_eq!(icon["base64"], encoded); }
        }
    }
    assert!(execute_request(json!({"op":"create_graph_icon","base64":STANDARD.encode("<svg xmlns='http://www.w3.org/2000/svg'><script/></svg>"),"mime_type":"image/svg+xml"})).is_err());
}

#[test]
fn graph_node_icons_render_as_native_pictures_with_separate_labels_and_connections() {
    let mut spec = graph();
    spec["nodes"][0]["icon"] = node_icon("#0017c1");
    let element = execute_request(json!({"op":"create_graph","id":"icons","spec":spec})).unwrap();
    let children = element["children"].as_array().unwrap();
    let picture = children.iter().find(|child| child["type"] == "picture").unwrap();
    let label = children.iter().find(|child| child["id"].as_str().unwrap().ends_with("-nt-client")).unwrap();
    let connector = children.iter().find(|child| child["type"] == "connector").unwrap();
    assert_eq!(picture["base64"], spec["nodes"][0]["icon"]["base64"]);
    assert_eq!(picture["alt"], "Client icon");
    assert_eq!(picture["width"].as_f64().unwrap() / picture["height"].as_f64().unwrap(), 2.0);
    assert!(picture["x"].as_f64().unwrap() >= 48.0);
    assert!(picture["y"].as_f64().unwrap() >= 160.0);
    assert!(picture["y"].as_f64().unwrap() + picture["height"].as_f64().unwrap() <= 256.0);
    assert!(picture["x"].as_f64().unwrap() + picture["width"].as_f64().unwrap() < label["x"].as_f64().unwrap());
    assert!(label["x"].as_f64().unwrap() + label["width"].as_f64().unwrap() <= 248.0);
    assert!(connector["start"]["element_id"].as_str().unwrap().ends_with("-n-client"));
    let saved = execute_request(json!({"op":"export","deck":deck(element)})).unwrap();
    let bytes = STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap();
    let package = Package::open(bytes).unwrap();
    assert!(package.text("ppt/slides/slide1.xml").unwrap().contains("<p:pic>"));
    assert_eq!(package.parts().keys().filter(|name| name.starts_with("ppt/media/")).count(), 1);
    let opened = execute_request(json!({"op":"open_presentation","id":"icon-open","base64":saved["base64"]})).unwrap();
    let restored = opened["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().iter().find(|child| child["type"] == "picture").unwrap();
    assert_eq!(restored["base64"], spec["nodes"][0]["icon"]["base64"]);
    let mut default_width = graph();
    default_width["nodes"][0]["width"] = json!(176);
    default_width["nodes"][0]["label"] = json!("Application");
    default_width["nodes"][0]["icon"] = node_icon("#0017c1");
    let compact = execute_request(json!({"op":"create_graph","id":"compact","spec":default_width})).unwrap();
    let compact_label = compact["children"].as_array().unwrap().iter().find(|child| child["id"].as_str().unwrap().ends_with("-nt-client")).unwrap();
    assert!(compact_label["width"].as_f64().unwrap() >= 97.0);
}

#[test]
fn graph_icons_reject_invalid_images_and_preserve_icon_free_output() {
    let icon = node_icon("#007a4d");
    for replacement in [
        json!({"base64":"https://example.invalid/icon.png","mime_type":"image/png","alt":"URL"}),
        json!({"base64":STANDARD.encode("<svg/>"),"mime_type":"image/svg+xml","alt":"Raw SVG"}),
        json!({"base64":icon["base64"],"mime_type":"image/jpeg","alt":"Wrong MIME"}),
        json!({"base64":icon["base64"],"mime_type":"image/png","alt":"x".repeat(501)}),
        json!({"base64":icon["base64"],"mime_type":"image/png","alt":"External","url":"https://example.invalid/icon.png"}),
    ] {
        let mut spec = graph(); spec["nodes"][0]["icon"] = replacement;
        assert!(execute_request(json!({"op":"create_graph","id":"bad-icon","spec":spec})).is_err());
        assert!(execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"move","ids":["client"],"dx":8,"dy":0}]})).is_err());
    }
    let mut excessive = graph();
    excessive["nodes"] = json!((0..4).map(|index| json!({"id":format!("node-{index}"),"label":"Node","x":48,"y":160,"icon":{"base64":"A".repeat(800000),"mime_type":"image/png"}})).collect::<Vec<_>>());
    excessive["edges"] = json!([]);
    assert!(execute_request(json!({"op":"create_graph","id":"budget","spec":excessive})).unwrap_err().to_string().contains("3 MiB"));
    let original = execute_request(json!({"op":"create_graph","id":"unchanged","spec":graph()})).unwrap();
    let mut without_icon = graph(); without_icon["nodes"][0]["icon"] = Value::Null;
    assert_eq!(original, execute_request(json!({"op":"create_graph","id":"unchanged","spec":without_icon})).unwrap());
}

#[test]
fn graph_icons_follow_nodes_for_all_shapes_without_changing_connection_sites() {
    let icon = node_icon("#0017c1");
    for kind in ["rectangle", "rounded_rectangle", "ellipse", "diamond", "cylinder", "cloud"] {
        let mut spec = graph(); spec["nodes"][0]["kind"] = json!(kind);
        let plain = execute_request(json!({"op":"create_graph","id":"plain","spec":spec})).unwrap();
        spec["nodes"][0]["icon"] = icon.clone();
        let original = execute_request(json!({"op":"create_graph","id":"icon","spec":spec})).unwrap();
        let children = original["children"].as_array().unwrap();
        let picture = children.iter().find(|child| child["type"] == "picture").unwrap();
        let plain_edge = plain["children"].as_array().unwrap().iter().find(|child| child["type"] == "connector").unwrap();
        let icon_edge = children.iter().find(|child| child["type"] == "connector").unwrap();
        assert_eq!(plain_edge["start"]["site"], icon_edge["start"]["site"], "{kind}");
        assert_eq!(plain_edge["x"], icon_edge["x"], "{kind}");
        let moved = execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"move","ids":["client"],"dx":24,"dy":16}]})).unwrap();
        assert_eq!(moved["nodes"][0]["icon"], icon);
        let rendered = execute_request(json!({"op":"create_graph","id":"moved","spec":moved})).unwrap();
        let shifted = rendered["children"].as_array().unwrap().iter().find(|child| child["type"] == "picture").unwrap();
        assert_eq!(shifted["x"].as_f64().unwrap(), picture["x"].as_f64().unwrap() + 24.0);
        assert_eq!(shifted["y"].as_f64().unwrap(), picture["y"].as_f64().unwrap() + 16.0);
    }
}

#[test]
fn graph_icon_metadata_reopens_replaces_removes_and_undoes_with_stale_image_protection() {
    let mut spec = graph(); spec["nodes"][0]["icon"] = node_icon("#0017c1");
    let document = execute_request(json!({"op":"create_presentation","id":"graph-icons","title":"Icon history"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide-1","id":"architecture","spec":spec})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"icons-open","base64":saved["base64"]})).unwrap();
    assert_eq!(opened["document"]["parts"][0]["stale"], false);
    assert_eq!(opened["document"]["parts"][0]["spec"]["data"]["graph"]["nodes"][0]["icon"], spec["nodes"][0]["icon"]);
    let changed_icon = node_icon("#007a4d");
    let mut replacement = spec.clone(); replacement["nodes"][0]["icon"] = changed_icon.clone();
    let updated = execute_request(json!({"op":"update_graph","document":opened["document"],"expected_revision":0,"slide_id":"slide-1","id":"architecture","spec":replacement})).unwrap();
    let updated_saved = execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
    let updated_opened = execute_request(json!({"op":"open_presentation","id":"icons-updated","base64":updated_saved["base64"]})).unwrap();
    assert_eq!(updated_opened["document"]["parts"][0]["stale"], false);
    assert_eq!(updated_opened["document"]["parts"][0]["spec"]["data"]["graph"]["nodes"][0]["icon"], changed_icon);
    replacement["nodes"][0]["icon"] = Value::Null;
    let removed = execute_request(json!({"op":"update_graph","document":updated["document"],"expected_revision":1,"slide_id":"slide-1","id":"architecture","spec":replacement})).unwrap();
    assert!(removed["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().iter().all(|child| child["type"] != "picture"));
    let undone = execute_request(json!({"op":"undo_transaction","document":removed["document"],"expected_revision":2,"receipt":removed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], updated["document"]["hash"]);
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], updated_saved["base64"]);
    let mut package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    let image_path = package.parts().keys().find(|name| name.starts_with("ppt/media/")).unwrap().clone();
    package.replace_part(&image_path, STANDARD.decode(changed_icon["base64"].as_str().unwrap()).unwrap()).unwrap();
    let external = execute_request(json!({"op":"open_presentation","id":"external-icon","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    assert_eq!(external["document"]["parts"][0]["stale"], true);
    assert!(execute_request(json!({"op":"update_graph","document":external["document"],"expected_revision":0,"slide_id":"slide-1","id":"architecture","spec":spec})).unwrap_err().to_string().contains("stale"));
}

#[test]
fn graph_api_creates_native_attached_nodes_and_recomputes_moved_endpoints() {
    let spec = graph();
    let first = execute_request(json!({"op":"create_graph","id":"topology","spec":spec})).unwrap();
    assert_eq!(first["type"], "group");
    assert_eq!(first, execute_request(json!({"op":"create_graph","id":"topology","spec":spec})).unwrap());
    let edge = first["children"].as_array().unwrap().iter().find(|element| element["type"] == "connector").unwrap();
    assert!(edge["start"]["element_id"].is_string());
    assert!(edge["end"]["element_id"].is_string());
    assert_eq!(edge["x"], 248.0);
    assert_eq!(edge["width"], 312.0);
    let mut moved = spec;
    moved["nodes"][1]["x"] = json!(720);
    let result = execute_request(json!({"op":"create_graph","id":"topology","spec":moved})).unwrap();
    let moved_edge = result["children"].as_array().unwrap().iter().find(|element| element["type"] == "connector").unwrap();
    assert_eq!(moved_edge["width"], 472.0);
    let exported = execute_request(json!({"op":"export","deck":deck(first)})).unwrap();
    let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap();
    assert!(xml.contains("<p:cxnSp>"));
    assert!(xml.contains("<a:stCxn"));
    assert!(xml.contains("<a:endCxn"));
    assert!(!package.parts().keys().any(|path| path.starts_with("ppt/media/")));
}

#[test]
fn graph_contract_rejects_ambiguous_ids_missing_endpoints_and_excessive_counts() {
    for modify in ["duplicate", "endpoint", "bounds", "count", "group"] {
        let mut spec = graph();
        match modify {
            "duplicate" => spec["nodes"][1]["id"] = json!("client"),
            "endpoint" => spec["edges"][0]["target"] = json!("missing"),
            "bounds" => spec["nodes"][0]["x"] = json!(-1),
            "count" => spec["nodes"] = json!((0..49).map(|index| json!({"id":format!("node-{index}"),"label":"Node","kind":"rectangle","x":48,"y":160,"width":200,"height":96})).collect::<Vec<_>>()),
            "group" => spec["nodes"][0]["group"] = json!("missing"),
            _ => unreachable!(),
        }
        assert!(execute_request(json!({"op":"create_graph","id":"invalid","spec":spec})).is_err(), "{modify}");
    }
    let catalog = execute_request(json!({"op":"graph_catalog"})).unwrap();
    assert_eq!(catalog["limits"]["nodes"], 48);
    assert!(catalog["examples"].as_array().is_some_and(|examples| examples.len() >= 3));
    assert_eq!(execute_request(json!({"op":"part_catalog"})).unwrap()["presets"].as_array().unwrap().len(), 108);
}

#[test]
fn routed_reverse_edges_preserve_geometry_arrows_and_native_connections() {
    let mut spec = graph();
    spec["edges"][0] = json!({"id":"reply","source":"api","target":"client","source_port":"left","target_port":"right","route":"elbow","label":"Reply","start_arrow":true,"dashed":true});
    let element = execute_request(json!({"op":"create_graph","id":"reverse","spec":spec})).unwrap();
    let edge = element["children"].as_array().unwrap().iter().find(|element| element["type"] == "connector").unwrap();
    let points = edge["routing"]["points"].as_array().unwrap();
    assert!(points.len() >= 3);
    assert_eq!(points.first().unwrap()[0], 1.0);
    assert_eq!(points.last().unwrap()[0], 0.0);
    assert!(points.windows(2).all(|pair| pair[0][0] == pair[1][0] || pair[0][1] == pair[1][1]));
    let exported = execute_request(json!({"op":"export","deck":deck(element.clone())})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"reverse-open","base64":exported["base64"]})).unwrap();
    let restored = opened["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().iter().find(|element| element["type"] == "connector").unwrap();
    assert_eq!(restored["routing"], edge["routing"]);
    assert_eq!(restored["arrow"], true);
    let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap();
    assert!(xml.contains("<a:prstGeom prst=\"bentConnector3\""));
    assert!(xml.contains("<a:headEnd type=\"triangle\""));
    assert!(xml.contains("<a:prstDash val=\"dash\""));
}

#[test]
fn graph_routes_use_office_presets_in_every_port_direction() {
    for route in ["straight", "elbow"] {
        for source_port in ["top", "left", "bottom", "right"] {
            for target_port in ["top", "left", "bottom", "right"] {
                let mut spec = graph();
                spec["edges"][0] = json!({"id":"route","source":"api","target":"client","source_port":source_port,"target_port":target_port,"route":route,"start_arrow":true,"dashed":true});
                let element = execute_request(json!({"op":"create_graph","id":"ports","spec":spec})).unwrap();
                let edge = element["children"].as_array().unwrap().iter().find(|element| element["type"] == "connector").unwrap();
                let exported = execute_request(json!({"op":"export","deck":deck(element.clone())})).unwrap();
                let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
                let xml = package.text("ppt/slides/slide1.xml").unwrap();
                assert!(!xml.contains("<a:custGeom>"), "{route} {source_port} {target_port}");
                assert!(xml.contains("Connector"), "{route} {source_port} {target_port}");
                let opened = execute_request(json!({"op":"open_presentation","id":"ports-open","base64":exported["base64"]})).unwrap();
                let restored = opened["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().iter().find(|element| element["type"] == "connector").unwrap();
                assert_eq!(restored["routing"], edge["routing"], "{route} {source_port} {target_port}");
                assert_eq!(restored["start"], edge["start"]);
                assert_eq!(restored["end"], edge["end"]);
                assert_eq!(restored["arrow"], true);
                assert_eq!(restored["flip_v"], false);
            }
        }
    }
}

#[test]
fn routed_connectors_reject_unrepresentable_paths_before_export() {
    for points in [
        json!([[0,0],[0.25,0.25],[1,1]]),
        json!([[0.1,0],[1,1]]),
        json!([[0,0],[0.123456,0],[0.123456,1],[1,1]]),
        json!([[0,0],[0.25,0],[0.25,0.75],[1,0.75],[1,1]]),
    ] {
        let mut element = execute_request(json!({"op":"create_graph","id":"invalid-route","spec":graph()})).unwrap();
        let edge = element["children"].as_array_mut().unwrap().iter_mut().find(|element| element["type"] == "connector").unwrap();
        edge["routing"]["points"] = points;
        assert!(execute_request(json!({"op":"export","deck":deck(element)})).is_err());
    }
}

#[test]
fn graph_ports_match_native_shape_connection_sites() {
    use aislide_core::graphs::{GraphNode, Port, endpoint};
    let other: GraphNode = serde_json::from_value(graph()["nodes"][1].clone()).unwrap();
    for (kind, sites, positions) in [
        ("rectangle", [0,1,2,3], [[0.5,0.0],[0.0,0.5],[0.5,1.0],[1.0,0.5]]),
        ("rounded_rectangle", [0,1,2,3], [[0.5,0.0],[0.0,0.5],[0.5,1.0],[1.0,0.5]]),
        ("diamond", [0,1,2,3], [[0.5,0.0],[0.0,0.5],[0.5,1.0],[1.0,0.5]]),
        ("ellipse", [0,2,4,6], [[0.5,0.0],[0.0,0.5],[0.5,1.0],[1.0,0.5]]),
        ("cylinder", [1,2,3,4], [[0.5,0.0],[0.0,0.5],[0.5,1.0],[1.0,0.5]]),
        ("cloud", [3,2,1,0], [[0.5,1235.0/21600.0],[67.0/21600.0,0.5],[0.5,21577.0/21600.0],[21582.0/21600.0,0.5]]),
    ] {
        let node: GraphNode = serde_json::from_value(json!({"id":"node","label":"Node","kind":kind,"x":48,"y":160,"width":200,"height":96})).unwrap();
        for (index, port) in [Port::Top, Port::Left, Port::Bottom, Port::Right].into_iter().enumerate() {
            let actual = endpoint(&node, port, &other);
            assert_eq!(actual.2, sites[index], "{kind} {port:?}");
            assert!((actual.0 - node.x - positions[index][0] * node.width).abs() < 1e-9);
            assert!((actual.1 - node.y - positions[index][1] * node.height).abs() < 1e-9);
            let mut spec = graph(); spec["nodes"][0]["kind"] = json!(kind);
            spec["edges"][0]["source_port"] = serde_json::to_value(port).unwrap();
            let element = execute_request(json!({"op":"create_graph","id":"sites","spec":spec})).unwrap();
            let saved = execute_request(json!({"op":"export","deck":deck(element)})).unwrap();
            execute_request(json!({"op":"open_presentation","id":"sites-open","base64":saved["base64"]})).unwrap();
        }
    }
}

#[test]
fn element_commands_duplicate_managed_graphs_and_remove_attached_edges_atomically() {
    let document = execute_request(json!({"op":"create_presentation","id":"element-commands","title":"Objects"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide-1","id":"architecture","spec":graph()})).unwrap();
    let duplicated = execute_request(json!({"op":"edit_elements","document":inserted["document"],"expected_revision":1,"slide_id":"slide-1","operations":[{"op":"duplicate","id":"architecture","new_id":"copy"}]})).unwrap();
    assert_eq!(duplicated["document"]["parts"].as_array().unwrap().len(), 2);
    assert!(duplicated["document"]["parts"].as_array().unwrap().iter().all(|part| part["stale"] == false));
    let removed = execute_request(json!({"op":"edit_elements","document":duplicated["document"],"expected_revision":2,"slide_id":"slide-1","operations":[{"op":"remove","id":"copy"}]})).unwrap();
    assert_eq!(removed["document"]["parts"].as_array().unwrap().len(), 1);
    let undone = execute_request(json!({"op":"undo_transaction","document":removed["document"],"expected_revision":3,"receipt":removed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], duplicated["document"]["hash"]);
}

#[test]
fn managed_graphs_share_parts_history_and_preserve_native_edits() {
    let mut scene = deck(json!({})); scene["slides"][0]["elements"] = json!([]);
    let document = execute_request(json!({"op":"new_document","id":"managed","deck":scene})).unwrap();
    let inserted = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide","id":"architecture","spec":graph()})).unwrap();
    assert_eq!(inserted["document"]["parts"][0]["spec"]["preset"], "diagram/custom");
    let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"managed-open","base64":saved["base64"]})).unwrap();
    assert_eq!(opened["document"]["parts"][0]["stale"], false);
    let mut spec = graph(); spec["nodes"][1]["x"] = json!(720); spec["edges"][0]["route"] = json!("elbow");
    let updated = execute_request(json!({"op":"update_graph","document":opened["document"],"expected_revision":0,"slide_id":"slide","id":"architecture","spec":spec})).unwrap();
    let updated_saved = execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
    let updated_opened = execute_request(json!({"op":"open_presentation","id":"updated","base64":updated_saved["base64"]})).unwrap();
    assert_eq!(updated_opened["document"]["parts"][0]["stale"], false);
    assert_eq!(updated_opened["document"]["parts"][0]["spec"]["data"]["graph"]["nodes"][1]["x"], 720.0);
    let undone = execute_request(json!({"op":"undo_transaction","document":updated["document"],"expected_revision":1,"receipt":updated["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], opened["document"]["hash"]);
    assert_eq!(execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap()["base64"], saved["base64"]);
    let mut package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().replace(">API<", ">Changed externally<");
    package.replace_part("ppt/slides/slide1.xml", xml.into_bytes()).unwrap();
    let external = execute_request(json!({"op":"open_presentation","id":"external","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    assert_eq!(external["document"]["parts"][0]["stale"], true);
    assert!(execute_request(json!({"op":"update_graph","document":external["document"],"expected_revision":0,"slide_id":"slide","id":"architecture","spec":graph()})).unwrap_err().to_string().contains("stale"));
}

#[test]
fn graph_operations_move_groups_align_and_remove_incident_edges_atomically() {
    let mut spec = graph();
    spec["groups"] = json!([{"id":"zone","label":"Boundary","x":16,"y":104,"width":300,"height":240}]);
    spec["nodes"][0]["group"] = json!("zone");
    let moved = execute_request(json!({"op":"transform_graph","spec":spec,"operations":[{"op":"move","ids":["zone","client"],"dx":32,"dy":8}]})).unwrap();
    assert_eq!(moved["nodes"][0]["x"], 80.0);
    assert_eq!(moved["groups"][0]["x"], 48.0);
    let removed = execute_request(json!({"op":"transform_graph","spec":moved,"operations":[{"op":"remove","ids":["client"]}]})).unwrap();
    assert_eq!(removed["nodes"].as_array().unwrap().len(), 1);
    assert!(removed["edges"].as_array().unwrap().is_empty());
    let aligned = execute_request(json!({"op":"transform_graph","spec":graph(),"operations":[{"op":"align","ids":["client","api"],"alignment":"top"}]})).unwrap();
    assert_eq!(aligned["nodes"][0]["y"], aligned["nodes"][1]["y"]);
    let arranged = execute_request(json!({"op":"transform_graph","spec":graph(),"operations":[{"op":"layout","columns":2}]})).unwrap();
    assert_ne!(arranged["nodes"][0]["x"], arranged["nodes"][1]["x"]);
    execute_request(json!({"op":"create_graph","id":"arranged","spec":arranged})).unwrap();
    assert!(execute_request(json!({"op":"transform_graph","spec":graph(),"operations":[{"op":"move","ids":["client"],"dx":10000,"dy":0}]})).is_err());
}