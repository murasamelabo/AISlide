use aislide_core::execute_request;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

fn asset(alt: &str, decorative: bool, color: &str) -> Value {
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16"><rect width="32" height="16" fill="{color}"/></svg>"#);
    let picture = execute_request(json!({"op":"create_asset","id":"icon","base64":STANDARD.encode(svg),"mime_type":"image/svg+xml","alt":"Synthetic icon","size":48})).unwrap();
    let mut icon = json!({"base64":picture["base64"],"mime_type":picture["mime_type"],"alt":alt});
    if decorative { icon["decorative"] = json!(true); }
    icon
}

/// `a` has no icon; every other node carries its icon. The nodes are chained left to right, the first edge labelled `label`.
fn graph(label: &str, icons: &[(&str, Value)]) -> Value {
    let mut nodes = vec![json!({"id":"a","label":"A","kind":"rounded_rectangle","x":24,"y":160,"width":200,"height":96})];
    let mut edges = Vec::new();
    let mut previous = "a".to_owned();
    for (index, (id, icon)) in icons.iter().enumerate() {
        nodes.push(json!({"id":id,"label":id.to_uppercase(),"kind":"rounded_rectangle","x":300 + 280 * index,"y":160,"width":200,"height":96,"icon":icon}));
        edges.push(json!({"id":format!("e{index}"),"source":previous,"target":id,"label":if index == 0 { label } else { "" },"source_port":"right","target_port":"left","route":"straight"}));
        previous = (*id).to_owned();
    }
    json!({"version":1,"title":"Topology","subtitle":"","show_title":false,"nodes":nodes,"edges":edges,"groups":[]})
}

fn part(graph: Value) -> Value {
    json!({"version":1,"preset":"diagram/custom","title":"Topology","subtitle":"","data":{"kind":"diagram","graph":graph},"layout":{"x":64,"y":104,"width":1152,"height":512,"show_title":false}})
}

fn new_document() -> Value { execute_request(json!({"op":"create_presentation","id":"issue80","title":"Synthetic"})).unwrap() }

fn apply(document: &Value, operations: Value) -> Value {
    execute_request(json!({"op":"apply_operations","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":operations})).unwrap()["document"].clone()
}

fn apply_error(document: &Value, operations: Value) -> String {
    execute_request(json!({"op":"apply_operations","document":document,"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":operations})).unwrap_err().to_string()
}

fn collect<'a>(value: &'a Value, kind: &str, output: &mut Vec<&'a Value>) {
    match value {
        Value::Array(values) => for value in values { collect(value, kind, output); },
        Value::Object(object) => {
            if object.get("type").is_some_and(|found| found == kind) { output.push(value); }
            for child in object.values() { collect(child, kind, output); }
        }
        _ => {}
    }
}

/// The picture a diagram draws for the icon of `node`.
fn icon_of(document: &Value, node: &str) -> Value {
    let mut pictures = Vec::new();
    collect(&document["deck"]["slides"][0]["elements"], "picture", &mut pictures);
    pictures.into_iter().find(|picture| picture["id"].as_str().unwrap().ends_with(&format!("-ni-{node}"))).unwrap_or_else(|| panic!("no icon for {node}")).clone()
}

fn id_of(picture: &Value) -> String { picture["id"].as_str().unwrap().to_owned() }

fn review(document: &Value) -> &Value { &document["deck"]["slides"][0]["review"]["accessibility"] }

fn findings(document: &Value, code: &str) -> Vec<Value> {
    let report = execute_request(json!({"op":"check_accessibility","document":document})).unwrap();
    report["issues"].as_array().unwrap().iter().filter(|issue| issue["code"] == code).cloned().collect()
}

fn set(element_id: &str, metadata: Value) -> Value { json!({"op":"set_accessibility","slide_id":"slide-1","element_id":element_id,"metadata":metadata}) }

