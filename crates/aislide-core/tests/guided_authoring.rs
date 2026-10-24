use aislide_core::execute_request;
use serde_json::{json, Value};

fn brief(profile: &str) -> Value {
    json!({"version":1,"profile_id":profile,"title":"Source-bound briefing","audience":"Decision owners","purpose":"Choose the first remediation step","governing_message":"We should prioritize the highest measured exposure.","language":"en","evidence":[{"id":"source-a","kind":"source","reference":"Provided synthetic test fixture, not factual business data","statement":"The fixture values are 12 and 24."}],"slides":[{"id":"evidence-slide","section":"Exposure","headline":"We should prioritize the larger exposure because it doubles the baseline.","sentence_form":"causal","pattern_id":"native-part","question":"Which exposure should be addressed first?","parent_message":"governing","transition":"because","parallel_basis":"exposure","part":{"version":1,"preset":"horizontal-bar-graph/labeled","title":"Exposure by area","subtitle":"Synthetic test fixture / units","data":{"kind":"chart","categories":["Baseline","Priority"],"series":[{"name":"Units","values":[12,24]}],"x_axis":"Units","y_axis":"Area"}},"support":[{"clause":"We should prioritize the larger exposure because it doubles the baseline.","body_paths":["/data/series/0/values"],"evidence_ids":["source-a"]}],"numbers":[{"path":"/data/series/0/values/0","value":12,"evidence_id":"source-a"},{"path":"/data/series/0/values/1","value":24,"evidence_id":"source-a"}]}]})
}

fn authoring_brief(profile: &str) -> Value {
    let mut input = brief(profile);
    input["slides"][0]["headline"] = json!("Review the evidence before proceeding.");
    input["slides"][0]["support"][0]["clause"] = input["slides"][0]["headline"].clone();
    input["slides"][0]["support"][0]["body_paths"] = json!(["/data/items"]);
    input["slides"][0]["numbers"] = json!([]);
    input["slides"][0]["part"] = json!({"version":1,"preset":"list-horizontal/balanced","title":"Stages","subtitle":"Review sequence","data":{"kind":"items","items":[{"label":"Check","detail":"Review"},{"label":"Act","detail":"Proceed"}]}});
    input
}

fn part_brief(spec: Value) -> Value {
    let mut input = authoring_brief("status-report");
    input["slides"][0]["part"] = spec;
    input["slides"][0]["support"][0]["body_paths"] = json!(["/data"]);
    input
}

fn catalog_brief(preset: &str) -> Value {
    let catalog = execute_request(json!({"op":"part_catalog"})).unwrap();
    let spec = catalog["presets"].as_array().unwrap().iter().find(|entry| entry["id"] == preset).unwrap()["example"].clone();
    let mut input = part_brief(spec.clone());
    fn numbers(value: &Value, path: &str, output: &mut Vec<Value>) {
        match value {
            Value::Number(_) => output.push(json!({"path":path,"value":value,"evidence_id":"source-a"})),
            Value::Array(values) => for (index, value) in values.iter().enumerate() { numbers(value,&format!("{path}/{index}"),output); },
            Value::Object(values) => for (key, value) in values { numbers(value,&format!("{path}/{}",key.replace('~',"~0").replace('/',"~1")),output); },
            _ => {}
        }
    }
    let mut declarations = Vec::new();
    numbers(&spec["data"],"/data",&mut declarations);
    input["slides"][0]["numbers"] = json!(declarations);
    input
}

#[test]
fn review46_data_labels_keep_the_requested_body_floor() {
    for preset in ["matrix/raci", "flow/sankey", "flow/journey", "horizontal-bar-graph/bullet", "list/agenda"] {
        let mut input = catalog_brief(preset);
        input["authoring"] = json!({"font_family":"BIZ UDPGothic","body_font_min":16});
        let created = execute_request(json!({"op":"create_guided_presentation","id":"data-label-floor","input":input})).unwrap_or_else(|error| panic!("{preset}: {error}"));
        assert_body_floor(&created["document"]["deck"]["slides"][0]["elements"][4],1.0,16.0);
        fn font_families(value: &Value) {
            match value {
                Value::Object(fields) => for (key,value) in fields {
                    if key=="font_family" && !value.is_null() {assert_eq!(value,"BIZ UDPGothic");}
                    font_families(value);
                },
                Value::Array(values) => for value in values {font_families(value);},
                _ => {}
            }
        }
        font_families(&created["document"]["deck"]["slides"][0]["elements"][4]);
        let report = execute_request(json!({"op":"preflight_presentation","document":created["document"],"options":{"min_font_size":16}})).unwrap();
        assert!(!report["findings"].as_array().unwrap().iter().any(|finding| finding["code"] == "SMALL_TEXT" && finding["element_ids"].as_array().unwrap().iter().any(|id| id.as_str().is_some_and(|id| id.starts_with("guided-part-")))), "{preset}: {report}");
    }
}

#[test]
fn review46_legacy_briefing_text_keeps_the_twelve_pixel_floor() {
    for preset in ["flow/cards", "horizontal-bar-graph/bullet", "matrix/risk", "flow/swimlane"] {
        let created = execute_request(json!({"op":"create_guided_presentation","id":"legacy-label-floor","input":catalog_brief(preset)})).unwrap();
        assert_body_floor(&created["document"]["deck"]["slides"][0]["elements"][4],1.0,12.0);
    }
}

#[test]
fn review46_generated_guided_fields_use_ooxml_guid_lexical_form() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let created = execute_request(json!({"op":"create_guided_presentation","id":"guid-lexical-form","input":authoring_brief("status-report")})).unwrap();
    let exported = execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
    let package = aislide_core::package::Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    let xml = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    let fields = xml.descendants().filter(|node| node.tag_name().name() == "fld").collect::<Vec<_>>();
    assert_eq!(fields.len(),2);
    for field in fields {
        let id = field.attribute("id").unwrap();
        assert_eq!(id.len(),38,"DrawingML ST_Guid requires braces: {id}");
        assert!(id.starts_with('{') && id.ends_with('}'),"{id}");
        assert_eq!(id,id.to_ascii_uppercase(),"DrawingML ST_Guid requires uppercase: {id}");
        aislide_core::fields::parse_id(id).unwrap();
    }
}

#[test]
fn review46_managed_page_fields_do_not_add_cached_text_warnings() {
    let created = execute_request(json!({"op":"create_guided_presentation","id":"page-warning-scope","input":authoring_brief("status-report")})).unwrap();
    let exported = execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"page-warning-native","base64":exported["base64"]})).unwrap();
    for document in [&created["document"],&reopened["document"]] {
        let deck = serde_json::from_value(document["deck"].clone()).unwrap();
        let rendered = aislide_core::render::render_slide_svg(&deck,0,false).unwrap();
        assert!(!rendered.warnings.iter().any(|warning| warning.code == "FIELD_CACHED_TEXT"),"{:?}",rendered.warnings);
        let report = execute_request(json!({"op":"preflight_presentation","document":document})).unwrap();
        assert!(!report["findings"].as_array().unwrap().iter().any(|finding| finding["code"] == "FIELD_CACHED_TEXT"),"{report}");
    }
    for kind in ["datetime1","vendor_unknown"] {
        let mut deck = created["document"]["deck"].clone();
        deck["slides"][0]["elements"][3]["format"]["paragraphs"][0]["runs"][0]["field"]["kind"] = json!(kind);
        let rendered = aislide_core::render::render_slide_svg(&serde_json::from_value(deck).unwrap(),0,false).unwrap();
        assert!(rendered.warnings.iter().any(|warning| warning.code == "FIELD_CACHED_TEXT"),"cached {kind} must still warn");
    }
}

#[test]
fn review46_alt_descriptions_avoid_empty_sections_and_keep_matrix_relations() {
    let created = execute_request(json!({"op":"create_guided_presentation","id":"readable-chart-alt","input":brief("status-report")})).unwrap();
    let root = &created["document"]["deck"]["slides"][0]["review"]["accessibility"]["guided-part-1"]["description"];
    let description = root.as_str().unwrap();
    assert_eq!(description.matches("Exposure by area").count(),1,"{description}");
    assert!(!description.contains("Connections:"),"{description}");
    let created = execute_request(json!({"op":"create_guided_presentation","id":"readable-matrix-alt","input":catalog_brief("matrix/raci")})).unwrap();
    let description = created["document"]["deck"]["slides"][0]["review"]["accessibility"]["guided-part-1"]["description"].as_str().unwrap();
    assert!(description.contains("Define requirements: Product = A/R"),"{description}");
    assert!(description.contains("Build and test: Product = C; Engineering = A/R"),"{description}");
}

#[test]
fn review46_protected_page_edits_explain_the_supported_authored_style_path() {
    let created = execute_request(json!({"op":"create_guided_presentation","id":"page-edit-guidance","input":authoring_brief("status-report")})).unwrap();
    let document = &created["document"];
    let styled = execute_request(json!({"op":"apply_operations","document":document,
        "expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
            {"op":"set_text_style","slide_id":"evidence-slide","ids":["page"],"style":{"bold":true,"font_size":14}}
        ]})).unwrap();
    let page = &styled["document"]["deck"]["slides"][0]["elements"][3];
    assert_eq!(page["font_size"].as_f64(),Some(14.0)); assert_eq!(page["bold"],true); assert_eq!(page["text"],"1 / 1");
    for index in [0,2] {
        assert_eq!(page["format"]["paragraphs"][0]["runs"][index]["field"],document["deck"]["slides"][0]["elements"][3]["format"]["paragraphs"][0]["runs"][index]["field"]);
    }
    let saved_style = execute_request(json!({"op":"export_presentation","document":styled["document"]})).unwrap();
    let reopened_style = execute_request(json!({"op":"open_presentation","id":"page-styled-native","base64":saved_style["base64"]})).unwrap();
    let page = &reopened_style["document"]["deck"]["slides"][0]["elements"][3];
    assert_eq!(page["font_size"].as_f64(),Some(14.0)); assert_eq!(page["bold"],true); assert_eq!(page["text"],"1 / 1");
    for index in [0,2] {
        assert_eq!(page["format"]["paragraphs"][0]["runs"][index]["field"],document["deck"]["slides"][0]["elements"][3]["format"]["paragraphs"][0]["runs"][index]["field"]);
    }
    let error = execute_request(json!({"op":"replace_text_content","document":document,
        "expected_revision":document["revision"],"slide_id":"evidence-slide","id":"page","text":"Manual footer"})).unwrap_err().to_string();
    assert!(error.contains("set_text_style") && error.contains("authored source") && error.contains("refresh_fields"),"{error}");
    let exported = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"page-edit-guidance-native","base64":exported["base64"]})).unwrap();
    let document = &reopened["document"];
    let error = execute_request(json!({"op":"apply_operations","document":document,
        "expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[
            {"op":"set_text_style","slide_id":"evidence-slide","ids":["page"],"style":{"bold":true}}
        ]})).unwrap_err().to_string();
    assert!(error.contains("set_text_style") && error.contains("authored source") && error.contains("protected"),"{error}");
}

