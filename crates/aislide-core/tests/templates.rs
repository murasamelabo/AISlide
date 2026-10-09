use aislide_core::{design::{Design, Theme}, document, editing, package::Package, templates};
use base64::{Engine, engine::general_purpose::STANDARD};

const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const PROVENANCE: &str = "urn:aislide:provenance:1";

#[test]
fn review19_inert_external_hyperlinks_allow_inspection_and_selected_import() {
    let url = "https://www.example.com/contact";
    let mut deck = editing::create("links".into(), "Synthetic links".into()).unwrap().deck;
    deck.slides[0].elements.push(serde_json::from_value(serde_json::json!({"type":"text","id":"contact","x":80,"y":560,"width":600,"height":60,"text":"Contact","font_size":20,"color":"000000","bold":false,"format":{"hyperlink":url}})).unwrap());
    let source = document::create("links".into(), deck, vec![], vec![], None).unwrap();
    let bytes = templates::export_potx(&source).unwrap();
    let original = Package::open(bytes.clone()).unwrap();
    let path = "ppt/slides/_rels/slide1.xml.rels";
    assert!(original.text(path).unwrap().contains("TargetMode=\"External\""));
    let plain = templates::load_potx("plain".into(), bytes.clone()).unwrap();
    let inspection = templates::inspect_potx(bytes.clone()).unwrap();
    assert_eq!(inspection["sample_slide_count"], 1);
    let defaults = templates::load_potx_with_options("defaults".into(), bytes.clone(), &templates::TemplateImportOptions::default()).unwrap();
    assert_eq!(serde_json::to_value(&plain.deck).unwrap(), serde_json::to_value(&defaults.deck).unwrap());
    let options = templates::TemplateImportOptions { include_sample_slides: false, source_sha256: Some(inspection["source_sha256"].as_str().unwrap().into()), ..Default::default() };
    let selected = templates::load_potx_with_options("blank".into(), bytes.clone(), &options).unwrap();
    assert_eq!(selected.deck.slides.len(), 1);
    assert!(selected.deck.slides[0].elements.is_empty());
    for document in [defaults, selected] {
        let exported = document::export_presentation(&document).unwrap();
        let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
        assert_eq!(package.part(path).unwrap(), original.part(path).unwrap());
    }
    for kind in [format!("{R}/image"), format!("{R}/slide"), "urn:vendor:external".into()] {
        let mut package = Package::open(bytes.clone()).unwrap();
        let xml = package.text(path).unwrap().replace(&format!("{R}/hyperlink"), &kind);
        package.replace_part(path, xml.into_bytes()).unwrap();
        let unsafe_source = package.save().unwrap();
        assert!(templates::inspect_potx(unsafe_source.clone()).is_err(), "external resource accepted: {kind}");
        assert!(templates::load_potx_with_options("unsafe".into(), unsafe_source, &options).is_err(), "external resource selected: {kind}");
    }
}

fn replace_xml_node(package: &mut Package, path: &str, namespace: &str, tag: &str, replacement: &str) {
    let mut xml = package.text(path).unwrap().to_owned();
    let range = roxmltree::Document::parse(&xml).unwrap().descendants().find(|node| node.has_tag_name((namespace, tag))).unwrap().range();
    xml.replace_range(range, replacement);
    package.replace_part(path, xml.into_bytes()).unwrap();
}

fn metadata_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(fields) => format!("<m:object>{}</m:object>", fields.iter().map(|(key, value)| format!("<m:field name=\"{}\">{}</m:field>", quick_xml::escape::escape(key), metadata_value(value))).collect::<String>()),
        serde_json::Value::Array(values) => format!("<m:array>{}</m:array>", values.iter().map(metadata_value).collect::<String>()),
        serde_json::Value::String(value) => format!("<m:string>{}</m:string>", quick_xml::escape::escape(value)),
        serde_json::Value::Number(value) => format!("<m:number>{value}</m:number>"),
        _ => panic!("fixture metadata uses objects, arrays, strings and numbers only"),
    }
}

fn oversized_source() -> Vec<u8> {
    synthetic_source(118, 62)
}

