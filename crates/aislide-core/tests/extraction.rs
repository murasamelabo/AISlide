use aislide_core::execute_request;
use base64::{Engine, engine::general_purpose::STANDARD};
use lopdf::{dictionary, content::{Content, Operation}, Document, Object, Stream};
use serde_json::json;

fn pdf(pages: usize, text: &str, compress: bool) -> Vec<u8> {
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let font = document.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    let resources = document.add_object(dictionary! {"Font"=>dictionary! {"F1"=>font}});
    let content = Content { operations: vec![Operation::new("BT", vec![]), Operation::new("Tf", vec!["F1".into(), 24.into()]), Operation::new("Td", vec![30.into(), 700.into()]), Operation::new("Tj", vec![Object::string_literal(text)]), Operation::new("ET", vec![])] };
    let content_id = document.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let mut children = Vec::new();
    for _page in 0..pages {
        children.push(document.add_object(dictionary! {"Type"=>"Page","Parent"=>pages_id,"Resources"=>resources,"Contents"=>content_id,"MediaBox"=>vec![0.into(),0.into(),600.into(),800.into()]}).into());
    }
    document.objects.insert(pages_id, Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>children,"Count"=>pages as i64}));
    let catalog = document.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages_id});
    document.trailer.set("Root", catalog);
    if compress { document.compress(); }
    let mut bytes = Vec::new(); document.save_to(&mut bytes).unwrap(); bytes
}

#[test]
fn pdf_text_has_page_provenance_and_no_factual_verification_claim() {
    let source = execute_request(json!({"op":"ingest","input":{"name":"fixture.pdf","format":"pdf","base64":STANDARD.encode(pdf(2,"Provided value 42",true))}})).unwrap();
    assert_eq!(source["pages"].as_array().unwrap().len(), 2);
    assert!(source["pages"][0]["text"].as_str().unwrap().contains("Provided value 42"));
    assert_eq!(source["pages"][1]["locator"], "page:2");
    assert!(source["warnings"].as_array().unwrap().iter().any(|warning| warning.as_str().unwrap().contains("review")));
    assert!(source["tables"].as_array().unwrap().is_empty());
}

#[test]
fn pdf_pages_and_inflated_content_are_bounded() {
    for bytes in [pdf(33, "too many pages", false), pdf(1, &"A".repeat(5 * 1024 * 1024), true), b"%PDF-1.7 invalid".to_vec()] {
        assert!(execute_request(json!({"op":"ingest","input":{"name":"bad.pdf","format":"pdf","base64":STANDARD.encode(bytes)}})).is_err());
    }
}

#[test]
fn local_ocr_status_is_explicit_about_platform_and_languages() {
    let status = execute_request(json!({"op":"ocr_status"})).unwrap();
    assert!(status["available"].is_boolean());
    assert_eq!(status["remote"], false);
    assert!(status["languages"].is_array());
}