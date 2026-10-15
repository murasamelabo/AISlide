use aislide_core::{design::{Design, Master, SlideLayout}, document::{self, Document, Transaction}, editing, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::json;
use std::collections::BTreeSet;

fn source() -> Document {
    let mut deck = editing::create("topology".into(), "Topology".into()).unwrap().deck;
    deck.design = Some(Design::default());
    document::create("topology".into(), deck, vec![], vec![], None).unwrap()
}

fn save(document: &Document) -> Vec<u8> {
    STANDARD.decode(document::export_presentation(document).unwrap()["base64"].as_str().unwrap()).unwrap()
}

fn open(bytes: Vec<u8>) -> Document {
    serde_json::from_value(document::open_presentation("topology".into(), bytes).unwrap()["document"].clone()).unwrap()
}

fn change(document: &Document, deck: aislide_core::model::Deck) -> document::TransactionResult {
    document::transact(document, Transaction { expected_revision: document.revision, expected_hash: document.hash.clone(), operations: serde_json::from_value(json!([{"op":"replace","path":"/deck","value":deck}])).unwrap() }).unwrap()
}

fn issue23_package(elements: Vec<aislide_core::model::Element>, canvas: (u32, u32)) -> Package {
    let mut deck = source().deck;
    deck.width = canvas.0; deck.height = canvas.1; deck.design = None;
    deck.slides[0].layout_id = None; deck.slides[0].inherit_background = false;
    deck.slides[0].background = "FFFFFF".into(); deck.slides[0].elements = elements;
    Package::open(aislide_core::pptx::export_pptx(&deck).unwrap()).unwrap()
}

fn issue23_shape_range(xml: &str, id: &str) -> std::ops::Range<usize> {
    let parsed = roxmltree::Document::parse(xml).unwrap();
    parsed.descendants().find(|node| node.tag_name().name() == "cNvPr" && node.attribute("name") == Some(id)).unwrap().ancestors()
        .find(|node| node.tag_name().namespace() == Some("http://schemas.openxmlformats.org/presentationml/2006/main") && ["sp", "pic", "grpSp"].contains(&node.tag_name().name())).unwrap().range()
}

fn issue23_edit_shape(package: &mut Package, path: &str, id: &str, edit: impl FnOnce(&str) -> String) {
    let mut xml = package.text(path).unwrap().to_owned();
    let range = issue23_shape_range(&xml, id);
    let replacement = edit(&xml[range.clone()]);
    xml.replace_range(range, &replacement);
    package.replace_part(path, xml.into_bytes()).unwrap();
}

fn issue23_set_frame(package: &mut Package, path: &str, id: &str, bounds: [i64; 4]) {
    let mut xml = package.text(path).unwrap().to_owned();
    let mut replacements = {
        let parsed = roxmltree::Document::parse(&xml).unwrap();
        let range = issue23_shape_range(&xml, id);
        let shape = parsed.descendants().find(|node| node.range() == range).unwrap();
        [("off", [("x", bounds[0]), ("y", bounds[1])]), ("ext", [("cx", bounds[2]), ("cy", bounds[3])])].into_iter().map(|(tag, attributes)| {
            let node = shape.descendants().find(|node| node.has_tag_name(("http://schemas.openxmlformats.org/drawingml/2006/main", tag))).unwrap();
            let mut replacement = xml[node.range()].to_owned();
            for (name, value) in attributes { replacement = replacement.replace(&format!("{name}=\"{}\"", node.attribute(name).unwrap()), &format!("{name}=\"{value}\"")); }
            (node.range(), replacement)
        }).collect::<Vec<_>>()
    };
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, replacement) in replacements { xml.replace_range(range, &replacement); }
    package.replace_part(path, xml.into_bytes()).unwrap();
}

fn issue23_rect(id: &str, x: f64, y: f64, width: f64, height: f64) -> aislide_core::model::Element {
    aislide_core::model::Element::Rect { visual: None, id: id.into(), x, y, width, height, fill: "112233".into() }
}

fn issue23_native_rects() -> Vec<u8> {
    use aislide_core::model::Element;
    let mut deck = source().deck;
    deck.slides[0].elements = vec![
        Element::Rect { visual: None, id: "edge-left".into(), x: 0.0, y: 160.0, width: 160.0, height: 80.0, fill: "112233".into() },
        Element::Rect { visual: None, id: "edge-right".into(), x: f64::from(deck.width) - 160.0, y: 280.0, width: 160.0, height: 80.0, fill: "445566".into() },
        issue23_rect("control", 80.0, 40.0, 160.0, 80.0),
    ];
    let mut package = Package::open(aislide_core::pptx::export_pptx(&deck).unwrap()).unwrap();
    let path = "ppt/slides/slide1.xml";
    let mut xml = package.text(path).unwrap().to_owned();
    let mut replacements = {
        let parsed = roxmltree::Document::parse(&xml).unwrap();
        [("edge-left", "off", "x", -1), ("edge-right", "ext", "cx", 2259)].into_iter().map(|(id, tag, attribute, delta)| {
            let shape = parsed.descendants().find(|node| node.tag_name().name() == "cNvPr" && node.attribute("name") == Some(id)).unwrap().ancestors().find(|node| node.tag_name().name() == "sp").unwrap();
            let frame = shape.descendants().find(|node| node.has_tag_name(("http://schemas.openxmlformats.org/drawingml/2006/main", tag))).unwrap();
            let value = frame.attribute(attribute).unwrap().parse::<i64>().unwrap();
            let replacement = xml[frame.range()].replace(&format!("{attribute}=\"{value}\""), &format!("{attribute}=\"{}\"", value + delta));
            (frame.range(), replacement)
        }).collect::<Vec<_>>()
    };
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, replacement) in replacements { xml.replace_range(range, &replacement); }
    package.replace_part(path, xml.into_bytes()).unwrap();
    package.save().unwrap()
}

#[test]
fn issue23_native_negative_one_emu_rect_stays_in_projection() {
    let document = open(issue23_native_rects());
    let element = document.deck.slides[0].elements.iter().find(|element| element.bounds().0 == "edge-left").expect("native x=-1 EMU rectangle must remain visible");
    assert_eq!(element.bounds().1, 0.0);
    assert!(element.bounds().3 > 159.99);
}

#[test]
fn issue23_native_right_2259_emu_rect_stays_in_projection() {
    let document = open(issue23_native_rects());
    let element = document.deck.slides[0].elements.iter().find(|element| element.bounds().0 == "edge-right").expect("native right overflow of 2259 EMU must remain visible");
    let (_, x, _, width, _) = element.bounds();
    assert!((x + width - f64::from(document.deck.width)).abs() < 1e-8);
}

