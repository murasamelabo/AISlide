use aislide_core::{design::{Design, Theme}, document, editing, package::Package, templates};
use base64::{Engine, engine::general_purpose::STANDARD};

#[test]
fn potx_factory_preserves_opaque_content_and_produces_a_pptx_document() {
    let mut deck = editing::create("template-source".into(), "Template".into()).unwrap().deck;
    deck.design = Some(Design::default());
    let source = document::create("template-source".into(), deck, vec![], vec![], None).unwrap();
    let potx = templates::export_potx(&source).unwrap();
    let mut parts = Package::open(potx).unwrap().parts().clone();
    parts.insert("vendor/opaque.bin".into(), vec![11, 22, 33]);
    let potx = Package::from_parts(parts).unwrap().save().unwrap();
    assert!(document::open_presentation("ordinary-open".into(), potx.clone()).is_err());
    let created = templates::load_potx("from-template".into(), potx.clone()).unwrap();
    assert_eq!(created.id, "from-template");
    assert_eq!(created.deck.design.as_ref().unwrap().layouts.len(), 4);
    let output = document::export_presentation(&created).unwrap();
    let pptx = STANDARD.decode(output["base64"].as_str().unwrap()).unwrap();
    let package = Package::open(pptx.clone()).unwrap();
    assert!(package.text("[Content_Types].xml").unwrap().contains("presentationml.presentation.main+xml"));
    assert_eq!(package.part("vendor/opaque.bin").unwrap(), &[11, 22, 33]);
    document::open_presentation("ordinary-open".into(), pptx).unwrap();
    let exported = Package::open(templates::export_potx(&created).unwrap()).unwrap();
    assert!(exported.text("[Content_Types].xml").unwrap().contains("presentationml.template.main+xml"));
    assert_eq!(exported.part("vendor/opaque.bin").unwrap(), &[11, 22, 33]);
}

#[test]
fn thmx_palette_and_fonts_roundtrip_via_declared_theme_manager() {
    let mut theme = Theme::default();
    theme.name = "Brand & Studio".into();
    theme.colors.insert("accent1".into(), "12AB45".into());
    theme.fonts.major = "Georgia".into();
    theme.fonts.minor = "Verdana".into();
    theme.fonts.east_asian = "Yu Mincho".into();
    let bytes = templates::export_thmx(&theme).unwrap();
    let package = Package::open(bytes.clone()).unwrap();
    assert!(package.text("[Content_Types].xml").unwrap().contains("themeManager+xml"));
    let loaded = templates::load_thmx(bytes).unwrap();
    assert_eq!(serde_json::to_value(loaded).unwrap(), serde_json::to_value(theme).unwrap());
}

#[test]
fn template_factories_reject_macros_protection_and_unsafe_theme_targets() {
    assert!(templates::load_potx("protected".into(), vec![0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]).is_err());
    assert!(templates::load_thmx(vec![0xd0, 0xcf, 0x11, 0xe0]).is_err());
    let source = editing::create("template-source".into(), "Template".into()).unwrap();
    let potx = templates::export_potx(&source).unwrap();
    let mut parts = Package::open(potx).unwrap().parts().clone();
    parts.insert("ppt/vbaProject.bin".into(), vec![1, 2, 3]);
    assert!(templates::load_potx("macros".into(), Package::from_parts(parts).unwrap().save().unwrap()).is_err());
    let theme = templates::export_thmx(&Theme::default()).unwrap();
    for target in ["../../../outside.xml", "https://example.invalid/theme.xml", "%2e%2e/%2e%2e/outside.xml"] {
        let mut package = Package::open(theme.clone()).unwrap();
        let path = "theme/theme/_rels/themeManager.xml.rels";
        let xml = package.text(path).unwrap().replace("Target=\"theme1.xml\"", &format!("Target=\"{target}\""));
        package.replace_part(path, xml.into_bytes()).unwrap();
        assert!(templates::load_thmx(package.save().unwrap()).is_err(), "{target}");
    }
}