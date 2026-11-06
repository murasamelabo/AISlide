use aislide_core::{authoring_ops::{export_template, TemplateKind}, authoring_preflight::{preflight_presentation, PreflightOptions}, design::{self, Design, Master, SlideLayout, ThemeFonts}, document::{self, Document, Transaction}, editing, execute_request, package::Package, theme_fonts::{FontGroup, FontScript, UnsetFont}};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

const THEME: &str = "ppt/theme/theme1.xml";
const CONTOSO: &str = "Contoso Display Semilight";
const EMPTY_SCRIPTS: &str = "<a:latin typeface=\"Contoso Display Semilight\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/>";

fn source(elements: Value) -> Document {
    let mut deck = editing::create("fonts".into(), "Fonts".into()).unwrap().deck;
    deck.design = Some(Design::default());
    deck.slides[0].elements = serde_json::from_value(elements).unwrap();
    document::create("fonts".into(), deck, vec![], vec![], None).unwrap()
}

fn save(document: &Document) -> Vec<u8> {
    STANDARD.decode(document::export_presentation(document).unwrap()["base64"].as_str().unwrap()).unwrap()
}

fn open(bytes: Vec<u8>) -> Document {
    serde_json::from_value(document::open_presentation("fonts".into(), bytes).unwrap()["document"].clone()).unwrap()
}

fn change(document: &Document, deck: aislide_core::model::Deck) -> Document {
    document::transact(document, Transaction { expected_revision: document.revision, expected_hash: document.hash.clone(), operations: serde_json::from_value(json!([{"op":"replace","path":"/deck","value":deck}])).unwrap() }).unwrap().document
}

/// A package of `elements` whose theme has the given font collections.
fn package_with(elements: Value, major: &str, minor: &str) -> Package {
    let mut package = Package::open(save(&source(elements))).unwrap();
    let xml = package.text(THEME).unwrap().to_owned();
    let start = xml.find("<a:fontScheme").unwrap();
    let opened = start + xml[start..].find('>').unwrap() + 1;
    let closed = xml.find("</a:fontScheme>").unwrap();
    package.replace_part(THEME, format!("{}<a:majorFont>{major}</a:majorFont><a:minorFont>{minor}</a:minorFont>{}", &xml[..opened], &xml[closed..]).into_bytes()).unwrap();
    package
}

fn opened(major: &str, minor: &str) -> Document { open(package_with(json!([]), major, minor).save().unwrap()) }

fn theme_of(document: &Document) -> aislide_core::design::Theme { document.deck.design.as_ref().unwrap().theme.clone() }

fn exported_theme(document: &Document) -> String { Package::open(save(document)).unwrap().text(THEME).unwrap().to_owned() }

fn collections(xml: &str) -> Vec<Vec<String>> {
    let parsed = roxmltree::Document::parse(xml).unwrap();
    parsed.descendants().filter(|node| matches!(node.tag_name().name(), "majorFont" | "minorFont"))
        .map(|node| node.children().filter(|child| child.is_element()).map(|child| format!("{}={}", child.tag_name().name(), child.attribute("typeface").unwrap_or(""))).collect()).collect()
}

#[test]
fn issue78_empty_theme_fonts_are_reported_as_unset_and_survive_an_unrelated_export() {
    let package = package_with(json!([]), EMPTY_SCRIPTS, EMPTY_SCRIPTS);
    let original = package.part(THEME).unwrap().to_vec();
    let document = open(package.save().unwrap());
    let fonts = &document.deck.design.as_ref().unwrap().theme.fonts;
    // The model still carries the fonts AISlide draws with, so existing readers see what they always saw.
    assert_eq!((fonts.major.as_str(), fonts.minor.as_str(), fonts.east_asian.as_str(), fonts.complex_script.as_str()), (CONTOSO, CONTOSO, "Yu Gothic", "Arial"));
    assert_eq!(serde_json::to_value(fonts).unwrap()["unset"], json!({"major":{"east_asian":"empty","complex_script":"empty"},"minor":{"east_asian":"empty","complex_script":"empty"}}));
    for group in FontGroup::ALL {
        assert_eq!(fonts.unset.state(group, FontScript::Latin), None);
        assert_eq!(fonts.unset.state(group, FontScript::EastAsian), Some(UnsetFont::Empty));
    }
    let mut deck = document.deck.clone(); deck.title = "Renamed".into();
    let output = Package::open(save(&change(&document, deck))).unwrap();
    assert_eq!(output.part(THEME).unwrap(), original.as_slice(), "an unrelated edit must leave the package's own empty slots alone");
}