fn synthetic_source(layout_count: usize, slide_count: usize) -> Vec<u8> {
    let mut deck = editing::create("large-template".into(), "Synthetic template".into()).unwrap().deck;
    let design = deck.design.as_mut().unwrap();
    design.masters[0].id = "brand-a".into(); design.masters[0].name = "Brand A".into();
    let mut second = design.masters[0].clone(); second.id = "brand-b".into(); second.name = "Brand B".into(); design.masters.push(second);
    design.layouts.truncate(1); design.layouts[0].elements.clear(); design.layouts[0].id = "source-layout-1".into(); design.layouts[0].master_id = "brand-a".into();
    let mut second = design.layouts[0].clone(); second.id = "source-layout-2".into(); second.master_id = "brand-b".into(); design.layouts.push(second);
    deck.slides[0].layout_id = Some("source-layout-1".into());
    let source = document::create("large-template".into(), deck, vec![], vec![], None).unwrap();
    let mut package = Package::open(templates::export_potx(&source).unwrap()).unwrap();
    let master_rows = (1..=2).map(|number| format!("<p:sldMasterId id=\"{}\" r:id=\"rIdMaster{number}\"/>", 2147483647u64 + number)).collect::<String>();
    replace_xml_node(&mut package, "ppt/presentation.xml", P, "sldMasterIdLst", &format!("<p:sldMasterIdLst>{master_rows}</p:sldMasterIdLst>"));
    let slide_rows = (1..=slide_count).map(|number| format!("<p:sldId id=\"{}\" r:id=\"rId{number}\"/>", 255 + number)).collect::<String>();
    replace_xml_node(&mut package, "ppt/presentation.xml", P, "sldIdLst", &format!("<p:sldIdLst>{slide_rows}</p:sldIdLst>"));
    let mut parts = Package::open(package.save().unwrap()).unwrap().parts().clone();
    let base_layout = parts["ppt/slideLayouts/slideLayout1.xml"].clone();
    let base_slide = parts["ppt/slides/slide1.xml"].clone();
    let mut main_rels = format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"rIdNotesMaster\" Type=\"{R}/notesMaster\" Target=\"notesMasters/notesMaster1.xml\"/>");
    let mut identity_rows = String::new(); let mut identities = vec![];
    for number in 1..=2 {
        let path = format!("ppt/slideMasters/slideMaster{number}.xml");
        let id = if number == 1 { "brand-a" } else { "brand-b" };
        main_rels.push_str(&format!("<Relationship Id=\"rIdMaster{number}\" Type=\"{R}/slideMaster\" Target=\"/{path}\"/>"));
        identity_rows.push_str(&format!("<d:master id=\"{id}\" part=\"{path}\"/>"));
        identities.push(serde_json::json!({"id":id,"part":path,"objects":{}}));
        let range = if number == 1 { 1..=13 } else { 14..=layout_count };
        let rows = range.clone().map(|layout| format!("<p:sldLayoutId id=\"{}\" r:id=\"rIdLayout{layout}\"/>", 2147483650u64 + layout as u64)).collect::<String>();
        let mut master = Package::from_parts(parts.clone()).unwrap();
        replace_xml_node(&mut master, &path, P, "sldLayoutIdLst", &format!("<p:sldLayoutIdLst>{rows}</p:sldLayoutIdLst>"));
        parts.insert(path, master.part(&format!("ppt/slideMasters/slideMaster{number}.xml")).unwrap().to_vec());
        let rels = range.map(|layout| format!("<Relationship Id=\"rIdLayout{layout}\" Type=\"{R}/slideLayout\" Target=\"../slideLayouts/slideLayout{layout}.xml\"/>")).collect::<String>();
        parts.insert(format!("ppt/slideMasters/_rels/slideMaster{number}.xml.rels"), format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"rIdTheme\" Type=\"{R}/theme\" Target=\"../theme/theme1.xml\"/>{rels}</Relationships>").into_bytes());
    }
    let mut types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
    let mut extra_types = String::new();
    for number in 1..=layout_count {
        let path = format!("ppt/slideLayouts/slideLayout{number}.xml");
        let mut xml = String::from_utf8(base_layout.clone()).unwrap();
        let parsed = roxmltree::Document::parse(&xml).unwrap();
        let name = parsed.descendants().find(|node| node.has_tag_name((P, "cSld"))).unwrap().attributes().find(|attribute| attribute.name() == "name").unwrap().range_value();
        xml.replace_range(name, &format!("Layout {number:03}"));
        parts.insert(path.clone(), xml.into_bytes());
        let master = if number <= 13 { 1 } else { 2 };
        parts.insert(format!("ppt/slideLayouts/_rels/slideLayout{number}.xml.rels"), format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"rIdMaster\" Type=\"{R}/slideMaster\" Target=\"../slideMasters/slideMaster{master}.xml\"/></Relationships>").into_bytes());
        if number > 2 { extra_types.push_str(&format!("<Override PartName=\"/{path}\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml\"/>")); }
        identity_rows.push_str(&format!("<d:layout id=\"source-layout-{number}\" part=\"{path}\"/>"));
        identities.push(serde_json::json!({"id":format!("source-layout-{number}"),"part":path,"objects":{}}));
    }
    for number in 1..=slide_count {
        let path = format!("ppt/slides/slide{number}.xml");
        parts.insert(path.clone(), base_slide.clone());
        let layout = (number - 1) % layout_count + 1;
        parts.insert(format!("ppt/slides/_rels/slide{number}.xml.rels"), format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"rId1\" Type=\"{R}/slideLayout\" Target=\"../slideLayouts/slideLayout{layout}.xml\"/></Relationships>").into_bytes());
        main_rels.push_str(&format!("<Relationship Id=\"rId{number}\" Type=\"{R}/slide\" Target=\"/{path}\"/>"));
        if number > 1 { extra_types.push_str(&format!("<Override PartName=\"/{path}\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>")); }
        identities.push(serde_json::json!({"id":format!("sample-{number}"),"part":path,"objects":{}}));
    }
    main_rels.push_str(&format!("<Relationship Id=\"rIdMetadata\" Type=\"{R}/customXml\" Target=\"/customXml/aislide-provenance1.xml\"/><Relationship Id=\"rIdVendor\" Type=\"{R}/customXml\" Target=\"/customXml/vendor.xml\"/></Relationships>"));
    parts.insert("ppt/_rels/presentation.xml.rels".into(), main_rels.into_bytes());
    let metadata = serde_json::json!({"version":1,"sources":[],"bindings":[],"identities":identities});
    parts.insert("customXml/aislide-provenance1.xml".into(), format!("<m:provenance xmlns:m=\"{PROVENANCE}\">{}</m:provenance>", metadata_value(&metadata)).into_bytes());
    parts.insert("customXml/vendor.xml".into(), b"<v:data xmlns:v=\"urn:vendor\">  opaque &amp; exact  </v:data>".to_vec());
    parts.insert("vendor/opaque.bin".into(), vec![11,22,33,44]);
    extra_types.push_str("<Override PartName=\"/customXml/vendor.xml\" ContentType=\"application/xml\"/>");
    types.insert_str(types.rfind("</").unwrap(), &extra_types); parts.insert("[Content_Types].xml".into(), types.into_bytes());
    let mut xml = String::from_utf8(parts["ppt/presentation.xml"].clone()).unwrap();
    xml.insert_str(xml.rfind("</").unwrap(), &format!("<p:extLst><p:ext uri=\"urn:aislide:design:1\"><d:design xmlns:d=\"urn:aislide:design:1\">{identity_rows}</d:design></p:ext></p:extLst>"));
    parts.insert("ppt/presentation.xml".into(), xml.into_bytes());
    Package::from_parts(parts).unwrap().save().unwrap()
}

