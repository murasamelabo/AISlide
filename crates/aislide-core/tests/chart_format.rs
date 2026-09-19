use aislide_core::{model::{Deck, validate_deck}, package::Package, pptx::export_pptx};
use serde_json::{json, Value};
use base64::{Engine, engine::general_purpose::STANDARD};
use aislide_core::execute_request;

fn scene(kind: &str) -> Value {
    json!({"version":1,"title":"Synthetic native chart","width":1280,"height":720,"slides":[{
        "id":"slide-1","title":"Synthetic","background":"FFFFFF","notes":"Synthetic values",
        "elements":[{"type":"chart","id":"chart-1","x":64,"y":100,"width":1100,"height":500,
            "kind":kind,"categories":["1","2","3"],"series":[
                {"name":"First","values":[2,4,6],"color":"087F73"},
                {"name":"Second","values":[3,5,7],"color":"CF5847"}]}]}]})
}

fn package(input: Value) -> Package {
    let deck: Deck = serde_json::from_value(input).unwrap();
    validate_deck(&deck).unwrap();
    Package::open(export_pptx(&deck).unwrap()).unwrap()
}

#[test]
fn percent_stacked_bar_keeps_raw_workbook_values() {
    let package = package(scene("percent_stacked_bar"));
    let chart = package.text("ppt/charts/chart1.xml").unwrap();
    let document = roxmltree::Document::parse(chart).unwrap();
    for (name, value) in [("barDir", "bar"), ("grouping", "percentStacked"), ("overlap", "100"), ("max", "1"), ("min", "0")] {
        assert!(document.descendants().any(|node| node.tag_name().name() == name && node.attribute("val") == Some(value)), "{name}={value}");
    }
    let workbook = Package::open(package.part("ppt/embeddings/chart1.xlsx").unwrap().to_vec()).unwrap();
    let sheet = roxmltree::Document::parse(workbook.text("xl/worksheets/sheet1.xml").unwrap()).unwrap();
    let cell = sheet.descendants().find(|node| node.attribute("r") == Some("B2")).unwrap();
    assert_eq!(cell.children().find(|node| node.tag_name().name() == "v").unwrap().text(), Some("2"));
}

fn open(package: &Package) -> Value {
    execute_request(json!({"op":"open_presentation","id":"chart-test","base64":STANDARD.encode(package.save().unwrap())})).unwrap()["document"].clone()
}

#[test]
fn extended_remaining_factories_preserve_typed_data_in_native_parts() {
    for (kind, field, layout) in [("histogram", "histogram", "clusteredColumn"), ("box_whisker", "box_whisker", "boxWhisker"), ("treemap", "hierarchy", "treemap"), ("sunburst", "hierarchy", "sunburst")] {
        let element = execute_request(json!({"op":"create_object","id":"chart-1","kind":"chart","preset":kind})).unwrap();
        assert!(element["options"][field].is_object(), "{kind} requires typed data");
        let mut input = scene(kind);
        input["slides"][0]["elements"] = json!([element]);
        let original = package(input);
        let chart = roxmltree::Document::parse(original.text("ppt/charts/chart1.xml").unwrap()).unwrap();
        assert!(chart.descendants().any(|node| node.attribute("layoutId") == Some(layout)), "{kind}");
        let reopened = open(&original);
        let restored = &reopened["deck"]["slides"][0]["elements"][0];
        assert_eq!(restored["kind"], kind);
        assert_eq!(restored["categories"], element["categories"]);
        assert_eq!(restored["series"], element["series"]);
        assert_eq!(restored["options"], element["options"]);
    }
}

fn remaining_scene(kind: &str) -> Value {
    let (categories, values, options) = match kind {
        "histogram" => (json!([]), json!([]), json!({"histogram":{"samples":[3.25,-2.5,3.25,0.125,8,1.5,4,6.75,10,2],"binning":{"rule":"count","count":4},"interval_closed":"right"}})),
        "box_whisker" => (json!(["Group A","Group B","Group C"]), json!([]), json!({"box_whisker":{"samples":[[7,-2.5,1.25,3],[2,4,4,6,25],[0.125,2.5,3,5,8,9]],"quartile_method":"inclusive","mean_line":false,"mean_marker":true,"nonoutliers":false,"outliers":true}})),
        _ => (json!(["A","B","C","D"]), json!([10.25,6.5,8.125,12]), json!({"hierarchy":{"paths":[["North","A"],["North","B"],["South","C"],["South","D"]]}})),
    };
    let element = json!({"type":"chart","id":"chart-1","x":64,"y":100,"width":1100,"height":500,"kind":kind,"categories":categories,"series":[{"name":"Synthetic observations","values":values,"color":"087F73"}],"options":options});
    let mut input = scene(kind);
    input["slides"][0]["elements"] = json!([element, {"type":"text","id":"title","x":64,"y":24,"width":1100,"height":60,"text":format!("Synthetic native {kind}"),"font_size":28,"color":"111111","bold":false}]);
    input
}

#[test]
fn extended_remaining_reports_keep_options_and_reject_wrong_data_shapes() {
    for kind in ["histogram", "box_whisker", "treemap", "sunburst"] {
        let input = remaining_scene(kind);
        let element = &input["slides"][0]["elements"][0];
        let chart = json!({"kind":kind,"categories":element["categories"],"series":element["series"],"options":element["options"]});
        let report = json!({"title":"Synthetic report","subtitle":"","period":"","source":"Synthetic inputs","sections":[{"title":"Synthetic chart","layout":"chart","chart":chart}]});
        let result = execute_request(json!({"op":"compile","report":report})).unwrap();
        let compiled = result["deck"]["slides"][0]["elements"].as_array().unwrap().iter().find(|element| element["type"] == "chart").unwrap();
        assert_eq!(serde_json::from_value::<aislide_core::model::ChartOptions>(compiled["options"].clone()).unwrap(), serde_json::from_value::<aislide_core::model::ChartOptions>(chart["options"].clone()).unwrap());
        let invalid_options = if kind == "histogram" { vec![json!({"histogram":{"samples":[1,2],"binning":{"rule":"count","count":0},"interval_closed":"right"}}), json!({"histogram":{"samples":vec![1;4097],"binning":{"rule":"count","count":2},"interval_closed":"right"}}), json!({"histogram":{"samples":[1,1000],"binning":{"rule":"width","width":0.1},"interval_closed":"right"}})] } else if kind == "box_whisker" { vec![json!({"box_whisker":{"samples":[[1,2],[3,4]],"quartile_method":"inclusive","mean_line":false,"mean_marker":true,"nonoutliers":false,"outliers":true}})] } else { vec![json!({"hierarchy":{"paths":[["Root","A"],["Root","A"],["Root","C"],["Root","D"]]}}), json!({"hierarchy":{"paths":[["Root","Wrong"],["Root","B"],["Root","C"],["Root","D"]]}})] };
        for options in invalid_options {
            let mut invalid = input.clone(); invalid["slides"][0]["elements"][0]["options"] = options;
            assert!(validate_deck(&serde_json::from_value(invalid).unwrap()).is_err(), "{kind}");
        }
    }
}

fn remaining_edit(document: &Value) -> Value {
    let mut element = document["deck"]["slides"][0]["elements"][0].clone();
    match element["kind"].as_str().unwrap() {
        "histogram" => { element["options"]["histogram"] = json!({"samples":[0,1,2,2,3,4,5,7,9,12],"binning":{"rule":"width","width":2},"interval_closed":"left","underflow":1,"overflow":9}); }
        "box_whisker" => { element["categories"] = json!(["Group A", "Group B", "Group C"]); element["options"]["box_whisker"] = json!({"samples":[[1,2,2,3,4,6,21],[4,6,7,8,9,10],[2,4,5,6,8,12]],"quartile_method":"exclusive","mean_line":true,"mean_marker":false,"nonoutliers":true,"outliers":false}); }
        _ => {
            element["categories"] = json!(["A", "B", "C", "D", "E"]);
            element["series"][0]["values"] = json!([20,15,12,18,7]);
            element["options"]["hierarchy"]["paths"] = json!([["North","Retail","A"],["North","Retail","B"],["South","Retail","C"],["South","Online","D"],["South","Online","E"]]);
            if element["kind"] == "treemap" { element["options"]["hierarchy"]["parent_labels"] = json!("overlapping"); }
        }
    }
    replace(document, "/deck/slides/0/elements/0", element).unwrap()
}

