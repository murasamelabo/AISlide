use aislide_core::execute_request;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

fn document() -> Value {
    let report = execute_request(json!({"op":"sample"})).unwrap();
    let compiled = execute_request(json!({"op":"compile","report":report})).unwrap();
    execute_request(json!({"op":"new_document","id":"test-document","deck":compiled["deck"]})).unwrap()
}

fn title_path(document: &Value) -> String {
    let index = document["deck"]["slides"][0]["elements"].as_array().unwrap().iter().position(|element| element["id"] == "title").unwrap();
    format!("/deck/slides/0/elements/{index}/text")
}

fn transact(document: &Value, operations: Value) -> aislide_core::Result<Value> {
    execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":operations}}))
}

fn undo(document: &Value, receipt: &Value) -> aislide_core::Result<Value> {
    execute_request(json!({"op":"undo_transaction","document":document,"expected_revision":document["revision"],"receipt":receipt}))
}

#[test]
fn batch_is_atomic_revision_checked_and_undoable() {
    let original = document();
    let path = title_path(&original);
    let changed = transact(&original, json!([
        {"op":"test","path":path,"value":original.pointer(&path).unwrap()},
        {"op":"replace","path":path,"value":"Edited as one transaction"},
        {"op":"replace","path":"/deck/slides/0/background","value":"EDF3F0"}
    ])).unwrap();
    assert_eq!(changed["document"]["revision"], 1);
    assert_eq!(changed["document"].pointer(&path).unwrap(), "Edited as one transaction");
    assert_eq!(undo(&changed["document"], &changed["receipt"]).unwrap()["document"]["deck"], original["deck"]);
    let stale = execute_request(json!({"op":"transaction","document":changed["document"],"transaction":{"expected_revision":0,"expected_hash":original["hash"],"operations":[{"op":"replace","path":path,"value":"stale"}]}}));
    assert!(stale.is_err());
    assert!(transact(&original, json!([
        {"op":"replace","path":path,"value":"Must not escape"},
        {"op":"replace","path":"/deck/slides/0/elements/0/width","value":-10}
    ])).is_err());
}

#[test]
fn consecutive_undo_uses_content_preconditions_and_monotonic_revisions() {
    let original = document();
    let path = title_path(&original);
    let first = transact(&original, json!([{"op":"replace","path":path,"value":"First"}])).unwrap();
    let second = transact(&first["document"], json!([{"op":"replace","path":path,"value":"Second"}])).unwrap();
    assert!(undo(&second["document"], &first["receipt"]).is_err());
    let undo_second = undo(&second["document"], &second["receipt"]).unwrap();
    let undo_first = undo(&undo_second["document"], &first["receipt"]).unwrap();
    assert_eq!(undo_first["document"]["deck"], original["deck"]);
    assert_eq!(undo_first["document"]["revision"], 4);
    let redo = undo(&undo_first["document"], &undo_first["receipt"]).unwrap();
    assert_eq!(redo["document"].pointer(&path).unwrap(), "First");
}

#[test]
fn malformed_patches_and_direct_document_tampering_are_rejected() {
    let original = document();
    for operations in [json!([]), json!([{"op":"replace","path":"/revision","value":0}]), json!([{"op":"remove","path":"/deck"}]), json!([{"op":"replace","path":"/deck/slides/0/background","value":"<script>"}])] {
        assert!(transact(&original, operations).is_err());
    }
    let mut tampered = original.clone();
    tampered["deck"]["title"] = json!("tampered outside transactions");
    assert!(transact(&tampered, json!([{"op":"replace","path":title_path(&original),"value":"New"}])).is_err());
}

#[test]
fn checkpoints_require_the_exact_pptx_and_matching_scene() {
    let original = document();
    let exported = execute_request(json!({"op":"export_project","document":original})).unwrap();
    let restored = execute_request(json!({"op":"open_project","base64":exported["base64"],"checkpoint":exported["checkpoint"]})).unwrap();
    assert_eq!(restored, original);
    let mut bytes = STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap(); bytes.push(0);
    assert!(execute_request(json!({"op":"open_project","base64":STANDARD.encode(bytes),"checkpoint":exported["checkpoint"]})).is_err());
    let mut checkpoint = exported["checkpoint"].clone(); checkpoint["document"]["deck"]["title"] = json!("changed");
    assert!(execute_request(json!({"op":"open_project","base64":exported["base64"],"checkpoint":checkpoint})).is_err());
}

