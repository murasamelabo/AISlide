use aislide_core::model::Element;
use serde_json::{json, Value};
use aislide_core::table_format::{self, Dimensions, DimensionUnit, MergeRegion, TableFormat};
use aislide_core::{execute_request, package::Package};
use base64::{Engine, engine::general_purpose::STANDARD};

fn table() -> Value {
    json!({"type":"table","id":"table-1","x":40.0,"y":100.0,"width":600.0,"height":180.0,"font_size":20.0,"rows":[["Header", "", "C"],["", "", "F"],["G", "H", "I"]]})
}

#[test]
fn optional_format_json_is_backwards_compatible() {
    let old: Element = serde_json::from_value(table()).unwrap();
    assert!(serde_json::to_value(old).unwrap().get("format").is_none());
    let mut input = table();
    input["format"] = json!({"column_widths":{"unit":"relative","values":[1.0,2.0,3.0]},"row_heights":{"unit":"absolute","values":[40.0,60.0,80.0]},"merges":[{"row":0,"column":0,"row_span":2,"col_span":2}]});
    let parsed: Element = serde_json::from_value(input.clone()).expect("table format must be accepted");
    assert_eq!(serde_json::to_value(parsed).unwrap(), input);
}

fn element() -> Element { serde_json::from_value(table()).unwrap() }

fn valid(value: Value) -> bool {
    let deck = json!({"version":1,"title":"Tables","width":1280,"height":720,"slides":[{"id":"slide-1","title":"Tables","background":"FFFFFF","notes":"","elements":[value]}]});
    serde_json::from_value(deck).ok().is_some_and(|deck| aislide_core::model::validate_deck(&deck).is_ok())
}

#[test]
fn dimensions_are_positive_bounded_and_sum_to_the_frame() {
    let dimensions = Dimensions { unit: DimensionUnit::Relative, values: vec![1.0, 2.0, 3.0] };
    assert_eq!(dimensions.resolve(3, 600.0).unwrap(), vec![100.0,200.0,300.0]);
    for (unit, values) in [("relative",json!([0,1,2])),("relative",json!([1,2])),("relative",json!([1e100,1,1])),("absolute",json!([100,200,299])),("absolute",json!([-1,1,600]))] {
        let mut value = table(); value["format"] = json!({"column_widths":{"unit":unit,"values":values}});
        assert!(!valid(value), "{unit} {values}");
    }
    assert!(Dimensions { unit: DimensionUnit::Relative, values: vec![f64::NAN] }.resolve(1,600.0).is_err());
    assert!(Dimensions { unit: DimensionUnit::Relative, values: vec![f64::INFINITY] }.resolve(1,600.0).is_err());
    let mut value = table(); value["format"] = json!({"row_heights":{"unit":"absolute","values":[40,60,80]}}); assert!(valid(value));
}

#[test]
fn merged_grid_requires_disjoint_bounded_regions_and_empty_followers() {
    let region = json!({"row":0,"column":0,"row_span":2,"col_span":2});
    let mut value = table(); value["format"] = json!({"merges":[region]}); assert!(valid(value.clone()));
    value["rows"][1][1] = json!("do not discard"); assert!(!valid(value));
    for regions in [json!([region,region]),json!([{"row":0,"column":0,"row_span":0,"col_span":2}]),json!([{"row":2,"column":2,"row_span":2,"col_span":2}]),json!([{"row":0,"column":0,"row_span":1,"col_span":1}]),json!([{"row":18446744073709551615_u64,"column":0,"row_span":2,"col_span":2}])] {
        let mut value = table(); value["format"] = json!({"merges":regions}); assert!(!valid(value));
    }
}

