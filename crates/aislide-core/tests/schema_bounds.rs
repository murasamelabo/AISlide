use aislide_core::execute_request;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

// `part_catalog` publishes the limits that callers build inputs from. Every schema site that the preset examples reach is probed against the
// validators: a published bound holds at the bound and fails just beyond it, and a limit that a validator names but the schema omits is reported.
// Presets share variants and can narrow a limit, so a site is probed through every preset category that reaches it: some category has to accept
// the published bound, none may accept a value beyond it, and some validator has to name it.

const PROBE_LENGTH: u64 = 5000;
const PROBE_COUNT: usize = 60;

/// Differences that JSON Schema cannot express or that another rule masks: (site, probes, reason). A stale entry fails the test.
const ALLOWED: &[(&str, &[&str], &str)] = &[
    // The count is not the rule that rejects the probe.
    ("GraphIcon.base64", &["text-over"], "the decoder limits image bytes (1 MiB) and does not name the base64 length; issue61_graph_icon_schema_tracks_the_struct_and_core_rejects_asset_handles checks it"),
    ("IconColumn.detail_emphasis", &["list-over"], "deprecated and published as maxItems 0; any entry is rejected as Unsupported, which names no count (issue61_part_catalog_schema_matches_icon_column_graph_icon_and_body_size_limits checks it)"),
    ("PartSpec.title", &["text-min-unpublished"], "only the interactive presets require a title; the other presets accept an empty one, so no minLength can be published for the shared spec"),
    // Cloning the last item reaches the count but breaks another rule.
    ("IconRow.detail_emphasis", &["list-max"], "emphasized phrases of one row may not overlap, and the cloned phrases overlap"),
    ("PartData::architecture.elements", &["list-max"], "at most three people and three external systems, so cloning one kind cannot reach 12 elements"),
    ("PartData::architecture.relations", &["list-max"], "a relation may appear once per direction, and the cloned relations repeat"),
    ("PartData::risk_matrix.risks", &["list-max"], "a matrix cell holds at most four risks, and the cloned risks share one cell"),
    ("PartData::sankey.links", &["list-max"], "a link may appear once per pair of nodes, and the cloned links repeat"),
    ("PartData::sankey.nodes", &["list-max"], "every node needs a link, and the cloned nodes have none"),
    ("PartData::swimlane.steps", &["list-max"], "steps of one lane need distinct columns, and the cloned steps share one"),
    // The default frame does not leave room at the maximum count.
    ("PartData::harvey_matrix.rows", &["list-max"], "also limited by the frame: eight rows need a taller frame than the default"),
    ("PartData::icon_cards.cards", &["list-max"], "also limited by the frame: six cards of the example text do not fit at the fixed font size"),
    ("PartData::network.edges", &["list-max"], "also limited by the frame: relationship labels need room"),
    ("PartData::network.nodes", &["list-max"], "also limited by the frame: relationship labels need room"),
    ("PartData::raci.tasks", &["list-max"], "also limited by the frame: ten rows need a taller frame than the default"),
    // A number whose bound another field or the rendering narrows.
    ("PartEdge.to", &["number-at-min"], "the endpoints of an edge differ, so 0 collides with `from` in the example"),
    ("BulletRow.actual", &["number-at-max"], "actual and target must not exceed the last range, which the example keeps"),
    ("BulletRow.target", &["number-at-max"], "actual and target must not exceed the last range, which the example keeps"),
    ("BulletRow.ranges[]", &["number-at-max"], "the limits ascend, so the first one cannot take the maximum"),
    ("PartData::control_chart.values[]", &["number-at-max", "number-at-min"], "the padded axis has to stay within +/-1e15 as well, so values near the limit are rejected"),
    ("PartItem.value", &["number-at-min"], "the pictogram presets narrow it to percentages from 0 to 100"),
    ("SankeyLink.value", &["number-at-max"], "a node has to send what it receives, so one link cannot take the maximum alone"),
    ("WaterfallStep.value", &["number-at-max", "number-at-min"], "running totals stay within +/-1e15 too, and a total step has to reconcile with the steps before it"),
];