#[test]
fn issue78_a_theme_that_specifies_every_slot_serializes_exactly_as_before() {
    let fonts = &Design::default().theme.fonts;
    assert!(fonts.unset.is_empty());
    assert_eq!(serde_json::to_value(fonts).unwrap(), json!({"major":"Aptos","minor":"Aptos","east_asian":"Yu Gothic","complex_script":"Arial"}));
    let document = opened("<a:latin typeface=\"A\"/><a:ea typeface=\"B\"/><a:cs typeface=\"C\"/>", "<a:latin typeface=\"D\"/><a:ea typeface=\"E\"/><a:cs typeface=\"F\"/>");
    let value = serde_json::to_value(&theme_of(&document).fonts).unwrap();
    assert_eq!(value, json!({"major":"A","minor":"D","east_asian":"E","complex_script":"F"}));
}

#[test]
fn issue78_apply_theme_keeps_unset_slots_that_the_caller_sends_back_unchanged() {
    let document = opened(EMPTY_SCRIPTS, EMPTY_SCRIPTS);
    let mut theme = theme_of(&document);
    theme.colors.insert("accent1".into(), "ABC123".into());
    let deck = design::apply_theme(document.deck.clone(), theme).unwrap();
    assert!(!deck.design.as_ref().unwrap().theme.fonts.unset.is_empty());
    let xml = exported_theme(&change(&document, deck));
    assert!(xml.contains("ABC123"));
    assert_eq!(collections(&xml), vec![vec![format!("latin={CONTOSO}"), "ea=".to_owned(), "cs=".to_owned()]; 2]);
}

#[test]
fn issue78_apply_theme_writes_the_value_the_document_showed_when_the_slot_was_empty() {
    // The issue's step 8: the same fonts get_document showed, sent without the unset marker, are explicit and written.
    let document = opened(EMPTY_SCRIPTS, EMPTY_SCRIPTS);
    let mut theme = theme_of(&document);
    theme.fonts = ThemeFonts::new(CONTOSO, CONTOSO, "Yu Gothic", "Arial");
    let deck = design::apply_theme(document.deck.clone(), theme).unwrap();
    // The slots stay recorded as named, so that this deck is not mistaken for one saved before the markers existed.
    let fonts = &deck.design.as_ref().unwrap().theme.fonts;
    for group in FontGroup::ALL {
        assert_eq!(fonts.unset.state(group, FontScript::Latin), None);
        assert_eq!(fonts.unset.state(group, FontScript::EastAsian), Some(UnsetFont::Written));
        assert_eq!(fonts.unset.state(group, FontScript::ComplexScript), Some(UnsetFont::Written));
    }
    assert_eq!(serde_json::to_value(fonts).unwrap()["unset"]["minor"]["east_asian"], "written");
    let changed = change(&document, deck);
    let xml = exported_theme(&changed);
    assert_eq!(collections(&xml), vec![vec![format!("latin={CONTOSO}"), "ea=Yu Gothic".to_owned(), "cs=Arial".to_owned()]; 2]);
    // Reopened, nothing is unset any more.
    let reopened = open(save(&changed));
    assert!(theme_of(&reopened).fonts.unset.is_empty());
}

#[test]
fn issue78_naming_one_slot_does_not_fill_the_slots_the_caller_left_unspecified() {
    // The issue's step 9: Meiryo UI for East Asian; the complex-script font stays empty because the unset marker came back unchanged.
    let document = opened(EMPTY_SCRIPTS, EMPTY_SCRIPTS);
    let mut theme = theme_of(&document);
    theme.fonts.east_asian = "Meiryo UI".into();
    let deck = design::apply_theme(document.deck.clone(), theme).unwrap();
    let fonts = &deck.design.as_ref().unwrap().theme.fonts;
    assert_eq!(fonts.unset.state(FontGroup::Minor, FontScript::EastAsian), Some(UnsetFont::Written));
    assert_eq!(fonts.unset.state(FontGroup::Major, FontScript::EastAsian), Some(UnsetFont::Written));
    assert_eq!(fonts.unset.state(FontGroup::Minor, FontScript::ComplexScript), Some(UnsetFont::Empty));
    let xml = exported_theme(&change(&document, deck));
    assert_eq!(collections(&xml), vec![vec![format!("latin={CONTOSO}"), "ea=Meiryo UI".to_owned(), "cs=".to_owned()]; 2]);
}

