use aislide_core::{comments::{self, Comment}, document::{self, Document, Transaction}, fields, model::Deck, package::Package, pptx, review};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::json;

fn deck() -> Deck {
    serde_json::from_value(json!({"version":1,"title":"Synthetic review test","width":1280,"height":720,"slides":[{"id":"slide-1","title":"Synthetic","background":"FFFFFF","notes":"Private synthetic notes","elements":[
        {"type":"text","id":"title","x":40,"y":40,"width":1000,"height":80,"text":"Synthetic","font_size":20,"color":"DDDDDD","bold":false},
        {"type":"rect","id":"box","x":40,"y":180,"width":300,"height":150,"fill":"008800"}
    ]}]})).unwrap()
}

fn comment(id: &str) -> Comment {
    serde_json::from_value(json!({"id":id,"author":"Synthetic local reviewer","initials":"SL","timestamp":"2026-09-17T10:15:00Z","text":"@Local only <no service>","x":20,"y":40})).unwrap()
}

fn open(bytes: &[u8]) -> Document {
    serde_json::from_value(document::open_presentation("opened".into(), bytes.to_vec()).unwrap()["document"].clone()).unwrap()
}

fn save(document: &Document) -> Vec<u8> {
    STANDARD.decode(document::export_presentation(document).unwrap()["base64"].as_str().unwrap()).unwrap()
}

fn transact(document: &Document, deck: Deck) -> Document {
    document::transact(document, Transaction { expected_revision: document.revision, expected_hash: document.hash.clone(), operations: serde_json::from_value(json!([{"op":"replace","path":"/deck","value":deck}])).unwrap() }).unwrap().document
}

fn modern_fixture() -> Vec<u8> {
    let mut parts = Package::open(pptx::export_pptx(&deck()).unwrap()).unwrap().parts().clone();
    let ns = "http://schemas.microsoft.com/office/powerpoint/2018/8/main";
    let rel = "http://schemas.microsoft.com/office/2018/10/relationships";
    let author = "{11111111-1111-4111-8111-111111111111}";
    let other = "{22222222-2222-4222-8222-222222222222}";
    parts.insert("ppt/comments/modern.xml".into(), format!(r#"<m:cmLst xmlns:m="{ns}" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><m:cm id="{{33333333-3333-4333-8333-333333333333}}" authorId="{author}" created="2026-09-19T10:00:00Z" status="closed"><m:unknownAnchor/><m:replyLst><m:reply id="{{44444444-4444-4444-8444-444444444444}}" authorId="{other}" created="2026-09-19T10:01:00Z" status="resolved"><m:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Reply fixture</a:t></a:r></a:p></m:txBody></m:reply></m:replyLst><m:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr b="1"/><a:t>Thread fixture</a:t></a:r></a:p></m:txBody><m:extLst xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:x="urn:synthetic:phase6" mc:Ignorable="x"><x:sentinel value="KEEP-UNKNOWN"/></m:extLst></m:cm></m:cmLst>"#).into_bytes());
    parts.insert("ppt/authors/modern.xml".into(), format!(r#"<m:authorLst xmlns:m="{ns}"><m:author id="{author}" name="Same local name" userId="Same local name" providerId="None"/><m:author id="{other}" name="Same local name" userId="Same local name" providerId="None"/></m:authorLst>"#).into_bytes());
    let mut package = Package::from_parts(parts).unwrap();
    for (path, fragment) in [
        ("[Content_Types].xml", "<Override PartName=\"/ppt/comments/modern.xml\" ContentType=\"application/vnd.ms-powerpoint.comments+xml\"/><Override PartName=\"/ppt/authors/modern.xml\" ContentType=\"application/vnd.ms-powerpoint.authors+xml\"/>".to_owned()),
        ("ppt/slides/_rels/slide1.xml.rels", format!("<Relationship Id=\"modernComment\" Type=\"{rel}/comments\" Target=\"../comments/modern.xml\"/>")),
        ("ppt/_rels/presentation.xml.rels", format!("<Relationship Id=\"modernAuthors\" Type=\"{rel}/authors\" Target=\"authors/modern.xml\"/>")),
        ("ppt/slides/slide1.xml", format!("<p:extLst><p:ext uri=\"{{6950BFC3-D8DA-4A85-94F7-54DA5524770B}}\"><m:commentRel xmlns:m=\"{ns}\" r:id=\"modernComment\"/></p:ext></p:extLst>")),
    ] {
        let mut xml = package.text(path).unwrap().to_owned();
        let end = xml.rfind("</").unwrap();
        xml.insert_str(end, &fragment);
        package.replace_part(path, xml.into_bytes()).unwrap();
    }
    package.save().unwrap()
}

#[test]
fn modern_body_can_be_edited_repeatedly_after_native_reopen() {
    use comments::modern::{self, Anchor, Draft, Operation};
    let draft = Draft { author_name: "Synthetic editor".into(), initials: None, created: "2026-09-19T11:00:00Z".into(), body: serde_json::from_value(json!([{"runs":[{"text":"Original", "style":{"bold":true}}]}])).unwrap() };
    let created = modern::apply(&deck(), "slide-1", Operation::Create { draft, anchor: Anchor::Unknown }).unwrap();
    let mut document = open(&pptx::export_pptx(&created).unwrap());
    let id = document.deck.slides[0].review.as_ref().unwrap().modern_threads.as_ref().unwrap()[0].id.clone();
    for text in ["First edit", "Second edit", "Third edit"] {
        let body = serde_json::from_value(json!([{"runs":[{"text":text,"style":{"italic":true}}]}])).unwrap();
        let changed = aislide_core::authoring_ops::update_review(&document, document.revision, |deck| modern::apply(deck, "slide-1", Operation::UpdateBody { comment_id: id.clone(), body })).unwrap();
        let bytes = save(&changed.document);
        document = open(&bytes);
        assert_eq!(document.deck.slides[0].review.as_ref().unwrap().modern_threads.as_ref().unwrap()[0].body[0].runs[0].text, text);
        assert_eq!(save(&document), bytes);
    }
}

#[test]
fn modern_native_order_changes_reject_without_mutating_the_document() {
    use comments::modern::{self, Anchor, Draft, Operation};
    let draft = Draft { author_name: "Synthetic editor".into(), initials: None, created: "2026-09-19T11:00:00Z".into(), body: serde_json::from_value(json!([{"runs":[{"text":"Ordered"}]}])).unwrap() };
    let mut created = modern::apply(&deck(), "slide-1", Operation::Create { draft: draft.clone(), anchor: Anchor::Unknown }).unwrap();
    created = modern::apply(&created, "slide-1", Operation::Create { draft: draft.clone(), anchor: Anchor::Unknown }).unwrap();
    let id = created.slides[0].review.as_ref().unwrap().modern_threads.as_ref().unwrap()[0].id.clone();
    for _ in 0..2 { created = modern::apply(&created, "slide-1", Operation::Reply { thread_id: id.clone(), draft: draft.clone() }).unwrap(); }
    let bytes = pptx::export_pptx(&created).unwrap();
    let document = open(&bytes);
    for replies in [false, true] {
        let mut candidate = document.deck.clone();
        let threads = candidate.slides[0].review.as_mut().unwrap().modern_threads.as_mut().unwrap();
        if replies { threads[0].replies.reverse(); } else { threads.reverse(); }
        let result = aislide_core::execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document.revision,"expected_hash":document.hash,"operations":[{"op":"replace","path":"/deck","value":candidate}]}}));
        assert!(result.is_err(), "unsupported native comment reordering must not silently succeed");
        assert_eq!(save(&document), bytes);
    }
}

