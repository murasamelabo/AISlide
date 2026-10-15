use aislide_core::{model::{Deck, Element, Slide}, selection::{apply, SelectionOperation}};
use serde_json::{json, Value};

fn rect(id: &str, x: f64, y: f64, width: f64, height: f64) -> Element {
    Element::Rect { visual: None, id: id.into(), x, y, width, height, fill: "@accent1".into() }
}

fn text(id: &str, x: f64, y: f64) -> Element {
    serde_json::from_value(json!({"type":"text", "id":id, "x":x, "y":y, "width":80, "height":40,
        "text":"Synthetic", "font_size":20, "color":"@accent2", "bold":false,
        "format":{"font_family":"@minor"}})).unwrap()
}

fn deck(elements: Vec<Element>) -> Deck {
    Deck { version: 1, title: "Synthetic selection tests".into(), width: 1280, height: 720,
        slides: vec![Slide { id: "slide".into(), title: "Synthetic".into(), background: "FFFFFF".into(), elements,
            notes: String::new(), notes_paragraphs: Vec::new(), layout_id: None, inherit_background: false, hide_master_graphics: false, native_source_id: None, review: None }],
        design: Some(Default::default()), embedded_fonts: Vec::new(), auxiliary_design: None }
}

fn operation(value: Value) -> SelectionOperation { serde_json::from_value(value).unwrap() }

#[test]
fn native_identity_validation_uses_current_canvas_not_default_canvas() {
    let elements = vec![rect("wide", 1500.0, 10.0, 100.0, 40.0)];
    let allocation = std::collections::BTreeMap::from([("wide".into(), 2)]);
    assert!(aislide_core::selection::validate_native_ids(&elements, &allocation).is_err());
    aislide_core::selection::validate_native_ids_on_canvas(&elements, &allocation, 1920, 1080).unwrap();
}

fn run(source: &Deck, value: Value) -> aislide_core::selection::SelectionResult {
    apply(source, "slide", &operation(value), None).unwrap()
}

fn geometry(element: &Element) -> [f64; 4] {
    let (_, x, y, width, height) = element.bounds(); [x, y, width, height]
}

#[test]
fn translate_is_selection_wide_and_atomic() {
    let source = deck(vec![rect("a", 10.0, 10.0, 100.0, 30.0), rect("b", 1100.0, 30.0, 100.0, 30.0)]);
    let before = serde_json::to_value(&source).unwrap();
    let result = run(&source, json!({"op":"translate", "ids":["a","b"], "dx":50, "dy":20}));
    assert_eq!(geometry(&result.deck.slides[0].elements[0]), [60.0, 30.0, 100.0, 30.0]);
    assert_eq!(geometry(&result.deck.slides[0].elements[1]), [1150.0, 50.0, 100.0, 30.0]);
    for dx in [100.0, -20.0] {
        assert!(apply(&source, "slide", &operation(json!({"op":"translate", "ids":["a","b"], "dx":dx, "dy":0})), None).is_err());
    }
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
}

#[test]
fn selection_rejects_empty_duplicate_missing_and_nested_ids() {
    let group: Element = serde_json::from_value(json!({"type":"group","id":"group","x":0,"y":0,"width":200,"height":200,
        "view_width":200,"view_height":200,"children":[rect("nested", 1.0, 1.0, 10.0, 10.0)]})).unwrap();
    let source = deck(vec![group]);
    for ids in [json!([]), json!(["group","group"]), json!(["absent"]), json!(["nested"]), json!(["group","nested"])] {
        assert!(apply(&source, "slide", &operation(json!({"op":"translate","ids":ids,"dx":1,"dy":0})), None).is_err());
    }
    assert!(apply(&source, "absent", &operation(json!({"op":"translate","ids":["group"],"dx":1,"dy":0})), None).is_err());
}

#[test]
fn resize_scales_coordinates_fonts_and_rejects_small_fonts() {
    let source = deck(vec![text("a", 100.0, 100.0), text("b", 220.0, 140.0)]);
    let result = run(&source, json!({"op":"resize","ids":["a","b"],"x":50,"y":60,"width":400,"height":160}));
    assert_eq!(geometry(&result.deck.slides[0].elements[1]), [290.0, 140.0, 160.0, 80.0]);
    assert!(matches!(&result.deck.slides[0].elements[0], Element::Text { font_size, .. } if *font_size == 40.0));
    assert!(apply(&source, "slide", &operation(json!({"op":"resize","ids":["a","b"],"x":0,"y":0,"width":20,"height":8})), None).is_err());
    assert!(matches!(&source.slides[0].elements[0], Element::Text { font_size, .. } if *font_size == 20.0));
}