#[test]
fn issue78_a_marker_does_not_override_a_font_name_the_caller_changed_or_invent_one() {
    let document = opened(EMPTY_SCRIPTS, "<a:latin typeface=\"Body\"/><a:ea typeface=\"Yu Gothic\"/><a:cs typeface=\"\"/>");
    let mut theme = theme_of(&document);
    // Marking a slot the document specified, or editing a slot while keeping its marker, are both explicit edits.
    theme.fonts.unset.minor.complex_script = Some(UnsetFont::Empty);
    theme.fonts.unset.minor.east_asian = Some(UnsetFont::Empty);
    let deck = design::apply_theme(document.deck.clone(), theme).unwrap();
    let fonts = &deck.design.as_ref().unwrap().theme.fonts;
    assert_eq!(fonts.unset.state(FontGroup::Minor, FontScript::EastAsian), None, "a specified slot cannot be made unset by a marker");
    assert_eq!(fonts.unset.state(FontGroup::Minor, FontScript::ComplexScript), Some(UnsetFont::Empty));
}

#[test]
fn issue78_set_master_theme_writes_explicit_values_and_keeps_returned_markers() {
    let document = opened(EMPTY_SCRIPTS, EMPTY_SCRIPTS);
    let echoed = theme_of(&document);
    let unchanged = design::set_master_theme(document.deck.clone(), "master-1", Some(echoed.clone())).unwrap();
    assert_eq!(exported_theme(&change(&document, unchanged)), Package::open(save(&document)).unwrap().text(THEME).unwrap());
    let mut explicit = echoed;
    explicit.fonts = ThemeFonts::new(CONTOSO, CONTOSO, "Yu Gothic", "Arial");
    let deck = design::set_master_theme(document.deck.clone(), "master-1", Some(explicit)).unwrap();
    assert_eq!(collections(&exported_theme(&change(&document, deck))), vec![vec![format!("latin={CONTOSO}"), "ea=Yu Gothic".to_owned(), "cs=Arial".to_owned()]; 2]);
}

/// A package whose two masters share one theme part with empty East Asian and complex-script slots, and the document opened from it.
fn shared_theme() -> (Package, Document) {
    let mut deck = source(json!([])).deck;
    let design = deck.design.as_mut().unwrap();
    design.masters.push(Master { id: "second".into(), name: "Second".into(), background: "@lt1".into(), elements: vec![], theme: None });
    design.layouts.push(SlideLayout { id: "second-layout".into(), name: "Second".into(), master_id: "second".into(), source_id: None, background: None, elements: vec![] });
    let mut package = Package::open(aislide_core::pptx::export_pptx(&deck).unwrap()).unwrap();
    let rel = "ppt/slideMasters/_rels/slideMaster2.xml.rels";
    package.replace_part(rel, package.text(rel).unwrap().replace("theme3.xml", "theme1.xml").into_bytes()).unwrap();
    let xml = package.text(THEME).unwrap().to_owned();
    let start = xml.find("<a:fontScheme").unwrap();
    let opened_at = start + xml[start..].find('>').unwrap() + 1;
    let closed = xml.find("</a:fontScheme>").unwrap();
    package.replace_part(THEME, format!("{}<a:majorFont>{EMPTY_SCRIPTS}</a:majorFont><a:minorFont>{EMPTY_SCRIPTS}</a:minorFont>{}", &xml[..opened_at], &xml[closed..]).into_bytes()).unwrap();
    let document = open(package.save().unwrap());
    (package, document)
}

