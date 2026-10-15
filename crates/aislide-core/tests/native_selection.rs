use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};

const SLIDE: &str = "ppt/slides/slide1.xml";
const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";

fn rect(id: &str, x: f64) -> Value {
    json!({"type":"rect","id":id,"x":x,"y":40,"width":100,"height":80,"fill":"087F73"})
}

fn fixture(elements: Value) -> Package {
    let deck = json!({"version":1,"title":"Synthetic native selection","width":1280,"height":720,
        "slides":[{"id":"slide","title":"Synthetic","background":"FFFFFF","elements":elements,"notes":""}]});
    let exported = execute_request(json!({"op":"export","deck":deck})).unwrap();
    Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap()
}

fn open(bytes: &[u8]) -> Value {
    execute_request(json!({"op":"open_presentation","id":"native-selection","base64":STANDARD.encode(bytes)})).unwrap()["document"].clone()
}

fn save(document: &Value) -> Vec<u8> {
    let exported = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()
}

fn select(document: &Value, operation: Value) -> Value {
    execute_request(json!({"op":"edit_selection","document":document,"expected_revision":document["revision"],
        "slide_id":document["deck"]["slides"][0]["id"],"operation":operation})).unwrap()
}

#[test]
fn native_group_and_ungroup_preserve_original_fragments_and_byte_undo() {
    let mut package = fixture(json!([rect("a", 20.0), rect("b", 160.0), rect("front", 300.0)]));
    let sentinel = "<p:extLst><p:ext uri=\"urn:selection\"><vendor:data xmlns:vendor=\"urn:vendor\" keep=\"exact\"/></p:ext></p:extLst>";
    let xml = package.text(SLIDE).unwrap().replacen("</p:sp>", &format!("{sentinel}</p:sp>"), 1);
    package.replace_part(SLIDE, xml.into_bytes()).unwrap();
    let original = package.save().unwrap();
    let document = open(&original);
    let ids: Vec<_> = document["deck"]["slides"][0]["elements"].as_array().unwrap().iter().take(2).map(|element| element["id"].clone()).collect();
    let changed = select(&document, json!({"op":"group","ids":ids,"group_id":"combined"}));
    let output = save(&changed["transaction"]["document"]);
    let grouped = Package::open(output.clone()).unwrap();
    assert!(grouped.text(SLIDE).unwrap().contains(sentinel));
    let parsed = roxmltree::Document::parse(grouped.text(SLIDE).unwrap()).unwrap();
    let group = parsed.descendants().find(|node| node.has_tag_name((P,"grpSp"))).unwrap();
    assert_eq!(group.children().filter(|node| node.has_tag_name((P,"sp"))).count(), 2);
    let reopened = open(&output);
    let ungrouped = select(&reopened, json!({"op":"ungroup","ids":["combined"]}));
    let flat = Package::open(save(&ungrouped["transaction"]["document"])).unwrap();
    assert!(flat.text(SLIDE).unwrap().contains(sentinel));
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["transaction"]["document"],
        "expected_revision":changed["transaction"]["document"]["revision"],"receipt":changed["transaction"]["receipt"]})).unwrap();
    assert_eq!(save(&undone["document"]), original);
}

#[test]
fn native_preservation_rejects_ungroup_with_retained_animation_target() {
    let group = json!({"type":"group","id":"animated-group","x":0,"y":0,"width":400,"height":200,"view_width":400,"view_height":200,"children":[rect("child",20.0)]});
    let mut package = fixture(json!([group]));
    let xml = package.text(SLIDE).unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let numeric = parsed.descendants().find(|node| node.has_tag_name((P, "cNvPr")) && node.attribute("name") == Some("animated-group")).unwrap().attribute("id").unwrap();
    let timing = format!("<p:timing><p:tnLst><p:par><p:cTn id=\"1\"><p:childTnLst><p:anim><p:cBhvr><p:cTn id=\"2\"/><p:tgtEl><p:spTgt spid=\"{numeric}\"/></p:tgtEl></p:cBhvr></p:anim></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>");
    package.replace_part(SLIDE, xml.replace("</p:sld>", &format!("{timing}</p:sld>")).into_bytes()).unwrap();
    let original = package.save().unwrap();
    let document = open(&original);
    let error = execute_request(json!({"op":"edit_selection","document":document,"expected_revision":0,"slide_id":document["deck"]["slides"][0]["id"],"operation":{"op":"ungroup","ids":["animated-group"]}})).unwrap_err().to_string();
    assert!(error.contains("retained shape reference"), "{error}");
    assert_eq!(save(&document), original);
}