#[test]
fn issue38_guided_page_fields_follow_import_reorder_remove_and_native_reopen() {
    use aislide_core::{document, slide_import};
    use base64::{engine::general_purpose::STANDARD, Engine};
    let mut input = authoring_brief("status-report");
    let mut second = input["slides"][0].clone(); second["id"] = json!("second");
    input["slides"].as_array_mut().unwrap().push(second);
    let source: document::Document = serde_json::from_value(execute_request(json!({"op":"create_guided_presentation","id":"pages-source","input":input})).unwrap()["document"].clone()).unwrap();
    let target: document::Document = serde_json::from_value(execute_request(json!({"op":"create_presentation","id":"pages-target","title":"Pagination"})).unwrap()).unwrap();
    for (native_source, native_target) in [(false,false),(false,true),(true,false),(true,true)] {
    let native = |document: &document::Document, id: &str| -> document::Document {
        let exported = document::export_presentation(document).unwrap();
        serde_json::from_value(document::open_presentation(id.into(), STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap()["document"].clone()).unwrap()
    };
    let source = if native_source { native(&source,"pages-native-source") } else { source.clone() };
    let target = if native_target { native(&target,"pages-native-target") } else { target.clone() };
    if native_source {
        let before = serde_json::to_value(&target).unwrap();
        let Err(error) = slide_import::import(&target,target.revision,&target.hash,&source,
            &source.deck.slides.iter().map(|slide|slide.id.clone()).collect::<Vec<_>>(),"imported",None)
            else { panic!("native-source cross-package imports must remain unsupported"); };
        assert!(error.to_string().contains("authored source documents only"), "{error}");
        assert_eq!(serde_json::to_value(&target).unwrap(), before);
        continue;
    }
    let imported = slide_import::import(&target, target.revision, &target.hash, &source,
        &source.deck.slides.iter().map(|slide| slide.id.clone()).collect::<Vec<_>>(), "imported", None).unwrap();
    fn check(deck: &aislide_core::model::Deck, expected: &[&str]) {
        for (index, slide) in deck.slides.iter().enumerate() {
            for element in &slide.elements {
                if let aislide_core::model::Element::Text { format, .. } = element {
                    let fields = format.paragraphs.iter().flat_map(|paragraph| &paragraph.runs).filter_map(|run| run.field.as_ref().map(|field| (field, &run.text))).collect::<Vec<_>>();
                    if fields.iter().any(|(field, _)| field.kind == "slidenum") {
                        assert_eq!(fields.len(), 2);
                        assert_eq!(fields[0].0.kind, "slidenum"); assert_eq!(fields[0].1, &(index + 1).to_string());
                        assert_eq!(fields[1].0.kind, "aislide_slidetotal"); assert_eq!(fields[1].1, &deck.slides.len().to_string());
                        assert_ne!(fields[0].0.id, fields[1].0.id);
                    }
                }
            }
        }
        let pages: Vec<_> = deck.slides.iter().flat_map(|slide| &slide.elements).filter_map(|element| {
            if let aislide_core::model::Element::Text { text, format, .. } = element {
                if format.paragraphs.iter().flat_map(|paragraph| &paragraph.runs).any(|run| run.field.as_ref().is_some_and(|field| field.kind == "slidenum")) { return Some(text.as_str()); }
            }
            None
        }).collect();
        assert_eq!(pages, expected);
    }
    fn field_ids(deck: &aislide_core::model::Deck) -> Vec<String> {
        deck.slides.iter().flat_map(|slide| &slide.elements).filter_map(|element| match element {
            aislide_core::model::Element::Text { format, .. } => Some(format),
            _ => None,
        }).flat_map(|format| format.paragraphs.iter().flat_map(|paragraph| &paragraph.runs))
            .filter_map(|run| run.field.as_ref().map(|field| field.id.clone())).collect()
    }
    check(&imported.document.deck, &["2 / 3", "3 / 3"]);
    let changed = document::transact(&imported.document, document::Transaction {
        expected_revision: imported.document.revision, expected_hash: imported.document.hash.clone(),
        operations: serde_json::from_value(json!([
            {"op":"move","from":"/deck/slides/2","path":"/deck/slides/0"},
            {"op":"remove","path":"/deck/slides/1"}
        ])).unwrap(),
    }).unwrap_or_else(|error| panic!("native_source={native_source}, native_target={native_target}: {error}"));
    check(&changed.document.deck, &["1 / 2", "2 / 2"]);
    let exported = document::export_presentation(&changed.document).unwrap();
    let bytes = STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap();
    let reopened: document::Document = serde_json::from_value(document::open_presentation("pages-reopened".into(), bytes.clone()).unwrap()["document"].clone()).unwrap();
    check(&reopened.deck, &["1 / 2", "2 / 2"]);
    assert_eq!(field_ids(&reopened.deck), field_ids(&changed.document.deck));
    let mut package = aislide_core::package::Package::open(bytes).unwrap();
    let paths = package.parts().keys().filter(|path| path.starts_with("ppt/slides/") && path.ends_with(".xml")).cloned().collect::<Vec<_>>();
    for path in paths {
        let mut xml = package.text(&path).unwrap().to_owned();
        let parsed = roxmltree::Document::parse(&xml).unwrap();
        let mut ranges = parsed.descendants().filter(|node| node.tag_name().name() == "fld" && node.attribute("type") == Some("aislide_slidetotal"))
            .flat_map(|node| node.children().filter(|node| node.tag_name().name() == "t").map(|node| node.range())).collect::<Vec<_>>();
        ranges.reverse();
        for range in ranges { xml.replace_range(range, "<a:t>99</a:t>"); }
        package.replace_part(&path, xml.into_bytes()).unwrap();
    }
    let stale: document::Document = serde_json::from_value(document::open_presentation("pages-stale".into(), package.save().unwrap()).unwrap()["document"].clone()).unwrap();
    let stale_deck = serde_json::to_value(&stale.deck).unwrap();
    for slide in stale_deck["slides"].as_array().unwrap() {
        assert_eq!(slide["elements"][3]["format"]["paragraphs"][0]["runs"][2]["text"], "99");
    }
    let refreshed = aislide_core::authoring_ops::refresh_fields(&stale, stale.revision, "2026-10-24").unwrap();
    check(&refreshed.document.deck, &["1 / 2", "2 / 2"]);
    assert_eq!(field_ids(&refreshed.document.deck), field_ids(&reopened.deck));
    let restored = document::undo(&changed.document, changed.document.revision, changed.receipt.unwrap()).unwrap();
    assert_eq!(restored.document.hash, imported.document.hash);
    }
}

fn icon_cards_brief() -> Value {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use image::ImageEncoder;
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(&[20, 180, 90, 255], 1, 1, image::ExtendedColorType::Rgba8).unwrap();
    let icon = json!({"base64":STANDARD.encode(png),"mime_type":"image/png","alt":"Provided synthetic square icon"});
    part_brief(json!({"version":1,"preset":"list-horizontal/icon-cards","title":"Provided stages","data":{
        "kind":"icon_cards","cards":[{"label":"Review","detail":"Check the supplied evidence.","icon":icon},{"label":"Act","detail":"Proceed after review.","icon":icon}]
    }}))
}

#[test]
fn review_native_pagination_combines_cache_and_frame_edits_without_replacing_fields() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let mut input = authoring_brief("status-report");
    let mut second = input["slides"][0].clone(); second["id"] = json!("second");
    input["slides"].as_array_mut().unwrap().push(second);
    let created = execute_request(json!({"op":"create_guided_presentation","id":"combined-fields","input":input})).unwrap();
    let original = execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
    let opened = execute_request(json!({"op":"open_presentation","id":"combined-fields-native","base64":original["base64"]})).unwrap()["document"].clone();
    let changed = execute_request(json!({"op":"transaction","document":opened,"transaction":{
        "expected_revision":opened["revision"],"expected_hash":opened["hash"],"operations":[
            {"op":"move","from":"/deck/slides/1","path":"/deck/slides/0"},
            {"op":"replace","path":"/deck/slides/0/elements/3/x","value":1092}
        ]
    }})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"combined-fields-result","base64":saved["base64"]})).unwrap();
    let footer = &reopened["document"]["deck"]["slides"][0]["elements"][3];
    assert_eq!(footer["text"], "1 / 2"); assert_eq!(footer["x"], 1092.0);
    assert_eq!(footer["format"]["paragraphs"][0]["runs"][0]["field"]["kind"], "slidenum");
    assert_eq!(footer["format"]["paragraphs"][0]["runs"][2]["field"]["kind"], "aislide_slidetotal");
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":1,"receipt":changed["receipt"]})).unwrap();
    let restored = execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap();
    assert_eq!(STANDARD.decode(restored["base64"].as_str().unwrap()).unwrap(), STANDARD.decode(original["base64"].as_str().unwrap()).unwrap());
}

#[test]
fn review_native_field_cache_refresh_composes_with_preset_and_explicit_padding() {
    let slides = (0..2).map(|index| json!({"id":format!("page-{index}"),"title":"Field padding","background":"FFFFFF","notes":"","elements":[{
        "type":"shape","id":"counter","x":40,"y":40,"width":240,"height":120,"preset":"roundRect","fill":"FFFFFF","stroke":"000000","stroke_width":1,
        "text":format!("{} / 2",index+1),"font_size":24,"color":"000000","bold":false,"format":{
            "padding":{"left":20,"right":20,"top":16,"bottom":16},"paragraphs":[{"runs":[
                {"text":(index+1).to_string(),"field":{"id":format!("00000000-0000-4000-8000-{:012}",index*2+1),"kind":"slidenum"}},
                {"text":" / "},{"text":"2","field":{"id":format!("00000000-0000-4000-8000-{:012}",index*2+2),"kind":"aislide_slidetotal"}}
            ]}]
        }
    }]})).collect::<Vec<_>>();
    let deck = json!({"version":1,"title":"Synthetic native field padding","width":1280,"height":720,"slides":slides});
    let source = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let original = execute_request(json!({"op":"open_presentation","id":"native-padding-fields","base64":source["base64"]})).unwrap()["document"].clone();
    for explicit in [false, true] {
        let mut operations = vec![
            json!({"op":"move","from":"/deck/slides/1","path":"/deck/slides/0"}),
            json!({"op":"replace","path":"/deck/slides/0/elements/0/width","value":400}),
            json!({"op":"replace","path":"/deck/slides/0/elements/0/height","value":200}),
        ];
        if explicit { operations.push(json!({"op":"replace","path":"/deck/slides/0/elements/0/format/padding","value":{"left":40,"right":24,"top":18,"bottom":20}})); }
        let changed = execute_request(json!({"op":"transaction","document":original,"transaction":{
            "expected_revision":original["revision"],"expected_hash":original["hash"],"operations":operations
        }})).unwrap();
        let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        let reopened = execute_request(json!({"op":"open_presentation","id":"native-padding-fields-reopened","base64":saved["base64"]})).unwrap();
        let before = &changed["document"]["deck"]["slides"][0]["elements"][0];
        let after = &reopened["document"]["deck"]["slides"][0]["elements"][0];
        assert_eq!(after["text"], "1 / 2");
        for side in ["left","right","top","bottom"] {
            assert!((before["format"]["padding"][side].as_f64().unwrap()-after["format"]["padding"][side].as_f64().unwrap()).abs()<0.001,
                "explicit={explicit}, {side}: expected {}, reopened {}",before["format"]["padding"][side],after["format"]["padding"][side]);
        }
        for index in [0,2] { assert_eq!(after["format"]["paragraphs"][0]["runs"][index]["field"], before["format"]["paragraphs"][0]["runs"][index]["field"]); }
    }
}

