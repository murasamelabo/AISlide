use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn authored() -> Value {
    let report = execute_request(json!({"op":"sample"})).unwrap();
    let compiled = execute_request(json!({"op":"compile","report":report})).unwrap();
    let design = execute_request(json!({"op":"design_defaults"})).unwrap();
    let mut deck = compiled["deck"].clone();
    deck["design"] = design;
    deck["design"]["theme"]["colors"]["accent1"] = json!("B53055");
    deck["design"]["theme"]["fonts"]["major"] = json!("Arial");
    let shape = execute_request(json!({"op":"create_object","id":"native-ellipse","kind":"shape","preset":"ellipse"})).unwrap();
    deck["slides"][1]["elements"].as_array_mut().unwrap().push(shape);
    execute_request(json!({"op":"assign_layout","deck":deck,"slide_id":"slide-3","layout_id":"title-content"})).unwrap()
}

fn exported(deck: Value) -> Vec<u8> {
    let result = execute_request(json!({"op":"export","deck":deck})).unwrap();
    STANDARD.decode(result["base64"].as_str().unwrap()).unwrap()
}

fn without_xml_nodes(xml: &str, predicate: impl Fn(roxmltree::Node<'_, '_>) -> bool) -> String {
    let parsed = roxmltree::Document::parse(xml).unwrap();
    let mut ranges = parsed.descendants().filter(|node| predicate(*node)).map(|node| node.range()).collect::<Vec<_>>();
    ranges.sort_by_key(|range| range.start);
    let mut result = xml.to_owned();
    for range in ranges.into_iter().rev() { result.replace_range(range, ""); }
    result
}

fn native_change(document: &Value, operations: Value) -> Value {
    execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":operations}})).unwrap()
}

fn assert_native_undo(document: &Value, changed: &Value, original: &[u8]) {
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], document["hash"]);
    let restored = execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap();
    assert_eq!(STANDARD.decode(restored["base64"].as_str().unwrap()).unwrap(), original);
}

#[test]
fn native_notes_empty_bodies_and_paragraphs_preserve_unrelated_xml() {
    let path = "ppt/notesSlides/notesSlide1.xml";
    let namespace = "http://schemas.openxmlformats.org/presentationml/2006/main";
    for variant in ["empty-p", "styled-empty-p", "no-p", "no-body", "no-placeholder"] {
        let mut package = Package::open(exported(authored())).unwrap();
        let mut xml = package.text(path).unwrap().to_owned();
        let parsed = roxmltree::Document::parse(&xml).unwrap();
        let body = parsed.descendants().find(|node| node.has_tag_name((namespace, "txBody"))).unwrap().range();
        let paragraph = if variant == "styled-empty-p" { "<a:p><a:pPr marL=\"42\"/><a:endParaRPr lang=\"en-US\"/></a:p>" } else { "<a:p/>" };
        let replacement = match variant {
            "no-body" => String::new(),
            "no-p" => "<p:txBody><a:bodyPr/><a:lstStyle/></p:txBody>".into(),
            _ => format!("<p:txBody><a:bodyPr/><a:lstStyle/>{paragraph}</p:txBody>"),
        };
        xml.replace_range(body, &replacement);
        if variant == "no-placeholder" { xml = xml.replace("type=\"body\"", "type=\"sldImg\""); }
        let sentinel = "<p:extLst><p:ext uri=\"urn:empty-notes\"><v:data xmlns:v=\"urn:vendor\" keep=\"true\"/></p:ext></p:extLst>";
        xml = xml.replace("</p:notes>", &format!("{sentinel}</p:notes>"));
        package.replace_part(path, xml.into_bytes()).unwrap();
        let original = package.save().unwrap();
        let opened = execute_request(json!({"op":"open_presentation","id":"empty-notes","base64":STANDARD.encode(&original)})).unwrap();
        assert_eq!(opened["document"]["deck"]["slides"][0]["notes"], "", "{variant}");
        let changed = native_change(&opened["document"], json!([{"op":"replace","path":"/deck/slides/0/notes","value":" First & <line> \n\nLast"}]));
        let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        let result = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
        assert!(result.text(path).unwrap().contains(sentinel), "{variant}");
        if variant == "styled-empty-p" { assert!(result.text(path).unwrap().contains("<a:pPr marL=\"42\"/>")); }
        for part in package.parts().keys().filter(|part| !part.starts_with("customXml/") && part.as_str() != path) {
            assert_eq!(result.part(part).unwrap(), package.part(part).unwrap(), "unrelated part: {part}, {variant}");
        }
        let reopened = execute_request(json!({"op":"open_presentation","id":"empty-notes-reopened","base64":output["base64"]})).unwrap();
        assert_eq!(reopened["document"]["deck"]["slides"][0]["notes"], " First & <line> \n\nLast", "{variant}");
        let cleared = if reopened["document"]["deck"]["slides"][0]["notes_paragraphs"].as_array().is_some_and(|paragraphs| !paragraphs.is_empty()) {
            execute_request(json!({"op":"update_rich_notes","document":reopened["document"],"expected_revision":0,"slide_id":"slide-1","paragraphs":[]})).unwrap()
        } else {
            native_change(&reopened["document"], json!([{"op":"replace","path":"/deck/slides/0/notes","value":""}]))
        };
        let saved = execute_request(json!({"op":"export_presentation","document":cleared["document"]})).unwrap();
        let cleared_open = execute_request(json!({"op":"open_presentation","id":"cleared-notes","base64":saved["base64"]})).unwrap();
        assert_eq!(cleared_open["document"]["deck"]["slides"][0]["notes"], "");
        assert_native_undo(&opened["document"], &changed, &original);
    }
}

#[test]
fn native_notes_complex_runs_fields_and_multiple_bodies_reject_without_changes() {
    let path = "ppt/notesSlides/notesSlide1.xml";
    for content in [
        "<a:p><a:r><a:rPr b=\"1\"/><a:t>Mixed</a:t></a:r><a:r><a:rPr i=\"1\"/><a:t> styles</a:t></a:r></a:p>",
        "<a:p><a:fld id=\"{00000000-0000-0000-0000-000000000001}\" type=\"slidenum\"><a:t>1</a:t></a:fld></a:p>",
        "<a:p><a:r><a:t>Line</a:t></a:r><a:br/></a:p>",
        "multiple-bodies",
    ] {
        let mut package = Package::open(exported(authored())).unwrap();
        let mut xml = package.text(path).unwrap().to_owned();
        let parsed = roxmltree::Document::parse(&xml).unwrap();
        if content == "multiple-bodies" {
            let shape = parsed.descendants().find(|node| node.tag_name().name() == "sp").unwrap();
            let extra = xml[shape.range()].replace("id=\"2\"", "id=\"99\"");
            xml = xml.replace("</p:spTree>", &format!("{extra}</p:spTree>"));
        } else {
            let body = parsed.descendants().find(|node| node.tag_name().name() == "txBody").unwrap().range();
            xml.replace_range(body, &format!("<p:txBody><a:bodyPr/><a:lstStyle/>{content}</p:txBody>"));
        }
        package.replace_part(path, xml.into_bytes()).unwrap();
        let original = package.save().unwrap();
        let opened = execute_request(json!({"op":"open_presentation","id":"complex-notes","base64":STANDARD.encode(&original)})).unwrap();
        let document = &opened["document"];
        let error = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/slides/0/notes","value":"Replacement"}]}})).unwrap_err();
        assert!(matches!(error, aislide_core::Error::Unsupported(_)), "{error}");
        let unchanged = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        assert_eq!(STANDARD.decode(unchanged["base64"].as_str().unwrap()).unwrap(), original);
    }
}

