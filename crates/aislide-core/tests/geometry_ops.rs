use aislide_core::vector::{PathCommand, VectorPath};
use aislide_core::execute_request;
use serde_json::{json, Value};

fn contours() -> VectorPath {
    use PathCommand::{Close, Line, Move};
    VectorPath { commands: vec![
        Move { point: [0.0, 0.0] }, Line { point: [1.0, 0.0] },
        Line { point: [1.0, 1.0] }, Line { point: [0.0, 1.0] }, Close,
        Move { point: [0.25, 0.25] }, Line { point: [0.25, 0.75] },
        Line { point: [0.75, 0.75] }, Line { point: [0.75, 0.25] }, Close,
    ] }
}

#[test]
fn closed_compound_paths_preserve_contours_and_reject_open_sequences() {
    let path = contours();
    assert!(path.validate().is_ok(), "closed outer and hole contours must be representable");
    let mut open = path.clone();
    open.commands.remove(4);
    assert!(open.validate().is_err());
    let mut dangling = path.clone();
    dangling.commands.insert(5, PathCommand::Line { point: [0.1, 0.1] });
    assert!(dangling.validate().is_err());
    let mut empty = path;
    empty.commands.splice(5..9, [PathCommand::Move { point: [0.5, 0.5] }]);
    assert!(empty.validate().is_err());
}

fn rectangle(id: &str, x: f64, y: f64, width: f64, height: f64) -> Value {
    json!({"type":"polygon","id":id,"x":x,"y":y,"width":width,"height":height,
        "points":[[0,0],[1,0],[1,1],[0,1]],"fill":"@accent2","stroke":"@dk1","stroke_width":2})
}

fn new_document(elements: Value) -> Value {
    execute_request(json!({"op":"new_document","id":"geometry-test","deck":{
        "version":1,"title":"Synthetic geometry","width":1280,"height":720,
        "slides":[{"id":"slide","title":"Shapes","background":"FFFFFF","notes":"","elements":elements}]}})).unwrap()
}

fn combine(document: &Value, operation: &str) -> aislide_core::Result<Value> {
    execute_request(json!({"op":"combine_shapes","document":document,"expected_revision":document["revision"],
        "slide_id":document["deck"]["slides"][0]["id"],"ids":["first","second"],"operation":operation,"result_id":"combined"}))
}

fn area(element: &Value) -> f64 {
    let rings: Vec<Vec<[f64; 2]>> = if element["visual"]["path"].is_object() {
        let path: VectorPath = serde_json::from_value(element["visual"]["path"].clone()).unwrap();
        let mut rings = Vec::new();
        let mut ring = Vec::new();
        for command in path.commands {
            match command {
                PathCommand::Move { point } | PathCommand::Line { point } => ring.push(point),
                PathCommand::Close => rings.push(std::mem::take(&mut ring)),
                _ => panic!("boolean geometry must not contain curves"),
            }
        }
        rings
    } else { vec![serde_json::from_value(element["points"].clone()).unwrap()] };
    rings.iter().map(|ring| ring.iter().zip(ring.iter().cycle().skip(1)).map(|(start,end)| start[0]*end[1]-end[0]*start[1]).sum::<f64>() / 2.0).sum::<f64>().abs()
        * element["width"].as_f64().unwrap() * element["height"].as_f64().unwrap()
}

#[test]
fn protocol_boolean_areas_style_and_single_transaction() {
    let mut second = rectangle("second", 200.0, 100.0, 200.0, 200.0);
    second["fill"] = json!("@accent3");
    let document = new_document(json!([rectangle("first",100.0,100.0,200.0,200.0),second]));
    for document in [document.clone(),open(&save(&document))] {
    for (operation, expected) in [("union",60000.0),("intersect",20000.0),("subtract",20000.0),("xor",40000.0)] {
        let result = combine(&document,operation).unwrap();
        let transaction = &result["transaction"];
        let output = &transaction["document"]["deck"]["slides"][0]["elements"];
        assert_eq!(output.as_array().unwrap().len(),1);
        assert_eq!(output[0]["id"],"combined");
        assert_eq!(output[0]["fill"],"@accent2");
        assert_eq!(output[0]["stroke_width"],2.0);
        assert!((area(&output[0])-expected).abs()<0.1,"{operation}");
        assert_eq!(result["effects"]["removed_ids"],json!(["first","second"]));
        assert_eq!(transaction["document"]["revision"].as_u64(),Some(document["revision"].as_u64().unwrap()+1));
        let undone = execute_request(json!({"op":"undo_transaction","document":transaction["document"],
            "expected_revision":transaction["document"]["revision"],"receipt":transaction["receipt"]})).unwrap();
        assert_eq!(undone["document"]["hash"],document["hash"]);
    }
    }
}