#[test]
fn extended_remaining_boundary_contracts_and_exact_sample_cells() {
    for kind in ["histogram", "box_whisker", "treemap", "sunburst"] {
        let input = remaining_scene(kind);
        let original = package(input.clone());
        let document = open(&original);
        let expected: aislide_core::model::Element = serde_json::from_value(input["slides"][0]["elements"][0].clone()).unwrap();
        let actual: aislide_core::model::Element = serde_json::from_value(document["deck"]["slides"][0]["elements"][0].clone()).unwrap();
        assert_eq!(serde_json::to_value(actual).unwrap(), serde_json::to_value(expected).unwrap(), "{kind} independent native input");
        let workbook = Package::open(original.part("ppt/embeddings/chart1.xlsx").unwrap().to_vec()).unwrap();
        let sheet = roxmltree::Document::parse(workbook.text("xl/worksheets/sheet1.xml").unwrap()).unwrap();
        let numbers: Vec<f64> = sheet.descendants().filter(|node| node.tag_name().name() == "v").map(|node| node.text().unwrap().parse().unwrap()).collect();
        let element = &input["slides"][0]["elements"][0];
        let expected_numbers: Vec<f64> = match kind {
            "histogram" => serde_json::from_value(element["options"]["histogram"]["samples"].clone()).unwrap(),
            "box_whisker" => serde_json::from_value::<Vec<Vec<f64>>>(element["options"]["box_whisker"]["samples"].clone()).unwrap().into_iter().flatten().collect(),
            _ => serde_json::from_value(element["series"][0]["values"].clone()).unwrap(),
        };
        assert_eq!(numbers, expected_numbers, "{kind} raw workbook order/duplicates/fractions");
        let invalid: Vec<(&str, Value)> = match kind {
            "histogram" => vec![("/options/histogram/samples", json!([])), ("/options/histogram/samples", json!([1e16])), ("/options/histogram/binning", json!({"rule":"count","count":129})), ("/options/histogram/binning", json!({"rule":"width","width":0})), ("/options/histogram/binning", json!({"rule":"width","width":1e-20})), ("/options/histogram/underflow", json!(1e16)), ("/options/histogram/overflow", json!(-1e16)), ("/categories", json!(["Fake bin"])), ("/series/0/values", json!([10]))],
            "box_whisker" => vec![("/options/box_whisker/samples/0", json!([1,2,3])), ("/options/box_whisker/samples/0", json!([1,2,3,1e16])), ("/options/box_whisker/samples", json!([vec![1;2049],vec![2;2048],vec![3;4]])), ("/categories", json!(["Same","Same","Other"])), ("/series/0/values", json!([1,2,3]))],
            _ => vec![("/options/hierarchy/paths/0", json!(["A"])), ("/options/hierarchy/paths/0", json!(["R","S","T","U","A"])), ("/options/hierarchy/paths/0", json!(["","A"])), ("/options/hierarchy/paths/0", json!(["North","B"])), ("/series/0/values/0", json!(0)), ("/series/0/values/0", json!(-1))],
        };
        for (path, value) in invalid {
            let mut candidate = element.clone();
            let parent = &path[..path.rfind('/').unwrap()];
            let key = &path[path.rfind('/').unwrap() + 1..];
            let target = candidate.pointer_mut(parent).unwrap();
            if let Some(array) = target.as_array_mut() { array[key.parse::<usize>().unwrap()] = value; } else { target[key] = value; }
            let result = replace(&document, "/deck/slides/0/elements/0", candidate);
            assert!(result.is_err(), "{kind} accepted {path}");
        }
        if kind == "histogram" {
            for (underflow, overflow) in [(3.0, 3.0), (4.0, 3.0)] {
                let mut candidate = element.clone();
                candidate["options"]["histogram"]["underflow"] = json!(underflow);
                candidate["options"]["histogram"]["overflow"] = json!(overflow);
                assert!(replace(&document, "/deck/slides/0/elements/0", candidate).is_err());
            }
            let mut candidate = element.clone();
            candidate["options"]["histogram"]["samples"] = json!(vec![1.25;4096]);
            candidate["options"]["histogram"]["binning"] = json!({"rule":"count","count":128});
            assert!(replace(&document, "/deck/slides/0/elements/0", candidate).is_ok());
        }
        if matches!(kind, "treemap" | "sunburst") {
            let mut duplicate = element.clone();
            duplicate["categories"][1] = json!("A");
            duplicate["options"]["hierarchy"]["paths"][1] = json!(["North","A"]);
            assert!(replace(&document, "/deck/slides/0/elements/0", duplicate).is_err());
            for depth in [2, 3, 4] {
                let mut candidate = element.clone();
                candidate["options"]["hierarchy"]["paths"] = json!(["A","B","C","D"].map(|leaf| { let mut path = vec!["Root";depth - 1]; path.push(leaf); path }));
                let changed = replace(&document, "/deck/slides/0/elements/0", candidate).unwrap();
                let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
                let reopened = execute_request(json!({"op":"open_presentation","id":"depth","base64":output["base64"]})).unwrap();
                assert_eq!(reopened["document"]["deck"]["slides"][0]["elements"][0]["options"], changed["document"]["deck"]["slides"][0]["elements"][0]["options"]);
            }
        }
        for mutation in ["sparse", "duplicate", "count", "nonnumeric", "formula"] {
            let mut malformed = original.clone();
            let xml = original.text("ppt/charts/chart1.xml").unwrap();
            let parsed = roxmltree::Document::parse(xml).unwrap();
            let dimension = parsed.descendants().find(|node| node.tag_name().name() == "numDim").unwrap();
            let level = dimension.children().find(|node| node.tag_name().name() == "lvl").unwrap();
            let point = level.children().find(|node| node.is_element()).unwrap();
            let mut updated = xml.to_owned();
            match mutation {
                "sparse" => { updated.replace_range(point.range(), ""); },
                "duplicate" => { updated.insert_str(point.range().end, &xml[point.range()]); },
                "count" => { let source = &xml[level.range()]; updated.replace_range(level.range(), &source.replacen(&format!("ptCount=\"{}\"", level.attribute("ptCount").unwrap()), "ptCount=\"4097\"", 1)); },
                "nonnumeric" => { updated.replace_range(point.range(), "<cx:pt idx=\"0\">NaN</cx:pt>"); },
                _ => { let formula = dimension.children().find(|node| node.tag_name().name() == "f").unwrap(); updated.replace_range(formula.range(), "<cx:f>Sheet1!$E$2:$E$9999</cx:f>"); },
            }
            malformed.replace_part("ppt/charts/chart1.xml", updated.into_bytes()).unwrap();
            let opened = open(&malformed);
            let saved = execute_request(json!({"op":"export_presentation","document":opened})).unwrap();
            assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), malformed.save().unwrap());
            assert!(replace(&opened, "/deck/slides/0/elements/0/series/0/name", json!("Unsafe replacement")).is_err(), "{kind} {mutation}");
        }
    }
}

fn assert_remaining_caches(package: &Package, number: usize) {
    let chart = roxmltree::Document::parse(package.text(&format!("ppt/charts/chart{number}.xml")).unwrap()).unwrap();
    let workbook = Package::open(package.part(&format!("ppt/embeddings/chart{number}.xlsx")).unwrap().to_vec()).unwrap();
    let sheet = roxmltree::Document::parse(workbook.text("xl/worksheets/sheet1.xml").unwrap()).unwrap();
    let mut checked = 0;
    for dimension in chart.descendants().filter(|node| ["strDim", "numDim"].contains(&node.tag_name().name())) {
        let formula = dimension.children().find(|node| node.tag_name().name() == "f").unwrap().text().unwrap();
        let range = formula.strip_prefix("Sheet1!$").unwrap().split(':').collect::<Vec<_>>();
        let last_column = range[1].as_bytes()[1];
        for (level, cache) in dimension.children().filter(|node| node.tag_name().name() == "lvl").enumerate() {
            let column = char::from(last_column - level as u8);
            for point in cache.children().filter(|node| node.is_element()) {
                let row = point.attribute("idx").unwrap().parse::<usize>().unwrap() + 2;
                let address = format!("{column}{row}");
                let cell = sheet.descendants().find(|node| node.attribute("r") == Some(&address)).unwrap();
                let value = cell.descendants().find(|node| ["t", "v"].contains(&node.tag_name().name())).unwrap().text();
                assert_eq!(point.text(), value, "{formula} {address}"); checked += 1;
            }
        }
    }
    assert!(checked >= 10);
}

#[test]
fn extended_remaining_native_edits_caches_undo_and_unknown_guards() {
    for kind in ["histogram", "box_whisker", "treemap", "sunburst"] {
        let original = package(remaining_scene(kind));
        assert_remaining_caches(&original, 1);
        let document = open(&original);
        let changed = remaining_edit(&document);
        let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        let edited = Package::open(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap()).unwrap();
        assert_remaining_caches(&edited, 2);
        let reopened = open(&edited);
        for field in ["kind", "categories", "series", "options"] { assert_eq!(reopened["deck"]["slides"][0]["elements"][0][field], changed["document"]["deck"]["slides"][0]["elements"][0][field], "{kind} {field}"); }
        let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
        let restored = execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap();
        assert_eq!(STANDARD.decode(restored["base64"].as_str().unwrap()).unwrap(), original.save().unwrap());
        for mutation in ["chart", "workbook", "style"] {
            let mut unknown = original.clone();
            if mutation == "workbook" {
                let mut workbook = Package::open(unknown.part("ppt/embeddings/chart1.xlsx").unwrap().to_vec()).unwrap();
                let sheet = workbook.text("xl/worksheets/sheet1.xml").unwrap().replace("<v>", "<v>9");
                workbook.replace_part("xl/worksheets/sheet1.xml", sheet.into_bytes()).unwrap();
                unknown.replace_part("ppt/embeddings/chart1.xlsx", workbook.save().unwrap()).unwrap();
            } else {
                let path = if mutation == "chart" { "ppt/charts/chart1.xml" } else { "ppt/charts/style1.xml" };
                let source = unknown.text(path).unwrap().to_owned();
                let parsed = roxmltree::Document::parse(&source).unwrap();
                let attribute = parsed.root_element().range().start + source[parsed.root_element().range()].find(' ').unwrap();
                let mut xml = source.clone(); xml.insert_str(attribute, " unknown=\"retained\""); unknown.replace_part(path, xml.into_bytes()).unwrap();
            }
            let document = open(&unknown);
            let noop = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
            assert_eq!(STANDARD.decode(noop["base64"].as_str().unwrap()).unwrap(), unknown.save().unwrap());
            assert!(replace(&document, "/deck/slides/0/elements/0/series/0/name", json!("Changed")).is_err(), "{kind} {mutation}");
        }
    }
}

#[test]
fn sunburst_boundaries_preserve_legacy_edits_and_reject_custom_lines() {
    let original = package(remaining_scene("sunburst"));
    let xml = original.text("ppt/charts/chart1.xml").unwrap();
    let parsed = roxmltree::Document::parse(xml).unwrap();
    let series = parsed.descendants().find(|node| node.tag_name().name() == "series").unwrap();
    let properties = series.children().find(|node| node.tag_name().name() == "spPr").unwrap();
    let line = properties.children().find(|node| node.has_tag_name(("http://schemas.openxmlformats.org/drawingml/2006/main", "ln"))).expect("hierarchy boundaries must be explicit");
    assert_eq!(line.attribute("w"), Some("19050"));
    assert!(line.descendants().any(|node| node.tag_name().name() == "schemeClr" && node.attribute("val") == Some("lt1")));
    let mut legacy_xml = xml.to_owned(); legacy_xml.replace_range(line.range(), "");
    for source in [xml.to_owned(), legacy_xml] {
        let mut candidate = original.clone(); candidate.replace_part("ppt/charts/chart1.xml", source.clone().into_bytes()).unwrap();
        let document = open(&candidate);
        let noop = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        assert_eq!(STANDARD.decode(noop["base64"].as_str().unwrap()).unwrap(), candidate.save().unwrap());
        let changed = replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(20)).unwrap();
        let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        let edited = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
        let updated = roxmltree::Document::parse(edited.text("ppt/charts/chart2.xml").unwrap()).unwrap();
        assert!(updated.descendants().any(|node| node.tag_name().name() == "ln" && node.attribute("w") == Some("19050")));
        assert_eq!(open(&edited)["deck"]["slides"][0]["elements"][0]["series"][0]["values"][0].as_f64(), Some(20.0));
        let undone = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
        let restored = execute_request(json!({"op":"export_presentation","document":undone["document"]})).unwrap();
        assert_eq!(STANDARD.decode(restored["base64"].as_str().unwrap()).unwrap(), candidate.save().unwrap());
        let foreign = source.replacen("<cx:spPr>", "<cx:spPr unknown=\"retained\">", 1);
        candidate.replace_part("ppt/charts/chart1.xml", foreign.into_bytes()).unwrap();
        assert!(replace(&open(&candidate), "/deck/slides/0/elements/0/series/0/values/0", json!(20)).is_err());
    }
    let mut custom = original.clone(); custom.replace_part("ppt/charts/chart1.xml", xml.replacen("w=\"19050\"", "w=\"38100\"", 1).into_bytes()).unwrap();
    let document = open(&custom);
    let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
    assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), custom.save().unwrap());
    assert!(replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(20)).is_err());
}