#[test]
fn issue39_guided_icons_keep_square_frames_and_native_aspect() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let input = icon_cards_brief();
    let created = execute_request(json!({"op":"create_guided_presentation","id":"icon-aspect","input":input})).unwrap();
    fn check(deck: &Value) {
        let children = deck["slides"][0]["elements"][4]["children"].as_array().unwrap();
        let pictures: Vec<_> = children.iter().filter(|element| element["type"] == "picture").collect();
        assert_eq!(pictures.len(), 2);
        for picture in pictures {
            let ratio = picture["width"].as_f64().unwrap() / picture["height"].as_f64().unwrap();
            assert!((ratio - 1.0).abs() < 0.001, "square icon was distorted to {ratio}");
            assert_eq!(picture["alt"], "Provided synthetic square icon");
        }
        let circles = children.iter().filter(|element| element["preset"] == "ellipse").collect::<Vec<_>>();
        assert_eq!(circles.len(), 2);
        for circle in circles {
            assert!((circle["width"].as_f64().unwrap() / circle["height"].as_f64().unwrap() - 1.0).abs() < 0.001, "icon marker became an oval");
        }
    }
    check(&created["document"]["deck"]);
    let preflight = execute_request(json!({"op":"preflight_presentation","document":created["document"],"options":{"page_indices":[0]}})).unwrap();
    assert!(!preflight["findings"].as_array().unwrap().iter().any(|finding| finding["code"] == "IMAGE_ASPECT_DISTORTED"));
    let saved = execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"icon-reopened","base64":STANDARD.encode(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap())})).unwrap();
    check(&reopened["document"]["deck"]);
}

#[test]
fn review_guided_non_square_cover_preserves_crop_and_native_pixels() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use image::ImageEncoder;
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(
        &[255,0,0,255, 0,255,0,255, 0,0,255,255, 255,255,0,255], 4, 1, image::ExtendedColorType::Rgba8).unwrap();
    let image = json!({"base64":STANDARD.encode(png),"mime_type":"image/png","alt":"Supplied synthetic color strips"});
    let input = part_brief(json!({"version":1,"preset":"flow/cards","title":"Supplied cover images","data":{
        "kind":"step_cards","steps":[{"label":"Review","image":image},{"label":"Decide","image":image},{"label":"Act","image":image}]
    }}));
    let created = execute_request(json!({"op":"create_guided_presentation","id":"cover-crop","input":input})).unwrap();
    let exported = execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"cover-crop-reopened","base64":exported["base64"]})).unwrap();
    let pictures = |deck: &Value| deck["slides"][0]["elements"][4]["children"].as_array().unwrap().iter().filter(|element| element["type"]=="picture").cloned().collect::<Vec<_>>();
    let before = pictures(&created["document"]["deck"]);
    let after = pictures(&reopened["document"]["deck"]);
    assert_eq!(before.len(), 3); assert_eq!(after.len(), before.len());
    fn pixels(picture: &Value) -> Vec<u8> {
        let mut picture = picture.clone();
        let aspect = picture["width"].as_f64().unwrap()/picture["height"].as_f64().unwrap();
        picture["x"]=json!(16);picture["y"]=json!(16);picture["width"]=json!(96.0*aspect);picture["height"]=json!(96);
        let deck = serde_json::from_value(json!({"version":1,"title":"Crop pixels","width":320,"height":320,
            "slides":[{"id":"image","title":"Synthetic crop","background":"FFFFFF","notes":"","elements":[picture]}]})).unwrap();
        let output = aislide_core::export_static::export_static(&deck, &aislide_core::export_static::ExportOptions::default()).unwrap();
        image::load_from_memory(&output.artifacts[0].bytes).unwrap().into_rgba8().into_raw()
    }
    for (before, after) in before.iter().zip(&after) {
        assert!(["left","right","top","bottom"].iter().any(|side| before["crop"][side].as_f64().is_some_and(|value| value>0.1)));
        for side in ["left","right","top","bottom"] {
            assert!((before["crop"][side].as_f64().unwrap_or(0.0)-after["crop"][side].as_f64().unwrap_or(0.0)).abs()<0.00002);
        }
        assert_eq!(after["alt"], before["alt"]);
        let original=pixels(before);let restored=pixels(after);
        let difference=original.iter().zip(&restored).map(|(a,b)| (*a as f64-*b as f64).abs()).sum::<f64>()/original.len() as f64;
        assert!(difference<0.1, "native crop pixels changed by mean {difference}");
    }
}

#[test]
fn issue40_layout_parameters_do_not_require_factual_numeric_evidence() {
    let mut input = icon_cards_brief();
    input["slides"][0]["part"]["data"]["columns"] = json!(2);
    input["slides"][0]["part"]["data"]["body_size"] = json!(16);
    let checked = execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
    assert_eq!(checked["ready"], true, "{checked}");
    let mut factual = brief("status-report");
    factual["slides"][0]["numbers"] = json!([]);
    let checked = execute_request(json!({"op":"validate_guided_presentation","input":factual})).unwrap();
    assert_eq!(checked["ready"], false);
    assert!(checked["issues"].to_string().contains("/data/series/0/values/0"));
}

#[test]
fn issue41_labeled_charts_show_grouped_exact_values_without_mixed_units() {
    let spec = json!({"version":1,"preset":"horizontal-bar-graph/labeled","title":"Supplied test values",
        "data":{"kind":"chart","categories":["Large","Decimal","Precise"],"series":[
            {"name":"First","values":[41172,4.3,-1234.567890123]},
            {"name":"Second","values":[7497,2.2,0.00000012345]}
        ]}});
    let element = execute_request(json!({"op":"create_part","id":"exact-values","spec":spec})).unwrap();
    let strings: Vec<_> = element["children"].as_array().unwrap().iter().filter_map(|element| element["text"].as_str()).collect();
    for expected in ["41,172 / 7,497", "4.3 / 2.2", "-1,234.567890123 / 0.00000012345"] {
        assert!(strings.contains(&expected), "{expected}: {strings:?}");
    }
    let deck = json!({"version":1,"title":"Synthetic exact native values","width":1280,"height":720,
        "slides":[{"id":"values","title":"Values","background":"FFFFFF","notes":"","elements":[element]}]});
    let saved = execute_request(json!({"op":"export","deck":deck})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"exact-values-reopened","base64":saved["base64"]})).unwrap();
    let before = element["children"].as_array().unwrap().iter().find(|element| element["type"] == "chart").unwrap();
    let after = reopened["document"]["deck"]["slides"][0]["elements"][0]["children"].as_array().unwrap().iter().find(|element| element["type"] == "chart").unwrap();
    assert_eq!(after["series"].as_array().unwrap().len(), before["series"].as_array().unwrap().len());
    assert_eq!(after["categories"], before["categories"]);
    for (before, after) in before["series"].as_array().unwrap().iter().zip(after["series"].as_array().unwrap()) {
        assert_eq!(after["values"], before["values"]);
    }
}

#[test]
fn issue42_guided_group_and_chart_descriptions_preserve_supplied_content() {
    let created = execute_request(json!({"op":"create_guided_presentation","id":"guided-alt","input":brief("status-report")})).unwrap();
    let document = &created["document"];
    let report = execute_request(json!({"op":"check_accessibility","document":document})).unwrap();
    assert!(!report["issues"].as_array().unwrap().iter().any(|issue| ["group_alt_missing","chart_alt_missing"].contains(&issue["code"].as_str().unwrap())), "{report}");
    let entries = &document["deck"]["slides"][0]["review"]["accessibility"];
    assert!(entries.to_string().contains("Exposure by area"));
    for text in ["Baseline", "Priority", "Units", "12", "24"] { assert!(entries.to_string().contains(text), "{text}: {entries}"); }
}

#[test]
fn issue43_guided_typography_preserves_small_explicit_diagram_headers() {
    let graph = json!({"version":1,"title":"Provided flow","show_title":false,
        "groups":[{"id":"boundary","label":"Boundary","x":24,"y":24,"width":1104,"height":440,"padding":16,"header_height":24,"header_font_size":8}],
        "nodes":[{"id":"a","label":"Review","x":64,"y":100,"width":240,"height":120,"group":"boundary"},
                 {"id":"b","label":"Act","x":760,"y":100,"width":240,"height":120,"group":"boundary"}],
        "edges":[{"id":"next","source":"a","target":"b","label":"Approved"}]});
    let mut input = part_brief(json!({"version":1,"preset":"diagram/custom","title":"Provided flow","data":{"kind":"diagram","graph":graph}}));
    input["authoring"] = json!({"font_family":"BIZ UDPGothic","body_font_min":16});
    input["slides"][0]["numbers"] = json!([
        {"path":"/data/graph/groups/0/padding","value":16,"evidence_id":"source-a"},
        {"path":"/data/graph/groups/0/header_height","value":24,"evidence_id":"source-a"},
        {"path":"/data/graph/groups/0/header_font_size","value":8,"evidence_id":"source-a"}
    ]);
    let mut without_style_evidence = input.clone();
    without_style_evidence["slides"][0]["numbers"] = json!([]);
    let checked = execute_request(json!({"op":"validate_guided_presentation","input":without_style_evidence})).unwrap();
    assert_eq!(checked["ready"], true, "{checked}");
    let created = execute_request(json!({"op":"create_guided_presentation","id":"fixed-header","input":input})).unwrap();
    let header = created["document"]["deck"]["slides"][0]["elements"][4]["children"].as_array().unwrap().iter()
        .find(|element| element["text"] == "Boundary").unwrap();
    assert!(header["font_size"].as_f64().unwrap() < 12.0, "{header}");
    assert_eq!(header["format"]["font_family"], "BIZ UDPGothic");
    let nodes = created["document"]["deck"]["slides"][0]["elements"][4]["children"].as_array().unwrap().iter()
        .filter(|element| element["id"].as_str().is_some_and(|id| id.contains("-n-"))).collect::<Vec<_>>();
    assert_eq!(nodes.len(), 2);
    for node in nodes { assert_body_floor(node, 1.0, 16.0); }
}

#[test]
fn issue44_guided_step_badges_fit_the_requested_wide_font() {
    let mut input = part_brief(json!({"version":1,"preset":"flow/cards","title":"Provided sequence",
        "data":{"kind":"step_cards","step_label":"STEP","steps":[
            {"label":"Review","detail":"Read the evidence."},{"label":"Decide","detail":"Make a decision."},{"label":"Act","detail":"Carry it out."}
        ]}}));
    input["authoring"] = json!({"font_family":"BIZ UDPGothic","body_font_min":16});
    let created = execute_request(json!({"op":"create_guided_presentation","id":"wide-step-badges","input":input})).unwrap();
    let measured = execute_request(json!({"op":"measure_layout","deck":created["document"]["deck"]})).unwrap();
    assert!(!measured["measurements"].as_array().unwrap().iter().any(|measurement| measurement["overflow"] == true), "{measured}");
    let children = created["document"]["deck"]["slides"][0]["elements"][4]["children"].as_array().unwrap();
    for badge in children.iter().filter(|element| element["text"].as_str().is_some_and(|text| text.starts_with("STEP "))) {
        assert!(badge["font_size"].as_f64().unwrap() <= 12.0, "{badge}");
        assert_eq!(badge["format"]["font_family"], "BIZ UDPGothic");
    }
    for body in children.iter().filter(|element| element["text"].as_str().is_some_and(|text| !text.is_empty() && !text.starts_with("STEP "))) {
        assert_body_floor(body, 1.0, 16.0);
    }
}

fn assert_body_floor(element: &Value, scale: f64, floor: f64) {
    if let Some(size) = element["font_size"].as_f64() {
        assert!(size * scale + 0.001 >= floor, "effective size {} below {floor}: {element}", size * scale);
    }
    fn styles(value: &Value, scale: f64, floor: f64) {
        match value {
            Value::Object(fields) => {for (name,value) in fields {
                if name=="font_size" {if let Some(size)=value.as_f64() {assert!(size*scale+0.001>=floor,"rich/cell size {size} below effective floor {floor}");}}
                styles(value,scale,floor);
            }}
            Value::Array(values) => for value in values {styles(value,scale,floor);},
            _ => (),
        }
    }
    styles(&element["format"],scale,floor);
    if let Some(children) = element["children"].as_array() {
        let child_scale = scale * (element["width"].as_f64().unwrap() / element["view_width"].as_f64().unwrap())
            .min(element["height"].as_f64().unwrap() / element["view_height"].as_f64().unwrap());
        for child in children { assert_body_floor(child, child_scale, floor); }
    }
}

