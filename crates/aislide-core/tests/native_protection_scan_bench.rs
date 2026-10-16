use aislide_core::{document::{self, Document, Transaction}, model::Deck, package::Package, pptx};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{hint::black_box, io::Write, time::Instant};

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn fixture(count: usize, mixed: bool) -> Vec<u8> {
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(32, 32).write_to(&mut png, image::ImageFormat::Png).unwrap();
    let picture = STANDARD.encode(png.into_inner());
    let slides: Vec<_> = (0..count).map(|index| {
        let mut elements: Vec<_> = (0..8).map(|row| json!({
            "type":"text","id":format!("text-{row}"),"x":40,"y":40+row*50,"width":700,"height":40,
            "text":format!("Synthetic slide {index} row {row}"),"font_size":20,"color":"222222","bold":false
        })).collect();
        elements.push(json!({"type":"rect","id":"box","x":820,"y":40,"width":300,"height":150,"fill":"008800"}));
        if mixed {
            elements.push(json!({"type":"picture","id":"image","x":900,"y":220,"width":96,"height":96,
                "base64":picture,"mime_type":"image/png","alt":"Synthetic benchmark image"}));
            if index % 3 == 0 {
                elements.push(json!({"type":"chart","id":"chart","x":40,"y":450,"width":700,"height":200,
                    "kind":"column","categories":["A","B","C"],"series":[{"name":"Synthetic","values":[4,6,5],"color":"087F73"}]}));
            }
        }
        json!({"id":format!("slide-{index}"),"title":format!("Synthetic {index}"),"notes":"Synthetic baseline notes",
            "background":"FFFFFF","elements":elements})
    }).collect();
    let deck: Deck = serde_json::from_value(json!({"version":1,"title":"Protection scan benchmark","width":1280,"height":720,"slides":slides})).unwrap();
    let mut parts = Package::open(pptx::export_pptx(&deck).unwrap()).unwrap().parts().clone();
    for index in 0..8 {
        parts.insert(format!("customXml/benchmark-{index}.xml"), format!("<data>{}</data>", "<item>synthetic</item>".repeat(1000)).into_bytes());
    }
    Package::from_parts(parts).unwrap().save().unwrap()
}

fn edit(source: &Document, text: &str) -> document::TransactionResult {
    document::transact(source, Transaction {
        expected_revision: source.revision, expected_hash: source.hash.clone(),
        operations: serde_json::from_value(json!([{"op":"replace","path":"/deck/slides/0/notes","value":text}])).unwrap(),
    }).unwrap()
}

#[test]
#[ignore = "opt-in release benchmark; writes only the explicitly selected JSON report"]
fn native_note_edit_release_benchmark() {
    assert!(!cfg!(debug_assertions), "run this benchmark with --release");
    let output = std::env::var("AISLIDE_BENCH_OUTPUT").expect("set a new AISLIDE_BENCH_OUTPUT JSON filename");
    let variant = std::env::var("AISLIDE_BENCH_VARIANT").expect("set AISLIDE_BENCH_VARIANT");
    let mut rows = Vec::<Value>::new();
    for (count, mixed) in [(30, false), (128, false), (256, false), (30, true)] {
        let bytes = fixture(count, mixed);
        let package = Package::open(bytes.clone()).unwrap();
        let declarations = package.text("[Content_Types].xml").unwrap();
        let declarations = roxmltree::Document::parse(declarations).unwrap().root_element().children().filter(|node| node.is_element()).count();
        let source: Document = serde_json::from_value(document::open_presentation("benchmark".into(), bytes.clone()).unwrap()["document"].clone()).unwrap();
        let warm = edit(&source, "Already edited synthetic notes").document;
        let expected = edit(&warm, "Measured synthetic notes");
        for _ in 0..2 { black_box(edit(black_box(&warm), "Measured synthetic notes")); }
        let mut samples = Vec::new();
        for _ in 0..9 {
            // transact verifies and seals via native::save; both saves run the protection scan.
            let started = Instant::now();
            let changed = edit(black_box(&warm), "Measured synthetic notes");
            samples.push(started.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(changed.document.hash, expected.document.hash);
        }
        let saved = STANDARD.decode(document::export_presentation(&expected.document).unwrap()["base64"].as_str().unwrap()).unwrap();
        let undone = document::undo(&expected.document, expected.document.revision, expected.receipt.unwrap()).unwrap();
        assert_eq!(undone.document.hash, warm.hash);
        rows.push(json!({"slides":count,"mixed":mixed,"parts":package.parts().len(),"declarations":declarations,
            "fixture_sha256":digest(&bytes),"input_hash":warm.hash,"result_hash":expected.document.hash,
            "result_pptx_sha256":digest(&saved),"samples_ms":samples}));
    }
    let report = json!({"variant":variant,"profile":"release","warmups":2,"samples_per_case":9,"cases":rows});
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(output).unwrap();
    file.write_all(&serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}