fn update(id: &str, spec: Value) -> Value { json!({"op":"update_part","slide_id":"slide-1","id":id,"spec":spec}) }

fn add(id: &str, spec: Value) -> Value { json!({"op":"add_part","slide_id":"slide-1","id":id,"spec":spec}) }

#[test]
fn issue80_update_part_keeps_the_settings_of_the_children_it_draws_again() {
    let definition = |label: &str| part(graph(label, &[("b", asset("", false, "#008080"))]));
    let document = apply(&new_document(), json!([add("g", definition("HTTPS"))]));
    assert_eq!((findings(&document, "group_alt_missing").len(), findings(&document, "image_alt_missing").len()), (1, 1));
    let before = id_of(&icon_of(&document, "b"));
    let document = apply(&document, json!([set(&before, json!({"decorative":true})), set("g", json!({"title":"Topology","description":"The client calls the API"}))]));
    assert!(findings(&document, "group_alt_missing").is_empty() && findings(&document, "image_alt_missing").is_empty());

    // Changing one edge label draws every child again under a new ID. The update used to fail with "accessibility target missing".
    let updated = apply(&document, json!([update("g", definition("HTTP"))]));
    let after = id_of(&icon_of(&updated, "b"));
    assert_ne!(after, before, "the children are named after the definition");
    assert_eq!(review(&updated)[&after]["decorative"], true);
    assert!(review(&updated).get(&before).is_none(), "the setting for the old ID is gone: {}", review(&updated));
    assert_eq!(review(&updated)["g"], json!({"title":"Topology","description":"The client calls the API"}), "the root keeps its settings");
    assert!(findings(&updated, "group_alt_missing").is_empty() && findings(&updated, "image_alt_missing").is_empty());

    // update_graph is the same step.
    let through_graph = apply(&updated, json!([{"op":"update_graph","slide_id":"slide-1","id":"g","spec":graph("HTTP/2", &[("b", asset("", false, "#008080"))])}]));
    let third = id_of(&icon_of(&through_graph, "b"));
    assert_ne!(third, after);
    assert_eq!(review(&through_graph)[&third]["decorative"], true);
    assert_eq!(review(&through_graph).as_object().unwrap().len(), 2);
}

#[test]
fn issue80_a_node_that_goes_takes_its_settings_with_it_and_nothing_moves_to_another_node() {
    let definition = |icons: &[(&str, Value)]| part(graph("calls", icons));
    let document = apply(&new_document(), json!([add("g", definition(&[("b", asset("", false, "#008080")), ("c", asset("", false, "#800080"))]))]));
    let (b, c) = (id_of(&icon_of(&document, "b")), id_of(&icon_of(&document, "c")));
    let document = apply(&document, json!([set(&b, json!({"decorative":true})), set(&c, json!({"title":"Database"}))]));

    // b stays, c is removed and d is new: only b's setting survives, and d does not inherit c's.
    let removed = apply(&document, json!([update("g", definition(&[("b", asset("", false, "#008080")), ("d", asset("", false, "#800080"))]))]));
    let (new_b, new_d) = (id_of(&icon_of(&removed, "b")), id_of(&icon_of(&removed, "d")));
    assert_eq!(review(&removed).as_object().unwrap().keys().cloned().collect::<Vec<_>>(), [new_b.clone()], "{}", review(&removed));
    assert_eq!(review(&removed)[&new_b]["decorative"], true);
    assert!(review(&removed).get(&new_d).is_none());
    let missing = findings(&removed, "image_alt_missing");
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert_eq!(missing[0]["element_id"], json!(new_d));

    // A renamed node is a different node: its settings are not guessed across.
    let renamed = apply(&document, json!([update("g", definition(&[("b2", asset("", false, "#008080")), ("c", asset("", false, "#800080"))]))]));
    assert!(review(&renamed).get(id_of(&icon_of(&renamed, "b2"))).is_none());
    assert_eq!(review(&renamed)[id_of(&icon_of(&renamed, "c"))]["title"], "Database");
}