enum Kind {
    Text { min: Option<u64>, max: Option<u64> },
    /// `primitive` lists hold strings, numbers or lists of them: the lists that validators tie to the length of another list.
    List { min: Option<u64>, max: Option<u64>, primitive: bool },
    Number { min: Option<f64>, max: Option<f64>, excluded_min: Option<f64>, excluded_max: Option<f64>, integer: bool },
}

#[derive(Clone, Copy)]
enum Side { Min, Max }

/// `seed` marks an optional property that the example omits: only values outside the bounds are probed for numbers, because an in-range one is not known to be valid.
struct Occurrence { preset: usize, pointer: String, seed: bool, filler: Value }

struct Site { key: String, kind: Kind, occurrences: Vec<Occurrence> }

enum Verdict { Accepted, Rejected(String) }

type Flag = (&'static str, String);

fn create(spec: &Value) -> Result<(), String> {
    execute_request(json!({"op":"create_part","id":"bounds","spec":spec})).map(|_| ()).map_err(|error| error.to_string())
}

/// Follows `$ref` (keeping sibling keywords) and reports the name of the last definition entered.
fn resolve(defs: &Value, schema: &Value) -> (Value, Option<String>) {
    let mut schema = schema.clone();
    let mut named = None;
    while let Some(reference) = schema.get("$ref").and_then(Value::as_str).map(str::to_owned) {
        let name = reference.rsplit('/').next().unwrap().to_owned();
        let mut target = defs[&name].clone();
        if let (Some(target), Some(own)) = (target.as_object_mut(), schema.as_object()) {
            for (key, value) in own { if key != "$ref" { target.insert(key.clone(), value.clone()); } }
        }
        named = Some(name);
        schema = target;
    }
    (schema, named)
}

/// The concrete schema of `value`: references resolved and optional or tagged alternatives narrowed to the one that describes it.
fn concrete(defs: &Value, schema: &Value, value: &Value, owner: &str) -> Option<(Value, String)> {
    let mut owner = owner.to_owned();
    let (mut schema, named) = resolve(defs, schema);
    if let Some(name) = named { owner = name; }
    for _ in 0..8 {
        let Some(branches) = schema.get("anyOf").or_else(|| schema.get("oneOf")).and_then(Value::as_array).cloned() else { break };
        if value.is_null() { return None; }
        let candidates: Vec<&Value> = branches.iter().filter(|branch| branch["type"] != "null").collect();
        let chosen = if candidates.len() == 1 { candidates[0].clone() } else {
            let kind = value.get("kind").and_then(Value::as_str)?;
            let branch = candidates.iter().find(|branch| branch["properties"]["kind"]["const"] == kind)?;
            owner = format!("{owner}::{kind}");
            (*branch).clone()
        };
        let (next, named) = resolve(defs, &chosen);
        if let Some(name) = named { owner = name; }
        schema = next;
    }
    Some((schema, owner))
}

fn has_type(schema: &Value, kind: &str) -> bool {
    schema["type"] == kind || schema["type"].as_array().is_some_and(|kinds| kinds.iter().any(|found| found == kind))
}

fn plain_string(schema: &Value) -> bool { has_type(schema, "string") && schema.get("const").is_none() && schema.get("enum").is_none() }

fn number_kind(schema: &Value) -> Option<Kind> {
    let bound = |keyword: &str| schema[keyword].as_f64();
    Some(Kind::Number { min: bound("minimum"), max: bound("maximum"), excluded_min: bound("exclusiveMinimum"), excluded_max: bound("exclusiveMaximum"), integer: has_type(schema, "integer") })
}

/// The value that pads a list the example omits, when its items are plain strings or integers.
fn filler(items: &Value) -> Option<Value> {
    if plain_string(items) { Some(json!("x")) } else if has_type(items, "integer") { Some(json!(0)) } else { None }
}

fn primitive(defs: &Value, schema: &Value) -> bool {
    let (schema, _) = resolve(defs, schema);
    ["string", "number", "integer", "boolean"].iter().any(|kind| has_type(&schema, kind)) || (has_type(&schema, "array") && primitive(defs, &schema["items"]))
}

struct Walker<'a> {
    defs: &'a Value,
    categories: Vec<String>,
    sites: Vec<Site>,
    index: BTreeMap<String, usize>,
    reached: BTreeSet<(usize, String)>,
    /// Every pointer to a list, by preset and site key.
    lists: BTreeMap<(usize, String), Vec<String>>,
}