#[test]
#[ignore = "writes new synthetic Office fixtures only to an explicit temporary directory"]
fn extended_remaining_office_fixture() {
    use std::io::Write;
    let directory = std::env::var_os("AISLIDE_CHARTEX_SCHEMA_DIR").expect("explicit temporary fixture directory");
    for kind in ["histogram", "box_whisker", "treemap", "sunburst"] {
        let original = package(remaining_scene(kind));
        let changed = remaining_edit(&open(&original));
        let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        for (prefix, bytes) in [("native", original.save().unwrap()), ("edited", STANDARD.decode(output["base64"].as_str().unwrap()).unwrap())] {
            let path = std::path::Path::new(&directory).join(format!("{prefix}-{kind}.pptx"));
            std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap().write_all(&bytes).unwrap();
        }
    }
}

#[test]
fn histogram_office_bin_encoding_and_ambiguous_input_preservation() {
    for (binning, tag, value) in [
        (json!({"rule":"count","count":4}), "binCount", "4"),
        (json!({"rule":"width","width":2}), "binSize", "2"),
    ] {
        let mut input = remaining_scene("histogram");
        input["slides"][0]["elements"][0]["options"]["histogram"]["binning"] = binning;
        let original = package(input.clone());
        let xml = original.text("ppt/charts/chart1.xml").unwrap();
        let parsed = roxmltree::Document::parse(xml).unwrap();
        let node = parsed.descendants().find(|node| node.tag_name().name() == tag).unwrap();
        assert_eq!(node.attribute("val"), Some(value));
        assert_eq!(node.text(), None);
        for replacement in [
            format!("<cx:{tag}>{value}</cx:{tag}>"),
            format!("<cx:{tag} val=\"{value}\"/>"),
        ] {
            let mut candidate = original.clone();
            let mut updated = xml.to_owned();
            updated.replace_range(node.range(), &replacement);
            candidate.replace_part("ppt/charts/chart1.xml", updated.into_bytes()).unwrap();
            let document = open(&candidate);
            let actual: aislide_core::model::chart_format::HistogramOptions = serde_json::from_value(document["deck"]["slides"][0]["elements"][0]["options"]["histogram"].clone()).unwrap();
            let expected: aislide_core::model::chart_format::HistogramOptions = serde_json::from_value(input["slides"][0]["elements"][0]["options"]["histogram"].clone()).unwrap();
            assert_eq!(actual, expected);
            let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
            assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), candidate.save().unwrap());
        }
        for replacement in [
            format!("<cx:{tag} val=\"{value}\">9</cx:{tag}>"),
            format!("<cx:{tag} val=\"{value}\"><cx:v>9</cx:v></cx:{tag}>"),
            format!("<cx:{tag} val=\"{value}\" unknown=\"retained\"/>"),
        ] {
            let mut candidate = original.clone();
            let mut updated = xml.to_owned();
            updated.replace_range(node.range(), &replacement);
            candidate.replace_part("ppt/charts/chart1.xml", updated.into_bytes()).unwrap();
            let document = open(&candidate);
            let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
            assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), candidate.save().unwrap());
            assert!(replace(&document, "/deck/slides/0/elements/0/series/0/name", json!("Unsafe replacement")).is_err());
        }
    }
}

#[test]
#[ignore = "requires explicit temporary output and local PowerPoint"]
fn histogram_office_count_width_and_single_sample_regression() {
    use std::io::Write;
    let directory = std::env::var_os("AISLIDE_CHARTEX_SCHEMA_DIR").expect("explicit temporary fixture directory");
    let directory = std::path::Path::new(&directory).canonicalize().unwrap();
    assert!(directory.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    let original = package(remaining_scene("histogram"));
    let changed = remaining_edit(&open(&original));
    let output = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let mut single = remaining_scene("histogram");
    single["slides"][0]["elements"][0]["options"]["histogram"]["samples"] = json!([-2.5]);
    single["slides"][0]["elements"][0]["options"]["histogram"]["binning"] = json!({"rule":"count","count":1});
    let cases = [
        ("native-histogram-count", original.save().unwrap(), 4, 4.0),
        ("edited-histogram-width", STANDARD.decode(output["base64"].as_str().unwrap()).unwrap(), 3, 2.0),
        ("native-histogram-single", package(single).save().unwrap(), 4, 1.0),
    ];
    let helper = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/verify-powerpoint.ps1");
    let mut failures = Vec::new();
    for (name, bytes, mode, value) in cases {
        let path = directory.join(format!("{name}.pptx"));
        std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap().write_all(&bytes).unwrap();
        let checked = std::process::Command::new("pwsh")
            .args(["-NoProfile", "-File"]).arg(&helper).arg("-Path").arg(&path)
            .args(["-ExpectedSlides", "1", "-ExpectedCharts", "1", "-MinimumTables", "0", "-InspectHistogramBins", "-VerifyChartData", "-ChartValueCell", "B2", "-CaptureSlides", "1"])
            .output().unwrap();
        let log = format!("{}\n{}", String::from_utf8_lossy(&checked.stdout), String::from_utf8_lossy(&checked.stderr));
        std::fs::OpenOptions::new().write(true).create_new(true).open(directory.join(format!("{name}.log"))).unwrap().write_all(log.as_bytes()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "source changed: {name}");
        if !checked.status.success() { failures.push(name); continue; }
        let proof: Value = serde_json::from_slice(&checked.stdout).unwrap();
        assert_eq!(proof["EmbeddedWorkbooksEdited"], 1);
        assert_eq!(proof["HistogramBins"][0]["ChartType"], 118);
        assert_eq!(proof["HistogramBins"][0]["Mode"], mode);
        assert_eq!(proof["HistogramBins"][0][if mode == 4 { "Count" } else { "Width" }].as_f64(), Some(value));
    }
    assert!(failures.is_empty(), "PowerPoint rejected histogram fixtures: {failures:?}");
}

#[test]
fn fixed_center_labels_omit_office_rejected_position_and_keep_native_edits() {
    for kind in ["doughnut", "area", "radar", "radar_filled", "column3d", "bar3d"] {
        let mut input = scene(kind);
        if kind == "doughnut" { input["slides"][0]["elements"][0]["series"].as_array_mut().unwrap().truncate(1); }
        input["slides"][0]["elements"][0]["options"] = json!({"legend":"right","data_labels":{"show_value":true,"position":"center"}});
        let original = package(input);
        let chart = roxmltree::Document::parse(original.text("ppt/charts/chart1.xml").unwrap()).unwrap();
        assert!(!chart.descendants().any(|node| node.has_tag_name(("http://schemas.openxmlformats.org/drawingml/2006/chart", "dLblPos"))), "{kind} emits an Office-rejected position");
        assert!(chart.descendants().any(|node| node.tag_name().name() == "showVal" && node.attribute("val") == Some("1")));
        let document = open(&original);
        assert_eq!(document["deck"]["slides"][0]["elements"][0]["options"]["data_labels"]["position"], "center", "{kind}");
        let changed = replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(8)).unwrap();
        let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        let edited = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
        assert_workbook_caches(&edited, "ppt/charts/chart2.xml", "ppt/embeddings/chart2.xlsx");
        let undo = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
        let restored = execute_request(json!({"op":"export_presentation","document":undo["document"]})).unwrap();
        assert_eq!(STANDARD.decode(restored["base64"].as_str().unwrap()).unwrap(), original.save().unwrap());
    }
}

#[test]
fn fixed_center_labels_preserve_implicit_positions_and_reject_unknown_extensions() {
    for kind in ["doughnut", "area", "radar", "radar_filled", "column3d", "bar3d", "combo"] {
        let mut input = scene(kind);
        let element = &mut input["slides"][0]["elements"][0];
        if kind == "doughnut" { element["series"].as_array_mut().unwrap().truncate(1); }
        if kind == "combo" { element["series"][0]["kind"] = json!("area"); element["series"][1]["kind"] = json!("line"); }
        element["options"] = json!({"data_labels":{"show_value":true}});
        let implicit = package(input.clone());
        assert!(open(&implicit)["deck"]["slides"][0]["elements"][0]["options"]["data_labels"].get("position").is_none());
        replace(&open(&implicit), "/deck/slides/0/elements/0/series/0/values/0", json!(8)).unwrap();
        input["slides"][0]["elements"][0]["options"]["data_labels"]["position"] = json!("center");
        let explicit = package(input.clone());
        let document = open(&explicit);
        assert_eq!(document["deck"]["slides"][0]["elements"][0]["options"]["data_labels"]["position"], "center");
        replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(8)).unwrap();
        let xml = explicit.text("ppt/charts/chart1.xml").unwrap();
        let parsed = roxmltree::Document::parse(xml).unwrap();
        let marker = parsed.descendants().find(|node| node.has_tag_name(("urn:aislide:chart-label-position:v1", "position"))).unwrap();
        let mut changed = xml.to_owned();
        changed.replace_range(marker.range(), "<ais:position xmlns:ais=\"urn:aislide:chart-label-position:v1\" val=\"center\" unknown=\"retained\"/>");
        let mut unknown = explicit.clone(); unknown.replace_part("ppt/charts/chart1.xml", changed.into_bytes()).unwrap();
        let document = open(&unknown);
        let noop = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        assert_eq!(STANDARD.decode(noop["base64"].as_str().unwrap()).unwrap(), unknown.save().unwrap());
        assert!(replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(8)).is_err());
        for position in ["inside_end", "outside_end", "best_fit"] {
            input["slides"][0]["elements"][0]["options"]["data_labels"]["position"] = json!(position);
            assert!(validate_deck(&serde_json::from_value(input.clone()).unwrap()).is_err(), "{kind} {position}");
        }
    }
}