#[test]
fn native_notes_absent_creates_one_unique_master_for_existing_and_inserted_slides() {
    let mut deck = authored(); deck["slides"].as_array_mut().unwrap().truncate(2);
    let mut parts = Package::open(exported(deck)).unwrap().parts().clone();
    let removed = parts.keys().filter(|path| path.starts_with("ppt/notes") || path.as_str() == "ppt/theme/theme2.xml").cloned().collect::<Vec<_>>();
    for path in &removed { parts.remove(path); }
    let mut package = Package::from_parts(parts).unwrap();
    for path in ["ppt/slides/_rels/slide1.xml.rels", "ppt/slides/_rels/slide2.xml.rels", "ppt/_rels/presentation.xml.rels"] {
        let xml = without_xml_nodes(package.text(path).unwrap(), |node| node.attribute("Type").is_some_and(|kind| kind.ends_with("/notesSlide") || kind.ends_with("/notesMaster")));
        package.replace_part(path, xml.into_bytes()).unwrap();
    }
    let xml = without_xml_nodes(package.text("ppt/presentation.xml").unwrap(), |node| node.tag_name().name() == "notesMasterIdLst");
    package.replace_part("ppt/presentation.xml", xml.into_bytes()).unwrap();
    let xml = without_xml_nodes(package.text("[Content_Types].xml").unwrap(), |node| node.attribute("PartName").is_some_and(|name| removed.iter().any(|path| name.trim_start_matches('/') == path)));
    package.replace_part("[Content_Types].xml", xml.into_bytes()).unwrap();
    let mut parts = Package::open(package.save().unwrap()).unwrap().parts().clone();
    for path in ["ppt/notesSlides/AISLIDE-NOTES-1.xml", "ppt/notesMasters/AISLIDE-NOTES-MASTER-1.xml", "ppt/theme/AISLIDE-NOTES-THEME-1.xml"] {
        parts.insert(path.into(), b"<reserved xmlns='urn:vendor'/>".to_vec());
    }
    parts.insert("ppt/notesSlides/_rels/aislide-notes-2.xml.rels".into(), b"<Relationships xmlns='http://schemas.openxmlformats.org/package/2006/relationships'/>".to_vec());
    let reserved_relation = "<Relationship Id=\"rIdAislideNotes1\" Type=\"urn:vendor:preserve\" Target=\"/ppt/theme/theme1.xml\"/>";
    for path in ["ppt/_rels/presentation.xml.rels", "ppt/slides/_rels/slide1.xml.rels"] {
        let xml = String::from_utf8(parts[path].clone()).unwrap().replace("</Relationships>", &format!("{reserved_relation}</Relationships>"));
        parts.insert(path.into(), xml.into_bytes());
    }
    let package = Package::from_parts(parts).unwrap();
    let original = package.save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"new-master","base64":STANDARD.encode(&original)})).unwrap();
    let mut slides = opened["document"]["deck"]["slides"].as_array().unwrap().clone();
    slides[0]["notes"] = json!("First notes"); slides[1]["notes"] = json!("Second notes");
    let mut inserted = slides[0].clone(); inserted["id"] = json!("notes-new-slide"); inserted["elements"] = json!([]); inserted["notes"] = json!("Inserted notes");
    slides.push(inserted);
    let changed = native_change(&opened["document"], json!([{"op":"replace","path":"/deck/slides","value":slides}]));
    let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let result = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    for (path, bytes) in package.parts() {
        if !["[Content_Types].xml", "ppt/presentation.xml", "ppt/_rels/presentation.xml.rels", "ppt/slides/_rels/slide1.xml.rels", "ppt/slides/_rels/slide2.xml.rels"].contains(&path.as_str()) && !path.starts_with("customXml/") {
            assert_eq!(result.part(path).unwrap(), bytes, "unrelated part: {path}");
        }
    }
    let rels = roxmltree::Document::parse(result.text("ppt/_rels/presentation.xml.rels").unwrap()).unwrap();
    let masters = rels.descendants().filter(|node| node.attribute("Type").is_some_and(|kind| kind.ends_with("/notesMaster"))).collect::<Vec<_>>();
    assert_eq!(masters.len(), 1);
    assert_eq!(masters[0].attribute("Target"), Some("/ppt/notesMasters/aislide-notes-master-2.xml"));
    assert!(result.text("ppt/_rels/presentation.xml.rels").unwrap().contains(reserved_relation));
    assert!(result.text("ppt/slides/_rels/slide1.xml.rels").unwrap().contains(reserved_relation));
    let master_rels = roxmltree::Document::parse(result.text("ppt/notesMasters/_rels/aislide-notes-master-2.xml.rels").unwrap()).unwrap();
    let theme = master_rels.descendants().find(|node| node.attribute("Type").is_some_and(|kind| kind.ends_with("/theme"))).unwrap().attribute("Target").unwrap();
    assert_eq!(theme, "/ppt/theme/aislide-notes-theme-2.xml");
    for path in result.parts().keys().filter(|path| path.ends_with(".rels")) {
        let parsed = roxmltree::Document::parse(result.text(path).unwrap()).unwrap();
        let ids = parsed.root_element().children().filter_map(|node| node.attribute("Id")).collect::<Vec<_>>();
        assert_eq!(ids.len(), ids.iter().collect::<std::collections::BTreeSet<_>>().len(), "unique relationship IDs: {path}");
    }
    let main = roxmltree::Document::parse(result.text("ppt/presentation.xml").unwrap()).unwrap();
    let entry = main.descendants().find(|node| node.tag_name().name() == "notesMasterId").unwrap();
    assert_eq!(entry.attribute(("http://schemas.openxmlformats.org/officeDocument/2006/relationships", "id")), masters[0].attribute("Id"));
    let reopened = execute_request(json!({"op":"open_presentation","id":"new-master-reopen","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][0]["notes"], "First notes");
    assert_eq!(reopened["document"]["deck"]["slides"][1]["notes"], "Second notes");
    assert_eq!(reopened["document"]["deck"]["slides"][2]["notes"], "Inserted notes");
    assert_native_undo(&opened["document"], &changed, &original);
}

#[test]
fn native_notes_absent_reuses_master_without_overwriting_orphan_parts() {
    let mut package = Package::open(exported(authored())).unwrap();
    let relations = "ppt/slides/_rels/slide1.xml.rels";
    let xml = without_xml_nodes(package.text(relations).unwrap(), |node| node.attribute("Type").is_some_and(|kind| kind.ends_with("/notesSlide")));
    package.replace_part(relations, xml.into_bytes()).unwrap();
    let original = package.save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"absent-notes","base64":STANDARD.encode(&original)})).unwrap();
    assert_eq!(opened["document"]["deck"]["slides"][0]["notes"], "");
    let changed = native_change(&opened["document"], json!([{"op":"replace","path":"/deck/slides/0/notes","value":"New notes & <evidence>\nSecond line"}]));
    let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let result = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    for (path, bytes) in package.parts() {
        if path != relations && path != "[Content_Types].xml" && !path.starts_with("customXml/") {
            assert_eq!(result.part(path).unwrap(), bytes, "unrelated part: {path}");
        }
    }
    let reopened = execute_request(json!({"op":"open_presentation","id":"absent-notes-reopen","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][0]["notes"], "New notes & <evidence>\nSecond line");
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], opened["document"]["hash"]);
    let restored = execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap();
    assert_eq!(STANDARD.decode(restored["base64"].as_str().unwrap()).unwrap(), original);
}

#[test]
fn native_order_insertion_between_unchanged_neighbors_survives_reopen() {
    let opened = execute_request(json!({"op":"open_presentation","id":"insert-order","base64":STANDARD.encode(exported(authored()))})).unwrap();
    let mut elements = opened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().clone();
    let inserted = execute_request(json!({"op":"create_object","id":"inserted-between","kind":"text"})).unwrap();
    elements.insert(1, inserted);
    let changed = native_change(&opened["document"], json!([{"op":"replace","path":"/deck/slides/0/elements","value":elements}]));
    let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"insert-order-reopen","base64":output["base64"]})).unwrap();
    let actual = reopened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
    assert_eq!(actual.iter().map(|element| &element["id"]).collect::<Vec<_>>(), elements.iter().map(|element| &element["id"]).collect::<Vec<_>>());
}