#[test]
fn modern_independent_xml_reads_three_states_and_preserves_noop_bytes() {
    let bytes = modern_fixture();
    let document = open(&bytes);
    if let Ok(directory) = std::env::var("AISLIDE_PHASE6_ARTIFACT_DIR") { std::fs::write(std::path::Path::new(&directory).join("modern-independent.pptx"), &bytes).unwrap(); }
    let value = serde_json::to_value(&document.deck.slides[0].review).unwrap();
    assert_eq!(value["modern_threads"][0]["status"], "closed");
    assert_eq!(value["modern_threads"][0]["replies"][0]["status"], "resolved");
    assert_ne!(value["modern_threads"][0]["author"]["id"], value["modern_threads"][0]["replies"][0]["author"]["id"]);
    assert!(value.get("comments").is_none());
    assert_eq!(save(&document), bytes);
}

#[test]
fn modern_status_reply_save_preserves_unknown_and_other_parts() {
    let bytes = modern_fixture();
    let original = open(&bytes);
    let mut value = serde_json::to_value(&original.deck).unwrap();
    let thread = &mut value["slides"][0]["review"]["modern_threads"][0];
    thread["status"] = json!("active");
    let mut reply = thread["replies"][0].clone();
    reply["id"] = json!("{55555555-5555-4555-8555-555555555555}");
    reply["author"]["id"] = json!("{66666666-6666-4666-8666-666666666666}");
    thread["replies"].as_array_mut().unwrap().push(reply);
    let changed = transact(&original, serde_json::from_value(value).unwrap());
    let saved = save(&changed);
    let reopened = open(&saved);
    let review = serde_json::to_value(&reopened.deck.slides[0].review).unwrap();
    assert_eq!(review["modern_threads"][0]["status"], "active");
    assert_eq!(review["modern_threads"][0]["replies"].as_array().unwrap().len(), 2);
    let before = Package::open(bytes.clone()).unwrap();
    let after = Package::open(saved).unwrap();
    assert!(after.text("ppt/comments/modern.xml").unwrap().contains("KEEP-UNKNOWN"));
    for (path, data) in before.parts() {
        if path != "ppt/comments/modern.xml" && path != "ppt/authors/modern.xml" && !path.contains("aislide") { assert_eq!(after.part(path).unwrap(), data, "{path}"); }
    }
    assert_eq!(save(&original), bytes);
}

#[test]
fn modern_fresh_offline_writer_and_clean_copy_remove_exact_comment_links() {
    use comments::modern::{self, Anchor, Draft, Operation};
    let draft = Draft { author_name: "Offline local".into(), initials: None, created: "2026-09-19T11:00:00Z".into(), body: serde_json::from_value(json!([{"runs":[{"text":"Synthetic <&> rich", "style":{"bold":true}}]}])).unwrap() };
    let next = modern::apply(&deck(), "slide-1", Operation::Create { draft: draft.clone(), anchor: Anchor::Unknown }).unwrap();
    let next = modern::apply(&next, "slide-1", Operation::Create { draft, anchor: Anchor::Unknown }).unwrap();
    let bytes = pptx::export_pptx(&next).unwrap();
    let original = open(&bytes);
    let threads = original.deck.slides[0].review.as_ref().unwrap().modern_threads.as_ref().unwrap();
    assert_eq!(threads.len(), 2);
    assert_ne!(threads[0].id, threads[1].id);
    assert_ne!(threads[0].author.id, threads[1].author.id);
    assert_eq!(threads[0].author.provider_id, "None");
    assert_eq!(threads[0].body[0].runs[0].text, "Synthetic <&> rich");
    let clean = review::export_clean_copy(&original, &clean_options(&[review::InspectionCategory::Comments])).unwrap();
    let package = Package::open(clean.bytes).unwrap();
    assert!(!package.text("ppt/slides/slide1.xml").unwrap().contains("6950BFC3-D8DA-4A85-94F7-54DA5524770B"));
    assert!(!package.text("[Content_Types].xml").unwrap().contains("ms-powerpoint.authors"));
    assert!(!package.parts().keys().any(|path| path.starts_with("ppt/comments/") || path.starts_with("ppt/authors/")));
    assert_eq!(save(&original), bytes);
}

#[test]
fn modern_invalid_identities_namespaces_and_metadata_fail_closed() {
    for (path, from, to) in [
        ("ppt/comments/modern.xml", "{44444444-4444-4444-8444-444444444444}", "{33333333-3333-4333-8333-333333333333}"),
        ("ppt/comments/modern.xml", "authorId=\"{22222222-2222-4222-8222-222222222222}\"", "authorId=\"{99999999-9999-4999-8999-999999999999}\""),
        ("ppt/comments/modern.xml", "2026-09-19T10:00:00Z", "2026-02-30T10:00:00Z"),
        ("ppt/comments/modern.xml", "2018/8/main", "2018/8/forged"),
        ("[Content_Types].xml", "application/vnd.ms-powerpoint.comments+xml", "application/xml"),
        ("ppt/slides/_rels/slide1.xml.rels", "Id=\"modernComment\"", "Id=\"modernComment\" TargetMode=\"External\""),
        ("ppt/authors/modern.xml", "{22222222-2222-4222-8222-222222222222}", "{11111111-1111-4111-8111-111111111111}"),
    ] {
        let mut package = Package::open(modern_fixture()).unwrap();
        let xml = package.text(path).unwrap().replace(from, to);
        package.replace_part(path, xml.into_bytes()).unwrap();
        let result = document::open_presentation("invalid".into(), package.save().unwrap());
        assert!(result.is_err(), "malformed fixture accepted: {path} / {from}");
    }
    let original = open(&modern_fixture());
    for field in ["created", "author", "anchor"] {
        let mut next = serde_json::to_value(&original.deck).unwrap();
        next["slides"][0]["review"]["modern_threads"][0][field] = match field { "created" => json!("2026-09-19T12:00:00Z"), "author" => { let mut author = next["slides"][0]["review"]["modern_threads"][0]["author"].clone(); author["name"] = json!("Forged local name"); author }, _ => json!({"kind":"preserved"}) };
        assert!(document::transact(&original, Transaction { expected_revision: original.revision, expected_hash: original.hash.clone(), operations: serde_json::from_value(json!([{"op":"replace","path":"/deck","value":next}])).unwrap() }).is_err());
    }
}