fn save(document: &Value) -> Value {
    execute_request(json!({"op":"export_presentation","document":document})).unwrap()["base64"].clone()
}

fn open(bytes: &Value) -> Value {
    execute_request(json!({"op":"open_presentation","id":"geometry-test","base64":bytes})).unwrap()["document"].clone()
}

#[test]
fn holes_and_disconnected_regions_native_reopen_and_undo() {
    for (operation, second, expected) in [
        ("subtract",rectangle("second",150.0,150.0,100.0,100.0),30000.0),
        ("union",rectangle("second",400.0,100.0,100.0,100.0),50000.0),
    ] {
        let source = save(&new_document(json!([rectangle("first",100.0,100.0,200.0,200.0),second])));
        let document = open(&source);
        let result = combine(&document,operation).unwrap();
        let transaction = &result["transaction"];
        let reopened = open(&save(&transaction["document"]));
        let output = &reopened["deck"]["slides"][0]["elements"][0];
        assert!(output["visual"]["path"].is_object());
        assert!((area(output)-expected).abs()<0.1);
        assert_eq!(reopened["deck"]["slides"][0]["elements"].as_array().unwrap().len(),1);
        let undone = execute_request(json!({"op":"undo_transaction","document":transaction["document"],
            "expected_revision":transaction["document"]["revision"],"receipt":transaction["receipt"]})).unwrap();
        assert_eq!(save(&undone["document"]),source);
        assert_eq!(save(&document),source);
    }
}

#[test]
fn winding_and_repeated_boolean_operations_preserve_holes() {
    let mut first = rectangle("first",100.0,100.0,200.0,200.0);
    first["points"].as_array_mut().unwrap().reverse();
    first["visual"] = json!({"opacity":0.5});
    let document = new_document(json!([first,rectangle("second",150.0,150.0,100.0,100.0)]));
    let result = combine(&document,"subtract").unwrap();
    let element = &result["transaction"]["document"]["deck"]["slides"][0]["elements"][0];
    assert_eq!(element["visual"]["opacity"],0.5);
    assert!((area(element)-30000.0).abs()<0.1);
    let mut compound = element.clone(); compound["id"] = json!("first");
    let next = new_document(json!([compound,rectangle("second",400.0,100.0,100.0,100.0)]));
    assert!((area(&combine(&next,"union").unwrap()["transaction"]["document"]["deck"]["slides"][0]["elements"][0])-40000.0).abs()<0.1);
}

#[test]
fn unsupported_geometry_and_empty_results_leave_document_unchanged() {
    let first = rectangle("first",100.0,100.0,200.0,200.0);
    let second = rectangle("second",150.0,150.0,100.0,100.0);
    let mut invalid = Vec::new();
    for visual in [json!({"rotation":30}),json!({"flip_h":true}),json!({"locked":true}),json!({"hidden":true}),
        json!({"shadow":{"color":"000000","opacity":0.5,"blur":5,"distance":3,"angle":45}}),
        json!({"gradient":{"kind":"linear","angle":0,"stops":[{"offset":0,"color":"@accent2","opacity":1},{"offset":1,"color":"@accent3","opacity":1}]}})] {
        let mut shape = first.clone(); shape["visual"] = visual; invalid.push(shape);
    }
    let mut no_fill = first.clone(); no_fill["fill"] = json!("none"); invalid.push(no_fill);
    let mut bowtie = first.clone(); bowtie["points"] = json!([[0,0],[1,1],[0,1],[1,0]]); invalid.push(bowtie);
    let mut flat = first.clone(); flat["points"] = json!([[0,0],[0.5,0.5],[1,1]]); invalid.push(flat);
    for shape in invalid {
        let document = new_document(json!([shape,second]));
        let before = document.clone();
        assert!(combine(&document,"union").is_err());
        assert_eq!(document,before);
    }
    let identical = new_document(json!([first,rectangle("second",100.0,100.0,200.0,200.0)]));
    for operation in ["subtract","xor"] { assert!(combine(&identical,operation).unwrap_err().to_string().contains("empty")); }
    let disjoint = new_document(json!([first,rectangle("second",500.0,100.0,100.0,100.0)]));
    assert!(combine(&disjoint,"intersect").unwrap_err().to_string().contains("empty"));
}