#[test]
fn native_order_nested_edits_keep_opaque_slots_ids_relations_and_undo() {
    let namespace = "http://schemas.openxmlformats.org/presentationml/2006/main";
    let text = |id: &str| json!({"id":id,"type":"text","x":10,"y":10,"width":120,"height":30,"text":id,"font_size":16,"color":"202525","bold":false});
    let inner = json!({"id":"inner","type":"group","x":0,"y":0,"width":200,"height":100,"view_width":200,"view_height":100,"children":[text("inside-1"),text("inside-2")]});
    let group = json!({"id":"outer","type":"group","x":20,"y":100,"width":500,"height":300,"view_width":500,"view_height":300,"children":[inner,text("group-label")]});
    let mut linked = text("linked"); linked["format"] = json!({"hyperlink":"https://example.invalid/native?keep=1&raw=2"});
    let mut deck = authored(); deck["slides"].as_array_mut().unwrap().truncate(1);
    deck["slides"][0]["elements"] = json!([linked,group,text("last"),text("spare")]);
    let mut package = Package::open(exported(deck)).unwrap();
    let path = "ppt/slides/slide1.xml";
    let mut xml = package.text(path).unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let first = parsed.descendants().find(|node| node.has_tag_name((namespace, "sp"))).unwrap().range().end;
    let opaque = "<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id=\"9999\" name=\"Opaque\"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"100000\" cy=\"100000\"/></p:xfrm><a:graphic><a:graphicData uri=\"urn:vendor:opaque\"><v:payload xmlns:v=\"urn:vendor\" r:id=\"rIdVendor\" untouched=\"&amp;\"/></a:graphicData></a:graphic></p:graphicFrame>";
    xml.insert_str(first, opaque);
    let nested_sentinel = "<v:opaque xmlns:v=\"urn:nested-vendor\" keep=\"raw\"/>";
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let inner = parsed.descendants().find(|node| node.has_tag_name((namespace,"grpSp")) && node.descendants().find(|child| child.has_tag_name((namespace,"cNvPr"))).is_some_and(|identity| identity.attribute("name") == Some("inner"))).unwrap();
    let position = inner.children().find(|node| node.has_tag_name((namespace,"grpSpPr"))).unwrap().range().end;
    xml.insert_str(position, nested_sentinel);
    package.replace_part(path, xml.into_bytes()).unwrap();
    let rel_path = "ppt/slides/_rels/slide1.xml.rels";
    let rel = "<Relationship Id=\"rIdVendor\" Type=\"urn:vendor:opaque\" Target=\"/ppt/vendor/opaque.bin\"/>";
    let xml = package.text(rel_path).unwrap().replace("</Relationships>", &format!("{rel}</Relationships>"));
    package.replace_part(rel_path, xml.into_bytes()).unwrap();
    let mut parts = Package::open(package.save().unwrap()).unwrap().parts().clone();
    parts.insert("ppt/vendor/opaque.bin".into(), vec![0, 13, 255, 42]);
    let package = Package::from_parts(parts).unwrap(); let original = package.save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"nested-order","base64":STANDARD.encode(&original)})).unwrap();
    assert_eq!(opened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().len(), 4);
    let shapes = |source: &str| {
        let parsed = roxmltree::Document::parse(source).unwrap();
        parsed.descendants().filter(|node| node.tag_name().namespace() == Some(namespace) && ["sp","grpSp","graphicFrame"].contains(&node.tag_name().name()))
            .map(|node| { let identity = node.descendants().find(|child| child.has_tag_name((namespace,"cNvPr"))).unwrap(); (identity.attribute("name").unwrap().to_owned(), (identity.attribute("id").unwrap().to_owned(), source[node.range()].to_owned())) }).collect::<std::collections::BTreeMap<_,_>>()
    };
    let before = shapes(package.text(path).unwrap());
    for mode in ["reorder", "insert", "delete"] {
        let mut elements = opened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().clone();
        elements[1]["children"][0]["children"].as_array_mut().unwrap().swap(0,1);
        elements[1]["children"][0]["children"][0]["text"] = json!("Nested edit & <raw>");
        elements[1]["children"].as_array_mut().unwrap().swap(0,1);
        elements[2]["text"] = json!("Top-level edit"); elements.swap(0,2);
        if mode == "insert" { let mut inserted = elements[3].clone(); inserted["id"] = json!("inserted-native"); inserted["text"] = json!("inserted-native"); elements.insert(1, inserted); }
        if mode == "delete" { elements.pop(); }
        let changed = native_change(&opened["document"], json!([{"op":"replace","path":"/deck/slides/0/elements","value":elements}]));
        let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        let result = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
        let source = result.text(path).unwrap();
        assert!(source.contains(opaque), "{mode}"); assert!(source.contains(nested_sentinel), "{mode}");
        let parsed = roxmltree::Document::parse(source).unwrap();
        let tree = parsed.descendants().find(|node| node.has_tag_name((namespace,"spTree"))).unwrap();
        let slot = tree.children().filter(|node| node.tag_name().namespace() == Some(namespace) && ["sp","grpSp","graphicFrame"].contains(&node.tag_name().name())).nth(1).unwrap();
        assert_eq!(&source[slot.range()], opaque, "opaque slot: {mode}");
        let after = shapes(source);
        for (name, (id, raw)) in &before {
            if mode == "delete" && name == "spare" { assert!(!after.contains_key(name)); continue; }
            assert_eq!(&after[name].0, id, "identity: {name}, {mode}");
            if !["outer","inner","inside-2","last"].contains(&name.as_str()) { assert_eq!(&after[name].1, raw, "raw shape: {name}, {mode}"); }
        }
        let ids = parsed.descendants().filter(|node| node.has_tag_name((namespace,"cNvPr"))).map(|node| node.attribute("id").unwrap()).collect::<Vec<_>>();
        assert_eq!(ids.len(), ids.iter().collect::<std::collections::BTreeSet<_>>().len());
        for part in package.parts().keys().filter(|part| part.as_str() != path && !part.starts_with("customXml/")) { assert_eq!(result.part(part).unwrap(), package.part(part).unwrap(), "unrelated part: {part}, {mode}"); }
        let reopened = execute_request(json!({"op":"open_presentation","id":"nested-order-reopen","base64":output["base64"]})).unwrap();
        assert_eq!(reopened["document"]["deck"]["slides"][0]["elements"], json!(elements), "{mode}");
        let unchanged = execute_request(json!({"op":"export_presentation","document":reopened["document"]})).unwrap(); assert_eq!(unchanged["base64"], output["base64"]);
        assert_native_undo(&opened["document"], &changed, &original);
    }
}

#[test]
fn native_notes_duplicate_uses_current_binding_not_source_path() {
    let mut package = Package::open(exported(authored())).unwrap();
    let path = "ppt/notesSlides/notesSlide1.xml";
    let sentinel = "<p:extLst><p:ext uri=\"urn:copy-notes-raw\"/></p:extLst>";
    let xml = package.text(path).unwrap().replace("</p:notes>", &format!("{sentinel}</p:notes>"));
    package.replace_part(path, xml.into_bytes()).unwrap();
    let original = package.save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"copy-notes","base64":STANDARD.encode(&original)})).unwrap();
    let copied = execute_request(json!({"op":"edit_slides","document":opened["document"],"expected_revision":0,"operations":[{"op":"duplicate","slide_id":"slide-1","id":"notes-copy"}]})).unwrap();
    let copy_saved = execute_request(json!({"op":"export_presentation","document":copied["document"]})).unwrap();
    let changed = native_change(&copied["document"], json!([
        {"op":"replace","path":"/deck/slides/0/notes","value":"Source notes revised"},
        {"op":"replace","path":"/deck/slides/1/notes","value":"Copy notes revised independently"}
    ]));
    let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let result = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    let copy_path = "ppt/notesSlides/aislide-added-1.xml";
    assert!(result.text(path).unwrap().contains("Source notes revised"));
    assert!(!result.text(path).unwrap().contains("Copy notes revised independently"));
    assert!(result.text(copy_path).unwrap().contains("Copy notes revised independently"));
    assert!(result.text(copy_path).unwrap().contains(sentinel));
    assert_eq!(result.part("ppt/notesSlides/_rels/notesSlide1.xml.rels").unwrap(), package.part("ppt/notesSlides/_rels/notesSlide1.xml.rels").unwrap());
    let rels = roxmltree::Document::parse(result.text("ppt/notesSlides/_rels/aislide-added-1.xml.rels").unwrap()).unwrap();
    let slide = rels.descendants().find(|node| node.attribute("Type").is_some_and(|kind| kind.ends_with("/slide"))).unwrap();
    assert_eq!(slide.attribute("Target"), Some("/ppt/slides/aislide-added-1.xml"));
    let reopened = execute_request(json!({"op":"open_presentation","id":"copy-notes-reopen","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][0]["notes"], "Source notes revised");
    assert_eq!(reopened["document"]["deck"]["slides"][1]["notes"], "Copy notes revised independently");
    assert_native_undo(&copied["document"], &changed, &STANDARD.decode(copy_saved["base64"].as_str().unwrap()).unwrap());
    assert_native_undo(&opened["document"], &copied, &original);
}

#[test]
fn native_notes_shared_targets_reject_without_mutating_other_slides() {
    let mut package = Package::open(exported(authored())).unwrap();
    let path = "ppt/slides/_rels/slide2.xml.rels";
    let mut xml = package.text(path).unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let relation = parsed.root_element().children().find(|node| node.attribute("Type").is_some_and(|kind| kind.ends_with("/notesSlide"))).unwrap();
    let target = relation.attributes().find(|attribute| attribute.name() == "Target").unwrap().range_value();
    xml.replace_range(target, "../notesSlides/./notesSlide1.xml");
    package.replace_part(path, xml.into_bytes()).unwrap();
    let original = package.save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"shared-notes","base64":STANDARD.encode(&original)})).unwrap();
    let document = &opened["document"];
    assert_eq!(document["deck"]["slides"][0]["notes"], document["deck"]["slides"][1]["notes"]);
    for index in [0, 1] {
        let error = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"replace","path":format!("/deck/slides/{index}/notes"),"value":"Must not change another slide"}]}})).err().expect("shared notes edit must be rejected");
        assert!(matches!(error, aislide_core::Error::Unsupported(_)), "{error}");
        assert!(error.to_string().contains("shared notes"), "{error}");
    }
    let unchanged = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    assert_eq!(STANDARD.decode(unchanged["base64"].as_str().unwrap()).unwrap(), original);
}