#[test]
fn pii_candidates_are_masked_scoped_and_never_clean_copy_categories() {
    let mut next = deck();
    next.slides[0].notes = "SYNTHETIC.person@example.invalid +1 (202) 555-0147".into();
    let original = open(&pptx::export_pptx(&next).unwrap());
    let report = serde_json::to_value(review::inspect_document(&original).unwrap()).unwrap();
    let candidates = report["candidates"].as_array().expect("separate manual review candidates");
    for scope in ["current_deck", "embedded_origin"] {
        assert!(candidates.iter().any(|candidate| candidate["rule"] == "email_candidate" && candidate["scope"] == scope));
        assert!(candidates.iter().any(|candidate| candidate["rule"] == "phone_candidate" && candidate["scope"] == scope));
    }
    let serialized = report.to_string();
    for sentinel in ["SYNTHETIC.person", "example.invalid", "555-0147", "ppt/", "Private synthetic notes"] { assert!(!serialized.contains(sentinel), "inspection leaked content"); }
    for candidate in candidates { assert_eq!(candidate["masked_value"], "[REDACTED]"); assert!(candidate["locations"].as_array().unwrap().iter().all(|location| location.as_array().unwrap().iter().all(|value| value.is_u64()))); }
    assert!(serde_json::from_value::<review::CleanCopyOptions>(json!({"new_document_id":"clean","categories":["email_candidate"],"confirmed":true})).is_err());
    next.slides[0].notes = "2026-09-19 {33333333-3333-4333-8333-333333333333}".into();
    let report = serde_json::to_value(review::inspect_document(&open(&pptx::export_pptx(&next).unwrap())).unwrap()).unwrap();
    assert!(!report["candidates"].as_array().unwrap().iter().any(|candidate| candidate["rule"] == "phone_candidate"));
}

#[test]
fn accessibility_table_headers_are_declared_not_styling_and_survive_native_save() {
    let mut value = serde_json::to_value(deck()).unwrap();
    value["slides"][0]["elements"].as_array_mut().unwrap().push(json!({"type":"table","id":"table","x":40,"y":400,"width":700,"height":200,"rows":[["","Heading"],["Row","Value"]],"font_size":20}));
    value["slides"][0]["review"] = json!({"table_headers":{"table":"first_row"}});
    let next: Deck = serde_json::from_value(value).unwrap();
    let report = review::check_accessibility(&next);
    assert!(report.issues.iter().any(|issue| issue.code == "table_declared_header_empty"));
    let original = open(&pptx::export_pptx(&next).unwrap());
    let review = serde_json::to_value(&original.deck.slides[0].review).unwrap();
    assert_eq!(review["table_headers"]["table"], "first_row");
    let mut value = serde_json::to_value(&original.deck).unwrap();
    value["slides"][0]["review"]["table_headers"]["table"] = json!("none");
    let next = transact(&original, serde_json::from_value(value).unwrap());
    let reopened = open(&save(&next));
    let report = review::check_accessibility(&reopened.deck);
    assert!(!report.issues.iter().any(|issue| issue.code == "table_declared_header_empty"));
    assert!(report.issues.iter().any(|issue| issue.code == "table_headers_none_review"));
}

#[test]
fn modern_protocol_operations_are_reachable_and_revision_guarded() {
    let original = open(&modern_fixture());
    let thread_id = original.deck.slides[0].review.as_ref().unwrap().modern_threads.as_ref().unwrap()[0].id.clone();
    let result = aislide_core::execute_request(json!({"op":"modern_comment","document":original,"expected_revision":original.revision,"slide_id":"slide-1","operation":{"type":"reply","thread_id":thread_id,"draft":{"author_name":"Same local name","created":"2026-09-19T12:00:00Z","body":[{"runs":[{"text":"Protocol reply"}]}]}}})).unwrap();
    let next: Document = serde_json::from_value(result["document"].clone()).unwrap();
    assert_eq!(next.revision, original.revision + 1);
    assert_eq!(next.deck.slides[0].review.as_ref().unwrap().modern_threads.as_ref().unwrap()[0].replies.len(), 2);
    assert!(aislide_core::execute_request(json!({"op":"modern_comment","document":next,"expected_revision":original.revision,"slide_id":"slide-1","operation":{"type":"set_status","comment_id":thread_id,"status":"active"}})).is_err());
}