impl Walker<'_> {
    fn record(&mut self, tag: &str, key: &str, preset: usize, pointer: &str, seed: bool, filler: Value, kind: Kind) {
        let id = format!("{tag}:{key}");
        let position = match self.index.get(&id) {
            Some(position) => *position,
            None => {
                self.sites.push(Site { key: key.into(), kind, occurrences: Vec::new() });
                self.index.insert(id, self.sites.len() - 1);
                self.sites.len() - 1
            }
        };
        if self.reached.insert((position, self.categories[preset].clone())) {
            self.sites[position].occurrences.push(Occurrence { preset, pointer: pointer.into(), seed, filler });
        }
    }

    fn text(&mut self, key: &str, preset: usize, pointer: &str, seed: bool, schema: &Value) {
        self.record("text", key, preset, pointer, seed, Value::Null, Kind::Text { min: schema["minLength"].as_u64(), max: schema["maxLength"].as_u64() });
    }

    fn list(&mut self, key: &str, preset: usize, pointer: &str, seed: bool, filler: Value, schema: &Value) {
        let kind = Kind::List { min: schema["minItems"].as_u64(), max: schema["maxItems"].as_u64(), primitive: primitive(self.defs, &schema["items"]) };
        if !seed { self.lists.entry((preset, key.to_owned())).or_default().push(pointer.to_owned()); }
        self.record("list", key, preset, pointer, seed, filler, kind);
    }

    fn walk(&mut self, schema: &Value, value: &Value, owner: &str, pointer: &str, key: &str, preset: usize) {
        let Some((schema, owner)) = concrete(self.defs, schema, value, owner) else { return };
        match value {
            Value::String(_) if plain_string(&schema) => self.text(key, preset, pointer, false, &schema),
            Value::Array(items) => {
                self.list(key, preset, pointer, false, Value::Null, &schema);
                for (position, item) in items.iter().enumerate() { self.walk(&schema["items"], item, &owner, &format!("{pointer}/{position}"), &format!("{key}[]"), preset); }
            }
            Value::Number(_) => {
                if let Some(kind) = number_kind(&schema) { self.record("number", key, preset, pointer, false, Value::Null, kind); }
            }
            Value::Object(map) => {
                for (name, child) in map {
                    if let Some(property) = schema["properties"].get(name) { self.walk(property, child, &owner, &format!("{pointer}/{name}"), &format!("{owner}.{name}"), preset); }
                }
                // Optional text, numbers and primitive lists that the example leaves out are probed as well.
                for (name, property) in schema["properties"].as_object().into_iter().flatten().filter(|(name, _)| !map.contains_key(*name)) {
                    let Some((property, _)) = concrete(self.defs, property, &json!(0), &owner) else { continue };
                    let (key, pointer) = (format!("{owner}.{name}"), format!("{pointer}/{name}"));
                    if plain_string(&property) {
                        self.text(&key, preset, &pointer, true, &property);
                    } else if has_type(&property, "number") || has_type(&property, "integer") {
                        if let Some(kind) = number_kind(&property) { self.record("number", &key, preset, &pointer, true, Value::Null, kind); }
                    } else if has_type(&property, "array") {
                        if let Some(filler) = filler(&property["items"]) { self.list(&key, preset, &pointer, true, filler, &property); }
                    }
                }
            }
            _ => {}
        }
    }
}

fn set_at(spec: &mut Value, pointer: &str, value: Value) {
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    let parent = spec.pointer_mut(parent).unwrap();
    match parent {
        Value::Object(map) => { map.insert(key.into(), value); }
        Value::Array(list) => list[key.parse::<usize>().unwrap()] = value,
        _ => unreachable!("a site's parent is an object or a list"),
    }
}

fn with_value(example: &Value, pointer: &str, value: Value) -> Value {
    let mut spec = example.clone();
    set_at(&mut spec, pointer, value);
    spec
}