#[test]
fn reopened_notes_edit_preserves_native_notes_parts_and_undo() {
    let mut package = Package::open(exported(authored())).unwrap();
    let note_path = "ppt/notesSlides/notesSlide1.xml";
    let notes = package.text(note_path).unwrap().replace("</p:notes>", "<p:extLst><p:ext uri=\"urn:notes-preserve\"><v:data xmlns:v=\"urn:vendor\" value=\"retain\"/></p:ext></p:extLst></p:notes>");
    package.replace_part(note_path, notes.into_bytes()).unwrap();
    let original = package.save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"notes-open","base64":STANDARD.encode(&original)})).unwrap();
    let changed = execute_request(json!({"op":"transaction","document":opened["document"],"transaction":{"expected_revision":0,"expected_hash":opened["document"]["hash"],"operations":[{"op":"replace","path":"/deck/slides/0/notes","value":"Revised notes\nSecond paragraph & <evidence>"}]}})).unwrap();
    let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let result = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert!(result.text(note_path).unwrap().contains("urn:notes-preserve"));
    for path in ["ppt/notesSlides/_rels/notesSlide1.xml.rels", "ppt/notesMasters/notesMaster1.xml", "ppt/slides/slide1.xml", "ppt/slides/slide2.xml"] {
        assert_eq!(result.part(path).unwrap(), package.part(path).unwrap(), "non-target part: {path}");
    }
    let reopened = execute_request(json!({"op":"open_presentation","id":"notes-reopen","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][0]["notes"], "Revised notes\nSecond paragraph & <evidence>");
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":1,"receipt":changed["receipt"]})).unwrap();
    let restored = execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap();
    assert_eq!(STANDARD.decode(restored["base64"].as_str().unwrap()).unwrap(), original);
}

#[test]
fn reopened_object_order_preserves_raw_shapes_and_undo() {
    let mut package = Package::open(exported(authored())).unwrap();
    let path = "ppt/slides/slide1.xml";
    let xml = package.text(path).unwrap().replace("</p:sld>", "<p:extLst><p:ext uri=\"urn:order-preserve\"/></p:extLst></p:sld>");
    package.replace_part(path, xml.into_bytes()).unwrap();
    let original = package.save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"order-open","base64":STANDARD.encode(&original)})).unwrap();
    let elements = opened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
    let first_id = elements[0]["id"].as_str().unwrap();
    let changed = execute_request(json!({"op":"edit_elements","document":opened["document"],"expected_revision":0,"slide_id":"slide-1","operations":[{"op":"order","id":first_id,"index":elements.len()-1}]})).unwrap();
    let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let result = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    let original_xml = roxmltree::Document::parse(package.text(path).unwrap()).unwrap();
    let updated_xml = roxmltree::Document::parse(result.text(path).unwrap()).unwrap();
    let namespace = "http://schemas.openxmlformats.org/presentationml/2006/main";
    let raw_shapes = |document: &roxmltree::Document<'_>, source: &str| {
        let mut shapes = document.descendants().find(|node| node.has_tag_name((namespace, "spTree"))).unwrap().children().filter(|node| node.has_tag_name((namespace, "sp"))).map(|node| source[node.range()].to_owned()).collect::<Vec<_>>();
        shapes.sort();
        shapes
    };
    assert_eq!(raw_shapes(&original_xml, package.text(path).unwrap()), raw_shapes(&updated_xml, result.text(path).unwrap()));
    assert!(result.text(path).unwrap().contains("urn:order-preserve"));
    assert_eq!(result.part("ppt/slides/slide2.xml").unwrap(), package.part("ppt/slides/slide2.xml").unwrap());
    let reopened = execute_request(json!({"op":"open_presentation","id":"order-reopen","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().last().unwrap()["id"], first_id);
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":1,"receipt":changed["receipt"]})).unwrap();
    let restored = execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap();
    assert_eq!(STANDARD.decode(restored["base64"].as_str().unwrap()).unwrap(), original);
}

#[test]
fn slide_commands_create_blank_insert_duplicate_reorder_and_undo() {
    let document = execute_request(json!({"op":"create_presentation","id":"blank","title":"Untitled presentation"})).unwrap();
    assert_eq!(document["deck"]["slides"].as_array().unwrap().len(), 1);
    assert!(document["deck"]["slides"][0]["elements"].as_array().unwrap().is_empty());
    let changed = execute_request(json!({"op":"edit_slides","document":document,"expected_revision":0,"operations":[
        {"op":"insert","id":"next","after":"slide-1","title":"Next slide"},
        {"op":"duplicate","slide_id":"next","id":"copy"},
        {"op":"move","slide_id":"copy","index":0}
    ]})).unwrap();
    assert_eq!(changed["document"]["deck"]["slides"][0]["id"], "copy");
    assert_eq!(changed["document"]["deck"]["slides"].as_array().unwrap().len(), 3);
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":1,"receipt":changed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], document["hash"]);
    assert!(execute_request(json!({"op":"edit_slides","document":document,"expected_revision":0,"operations":[{"op":"remove","slide_id":"slide-1"}]})).is_err());
}

#[test]
fn slide_commands_on_reopened_pptx_preserve_native_parts_and_original_bytes() {
    let document = execute_request(json!({"op":"create_presentation","id":"native-slides","title":"Native slide operations"})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    let mut parts = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap().parts().clone();
    parts.insert("ppt/vendor/preserved.bin".into(), vec![2, 4, 6, 8]);
    let original = Package::from_parts(parts).unwrap().save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"native-slides-open","base64":STANDARD.encode(&original)})).unwrap();
    let changed = execute_request(json!({"op":"edit_slides","document":opened["document"],"expected_revision":0,"operations":[
        {"op":"insert","id":"inserted","after":"slide-1","title":"Inserted slide"},
        {"op":"move","slide_id":"inserted","index":0}
    ]})).unwrap();
    let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let result = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(result.part("ppt/vendor/preserved.bin").unwrap(), &[2, 4, 6, 8]);
    let reopened = execute_request(json!({"op":"open_presentation","id":"native-slides-reopen","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][0]["title"], "Inserted slide");
    assert_eq!(reopened["document"]["deck"]["slides"].as_array().unwrap().len(), 2);
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":1,"receipt":changed["receipt"]})).unwrap();
    let restored = execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap();
    assert_eq!(STANDARD.decode(restored["base64"].as_str().unwrap()).unwrap(), original);
}

#[test]
fn native_slide_duplicate_preserves_opaque_xml_graph_metadata_and_identity() {
    let document = execute_request(json!({"op":"create_presentation","id":"copy-native","title":"Copy test"})).unwrap();
    let catalog = execute_request(json!({"op":"graph_catalog"})).unwrap();
    let inserted = execute_request(json!({"op":"insert_graph","document":document,"expected_revision":0,"slide_id":"slide-1","id":"diagram","spec":catalog["examples"][0]["spec"]})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":inserted["document"]})).unwrap();
    let mut package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().replace("</p:sld>", "<p:extLst><p:ext uri=\"urn:vendor:slide\"><v:setting xmlns:v=\"urn:vendor\" value=\"preserve\"/></p:ext></p:extLst></p:sld>");
    package.replace_part("ppt/slides/slide1.xml", xml.clone().into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"copy-open","base64":STANDARD.encode(&bytes)})).unwrap();
    let duplicated = execute_request(json!({"op":"edit_slides","document":opened["document"],"expected_revision":0,"operations":[{"op":"duplicate","slide_id":"slide-1","id":"copy-slide"}]})).unwrap();
    assert_eq!(duplicated["document"]["parts"].as_array().unwrap().len(), 2);
    assert!(duplicated["document"]["parts"].as_array().unwrap().iter().all(|part| part["stale"] == false));
    let output = execute_request(json!({"op":"export_presentation","document":duplicated["document"]})).unwrap();
    let result = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(result.text("ppt/slides/slide1.xml").unwrap(), xml);
    assert!(result.text("ppt/slides/aislide-added-1.xml").unwrap().contains("urn:vendor:slide"));
    let reopened = execute_request(json!({"op":"open_presentation","id":"copy-reopen","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][1]["id"], "copy-slide");
    assert!(reopened["document"]["parts"].as_array().unwrap().iter().all(|part| part["stale"] == false));
    let moved = execute_request(json!({"op":"apply_graph","document":reopened["document"],"expected_revision":0,"slide_id":"copy-slide","id":"diagram","operations":[{"op":"move","ids":["user"],"dx":16,"dy":0}]})).unwrap();
    execute_request(json!({"op":"export_presentation","document":moved["document"]})).unwrap();
    let removed = execute_request(json!({"op":"edit_slides","document":reopened["document"],"expected_revision":0,"operations":[{"op":"remove","slide_id":"slide-1"}]})).unwrap();
    assert_eq!(removed["document"]["parts"].as_array().unwrap().len(), 1);
    let undone = execute_request(json!({"op":"undo_transaction","document":removed["document"],"expected_revision":1,"receipt":removed["receipt"]})).unwrap();
    assert_eq!(undone["document"]["hash"], reopened["document"]["hash"]);
}

#[test]
fn native_slide_chart_copies_have_independent_embedded_workbooks() {
    let blank = execute_request(json!({"op":"create_presentation","id":"chart-copy","title":"Chart copy"})).unwrap();
    let mut deck = blank["deck"].clone();
    deck["slides"][0]["elements"] = json!([execute_request(json!({"op":"create_object","id":"chart","kind":"chart","preset":"column"})).unwrap()]);
    let document = execute_request(json!({"op":"new_document","id":"chart-copy-source","deck":deck})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"chart-copy-open","base64":saved["base64"]})).unwrap();
    let copied = execute_request(json!({"op":"edit_slides","document":opened["document"],"expected_revision":0,"operations":[{"op":"duplicate","slide_id":"slide-1","id":"chart-copy-slide"}]})).unwrap();
    let output = execute_request(json!({"op":"export_presentation","document":copied["document"]})).unwrap();
    let package = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    let relations = roxmltree::Document::parse(package.text("ppt/slides/_rels/aislide-added-1.xml.rels").unwrap()).unwrap();
    let copied_chart = relations.root_element().children().find(|node| node.attribute("Type").is_some_and(|kind| kind.ends_with("/chart"))).unwrap().attribute("Target").unwrap().trim_start_matches('/');
    assert_ne!(copied_chart, "ppt/charts/chart1.xml", "copy must not share its mutable native chart");
    assert_eq!(package.part(copied_chart).unwrap(), package.part("ppt/charts/chart1.xml").unwrap());
    let workbooks: Vec<_> = package.parts().iter().filter(|(name, _)| name.ends_with(".xlsx")).collect();
    assert_eq!(workbooks.len(), 2, "both slides need an independent workbook");
    assert_eq!(workbooks[0].1, workbooks[1].1);
    let reopened = execute_request(json!({"op":"open_presentation","id":"chart-copy-reopen","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"].as_array().unwrap().len(), 2);
}

#[test]
fn removing_native_slides_removes_their_slide_and_notes_parts() {
    let blank = execute_request(json!({"op":"create_presentation","id":"remove-native","title":"Remove slide"})).unwrap();
    let inserted = execute_request(json!({"op":"edit_slides","document":blank,"expected_revision":0,"operations":[{"op":"insert","id":"remove-me","title":"Deleted slide sentinel"}]})).unwrap();
    let mut document = inserted["document"].clone();
    let updated = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/slides/1/notes","value":"Deleted notes sentinel"}]}})).unwrap();
    document = updated["document"].clone();
    let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"remove-native-open","base64":saved["base64"]})).unwrap();
    let removed = execute_request(json!({"op":"edit_slides","document":opened["document"],"expected_revision":0,"operations":[{"op":"remove","slide_id":"remove-me"}]})).unwrap();
    let output = execute_request(json!({"op":"export_presentation","document":removed["document"]})).unwrap();
    let package = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert!(!package.parts().contains_key("ppt/slides/slide2.xml"));
    assert!(!package.parts().contains_key("ppt/notesSlides/notesSlide2.xml"));
    assert!(!package.parts().values().any(|bytes| String::from_utf8_lossy(bytes).contains("Deleted notes sentinel")));
    let reopened = execute_request(json!({"op":"open_presentation","id":"remove-native-final","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"].as_array().unwrap().len(), 1);
}

#[test]
fn native_presentation_title_can_be_renamed_without_replacing_slides() {
    let bytes = exported(authored());
    let opened = execute_request(json!({"op":"open_presentation","id":"rename-native","base64":STANDARD.encode(&bytes)})).unwrap();
    let changed = execute_request(json!({"op":"transaction","document":opened["document"],"transaction":{"expected_revision":0,"expected_hash":opened["document"]["hash"],"operations":[{"op":"replace","path":"/deck/title","value":"Renamed presentation"}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"renamed","base64":saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["title"], "Renamed presentation");
    let result = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(result.part("ppt/slides/slide1.xml").unwrap(), Package::open(bytes).unwrap().part("ppt/slides/slide1.xml").unwrap());
}

#[test]
fn pptx_alone_restores_native_theme_layout_text_shapes_and_notes() {
    let deck = authored();
    let bytes = exported(deck.clone());
    let opened = execute_request(json!({"op":"open_presentation","id":"standalone","base64":STANDARD.encode(&bytes)})).unwrap();
    let restored = &opened["document"]["deck"];
    assert_eq!(restored["title"], deck["title"]);
    assert_eq!(restored["slides"].as_array().unwrap().len(), 12);
    assert_eq!(restored["design"]["theme"]["colors"]["accent1"], "B53055");
    assert_eq!(restored["design"]["theme"]["fonts"]["major"], "Arial");
    assert_eq!(restored["design"]["layouts"].as_array().unwrap().len(), 4);
    assert!(restored["slides"][1]["elements"].as_array().unwrap().iter().any(|element| element["type"] == "shape" && element["preset"] == "ellipse"));
    assert!(restored["slides"][2]["elements"].as_array().unwrap().iter().any(|element| element["text"] == "Three signals.\nOne direction." || element["format"]["placeholder"]["kind"] == "title"));
    assert_eq!(restored["slides"][0]["notes"], deck["slides"][0]["notes"]);
    let saved = execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap();
    assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), bytes);
    assert!(saved.get("checkpoint").is_none());
}

#[test]
fn actual_open_xml_is_authoritative_without_an_external_snapshot() {
    let mut package = Package::open(exported(authored())).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().replace("Quarterly performance", "Changed in PowerPoint");
    package.replace_part("ppt/slides/slide1.xml", xml.into_bytes()).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"external-edit","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    assert!(opened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().any(|element| element["text"].as_str().is_some_and(|text| text.contains("Changed in PowerPoint"))));
}

#[test]
fn unknown_package_parts_survive_supported_single_file_edits() {
    let mut parts = Package::open(exported(authored())).unwrap().parts().clone();
    parts.insert("ppt/opaque/vendor.bin".into(), vec![0, 17, 254, 255]);
    let bytes = Package::from_parts(parts).unwrap().save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"opaque","base64":STANDARD.encode(&bytes)})).unwrap();
    let document = &opened["document"];
    let elements = document["deck"]["slides"][0]["elements"].as_array().unwrap();
    let index = elements.iter().position(|element| element["type"] == "text" && element["text"].as_str().is_some_and(|text| text.contains("Quarterly"))).unwrap();
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":format!("/deck/slides/0/elements/{index}/text"),"value":"Single file editing\nNative XML"}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(package.part("ppt/opaque/vendor.bin").unwrap(), &[0, 17, 254, 255]);
    let reopened = execute_request(json!({"op":"open_presentation","id":"reopened","base64":saved["base64"]})).unwrap();
    assert!(reopened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().any(|element| element["text"] == "Single file editing\nNative XML"));
}

#[test]
fn protected_or_legacy_containers_are_identified_before_zip_or_checkpoint_errors() {
    let bytes = [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1, 0, 0, 0, 0];
    let error = execute_request(json!({"op":"open_presentation","id":"protected","base64":STANDARD.encode(bytes)})).unwrap_err().to_string();
    assert!(error.contains("encrypted") && error.contains("legacy"), "{error}");
    assert!(!error.contains("checkpoint"), "{error}");
}

#[test]
fn reopened_pptx_supports_text_geometry_table_and_theme_edits() {
    let bytes = exported(authored());
    let opened = execute_request(json!({"op":"open_presentation","id":"native-edit","base64":STANDARD.encode(bytes)})).unwrap();
    let document = &opened["document"];
    let mut deck = document["deck"].clone();
    let title = deck["slides"][0]["elements"].as_array_mut().unwrap().iter_mut().find(|element| element["id"] == "title").unwrap();
    title["text"] = json!("Native edit\nThree lines\nNo sidecar");
    title["font_size"] = json!(36);
    title["x"] = json!(75);
    title["width"] = json!(1000);
    title["format"]["italic"] = json!(true);
    let table = deck["slides"][3]["elements"].as_array_mut().unwrap().iter_mut().find(|element| element["type"] == "table").unwrap();
    table["rows"][1][0] = json!("Edited native cell");
    deck["design"]["theme"]["colors"]["accent1"] = json!("2468AC");
    deck["design"]["theme"]["fonts"]["minor"] = json!("Arial");
    let result = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck","value":deck}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":result["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"edited","base64":saved["base64"]})).unwrap();
    let deck = &reopened["document"]["deck"];
    let title = deck["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["id"] == "title").unwrap();
    assert_eq!(title["text"], "Native edit\nThree lines\nNo sidecar");
    assert_eq!(title["x"], 75.0);
    assert_eq!(title["font_size"], 36.0);
    assert_eq!(title["format"]["italic"], true);
    assert_eq!(deck["design"]["theme"]["colors"]["accent1"], "2468AC");
    assert!(deck["slides"][3]["elements"].as_array().unwrap().iter().any(|element| element["rows"][1][0] == "Edited native cell"));
}

#[test]
fn native_master_layout_edits_and_new_shapes_survive_reopen() {
    let opened = execute_request(json!({"op":"open_presentation","id":"design-edit","base64":STANDARD.encode(exported(authored()))})).unwrap();
    let document = &opened["document"];
    let mut design = document["deck"]["design"].clone();
    design["layouts"][1]["elements"][0]["x"] = json!(80);
    design["layouts"][1]["elements"][0]["width"] = json!(1080);
    design["masters"][0]["elements"].as_array_mut().unwrap().push(json!({"id":"master-text","type":"text","x":64,"y":668,"width":400,"height":28,"text":"New common footer","font_size":14,"color":"@dk2","bold":false}));
    let mut deck = execute_request(json!({"op":"update_design","deck":document["deck"],"design":design})).unwrap();
    deck["slides"][1]["elements"].as_array_mut().unwrap().push(execute_request(json!({"op":"create_object","id":"new-shape","kind":"shape","preset":"diamond"})).unwrap());
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck","value":deck}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"design-reopen","base64":saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["design"]["layouts"][1]["elements"][0]["x"], 80.0);
    assert!(reopened["document"]["deck"]["slides"][2]["elements"].as_array().unwrap().iter().any(|element| element["format"]["inherit_layout"] == true && element["x"] == 80.0));
    assert!(reopened["document"]["deck"]["design"]["masters"][0]["elements"].as_array().unwrap().iter().any(|element| element["text"] == "New common footer"));
    assert!(reopened["document"]["deck"]["slides"][1]["elements"].as_array().unwrap().iter().any(|element| element["id"] == "new-shape"));
}

#[test]
fn new_embedded_images_charts_and_links_get_unique_native_relationships() {
    let opened = execute_request(json!({"op":"open_presentation","id":"resources","base64":STANDARD.encode(exported(authored()))})).unwrap();
    let document = &opened["document"];
    let mut deck = document["deck"].clone();
    for kind in ["pie", "doughnut", "area", "scatter", "stacked_column", "stacked_bar", "line", "column", "bar"] {
        let chart = execute_request(json!({"op":"create_object","id":format!("chart-{kind}"),"kind":"chart","preset":kind})).unwrap();
        deck["slides"][1]["elements"].as_array_mut().unwrap().push(chart);
    }
    let mut image_bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(8, 8, image::Rgb([20, 90, 140]))).write_to(&mut image_bytes, image::ImageFormat::Png).unwrap();
    let picture = execute_request(json!({"op":"create_picture","id":"new-picture","base64":STANDARD.encode(image_bytes.into_inner()),"mime_type":"image/png","alt":"Synthetic pixels"})).unwrap();
    deck["slides"][1]["elements"].as_array_mut().unwrap().push(picture);
    let mut label = execute_request(json!({"op":"create_object","id":"linked-label","kind":"text"})).unwrap();
    label["format"] = json!({"hyperlink":"https://example.invalid/?left=1&right=2"});
    deck["slides"][1]["elements"].as_array_mut().unwrap().push(label);
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck","value":deck}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"resources-reopened","base64":saved["base64"]})).unwrap();
    let elements = reopened["document"]["deck"]["slides"][1]["elements"].as_array().unwrap();
    assert_eq!(elements.iter().filter(|element| element["type"] == "chart").count(), 9);
    assert!(elements.iter().any(|element| element["type"] == "picture" && element["alt"] == "Synthetic pixels"));
    assert!(elements.iter().any(|element| element["format"]["hyperlink"] == "https://example.invalid/?left=1&right=2"));
}

#[test]
fn sources_and_bindings_travel_inside_pptx_without_an_embedded_deck_snapshot() {
    let source = execute_request(json!({"op":"ingest","input":{"name":"fixture.csv","format":"csv","base64":STANDARD.encode(b"Quarter,Value\nQ1,10\nQ2,12\n")}})).unwrap();
    let report = execute_request(json!({"op":"data_report","source":source,"mapping":{"title":"Source-bound single file","period":"Synthetic fixture","table_index":0,"category_column":0,"value_columns":[1],"row_start":0,"row_count":2,"chart_kind":"column"}})).unwrap();
    let document = execute_request(json!({"op":"new_document","id":"sources","deck":report["compiled"]["deck"],"sources":[source],"bindings":report["bindings"]})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    assert!(saved.get("checkpoint").is_none());
    let package = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    let metadata: Vec<_> = package.parts().iter().filter(|(name, _)| name.starts_with("customXml/") && name.ends_with(".xml")).collect();
    assert!(!metadata.is_empty());
    for (_, bytes) in metadata { let xml = std::str::from_utf8(bytes).unwrap(); assert!(!xml.contains("\"deck\"")); assert!(!xml.contains("name=\"deck\"")); }
    let opened = execute_request(json!({"op":"open_presentation","id":"sources-reopened","base64":saved["base64"]})).unwrap();
    assert_eq!(opened["document"]["sources"][0]["sha256"], source["sha256"]);
    assert_eq!(opened["document"]["bindings"].as_array().unwrap().len(), report["bindings"].as_array().unwrap().len());
    assert!(opened["document"]["bindings"].as_array().unwrap().iter().all(|binding| binding["stale"] == false));
    let reexported = execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap();
    assert!(reexported["base64"] == saved["base64"], "unmodified single-file project must remain byte-identical");
}

#[test]
fn explicit_local_placeholder_styles_override_inherited_template_styles() {
    let mut package = Package::open(exported(authored())).unwrap();
    let xml = package.text("ppt/slides/slide3.xml").unwrap().replace("<a:rPr lang=\"ja-JP\"/>", "<a:rPr lang=\"ja-JP\" sz=\"1800\" b=\"0\"/>");
    package.replace_part("ppt/slides/slide3.xml", xml.into_bytes()).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"local-style","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    let title = opened["document"]["deck"]["slides"][2]["elements"].as_array().unwrap().iter().find(|element| element["format"]["placeholder"]["kind"] == "title").unwrap();
    assert_eq!(title["font_size"], 24.0);
    assert_eq!(title["bold"], false);
    assert_eq!(title["format"]["inherit_layout"], false);
}

#[test]
fn unrepresented_text_and_chart_formatting_is_preserved_or_edit_is_rejected() {
    let mut deck = authored();
    deck["slides"][1]["elements"].as_array_mut().unwrap().push(execute_request(json!({"op":"create_object","id":"native-chart","kind":"chart","preset":"column"})).unwrap());
    let mut package = Package::open(exported(deck)).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().replace("<a:bodyPr ", "<a:bodyPr vert=\"vert270\" ");
    package.replace_part("ppt/slides/slide1.xml", xml.into_bytes()).unwrap();
    let chart = package.text("ppt/charts/chart1.xml").unwrap().replace("<c:gapWidth", "<c:dLbls><c:showVal val=\"1\"/></c:dLbls><c:gapWidth");
    package.replace_part("ppt/charts/chart1.xml", chart.into_bytes()).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"complex","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    let document = &opened["document"];
    let title = document["deck"]["slides"][0]["elements"].as_array().unwrap().iter().position(|element| element["id"] == "title").unwrap();
    let chart = document["deck"]["slides"][1]["elements"].as_array().unwrap().iter().position(|element| element["id"] == "native-chart").unwrap();
    for operation in [json!({"op":"replace","path":format!("/deck/slides/0/elements/{title}/font_size"),"value":36}), json!({"op":"replace","path":format!("/deck/slides/1/elements/{chart}/series/0/values/0"),"value":25})] {
        let result = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[operation]}}));
        assert!(result.is_err(), "unrepresented formatting must not be silently dropped");
    }
}

