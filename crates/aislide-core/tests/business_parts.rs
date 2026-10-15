use aislide_core::execute_request;
use serde_json::{json, Value};
use std::collections::BTreeSet;

const PRESETS: [&str; 14] = [
    "list-horizontal/kpi-cards", "horizontal-bar-graph/bullet", "water-fall/variance", "matrix/harvey-balls", "matrix/heatmap", "matrix/raci", "matrix/risk",
    "vertical-bar-graph/pareto", "line-graph/control-chart", "tree/fishbone", "flow/swimlane", "flow/sankey", "flow/journey", "correlation/c4-container",
];
const FRAME: [f64; 4] = [48.0, 120.0, 1184.0, 540.0];

fn example(id: &str) -> Value {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    catalog["presets"].as_array().unwrap().iter().find(|entry| entry["id"] == id).unwrap_or_else(|| panic!("missing {id}"))["example"].clone()
}

fn framed(mut spec: Value) -> Value {
    spec["layout"] = json!({"x":FRAME[0],"y":FRAME[1],"width":FRAME[2],"height":FRAME[3],"show_title":false});
    spec
}

fn create(spec: &Value) -> Result<Value, String> { execute_request(json!({"op":"create_part","id":"business","spec":spec})).map_err(|error| error.to_string()) }

fn children(element: &Value) -> &Vec<Value> { element["children"].as_array().unwrap() }

fn number(value: &Value, field: &str) -> f64 { value[field].as_f64().unwrap() }

fn deck(elements: Vec<Value>) -> Value {
    json!({"version":1,"title":"Business parts","width":1280,"height":720,"slides":[{"id":"slide","title":"Business","background":"FFFFFF","notes":"Synthetic examples","elements":elements}]})
}

fn run_sizes(child: &Value) -> Vec<f64> {
    let mut sizes: Vec<f64> = child["format"]["paragraphs"].as_array().map(|paragraphs| paragraphs.iter().flat_map(|paragraph| paragraph["runs"].as_array().unwrap().iter().filter_map(|run| run["style"]["font_size"].as_f64())).collect()).unwrap_or_default();
    if sizes.is_empty() && child["text"].as_str().is_some_and(|text| !text.is_empty()) { sizes.push(number(child, "font_size")); }
    sizes
}

fn descendants(element: &Value) -> Vec<&Value> {
    let mut found = Vec::new();
    for child in children(element) {
        found.push(child);
        if child["type"] == "group" { found.extend(descendants(child)); }
    }
    found
}

fn with_data(id: &str, edit: impl FnOnce(&mut Value)) -> Result<Value, String> {
    let mut spec = example(id);
    edit(&mut spec["data"]);
    create(&spec)
}

/// Replaces the C4 example's inner boxes with `containers` containers and `databases` databases.
/// The person relates to the first inner box and the last inner box relates to the external system.
fn c4_inner(data: &mut Value, containers: usize, databases: usize) {
    let mut elements = vec![json!({"id":"customer","label":"Customer","kind":"person"}), json!({"id":"payments","label":"Payments","kind":"external"})];
    elements.extend((0..containers).map(|index| json!({"id":format!("c{index}"),"label":format!("Service {index}"),"kind":"container"})));
    elements.extend((0..databases).map(|index| json!({"id":format!("d{index}"),"label":format!("Store {index}"),"kind":"database"})));
    let first = if containers > 0 { "c0".to_string() } else { "d0".to_string() };
    let last = if databases > 0 { format!("d{}", databases - 1) } else { format!("c{}", containers - 1) };
    let mut relations = vec![json!({"from":"customer","to":first,"label":"Uses"}), json!({"from":last,"to":"payments","label":"Pays"})];
    if containers > 0 && databases > 0 { relations.push(json!({"from":"c0","to":"d0","label":"Reads"})); }
    data["elements"] = json!(elements);
    data["relations"] = json!(relations);
}

fn route_points(connector: &Value) -> Vec<[f64; 2]> {
    let [x, y, width, height] = ["x", "y", "width", "height"].map(|field| number(connector, field));
    connector["routing"]["points"].as_array().unwrap().iter().map(|point| [x + point[0].as_f64().unwrap() * width, y + point[1].as_f64().unwrap() * height]).collect()
}

/// Samples every connector segment and fails when it passes through the interior of any other diagram node box.
/// A connector's own endpoints are skipped: ports on a diamond or ellipse outline lie inside its bounding box.
fn assert_routes_clear_nodes(element: &Value, context: &str) {
    let all = descendants(element);
    let boxes: Vec<(&str, [f64; 4])> = all.iter().filter(|child| child["type"] == "shape" && child["id"].as_str().unwrap().contains("-n-"))
        .map(|child| (child["id"].as_str().unwrap(), ["x", "y", "width", "height"].map(|field| number(child, field)))).collect();
    assert!(!boxes.is_empty(), "{context}: no nodes");
    for connector in all.iter().filter(|child| child["type"] == "connector") {
        let own = [connector["start"]["element_id"].as_str().unwrap(), connector["end"]["element_id"].as_str().unwrap()];
        for segment in route_points(connector).windows(2) {
            for step in 1..64 {
                let t = f64::from(step) / 64.0;
                let point = [segment[0][0] + (segment[1][0] - segment[0][0]) * t, segment[0][1] + (segment[1][1] - segment[0][1]) * t];
                let crossed = boxes.iter().find(|(id, bounds)| !own.contains(id) && point[0] > bounds[0] + 1.0 && point[0] < bounds[0] + bounds[2] - 1.0 && point[1] > bounds[1] + 1.0 && point[1] < bounds[1] + bounds[3] - 1.0);
                assert!(crossed.is_none(), "{context}: {} runs through {} at {point:?}", connector["id"], crossed.unwrap().0);
            }
        }
    }
}