#[test]
fn guided_authoring_slide_limit_is_explicit_and_bounded() {
    let mut input=authoring_brief("status-report");
    let slide=input["slides"][0].clone();
    input["slides"]=json!((0..32).map(|index| {
        let mut slide=slide.clone();slide["id"]=json!(format!("slide-{index}"));slide
    }).collect::<Vec<_>>());
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],true);
    input["slides"]=json!((0..39).map(|index| {
        let mut slide=slide.clone();slide["id"]=json!(format!("slide-{index}"));slide
    }).collect::<Vec<_>>());
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
    input["authoring"]=json!({"slide_limit":39});
    let checked=execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
    assert_eq!(checked["ready"],true,"{checked}");
    let created=execute_request(json!({"op":"create_guided_presentation","id":"bounded-slides","input":input})).unwrap();
    assert_eq!(created["document"]["deck"]["slides"].as_array().unwrap().len(),39);
    for limit in [31,38,129] {
        input["authoring"]["slide_limit"]=json!(limit);
        assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
        assert!(execute_request(json!({"op":"create_guided_presentation","id":"invalid-limit","input":input})).is_err());
    }
}

#[test]
fn guided_authoring_keyword_headlines_keep_support_and_numeric_checks() {
    let mut input=brief("consulting-decision");
    input["language"]=json!("ja");
    input["slides"][0]["headline"]=json!("Priority");
    input["slides"][0]["support"][0]["clause"]=json!("Priority");
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
    input["authoring"]=json!({"headline_style":"sentence"});
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
    input["authoring"]["headline_style"]=json!("keyword");
    let checked=execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
    assert_eq!(checked["ready"],true,"{checked}");
    for (path,value,expected) in [
        ("/slides/0/support/0/clause",json!("Other"),"reconstruct"),
        ("/slides/0/support/0/body_paths",json!(["/title"]),"body evidence"),
        ("/slides/0/support/0/evidence_ids",json!(["missing"]),"missing evidence"),
        ("/slides/0/parent_message",json!("missing"),"parent"),
        ("/slides/0/numbers",json!([]),"no source"),
        ("/slides/0/numbers/0/value",json!(13),"path/value mismatch"),
        ("/evidence/0/kind",json!("unknown"),"missing or unknown"),
        ("/slides/0/sentence_form",json!("unsupported"),"unknown sentence form"),
    ] {
        let mut invalid=input.clone();*invalid.pointer_mut(path).unwrap()=value;
        let checked=execute_request(json!({"op":"validate_guided_presentation","input":invalid})).unwrap();
        assert_eq!(checked["ready"],false,"{path}: {checked}");
        assert!(checked["issues"].to_string().contains(expected),"{path}: {checked}");
        assert!(execute_request(json!({"op":"create_guided_presentation","id":"invalid-keyword","input":invalid})).is_err());
    }
}

#[test]
fn guided_authoring_validation_options_preserve_legacy_typography() {
    let input=authoring_brief("status-report");
    let original=execute_request(json!({"op":"create_guided_presentation","id":"validation-options","input":input})).unwrap();
    for settings in [json!({"headline_style":"sentence"}),json!({"headline_style":"keyword"}),json!({"slide_limit":32}),json!({"slide_limit":128,"headline_style":"keyword"}),json!({"ledger_notes":"full"}),json!({"ledger_notes":"full","slide_limit":32,"headline_style":"sentence"})] {
        let mut input=input.clone();input["authoring"]=settings;
        let created=execute_request(json!({"op":"create_guided_presentation","id":"validation-options","input":input})).unwrap();
        assert_eq!(created,original);
    }
}

#[test]
fn guided_authoring_slide_limit_rejects_invalid_types_and_accepts_upper_bound() {
    let mut input=authoring_brief("status-report");
    for settings in [json!({"headline_style":"title"}),json!({"slide_limit":0}),json!({"slide_limit":31}),json!({"slide_limit":129}),json!({"slide_limit":-1}),json!({"slide_limit":39.5}),json!({"slide_limit":"39"})] {
        input["authoring"]=settings.clone();
        let checked=execute_request(json!({"op":"validate_guided_presentation","input":input}));
        assert!(checked.as_ref().map_or(true,|value|value["ready"]==false),"{settings}: {checked:?}");
    }
    let slide=input["slides"][0].clone();
    input["authoring"]=json!({"slide_limit":128});
    input["slides"]=json!((0..128).map(|index| {
        let mut slide=slide.clone();slide["id"]=json!(format!("slide-{index}"));slide
    }).collect::<Vec<_>>());
    let checked=execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
    assert_eq!(checked["ready"],true,"{checked}");
    let mut extra=slide;extra["id"]=json!("slide-128");input["slides"].as_array_mut().unwrap().push(extra);
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
}

#[test]
fn guided_authoring_keyword_only_waives_sentence_variation_in_decision_decks() {
    let mut input=brief("consulting-decision");
    input["issues"]=json!((1..=3).map(|index|json!({"id":format!("Q{index}"),"question":"Scope","requested_decision":"Approve pilot","criterion":"Evidence reviewed","owner":"Owner","due":"After approval","evidence_ids":["source-a"],"analysis_slide_ids":["evidence-slide"]})).collect::<Vec<_>>());
    let template=|id:&str,pattern:&str|json!({"id":id,"section":"Decision","headline":"Priority","sentence_form":"causal","pattern_id":pattern,"question":"What should be approved?","parent_message":"governing","transition":"therefore","parallel_basis":"decision issue","support":[{"clause":"Priority","body_paths":["/issues"],"evidence_ids":["source-a"]}],"numbers":[]});
    input["slides"]=json!([template("summary","C02"),input["slides"][0].clone(),template("closing","C03")]);
    let checked=execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
    assert_eq!(checked["ready"],false);
    assert!(checked["issues"].to_string().contains("vary sentence forms"),"{checked}");
    input["authoring"]=json!({"headline_style":"keyword"});
    let checked=execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
    assert_eq!(checked["ready"],true,"{checked}");
    for (path,value) in [("/issues/0/analysis_slide_ids",json!(["summary"])),("/slides/0/pattern_id",json!("C03")),("/slides/2/pattern_id",json!("C02")),("/issues",json!([]))] {
        let mut invalid=input.clone();*invalid.pointer_mut(path).unwrap()=value;
        assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":invalid})).unwrap()["ready"],false,"{path}");
    }
    let headline="12 delays across 4 teams within 30 days";
    input["slides"][1]["headline"]=json!(headline);input["slides"][1]["support"][0]["clause"]=json!(headline);
    let checked=execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
    assert_eq!(checked["ready"],false);
    assert!(checked["issues"].to_string().contains("more than two numeric claims"),"{checked}");
}

#[test]
fn guided_authoring_graph_detail_typography_is_not_a_factual_number() {
    let mut input=authoring_brief("status-report");
    input["authoring"]=json!({"headline_style":"keyword"});
    input["slides"][0]["part"]=json!({"version":1,"preset":"diagram/custom","title":"Review","data":{"kind":"diagram","graph":{"version":1,"title":"Review","show_title":false,"nodes":[{"id":"review","label":"Review","detail":"Check evidence\nConfirm scope","detail_font_size":16,"font_size":24,"text_align":"left","x":0,"y":0,"width":360,"height":200}]}}});
    input["slides"][0]["support"][0]["body_paths"]=json!(["/data/graph/nodes/0/detail"]);
    let created=execute_request(json!({"op":"create_guided_presentation","id":"guided-detail","input":input})).unwrap();
    assert_eq!(created["validation"]["ready"],true);
    assert_eq!(created["document"]["parts"][0]["stale"],false);
    assert_eq!(created["document"]["parts"][0]["spec"]["data"]["graph"]["nodes"][0]["detail_font_size"],16.0);
    let children=created["document"]["deck"]["slides"][0]["elements"][4]["children"].as_array().unwrap();
    let node=children.iter().find(|child|child["id"].as_str().unwrap().ends_with("-n-review")).unwrap();
    assert_eq!(node["text"],"Review\nCheck evidence\nConfirm scope");
    assert_eq!(node["format"]["paragraphs"][1]["runs"][0]["style"]["font_size"],16.0);
    assert!(children.iter().all(|child|!child["id"].as_str().unwrap().ends_with("-nd-review")));
    assert!(children.iter().all(|child|!child["id"].as_str().unwrap().ends_with("-title")));
}