fn issue23_edges(overflow: i64) -> [(&'static str, [i64; 4]); 4] {
    [("left", [-overflow, 160 * 9525, 160 * 9525, 80 * 9525]),
        ("top", [320 * 9525, -overflow, 160 * 9525, 80 * 9525]),
        ("right", [1120 * 9525, 280 * 9525, 160 * 9525 + overflow, 80 * 9525]),
        ("bottom", [640 * 9525, 640 * 9525, 160 * 9525, 80 * 9525 + overflow])]
}

fn issue23_edge_elements() -> Vec<aislide_core::model::Element> {
    vec![issue23_rect("left", 0.0, 160.0, 160.0, 80.0), issue23_rect("top", 320.0, 0.0, 160.0, 80.0),
        issue23_rect("right", 1120.0, 280.0, 160.0, 80.0), issue23_rect("bottom", 640.0, 640.0, 160.0, 80.0)]
}

#[test]
fn issue23_same_subpixel_tolerance_on_all_edges_keeps_authored_guard_strict() {
    for overflow in [1, 95, 96, 2259, 9524] {
        let mut package = issue23_package(issue23_edge_elements(), (1280, 720));
        for (id, bounds) in issue23_edges(overflow) { issue23_set_frame(&mut package, "ppt/slides/slide1.xml", id, bounds); }
        let bytes = package.save().unwrap();
        let opened = document::open_presentation("issue23".into(), bytes.clone()).unwrap();
        let document: Document = serde_json::from_value(opened["document"].clone()).unwrap();
        let warnings = opened["warnings"].as_array().unwrap();
        assert_eq!(document.deck.slides[0].elements.len(), 4, "overflow {overflow} EMU");
        for (id, bounds) in issue23_edges(overflow) {
            let element = document.deck.slides[0].elements.iter().find(|element| element.bounds().0 == id).unwrap();
            let (_, x, y, width, height) = element.bounds();
            let native = bounds.map(|value| value as f64 / 9525.0);
            assert_eq!(x, native[0].max(0.0)); assert_eq!(y, native[1].max(0.0));
            assert!((width - ((native[0] + native[2]).min(1280.0) - x)).abs() < 1e-8);
            assert!((height - ((native[1] + native[3]).min(720.0) - y)).abs() < 1e-8);
            let expected = format!("{id}={:.6}px", overflow as f64 / 9525.0);
            assert!(warnings.iter().filter_map(|warning| warning.as_str()).any(|warning| warning.contains("slide ") && warning.contains("ppt/slides/slide1.xml") && warning.contains(&format!("element {id}")) && warning.contains(&expected)), "missing scoped edge warning: {warnings:?}");
        }
        assert_eq!(save(&document), bytes);
    }
    for (_, bounds) in issue23_edges(2259) {
        let native = bounds.map(|value| value as f64 / 9525.0);
        let mut deck = source().deck;
        deck.slides[0].elements = vec![issue23_rect("authored", native[0], native[1], native[2], native[3])];
        assert!(aislide_core::model::validate_deck(&deck).is_err());
        assert!(aislide_core::pptx::export_pptx(&deck).is_err());
    }
}

#[test]
fn issue23_normalized_projection_preserves_native_frames_on_unrelated_edit_and_undo() {
    let bytes = issue23_native_rects();
    let original = open(bytes.clone());
    assert_eq!(save(&original), bytes);
    let mut deck = original.deck.clone();
    let control = deck.slides[0].elements.iter_mut().find(|element| element.bounds().0 == "control").unwrap();
    if let aislide_core::model::Element::Rect { fill, .. } = control { *fill = "AABBCC".into(); }
    let changed = change(&original, deck);
    let output = save(&changed.document);
    let before = Package::open(bytes.clone()).unwrap(); let after = Package::open(output.clone()).unwrap();
    for id in ["edge-left", "edge-right"] {
        let before_xml = before.text("ppt/slides/slide1.xml").unwrap(); let after_xml = after.text("ppt/slides/slide1.xml").unwrap();
        assert_eq!(&before_xml[issue23_shape_range(before_xml, id)], &after_xml[issue23_shape_range(after_xml, id)]);
    }
    let reopened = open(output);
    for id in ["edge-left", "edge-right"] {
        assert_eq!(serde_json::to_value(original.deck.slides[0].elements.iter().find(|element| element.bounds().0 == id).unwrap()).unwrap(), serde_json::to_value(reopened.deck.slides[0].elements.iter().find(|element| element.bounds().0 == id).unwrap()).unwrap());
    }
    let restored = document::undo(&changed.document, changed.document.revision, changed.receipt.unwrap()).unwrap();
    assert_eq!(save(&restored.document), bytes);
}

#[test]
fn issue23_master_layout_and_slide_warnings_are_scoped_and_xml_stays_original() {
    let mut deck = source().deck;
    deck.design.as_mut().unwrap().masters[0].elements = issue23_edge_elements();
    deck.design.as_mut().unwrap().layouts.iter_mut().find(|layout| layout.id == "blank").unwrap().elements = issue23_edge_elements();
    deck.slides[0].elements = issue23_edge_elements();
    let mut package = Package::open(aislide_core::pptx::export_pptx(&deck).unwrap()).unwrap();
    let paths = [("master", "ppt/slideMasters/slideMaster1.xml"), ("layout", "ppt/slideLayouts/slideLayout1.xml"), ("slide", "ppt/slides/slide1.xml")];
    for (_, path) in paths { for (id, bounds) in issue23_edges(2259) { issue23_set_frame(&mut package, path, id, bounds); } }
    let bytes = package.save().unwrap();
    let opened = document::open_presentation("issue23-owners".into(), bytes.clone()).unwrap();
    let original: Document = serde_json::from_value(opened["document"].clone()).unwrap();
    let design = original.deck.design.as_ref().unwrap();
    for elements in [&design.masters[0].elements, &design.layouts[0].elements, &original.deck.slides[0].elements] { assert_eq!(elements.len(), 4); }
    for (scope, path) in paths { for (id, _) in issue23_edges(2259) {
        assert!(opened["warnings"].as_array().unwrap().iter().filter_map(|warning| warning.as_str()).any(|warning| warning.contains(&format!("{scope} ")) && warning.contains(path) && warning.contains(&format!("element {id}")) && warning.contains(&format!("{id}=0.237165px"))));
    } }
    assert_eq!(save(&original), bytes);
    let mut deck = original.deck.clone(); deck.title = "Unrelated title edit".into();
    let changed = change(&original, deck);
    let output = Package::open(save(&changed.document)).unwrap();
    for (_, path) in paths { assert_eq!(output.part(path).unwrap(), package.part(path).unwrap(), "{path}"); }
    let restored = document::undo(&changed.document, changed.document.revision, changed.receipt.unwrap()).unwrap();
    assert_eq!(save(&restored.document), bytes);
}

#[test]
fn issue23_read_only_projection_does_not_block_whole_slide_removal() {
    use aislide_core::editing::{self, SlideOperation};
    let mut package = issue23_package(vec![issue23_rect("clipped", 0.0, 160.0, 1280.0, 80.0)], (1280, 720));
    issue23_set_frame(&mut package, "ppt/slides/slide1.xml", "clipped", [-2 * 9525, 160 * 9525, 1280 * 9525, 80 * 9525]);
    let bytes = package.save().unwrap();
    let original = open(bytes.clone());
    let source_id = original.deck.slides[0].id.clone();
    let added = editing::slides(&original, original.revision, &[SlideOperation::Insert { id: "survivor".into(), after: None, title: "Surviving slide".into(), layout_id: None }]).unwrap();
    let before = save(&added.document);
    let removed = editing::slides(&added.document, added.document.revision, &[SlideOperation::Remove { slide_id: source_id }]).unwrap();
    assert_eq!(removed.document.deck.slides.len(), 1);
    assert_eq!(removed.document.deck.slides[0].id, "survivor");
    assert_eq!(open(save(&removed.document)).deck.slides.len(), 1);
    let restored = document::undo(&removed.document, removed.document.revision, removed.receipt.unwrap()).unwrap();
    assert_eq!(save(&restored.document), before);
    assert_eq!(save(&original), bytes);
    for (content_type, payload) in [
        ("application/vnd.openxmlformats-package.digital-signature-xmlsignature+xml", "<ds:Signature xmlns:ds='http://www.w3.org/2000/09/xmldsig#'/>"),
        ("application/xml", "<p:modifyVerifier xmlns:p='http://schemas.openxmlformats.org/presentationml/2006/main'/>"),
    ] {
        let package = Package::open(before.clone()).unwrap();
        let mut parts = package.parts().clone();
        parts.insert("docProps/guard.xml".into(), payload.as_bytes().to_vec());
        let types = package.text("[Content_Types].xml").unwrap().replace("</Types>", &format!("<Override PartName='/docProps/guard.xml' ContentType='{content_type}'/></Types>"));
        parts.insert("[Content_Types].xml".into(), types.into_bytes());
        let guarded_bytes = Package::from_parts(parts).unwrap().save().unwrap();
        let guarded = open(guarded_bytes.clone());
        let result = editing::slides(&guarded, guarded.revision, &[SlideOperation::Remove { slide_id: guarded.deck.slides[0].id.clone() }]);
        assert!(result.is_err(), "protected source allowed whole-slide removal: {content_type}");
        assert_eq!(save(&guarded), guarded_bytes);
    }
}

#[test]
fn issue23_inside_and_full_canvas_groups_are_unchanged_but_large_group_clip_is_opaque() {
    use aislide_core::model::Element;
    let full = Element::Group { visual: None, id: "full".into(), x: 0.0, y: 0.0, width: 1280.0, height: 720.0, view_width: 1280.0, view_height: 720.0, children: vec![issue23_rect("full-child", 0.0, 0.0, 1280.0, 720.0)] };
    let inside = Element::Group { visual: None, id: "inside".into(), x: 40.0, y: 60.0, width: 400.0, height: 240.0, view_width: 200.0, view_height: 120.0, children: vec![issue23_rect("inside-child", 10.0, 10.0, 100.0, 80.0)] };
    let mut package = issue23_package(vec![full, inside, issue23_rect("probe", 0.0, 300.0, 160.0, 80.0)], (1280, 720));
    let baseline = open(package.save().unwrap());
    issue23_set_frame(&mut package, "ppt/slides/slide1.xml", "probe", [-1, 300 * 9525, 160 * 9525, 80 * 9525]);
    let normalized = open(package.save().unwrap());
    assert_eq!(serde_json::to_value(&normalized.deck.slides[0].elements[..2]).unwrap(), serde_json::to_value(&baseline.deck.slides[0].elements[..2]).unwrap());
    issue23_set_frame(&mut package, "ppt/slides/slide1.xml", "full", [-2 * 9525, 0, 1280 * 9525, 720 * 9525]);
    let bytes = package.save().unwrap(); let opened = document::open_presentation("issue23-group".into(), bytes.clone()).unwrap();
    let document: Document = serde_json::from_value(opened["document"].clone()).unwrap();
    assert!(!document.deck.slides[0].elements.iter().any(|element| element.bounds().0 == "full"));
    assert!(opened["warnings"].as_array().unwrap().iter().filter_map(|warning| warning.as_str()).any(|warning| warning.contains("element full") && warning.contains("left=2.000000px") && warning.contains("retained without preview")));
    assert_eq!(save(&document), bytes);
}

#[test]
fn issue23_large_flat_rect_is_clipped_locked_and_only_explicit_unlock_materializes_it() {
    let mut package = issue23_package(vec![issue23_rect("background", 0.0, 40.0, 200.0, 120.0)], (1280, 720));
    issue23_set_frame(&mut package, "ppt/slides/slide1.xml", "background", [-16 * 9525, 40 * 9525, 200 * 9525, 120 * 9525]);
    let bytes = package.save().unwrap(); let original = open(bytes.clone());
    let element = original.deck.slides[0].elements.iter().find(|element| element.bounds().0 == "background").expect("flat native rectangle should have a clipped read-only projection");
    assert_eq!(element.bounds(), ("background", 0.0, 40.0, 184.0, 120.0));
    assert!(element.visual().unwrap().locked);
    let rendered = aislide_core::export_static::export_static(&original.deck, &Default::default()).unwrap();
    let pixels = image::load_from_memory(&rendered.artifacts[0].bytes).unwrap().into_rgba8();
    assert_eq!(pixels.get_pixel(0, 60).0, [17, 34, 51, 255]);
    assert_eq!(pixels.get_pixel(183, 60).0, [17, 34, 51, 255]);
    assert_eq!(pixels.get_pixel(184, 60).0, [255, 255, 255, 255]);
    assert_eq!(save(&original), bytes);
    let mut edited = original.deck.clone();
    if let aislide_core::model::Element::Rect { fill, .. } = &mut edited.slides[0].elements[0] { *fill = "AABBCC".into(); }
    let transaction = Transaction { expected_revision: original.revision, expected_hash: original.hash.clone(), operations: serde_json::from_value(json!([{"op":"replace","path":"/deck","value":edited}])).unwrap() };
    assert!(document::transact(&original, transaction).err().expect("locked clipped rectangle edits must reject").to_string().contains("locked"));
    let mut unrelated = original.deck.clone(); unrelated.title = "Unrelated edit".into();
    let output = Package::open(save(&change(&original, unrelated).document)).unwrap();
    assert_eq!(output.part("ppt/slides/slide1.xml").unwrap(), package.part("ppt/slides/slide1.xml").unwrap());
    let mut unlocked = original.deck.clone(); *unlocked.slides[0].elements[0].visual_mut().unwrap() = None;
    let changed = change(&original, unlocked);
    let reopened = open(save(&changed.document));
    assert!(!reopened.deck.slides[0].elements[0].visual().is_some_and(|style| style.locked));
    assert_eq!(reopened.deck.slides[0].elements[0].bounds(), ("background", 0.0, 40.0, 184.0, 120.0));
    let restored = document::undo(&changed.document, changed.document.revision, changed.receipt.unwrap()).unwrap();
    assert_eq!(save(&restored.document), bytes);
}

#[test]
fn issue23_png_and_jpeg_clip_composes_existing_crop_without_pixel_shift_or_stretch() {
    use aislide_core::model::{Crop, Element};
    let image = image::RgbImage::from_fn(128, 128, |column, row| image::Rgb([(column * 2) as u8, (row * 2) as u8, if (column / 8 + row / 8) % 2 == 0 { 0 } else { 240 }]));
    for (format, mime_type) in [(image::ImageFormat::Png, "image/png"), (image::ImageFormat::Jpeg, "image/jpeg")] {
        let mut encoded = std::io::Cursor::new(Vec::new()); image.write_to(&mut encoded, format).unwrap();
        let image_bytes = encoded.into_inner();
        let picture = Element::Picture { visual: None, svg: None, id: "background".into(), x: 0.0, y: 0.0, width: 256.0, height: 256.0, base64: STANDARD.encode(&image_bytes), mime_type: mime_type.into(), alt: "Synthetic coordinate raster".into(), crop: Crop { left: 0.125, top: 0.125, right: 0.125, bottom: 0.125 } };
        let mut reference = source().deck; reference.design = None; reference.width = 384; reference.height = 384;
        reference.slides[0].layout_id = None; reference.slides[0].inherit_background = false; reference.slides[0].background = "FFFFFF".into();
        let mut reference_picture = picture.clone();
        if let Element::Picture { width, height, .. } = &mut reference_picture { *width = 384.0; *height = 384.0; }
        reference.slides[0].elements = vec![reference_picture];
        let rendered = aislide_core::export_static::export_static(&reference, &Default::default()).unwrap();
        let reference_pixels = image::load_from_memory(&rendered.artifacts[0].bytes).unwrap().into_rgba8();
        for (offset_x, offset_y) in [(-64, -32), (0, -32), (-32, -64), (-32, 0), (-32, -32)] {
            let mut package = issue23_package(vec![picture.clone()], (320, 320));
            issue23_set_frame(&mut package, "ppt/slides/slide1.xml", "background", [offset_x * 9525, offset_y * 9525, 384 * 9525, 384 * 9525]);
            let bytes = package.save().unwrap(); let original = open(bytes.clone());
            let before = Package::open(bytes.clone()).unwrap();
            let element = original.deck.slides[0].elements.first().expect("native raster must have a clipped read-only projection");
            assert!(element.visual().unwrap().locked);
            if let Element::Picture { base64, crop, .. } = element {
                assert_eq!(STANDARD.decode(base64).unwrap(), image_bytes);
                assert!((crop.left - (0.125 + 0.75 * (-offset_x as f64 / 384.0))).abs() < 1e-10);
                assert!((crop.top - (0.125 + 0.75 * (-offset_y as f64 / 384.0))).abs() < 1e-10);
            } else { panic!("raster projection must stay a picture"); }
            let rendered = aislide_core::export_static::export_static(&original.deck, &Default::default()).unwrap();
            let actual = image::load_from_memory(&rendered.artifacts[0].bytes).unwrap().into_rgba8();
            let expected = image::imageops::crop_imm(&reference_pixels, (-offset_x) as u32, (-offset_y) as u32, 320, 320).to_image();
            assert!(actual.as_raw() == expected.as_raw(), "decoded pixels shifted or stretched: {mime_type}, offset ({offset_x},{offset_y})");
            assert_eq!(save(&original), bytes);
            let mut unrelated = original.deck.clone(); unrelated.title = "Unrelated raster edit".into();
            let changed = change(&original, unrelated);
            let output = Package::open(save(&changed.document)).unwrap();
            for (path, data) in before.parts().iter().filter(|(path, _)| path.starts_with("ppt/slides/") || path.starts_with("ppt/media/")) { assert!(output.part(path).unwrap() == data, "native raster XML or media changed: {path}"); }
            let restored = document::undo(&changed.document, changed.document.revision, changed.receipt.unwrap()).unwrap();
            assert_eq!(save(&restored.document), bytes);
            let mut edited = original.deck.clone();
            if let Element::Picture { alt, .. } = &mut edited.slides[0].elements[0] { *alt = "Implicit edit must reject".into(); }
            let transaction = Transaction { expected_revision: 0, expected_hash: original.hash.clone(), operations: serde_json::from_value(json!([{"op":"replace","path":"/deck","value":edited}])).unwrap() };
            assert!(document::transact(&original, transaction).err().expect("locked raster edits must reject").to_string().contains("locked"));
            if offset_x == -64 {
                let mut unlocked = original.deck.clone(); *unlocked.slides[0].elements[0].visual_mut().unwrap() = None;
                let changed = change(&original, unlocked); let reopened = open(save(&changed.document));
                assert!(!reopened.deck.slides[0].elements[0].visual().is_some_and(|style| style.locked));
                let rendered = aislide_core::export_static::export_static(&reopened.deck, &Default::default()).unwrap();
                assert!(image::load_from_memory(&rendered.artifacts[0].bytes).unwrap().into_rgba8().as_raw() == expected.as_raw(), "explicit raster unlock changed decoded pixels");
                let restored = document::undo(&changed.document, changed.document.revision, changed.receipt.unwrap()).unwrap();
                assert_eq!(save(&restored.document), bytes);
            }
        }
    }
}

#[test]
fn issue23_malformed_native_emu_frames_are_not_repaired_into_valid_elements() {
    for (attribute, old, value) in [("cx", "1524000", "0"), ("cx", "1524000", "-1"), ("cx", "1524000", "NaN"), ("cy", "762000", "inf"),
        ("x", "-1", "NaN"), ("x", "-1", "0.5"), ("y", "1524000", "not-a-coordinate"), ("cx", "1524000", "9223372036854775808")] {
        let mut package = Package::open(issue23_native_rects()).unwrap();
        issue23_edit_shape(&mut package, "ppt/slides/slide1.xml", "edge-left", |fragment| {
            let token = format!("{attribute}=\"{old}\""); assert!(fragment.contains(&token));
            fragment.replacen(&token, &format!("{attribute}=\"{value}\""), 1)
        });
        let bytes = package.save().unwrap();
        let opened = document::open_presentation("issue23-malformed".into(), bytes.clone()).unwrap();
        let original: Document = serde_json::from_value(opened["document"].clone()).unwrap();
        assert!(!original.deck.slides[0].elements.iter().any(|element| element.bounds().0 == "edge-left"), "{attribute}={value}");
        assert!(original.deck.slides[0].elements.iter().any(|element| element.bounds().0 == "control"));
        assert!(opened["warnings"].as_array().unwrap().iter().filter_map(|warning| warning.as_str()).any(|warning| warning.contains("slide ") && warning.contains("ppt/slides/slide1.xml") && warning.contains("element edge-left") && warning.contains("native frame")));
        assert_eq!(save(&original), bytes);
    }
}

#[test]
fn issue23_one_pixel_is_read_only_and_unsupported_clip_has_quantified_warning() {
    let mut package = issue23_package(vec![issue23_rect("threshold", 0.0, 40.0, 200.0, 120.0)], (1280, 720));
    issue23_set_frame(&mut package, "ppt/slides/slide1.xml", "threshold", [-9525, 40 * 9525, 200 * 9525, 120 * 9525]);
    let opened = document::open_presentation("issue23-threshold".into(), package.save().unwrap()).unwrap();
    let document: Document = serde_json::from_value(opened["document"].clone()).unwrap();
    assert!(document.deck.slides[0].elements[0].visual().unwrap().locked);
    assert!(opened["warnings"].as_array().unwrap().iter().filter_map(|warning| warning.as_str()).any(|warning| warning.contains("clipped read-only") && warning.contains("left=1.000000px")));
    for (label, edit) in [("rotation", "<a:xfrm rot=\"60000\">"), ("flip", "<a:xfrm flipH=\"1\">"), ("opaque-effect", "<a:xfrm>")] {
        let mut package = issue23_package(vec![issue23_rect("candidate", 0.0, 40.0, 200.0, 120.0)], (1280, 720));
        issue23_set_frame(&mut package, "ppt/slides/slide1.xml", "candidate", [-16 * 9525, 40 * 9525, 200 * 9525, 120 * 9525]);
        issue23_edit_shape(&mut package, "ppt/slides/slide1.xml", "candidate", |fragment| {
            let value = fragment.replacen("<a:xfrm>", edit, 1);
            if label == "opaque-effect" { value.replace("</p:spPr>", "<a:effectDag/></p:spPr>") } else { value }
        });
        let bytes = package.save().unwrap(); let opened = document::open_presentation("issue23-unsupported".into(), bytes.clone()).unwrap();
        let document: Document = serde_json::from_value(opened["document"].clone()).unwrap();
        assert!(document.deck.slides[0].elements.is_empty(), "{label}");
        assert!(opened["warnings"].as_array().unwrap().iter().filter_map(|warning| warning.as_str()).any(|warning| warning.contains("element candidate") && warning.contains("left=16.000000px") && warning.contains("retained without preview")), "{label}");
        assert_eq!(save(&document), bytes);
    }
    let table = serde_json::from_value(json!({"type":"table","id":"candidate","x":0,"y":40,"width":200,"height":120,"rows":[["A","B"],["C","D"]],"font_size":16})).unwrap();
    let mut package = issue23_package(vec![table], (1280, 720));
    let path = "ppt/slides/slide1.xml";
    let xml = package.text(path).unwrap().replacen("<p:xfrm>", "<p:xfrm rot=\"60000\">", 1).replacen("<a:off x=\"0\" y=\"381000\"/>", "<a:off x=\"-152400\" y=\"381000\"/>", 1);
    package.replace_part(path, xml.into_bytes()).unwrap();
    let bytes = package.save().unwrap(); let opened = document::open_presentation("issue23-native-frame".into(), bytes.clone()).unwrap();
    assert!(opened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().is_empty());
    assert!(opened["warnings"].as_array().unwrap().iter().filter_map(|warning| warning.as_str()).any(|warning| warning.contains("element candidate") && warning.contains("left=16.000000px") && warning.contains("rotated native frame")));
    assert_eq!(save(&serde_json::from_value(opened["document"].clone()).unwrap()), bytes);
}

#[test]
fn issue23_subpixel_repair_does_not_relax_preset_font_text_path_or_visual_guards() {
    use aislide_core::model::Element;
    for label in ["preset", "font-family", "font-size", "text", "path", "opacity"] {
        let candidate = match label {
            "font-family" | "font-size" | "text" => serde_json::from_value(json!({"type":"text","id":"candidate","x":0,"y":160,"width":160,"height":80,"text":"TINY","font_size":24,"color":"202525","bold":false,"format":{"font_family":"Aptos"}})).unwrap(),
            "path" => Element::Polygon { visual: None, id: "candidate".into(), x: 0.0, y: 160.0, width: 160.0, height: 80.0, points: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], fill: "112233".into(), stroke: "202525".into(), stroke_width: 0.0 },
            "opacity" => { let mut element = issue23_rect("candidate", 0.0, 160.0, 160.0, 80.0); *element.visual_mut().unwrap() = Some(aislide_core::visual::VisualStyle { opacity: Some(0.5), ..Default::default() }); element },
            _ => issue23_rect("candidate", 0.0, 160.0, 160.0, 80.0),
        };
        let mut package = issue23_package(vec![candidate, issue23_rect("control", 400.0, 40.0, 100.0, 80.0)], (1280, 720));
        issue23_set_frame(&mut package, "ppt/slides/slide1.xml", "candidate", [-1, 160 * 9525, 160 * 9525, 80 * 9525]);
        issue23_edit_shape(&mut package, "ppt/slides/slide1.xml", "candidate", |fragment| {
            let value = match label {
                "preset" => fragment.replace("prst=\"rect\"", "prst=\"not-supported\""),
                "font-family" => fragment.replace("typeface=\"Aptos\"", "typeface=\"\""),
                "font-size" => fragment.replace("sz=\"1800\"", "sz=\"999999\""),
                "text" => fragment.replace("TINY", &"X".repeat(4001)),
                "path" => fragment.replacen("<a:pt x=\"0\" y=\"0\"/>", "<a:pt x=\"999999999999\" y=\"0\"/>", 1),
                _ => fragment.replace("<a:alpha val=\"50000\"/>", "<a:alpha val=\"750000\"/>"),
            };
            assert_ne!(value, fragment, "invalid native fixture {label}"); value
        });
        let bytes = package.save().unwrap(); let opened = document::open_presentation("issue23-invalid-content".into(), bytes.clone()).unwrap();
        let original: Document = serde_json::from_value(opened["document"].clone()).unwrap();
        assert_eq!(original.deck.slides[0].elements.len(), 1, "{label}");
        assert_eq!(original.deck.slides[0].elements[0].bounds().0, "control");
        assert!(!opened["warnings"].as_array().unwrap().iter().filter_map(|warning| warning.as_str()).any(|warning| warning.contains("element candidate") && warning.contains("normalized in disposable projection")), "rejected {label} must not be advertised as a normalized projection");
        assert_eq!(save(&original), bytes);
    }
}