fn exported_package(document: &document::Document) -> Package {
    let result = document::export_presentation(document).unwrap();
    Package::open(STANDARD.decode(result["base64"].as_str().unwrap()).unwrap()).unwrap()
}

fn assert_editable(document: &document::Document) {
    let added = editing::slides(document, document.revision, &[editing::SlideOperation::Insert { id:"added".into(), after:None, title:"Added".into(), layout_id:document.deck.slides[0].layout_id.clone() }]).unwrap().document;
    let removed = editing::slides(&added, added.revision, &[editing::SlideOperation::Remove { slide_id:"added".into() }]).unwrap().document;
    assert_eq!(removed.deck.slides.len(), document.deck.slides.len());
    exported_package(&removed);
}

fn issue18_source(slide_list: &str) -> Vec<u8> {
    let mut package = Package::open(synthetic_source(14, 0)).unwrap();
    replace_xml_node(&mut package, "ppt/presentation.xml", P, "sldIdLst", slide_list);
    let mut xml = package.text("ppt/presentation.xml").unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let extensions = parsed.root_element().children().find(|node| node.has_tag_name((P, "extLst"))).unwrap();
    let position = xml[..extensions.range().end].rfind("</").unwrap();
    xml.insert_str(position, "<p:ext uri=\"urn:vendor\"><v:keep xmlns:v=\"urn:vendor\" value=\"exact &amp; opaque\"><v:sldIdLst/><p:sldIdLst><p:sldId id=\"999\" r:id=\"vendor-only\"/></p:sldIdLst></v:keep></p:ext>");
    package.replace_part("ppt/presentation.xml", xml.into_bytes()).unwrap();
    package.save().unwrap()
}

#[test]
fn issue18_inspection_without_slide_list_reports_zero_samples() {
    let bytes = issue18_source("");
    let inspection = templates::inspect_potx(bytes.clone()).unwrap();
    assert_eq!(inspection["sample_slide_count"], 0);
    assert_eq!(inspection["masters"].as_array().unwrap().len(), 2);
    assert_eq!(inspection["layouts"].as_array().unwrap().len(), 14);
    assert!(templates::load_potx("default".into(), bytes.clone()).is_err());
    assert!(templates::load_potx_with_options("defaults".into(), bytes, &Default::default()).is_err());
}

fn assert_issue18_blank_import(slide_list: &str) {
    use sha2::{Digest, Sha256};
    let bytes = issue18_source(slide_list); let original = bytes.clone();
    for layout_names in [vec![], vec!["Layout 014".into()]] {
        let options = templates::TemplateImportOptions {
            include_sample_slides:false, layout_names,
            source_sha256:Some(format!("{:x}", Sha256::digest(&bytes))), ..Default::default()
        };
        let imported = templates::load_potx_with_options("blank".into(), bytes.clone(), &options).unwrap();
        assert_eq!(imported.deck.slides.len(), 1);
        assert!(imported.deck.slides[0].elements.is_empty());
        let expected_layout = if options.layout_names.is_empty() { "source-layout-1" } else { "source-layout-14" };
        assert_eq!(imported.deck.slides[0].layout_id.as_deref(), Some(expected_layout));
        let output = exported_package(&imported); let source = Package::open(bytes.clone()).unwrap();
        let xml = output.text("ppt/presentation.xml").unwrap();
        let parsed = roxmltree::Document::parse(xml).unwrap();
        let children: Vec<_> = parsed.root_element().children().filter(|node| node.is_element() && node.tag_name().namespace() == Some(P)).collect();
        let lists: Vec<_> = children.iter().filter(|node| node.has_tag_name((P, "sldIdLst"))).collect();
        assert_eq!(lists.len(), 1);
        let slides: Vec<_> = lists[0].children().filter(|node| node.has_tag_name((P, "sldId"))).collect();
        assert_eq!(slides.len(), 1);
        assert_eq!(slides[0].attribute("id"), Some("256"));
        let list_position = lists[0].range().start;
        for node in &children {
            match node.tag_name().name() {
                "sldMasterIdLst" | "notesMasterIdLst" | "handoutMasterIdLst" => assert!(node.range().start < list_position),
                "sldSz" | "notesSz" | "smartTags" | "embeddedFontLst" | "custShowLst" | "photoAlbum" | "custDataLst" | "kinsoku" | "defaultTextStyle" | "modifyVerifier" | "extLst" => assert!(node.range().start > list_position),
                _ => {},
            }
        }
        let source_xml = source.text("ppt/presentation.xml").unwrap();
        let source_parsed = roxmltree::Document::parse(source_xml).unwrap();
        let vendor = source_parsed.descendants().find(|node| node.has_tag_name((P, "ext")) && node.attribute("uri") == Some("urn:vendor")).unwrap();
        assert!(xml.contains(&source_xml[vendor.range()]));
        for path in ["vendor/opaque.bin", "customXml/vendor.xml", "ppt/slides/slide1.xml", "ppt/slideLayouts/slideLayout14.xml"] {
            assert_eq!(output.part(path).unwrap(), source.part(path).unwrap(), "{path}");
        }
        let reopened: document::Document = serde_json::from_value(document::open_presentation("reopened".into(), output.save().unwrap()).unwrap()["document"].clone()).unwrap();
        assert_eq!(reopened.deck.slides.len(), 1);
        assert_eq!(reopened.deck.slides[0].layout_id.as_deref(), Some(expected_layout));
        assert_editable(&imported);
    }
    assert_eq!(bytes, original);
}