fn text(id: &str) -> Value {
    json!({"type":"text","id":id,"x":10,"y":10,"width":120,"height":60,"text":"Mixed text",
        "font_size":20,"color":"222222","bold":false})
}

#[test]
fn native_scaled_ungroup_rebases_nonzero_child_origin_and_inherited_prefixes() {
    let inner = json!({"type":"group","id":"inner","x":10,"y":10,"width":140,"height":80,
        "view_width":140,"view_height":80,"children":[text("label")]});
    let outer = json!({"type":"group","id":"outer","x":100,"y":100,"width":400,"height":240,
        "view_width":200,"view_height":120,"children":[inner]});
    let mut package = fixture(json!([outer, rect("front", 700.0)]));
    let mut xml = package.text(SLIDE).unwrap().to_owned();
    xml = xml.replacen("<p:grpSp>", "<p:grpSp xmlns:vendor=\"urn:inherited\">", 1);
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let offset = parsed.descendants().find(|node| node.tag_name().name() == "chOff" && node.ancestors().any(|node| node.has_tag_name((P,"grpSp")))).unwrap().range();
    xml.replace_range(offset, "<a:chOff x=\"95250\" y=\"47625\"/>");
    xml = xml.replacen("</p:sp>", "<p:extLst><p:ext uri=\"urn:selection\"><vendor:kept/></p:ext></p:extLst></p:sp>", 1);
    package.replace_part(SLIDE, xml.into_bytes()).unwrap();
    let original = package.save().unwrap();
    let document = open(&original);
    assert_eq!(document["deck"]["slides"][0]["elements"][0]["id"], "outer");
    let first = select(&document, json!({"op":"ungroup","ids":["outer"]}));
    let reopened = open(&save(&first["transaction"]["document"]));
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["x"], 100.0);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["y"], 110.0);
    let second = select(&reopened, json!({"op":"ungroup","ids":["inner"]}));
    let output = save(&second["transaction"]["document"]);
    let flat = open(&output);
    let label = &flat["deck"]["slides"][0]["elements"][0];
    assert_eq!(label["x"], 120.0); assert_eq!(label["y"], 130.0);
    assert_eq!(label["font_size"], 40.0);
    let result = Package::open(output).unwrap();
    let parsed = roxmltree::Document::parse(result.text(SLIDE).unwrap()).unwrap();
    assert!(parsed.descendants().any(|node| node.has_tag_name(("urn:inherited", "kept"))));
}

#[test]
fn native_clipboard_across_documents_and_after_cut_uses_representable_content() {
    let source = open(&fixture(json!([rect("a", 20.0), rect("b", 160.0)])).save().unwrap());
    let copied = select(&source, json!({"op":"copy","ids":["a"],"format":"keep_source_formatting"}));
    let destination = open(&fixture(json!([rect("a", 400.0)])).save().unwrap());
    let pasted = execute_request(json!({"op":"edit_selection","document":destination,"expected_revision":0,
        "slide_id":destination["deck"]["slides"][0]["id"],"operation":{"op":"paste","id_prefix":"copy","dx":0,"dy":100},"clipboard":copied["clipboard"]})).unwrap();
    let reopened = open(&save(&pasted["transaction"]["document"]));
    assert_eq!(reopened["deck"]["slides"][0]["elements"].as_array().unwrap().len(), 2);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][1]["x"], 20.0);
    assert_eq!(pasted["effects"]["raw_native_preserved"], false);
    let cut = select(&source, json!({"op":"cut","ids":["a"],"format":"keep_source_formatting"}));
    let pasted = execute_request(json!({"op":"edit_selection","document":cut["transaction"]["document"],"expected_revision":1,
        "slide_id":source["deck"]["slides"][0]["id"],"operation":{"op":"paste","id_prefix":"cut","dx":0,"dy":100},"clipboard":cut["clipboard"]})).unwrap();
    assert_eq!(open(&save(&pasted["transaction"]["document"]))["deck"]["slides"][0]["elements"].as_array().unwrap().len(), 2);
}