#[test]
fn modern_opaque_body_protection_external_refs_and_omission_are_safe() {
    use comments::modern::{self, Operation, Status};
    let mut package = Package::open(modern_fixture()).unwrap();
    let xml = package.text("ppt/comments/modern.xml").unwrap().replace("<a:rPr b=\"1\"/>", "<a:rPr b=\"1\"><a:extLst><a:ext uri=\"synthetic-rich\"><x:keep xmlns:x=\"urn:synthetic\"/></a:ext></a:extLst></a:rPr>");
    package.replace_part("ppt/comments/modern.xml", xml.into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let original = open(&bytes);
    let id = original.deck.slides[0].review.as_ref().unwrap().modern_threads.as_ref().unwrap()[0].id.clone();
    let next = modern::apply(&original.deck, "slide-1", Operation::SetStatus { comment_id: id.clone(), status: Status::Active }).unwrap();
    let next = transact(&original, next);
    assert!(Package::open(save(&next)).unwrap().text("ppt/comments/modern.xml").unwrap().contains("synthetic-rich"));
    assert!(aislide_core::authoring_ops::update_review(&original, original.revision, |deck| modern::apply(deck, "slide-1", Operation::UpdateBody { comment_id: id.clone(), body: serde_json::from_value(json!([{"runs":[{"text":"Replace"}]}])).unwrap() })).is_err());
    let mut omitted = original.deck.clone(); omitted.slides[0].review.as_mut().unwrap().modern_threads = None;
    let omitted = transact(&original, omitted);
    assert_eq!(open(&save(&omitted)).deck.slides[0].review.as_ref().unwrap().modern_threads.as_ref().unwrap().len(), 1);
    assert!(aislide_core::editing::slides(&original, original.revision, &[aislide_core::editing::SlideOperation::Duplicate { slide_id: "slide-1".into(), id: "copy".into() }]).is_err());
    for path in ["_xmlsignatures/sig1.xml", "docMetadata/LabelInfo.xml", "ppt/comments/_rels/modern.xml.rels"] {
        let mut parts = Package::open(bytes.clone()).unwrap().parts().clone();
        let xml = if path.ends_with(".rels") { "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"external\" Type=\"urn:synthetic:link\" TargetMode=\"External\" Target=\"https://example.invalid/\"/></Relationships>" } else { "<sentinel/>" };
        parts.insert(path.into(), xml.as_bytes().to_vec());
        let input = Package::from_parts(parts).unwrap().save().unwrap();
        let original = open(&input);
        assert_eq!(save(&original), input);
        assert!(aislide_core::authoring_ops::update_review(&original, original.revision, |deck| modern::apply(deck, "slide-1", Operation::SetStatus { comment_id: id.clone(), status: Status::Active })).is_err(), "modern mutation accepted unsafe package");
    }
}

#[test]
fn modern_guids_are_unique_across_slides_and_omitted_copy_is_rejected() {
    let original = open(&modern_fixture());
    let mut next = original.deck.clone();
    let mut second = next.slides[0].clone(); second.id = "second".into(); second.native_source_id = None;
    next.slides.push(second);
    assert!(pptx::export_pptx(&next).is_err(), "duplicate modern GUIDs across slides must reject");
    let mut omitted = original.deck.clone(); omitted.slides[0].review.as_mut().unwrap().modern_threads = None;
    let omitted = transact(&original, omitted);
    assert!(aislide_core::editing::slides(&omitted, omitted.revision, &[aislide_core::editing::SlideOperation::Duplicate { slide_id: "slide-1".into(), id: "copy".into() }]).is_err());
}

#[test]
fn accessibility_effective_master_theme_and_combined_table_metadata() {
    let mut next = deck();
    next.design = Some(aislide_core::design::Design::default());
    let design = next.design.as_mut().unwrap();
    let mut theme = design.theme.clone(); theme.colors.insert("dk1".into(), "FFFFFF".into());
    design.masters[0].theme = Some(theme);
    next.slides[0].layout_id = Some("blank".into());
    let mut value = serde_json::to_value(&next).unwrap();
    value["slides"][0]["elements"][0]["color"] = json!("@dk1");
    value["slides"][0]["elements"].as_array_mut().unwrap().push(json!({"type":"table","id":"table","x":40,"y":400,"width":700,"height":200,"rows":[["Heading","Heading"],["Row","Value"]],"font_size":20}));
    value["slides"][0]["review"] = json!({"table_headers":{"table":"both"},"accessibility":{"table":{"decorative":true,"description":"Meaningful table"}}});
    let next: Deck = serde_json::from_value(value).unwrap();
    let report = review::check_accessibility(&next);
    assert!(report.issues.iter().any(|issue| issue.code == "text_contrast_low" && issue.contrast_ratio == Some(1.0)));
    let bytes = pptx::export_pptx(&next).unwrap();
    let package = Package::open(bytes.clone()).unwrap();
    let parsed = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    let props = parsed.descendants().find(|node| node.tag_name().name() == "cNvPr" && node.attribute("name") == Some("table")).unwrap();
    assert_eq!(props.children().filter(|node| node.tag_name().name() == "extLst").count(), 1);
    let reopened = open(&bytes);
    let review = reopened.deck.slides[0].review.as_ref().unwrap();
    assert_eq!(review.table_headers["table"], aislide_core::review::TableHeaders::Both);
    assert_eq!(review.accessibility["table"].decorative, Some(true));
}

#[test]
fn modern_body_edit_preserves_frame_and_rejects_unrepresented_font_variants() {
    use comments::modern::{self, Operation};
    let mut package = Package::open(modern_fixture()).unwrap();
    let xml = package.text("ppt/comments/modern.xml").unwrap().replace("<a:bodyPr/>", "<a:bodyPr lIns=\"12345\" anchor=\"b\"/>");
    package.replace_part("ppt/comments/modern.xml", xml.into_bytes()).unwrap();
    let original = open(&package.save().unwrap());
    let thread = &original.deck.slides[0].review.as_ref().unwrap().modern_threads.as_ref().unwrap()[0];
    let id = thread.id.clone(); let mut body = thread.body.clone(); body[0].runs[0].text = "Updated <&> body".into();
    let next = aislide_core::authoring_ops::update_review(&original, original.revision, |deck| modern::apply(deck, "slide-1", Operation::UpdateBody { comment_id: id.clone(), body: body.clone() })).unwrap().document;
    let saved = Package::open(save(&next)).unwrap();
    assert_eq!(saved.text("ppt/comments/modern.xml").unwrap().matches("lIns=\"12345\"").count(), 2);
    let xml = package.text("ppt/comments/modern.xml").unwrap().replace("<a:rPr b=\"1\"/>", "<a:rPr b=\"1\"><a:latin typeface=\"Arial\"/><a:ea typeface=\"Unrepresented synthetic font\"/><a:cs typeface=\"Arial\"/></a:rPr>");
    package.replace_part("ppt/comments/modern.xml", xml.into_bytes()).unwrap();
    let original = open(&package.save().unwrap());
    assert!(aislide_core::authoring_ops::update_review(&original, original.revision, |deck| modern::apply(deck, "slide-1", Operation::UpdateBody { comment_id: id, body })).is_err());
}

#[test]
fn modern_simple_anchors_are_typed_and_complex_monikers_remain_preserved() {
    let slide = "<pc:sldMkLst xmlns:pc=\"http://schemas.microsoft.com/office/powerpoint/2013/main/command\"><pc:docMk/><pc:sldMk sldId=\"256\" cId=\"1\"/></pc:sldMkLst>";
    let shape = format!("<ac:deMkLst xmlns:ac=\"http://schemas.microsoft.com/office/drawing/2013/main/command\">{slide}<ac:spMk id=\"2\" creationId=\"{{11111111-1111-4111-8111-111111111111}}\"/></ac:deMkLst>");
    for (anchor, kind) in [(slide.to_owned(), "slide"), (shape.clone(), "shape"), (format!("{shape}{shape}"), "preserved")] {
        let mut package = Package::open(modern_fixture()).unwrap();
        let xml = package.text("ppt/comments/modern.xml").unwrap().replace("<m:unknownAnchor/>", &anchor);
        package.replace_part("ppt/comments/modern.xml", xml.into_bytes()).unwrap();
        let bytes = package.save().unwrap(); let original = open(&bytes);
        let value = serde_json::to_value(&original.deck.slides[0].review).unwrap();
        assert_eq!(value["modern_threads"][0]["anchor"]["kind"], kind);
        if kind != "preserved" { assert_eq!(value["modern_threads"][0]["anchor"]["slide_id"], 256); assert_eq!(value["modern_threads"][0]["anchor"]["slide_creation_id"], 1); }
        assert_eq!(save(&original), bytes);
    }
}

#[test]
fn native_comments_roundtrip_resolve_reply_remove_preserving_other_parts() {
    let deck = comments::add(&deck(), "slide-1", comment("first")).unwrap();
    let bytes = pptx::export_pptx(&deck).unwrap();
    let package = Package::open(bytes.clone()).unwrap();
    assert!(package.parts().iter().any(|(_, data)| String::from_utf8_lossy(data).contains("<p:cmLst")));
    assert!(package.parts().iter().any(|(_, data)| String::from_utf8_lossy(data).contains("<p:cmAuthor ")));
    let original = open(&bytes); assert_eq!(save(&original), bytes);
    let changed = comments::reply(&original.deck, "slide-1", "first", comment("reply")).unwrap();
    let changed = comments::resolve(&changed, "slide-1", "first", true).unwrap();
    let changed = transact(&original, changed); let saved = save(&changed);
    let reopened = open(&saved);
    let comments = &reopened.deck.slides[0].review.as_ref().unwrap().comments;
    assert_eq!(comments.len(), 2); assert!(comments[0].resolved); assert_eq!(comments[1].parent_id.as_deref(), Some("first"));
    assert_eq!(comments[0].timestamp, "2026-09-17T10:15:00Z");
    let updated = Package::open(saved).unwrap();
    assert_eq!(updated.part("ppt/slides/slide1.xml").unwrap(), package.part("ppt/slides/slide1.xml").unwrap());
    let removed = transact(&reopened, comments::remove(&reopened.deck, "slide-1", "first").unwrap());
    assert!(open(&save(&removed)).deck.slides[0].review.as_ref().is_none_or(|review| review.comments.is_empty()));
    assert_eq!(save(&original), bytes);
}

#[test]
fn native_duplicate_comments_have_independent_indices() {
    let deck = comments::add(&deck(), "slide-1", comment("first")).unwrap();
    let original = open(&pptx::export_pptx(&deck).unwrap());
    let duplicated = aislide_core::editing::slides(&original, original.revision, &[aislide_core::editing::SlideOperation::Duplicate { slide_id: "slide-1".into(), id: "copy".into() }]).unwrap().document;
    let copied = open(&save(&duplicated));
    let first = &copied.deck.slides[0].review.as_ref().unwrap().comments[0];
    let second = &copied.deck.slides[1].review.as_ref().unwrap().comments[0];
    assert_ne!((first.native_author_id, first.native_index), (second.native_author_id, second.native_index));
    let changed = comments::remove(&copied.deck, &copied.deck.slides[1].id, "first").unwrap();
    let changed = open(&save(&transact(&copied, changed)));
    assert_eq!(changed.deck.slides[0].review.as_ref().unwrap().comments.len(), 1);
    assert!(changed.deck.slides[1].review.as_ref().is_none_or(|review| review.comments.is_empty()));
}

#[test]
fn native_preservation_resolving_comments_keeps_complete_text_and_raw_xml() {
    let deck = comments::add(&deck(), "slide-1", comment("split-text")).unwrap();
    let mut package = Package::open(pptx::export_pptx(&deck).unwrap()).unwrap();
    let path = package.parts().iter().find(|(_, bytes)| String::from_utf8_lossy(bytes).contains("<p:cmLst")).unwrap().0.clone();
    let mut xml = package.text(&path).unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let range = parsed.descendants().find(|node| node.has_tag_name(("http://schemas.openxmlformats.org/presentationml/2006/main", "text"))).unwrap().range();
    let original_text = "<p:text>HEAD<!--separator-->TAIL &amp; END</p:text>";
    xml.replace_range(range, original_text);
    package.replace_part(&path, xml.into_bytes()).unwrap();
    let original_bytes = package.save().unwrap();
    let original = open(&original_bytes);
    assert_eq!(original.deck.slides[0].review.as_ref().unwrap().comments[0].text, "HEADTAIL & END");
    let changed = comments::resolve(&original.deck, "slide-1", "split-text", true).unwrap();
    let changed = transact(&original, changed);
    let bytes = save(&changed);
    let updated = Package::open(bytes.clone()).unwrap();
    assert!(updated.text(&path).unwrap().contains(original_text));
    assert_eq!(open(&bytes).deck.slides[0].review.as_ref().unwrap().comments[0].text, "HEADTAIL & END");
    assert_eq!(save(&original), original_bytes);
}

#[test]
fn native_accessibility_and_actual_reading_order_survive_reopen() {
    let original = open(&pptx::export_pptx(&deck()).unwrap());
    let next = review::set_accessibility(&original.deck, "slide-1", "box", Some(review::ElementAccessibility { title: Some("Status".into()), description: Some("Synthetic green indicator".into()), decorative: Some(true) })).unwrap();
    let next = review::set_reading_order(&next, "slide-1", vec!["box".into(), "title".into()]).unwrap();
    let next = transact(&original, next); let bytes = save(&next); let reopened = open(&bytes);
    assert_eq!(reopened.deck.slides[0].elements[0].bounds().0, "box");
    let review = reopened.deck.slides[0].review.as_ref().unwrap();
    assert_eq!(review.reading_order, ["box", "title"]);
    assert_eq!(review.accessibility["box"].description.as_deref(), Some("Synthetic green indicator"));
    assert_eq!(review.accessibility["box"].decorative, Some(true));
    let package = Package::open(bytes).unwrap(); let parsed = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    assert!(parsed.descendants().any(|node| node.tag_name().name() == "decorative" && node.attribute("val") == Some("1")));
}

#[test]
fn contrast_checker_is_numeric_and_explicit_about_coverage() {
    assert!((review::contrast_ratio("000000", "FFFFFF").unwrap() - 21.0).abs() < 1e-10);
    assert_eq!(review::contrast_ratio("777777", "777777").unwrap(), 1.0);
    assert!(review::contrast_ratio("@dk1", "FFFFFF").is_err());
    let report = review::check_accessibility(&deck());
    assert!(!report.wcag_certified);
    assert!(report.issues.iter().any(|issue| issue.code == "text_contrast_low" && issue.required_ratio == Some(4.5)));
    assert!(report.issues.iter().any(|issue| issue.code == "reading_order_review"));
}

#[test]
fn fields_are_native_deterministic_and_body_edits_keep_identity() {
    let mut deck = deck();
    deck.slides[0].elements[0] = serde_json::from_value(json!({"type":"text","id":"title","x":40,"y":40,"width":1000,"height":80,"text":"Page 9 end","font_size":24,"color":"000000","bold":false,"format":{"paragraphs":[{"runs":[{"text":"Page "},{"text":"9","field":{"id":"{00112233-4455-6677-8899-aabbccddeeff}","kind":"slidenum"}},{"text":" end"}]}]}})).unwrap();
    let next = fields::refresh(&deck, "2026-09-17").unwrap();
    assert_eq!(serde_json::to_value(&next.slides[0].elements[0]).unwrap()["text"], "Page 1 end");
    let changed = aislide_core::rich_text::replace_text_content(next.slides[0].elements[0].clone(), "Slide 1 end".into()).unwrap();
    let value = serde_json::to_value(&changed).unwrap();
    let retained: Vec<_> = value["format"]["paragraphs"][0]["runs"].as_array().unwrap().iter().filter_map(|run| run.get("field")).collect();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0]["id"], "{00112233-4455-6677-8899-aabbccddeeff}");
    assert!(aislide_core::rich_text::replace_text_content(changed, "Slide end".into()).is_err());
    let bytes = pptx::export_pptx(&next).unwrap();
    let package = Package::open(bytes.clone()).unwrap();
    let parsed = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    assert!(parsed.descendants().any(|node| node.tag_name().name() == "fld" && node.attribute("type") == Some("slidenum")));
    let reopened = open(&bytes); assert_eq!(save(&reopened), bytes);
    assert!(fields::has_fields(&reopened.deck.slides[0].elements[0]));
}