#[test]
fn strict_protocol_rejects_bad_ids_revisions_and_operations() {
    let document = new_document(json!([rectangle("first",100.0,100.0,200.0,200.0),rectangle("second",150.0,150.0,100.0,100.0)]));
    let request = json!({"op":"combine_shapes","document":document,"expected_revision":document["revision"],"slide_id":"slide","ids":["first","second"],"operation":"union","result_id":"combined"});
    for (key,value) in [("ids",json!(["first","first"])),("ids",json!(["first","missing"])),("ids",json!(["first"])),("ids",json!(vec!["first";33])),
        ("expected_revision",json!(999)),("result_id",json!("first")),("result_id",json!("")),("result_id",json!("x".repeat(81))),
        ("slide_id",json!("missing")),("operation",json!("invalid_operation")),("unknown",json!(true))] {
        let mut invalid = request.clone(); invalid[key] = value;
        assert!(execute_request(invalid).is_err(),"{key}");
    }
}

#[test]
fn connected_shapes_and_unknown_native_effects_are_not_silently_deleted() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let first = json!({"type":"rect","id":"first","x":100,"y":100,"width":200,"height":200,"fill":"@accent2"});
    let second = rectangle("second",150.0,150.0,100.0,100.0);
    let document = new_document(json!([first,second,{"type":"connector","id":"link","x":100,"y":100,"width":200,"height":100,"color":"000000","stroke_width":2,"arrow":true,"start":{"element_id":"first","site":0}}]));
    assert!(combine(&document,"union").is_err());
    let source = save(&document); let native = open(&source);
    assert!(combine(&native,"union").is_err());
    assert_eq!(save(&native),source);
    let deck = new_document(json!([first,second]))["deck"].clone();
    let exported = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let mut package = aislide_core::package::Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let path = "ppt/slides/slide1.xml";
    let xml = package.text(path).unwrap().replacen("</p:spPr>","<a:effectLst><a:blur rad=\"9525\" grow=\"1\"/></a:effectLst></p:spPr>",1);
    package.replace_part(path,xml.into_bytes()).unwrap();
    let modified = json!(STANDARD.encode(package.save().unwrap()));
    let native = open(&modified);
    let ids: Vec<_> = native["deck"]["slides"][0]["elements"].as_array().unwrap().iter().map(|element| element["id"].clone()).collect();
    let result = execute_request(json!({"op":"combine_shapes","document":native,"expected_revision":native["revision"],"slide_id":native["deck"]["slides"][0]["id"],"ids":ids,"operation":"union","result_id":"combined"}));
    assert!(result.unwrap_err().to_string().contains("not represented"));
    assert_eq!(save(&native),modified);
}

fn many_points(id: &str, count: usize, x: f64) -> Value {
    let mut element = rectangle(id,x,100.0,200.0,200.0);
    element["points"] = json!((0..count).map(|index| {
        let angle = index as f64 * std::f64::consts::TAU / count as f64;
        [0.5+0.5*angle.cos(),0.5+0.5*angle.sin()]
    }).collect::<Vec<_>>());
    element
}

#[test]
fn vertex_command_and_clipping_work_budgets_reject_without_truncation() {
    let too_many = new_document(json!([many_points("first",2050,100.0),many_points("second",2050,200.0)]));
    assert!(combine(&too_many,"union").unwrap_err().to_string().contains("4096"));
    let compound = new_document(json!([many_points("first",257,100.0),rectangle("second",500.0,100.0,100.0,100.0)]));
    assert!(combine(&compound,"union").is_err());
    let expensive = new_document(json!([many_points("first",1024,100.0),many_points("second",1024,100.0)]));
    assert!(combine(&expensive,"union").err().is_some_and(|error| error.to_string().contains("work budget")));
}