#[test]
fn native_grouping_rejects_opaque_gaps_and_ungroup_rejects_unmapped_children() {
    let mut package = fixture(json!([rect("a", 20.0), rect("b", 160.0), rect("front", 300.0)]));
    let opaque = "<vendor:opaque xmlns:vendor=\"urn:opaque\"/>";
    let xml = package.text(SLIDE).unwrap().replacen("</p:sp>", &format!("</p:sp>{opaque}"), 1);
    package.replace_part(SLIDE, xml.into_bytes()).unwrap();
    let original = package.save().unwrap(); let document = open(&original);
    assert!(execute_request(json!({"op":"edit_selection","document":document,"expected_revision":0,"slide_id":document["deck"]["slides"][0]["id"],
        "operation":{"op":"group","ids":["a","b"],"group_id":"combined"}})).is_err());
    assert_eq!(save(&document), original);
    let group = json!({"type":"group","id":"group","x":0,"y":0,"width":400,"height":200,"view_width":400,"view_height":200,"children":[rect("a",20.0)]});
    let mut package = fixture(json!([group]));
    let xml = package.text(SLIDE).unwrap().replacen("</p:grpSp>", &format!("{opaque}</p:grpSp>"), 1);
    package.replace_part(SLIDE, xml.into_bytes()).unwrap();
    let original = package.save().unwrap(); let document = open(&original);
    assert!(execute_request(json!({"op":"edit_selection","document":document,"expected_revision":0,"slide_id":document["deck"]["slides"][0]["id"],
        "operation":{"op":"ungroup","ids":["group"]}})).is_err());
    assert_eq!(save(&document), original);
}

#[test]
fn native_selected_chart_copy_has_independent_data_and_rejects_custom_resources() {
    let mut chart = execute_request(json!({"op":"create_object","id":"chart","kind":"chart","preset":"column"})).unwrap();
    chart["x"] = json!(100); chart["y"] = json!(100);
    let source = open(&fixture(json!([chart])).save().unwrap());
    let copied = select(&source, json!({"op":"copy","ids":["chart"],"format":"keep_source_formatting"}));
    let pasted = execute_request(json!({"op":"edit_selection","document":source,"expected_revision":0,"slide_id":source["deck"]["slides"][0]["id"],
        "operation":{"op":"paste","id_prefix":"copy","dx":0,"dy":0},"clipboard":copied["clipboard"]})).unwrap();
    let saved = save(&pasted["transaction"]["document"]);
    let package = Package::open(saved.clone()).unwrap();
    assert_eq!(package.parts().keys().filter(|path| path.starts_with("ppt/charts/chart") && path.ends_with(".xml")).count(), 2);
    assert_eq!(package.parts().keys().filter(|path| path.starts_with("ppt/embeddings/")).count(), 2);
    let reopened = open(&saved);
    let copy_id = reopened["deck"]["slides"][0]["elements"][1]["id"].clone();
    select(&reopened, json!({"op":"copy","ids":[copy_id],"format":"keep_source_formatting"}));
    let original_value = reopened["deck"]["slides"][0]["elements"][0]["series"][0]["values"][0].clone();
    let changed = execute_request(json!({"op":"transaction","document":reopened,"transaction":{"expected_revision":0,"expected_hash":reopened["hash"],
        "operations":[{"op":"replace","path":"/deck/slides/0/elements/1/series/0/values/0","value":99}]}})).unwrap();
    let result = open(&save(&changed["document"]));
    assert_eq!(result["deck"]["slides"][0]["elements"][0]["series"][0]["values"][0], original_value);
    assert_eq!(result["deck"]["slides"][0]["elements"][1]["series"][0]["values"][0], 99.0);
    let mut custom = fixture(json!([chart]));
    let xml = custom.text("ppt/charts/chart1.xml").unwrap().replace("</c:chartSpace>", "<c:extLst><c:ext uri=\"urn:custom-chart\"/></c:extLst></c:chartSpace>");
    custom.replace_part("ppt/charts/chart1.xml", xml.into_bytes()).unwrap();
    let document = open(&custom.save().unwrap());
    assert!(execute_request(json!({"op":"edit_selection","document":document,"expected_revision":0,"slide_id":document["deck"]["slides"][0]["id"],
        "operation":{"op":"copy","ids":["chart"],"format":"keep_source_formatting"}})).is_err());
}