#[test]
fn fixed_center_labels_legacy_native_encoding_can_be_edited_without_losing_unknown_content() {
    for kind in ["doughnut", "area", "radar", "radar_filled", "column3d", "bar3d"] {
        let mut input = scene(kind);
        if kind == "doughnut" { input["slides"][0]["elements"][0]["series"].as_array_mut().unwrap().truncate(1); }
        input["slides"][0]["elements"][0]["options"] = json!({"data_labels":{"show_value":true,"position":"center"}});
        let mut legacy = package(input);
        let xml = legacy.text("ppt/charts/chart1.xml").unwrap().to_owned();
        let parsed = roxmltree::Document::parse(&xml).unwrap();
        let labels = parsed.descendants().find(|node| node.tag_name().name() == "dLbls").unwrap();
        let extension = labels.children().find(|node| node.tag_name().name() == "extLst").unwrap();
        let first_flag = labels.children().find(|node| node.tag_name().name() == "showLegendKey").unwrap();
        let mut old_xml = xml.clone(); old_xml.replace_range(extension.range(), "");
        old_xml.insert_str(first_flag.range().start, "<c:dLblPos val=\"ctr\"/>");
        legacy.replace_part("ppt/charts/chart1.xml", old_xml.clone().into_bytes()).unwrap();
        let document = open(&legacy);
        let noop = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        assert_eq!(STANDARD.decode(noop["base64"].as_str().unwrap()).unwrap(), legacy.save().unwrap());
        let changed = replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(8)).unwrap();
        let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        let edited = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
        let chart = roxmltree::Document::parse(edited.text("ppt/charts/chart2.xml").unwrap()).unwrap();
        assert!(!chart.descendants().any(|node| node.tag_name().name() == "dLblPos"));
        assert_workbook_caches(&edited, "ppt/charts/chart2.xml", "ppt/embeddings/chart2.xlsx");
        legacy.replace_part("ppt/charts/chart1.xml", old_xml.replace("val=\"ctr\"", "val=\"ctr\" unknown=\"retained\"").into_bytes()).unwrap();
        assert!(replace(&open(&legacy), "/deck/slides/0/elements/0/series/0/values/0", json!(8)).is_err());
    }
}

#[test]
fn expanded_families_use_real_native_groups_and_roundtrip() {
    for (kind, tag) in [("percent_stacked_bar", "barChart"), ("bubble", "bubbleChart"), ("radar", "radarChart"), ("radar_filled", "radarChart"), ("column3d", "bar3DChart"), ("bar3d", "bar3DChart"), ("pie3d", "pie3DChart")] {
        let mut input = scene(kind);
        if kind == "pie3d" { input["slides"][0]["elements"][0]["series"].as_array_mut().unwrap().truncate(1); }
        if kind == "bubble" { for entry in input["slides"][0]["elements"][0]["series"].as_array_mut().unwrap() { entry["bubble_sizes"] = json!([4,9,16]); } }
        let package = package(input);
        let chart = roxmltree::Document::parse(package.text("ppt/charts/chart1.xml").unwrap()).unwrap();
        assert!(chart.descendants().any(|node| node.tag_name().name() == tag), "{kind}");
        let document = open(&package);
        assert_eq!(document["deck"]["slides"][0]["elements"][0]["kind"], kind);
        let noop = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        assert_eq!(STANDARD.decode(noop["base64"].as_str().unwrap()).unwrap(), package.save().unwrap());
        let changed = replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(8)).unwrap();
        let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        let edited = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
        assert_workbook_caches(&edited, "ppt/charts/chart2.xml", "ppt/embeddings/chart2.xlsx");
        assert_eq!(open(&edited)["deck"]["slides"][0]["elements"][0]["series"][0]["values"][0].as_f64(), Some(8.0));
        if kind == "bubble" {
            assert_eq!(document["deck"]["slides"][0]["elements"][0]["series"][1]["bubble_sizes"], json!([4.0,9.0,16.0]));
            assert!(chart.descendants().any(|node| node.tag_name().name() == "f" && node.text() == Some("Sheet1!$E$2:$E$4")));
        }
    }
}

#[test]
fn combo_controls_labels_and_series_statistics_roundtrip_and_edit() {
    let mut input = scene("combo");
    let element = &mut input["slides"][0]["elements"][0];
    element["series"][0]["kind"] = json!("column");
    element["series"][1]["kind"] = json!("line");
    element["series"][1]["axis"] = json!("secondary");
    element["series"][1]["trendline"] = json!({"kind":"linear","intercept":0,"forward":1,"display_equation":true});
    element["series"][1]["error_bars"] = json!({"kind":"custom","plus":[1,2,3],"minus":[0.5,1,1.5]});
    element["options"] = json!({"primary_axis":{"min":-2,"max":20,"major_unit":2,"minor_unit":1,"number_format":"0.0"},"secondary_axis":{"min":1,"max":100,"log_base":10},"category_axis":{"reverse":true},"legend":"right","data_labels":{"show_value":true,"show_category_name":true,"position":"center"}});
    let package = package(input);
    let chart = roxmltree::Document::parse(package.text("ppt/charts/chart1.xml").unwrap()).unwrap();
    let plot = chart.descendants().find(|node| node.tag_name().name() == "plotArea").unwrap();
    let line = plot.children().find(|node| node.tag_name().name() == "lineChart").unwrap();
    assert_eq!(line.children().filter(|node| node.tag_name().name() == "axId").map(|node| node.attribute("val").unwrap()).collect::<Vec<_>>(), ["3","4"]);
    assert_eq!(chart.descendants().filter(|node| node.tag_name().name() == "dLbls").count(), 2);
    assert_eq!(chart.descendants().filter(|node| node.tag_name().name() == "trendline").count(), 1);
    let document = open(&package);
    let element = &document["deck"]["slides"][0]["elements"][0];
    assert_eq!(element["kind"], "combo");
    assert_eq!(element["series"][1]["axis"], "secondary");
    assert_eq!(element["options"]["primary_axis"]["min"].as_f64(), Some(-2.0));
    assert_eq!(element["options"]["category_axis"]["reverse"], true);
    assert_eq!(element["series"][1]["error_bars"]["minus"], json!([0.5,1.0,1.5]));
    let changed = execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/slides/0/elements/0/series/1/values/0","value":9}]}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let reopened = execute_request(json!({"op":"open_presentation","id":"chart-edit","base64":saved["base64"]})).unwrap();
    assert_eq!(reopened["document"]["deck"]["slides"][0]["elements"][0]["series"][1]["values"][0].as_f64(), Some(9.0));
}

#[test]
fn invalid_controls_are_rejected_not_silently_dropped() {
    for patch in [json!({"options":{"primary_axis":{"min":5,"max":4}}}), json!({"options":{"primary_axis":{"major_unit":0}}}), json!({"options":{"primary_axis":{"log_base":1}}}), json!({"options":{"primary_axis":{"log_base":10,"min":0}}}), json!({"options":{"category_axis":{"min":1}}}), json!({"options":{"secondary_axis":{"max":10}}}), json!({"kind":"bubble"}), json!({"kind":"combo"}), json!({"kind":"box_whisker"}), json!({"series":[{"name":"Invalid","color":"087F73","values":[2,4,6],"bubble_sizes":[1,2,3]}]}), json!({"series":[{"name":"Invalid","color":"087F73","values":[2,4,6],"trendline":{"kind":"polynomial","order":7}}]}), json!({"series":[{"name":"Invalid","color":"087F73","values":[2,4,6],"error_bars":{"kind":"custom","plus":[1],"minus":[1,2,3]}}]})] {
        let mut input = scene("column");
        for (key, value) in patch.as_object().unwrap() { input["slides"][0]["elements"][0][key] = value.clone(); }
        if let Ok(deck) = serde_json::from_value::<Deck>(input) { assert!(validate_deck(&deck).is_err(), "accepted {patch}"); }
    }
}

fn replace(document: &Value, path: &str, value: Value) -> aislide_core::Result<Value> {
    execute_request(json!({"op":"transaction","document":document,"transaction":{"expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":path,"value":value}]}}))
}

fn assert_workbook_caches(package: &Package, chart_path: &str, workbook_path: &str) {
    let chart = roxmltree::Document::parse(package.text(chart_path).unwrap()).unwrap();
    let workbook = Package::open(package.part(workbook_path).unwrap().to_vec()).unwrap();
    let sheet = roxmltree::Document::parse(workbook.text("xl/worksheets/sheet1.xml").unwrap()).unwrap();
    for reference in chart.descendants().filter(|node| ["strRef", "numRef"].contains(&node.tag_name().name())) {
        let formula = reference.children().find(|node| node.tag_name().name() == "f").unwrap().text().unwrap();
        let address = formula.strip_prefix("Sheet1!").unwrap().replace('$', "");
        let start = address.split(':').next().unwrap();
        let boundary = start.find(|character: char| character.is_ascii_digit()).unwrap();
        let (column, row) = start.split_at(boundary);
        let row: usize = row.parse().unwrap();
        for (offset, point) in reference.descendants().filter(|node| node.tag_name().name() == "pt").enumerate() {
            let expected = point.children().find(|node| node.tag_name().name() == "v").unwrap().text().unwrap();
            let cell_address = format!("{column}{}", row + offset);
            let cell = sheet.descendants().find(|node| node.tag_name().name() == "c" && node.attribute("r") == Some(&cell_address)).unwrap();
            let actual = cell.descendants().find(|node| ["t", "v"].contains(&node.tag_name().name())).unwrap().text().unwrap();
            assert_eq!(actual, expected, "{formula} / {cell_address}");
            if reference.tag_name().name() == "numRef" { assert_ne!(cell.attribute("t"), Some("inlineStr")); }
        }
    }
}