#[test]
fn alignment_uses_selection_or_page_without_reordering() {
    let source = deck(vec![rect("a", 100.0, 100.0, 100.0, 40.0), rect("b", 300.0, 200.0, 200.0, 80.0)]);
    for (alignment, relative_to, expected) in [("left", "selection", [100.0,200.0,200.0,80.0]),
        ("center", "page", [540.0,200.0,200.0,80.0]), ("bottom", "page", [300.0,640.0,200.0,80.0]),
        ("middle", "selection", [300.0,150.0,200.0,80.0])] {
        let result = run(&source, json!({"op":"align","ids":["b","a"],"alignment":alignment,"relative_to":relative_to}));
        assert_eq!(result.deck.slides[0].elements[0].bounds().0, "a");
        assert_eq!(geometry(&result.deck.slides[0].elements[1]), expected);
    }
}

#[test]
fn distribution_spaces_edges_for_selection_and_page() {
    let source = deck(vec![rect("a", 10.0, 10.0, 100.0, 40.0), rect("b", 140.0, 130.0, 100.0, 40.0), rect("c", 510.0, 410.0, 100.0, 40.0)]);
    let result = run(&source, json!({"op":"distribute","ids":["c","a","b"],"axis":"horizontal","relative_to":"selection"}));
    assert_eq!(result.deck.slides[0].elements[1].bounds().1, 260.0);
    let result = run(&source, json!({"op":"distribute","ids":["a","b","c"],"axis":"vertical","relative_to":"page"}));
    assert_eq!(result.deck.slides[0].elements[0].bounds().2, 0.0);
    assert_eq!(result.deck.slides[0].elements[1].bounds().2, 340.0);
    assert_eq!(result.deck.slides[0].elements[2].bounds().2, 680.0);
    assert!(apply(&source, "slide", &operation(json!({"op":"distribute","ids":["a","b"],"axis":"horizontal","relative_to":"page"})), None).is_err());
}

#[test]
fn grouping_infers_bounds_preserves_z_order_and_reports_affected_ids() {
    let source = deck(vec![rect("a", 10.0, 20.0, 100.0, 40.0), rect("middle", 0.0, 0.0, 10.0, 10.0), rect("b", 150.0, 80.0, 50.0, 40.0)]);
    let result = run(&source, json!({"op":"group","ids":["b","a"],"group_id":"combined"}));
    assert_eq!(result.deck.slides[0].elements.len(), 2);
    assert_eq!(geometry(&result.deck.slides[0].elements[0]), [10.0,20.0,190.0,100.0]);
    match &result.deck.slides[0].elements[0] {
        Element::Group { children, view_width, view_height, .. } => {
            assert_eq!((*view_width, *view_height), (190.0,100.0));
            assert_eq!(children[0].bounds().0, "a"); assert_eq!(geometry(&children[1]), [140.0,60.0,50.0,40.0]);
        }
        _ => panic!("expected group"),
    }
    assert!(result.effects.affected_ids.contains(&"a".into()));
    assert!(result.effects.metadata_review_required);
    assert!(!result.effects.raw_native_preserved);
}

#[test]
fn ungroup_flattens_nested_view_scales_without_id_changes() {
    let inner = json!({"type":"group","id":"inner","x":10,"y":20,"width":100,"height":60,
        "view_width":50,"view_height":30,"children":[text("leaf", 1.0, 2.0)]});
    let mut inner: Element = serde_json::from_value(inner).unwrap();
    if let Element::Group { children, .. } = &mut inner {
        if let Element::Text { width, height, .. } = &mut children[0] { *width = 20.0; *height = 10.0; }
    }
    let outer: Element = serde_json::from_value(json!({"type":"group","id":"outer","x":100,"y":200,"width":400,"height":200,
        "view_width":200,"view_height":100,"children":[inner]})).unwrap();
    let source = deck(vec![outer]);
    let first = run(&source, json!({"op":"ungroup","ids":["outer"]}));
    assert_eq!(geometry(&first.deck.slides[0].elements[0]), [120.0,240.0,200.0,120.0]);
    let second = run(&first.deck, json!({"op":"ungroup","ids":["inner"]}));
    assert_eq!(geometry(&second.deck.slides[0].elements[0]), [124.0,248.0,80.0,40.0]);
    assert!(matches!(&second.deck.slides[0].elements[0], Element::Text { id, font_size, format, .. }
        if id == "leaf" && *font_size == 80.0 && format.placeholder.is_none() && !format.inherit_layout));
    assert_eq!(first.effects.removed_ids, ["outer"]);
}