#[test]
fn inherited_fields_refresh_per_slide_without_mutating_shared_templates() {
    let mut deck = deck();
    let template: aislide_core::model::Element = serde_json::from_value(json!({
        "type":"text","id":"page-number","x":1100,"y":660,"width":100,"height":40,
        "text":"9","font_size":18,"color":"@dk1","bold":false,
        "format":{"placeholder":{"kind":"slide_number","index":10},"paragraphs":[{"runs":[
            {"text":"9","field":{"id":"{00000000-0000-0000-0000-000000000010}","kind":"slidenum"}}
        ]}]}
    })).unwrap();
    let mut design = aislide_core::design::Design::default();
    design.masters[0].elements.push(template.clone());
    design.layouts[0].elements.push(template);
    deck.design = Some(design);
    deck = aislide_core::design::assign_layout(deck, "slide-1", "blank").unwrap();
    let mut second = deck.slides[0].clone(); second.id = "slide-2".into(); deck.slides.push(second);
    let bytes = pptx::export_pptx(&deck).unwrap();
    let before = Package::open(bytes.clone()).unwrap();
    for path in ["ppt/slideMasters/slideMaster1.xml", "ppt/slideLayouts/slideLayout1.xml", "ppt/slides/slide1.xml"] {
        let parsed = roxmltree::Document::parse(before.text(path).unwrap()).unwrap();
        assert!(parsed.descendants().any(|node| node.tag_name().name() == "fld" && node.attribute("type") == Some("slidenum")), "{path}");
        assert!(parsed.descendants().any(|node| node.tag_name().name() == "ph" && node.attribute("type") == Some("sldNum")), "{path}");
    }
    let refreshed = fields::refresh(&deck, "2026-09-18").unwrap();
    for (index, slide) in refreshed.slides.iter().enumerate() {
        let field = slide.elements.iter().find(|element| fields::has_fields(element)).unwrap();
        assert_eq!(serde_json::to_value(field).unwrap()["text"], (index + 1).to_string());
    }
    assert_eq!(serde_json::to_value(&refreshed.design).unwrap(), serde_json::to_value(&deck.design).unwrap());
    let refreshed_bytes = fields::refresh_native(bytes.clone(), "2026-09-18").unwrap();
    let after = Package::open(refreshed_bytes.clone()).unwrap();
    for path in ["ppt/slideMasters/slideMaster1.xml", "ppt/slideLayouts/slideLayout1.xml"] {
        assert_eq!(after.part(path).unwrap(), before.part(path).unwrap());
    }
    let reopened = open(&refreshed_bytes);
    for (index, slide) in reopened.deck.slides.iter().enumerate() {
        let field = slide.elements.iter().find(|element| fields::has_fields(element)).unwrap();
        assert_eq!(serde_json::to_value(field).unwrap()["text"], (index + 1).to_string());
    }
    assert_eq!(save(&open(&bytes)), bytes);
}