#[test]
fn compound_contours_reject_touching_crossing_same_winding_and_excess_commands() {
    for second in [vec![[0.0,0.25],[0.0,0.75],[0.5,0.75],[0.5,0.25]],vec![[0.25,0.25],[0.75,0.25],[0.75,0.75],[0.25,0.75]]] {
        let mut path = contours();
        for (offset,point) in second.into_iter().enumerate() { path.commands[5+offset] = if offset == 0 { PathCommand::Move { point } } else { PathCommand::Line { point } }; }
        assert!(path.validate().is_err());
    }
    let mut path = contours();
    path.commands = (0..26).flat_map(|_| path.commands.clone()).collect();
    assert!(path.validate().is_err());
}

#[test]
fn source_binding_cleanup_is_atomic_and_undo_restores_all_bindings() {
    use base64::{Engine,engine::general_purpose::STANDARD};
    let source = execute_request(json!({"op":"ingest","input":{"name":"synthetic.csv","format":"csv","base64":STANDARD.encode("color,value\n@accent2,1\n")}})).unwrap();
    let deck = new_document(json!([rectangle("first",100.0,100.0,200.0,200.0),rectangle("second",150.0,150.0,100.0,100.0),rectangle("retained",500.0,100.0,100.0,100.0)]))["deck"].clone();
    let bindings: Vec<_> = ["first","second","retained"].into_iter().map(|id| json!({"slide_id":"slide","element_id":id,"field":"/fill","source_id":source["id"],"source_sha256":source["sha256"],"locator":source["tables"][0]["locators"][0][0],"raw_value":"@accent2","value":"@accent2","transform":"display_scalar","stale":false})).collect();
    let document = execute_request(json!({"op":"new_document","id":"bound-geometry","deck":deck,"sources":[source],"bindings":bindings})).unwrap();
    let result = combine(&document,"subtract").unwrap();
    let transaction = &result["transaction"];
    assert_eq!(transaction["document"]["bindings"].as_array().unwrap().len(),1);
    assert_eq!(transaction["document"]["bindings"][0]["element_id"],"retained");
    assert_eq!(transaction["document"]["sources"],document["sources"]);
    let undone = execute_request(json!({"op":"undo_transaction","document":transaction["document"],"expected_revision":transaction["document"]["revision"],"receipt":transaction["receipt"]})).unwrap();
    assert_eq!(undone["document"]["bindings"],document["bindings"]);
    assert_eq!(undone["document"]["hash"],document["hash"]);
}

#[test]
fn preset_rectangle_text_metadata_is_never_discarded() {
    let rectangle = execute_request(json!({"op":"create_object","id":"first","kind":"shape","preset":"rect"})).unwrap();
    let mut rectangle = rectangle;
    rectangle["text"] = json!("");
    rectangle["x"] = json!(100); rectangle["y"] = json!(100); rectangle["width"] = json!(200); rectangle["height"] = json!(200);
    let other = json!({"type":"rect","id":"second","x":150,"y":150,"width":100,"height":100,"fill":"123456"});
    assert!(combine(&new_document(json!([rectangle,other])),"union").is_ok());
    rectangle["format"] = json!({"hyperlink":"https://example.invalid/inert-never-fetched"});
    assert!(combine(&new_document(json!([rectangle,other])),"union").is_err());
}

#[test]
fn simple_polygons_keep_the_existing_vertex_capacity_and_capabilities_are_explicit() {
    let document = new_document(json!([many_points("first",4092,100.0),rectangle("second",175.0,175.0,50.0,50.0)]));
    let result = combine(&document,"union").unwrap();
    let polygon = &result["transaction"]["document"]["deck"]["slides"][0]["elements"][0];
    assert_eq!(polygon["points"].as_array().unwrap().len(),4092);
    assert!(polygon["visual"]["path"].is_null());
    let capability = execute_request(json!({"op":"authoring_capabilities"})).unwrap();
    assert!(capability["operations"].as_array().unwrap().contains(&json!("combine_shapes")));
    assert_eq!(capability["geometry"]["operations"],json!(["union","intersect","subtract","xor","fragment"]));
    assert_eq!(capability["geometry"]["max_path_commands"],256);
    assert_eq!(capability["geometry"]["max_vertices"],4096);
    assert_eq!(capability["geometry"]["max_edge_pairs"],262144);
    assert_eq!(capability["geometry"]["curve_flattening"],true);
    assert_eq!(capability["geometry"]["flatten_tolerance_px"],0.25);
}