#[test]
fn issue23_hidden_locks_and_raster_asset_guards_survive_projection() {
    use aislide_core::model::Element;
    let mut candidate = issue23_rect("candidate", 0.0, 160.0, 160.0, 80.0);
    *candidate.visual_mut().unwrap() = Some(aislide_core::visual::VisualStyle { hidden: true, locked: true, ..Default::default() });
    let mut package = issue23_package(vec![candidate], (1280, 720));
    issue23_set_frame(&mut package, "ppt/slides/slide1.xml", "candidate", [-1, 160 * 9525, 160 * 9525, 80 * 9525]);
    let bytes = package.save().unwrap(); let original = open(bytes.clone());
    let style = original.deck.slides[0].elements[0].visual().unwrap(); assert!(style.hidden && style.locked);
    assert!(aislide_core::execute_request(json!({"op":"edit_selection","document":original,"expected_revision":0,"slide_id":original.deck.slides[0].id,"operation":{"op":"translate","ids":["candidate"],"dx":1,"dy":0}})).is_err());
    assert_eq!(save(&original), bytes);
    let mut encoded = std::io::Cursor::new(Vec::new()); image::RgbImage::new(2, 2).write_to(&mut encoded, image::ImageFormat::Png).unwrap();
    let picture: Element = serde_json::from_value(json!({"type":"picture","id":"candidate","x":0,"y":160,"width":160,"height":80,"base64":STANDARD.encode(encoded.into_inner()),"mime_type":"image/png","alt":"Synthetic"})).unwrap();
    for label in ["truncated", "external", "crop"] {
        let mut package = issue23_package(vec![picture.clone(), issue23_rect("control", 400.0, 40.0, 100.0, 80.0)], (1280, 720));
        issue23_set_frame(&mut package, "ppt/slides/slide1.xml", "candidate", [-1, 160 * 9525, 160 * 9525, 80 * 9525]);
        match label {
            "truncated" => { let path = package.parts().keys().find(|path| path.starts_with("ppt/media/")).unwrap().clone(); package.replace_part(&path, package.part(&path).unwrap()[..24].to_vec()).unwrap(); },
            "external" => { let path = "ppt/slides/_rels/slide1.xml.rels"; let xml = package.text(path).unwrap().replace("Target=\"../media/image1.png\"", "Target=\"https://invalid.example/never-fetch.png\" TargetMode=\"External\""); assert!(xml.contains("TargetMode=\"External\"")); package.replace_part(path, xml.into_bytes()).unwrap(); },
            _ => issue23_edit_shape(&mut package, "ppt/slides/slide1.xml", "candidate", |fragment| {
                let value = if fragment.contains("<a:srcRect ") { fragment.replace("<a:srcRect l=\"0\"", "<a:srcRect l=\"100000\"") }
                    else { fragment.replace("<a:stretch>", "<a:srcRect l=\"100000\"/><a:stretch>") };
                assert_ne!(value, fragment, "invalid native crop fixture must change XML"); value
            }),
        }
        let bytes = package.save().unwrap(); let opened = document::open_presentation("issue23-raster-guard".into(), bytes.clone()).unwrap();
        let original: Document = serde_json::from_value(opened["document"].clone()).unwrap();
        assert!(!original.deck.slides[0].elements.iter().any(|element| matches!(element, Element::Picture { .. })), "{label}");
        assert_eq!(save(&original), bytes);
    }
}