#[test]
fn guided_authoring_defaults_distinguish_reading_and_projection() {
    for (profile, floor, headline_size) in [("status-report",16.0,34.0),("technical-explainer",16.0,34.0),("consulting-decision",16.0,34.0),("event-talk",24.0,40.0)] {
        let mut input = authoring_brief(profile);
        input["authoring"] = json!({});
        let result = execute_request(json!({"op":"create_guided_presentation","id":"authoring-defaults","input":input}));
        assert!(result.is_ok(), "authoring settings should compile: {result:?}");
        let created = result.unwrap();
        let elements = created["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
        assert_eq!(elements.iter().find(|element|element["id"]=="headline").unwrap()["font_size"], headline_size);
        assert_body_floor(elements.iter().find(|element|element["id"]=="guided-part-1").unwrap(), 1.0, floor);
        assert_eq!(created["document"]["parts"][0]["stale"], false);
    }
}

#[test]
fn guided_authoring_omitted_preserves_legacy_rendering_and_notes() {
    for profile in ["status-report","event-talk"] {for double in [false,true] {
        let mut input=authoring_brief(profile);
        if double {
            input["slides"][0]["headline"]=json!("The measured baseline needs another review because current evidence does not establish the expected operational benefit.");
            input["slides"][0]["support"][0]["clause"]=input["slides"][0]["headline"].clone();
        }
        let created=execute_request(json!({"op":"create_guided_presentation","id":"legacy","input":input})).unwrap();
        let slide=&created["document"]["deck"]["slides"][0];
        let elements=slide["elements"].as_array().unwrap();
        let headline=elements.iter().find(|element|element["id"]=="headline").unwrap();
        assert_eq!(headline["font_size"],if double {29.0} else {32.0});
        assert_eq!(headline["height"],if double {86.0} else {52.0});
        assert_eq!(elements[0]["font_size"],18.0);
        assert_eq!(elements[2]["font_size"],12.0);
        let mut expected=execute_request(json!({"op":"create_part","id":"guided-part-1","spec":input["slides"][0]["part"],"theme":created["document"]["deck"]["design"]["theme"]})).unwrap();
        if expected.get("element").is_some() {expected=expected["element"].clone();}
        let height=if double {474.0} else {498.0};
        fn scale(value:&mut Value,factor:f64) {
            for field in ["y","height","view_height"] {if let Some(number)=value[field].as_f64() {value[field]=json!(number*factor);}}
            if let Some(size)=value["font_size"].as_f64() {value["font_size"]=json!((size*factor).clamp(8.0,120.0).max(12.0));}
            if let Some(size)=value["stroke_width"].as_f64().filter(|size|*size>0.0) {value["stroke_width"]=json!((size*factor).max(0.5));}
            if let Some(children)=value.get_mut("children").and_then(Value::as_array_mut) {for child in children {scale(child,factor);}}
        }
        for child in expected["children"].as_array_mut().unwrap() {
            scale(child,height/512.0);
            if profile=="event-talk" && child["type"]=="text" && child["y"].as_f64().unwrap()>70.0 && child["height"].as_f64().unwrap()>=40.0 {child["font_size"]=json!(child["font_size"].as_f64().unwrap().max(22.0));}
        }
        expected["height"]=json!(height);expected["view_height"]=json!(height);expected["y"]=json!(if double {168.0} else {144.0});
        assert_eq!(elements.iter().find(|element|element["id"]=="guided-part-1").unwrap(),&expected);
        let notes=slide["notes"].as_str().unwrap();
        assert!(!notes.contains("speaker_notes"));
        assert!(notes.ends_with("Semantic truth and Office parity require human review."));
        input["authoring"]=Value::Null;
        let null_settings=execute_request(json!({"op":"create_guided_presentation","id":"legacy","input":input})).unwrap();
        assert_eq!(created,null_settings);
    }}
}

#[test]
fn guided_authoring_overrides_are_visible_and_native_metadata_roundtrips() {
    for (profile,context,floor,headline_size) in [("status-report","projection",24.0,40.0),("event-talk","reading",16.0,34.0)] {
        let mut input=authoring_brief(profile);
        input["authoring"]=json!({"context":context});
        let created=execute_request(json!({"op":"create_guided_presentation","id":"context-override","input":input})).unwrap();
        let elements=created["document"]["deck"]["slides"][0]["elements"].as_array().unwrap();
        assert_eq!(elements[1]["font_size"],headline_size);
        assert_body_floor(&elements[4],1.0,floor);
    }
    let mut input=authoring_brief("status-report");
    input["authoring"]=json!({"body_font_min":24,"headline_font_size":48,"font_family":"Arial","density":"compact","spacing":"relaxed"});
    input["brand_color"]=json!("A12233");
    input["slides"][0]["part"]["data"]["items"][0]["detail"]=json!("Review\nThen act");
    let created=execute_request(json!({"op":"create_guided_presentation","id":"explicit-authoring","input":input})).unwrap();
    let deck=&created["document"]["deck"];
    assert_eq!(deck["design"]["theme"]["colors"]["accent1"],"A12233");
    for role in ["major","minor","east_asian","complex_script"] {assert_eq!(deck["design"]["theme"]["fonts"][role],"Arial");}
    let elements=deck["slides"][0]["elements"].as_array().unwrap();
    assert_eq!(elements[1]["font_size"],48.0);
    assert_eq!(elements[4]["x"],48.0);assert_eq!(elements[4]["width"],1184.0);
    assert_body_floor(&elements[4],1.0,24.0);
    let detail=elements[4]["children"].as_array().unwrap().iter().find(|child|child["text"]=="Review\nThen act").unwrap();
    assert_eq!(detail["format"]["font_family"],"Arial");
    assert_eq!(detail["format"]["paragraphs"][0]["line_spacing"],json!({"kind":"percent","value":130000}));
    assert_eq!(detail["format"]["paragraphs"][0]["space_after"],json!({"kind":"percent","value":0}));
    let mut comfortable=input.clone();comfortable["authoring"]["density"]=json!("comfortable");comfortable["authoring"]["spacing"]=json!("standard");
    let alternative=execute_request(json!({"op":"create_guided_presentation","id":"comfortable-authoring","input":comfortable})).unwrap();
    assert_ne!(deck["slides"][0]["elements"],alternative["document"]["deck"]["slides"][0]["elements"]);
    let exported=execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"authoring-reopened","base64":exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["parts"][0]["stale"],false);
    assert_eq!(reopened["document"]["parts"][0]["spec"],created["document"]["parts"][0]["spec"]);
    let part=reopened["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().iter().find(|element|element["id"]=="guided-part-1").unwrap();
    assert_body_floor(part,1.0,24.0);
    assert!(part["children"].as_array().unwrap().iter().any(|child|child["type"]=="text"));
}

#[test]
fn guided_authoring_speaker_notes_append_once_and_preserve_evidence() {
    let mut input=authoring_brief("status-report");
    let original=execute_request(json!({"op":"create_guided_presentation","id":"notes","input":input})).unwrap();
    let speaker="\u{1f4ac}".repeat(4000);
    input["slides"][0]["speaker_notes"]=json!(speaker);
    let created=execute_request(json!({"op":"create_guided_presentation","id":"notes","input":input})).unwrap();
    let expected=format!("{}\nSpeaker notes:\n{speaker}",original["document"]["deck"]["slides"][0]["notes"].as_str().unwrap());
    assert_eq!(created["document"]["deck"]["slides"][0]["notes"],expected);
    assert_eq!(created["document"]["parts"],original["document"]["parts"]);
    let exported=execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"notes-reopened","base64":exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][0]["notes"],expected);
    assert!(expected.contains("Ledger:") && expected.contains("Evidence:") && expected.contains("source-a"));
    input["slides"][0]["speaker_notes"]=json!(format!("{speaker}x"));
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
    input["slides"][0]["speaker_notes"]=json!(speaker);
    input["evidence"]=json!((0..4).map(|index|json!({"id":format!("source-{index}"),"kind":"source","reference":"Synthetic fixture","statement":"s".repeat(1200)})).collect::<Vec<_>>());
    input["slides"][0]["support"][0]["evidence_ids"]=json!(["source-0","source-1","source-2","source-3"]);
    let checked=execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
    assert_eq!(checked["ready"],false);
    assert!(checked["issues"].to_string().contains("8000"),"{checked}");
}

#[test]
fn guided_ledger_notes_none_keeps_speaker_text_and_legacy_slide_geometry() {
    let mut input=authoring_brief("status-report");
    input["slides"][0]["speaker_notes"]=json!("Explain the evidence first.\nKeep the original caveat.");
    let legacy=execute_request(json!({"op":"create_guided_presentation","id":"notes-mode","input":input})).unwrap();
    input["authoring"]=json!({"ledger_notes":"none"});
    let created=execute_request(json!({"op":"create_guided_presentation","id":"notes-mode","input":input})).unwrap();
    assert_eq!(created["document"]["deck"]["slides"][0]["notes"],input["slides"][0]["speaker_notes"]);
    assert_eq!(created["document"]["deck"]["slides"][0]["elements"],legacy["document"]["deck"]["slides"][0]["elements"]);
    assert_eq!(created["document"]["parts"],legacy["document"]["parts"]);
}

#[test]
fn guided_ledger_notes_records_roundtrip_outside_presenter_notes() {
    for mode in ["summary","none"] {
        let mut input=brief("status-report");
        input["authoring"]=json!({"ledger_notes":mode});
        input["evidence"][0]["reference"]=json!("Approved synthetic & illustrative <fixture> \"reference\"");
        input["slides"][0]["speaker_notes"]=json!("Explain the synthetic comparison first.\nPause before the recommendation.");
        let created=execute_request(json!({"op":"create_guided_presentation","id":"notes-record","input":input})).unwrap();
        let document=&created["document"];
        let notes=document["deck"]["slides"][0]["notes"].as_str().unwrap();
        assert!(notes.starts_with(input["slides"][0]["speaker_notes"].as_str().unwrap()));
        assert!(!notes.contains("\"body_paths\"") && !notes.contains("\"statement\""),"{notes}");
        if mode=="summary" {assert!(notes.contains("Authoring summary") && notes.contains("source-a"),"{notes}");}
        let record=&document["guided_record"];
        assert_eq!(record["version"],1);
        assert_eq!(record["input"]["evidence"],input["evidence"]);
        assert_eq!(record["input"]["slides"][0]["support"],input["slides"][0]["support"]);
        assert_eq!(record["input"]["slides"][0]["numbers"],input["slides"][0]["numbers"]);
        assert!(record["input"]["slides"][0].get("speaker_notes").is_none());
        let exported=execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        let reopened=execute_request(json!({"op":"open_presentation","id":"notes-record-reopened","base64":exported["base64"]})).unwrap();
        assert_eq!(reopened["document"]["guided_record"],*record);
        assert_eq!(reopened["document"]["deck"]["slides"][0]["notes"],notes);
        assert_eq!(reopened["document"]["parts"][0]["stale"],false);
    }
}

#[test]
fn guided_ledger_notes_budgets_remain_independent_and_reject_invalid_modes() {
    let mut input=authoring_brief("status-report");
    let speaker="\u{1f4ac}".repeat(4000);
    input["slides"][0]["speaker_notes"]=json!(speaker);
    input["evidence"]=json!((0..4).map(|index|json!({"id":format!("source-{index}"),"kind":"source","reference":"Synthetic fixture","statement":"s".repeat(1200)})).collect::<Vec<_>>());
    input["slides"][0]["support"][0]["evidence_ids"]=json!(["source-0","source-1","source-2","source-3"]);
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
    for mode in ["summary","none"] {
        input["authoring"]=json!({"ledger_notes":mode});
        let created=execute_request(json!({"op":"create_guided_presentation","id":"notes-budgets","input":input})).unwrap();
        let notes=created["document"]["deck"]["slides"][0]["notes"].as_str().unwrap();
        assert!(notes.starts_with(&speaker) && notes.chars().count()<=8000);
        if mode=="none" {assert_eq!(notes,speaker);}
        assert_eq!(created["document"]["guided_record"]["input"]["evidence"],input["evidence"]);
        input["slides"][0]["speaker_notes"]=json!(format!("{speaker}x"));
        let checked=execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
        assert_eq!(checked["ready"],false);
        assert!(checked["issues"].to_string().contains("4000"),"{checked}");
        input["slides"][0]["speaker_notes"]=json!(speaker);
    }
    for mode in [json!("private"),json!(true),json!(1),json!({}),json!([])] {
        input["authoring"]["ledger_notes"]=mode;
        assert!(execute_request(json!({"op":"create_guided_presentation","id":"invalid-notes-mode","input":input})).is_err());
    }
    for mode in ["summary","none"] {
        let mut empty=authoring_brief("status-report");empty["authoring"]=json!({"ledger_notes":mode});
        let created=execute_request(json!({"op":"create_guided_presentation","id":"empty-speaker","input":empty})).unwrap();
        let notes=created["document"]["deck"]["slides"][0]["notes"].as_str().unwrap();
        if mode=="none" {assert_eq!(notes,"");} else {assert!(notes.starts_with("Authoring summary"));}
    }
    input["language"]=json!("ja");input["authoring"]=json!({"ledger_notes":"summary"});
    let created=execute_request(json!({"op":"create_guided_presentation","id":"japanese-notes","input":input})).unwrap();
    assert!(created["document"]["deck"]["slides"][0]["notes"].as_str().unwrap().contains("作成根拠の要約"));
}