#[test]
fn issue78_a_shared_theme_is_cloned_before_an_explicit_value_is_written_to_one_owner() {
    let (package, document) = shared_theme();
    let mut theme = theme_of(&document);
    theme.fonts = ThemeFonts::new(CONTOSO, CONTOSO, "Meiryo UI", "Arial");
    let owner = document.deck.design.as_ref().unwrap().masters[1].id.clone();
    let output = Package::open(save(&change(&document, design::set_master_theme(document.deck.clone(), &owner, Some(theme)).unwrap()))).unwrap();
    assert_eq!(output.part(THEME).unwrap(), package.part(THEME).unwrap(), "the other owner keeps its empty slots");
    let clone = output.parts().keys().find(|path| path.starts_with("ppt/theme/aislide-theme-")).unwrap();
    assert_eq!(collections(output.text(clone).unwrap()), vec![vec![format!("latin={CONTOSO}"), "ea=Meiryo UI".to_owned(), "cs=Arial".to_owned()]; 2]);
}

#[test]
fn issue78_a_deck_without_markers_does_not_split_a_shared_theme_part() {
    let (package, document) = shared_theme();
    let mut deck = document.deck.clone();
    deck.design.as_mut().unwrap().theme.fonts.unset = Default::default();
    deck.title = "Renamed".into();
    let output = Package::open(save(&change(&document, deck))).unwrap();
    assert_eq!(output.part(THEME).unwrap(), package.part(THEME).unwrap());
    assert!(output.parts().keys().all(|path| !path.starts_with("ppt/theme/aislide-theme-")), "nothing was edited in the theme, so no copy of it is made: {:?}", output.parts().keys().collect::<Vec<_>>());
}

#[test]
fn issue78_naming_the_stand_in_in_a_notes_master_theme_is_saved_and_echoing_the_markers_is_not() {
    const NOTES_THEME: &str = "ppt/theme/theme2.xml";
    let mut deck = source(json!([])).deck;
    let theme = deck.design.as_ref().unwrap().theme.clone();
    deck.auxiliary_design = Some(aislide_core::model::AuxiliaryDesign { width: 720, height: 960, handout_master: None,
        notes_master: Some(aislide_core::model::AuxiliaryMaster { name: "Notes".into(), background: "@lt1".into(), theme, elements: vec![] }) });
    let mut package = Package::open(aislide_core::pptx::export_pptx(&deck).unwrap()).unwrap();
    let xml = package.text(NOTES_THEME).unwrap().to_owned();
    let start = xml.find("<a:fontScheme").unwrap();
    let opened_at = start + xml[start..].find('>').unwrap() + 1;
    let closed = xml.find("</a:fontScheme>").unwrap();
    package.replace_part(NOTES_THEME, format!("{}<a:majorFont>{EMPTY_SCRIPTS}</a:majorFont><a:minorFont>{EMPTY_SCRIPTS}</a:minorFont>{}", &xml[..opened_at], &xml[closed..]).into_bytes()).unwrap();
    let document = open(package.save().unwrap());
    let design = document.deck.auxiliary_design.clone().unwrap();
    assert_eq!(design.notes_master.as_ref().unwrap().theme.fonts.unset.state(FontGroup::Minor, FontScript::EastAsian), Some(UnsetFont::Empty));
    let notes_theme = |document: &Document| collections(Package::open(save(document)).unwrap().text(NOTES_THEME).unwrap());
    let empty = vec![vec![format!("latin={CONTOSO}"), "ea=".to_owned(), "cs=".to_owned()]; 2];
    // The markers sent back unchanged keep the file's empty slots.
    let echoed = aislide_core::authoring_ops::update_auxiliary_design(&document, document.revision, design.clone()).unwrap().document;
    assert_eq!(notes_theme(&echoed), empty);
    // The fonts the document showed, sent without the markers, name the slots.
    let mut named = design;
    named.notes_master.as_mut().unwrap().theme.fonts = ThemeFonts::new(CONTOSO, CONTOSO, "Yu Gothic", "Arial");
    let named = aislide_core::authoring_ops::update_auxiliary_design(&document, document.revision, named).unwrap().document;
    assert_eq!(notes_theme(&named), vec![vec![format!("latin={CONTOSO}"), "ea=Yu Gothic".to_owned(), "cs=Arial".to_owned()]; 2]);
}

