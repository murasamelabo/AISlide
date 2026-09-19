use aislide_core::protocol::execute_request;
use serde_json::json;

#[test]
fn local_dictionary_proofing_uses_scalar_offsets_and_explicit_languages() {
    let dictionary = execute_request(json!({"op":"import_proofing_dictionary", "language":"en-US",
        "format":"wordlist", "content":"hello\nworld\nreport\n"})).unwrap();
    let result = execute_request(json!({"op":"proof_text", "language":"en-US",
        "text":"\u{1f600} hello wurld", "dictionary":dictionary})).unwrap();
    assert_eq!(result["issues"][0]["word"], "wurld");
    assert_eq!(result["issues"][0]["start"], 8);
    assert_eq!(result["issues"][0]["end"], 13);
    assert_eq!(result["issues"][0]["suggestions"][0], "world");
    assert_eq!(result["complete_dictionary"], false);
    assert!(execute_request(json!({"op":"proof_text", "language":"fr-FR", "text":"hello",
        "dictionary":dictionary})).is_err());
}

fn document() -> serde_json::Value {
    let source = json!({"type":"text","id":"source","x":10,"y":20,"width":300,"height":100,"text":"Hello","font_size":32,"color":"CC2200","bold":true,
        "format":{"italic":true,"alignment":"center","hyperlink":"https://example.invalid/source","paragraphs":[{"space_after":{"kind":"points","value":800},"runs":[{"text":"Hello","style":{"underline":true,"highlight":"FFFF00","language":"en-US"}}]}]}});
    let target = json!({"type":"text","id":"target","x":400,"y":50,"width":300,"height":200,"text":"Keep mixed\nSecond","font_size":20,"color":"111111","bold":false,
        "format":{"hyperlink":"https://example.invalid/target","paragraphs":[{"runs":[{"text":"Keep ","style":{"highlight":"00FF00","language":"fr-FR"}},{"text":"mixed","style":{"italic":false}}]},{"runs":[{"text":"Second","style":{}}]}]}});
    let locked = json!({"type":"text","id":"locked","x":10,"y":300,"width":300,"height":100,"text":"Locked","font_size":20,"color":"111111","bold":false,"visual":{"locked":true}});
    execute_request(json!({"op":"new_document", "id":"proof-test", "deck":{
        "version":1,"title":"Proof test","width":1280,"height":720,"slides":[{
            "id":"slide","title":"Slide","background":"FFFFFF","notes":"","elements":[source,target,locked]}]}})).unwrap()
}

#[test]
fn painter_is_read_only_then_atomic_preserving_content_geometry_and_semantics() {
    let document = document();
    let style = execute_request(json!({"op":"copy_format","document":document,"slide_id":"slide","id":"source","paragraph_index":0,"run_index":0})).unwrap();
    assert!(!style.to_string().contains("example.invalid"));
    let result = execute_request(json!({"op":"apply_format","document":document,"expected_revision":0,"slide_id":"slide","ids":["target"],"style":style})).unwrap();
    let target = &result["document"]["deck"]["slides"][0]["elements"][1];
    for field in ["text","id","x","y","width","height"] { assert_eq!(target[field], document["deck"]["slides"][0]["elements"][1][field]); }
    assert_eq!(target["font_size"].as_f64(),Some(20.0));
    assert_eq!(target["format"]["paragraphs"][0]["runs"][0]["style"]["font_size"].as_f64(),Some(32.0));
    assert_eq!(target["format"]["hyperlink"],"https://example.invalid/target");
    assert_eq!(target["format"]["paragraphs"].as_array().unwrap().len(),2);
    assert_eq!(target["format"]["paragraphs"][0]["runs"][0]["style"]["highlight"],"00FF00");
    assert_eq!(target["format"]["paragraphs"][0]["runs"][0]["style"]["language"],"fr-FR");
    assert_eq!(target["format"]["paragraphs"][1]["runs"][0]["style"]["underline"],true);
    for ids in [json!(["target","locked"]), json!(["target","target"]), json!([]), json!(["missing"])] {
        assert!(execute_request(json!({"op":"apply_format","document":document,"expected_revision":0,"slide_id":"slide","ids":ids,"style":style})).is_err());
    }
    assert!(execute_request(json!({"op":"apply_format","document":document,"expected_revision":9,"slide_id":"slide","ids":["target"],"style":style})).is_err());
    let undo = execute_request(json!({"op":"undo_transaction","document":result["document"],"expected_revision":1,"receipt":result["receipt"]})).unwrap();
    assert_eq!(undo["document"]["deck"],document["deck"]);
}