fn connected() -> Deck {
    let connector = serde_json::from_value(json!({"type":"connector","id":"edge","x":100,"y":30,"width":150,"height":1,
        "color":"@accent3","stroke_width":2,"arrow":true,"start":{"element_id":"a","site":1},"end":{"element_id":"b","site":3}})).unwrap();
    deck(vec![rect("a", 10.0, 10.0, 90.0, 40.0), rect("b", 250.0, 10.0, 90.0, 40.0), connector])
}

#[test]
fn cross_container_connectors_reject_group_copy_and_cut_atomically() {
    let source = connected();
    for value in [json!({"op":"group","ids":["a","edge"],"group_id":"combined"}),
        json!({"op":"group","ids":["a","b"],"group_id":"combined"}),
        json!({"op":"copy","ids":["edge"],"format":"keep_source_formatting"}),
        json!({"op":"cut","ids":["a"],"format":"keep_source_formatting"})] {
        assert!(apply(&source, "slide", &operation(value), None).is_err());
    }
    let result = run(&source, json!({"op":"group","ids":["a","b","edge"],"group_id":"combined"}));
    aislide_core::model::validate_deck(&result.deck).unwrap();
}

#[test]
fn clipboard_remaps_recursive_ids_connections_and_keeps_source_theme() {
    let mut source = connected(); source.slides[0].elements.push(text("label", 10.0, 100.0));
    source.design.as_mut().unwrap().theme.fonts.minor = "Source Font".into();
    let grouped = run(&source, json!({"op":"group","ids":["a","b","edge","label"],"group_id":"combined"}));
    let before = serde_json::to_value(&grouped.deck).unwrap();
    let copied = run(&grouped.deck, json!({"op":"copy","ids":["combined"],"format":"keep_source_formatting"}));
    assert_eq!(serde_json::to_value(&copied.deck).unwrap(), before);
    let mut target = deck(vec![rect("paste-1", 0.0, 0.0, 10.0, 10.0)]);
    target.design.as_mut().unwrap().theme.colors.insert("accent1".into(), "FF0000".into());
    target.design.as_mut().unwrap().theme.fonts.minor = "Target Font".into();
    let paste = operation(json!({"op":"paste","id_prefix":"paste","dx":24,"dy":24}));
    let pasted = apply(&target, "slide", &paste, copied.clipboard.as_ref()).unwrap();
    assert_eq!(pasted.effects.id_map.len(), 5);
    assert!(pasted.effects.id_map.values().all(|id| id != "paste-1"));
    let group = &pasted.deck.slides[0].elements[1];
    assert_eq!(geometry(group), [34.0,34.0,330.0,130.0]);
    if let Element::Group { children, .. } = group {
        assert!(matches!(&children[0], Element::Rect { fill, .. } if fill == "087F73"));
        assert!(matches!(&children[3], Element::Text { color, format, .. } if color == "CF5847" && format.font_family.as_deref() == Some("Source Font")));
        assert!(matches!(&children[2], Element::Connector { start: Some(start), end: Some(end), .. }
            if start.element_id == children[0].bounds().0 && end.element_id == children[1].bounds().0));
    } else { panic!("expected pasted group"); }
    aislide_core::model::validate_deck(&pasted.deck).unwrap();
    assert_eq!(serde_json::to_value(&grouped.deck).unwrap(), before);
}

#[test]
fn cut_reports_all_removed_nodes_and_paste_requires_bundle() {
    let source = connected();
    let cut = run(&source, json!({"op":"cut","ids":["a","b","edge"],"format":"keep_source_formatting"}));
    assert!(cut.deck.slides[0].elements.is_empty());
    assert_eq!(cut.effects.removed_ids.len(), 3);
    assert!(cut.clipboard.is_some());
    assert!(apply(&source, "slide", &operation(json!({"op":"paste","id_prefix":"copy","dx":1,"dy":1})), None).is_err());
}