#[test]
fn issue78_an_omitted_font_element_is_reported_and_inserted_in_schema_order_when_named() {
    let document = opened("<a:latin typeface=\"X\"/><a:cs typeface=\"\"/>", "<a:latin typeface=\"X\"/><a:ea typeface=\"\"/><a:font script=\"Jpan\" typeface=\"Yu Gothic\"/>");
    let fonts = theme_of(&document).fonts;
    assert_eq!(fonts.unset.state(FontGroup::Major, FontScript::EastAsian), Some(UnsetFont::Absent));
    assert_eq!(fonts.unset.state(FontGroup::Minor, FontScript::ComplexScript), Some(UnsetFont::Absent));
    assert_eq!(serde_json::to_value(&fonts).unwrap()["unset"]["minor"]["script_fonts"], json!({"Jpan":"Yu Gothic"}));
    let mut theme = theme_of(&document);
    theme.fonts = ThemeFonts::new("X", "X", "Yu Gothic", "Arial");
    let xml = exported_theme(&change(&document, design::apply_theme(document.deck.clone(), theme).unwrap()));
    assert_eq!(collections(&xml), vec![
        vec!["latin=X".to_owned(), "ea=Yu Gothic".to_owned(), "cs=Arial".to_owned()],
        vec!["latin=X".to_owned(), "ea=Yu Gothic".to_owned(), "cs=Arial".to_owned(), "font=Yu Gothic".to_owned()],
    ]);
}

#[test]
fn issue78_preflight_warns_about_text_that_depends_on_an_empty_theme_font() {
    let elements = json!([
        {"type":"text","id":"jp","x":64,"y":80,"width":1000,"height":80,"text":"日本語の本文です。視認性を確かめる","font_size":40,"color":"@dk1","bold":false},
        {"type":"text","id":"jp-more","x":64,"y":200,"width":1000,"height":80,"text":"もう一つの日本語の本文","font_size":40,"color":"@dk1","bold":false},
        {"type":"text","id":"latin","x":64,"y":320,"width":1000,"height":80,"text":"Latin only text","font_size":40,"color":"@dk1","bold":false},
        {"type":"text","id":"own","x":64,"y":440,"width":1000,"height":80,"text":"自分のフォントを持つ日本語","font_size":40,"color":"@dk1","bold":false,"format":{"font_family":"Yu Gothic"}}
    ]);
    let document = open(package_with(elements.clone(), EMPTY_SCRIPTS, EMPTY_SCRIPTS).save().unwrap());
    let report = preflight_presentation(&document, &PreflightOptions::default()).unwrap();
    let findings: Vec<_> = report.findings.iter().filter(|finding| finding.code == "THEME_FONT_UNSET").collect();
    assert_eq!(findings.len(), 1, "one finding per page and message: {findings:?}");
    let finding = findings[0];
    assert_eq!(finding.severity, "warning");
    assert_eq!(finding.element_ids, vec!["jp".to_owned(), "jp-more".to_owned()]);
    assert!(finding.message.contains("minor East Asian") && finding.message.contains("Yu Gothic") && finding.message.contains("apply_theme"), "{}", finding.message);
    assert!(report.findings.iter().all(|finding| finding.code != "THEME_FONT_UNSET" || !finding.element_ids.contains(&"latin".to_owned())));
    // The preview reports the same text once for the page.
    let preview = aislide_core::export_static::preview_presentation(&document, &Default::default()).unwrap();
    let notices: Vec<_> = preview.warnings.iter().filter(|warning| warning.code == "THEME_FONT_UNSET").collect();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].element_ids, vec!["jp".to_owned(), "jp-more".to_owned()]);

    // A script-specific font in the font scheme gives Japanese text its font while the slot is empty, so PowerPoint does not choose.
    let covered = format!("{EMPTY_SCRIPTS}<a:font script=\"Jpan\" typeface=\"Yu Gothic\"/>");
    let document = open(package_with(elements.clone(), &covered, &covered).save().unwrap());
    let report = preflight_presentation(&document, &PreflightOptions::default()).unwrap();
    assert!(report.findings.iter().all(|finding| finding.code != "THEME_FONT_UNSET"), "{:?}", report.findings);

    // A specified East Asian font reports nothing either.
    let specified = "<a:latin typeface=\"Contoso Display Semilight\"/><a:ea typeface=\"Yu Gothic\"/><a:cs typeface=\"\"/>";
    let document = open(package_with(elements.clone(), specified, specified).save().unwrap());
    assert!(preflight_presentation(&document, &PreflightOptions::default()).unwrap().findings.iter().all(|finding| finding.code != "THEME_FONT_UNSET"));

    // Only the heading slot is empty: body text is fine, a heading that uses it is reported.
    let heading = json!([
        {"type":"text","id":"body","x":64,"y":80,"width":1000,"height":80,"text":"本文の日本語","font_size":40,"color":"@dk1","bold":false,"format":{"font_family":"@minor"}},
        {"type":"text","id":"title","x":64,"y":200,"width":1000,"height":80,"text":"見出しの日本語","font_size":40,"color":"@dk1","bold":true,"format":{"font_family":"@major"}}
    ]);
    let major_empty = "<a:latin typeface=\"Contoso Display Semilight\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/>";
    let document = open(package_with(heading, major_empty, specified).save().unwrap());
    let report = preflight_presentation(&document, &PreflightOptions::default()).unwrap();
    let ids: Vec<_> = report.findings.iter().filter(|finding| finding.code == "THEME_FONT_UNSET").flat_map(|finding| finding.element_ids.clone()).collect();
    assert_eq!(ids, vec!["title".to_owned()]);
}

