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