#[test]
fn cell_styles_validate_unicode_fonts_rich_text_and_limits() {
    let mut value = table(); value["rows"][0][0] = json!("日本😀e\u{301}");
    value["format"] = json!({"cells":[{"row":0,"column":0,"style":{"fill":"@accent2","outline":{"color":"112233","width":1.5},"padding":{"left":2,"right":3,"top":4,"bottom":5},"vertical":"middle","text_style":{"font_family":"Yu Gothic","color":"FF0000","bold":false},"text_format":{"paragraphs":[{"runs":[{"text":"日本😀e\u{301}","style":{"italic":true,"font_size":28}}]}]}}}]});
    assert!(valid(value.clone()));
    for (field, invalid) in [("fill",json!("red")),("padding",json!({"left":-1,"right":0,"top":0,"bottom":0})),("outline",json!({"width":21,"color":"FFFFFF"})),("text_style",json!({"font_family":"\n"})),("text_style",json!({"font_size":401})),("text_format",json!({"hyperlink":"https://example.invalid"})),("text_format",json!({"paragraphs":[{"runs":[{"text":"mismatch"}]}]}))] {
        let mut bad = value.clone(); bad["format"]["cells"][0]["style"][field] = invalid; assert!(!valid(bad), "{field}");
    }
    let mut bad = value.clone(); bad["format"]["cells"][0]["row"] = json!(12); assert!(!valid(bad));
    let mut bad = value.clone(); bad["format"]["cells"] = json!([value["format"]["cells"][0], value["format"]["cells"][0]]); assert!(!valid(bad));
    let mut bad = table(); bad["rows"][0][0] = json!("x".repeat(201)); assert!(!valid(bad));
    let mut bad = table(); bad["rows"][0][0] = json!("\u{0}"); assert!(!valid(bad));
}

#[test]
fn functional_merge_split_and_axis_edits_preserve_anchors() {
    let original = element();
    let merged = table_format::merge_cells(original.clone(), MergeRegion { row:0,column:0,row_span:2,col_span:2 }).unwrap();
    assert!(table_format::insert_row(merged.clone(),1,vec![String::new();3]).is_err());
    assert!(table_format::remove_column(merged.clone(),0).is_err());
    let shifted = table_format::insert_row(merged.clone(),0,vec![String::new();3]).unwrap();
    if let Element::Table { format, .. } = &shifted { assert_eq!(format.merges[0].row,1); }
    let restored = table_format::remove_row(shifted,0).unwrap();
    assert_eq!(serde_json::to_value(restored).unwrap(),serde_json::to_value(&merged).unwrap());
    let split = table_format::split_cell(merged,1,1).unwrap();
    assert_eq!(serde_json::to_value(split).unwrap(),serde_json::to_value(&original).unwrap());
    let mut format = TableFormat::default(); format.column_widths = Some(Dimensions {unit:DimensionUnit::Relative,values:vec![1.0,2.0,3.0]});
    let formatted = table_format::update_format(original,format).unwrap();
    let inserted = table_format::insert_column(formatted,1,vec![String::new();3]).unwrap();
    assert!(table_format::validate_element(&inserted).is_ok());
    assert!(table_format::validate_element(&table_format::remove_column(inserted,1).unwrap()).is_ok());
}

fn export_table(element: Value) -> Vec<u8> {
    let deck = serde_json::from_value(json!({"version":1,"title":"Tables","width":1280,"height":720,"slides":[{"id":"slide-1","title":"Tables","background":"FFFFFF","notes":"","elements":[element]}]})).unwrap();
    aislide_core::pptx::export_pptx(&deck).unwrap()
}

fn open(bytes: &[u8]) -> Value { execute_request(json!({"op":"open_presentation","id":"table-native","base64":STANDARD.encode(bytes)})).unwrap()["document"].clone() }
fn current(document: &Value) -> Value { document["deck"]["slides"][0]["elements"][0].clone() }
fn export_document(document: &Value) -> Vec<u8> { STANDARD.decode(execute_request(json!({"op":"export_presentation","document":document})).unwrap()["base64"].as_str().unwrap()).unwrap() }
fn transact(document: &Value, replacement: Value) -> aislide_core::Result<Value> {
    execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/slides/0/elements/0","value":replacement}]}}))
}

fn formatted() -> Value {
    let mut value = table(); value["rows"][0][0] = json!("日本😀 & English");
    value["format"] = json!({
        "column_widths":{"unit":"relative","values":[1,2,3]},
        "row_heights":{"unit":"absolute","values":[40,60,80]},
        "merges":[{"row":0,"column":0,"row_span":2,"col_span":2}],
        "cells":[{"row":0,"column":0,"style":{
            "fill":"ABCDEF","outline":{"color":"102030","width":2},"padding":{"left":2,"right":3,"top":4,"bottom":5},"vertical":"middle",
            "text_format":{"paragraphs":[{"alignment":"center","runs":[{"text":"日本😀 & ","style":{"font_family":"Yu Gothic","font_size":28,"color":"FF0000"}},{"text":"English","style":{"font_family":"Aptos","italic":true,"language":"en-US"}}]}]}
        }}]
    }); value
}