#[test]
fn issue78_inspect_template_and_design_tokens_report_each_slot_with_availability() {
    let package = package_with(json!([]), "<a:latin typeface=\"Contoso Display Semilight\"/><a:ea typeface=\"Yu Mincho\"/><a:cs typeface=\"\"/>", &format!("{EMPTY_SCRIPTS}<a:font script=\"Jpan\" typeface=\"Yu Gothic\"/>"));
    let document = open(package.save().unwrap());
    let potx = export_template(&document, TemplateKind::Potx).unwrap();
    let inspection = execute_request(json!({"op":"inspect_template","kind":"potx","base64":STANDARD.encode(&potx)})).unwrap();
    let reports = inspection["theme_fonts"].as_array().unwrap();
    assert_eq!(reports.len(), inspection["masters"].as_array().unwrap().len());
    let report = &reports[0];
    assert_eq!(report["master_id"], inspection["masters"][0]["id"]);
    // The package's own typefaces, per collection, including the heading's Yu Mincho that the one-font model does not carry.
    assert_eq!(report["major"]["latin"]["font"], CONTOSO);
    assert_eq!(report["major"]["latin"]["state"], "specified");
    assert_eq!(report["major"]["latin"]["installed"], false, "{report}");
    assert_eq!(report["major"]["east_asian"]["font"], "Yu Mincho");
    assert_eq!(report["major"]["east_asian"]["state"], "specified");
    assert_eq!(report["major"]["complex_script"]["state"], "empty");
    assert_eq!(report["major"]["complex_script"]["font"], "Arial");
    assert_eq!(report["minor"]["east_asian"]["state"], "empty");
    assert_eq!(report["minor"]["east_asian"]["font"], "Yu Gothic");
    assert_eq!(report["minor"]["script_fonts"], json!({"Jpan":"Yu Gothic"}));
    assert!(report["major"].get("script_fonts").is_none());
    assert!(report["notes"].as_array().unwrap().iter().any(|note| note.as_str().unwrap().starts_with("minor.east_asian is empty in the file")));

    // Importing the template keeps the empty slots; the document and design_tokens report them.
    let imported: Document = serde_json::from_value(execute_request(json!({"op":"import_template","id":"imported","kind":"potx","base64":STANDARD.encode(&potx)})).unwrap()).unwrap();
    let theme = theme_of(&imported);
    assert_eq!(theme.fonts.unset.state(FontGroup::Minor, FontScript::EastAsian), Some(UnsetFont::Empty));
    assert_eq!(theme.fonts.unset.state(FontGroup::Major, FontScript::EastAsian), None);
    let tokens = execute_request(json!({"op":"design_tokens","deck":imported.deck})).unwrap();
    let themes = tokens["theme_fonts"].as_array().unwrap();
    assert_eq!(themes[0]["scope"], "default");
    assert_eq!(themes[0]["minor"]["east_asian"]["state"], "empty");
    assert!(!themes[0]["notes"].as_array().unwrap().is_empty());
    let plain = execute_request(json!({"op":"design_tokens","deck":source(json!([])).deck})).unwrap();
    assert_eq!(plain["theme_fonts"][0]["notes"], json!([]));
    assert_eq!(plain["theme_fonts"][0]["minor"]["east_asian"]["state"], "specified");
}