#[test]
fn all_auxiliary_caches_match_workbooks_and_edits_are_atomic() {
    let mut input = scene("bubble");
    for entry in input["slides"][0]["elements"][0]["series"].as_array_mut().unwrap() {
        entry["bubble_sizes"] = json!([0,9,16]);
        entry["error_bars"] = json!({"kind":"custom","direction":"x","plus":[1,2,3],"minus":[0,0.5,1]});
    }
    let original = package(input);
    assert_workbook_caches(&original, "ppt/charts/chart1.xml", "ppt/embeddings/chart1.xlsx");
    let document = open(&original);
    let changed = replace(&document, "/deck/slides/0/elements/0/series/1/bubble_sizes/1", json!(25)).unwrap();
    let exported = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let result = Package::open(STANDARD.decode(exported["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_workbook_caches(&result, "ppt/charts/chart2.xml", "ppt/embeddings/chart2.xlsx");
    let undo = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
    let restored = execute_request(json!({"op":"export_presentation","document":undo["document"]})).unwrap();
    assert_eq!(STANDARD.decode(restored["base64"].as_str().unwrap()).unwrap(), original.save().unwrap());
}

#[test]
fn original_ten_defaults_and_noop_are_preserved() {
    for kind in ["column","bar","line","pie","doughnut","area","scatter","stacked_column","stacked_bar","percent_stacked_column"] {
        let mut input = scene(kind);
        if ["pie", "doughnut"].contains(&kind) { input["slides"][0]["elements"][0]["series"].as_array_mut().unwrap().truncate(1); }
        let original = package(input.clone());
        input["slides"][0]["elements"][0]["options"] = json!({});
        assert_eq!(package(input).save().unwrap(), original.save().unwrap());
        let document = open(&original);
        assert!(document["deck"]["slides"][0]["elements"][0].get("options").is_none(), "{kind}");
        let output = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        assert_eq!(STANDARD.decode(output["base64"].as_str().unwrap()).unwrap(), original.save().unwrap());
        assert_workbook_caches(&original, "ppt/charts/chart1.xml", "ppt/embeddings/chart1.xlsx");
    }
}

#[test]
fn external_workbook_or_unmodeled_style_is_never_replaced() {
    for workbook_change in [true, false] {
        let mut original = package(scene("column"));
        if workbook_change {
            let mut workbook = Package::open(original.part("ppt/embeddings/chart1.xlsx").unwrap().to_vec()).unwrap();
            let xml = workbook.text("xl/worksheets/sheet1.xml").unwrap().to_owned();
            let parsed = roxmltree::Document::parse(&xml).unwrap();
            let value = parsed.descendants().find(|node| node.attribute("r") == Some("B2")).unwrap().children().find(|node| node.tag_name().name() == "v").unwrap().first_child().unwrap().range();
            let mut changed = xml.clone(); changed.replace_range(value, "99");
            workbook.replace_part("xl/worksheets/sheet1.xml", changed.into_bytes()).unwrap();
            original.replace_part("ppt/embeddings/chart1.xlsx", workbook.save().unwrap()).unwrap();
        } else {
            let xml = original.text("ppt/charts/chart1.xml").unwrap().replace("</c:chartSpace>", "<c:spPr><a:solidFill><a:srgbClr val=\"123456\"/></a:solidFill></c:spPr></c:chartSpace>");
            original.replace_part("ppt/charts/chart1.xml", xml.into_bytes()).unwrap();
        }
        let document = open(&original);
        let noop = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        assert_eq!(STANDARD.decode(noop["base64"].as_str().unwrap()).unwrap(), original.save().unwrap());
        assert!(replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(3)).is_err());
    }
}

#[test]
fn catalog_flags_match_actual_factory_capabilities() {
    let catalog = execute_request(json!({"op":"object_catalog"})).unwrap();
    let flags = catalog["chart_capabilities"].as_array().expect("chart capabilities for the parent UI");
    assert!(flags.iter().any(|entry| entry["id"] == "surface3d" && entry["native"] == false));
    for kind in ["histogram", "box_whisker", "treemap", "sunburst"] {
        assert!(flags.iter().any(|entry| entry["id"] == kind && entry["native"] == true && entry["create"] == true));
    }
    for kind in catalog["charts"].as_array().unwrap() {
        assert!(flags.iter().any(|entry| entry["id"] == *kind && entry["native"] == true && entry["create"] == true && entry["office_validated"] == false));
        let element = execute_request(json!({"op":"create_object","id":"chart-1","kind":"chart","preset":kind})).unwrap();
        let mut input = scene(kind.as_str().unwrap()); input["slides"][0]["elements"][0] = element;
        assert_eq!(open(&package(input))["deck"]["slides"][0]["elements"][0]["kind"], *kind);
    }
}

#[test]
fn effective_axes_and_degenerate_statistics_are_rejected() {
    for (kind, options, series_patch) in [
        ("column", json!({"primary_axis":{"max":-1}}), json!({})),
        ("line", json!({"primary_axis":{"log_base":10}}), json!({"values":[0,1,2]})),
        ("scatter", json!({}), json!({"trendline":{"kind":"linear"}})),
    ] {
        let mut input = scene(kind); let element = &mut input["slides"][0]["elements"][0];
        element["options"] = options;
        if kind == "scatter" { element["categories"] = json!(["1","1","1"]); }
        for (key, value) in series_patch.as_object().unwrap() { element["series"][0][key] = value.clone(); }
        let deck: Deck = serde_json::from_value(input).unwrap();
        assert!(validate_deck(&deck).is_err(), "{kind}");
    }
}

#[test]
fn histogram_derives_only_explicit_bin_counts() {
    use aislide_core::model::chart_format::histogram;
    let result = histogram(&[0.0, 0.5, 1.0, 1.0, 2.0, 3.0], &[0.0, 1.0, 2.0, 3.0]).unwrap();
    assert_eq!(result.sample_count, 6);
    assert_eq!(result.bins.iter().map(|bin| bin.count).collect::<Vec<_>>(), [2, 2, 2]);
    assert_eq!(result.categories, ["[0, 1)", "[1, 2)", "[2, 3]"]);
    assert_eq!(result.series[0].values, [2.0, 2.0, 2.0]);
    let mut input = scene("column");
    input["slides"][0]["elements"][0]["categories"] = json!(result.categories);
    input["slides"][0]["elements"][0]["series"] = json!(result.series);
    let exported = package(input);
    assert_workbook_caches(&exported, "ppt/charts/chart1.xml", "ppt/embeddings/chart1.xlsx");
    for (samples, edges) in [(vec![], vec![0.0,1.0]), (vec![1.0], vec![]), (vec![1.0], vec![0.0,0.0,2.0]), (vec![3.0], vec![0.0,2.0]), (vec![f64::NAN], vec![0.0,2.0]), (vec![1.0], vec![0.0,f64::INFINITY])] { assert!(histogram(&samples, &edges).is_err()); }
}

#[test]
fn trendline_and_errorbar_variants_keep_native_order_and_values() {
    for trend in [json!({"kind":"linear","intercept":0,"backward":1}), json!({"kind":"exponential"}), json!({"kind":"logarithmic"}), json!({"kind":"power"}), json!({"kind":"polynomial","order":2}), json!({"kind":"moving_average","period":2})] {
        for errors in [json!({"kind":"fixed_value","value":1}), json!({"kind":"percentage","value":5}), json!({"kind":"standard_deviation","value":2}), json!({"kind":"standard_error"}), json!({"kind":"custom","bar_type":"plus","plus":[1,0,2]})] {
            let mut input = scene("scatter");
            input["slides"][0]["elements"][0]["series"][0]["trendline"] = trend.clone();
            input["slides"][0]["elements"][0]["series"][0]["error_bars"] = errors.clone();
            let exported = package(input);
            let parsed = roxmltree::Document::parse(exported.text("ppt/charts/chart1.xml").unwrap()).unwrap();
            let series = parsed.descendants().find(|node| node.tag_name().name() == "ser").unwrap();
            let names: Vec<_> = series.children().filter(|node| node.is_element()).map(|node| node.tag_name().name()).collect();
            assert!(names.iter().position(|name| *name == "trendline") < names.iter().position(|name| *name == "errBars"));
            assert!(names.iter().position(|name| *name == "errBars") < names.iter().position(|name| *name == "xVal"));
            assert_workbook_caches(&exported, "ppt/charts/chart1.xml", "ppt/embeddings/chart1.xlsx");
            let document = open(&exported);
            assert_eq!(document["deck"]["slides"][0]["elements"][0]["series"][0]["trendline"]["kind"], trend["kind"]);
            assert_eq!(document["deck"]["slides"][0]["elements"][0]["series"][0]["error_bars"]["kind"], errors["kind"]);
            replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(8)).unwrap();
        }
    }
}

#[test]
fn combo_series_order_is_independent_of_group_and_axis_order() {
    let mut input = scene("combo");
    let series = input["slides"][0]["elements"][0]["series"].as_array_mut().unwrap();
    series[0]["kind"] = json!("column"); series[1]["kind"] = json!("line"); series[1]["axis"] = json!("secondary");
    series.push(json!({"name":"Third","kind":"column","values":[10,11,12],"color":"123456"}));
    let exported = package(input);
    let document = open(&exported);
    let restored = document["deck"]["slides"][0]["elements"][0]["series"].as_array().unwrap();
    assert_eq!(restored.iter().map(|entry| entry["name"].as_str().unwrap()).collect::<Vec<_>>(), ["First", "Second", "Third"]);
    assert_eq!(restored[1]["axis"], "secondary");
    assert!(restored[2].get("axis").is_none());
    assert_workbook_caches(&exported, "ppt/charts/chart1.xml", "ppt/embeddings/chart1.xlsx");
    let mut changed = exported.clone();
    let xml = changed.text("ppt/charts/chart1.xml").unwrap().to_owned();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    let plot = parsed.descendants().find(|node| node.tag_name().name() == "plotArea").unwrap();
    let axis_ranges: Vec<_> = plot.children().filter(|node| ["catAx", "valAx"].contains(&node.tag_name().name())).map(|node| node.range()).collect();
    let mut rewritten = xml.clone();
    for (target, source) in axis_ranges.iter().rev().zip(axis_ranges.iter()) { rewritten.replace_range(target.clone(), &xml[source.clone()]); }
    changed.replace_part("ppt/charts/chart1.xml", rewritten.into_bytes()).unwrap();
    assert_eq!(open(&changed)["deck"]["slides"][0]["elements"][0]["series"], document["deck"]["slides"][0]["elements"][0]["series"]);
}

#[test]
fn direct_nonfinite_options_and_series_never_validate() {
    use aislide_core::model::{Element, chart_format::{ErrorBars, ErrorBarKind, ErrorBarDirection, ErrorBarType, Trendline, TrendlineKind}};
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e16] {
        for field in ["axis", "trend", "error", "size"] {
            let mut input = scene(if field == "size" { "bubble" } else { "line" });
            if field == "size" { for entry in input["slides"][0]["elements"][0]["series"].as_array_mut().unwrap() { entry["bubble_sizes"] = json!([1,2,3]); } }
            let mut deck: Deck = serde_json::from_value(input).unwrap();
            let Element::Chart { options, series, .. } = &mut deck.slides[0].elements[0] else { unreachable!() };
            match field {
                "axis" => options.primary_axis.min = Some(value),
                "trend" => series[0].trendline = Some(Trendline { kind: TrendlineKind::Linear, order: None, period: None, intercept: Some(value), forward: None, backward: None, display_equation: false, display_r_squared: false }),
                "error" => series[0].error_bars = Some(ErrorBars { kind: ErrorBarKind::FixedValue, direction: ErrorBarDirection::Y, bar_type: ErrorBarType::Both, value: Some(value), plus: None, minus: None }),
                "size" => series[0].bubble_sizes.as_mut().unwrap()[0] = value,
                _ => unreachable!(),
            }
            assert!(validate_deck(&deck).is_err(), "{field} {value}");
        }
    }
}