#[test]
fn issue23_native_projection_keeps_element_budget_and_signed_package_guards() {
    let elements = (0..256).map(|index| issue23_rect(&format!("object-{index}"), 0.0, 40.0, 20.0, 20.0)).collect();
    let mut package = issue23_package(elements, (1280, 720));
    let path = "ppt/slides/slide1.xml"; let xml = package.text(path).unwrap().to_owned();
    let extra = xml[issue23_shape_range(&xml, "object-0")].replace("name=\"object-0\"", "name=\"object-256\"").replace("<p:cNvPr id=\"2\"", "<p:cNvPr id=\"99999\"").replace("<a:off x=\"0\"", "<a:off x=\"-1\"");
    assert!(extra.contains("id=\"99999\""));
    package.replace_part(path, xml.replace("</p:spTree>", &format!("{extra}</p:spTree>")).into_bytes()).unwrap();
    let error = document::open_presentation("issue23-budget".into(), package.save().unwrap()).err().expect("257 native objects must remain over budget").to_string();
    assert!(error.contains("too many") || error.contains("limit"), "{error}");
    let mut parts = Package::open(issue23_native_rects()).unwrap().parts().clone();
    parts.insert("_xmlsignatures/sig1.xml".into(), b"<signature/>".to_vec());
    let bytes = Package::from_parts(parts).unwrap().save().unwrap(); let original = open(bytes.clone());
    assert_eq!(save(&original), bytes);
    let mut deck = original.deck.clone(); deck.title = "Signed edit must reject".into();
    let transaction = Transaction { expected_revision: 0, expected_hash: original.hash.clone(), operations: serde_json::from_value(json!([{"op":"replace","path":"/deck","value":deck}])).unwrap() };
    assert!(document::transact(&original, transaction).err().expect("signed package edit must reject").to_string().contains("signed"));
    assert_eq!(save(&original), bytes);
}