#[test]
fn native_table_xml_has_full_merge_grid_dimensions_and_styles() {
    let bytes = export_table(formatted()); let package = Package::open(bytes.clone()).unwrap();
    let parsed = roxmltree::Document::parse(package.text("ppt/slides/slide1.xml").unwrap()).unwrap();
    let named = |name| parsed.descendants().filter(move |node| node.tag_name().name() == name).collect::<Vec<_>>();
    let columns = named("gridCol"); assert_eq!(columns.iter().map(|node| node.attribute("w").unwrap()).collect::<Vec<_>>(),vec!["952500","1905000","2857500"]);
    assert_eq!(named("tr").iter().map(|node| node.attribute("h").unwrap()).collect::<Vec<_>>(),vec!["381000","571500","762000"]);
    let cells = named("tc"); assert_eq!(cells.len(),9);
    assert_eq!(cells[0].attribute("gridSpan"),Some("2")); assert_eq!(cells[0].attribute("rowSpan"),Some("2"));
    assert_eq!(cells[1].attribute("hMerge"),Some("1")); assert_eq!(cells[1].attribute("rowSpan"),Some("2"));
    assert_eq!(cells[3].attribute("vMerge"),Some("1")); assert_eq!(cells[3].attribute("gridSpan"),Some("2"));
    assert_eq!(cells[4].attribute("hMerge"),Some("1")); assert_eq!(cells[4].attribute("vMerge"),Some("1"));
    let properties = named("tcPr"); assert_eq!(properties[0].attribute("anchor"),Some("ctr")); assert_eq!(properties[0].attribute("marR"),Some("28575"));
    assert_eq!(named("lnL")[0].attribute("w"),Some("19050"));
    let actual = current(&open(&bytes));
    assert_eq!(actual["rows"], formatted()["rows"]);
    assert_eq!(actual["format"]["merges"],formatted()["format"]["merges"]);
    assert_eq!(actual["format"]["column_widths"]["values"],json!([100.0,200.0,300.0]));
    let style = &actual["format"]["cells"][0]["style"];
    assert_eq!(style["fill"],"ABCDEF"); assert_eq!(style["outline"]["width"],2.0); assert_eq!(style["vertical"],"middle");
    assert_eq!(style["text_format"]["paragraphs"][0]["runs"][0]["style"]["font_family"],"Yu Gothic");
    assert_eq!(style["text_format"]["paragraphs"][0]["runs"][1]["style"]["italic"],true);
}

#[test]
fn legacy_native_table_does_not_grow_format_defaults() {
    let bytes = export_table(table()); let document = open(&bytes);
    assert!(current(&document).get("format").is_none(),"{}",current(&document));
    assert_eq!(export_document(&document),bytes);
}

#[test]
fn malformed_native_merge_flags_are_rejected() {
    let mut package = Package::open(export_table(table())).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().replacen("<a:tc>","<a:tc hMerge=\"1\">",1);
    package.replace_part("ppt/slides/slide1.xml",xml.into_bytes()).unwrap();
    let bytes = package.save().unwrap();
    let result = execute_request(json!({"op":"open_presentation","id":"invalid-merge","base64":STANDARD.encode(&bytes)})).unwrap();
    assert!(result["document"]["deck"]["slides"][0]["elements"].as_array().unwrap().is_empty());
    assert!(result["warnings"].as_array().unwrap().iter().any(|warning| warning.as_str().unwrap().contains("merge grid")));
    assert_eq!(export_document(&result["document"]),bytes);
}