fn clean_options(categories: &[review::InspectionCategory]) -> review::CleanCopyOptions {
    review::CleanCopyOptions { new_document_id: "new-clean-copy".into(), categories: categories.iter().copied().collect(), confirmed: true }
}

#[test]
fn clean_copy_requires_explicit_action_and_removes_only_selected_categories() {
    use review::InspectionCategory::*;
    let deck = comments::add(&deck(), "slide-1", comment("first")).unwrap();
    let bytes = pptx::export_pptx(&deck).unwrap(); let original = open(&bytes);
    let before = serde_json::to_vec(&original).unwrap();
    let mut options = clean_options(&[Notes]); options.confirmed = false;
    assert!(review::export_clean_copy(&original, &options).is_err());
    options.confirmed = true; options.new_document_id = original.id.clone();
    assert!(review::export_clean_copy(&original, &options).is_err());
    let cleaned = review::export_clean_copy(&original, &clean_options(&[Notes])).unwrap();
    assert_eq!(cleaned.document.id, "new-clean-copy"); assert!(cleaned.document.deck.slides[0].notes.is_empty());
    assert_eq!(cleaned.document.deck.slides[0].review.as_ref().unwrap().comments.len(), 1);
    let old = Package::open(bytes).unwrap(); let new = Package::open(cleaned.bytes).unwrap();
    for (path, data) in old.parts().iter().filter(|(path, _)| path.starts_with("ppt/comments/") || path.starts_with("ppt/aislide-commentAuthors") || path.starts_with("ppt/theme/")) { assert_eq!(new.part(path).unwrap(), data); }
    assert!(!new.parts().keys().any(|path| path.starts_with("ppt/notesSlides/")));
    assert_eq!(serde_json::to_vec(&original).unwrap(), before);
    let cleaned = review::export_clean_copy(&original, &clean_options(&[Comments])).unwrap();
    assert!(!cleaned.document.deck.slides[0].notes.is_empty());
    assert!(cleaned.document.deck.slides[0].review.as_ref().is_none_or(|review| review.comments.is_empty()));
}

fn sourced() -> Document {
    let source = aislide_core::sources::ingest(serde_json::from_value(json!({"name":"fixture.csv","format":"csv","base64":STANDARD.encode("Value\nSynthetic\n")})).unwrap()).unwrap();
    let binding = aislide_core::data_report::SourceBinding { slide_id: "slide-1".into(), element_id: "title".into(), field: "/text".into(), source_id: source.id.clone(), source_sha256: source.sha256.clone(), locator: source.tables[0].locators[0][0].clone(), value: json!("Synthetic"), raw_value: source.tables[0].rows[0][0].clone(), transform: "display_scalar".into(), stale: false };
    document::create("sourced".into(), deck(), vec![source], vec![binding], None).unwrap()
}

#[test]
fn custom_xml_cleanup_preserves_unselected_sources_and_bindings() {
    use review::InspectionCategory::*;
    let source = sourced(); let source_bytes = save(&source);
    let mut parts = Package::open(source_bytes).unwrap().parts().clone();
    parts.insert("customXml/vendor.xml".into(), b"<v:private xmlns:v='urn:vendor'>SYNTHETIC_PRIVATE_SENTINEL</v:private>".to_vec());
    let original = open(&Package::from_parts(parts).unwrap().save().unwrap());
    let inspection = serde_json::to_string(&review::inspect_document(&original).unwrap()).unwrap();
    assert!(!inspection.contains("SYNTHETIC_PRIVATE_SENTINEL"));
    assert!(!inspection.contains("Private synthetic notes"));
    assert!(!inspection.contains("Synthetic local reviewer"));
    let cleaned = review::export_clean_copy(&original, &clean_options(&[CustomXml])).unwrap();
    assert_eq!(serde_json::to_value(&cleaned.document.sources).unwrap(), serde_json::to_value(&original.sources).unwrap());
    assert_eq!(serde_json::to_value(&cleaned.document.bindings).unwrap(), serde_json::to_value(&original.bindings).unwrap());
    assert!(Package::open(cleaned.bytes).unwrap().part("customXml/vendor.xml").is_err());
    let cleaned = review::export_clean_copy(&original, &clean_options(&[Sources])).unwrap();
    assert!(cleaned.document.sources.is_empty()); assert!(cleaned.document.bindings.is_empty());
    assert!(Package::open(cleaned.bytes).unwrap().part("customXml/vendor.xml").is_ok());
    assert_eq!(original.sources.len(), 1); assert_eq!(original.bindings.len(), 1);
}

