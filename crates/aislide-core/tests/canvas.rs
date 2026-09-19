use aislide_core::{model::{Deck, Slide, validate_deck}, package::Package, pptx};

fn blank(width: u32, height: u32) -> Deck {
    Deck { version: 1, title: "Canvas".into(), width, height, design: None, embedded_fonts: Vec::new(), auxiliary_design: None, slides: vec![Slide {
        id: "page".into(), title: "Page".into(), background: "FFFFFF".into(), elements: vec![], notes: String::new(), notes_paragraphs: Vec::new(),
        layout_id: None, inherit_background: false, hide_master_graphics: false, native_source_id: None, review: None,
    }] }
}

#[test]
fn variable_page_sizes_validate_and_export_actual_dimensions() {
    for (width, height) in [(1280, 720), (960, 720), (720, 1280), (320, 320), (4096, 4096)] {
        let deck = blank(width, height);
        validate_deck(&deck).unwrap();
        let package = Package::open(pptx::export_pptx(&deck).unwrap()).unwrap();
        let xml = roxmltree::Document::parse(package.text("ppt/presentation.xml").unwrap()).unwrap();
        let size = xml.descendants().find(|node| node.tag_name().name() == "sldSz").unwrap();
        assert_eq!(size.attribute("cx").unwrap().parse::<u64>().unwrap(), u64::from(width) * 9525);
        assert_eq!(size.attribute("cy").unwrap().parse::<u64>().unwrap(), u64::from(height) * 9525);
    }
}

#[test]
fn invalid_canvas_dimensions_remain_rejected() {
    for (width, height) in [(0, 720), (1280, 0), (319, 720), (4097, 720), (720, u32::MAX)] {
        assert!(validate_deck(&blank(width, height)).is_err());
    }
}

fn opened(bytes: Vec<u8>) -> aislide_core::document::Document {
    serde_json::from_value(aislide_core::document::open_presentation("canvas-test".into(), bytes).unwrap()["document"].clone()).unwrap()
}

fn saved(document: &aislide_core::document::Document) -> Vec<u8> {
    use base64::Engine;
    let result = aislide_core::document::export_presentation(document).unwrap();
    base64::engine::general_purpose::STANDARD.decode(result["base64"].as_str().unwrap()).unwrap()
}