#[test]
fn guided_ledger_notes_record_is_bounded_hashed_and_recoverable() {
    use aislide_core::document::{self,Document,SessionRecovery,Transaction};
    let mut input=authoring_brief("status-report");input["authoring"]=json!({"ledger_notes":"none"});
    let created=execute_request(json!({"op":"create_guided_presentation","id":"record-lifecycle","input":input})).unwrap();
    let original:Document=serde_json::from_value(created["document"].clone()).unwrap();
    let checkpoint=document::export(&original).unwrap();
    let restored=document::open(checkpoint["base64"].as_str().unwrap(),serde_json::from_value(checkpoint["checkpoint"].clone()).unwrap()).unwrap();
    assert_eq!(restored.hash,original.hash);
    let reopened=document::open_presentation("checkpoint-native".into(),base64::Engine::decode(&base64::engine::general_purpose::STANDARD,checkpoint["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(reopened["document"]["guided_record"],created["document"]["guided_record"]);
    let changed=document::transact(&original,Transaction{expected_revision:original.revision,expected_hash:original.hash.clone(),operations:serde_json::from_value(json!([
        {"op":"replace","path":"/deck/slides/0/notes","value":"Later human explanation"},
        {"op":"replace","path":"/guided_record/input/title","value":"Explicitly corrected creation record"},
    ])).unwrap()}).unwrap();
    let receipt=changed.receipt.clone().unwrap();
    assert_eq!(changed.document.guided_record.as_ref().unwrap().input.slides[0].headline,original.guided_record.as_ref().unwrap().input.slides[0].headline);
    document::verify_session_recovery(SessionRecovery{format:"aislide.session".into(),version:1,capacity_profile:original.capacity_profile,document:changed.document.clone(),past:vec![receipt.clone()],future:Vec::new(),history_boundary:None}).unwrap();
    let undone=document::undo(&changed.document,changed.document.revision,receipt).unwrap();
    assert_eq!(undone.document.hash,original.hash);
    let mut tampered=original.clone();tampered.guided_record.as_mut().unwrap().input.title="Tampered outside a transaction".into();
    assert!(document::verify(&tampered).is_err());
    for (path,value) in [
        ("/guided_record/version",json!(2)),
        ("/guided_record/input/slides/0/speaker_notes",json!("Duplicated human text")),
        ("/guided_record/input/title",json!("s".repeat(aislide_core::guided::MAX_RECORD_BYTES))),
    ] {
        let result=document::transact(&original,Transaction{expected_revision:original.revision,expected_hash:original.hash.clone(),operations:serde_json::from_value(json!([{"op":"add","path":path,"value":value}])).unwrap()});
        assert!(result.is_err(),"{path}");
        assert!(result.err().unwrap().to_string().contains("guided creation record"),"{path}");
        document::verify(&original).unwrap();
    }
}

#[test]
fn guided_record_is_inspected_and_removed_by_explicit_clean_copy() {
    for mode in ["summary","none"] {
        let mut input=brief("status-report");
        input["authoring"]=json!({"ledger_notes":mode});
        input["evidence"][0]["statement"]=json!("The fixture values are 12 and 24. Contact synthetic.reviewer@example.com.");
        let created=execute_request(json!({"op":"create_guided_presentation","id":"record-inspection","input":input})).unwrap();
        let document=&created["document"];
        assert!(!document["deck"]["slides"][0]["notes"].as_str().unwrap().contains("@example.com"));
        let found=|inspection:&serde_json::Value,scope:&str|inspection["candidates"].as_array().unwrap().iter().any(|candidate|candidate["rule"]=="email_candidate" && candidate["surface"]=="guided_record" && candidate["scope"]==scope);
        let inspection=execute_request(json!({"op":"inspect_document","document":document})).unwrap();
        assert!(found(&inspection,"current_deck"),"{inspection}");
        assert!(!inspection.to_string().contains("synthetic.reviewer"));
        let exported=execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        let reopened=execute_request(json!({"op":"open_presentation","id":"record-inspection-native","base64":exported["base64"]})).unwrap();
        let native=execute_request(json!({"op":"inspect_document","document":reopened["document"]})).unwrap();
        assert!(found(&native,"current_deck") && found(&native,"embedded_origin"),"{native}");
        for category in ["sources","notes"] {
            let clean=execute_request(json!({"op":"export_clean_copy","document":document,"options":{"new_document_id":"record-clean","categories":[category],"confirmed":true}})).unwrap();
            assert!(clean["document"].get("guided_record").is_none(),"{category}");
            let opened=execute_request(json!({"op":"open_presentation","id":"record-clean-reopened","base64":clean["base64"]})).unwrap();
            assert!(opened["document"].get("guided_record").is_none(),"{category}");
            let rescanned=execute_request(json!({"op":"inspect_document","document":opened["document"]})).unwrap();
            assert!(!found(&rescanned,"current_deck") && !found(&rescanned,"embedded_origin"),"{category}: {rescanned}");
        }
        let kept=execute_request(json!({"op":"export_clean_copy","document":document,"options":{"new_document_id":"record-kept","categories":["comments"],"confirmed":true}})).unwrap();
        assert_eq!(kept["document"]["guided_record"],document["guided_record"]);
    }
}

#[test]
fn guided_record_rejects_xml_escaping_beyond_provenance_limit_at_creation() {
    let mut input=authoring_brief("status-report");
    let id="'".repeat(40);
    input["evidence"]=json!([{"id":id,"kind":"source","reference":"Synthetic escaping fixture","statement":"Synthetic fixture for XML escaping only."}]);
    let template=input["slides"][0].clone();
    input["slides"]=json!((0..96).map(|index| {
        let mut slide=template.clone();
        slide["id"]=json!(format!("slide-{index}"));
        slide["question"]=json!("'".repeat(240));
        slide["transition"]=json!("'".repeat(80));
        slide["parallel_basis"]=json!("'".repeat(80));
        slide["support"]=json!(["Review","the","evidence","before","proceeding."].iter().map(|clause|json!({"clause":clause,"body_paths":["/data/items"],"evidence_ids":vec![id.clone();16]})).collect::<Vec<_>>());
        slide
    }).collect::<Vec<_>>());
    input["authoring"]=json!({"slide_limit":96,"ledger_notes":"summary"});
    assert!(serde_json::to_vec(&input).unwrap().len()<aislide_core::guided::MAX_RECORD_BYTES);
    let checked=execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
    assert_eq!(checked["ready"],false);
    assert!(checked["issues"].to_string().contains("provenance XML"),"{checked}");
    assert!(execute_request(json!({"op":"create_guided_presentation","id":"escaping-record","input":input})).is_err());
    input["authoring"]["ledger_notes"]=json!("full");
    let created=execute_request(json!({"op":"create_guided_presentation","id":"escaping-full","input":input})).unwrap();
    assert!(execute_request(json!({"op":"export_presentation","document":created["document"]})).is_ok());
}

#[test]
fn guided_record_edits_are_structural_and_do_not_rerun_creation_checks() {
    use aislide_core::document::{self,Document,Transaction};
    let mut input=authoring_brief("status-report");input["authoring"]=json!({"ledger_notes":"summary"});
    let created=execute_request(json!({"op":"create_guided_presentation","id":"record-structure","input":input})).unwrap();
    let original:Document=serde_json::from_value(created["document"].clone()).unwrap();
    let changed=document::transact(&original,Transaction{expected_revision:original.revision,expected_hash:original.hash.clone(),operations:serde_json::from_value(json!([
        {"op":"replace","path":"/guided_record/input/slides/0/support","value":[]},
    ])).unwrap()}).unwrap().document;
    let exported=execute_request(json!({"op":"export_presentation","document":changed})).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"record-structure-reopened","base64":exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["guided_record"]["input"]["slides"][0]["support"],json!([]));
    let checkpoint=document::export(&changed).unwrap();
    assert_eq!(document::open(checkpoint["base64"].as_str().unwrap(),serde_json::from_value(checkpoint["checkpoint"].clone()).unwrap()).unwrap().hash,changed.hash);
}

#[test]
fn guided_record_rejects_values_the_pptx_reader_cannot_reopen() {
    use aislide_core::document::{self,Document,Transaction};
    for mode in ["summary","none"] {
        let mut input=authoring_brief("status-report");input["authoring"]=json!({"ledger_notes":mode});
        input["slides"][0]["support"][0]["clause"]=json!("Review the evidence before proceeding.\u{b}");
        let checked=execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
        assert_eq!(checked["ready"],false,"{checked}");
        let error=execute_request(json!({"op":"create_guided_presentation","id":"record-control","input":input})).unwrap_err().to_string();
        assert!(error.contains("guided creation record") && error.contains("XML 1.0"),"{error}");
    }
    let mut input=authoring_brief("status-report");input["authoring"]=json!({"ledger_notes":"summary"});
    let created=execute_request(json!({"op":"create_guided_presentation","id":"record-storable","input":input})).unwrap();
    let original:Document=serde_json::from_value(created["document"].clone()).unwrap();
    let nested=|levels:usize|(0..levels).fold(json!("leaf"),|value,_|json!([value]));
    let number=|value:Value|json!({"op":"add","path":"/guided_record/input/slides/0/numbers/-","value":{"path":"/data/items/0","value":value,"evidence_id":"source-a"}});
    let edit=|operation:Value|document::transact(&original,Transaction{expected_revision:original.revision,expected_hash:original.hash.clone(),operations:serde_json::from_value(json!([operation])).unwrap()});
    for operation in [
        json!({"op":"replace","path":"/guided_record/input/title","value":"Before\u{0}After"}),
        json!({"op":"replace","path":"/guided_record/input/audience","value":"Before\u{fffe}After"}),
        number(json!({"bad\u{1}key":1})),
        number(nested(24)),
    ] {
        let error=edit(operation.clone()).err().unwrap_or_else(||panic!("{operation}")).to_string();
        assert!(error.contains("guided creation record"),"{operation}: {error}");
    }
    let deepest=edit(number(nested(23))).unwrap().document;
    let exported=execute_request(json!({"op":"export_presentation","document":deepest})).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"record-deepest","base64":exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["guided_record"],serde_json::to_value(&deepest.guided_record).unwrap());
}

#[test]
fn guided_record_preserves_carriage_returns_and_attribute_whitespace_exactly() {
    use aislide_core::document::{self,Document,Transaction};
    let mut input=authoring_brief("status-report");input["authoring"]=json!({"ledger_notes":"summary"});
    input["audience"]=json!("Before\rAfter");
    input["evidence"][0]["statement"]=json!("Line one\r\nLine two\n\rLine three\r");
    let created=execute_request(json!({"op":"create_guided_presentation","id":"record-newlines","input":input})).unwrap();
    assert_eq!(created["document"]["guided_record"]["input"]["audience"],"Before\rAfter");
    let original:Document=serde_json::from_value(created["document"].clone()).unwrap();
    let changed=document::transact(&original,Transaction{expected_revision:original.revision,expected_hash:original.hash.clone(),operations:serde_json::from_value(json!([
        {"op":"add","path":"/guided_record/input/slides/0/numbers/-","value":{"path":"/data/items/0","value":{"tab\tline\nreturn\r end":"value\r\n\t kept"},"evidence_id":"source-a"}},
    ])).unwrap()}).unwrap().document;
    let record=serde_json::to_value(&changed.guided_record).unwrap();
    let exported=execute_request(json!({"op":"export_presentation","document":changed})).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"record-newlines-reopened","base64":exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["guided_record"],record);
    let checkpoint=document::export(&changed).unwrap();
    let restored=document::open(checkpoint["base64"].as_str().unwrap(),serde_json::from_value(checkpoint["checkpoint"].clone()).unwrap()).unwrap();
    assert_eq!(restored.hash,changed.hash);
    let native=execute_request(json!({"op":"open_presentation","id":"record-newlines-checkpoint","base64":checkpoint["base64"]})).unwrap();
    assert_eq!(native["document"]["guided_record"],record);
}

#[test]
fn guided_record_removed_from_native_document_stays_removed_in_project_export() {
    use aislide_core::document::{self,Document,Transaction};
    let mut input=authoring_brief("status-report");input["authoring"]=json!({"ledger_notes":"none"});
    let created=execute_request(json!({"op":"create_guided_presentation","id":"record-removal","input":input})).unwrap();
    let exported=execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
    let opened=execute_request(json!({"op":"open_presentation","id":"record-removal-native","base64":exported["base64"]})).unwrap();
    let native:Document=serde_json::from_value(opened["document"].clone()).unwrap();
    assert!(native.guided_record.is_some() && native.origin.is_some());
    let removed=document::transact(&native,Transaction{expected_revision:native.revision,expected_hash:native.hash.clone(),operations:serde_json::from_value(json!([{"op":"remove","path":"/guided_record"}])).unwrap()}).unwrap().document;
    let presentation=document::export_presentation(&removed).unwrap();
    let project=document::export(&removed).unwrap();
    for (label,base64) in [("presentation",&presentation["base64"]),("project",&project["base64"])] {
        let reopened=execute_request(json!({"op":"open_presentation","id":"record-removal-reopened","base64":base64})).unwrap();
        assert!(reopened["document"].get("guided_record").is_none(),"{label}");
    }
    let restored=document::open(project["base64"].as_str().unwrap(),serde_json::from_value(project["checkpoint"].clone()).unwrap()).unwrap();
    assert_eq!(restored.hash,removed.hash);
    assert!(restored.guided_record.is_none());
    let unchanged=document::export(&native).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"record-removal-unchanged","base64":unchanged["base64"]})).unwrap();
    assert_eq!(reopened["document"]["guided_record"],opened["document"]["guided_record"]);
}