#[test]
fn unused_media_uses_opc_reachability_and_keeps_external_relationships_inert() {
    use review::InspectionCategory::*;
    let mut parts = Package::open(pptx::export_pptx(&deck()).unwrap()).unwrap().parts().clone();
    parts.insert("ppt/media/orphan.png".into(), b"synthetic-unused-media".to_vec());
    parts.insert("ppt/media/reachable.png".into(), b"synthetic-referenced-media".to_vec());
    let path = "ppt/slides/_rels/slide1.xml.rels";
    let xml = String::from_utf8(parts[path].clone()).unwrap().replace("</Relationships>", "<Relationship Id='keep' Type='urn:vendor:media' Target='../media/reachable.png'/><Relationship Id='external' Type='urn:vendor:external' TargetMode='External' Target='https://invalid.example/do-not-fetch'/></Relationships>");
    parts.insert(path.into(), xml.into_bytes());
    let types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap().replace("</Types>", "<Default Extension='png' ContentType='image/png'/></Types>");
    parts.insert("[Content_Types].xml".into(), types.into_bytes());
    let original = open(&Package::from_parts(parts).unwrap().save().unwrap());
    let cleaned = review::export_clean_copy(&original, &clean_options(&[UnusedMedia])).unwrap();
    let package = Package::open(cleaned.bytes).unwrap();
    assert!(package.part("ppt/media/orphan.png").is_err());
    assert_eq!(package.part("ppt/media/reachable.png").unwrap(), b"synthetic-referenced-media");
    assert!(package.text(path).unwrap().contains("https://invalid.example/do-not-fetch"));
}

#[test]
fn native_offslide_and_hidden_objects_are_inspected_and_removed_on_copy() {
    use review::InspectionCategory::*;
    let mut package = Package::open(pptx::export_pptx(&deck()).unwrap()).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().to_owned(); let parsed = roxmltree::Document::parse(&xml).unwrap();
    let shape = parsed.descendants().find(|node| node.tag_name().name() == "sp" && node.descendants().any(|node| node.attribute("name") == Some("box"))).unwrap();
    let original_shape = &xml[shape.range()];
    let hidden = original_shape.replace("name=\"box\"", "name=\"hidden\" hidden=\"1\"").replace("id=\"3\"", "id=\"99\"");
    let outside = original_shape.replace("name=\"box\"", "name=\"outside\"").replace("id=\"3\"", "id=\"100\"").replace("x=\"381000\"", "x=\"20000000\"");
    package.replace_part("ppt/slides/slide1.xml", xml.replace("</p:spTree>", &format!("{hidden}{outside}</p:spTree>")).into_bytes()).unwrap();
    let bytes = package.save().unwrap(); let original = open(&bytes);
    let report = review::inspect_document(&original).unwrap();
    assert!(report.findings.iter().any(|finding| finding.category == OffSlide && finding.count > 0));
    assert!(report.findings.iter().any(|finding| finding.category == Invisible && finding.count > 0));
    let cleaned = review::export_clean_copy(&original, &clean_options(&[OffSlide, Invisible])).unwrap();
    let package = Package::open(cleaned.bytes).unwrap(); let xml = package.text("ppt/slides/slide1.xml").unwrap();
    assert!(!xml.contains("name=\"hidden\"")); assert!(!xml.contains("name=\"outside\"")); assert!(xml.contains("name=\"box\""));
    assert_eq!(save(&original), bytes);
}

#[test]
fn signed_and_labelled_packages_refuse_clean_copy_and_field_refresh() {
    use review::InspectionCategory::*;
    for path in ["_xmlsignatures/sig1.xml", "docMetadata/LabelInfo.xml"] {
        let mut parts = Package::open(pptx::export_pptx(&deck()).unwrap()).unwrap().parts().clone();
        parts.insert(path.into(), b"<sentinel xmlns='urn:synthetic-protection'/>".to_vec());
        let bytes = Package::from_parts(parts).unwrap().save().unwrap(); let original = open(&bytes);
        assert!(review::export_clean_copy(&original, &clean_options(&[Comments, Notes, CustomXml, Sources])).is_err());
        assert!(fields::refresh_native(bytes.clone(), "2026-09-17").is_err());
        assert_eq!(save(&original), bytes);
    }
}

#[test]
fn native_field_refresh_changes_only_supported_caches() {
    let mut value = serde_json::to_value(deck()).unwrap();
    value["slides"][0]["elements"][0]["text"] = json!("9 unknown");
    value["slides"][0]["elements"][0]["format"] = json!({"paragraphs":[{"runs":[
        {"text":"9","field":{"id":"{00000000-0000-0000-0000-000000000001}","kind":"slidenum"}},
        {"text":" unknown","field":{"id":"{00000000-0000-0000-0000-000000000002}","kind":"vendor-kind"}}
    ]}]});
    let bytes = pptx::export_pptx(&serde_json::from_value(value).unwrap()).unwrap();
    let old = Package::open(bytes.clone()).unwrap();
    let refreshed = fields::refresh_native(bytes, "2026-09-17").unwrap(); let opened = open(&refreshed);
    assert_eq!(serde_json::to_value(&opened.deck.slides[0].elements[0]).unwrap()["text"], "1 unknown");
    let package = Package::open(refreshed.clone()).unwrap();
    for (path, data) in old.parts().iter().filter(|(path, _)| path.as_str() != "ppt/slides/slide1.xml") { assert_eq!(package.part(path).unwrap(), data); }
    assert!(package.text("ppt/slides/slide1.xml").unwrap().contains("vendor-kind"));
    assert_eq!(fields::refresh_native(refreshed.clone(), "2026-09-17").unwrap(), refreshed);
}

#[test]
fn removing_source_bound_hidden_content_requires_sources_selection() {
    use review::InspectionCategory::*;
    let source = sourced(); let mut value = serde_json::to_value(&source.deck).unwrap();
    value["slides"][0]["elements"][0]["visual"] = json!({"hidden":true});
    let source = transact(&source, serde_json::from_value(value).unwrap()); let source = open(&save(&source));
    let snapshot = serde_json::to_vec(&source).unwrap();
    assert!(review::export_clean_copy(&source, &clean_options(&[Invisible])).is_err());
    let cleaned = review::export_clean_copy(&source, &clean_options(&[Invisible, Sources])).unwrap();
    assert!(cleaned.document.sources.is_empty()); assert!(cleaned.document.bindings.is_empty());
    assert!(cleaned.document.deck.slides[0].elements.iter().all(|element| element.bounds().0 != "title"));
    assert_eq!(serde_json::to_vec(&source).unwrap(), snapshot);
}

#[test]
fn foreign_comment_extensions_survive_and_native_identity_cannot_be_reassigned() {
    let deck = comments::add(&deck(), "slide-1", comment("first")).unwrap();
    let mut package = Package::open(pptx::export_pptx(&deck).unwrap()).unwrap();
    let path = package.parts().keys().find(|path| path.starts_with("ppt/comments/")).unwrap().clone();
    let sentinel = "<p:ext uri=\"urn:vendor:retain\"><v:state xmlns:v=\"urn:vendor\" value=\"retained\"/></p:ext>";
    let xml = package.text(&path).unwrap().replace("</p:extLst>", &format!("{sentinel}</p:extLst>"));
    package.replace_part(&path, xml.into_bytes()).unwrap(); let original = open(&package.save().unwrap());
    let changed = comments::resolve(&original.deck, "slide-1", "first", true).unwrap();
    let changed = transact(&original, changed);
    assert!(Package::open(save(&changed)).unwrap().text(&path).unwrap().contains(sentinel));
    let mut invalid = original.deck.clone(); invalid.slides[0].review.as_mut().unwrap().comments[0].native_index = Some(9999);
    assert!(document::transact(&original, Transaction { expected_revision: original.revision, expected_hash: original.hash.clone(), operations: serde_json::from_value(json!([{"op":"replace","path":"/deck","value":invalid}])).unwrap() }).is_err());
    assert!(comments::validate_timestamp("2026-09-17T+1:15:00Z").is_err());
}

