use aislide_core::execute_request;
use serde_json::{json, Value};
use std::collections::BTreeMap;

// `diagram/custom` has no preset example, so `schema_bounds.rs` never reaches the GraphSpec family. This test covers it directly. Every bound that
// `graph_catalog` and `part_catalog` publish for GraphSpec, GraphNode, GraphEdge, GraphGroup and what they embed has to be listed here with the value the
// validator enforces, and every listed bound has to be published. A bound holds at its value and fails just beyond it.

const KEYWORDS: [&str; 6] = ["minimum", "maximum", "minLength", "maxLength", "minItems", "maxItems"];
const FAMILY: [&str; 7] = ["GraphSpec", "GraphNode", "GraphEdge", "GraphGroup", "GraphIcon", "GraphBadge", "GraphLabelPlacement"];

/// Writes the probed value into a valid graph: a number, a text for the length keywords or a count for the item keywords.
type Set = fn(&mut Value, Value);

struct Row {
    site: &'static str,
    bounds: &'static [(&'static str, f64)],
    set: Set,
    /// A bound that another rule masks, so that no graph at the bound is accepted: (keyword, reason). A stale entry fails the test.
    masked: &'static [(&'static str, &'static str)],
}

fn row(site: &'static str, bounds: &'static [(&'static str, f64)], set: Set) -> Row { Row { site, bounds, set, masked: &[] } }

fn node(spec: &mut Value) -> &mut Value { &mut spec["nodes"][0] }
fn edge(spec: &mut Value) -> &mut Value { &mut spec["edges"][0] }
fn group(spec: &mut Value) -> &mut Value { &mut spec["groups"][0] }

/// Truncates or pads a list to `count` items; a padded item clones the first one under a fresh ID.
fn resize(list: &mut Value, count: Value, stem: &str) {
    let count = count.as_u64().unwrap() as usize;
    let items = list.as_array_mut().unwrap();
    items.truncate(count);
    while items.len() < count {
        let mut item = items[0].clone();
        item["id"] = json!(format!("{stem}{}", items.len()));
        items.push(item);
    }
}

/// Manual route points that never collapse into one another: `count` of them, each with `coordinates` numbers.
fn points(count: usize, coordinates: usize) -> Value {
    json!((0..count).map(|index| (0..coordinates).map(|axis| if axis == 0 { 100 + 20 * index } else { 300 + 20 * (index % 2) + axis }).collect::<Vec<_>>()).collect::<Vec<_>>())
}

fn labelled(spec: &mut Value, placement: Value) {
    edge(spec)["label"] = json!("L");
    edge(spec)["label_placement"] = placement;
}

fn rows() -> Vec<Row> {
    vec![
        row("GraphSpec.version", &[("minimum", 1.0), ("maximum", 1.0)], |spec, value| spec["version"] = value),
        row("GraphSpec.title", &[("maxLength", 80.0)], |spec, value| spec["title"] = value),
        row("GraphSpec.subtitle", &[("maxLength", 120.0)], |spec, value| spec["subtitle"] = value),
        row("GraphSpec.nodes", &[("minItems", 1.0), ("maxItems", 48.0)], |spec, value| { spec["edges"] = json!([]); resize(&mut spec["nodes"], value, "n"); }),
        row("GraphSpec.edges", &[("maxItems", 64.0)], |spec, value| resize(&mut spec["edges"], value, "e")),
        row("GraphSpec.groups", &[("maxItems", 16.0)], |spec, value| resize(&mut spec["groups"], value, "g")),

        row("GraphNode.id", &[("minLength", 1.0), ("maxLength", 24.0)], |spec, value| { spec["edges"] = json!([]); node(spec)["id"] = value; }),
        row("GraphNode.label", &[("maxLength", 160.0)], |spec, value| node(spec)["label"] = value),
        row("GraphNode.detail", &[("maxLength", 240.0)], |spec, value| node(spec)["detail"] = value),
        row("GraphNode.detail_font_size", &[("minimum", 12.0), ("maximum", 40.0)], |spec, value| {
            node(spec)["detail"] = json!("d");
            node(spec)["font_size"] = json!(40);
            node(spec)["detail_font_size"] = value;
        }),
        // A node has to fit the canvas, so the left and top edges stop short of it by the smallest node.
        row("GraphNode.x", &[("minimum", 0.0), ("maximum", 1088.0)], |spec, value| { node(spec)["width"] = json!(64); node(spec)["x"] = value; }),
        row("GraphNode.y", &[("minimum", 0.0), ("maximum", 472.0)], |spec, value| { node(spec)["height"] = json!(40); node(spec)["y"] = value; }),
        row("GraphNode.width", &[("minimum", 64.0), ("maximum", 1152.0)], |spec, value| { node(spec)["x"] = json!(0); node(spec)["width"] = value; }),
        row("GraphNode.height", &[("minimum", 40.0), ("maximum", 512.0)], |spec, value| { node(spec)["y"] = json!(0); node(spec)["height"] = value; }),
        row("GraphNode.font_size", &[("minimum", 12.0), ("maximum", 40.0)], |spec, value| node(spec)["font_size"] = value),

        row("GraphEdge.id", &[("minLength", 1.0), ("maxLength", 24.0)], |spec, value| edge(spec)["id"] = value),
        row("GraphEdge.label", &[("maxLength", 64.0)], |spec, value| edge(spec)["label"] = value),
        row("GraphEdge.source_offset", &[("minimum", -0.5), ("maximum", 0.5)], |spec, value| edge(spec)["source_offset"] = value),
        row("GraphEdge.target_offset", &[("minimum", -0.5), ("maximum", 0.5)], |spec, value| edge(spec)["target_offset"] = value),
        row("GraphEdge.stroke_width", &[("minimum", 0.5), ("maximum", 12.0)], |spec, value| edge(spec)["stroke_width"] = value),
        row("GraphEdge.label_font_size", &[("minimum", 8.0), ("maximum", 40.0)], |spec, value| edge(spec)["label_font_size"] = value),
        row("GraphEdge.waypoints", &[("maxItems", 16.0)], |spec, value| {
            edge(spec)["route"] = json!("manual");
            edge(spec)["waypoints"] = points(value.as_u64().unwrap() as usize, 2);
        }),
        row("GraphEdge.waypoints[]", &[("minItems", 2.0), ("maxItems", 2.0)], |spec, value| {
            edge(spec)["route"] = json!("manual");
            edge(spec)["waypoints"] = points(1, value.as_u64().unwrap() as usize);
        }),
        // Every route point lies on the canvas; the title band only narrows it further while `show_title` is true.
        row("GraphEdge.waypoints[][0]", &[("minimum", 0.0), ("maximum", 1152.0)], |spec, value| {
            edge(spec)["route"] = json!("manual");
            edge(spec)["waypoints"] = json!([[value, 300]]);
        }),
        row("GraphEdge.waypoints[][1]", &[("minimum", 0.0), ("maximum", 512.0)], |spec, value| {
            edge(spec)["route"] = json!("manual");
            edge(spec)["waypoints"] = json!([[300, value]]);
        }),

        row("GraphLabelPlacement.position", &[("minimum", 0.0), ("maximum", 1.0)], |spec, value| labelled(spec, json!({"position": value}))),
        row("GraphLabelPlacement.offset", &[("minimum", 0.0), ("maximum", 128.0)], |spec, value| labelled(spec, json!({"position": 0.5, "offset": value}))),
        row("GraphBadge.number", &[("minimum", 1.0), ("maximum", 99.0)], |spec, value| edge(spec)["badge"] = json!({"number": value})),
        row("GraphBadge.position", &[("minimum", 0.0), ("maximum", 1.0)], |spec, value| edge(spec)["badge"] = json!({"number": 1, "position": value})),
        // The number has to fit the badge, so the smallest badge takes the smallest font.
        row("GraphBadge.size", &[("minimum", 16.0), ("maximum", 64.0)], |spec, value| edge(spec)["badge"] = json!({"number": 1, "size": value, "font_size": 8})),
        row("GraphBadge.font_size", &[("minimum", 8.0), ("maximum", 32.0)], |spec, value| edge(spec)["badge"] = json!({"number": 1, "size": 64, "font_size": value})),

        row("GraphGroup.id", &[("minLength", 1.0), ("maxLength", 24.0)], |spec, value| group(spec)["id"] = value),
        row("GraphGroup.label", &[("maxLength", 64.0)], |spec, value| group(spec)["label"] = value),
        row("GraphGroup.x", &[("minimum", 0.0), ("maximum", 1088.0)], |spec, value| { group(spec)["width"] = json!(64); group(spec)["x"] = value; }),
        row("GraphGroup.y", &[("minimum", 0.0), ("maximum", 472.0)], |spec, value| { group(spec)["height"] = json!(40); group(spec)["y"] = value; }),
        row("GraphGroup.width", &[("minimum", 64.0), ("maximum", 1152.0)], |spec, value| { group(spec)["x"] = json!(0); group(spec)["width"] = value; }),
        row("GraphGroup.height", &[("minimum", 40.0), ("maximum", 512.0)], |spec, value| { group(spec)["y"] = json!(0); group(spec)["height"] = value; }),
        row("GraphGroup.padding", &[("minimum", 0.0), ("maximum", 64.0)], |spec, value| group(spec)["padding"] = value),
        Row {
            masked: &[("minimum", "the label has to fit the header: header_font_size * 1.25 + 12 <= header_height, so with the smallest font of 8 the smallest usable height is 22")],
            ..row("GraphGroup.header_height", &[("minimum", 20.0), ("maximum", 128.0)], |spec, value| { group(spec)["header_font_size"] = json!(8); group(spec)["header_height"] = value; })
        },
        row("GraphGroup.header_font_size", &[("minimum", 8.0), ("maximum", 32.0)], |spec, value| { group(spec)["header_height"] = json!(128); group(spec)["header_font_size"] = value; }),
    ]
}

/// Bounds that the validators check but that need an image to probe, with the test that covers them.
const ELSEWHERE: &[(&str, &[(&str, f64)], &str)] = &[
    ("GraphIcon.base64", &[("minLength", 1.0), ("maxLength", 1398104.0)], "the decoder limits image bytes (1 MiB); issue61_graph_icon_schema_tracks_the_struct_and_core_rejects_asset_handles checks it"),
    ("GraphIcon.alt", &[("maxLength", 500.0)], "valid_text limits the alt text; briefing_parts.rs checks it through every catalog schema"),
];

fn base() -> Value {
    json!({
        "version": 1, "title": "Bounds", "show_title": false,
        "nodes": [{"id": "a", "label": "A", "x": 100, "y": 100}, {"id": "b", "label": "B", "x": 500, "y": 100}],
        "edges": [{"id": "ab", "source": "a", "target": "b"}],
        "groups": [{"id": "g", "label": "G", "x": 20, "y": 20, "width": 300, "height": 200}]
    })
}

/// `transform_graph` validates the graph before and after its operations and renders nothing; moving an entity by zero leaves it unchanged.
fn validate(spec: &Value) -> Result<(), String> {
    let id = spec["groups"][0]["id"].as_str().or_else(|| spec["nodes"][0]["id"].as_str()).unwrap_or("g");
    execute_request(json!({"op": "transform_graph", "spec": spec, "operations": [{"op": "move", "ids": [id], "dx": 0, "dy": 0}]})).map(|_| ()).map_err(|error| error.to_string())
}

type Published = BTreeMap<String, (BTreeMap<String, f64>, bool)>;

/// Every bound a definition of the family publishes, by site: `GraphNode.x`, `GraphEdge.waypoints[]`, `GraphEdge.waypoints[][0]`. The flag marks integers.
fn visit(site: &str, schema: &Value, found: &mut Published) {
    let bounds: BTreeMap<String, f64> = KEYWORDS.iter().filter_map(|keyword| schema.get(*keyword).and_then(Value::as_f64).map(|value| (keyword.to_string(), value))).collect();
    let integer = schema["type"] == "integer" || schema["type"].as_array().is_some_and(|kinds| kinds.iter().any(|kind| kind == "integer"));
    if !bounds.is_empty() { found.insert(site.to_owned(), (bounds, integer)); }
    if schema["items"].is_object() { visit(&format!("{site}[]"), &schema["items"], found); }
    for (index, item) in schema["prefixItems"].as_array().into_iter().flatten().enumerate() { visit(&format!("{site}[{index}]"), item, found); }
    for key in ["anyOf", "oneOf", "allOf"] {
        for branch in schema[key].as_array().into_iter().flatten() { visit(site, branch, found); }
    }
}

/// The family members a schema carries: a definition, or the root itself when the schema describes that type (`graph_catalog` describes GraphSpec).
fn members(schema: &Value) -> Vec<(&'static str, &Value)> {
    let defs = schema.get("$defs").or_else(|| schema.get("definitions")).unwrap();
    FAMILY.iter().filter_map(|name| defs.get(*name).or_else(|| (schema["title"] == *name).then_some(schema)).map(|definition| (*name, definition))).collect()
}

fn published(members: &[(&'static str, &Value)]) -> Published {
    let mut found = Published::new();
    for (name, definition) in members {
        for (property, value) in definition["properties"].as_object().into_iter().flatten() { visit(&format!("{name}.{property}"), value, &mut found); }
    }
    found
}

fn describe(bounds: &BTreeMap<String, f64>) -> String { bounds.iter().map(|(keyword, value)| format!("{keyword} {value}")).collect::<Vec<_>>().join(", ") }

/// The nearest value outside a bound that a floating-point sum cannot round back inside.
fn beyond(keyword: &str, bound: f64, integer: bool) -> f64 {
    let step = if integer { 1.0 } else { 1e-9 * bound.abs().max(1.0) };
    if keyword == "minimum" { bound - step } else { bound + step }
}

fn number(value: f64, integer: bool) -> Value { if integer { json!(value as i64) } else { json!(value) } }

/// The probe value at `bound`, or one past it, in the form the row expects for the keyword.
fn probe(keyword: &str, bound: f64, past: bool, integer: bool) -> Value {
    let size = |lower: bool| if !past { bound } else if lower { bound - 1.0 } else { bound + 1.0 };
    match keyword {
        "minLength" | "maxLength" => json!("x".repeat(size(keyword == "minLength") as usize)),
        "minItems" | "maxItems" => json!(size(keyword == "minItems") as u64),
        _ => number(if past { beyond(keyword, bound, integer) } else { bound }, integer),
    }
}

#[test]
fn published_graph_bounds_match_what_the_validator_enforces() {
    let graph_catalog = execute_request(json!({"op": "graph_catalog"})).unwrap();
    let part_catalog = execute_request(json!({"op": "part_catalog"})).unwrap();
    let rows = rows();

    // 1. The three schemas that carry the family publish the same definitions, and exactly the bounds listed here.
    let schemas = [("graph_catalog schema", &graph_catalog["schema"]), ("graph_catalog operation_schema", &graph_catalog["operation_schema"]), ("part_catalog schema", &part_catalog["schema"])];
    let mut expected: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
    for entry in &rows { expected.insert(entry.site.to_owned(), entry.bounds.iter().map(|(keyword, value)| (keyword.to_string(), *value)).collect()); }
    for (site, bounds, _) in ELSEWHERE { expected.insert(site.to_string(), bounds.iter().map(|(keyword, value)| (keyword.to_string(), *value)).collect()); }
    let mut report = Vec::new();
    for (name, schema) in schemas {
        let carried = members(schema);
        let found = published(&carried);
        for (site, bounds) in &expected {
            if !carried.iter().any(|(family, _)| site.starts_with(&format!("{family}."))) { continue; }
            match found.get(site) {
                None => report.push(format!("{name}: {site} publishes no bounds, expected {}", describe(bounds))),
                Some((published, _)) if published != bounds => report.push(format!("{name}: {site} publishes {}, expected {}", describe(published), describe(bounds))),
                Some(_) => {}
            }
        }
        for (site, (bounds, _)) in &found {
            if !expected.contains_key(site) { report.push(format!("{name}: {site} publishes {} but the test does not cover it", describe(bounds))); }
        }
    }
    assert!(report.is_empty(), "{} published bounds differ from the validator:\n{}", report.len(), report.join("\n"));

    // 2. The validator accepts a graph at every bound and rejects one just beyond it.
    let found = published(&members(&part_catalog["schema"]));
    assert_eq!(members(&part_catalog["schema"]).len(), FAMILY.len(), "part_catalog carries the whole family");
    assert!(validate(&base()).is_ok(), "the base graph is valid: {:?}", validate(&base()));
    let mut report = Vec::new();
    for entry in &rows {
        let integer = found[entry.site].1;
        for (keyword, bound) in entry.bounds {
            let mut at = base();
            (entry.set)(&mut at, probe(keyword, *bound, false, integer));
            let mut past = base();
            (entry.set)(&mut past, probe(keyword, *bound, true, integer));
            match (entry.masked.iter().find(|(masked, _)| masked == keyword), validate(&at)) {
                (None, Err(error)) => report.push(format!("{} {keyword} {bound}: rejected at the published bound: {error}", entry.site)),
                (Some((_, reason)), Ok(())) => report.push(format!("stale mask on {} {keyword} {bound}: accepted at the bound although {reason}", entry.site)),
                _ => {}
            }
            if validate(&past).is_ok() { report.push(format!("{} {keyword} {bound}: accepted just beyond the published bound", entry.site)); }
        }
    }
    assert!(report.is_empty(), "{} bounds are not enforced as published:\n{}", report.len(), report.join("\n"));

    // 3. The catalog's limits and canvas agree with the schema.
    let limits = &graph_catalog["limits"];
    let bound = |site: &str, keyword: &str| found[site].0[keyword];
    assert_eq!(limits["nodes"].as_f64(), Some(bound("GraphSpec.nodes", "maxItems")));
    assert_eq!(limits["edges"].as_f64(), Some(bound("GraphSpec.edges", "maxItems")));
    assert_eq!(limits["groups"].as_f64(), Some(bound("GraphSpec.groups", "maxItems")));
    assert_eq!(limits["waypoints_per_edge"].as_f64(), Some(bound("GraphEdge.waypoints", "maxItems")));
    assert_eq!(graph_catalog["node_text"]["detail_max_length"].as_f64(), Some(bound("GraphNode.detail", "maxLength")));
    assert_eq!(graph_catalog["node_text"]["detail_font_size_min"].as_f64(), Some(bound("GraphNode.detail_font_size", "minimum")));
    assert_eq!(graph_catalog["node_text"]["detail_font_size_max"].as_f64(), Some(bound("GraphNode.detail_font_size", "maximum")));
    let (width, height) = (graph_catalog["canvas"]["width"].as_f64().unwrap(), graph_catalog["canvas"]["height"].as_f64().unwrap());
    for owner in ["GraphNode", "GraphGroup"] {
        assert_eq!(bound(&format!("{owner}.width"), "maximum"), width, "{owner}.width");
        assert_eq!(bound(&format!("{owner}.height"), "maximum"), height, "{owner}.height");
        assert_eq!(bound(&format!("{owner}.x"), "maximum") + bound(&format!("{owner}.width"), "minimum"), width, "{owner}.x leaves room for the smallest {owner}");
        assert_eq!(bound(&format!("{owner}.y"), "maximum") + bound(&format!("{owner}.height"), "minimum"), height, "{owner}.y leaves room for the smallest {owner}");
    }
    assert_eq!(bound("GraphEdge.waypoints[][0]", "maximum"), width);
    assert_eq!(bound("GraphEdge.waypoints[][1]", "maximum"), height);
}