#[test]
fn switching_a_reopened_slide_to_blank_removes_its_placeholder_marker() {
    let opened = execute_request(json!({"op":"open_presentation","id":"switch","base64":STANDARD.encode(exported(authored()))})).unwrap();
    let document = &opened["document"];
    let blank = document["deck"]["design"]["layouts"][0]["id"].as_str().unwrap();
    let slide_id = document["deck"]["slides"][2]["id"].as_str().unwrap();
    let deck = execute_request(json!({"op":"assign_layout","deck":document["deck"],"slide_id":slide_id,"layout_id":blank})).unwrap();
    let result = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck","value":deck}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":result["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"blank","base64":saved["base64"]})).unwrap();
    assert!(reopened["document"]["deck"]["slides"][2]["elements"].as_array().unwrap().iter().all(|element| element["format"]["placeholder"].is_null()));
}

#[test]
fn native_insertions_use_the_original_presentations_coordinate_scale() {
    let mut package = Package::open(exported(authored())).unwrap();
    let xml = package.text("ppt/presentation.xml").unwrap().replace("cx=\"12192000\" cy=\"6858000\"", "cx=\"24384000\" cy=\"13716000\"");
    package.replace_part("ppt/presentation.xml", xml.into_bytes()).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"other-dimensions","base64":STANDARD.encode(package.save().unwrap())})).unwrap();
    let document = &opened["document"];
    let shape = execute_request(json!({"op":"create_object","id":"coordinate-probe","kind":"shape","preset":"rect"})).unwrap();
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[{"op":"add","path":"/deck/slides/0/elements/-","value":shape}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"coordinate-reopen","base64":saved["base64"]})).unwrap();
    let inserted = reopened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["id"] == "coordinate-probe").unwrap();
    for field in ["x", "y", "width", "height"] { assert_eq!(inserted[field], shape[field], "{field}"); }
}