/// Pads or truncates the list at `pointer`. A clone of the last item gets a fresh `id`, since validators reject duplicates.
fn resize(spec: &mut Value, pointer: &str, count: usize) -> Option<()> {
    let list = spec.pointer_mut(pointer)?.as_array_mut()?;
    if count == 0 { list.clear(); return Some(()); }
    list.last()?;
    list.truncate(count);
    let mut taken: BTreeSet<String> = list.iter().filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_owned)).collect();
    while list.len() < count {
        let mut item = list.last()?.clone();
        if let Some(id) = item.get("id").and_then(Value::as_str) {
            let stem = id.trim_end_matches(|character: char| character.is_ascii_digit()).to_owned();
            let fresh = (list.len()..).map(|number| format!("{stem}{number}")).find(|candidate| !taken.contains(candidate)).unwrap();
            taken.insert(fresh.clone());
            item["id"] = json!(fresh);
        }
        list.push(item);
    }
    Some(())
}

/// Validators tie some list lengths together (labels and values, columns and the values of every row). Lists that
/// publish the same bound on the probed side are resized with the probed one, so that the tie does not mask the count rule.
fn with_count(world: &Walker, site: &Site, example: &Value, occurrence: &Occurrence, side: Side, count: usize) -> Option<Value> {
    let Kind::List { min, max, .. } = &site.kind else { unreachable!("only lists are resized") };
    let bound = match side { Side::Min => *min, Side::Max => *max };
    let mut spec = example.clone();
    let mut pointers = Vec::new();
    for ((_, key), found) in world.lists.range((occurrence.preset, String::new())..(occurrence.preset + 1, String::new())) {
        let related = *key == site.key || bound.is_some_and(|bound| matches!(
            &world.sites[world.index[&format!("list:{key}")]].kind,
            Kind::List { min, max, primitive: true } if (match side { Side::Min => *min, Side::Max => *max }) == Some(bound)
        ));
        if related { pointers.extend(found.iter().cloned()); }
    }
    if occurrence.seed {
        if count == 0 { return None; }
        set_at(&mut spec, &occurrence.pointer, Value::Array(vec![occurrence.filler.clone(); count]));
    }
    // Inner lists first: a padded outer item is a clone of an already resized one.
    pointers.sort_by_key(|pointer| std::cmp::Reverse(pointer.matches('/').count()));
    for pointer in &pointers { resize(&mut spec, pointer, count)?; }
    Some(spec)
}

/// The limit in a text-length message, such as "exceeds 48" or "must be 1-24 ASCII letters".
fn length_limit(message: &str) -> Option<u64> {
    if let Some(at) = message.find("exceeds ") {
        return message[at + "exceeds ".len()..].chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok();
    }
    let before = &message[..message.find(" ASCII")?];
    before.chars().rev().take_while(char::is_ascii_digit).collect::<Vec<_>>().into_iter().rev().collect::<String>().parse().ok()
}

const SPELLED: [&str; 13] = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve"];

/// Whether a message names `number` as digits or, from two upward, as a word: "at most four points".
fn mentions(message: &str, number: u64) -> bool {
    message.split(|character: char| !character.is_ascii_digit()).any(|digits| digits.parse::<u64>().is_ok_and(|found| found == number))
        || (number >= 2 && SPELLED.get(number as usize).is_some_and(|word| message.split(|character: char| !character.is_ascii_alphabetic()).any(|found| found.eq_ignore_ascii_case(word))))
}

fn count_error(message: &str) -> bool {
    message.contains(" items") || message.contains(" entries") || message.as_bytes().windows(3).any(|window| window[0].is_ascii_digit() && window[1] == b'-' && window[2].is_ascii_digit())
}

fn run(site: &Site, examples: &[&Value], change: impl Fn(&Value, &Occurrence) -> Option<Value>) -> Vec<Verdict> {
    site.occurrences.iter()
        .filter_map(|occurrence| change(examples[occurrence.preset], occurrence))
        .map(|spec| match create(&spec) { Ok(()) => Verdict::Accepted, Err(error) => Verdict::Rejected(error) })
        .collect()
}

fn first_error(verdicts: &[Verdict]) -> &str {
    verdicts.iter().find_map(|verdict| match verdict { Verdict::Rejected(error) => Some(error.as_str()), Verdict::Accepted => None }).unwrap_or("")
}

/// A probe at a published bound: some preset category has to accept it.
fn at_bound(flags: &mut Vec<Flag>, probe: &'static str, verdicts: &[Verdict], detail: String) {
    if verdicts.is_empty() || verdicts.iter().any(|verdict| matches!(verdict, Verdict::Accepted)) { return; }
    flags.push((probe, format!("{detail}: {}", first_error(verdicts))));
}