fn mixed_runs(package: &mut Package) {
    let mut xml = package.text(SLIDE).unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let paragraph = parsed.descendants().find(|node| node.tag_name().name() == "p" && node.tag_name().namespace() != Some(P)).unwrap().range();
    xml.replace_range(paragraph, "<a:p><a:pPr marL=\"12700\"/><a:r><a:rPr sz=\"1500\" b=\"1\"/><a:t>Mixed</a:t></a:r><a:r><a:rPr sz=\"1800\" i=\"1\"/><a:t> text</a:t></a:r><a:endParaRPr lang=\"en-US\"/></a:p>");
    package.replace_part(SLIDE, xml.into_bytes()).unwrap();
}

#[test]
fn native_scaled_rich_ungroup_preserves_runs_and_unknown_style_attributes() {
    let group = json!({"type":"group","id":"group","x":100,"y":100,"width":400,"height":200,
        "view_width":200,"view_height":100,"children":[text("label")]});
    let mut package = fixture(json!([group])); mixed_runs(&mut package);
    let original = package.save().unwrap(); let document = open(&original);
    let changed = select(&document, json!({"op":"ungroup","ids":["group"]}));
    let output = save(&changed["transaction"]["document"]);
    let package = Package::open(output.clone()).unwrap();
    let xml = package.text(SLIDE).unwrap();
    assert!(xml.contains("sz=\"3000\" b=\"1\""));
    assert!(xml.contains("sz=\"3600\" i=\"1\""));
    assert!(xml.contains("lang=\"en-US\""));
    assert!(xml.contains("marL=\"25400\""));
    let reopened = open(&output);
    assert_eq!(reopened["deck"]["slides"][0]["elements"][0]["format"]["paragraphs"][0]["runs"][1]["style"]["font_size"], 48.0);
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["transaction"]["document"],"expected_revision":1,"receipt":changed["transaction"]["receipt"]})).unwrap();
    assert_eq!(save(&undone["document"]), original);
}

#[test]
fn native_group_keeps_picture_chart_rich_text_resources_and_front_interval() {
    let mut buffer = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(2, 2).write_to(&mut buffer, image::ImageFormat::Png).unwrap();
    let picture = json!({"type":"picture","id":"picture","x":220,"y":10,"width":80,"height":80,
        "base64":STANDARD.encode(buffer.into_inner()),"mime_type":"image/png","alt":"Synthetic raster","crop":{"left":0,"top":0,"right":0,"bottom":0}});
    let mut chart = execute_request(json!({"op":"create_object","id":"chart","kind":"chart","preset":"column"})).unwrap();
    chart["x"] = json!(330); chart["y"] = json!(100); chart["width"] = json!(400); chart["height"] = json!(260);
    let mut package = fixture(json!([text("label"), picture, chart, rect("front", 900.0)]));
    mixed_runs(&mut package);
    let marker = "<vendor:opaque xmlns:vendor=\"urn:opaque\"/>";
    let xml = package.text(SLIDE).unwrap().replacen("</p:graphicFrame>", &format!("</p:graphicFrame>{marker}"), 1);
    package.replace_part(SLIDE, xml.into_bytes()).unwrap();
    let original = package.save().unwrap(); let document = open(&original);
    let changed = select(&document, json!({"op":"group","ids":["label","picture","chart"],"group_id":"combined"}));
    let output = save(&changed["transaction"]["document"]);
    let grouped = Package::open(output.clone()).unwrap();
    for (path, bytes) in package.parts().iter().filter(|(path, _)| !path.starts_with("customXml/") && path.as_str() != SLIDE) {
        assert_eq!(grouped.part(path).unwrap(), bytes, "resource changed: {path}");
    }
    let xml = grouped.text(SLIDE).unwrap();
    assert!(xml.find("</p:grpSp>").unwrap() < xml.find(marker).unwrap());
    assert!(xml.find(marker).unwrap() < xml.find("name=\"front\"").unwrap());
    assert!(xml.contains("sz=\"1500\" b=\"1\""));
    let reopened = open(&output);
    let flat = select(&reopened, json!({"op":"ungroup","ids":["combined"]}));
    let flat_bytes = save(&flat["transaction"]["document"]);
    let flat_package = Package::open(flat_bytes.clone()).unwrap();
    let parsed = roxmltree::Document::parse(flat_package.text(SLIDE).unwrap()).unwrap();
    let numbers: Vec<_> = parsed.descendants().filter(|node| node.has_tag_name((P,"cNvPr"))).map(|node| node.attribute("id").unwrap()).collect();
    assert_eq!(numbers.len(), numbers.iter().collect::<std::collections::BTreeSet<_>>().len());
    assert_eq!(open(&flat_bytes)["deck"]["slides"][0]["elements"].as_array().unwrap().len(), 4);
}

