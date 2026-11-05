use aislide_core::{authoring_ops::{export_template, TemplateKind}, authoring_preflight::{preflight_presentation, PreflightOptions}, design::Design, document::{self, Document, Transaction}, editing, execute_request, package::Package};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const MASTER: &str = "ppt/slideMasters/slideMaster1.xml";
const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";
const FADE: &str = "<p:transition spd=\"med\"><p:fade/></p:transition>";
const PUSH: &str = "<p:transition spd=\"slow\"><p:push dir=\"u\"/></p:transition>";
const NONE: &str = "<p:transition/>";
// A sound action starts a sound (stSnd, with the embedded audio it plays) or stops the one that is playing (endSnd).
const SOUND_START: &str = "<p:transition><p:sndAc><p:stSnd><p:snd xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:embed=\"rIdChime\" name=\"chime.wav\"/></p:stSnd></p:sndAc></p:transition>";
const SOUND_STOP: &str = "<p:transition><p:sndAc><p:endSnd/></p:sndAc></p:transition>";
// An entrance effect: a set behavior that makes the shape visible.
const TIMING: &str = "<p:timing><p:tnLst><p:par><p:cTn id=\"1\" dur=\"indefinite\" restart=\"never\" nodeType=\"tmRoot\"><p:childTnLst><p:set><p:cBhvr><p:cTn id=\"2\" dur=\"1\" fill=\"hold\"/><p:tgtEl><p:spTgt spid=\"2\"/></p:tgtEl><p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val=\"visible\"/></p:to></p:set></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>";
// The root node alone, which PowerPoint can leave behind, defines no animation.
const ROOT_ONLY: &str = "<p:timing><p:tnLst><p:par><p:cTn id=\"1\" dur=\"indefinite\" restart=\"never\" nodeType=\"tmRoot\"/></p:par></p:tnLst></p:timing>";

fn layout(number: usize) -> String { format!("ppt/slideLayouts/slideLayout{number}.xml") }

fn alternate() -> String {
    format!("<mc:AlternateContent xmlns:mc=\"{MC}\"><mc:Choice xmlns:p14=\"{P14}\" Requires=\"p14\"><p:transition spd=\"slow\" p14:dur=\"2000\"><p14:vortex dir=\"r\"/></p:transition></mc:Choice><mc:Fallback><p:transition spd=\"slow\"><p:fade/></p:transition></mc:Fallback></mc:AlternateContent>")
}

fn source() -> Document {
    let mut deck = editing::create("effects".into(), "Effects".into()).unwrap().deck;
    deck.design = Some(Design::default());
    document::create("effects".into(), deck, vec![], vec![], None).unwrap()
}

fn text(parts: &BTreeMap<String, Vec<u8>>, path: &str) -> String { String::from_utf8(parts[path].clone()).unwrap() }

/// Puts `fragment` where the part's root allows it: after the color-map override when there is one, else after `cSld`.
fn set(parts: &mut BTreeMap<String, Vec<u8>>, path: &str, fragment: &str) {
    let xml = text(parts, path);
    let marker = ["</p:sldLayoutIdLst>", "</p:clrMapOvr>", "</p:cSld>"].into_iter().find(|marker| xml.contains(marker)).unwrap();
    parts.insert(path.into(), xml.replacen(marker, &format!("{marker}{fragment}"), 1).into_bytes());
}

/// Adds an image part and the relationship that references it.
fn reference(parts: &mut BTreeMap<String, Vec<u8>>, owner: &str, image: &str, bytes: usize) {
    parts.entry(image.into()).or_insert_with(|| vec![0x89; bytes]);
    let (folder, name) = owner.rsplit_once('/').unwrap();
    let relations = format!("{folder}/_rels/{name}.rels");
    let xml = text(parts, &relations);
    let target = format!("../media/{}", image.rsplit('/').next().unwrap());
    let id = format!("rIdImage{}", xml.matches("<Relationship ").count() + 1);
    parts.insert(relations, xml.replace("</Relationships>", &format!("<Relationship Id=\"{id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"{target}\"/></Relationships>")).into_bytes());
    let types = text(parts, "[Content_Types].xml");
    if !types.contains("Extension=\"png\"") { parts.insert("[Content_Types].xml".into(), types.replace("</Types>", "<Default Extension=\"png\" ContentType=\"image/png\"/></Types>").into_bytes()); }
}

