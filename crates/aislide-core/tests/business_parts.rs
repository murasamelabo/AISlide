use aislide_core::execute_request;
use serde_json::{json, Value};

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

#[test]
fn business_presets_are_recommended_deterministic_and_preflight_clean() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
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
        ("line-graph/control-chart", Box::new(|data| { data["upper"] = json!(10); data["lower"] = json!(12); }), "upper limit must exceed"),
        ("line-graph/control-chart", Box::new(|data| data["labels"] = json!(["W1"])), "labels must match values"),
        ("tree/fishbone", Box::new(|data| data["categories"][0]["causes"] = json!([{"text":"a"},{"text":"b"},{"text":"c"},{"text":"d"}])), "require 1-3 items"),
        ("flow/swimlane", Box::new(|data| data["flows"][4]["exception"] = json!(false)), "form a cycle"),
        ("flow/swimlane", Box::new(|data| data["steps"][1]["lane"] = json!(9)), "must index lanes"),
        ("flow/swimlane", Box::new(|data| data["flows"][0]["to"] = json!("missing")), "two different step ids"),
        ("flow/swimlane", Box::new(|data| { data["steps"][4]["lane"] = json!(1); data["steps"][4]["column"] = json!(2); }), "share lane 1 column 2"),
        ("flow/sankey", Box::new(|data| data["links"][5]["value"] = json!(20)), "receives 35 but sends 41"),
        ("flow/sankey", Box::new(|data| data["links"][0]["to"] = json!("search")), "different nodes"),
        ("flow/sankey", Box::new(|data| { data["nodes"] = json!([{"id":"a","label":"A"},{"id":"b","label":"B"}]); data["links"] = json!([{"from":"a","to":"b","value":1},{"from":"b","to":"a","value":1}]); }), "cycle"),
        ("flow/journey", Box::new(|data| data["emotions"] = json!([1, 0, -3, -1, 2])), "-2..2"),
        ("flow/journey", Box::new(|data| data["highlight"] = json!(7)), "must index a stage"),
        ("flow/journey", Box::new(|data| data["rows"][0]["cells"] = json!(["a"])), "one cell per stage"),
        ("correlation/c4-container", Box::new(|data| data["relations"][0]["to"] = json!("customer")), "two different element ids"),
        ("correlation/c4-container", Box::new(|data| data["elements"][1]["id"] = json!("customer")), "duplicate architecture element id customer"),
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