#[test]
fn issue78_a_theme_file_keeps_its_empty_slots_through_export_and_import() {
    let document = opened(EMPTY_SCRIPTS, &format!("{EMPTY_SCRIPTS}<a:font script=\"Jpan\" typeface=\"Yu Gothic\"/>"));
    let theme = theme_of(&document);
    let thmx = export_template(&document, TemplateKind::Thmx).unwrap();
    let loaded = aislide_core::templates::load_thmx(thmx).unwrap();
    assert_eq!(serde_json::to_value(&loaded.fonts).unwrap(), serde_json::to_value(&theme.fonts).unwrap());
}

#[test]
fn issue78_a_deck_saved_before_markers_existed_leaves_the_package_alone() {
    let package = package_with(json!([]), EMPTY_SCRIPTS, EMPTY_SCRIPTS);
    let original = package.part(THEME).unwrap().to_vec();
    let document = open(package.save().unwrap());
    // What a build without markers wrote for this deck: the four names and nothing else.
    let mut deck = document.deck.clone();
    deck.design.as_mut().unwrap().theme.fonts.unset = Default::default();
    deck.title = "Renamed".into();
    let output = Package::open(save(&change(&document, deck))).unwrap();
    assert_eq!(output.part(THEME).unwrap(), original.as_slice(), "no marker is not a request to write the stand-ins");
    // With nothing else changed, the file comes back as it was opened.
    let mut unchanged = document.deck.clone();
    unchanged.design.as_mut().unwrap().theme.fonts.unset = Default::default();
    assert_eq!(save(&change(&document, unchanged)), save(&document));
    // A name that changed is still written.
    let mut deck = document.deck.clone();
    let fonts = &mut deck.design.as_mut().unwrap().theme.fonts;
    fonts.unset = Default::default();
    fonts.east_asian = "Meiryo UI".into();
    let xml = exported_theme(&change(&document, deck));
    assert_eq!(collections(&xml), vec![vec![format!("latin={CONTOSO}"), "ea=Meiryo UI".to_owned(), "cs=".to_owned()]; 2]);
}

#[test]
fn issue78_a_marker_on_a_slot_the_file_specifies_cannot_empty_it() {
    let specified = "<a:latin typeface=\"Body\"/><a:ea typeface=\"Meiryo\"/><a:cs typeface=\"Arial\"/>";
    let document = opened(specified, specified);
    let mut deck = document.deck.clone();
    deck.title = "Renamed".into();
    let unset = &mut deck.design.as_mut().unwrap().theme.fonts.unset;
    unset.minor.east_asian = Some(UnsetFont::Empty);
    unset.minor.latin = Some(UnsetFont::Written);
    let output = Package::open(save(&change(&document, deck))).unwrap();
    assert_eq!(collections(output.text(THEME).unwrap()), vec![vec!["latin=Body".to_owned(), "ea=Meiryo".to_owned(), "cs=Arial".to_owned()]; 2]);
}