/// A synthetic template: the master sets a fade and an animation, the layouts set a push, an explicit no-effect transition, and a
/// PowerPoint 2010 vortex with a fade fallback; the first layout sets nothing and the sample slide auto-advances after a dissolve.
fn template() -> Vec<u8> {
    let bytes = export_template(&source(), TemplateKind::Potx).unwrap();
    let mut parts = Package::open(bytes).unwrap().parts().clone();
    set(&mut parts, MASTER, &format!("{FADE}{TIMING}"));
    set(&mut parts, &layout(2), PUSH);
    set(&mut parts, &layout(3), NONE);
    set(&mut parts, &layout(4), &format!("{}{TIMING}", alternate()));
    set(&mut parts, "ppt/slides/slide1.xml", "<p:transition advTm=\"3000\"><p:dissolve/></p:transition>");
    reference(&mut parts, MASTER, "ppt/media/shared.png", 300);
    reference(&mut parts, &layout(2), "ppt/media/shared.png", 300);
    reference(&mut parts, &layout(2), "ppt/media/own.png", 150);
    Package::from_parts(parts).unwrap().save().unwrap()
}

fn inspect(bytes: &[u8], options: Option<Value>) -> Value {
    let mut request = json!({"op":"inspect_template","kind":"potx","base64":STANDARD.encode(bytes)});
    if let Some(options) = options { request["options"] = options; }
    execute_request(request).unwrap()
}

fn import(bytes: &[u8], options: Option<Value>) -> Result<Document, aislide_core::Error> {
    let mut request = json!({"op":"import_template","id":"imported","kind":"potx","base64":STANDARD.encode(bytes)});
    if let Some(options) = options { request["options"] = options; }
    execute_request(request).map(|value| serde_json::from_value(value).unwrap())
}

fn origin(document: &Document) -> BTreeMap<String, Vec<u8>> {
    Package::open(STANDARD.decode(&document.origin.as_ref().unwrap().base64).unwrap()).unwrap().parts().clone()
}

fn named<'a>(entries: &'a Value, name: &str) -> &'a Value { entries.as_array().unwrap().iter().find(|entry| entry["name"] == name).unwrap_or_else(|| panic!("{name}")) }

#[test]
fn issue79_inspect_template_reports_transitions_animations_and_images_per_master_and_layout() {
    let inspection = inspect(&template(), None);
    let master = &inspection["masters"][0];
    assert_eq!(master["id"], MASTER);
    assert_eq!(master["transition"], json!({"kind":"fade","extended":false,"speed":"med"}));
    assert_eq!(master["animations"], true);
    assert_eq!(master["images"], json!({"count":1,"bytes":300}));

    let layouts = &inspection["layouts"];
    // Nothing set on the layout: a slide on it inherits the master's fade, which the response names with its source.
    let blank = named(layouts, "Blank");
    assert_eq!(blank["transition"], Value::Null);
    assert_eq!(blank["animations"], false);
    assert_eq!(blank["effective_transition"], json!({"transition":{"kind":"fade","extended":false,"speed":"med"},"source":{"scope":"master","id":MASTER}}));
    assert_eq!(blank["images"], json!({"count":0,"bytes":0,"with_master":{"count":1,"bytes":300}}));

    let content = named(layouts, "Title and content");
    assert_eq!(content["transition"], json!({"kind":"push","extended":false,"speed":"slow"}));
    assert_eq!(content["effective_transition"]["source"], json!({"scope":"layout","id":layout(2)}));
    // Two image parts, one of them shared with the master, which counts once.
    assert_eq!(content["images"], json!({"count":2,"bytes":450,"with_master":{"count":2,"bytes":450}}));

    // An explicit transition without an effect overrides the master's, which differs from setting none.
    let columns = named(layouts, "Two columns");
    assert_eq!(columns["transition"], json!({"kind":"none","extended":false}));
    assert_eq!(columns["effective_transition"]["transition"]["kind"], "none");
    assert_eq!(columns["effective_transition"]["source"]["scope"], "layout");

    // The PowerPoint 2010 effect is read from the alternate content's richer choice.
    let section = named(layouts, "Section header");
    assert_eq!(section["transition"], json!({"kind":"vortex","extended":true,"speed":"slow","duration_ms":2000}));
    assert_eq!(section["animations"], true);

    assert_eq!(inspection["sample_slides"], json!({"count":1,"with_transition":1,"with_animations":0}));
    assert_eq!(inspection["template_images"], json!({"count":2,"bytes":450}));
    assert_eq!(inspection["sample_slide_count"], 1);
    assert!(inspection.get("import").is_none());
}