#[test]
fn native_replacements_reject_unrepresented_paragraph_and_table_styles() {
    for table_case in [false, true] {
        let mut deck = authored();
        let title = deck["slides"][0]["elements"].as_array_mut().unwrap().iter_mut().find(|element| element["id"] == "title").unwrap();
        title["text"] = json!("First paragraph\nSecond paragraph");
        let mut package = Package::open(exported(deck)).unwrap();
        let path = if table_case { "ppt/slides/slide4.xml" } else { "ppt/slides/slide1.xml" };
        let mut xml = package.text(path).unwrap().to_owned();
        let parsed = roxmltree::Document::parse(&xml).unwrap();
        let drawing = "http://schemas.openxmlformats.org/drawingml/2006/main";
        let target = if table_case {
            parsed.descendants().find(|node| node.has_tag_name((drawing, "gridCol"))).unwrap()
        } else {
            let title = parsed.descendants().find(|node| node.attribute("name") == Some("title")).unwrap().parent().unwrap().parent().unwrap();
            title.descendants().filter(|node| node.has_tag_name((drawing, "pPr"))).nth(1).unwrap()
        };
        let range = target.range();
        let replacement = if table_case { "<a:gridCol w=\"777777\"/>" } else { "<a:pPr algn=\"r\"><a:lnSpc><a:spcPct val=\"200000\"/></a:lnSpc></a:pPr>" };
        xml.replace_range(range, replacement);
        package.replace_part(path, xml.into_bytes()).unwrap();
        let bytes = package.save().unwrap();
        let opened = execute_request(json!({"op":"open_presentation","id":"styled","base64":STANDARD.encode(&bytes)})).unwrap();
        let document = &opened["document"];
        let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), bytes);
        let slide_index = if table_case { 3 } else { 0 };
        let element_index = document["deck"]["slides"][slide_index]["elements"].as_array().unwrap().iter().position(|element| if table_case { element["type"] == "table" } else { element["id"] == "title" }).unwrap();
        let operation = if table_case {
            let row = document["deck"]["slides"][slide_index]["elements"][element_index]["rows"][1].clone();
            json!({"op":"add","path":format!("/deck/slides/{slide_index}/elements/{element_index}/rows/-"),"value":row})
        } else { json!({"op":"replace","path":format!("/deck/slides/0/elements/{element_index}/font_size"),"value":36}) };
        let result = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":0,"expected_hash":document["hash"],"operations":[operation]}}));
        assert!(result.is_err(), "unrepresented {} formatting must survive or reject replacement", if table_case { "table" } else { "paragraph" });
    }
}