#[test]
fn issue78_a_master_added_through_update_design_keeps_the_slots_its_source_left_empty() {
    let specified = "<a:latin typeface=\"Body\"/><a:ea typeface=\"Meiryo\"/><a:cs typeface=\"Arial\"/>";
    let document = opened(specified, specified);
    let mut design = document.deck.design.clone().unwrap();
    let mut theme = theme_of(&document);
    theme.fonts.unset.minor.east_asian = Some(UnsetFont::Empty);
    design.masters.push(Master { id: "imported".into(), name: "Imported".into(), background: "@lt1".into(), elements: vec![], theme: Some(theme) });
    design.layouts.push(SlideLayout { id: "imported-layout".into(), name: "Imported".into(), master_id: "imported".into(), source_id: None, background: None, elements: vec![] });
    let deck = design::update_design(document.deck.clone(), design).unwrap();
    let design = deck.design.as_ref().unwrap();
    let imported = design.masters.iter().find(|master| master.id == "imported").and_then(|master| master.theme.as_ref()).unwrap();
    // The default theme specified this slot, but the new master has no earlier theme to be compared with.
    assert_eq!(imported.fonts.unset.state(FontGroup::Minor, FontScript::EastAsian), Some(UnsetFont::Empty));
    assert!(design.theme.fonts.unset.is_empty());
}

#[test]
fn issue78_naming_slots_of_a_font_collection_without_children_fills_it_in_one_step() {
    for markup in ["<a:majorFont></a:majorFont>", "<a:majorFont/>"] {
        let mut package = package_with(json!([]), "", "");
        let xml = package.text(THEME).unwrap().replace("<a:majorFont></a:majorFont>", markup).replace("<a:minorFont></a:minorFont>", &markup.replace("major", "minor"));
        package.replace_part(THEME, xml.into_bytes()).unwrap();
        let document = open(package.save().unwrap());
        let fonts = theme_of(&document).fonts;
        for group in FontGroup::ALL { for script in FontScript::ALL { assert_eq!(fonts.unset.state(group, script), Some(UnsetFont::Absent), "{markup}"); } }
        let mut theme = theme_of(&document);
        theme.fonts = ThemeFonts::new("Aptos", "Aptos", "Yu Gothic", "Arial");
        let output = exported_theme(&change(&document, design::apply_theme(document.deck.clone(), theme).unwrap()));
        assert_eq!(collections(&output), vec![vec!["latin=Aptos".to_owned(), "ea=Yu Gothic".to_owned(), "cs=Arial".to_owned()]; 2], "{markup}");
    }
}

#[test]
fn issue78_replacing_the_stand_in_name_names_a_font_for_the_empty_slot() {
    let document = opened(EMPTY_SCRIPTS, EMPTY_SCRIPTS);
    let deck = aislide_core::text_ops::replace_font(document.deck.clone(), "Yu Gothic", "Meiryo UI").unwrap();
    assert_eq!(deck.design.as_ref().unwrap().theme.fonts.unset.state(FontGroup::Minor, FontScript::EastAsian), Some(UnsetFont::Written));
    let xml = exported_theme(&change(&document, deck));
    assert_eq!(collections(&xml), vec![vec![format!("latin={CONTOSO}"), "ea=Meiryo UI".to_owned(), "cs=".to_owned()]; 2]);
}

#[test]
fn issue78_a_slot_an_edit_named_is_no_longer_reported_as_unset() {
    let elements = json!([{"type":"text","id":"jp","x":64,"y":80,"width":1000,"height":80,"text":"日本語の本文です","font_size":40,"color":"@dk1","bold":false}]);
    let document = open(package_with(elements, EMPTY_SCRIPTS, EMPTY_SCRIPTS).save().unwrap());
    let unset = |document: &Document| preflight_presentation(document, &PreflightOptions::default()).unwrap().findings.iter().filter(|finding| finding.code == "THEME_FONT_UNSET").count();
    assert_eq!(unset(&document), 1);
    // Name the East Asian slots with the stand-in's own name: the file then names the font, so PowerPoint does not choose.
    let mut theme = theme_of(&document);
    theme.fonts.unset.major.east_asian = None;
    theme.fonts.unset.minor.east_asian = None;
    let named = change(&document, design::apply_theme(document.deck.clone(), theme).unwrap());
    assert_eq!(unset(&named), 0);
    let report = execute_request(json!({"op":"design_tokens","deck":named.deck})).unwrap();
    assert_eq!(report["theme_fonts"][0]["minor"]["east_asian"]["state"], "specified");
    assert_eq!(report["theme_fonts"][0]["minor"]["complex_script"]["state"], "empty");
    assert_eq!(report["theme_fonts"][0]["notes"].as_array().unwrap().len(), 2, "only the two complex-script slots are still empty: {report}");
}
