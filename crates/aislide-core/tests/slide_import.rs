use aislide_core::{
    data_report::SourceBinding,
    design::{self, Design, Theme},
    document::{self, Document, Transaction, TransactionResult},
    editing,
    limits::CapacityProfile,
    model::{Deck, Element},
    package::Package,
    parts,
    slide_import,
    sources::{self, SourceDocument, SourceFormat, SourceInput},
    Error, Result,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::ImageEncoder;
use serde_json::{json, to_value, Value};
use std::collections::BTreeSet;

fn authored(id: &str, count: usize) -> Document {
    let mut deck = editing::create(id.into(), "Import fixture".into()).unwrap().deck;
    let template = deck.slides[0].clone();
    deck.slides = (0..count).map(|index| {
        let mut slide = template.clone();
        slide.id = format!("page-{index}");
        slide.title = format!("Page {index}");
        slide
    }).collect();
    document::create(id.into(), deck, Vec::new(), Vec::new(), None).unwrap()
}

#[test]
fn thirty_nine_authored_imports_reuse_equivalent_design() {
    let original = authored("target", 1);
    let source = authored("source", 1);
    let source_before = to_value(&source).unwrap();
    let mut target = original.clone();
    for index in 0..39 {
        target = slide_import::import(&target, target.revision, &target.hash, &source,
            &[source.deck.slides[0].id.clone()], &format!("batch-{index}"), None).unwrap().document;
    }
    assert_eq!(target.deck.slides.len(), 40);
    assert_eq!(target.revision, 39);
    assert_eq!(to_value(&target.deck.design).unwrap(), to_value(&original.deck.design).unwrap());
    assert_eq!(to_value(&target.deck.slides[0]).unwrap(), to_value(&original.deck.slides[0]).unwrap());
    assert_eq!(to_value(&source).unwrap(), source_before);
    document::verify(&target).unwrap();
}

#[test]
fn selected_order_and_insertion_are_one_undo_without_source_mutation() {
    let target = authored("target", 2);
    let source = authored("source", 3);
    let source_before = to_value(&source).unwrap();
    let result = slide_import::import(&target, target.revision, &target.hash, &source,
        &["page-2".into(), "page-0".into()], "copy", Some("page-0")).unwrap();
    assert_eq!(result.document.revision, target.revision + 1);
    let slides = &result.document.deck.slides;
    assert_eq!(slides.iter().map(|slide| slide.title.as_str()).collect::<Vec<_>>(),
        vec!["Page 0", "Page 2", "Page 0", "Page 1"]);
    assert_ne!(slides[1].id, source.deck.slides[2].id);
    assert_ne!(slides[2].id, source.deck.slides[0].id);
    assert_eq!(to_value(&slides[1].elements).unwrap(), to_value(&source.deck.slides[2].elements).unwrap());
    let restored = document::undo(&result.document, result.document.revision, result.receipt.unwrap()).unwrap();
    assert_eq!(restored.document.hash, target.hash);
    assert_eq!(to_value(&restored.document.deck).unwrap(), to_value(&target.deck).unwrap());
    assert_eq!(to_value(&source).unwrap(), source_before);
}

fn edit(document: &Document, operations: Value) -> Document {
    document::transact(document, Transaction {
        expected_revision: document.revision,
        expected_hash: document.hash.clone(),
        operations: serde_json::from_value(operations).unwrap(),
    }).unwrap().document
}

fn from_deck(id: &str, deck: Deck) -> Document {
    document::create(id.into(), deck, Vec::new(), Vec::new(), None).unwrap()
}

fn import_all(target: &Document, source: &Document) -> Result<TransactionResult> {
    let ids = source.deck.slides.iter().map(|slide| slide.id.clone()).collect::<Vec<_>>();
    slide_import::import(target, target.revision, &target.hash, source, &ids, "copy", None)
}

fn text(id: &str, value: &str) -> Element {
    serde_json::from_value(json!({"type":"text","id":id,"x":64,"y":80,"width":600,"height":60,
        "text":value,"font_size":24,"color":"@accent1","bold":false,"format":{"font_family":"@minor"}})).unwrap()
}

fn picture(id: &str) -> Element {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes).write_image(
        &[20, 180, 90, 255], 1, 1, image::ExtendedColorType::Rgba8).unwrap();
    serde_json::from_value(json!({"type":"picture","id":id,"x":20,"y":20,"width":16,"height":16,
        "base64":STANDARD.encode(bytes),"mime_type":"image/png","alt":"Synthetic pixel"})).unwrap()
}

fn effective_theme(document: &Document, index: usize) -> Value {
    to_value(design::slide_theme(&document.deck.slides[index], document.deck.design.as_ref()).cloned().unwrap_or_default()).unwrap()
}