#[test]
fn issue80_other_parts_and_loose_elements_keep_their_settings_and_never_mix() {
    let definition = |label: &str| part(graph(label, &[("b", asset("", false, "#008080"))]));
    let loose = json!({"op":"add_elements","slide_id":"slide-1","elements":[{"type":"text","id":"note","x":40,"y":20,"width":400,"height":40,"text":"Synthetic note","font_size":20,"color":"000000","bold":false}]});
    let document = apply(&new_document(), json!([add("first", definition("HTTPS")), add("second", definition("HTTPS")), loose]));
    let (first, second) = {
        let mut ids = Vec::new();
        let mut pictures = Vec::new();
        collect(&document["deck"]["slides"][0]["elements"], "picture", &mut pictures);
        for picture in pictures { ids.push(id_of(picture)); }
        assert_eq!(ids.len(), 2);
        (ids[0].clone(), ids[1].clone())
    };
    let document = apply(&document, json!([set(&first, json!({"decorative":true})), set(&second, json!({"title":"Second icon"})), set("note", json!({"title":"Note"}))]));
    // The two parts share their node names; updating the first leaves the second part and the loose text alone.
    let updated = apply(&document, json!([update("first", definition("HTTP"))]));
    assert_eq!(review(&updated)[&second]["title"], "Second icon");
    assert_eq!(review(&updated)["note"]["title"], "Note");
    assert_eq!(review(&updated).as_object().unwrap().len(), 3, "{}", review(&updated));
    let decorative: Vec<_> = review(&updated).as_object().unwrap().iter().filter(|(_, value)| value["decorative"] == true).map(|(key, _)| key.clone()).collect();
    assert_eq!(decorative.len(), 1);
    assert_ne!(decorative[0], first);
    assert!(decorative[0].starts_with("first-") || decorative[0].starts_with("part-"), "the decoration moved to the first part's icon: {decorative:?}");
    assert_ne!(decorative[0], second);
}

#[test]
fn issue80_a_description_set_with_set_accessibility_survives_unless_the_definition_changes_its_alt() {
    let definition = |label: &str, alt: &str| part(graph(label, &[("b", asset(alt, false, "#008080"))]));
    let document = apply(&new_document(), json!([add("g", definition("HTTPS", "Original"))]));
    let id = id_of(&icon_of(&document, "b"));
    let document = apply(&document, json!([set(&id, json!({"description":"Reviewed description"}))]));
    assert_eq!(icon_of(&document, "b")["alt"], "Reviewed description");
    let kept = apply(&document, json!([update("g", definition("HTTP", "Original"))]));
    assert_eq!(icon_of(&kept, "b")["alt"], "Reviewed description", "the reviewed description is carried to the new picture");
    let changed = apply(&kept, json!([update("g", definition("HTTP", "A new alt in the definition"))]));
    assert_eq!(icon_of(&changed, "b")["alt"], "A new alt in the definition", "an explicit change of the definition wins");
}