#[test]
fn phase2_curves_flatten_and_fragments_partition_native_areas() {
    let curved = execute_request(json!({"op":"edit_vector","element":rectangle("first",100.0,100.0,200.0,200.0),"path":{"commands":[
        {"op":"move","point":[0,0]},{"op":"quadratic","control":[1,0],"point":[1,1]},
        {"op":"line","point":[0,1]},{"op":"close"}]}})).unwrap();
    let document = new_document(json!([curved,rectangle("second",500.0,100.0,100.0,100.0)]));
    let result = combine(&document,"union").unwrap();
    let native = open(&save(&result["transaction"]["document"]));
    assert!((area(&native["deck"]["slides"][0]["elements"][0])-43333.3333).abs()<150.0);
    assert!(result["effects"]["warnings"].to_string().contains("0.25"));
    let document = new_document(json!([rectangle("first",100.0,100.0,200.0,200.0),rectangle("second",200.0,100.0,200.0,200.0)]));
    for document in [document.clone(),open(&save(&document))] {
        let source = save(&document);
        let result = combine(&document,"fragment").unwrap();
        let transaction = &result["transaction"];
        let native = open(&save(&transaction["document"]));
        let elements = native["deck"]["slides"][0]["elements"].as_array().unwrap();
        assert_eq!(elements.len(),3);
        assert_eq!(result["effects"]["added_ids"].as_array().unwrap().len(),3);
        for element in elements { assert!((area(element)-20000.0).abs()<0.1); }
        for first in 0..elements.len() { for second in first+1..elements.len() {
            let mut left = elements[first].clone(); left["id"] = json!("first");
            let mut right = elements[second].clone(); right["id"] = json!("second");
            assert!(combine(&new_document(json!([left,right])),"intersect").is_err());
        } }
        let undone = execute_request(json!({"op":"undo_transaction","document":transaction["document"],"expected_revision":transaction["document"]["revision"],"receipt":transaction["receipt"]})).unwrap();
        assert_eq!(save(&undone["document"]),source);
    }
}

#[test]
fn phase2_cubic_holes_fragments_and_generated_id_conflicts() {
    let mut first = rectangle("first",100.0,100.0,200.0,200.0);
    first = execute_request(json!({"op":"edit_vector","element":first,"path":{"commands":[
        {"op":"move","point":[0,0]},{"op":"cubic","control1":[1,0],"control2":[1,1],"point":[0,1]},{"op":"close"}]}})).unwrap();
    let source = new_document(json!([first,rectangle("second",500.0,100.0,100.0,100.0)]));
    let result = combine(&open(&save(&source)),"union").unwrap();
    assert!((area(&result["transaction"]["document"]["deck"]["slides"][0]["elements"][0])-34000.0).abs()<150.0);
    let mut hole = contours();
    hole.commands.splice(7..9,[PathCommand::Cubic { control1:[0.9,0.75],control2:[0.9,0.25],point:[0.25,0.25] }]);
    let first = execute_request(json!({"op":"edit_vector","element":rectangle("first",100.0,100.0,200.0,200.0),"path":hole})).unwrap();
    let source = new_document(json!([first,rectangle("second",500.0,100.0,100.0,100.0)]));
    let result = combine(&source,"union").unwrap();
    let reopened = open(&save(&result["transaction"]["document"]));
    let element = &reopened["deck"]["slides"][0]["elements"][0];
    assert_eq!(element["visual"]["path"]["commands"].as_array().unwrap().iter().filter(|command| command["op"]=="move").count(),3);
    let document = new_document(json!([rectangle("first",100.0,100.0,200.0,200.0),rectangle("second",200.0,100.0,200.0,200.0),rectangle("third",250.0,100.0,200.0,200.0)]));
    let result = execute_request(json!({"op":"combine_shapes","document":document,"expected_revision":document["revision"],"slide_id":"slide","ids":["first","second","third"],"operation":"fragment","result_id":"pieces"})).unwrap();
    let elements = result["transaction"]["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
    assert_eq!(elements.len(),5);
    assert!((elements.iter().map(area).sum::<f64>()-70000.0).abs()<0.1);
    let collision = new_document(json!([rectangle("first",100.0,100.0,200.0,200.0),rectangle("second",200.0,100.0,200.0,200.0),rectangle("combined-2",600.0,100.0,100.0,100.0)]));
    let before = save(&collision);
    assert!(combine(&collision,"fragment").is_err());
    assert_eq!(save(&collision),before);
}