#[test]
fn synonym_and_term_translation_import_is_bounded_strict_and_offline() {
    let content = json!({"language":"en-US","words":["report"],"synonyms":{"report":["summary"]},"translations":{"ja-JP":{"report":"報告"}}}).to_string();
    let dictionary = execute_request(json!({"op":"import_proofing_dictionary","language":"en-US","format":"json","content":content})).unwrap();
    let result = execute_request(json!({"op":"proof_text","language":"en-US","text":"report","term":"REPORT","target_language":"ja-JP","dictionary":dictionary})).unwrap();
    assert_eq!(result["synonyms"],json!(["summary"]));
    assert_eq!(result["translation"],"報告");
    for content in ["word\u{0}", &"a".repeat(524289), "bad word"] {
        assert!(execute_request(json!({"op":"import_proofing_dictionary","language":"en-US","format":"wordlist","content":content})).is_err());
    }
    assert!(execute_request(json!({"op":"proof_text","text":"hello","language":"en-US","remote":true})).is_err());
    let result = execute_request(json!({"op":"proof_text","text":"hello zzz","language":"en-US"})).unwrap();
    assert_eq!(result["sample_dictionary"],true);
    assert_eq!(result["issues"][0]["word"],"zzz");
}

#[test]
fn painter_styles_empty_runs_without_cloning_field_identities() {
    let mut deck = document()["deck"].clone();
    deck["slides"][0]["elements"][1]["text"] = json!("");
    deck["slides"][0]["elements"][1]["format"]["paragraphs"] = json!([{"runs":[{"text":"","style":{}}]}]);
    deck["slides"][0]["elements"][2] = json!({"type":"shape","id":"field","x":10,"y":300,"width":300,"height":100,"preset":"rect","fill":"FFFFFF","stroke":"000000","stroke_width":1,"text":"1","font_size":20,"color":"111111","bold":false,
        "format":{"paragraphs":[{"runs":[{"text":"1","style":{},"field":{"id":"9a65ddcd-8d65-48c2-9d25-a6e59e50dc67","kind":"slidenum"}}]}]}});
    let document = execute_request(json!({"op":"new_document","id":"empty-field","deck":deck})).unwrap();
    let style = execute_request(json!({"op":"copy_format","document":document,"slide_id":"slide","id":"source"})).unwrap();
    let result = execute_request(json!({"op":"apply_format","document":document,"expected_revision":0,"slide_id":"slide","ids":["target","field"],"style":style})).unwrap();
    let elements = &result["document"]["deck"]["slides"][0]["elements"];
    assert_eq!(elements[1]["format"]["paragraphs"][0]["runs"][0]["style"]["font_size"].as_f64(),Some(32.0));
    assert_eq!(elements[2]["format"]["paragraphs"][0]["runs"][0]["field"],document["deck"]["slides"][0]["elements"][2]["format"]["paragraphs"][0]["runs"][0]["field"]);
    assert_eq!(elements[2]["fill"],"FFFFFF");
}