#[test]
fn issue18_blank_import_without_slide_list_preserves_order_and_payloads() {
    assert_issue18_blank_import("");
}

#[test]
fn issue18_explicit_empty_slide_lists_import_as_blank() {
    for list in ["<p:sldIdLst/>", "<p:sldIdLst></p:sldIdLst>"] {
        assert_eq!(templates::inspect_potx(issue18_source(list)).unwrap()["sample_slide_count"], 0);
        assert_issue18_blank_import(list);
    }
}

#[test]
fn issue18_optional_slide_list_keeps_topology_and_opc_guards() {
    let options = templates::TemplateImportOptions { include_sample_slides:false, ..Default::default() };
    for list in ["<p:sldIdLst/><p:sldIdLst/>", "<p:sldIdLst><p:sldId id=\"256\" r:id=\"missing\"/></p:sldIdLst>"] {
        let bytes = issue18_source(list);
        assert!(templates::inspect_potx(bytes.clone()).is_err());
        assert!(templates::load_potx_with_options("invalid-list".into(), bytes, &options).is_err());
    }
    for (target, mode) in [("https://example.invalid/slide.xml", "External"), ("../../../outside.xml", "Internal"), ("slides/missing.xml", "Internal")] {
        let mut package = Package::open(issue18_source("")).unwrap();
        let path = "ppt/_rels/presentation.xml.rels";
        let mut xml = package.text(path).unwrap().to_owned();
        xml.insert_str(xml.rfind("</").unwrap(), &format!("<Relationship Id=\"unsafe\" Type=\"{R}/slide\" Target=\"{target}\" TargetMode=\"{mode}\"/>"));
        package.replace_part(path, xml.into_bytes()).unwrap();
        let bytes = package.save().unwrap();
        assert!(templates::inspect_potx(bytes.clone()).is_err());
        assert!(templates::load_potx_with_options("unsafe".into(), bytes, &options).is_err());
    }
    let mut package = Package::open(issue18_source("")).unwrap();
    let path = "ppt/_rels/presentation.xml.rels";
    let mut xml = package.text(path).unwrap().to_owned();
    xml.insert_str(xml.rfind("</").unwrap(), &format!("<Relationship Id=\"vendor-only\" Type=\"{R}/slide\" Target=\"slides/slide1.xml\"/>"));
    package.replace_part(path, xml.into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    assert_eq!(templates::inspect_potx(bytes.clone()).unwrap()["sample_slide_count"], 0);
    let error = templates::load_potx_with_options("referenced".into(), bytes, &options).unwrap_err().to_string();
    assert!(error.contains("retained template XML references an excluded relationship"), "{error}");
}

#[test]
fn oversized_inspection_and_selected_blank_import_preserve_source_and_opaque_parts() {
    use sha2::{Digest, Sha256};
    let bytes = oversized_source(); let original = bytes.clone();
    let error = templates::load_potx("default".into(), bytes.clone()).unwrap_err().to_string();
    assert!(error.contains("design") && error.contains("limit"), "{error}");
    let inspection = templates::inspect_potx(bytes.clone()).unwrap();
    assert_eq!(inspection["source_sha256"], format!("{:x}", Sha256::digest(&bytes)));
    assert_eq!(inspection["masters"].as_array().unwrap().len(), 2);
    assert_eq!(inspection["layouts"].as_array().unwrap().len(), 118);
    assert_eq!(inspection["sample_slide_count"], 62);
    assert_eq!(inspection["layouts"][117]["name"], "Layout 118");
    let options = templates::TemplateImportOptions {
        include_sample_slides:false, master_ids:vec![inspection["masters"][0]["id"].as_str().unwrap().into()],
        layout_ids:inspection["layouts"].as_array().unwrap()[..13].iter().map(|layout| layout["id"].as_str().unwrap().into()).collect(),
        source_sha256:Some(inspection["source_sha256"].as_str().unwrap().into()), ..Default::default()
    };
    let imported = templates::load_potx_with_options("selected".into(), bytes.clone(), &options).unwrap();
    let design = imported.deck.design.as_ref().unwrap();
    assert_eq!(design.masters.len(), 1); assert_eq!(design.layouts.len(), 13);
    assert_eq!(design.masters[0].id, "brand-a"); assert_eq!(design.layouts[0].id, "source-layout-1");
    assert_eq!(imported.deck.slides.len(), 1); assert!(imported.deck.slides[0].elements.is_empty());
    assert_eq!(imported.deck.slides[0].layout_id.as_deref(), Some("source-layout-1"));
    assert_editable(&imported);
    let exported = exported_package(&imported); let source = Package::open(bytes.clone()).unwrap();
    for path in ["vendor/opaque.bin", "customXml/vendor.xml", "ppt/slideLayouts/slideLayout118.xml", "ppt/slides/slide62.xml"] { assert_eq!(exported.part(path).unwrap(), source.part(path).unwrap(), "{path}"); }
    assert_eq!(bytes, original);
    let reopened: document::Document = serde_json::from_value(document::open_presentation("reopened".into(), exported.save().unwrap()).unwrap()["document"].clone()).unwrap();
    assert_eq!(reopened.deck.design.unwrap().layouts.len(), 13);
}

fn section_source() -> Vec<u8> {
    let source = editing::create("sections-source".into(), "Sections".into()).unwrap();
    let mut package = Package::open(templates::export_potx(&source).unwrap()).unwrap();
    let xml = package.text("ppt/presentation.xml").unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let extension = format!("<p:ext xmlns:p=\"{P}\" uri=\"sections\"><s:sectionLst xmlns:s=\"{P14}\"><s:section name=\"Samples\" id=\"{{00000000-0000-4000-8000-000000000001}}\"><s:sldIdLst><s:sldId id=\"256\"/></s:sldIdLst></s:section></s:sectionLst><v:keep xmlns:v=\"urn:vendor\" value=\"exact &amp; opaque\"/></p:ext>");
    let list = parsed.root_element().children().find(|node| node.has_tag_name((P, "extLst")));
    let (position, fragment) = match list {
        Some(list) => (xml[..list.range().end].rfind("</").unwrap(), extension),
        None => (xml.rfind("</").unwrap(), format!("<p:extLst xmlns:p=\"{P}\">{extension}</p:extLst>")),
    };
    let mut changed = xml.clone(); changed.insert_str(position, &fragment);
    package.replace_part("ppt/presentation.xml", changed.into_bytes()).unwrap();
    package.save().unwrap()
}

#[test]
fn explicit_section_strip_preserves_unknown_siblings() {
    let bytes = section_source();
    let options = templates::TemplateImportOptions { strip_sections: true, ..Default::default() };
    let imported = templates::load_potx_with_options("stripped".into(), bytes.clone(), &options).unwrap();
    let exported = document::export_presentation(&imported).unwrap();
    let package = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/presentation.xml").unwrap();
    assert!(!roxmltree::Document::parse(xml).unwrap().descendants().any(|node| node.has_tag_name((P14, "sectionLst"))));
    assert!(xml.contains("<v:keep xmlns:v=\"urn:vendor\" value=\"exact &amp; opaque\"/>"));
    assert_editable(&imported);
    let default = templates::load_potx("default".into(), bytes.clone()).unwrap();
    let error = editing::slides(&default, 0, &[editing::SlideOperation::Insert { id:"blocked".into(), after:None, title:"Blocked".into(), layout_id:None }]).err().unwrap().to_string();
    assert!(error.contains("sections/custom"), "{error}");
    let default_bytes = templates::export_potx(&default).unwrap();
    assert_eq!(Package::open(default_bytes).unwrap().part("ppt/presentation.xml").unwrap(), Package::open(bytes).unwrap().part("ppt/presentation.xml").unwrap());
}

#[test]
fn template_selectors_are_exact_unique_and_source_bound() {
    let bytes = oversized_source();
    let options = templates::TemplateImportOptions { include_sample_slides:false, master_names:vec!["Brand A".into()], ..Default::default() };
    let imported = templates::load_potx_with_options("names".into(), bytes.clone(), &options).unwrap();
    assert_eq!(imported.deck.design.unwrap().layouts.len(), 13);
    let options = templates::TemplateImportOptions { include_sample_slides:false, layout_names:vec!["Layout 014".into()], ..Default::default() };
    let imported = templates::load_potx_with_options("layout-owner".into(), bytes.clone(), &options).unwrap();
    let design = imported.deck.design.as_ref().unwrap();
    assert_eq!(design.masters[0].id, "brand-b"); assert_eq!(design.layouts.len(), 1);
    assert_editable(&imported);
    for (value, expected) in [
        (serde_json::json!({"layout_names":["layout 014"]}), "missing or ambiguous"),
        (serde_json::json!({"layout_ids":["source-layout-14"]}), "ID not found"),
        (serde_json::json!({"layout_ids":["C:/outside.xml"]}), "ID not found"),
        (serde_json::json!({"layout_names":["Layout 014","Layout 014"]}), "duplicate"),
        (serde_json::json!({"layout_ids":["ppt/slideLayouts/slideLayout14.xml"],"layout_names":["Layout 014"]}), "duplicate"),
        (serde_json::json!({"master_names":["Brand A"],"layout_names":["Layout 014"]}), "outside"),
        (serde_json::json!({"master_names":["Brand B"]}), "1-32"),
        (serde_json::json!({"master_names":["Brand A"],"source_sha256":"0".repeat(64)}), "SHA-256 changed"),
        (serde_json::json!({"master_names":["Brand A"],"source_sha256":"invalid"}), "64 lowercase"),
        (serde_json::json!({"layout_names":vec!["Layout 001";33]}), "at most 32"),
    ] {
        let mut options: templates::TemplateImportOptions = serde_json::from_value(value).unwrap(); options.include_sample_slides = false;
        let error = templates::load_potx_with_options("reject".into(), bytes.clone(), &options).unwrap_err().to_string();
        assert!(error.contains(expected), "expected {expected}: {error}");
        if expected == "1-32" { assert!(error.contains("inspect_template") && !error.contains("inspect_potx"), "public recovery tool missing: {error}"); }
    }
    let mut package = Package::open(bytes).unwrap();
    let path = "ppt/slideLayouts/slideLayout15.xml";
    package.replace_part(path, package.text(path).unwrap().replace("Layout 015", "Layout 014").into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let error = templates::load_potx_with_options("ambiguous".into(), bytes.clone(), &options).unwrap_err().to_string();
    assert!(error.contains("ambiguous"), "{error}");
    assert_eq!(templates::inspect_potx(bytes).unwrap()["layouts"][14]["name"], "Layout 014");
    assert!(serde_json::from_value::<templates::TemplateImportOptions>(serde_json::json!({"unknown":true})).is_err());
    assert!(serde_json::from_value::<templates::TemplateImportOptions>(serde_json::json!({"layout_ids":null})).is_err());
    let defaults: templates::TemplateImportOptions = serde_json::from_value(serde_json::json!({})).unwrap();
    assert!(defaults.include_sample_slides); assert!(!defaults.strip_sections); assert!(defaults.layout_ids.is_empty());
    let schema = serde_json::to_value(schemars::schema_for!(templates::TemplateImportOptions)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["layout_ids"]["maxItems"], 32);
    assert_eq!(schema["properties"]["layout_names"]["maxItems"], 32);
    assert_eq!(schema["properties"]["master_ids"]["maxItems"], 8);
    assert_eq!(schema["properties"]["master_names"]["maxItems"], 8);
}

#[test]
fn selected_template_preserves_managed_fingerprints_and_removes_only_discarded_records() {
    use serde_json::json;
    let source = aislide_core::execute_request(json!({"op":"ingest","input":{"format":"csv","name":"synthetic.csv","base64":STANDARD.encode(b"Label,Value\nSelected,12\n")}})).unwrap();
    let mut deck = editing::create("managed".into(), "Managed template".into()).unwrap().deck;
    deck.slides[0].elements.push(serde_json::from_value(json!({"id":"source-marker","type":"text","x":10,"y":10,"width":120,"height":30,"text":"12","font_size":18,"color":"202426","bold":false})).unwrap());
    let binding = json!({"slide_id":"slide-1","element_id":"source-marker","field":"/text","source_id":source["id"],"source_sha256":source["sha256"],"locator":"record:2/column:2","raw_value":"12","value":"12","transform":"display_scalar","stale":false});
    let document = document::create("managed".into(), deck, vec![serde_json::from_value(source).unwrap()], vec![serde_json::from_value(binding).unwrap()], None).unwrap();
    let spec = serde_json::from_value(json!({"version":1,"preset":"vertical-bar-graph/balanced","title":"Synthetic volume","data":{"kind":"chart","categories":["A","B"],"series":[{"name":"Volume","values":[12,24]}]}})).unwrap();
    let document = aislide_core::parts::state::change(&document, 0, "slide-1", "managed", &spec, false).unwrap().document;
    let bytes = templates::export_potx(&document).unwrap();
    let control = templates::load_potx("control".into(), bytes.clone()).unwrap();
    let design = document.deck.design.as_ref().unwrap();
    let position = design.layouts.iter().position(|layout| Some(&layout.id) == document.deck.slides[0].layout_id.as_ref()).unwrap();
    let mut options = templates::TemplateImportOptions { layout_ids:vec![format!("ppt/slideLayouts/slideLayout{}.xml", position + 1)], ..Default::default() };
    let kept = templates::load_potx_with_options("kept".into(), bytes.clone(), &options).unwrap();
    assert_eq!(kept.parts.len(), 1); assert!(!kept.parts[0].stale);
    assert_eq!(kept.parts[0].native_sha256, control.parts[0].native_sha256);
    assert_eq!(kept.parts[0].render_sha256, control.parts[0].render_sha256);
    assert_eq!(kept.bindings.len(), 1); assert!(!kept.bindings[0].stale);
    assert_editable(&kept);
    let updated = aislide_core::parts::state::change(&kept, 0, "slide-1", "managed", &spec, true).unwrap().document;
    exported_package(&updated);
    let mut package = Package::open(bytes.clone()).unwrap();
    let chart = "ppt/charts/chart1.xml";
    let xml = package.text(chart).unwrap().replace("<c:v>12</c:v>", "<c:v>13</c:v>");
    assert_ne!(xml, package.text(chart).unwrap()); package.replace_part(chart, xml.into_bytes()).unwrap();
    let edited = templates::load_potx_with_options("edited-native".into(), package.save().unwrap(), &options).unwrap();
    assert!(edited.parts[0].stale, "selection must not bless externally changed fingerprints");
    options.include_sample_slides = false;
    let blank = templates::load_potx_with_options("blank".into(), bytes, &options).unwrap();
    assert!(blank.parts.is_empty()); assert!(blank.bindings.is_empty()); assert_eq!(blank.sources.len(), 1);
    assert!(blank.deck.slides[0].elements.is_empty());
    let package = exported_package(&blank);
    assert!(package.part("ppt/charts/chart1.xml").is_ok());
    let reopened: document::Document = serde_json::from_value(document::open_presentation("blank-reopened".into(), package.save().unwrap()).unwrap()["document"].clone()).unwrap();
    assert!(reopened.parts.is_empty()); assert!(reopened.bindings.is_empty()); assert_eq!(reopened.sources.len(), 1);
}

#[test]
fn selected_package_relationships_and_known_identity_topology_remain_consistent() {
    let bytes = oversized_source();
    let options = templates::TemplateImportOptions { include_sample_slides:false, layout_names:vec!["Layout 004".into()], ..Default::default() };
    let imported = templates::load_potx_with_options("one".into(), bytes.clone(), &options).unwrap();
    let package = exported_package(&imported);
    for (path, content) in package.parts().iter().filter(|(path, _)| path.ends_with(".rels")) {
        let source = if path == "_rels/.rels" { String::new() } else { let (directory, name) = path.rsplit_once("/_rels/").unwrap(); format!("{directory}/{}", name.trim_end_matches(".rels")) };
        for relation in roxmltree::Document::parse(std::str::from_utf8(content).unwrap()).unwrap().root_element().children().filter(|node| node.has_tag_name((REL, "Relationship"))) {
            let target = relation.attribute("Target").unwrap();
            let mut segments: Vec<_> = if target.starts_with('/') { vec![] } else { source.rsplit_once('/').map(|(folder, _)| folder.split('/').collect()).unwrap_or_default() };
            for segment in target.trim_start_matches('/').split('/') { match segment { ".." => { segments.pop().unwrap(); }, "." => {}, value => segments.push(value) } }
            assert!(package.part(&segments.join("/")).is_ok(), "dangling target {target} in {path}");
        }
    }
    let mut package = Package::open(bytes).unwrap();
    let path = "ppt/presentation.xml";
    let xml = package.text(path).unwrap().replace("id=\"source-layout-118\" part=\"ppt/slideLayouts/slideLayout118.xml\"", "id=\"source-layout-118\" part=\"ppt/slideLayouts/slideLayout117.xml\"");
    package.replace_part(path, xml.into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    assert!(templates::inspect_potx(bytes.clone()).is_err());
    assert!(templates::load_potx_with_options("bad-identities".into(), bytes, &options).is_err());
}

#[test]
fn template_slice_files_retain_single_utf8_bom() {
    for source in [include_bytes!("../src/templates.rs").as_slice(), include_bytes!("../src/template_selection.rs").as_slice(), include_bytes!("templates.rs").as_slice()] {
        assert!(source.starts_with(&[0xef,0xbb,0xbf]));
        assert!(!source[3..].starts_with(&[0xef,0xbb,0xbf]));
        std::str::from_utf8(source).unwrap();
    }
}

#[test]
fn samples_sections_and_custom_shows_cannot_be_silently_reassigned_or_dangled() {
    let options = templates::TemplateImportOptions { master_names:vec!["Brand A".into()], ..Default::default() };
    let error = templates::load_potx_with_options("samples".into(), oversized_source(), &options).unwrap_err().to_string();
    assert!(error.contains("sample slides reference excluded layouts"), "{error}");
    let bytes = section_source();
    let mut options = templates::TemplateImportOptions { include_sample_slides:false, ..Default::default() };
    let error = templates::load_potx_with_options("sections".into(), bytes.clone(), &options).unwrap_err().to_string();
    assert!(error.contains("strip_sections=true"), "{error}");
    options.strip_sections = true;
    let imported = templates::load_potx_with_options("blank".into(), bytes.clone(), &options).unwrap();
    assert_editable(&imported);
    let mut package = Package::open(bytes).unwrap();
    let mut xml = package.text("ppt/presentation.xml").unwrap().to_owned();
    let shows = format!("<p:custShowLst xmlns:p=\"{P}\" xmlns:r=\"{R}\"><p:custShow name=\"Demo\" id=\"0\"><p:sldLst><p:sld r:id=\"rId1\"/></p:sldLst></p:custShow></p:custShowLst>");
    xml.insert_str(xml.rfind("</").unwrap(), &shows); package.replace_part("ppt/presentation.xml", xml.into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    assert_eq!(templates::inspect_potx(bytes.clone()).unwrap()["has_custom_shows"], true);
    let error = templates::load_potx_with_options("custom-shows".into(), bytes.clone(), &options).unwrap_err().to_string();
    assert!(error.contains("custom slide shows"), "{error}");
    options.include_sample_slides = true;
    let imported = templates::load_potx_with_options("kept-shows".into(), bytes, &options).unwrap();
    assert!(exported_package(&imported).text("ppt/presentation.xml").unwrap().contains(&shows));
    let error = editing::slides(&imported, 0, &[editing::SlideOperation::Insert { id:"blocked".into(), after:None, title:"Blocked".into(), layout_id:None }]).err().unwrap().to_string();
    assert!(error.contains("sections/custom"), "{error}");
}

#[test]
fn inspection_is_bounded_and_selection_rejects_unsafe_packages_before_discarding_samples() {
    assert_eq!(templates::inspect_potx(synthetic_source(256, 1)).unwrap()["layouts"].as_array().unwrap().len(), 256);
    let error = templates::inspect_potx(synthetic_source(257, 1)).unwrap_err().to_string(); assert!(error.contains("256"), "{error}");
    let base = section_source();
    let options = templates::TemplateImportOptions { include_sample_slides:false, strip_sections:true, ..Default::default() };
    let mut variants = vec![vec![0xd0,0xcf,0x11,0xe0,0xa1,0xb1,0x1a,0xe1]];
    for path in ["ppt/vbaProject.bin", "_xmlsignatures/sig1.xml", "docMetadata/LabelInfo.xml"] {
        let mut parts = Package::open(base.clone()).unwrap().parts().clone(); parts.insert(path.into(), b"<protected/>".to_vec()); variants.push(Package::from_parts(parts).unwrap().save().unwrap());
    }
    let mut package = Package::open(base.clone()).unwrap();
    let xml = package.text("[Content_Types].xml").unwrap().replace("presentationml.template.main+xml", "ms-powerpoint.template.macroEnabled.main+xml");
    package.replace_part("[Content_Types].xml", xml.into_bytes()).unwrap(); variants.push(package.save().unwrap());
    let mut package = Package::open(base.clone()).unwrap();
    let mut xml = package.text("ppt/presentation.xml").unwrap().to_owned(); xml.insert_str(xml.rfind("</").unwrap(), "<p:modifyVerifier/>");
    package.replace_part("ppt/presentation.xml", xml.into_bytes()).unwrap(); variants.push(package.save().unwrap());
    for target in ["https://example.invalid/theme.xml", "../../../outside.xml", "%2e%2e/outside.xml"] {
        let mut package = Package::open(base.clone()).unwrap();
        let path = "ppt/slides/_rels/slide1.xml.rels";
        let mut xml = package.text(path).unwrap().to_owned();
        xml.insert_str(xml.rfind("</").unwrap(), &format!("<Relationship Id=\"unsafe\" Type=\"urn:vendor\" Target=\"{target}\" TargetMode=\"External\"/>"));
        package.replace_part(path, xml.into_bytes()).unwrap(); variants.push(package.save().unwrap());
    }
    for bytes in variants {
        assert!(templates::inspect_potx(bytes.clone()).is_err());
        assert!(templates::load_potx_with_options("unsafe".into(), bytes, &options).is_err());
    }
}

#[test]
fn default_options_preserve_every_entry_except_main_content_type() {
    let bytes = section_source();
    let imported = templates::load_potx_with_options("defaults".into(), bytes.clone(), &Default::default()).unwrap();
    let output = exported_package(&imported); let source = Package::open(bytes).unwrap();
    for (path, content) in source.parts() { if path != "[Content_Types].xml" { assert_eq!(output.part(path).unwrap(), content, "{path}"); } }
    assert_eq!(roxmltree::Document::parse(output.text("[Content_Types].xml").unwrap()).unwrap().root_element().tag_name().namespace(), Some(CT));
}

#[test]
fn default_and_strip_only_options_retain_unlisted_layout_relationships() {
    let mut parts = Package::open(synthetic_source(14, 1)).unwrap().parts().clone();
    parts.insert("ppt/slideLayouts/unlisted.xml".into(), parts["ppt/slideLayouts/slideLayout14.xml"].clone());
    parts.insert("ppt/slideLayouts/_rels/unlisted.xml.rels".into(), parts["ppt/slideLayouts/_rels/slideLayout14.xml.rels"].clone());
    let path = "ppt/slideMasters/_rels/slideMaster2.xml.rels";
    let mut xml = String::from_utf8(parts[path].clone()).unwrap();
    xml.insert_str(xml.rfind("</").unwrap(), &format!("<Relationship Id=\"unlisted\" Type=\"{R}/slideLayout\" Target=\"../slideLayouts/unlisted.xml\"/>"));
    parts.insert(path.into(), xml.into_bytes());
    let path = "[Content_Types].xml"; let mut xml = String::from_utf8(parts[path].clone()).unwrap();
    xml.insert_str(xml.rfind("</").unwrap(), "<Override PartName=\"/ppt/slideLayouts/unlisted.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml\"/>");
    parts.insert(path.into(), xml.into_bytes());
    let bytes = Package::from_parts(parts).unwrap().save().unwrap();
    let control = templates::load_potx("control".into(), bytes.clone()).unwrap();
    assert_eq!(control.deck.design.unwrap().masters.len(), 2);
    for strip_sections in [false, true] {
        let options = templates::TemplateImportOptions { strip_sections, ..Default::default() };
        let imported = templates::load_potx_with_options("defaults".into(), bytes.clone(), &options).unwrap();
        assert_eq!(imported.deck.design.as_ref().unwrap().masters.len(), 2);
        let output = Package::open(STANDARD.decode(&imported.origin.as_ref().unwrap().base64).unwrap()).unwrap();
        let source = Package::open(bytes.clone()).unwrap();
        for (path, content) in source.parts() { if path != "[Content_Types].xml" { assert!(output.part(path).unwrap() == content, "import changed {path}"); } }
    }
}

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