#[test]
fn native_partial_style_dimensions_split_and_undo_preserve_unknowns() {
    let mut package = Package::open(export_table(formatted())).unwrap();
    let sentinel = "<vendor:keep xmlns:vendor=\"urn:table-test\" value=\"42\"/>";
    let xml = package.text("ppt/slides/slide1.xml").unwrap().replacen("<a:tbl>","<a:tbl xmlns:v=\"urn:table-test\" v:keep=\"table\">",1).replacen("<a:gridCol ","<a:gridCol v:keep=\"column\" ",1).replacen("<a:tr ","<a:tr v:keep=\"row\" ",1).replacen("<a:tc ","<a:tc v:keep=\"cell\" ",1).replacen("<a:tcPr ","<a:tcPr v:keep=\"properties\" ",1).replacen("</a:tcPr>",&format!("{sentinel}</a:tcPr>"),1);
    package.replace_part("ppt/slides/slide1.xml",xml.into_bytes()).unwrap();
    let original = package.save().unwrap(); let document = open(&original);
    let mut next = current(&document);
    next["format"]["column_widths"] = json!({"unit":"relative","values":[3,2,1]});
    next["format"]["row_heights"] = json!({"unit":"absolute","values":[50,70,60]});
    next["format"]["merges"] = json!([]);
    next["format"]["cells"][0]["style"]["fill"] = json!("FEDCBA");
    next["format"]["cells"][0]["style"]["vertical"] = json!("bottom");
    next["format"]["cells"][0]["style"]["padding"]["left"] = json!(8);
    next["rows"][2][2] = json!("Updated & 日本");
    let changed = transact(&document,next).unwrap(); let saved = export_document(&changed["document"]);
    let result = Package::open(saved.clone()).unwrap(); let xml = result.text("ppt/slides/slide1.xml").unwrap();
    for value in [sentinel,"v:keep=\"table\"","v:keep=\"column\"","v:keep=\"row\"","v:keep=\"cell\"","v:keep=\"properties\""] { assert!(xml.contains(value),"{value}"); }
    for (part,bytes) in package.parts().iter().filter(|(part,_)| part.as_str() != "ppt/slides/slide1.xml" && !part.starts_with("customXml/")) { assert_eq!(result.part(part).unwrap(),bytes,"non-target {part}"); }
    let actual = current(&open(&saved));
    assert!(actual["format"].get("merges").is_none());
    assert_eq!(actual["format"]["column_widths"]["values"],json!([300.0,200.0,100.0]));
    assert_eq!(actual["format"]["row_heights"]["values"],json!([50.0,70.0,60.0]));
    assert_eq!(actual["format"]["cells"][0]["style"]["fill"],"FEDCBA");
    assert_eq!(actual["format"]["cells"][0]["style"]["vertical"],"bottom");
    assert_eq!(actual["rows"][2][2],"Updated & 日本");
    let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
    assert_eq!(export_document(&undone["document"]),original);
}

#[test]
fn native_rich_cell_edits_and_merge_keep_native_content() {
    let bytes = export_table(formatted()); let document = open(&bytes);
    let mut next = current(&document);
    next["format"]["cells"][0]["style"]["text_format"]["paragraphs"][0]["runs"][1]["style"]["underline"] = json!(true);
    let changed = transact(&document,next).unwrap();
    let actual = current(&open(&export_document(&changed["document"])));
    assert_eq!(actual["format"]["cells"][0]["style"]["text_format"]["paragraphs"][0]["runs"][1]["style"]["underline"],true);
    let simple = open(&export_table(table()));
    let merged = table_format::merge_cells(serde_json::from_value(current(&simple)).unwrap(),MergeRegion {row:0,column:0,row_span:2,col_span:2}).unwrap();
    let changed = transact(&simple,serde_json::to_value(merged).unwrap()).unwrap();
    assert_eq!(current(&open(&export_document(&changed["document"]))) ["format"]["merges"][0]["row_span"],2);
}

#[test]
fn custom_fill_and_complex_cell_body_changes_fail_closed() {
    for fragment in ["fill","field","unknown-run","independent-font"] {
        let mut package = Package::open(export_table(table())).unwrap();
        let xml = package.text("ppt/slides/slide1.xml").unwrap();
        let xml = match fragment {
            "fill" => xml.replacen("<a:schemeClr val=\"accent1\"/>","<a:schemeClr val=\"accent1\"><a:lumMod val=\"50000\"/></a:schemeClr>",1),
            "field" => xml.replacen("<a:r>","<a:fld id=\"{00000000-0000-0000-0000-000000000001}\" type=\"slidenum\">",1).replacen("</a:r>","</a:fld>",1),
            "unknown-run" => xml.replacen("<a:rPr ","<a:rPr custom=\"keep\" ",1),
            _ => xml.replacen("<a:ea typeface=\"+mn-ea\"/>","<a:ea typeface=\"Different font\"/>",1),
        };
        package.replace_part("ppt/slides/slide1.xml",xml.into_bytes()).unwrap();
        let bytes = package.save().unwrap(); let document = open(&bytes); let mut next = current(&document);
        next["format"] = if fragment == "fill" {json!({"cells":[{"row":0,"column":0,"style":{"fill":"123456"}}]})} else {json!({"cells":[{"row":0,"column":0,"style":{"text_style":{"italic":true}}}]})};
        assert!(transact(&document,next).is_err(),"must preserve {fragment}");
        assert_eq!(export_document(&document),bytes);
    }
}