#[test]
fn issue79_a_template_without_transitions_reports_none_and_keeps_its_old_response_fields() {
    let bytes = export_template(&source(), TemplateKind::Potx).unwrap();
    let inspection = inspect(&bytes, None);
    for entry in inspection["masters"].as_array().unwrap().iter().chain(inspection["layouts"].as_array().unwrap()) {
        assert_eq!((&entry["transition"], &entry["animations"]), (&Value::Null, &json!(false)));
        assert_eq!(entry["images"]["count"], 0);
    }
    assert!(inspection["layouts"].as_array().unwrap().iter().all(|entry| entry["effective_transition"].is_null()));
    assert_eq!(inspection["sample_slides"], json!({"count":1,"with_transition":0,"with_animations":0}));
    for field in ["source_sha256", "sample_slide_count", "has_sections", "has_custom_shows", "limits", "warnings", "office_visual_parity"] { assert!(inspection.get(field).is_some(), "{field}"); }
}

#[test]
fn issue79_import_keeps_transitions_and_animations_unless_asked_to_strip_them() {
    let bytes = template();
    let kept = origin(&import(&bytes, None).unwrap());
    assert!(text(&kept, MASTER).contains(FADE) && text(&kept, MASTER).contains(TIMING));
    assert!(text(&kept, &layout(2)).contains(PUSH));
    // The explicit default options keep them as well.
    let defaults = origin(&import(&bytes, Some(json!({"strip_transitions":false,"strip_animations":false}))).unwrap());
    assert!(text(&defaults, MASTER).contains(FADE) && text(&defaults, &layout(4)).contains("vortex"));
}

#[test]
fn issue79_strip_transitions_and_strip_animations_are_independent() {
    let bytes = template();
    let original = Package::open(bytes.clone()).unwrap().parts().clone();

    let transitions = origin(&import(&bytes, Some(json!({"strip_transitions":true}))).unwrap());
    // Exactly the transition elements are gone, wherever PowerPoint wrote them; everything else in the part is byte for byte the same.
    assert_eq!(text(&transitions, MASTER), text(&original, MASTER).replace(FADE, ""));
    assert_eq!(text(&transitions, &layout(2)), text(&original, &layout(2)).replace(PUSH, ""));
    assert_eq!(text(&transitions, &layout(3)), text(&original, &layout(3)).replace(NONE, ""));
    assert_eq!(text(&transitions, &layout(4)), text(&original, &layout(4)).replace(&alternate(), ""));
    assert!(text(&transitions, MASTER).contains(TIMING) && text(&transitions, &layout(4)).contains(TIMING), "animations stay");
    assert!(!text(&transitions, "ppt/slides/slide1.xml").contains("p:transition"));

    let animations = origin(&import(&bytes, Some(json!({"strip_animations":true}))).unwrap());
    assert_eq!(text(&animations, MASTER), text(&original, MASTER).replace(TIMING, ""));
    assert_eq!(text(&animations, &layout(4)), text(&original, &layout(4)).replace(TIMING, ""));
    assert!(text(&animations, MASTER).contains(FADE) && text(&animations, &layout(4)).contains("vortex"), "transitions stay");

    let both = origin(&import(&bytes, Some(json!({"strip_transitions":true,"strip_animations":true}))).unwrap());
    for path in [MASTER.to_owned(), layout(1), layout(2), layout(3), layout(4), "ppt/slides/slide1.xml".to_owned()] {
        let xml = text(&both, &path);
        assert!(!xml.contains("p:transition") && !xml.contains("p:timing") && !xml.contains("AlternateContent"), "{path}");
    }
    // Images and unrelated parts are retained.
    assert_eq!(both["ppt/media/shared.png"], original["ppt/media/shared.png"]);
    assert_eq!(both["ppt/media/own.png"], original["ppt/media/own.png"]);
}