#[test]
fn field_refresh_follows_slide_order_and_validates_duplicate_ids() {
    let mut value = serde_json::to_value(deck()).unwrap();
    value["slides"][0]["elements"][0]["text"] = json!("9 1/1/2000");
    value["slides"][0]["elements"][0]["format"] = json!({"paragraphs":[{"runs":[
        {"text":"9","field":{"id":"{00000000-0000-0000-0000-000000000001}","kind":"slidenum"}}, {"text":" "},
        {"text":"1/1/2000","field":{"id":"{00000000-0000-0000-0000-000000000002}","kind":"datetime1"}}
    ]}]});
    let mut first = value["slides"][0].clone(); first["id"] = json!("copy"); value["slides"].as_array_mut().unwrap().insert(0, first);
    let refreshed = fields::refresh(&serde_json::from_value(value.clone()).unwrap(), "2024-02-29").unwrap();
    let result = serde_json::to_value(&refreshed).unwrap();
    assert_eq!(result["slides"][0]["elements"][0]["text"], "1 2/29/2024");
    assert_eq!(result["slides"][1]["elements"][0]["text"], "2 2/29/2024");
    value["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"][2]["field"]["id"] = json!("00000000-0000-0000-0000-000000000001");
    assert!(serde_json::from_value::<Deck>(value).is_err());
}

#[test]
fn adding_field_to_opened_native_presentation_emits_real_field() {
    let original = open(&pptx::export_pptx(&deck()).unwrap());
    let mut next = original.deck.clone();
    let mut value = serde_json::to_value(&next.slides[0].elements[0]).unwrap();
    value["text"] = json!("Page 1");
    value["format"]["paragraphs"] = json!([{"runs":[{"text":"Page "},{"text":"1","field":{"id":"{00112233-4455-6677-8899-aabbccddeeff}","kind":"slidenum"}}]}]);
    next.slides[0].elements[0] = serde_json::from_value(value).unwrap();
    let updated = transact(&original, next); let bytes = save(&updated);
    assert!(fields::has_fields(&open(&bytes).deck.slides[0].elements[0]));
    let package = Package::open(bytes).unwrap();
    let parsed = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    assert_eq!(parsed.descendants().filter(|node| node.tag_name().name() == "fld").count(), 1);
}

#[test]
fn duplicate_display_names_do_not_merge_native_author_identities() {
    let deck = comments::add(&deck(), "slide-1", comment("first")).unwrap();
    let mut package = Package::open(pptx::export_pptx(&deck).unwrap()).unwrap();
    let authors = package.parts().keys().find(|path| path.starts_with("ppt/aislide-commentAuthors")).unwrap().clone();
    let comments_path = package.parts().keys().find(|path| path.starts_with("ppt/comments/")).unwrap().clone();
    let xml = package.text(&authors).unwrap().replace("</p:cmAuthorLst>", "<p:cmAuthor id=\"7\" name=\"Synthetic local reviewer\" initials=\"SL\" lastIdx=\"1\" clrIdx=\"7\"/></p:cmAuthorLst>");
    package.replace_part(&authors, xml.into_bytes()).unwrap();
    let xml = package.text(&comments_path).unwrap().replace("authorId=\"0\"", "authorId=\"7\"");
    package.replace_part(&comments_path, xml.into_bytes()).unwrap();
    let original = open(&package.save().unwrap());
    let next = comments::resolve(&original.deck, "slide-1", "first", true).unwrap();
    let reopened = open(&save(&transact(&original, next)));
    assert_eq!(reopened.deck.slides[0].review.as_ref().unwrap().comments[0].native_author_id, Some(7));
}

#[test]
fn master_field_operation_materializes_unique_linked_placeholders_and_keeps_rich_content() {
    let mut source = deck(); source.design = Some(aislide_core::design::Design::default());
    source.slides[0].layout_id = Some("blank".into());
    let mut second = source.slides[0].clone(); second.id = "slide-2".into(); source.slides.push(second);
    let source = serde_json::to_value(source).unwrap();
    let updated = aislide_core::execute_request(json!({"op":"set_design_field","deck":source,"field":{"master_id":"master-1","kind":"slide_number","reference_date":"2026-09-18"}})).unwrap();
    let updated: Deck = serde_json::from_value(updated).unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for (index, slide) in updated.slides.iter().enumerate() {
        let field = serde_json::to_value(slide.elements.iter().find(|element| fields::has_fields(element)).unwrap()).unwrap();
        assert_eq!(field["text"], (index + 1).to_string());
        assert_eq!(field["format"]["placeholder"]["kind"], "slide_number");
        assert!(ids.insert(field["format"]["paragraphs"][0]["runs"][0]["field"]["id"].as_str().unwrap().to_owned()));
    }
    let original = open(&pptx::export_pptx(&updated).unwrap());
    let refreshed = fields::refresh(&original.deck, "2026-09-19").unwrap();
    let design = refreshed.design.clone().unwrap();
    let reapplied = aislide_core::design::update_design(refreshed.clone(), design).unwrap();
    assert_eq!(serde_json::to_value(&refreshed.slides).unwrap(), serde_json::to_value(&reapplied.slides).unwrap());
    assert_eq!(open(&save(&transact(&original, reapplied))).deck.slides.len(), 2);
    let mut current = original;
    for (kind, text, date) in [("footer", "First footer", "2026-09-18"), ("footer", "Updated footer", "2026-09-18"), ("date", "", "2026-09-18"), ("date", "", "2026-09-19")] {
        let next = aislide_core::execute_request(json!({"op":"set_design_field","deck":current.deck,"field":{"master_id":current.deck.design.as_ref().unwrap().masters[0].id,"kind":kind,"reference_date":date,"text":text}})).unwrap();
        let next: Deck = serde_json::from_value(next).unwrap();
        let expected = if kind == "date" { if date.ends_with("19") { "9/19/2026" } else { "9/18/2026" } } else { text };
        for slide in &next.slides {
            assert!(slide.elements.iter().any(|element| serde_json::to_value(element).unwrap()["text"] == expected));
        }
        current = open(&save(&transact(&current, next)));
    }
}

#[test]
fn static_field_projection_uses_current_page_order_without_mutating_caches() {
    let mut source = deck(); source.design = Some(aislide_core::design::Design::default());
    let source = aislide_core::execute_request(json!({"op":"set_design_field","deck":source,"field":{"master_id":"master-1","kind":"slide_number","reference_date":"2026-09-18"}})).unwrap();
    let mut source: Deck = serde_json::from_value(source).unwrap();
    let mut second = source.slides[0].clone(); second.id = "second".into(); source.slides.insert(0, second);
    let before = serde_json::to_value(&source).unwrap();
    let expected = fields::refresh(&source, "2026-09-18").unwrap();
    let actual_svg = aislide_core::render::render_slide_svg(&source, 1, false).unwrap().svg;
    let expected_svg = aislide_core::render::render_slide_svg(&expected, 1, false).unwrap().svg;
    assert_eq!(actual_svg, expected_svg);
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
}