fn plain_pptx()->Vec<u8> {
    aislide_core::pptx::export_pptx(&serde_json::from_value(json!({"version":1,"title":"Synthetic custom XML fixture","width":1280,"height":720,"slides":[{"id":"slide-1","title":"Synthetic","background":"FFFFFF","notes":"","elements":[
        {"type":"text","id":"title","x":40,"y":40,"width":1000,"height":80,"text":"Synthetic fixture","font_size":20,"color":"333333","bold":false},
    ]}]})).unwrap()).unwrap()
}

fn with_custom_xml(bytes:Vec<u8>,path:&str,xml:Vec<u8>)->Vec<u8> {
    use aislide_core::package::Package;
    let mut parts=Package::open(bytes).unwrap().parts().clone();
    let relations="ppt/_rels/presentation.xml.rels";
    let text=String::from_utf8(parts[relations].clone()).unwrap().replace("</Relationships>",&format!("<Relationship Id=\"rIdSyntheticCustom\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml\" Target=\"/{path}\"/></Relationships>"));
    parts.insert(relations.into(),text.into_bytes());
    let types=String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap().replace("</Types>",&format!("<Override PartName=\"/{path}\" ContentType=\"application/xml\"/></Types>"));
    parts.insert("[Content_Types].xml".into(),types.into_bytes());
    parts.insert(path.into(),xml);
    Package::from_parts(parts).unwrap().save().unwrap()
}

#[test]
fn unrelated_custom_xml_stays_opaque_for_project_export_and_native_open() {
    use aislide_core::{document::{self,Document},package::Package};
    use base64::{Engine,engine::general_purpose::STANDARD};
    let plain=plain_pptx();
    let vendor=format!("<v:data xmlns:v=\"urn:synthetic:vendor\">{}</v:data>","x".repeat(2*1024*1024+64)).into_bytes();
    let bytes=with_custom_xml(plain,"customXml/vendor1.xml",vendor.clone());
    let imported:Document=serde_json::from_value(document::import_document("vendor-import".into(),bytes.clone()).unwrap()["document"].clone()).unwrap();
    assert!(imported.guided_record.is_none());
    let project=document::export(&imported).unwrap();
    let restored=document::open(project["base64"].as_str().unwrap(),serde_json::from_value(project["checkpoint"].clone()).unwrap()).unwrap();
    assert_eq!(restored.hash,imported.hash);
    assert_eq!(Package::open(STANDARD.decode(project["base64"].as_str().unwrap()).unwrap()).unwrap().part("customXml/vendor1.xml").unwrap(),vendor.as_slice());

    let mut input=authoring_brief("status-report");input["authoring"]=json!({"ledger_notes":"none"});
    let created=execute_request(json!({"op":"create_guided_presentation","id":"vendor-record","input":input})).unwrap();
    let exported=STANDARD.decode(execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap()["base64"].as_str().unwrap()).unwrap();
    let opened=document::open_presentation("vendor-native".into(),with_custom_xml(exported,"customXml/vendor1.xml",vendor.clone())).unwrap();
    assert_eq!(opened["document"]["guided_record"],created["document"]["guided_record"]);
    let native:Document=serde_json::from_value(opened["document"].clone()).unwrap();
    let project=document::export(&native).unwrap();
    assert_eq!(document::open(project["base64"].as_str().unwrap(),serde_json::from_value(project["checkpoint"].clone()).unwrap()).unwrap().hash,native.hash);
    assert_eq!(Package::open(STANDARD.decode(project["base64"].as_str().unwrap()).unwrap()).unwrap().part("customXml/vendor1.xml").unwrap(),vendor.as_slice());
}

#[test]
fn invalid_aislide_owned_custom_xml_is_still_rejected() {
    use aislide_core::document;
    let plain=plain_pptx();
    let oversized=|namespace:&str|format!("<m:provenance xmlns:m=\"{namespace}\"><m:null/>{}</m:provenance>"," ".repeat(2*1024*1024)).into_bytes();
    for (path,xml,message) in [
        ("customXml/item1.xml",oversized("urn:aislide:provenance:1"),"2 MiB"),
        ("customXml/item1.xml",oversized("urn:aislide:provenance:&#49;"),"2 MiB"),
        ("customXml/item1.xml",oversized("&#x75;rn:aislide:provenance:1"),"2 MiB"),
        ("customXml/aislide-provenance1.xml",b"not xml".to_vec(),""),
    ] {
        let bytes=with_custom_xml(plain.clone(),path,xml);
        let error=document::open_presentation("owned-invalid".into(),bytes.clone()).unwrap_err().to_string();
        assert!(error.contains(message),"{path}: {error}");
        let imported:document::Document=serde_json::from_value(document::import_document("owned-invalid-import".into(),bytes).unwrap()["document"].clone()).unwrap();
        assert!(document::export(&imported).is_err(),"{path}");
    }
}

#[test]
fn aislide_namespace_written_with_character_references_is_still_restored() {
    use aislide_core::{document,package::Package};
    use base64::{Engine,engine::general_purpose::STANDARD};
    let mut input=authoring_brief("status-report");input["authoring"]=json!({"ledger_notes":"none"});
    let created=execute_request(json!({"op":"create_guided_presentation","id":"namespace-spelling","input":input})).unwrap();
    let bytes=STANDARD.decode(execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap()["base64"].as_str().unwrap()).unwrap();
    let expected=document::open_presentation("namespace-plain".into(),bytes.clone()).unwrap()["document"].clone();
    assert!(expected["guided_record"].is_object() && !expected["parts"].as_array().unwrap().is_empty());
    let mut parts=Package::open(bytes).unwrap().parts().clone();
    let path=parts.keys().find(|path|path.starts_with("customXml/aislide-provenance")).unwrap().clone();
    let original=String::from_utf8(parts[&path].clone()).unwrap();
    let declaration="xmlns:m=\"urn:aislide:provenance:1\"";
    assert!(original.contains(declaration),"{original:.200}");
    for spelling in ["urn:aislide:provenance:&#49;","urn:aislide:provenance:&#x31;","&#x75;rn&#58;aislide:provenance:1"] {
        parts.insert(path.clone(),original.replacen(declaration,&format!("xmlns:m=\"{spelling}\""),1).into_bytes());
        let opened=document::open_presentation("namespace-reference".into(),Package::from_parts(parts.clone()).unwrap().save().unwrap()).unwrap()["document"].clone();
        for field in ["guided_record","parts","references"] {assert_eq!(opened[field],expected[field],"{spelling}: {field}");}
        let ids=|document:&Value|document["deck"]["slides"].as_array().unwrap().iter().map(|slide|slide["id"].clone()).collect::<Vec<_>>();
        assert_eq!(ids(&opened),ids(&expected),"{spelling}");
    }
}

#[test]
fn guided_authoring_invalid_settings_reject_without_fallback() {
    for settings in [json!({"context":"screen"}),json!({"density":"dense"}),json!({"spacing":"wide"}),json!({"extra":true}),json!({"body_font_min":11.99}),json!({"body_font_min":40.01}),json!({"headline_font_size":27.99}),json!({"headline_font_size":64.01}),json!({"font_family":""}),json!({"font_family":" "}),json!({"font_family":"a".repeat(101)}),json!({"font_family":"bad\nfont"}),json!({"body_font_min":"24"})] {
        let mut input=authoring_brief("status-report");input["authoring"]=settings.clone();
        let checked=execute_request(json!({"op":"validate_guided_presentation","input":input}));
        assert!(checked.as_ref().map_or(true,|value|value["ready"]==false),"{settings}: {checked:?}");
        assert!(execute_request(json!({"op":"create_guided_presentation","id":"invalid-settings","input":input})).is_err(),"{settings}");
    }
}

#[test]
fn guided_authoring_table_cells_keep_effective_floor_and_native_editability() {
    let mut input=authoring_brief("status-report");
    input["authoring"]=json!({"context":"projection","body_font_min":28,"font_family":"Arial","spacing":"relaxed"});
    input["slides"][0]["part"]=json!({"version":1,"preset":"matrix/labeled","title":"Review","subtitle":"Decisions","data":{"kind":"matrix","rows":["Now","Next"],"columns":["A","B"],"cells":[["Yes","No"],["No","Yes"]]}});
    input["slides"][0]["support"][0]["body_paths"]=json!(["/data/cells"]);
    let result=execute_request(json!({"op":"create_guided_presentation","id":"table-authoring","input":input})).unwrap();
    let part=&result["document"]["deck"]["slides"][0]["elements"][4];
    assert_body_floor(part,1.0,28.0);
    let table=part["children"].as_array().unwrap().iter().find(|child|child["type"]=="table").unwrap();
    let cells=table["format"]["cells"].as_array().unwrap();assert_eq!(cells.len(),9);
    for cell in cells {assert_eq!(cell["style"]["text_style"]["font_family"],"Arial");assert_eq!(cell["style"]["text_format"]["paragraphs"][0]["line_spacing"]["value"],130000);}
    let exported=execute_request(json!({"op":"export_presentation","document":result["document"]})).unwrap();
    let reopened=execute_request(json!({"op":"open_presentation","id":"table-authoring-reopened","base64":exported["base64"]})).unwrap();
    assert_eq!(reopened["document"]["parts"][0]["stale"],false);
    let part=&reopened["document"]["deck"]["slides"][0]["elements"][4];
    assert_body_floor(part,1.0,28.0);
    assert!(part["children"].as_array().unwrap().iter().any(|child|child["type"]=="table"));
}

#[test]
fn guided_authoring_rejects_rich_table_overflow_without_shrinking() {
    let mut input=authoring_brief("status-report");
    input["authoring"]=json!({"body_font_min":28,"spacing":"relaxed","font_family":"Arial"});
    input["slides"][0]["part"]=json!({"version":1,"preset":"matrix/labeled","title":"Review","subtitle":"Decisions","data":{"kind":"matrix","rows":["Now","Next"],"columns":["A","B"],"cells":[["A\nB\nC","No"],["No","Yes"]]}});
    input["slides"][0]["support"][0]["body_paths"]=json!(["/data/cells"]);
    let checked=execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
    assert_eq!(checked["ready"],false,"relaxed cell typography must be measured: {checked}");
    assert!(checked["issues"].to_string().contains("TEXT_OVERFLOW"),"{checked}");
    assert!(execute_request(json!({"op":"create_guided_presentation","id":"overflow","input":input})).is_err());
}

#[test]
fn guided_authoring_schema_exposes_only_the_locked_settings() {
    let guide=execute_request(json!({"op":"best_practice_guide","profile_id":"status-report"})).unwrap();
    let schema=&guide["input_schema"];
    let definitions=schema.get("$defs").or_else(||schema.get("definitions")).unwrap();
    let settings=&definitions["Authoring"];
    assert_eq!(settings["additionalProperties"],false);
    assert_eq!(settings["properties"].as_object().unwrap().len(),9);
    assert_eq!(settings["properties"]["slide_limit"]["minimum"],32);
    assert_eq!(settings["properties"]["slide_limit"]["maximum"],128);
    assert_eq!(definitions["HeadlineStyle"]["enum"],json!(["sentence","keyword"]));
    assert_eq!(definitions["LedgerNotes"]["enum"],json!(["full","summary","none"]));
    assert_eq!(guide["limits"]["slides"],32);
    assert_eq!(guide["limits"]["maximum_slides"],128);
    assert_eq!(guide["limits"]["speaker_notes_scalars"],4000);
    assert_eq!(guide["limits"]["slide_notes_scalars"],8000);
    assert_eq!(guide["limits"]["guided_record_bytes"],1048576);
    assert!(guide["markdown"].as_str().unwrap().contains("not privacy redaction"));
    assert_eq!(settings["properties"]["body_font_min"]["minimum"],12);
    assert_eq!(settings["properties"]["body_font_min"]["maximum"],40);
    assert_eq!(settings["properties"]["headline_font_size"]["minimum"],28);
    assert_eq!(settings["properties"]["headline_font_size"]["maximum"],64);
    assert_eq!(settings["properties"]["font_family"]["maxLength"],100);
    assert_eq!(definitions["GuidedSlide"]["properties"]["speaker_notes"]["maxLength"],4000);
}