#[test]
fn issue79_the_dry_run_reports_what_the_import_retains_and_removes() {
    let bytes = template();
    let options = json!({"include_sample_slides":false,"layout_ids":[layout(2), layout(4)],"strip_transitions":true});
    let inspection = inspect(&bytes, Some(options.clone()));
    let preview = &inspection["import"];
    assert_eq!(preview["removed"], json!({"transitions":3,"animations":0}), "the master, the push and the vortex");
    assert_eq!(preview["layouts"].as_array().unwrap().len(), 2);
    for entry in preview["masters"].as_array().unwrap().iter().chain(preview["layouts"].as_array().unwrap()) { assert_eq!(entry["transition"], Value::Null, "{entry}"); }
    assert_eq!(preview["masters"][0]["animations"], true);
    assert_eq!(preview["sample_slides"], json!({"count":0,"with_transition":0,"with_animations":0}));
    assert_eq!(preview["template_images"], json!({"count":2,"bytes":450}));
    // The unselected layouts keep their transitions in the dry run's source report, which is the template itself.
    assert_eq!(named(&inspection["layouts"], "Two columns")["transition"]["kind"], "none");

    // The import does what the dry run said.
    let imported = origin(&import(&bytes, Some(options)).unwrap());
    assert!(!text(&imported, MASTER).contains("p:transition") && !text(&imported, &layout(2)).contains("p:transition") && !text(&imported, &layout(4)).contains("p:transition"));
    let both = inspect(&bytes, Some(json!({"strip_transitions":true,"strip_animations":true})));
    assert_eq!(both["import"]["removed"], json!({"transitions":5,"animations":2}), "master, three layouts and the sample slide; the master and one layout");
    assert_eq!(both["import"]["sample_slides"], json!({"count":1,"with_transition":0,"with_animations":0}));
}

#[test]
fn issue79_stripping_refuses_alternate_content_it_cannot_remove_whole() {
    let bytes = export_template(&source(), TemplateKind::Potx).unwrap();
    let mut parts = Package::open(bytes).unwrap().parts().clone();
    let mixed = format!("<mc:AlternateContent xmlns:mc=\"{MC}\"><mc:Choice Requires=\"p14\"><p:transition spd=\"slow\"><p:fade/></p:transition><p:extLst/></mc:Choice><mc:Fallback/></mc:AlternateContent>");
    set(&mut parts, &layout(2), &mixed);
    let template = Package::from_parts(parts).unwrap().save().unwrap();
    // Inspection reads it; stripping does not guess, and does not report success.
    assert_eq!(named(&inspect(&template, None)["layouts"], "Title and content")["transition"]["kind"], "fade");
    let error = import(&template, Some(json!({"strip_transitions":true}))).unwrap_err();
    assert!(error.to_string().contains("alternate content") && error.to_string().contains("slideLayout2.xml"), "{error}");
    // The same template imports unchanged when nothing is stripped, and strip_animations has nothing to remove in it.
    assert!(text(&origin(&import(&template, None).unwrap()), &layout(2)).contains("AlternateContent"));
    assert!(import(&template, Some(json!({"strip_animations":true}))).is_ok());
}

fn with_added_slide(document: &Document, layout_name: &str) -> Document {
    let mut deck = document.deck.clone();
    let layout_id = deck.design.as_ref().unwrap().layouts.iter().find(|layout| layout.name == layout_name).unwrap().id.clone();
    let mut added = deck.slides[0].clone();
    added.id = "added".into(); added.layout_id = Some(layout_id); added.native_source_id = None;
    deck.slides.push(added);
    document::transact(document, Transaction { expected_revision: document.revision, expected_hash: document.hash.clone(), operations: serde_json::from_value(json!([{"op":"replace","path":"/deck","value":deck}])).unwrap() }).unwrap().document
}