#[test]
fn native_labels_legends_log_axes_and_percent_totals() {
    for legend in ["bottom", "top", "left", "right", "top_right", "hidden"] {
        let mut input = scene("scatter");
        input["slides"][0]["elements"][0]["options"] = json!({"legend":legend,"primary_axis":{"log_base":10,"min":1,"max":100,"reverse":true,"number_format":"0.00"},"category_axis":{"log_base":2,"min":1,"max":8,"number_format":"0.0"},"data_labels":{"show_value":true,"show_series_name":true,"position":"center","number_format":"0.0"}});
        let original = package(input);
        let document = open(&original);
        let options = &document["deck"]["slides"][0]["elements"][0]["options"];
        assert_eq!(options["primary_axis"]["log_base"].as_f64(), Some(10.0));
        assert_eq!(options["category_axis"]["log_base"].as_f64(), Some(2.0));
        assert_eq!(options["primary_axis"]["reverse"], true);
        assert_eq!(options["data_labels"]["show_value"], true);
        if legend != "bottom" { assert_eq!(options["legend"], legend); }
        replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(8)).unwrap();
    }
    for kind in ["percent_stacked_column", "percent_stacked_bar"] {
        for values in [[0,0,0], [-1,2,3]] {
            let mut input = scene(kind);
            for entry in input["slides"][0]["elements"][0]["series"].as_array_mut().unwrap() { entry["values"] = json!(values); }
            assert!(validate_deck(&serde_json::from_value(input).unwrap()).is_err());
        }
    }
    for kind in ["pie", "doughnut", "pie3d"] {
        let mut input = scene(kind);
        input["slides"][0]["elements"][0]["series"].as_array_mut().unwrap().truncate(1);
        input["slides"][0]["elements"][0]["options"] = json!({"data_labels":{"show_percent":true,"position":"center"}});
        let original = package(input);
        assert_eq!(open(&original)["deck"]["slides"][0]["elements"][0]["options"]["data_labels"]["show_percent"], true);
    }
}

#[test]
#[ignore = "writes a synthetic fixture only when AISLIDE_CHART_SCHEMA_DIR is explicitly supplied"]
fn offline_schema_fixture() {
    use std::io::Write;
    let directory = std::env::var("AISLIDE_CHART_SCHEMA_DIR").expect("set a new fixture directory");
    let mut deck: Deck = serde_json::from_value(scene("column")).unwrap();
    deck.slides.clear();
    for kind in aislide_core::model::chart_format::KINDS.iter().filter(|kind| !kind.is_extended()) {
        let kind = serde_json::to_value(kind).unwrap();
        let mut element = execute_request(json!({"op":"create_object","id":"chart-1","kind":"chart","preset":kind})).unwrap();
        element["options"] = json!({"legend":"right","data_labels":{"show_value":true,"show_category_name":true,"position":"center"}});
        if !["pie", "doughnut", "pie3d"].contains(&kind.as_str().unwrap()) { element["options"]["category_axis"] = json!({"reverse":true}); }
        if kind == "combo" {
            element["options"]["primary_axis"] = json!({"min":0,"max":30,"major_unit":5,"minor_unit":1,"number_format":"0.0"});
            element["options"]["secondary_axis"] = json!({"min":1,"max":100,"log_base":10});
            element["series"][1]["trendline"] = json!({"kind":"polynomial","order":2,"display_equation":true,"display_r_squared":true,"forward":1,"intercept":0});
            element["series"][1]["error_bars"] = json!({"kind":"custom","plus":[1,2,3,4],"minus":[0,1,2,3]});
        }
        if kind == "bubble" { element["series"][0]["error_bars"] = json!({"kind":"standard_deviation","value":2,"direction":"x"}); }
        let mut input = scene(kind.as_str().unwrap()); input["slides"][0]["elements"][0] = element;
        input["slides"][0]["id"] = json!(format!("slide-{}", deck.slides.len() + 1));
        let next: Deck = serde_json::from_value(input).unwrap(); deck.slides.extend(next.slides);
    }
    let output = std::path::Path::new(&directory).join("native-chart-controls.pptx");
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(output).unwrap();
    file.write_all(&export_pptx(&deck).unwrap()).unwrap();
}

#[test]
fn options_only_native_edit_replaces_chart_without_changing_workbook_values() {
    let mut input = scene("line");
    input["slides"][0]["elements"][0]["options"] = json!({"legend":"left"});
    let original = package(input);
    let document = open(&original);
    let changed = replace(&document, "/deck/slides/0/elements/0/options", json!({"legend":"top","primary_axis":{"min":-5,"max":50,"major_unit":5},"data_labels":{"show_value":true,"position":"center"}})).unwrap();
    let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
    let updated = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    assert_workbook_caches(&updated, "ppt/charts/chart2.xml", "ppt/embeddings/chart2.xlsx");
    assert_eq!(updated.part("ppt/embeddings/chart2.xlsx").unwrap(), original.part("ppt/embeddings/chart1.xlsx").unwrap());
    let options = &open(&updated)["deck"]["slides"][0]["elements"][0]["options"];
    assert_eq!(options["legend"], "top");
    assert_eq!(options["primary_axis"]["max"].as_f64(), Some(50.0));
    assert_eq!(options["data_labels"]["show_value"], true);
}

fn extended_scene(kind: &str) -> Value {
    let mut input = scene(kind);
    let chart = &mut input["slides"][0]["elements"][0];
    chart["categories"] = json!(["Start & one", "Change <two>", "End"]);
    chart["series"] = json!([{"name":"Synthetic & native","color":"087F73","values":[120,80,40]}]);
    if kind == "waterfall" {
        chart["series"][0]["values"] = json!([120,-80,40]);
        chart["options"] = json!({"waterfall_totals":[0,2]});
    }
    input
}

fn assert_extended_caches(package: &Package, chart_path: &str, workbook_path: &str) {
    let chart = roxmltree::Document::parse(package.text(chart_path).unwrap()).unwrap();
    let workbook = Package::open(package.part(workbook_path).unwrap().to_vec()).unwrap();
    let sheet = roxmltree::Document::parse(workbook.text("xl/worksheets/sheet1.xml").unwrap()).unwrap();
    for dimension in chart.descendants().filter(|node| ["strDim", "numDim", "txData"].contains(&node.tag_name().name())) {
        let formula = dimension.children().find(|node| node.tag_name().name() == "f").unwrap().text().unwrap();
        let address = formula.strip_prefix("Sheet1!").unwrap().replace('$', "");
        let start = address.split(':').next().unwrap();
        let boundary = start.find(|character: char| character.is_ascii_digit()).unwrap();
        let (column, row) = start.split_at(boundary);
        let row: usize = row.parse().unwrap();
        for (offset, point) in dimension.descendants().filter(|node| ["pt", "v"].contains(&node.tag_name().name())).enumerate() {
            let address = format!("{column}{}", row + offset);
            let cell = sheet.descendants().find(|node| node.attribute("r") == Some(&address)).unwrap();
            assert_eq!(cell.descendants().find(|node| ["t", "v"].contains(&node.tag_name().name())).unwrap().text(), point.text(), "{formula}");
            if dimension.tag_name().name() == "numDim" { assert_ne!(cell.attribute("t"), Some("inlineStr")); }
        }
    }
}

#[test]
fn extended_reference_is_direct_graphic_data() {
    for kind in ["funnel", "waterfall"] {
        let original = package(extended_scene(kind));
        let slide = roxmltree::Document::parse(original.text("ppt/slides/slide1.xml").unwrap()).unwrap();
        let chart = roxmltree::Document::parse(original.text("ppt/charts/chart1.xml").unwrap()).unwrap();
        assert!(!chart.descendants().any(|node| node.has_tag_name(("http://schemas.microsoft.com/office/drawing/2014/chartex", "axisId"))), "funnel/waterfall use implicit axes, not series axis references");
        let reference = slide.descendants().find(|node| node.has_tag_name(("http://schemas.microsoft.com/office/drawing/2014/chartex", "chart"))).unwrap();
        let data = reference.parent_element().unwrap();
        assert!(data.has_tag_name(("http://schemas.openxmlformats.org/drawingml/2006/main", "graphicData")));
        assert!(data.parent_element().unwrap().has_tag_name(("http://schemas.openxmlformats.org/drawingml/2006/main", "graphic")), "chartEx must not put AlternateContent inside a:graphic");
        assert_eq!(data.attribute("uri"), Some("http://schemas.microsoft.com/office/drawing/2014/chartex"));
    }
}

#[test]
fn extended_charts_have_style_and_color_resources_after_native_edit() {
    for kind in ["funnel", "waterfall"] {
        let original = package(extended_scene(kind));
        let changed = replace(&open(&original), "/deck/slides/0/elements/0/series/0/values/0", json!(140)).unwrap();
        let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        let edited = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
        for (package, number) in [(&original, 1), (&edited, 2)] {
            let rels = roxmltree::Document::parse(package.text(&format!("ppt/charts/_rels/chart{number}.xml.rels")).unwrap()).unwrap();
            let types = roxmltree::Document::parse(package.text("[Content_Types].xml").unwrap()).unwrap();
            for (kind, root, mime) in [("chartStyle", "chartStyle", "application/vnd.ms-office.chartstyle+xml"), ("chartColorStyle", "colorStyle", "application/vnd.ms-office.chartcolorstyle+xml")] {
                let relation = format!("http://schemas.microsoft.com/office/2011/relationships/{kind}");
                let target = rels.descendants().find(|node| node.attribute("Type") == Some(&relation)).expect("chartEx requires both native style relationships").attribute("Target").unwrap();
                let path = format!("ppt/charts/{target}");
                let style = roxmltree::Document::parse(package.text(&path).unwrap()).unwrap();
                assert!(style.root_element().has_tag_name(("http://schemas.microsoft.com/office/drawing/2012/chartStyle", root)));
                assert!(types.descendants().any(|node| node.attribute("PartName") == Some(&format!("/{path}")) && node.attribute("ContentType") == Some(mime)));
            }
        }
    }
}