#[test]
fn best_practice_profiles_expose_english_guidance_and_honest_pattern_capabilities() {
    let catalog = execute_request(json!({"op":"best_practice_profiles"})).unwrap();
    assert_eq!(catalog["profiles"].as_array().unwrap().len(), 4);
    for id in ["consulting-decision", "technical-explainer", "event-talk", "status-report"] {
        let guide = execute_request(json!({"op":"best_practice_guide","profile_id":id})).unwrap();
        assert_eq!(guide["language"], "en");
        assert!(guide["markdown"].as_str().unwrap().contains("Claim"));
        assert!(guide["markdown"].as_str().unwrap().contains("unknown"));
        assert!(guide["input_schema"].is_object());
        assert!(guide["semantic_truth_verified"] == false);
        if id == "consulting-decision" {
            assert_eq!(guide["patterns"].as_array().unwrap().len(), 48);
            assert!(guide["patterns"].as_array().unwrap().iter().any(|pattern|pattern["id"]=="C03" && pattern["capability"]=="native-template"));
            assert!(guide["patterns"].as_array().unwrap().iter().any(|pattern|pattern["capability"]=="guidance-only"));
            assert_eq!(guide["automatic_patterns"],json!(["native-part","C02","C03"]));
        } else {
            assert_eq!(guide["automatic_patterns"],json!(["native-part"]));
        }
    }
    assert!(execute_request(json!({"op":"best_practice_guide","profile_id":"../../secret"})).is_err());
}

#[test]
fn guided_creation_preserves_values_and_requires_numeric_and_clause_evidence() {
    for profile in ["consulting-decision", "technical-explainer", "event-talk", "status-report"] {
        let input = brief(profile);
        let checked = execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
        assert_eq!(checked["ready"], true, "{checked}");
        assert_eq!(checked["semantic_truth_verified"], false);
        let created = execute_request(json!({"op":"create_guided_presentation","id":"guided","input":input})).unwrap();
        assert_eq!(created["document"]["parts"].as_array().unwrap().len(), 1);
        assert_eq!(created["document"]["parts"][0]["spec"]["data"]["series"][0]["values"], json!([12.0,24.0]));
        assert!(created["document"]["deck"]["slides"][0]["notes"].as_str().unwrap().contains("source-a"));
        let exported = execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
        let opened = execute_request(json!({"op":"open_presentation","id":"reopened","base64":exported["base64"]})).unwrap();
        assert_eq!(opened["document"]["parts"][0]["stale"], false);
    }
    for case in ["missing-number", "wrong-number", "missing-support", "title-as-evidence", "empty-evidence", "unknown-reference", "unsupported-pattern", "invalid-geometry"] {
        let mut input = brief("status-report");
        match case {
            "missing-number" => input["slides"][0]["numbers"] = json!([]),
            "wrong-number" => input["slides"][0]["numbers"][0]["value"] = json!(999),
            "missing-support" => input["slides"][0]["support"] = json!([]),
            "title-as-evidence" => input["slides"][0]["support"][0]["body_paths"] = json!(["/title"]),
            "empty-evidence" => { input["slides"][0]["support"][0]["body_paths"] = json!(["/data/series"]); input["slides"][0]["part"]["data"]["series"] = json!([]); },
            "unknown-reference" => input["slides"][0]["numbers"][0]["evidence_id"] = json!("missing"),
            "unsupported-pattern" => input["slides"][0]["pattern_id"] = json!("unimplemented-special-chart"),
            _ => input["slides"][0]["part"]["data"]["series"][0]["values"] = json!([1]),
        }
        let checked = execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap();
        assert_eq!(checked["ready"], false, "{case}: {checked}");
        assert!(execute_request(json!({"op":"create_guided_presentation","id":"rejected","input":input})).is_err(), "{case}");
    }
}

#[test]
fn decision_templates_recover_the_same_issues_with_timing_and_criteria() {
    for count in [3,6] {
        let mut input=brief("consulting-decision");
        input["issues"]=json!((1..=count).map(|index|json!({"id":format!("Q{index}"),"question":"Scope approval","requested_decision":"Approve the scoped pilot","criterion":"Evidence reviewed","owner":"Review owner","due":"After approval","evidence_ids":["source-a"],"analysis_slide_ids":["evidence-slide"]})).collect::<Vec<_>>());
        let template=|id:&str,pattern:&str,headline:&str,form:&str|json!({"id":id,"section":"Decision","headline":headline,"sentence_form":form,"pattern_id":pattern,"question":"What should be approved?","parent_message":"governing","transition":"therefore","parallel_basis":"decision issue","support":[{"clause":headline,"body_paths":["/issues"],"evidence_ids":["source-a"]}],"numbers":[]});
        input["slides"]=json!([template("summary","C02","We should approve the scoped pilot before committing to rollout.","proposal"),input["slides"][0].clone(),template("closing","C03","We can proceed if owners confirm the evidence and acceptance criteria.","conditional")]);
        let created=execute_request(json!({"op":"create_guided_presentation","id":"decisions","input":input})).unwrap();
        let slides=created["document"]["deck"]["slides"].as_array().unwrap();
        for index in [0,2] {
            let table=slides[index]["elements"].as_array().unwrap().iter().find(|element|element["id"]=="decision-table").unwrap();
            assert_eq!(table["type"],"group","Decision templates need purpose-sized columns, not equal-width cells");
            let children=table["children"].as_array().unwrap();
            let heading=children.iter().find(|element|element["text"]=="Issue").unwrap();
            assert!(heading["height"].as_f64().unwrap()<=40.0);
            assert_eq!(children.iter().filter(|element|element["type"]=="text").count(),(count+1)*4);
            for issue in 1..=count {assert!(children.iter().any(|element|element["text"].as_str().is_some_and(|text|text.starts_with(&format!("Q{issue}:")))));}
            if index==0 {
                let evidence=children.iter().find(|element|element["text"]=="Evidence").unwrap();
                let pages=children.iter().find(|element|element["text"]=="Analysis pages").unwrap();
                assert!(evidence["width"].as_f64().unwrap()>pages["width"].as_f64().unwrap()*2.0);
            }
        }
        assert!(slides[0]["elements"].to_string().contains("The fixture values are 12 and 24."), "The summary must show evidence, not only an opaque reference id");
        assert!(slides[2].to_string().contains("Immediate execution schedule"));
        assert!(slides[2].to_string().contains("Evidence reviewed"));
        let exported=execute_request(json!({"op":"export_presentation","document":created["document"]})).unwrap();
        let reopened=execute_request(json!({"op":"open_presentation","id":"decisions-reopened","base64":exported["base64"]})).unwrap();
        assert_eq!(reopened["document"]["deck"]["slides"].as_array().unwrap().len(),3);
        for target in ["missing", "summary", "closing"] {
            let mut invalid=input.clone();invalid["issues"][0]["analysis_slide_ids"]=json!([target]);
            assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":invalid})).unwrap()["ready"],false,"{target} is not analysis evidence");
        }
    }
}

#[test]
fn unknown_quantities_remain_text_and_cannot_justify_a_numeric_chart() {
    let mut input=brief("status-report");
    input["evidence"][0]["kind"]=json!("unknown");
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
    input["slides"][0]["part"]=json!({"version":1,"preset":"list-horizontal/balanced","title":"Outstanding measurements","subtitle":"Values are not yet supplied","data":{"kind":"items","items":[{"label":"Volume","detail":"xx"},{"label":"Timing","detail":"xx"}]}});
    input["slides"][0]["numbers"]=json!([]);
    input["slides"][0]["support"][0]["body_paths"]=json!(["/data/items"]);
    let result=execute_request(json!({"op":"create_guided_presentation","id":"unknown-values","input":input})).unwrap();
    assert!(result["document"]["deck"]["slides"][0].to_string().contains("xx"));
    assert_eq!(result["document"]["parts"][0]["spec"]["data"]["items"][0]["value"],Value::Null);
    input["slides"][0]["support"][0]["clause"]=json!("Only half of the headline");
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
}

#[test]
fn guided_authoring_decision_bodies_reserve_citations_with_large_headlines() {
    let mut input=authoring_brief("consulting-decision");
    input["authoring"]=json!({"headline_font_size":48,"body_font_min":12});
    input["issues"]=json!((1..=3).map(|index|json!({"id":format!("Q{index}"),"question":"Scope","requested_decision":"Approve pilot","criterion":"Review evidence","owner":"Owner","due":"After approval","evidence_ids":["source-a"],"analysis_slide_ids":["evidence-slide"]})).collect::<Vec<_>>());
    let template=|id:&str,pattern:&str,headline:&str,form:&str|json!({"id":id,"section":"Decision","headline":headline,"sentence_form":form,"pattern_id":pattern,"question":"What should be approved?","parent_message":"governing","transition":"therefore","parallel_basis":"decision issue","support":[{"clause":headline,"body_paths":["/issues"],"evidence_ids":["source-a"]}],"numbers":[]});
    input["slides"]=json!([template("summary","C02","Approve the pilot.","proposal"),input["slides"][0].clone(),template("closing","C03","Confirm the conditions.","conditional")]);
    let result=execute_request(json!({"op":"create_guided_presentation","id":"large-decisions","input":input})).unwrap();
    for index in [0,2] {for element in result["document"]["deck"]["slides"][index]["elements"].as_array().unwrap().iter().skip(4) {
        assert!(element["y"].as_f64().unwrap()+element["height"].as_f64().unwrap()<=642.01,"body overlaps citation region: {element}");
    }}
}

#[test]
fn guided_headline_numbers_are_bounded_and_small_labels_keep_the_font_floor() {
    let mut input=brief("status-report");
    let headline="The measured baseline needs another review because current evidence does not establish the expected operational benefit.";
    input["slides"][0]["headline"]=json!(headline);input["slides"][0]["support"][0]["clause"]=json!(headline);
    input["slides"][0]["part"]["preset"]=json!("cycle/labeled");
    input["slides"][0]["part"]["data"]=json!({"kind":"items","center":"Review","items":[{"label":"Measure","detail":"Check the result"},{"label":"Assess","detail":"Confirm the context"},{"label":"Change","detail":"Agree the next step"}]});
    input["slides"][0]["support"][0]["body_paths"]=json!(["/data/items"]);input["slides"][0]["numbers"]=json!([]);
    let created=execute_request(json!({"op":"create_guided_presentation","id":"font-floor","input":input})).unwrap();
    fn check(elements:&[Value]) {for element in elements {if let Some(size)=element["font_size"].as_f64() {assert!(size>=12.0,"{element}");}if let Some(children)=element["children"].as_array(){check(children);}}}
    check(created["document"]["deck"]["slides"][0]["elements"].as_array().unwrap());
    input["profile_id"]=json!("consulting-decision");
    input["slides"][0]["headline"]=json!("We should address 12 delays across 4 teams within 30 days.");
    input["slides"][0]["support"][0]["clause"]=input["slides"][0]["headline"].clone();
    assert_eq!(execute_request(json!({"op":"validate_guided_presentation","input":input})).unwrap()["ready"],false);
}