#[test]
fn native_group_rejects_hidden_connector_references_crossing_new_container() {
    let mut package = fixture(json!([rect("a",20.0), rect("b",160.0), rect("unknown",300.0)]));
    let mut xml = package.text(SLIDE).unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let shape = parsed.descendants().find(|node| node.has_tag_name((P,"sp")) && node.descendants().any(|node| node.attribute("name") == Some("unknown"))).unwrap().range();
    xml.replace_range(shape, "<p:cxnSp><p:nvCxnSpPr><p:cNvPr id=\"4\" name=\"unknown\"/><p:cNvCxnSpPr><a:stCxn id=\"2\" idx=\"0\"/></p:cNvCxnSpPr><p:nvPr/></p:nvCxnSpPr><p:spPr/></p:cxnSp>");
    package.replace_part(SLIDE, xml.into_bytes()).unwrap();
    let original = package.save().unwrap(); let document = open(&original);
    assert_eq!(document["deck"]["slides"][0]["elements"].as_array().unwrap().len(),2);
    let result = execute_request(json!({"op":"edit_selection","document":document,"expected_revision":0,"slide_id":document["deck"]["slides"][0]["id"],
        "operation":{"op":"group","ids":["a","b"],"group_id":"combined"}}));
    assert!(result.is_err(), "unmodeled connector must not be split from its target");
    let cut = execute_request(json!({"op":"edit_selection","document":document,"expected_revision":0,"slide_id":document["deck"]["slides"][0]["id"],
        "operation":{"op":"cut","ids":["a"],"format":"keep_source_formatting"}}));
    assert!(cut.is_err(), "unmodeled connector must not lose its target during cut");
    assert_eq!(save(&document), original);
}

#[test]
fn native_group_detaches_only_affected_parts_and_failure_retains_metadata() {
    let base = open(&fixture(json!([rect("a",20.0), rect("b",160.0)])).save().unwrap());
    let mut document = base;
    let spec = json!({"version":1,"preset":"flow/balanced","title":"Synthetic flow","data":{"kind":"items","items":[{"label":"One"},{"label":"Two"}]}});
    for id in ["managed", "untouched"] {
        let added = execute_request(json!({"op":"insert_part","document":document,"expected_revision":document["revision"],
            "slide_id":document["deck"]["slides"][0]["id"],"id":id,"spec":spec})).unwrap();
        document = added["document"].clone();
    }
    let bytes = save(&document); let document = open(&bytes);
    assert_eq!(document["parts"].as_array().unwrap().len(), 2);
    let failed = execute_request(json!({"op":"edit_selection","document":document,"expected_revision":0,
        "slide_id":document["deck"]["slides"][0]["id"],"operation":{"op":"group","ids":["a","managed"],"group_id":"combined"}}));
    assert!(failed.is_err()); assert_eq!(save(&document), bytes);
    let grouped = select(&document, json!({"op":"group","ids":["b","managed"],"group_id":"combined"}));
    let candidate = &grouped["transaction"]["document"];
    assert_eq!(candidate["parts"].as_array().unwrap().len(),1);
    assert_eq!(candidate["parts"][0]["element_id"], "untouched");
    assert_eq!(candidate["parts"][0]["stale"], false);
    assert!(grouped["effects"]["warnings"].as_array().unwrap().iter().any(|warning| warning.as_str().unwrap().contains("Detached 1")));
    let reopened = open(&save(candidate));
    assert_eq!(reopened["parts"][0]["element_id"], "untouched");
    let undone = execute_request(json!({"op":"undo_transaction","document":candidate,"expected_revision":1,"receipt":grouped["transaction"]["receipt"]})).unwrap();
    assert_eq!(undone["document"]["parts"], document["parts"]);
    assert_eq!(save(&undone["document"]), bytes);
    let ungrouped = select(&document, json!({"op":"ungroup","ids":["managed"]}));
    assert_eq!(ungrouped["transaction"]["document"]["parts"].as_array().unwrap().len(),1);
    assert_eq!(open(&save(&ungrouped["transaction"]["document"]))["parts"][0]["element_id"], "untouched");
}