#[test]
fn extended_charts_roundtrip_edit_and_undo_without_fabricated_values() {
    for kind in ["funnel", "waterfall"] {
        let original = package(extended_scene(kind));
        assert_extended_caches(&original, "ppt/charts/chart1.xml", "ppt/embeddings/chart1.xlsx");
        let document = open(&original);
        let element = &document["deck"]["slides"][0]["elements"][0];
        assert_eq!(element["kind"], kind);
        assert_eq!(element["series"][0]["name"], "Synthetic & native");
        assert_eq!(element["categories"][1], "Change <two>");
        if kind == "waterfall" { assert_eq!(element["options"]["waterfall_totals"], json!([0,2])); }
        let noop = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        assert_eq!(STANDARD.decode(noop["base64"].as_str().unwrap()).unwrap(), original.save().unwrap());
        let changed = replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(140)).unwrap();
        let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        let edited = Package::open(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
        assert_extended_caches(&edited, "ppt/charts/chart2.xml", "ppt/embeddings/chart2.xlsx");
        assert_eq!(open(&edited)["deck"]["slides"][0]["elements"][0]["series"][0]["values"][0].as_f64(), Some(140.0));
        for (path, bytes) in original.parts() {
            if path != "[Content_Types].xml" && !path.starts_with("ppt/slides/") && !path.starts_with("customXml/") {
                assert_eq!(edited.part(path).unwrap(), bytes, "unchanged resource {path}");
            }
        }
        let undo = execute_request(json!({"op":"undo_transaction","document":changed["document"],"expected_revision":changed["document"]["revision"],"receipt":changed["receipt"]})).unwrap();
        let restored = execute_request(json!({"op":"export_presentation","document":undo["document"]})).unwrap();
        assert_eq!(STANDARD.decode(restored["base64"].as_str().unwrap()).unwrap(), original.save().unwrap());
    }
}

#[test]
fn extended_charts_reject_invalid_values_options_and_implicit_totals() {
    for (kind, patch) in [
        ("funnel", json!({"series":[{"name":"Bad","color":"087F73","values":[3,0,1]}]})),
        ("funnel", json!({"series":[{"name":"Bad","color":"087F73","values":[3,-1,1]}]})),
        ("funnel", json!({"options":{"waterfall_totals":[]}})),
        ("funnel", json!({"options":{"data_labels":{"show_value":true}}})),
        ("waterfall", json!({"options":{}})),
        ("waterfall", json!({"options":{"waterfall_totals":[3]}})),
        ("waterfall", json!({"options":{"waterfall_totals":[2,0]}})),
        ("waterfall", json!({"options":{"waterfall_totals":[0,0]}})),
        ("waterfall", json!({"options":{"waterfall_totals":[],"primary_axis":{"max":10}}})),
        ("column", json!({"options":{"waterfall_totals":[]}})),
    ] {
        let mut input = extended_scene(kind);
        for (key, value) in patch.as_object().unwrap() { input["slides"][0]["elements"][0][key] = value.clone(); }
        let deck: Deck = serde_json::from_value(input).unwrap();
        assert!(validate_deck(&deck).is_err(), "{kind}: {patch}");
    }
    let mut input = extended_scene("waterfall");
    input["slides"][0]["elements"][0]["options"]["waterfall_totals"] = json!([]);
    assert_eq!(open(&package(input))["deck"]["slides"][0]["elements"][0]["options"]["waterfall_totals"], json!([]));
}

#[test]
fn extended_unknown_xml_and_independently_edited_workbooks_are_preserved() {
    for kind in ["funnel", "waterfall"] {
        for workbook_change in [true, false] {
            let mut original = package(extended_scene(kind));
            if workbook_change {
                let mut workbook = Package::open(original.part("ppt/embeddings/chart1.xlsx").unwrap().to_vec()).unwrap();
                let sheet = workbook.text("xl/worksheets/sheet1.xml").unwrap().replace("<v>120</v>", "<v>999</v>");
                workbook.replace_part("xl/worksheets/sheet1.xml", sheet.into_bytes()).unwrap();
                original.replace_part("ppt/embeddings/chart1.xlsx", workbook.save().unwrap()).unwrap();
            } else {
                let chart = original.text("ppt/charts/chart1.xml").unwrap().replace("</cx:chartSpace>", "<cx:extLst><cx:ext uri=\"urn:synthetic-test\"><unknown xmlns=\"urn:synthetic-test\">Keep me</unknown></cx:ext></cx:extLst></cx:chartSpace>");
                original.replace_part("ppt/charts/chart1.xml", chart.into_bytes()).unwrap();
            }
            let document = open(&original);
            let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
            assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), original.save().unwrap());
            assert!(replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(90)).is_err());
        }
    }
}

#[test]
fn extended_custom_style_resources_are_not_silently_replaced() {
    for kind in ["funnel", "waterfall"] {
        for mutation in ["style", "colors", "extra_relationship"] {
            let mut original = package(extended_scene(kind));
            let (path, value) = match mutation {
                "style" => ("ppt/charts/style1.xml", original.text("ppt/charts/style1.xml").unwrap().replacen("sz=\"1400\"", "sz=\"1800\"", 1)),
                "colors" => ("ppt/charts/colors1.xml", original.text("ppt/charts/colors1.xml").unwrap().replacen("val=\"accent1\"", "val=\"accent6\"", 1)),
                _ => ("ppt/charts/_rels/chart1.xml.rels", original.text("ppt/charts/_rels/chart1.xml.rels").unwrap().replace("</Relationships>", "<Relationship Id=\"custom\" Type=\"urn:synthetic-test\" Target=\"style1.xml\"/></Relationships>")),
            };
            original.replace_part(path, value.into_bytes()).unwrap();
            let document = open(&original);
            let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
            assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), original.save().unwrap());
            assert!(replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(90)).is_err(), "{kind} {mutation}");
        }
    }
}

#[test]
fn extended_invalid_native_caches_and_references_stay_opaque() {
    for (old, new) in [("ptCount=\"3\"", "ptCount=\"33\""), ("idx=\"1\"", "idx=\"0\""), ("<cx:dataId val=\"0\"", "<cx:dataId val=\"9\""), ("type=\"val\"", "type=\"size\""), ("layoutId=\"funnel\"", "layoutId=\"treemap\""), (">120</cx:pt>", ">NaN</cx:pt>")] {
        let mut original = package(extended_scene("funnel"));
        let chart = original.text("ppt/charts/chart1.xml").unwrap();
        assert!(chart.contains(old), "fixture contains {old}");
        let chart = chart.replacen(old, new, 1);
        original.replace_part("ppt/charts/chart1.xml", chart.into_bytes()).unwrap();
        let document = open(&original);
        assert_eq!(document["deck"]["slides"][0]["elements"].as_array().unwrap().len(), 0, "{new}");
        let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), original.save().unwrap());
    }
}

#[test]
#[ignore = "writes new synthetic chartEx packages only to an explicitly supplied fixture directory"]
fn extended_offline_schema_fixture() {
    use std::io::Write;
    let directory = std::env::var_os("AISLIDE_CHARTEX_SCHEMA_DIR").expect("explicit temporary fixture directory");
    for kind in ["funnel", "waterfall"] {
        let path = std::path::Path::new(&directory).join(format!("native-{kind}.pptx"));
        let mut input = extended_scene(kind);
        input["slides"][0]["elements"].as_array_mut().unwrap().push(json!({"type":"text","id":"title","x":64,"y":24,"width":1100,"height":60,"text":format!("Synthetic native {kind}"),"font_size":28,"color":"111111","bold":false}));
        let original = package(input);
        std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap().write_all(&original.save().unwrap()).unwrap();
        let document = open(&original);
        let changed = replace(&document, "/deck/slides/0/elements/0/series/0/values/0", json!(140)).unwrap();
        let saved = execute_request(json!({"op":"export_presentation","document":changed["document"]})).unwrap();
        let path = std::path::Path::new(&directory).join(format!("edited-{kind}.pptx"));
        std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap().write_all(&STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap()).unwrap();
    }
}

#[test]
fn extended_waterfall_axis_identities_and_unmodeled_scaling_stay_opaque() {
    for mutation in ["axis_reference", "duplicate_axis", "scaling", "data_labels"] {
        let mut original = package(extended_scene("waterfall"));
        let source = original.text("ppt/charts/chart1.xml").unwrap().to_owned();
        let parsed = roxmltree::Document::parse(&source).unwrap();
        let mut changed = source.clone();
        match mutation {
            "axis_reference" => {
                let node = parsed.descendants().find(|node| node.tag_name().name() == "dataId").unwrap();
                changed.insert_str(node.range().end, "<cx:axisId>9</cx:axisId>");
            }
            "duplicate_axis" => {
                let node = parsed.descendants().filter(|node| node.tag_name().name() == "axis").nth(1).unwrap();
                changed.replace_range(node.attributes().find(|attribute| attribute.name() == "id").unwrap().range_value(), "0");
            }
            "scaling" => {
                let node = parsed.descendants().find(|node| node.tag_name().name() == "valScaling").unwrap();
                changed.replace_range(node.range(), "<cx:valScaling min=\"25\"/>");
            }
            _ => {
                let node = parsed.descendants().find(|node| node.tag_name().name() == "dataId").unwrap();
                changed.insert_str(node.range().start, "<cx:dataLabels><cx:visibility value=\"1\"/></cx:dataLabels>");
            }
        }
        original.replace_part("ppt/charts/chart1.xml", changed.into_bytes()).unwrap();
        let document = open(&original);
        assert!(document["deck"]["slides"][0]["elements"].as_array().unwrap().is_empty(), "{mutation}");
        let saved = execute_request(json!({"op":"export_presentation","document":document})).unwrap();
        assert_eq!(STANDARD.decode(saved["base64"].as_str().unwrap()).unwrap(), original.save().unwrap());
    }
}