#[test]
fn language_operations_are_undoable_strict_and_respect_locked_targets() {
    let document = document();
    for id in ["locked","missing"] {
        assert!(execute_request(json!({"op":"set_proofing_language","document":document,"expected_revision":0,"slide_id":"slide","id":id,"language":"en-US"})).is_err());
    }
    let result = execute_request(json!({"op":"set_proofing_language","document":document,"expected_revision":0,"slide_id":"slide","id":"target","language":"en-US"})).unwrap();
    assert_eq!(result["document"]["deck"]["slides"][0]["elements"][1]["text"],document["deck"]["slides"][0]["elements"][1]["text"]);
    assert_eq!(result["document"]["deck"]["slides"][0]["elements"][1]["format"]["paragraphs"][0]["runs"][0]["style"]["highlight"],"00FF00");
    let undo = execute_request(json!({"op":"undo_transaction","document":result["document"],"expected_revision":1,"receipt":result["receipt"]})).unwrap();
    assert_eq!(undo["document"]["hash"],document["hash"]);
    for language in ["", "-en", "en--US", "en_US"] {
        assert!(execute_request(json!({"op":"set_proofing_language","document":document,"expected_revision":0,"slide_id":"slide","id":"target","language":language})).is_err());
    }
    let mut deck = document["deck"].clone();
    let target = deck["slides"][0]["elements"][1].clone();
    deck["slides"][0]["elements"] = json!([{"type":"group","id":"locked-parent","x":0,"y":0,"width":1000,"height":600,"view_width":1000,"view_height":600,"visual":{"locked":true},"children":[target]}]);
    let nested = execute_request(json!({"op":"new_document","id":"nested-language","deck":deck})).unwrap();
    assert!(execute_request(json!({"op":"set_proofing_language","document":nested,"expected_revision":0,"slide_id":"slide","id":"target","language":"en-US"})).is_err());
}

#[test]
fn painter_rejects_content_semantics_injected_into_style_snapshot() {
    let document = document();
    let style = execute_request(json!({"op":"copy_format","document":document,"slide_id":"slide","id":"source"})).unwrap();
    for (field, value) in [("highlight",json!("none")),("language",json!("fr-FR")),("hyperlink",json!("https://example.invalid/"))] {
        let mut injected = style.clone(); injected["run"][field] = value;
        assert!(execute_request(json!({"op":"apply_format","document":document,"expected_revision":0,"slide_id":"slide","ids":["target"],"style":injected})).is_err());
    }
    let mut injected = style; injected["paragraph"]["runs"] = json!([{"text":"source content","style":{}}]);
    assert!(execute_request(json!({"op":"apply_format","document":document,"expected_revision":0,"slide_id":"slide","ids":["target"],"style":injected})).is_err());
}