/// A probe beyond a published bound: no preset category may accept it, and some validator has to name the bound.
fn beyond_bound(flags: &mut Vec<Flag>, probe: &'static str, verdicts: &[Verdict], names: impl Fn(&str) -> bool, accepted: String, bound: String) {
    if verdicts.iter().any(|verdict| matches!(verdict, Verdict::Accepted)) { flags.push((probe, accepted)); return; }
    if !verdicts.is_empty() && !verdicts.iter().any(|verdict| matches!(verdict, Verdict::Rejected(error) if names(error))) {
        flags.push((probe, format!("{bound} but the validator says: {}", first_error(verdicts))));
    }
}

fn probe(world: &Walker, site: &Site, examples: &[&Value]) -> Vec<Flag> {
    let mut flags = Vec::new();
    match &site.kind {
        Kind::Text { min, max } => {
            let sized = |length: u64| run(site, examples, |example, occurrence| Some(with_value(example, &occurrence.pointer, json!("x".repeat(length as usize)))));
            match *max {
                Some(max) => {
                    let at = sized(max);
                    // A format or a narrower preset limit can reject the filler text; only a verdict that names a length counts against the bound.
                    if !at.is_empty() && at.iter().all(|verdict| matches!(verdict, Verdict::Rejected(error) if length_limit(error).is_some())) {
                        flags.push(("text-max", format!("rejected at the published maxLength {max}: {}", first_error(&at))));
                    }
                    beyond_bound(&mut flags, "text-over", &sized(max + 1), |error| length_limit(error) == Some(max), format!("accepted {} scalars although maxLength is {max}", max + 1), format!("maxLength is {max}"));
                }
                None => {
                    let over = sized(PROBE_LENGTH);
                    if let Some(limit) = over.iter().find_map(|verdict| match verdict { Verdict::Rejected(error) => length_limit(error), Verdict::Accepted => None }) {
                        flags.push(("text-unpublished", format!("the validator limits this to {limit}: {}", first_error(&over))));
                    }
                }
            }
            let empty = sized(0);
            match *min {
                Some(min) if min >= 1 => {
                    if empty.iter().any(|verdict| matches!(verdict, Verdict::Accepted)) { flags.push(("text-min", format!("accepted empty text although minLength is {min}"))); }
                    // minLength counts scalars, so JSON Schema cannot say that a required text must hold more than whitespace; the validators do.
                    let blank = run(site, examples, |example, occurrence| Some(with_value(example, &occurrence.pointer, json!(" \t\u{3000}"))));
                    if blank.iter().any(|verdict| matches!(verdict, Verdict::Accepted)) { flags.push(("text-blank", format!("accepted whitespace-only text although minLength is {min}"))); }
                }
                _ => if let Some(error) = empty.iter().find_map(|verdict| match verdict { Verdict::Rejected(error) if error.contains("required") => Some(error), _ => None }) {
                    flags.push(("text-min-unpublished", format!("empty text is rejected ({error}) but minLength is not published")));
                },
            }
        }
        Kind::List { min, max, .. } => {
            let sized = |side: Side, count: usize| run(site, examples, |example, occurrence| with_count(world, site, example, occurrence, side, count));
            match *max {
                Some(max) => {
                    let max = max as usize;
                    at_bound(&mut flags, "list-max", &sized(Side::Max, max), format!("rejected {max} items although maxItems is {max}"));
                    beyond_bound(&mut flags, "list-over", &sized(Side::Max, max + 1), |error| mentions(error, max as u64), format!("accepted {} items although maxItems is {max}", max + 1), format!("maxItems is {max}"));
                }
                None => {
                    flags.push(("list-no-max", "maxItems is not published".into()));
                    let over = sized(Side::Max, PROBE_COUNT);
                    if let Some(error) = over.iter().find_map(|verdict| match verdict { Verdict::Rejected(error) if count_error(error) => Some(error), _ => None }) {
                        flags.push(("list-unpublished", format!("the validator limits the count: {error}")));
                    }
                }
            }
            match *min {
                Some(min) if min >= 1 => {
                    let min = min as usize;
                    beyond_bound(&mut flags, "list-min", &sized(Side::Min, min - 1), |error| mentions(error, min as u64), format!("accepted {} items although minItems is {min}", min - 1), format!("minItems is {min}"));
                }
                _ => {
                    let none = sized(Side::Min, 0);
                    if let Some(error) = none.iter().find_map(|verdict| match verdict { Verdict::Rejected(error) if count_error(error) => Some(error), _ => None }) {
                        flags.push(("list-min-unpublished", format!("an empty list is rejected ({error}) but minItems is not published")));
                    }
                }
            }
        }
        Kind::Number { min, max, excluded_min, excluded_max, integer } => {
            let number = |value: f64| if *integer { json!(value as i64) } else { json!(value) };
            let at = |value: f64| run(site, examples, |example, occurrence| if occurrence.seed { None } else { Some(with_value(example, &occurrence.pointer, number(value))) });
            let beyond = |value: f64| run(site, examples, |example, occurrence| Some(with_value(example, &occurrence.pointer, number(value))));
            // Part numbers are finite and within +/-1e15; the validators name that limit, so the schema has to publish both sides.
            for (value, published) in [(1e16, max.is_some() || excluded_max.is_some()), (-1e16, min.is_some() || excluded_min.is_some())] {
                if published { continue; }
                let over = beyond(value);
                if let Some(error) = over.iter().find_map(|verdict| match verdict { Verdict::Rejected(error) if error.contains("1e15") => Some(error), _ => None }) {
                    flags.push(("number-unpublished", format!("the validator limits this to +/-1e15: {error}")));
                    break;
                }
            }
            if let Some(max) = *max {
                beyond_bound(&mut flags, "number-over", &beyond(max + 1.0), |_| true, format!("accepted {} although maximum is {max}", max + 1.0), String::new());
                at_bound(&mut flags, "number-at-max", &at(max), format!("rejected the published maximum {max}"));
            }
            if let Some(min) = *min {
                beyond_bound(&mut flags, "number-under", &beyond(min - 1.0), |_| true, format!("accepted {} although minimum is {min}", min - 1.0), String::new());
                at_bound(&mut flags, "number-at-min", &at(min), format!("rejected the published minimum {min}"));
            }
            for (bound, keyword) in [(*excluded_min, "exclusiveMinimum"), (*excluded_max, "exclusiveMaximum")] {
                if let Some(bound) = bound {
                    beyond_bound(&mut flags, "number-excluded", &beyond(bound), |_| true, format!("accepted {bound} although {keyword} is {bound}"), String::new());
                }
            }
        }
    }
    flags
}