fn codes<'a>(report: &'a aislide_core::authoring_preflight::PreflightReport, code: &str) -> Vec<&'a aislide_core::authoring_preflight::Finding> {
    report.findings.iter().filter(|finding| finding.code == code).collect()
}

#[test]
fn issue79_preflight_reports_the_transition_each_slide_shows_and_where_it_is_set() {
    let bytes = template();
    let document = with_added_slide(&import(&bytes, Some(json!({"include_sample_slides":false}))).unwrap(), "Title and content");
    assert_eq!(document.deck.slides.len(), 2);
    let report = preflight_presentation(&document, &PreflightOptions::default()).unwrap();
    assert!(report.checks.iter().any(|check| check == "slide_transitions"));
    let transitions = codes(&report, "SLIDE_TRANSITION");
    assert_eq!(transitions.len(), 2, "{transitions:?}");
    // Page 0 is the blank slide on the first layout (the master's fade); page 1 was added later on the layout that sets a push.
    assert_eq!((transitions[0].page_index, transitions[0].severity.as_str()), (0, "info"));
    assert!(transitions[0].message.contains("fade") && transitions[0].message.contains("inherited from master"), "{}", transitions[0].message);
    assert_eq!(transitions[1].page_index, 1);
    assert!(transitions[1].message.contains("push") && transitions[1].message.contains("inherited from layout 'Title and content'"), "{}", transitions[1].message);
    assert!(transitions[1].message.contains("does not play"));
    // The master's animations are listed for both pages; the findings do not claim how PowerPoint inherits them.
    let animations = codes(&report, "SLIDE_ANIMATIONS");
    assert_eq!(animations.len(), 2);
    assert!(animations[0].message.contains(&format!("on master '")) && animations[0].message.contains("p:timing"));

    // The layout with an explicit no-effect transition overrides the master's fade, so that page has none to report.
    let columns = with_added_slide(&import(&bytes, Some(json!({"include_sample_slides":false}))).unwrap(), "Two columns");
    let report = preflight_presentation(&columns, &PreflightOptions { page_indices: Some(vec![1]), ..Default::default() }).unwrap();
    assert!(codes(&report, "SLIDE_TRANSITION").is_empty());

    // A sample slide that sets its own transition and auto-advance reports it as set on the slide.
    let sample = import(&bytes, None).unwrap();
    let report = preflight_presentation(&sample, &PreflightOptions { page_indices: Some(vec![0]), ..Default::default() }).unwrap();
    let own = codes(&report, "SLIDE_TRANSITION");
    assert_eq!(own.len(), 1);
    assert!(own[0].message.contains("dissolve") && own[0].message.contains("set on the slide itself") && own[0].message.contains("after 3000 ms"), "{}", own[0].message);
}

#[test]
fn issue79_a_stripped_import_has_nothing_left_to_report_and_other_documents_report_nothing() {
    let bytes = template();
    let stripped = with_added_slide(&import(&bytes, Some(json!({"include_sample_slides":false,"strip_transitions":true,"strip_animations":true}))).unwrap(), "Title and content");
    let report = preflight_presentation(&stripped, &PreflightOptions::default()).unwrap();
    assert!(codes(&report, "SLIDE_TRANSITION").is_empty() && codes(&report, "SLIDE_ANIMATIONS").is_empty());
    // A document AISlide created has no imported package to read.
    let created = source();
    let report = preflight_presentation(&created, &PreflightOptions::default()).unwrap();
    assert!(codes(&report, "SLIDE_TRANSITION").is_empty() && codes(&report, "SLIDE_ANIMATIONS").is_empty());
}

#[test]
fn issue79_import_options_are_validated_like_the_others() {
    let bytes = template();
    let error = execute_request(json!({"op":"import_template","id":"x","kind":"thmx","base64":STANDARD.encode(&bytes),"options":{"strip_transitions":true}})).unwrap_err();
    assert!(error.to_string().contains("only to POTX"), "{error}");
    let error = execute_request(json!({"op":"import_template","id":"x","kind":"potx","base64":STANDARD.encode(&bytes),"options":{"strip_transitions":"yes"}})).unwrap_err();
    assert!(error.to_string().to_lowercase().contains("invalid") || error.to_string().contains("boolean"), "{error}");
    let error = execute_request(json!({"op":"import_template","id":"x","kind":"potx","base64":STANDARD.encode(&bytes),"options":{"strip_transition":true}})).unwrap_err();
    assert!(error.to_string().contains("strip_transition"), "{error}");
}