#[test]
fn phase2_object_painter_native_styles_preserve_data_and_undo() {
    use base64::{Engine,engine::general_purpose::STANDARD};
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(2,2).write_to(&mut png,image::ImageFormat::Png).unwrap();
    let picture = execute_request(json!({"op":"create_asset","id":"source","base64":STANDARD.encode(png.into_inner()),"mime_type":"image/png","alt":"Retain asset","size":100})).unwrap();
    let mut sources = vec![
        json!({"type":"rect","id":"source","x":10,"y":20,"width":100,"height":100,"fill":"CC2200","visual":{"opacity":0.5}}),
        json!({"type":"polygon","id":"source","x":10,"y":20,"width":100,"height":100,"points":[[0,0],[1,0],[0,1]],"fill":"CC2200","stroke":"0055AA","stroke_width":3}),
        picture,
    ];
    for kind in ["shape","table","chart"] {
        sources.push(execute_request(json!({"op":"create_object","id":"source","kind":kind})).unwrap());
    }
    for mut source in sources {
        source["x"] = json!(10); source["y"] = json!(20); source["width"] = json!(400); source["height"] = json!(400);
        if source["type"] == "picture" { source["visual"] = json!({"opacity":0.5,"glow":{"color":"CC2200","opacity":0.8,"radius":4}}); }
        if source["type"] == "table" { source["font_size"] = json!(28); source["format"] = json!({"cells":[{"row":0,"column":0,"style":{"fill":"CC2200","text_style":{"bold":true}}}]}); }
        if source["type"] == "chart" { source["series"][0]["color"] = json!("CC2200"); source["options"] = json!({"legend":"top"}); }
        if source["type"] == "shape" { source["fill"] = json!("CC2200"); source["stroke_width"] = json!(3); }
        let mut target = source.clone(); target["id"] = json!("target"); target["x"] = json!(500);
        if target.get("fill").is_some() { target["fill"] = json!("FFFFFF"); }
        if target.get("visual").is_some() { target["visual"] = json!({}); }
        if target["type"] == "chart" { target["series"][0]["values"][0] = json!(123); target["series"][0]["color"] = json!("00AA11"); target["options"] = json!({}); }
        if target["type"] == "table" { target["rows"][0][0] = json!("Keep cell"); target["font_size"] = json!(18); target["format"] = json!({}); }
        if target["type"] == "shape" { target["text"] = json!("Keep shape"); target["format"]["hyperlink"] = json!("https://example.invalid/retain"); }
        let mut deck = document()["deck"].clone(); deck["slides"][0]["elements"] = json!([source,target]);
        let fresh = execute_request(json!({"op":"new_document","id":"object-style","deck":deck})).unwrap();
        let save = |document: &serde_json::Value| execute_request(json!({"op":"export_presentation","document":document})).unwrap()["base64"].clone();
        let open = |bytes: serde_json::Value| execute_request(json!({"op":"open_presentation","id":"object-style","base64":bytes})).unwrap()["document"].clone();
        for document in [fresh.clone(),open(save(&fresh))] {
            let bytes = save(&document);
            let style = execute_request(json!({"op":"copy_format","document":document,"slide_id":"slide","id":"source"})).unwrap();
            let result = execute_request(json!({"op":"apply_format","document":document,"expected_revision":document["revision"],"slide_id":"slide","ids":["target"],"style":style})).unwrap();
            let before = &document["deck"]["slides"][0]["elements"][1];
            let after = &result["document"]["deck"]["slides"][0]["elements"][1];
            for key in ["id","x","y","width","height","points","text","rows","categories","base64","mime_type","alt","crop","preset","rotation"] { assert_eq!(before[key],after[key],"{} {key}",source["type"]); }
            if source["type"] == "chart" { assert_eq!(before["series"][0]["values"],after["series"][0]["values"]); assert_eq!(after["series"][0]["color"],"CC2200"); }
            if source.get("fill").is_some() { assert_eq!(after["fill"],source["fill"]); }
            assert_eq!(before["format"]["hyperlink"],after["format"]["hyperlink"]);
            assert_ne!(before,after);
            let reopened = open(save(&result["document"])); assert_eq!(reopened["deck"]["slides"][0]["elements"].as_array().unwrap().len(),2);
            let undo = execute_request(json!({"op":"undo_transaction","document":result["document"],"expected_revision":result["document"]["revision"],"receipt":result["receipt"]})).unwrap();
            assert_eq!(save(&undo["document"]),bytes);
            assert!(execute_request(json!({"op":"apply_format","document":document,"expected_revision":document["revision"],"slide_id":"slide","ids":["target","missing"],"style":style})).is_err());
            assert_eq!(save(&document),bytes);
        }
    }
}