#[test]
fn native_page_aspects_and_large_physical_original_are_preserved() {
    for (width, height) in [(960, 720), (720, 1280), (1600, 1000), (4096, 4096)] {
        let bytes = pptx::export_pptx(&blank(width, height)).unwrap();
        let document = opened(bytes.clone());
        assert_eq!((document.deck.width, document.deck.height), (width, height));
        assert_eq!(saved(&document), bytes);
    }
    let mut package = Package::open(pptx::export_pptx(&blank(1280, 720)).unwrap()).unwrap();
    let xml = package.text("ppt/presentation.xml").unwrap().replace("cx=\"12192000\" cy=\"6858000\"", "cx=\"24384000\" cy=\"13716000\"");
    package.replace_part("ppt/presentation.xml", xml.into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let document = opened(bytes.clone());
    assert_eq!((document.deck.width, document.deck.height), (1280, 720));
    assert_eq!(saved(&document), bytes);
}

#[test]
fn resize_roundtrip_new_positions_and_undo() {
    use aislide_core::{canvas::{resize, ResizeMode}, document::{transact, Transaction}, model::Element};
    let mut deck = blank(1280, 720);
    deck.slides[0].elements.push(Element::Rect { visual: None, id: "box".into(), x: 640.0, y: 360.0, width: 100.0, height: 50.0, fill: "AABBCC".into() });
    assert!(resize(deck.clone(), 320, 320, ResizeMode::Keep).is_err());
    assert!(resize(deck.clone(), 0, 320, ResizeMode::Scale).is_err());
    let document = opened(pptx::export_pptx(&deck).unwrap());
    let original = saved(&document);
    let mut candidate = resize(document.deck.clone(), 720, 1280, ResizeMode::Scale).unwrap();
    assert_eq!(candidate.slides[0].elements[0].bounds().1, 360.0);
    assert_eq!(candidate.slides[0].elements[0].bounds().2, 640.0);
    candidate.slides[0].elements.push(Element::Rect { visual: None, id: "bottom".into(), x: 500.0, y: 1100.0, width: 100.0, height: 100.0, fill: "ABCDEF".into() });
    let result = transact(&document, Transaction { expected_revision: document.revision, expected_hash: document.hash.clone(), operations: serde_json::from_value(serde_json::json!([{"op":"replace","path":"/deck","value":candidate}])).unwrap() }).unwrap();
    let reopened = opened(saved(&result.document));
    assert_eq!((reopened.deck.width, reopened.deck.height), (720, 1280));
    assert_eq!(reopened.deck.slides[0].elements[1].bounds().2, 1100.0);
    let restored = aislide_core::document::undo(&result.document, result.document.revision, result.receipt.unwrap()).unwrap();
    assert_eq!(saved(&restored.document), original);
}

#[test]
fn portrait_clipboard_and_native_id_bounds_use_the_actual_canvas() {
    use aislide_core::{model::Element, selection::{self, ClipboardFormat, SelectionOperation}};
    let mut deck = blank(720, 1280);
    deck.slides[0].elements.push(Element::Rect { visual: None, id: "bottom".into(), x: 400.0, y: 1000.0, width: 100.0, height: 80.0, fill: "123456".into() });
    let copied = selection::apply(&deck, "page", &SelectionOperation::Copy { ids: vec!["bottom".into()], format: ClipboardFormat::KeepSourceFormatting }, None).unwrap();
    let pasted = selection::apply(&deck, "page", &SelectionOperation::Paste { id_prefix: "copy".into(), dx: 100.0, dy: 100.0 }, copied.clipboard.as_ref()).unwrap();
    assert_eq!(pasted.deck.slides[0].elements[1].bounds().2, 1100.0);
    let allocation = std::collections::BTreeMap::from([("bottom".into(), 2)]);
    selection::validate_native_ids_on_canvas(&deck.slides[0].elements, &allocation, 720, 1280).unwrap();
    assert!(selection::validate_native_ids_on_canvas(&deck.slides[0].elements, &allocation, 1280, 720).is_err());
}

#[test]
fn resizing_a_double_physical_native_page_keeps_its_units_and_marker() {
    use aislide_core::{canvas::{resize, ResizeMode}, document::{transact, Transaction}, model::Element};
    let mut package = Package::open(pptx::export_pptx(&blank(1280, 720)).unwrap()).unwrap();
    let xml = package.text("ppt/presentation.xml").unwrap().replace("cx=\"12192000\" cy=\"6858000\"", "cx=\"24384000\" cy=\"13716000\"");
    package.replace_part("ppt/presentation.xml", xml.into_bytes()).unwrap();
    let document = opened(package.save().unwrap());
    let mut candidate = resize(document.deck.clone(), 720, 1280, ResizeMode::Keep).unwrap();
    candidate.slides[0].elements.push(Element::Rect { visual: None, id: "new-bottom".into(), x: 500.0, y: 1100.0, width: 100.0, height: 100.0, fill: "ABCDEF".into() });
    let result = transact(&document, Transaction { expected_revision: 0, expected_hash: document.hash.clone(), operations: serde_json::from_value(serde_json::json!([{"op":"replace","path":"/deck","value":candidate}])).unwrap() }).unwrap();
    let bytes = saved(&result.document);
    let package = Package::open(bytes.clone()).unwrap();
    assert!(package.text("ppt/presentation.xml").unwrap().contains("cx=\"13716000\" cy=\"24384000\""));
    let reopened = opened(bytes);
    assert_eq!((reopened.deck.width, reopened.deck.height), (720, 1280));
    assert_eq!(reopened.deck.slides[0].elements[0].bounds().2, 1100.0);
}

#[test]
fn resize_scales_master_placeholders_and_group_child_canvas() {
    use aislide_core::{canvas::{resize, ResizeMode}, design::{self, Design}, document::{transact, Transaction}, model::Element};
    let mut deck = blank(1280, 720); deck.design = Some(Design::default());
    deck = design::assign_layout(deck, "page", "title-content").unwrap();
    deck.slides[0].elements.push(Element::Group { visual: None, id: "group".into(), x: 500.0, y: 400.0, width: 200.0, height: 200.0, view_width: 200.0, view_height: 200.0, children: vec![Element::Rect { visual: None, id: "child".into(), x: 20.0, y: 20.0, width: 100.0, height: 100.0, fill: "ABCDEF".into() }] });
    let authored = aislide_core::document::create("canvas-test".into(), deck, vec![], vec![], None).unwrap();
    let document = opened(saved(&authored));
    let candidate = resize(document.deck.clone(), 960, 720, ResizeMode::Scale).unwrap();
    let result = transact(&document, Transaction { expected_revision: 0, expected_hash: document.hash.clone(), operations: serde_json::from_value(serde_json::json!([{"op":"replace","path":"/deck","value":candidate}])).unwrap() }).unwrap();
    let reopened = opened(saved(&result.document));
    assert_eq!((reopened.deck.width, reopened.deck.height), (960, 720));
    assert_eq!(reopened.deck.slides[0].elements[0].bounds().1, 48.0);
    let group = reopened.deck.slides[0].elements.iter().find(|element| element.bounds().0 == "group").unwrap();
    if let Element::Group { width, view_width, children, .. } = group {
        assert_eq!(*width, 150.0); assert_eq!(*view_width, 200.0); assert_eq!(children[0].bounds().1, 20.0);
    } else { panic!("group not preserved"); }
}