#[test]
fn native_axis_edit_rejects_custom_structure_and_accepts_plain_generated_grid() {
    let bytes = export_table(table()); let document = open(&bytes);
    let added = table_format::insert_row(serde_json::from_value(current(&document)).unwrap(),3,vec!["J".into(),"K".into(),"L".into()]).unwrap();
    let changed = transact(&document,serde_json::to_value(&added).unwrap()).unwrap();
    assert_eq!(current(&open(&export_document(&changed["document"]))) ["rows"].as_array().unwrap().len(),4);
    let mut package = Package::open(bytes).unwrap();
    let xml = package.text("ppt/slides/slide1.xml").unwrap().replacen("<a:tr ","<a:tr custom=\"keep\" ",1);
    package.replace_part("ppt/slides/slide1.xml",xml.into_bytes()).unwrap();
    let document = open(&package.save().unwrap());
    assert!(transact(&document,serde_json::to_value(added).unwrap()).is_err());
}

#[test]
fn grid_limits_cr_and_padding_nonfinite_are_checked() {
    let mut maximum = table(); maximum["rows"] = json!(vec![vec!["";8];12]); assert!(valid(maximum.clone()));
    let parsed: Element = serde_json::from_value(maximum.clone()).unwrap();
    assert!(table_format::insert_row(parsed.clone(),12,vec![String::new();8]).is_err());
    assert!(table_format::insert_column(parsed.clone(),8,vec![String::new();12]).is_err());
    assert!(table_format::remove_row(parsed.clone(),usize::MAX).is_err());
    assert!(table_format::merge_cells(parsed,MergeRegion {row:0,column:0,row_span:12,col_span:8}).is_ok());
    maximum["rows"] = json!(vec![vec!["";8];13]); assert!(!valid(maximum));
    let mut value = table(); value["rows"][0][0] = json!("before\rafter"); assert!(!valid(value));
    let style = table_format::CellStyle { padding:Some(table_format::CellPadding {left:f64::NAN,right:0.0,top:0.0,bottom:0.0}),..Default::default() };
    assert!(table_format::set_cell_style(element(),0,0,style).is_err());
}

#[test]
fn native_cell_style_add_remove_and_relative_resize_roundtrip() {
    let original = open(&export_table(table()));
    let style = table_format::CellStyle {
        fill:Some("none".into()),outline:Some(table_format::CellOutline {color:"223344".into(),width:1.0}),
        text_style:Some(aislide_core::rich_text::RunStyle {font_family:Some("Aptos".into()),font_size:Some(30.0),underline:Some(true),language:Some("en-US".into()),..Default::default()}),..Default::default()
    };
    let next = table_format::set_cell_style(serde_json::from_value(current(&original)).unwrap(),2,2,style).unwrap();
    let changed = transact(&original,serde_json::to_value(next).unwrap()).unwrap();
    let reopened = open(&export_document(&changed["document"]));
    let value = current(&reopened);
    let style = &value["format"]["cells"][0]["style"];
    assert_eq!(style["fill"],"none"); assert_eq!(style["outline"]["width"],1.0);
    assert_eq!(style["text_style"]["font_family"],"Aptos"); assert_eq!(style["text_style"]["font_size"],30.0);
    let next = table_format::set_cell_style(serde_json::from_value(value).unwrap(),2,2,Default::default()).unwrap();
    let changed = transact(&reopened,serde_json::to_value(next).unwrap()).unwrap();
    assert!(current(&open(&export_document(&changed["document"]))).get("format").is_none());
    let mut next = current(&original); next["width"] = json!(900.0); next["height"] = json!(240.0);
    next["format"] = json!({"column_widths":{"unit":"relative","values":[1,2,3]},"row_heights":{"unit":"relative","values":[1,1,2]}});
    let changed = transact(&original,next).unwrap();
    let actual = current(&open(&export_document(&changed["document"])));
    assert_eq!(actual["format"]["column_widths"]["values"],json!([150.0,300.0,450.0]));
    assert_eq!(actual["format"]["row_heights"]["values"],json!([60.0,60.0,120.0]));
}