/// Custom routes leave and enter steps away from the points used by straight and elbow flows, so no two lines overlap there.
fn assert_custom_ends_stay_apart(element: &Value, context: &str) {
    let all = descendants(element);
    let connectors: Vec<&&Value> = all.iter().filter(|child| child["type"] == "connector").collect();
    let ends = |connector: &Value| { let points = route_points(connector); [points[0], *points.last().unwrap()] };
    for custom in connectors.iter().filter(|connector| connector["routing"]["custom"] == true) {
        for other in connectors.iter().filter(|connector| connector["routing"]["custom"] != true) {
            for a in ends(custom) {
                for b in ends(other) { assert!((a[0] - b[0]).hypot(a[1] - b[1]) > 4.0, "{context}: {} and {} meet at {a:?}", custom["id"], other["id"]); }
            }
        }
    }
}

fn swimlane(lanes: &[&str], steps: Value, flows: Value) -> Value {
    json!({"version":1,"preset":"flow/swimlane","title":"Swimlane","subtitle":"","data":{"kind":"swimlane","lanes":lanes,"steps":steps,"flows":flows}})
}

#[test]
fn business_presets_are_recommended_deterministic_and_preflight_clean() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    let specialized: Vec<&str> = catalog["presets"].as_array().unwrap().iter().filter(|entry| entry["specialized"] == true).map(|entry| entry["id"].as_str().unwrap()).collect();
    assert_eq!(specialized, PRESETS, "exactly the business presets are specialized, so editors keep general category defaults");
    for id in PRESETS {
        let entry = catalog["presets"].as_array().unwrap().iter().find(|entry| entry["id"] == id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(entry["recommended"], true, "{id}");
        assert!(!entry["use_when"].as_str().unwrap().is_empty() && !entry["avoid_when"].as_str().unwrap().is_empty(), "{id}");
        let spec = entry["example"].clone();
        let default = create(&spec).unwrap_or_else(|error| panic!("{id}: {error}"));
        assert_eq!(default, create(&spec).unwrap(), "{id}: deterministic");
        assert_eq!([number(&default, "x"), number(&default, "y"), number(&default, "width"), number(&default, "height")], [64.0, 144.0, 1152.0, 512.0], "{id}");
        let element = create(&framed(spec)).unwrap_or_else(|error| panic!("{id} framed: {error}"));
        assert_eq!([number(&element, "x"), number(&element, "y"), number(&element, "width"), number(&element, "height")], FRAME, "{id}");
        let largest = if id == "list-horizontal/kpi-cards" { 44.0 } else { 22.0 };
        for child in children(&element) {
            let [x, y, width, height] = ["x", "y", "width", "height"].map(|field| number(child, field));
            assert!(x >= -0.5 && y >= -0.5 && x + width <= FRAME[2] + 0.5 && y + height <= FRAME[3] + 0.5, "{id}: child outside frame {child}");
        }
        for child in descendants(&element) {
            for size in run_sizes(child) { assert!((12.0..=largest).contains(&size), "{id}: unexpected font size {size} in {child}"); }
        }
        let scene = deck(vec![element.clone()]);
        let measured = execute_request(json!({"op":"measure_layout","deck":scene})).unwrap();
        assert!(measured["measurements"].as_array().unwrap().iter().all(|entry| entry["overflow"] == false && entry["missing_glyphs"] == 0), "{id}: {measured}");
        for (variant, rendered) in [("default", &default), ("framed", &element)] {
            let document = execute_request(json!({"op":"new_document","id":"business-preflight","deck":deck(vec![rendered.clone()])})).unwrap();
            let preflight = execute_request(json!({"op":"preflight_presentation","document":document,"options":{"page_indices":[0],"min_font_size":12}})).unwrap();
            let blocking: Vec<&Value> = preflight["findings"].as_array().unwrap().iter()
                .filter(|finding| ["CONTAINER_PADDING", "CONTAINER_CORNER_OVERFLOW", "TEXT_OVERLAP", "SMALL_TEXT", "OFF_SLIDE", "TEXT_OVERFLOW", "TEXT_CLIPPED", "CONNECTOR_LABEL_INTERFERENCE"].contains(&finding["code"].as_str().unwrap())).collect();
            assert!(blocking.is_empty(), "{id} {variant}: {blocking:?}");
        }
        let exported = execute_request(json!({"op":"export","deck":scene})).unwrap();
        let opened = execute_request(json!({"op":"open_presentation","id":"business-native","base64":exported["base64"]})).unwrap();
        assert_eq!(children(&opened["document"]["deck"]["slides"][0]["elements"][0]).len(), children(&element).len(), "{id}: reopen keeps the part structure");
    }
}