#[test]
fn native_topology_add_move_remove_and_undo_retains_unknown_parts() {
    let mut parts = Package::open(save(&source())).unwrap().parts().clone();
    parts.insert("vendor/opaque.bin".into(), vec![1, 2, 3, 4]);
    let mut package = Package::from_parts(parts).unwrap();
    let path = "ppt/slideMasters/slideMaster1.xml";
    let sentinel = "<p:extLst><p:ext uri=\"urn:vendor:retained\"><v:data xmlns:v=\"urn:vendor\" value=\"opaque\"/></p:ext></p:extLst>";
    let xml = package.text(path).unwrap().replace("</p:sldMaster>", &format!("{sentinel}</p:sldMaster>"));
    package.replace_part(path, xml.into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let original = open(bytes.clone());
    assert_eq!(save(&original), bytes);
    let mut deck = original.deck.clone();
    let design = deck.design.as_mut().unwrap();
    design.masters.push(Master { id: "brand-master".into(), name: "Brand master".into(), background: "@accent1".into(), elements: vec![], theme: None });
    design.layouts.push(SlideLayout { id: "brand-layout".into(), name: "Brand layout".into(), master_id: "brand-master".into(), background: None, elements: vec![] });
    deck.slides[0].layout_id = Some("brand-layout".into());
    let changed = change(&original, deck);
    let output = save(&changed.document);
    let package = Package::open(output.clone()).unwrap();
    assert_eq!(package.part("vendor/opaque.bin").unwrap(), &[1, 2, 3, 4]);
    assert!(package.text(path).unwrap().contains(sentinel));
    let mut numeric = BTreeSet::new();
    for (path, bytes) in package.parts().iter().filter(|(path, _)| path.ends_with(".xml") && (path.contains("slideMasters/") || path.as_str() == "ppt/presentation.xml")) {
        let xml = roxmltree::Document::parse(std::str::from_utf8(bytes).unwrap()).unwrap();
        for node in xml.descendants().filter(|node| ["sldMasterId", "sldLayoutId"].contains(&node.tag_name().name())) {
            assert!(numeric.insert(node.attribute("id").unwrap().to_owned()), "numeric ID collision: {path}");
        }
    }
    let reopened = open(output);
    assert_eq!(reopened.deck.slides[0].layout_id.as_deref(), Some("brand-layout"));
    assert_eq!(reopened.deck.design.as_ref().unwrap().masters.len(), 2);
    let mut moved = reopened.deck.clone();
    let design = moved.design.as_mut().unwrap();
    design.layouts[1].master_id = "brand-master".into();
    design.masters.reverse();
    let moved = open(save(&change(&reopened, moved).document));
    assert_eq!(moved.deck.design.as_ref().unwrap().layouts[1].master_id, "brand-master");
    let mut removed = moved.deck.clone();
    removed.slides[0].layout_id = Some("blank".into());
    let design = removed.design.as_mut().unwrap();
    design.layouts.retain(|layout| layout.id != "brand-layout");
    for layout in &mut design.layouts { layout.master_id = "master-1".into(); }
    design.masters.retain(|master| master.id == "master-1");
    let removed = open(save(&change(&moved, removed).document));
    assert_eq!(removed.deck.design.as_ref().unwrap().masters.len(), 1);
    assert_eq!(removed.deck.design.as_ref().unwrap().layouts.len(), 4);
    let restored = document::undo(&changed.document, changed.document.revision, changed.receipt.unwrap()).unwrap();
    assert_eq!(save(&restored.document), bytes);
}

#[test]
fn deleting_a_referenced_layout_or_master_is_rejected() {
    let document = open(save(&source()));
    let mut deck = document.deck.clone();
    deck.slides[0].layout_id = Some("blank".into());
    deck.design.as_mut().unwrap().layouts.retain(|layout| layout.id != "blank");
    assert!(aislide_core::model::validate_deck(&deck).is_err());
    let mut deck = document.deck.clone();
    deck.design.as_mut().unwrap().masters.clear();
    assert!(aislide_core::model::validate_deck(&deck).is_err());
}

#[test]
fn simultaneous_native_master_layout_and_slide_addition_writes_real_theme_and_art() {
    let original = open(save(&source()));
    let mut deck = original.deck.clone();
    let design = deck.design.as_mut().unwrap();
    design.theme.colors.insert("accent2".into(), "FEDCBA".into());
    design.masters.push(Master { id: "new-master".into(), name: "New & master".into(), background: "@lt1".into(), elements: vec![aislide_core::model::Element::Rect { visual: None, id: "brand-band".into(), x: 0.0, y: 0.0, width: 1280.0, height: 20.0, fill: "@accent2".into() }], theme: None });
    design.layouts.push(SlideLayout { id: "new-layout".into(), name: "New layout".into(), master_id: "new-master".into(), background: None, elements: vec![] });
    let mut slide = deck.slides[0].clone(); slide.id = "new-slide".into(); slide.layout_id = Some("new-layout".into()); slide.notes = "New speaker notes".into(); deck.slides.push(slide);
    let output = save(&change(&original, deck).document);
    let package = Package::open(output.clone()).unwrap();
    let theme = package.parts().keys().find(|path| path.starts_with("ppt/theme/aislide-theme-")).unwrap();
    assert!(package.text(theme).unwrap().contains("FEDCBA"));
    assert_ne!(theme, "ppt/theme/theme2.xml");
    assert!(package.text("ppt/notesMasters/_rels/notesMaster1.xml.rels").unwrap().contains("../theme/theme2.xml"));
    let reopened = open(output);
    assert_eq!(reopened.deck.slides[1].notes, "New speaker notes");
    assert_eq!(reopened.deck.slides[1].layout_id.as_deref(), Some("new-layout"));
    let master = reopened.deck.design.as_ref().unwrap().masters.iter().find(|master| master.id == "new-master").unwrap();
    assert_eq!(master.elements[0].bounds().0, "brand-band");
}

#[test]
fn different_native_themes_are_retained_and_default_theme_edits_are_owner_scoped() {
    let mut original = source();
    let design = original.deck.design.as_mut().unwrap();
    design.masters.push(Master { id: "second".into(), name: "Second".into(), background: "@lt1".into(), elements: vec![], theme: None });
    design.layouts.push(SlideLayout { id: "second-layout".into(), name: "Second layout".into(), master_id: "second".into(), background: None, elements: vec![] });
    let original = document::create("topology".into(), original.deck, vec![], vec![], None).unwrap();
    let mut package = Package::open(save(&original)).unwrap();
    let xml = package.text("ppt/theme/theme3.xml").unwrap().replace("087F73", "ABC123");
    package.replace_part("ppt/theme/theme3.xml", xml.clone().into_bytes()).unwrap();
    let document = open(package.save().unwrap());
    assert_eq!(serde_json::to_value(&document.deck.design).unwrap()["masters"][1]["theme"]["colors"]["accent1"], "ABC123");
    let mut deck = document.deck.clone(); deck.title = "Renamed".into();
    let package = Package::open(save(&change(&document, deck).document)).unwrap();
    assert_eq!(package.text("ppt/theme/theme3.xml").unwrap(), xml);
    let mut deck = document.deck.clone(); deck.design.as_mut().unwrap().theme.colors.insert("accent1".into(), "334455".into());
    let result = document::transact(&document, Transaction { expected_revision: 0, expected_hash: document.hash.clone(), operations: serde_json::from_value(json!([{"op":"replace","path":"/deck","value":deck}])).unwrap() });
    let updated = Package::open(save(&result.unwrap().document)).unwrap();
    assert_eq!(updated.text("ppt/theme/theme3.xml").unwrap(), xml);
    assert!(updated.text("ppt/theme/theme1.xml").unwrap().contains("334455"));
}

#[test]
fn unknown_relationship_to_a_removed_layout_blocks_deletion() {
    let mut package = Package::open(save(&source())).unwrap();
    let path = "ppt/slides/_rels/slide1.xml.rels";
    let xml = package.text(path).unwrap().replace("</Relationships>", "<Relationship Id=\"rIdVendor\" Type=\"urn:vendor:layout\" Target=\"../slideLayouts/slideLayout4.xml\"/></Relationships>");
    package.replace_part(path, xml.into_bytes()).unwrap();
    let document = open(package.save().unwrap());
    let mut deck = document.deck.clone(); deck.design.as_mut().unwrap().layouts.retain(|layout| layout.id != "section");
    let result = document::transact(&document, Transaction { expected_revision: 0, expected_hash: document.hash.clone(), operations: serde_json::from_value(json!([{"op":"replace","path":"/deck","value":deck}])).unwrap() });
    assert!(matches!(result, Err(aislide_core::Error::Unsupported(_))));
}

#[test]
fn retained_topology_rows_keep_unknown_attributes() {
    let mut package = Package::open(save(&source())).unwrap();
    for (path, tag) in [("ppt/presentation.xml", "sldMasterId"), ("ppt/slideMasters/slideMaster1.xml", "sldLayoutId")] {
        let xml = package.text(path).unwrap().replace(&format!("<p:{tag} id="), &format!("<p:{tag} xmlns:v=\"urn:vendor\" v:retain=\"yes\" id="));
        package.replace_part(path, xml.into_bytes()).unwrap();
    }
    let original = open(package.save().unwrap());
    let mut deck = original.deck.clone();
    let design = deck.design.as_mut().unwrap();
    design.masters.push(Master { id: "extra".into(), name: "Extra".into(), background: "@lt1".into(), elements: vec![], theme: None });
    design.layouts.push(SlideLayout { id: "extra-layout".into(), name: "Extra layout".into(), master_id: "extra".into(), background: None, elements: vec![] });
    design.layouts.push(SlideLayout { id: "main-extra-layout".into(), name: "Main extra layout".into(), master_id: "master-1".into(), background: None, elements: vec![] });
    let output = Package::open(save(&change(&original, deck).document)).unwrap();
    for (path, count) in [("ppt/presentation.xml", 1), ("ppt/slideMasters/slideMaster1.xml", 4)] {
        let parsed = roxmltree::Document::parse(output.text(path).unwrap()).unwrap();
        assert_eq!(parsed.descendants().filter(|node| node.attribute(("urn:vendor", "retain")) == Some("yes")).count(), count, "{path}");
    }
}

#[test]
fn layout_assignment_preserves_freeform_objects_with_template_ids() {
    use aislide_core::model::{Element, TextFormat};
    let mut deck = source().deck;
    let slide = &mut deck.slides[0];
    slide.elements.push(Element::Text { visual: None, id: "title".into(), x: 100.0, y: 400.0, width: 200.0, height: 40.0, text: "User content".into(), font_size: 24.0, color: "123456".into(), bold: false, format: TextFormat::default() });
    let slide_id = slide.id.clone();
    let assigned = aislide_core::design::assign_layout_with_options(deck.clone(), &slide_id, "title-content", true).unwrap();
    let retained = assigned.slides[0].elements.iter().find(|element| element.bounds().0 == "title").unwrap();
    assert_eq!(serde_json::to_value(retained).unwrap(), serde_json::to_value(&deck.slides[0].elements[0]).unwrap());
    assert_eq!(assigned.slides[0].elements.len(), 3);
    let repeated = aislide_core::design::assign_layout_with_options(assigned.clone(), &slide_id, "title-content", true).unwrap();
    assert_eq!(serde_json::to_value(repeated).unwrap(), serde_json::to_value(assigned).unwrap());
    deck.design = None; deck.width = 320; deck.height = 720;
    assert!(aislide_core::design::assign_layout(deck, &slide_id, "title-content").is_err());
}

#[test]
fn authored_master_themes_keep_distinct_fonts_and_owner_scoped_native_edits() {
    let mut deck = source().deck;
    let mut design = serde_json::to_value(deck.design.as_ref().unwrap()).unwrap();
    let mut theme = design["theme"].clone();
    theme["fonts"]["minor"] = json!("Courier New");
    theme["fonts"]["east_asian"] = json!("MS Gothic");
    design["masters"].as_array_mut().unwrap().push(json!({"id":"second","name":"Second","background":"@lt1","elements":[],"theme":theme}));
    design["layouts"].as_array_mut().unwrap().push(json!({"id":"second-layout","name":"Second layout","master_id":"second","elements":[]}));
    deck.design = Some(serde_json::from_value(design).unwrap());
    deck.slides[0].layout_id = Some("second-layout".into());
    let doc = document::create("authored-themes".into(), deck, vec![], vec![], None).unwrap();
    let bytes = save(&doc);
    let original = open(bytes.clone());
    assert_eq!(aislide_core::design::slide_theme(&original.deck.slides[0], original.deck.design.as_ref()).unwrap().fonts.minor, "Courier New");
    let mut updated = original.deck.clone();
    updated.design.as_mut().unwrap().masters[1].theme.as_mut().unwrap().fonts.minor = "Arial".into();
    let before = Package::open(bytes.clone()).unwrap();
    let output = save(&change(&original, updated).document);
    let after = Package::open(output.clone()).unwrap();
    for (path, data) in before.parts().iter().filter(|(path, _)| path.as_str() != "ppt/theme/theme3.xml" && !path.starts_with("customXml/")) {
        assert_eq!(after.part(path).unwrap(), data, "{path}");
    }
    let reopened = open(output);
    assert_eq!(aislide_core::design::slide_theme(&reopened.deck.slides[0], reopened.deck.design.as_ref()).unwrap().fonts.minor, "Arial");
    assert_eq!(save(&original), bytes);
}

#[test]
fn shared_native_theme_is_cloned_without_losing_unknown_xml_or_other_owner_bytes() {
    let mut deck = source().deck;
    let design = deck.design.as_mut().unwrap();
    design.masters.push(Master { id: "second".into(), name: "Second".into(), background: "@lt1".into(), elements: vec![], theme: None });
    design.layouts.push(SlideLayout { id: "second-layout".into(), name: "Second".into(), master_id: "second".into(), background: None, elements: vec![] });
    let mut package = Package::open(aislide_core::pptx::export_pptx(&deck).unwrap()).unwrap();
    let rel = "ppt/slideMasters/_rels/slideMaster2.xml.rels";
    package.replace_part(rel, package.text(rel).unwrap().replace("theme3.xml", "theme1.xml").into_bytes()).unwrap();
    let sentinel = "<a:extLst><a:ext uri=\"urn:vendor:theme\"><v:data xmlns:v=\"urn:vendor\" value=\"preserved\"/></a:ext></a:extLst>";
    package.replace_part("ppt/theme/theme1.xml", package.text("ppt/theme/theme1.xml").unwrap().replace("</a:theme>", &format!("{sentinel}</a:theme>")).into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let original = open(bytes.clone());
    let mut theme = original.deck.design.as_ref().unwrap().theme.clone(); theme.fonts.minor = "Courier New".into();
    let owner = &original.deck.design.as_ref().unwrap().masters[1].id;
    let changed = aislide_core::design::set_master_theme(original.deck.clone(), owner, Some(theme)).unwrap();
    let output = Package::open(save(&change(&original, changed).document)).unwrap();
    assert_eq!(output.part("ppt/theme/theme1.xml").unwrap(), package.part("ppt/theme/theme1.xml").unwrap());
    let clone = output.parts().keys().find(|path| path.starts_with("ppt/theme/aislide-theme-")).unwrap();
    assert!(output.text(clone).unwrap().contains(sentinel));
    assert!(output.text(clone).unwrap().contains("Courier New"));
    assert_eq!(output.part("ppt/slideMasters/_rels/slideMaster1.xml.rels").unwrap(), package.part("ppt/slideMasters/_rels/slideMaster1.xml.rels").unwrap());
    assert_eq!(save(&original), bytes);
}

#[test]
fn extra_same_owner_reference_keeps_original_theme_immutable() {
    let mut package = Package::open(save(&source())).unwrap();
    let path = "ppt/slideMasters/_rels/slideMaster1.xml.rels";
    package.replace_part(path, package.text(path).unwrap().replace("</Relationships>", "<Relationship Id=\"vendor-theme\" Type=\"urn:vendor\" Target=\"../theme/theme1.xml\"/></Relationships>").into_bytes()).unwrap();
    let original = open(package.save().unwrap());
    let mut theme = original.deck.design.as_ref().unwrap().theme.clone(); theme.fonts.minor = "Courier New".into();
    let deck = aislide_core::design::set_master_theme(original.deck.clone(), "master-1", Some(theme)).unwrap();
    let output = Package::open(save(&change(&original, deck).document)).unwrap();
    assert_eq!(output.part("ppt/theme/theme1.xml").unwrap(), package.part("ppt/theme/theme1.xml").unwrap());
    assert!(output.text(path).unwrap().contains("Id=\"vendor-theme\" Type=\"urn:vendor\" Target=\"../theme/theme1.xml\""));
}

#[test]
fn theme_attribute_edits_preserve_unknown_color_transforms_and_other_script_fonts() {
    let mut package = Package::open(save(&source())).unwrap();
    let path = "ppt/theme/theme1.xml";
    let sentinel = "<a:srgbClr val=\"087F73\"><a:alpha val=\"75000\"/></a:srgbClr>";
    let xml = package.text(path).unwrap().replace("<a:srgbClr val=\"087F73\"/>", sentinel).replacen("<a:ea typeface=\"Yu Gothic\"/>", "<a:ea typeface=\"MS Gothic\"/>", 1);
    assert!(xml.contains(sentinel));
    package.replace_part(path, xml.into_bytes()).unwrap();
    let original = open(package.save().unwrap());
    let mut theme = original.deck.design.as_ref().unwrap().theme.clone();
    theme.colors.insert("accent1".into(), "ABC123".into()); theme.fonts.minor = "Courier New".into();
    let deck = aislide_core::design::set_master_theme(original.deck.clone(), "master-1", Some(theme)).unwrap();
    let output = Package::open(save(&change(&original, deck).document)).unwrap();
    assert!(output.text(path).unwrap().contains(&sentinel.replace("087F73", "ABC123")));
    assert!(output.text(path).unwrap().contains("<a:ea typeface=\"MS Gothic\"/>"));
}

#[test]
fn default_theme_application_does_not_rebind_explicit_master_content() {
    let mut deck = source().deck;
    let design = deck.design.as_mut().unwrap();
    let theme = design.theme.clone();
    design.masters[0].theme = Some(theme.clone());
    deck.slides[0].elements = vec![serde_json::from_value(json!({"type":"text","id":"explicit","x":40,"y":40,"width":600,"height":80,"text":"Retain literal styling","font_size":24,"color":"202525","bold":false})).unwrap()];
    let before = serde_json::to_value(&deck.slides).unwrap();
    let mut replacement = theme; replacement.colors.insert("dk1".into(), "ABCDEF".into());
    let updated = aislide_core::design::apply_theme(deck, replacement).unwrap();
    assert_eq!(serde_json::to_value(&updated.slides).unwrap(), before);
}

#[test]
fn native_field_instances_keep_inheritance_and_local_geometry_controls() {
    use aislide_core::{fields::{DesignField, DesignFieldKind}, model::Element};
    let mut deck = source().deck;
    for index in 2..=4 {
        let mut slide = deck.slides[0].clone();
        slide.id = format!("slide-{index}");
        deck.slides.push(slide);
    }
    let mut deck = aislide_core::fields::set_design_field(deck, DesignField { master_id: "master-1".into(), layout_id: Some("blank".into()), kind: DesignFieldKind::SlideNumber, reference_date: "2026-09-18".into(), text: String::new() }).unwrap();
    if let Element::Text { x, format, .. } = &mut deck.slides[2].elements[0] { *x = 700.0; format.inherit_layout = false; }
    if let Element::Text { font_size, format, .. } = &mut deck.slides[3].elements[0] { *font_size = 28.0; format.inherit_layout = false; }
    let mut package = Package::open(aislide_core::pptx::export_pptx(&deck).unwrap()).unwrap();
    for path in ["ppt/slides/slide1.xml", "ppt/slides/slide2.xml"] {
        let xml = package.text(path).unwrap().replace("<a:p>", "<a:p xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\" a14:paraId=\"00000042\">")
            .replace("</a:fld>", "<a:extLst><a:ext uri=\"urn:field-instance\"><v:keep xmlns:v=\"urn:vendor\"/></a:ext></a:extLst></a:fld>");
        package.replace_part(path, xml.into_bytes()).unwrap();
    }
    let original = open(package.save().unwrap());
    for slide in &original.deck.slides[..2] {
        assert!(matches!(&slide.elements[0], Element::Text { format, .. } if format.inherit_layout));
    }
    assert!(matches!(&original.deck.slides[3].elements[0], Element::Text { format, .. } if !format.inherit_layout));
    let before: Vec<_> = original.deck.slides.iter().map(|slide| match &slide.elements[0] { Element::Text { format, .. } => format.paragraphs.clone(), _ => panic!() }).collect();
    let mut updated = original.deck.clone();
    let layout_id = updated.slides[0].layout_id.clone().unwrap();
    let mut design = updated.design.clone().unwrap();
    let layout = design.layouts.iter_mut().find(|layout| layout.id == layout_id).unwrap();
    if let Element::Text { x, .. } = &mut layout.elements[0] { *x = 900.0; }
    updated = aislide_core::design::update_design(updated, design).unwrap();
    let changed = change(&original, updated);
    let output = save(&changed.document);
    let saved = Package::open(output.clone()).unwrap();
    for path in ["ppt/slides/slide1.xml", "ppt/slides/slide2.xml"] { assert_eq!(saved.part(path).unwrap(), package.part(path).unwrap()); }
    let reopened = open(output);
    for (index, slide) in reopened.deck.slides.iter().enumerate() {
        assert_eq!(slide.elements[0].bounds().1, if index == 2 { 700.0 } else if index == 3 { original.deck.slides[3].elements[0].bounds().1 } else { 900.0 });
        if let Element::Text { format, .. } = &slide.elements[0] { assert_eq!(format.paragraphs, before[index]); }
    }
    assert_ne!(before[0][0].runs[0].field, before[1][0].runs[0].field);
    let restored = document::undo(&changed.document, changed.document.revision, changed.receipt.unwrap()).unwrap();
    assert_eq!(save(&restored.document), save(&original));
}

#[test]
fn auxiliary_notes_and_handout_masters_are_native_independent_and_editable() {
    let original = open(save(&source()));
    let mut value = serde_json::to_value(&original.deck).unwrap();
    let master = json!({"name":"Auxiliary page","background":"@lt1","theme":value["design"]["theme"],"elements":[{"type":"text","id":"aux-title","x":50,"y":60,"width":500,"height":50,"text":"Notes & handouts","font_size":24,"color":"@accent1","bold":false}]});
    value["auxiliary_design"] = json!({"width":720,"height":960,"notes_master":master,"handout_master":master});
    let changed = change(&original, serde_json::from_value(value).unwrap());
    let output = save(&changed.document);
    let package = Package::open(output.clone()).unwrap();
    assert!(package.text("ppt/presentation.xml").unwrap().contains("handoutMasterIdLst"));
    assert!(package.text("[Content_Types].xml").unwrap().contains("handoutMaster+xml"));
    let reopened = open(output);
    let mut value = serde_json::to_value(&reopened.deck).unwrap();
    assert_eq!(value["auxiliary_design"]["notes_master"]["elements"][0]["text"], "Notes & handouts");
    assert_eq!(value["auxiliary_design"]["handout_master"]["elements"][0]["text"], "Notes & handouts");
    assert_eq!(value["design"]["masters"].as_array().unwrap().len(), 1);
    value["auxiliary_design"]["handout_master"]["elements"][0]["x"] = json!(80);
    value["auxiliary_design"]["handout_master"]["theme"]["colors"]["accent1"] = json!("ABCDEF");
    let moved = open(save(&change(&reopened, serde_json::from_value(value).unwrap()).document));
    let moved = serde_json::to_value(&moved.deck).unwrap();
    assert_eq!(moved["auxiliary_design"]["handout_master"]["elements"][0]["x"], 80.0);
    assert_eq!(moved["auxiliary_design"]["notes_master"]["elements"][0]["x"], 50.0);
    assert_eq!(moved["auxiliary_design"]["handout_master"]["theme"]["colors"]["accent1"], "ABCDEF");
    let restored = document::undo(&changed.document, changed.document.revision, changed.receipt.unwrap()).unwrap();
    assert_eq!(save(&restored.document), save(&original));
}

#[test]
fn auxiliary_native_edits_retain_notes_styles_extensions_and_shared_themes() {
    let mut package = Package::open(save(&source())).unwrap();
    let path = "ppt/notesMasters/notesMaster1.xml";
    let sentinel = "<p:notesStyle><a:lvl1pPr marL=\"42\"><a:defRPr sz=\"1300\"/></a:lvl1pPr></p:notesStyle><p:extLst><p:ext uri=\"urn:auxiliary-test\"><v:data xmlns:v=\"urn:vendor\"/></p:ext></p:extLst>";
    package.replace_part(path, package.text(path).unwrap().replace("</p:notesMaster>", &format!("{sentinel}</p:notesMaster>")).into_bytes()).unwrap();
    let rels = "ppt/notesMasters/_rels/notesMaster1.xml.rels";
    package.replace_part(rels, package.text(rels).unwrap().replace("theme2.xml", "theme1.xml").replace("</Relationships>", "<Relationship Id=\"vendor\" Type=\"urn:vendor\" Target=\"../theme/theme1.xml\"/></Relationships>").into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let original = open(bytes.clone());
    assert_eq!(save(&original), bytes);
    let mut value = serde_json::to_value(&original.deck).unwrap();
    value["auxiliary_design"]["notes_master"]["name"] = json!("Edited notes master");
    value["auxiliary_design"]["notes_master"]["theme"]["colors"]["accent1"] = json!("ABCDEF");
    let changed = change(&original, serde_json::from_value(value).unwrap());
    let updated = Package::open(save(&changed.document)).unwrap();
    assert!(updated.text(path).unwrap().contains(sentinel));
    assert_eq!(updated.part("ppt/theme/theme1.xml").unwrap(), package.part("ppt/theme/theme1.xml").unwrap());
    assert!(updated.text(rels).unwrap().contains("Id=\"vendor\" Type=\"urn:vendor\" Target=\"../theme/theme1.xml\""));
    for (path, content) in package.parts().iter().filter(|(path, _)| path.starts_with("ppt/slides/") || path.starts_with("ppt/notesSlides/")) { assert_eq!(updated.part(path).unwrap(), content, "{path}"); }
    let restored = document::undo(&changed.document, changed.document.revision, changed.receipt.unwrap()).unwrap();
    assert_eq!(save(&restored.document), bytes);
}