#[test]
fn rich_notes_format_only_round_trip_preserves_body_identity_and_unknown_parts() {
    let mut deck = authored();
    deck["slides"][0]["notes"] = json!("Rich notes");
    deck["slides"][0]["notes_paragraphs"] = json!([{"runs":[{"text":"Rich ","style":{"bold":true}},{"text":"notes","style":{"italic":true}}]}]);
    let mut package = Package::open(exported(deck)).unwrap();
    let path = "ppt/notesSlides/notesSlide1.xml";
    let sentinel = "<p:extLst><p:ext uri=\"urn:rich-notes\"><v:data xmlns:v=\"urn:vendor\" keep=\"true\"/></p:ext></p:extLst>";
    package.replace_part(path, package.text(path).unwrap().replace("</p:notes>", &format!("{sentinel}</p:notes>")).into_bytes()).unwrap();
    let original = package.save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"rich-notes","base64":STANDARD.encode(&original)})).unwrap();
    let document = &opened["document"];
    assert_eq!(document["deck"]["slides"][0]["notes_paragraphs"][0]["runs"][0]["style"]["bold"], true);
    let changed = native_change(document, json!([{"op":"replace","path":"/deck/slides/0/notes_paragraphs/0/runs/0/style/bold","value":false}]));
    let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let result = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert!(result.text(path).unwrap().contains(sentinel));
    for part in package.parts().keys().filter(|part| !part.starts_with("customXml/") && part.as_str() != path) { assert_eq!(result.part(part).unwrap(), package.part(part).unwrap(), "{part}"); }
    let reopened = execute_request(json!({"op":"open_presentation","id":"rich-notes","base64":output["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][0]["notes"], "Rich notes");
    assert_eq!(reopened["document"]["deck"]["slides"][0]["notes_paragraphs"][0]["runs"][0]["style"]["bold"], false);
    let cleared = execute_request(json!({"op":"update_rich_notes","document":reopened["document"],"expected_revision":0,"slide_id":"slide-1","paragraphs":[]})).unwrap();
    assert_eq!(cleared["document"]["deck"]["slides"][0]["notes"], "");
    let cleared_output = execute_request(json!({"op":"export_presentation","document":cleared["document"]})).unwrap();
    let cleared_open = execute_request(json!({"op":"open_presentation","id":"cleared-rich","base64":cleared_output["base64"]})).unwrap();
    assert_eq!(cleared_open["document"]["deck"]["slides"][0]["notes"], "");
    assert_native_undo(document, &changed, &original);
}

#[test]
fn rich_notes_cache_refresh_retains_native_field_extensions_and_other_paragraphs() {
    let mut deck = authored();
    deck["slides"][0]["notes"] = json!("Before\n9");
    deck["slides"][0]["notes_paragraphs"] = json!([{"runs":[{"text":"Before"}]},{"runs":[{"text":"9","field":{"id":"00112233-4455-6677-8899-aabbccddeeff","kind":"slidenum"}}]}]);
    let mut package = Package::open(exported(deck)).unwrap();
    let path = "ppt/notesSlides/notesSlide1.xml";
    let extension = "<a:extLst><a:ext uri=\"urn:notes-field\"><v:keep xmlns:v=\"urn:vendor\"/></a:ext></a:extLst>";
    let xml = package.text(path).unwrap().replace("<a:fld ", "<a:fld xmlns:v=\"urn:vendor\" v:retain=\"yes\" ").replace("</a:fld>", &format!("{extension}</a:fld>"));
    package.replace_part(path, xml.into_bytes()).unwrap();
    let original = package.save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"notes-fields","base64":STANDARD.encode(&original)})).unwrap();
    let mut paragraphs = opened["document"]["deck"]["slides"][0]["notes_paragraphs"].clone();
    paragraphs[0]["runs"][0]["text"] = json!("After");
    let changed = execute_request(json!({"op":"update_rich_notes","document":opened["document"],"expected_revision":0,"slide_id":"slide-1","paragraphs":paragraphs})).unwrap();
    let refreshed = execute_request(json!({"op":"refresh_fields","document":changed["document"],"expected_revision":1,"reference_date":"2026-09-18"})).unwrap();
    assert_eq!(refreshed["document"]["deck"]["slides"][0]["notes"], "After\n1");
    let output = execute_request(json!({"op":"export_presentation","document":refreshed["document"]})).unwrap();
    let saved = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert!(saved.text(path).unwrap().contains(extension));
    assert!(saved.text(path).unwrap().contains("v:retain=\"yes\""));
    assert_native_undo(&opened["document"], &changed, &original);
}