#[test]
fn business_presets_bind_to_their_data_kinds() {
    let mut wrong = example("matrix/raci");
    wrong["preset"] = json!("matrix/heatmap");
    assert!(create(&wrong).unwrap_err().contains("requires heatmap data"));
    let mut legacy = example("matrix/raci");
    legacy["preset"] = json!("matrix/balanced");
    assert!(create(&legacy).unwrap_err().contains("matching briefing preset"));
    let mut unknown = example("list-horizontal/kpi-cards");
    unknown["data"]["cards"][0]["unexpected"] = json!(true);
    assert!(create(&unknown).is_err());
}

#[test]
fn business_parts_validate_their_domain_rules() {
    let cases: Vec<(&str, Box<dyn FnOnce(&mut Value)>, &str)> = vec![
        ("list-horizontal/kpi-cards", Box::new(|data| data["columns"] = json!(1)), "at most two rows"),
        ("list-horizontal/kpi-cards", Box::new(|data| data["cards"][0]["value"] = json!("12\n4")), "kpi value must be one line"),
        ("horizontal-bar-graph/bullet", Box::new(|data| data["rows"][0]["ranges"] = json!([9, 8, 14])), "strictly ascending"),
        ("horizontal-bar-graph/bullet", Box::new(|data| data["rows"][0]["actual"] = json!(15)), "must not exceed the last range"),
        ("water-fall/variance", Box::new(|data| data["rows"] = json!([{"label":"Only","plan":1,"actual":2}])), "require 2-10 items"),
        ("matrix/harvey-balls", Box::new(|data| data["rows"][0]["levels"] = json!([1, 2, 5, 3])), "0-4 value per column"),
        ("matrix/harvey-balls", Box::new(|data| data["legend"] = json!(["None", "Full"])), "exactly 5 entries"),
        ("matrix/heatmap", Box::new(|data| data["rows"][0]["values"] = json!([1, 2])), "one value per column"),
        ("matrix/raci", Box::new(|data| data["tasks"][0]["assignments"] = json!(["A", "A", "C", "I"])), "needs exactly one A (found 2)"),
        ("matrix/raci", Box::new(|data| data["tasks"][2]["assignments"] = json!(["I", "C", "A", ""])), "needs at least one R"),
        ("matrix/raci", Box::new(|data| data["tasks"][0]["assignments"] = json!(["A/R", "X", "C", "I"])), "one of"),
        ("matrix/risk", Box::new(|data| data["risks"][1]["id"] = json!("R1")), "duplicate risk id R1"),
        ("matrix/risk", Box::new(|data| data["risks"][0]["impact"] = json!(6)), "must be 1-5"),
        ("matrix/risk", Box::new(|data| { for index in 0..5 { data["risks"][index]["likelihood"] = json!(3); data["risks"][index]["impact"] = json!(3); } }), "at most four risks"),
        ("vertical-bar-graph/pareto", Box::new(|data| data["items"][0]["value"] = json!(-1)), "nonnegative"),
        ("vertical-bar-graph/pareto", Box::new(|data| data["threshold"] = json!(1.2)), "between 0 and 1"),
        ("line-graph/control-chart", Box::new(|data| { data["upper"] = json!(10); data["lower"] = json!(12); }), "lower < center < upper"),
        ("line-graph/control-chart", Box::new(|data| data["upper"] = json!(5)), "lower < center < upper after defaults; got lower"),
        ("line-graph/control-chart", Box::new(|data| data["lower"] = json!(13)), "lower < center < upper"),
        ("line-graph/control-chart", Box::new(|data| { let count = data["values"].as_array().unwrap().len(); data["values"] = json!(vec![7.0; count]); }), "do not vary"),
        ("line-graph/control-chart", Box::new(|data| { let count = data["values"].as_array().unwrap().len(); data["values"] = json!(vec![7.0; count]); data["upper"] = json!(9); }), "supply both upper and lower"),
        ("line-graph/control-chart", Box::new(|data| data["labels"] = json!(["W1"])), "labels must match values"),
        ("tree/fishbone", Box::new(|data| data["categories"][0]["causes"] = json!([{"text":"a"},{"text":"b"},{"text":"c"},{"text":"d"}])), "require 1-3 items"),
        ("flow/swimlane", Box::new(|data| data["flows"][4]["exception"] = json!(false)), "form a cycle"),
        ("flow/swimlane", Box::new(|data| data["steps"][1]["lane"] = json!(9)), "must index lanes"),
        ("flow/swimlane", Box::new(|data| data["flows"][0]["to"] = json!("missing")), "two different step ids"),
        ("flow/swimlane", Box::new(|data| { data["steps"][4]["lane"] = json!(1); data["steps"][4]["column"] = json!(2); }), "share lane 1 column 2"),
        ("flow/swimlane", Box::new(|data| data["lanes"] = json!(["A", "B", "C", "D", "E", "F"])), "swimlane lanes require 2-5 items"),
        ("flow/swimlane", Box::new(|data| data["steps"][0]["column"] = json!(7)), "at most seven columns"),
        ("line-graph/control-chart", Box::new(|data| { data["labels"] = json!((1..=33).map(|index| format!("W{index}")).collect::<Vec<_>>()); data["values"] = json!(vec![12.0; 33]); }), "control chart values require 5-32 items"),
        ("flow/sankey", Box::new(|data| data["links"][5]["value"] = json!(20)), "receives 35 but sends 41"),
        ("flow/sankey", Box::new(|data| data["links"][0]["to"] = json!("search")), "different nodes"),
        ("flow/sankey", Box::new(|data| { data["nodes"] = json!([{"id":"a","label":"A"},{"id":"b","label":"B"}]); data["links"] = json!([{"from":"a","to":"b","value":1},{"from":"b","to":"a","value":1}]); }), "cycle"),
        ("flow/journey", Box::new(|data| data["emotions"] = json!([1, 0, -3, -1, 2])), "-2..2"),
        ("flow/journey", Box::new(|data| data["highlight"] = json!(7)), "must index a stage"),
        ("flow/journey", Box::new(|data| data["rows"][0]["cells"] = json!(["a"])), "one cell per stage"),
        ("correlation/c4-container", Box::new(|data| data["relations"][0]["to"] = json!("customer")), "two different element ids"),
        ("correlation/c4-container", Box::new(|data| data["elements"][1]["id"] = json!("customer")), "duplicate architecture element id customer"),
        ("correlation/c4-container", Box::new(|data| c4_inner(data, 4, 4)), "4 containers and 4 databases need 4 rows"),
        ("correlation/c4-container", Box::new(|data| c4_inner(data, 7, 2)), "7 containers and 2 databases need 4 rows"),
        ("correlation/c4-container", Box::new(|data| c4_inner(data, 1, 7)), "1 containers and 7 databases need 4 rows"),
    ];
    for (id, edit, expected) in cases {
        let error = with_data(id, edit).unwrap_err();
        assert!(error.contains(expected), "{id}: expected \"{expected}\" in {error}");
    }
}

