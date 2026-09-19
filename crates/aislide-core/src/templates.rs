use crate::{design::{Design, Theme}, document::{self, Document}, native::{A, R, REL}, native_save::{apply, set_attribute}, package::Package, pptx::{parse, relationship_targets}, Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::collections::{BTreeMap, BTreeSet};

const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const PPTX: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";
const POTX: &str = "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml";
const THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";
const MANAGER: &str = "application/vnd.openxmlformats-officedocument.themeManager+xml";

fn safe(package: &Package) -> Result<()> {
    if package.part_names().iter().any(|path| {
        let path = path.to_ascii_lowercase();
        path.ends_with("vbaproject.bin") || path.starts_with("_xmlsignatures/") || path.ends_with("/encryptedpackage") || path.ends_with("/encryptioninfo")
    }) { return Err(Error::Unsupported("macro, signed or protected template conversion is not supported".into())); }
    let parsed = parse(package.text("[Content_Types].xml")?)?;
    if !parsed.root_element().has_tag_name((CT, "Types")) { return Err(Error::Invalid("template content types root".into())); }
    let mut parts = BTreeSet::new();
    for node in parsed.root_element().children().filter(|node| node.is_element()) {
        if let Some(path) = node.attribute("PartName") { if !parts.insert(path.to_ascii_lowercase()) { return Err(Error::Invalid("duplicate template content type override".into())); } }
        let kind = node.attribute("ContentType").unwrap_or("").to_ascii_lowercase();
        if kind.contains("macroenabled") || kind.contains("vbaproject") || kind.contains("digital-signature") { return Err(Error::Unsupported("macro or signed template format".into())); }
    }
    Ok(())
}

fn content_type(package: &Package, part: &str) -> Result<String> {
    let parsed = parse(package.text("[Content_Types].xml")?)?;
    parsed.root_element().children().find(|node| node.has_tag_name((CT, "Override")) && node.attribute("PartName") == Some(format!("/{part}").as_str()))
        .and_then(|node| node.attribute("ContentType")).map(str::to_owned).ok_or_else(|| Error::Unsupported("declared template main/theme content type missing".into()))
}

fn main_part(package: &Package) -> Result<String> {
    let roots = relationship_targets(package, "", "officeDocument")?;
    if roots.len() != 1 { return Err(Error::Invalid("template requires exactly one declared officeDocument".into())); }
    roots.into_values().next().ok_or_else(|| Error::Invalid("template main part missing".into()))
}

fn convert(mut package: Package, expected: &str, target: &str) -> Result<Vec<u8>> {
    safe(&package)?;
    let main = main_part(&package)?;
    if content_type(&package, &main)? != expected { return Err(Error::Unsupported("template factory requires the explicitly selected non-macro format".into())); }
    let xml = package.text("[Content_Types].xml")?.to_owned(); let parsed = parse(&xml)?;
    let entry = parsed.root_element().children().find(|node| node.has_tag_name((CT, "Override")) && node.attribute("PartName") == Some(format!("/{main}").as_str())).ok_or_else(|| Error::Invalid("template main content type missing".into()))?;
    let mut edits = Vec::new(); set_attribute(&xml, entry, "ContentType", target, &mut edits)?;
    package.replace_part("[Content_Types].xml", apply(xml, edits)?)?;
    package.save()
}

pub fn load_potx(id: String, bytes: Vec<u8>) -> Result<Document> {
    let pptx = convert(Package::open(bytes)?, POTX, PPTX)?;
    let result = document::open_presentation(id, pptx)?;
    Ok(serde_json::from_value(result["document"].clone())?)
}

pub fn export_potx(document: &Document) -> Result<Vec<u8>> {
    let result = document::export_presentation(document)?;
    let bytes = STANDARD.decode(result["base64"].as_str().ok_or_else(|| Error::Invalid("presentation export bytes missing".into()))?).map_err(|_| Error::Invalid("presentation export encoding".into()))?;
    convert(Package::open(bytes)?, PPTX, POTX)
}

pub fn load_thmx(bytes: Vec<u8>) -> Result<Theme> {
    let package = Package::open(bytes)?;
    safe(&package)?;
    let main = main_part(&package)?;
    let path = match content_type(&package, &main)?.as_str() {
        THEME => main,
        MANAGER => {
            let parsed = parse(package.text(&main)?)?;
            if !parsed.root_element().has_tag_name((A, "themeManager")) { return Err(Error::Unsupported("THMX theme manager root".into())); }
            let targets = relationship_targets(&package, &main, "theme")?;
            if targets.len() != 1 { return Err(Error::Unsupported("THMX requires exactly one declared theme".into())); }
            targets.into_values().next().ok_or_else(|| Error::Invalid("THMX theme missing".into()))?
        }
        _ => return Err(Error::Unsupported("not an explicitly declared non-macro THMX theme".into())),
    };
    if content_type(&package, &path)? != THEME || !parse(package.text(&path)?)?.root_element().has_tag_name((A, "theme")) { return Err(Error::Unsupported("THMX theme part type".into())); }
    crate::native::read_theme(&package, &path)
}

pub fn export_thmx(theme: &Theme) -> Result<Vec<u8>> {
    crate::design::validate_theme(theme)?;
    let mut deck = crate::editing::create("theme-export".into(), theme.name.clone())?.deck;
    let mut design = Design::default(); design.layouts.truncate(1); design.theme = theme.clone(); deck.design = Some(design);
    let generated = Package::open(crate::pptx::export_pptx(&deck)?)?;
    let parts = BTreeMap::from([
        ("[Content_Types].xml".into(), format!("<Types xmlns=\"{CT}\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Override PartName=\"/theme/theme/themeManager.xml\" ContentType=\"{MANAGER}\"/><Override PartName=\"/theme/theme/theme1.xml\" ContentType=\"{THEME}\"/></Types>").into_bytes()),
        ("_rels/.rels".into(), format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"rId1\" Type=\"{R}/officeDocument\" Target=\"theme/theme/themeManager.xml\"/></Relationships>").into_bytes()),
        ("theme/theme/themeManager.xml".into(), format!("<a:themeManager xmlns:a=\"{A}\"/>").into_bytes()),
        ("theme/theme/_rels/themeManager.xml.rels".into(), format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"rId1\" Type=\"{R}/theme\" Target=\"theme1.xml\"/></Relationships>").into_bytes()),
        ("theme/theme/theme1.xml".into(), generated.part("ppt/theme/theme1.xml")?.to_vec()),
    ]);
    Package::from_parts(parts)?.save()
}