#[test]
fn schema_and_deserialization_are_strict() {
    let schema = serde_json::to_value(schemars::schema_for!(SelectionOperation)).unwrap();
    let encoded = schema.to_string();
    for name in ["translate", "resize", "align", "distribute", "group", "ungroup", "copy", "cut", "paste", "keep_source_formatting"] {
        assert!(encoded.contains(name), "missing schema operation {name}");
    }
    assert!(serde_json::from_value::<SelectionOperation>(json!({"op":"translate","ids":["a"],"dx":1,"dy":1,"hidden":true})).is_err());
}

#[test]
fn every_alignment_and_distribution_axis_obeys_reference_frame() {
    let source = deck(vec![rect("a", 100.0, 100.0, 100.0, 40.0), rect("b", 300.0, 200.0, 200.0, 80.0)]);
    for (relative_to, frame) in [("selection", [100.0,100.0,400.0,180.0]), ("page", [0.0,0.0,1280.0,720.0])] {
        for (alignment, expected) in [("left", [frame[0],200.0]), ("center", [frame[0]+(frame[2]-200.0)/2.0,200.0]),
            ("right", [frame[0]+frame[2]-200.0,200.0]), ("top", [300.0,frame[1]]),
            ("middle", [300.0,frame[1]+(frame[3]-80.0)/2.0]), ("bottom", [300.0,frame[1]+frame[3]-80.0])] {
            let result = run(&source, json!({"op":"align","ids":["a","b"],"alignment":alignment,"relative_to":relative_to}));
            let bounds = geometry(&result.deck.slides[0].elements[1]);
            assert_eq!([bounds[0], bounds[1]], expected);
        }
    }
    let source = deck(vec![rect("a", 10.0, 10.0, 100.0, 40.0), rect("b", 160.0, 130.0, 60.0, 60.0), rect("c", 510.0, 410.0, 100.0, 40.0)]);
    for (axis, relative_to, expected) in [("horizontal","selection",280.0), ("horizontal","page",610.0),
        ("vertical","selection",200.0), ("vertical","page",330.0)] {
        let result = run(&source, json!({"op":"distribute","ids":["c","b","a"],"axis":axis,"relative_to":relative_to}));
        let bounds = geometry(&result.deck.slides[0].elements[1]);
        assert_eq!(bounds[usize::from(axis == "vertical")], expected);
        assert_eq!(result.deck.slides[0].elements[0].bounds().0, "a");
    }
}

#[test]
fn nonfinite_and_nonpositive_transforms_never_mutate_source() {
    let source = deck(vec![text("a", 100.0, 100.0)]);
    let before = serde_json::to_value(&source).unwrap();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for operation in [SelectionOperation::Translate { ids: vec!["a".into()], dx: value, dy: 0.0 },
            SelectionOperation::Translate { ids: vec!["a".into()], dx: 0.0, dy: value },
            SelectionOperation::Resize { ids: vec!["a".into()], x: 0.0, y: 0.0, width: value, height: 40.0 }] {
            assert!(apply(&source, "slide", &operation, None).is_err());
        }
    }
    for (width, height) in [(0.0,40.0), (-1.0,40.0), (80.0,0.0), (80.0,-1.0), (f64::MAX, 40.0)] {
        assert!(apply(&source, "slide", &SelectionOperation::Resize { ids: vec!["a".into()], x: 0.0, y: 0.0, width, height }, None).is_err());
    }
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
}

#[test]
fn resize_normalizes_group_view_and_scales_fonts_once() {
    let source = deck(vec![text("a", 100.0, 100.0), text("b", 220.0, 140.0)]);
    let grouped = run(&source, json!({"op":"group","ids":["a","b"],"group_id":"group"}));
    let result = run(&grouped.deck, json!({"op":"resize","ids":["group"],"x":50,"y":60,"width":400,"height":240}));
    if let Element::Group { children, view_width, view_height, .. } = &result.deck.slides[0].elements[0] {
        assert_eq!((*view_width, *view_height), (400.0, 240.0));
        assert_eq!(geometry(&children[1]), [240.0,120.0,160.0,120.0]);
        assert!(matches!(&children[0], Element::Text { font_size, .. } if *font_size == 40.0));
    } else { panic!("expected group"); }
    let flattened = run(&result.deck, json!({"op":"ungroup","ids":["group"]}));
    assert_eq!(geometry(&flattened.deck.slides[0].elements[1]), [290.0,180.0,160.0,120.0]);
    assert!(matches!(&flattened.deck.slides[0].elements[0], Element::Text { font_size, .. } if *font_size == 40.0));
}