#[test]
fn business_parts_reject_frames_that_are_too_small() {
    for id in PRESETS {
        let mut spec = example(id);
        spec["layout"] = json!({"x":40,"y":120,"width":360,"height":150,"show_title":false});
        assert!(create(&spec).is_err(), "{id} accepted a 360x150 frame");
    }
}

fn repeated(data: &mut Value, key: &str, count: usize, edit: impl Fn(&mut Value, usize)) {
    let template = data[key][0].clone();
    data[key] = json!((0..count).map(|index| { let mut item = template.clone(); edit(&mut item, index); item }).collect::<Vec<_>>());
}

/// Every maximum stated in use_when renders in the default and a framed layout, so documented and actual limits agree.
#[test]
fn business_parts_render_at_their_documented_capacity() {
    let cases: Vec<(&str, Box<dyn Fn(&mut Value)>)> = vec![
        ("list-horizontal/kpi-cards", Box::new(|data| repeated(data, "cards", 8, |card, index| card["label"] = json!(format!("Metric {index}"))))),
        ("horizontal-bar-graph/bullet", Box::new(|data| repeated(data, "rows", 6, |row, index| row["label"] = json!(format!("Metric {index}"))))),
        ("water-fall/variance", Box::new(|data| repeated(data, "rows", 10, |row, index| row["label"] = json!(format!("Line item {index}"))))),
        ("matrix/harvey-balls", Box::new(|data| {
            // Eight options fit with the legend; together with a message band the limit is seven.
            data.as_object_mut().unwrap().remove("message");
            data["columns"] = json!(["Speed", "Cost", "Control", "Scale", "Risk", "Support"]);
            repeated(data, "rows", 8, |row, index| { row["label"] = json!(format!("Option {index}")); row["levels"] = json!((0..6).map(|column| (index + column) % 5).collect::<Vec<_>>()); });
        })),
        ("matrix/heatmap", Box::new(|data| {
            data["columns"] = json!(["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]);
            repeated(data, "rows", 10, |row, index| { row["label"] = json!(format!("Region {index}")); row["values"] = json!((0..12).map(|column| 80 + (index * 3 + column) % 19).collect::<Vec<_>>()); });
        })),
        ("matrix/raci", Box::new(|data| {
            // Ten tasks fit without the legend; with it the limit is nine.
            data.as_object_mut().unwrap().remove("legend");
            data["roles"] = json!(["Product", "Engineering", "Security", "Support", "Legal", "Finance", "Sales", "Operations"]);
            repeated(data, "tasks", 10, |task, index| {
                task["label"] = json!(format!("Task {index}"));
                task["assignments"] = json!((0..8).map(|role| if role == index % 8 { "A/R" } else if role % 3 == 0 { "C" } else { "I" }).collect::<Vec<_>>());
            });
        })),
        ("matrix/risk", Box::new(|data| repeated(data, "risks", 10, |risk, index| {
            risk["id"] = json!(format!("R{}", index + 1)); risk["label"] = json!(format!("Risk {index}"));
            risk["likelihood"] = json!(1 + index % 5); risk["impact"] = json!(1 + (index * 2) % 5);
        }))),
        ("vertical-bar-graph/pareto", Box::new(|data| repeated(data, "items", 12, |item, index| { item["label"] = json!(format!("Cause {index}")); item["value"] = json!(50 - 3 * index); }))),
        ("line-graph/control-chart", Box::new(|data| {
            data["labels"] = json!((1..=32).map(|index| format!("W{index}")).collect::<Vec<_>>());
            data["values"] = json!((0..32).map(|index| 12.0 + ((index * 7) % 11) as f64 / 10.0).collect::<Vec<_>>());
        })),
        ("tree/fishbone", Box::new(|data| repeated(data, "categories", 6, |category, index| {
            category["label"] = json!(format!("Category {index}"));
            category["causes"] = json!((0..3).map(|cause| json!({"text":format!("Cause {index}.{cause}")})).collect::<Vec<_>>());
        }))),
        ("flow/swimlane", Box::new(|data| {
            data["lanes"] = json!(["Customer", "Sales", "Finance", "Operations", "Support"]);
            data["steps"] = json!((0..16).map(|index| json!({"id":format!("s{index}"),"label":format!("Step {index}"),"lane":index % 5,"column":index * 7 / 16})).collect::<Vec<_>>());
            let mut flows: Vec<Value> = (0..15).map(|index| json!({"from":format!("s{index}"),"to":format!("s{}", index + 1)})).collect();
            flows.push(json!({"from":"s15","to":"s0","label":"Restart","exception":true}));
            data["flows"] = json!(flows);
        })),
        ("flow/sankey", Box::new(|data| {
            let stages = [4, 4, 4, 2, 2];
            data["nodes"] = json!(stages.iter().enumerate().flat_map(|(stage, count)| (0..*count).map(move |index| json!({"id":format!("n{stage}{index}"),"label":format!("Node {stage}.{index}")}))).collect::<Vec<_>>());
            let mut links: Vec<Value> = (0..4).flat_map(|index| [json!({"from":format!("n0{index}"),"to":format!("n1{index}"),"value":10}), json!({"from":format!("n1{index}"),"to":format!("n2{index}"),"value":10})]).collect();
            links.extend((0..4).map(|index| json!({"from":format!("n2{index}"),"to":format!("n3{}", index / 2),"value":10})));
            links.extend((0..2).map(|index| json!({"from":format!("n3{index}"),"to":format!("n4{index}"),"value":20})));
            data["links"] = json!(links);
        })),
        ("flow/journey", Box::new(|data| {
            data["stages"] = json!(["Discover", "Compare", "Sign up", "Onboard", "Use", "Renew"]);
            data["emotions"] = json!([1, 0, -2, -1, 1, 2]);
            data["emotion_notes"] = json!(["Curious", "Unsure", "Form too long", "Needs help", "Productive", "Clear value"]);
            repeated(data, "rows", 4, |row, index| { row["label"] = json!(format!("Row {index}")); row["cells"] = json!((0..6).map(|stage| format!("Cell {index}.{stage}")).collect::<Vec<_>>()); });
        })),
        ("correlation/c4-container", Box::new(|data| {
            let mut elements: Vec<Value> = (0..3).map(|index| json!({"id":format!("p{index}"),"label":format!("Person {index}"),"kind":"person"})).collect();
            elements.extend((0..3).map(|index| json!({"id":format!("c{index}"),"label":format!("Service {index}"),"kind":"container"})));
            elements.extend((0..3).map(|index| json!({"id":format!("d{index}"),"label":format!("Store {index}"),"kind":"database"})));
            elements.extend((0..3).map(|index| json!({"id":format!("x{index}"),"label":format!("External {index}"),"kind":"external"})));
            let mut relations: Vec<Value> = (0..3).map(|index| json!({"from":format!("p{index}"),"to":"c0"})).collect();
            relations.extend([json!({"from":"c0","to":"c1"}), json!({"from":"c1","to":"c2"})]);
            relations.extend((0..3).map(|index| json!({"from":format!("c{index}"),"to":format!("d{index}")})));
            relations.extend((0..3).map(|index| json!({"from":"c2","to":format!("x{index}")})));
            data["elements"] = json!(elements);
            data["relations"] = json!(relations);
        })),
    ];
    let mut failures = Vec::new();
    for (id, edit) in cases {
        for spec in [example(id), framed(example(id))] {
            let mut spec = spec;
            edit(&mut spec["data"]);
            if let Err(error) = create(&spec) { failures.push(format!("{id} (layout {}): {error}", spec["layout"])); }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn pareto_sorts_causes_and_puts_the_cumulative_share_on_the_secondary_percent_axis() {
    let element = create(&example("vertical-bar-graph/pareto")).unwrap();
    let chart = children(&element).iter().find(|child| child["type"] == "chart").unwrap();
    assert_eq!(chart["kind"], "combo");
    let series = chart["series"].as_array().unwrap();
    let counts: Vec<f64> = series[0]["values"].as_array().unwrap().iter().map(|value| value.as_f64().unwrap()).collect();
    assert!(counts.windows(2).all(|pair| pair[0] >= pair[1]), "sorted descending: {counts:?}");
    assert_eq!(series[1]["axis"], "secondary");
    let cumulative: Vec<f64> = series[1]["values"].as_array().unwrap().iter().map(|value| value.as_f64().unwrap()).collect();
    assert!((cumulative[1] - 0.69).abs() < 1e-9 && (cumulative.last().unwrap() - 1.0).abs() < 1e-9, "{cumulative:?}");
    assert_eq!(series[2]["name"], "80%");
    assert_eq!(chart["options"]["secondary_axis"]["max"], 1.0);
    assert_eq!(chart["options"]["secondary_axis"]["number_format"], "0%");
}

#[test]
fn control_chart_derives_individuals_limits_from_the_average_moving_range() {
    let spec = example("line-graph/control-chart");
    let values: Vec<f64> = spec["data"]["values"].as_array().unwrap().iter().map(|value| value.as_f64().unwrap()).collect();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let moving = values.windows(2).map(|pair| (pair[1] - pair[0]).abs()).sum::<f64>() / (values.len() - 1) as f64;
    let upper = mean + 3.0 * moving / 1.128;
    let element = create(&spec).unwrap();
    let chart = children(&element).iter().find(|child| child["type"] == "chart").unwrap();
    let series = chart["series"].as_array().unwrap();
    assert_eq!(series.len(), 4);
    assert!(series[1]["name"].as_str().unwrap().starts_with("UCL "));
    assert!((series[1]["values"][0].as_f64().unwrap() - upper).abs() < 1e-3, "{} vs {upper}", series[1]["values"][0]);
    assert!((series[2]["values"][0].as_f64().unwrap() - mean).abs() < 1e-3);
    let axis = &chart["options"]["primary_axis"];
    let lowest = values.iter().copied().fold(f64::INFINITY, f64::min);
    assert!(axis["min"].as_f64().unwrap() > 0.0 && axis["min"].as_f64().unwrap() <= lowest.min(mean - (upper - mean)), "the value axis zooms to the data: {axis}");
    assert!(axis["max"].as_f64().unwrap() >= upper, "{axis}");
    let mut pinned = spec.clone();
    pinned["data"]["upper"] = json!(14); pinned["data"]["lower"] = json!(10); pinned["data"]["center"] = json!(12);
    let pinned = create(&pinned).unwrap();
    let series = children(&pinned).iter().find(|child| child["type"] == "chart").unwrap()["series"].clone();
    assert_eq!([series[1]["name"].as_str().unwrap(), series[2]["name"].as_str().unwrap(), series[3]["name"].as_str().unwrap()], ["UCL 14", "CL 12", "LCL 10"]);
}

#[test]
fn variance_colors_follow_whether_the_gap_helps() {
    let fills = |element: &Value| -> Vec<String> {
        children(element).iter().filter(|child| child["preset"] == "rect" && ["@accent1", "@accent2"].contains(&child["fill"].as_str().unwrap())).map(|child| child["fill"].as_str().unwrap().to_string()).collect()
    };
    let spec = example("water-fall/variance");
    assert_eq!(fills(&create(&spec).unwrap()), ["@accent1", "@accent2", "@accent2", "@accent1", "@accent1"], "higher is better");
    let mut inverted = spec;
    inverted["data"]["lower_is_better"] = json!(true);
    assert_eq!(fills(&create(&inverted).unwrap()), ["@accent2", "@accent1", "@accent1", "@accent2", "@accent2"]);
}

#[test]
fn harvey_balls_draw_quarter_wedges_as_native_curved_freeforms() {
    let element = create(&example("matrix/harvey-balls")).unwrap();
    let wedges: Vec<&Value> = children(&element).iter().filter(|child| child["type"] == "polygon").collect();
    // Levels 1-3 draw one wedge each: rows 1,2,3 / 3,2,2 / 3,3,3 plus legend levels 1-3.
    assert_eq!(wedges.len(), 3 + 3 + 3 + 3);
    let commands = wedges[0]["visual"]["path"]["commands"].as_array().unwrap();
    assert_eq!(commands.iter().filter(|command| command["op"] == "cubic").count(), 1, "level 1 is one quarter arc");
    let full = children(&element).iter().filter(|child| child["preset"] == "ellipse" && child["fill"] == "@dk1").count();
    assert_eq!(full, 4, "three level-4 cells plus the legend are filled circles");
}

#[test]
fn risk_markers_sit_in_their_scored_cells_and_the_list_ranks_by_score() {
    let element = create(&framed(example("matrix/risk"))).unwrap();
    let items = children(&element);
    let markers: Vec<&Value> = items.iter().filter(|child| child["preset"] == "ellipse").collect();
    assert_eq!(markers.len(), 10, "five grid markers and five list markers");
    let listed: Vec<&str> = markers[5..].iter().map(|marker| marker["text"].as_str().unwrap()).collect();
    assert_eq!(listed, ["R1", "R2", "R3", "R4", "R5"]);
    let chips: Vec<&str> = items.iter().filter(|child| child["preset"] == "roundRect").map(|chip| chip["text"].as_str().unwrap()).collect();
    assert_eq!(chips, ["4×5 Critical", "3×4 High", "3×3 Medium", "2×3 Medium", "1×5 Medium"]);
    let cells: Vec<&Value> = items.iter().filter(|child| child["preset"] == "rect" && child["text"] == "").collect();
    assert_eq!(cells.len(), 25);
    let r1 = markers.iter().find(|marker| marker["text"] == "R1").unwrap();
    let (cx, cy) = (number(r1, "x") + number(r1, "width") / 2.0, number(r1, "y") + number(r1, "height") / 2.0);
    let cell = cells.iter().find(|cell| cx > number(cell, "x") && cx < number(cell, "x") + number(cell, "width") && cy > number(cell, "y") && cy < number(cell, "y") + number(cell, "height")).unwrap();
    let column = cells.iter().filter(|other| (number(other, "y") - number(cell, "y")).abs() < 0.01 && number(other, "x") < number(cell, "x")).count();
    let row = cells.iter().filter(|other| (number(other, "x") - number(cell, "x")).abs() < 0.01 && number(other, "y") < number(cell, "y")).count();
    assert_eq!((column + 1, 5 - row), (5, 4), "R1 is impact 5, likelihood 4");
}

#[test]
fn sankey_ribbons_follow_the_links_and_keep_node_heights_proportional() {
    let element = create(&example("flow/sankey")).unwrap();
    let ribbons: Vec<&Value> = children(&element).iter().filter(|child| child["type"] == "polygon").collect();
    assert_eq!(ribbons.len(), 7);
    assert!(ribbons.iter().all(|ribbon| ribbon["visual"]["opacity"] == 0.35));
    let bars: Vec<&Value> = children(&element).iter().filter(|child| child["preset"] == "rect").collect();
    assert_eq!(bars.len(), 8);
    let height = |index: usize| number(bars[index], "height");
    assert!((height(3) / height(0) - 100.0 / 48.0).abs() < 1e-6, "visits (100) versus search (48)");
    assert!((height(4) / height(6) - 35.0 / 14.0).abs() < 1e-6, "trials (35) versus paid (14)");
}

#[test]
fn swimlanes_render_through_the_diagram_engine_with_glued_connectors() {
    let element = create(&example("flow/swimlane")).unwrap();
    let all = descendants(&element);
    let connectors: Vec<&&Value> = all.iter().filter(|child| child["type"] == "connector").collect();
    assert_eq!(connectors.len(), 5);
    assert!(connectors.iter().all(|connector| connector["start"].is_object() && connector["end"].is_object()), "connectors stay glued to their steps");
    let returns: Vec<&&&Value> = connectors.iter().filter(|connector| connector["visual"]["dash"].is_string() || connector["dashed"] == true || connector["routing"]["custom"] == true).collect();
    assert!(!returns.is_empty(), "the exception flow is routed as a custom return path");
    assert!(all.iter().any(|child| child["text"] == "Operations"), "lane headers are drawn");
}

#[test]
fn c4_relations_run_between_boxes_instead_of_through_them() {
    for spec in [example("correlation/c4-container"), framed(example("correlation/c4-container"))] {
        let element = create(&spec).unwrap();
        let all = descendants(&element);
        let boxes: Vec<[f64; 4]> = all.iter().filter(|child| child["type"] == "shape" && child["id"].as_str().unwrap().contains("-n-")).map(|child| ["x", "y", "width", "height"].map(|field| number(child, field))).collect();
        assert_eq!(boxes.len(), 6);
        let connectors: Vec<&&Value> = all.iter().filter(|child| child["type"] == "connector").collect();
        assert_eq!(connectors.len(), 5);
        for connector in connectors {
            let [x, y, width, height] = ["x", "y", "width", "height"].map(|field| number(connector, field));
            let (flip_h, flip_v) = (connector["visual"]["flip_h"] == true, connector["flip_v"] == true);
            let start = [if flip_h { x + width } else { x }, if flip_v { y + height } else { y }];
            let end = [if flip_h { x } else { x + width }, if flip_v { y } else { y + height }];
            for step in [0.1, 0.5, 0.9] {
                let point = [start[0] + (end[0] - start[0]) * step, start[1] + (end[1] - start[1]) * step];
                let inside = boxes.iter().any(|bounds| point[0] > bounds[0] + 1.0 && point[0] < bounds[0] + bounds[2] - 1.0 && point[1] > bounds[1] + 1.0 && point[1] < bounds[1] + bounds[3] - 1.0);
                assert!(!inside, "{} runs through a box at {point:?}", connector["id"]);
            }
        }
    }
}

#[test]
fn c4_capacity_follows_the_documented_row_rule() {
    for (containers, databases) in [(9, 0), (6, 3), (3, 6), (0, 9), (4, 2), (1, 1)] {
        let element = with_data("correlation/c4-container", |data| c4_inner(data, containers, databases)).unwrap_or_else(|error| panic!("{containers}+{databases}: {error}"));
        assert_routes_clear_nodes(&element, &format!("c4 {containers} containers and {databases} databases"));
    }
    // A middle box of a full row cannot reach both sides with straight lines, so creation fails instead of drawing through a box.
    let error = with_data("correlation/c4-container", |data| {
        c4_inner(data, 9, 0);
        data["relations"] = json!([{"from":"customer","to":"c0"},{"from":"c0","to":"payments"}]);
    }).unwrap_err();
    assert!(error.contains("architecture relation") && error.contains("would run through"), "{error}");
}

#[test]
fn swimlane_returns_skips_and_blocked_columns_route_around_steps() {
    let cases = [
        ("review repro: rework return under an intermediate lane", swimlane(&["Customer", "Sales"],
            json!([{"id":"a","label":"Request","lane":1},{"id":"b","label":"Review","lane":0},{"id":"c","label":"Approve","lane":0},{"id":"d","label":"Notify","lane":1}]),
            json!([{"from":"a","to":"b"},{"from":"b","to":"c"},{"from":"b","to":"d"},{"from":"c","to":"a","label":"Rework","exception":true}]))),
        ("forward flow past an occupied cell", swimlane(&["Team", "Lead"],
            json!([{"id":"a","label":"Draft","lane":0},{"id":"b","label":"Edit","lane":0},{"id":"c","label":"Publish","lane":0},{"id":"r","label":"Sign off","lane":1,"column":1}]),
            json!([{"from":"a","to":"b"},{"from":"b","to":"c"},{"from":"a","to":"c","label":"Minor"},{"from":"a","to":"r"}]))),
        ("same column past a middle lane", swimlane(&["Desk", "Review", "Archive"],
            json!([{"id":"p","label":"Receive","lane":0,"column":0},{"id":"q","label":"Assess","lane":1,"column":0},{"id":"r","label":"File","lane":2,"column":0},{"id":"s","label":"Close","lane":1,"column":1}]),
            json!([{"from":"p","to":"r"},{"from":"q","to":"s"}]))),
        ("returns and exceptions across three lanes", swimlane(&["Intake", "Work", "Check"],
            json!([{"id":"a","label":"Start","lane":0},{"id":"b","label":"Work","lane":1},{"id":"c","label":"Check","lane":2},{"id":"x","label":"Side task","lane":1,"column":0},{"id":"y","label":"Escalated","lane":0,"column":2}]),
            json!([{"from":"a","to":"b"},{"from":"b","to":"c"},{"from":"c","to":"a","label":"Redo","exception":true},{"from":"c","to":"x","exception":true},{"from":"a","to":"y","label":"Escalate","exception":true}]))),
    ];
    for (name, spec) in cases {
        let element = create(&spec).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_routes_clear_nodes(&element, name);
        assert_custom_ends_stay_apart(&element, name);
        let document = execute_request(json!({"op":"new_document","id":"swimlane-preflight","deck":deck(vec![element])})).unwrap();
        let preflight = execute_request(json!({"op":"preflight_presentation","document":document,"options":{"page_indices":[0],"min_font_size":12}})).unwrap();
        let findings: Vec<&Value> = preflight["findings"].as_array().unwrap().iter()
            .filter(|finding| ["CONNECTOR_LABEL_INTERFERENCE", "TEXT_OVERLAP", "CONTAINER_PADDING", "OFF_SLIDE"].contains(&finding["code"].as_str().unwrap())).collect();
        assert!(findings.is_empty(), "{name}: {findings:?}");
    }
}

#[test]
fn swimlane_routes_never_cross_steps_in_generated_processes() {
    let mut seed = 0x2545_F491_4F6C_DD1D_u64;
    let mut next = |bound: usize| { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % bound as u64) as usize };
    let names = ["Customer", "Sales", "Finance", "Operations", "Support"];
    for case in 0..120 {
        let (lanes, columns) = (2 + next(4), 2 + next(5));
        let count = (2 + next(10)).min(lanes * columns);
        let mut cells: Vec<(usize, usize)> = (0..lanes).flat_map(|lane| (0..columns).map(move |column| (lane, column))).collect();
        for index in (1..cells.len()).rev() { let other = next(index + 1); cells.swap(index, other); }
        let mut chosen = cells[..count].to_vec();
        chosen.sort_by_key(|&(lane, column)| (column, lane));
        let steps: Vec<Value> = chosen.iter().enumerate().map(|(index, &(lane, column))| {
            let shape = ["task", "decision", "event"][next(3)];
            json!({"id":format!("s{index}"),"label":format!("Step {index}"),"lane":lane,"column":column,"shape":shape})
        }).collect();
        let mut pairs = BTreeSet::new();
        let mut flows = Vec::new();
        for _ in 0..count + next(count) {
            let (from, to) = (next(count), next(count));
            if from == to || !pairs.insert((from, to)) { continue; }
            // Forward flows follow the (column, lane) order of the step ids, so only exceptions can point back.
            let exception = to < from || next(5) == 0;
            flows.push(json!({"from":format!("s{from}"),"to":format!("s{to}"),"exception":exception,"label":if next(3) == 0 { "Rework" } else { "" }}));
        }
        if flows.is_empty() { flows.push(json!({"from":"s0","to":"s1"})); }
        let spec = swimlane(&names[..lanes], json!(steps), json!(flows));
        let element = create(&spec).unwrap_or_else(|error| panic!("case {case}: {error}\n{spec}"));
        assert_routes_clear_nodes(&element, &format!("case {case}: {spec}"));
        assert_custom_ends_stay_apart(&element, &format!("case {case}: {spec}"));
    }
}

#[test]
fn journey_maps_mark_the_highlighted_stage_and_plot_emotions() {
    let element = create(&example("flow/journey")).unwrap();
    let items = children(&element);
    let chevrons: Vec<&Value> = items.iter().filter(|child| ["homePlate", "chevron"].contains(&child["preset"].as_str().unwrap_or(""))).collect();
    assert_eq!(chevrons.len(), 5);
    assert_eq!(chevrons[2]["fill"], "@dk1", "highlighted stage");
    let markers: Vec<&Value> = items.iter().filter(|child| child["preset"] == "ellipse").collect();
    assert_eq!(markers.len(), 5);
    let low = markers.iter().map(|marker| number(marker, "y")).fold(f64::NEG_INFINITY, f64::max);
    assert_eq!(number(markers[2], "y"), low, "the -2 stage is the lowest point");
    assert_eq!(items.iter().filter(|child| child["type"] == "connector").count(), 4);
}