#[test]
fn source_bindings_become_stale_on_data_edit_and_recover_on_undo() {
    let source = execute_request(json!({"op":"ingest","input":{"format":"csv","name":"provided.csv","base64":STANDARD.encode(b"Quarter,Value\nQ1,-2\nQ2,0\nQ3,12\n")}})).unwrap();
    let report = execute_request(json!({"op":"data_report","source":source,"mapping":{"title":"Provided values","period":"Input period","table_index":0,"category_column":0,"value_columns":[1],"row_start":0,"row_count":3,"chart_kind":"column"}})).unwrap();
    let original = execute_request(json!({"op":"new_document","id":"bound-document","deck":report["compiled"]["deck"],"sources":[source],"bindings":report["bindings"]})).unwrap();
    let chart_index = original["deck"]["slides"][3]["elements"].as_array().unwrap().iter().position(|element| element["type"] == "chart").unwrap();
    let changed = transact(&original, json!([{"op":"replace","path":format!("/deck/slides/3/elements/{chart_index}/series/0/values/0"),"value":999}])).unwrap();
    assert!(changed["document"]["bindings"].as_array().unwrap().iter().any(|binding| binding["stale"] == true));
    assert!(execute_request(json!({"op":"export_project","document":changed["document"]})).is_err());
    let restored = undo(&changed["document"], &changed["receipt"]).unwrap();
    assert!(execute_request(json!({"op":"export_project","document":restored["document"]})).is_ok());
}

#[test]
fn new_project_export_rejects_measured_text_overflow() {
    let original = document();
    let path = title_path(&original);
    let changed = transact(&original, json!([{"op":"replace","path":path,"value":"Long words cannot safely fit into the slide title. ".repeat(50)}])).unwrap();
    let exported = execute_request(json!({"op":"export_project","document":changed["document"]}));
    assert!(exported.is_err(), "project publication must block measured overflow rather than silently clipping content");
}

mod write_performance {
    use super::*;
    use aislide_core::{document::{self, Document, Transaction}, model::Deck};
    use std::{alloc::{GlobalAlloc, Layout, System}, cell::Cell, hint::black_box, time::Instant};

    struct CountingAllocator;
    thread_local! {
        static ALLOCATIONS: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
    }