#[test]
fn native_new_selection_ids_cannot_alias_opaque_original_ids() {
    let mut package = fixture(json!([rect("a",20.0),rect("b",160.0)]));
    let unknown = "<p:sp><p:nvSpPr><p:cNvPr id=\"90\" name=\"copy-1\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr/></p:sp>";
    let xml = package.text(SLIDE).unwrap().replace("</p:spTree>", &format!("{unknown}</p:spTree>"));
    package.replace_part(SLIDE, xml.into_bytes()).unwrap();
    let bytes = package.save().unwrap(); let document = open(&bytes);
    let copied = select(&document, json!({"op":"copy","ids":["a"],"format":"keep_source_formatting"}));
    let result = execute_request(json!({"op":"edit_selection","document":document,"expected_revision":0,
        "slide_id":document["deck"]["slides"][0]["id"],"operation":{"op":"paste","id_prefix":"copy","dx":0,"dy":0},"clipboard":copied["clipboard"]}));
    assert!(result.is_err());
    let result = execute_request(json!({"op":"edit_selection","document":document,"expected_revision":0,
        "slide_id":document["deck"]["slides"][0]["id"],"operation":{"op":"group","ids":["a","b"],"group_id":"copy-1"}}));
    assert!(result.is_err()); assert_eq!(save(&document), bytes);
}

#[test]
fn native_wrapped_group_retains_its_original_child_canvas_and_opaque_content() {
    let group = json!({"type":"group","id":"existing","x":100,"y":100,"width":400,"height":200,
        "view_width":200,"view_height":100,"children":[text("label")]});
    let mut package = fixture(json!([group,rect("b",600.0)]));
    let mut xml = package.text(SLIDE).unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let offset = parsed.descendants().find(|node| node.tag_name().name() == "chOff" && node.ancestors().any(|node| node.has_tag_name((P,"grpSp")))).unwrap().range();
    xml.replace_range(offset, "<a:chOff x=\"9525\" y=\"9525\"/>");
    let sentinel = "<vendor:opaque xmlns:vendor=\"urn:opaque\" childOrigin=\"9525\"/>";
    xml = xml.replace("</p:grpSp>", &format!("{sentinel}</p:grpSp>"));
    package.replace_part(SLIDE, xml.into_bytes()).unwrap();
    let document = open(&package.save().unwrap());
    let changed = select(&document,json!({"op":"group","ids":["existing","b"],"group_id":"wrapper"}));
    let output = Package::open(save(&changed["transaction"]["document"])).unwrap();
    let parsed = roxmltree::Document::parse(output.text(SLIDE).unwrap()).unwrap();
    let inner = parsed.descendants().find(|node| node.has_tag_name((P,"grpSp")) && node.children().any(|node| node.descendants().any(|node| node.attribute("name") == Some("existing")) && node.tag_name().name() == "nvGrpSpPr")).unwrap();
    let offset = inner.children().find(|node| node.tag_name().name() == "grpSpPr").unwrap().descendants().find(|node| node.tag_name().name() == "chOff").unwrap();
    assert_eq!(offset.attribute("x"),Some("9525"));
    assert!(output.text(SLIDE).unwrap().contains(sentinel));
}

#[test]
fn native_ungroup_rejects_unmodeled_group_nonvisual_metadata() {
    let group = json!({"type":"group","id":"existing","x":100,"y":100,"width":400,"height":200,
        "view_width":400,"view_height":200,"children":[text("label")]});
    let mut package = fixture(json!([group]));
    let mut xml = package.text(SLIDE).unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let properties = parsed.descendants().find(|node| node.has_tag_name((P,"nvGrpSpPr")) && node.parent().is_some_and(|parent| parent.has_tag_name((P,"grpSp")))).unwrap().range();
    let position = properties.start + xml[properties.clone()].rfind("</").unwrap();
    xml.insert_str(position, "<vendor:metadata xmlns:vendor=\"urn:vendor\"/>");
    package.replace_part(SLIDE,xml.into_bytes()).unwrap();
    let original = package.save().unwrap(); let document = open(&original);
    let result = execute_request(json!({"op":"edit_selection","document":document,"expected_revision":0,
        "slide_id":document["deck"]["slides"][0]["id"],"operation":{"op":"ungroup","ids":["existing"]}}));
    assert!(result.is_err()); assert_eq!(save(&document),original);
}