#[test]
fn issue80_the_definition_can_declare_an_icon_decorative_and_that_is_applied_on_every_draw() {
    let definition = |label: &str, icon: Value| part(graph(label, &[("b", icon)]));
    let document = apply(&new_document(), json!([add("g", definition("HTTPS", asset("", true, "#008080")))]));
    let id = id_of(&icon_of(&document, "b"));
    assert_eq!(review(&document)[&id], json!({"decorative":true}));
    assert!(findings(&document, "image_alt_missing").is_empty(), "a declared decoration is not a missing description");
    assert_eq!(icon_of(&document, "b")["alt"], "");

    // Nothing has to be set again after an update, and a setting cleared by hand comes back because the definition still declares it.
    let updated = apply(&document, json!([update("g", definition("HTTP", asset("", true, "#008080")))]));
    let new_id = id_of(&icon_of(&updated, "b"));
    assert_eq!(review(&updated)[&new_id], json!({"decorative":true}));
    let cleared = apply(&updated, json!([set(&new_id, Value::Null)]));
    assert_eq!(findings(&cleared, "image_alt_missing").len(), 1, "an empty alt without a declaration is still reported");
    let again = apply(&cleared, json!([update("g", definition("HTTP", asset("", true, "#008080")))]));
    assert_eq!(review(&again)[id_of(&icon_of(&again, "b"))], json!({"decorative":true}));

    // Taking the declaration out of the definition takes the decoration back.
    let withdrawn = apply(&again, json!([update("g", definition("HTTP", asset("", false, "#008080")))]));
    assert!(review(&withdrawn).as_object().is_none_or(|entries| entries.is_empty()), "{}", review(&withdrawn));
    assert_eq!(findings(&withdrawn, "image_alt_missing").len(), 1);

    // A decorative picture has no description.
    let mut icon = asset("Client icon", true, "#008080");
    let error = apply_error(&new_document(), json!([add("bad", definition("HTTPS", icon.clone()))]));
    assert!(error.contains("decorative") && error.contains("empty alt"), "{error}");
    icon["alt"] = json!("");
    apply(&new_document(), json!([add("fine", definition("HTTPS", icon))]));
}

#[test]
fn issue80_parts_that_do_not_name_their_children_drop_what_is_gone_and_declare_decoration_in_the_definition() {
    let rows = |label: &str, decorative: bool| part_rows(label, decorative);
    let document = apply(&new_document(), json!([add("rows", rows("First", true))]));
    let mut pictures = Vec::new();
    collect(&document["deck"]["slides"][0]["elements"], "picture", &mut pictures);
    assert_eq!(pictures.len(), 2);
    let ids: Vec<_> = pictures.iter().map(|picture| id_of(picture)).collect();
    for id in &ids { assert_eq!(review(&document)[id], json!({"decorative":true}), "{}", review(&document)); }
    assert!(findings(&document, "image_alt_missing").is_empty());

    // A setting made by hand on such a child cannot be matched, but updating the part no longer fails because of it, and the
    // definition's own declaration is drawn again for the new pictures.
    let document = apply(&document, json!([set(&ids[0], json!({"title":"By hand"}))]));
    let updated = apply(&document, json!([update("rows", rows("Changed", true))]));
    let mut pictures = Vec::new();
    collect(&updated["deck"]["slides"][0]["elements"], "picture", &mut pictures);
    let new_ids: Vec<_> = pictures.iter().map(|picture| id_of(picture)).collect();
    assert!(new_ids.iter().all(|id| !ids.contains(id)));
    assert_eq!(review(&updated).as_object().unwrap().len(), 2, "{}", review(&updated));
    for id in &new_ids { assert_eq!(review(&updated)[id], json!({"decorative":true})); }
}

fn part_rows(label: &str, decorative: bool) -> Value {
    let icon = |color: &str| asset("", decorative, color);
    json!({"version":1,"preset":"list/icon-rows","title":"Rows","data":{"kind":"icon_rows","rows":[
        {"label":label,"detail":"First row","icon":icon("#008080")},{"label":"Second","detail":"Second row","icon":icon("#800080")}]},
        "layout":{"x":64,"y":104,"width":1152,"height":420,"show_title":false}})
}

#[test]
fn issue80_an_orphan_setting_is_reported_with_its_element_and_the_way_out() {
    let document = new_document();
    let mut deck = document["deck"].clone();
    deck["slides"][0]["review"] = json!({"accessibility":{"ghost":{"decorative":true}}});
    let error = execute_request(json!({"op":"new_document","id":"orphan","deck":deck})).unwrap_err().to_string();
    assert!(error.contains("ghost") && error.contains("slide-1") && error.contains("set_accessibility") && error.contains("null"), "{error}");
}