#[test]
fn phase2_object_snapshots_reject_semantics_and_preserve_target_geometry() {
    let source = json!({"type":"polygon","id":"source","x":10,"y":20,"width":100,"height":100,"points":[[0,0],[1,0],[0,1]],"fill":"CC2200","stroke":"0055AA","stroke_width":3,"visual":{"flip_h":true,"rotation":20,"shadow":{"color":"000000","opacity":0.5,"blur":5,"distance":3,"angle":45}}});
    let target = json!({"type":"polygon","id":"target","x":300,"y":20,"width":150,"height":100,"points":[[0,0],[1,0],[1,1],[0,1]],"fill":"FFFFFF","stroke":"000000","stroke_width":1,"visual":{"rotation":45,"flip_v":true}});
    let mut deck = document()["deck"].clone(); deck["slides"][0]["elements"] = json!([source,target]);
    let document = execute_request(json!({"op":"new_document","id":"style-guard","deck":deck})).unwrap();
    let before = document.clone();
    let style = execute_request(json!({"op":"copy_format","document":document,"slide_id":"slide","id":"source"})).unwrap();
    assert!(!style.to_string().contains("rotation")); assert!(!style.to_string().contains("points"));
    let apply = |style| execute_request(json!({"op":"apply_format","document":document,"expected_revision":0,"slide_id":"slide","ids":["target"],"style":style}));
    let result = apply(style.clone()).unwrap();
    let after = &result["document"]["deck"]["slides"][0]["elements"][1];
    assert_eq!(after["points"],before["deck"]["slides"][0]["elements"][1]["points"]);
    assert_eq!(after["visual"]["rotation"],45.0); assert_eq!(after["visual"]["flip_v"],true);
    assert_eq!(after["visual"]["shadow"],before["deck"]["slides"][0]["elements"][0]["visual"]["shadow"]);
    for (field,value) in [("path",json!({"commands":[]})),("rotation",json!(70)),("hidden",json!(true)),("url",json!("file:///never-read"))] {
        let mut invalid=style.clone(); invalid["effects"][field]=value; assert!(apply(invalid).is_err());
    }
    let table_snapshot=json!({"kind":"table","font_size":24,"rows":1,"columns":1,"cells":[{"row":0,"column":0,"style":{"text_format":{"paragraphs":[{"runs":[{"text":"Injected","style":{}}]}]}}}]});
    assert!(apply(table_snapshot).is_err());
    assert_eq!(document,before);
}

#[test]
fn phase2_table_painter_copies_rich_style_not_cell_content_or_track_geometry() {
    let source = json!({"type":"table","id":"source","x":10,"y":10,"width":400,"height":200,"rows":[["Source"]],"font_size":24,"format":{"cells":[{"row":0,"column":0,"style":{"fill":"CC2200","text_format":{"paragraphs":[{"alignment":"center","space_after":{"kind":"points","value":600},"runs":[{"text":"Source","style":{"bold":true,"color":"0055AA","highlight":"FFFF00","language":"en-US"}}]}]}}}]}});
    let target = json!({"type":"table","id":"target","x":500,"y":10,"width":400,"height":200,"rows":[["Keep"]],"font_size":18,"format":{"column_widths":{"unit":"relative","values":[2]},"cells":[{"row":0,"column":0,"style":{"text_format":{"paragraphs":[{"runs":[{"text":"Keep","style":{"highlight":"00FF00","language":"fr-FR"}}]}]}}}]}});
    let mut deck=document()["deck"].clone(); deck["slides"][0]["elements"]=json!([source,target]);
    let document=execute_request(json!({"op":"new_document","id":"table-style","deck":deck})).unwrap();
    let style=execute_request(json!({"op":"copy_format","document":document,"slide_id":"slide","id":"source"})).unwrap();
    assert!(!style.to_string().contains("Source"));
    let result=execute_request(json!({"op":"apply_format","document":document,"expected_revision":0,"slide_id":"slide","ids":["target"],"style":style})).unwrap();
    let painted=&result["document"]["deck"]["slides"][0]["elements"][1];
    let paragraph=&painted["format"]["cells"][0]["style"]["text_format"]["paragraphs"][0];
    assert_eq!(paragraph["runs"][0]["style"]["bold"],true);
    assert_eq!(paragraph["runs"][0]["style"]["color"],"0055AA");
    assert_eq!(paragraph["runs"][0]["style"]["language"],"fr-FR");
    assert_eq!(paragraph["runs"][0]["style"]["highlight"],"00FF00");
    assert_eq!(paragraph["space_after"]["value"],600);
    assert_eq!(painted["rows"],target["rows"]);
    assert_eq!(painted["format"]["column_widths"],document["deck"]["slides"][0]["elements"][1]["format"]["column_widths"]);
    let bytes=execute_request(json!({"op":"export_presentation","document":result["document"]})).unwrap()["base64"].clone();
    assert!(execute_request(json!({"op":"open_presentation","id":"table-style","base64":bytes})).is_ok());
}