#[test]
fn published_bounds_match_what_the_validators_enforce() {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    let presets = catalog["presets"].as_array().unwrap();
    let examples: Vec<&Value> = presets.iter().map(|preset| &preset["example"]).collect();
    let categories = presets.iter().map(|preset| preset["category"].as_str().unwrap().to_owned()).collect();
    let mut walker = Walker { defs: &catalog["schema"]["$defs"], categories, sites: Vec::new(), index: BTreeMap::new(), reached: BTreeSet::new(), lists: BTreeMap::new() };
    for (index, example) in examples.iter().enumerate() { walker.walk(&catalog["schema"], example, "PartSpec", "", "PartSpec", index); }
    assert!(walker.sites.len() >= 250, "the walker reached only {} schema sites", walker.sites.len());

    let mut found: BTreeMap<(String, &'static str), String> = BTreeMap::new();
    for site in &walker.sites {
        for (probe, detail) in probe(&walker, site, &examples) { found.insert((site.key.clone(), probe), detail); }
    }

    let mut report = Vec::new();
    for ((key, probe), detail) in &found {
        if !ALLOWED.iter().any(|(site, probes, _)| site == key && probes.contains(probe)) { report.push(format!("{key} [{probe}] {detail}")); }
    }
    for (site, probes, reason) in ALLOWED {
        assert!(!reason.trim().is_empty(), "{site} needs a reason");
        for probe in *probes {
            if !found.contains_key(&(site.to_string(), *probe)) { report.push(format!("stale allowlist entry {site} [{probe}]")); }
        }
    }
    report.sort();
    assert!(report.is_empty(), "{} schema/validator differences over {} sites:\n{}", report.len(), walker.sites.len(), report.join("\n"));
}