fn reopen(document: &Document) -> Document {
    let exported = document::export_presentation(document).unwrap();
    let opened = document::open_presentation("reopened".into(), STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    serde_json::from_value(opened["document"].clone()).unwrap()
}

#[test]
fn twenty_selected_layouts_with_different_ids_and_names_reuse_target_design() {
    let target = authored("target", 25);
    let mut deck = authored("source", 20).deck;
    let design = deck.design.as_mut().unwrap();
    design.masters[0].id = "source-master".into();
    design.masters[0].name = "Source display name".into();
    design.theme.name = "Equivalent theme display name".into();
    let template = design.layouts[0].clone();
    design.layouts = (0..20).map(|index| {
        let mut layout = template.clone();
        layout.id = format!("source-layout-{index}");
        layout.name = format!("Source layout {index}");
        layout.master_id = "source-master".into();
        layout
    }).collect();
    for (slide, layout) in deck.slides.iter_mut().zip(&design.layouts) { slide.layout_id = Some(layout.id.clone()); }
    let source = from_deck("source", deck);
    let result = import_all(&target, &source).unwrap();
    assert_eq!(result.document.deck.slides.len(), 45);
    assert_eq!(to_value(&result.document.deck.design).unwrap(), to_value(&target.deck.design).unwrap());
    assert!(result.document.deck.slides[25..].iter().all(|slide| slide.layout_id.as_deref() == Some("blank")));
}

#[test]
fn equivalent_layout_on_later_equivalent_master_is_reused_at_layout_limit() {
    let mut deck = authored("target", 1).deck;
    let design = deck.design.as_mut().unwrap();
    let mut second_master = design.masters[0].clone();
    second_master.id = "second".into();
    design.masters.push(second_master);
    let template = design.layouts[0].clone();
    design.layouts = (0..32).map(|index| {
        let mut layout = template.clone();
        layout.id = format!("layout-{index}");
        layout.master_id = if index == 31 { "second" } else { "master-1" }.into();
        layout.background = Some(if index == 31 { "FFFFFF" } else { "000000" }.into());
        layout
    }).collect();
    deck.slides[0].layout_id = Some("layout-0".into());
    let target = from_deck("target", deck);
    let source = edit(&authored("source", 1), json!([{"op":"replace","path":"/deck/design/layouts/0/background","value":"FFFFFF"}]));
    let result = import_all(&target, &source).unwrap();
    assert_eq!(result.document.deck.slides[1].layout_id.as_deref(), Some("layout-31"));
    assert_eq!(to_value(&result.document.deck.design).unwrap(), to_value(&target.deck.design).unwrap());
}

#[test]
fn different_owner_themes_and_inband_design_artwork_are_preserved() {
    let mut target_deck = authored("target", 1).deck;
    target_deck.slides[0].elements.push(text("target-text", "Target"));
    let target = from_deck("target", target_deck);
    let mut source_deck = authored("source", 1).deck;
    source_deck.slides[0].elements = vec![text("source-text", "Source"), picture("source-picture")];
    source_deck.slides[0].inherit_background = true;
    let source_design = source_deck.design.as_mut().unwrap();
    let mut owner_theme = Theme::default();
    owner_theme.colors.insert("accent1".into(), "B02244".into());
    owner_theme.colors.insert("lt1".into(), "E0F0FF".into());
    owner_theme.fonts.minor = "Arial".into();
    source_design.masters[0].theme = Some(owner_theme.clone());
    source_design.masters[0].elements.push(picture("master-picture"));
    source_design.layouts[0].elements.push(picture("layout-picture"));
    let source = from_deck("source", source_deck);
    let result = import_all(&target, &source).unwrap();
    let design = result.document.deck.design.as_ref().unwrap();
    assert_eq!(design.masters.len(), 2);
    assert_eq!(design.layouts.len(), 5);
    assert_eq!(effective_theme(&result.document, 0), effective_theme(&target, 0));
    assert_eq!(effective_theme(&result.document, 1), to_value(owner_theme).unwrap());
    assert_eq!(to_value(&result.document.deck.slides[0]).unwrap(), to_value(&target.deck.slides[0]).unwrap());
    assert_eq!(to_value(&result.document.deck.slides[1].elements).unwrap(), to_value(&source.deck.slides[0].elements).unwrap());
    assert_eq!(to_value(&design.masters[1].elements).unwrap(), to_value(&source.deck.design.as_ref().unwrap().masters[0].elements).unwrap());
    assert_eq!(to_value(&design.layouts[4].elements).unwrap(), to_value(&source.deck.design.as_ref().unwrap().layouts[0].elements).unwrap());
    let opened = reopen(&result.document);
    assert_eq!(effective_theme(&opened, 0)["colors"], effective_theme(&target, 0)["colors"]);
    assert_eq!(effective_theme(&opened, 1)["colors"], effective_theme(&source, 0)["colors"]);
    assert_eq!(to_value(&opened.deck.slides[1].elements[1]).unwrap()["base64"], to_value(&source.deck.slides[0].elements[1]).unwrap()["base64"]);
}

#[test]
fn designless_target_keeps_default_theme_and_existing_slide_exactly() {
    let mut deck = authored("target", 1).deck;
    deck.design = None;
    deck.slides[0].layout_id = None;
    deck.slides[0].elements.push(text("target-text", "Target"));
    let target = from_deck("target", deck);
    let source = edit(&authored("source", 1), json!([{"op":"replace","path":"/deck/design/theme/colors/accent1","value":"BB0011"}]));
    let result = import_all(&target, &source).unwrap();
    assert_eq!(effective_theme(&result.document, 0), effective_theme(&target, 0));
    assert_eq!(effective_theme(&result.document, 1), effective_theme(&source, 0));
    assert_eq!(to_value(&result.document.deck.slides[0]).unwrap(), to_value(&target.deck.slides[0]).unwrap());
    assert!(result.document.deck.design.as_ref().unwrap().masters[0].theme.is_some());
    assert!(result.document.deck.design.as_ref().unwrap().masters[0].id.starts_with("copy-m"));
    assert!(result.document.deck.design.as_ref().unwrap().layouts[0].id.starts_with("copy-l"));
    let opened = reopen(&result.document);
    assert_eq!(effective_theme(&opened, 0)["colors"], effective_theme(&target, 0)["colors"]);
}

#[test]
fn designless_source_uses_explicit_default_theme_in_custom_target() {
    let target = edit(&authored("target", 1), json!([{"op":"replace","path":"/deck/design/theme/colors/accent1","value":"BB0011"}]));
    let mut deck = authored("source", 1).deck;
    deck.design = None;
    deck.slides[0].layout_id = None;
    deck.slides[0].inherit_background = true;
    deck.slides[0].background = "@accent1".into();
    let source = from_deck("source", deck);
    let result = import_all(&target, &source).unwrap();
    assert_eq!(effective_theme(&result.document, 0), effective_theme(&target, 0));
    assert_eq!(effective_theme(&result.document, 1), effective_theme(&source, 0));
    assert_eq!(design::slide_background(&result.document.deck.slides[1], result.document.deck.design.as_ref()), "@accent1");
}

#[test]
fn implicit_source_layout_keeps_its_artwork_theme_and_noninherited_background() {
    let target = authored("target", 1);
    let mut deck = authored("source", 1).deck;
    deck.slides[0].layout_id = None;
    deck.slides[0].inherit_background = true;
    deck.slides[0].background = "ABCDEF".into();
    deck.design.as_mut().unwrap().layouts[0].background = Some("000000".into());
    deck.design.as_mut().unwrap().layouts[0].elements.push(text("layout-art", "Layout"));
    let source = from_deck("source", deck);
    let result = import_all(&target, &source).unwrap();
    assert_eq!(effective_theme(&result.document, 1), effective_theme(&source, 0));
    assert_eq!(design::slide_background(&result.document.deck.slides[1], result.document.deck.design.as_ref()), "ABCDEF");
    let layout_id = result.document.deck.slides[1].layout_id.as_ref().unwrap();
    let layout = result.document.deck.design.as_ref().unwrap().layouts.iter().find(|layout| &layout.id == layout_id).unwrap();
    assert_eq!(to_value(&layout.elements).unwrap(), to_value(&source.deck.design.as_ref().unwrap().layouts[0].elements).unwrap());
}

#[test]
fn deterministic_ids_avoid_slide_master_and_layout_collisions() {
    let mut deck = authored("target", 3).deck;
    for (slide, id) in deck.slides.iter_mut().zip(["copy-s1", "copy-m1", "copy-l1"]) { slide.id = id.into(); }
    let target = from_deck("target", deck);
    let source = edit(&authored("source", 2), json!([{"op":"replace","path":"/deck/design/theme/colors/accent1","value":"BB0011"}]));
    let first = import_all(&target, &source).unwrap().document;
    let second = import_all(&target, &source).unwrap().document;
    assert_eq!(first.hash, second.hash);
    let design = first.deck.design.as_ref().unwrap();
    let ids = first.deck.slides.iter().map(|slide| &slide.id)
        .chain(design.masters.iter().map(|master| &master.id))
        .chain(design.layouts.iter().map(|layout| &layout.id)).collect::<Vec<_>>();
    assert_eq!(ids.iter().copied().collect::<BTreeSet<_>>().len(), ids.len());
    assert!(ids.iter().all(|id| id.len() <= 80));
    let repeated = import_all(&first, &source).unwrap().document;
    assert_eq!(repeated.deck.slides.len(), 7);
    assert_eq!(to_value(&repeated.deck.design).unwrap(), to_value(design).unwrap());
}

#[test]
fn invalid_selections_prefixes_anchors_and_stale_guards_are_atomic() {
    let target = authored("target", 1);
    let source = authored("source", 129);
    let before = to_value(&target).unwrap();
    for ids in [vec![], vec!["page-0".into(), "page-0".into()], vec!["missing".into()],
        vec![String::new()], vec![" ".into()], vec!["x".repeat(81)], vec!["page-0\n".into()],
        source.deck.slides.iter().map(|slide| slide.id.clone()).collect()]
    {
        assert!(slide_import::import(&target, target.revision, &target.hash, &source, &ids, "copy", None).is_err());
    }
    for prefix in ["", "has space", "../path", "a/b", "a.b", "\u{00e9}", "x\n", &"x".repeat(25)] {
        assert!(slide_import::import(&target, target.revision, &target.hash, &source, &["page-0".into()], prefix, None).is_err());
    }
    assert!(matches!(slide_import::import(&target, 1, &target.hash, &source, &["page-0".into()], "copy", None), Err(Error::Conflict(_))));
    assert!(matches!(slide_import::import(&target, 0, &"0".repeat(64), &source, &["page-0".into()], "copy", None), Err(Error::Conflict(_))));
    assert!(slide_import::import(&target, 0, &target.hash, &source, &["page-0".into()], "copy", Some("missing")).is_err());
    assert_eq!(to_value(&target).unwrap(), before);
}

#[test]
fn maximum_selection_id_and_prefix_lengths_are_supported() {
    let target = authored("target", 1);
    let mut deck = authored("source", 128).deck;
    deck.slides[0].id = "x".repeat(80);
    let source = from_deck("source", deck);
    let ids = source.deck.slides.iter().map(|slide| slide.id.clone()).collect::<Vec<_>>();
    let result = slide_import::import(&target, 0, &target.hash, &source, &ids, &"p".repeat(24), None).unwrap();
    assert_eq!(result.document.deck.slides.len(), 129);
    assert_eq!(result.document.revision, 1);
    let undone = document::undo(&result.document, 1, result.receipt.unwrap()).unwrap();
    assert_eq!(undone.document.hash, target.hash);
}

#[test]
fn both_document_hashes_are_verified_including_unselected_source_slides() {
    let target = authored("target", 1);
    let source = authored("source", 2);
    let mut invalid_source = source.clone();
    invalid_source.deck.slides[1].title = "Out of transaction".into();
    assert!(matches!(slide_import::import(&target, 0, &target.hash, &invalid_source, &["page-0".into()], "copy", None), Err(Error::Conflict(_))));
    let mut invalid_target = target.clone();
    invalid_target.deck.title = "Out of transaction".into();
    assert!(matches!(import_all(&invalid_target, &source), Err(Error::Conflict(_))));
    let mut invalid_hash = source.clone();
    invalid_hash.hash = "0".repeat(64);
    assert!(matches!(import_all(&target, &invalid_hash), Err(Error::Conflict(_))));
}

#[test]
fn mismatched_canvas_and_target_slide_capacity_are_rejected() {
    let target = authored("target", 1);
    let source = edit(&authored("source", 1), json!([{"op":"replace","path":"/deck/width","value":1400}]));
    assert!(matches!(import_all(&target, &source), Err(Error::Unsupported(_))));
    let target = document::create_with_profile("small".into(), authored("small", 25).deck,
        Vec::new(), Vec::new(), None, CapacityProfile::Legacy).unwrap();
    assert!(matches!(import_all(&target, &authored("source", 8)), Err(Error::Limit(_))));
    assert_eq!(import_all(&target, &authored("source", 7)).unwrap().document.deck.slides.len(), 32);
}

#[test]
fn unequal_designs_do_not_bypass_master_and_layout_limits() {
    let mut deck = authored("target", 1).deck;
    let template = Design::default();
    let design = deck.design.as_mut().unwrap();
    design.masters.clear();
    design.layouts.clear();
    for index in 0..8 {
        let mut master = template.masters[0].clone();
        master.id = format!("master-{index}");
        master.background = format!("{index:06X}");
        let mut layout = template.layouts[0].clone();
        layout.id = format!("layout-{index}");
        layout.master_id = master.id.clone();
        design.masters.push(master);
        design.layouts.push(layout);
    }
    deck.slides[0].layout_id = Some("layout-0".into());
    let target = from_deck("target", deck);
    assert!(matches!(import_all(&target, &authored("source", 1)), Err(Error::Limit(_))));
    let mut deck = authored("target", 1).deck;
    let design = deck.design.as_mut().unwrap();
    design.layouts = (0..32).map(|index| {
        let mut layout = template.layouts[0].clone();
        layout.id = format!("layout-{index}");
        layout.background = Some(format!("{index:06X}"));
        layout
    }).collect();
    deck.slides[0].layout_id = Some("layout-0".into());
    let target = from_deck("target", deck);
    assert!(matches!(import_all(&target, &authored("source", 1)), Err(Error::Limit(_))));
}

fn source_record(name: &str, value: &str) -> SourceDocument {
    sources::ingest(SourceInput { name: name.into(), format: SourceFormat::Csv,
        base64: STANDARD.encode(format!("Value\n{value}\n")), ocr: false, ocr_language: None, attribution: None }).unwrap()
}

fn bound_document(id: &str, record: SourceDocument) -> Document {
    let mut deck = authored(id, 2).deck;
    let raw = record.tables[0].rows[0][0].clone();
    let value = raw.as_str().unwrap();
    for slide in &mut deck.slides { slide.elements.push(text("bound-text", value)); }
    let bindings = deck.slides.iter().map(|slide| SourceBinding {
        slide_id: slide.id.clone(), element_id: "bound-text".into(), field: "/text".into(),
        source_id: record.id.clone(), source_sha256: record.sha256.clone(), locator: record.tables[0].locators[0][0].clone(),
        value: raw.clone(), raw_value: raw.clone(), transform: "display_scalar".into(), stale: false,
    }).collect();
    document::create(id.into(), deck, vec![record], bindings, None).unwrap()
}

fn with_part(document: &Document, slide_id: &str) -> Document {
    let spec = serde_json::from_value(json!({"version":1,"preset":"flow/balanced","title":"Synthetic process",
        "data":{"kind":"items","items":[{"label":"Input"},{"label":"Output"}]}})).unwrap();
    parts::state::change(document, document.revision, slide_id, "process", &spec, false).unwrap().document
}

#[test]
fn selected_parts_bindings_sources_and_frames_survive_undo_export_and_reopen() {
    let target = with_part(&bound_document("target", source_record("target.csv", "17")), "page-1");
    let source = with_part(&bound_document("source", source_record("source.csv", "23")), "page-1");
    let unbound = source_record("excluded.csv", "99");
    let source = edit(&source, json!([{"op":"add","path":"/sources/-","value":unbound}]));
    let before = to_value(&source).unwrap();
    let result = slide_import::import(&target, target.revision, &target.hash, &source, &["page-1".into()], "copy", None).unwrap();
    let imported_id = &result.document.deck.slides[2].id;
    assert_eq!(result.document.sources.len(), 2);
    assert_eq!(result.document.bindings.len(), target.bindings.len() + 1);
    assert_eq!(to_value(&result.document.bindings[..target.bindings.len()]).unwrap(), to_value(&target.bindings).unwrap());
    let mut expected_binding = source.bindings[1].clone();
    expected_binding.slide_id = imported_id.clone();
    assert_eq!(to_value(result.document.bindings.last().unwrap()).unwrap(), to_value(expected_binding).unwrap());
    assert_eq!(to_value(&result.document.parts[0]).unwrap(), to_value(&target.parts[0]).unwrap());
    let mut expected_part = source.parts[0].clone();
    expected_part.slide_id = imported_id.clone();
    assert_eq!(to_value(&result.document.parts[1]).unwrap(), to_value(expected_part).unwrap());
    assert_eq!(to_value(&result.document.deck.slides[2].elements).unwrap(), to_value(&source.deck.slides[1].elements).unwrap());
    let opened = reopen(&result.document);
    assert_eq!(opened.deck.slides.len(), 3);
    assert_eq!(opened.parts.len(), 2);
    assert!(opened.parts.iter().all(|part| !part.stale));
    assert!(opened.bindings.iter().all(|binding| !binding.stale));
    assert_eq!(to_value(&opened.sources).unwrap(), to_value(&result.document.sources).unwrap());
    assert_eq!(opened.parts[1].slide_id, *imported_id);
    let undone = document::undo(&result.document, result.document.revision, result.receipt.unwrap()).unwrap();
    assert_eq!(undone.document.hash, target.hash);
    assert_eq!(to_value(&source).unwrap(), before);
}

#[test]
fn same_sha_sources_reuse_target_record_without_renaming_bindings() {
    let target = bound_document("target", source_record("target.csv", "17"));
    let source = bound_document("source", source_record("source-display.csv", "17"));
    let result = import_all(&target, &source).unwrap();
    assert_eq!(to_value(&result.document.sources).unwrap(), to_value(&target.sources).unwrap());
    assert_eq!(result.document.bindings.len(), 4);
    assert!(result.document.bindings.iter().all(|binding| binding.source_id == target.sources[0].id && !binding.stale));
    let mut corrupted = source.clone();
    corrupted.sources[0].sha256 = "0".repeat(64);
    assert!(import_all(&target, &corrupted).is_err());
}

#[test]
fn selected_stale_parts_and_bindings_are_rejected_but_excluded_metadata_is_not_copied() {
    let target = authored("target", 1);
    let source = with_part(&authored("source", 2), "page-1");
    let stale = edit(&source, json!([{"op":"replace","path":"/deck/slides/1/elements/0/children/0/text","value":"Manual change"}]));
    assert!(stale.parts[0].stale);
    assert!(matches!(import_all(&target, &stale), Err(Error::Conflict(_))));
    let result = slide_import::import(&target, 0, &target.hash, &stale, &["page-0".into()], "copy", None).unwrap();
    assert!(result.document.parts.is_empty());
    let bound = bound_document("source", source_record("source.csv", "17"));
    let stale = edit(&bound, json!([{"op":"replace","path":"/deck/slides/0/elements/0/text","value":"Manual change"}]));
    assert!(matches!(import_all(&target, &stale), Err(Error::Conflict(_))));
}

#[test]
fn source_and_part_resource_capacity_is_checked_after_merging() {
    let records = (0..8).map(|index| source_record(&format!("source-{index}.csv"), &index.to_string())).collect();
    let target = document::create("target".into(), authored("target", 1).deck, records, Vec::new(), None).unwrap();
    let source = bound_document("source", source_record("ninth.csv", "99"));
    assert!(import_all(&target, &source).is_err());
    let template = with_part(&authored("template", 1), "page-0");
    let target = authored("target", 128);
    let mut deck = target.deck.clone();
    let mut instances = Vec::new();
    for slide in &mut deck.slides {
        slide.elements = template.deck.slides[0].elements.clone();
        let mut part = template.parts[0].clone();
        part.slide_id = slide.id.clone();
        instances.push(part);
    }
    let target = edit(&target, json!([
        {"op":"replace","path":"/deck","value":deck},
        {"op":"add","path":"/parts","value":instances}
    ]));
    let source = with_part(&authored("source", 1), "page-0");
    assert!(matches!(import_all(&target, &source), Err(Error::Limit(_))));
}

#[test]
fn native_sources_fail_closed_and_native_target_parts_stay_byte_identical() {
    let source = authored("source", 1);
    let opened_source = reopen(&source);
    let target = authored("target", 1);
    let error = import_all(&target, &opened_source).err().expect("native source must reject");
    assert!(matches!(error, Error::Unsupported(_)));
    assert!(error.to_string().contains("authored source"));
    let native_target = reopen(&with_part(&target, "page-0"));
    let original = Package::open(STANDARD.decode(&native_target.origin.as_ref().unwrap().base64).unwrap()).unwrap();
    let result = import_all(&native_target, &with_part(&source, "page-0")).unwrap();
    assert_eq!(to_value(&result.document.origin).unwrap(), to_value(&native_target.origin).unwrap());
    assert_eq!(to_value(&result.document.parts[0]).unwrap(), to_value(&native_target.parts[0]).unwrap());
    assert_eq!(to_value(&result.document.deck.slides[0]).unwrap(), to_value(&native_target.deck.slides[0]).unwrap());
    let saved = document::export_presentation(&result.document).unwrap();
    let saved = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    for path in ["ppt/slides/slide1.xml", "ppt/slideMasters/slideMaster1.xml", "ppt/slideLayouts/slideLayout1.xml", "ppt/theme/theme1.xml"] {
        assert_eq!(saved.part(path).unwrap(), original.part(path).unwrap(), "{path}");
    }
    let undone = document::undo(&result.document, result.document.revision, result.receipt.unwrap()).unwrap();
    assert_eq!(undone.document.hash, native_target.hash);
}

#[test]
fn regular_comments_remain_slide_local_and_modern_reviews_fail_closed() {
    let mut deck = authored("source", 2).deck;
    let review: aislide_core::review::SlideReview = serde_json::from_value(json!({"comments":[
        {"id":"comment","author":"Synthetic Author","initials":"SA","timestamp":"2026-09-23T00:00:00Z","text":"Comment","native_author_id":7,"native_index":1},
        {"id":"reply","author":"Synthetic Author","initials":"SA","timestamp":"2026-09-23T00:00:00Z","text":"Reply","parent_id":"comment","resolved":true}
    ]})).unwrap();
    for slide in &mut deck.slides { slide.review = Some(review.clone()); }
    let source = from_deck("source", deck);
    let target = authored("target", 1);
    let result = import_all(&target, &source).unwrap();
    for slide in &result.document.deck.slides[1..] {
        let review = slide.review.as_ref().unwrap();
        assert_eq!(review.comments[0].id, "comment");
        assert_eq!(review.comments[1].parent_id.as_deref(), Some("comment"));
        assert!(review.comments[1].resolved);
        assert!(review.comments.iter().all(|comment| comment.native_author_id.is_none() && comment.native_index.is_none()));
    }
    let opened = reopen(&result.document);
    assert_eq!(opened.deck.slides[1].review.as_ref().unwrap().comments[0].author, "Synthetic Author");
    let thread = json!({
        "id":"{00112233-4455-6677-8899-aabbccddeeff}",
        "author":{"id":"{10112233-4455-6677-8899-aabbccddeeff}","name":"Synthetic Author","user_id":"offline","provider_id":"offline"},
        "created":"2026-09-23T00:00:00Z","anchor":{"kind":"unknown"},"body":[],"replies":[]
    });
    for threads in [json!([]), json!([thread])] {
        let modern = edit(&source, json!([{"op":"add","path":"/deck/slides/0/review/modern_threads","value":threads}]));
        assert!(matches!(import_all(&target, &modern), Err(Error::Unsupported(_))));
    }
    let mut unknown = to_value(&source).unwrap();
    unknown["deck"]["slides"][0]["review"]["unknown_review"] = json!({});
    assert!(serde_json::from_value::<Document>(unknown).is_err());
}

#[test]
fn altered_slide_list_clears_target_report_and_undo_restores_it() {
    let report = aislide_core::report::sample_report();
    let report = serde_json::to_value(report).unwrap();
    let target = edit(&authored("target", 1), json!([{"op":"add","path":"/report","value":report}]));
    let result = import_all(&target, &authored("source", 1)).unwrap();
    assert!(result.document.report.is_none());
    let undone = document::undo(&result.document, result.document.revision, result.receipt.unwrap()).unwrap();
    assert_eq!(undone.document.hash, target.hash);
    assert_eq!(to_value(&undone.document.report).unwrap(), to_value(&target.report).unwrap());
}

#[test]
fn import_does_not_reactivate_or_collide_with_orphaned_target_metadata() {
    let target = with_part(&bound_document("target", source_record("target.csv", "17")), "page-0");
    let target = edit(&target, json!([
        {"op":"replace","path":"/bindings/0/slide_id","value":"copy-s1"},
        {"op":"replace","path":"/parts/0/slide_id","value":"copy-s1"}
    ]));
    assert!(target.bindings[0].stale);
    assert!(target.parts[0].stale);
    let source = with_part(&bound_document("source", source_record("source.csv", "17")), "page-0");
    let result = slide_import::import(&target, target.revision, &target.hash, &source, &["page-0".into()], "copy", None).unwrap();
    assert_ne!(result.document.deck.slides[2].id, "copy-s1");
    assert_eq!(to_value(&result.document.bindings[..target.bindings.len()]).unwrap(), to_value(&target.bindings).unwrap());
    assert_eq!(to_value(&result.document.parts[0]).unwrap(), to_value(&target.parts[0]).unwrap());
}

fn synthetic_font(variant: u8, license_acknowledged: bool) -> aislide_core::fonts::EmbeddedFont {
    let mut tables = std::collections::BTreeMap::new();
    let entries = [(1u16, "AISlide Import Synthetic"), (2, "Regular"), (4, "AISlide Import Synthetic Regular"),
        (5, "Version 1.0"), (6, "AISlideImportSynthetic-Regular")];
    let mut names = vec![0u8; 6 + entries.len() * 12];
    names[2..4].copy_from_slice(&(entries.len() as u16).to_be_bytes());
    let storage = names.len();
    names[4..6].copy_from_slice(&(storage as u16).to_be_bytes());
    for (index, (id, name)) in entries.into_iter().enumerate() {
        let encoded = name.encode_utf16().flat_map(u16::to_be_bytes).collect::<Vec<_>>();
        let offset = names.len() - storage;
        for (field, value) in [3u16, 1, 0x409, id, encoded.len() as u16, offset as u16].into_iter().enumerate() {
            let position = 6 + index * 12 + field * 2;
            names[position..position + 2].copy_from_slice(&value.to_be_bytes());
        }
        names.extend(encoded);
    }
    let mut metrics = vec![0u8; 96];
    metrics[..2].copy_from_slice(&4u16.to_be_bytes());
    metrics[4..6].copy_from_slice(&400u16.to_be_bytes());
    let mut head = vec![0u8; 54];
    head[..4].copy_from_slice(&0x10000u32.to_be_bytes());
    head[12..16].copy_from_slice(&0x5f0f3cf5u32.to_be_bytes());
    head[18..20].copy_from_slice(&1000u16.to_be_bytes());
    let mut hhea = vec![0u8; 36];
    hhea[..4].copy_from_slice(&0x10000u32.to_be_bytes());
    hhea[4..6].copy_from_slice(&800i16.to_be_bytes());
    hhea[6..8].copy_from_slice(&(-200i16).to_be_bytes());
    hhea[8..10].copy_from_slice(&i16::from(variant).to_be_bytes());
    hhea[34..36].copy_from_slice(&2u16.to_be_bytes());
    let mut maxp = vec![0u8; 32];
    maxp[..4].copy_from_slice(&0x10000u32.to_be_bytes());
    maxp[4..6].copy_from_slice(&2u16.to_be_bytes());
    let mut cmap = vec![0, 0, 0, 1, 0, 3, 0, 1, 0, 0, 0, 12, 0, 0, 1, 6, 0, 0];
    cmap.extend(vec![0u8; 256]);
    cmap[18 + 65] = 1;
    let mut glyph = vec![0, 1, 0, 0, 0, 0, 1, 244, 2, 188, 0, 2, 0, 0, 1, 1, 1];
    for coordinate in [0i16, 500, -250, 0, 0, 700] { glyph.extend(coordinate.to_be_bytes()); }
    glyph.push(0);
    let mut post = vec![0u8; 32];
    post[..4].copy_from_slice(&0x30000u32.to_be_bytes());
    tables.extend([(*b"name", names), (*b"OS/2", metrics), (*b"head", head), (*b"hhea", hhea),
        (*b"hmtx", vec![2, 88, 0, 0, 2, 88, 0, 0]), (*b"maxp", maxp), (*b"cmap", cmap),
        (*b"glyf", glyph), (*b"loca", vec![0, 0, 0, 0, 0, 15]), (*b"post", post)]);
    let mut bytes = vec![0u8; 12 + tables.len() * 16];
    bytes[..4].copy_from_slice(&0x10000u32.to_be_bytes());
    bytes[4..6].copy_from_slice(&(tables.len() as u16).to_be_bytes());
    for (index, (tag, data)) in tables.into_iter().enumerate() {
        while bytes.len() % 4 != 0 { bytes.push(0); }
        let record = 12 + index * 16;
        bytes[record..record + 4].copy_from_slice(&tag);
        let offset = bytes.len() as u32;
        bytes[record + 8..record + 12].copy_from_slice(&offset.to_be_bytes());
        bytes[record + 12..record + 16].copy_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend(data);
    }
    let info = aislide_core::fonts::inspect(&bytes).unwrap();
    assert!(info.usable);
    aislide_core::fonts::EmbeddedFont { family: info.family, style: info.style, base64: STANDARD.encode(bytes), license_acknowledged }
}

fn font_document(id: &str, variant: u8, consent: bool) -> Document {
    let mut deck = authored(id, 1).deck;
    let font = synthetic_font(variant, consent);
    let mut label = text("font-sample", "A");
    if let Element::Text { format, .. } = &mut label { format.font_family = Some(font.family.clone()); }
    deck.slides[0].elements.push(label);
    deck.embedded_fonts.push(font);
    from_deck(id, deck)
}

#[test]
fn consented_fonts_are_copied_by_bytes_deduplicated_and_undoable() {
    let target = authored("target", 1);
    let source = font_document("source", 0, true);
    let source_before = to_value(&source).unwrap();
    let first = import_all(&target, &source).unwrap();
    assert_eq!(first.document.deck.embedded_fonts, source.deck.embedded_fonts);
    let second = import_all(&first.document, &source).unwrap();
    assert_eq!(second.document.deck.embedded_fonts.len(), 1);
    assert_eq!(second.document.deck.embedded_fonts, source.deck.embedded_fonts);
    let exported = document::export_presentation(&first.document).unwrap();
    let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let font_bytes = STANDARD.decode(&source.deck.embedded_fonts[0].base64).unwrap();
    assert!(package.parts().iter().any(|(name, bytes)| name.ends_with(".fntdata") && bytes.ends_with(&font_bytes)));
    let undone = document::undo(&first.document, first.document.revision, first.receipt.unwrap()).unwrap();
    assert_eq!(undone.document.hash, target.hash);
    assert!(undone.document.deck.embedded_fonts.is_empty());
    assert_eq!(to_value(&source).unwrap(), source_before);
}

#[test]
fn unconsented_or_ambiguous_fonts_never_enable_or_replace_target_resources() {
    let target = authored("target", 1);
    let unconsented = font_document("source", 0, false);
    document::verify(&unconsented).unwrap();
    assert!(matches!(import_all(&target, &unconsented), Err(Error::Unsupported(_))));
    let source = font_document("source", 0, true);
    for target in [font_document("target", 0, false), font_document("target", 1, true)] {
        let before = to_value(&target).unwrap();
        assert!(matches!(import_all(&target, &source), Err(Error::Conflict(_))));
        assert_eq!(to_value(&target).unwrap(), before);
    }
}

#[test]
fn native_references_and_incompatible_auxiliary_design_fail_closed() {
    let target = authored("target", 1);
    let source = edit(&authored("source", 1), json!([{"op":"add","path":"/deck/slides/0/native_source_id","value":"foreign-native"}]));
    assert!(matches!(import_all(&target, &source), Err(Error::Unsupported(_))));
    let source = edit(&authored("source", 1), json!([{"op":"add","path":"/deck/auxiliary_design","value":{
        "width":720,"height":960,"notes_master":{"name":"Source notes","background":"FFFFFF","theme":Theme::default(),"elements":[]}
    }}]));
    assert!(matches!(import_all(&target, &source), Err(Error::Unsupported(_))));
    let target = edit(&target, json!([{"op":"add","path":"/deck/auxiliary_design","value":source.deck.auxiliary_design}]));
    let result = import_all(&target, &source).unwrap();
    assert_eq!(to_value(&result.document.deck.auxiliary_design).unwrap(), to_value(&target.deck.auxiliary_design).unwrap());
}

#[test]
fn aggregate_element_budget_is_checked_without_changing_target_profile() {
    let mut deck = authored("target", 32).deck;
    deck.design = None;
    for slide in &mut deck.slides {
        slide.layout_id = None;
        slide.elements = (0..256).map(|index| serde_json::from_value(json!({
            "type":"rect","id":format!("rect-{index}"),"x":0,"y":0,"width":1,"height":1,"fill":"000000"
        })).unwrap()).collect();
    }
    let target = from_deck("target", deck);
    let mut deck = authored("source", 1).deck;
    deck.slides[0].elements.push(text("extra", "Extra"));
    let source = from_deck("source", deck);
    assert!(matches!(import_all(&target, &source), Err(Error::Limit(_))));
    assert_eq!(target.capacity_profile, CapacityProfile::Large);
    assert_eq!(target.deck.slides.len(), 32);
}