#[test]
fn group_and_clipboard_detach_inherited_text_without_losing_content() {
    let source = aislide_core::design::assign_layout(deck(Vec::new()), "slide", "title-content").unwrap();
    assert!(matches!(&source.slides[0].elements[0], Element::Text { format, .. } if format.inherit_layout));
    let grouped = run(&source, json!({"op":"group","ids":["title","body"],"group_id":"group"}));
    assert_eq!(grouped.effects.reparented_ids, ["body", "title"]);
    let flattened = run(&grouped.deck, json!({"op":"ungroup","ids":["group"]}));
    for (actual, expected) in flattened.deck.slides[0].elements.iter().zip(&source.slides[0].elements) {
        assert_eq!(geometry(actual), geometry(expected));
        match (actual, expected) {
            (Element::Text { text, format, .. }, Element::Text { text: original, .. }) => {
                assert_eq!(text, original); assert!(!format.inherit_layout); assert!(format.placeholder.is_none());
            }
            _ => panic!("expected text"),
        }
    }
    let copy = run(&source, json!({"op":"copy","ids":["title","body"],"format":"keep_source_formatting"}));
    for element in &copy.clipboard.unwrap().elements {
        assert!(matches!(element, Element::Text { format, .. } if !format.inherit_layout && format.placeholder.is_none()));
    }
    assert!(matches!(&source.slides[0].elements[0], Element::Text { format, .. } if format.inherit_layout));
}

#[test]
fn repeated_paste_is_deterministic_and_avoids_ids_across_slides() {
    let source = connected();
    let copy = run(&source, json!({"op":"copy","ids":["a","b","edge"],"format":"keep_source_formatting"}));
    let bundle: aislide_core::selection::ElementBundle = serde_json::from_value(serde_json::to_value(copy.clipboard.unwrap()).unwrap()).unwrap();
    let before = serde_json::to_value(&bundle).unwrap();
    let mut target = deck(Vec::new());
    let mut other = deck(vec![rect("paste-1", 0.0, 0.0, 10.0, 10.0)]).slides.remove(0); other.id = "other".into();
    target.slides.push(other);
    let operation = operation(json!({"op":"paste","id_prefix":"paste","dx":0,"dy":0}));
    let first = apply(&target, "slide", &operation, Some(&bundle)).unwrap();
    let deterministic = apply(&target, "slide", &operation, Some(&bundle)).unwrap();
    assert_eq!(serde_json::to_value(&first).unwrap(), serde_json::to_value(&deterministic).unwrap());
    assert!(first.effects.id_map.values().all(|id| id != "paste-1"));
    let second = apply(&first.deck, "slide", &operation, Some(&bundle)).unwrap();
    assert!(second.effects.id_map.values().all(|id| !first.effects.id_map.values().any(|prior| prior == id)));
    assert_eq!(serde_json::to_value(&bundle).unwrap(), before);
    let outside = SelectionOperation::Paste { id_prefix: "paste".into(), dx: 1000.0, dy: 0.0 };
    assert!(apply(&target, "slide", &outside, Some(&bundle)).is_err());
}

#[test]
fn implicit_theme_styles_fail_closed_and_explicit_fonts_resolve() {
    let mut source = deck(vec![text("title", 10.0, 10.0), text("body", 100.0, 10.0)]);
    if let Element::Text { format, .. } = &mut source.slides[0].elements[0] { format.font_family = Some("@major".into()); }
    if let Element::Text { format, .. } = &mut source.slides[0].elements[1] { format.font_family = None; }
    source.design.as_mut().unwrap().theme.fonts.major = "Source Heading".into();
    source.design.as_mut().unwrap().theme.fonts.minor = "Source Body".into();
    let copy = run(&source, json!({"op":"copy","ids":["title","body"],"format":"keep_source_formatting"}));
    let bundle = copy.clipboard.unwrap();
    for (element, font) in bundle.elements.iter().zip(["Source Heading", "Source Body"]) {
        assert!(matches!(element, Element::Text { format, .. } if format.font_family.as_deref() == Some(font)));
    }
    let mut target = deck(Vec::new());
    let paste = operation(json!({"op":"paste","id_prefix":"copy","dx":0,"dy":0}));
    assert!(apply(&target, "slide", &paste, Some(&bundle)).is_ok());
    target.design.as_mut().unwrap().theme.fonts.east_asian = "Different Script Font".into();
    assert!(apply(&target, "slide", &paste, Some(&bundle)).is_err());
    let table = Element::Table { id: "table".into(), x: 0.0, y: 0.0, width: 200.0, height: 100.0, rows: vec![vec!["Synthetic".into()]], font_size: 20.0, format: Default::default() };
    let source = deck(vec![table]);
    let copy = run(&source, json!({"op":"copy","ids":["table"],"format":"keep_source_formatting"}));
    let mut target = deck(Vec::new());
    assert!(apply(&target, "slide", &paste, copy.clipboard.as_ref()).is_ok());
    target.design.as_mut().unwrap().theme.colors.insert("accent1".into(), "FF0000".into());
    assert!(apply(&target, "slide", &paste, copy.clipboard.as_ref()).is_err());
}

