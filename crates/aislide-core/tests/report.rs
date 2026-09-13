use aislide_core::{package::Package, report::{compile_report, sample_report}, pptx::{export_pptx, inspect_pptx, patch_text}};

#[test]
fn twelve_native_slides_are_deterministic() {
    let compiled = compile_report(&sample_report()).unwrap();
    assert_eq!(compiled.deck.slides.len(), 12);
    let bytes = export_pptx(&compiled.deck).unwrap();
    assert_eq!(bytes, export_pptx(&compiled.deck).unwrap());
    let package = Package::open(bytes.clone()).unwrap();
    assert_eq!(package.save().unwrap(), bytes);
    assert_eq!(inspect_pptx(bytes).unwrap().slides.len(), 12);
    let mut tables = 0;
    for (name, data) in package.parts() {
        if name.ends_with(".xml") || name.ends_with(".rels") {
            let document = roxmltree::Document::parse(std::str::from_utf8(data).unwrap()).unwrap();
            tables += document.descendants().filter(|node| node.has_tag_name(("http://schemas.openxmlformats.org/drawingml/2006/main", "tbl"))).count();
        }
        assert!(!name.starts_with("ppt/media/"), "must not render slides as pictures");
    }
    assert!(tables > 0);
}

#[test]
fn notes_and_slide_masters_reference_distinct_theme_parts() {
    let bytes = export_pptx(&compile_report(&sample_report()).unwrap().deck).unwrap();
    let package = Package::open(bytes).unwrap();
    let target = |path: &str| {
        let document = roxmltree::Document::parse(package.text(path).unwrap()).unwrap();
        document.root_element().children().find(|node| node.attribute("Type").is_some_and(|kind| kind.ends_with("/theme"))).unwrap().attribute("Target").unwrap().to_string()
    };
    assert_ne!(target("ppt/slideMasters/_rels/slideMaster1.xml.rels"), target("ppt/notesMasters/_rels/notesMaster1.xml.rels"));
}

#[test]
fn text_patch_changes_only_the_addressed_slide() {
    let bytes = export_pptx(&compile_report(&sample_report()).unwrap().deck).unwrap();
    let original = Package::open(bytes.clone()).unwrap();
    let inspection = inspect_pptx(bytes.clone()).unwrap();
    let slide = &inspection.slides[0];
    let text = &slide.texts[0];
    let updated = patch_text(bytes, &slide.part, &text.shape_id, text.run_index, &text.text, "\u{65e5}\u{672c}\u{8a9e} & English <report>").unwrap();
    let package = Package::open(updated.clone()).unwrap();
    for (name, data) in original.parts() {
        if name != &slide.part { assert_eq!(data, &package.parts()[name], "changed {name}"); }
    }
    assert_eq!(inspect_pptx(updated).unwrap().slides[0].texts[0].text, "\u{65e5}\u{672c}\u{8a9e} & English <report>");
}

#[test]
fn patch_no_op_returns_original_bytes_and_stale_expected_fails() {
    let bytes = export_pptx(&compile_report(&sample_report()).unwrap().deck).unwrap();
    let inspection = inspect_pptx(bytes.clone()).unwrap();
    let slide = &inspection.slides[0];
    let text = &slide.texts[0];
    assert_eq!(patch_text(bytes.clone(), &slide.part, &text.shape_id, text.run_index, &text.text, &text.text).unwrap(), bytes);
    assert!(patch_text(bytes, &slide.part, &text.shape_id, text.run_index, "stale", "new").is_err());
}

#[test]
fn invalid_reports_are_rejected_before_layout() {
    let mut report = sample_report();
    report.sections.clear();
    assert!(compile_report(&report).is_err());
    let mut report = sample_report();
    report.title = "bad\0text".into();
    assert!(compile_report(&report).is_err());
}

#[test]
fn invalid_scene_does_not_export() {
    let mut deck = compile_report(&sample_report()).unwrap().deck;
    deck.slides[0].background = "\"/><bad/>".into();
    assert!(export_pptx(&deck).is_err());
}

#[test]
fn extension_slide_ids_are_not_counted() {
    let bytes = export_pptx(&compile_report(&sample_report()).unwrap().deck).unwrap();
    let mut package = Package::open(bytes).unwrap();
    let xml = package.text("ppt/presentation.xml").unwrap().replace("</p:presentation>", "<p:extLst><p:ext uri='test'><p:sldIdLst><p:sldId id='999' r:id='invalid'/></p:sldIdLst></p:ext></p:extLst></p:presentation>");
    package.replace_part("ppt/presentation.xml", xml.into_bytes()).unwrap();
    assert_eq!(inspect_pptx(package.save().unwrap()).unwrap().slides.len(), 12);
}

#[test]
fn inspection_rejects_dtd_and_external_slide_relationships() {
    let bytes = export_pptx(&compile_report(&sample_report()).unwrap().deck).unwrap();
    let mut dtd = Package::open(bytes.clone()).unwrap();
    let value = format!("<!DOCTYPE p:presentation [<!ENTITY sample 'bad'>]>{}", dtd.text("ppt/presentation.xml").unwrap());
    dtd.replace_part("ppt/presentation.xml", value.into_bytes()).unwrap();
    assert!(inspect_pptx(dtd.save().unwrap()).is_err());
    let mut external = Package::open(bytes).unwrap();
    let rels = external.text("ppt/_rels/presentation.xml.rels").unwrap().replace("Target=\"slides/slide1.xml\"", "Target=\"https://example.invalid/slide.xml\" TargetMode=\"External\"");
    external.replace_part("ppt/_rels/presentation.xml.rels", rels.into_bytes()).unwrap();
    assert!(inspect_pptx(external.save().unwrap()).is_err());
}

#[test]
fn patch_rejects_illegal_characters_unknown_parts_and_signed_packages() {
    let bytes = export_pptx(&compile_report(&sample_report()).unwrap().deck).unwrap();
    let inspection = inspect_pptx(bytes.clone()).unwrap();
    let slide = &inspection.slides[0];
    let text = &slide.texts[0];
    assert!(patch_text(bytes.clone(), &slide.part, &text.shape_id, text.run_index, &text.text, "bad\0").is_err());
    assert!(patch_text(bytes.clone(), "ppt/theme/theme1.xml", &text.shape_id, text.run_index, &text.text, "new").is_err());
    let mut parts = Package::open(bytes).unwrap().parts().clone();
    parts.insert("_xmlsignatures/sig1.xml".into(), b"<signature/>".to_vec());
    let signed = Package::from_parts(parts).unwrap().save().unwrap();
    assert!(patch_text(signed, &slide.part, &text.shape_id, text.run_index, &text.text, "new").is_err());
}

#[test]
fn invalid_geometry_and_duplicate_ids_are_rejected() {
    let original = compile_report(&sample_report()).unwrap().deck;
    let mut duplicate = original.clone();
    duplicate.slides[1].id = duplicate.slides[0].id.clone();
    assert!(export_pptx(&duplicate).is_err());
    let mut invalid = original;
    if let aislide_core::model::Element::Rect { x, .. } = &mut invalid.slides[0].elements[0] { *x = f64::NAN; }
    assert!(export_pptx(&invalid).is_err());
}