#[test]
fn phase3_shared_projection_exact_regression_and_error_endpoints() {
    use aislide_core::model::{ChartKind, ChartSeries, ChartOptions, chart_format::compute_chart_presentation};
    let categories = vec!["1".into(), "2".into(), "3".into()];
    let series: Vec<ChartSeries> = serde_json::from_value(json!([{"name":"Measured","values":[3,5,7],"color":"087F73","trendline":{"kind":"linear","forward":1,"display_equation":true,"display_r_squared":true},"error_bars":{"kind":"fixed_value","value":1}}])).unwrap();
    let before = serde_json::to_vec(&series).unwrap();
    let result = compute_chart_presentation(ChartKind::Line, &categories, &series, &ChartOptions::default()).unwrap();
    let trend = result.series[0].trend.as_ref().unwrap();
    let slope = trend.coefficients[1] / trend.scale;
    assert!((slope - 2.0).abs() < 1e-10);
    assert!((trend.coefficients[0] - slope * trend.center - 1.0).abs() < 1e-10);
    assert!((trend.points.last().unwrap().y - 9.0).abs() < 1e-10);
    assert_eq!(trend.points.last().unwrap().x, 4.0);
    assert_eq!(trend.r_squared, Some(1.0));
    assert!(result.primary_axis.ticks.iter().all(|tick| tick.label.len() < 12));
    assert!(trend.points.len() <= 256);
    assert_eq!(result.series[0].errors[0].lower.y, 2.0);
    assert_eq!(result.series[0].errors[0].upper.y, 4.0);
    assert_eq!(serde_json::to_vec(&series).unwrap(), before);
}

fn presentation(input: Value) -> aislide_core::Result<Value> {
    let element = &input["slides"][0]["elements"][0];
    execute_request(json!({"op":"compute_chart_presentation","kind":element["kind"],"categories":element["categories"],"series":element["series"],"options":element.get("options").cloned().unwrap_or(json!({}))}))
}

#[test]
fn phase3_six_regressions_domains_rank_and_budgets() {
    for kind in ["linear", "logarithmic", "exponential", "power", "polynomial", "moving_average"] {
        let mut input = scene("scatter");
        input["slides"][0]["elements"][0]["series"] = json!([{"name":"Observations","color":"087F73","values":[3.0,5.0,7.0]}]);
        let entry = &mut input["slides"][0]["elements"][0]["series"][0];
        entry["trendline"] = json!({"kind":kind});
        if kind == "polynomial" { entry["trendline"]["order"] = json!(2); entry["values"] = json!([1,4,9]); }
        if kind == "moving_average" { entry["trendline"]["period"] = json!(2); }
        if kind == "exponential" { entry["values"] = json!([2.0f64.exp(),4.0f64.exp(),6.0f64.exp()]); }
        if kind == "power" { entry["values"] = json!([2,8,18]); }
        if kind == "logarithmic" { entry["values"] = json!([1.0,1.0+2.0*2.0f64.ln(),1.0+2.0*3.0f64.ln()]); }
        let result = presentation(input.clone()).unwrap();
        let trend = &result["series"][0]["trend"];
        assert!(trend["points"].as_array().unwrap().len() <= 256, "{kind}");
        if kind == "moving_average" { assert_eq!(trend["points"], json!([{"x":2.0,"y":4.0},{"x":3.0,"y":6.0}])); }
        else { assert!((trend["r_squared"].as_f64().unwrap() - 1.0).abs() < 1e-10, "{kind}"); }
        if matches!(kind, "power" | "exponential") {
            let mut invalid = input.clone(); invalid["slides"][0]["elements"][0]["series"][0]["values"][0] = json!(-1);
            assert!(presentation(invalid).is_err());
        }
        if kind != "moving_average" {
            let mut invalid = input.clone(); invalid["slides"][0]["elements"][0]["categories"] = json!(["1","1","1"]);
            assert!(presentation(invalid).is_err());
            let mut invalid = input.clone(); invalid["slides"][0]["elements"][0]["series"][0]["trendline"]["forward"] = json!(100001);
            assert!(presentation(invalid).is_err());
        }
        if matches!(kind, "power" | "logarithmic") {
            let mut invalid = input.clone(); invalid["slides"][0]["elements"][0]["series"][0]["trendline"]["backward"] = json!(1);
            assert!(presentation(invalid).is_err());
        }
    }
    for order in 2..=6 {
        let mut input = scene("scatter");
        input["slides"][0]["elements"][0]["categories"] = json!((1..=8).map(|value| value.to_string()).collect::<Vec<_>>());
        input["slides"][0]["elements"][0]["series"] = json!([{"name":"Polynomial","values":(1..=8).map(|value| (value as f64).powi(order)).collect::<Vec<_>>(),"color":"087F73","trendline":{"kind":"polynomial","order":order,"intercept":0}}]);
        assert!((presentation(input).unwrap()["series"][0]["trend"]["r_squared"].as_f64().unwrap() - 1.0).abs() < 1e-9);
    }
    let mut fixed = scene("line");
    fixed["slides"][0]["elements"][0]["series"] = json!([{"name":"Fixed","values":[2,4,8],"color":"087F73","trendline":{"kind":"exponential","intercept":1}}]);
    assert!(presentation(fixed.clone()).is_ok());
    fixed["slides"][0]["elements"][0]["series"][0]["trendline"]["intercept"] = json!(0);
    assert!(presentation(fixed).is_err());
    let mut residual = scene("scatter");
    residual["slides"][0]["elements"][0]["series"] = json!([{"name":"Residual","color":"087F73","values":[1,3,2],"trendline":{"kind":"linear"}}]);
    assert!((presentation(residual.clone()).unwrap()["series"][0]["trend"]["r_squared"].as_f64().unwrap()-0.25).abs()<1e-10);
    residual["slides"][0]["elements"][0]["series"][0]["values"]=json!([5,5,5]);
    assert!(presentation(residual.clone()).unwrap()["series"][0]["trend"]["r_squared"].is_null());
    residual["slides"][0]["elements"][0]["categories"]=json!(["1000000000000","1000000000000.001","1000000000000.002"]);
    assert!(presentation(residual).is_err());
}

#[test]
fn phase3_absolute_errors_combo_axes_formats_and_native_bytes() {
    use aislide_core::model::chart_format::format_chart_number;
    assert_eq!(format_chart_number(1234.5,Some("#,##0.00")),("1,234.50".into(),true));
    assert_eq!(format_chart_number(0.125,Some("0.0%")),("12.5%".into(),true));
    assert_eq!(format_chart_number(1234.5,Some("0.00E+00")),("1.23E+03".into(),true));
    let mut input = scene("scatter");
    input["slides"][0]["elements"][0]["series"] = json!([{"name":"Observed","values":[3,5,7],"color":"087F73"}]);
    for (config, low, high) in [(json!({"kind":"fixed_value","value":1}),2.0,4.0),(json!({"kind":"percentage","value":10}),2.7,3.3),(json!({"kind":"standard_deviation","value":1}),3.0,7.0),(json!({"kind":"standard_error"}),3.0-2.0/3.0f64.sqrt(),3.0+2.0/3.0f64.sqrt()),(json!({"kind":"custom","plus":[2,3,4],"minus":[1,2,3]}),2.0,5.0),(json!({"kind":"fixed_value","value":1,"bar_type":"plus"}),3.0,4.0),(json!({"kind":"fixed_value","value":1,"bar_type":"minus"}),2.0,3.0)] {
        input["slides"][0]["elements"][0]["series"][0]["error_bars"] = config;
        let original = package(input.clone());
        let original_bytes = original.save().unwrap();
        let model = presentation(input.clone()).unwrap();
        let error = &model["series"][0]["errors"][0];
        assert!((error["lower"]["y"].as_f64().unwrap()-low).abs()<1e-10);
        assert!((error["upper"]["y"].as_f64().unwrap()-high).abs()<1e-10);
        assert_eq!(original.save().unwrap(),original_bytes);
        assert_eq!(package(input.clone()).part("ppt/embeddings/chart1.xlsx").unwrap(), original.part("ppt/embeddings/chart1.xlsx").unwrap());
    }
    input["slides"][0]["elements"][0]["series"][0]["error_bars"] = json!({"kind":"fixed_value","direction":"x","value":0.5});
    let model = presentation(input.clone()).unwrap();
    assert_eq!(model["series"][0]["errors"][0]["lower"]["x"],0.5);
    assert_eq!(model["series"][0]["errors"][0]["upper"]["x"],1.5);
    input["slides"][0]["elements"][0]["options"] = json!({"category_axis":{"min":0.25,"max":4,"log_base":2,"reverse":true},"primary_axis":{"min":1,"max":10,"major_unit":1,"minor_unit":0.5,"number_format":"0.00"}});
    let model = presentation(input.clone()).unwrap();
    assert_eq!(model["category_axis"]["domain"],json!([0.25,4.0]));
    assert_eq!(model["category_axis"]["ticks"][0]["position"],1.0);
    assert_eq!(model["primary_axis"]["ticks"][0]["label"],"1.00");
    input["slides"][0]["elements"][0]["options"]["primary_axis"]["number_format"] = json!("yyyy/mm/dd");
    assert!(presentation(input.clone()).unwrap()["warnings"].as_array().unwrap().iter().any(|warning|warning.as_str().unwrap().contains("Unsupported number format")));
    input["slides"][0]["elements"][0]["options"]["primary_axis"]["major_unit"] = json!(0.00001);
    assert!(presentation(input).is_err());
    let mut combo=scene("combo");
    combo["slides"][0]["elements"][0]["series"][0]["kind"]=json!("column");
    combo["slides"][0]["elements"][0]["series"][1]["kind"]=json!("line");
    combo["slides"][0]["elements"][0]["series"][1]["axis"]=json!("secondary");
    combo["slides"][0]["elements"][0]["series"][1]["values"]=json!([100,1000,10000]);
    combo["slides"][0]["elements"][0]["options"]=json!({"secondary_axis":{"log_base":10,"reverse":true}});
    let model=presentation(combo).unwrap();
    assert_eq!(model["primary_axis"]["domain"],json!([0.0,6.0]));
    assert_eq!(model["secondary_axis"]["domain"],json!([100.0,10000.0]));
}

#[test]
fn phase3_typed_preview_read_only_and_untrusted_input_rejection() {
    let mut element=scene("line")["slides"][0]["elements"][0].clone();
    element["series"][0]["trendline"]=json!({"kind":"linear","forward":1});
    let original=element.clone();
    let output=execute_request(json!({"op":"render_element_preview","element":element})).unwrap();
    assert!(output["svg"].as_str().unwrap().contains("stroke-dasharray"));
    assert_eq!(element,original);
    assert_eq!(output["office_parity_verified"],false);
    assert!(execute_request(json!({"op":"render_element_preview","element":element,"svg":"<script/>"})).is_err());
}