#[test]
fn invalid_bundles_and_existing_node_limits_are_rejected() {
    let source = connected();
    let bundle = run(&source, json!({"op":"copy","ids":["a","b","edge"],"format":"keep_source_formatting"})).clipboard.unwrap();
    let paste = operation(json!({"op":"paste","id_prefix":"copy","dx":0,"dy":0}));
    let mut empty = bundle.clone(); empty.elements.clear();
    let mut duplicate = bundle.clone(); duplicate.elements.push(duplicate.elements[0].clone());
    let mut dangling = bundle.clone(); dangling.elements.remove(0);
    let mut unknown = bundle.clone(); unknown.version = 2;
    for invalid in [empty, duplicate, dangling, unknown] { assert!(apply(&source, "slide", &paste, Some(&invalid)).is_err()); }
    let source = deck((0..256).map(|index| rect(&format!("item-{index}"), 10.0, 10.0, 10.0, 10.0)).collect());
    assert!(apply(&source, "slide", &operation(json!({"op":"group","ids":["item-0","item-1"],"group_id":"group"})), None).is_err());
    assert!(apply(&source, "slide", &paste, Some(&bundle)).is_err());
    let mut total = deck((0..64).map(|index| rect(&format!("item-{index}"), 10.0, 10.0, 10.0, 10.0)).collect());
    for index in 1..32 { let mut slide = total.slides[0].clone(); slide.id = format!("slide-{index}"); total.slides.push(slide); }
    aislide_core::model::validate_deck(&total).unwrap();
    assert!(apply(&total, "slide", &paste, Some(&bundle)).is_ok());
    total.design = None;
    for index in 32..128 { let mut slide = total.slides[0].clone(); slide.id = format!("slide-{index}"); total.slides.push(slide); }
    aislide_core::model::validate_deck(&total).unwrap();
    assert!(apply(&total, "slide", &paste, Some(&bundle)).is_err());
}

#[test]
fn native_identity_gate_checks_collisions_coverage_and_bounded_geometry() {
    use aislide_core::selection::validate_native_ids;
    let source = connected();
    let grouped = run(&source, json!({"op":"group","ids":["a","b","edge"],"group_id":"group"}));
    let allocation = [("group".into(), 2), ("a".into(), 3), ("b".into(), 4), ("edge".into(), 5)].into_iter().collect();
    validate_native_ids(&grouped.deck.slides[0].elements, &allocation).unwrap();
    let ungrouped = run(&grouped.deck, json!({"op":"ungroup","ids":["group"]}));
    validate_native_ids(&ungrouped.deck.slides[0].elements, &allocation).unwrap();
    let mut collision = allocation.clone(); collision.insert("unmodeled-retained-node".into(), 3);
    assert!(validate_native_ids(&ungrouped.deck.slides[0].elements, &collision).is_err());
    let mut missing = allocation.clone(); missing.remove("edge");
    assert!(validate_native_ids(&ungrouped.deck.slides[0].elements, &missing).is_err());
    let mut reserved = allocation.clone(); reserved.insert("a".into(), 1);
    assert!(validate_native_ids(&ungrouped.deck.slides[0].elements, &reserved).is_err());
    let mut invalid = ungrouped.deck.slides[0].elements.clone();
    if let Element::Rect { x, .. } = &mut invalid[0] { *x = -1.0; }
    assert!(validate_native_ids(&invalid, &allocation).is_err());
}