#[test]
fn rich_notes_single_property_formats_survive_native_reopen() {
    for paragraph in [
        json!({"runs":[{"text":"Notes","style":{"language":"en-US"}}]}),
        json!({"runs":[{"text":"Notes"}],"margin_left":285750,"indent":-142875,"level":1}),
        json!({"runs":[{"text":"Notes"}],"line_spacing":{"kind":"points","value":2400}}),
    ] {
        let mut deck = authored();
        deck["slides"][0]["notes"] = json!("Notes");
        deck["slides"][0]["notes_paragraphs"] = json!([paragraph]);
        let bytes = exported(deck);
        let opened = execute_request(json!({"op":"open_presentation","id":"note-property","base64":STANDARD.encode(&bytes)})).unwrap();
        let actual = &opened["document"]["deck"]["slides"][0]["notes_paragraphs"][0];
        assert!(actual.is_object(), "missing rich note format: {paragraph}");
        for name in ["margin_left", "indent", "level", "line_spacing"] {
            if let Some(expected) = paragraph.get(name) { assert_eq!(&actual[name], expected, "{name}"); }
        }
        if let Some(language) = paragraph.pointer("/runs/0/style/language") { assert_eq!(&actual["runs"][0]["style"]["language"], language); }
        let saved = execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap();
        assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), bytes);
    }
}

#[test]
fn rich_notes_single_run_font_and_color_remain_editable_after_native_reopen() {
    let mut deck = authored();
    deck["slides"][0]["notes"] = json!("Notes");
    deck["slides"][0]["notes_paragraphs"] = json!([{"runs":[{"text":"Notes","style":{"font_size":28,"color":"ABCDEF"}}]}]);
    let bytes = exported(deck);
    let opened = execute_request(json!({"op":"open_presentation","id":"single-rich-note","base64":STANDARD.encode(bytes)})).unwrap();
    assert_eq!(opened["document"]["deck"]["slides"][0]["notes_paragraphs"][0]["runs"][0]["style"]["font_size"], 28.0);
    assert_eq!(opened["document"]["deck"]["slides"][0]["notes_paragraphs"][0]["runs"][0]["style"]["color"], "ABCDEF");
}

#[test]
fn rich_notes_unmodeled_hyperlinks_fail_closed_on_changed_paragraph() {
    let mut deck = authored();
    deck["slides"][0]["notes"] = json!("Linked notes");
    deck["slides"][0]["notes_paragraphs"] = json!([{"runs":[{"text":"Linked notes","style":{"bold":true}}]}]);
    let mut package = Package::open(exported(deck)).unwrap();
    let path = "ppt/notesSlides/notesSlide1.xml";
    package.replace_part(path, package.text(path).unwrap().replace("</a:rPr>", "<a:hlinkClick r:id=\"external-link\"/></a:rPr>").into_bytes()).unwrap();
    let path = "ppt/notesSlides/_rels/notesSlide1.xml.rels";
    package.replace_part(path, package.text(path).unwrap().replace("</Relationships>", "<Relationship Id=\"external-link\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"https://example.invalid/\" TargetMode=\"External\"/></Relationships>").into_bytes()).unwrap();
    let original = package.save().unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"linked-notes","base64":STANDARD.encode(&original)})).unwrap();
    let mut paragraphs = opened["document"]["deck"]["slides"][0]["notes_paragraphs"].clone();
    paragraphs[0]["runs"][0]["text"] = json!("Changed");
    assert!(execute_request(json!({"op":"update_rich_notes","document":opened["document"],"expected_revision":0,"slide_id":"slide-1","paragraphs":paragraphs})).is_err());
    let result = execute_request(json!({"op":"export_presentation","document":opened["document"]})).unwrap();
    assert_eq!(STANDARD.decode(result["base64"].as_str().unwrap()).unwrap(), original);
}

#[test]
fn omitted_optional_notes_models_preserve_native_content_for_older_clients() {
    let mut deck = authored();
    deck["slides"][0]["notes"] = json!("Retain rich notes");
    deck["slides"][0]["notes_paragraphs"] = json!([{"runs":[{"text":"Retain rich notes","style":{"bold":true}}]}]);
    let bytes = exported(deck);
    let opened = execute_request(json!({"op":"open_presentation","id":"old-notes-client","base64":STANDARD.encode(&bytes)})).unwrap();
    let mut legacy = opened["document"]["deck"].clone();
    legacy.as_object_mut().unwrap().remove("auxiliary_design");
    legacy["slides"][0].as_object_mut().unwrap().remove("notes_paragraphs");
    let updated = native_change(&opened["document"], json!([{"op":"replace","path":"/deck","value":legacy}]));
    let output = execute_request(json!({"op":"export_presentation","document":updated["document"]})).unwrap();
    let package = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
    let before = Package::open(bytes).unwrap();
    for path in ["ppt/notesSlides/notesSlide1.xml", "ppt/notesMasters/notesMaster1.xml"] { assert_eq!(package.part(path).unwrap(), before.part(path).unwrap()); }
}