    fn record(bytes: usize) {
        let _ = ALLOCATIONS.try_with(|counter| {
            if let Some((calls, total)) = counter.get() { counter.set(Some((calls + 1, total + bytes))); }
        });
    }

    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() { record(layout.size()); }
            pointer
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc_zeroed(layout) };
            if !pointer.is_null() { record(layout.size()); }
            pointer
        }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) { unsafe { System.dealloc(pointer, layout) }; }
        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            let resized = unsafe { System.realloc(pointer, layout, size) };
            if !resized.is_null() { record(size); }
            resized
        }
    }

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    fn measured<Output>(operation: impl FnOnce() -> Output) -> (Output, u128, usize, usize) {
        ALLOCATIONS.with(|counter| counter.set(Some((0, 0))));
        let started = Instant::now();
        let output = operation();
        let elapsed = started.elapsed().as_nanos();
        let (calls, bytes) = ALLOCATIONS.with(|counter| counter.replace(None).unwrap());
        (output, elapsed, calls, bytes)
    }

    fn fixture(slides: usize) -> Document {
        let deck: Deck = serde_json::from_value(json!({"version":1,"title":"Synthetic write benchmark","width":1280,"height":720,
            "slides":(0..slides).map(|slide| json!({"id":format!("slide-{slide}"),"title":"Synthetic","background":"FFFFFF","notes":"",
                "elements":(0..4).map(|group| json!({"type":"group","id":format!("group-{group}"),"x":64,"y":120+110*group,
                    "width":1152,"height":100,"view_width":1152,"view_height":100,
                    "children":(0..8).map(|child| json!({"type":"text","id":format!("text-{group}-{child}"),"x":10+140*child,"y":20,
                        "width":130,"height":60,"text":format!("Synthetic {slide}/{group}/{child}"),"font_size":18,"color":"202020","bold":false
                    })).collect::<Vec<_>>()
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>()
        })).unwrap();
        document::create("write-performance".into(), deck, vec![], vec![], None).unwrap()
    }

    fn edit(document: &Document, operations: Value) -> aislide_core::Result<document::TransactionResult> {
        document::transact(document, Transaction { expected_revision: document.revision, expected_hash: document.hash.clone(),
            operations: serde_json::from_value(operations).unwrap() })
    }

    #[test]
    fn unbound_elements_still_validate_and_transactions_keep_exact_undo() {
        let original = fixture(8);
        let (_, _, serialization_calls, _) = measured(|| serde_json::to_value(&original).unwrap());
        let (verified, _, verify_calls, _) = measured(|| document::verify(&original));
        verified.unwrap();
        assert!(verify_calls < serialization_calls * 2, "unbound verification must not build a redundant full element JSON lookup");
        let before = serde_json::to_value(&original).unwrap();
        let path = "/deck/slides/0/elements/0/children/0/text";
        let changed = edit(&original, json!([{"op":"replace","path":path,"value":"Changed"}])).unwrap();
        document::verify(&changed.document).unwrap();
        let restored = document::undo(&changed.document, 1, changed.receipt.unwrap()).unwrap();
        assert_eq!(restored.document.hash, original.hash);
        assert_eq!(serde_json::to_value(&restored.document.deck).unwrap(), before["deck"]);
        assert_eq!(restored.document.revision, 2);
        let noop = edit(&original, json!([{"op":"test","path":path,"value":"Synthetic 0/0/0"}])).unwrap();
        assert!(noop.receipt.is_none());
        assert_eq!(serde_json::to_value(noop.document).unwrap(), before);
        for operations in [
            json!([{"op":"replace","path":"/deck/slides/7/elements/3/children/7/width","value":-1}]),
            json!([{"op":"replace","path":"/deck/title","value":"Temporary"},{"op":"test","path":path,"value":"Wrong"}]),
            json!([{"op":"add","path":"/origin","value":{"base64":"","sha256":"0".repeat(64)}}]),
        ] { assert!(edit(&original, operations).is_err()); }
        let mut tampered = original.clone();
        tampered.deck.title = "Tampered".into();
        assert!(document::verify(&tampered).is_err());
        tampered = original.clone();
        tampered.revision = 9_007_199_254_740_992;
        assert!(document::verify(&tampered).is_err());
        assert!(document::transact(&original, Transaction { expected_revision:1, expected_hash:original.hash.clone(),
            operations:serde_json::from_value(json!([{"op":"replace","path":path,"value":"Ahead"}])).unwrap() }).is_err());
        assert_eq!(serde_json::to_value(&original).unwrap(), before);
    }

    #[test]
    fn sparse_nested_bindings_keep_staleness_source_integrity_and_undo() {
        let source: aislide_core::sources::SourceDocument = serde_json::from_value(execute_request(json!({"op":"ingest","input":{
            "format":"csv","name":"synthetic.csv","base64":STANDARD.encode(b"Label\nSynthetic 0/0/0\n")
        }})).unwrap()).unwrap();
        let bindings = [("text-0-0", "/text"), ("group-0", "/children/0/text"), ("missing", "/text")].map(|(element_id, field)| {
            aislide_core::data_report::SourceBinding { slide_id:"slide-0".into(), element_id:element_id.into(), field:field.into(),
                source_id:source.id.clone(), source_sha256:source.sha256.clone(), locator:source.tables[0].locators[0][0].clone(),
                value:json!("Synthetic 0/0/0"), raw_value:source.tables[0].rows[0][0].clone(), transform:"display_scalar".into(), stale:false }
        }).to_vec();
        let original = document::create("sparse-bindings".into(), fixture(8).deck, vec![source], bindings, None).unwrap();
        assert_eq!(original.bindings.iter().map(|binding|binding.stale).collect::<Vec<_>>(), [false, false, true]);
        document::verify(&original).unwrap();
        let changed = edit(&original, json!([{"op":"replace","path":"/deck/slides/0/elements/0/children/0/text","value":"Changed"}])).unwrap();
        assert!(changed.document.bindings.iter().all(|binding|binding.stale));
        let restored = document::undo(&changed.document, 1, changed.receipt.unwrap()).unwrap();
        assert_eq!(restored.document.hash, original.hash);
        assert_eq!(serde_json::to_value(restored.document.bindings).unwrap(), serde_json::to_value(&original.bindings).unwrap());
        for (field, value) in [("source_sha256", json!("0".repeat(64))), ("locator", json!("missing")),
            ("raw_value", json!("forged")), ("value", json!("forged")), ("transform", json!("unknown"))] {
            assert!(edit(&original, json!([{"op":"replace","path":format!("/bindings/0/{field}"),"value":value}])).is_err());
        }
        assert!(edit(&original, json!([{"op":"replace","path":"/sources/0/tables/0/rows/0/0","value":"forged"}])).is_err());
    }

    #[test]
    fn intermediate_byte_limits_apply_even_without_document_keys() {
        let mut original = fixture(1);
        original.capacity_profile = aislide_core::limits::CapacityProfile::Legacy;
        document::verify(&original).unwrap();
        let oversized = "x".repeat(original.capacity_profile.limits().document_bytes + 1);
        let content = json!({"deck":original.deck,"sources":[],"bindings":[]});
        let mut cases = vec![
            json!([{"op":"add","path":"/padding","value":oversized},{"op":"remove","path":"/padding"}]),
            json!([{"op":"replace","path":"","value":oversized},{"op":"replace","path":"","value":content}]),
        ];
        for key in ["deck", "sources", "bindings"] {
            cases.push(json!([{"op":"remove","path":format!("/{key}")},{"op":"add","path":"/padding","value":oversized},
                {"op":"remove","path":"/padding"},{"op":"add","path":format!("/{key}"),"value":content[key]}]));
        }
        for operations in cases {
            let error = edit(&original, operations).err().expect("intermediate size must reject before a later operation can shrink it");
            assert!(matches!(error, aislide_core::Error::Limit(_)), "{error}");
        }
        let noop = edit(&original, json!([{"op":"remove","path":"/bindings"},{"op":"add","path":"/bindings","value":[]}])).unwrap();
        assert!(noop.receipt.is_none());
        assert_eq!(noop.document.hash, original.hash);
        document::verify(&original).unwrap();
    }

    #[test]
    #[ignore = "bounded synthetic allocation and timing benchmark; run explicitly with --nocapture"]
    fn write_benchmark() {
        let enforce = std::env::var_os("WRITE_ENFORCE_ALLOCATION_BUDGET").is_some();
        for slides in [8, 32, 64] {
            let original = fixture(slides);
            let original_bytes = serde_json::to_vec(&original).unwrap();
            let (_, _, serialization_calls, _) = measured(|| serde_json::to_value(&original).unwrap());
            for operation_count in [1, 16] {
                let operations = json!((0..operation_count).map(|index| json!({"op":"replace",
                    "path":format!("/deck/slides/0/elements/{}/children/{}/text", index / 8, index % 8),"value":"Changed"})).collect::<Vec<_>>());
                let transaction = || Transaction { expected_revision:0, expected_hash:original.hash.clone(), operations:serde_json::from_value(operations.clone()).unwrap() };
                document::verify(&original).unwrap();
                let warm = document::transact(&original, transaction()).unwrap();
                let expected = serde_json::to_vec(&warm).unwrap();
                let mut verify_samples = Vec::new();
                let mut transact_samples = Vec::new();
                let mut verify_calls = 0;
                let mut verify_bytes = 0;
                let mut transact_calls = 0;
                let mut transact_bytes = 0;
                for _ in 0..7 {
                    let (result, elapsed, calls, bytes) = measured(|| document::verify(black_box(&original)));
                    result.unwrap();
                    verify_samples.push(elapsed);
                    verify_calls = calls; verify_bytes = bytes;
                    let request = transaction();
                    let (result, elapsed, calls, bytes) = measured(|| document::transact(black_box(&original), request));
                    let result = result.unwrap();
                    transact_samples.push(elapsed);
                    transact_calls = calls; transact_bytes = bytes;
                    assert_eq!(serde_json::to_vec(&result).unwrap(), expected);
                    let restored = document::undo(&result.document, 1, result.receipt.unwrap()).unwrap();
                    assert_eq!(restored.document.hash, original.hash);
                    assert_eq!(serde_json::to_value(restored.document.deck).unwrap(), serde_json::to_value(&original.deck).unwrap());
                }
                let mut verify_sorted = verify_samples.clone(); verify_sorted.sort_unstable();
                let mut transact_sorted = transact_samples.clone(); transact_sorted.sort_unstable();
                println!("WRITE_BENCH {}", json!({"slides":slides,"elements":slides*36,"operations":operation_count,"samples":7,
                    "document_bytes":original_bytes.len(),"input_sha256":format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(&original_bytes)),
                    "output_sha256":format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(&expected)),"content_hash":warm.document.hash,
                    "verify_ns":verify_samples,"transact_ns":transact_samples,"verify_median_ns":verify_sorted[3],"transact_median_ns":transact_sorted[3],
                    "verify_allocations":verify_calls,"verify_allocated_bytes":verify_bytes,"transact_allocations":transact_calls,"transact_allocated_bytes":transact_bytes,
                    "serialization_allocations":serialization_calls}));
                if enforce { assert!(verify_calls < serialization_calls * 2, "unbound verify must not allocate a second full element JSON lookup: {verify_calls} >= {}", serialization_calls * 2); }
            }
        }
    }
}