#[test]
fn full_synthetic_sample_remains_intact_after_structural_clipboard_roundtrips() {
    use aislide_core::{report::{compile_report, sample_report}, pptx::{export_pptx, inspect_pptx}};
    let source = compile_report(&sample_report()).unwrap().deck;
    let before = serde_json::to_value(&source).unwrap();
    let original_pptx = export_pptx(&source).unwrap();
    assert_eq!(source.slides.len(), 12);
    let mut target = source.clone();
    for slide in &mut target.slides { slide.elements.clear(); }
    for slide in &source.slides {
        let ids: Vec<_> = slide.elements.iter().map(|element| element.bounds().0).collect();
        let copied = apply(&source, &slide.id, &operation(json!({"op":"copy","ids":ids,"format":"keep_source_formatting"})), None).unwrap();
        let pasted = apply(&target, &slide.id, &operation(json!({"op":"paste","id_prefix":"sample-copy","dx":0,"dy":0})), copied.clipboard.as_ref()).unwrap();
        let output = pasted.deck.slides.iter().find(|entry| entry.id == slide.id).unwrap();
        assert_eq!(output.elements.len(), slide.elements.len());
        for (actual, expected) in output.elements.iter().zip(&slide.elements) {
            assert_eq!(geometry(actual), geometry(expected));
            match (actual, expected) {
                (Element::Text { text, font_size, .. }, Element::Text { text: original, font_size: original_size, .. }) => { assert_eq!(text, original); assert_eq!(font_size, original_size); }
                (Element::Table { rows, .. }, Element::Table { rows: original, .. }) => assert_eq!(rows, original),
                _ => {}
            }
        }
        target = pasted.deck;
    }
    assert_eq!(inspect_pptx(export_pptx(&target).unwrap()).unwrap().slides.len(), 12);
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    assert_eq!(export_pptx(&source).unwrap(), original_pptx);
}

#[test]
fn rich_run_fonts_colors_and_absolute_paragraph_geometry_follow_selection() {
    let element = serde_json::from_value(json!({"type":"text","id":"rich","x":100,"y":100,"width":200,"height":100,
        "text":"Synthetic","font_size":20,"color":"@dk1","bold":false,"format":{"font_family":"@minor",
        "paragraphs":[{"margin_left":95250,"indent":-9525,"line_spacing":{"kind":"points","value":1800},
            "space_before":{"kind":"percent","value":100000},"space_after":{"kind":"points","value":600},
            "tabs":[{"position":190500}],"runs":[{"text":"Synthetic","style":{"font_size":24,
                "font_family":"@major","color":"@accent1","highlight":"@accent2","baseline":10000}}]}]}})).unwrap();
    let mut source = deck(vec![element]);
    source.design.as_mut().unwrap().theme.fonts.major = "Rich Source".into();
    let resized = run(&source, json!({"op":"resize","ids":["rich"],"x":100,"y":100,"width":400,"height":300}));
    let value = serde_json::to_value(&resized.deck.slides[0].elements[0]).unwrap();
    let paragraph = &value["format"]["paragraphs"][0];
    assert_eq!(paragraph["runs"][0]["style"]["font_size"], 48.0);
    assert_eq!(paragraph["margin_left"], 190500); assert_eq!(paragraph["indent"], -19050);
    assert_eq!(paragraph["tabs"][0]["position"], 381000);
    assert_eq!(paragraph["line_spacing"]["value"], 3600);
    assert_eq!(paragraph["space_before"]["value"], 100000);
    assert_eq!(paragraph["space_after"]["value"], 1200);
    assert_eq!(paragraph["runs"][0]["style"]["baseline"], 10000);
    let copied = run(&source, json!({"op":"copy","ids":["rich"],"format":"keep_source_formatting"}));
    let bundle = copied.clipboard.unwrap();
    let value = serde_json::to_value(&bundle.elements[0]).unwrap();
    assert_eq!(value["format"]["paragraphs"][0]["runs"][0]["style"]["font_family"], "Rich Source");
    assert_eq!(value["format"]["paragraphs"][0]["runs"][0]["style"]["color"], "087F73");
    assert_eq!(value["format"]["paragraphs"][0]["runs"][0]["style"]["highlight"], "CF5847");
    let mut tiny = source.clone();
    if let Element::Text { format, .. } = &mut tiny.slides[0].elements[0] { format.paragraphs[0].runs[0].style.font_size = Some(8.0); }
    assert!(apply(&tiny, "slide", &operation(json!({"op":"resize","ids":["rich"],"x":0,"y":0,"width":100,"height":50})), None).is_err());
}