#[test]
fn issue79_a_transition_without_an_effect_is_reported_when_it_starts_or_stops_a_sound() {
    for (action, sound, wording) in [(SOUND_START, "start", "starts a sound"), (SOUND_STOP, "stop", "stops the sound that is playing")] {
        let bytes = export_template(&source(), TemplateKind::Potx).unwrap();
        let mut parts = Package::open(bytes).unwrap().parts().clone();
        set(&mut parts, MASTER, action);
        let template = Package::from_parts(parts).unwrap().save().unwrap();
        assert_eq!(inspect(&template, None)["masters"][0]["transition"], json!({"kind":"none","extended":false,"sound":sound}));
        let report = preflight_presentation(&import(&template, None).unwrap(), &PreflightOptions::default()).unwrap();
        let findings = codes(&report, "SLIDE_TRANSITION");
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].message.contains("no visual effect") && findings[0].message.contains(wording), "{}", findings[0].message);
    }
}

#[test]
fn issue79_a_timing_element_without_animation_behavior_is_not_an_animation() {
    let bytes = export_template(&source(), TemplateKind::Potx).unwrap();
    let mut parts = Package::open(bytes).unwrap().parts().clone();
    set(&mut parts, MASTER, ROOT_ONLY);
    set(&mut parts, &layout(2), "<p:timing/>");
    let template = Package::from_parts(parts).unwrap().save().unwrap();
    let inspection = inspect(&template, None);
    assert_eq!(inspection["masters"][0]["animations"], false);
    assert_eq!(named(&inspection["layouts"], "Title and content")["animations"], false);
    // Stripping removes the markup but counts only what animates.
    let options = json!({"strip_animations":true});
    assert_eq!(inspect(&template, Some(options.clone()))["import"]["removed"]["animations"], 0);
    let imported = origin(&import(&template, Some(options)).unwrap());
    assert!(!text(&imported, MASTER).contains("p:timing") && !text(&imported, &layout(2)).contains("p:timing"));
    let report = preflight_presentation(&import(&template, None).unwrap(), &PreflightOptions::default()).unwrap();
    assert!(codes(&report, "SLIDE_ANIMATIONS").is_empty());
}

#[test]
fn issue79_effect_findings_are_document_evidence_and_include_inherited_false_keeps_only_the_slides_own() {
    let bytes = template();
    // Page 0 is the sample slide with its own dissolve; page 1 was added on the layout that sets a push, and the master defines an animation.
    let document = with_added_slide(&import(&bytes, None).unwrap(), "Title and content");
    let all = preflight_presentation(&document, &PreflightOptions::default()).unwrap();
    let own = preflight_presentation(&document, &PreflightOptions { include_inherited: false, ..Default::default() }).unwrap();
    assert_eq!(codes(&all, "SLIDE_TRANSITION").iter().map(|finding| finding.page_index).collect::<Vec<_>>(), vec![0, 1]);
    assert!(codes(&all, "SLIDE_TRANSITION").iter().chain(&codes(&all, "SLIDE_ANIMATIONS")).all(|finding| finding.evidence == "document"));
    assert_eq!(codes(&own, "SLIDE_TRANSITION").iter().map(|finding| finding.page_index).collect::<Vec<_>>(), vec![0], "the push comes from a layout");
    assert!(!codes(&all, "SLIDE_ANIMATIONS").is_empty());
    assert!(codes(&own, "SLIDE_ANIMATIONS").is_empty(), "only a master and a layout define animations");
}

#[test]
fn issue79_an_alternate_content_wrapper_that_repeats_a_transition_counts_once() {
    let preview = inspect(&template(), Some(json!({"layout_ids":[layout(4)],"include_sample_slides":false,"strip_transitions":true})));
    // The master's fade and the wrapper around the vortex and its fade fallback.
    assert_eq!(preview["import"]["removed"]["transitions"], 2);
}
