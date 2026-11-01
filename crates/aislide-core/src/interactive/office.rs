//! Content add-in exports use generated views, never imported HTML.
use crate::{design::Theme, document::Document, model::Element, parts::{PartData, PartSpec}, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use base64::{Engine, engine::general_purpose::STANDARD};
use crate::{native::{P, A, R, REL, properties, relations_path}, native_save::{apply, insert_child}, package::Package, pptx::parse};
use std::collections::BTreeMap;
use super::{Scale, State, Template};

pub const ADDIN_ID: &str = "27b2fe44-fec3-4a21-b8bd-73781ec5b010";
pub const SETTINGS_KEY: &str = "aislideInteractive";
pub const EXPERIMENTAL_COPILOT_ID: &str = "fa000000210";
const MAX_PAYLOAD_BYTES: usize = 2 * 1024 * 1024;
const WE: &str = "http://schemas.microsoft.com/office/webextensions/webextension/2010/11";
const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const WEB_REL: &str = "http://schemas.microsoft.com/office/2011/relationships/webextension";
const WEB_TYPE: &str = "application/vnd.ms-office.webextension+xml";
const AISLIDE_OFFICE: &str = "urn:aislide:office-content:1";

pub(crate) fn static_fallback<'a,'input>(node:roxmltree::Node<'a,'input>)->Option<roxmltree::Node<'a,'input>> {
    if !node.has_tag_name((MC,"AlternateContent")) || node.attribute((AISLIDE_OFFICE,"version"))!=Some("1") { return None; }
    let fallback=node.children().find(|node|node.has_tag_name((MC,"Fallback")))?;
    let shapes:Vec<_>=fallback.children().filter(|node|node.is_element()).collect();
    (shapes.len()==1 && shapes[0].has_tag_name((P,"grpSp"))).then(||shapes[0])
}

pub(crate) fn in_playback_choice(node:roxmltree::Node<'_,'_>)->bool {
    node.ancestors().any(|ancestor|ancestor.has_tag_name((MC,"Choice")) && ancestor.parent().is_some_and(|parent|static_fallback(parent).is_some()))
}

pub fn compile(id: &str, part: &PartSpec, theme: Option<Theme>) -> Result<Value> {
    crate::parts::validate_spec(part)?;
    let PartData::Interactive { spec } = &part.data else {
        return Err(Error::Invalid("compile_interactive requires interactive part data".into()));
    };
    let theme = theme.unwrap_or_default();
    crate::design::validate_theme(&theme)?;
    let mut views = Vec::new();
    let scales: &[Scale] = if spec.template == Template::Dashboard { &[Scale::Linear, Scale::Logarithmic] } else { std::slice::from_ref(&spec.initial_state.scale) };
    let dimensions: Vec<_> = if spec.template == Template::Comparison { (0..spec.dimensions.len()).collect() } else { vec![spec.initial_state.dimension] };
    for item in &spec.items {
        let mut groups = vec![None];
        if let Some(group) = &item.group_id { groups.push(Some(group.clone())); }
        for group_id in groups {
            for dimension in &dimensions {
                for scale in scales {
                    let state = State { selected_id: item.id.clone(), group_id: group_id.clone(), dimension: *dimension, scale: *scale };
                    let element = super::create(id, part, spec, &state, &theme)?;
                    let mut view=json!({"state": state, "element": element});
                    if spec.appearance==super::Appearance::Showcase {
                        let bounds=element.bounds();
                        view["hotspots"]=json!(super::showcase::hotspots(part,spec,&state,bounds.3,bounds.4)?);
                        view["controls_frame"]=super::showcase::controls_frame(part,bounds.3);
                    }
                    views.push(view);
                }
            }
        }
    }
    let result = json!({
        "version": 1, "title": part.title, "template": spec.template, "source": spec.source,
        "groups": spec.groups, "dimensions": spec.dimensions,
        "items": spec.items.iter().map(|item| json!({"id":item.id,"label":item.label,"group_id":item.group_id})).collect::<Vec<_>>(),
        "initial_state": spec.initial_state, "colors": theme.colors,
        "font_family": theme.fonts.minor, "views": views,
    });
    if serde_json::to_vec(&result)?.len() > MAX_PAYLOAD_BYTES {
        return Err(Error::Limit("compiled interactive component exceeds 2 MiB; reduce items, dimensions or text".into()));
    }
    Ok(result)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportOptions {
    pub runtime_url: String,
    pub catalog: Catalog,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Catalog {
    FileSystem { location: String },
}

pub fn manifest(runtime_url: &str) -> Result<String> {
    let url = reqwest::Url::parse(runtime_url).map_err(|_| Error::Invalid("runtime_url must be an absolute HTTPS URL".into()))?;
    if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() || !url.path().ends_with("/office.html") {
        return Err(Error::Invalid("runtime_url must be HTTPS ending in /office.html without credentials, query or fragment".into()));
    }
    crate::model::valid_text(runtime_url, 2048)?;
    let escaped = quick_xml::escape::escape(runtime_url);
    Ok(format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<OfficeApp xmlns="http://schemas.microsoft.com/office/appforoffice/1.1" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:type="ContentApp">
  <Id>{ADDIN_ID}</Id><Version>1.0.0.0</Version><ProviderName>AISlide</ProviderName><DefaultLocale>en-US</DefaultLocale>
  <DisplayName DefaultValue="AISlide Interactive"/><Description DefaultValue="Generated AISlide interactive components. Requires this trusted HTTPS runtime; does not execute imported HTML."/>
  <Hosts><Host Name="Presentation"/></Hosts>
  <DefaultSettings><SourceLocation DefaultValue="{escaped}"/><RequestedWidth>1152</RequestedWidth><RequestedHeight>600</RequestedHeight></DefaultSettings>
  <Permissions>Restricted</Permissions><AllowSnapshot>true</AllowSnapshot>
</OfficeApp>"#))
}

pub fn export(document: &Document, options: &ExportOptions) -> Result<Value> {
    let manifest = manifest(&options.runtime_url)?;
    let Catalog::FileSystem { location } = &options.catalog;
    crate::model::valid_text(location, 2048)?;
    if !location.starts_with(r"\\") || location.starts_with(r"\\?\") || location.starts_with(r"\\.\")
        || location[2..].split('\\').filter(|component| !component.is_empty()).count() < 2
        || location.chars().any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '|' | '?' | '*' | '/'))
        || location.split('\\').any(|component| matches!(component, "." | "..")) {
        return Err(Error::Invalid("catalog.location must be the explicit UNC path of a trusted Windows add-in catalog, such as \\\\server\\AISlideAddins".into()));
    }
    export_target(document,Some((&manifest,location)),&[])
}

pub fn export_copilot(document:&Document,acknowledged_experimental:bool,labs:&[super::javascript_lab::Selection])->Result<Value> {
    if !acknowledged_experimental {
        return Err(Error::Invalid("Copilot catalog export requires acknowledged_experimental=true: its observed contentBlockHtml contract is not a supported public authoring API and playback is unverified".into()));
    }
    export_target(document,None,labs)
}

fn standalone_html(payload:&Value)->Result<String> {
    let runtime=include_str!("../../../../apps/office-content/runtime.mjs").trim_start_matches('\u{feff}');
    let css=include_str!("../../../../apps/office-content/runtime.css").trim_start_matches('\u{feff}');
    let declaration="export function mountInteractive";
    if runtime.matches(declaration).count()!=1 || runtime.to_ascii_lowercase().contains("</script") || css.to_ascii_lowercase().contains("</style") {
        return Err(Error::Invalid("trusted interactive runtime cannot be embedded as a self-contained HTML document".into()));
    }
    let runtime=runtime.replacen(declaration,"function mountInteractive",1);
    let data=serde_json::to_string(payload)?.replace('&',"\\u0026").replace('<',"\\u003c").replace('>',"\\u003e").replace('\u{2028}',"\\u2028").replace('\u{2029}',"\\u2029");
    let html=format!(r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'"><title>AISlide interactive component</title><style>{css}</style></head><body><main id="interactive-root"></main><script>(()=>{{"use strict";{runtime}
try {{ mountInteractive(document.getElementById("interactive-root"),{data}); }}
catch (reason) {{ const message=document.createElement("p");message.setAttribute("role","alert");message.textContent="AISlide playback could not start: "+(reason instanceof Error?reason.message:String(reason));document.getElementById("interactive-root").replaceChildren(message); }}
}})();</script></body></html>"#);
    if html.len()>MAX_PAYLOAD_BYTES { return Err(Error::Limit("self-contained interactive HTML exceeds 2 MiB".into())); }
    Ok(html)
}

fn export_target(document:&Document,own_addin:Option<(&str,&str)>,labs:&[super::javascript_lab::Selection])->Result<Value> {
    let managed: Vec<_> = document.parts.iter().filter(|part| matches!(part.spec.data, PartData::Interactive { .. })).collect();
    if managed.is_empty() || managed.len() > 16 { return Err(Error::Limit("PowerPoint interactive export requires 1-16 managed interactive components".into())); }
    if labs.len()>8 {return Err(Error::Limit("JavaScript lab export accepts at most 8 explicit components".into()));}
    let mut selected=std::collections::BTreeSet::new();
    for lab in labs {
        if !selected.insert((&lab.slide_id,&lab.element_id)) || !managed.iter().any(|part|part.slide_id==lab.slide_id && part.element_id==lab.element_id) {
            return Err(Error::Invalid("JavaScript lab selections must identify unique existing managed interactive components".into()));
        }
    }
    if managed.iter().any(|part| part.stale) { return Err(Error::Conflict("interactive metadata is stale; do not replace manually edited content with active playback".into())); }
    let static_export = crate::document::export_presentation(document)?;
    let bytes = STANDARD.decode(static_export["base64"].as_str().ok_or_else(||Error::Invalid("static export omitted its PPTX".into()))?).map_err(|_|Error::Invalid("static export base64".into()))?;
    let mut package = Package::open(bytes)?;
    if package.part_names().iter().any(|name| name.starts_with("ppt/webextensions/")) {
        return Err(Error::Unsupported("this PPTX already contains web extensions; retain its original or use the editable static source, rather than replacing imported active content".into()));
    }
    let native = crate::native::read(&package)?;
    let mut slide_edits = BTreeMap::<String, Vec<(std::ops::Range<usize>, String)>>::new();
    let mut content_types = String::new();
    let mut total_payload = 0;
    let mut warnings=Vec::new();
    for (index, part) in managed.iter().enumerate() {
        let slide_index=document.deck.slides.iter().position(|slide|slide.id==part.slide_id).ok_or_else(||Error::Invalid("interactive slide missing".into()))?;
        let slide=&document.deck.slides[slide_index];
        let element=slide.elements.iter().find(|element|element.bounds().0==part.element_id).ok_or_else(||Error::Conflict("interactive native group missing".into()))?;
        let fallback=Theme::default();
        let theme=crate::design::slide_theme(slide,document.deck.design.as_ref()).unwrap_or(&fallback);
        let mut spec=part.spec.clone();
        let (_,x,y,width,height)=element.bounds();
        let show_title=spec.layout.as_ref().is_none_or(|layout|layout.show_title);
        spec.layout=Some(crate::parts::PartLayout { x,y,width,height,show_title,fit: crate::parts::PartFit::Stretch });
        let payload=compile(&part.element_id,&spec,Some(theme.clone()))?;
        let serialized=serde_json::to_string(&payload)?;
        total_payload+=serialized.len();
        if total_payload>16*1024*1024 { return Err(Error::Limit("compiled PowerPoint interaction data exceeds 16 MiB".into())); }
        let binding=native.slides.get(slide_index).ok_or_else(||Error::Invalid("native slide binding missing".into()))?;
        let numeric=binding.nodes.get(&part.element_id).ok_or_else(||Error::Invalid("native interactive identity missing".into()))?;
        let xml=package.text(&binding.path)?.to_owned();
        let parsed=parse(&xml)?;
        let node=parsed.descendants().find(|node|node.has_tag_name((P,"grpSp")) && properties(*node).and_then(|p|p.attribute("id"))==Some(numeric.as_str()))
            .ok_or_else(||Error::Unsupported("interactive component must have a native group fallback".into()))?;
        if !node.parent().is_some_and(|parent|parent.has_tag_name((P,"spTree"))) { return Err(Error::Unsupported("interactive component must be a top-level native group".into())); }
        let max_id=parsed.descendants().filter_map(|node|node.attribute("id").and_then(|id|id.parse::<u32>().ok())).max().unwrap_or(1);
        let frame_id=max_id.checked_add(index as u32+1).ok_or_else(||Error::Limit("native frame identity exhausted".into()))?;
        let sequence=index+1;
        let relation=format!("rIdAISlideInteractive{sequence}");
        let extension_path=format!("ppt/webextensions/webextension{sequence}.xml");
        let image_path=format!("ppt/media/aislide-interactive-{sequence}.png");
        if package.part_names().contains(image_path.as_str()) { return Err(Error::Conflict("interactive snapshot destination already exists".into())); }
        let default_view=payload["views"].as_array().and_then(|views|views.iter().find(|view|view["state"]==payload["initial_state"]))
            .ok_or_else(||Error::Invalid("compiled initial state missing".into()))?;
        let (png,render_warnings)=snapshot(&serde_json::from_value(default_view["element"].clone())?,theme)?;
        for warning in render_warnings {
            warnings.push(json!({"slide_id":part.slide_id,"element_id":part.element_id,"warning":warning}));
        }
        package.add_part(image_path.clone(),png)?;
        let (reference,property)=if let Some((_,location))=own_addin {
            (format!(r#"<we:reference id="{ADDIN_ID}" version="1.0.0.0" store="{}" storeType="FileSystem"/>"#,attribute(location)),
                format!(r#"<we:property name="{SETTINGS_KEY}" value="{}"/>"#,attribute(&serialized)))
        } else {
            // The observed Copilot property contains a JSON string, not raw HTML or a settings object.
            let html=if let Some(lab)=labs.iter().find(|lab|lab.slide_id==part.slide_id && lab.element_id==part.element_id) {
                let PartData::Interactive{spec:interactive}=&part.spec.data else {return Err(Error::Invalid("JavaScript lab source metadata missing".into()));};
                if !super::javascript_lab::supports_frame(width,height) {
                    return Err(Error::Invalid(format!("{}; slide {} element {} is {width}x{height}: enlarge its frame to at least 960x480, or leave it out of javascript_labs to export its typed content",super::javascript_lab::CANVAS_REQUIREMENT,part.slide_id,part.element_id)));
                }
                super::javascript_lab::html(lab.kind,&part.spec.title,&interactive.source,width,height)?
            } else {standalone_html(&payload)?};
            let encoded=serde_json::to_string(&html)?;
            total_payload+=encoded.len();
            if total_payload>16*1024*1024 { return Err(Error::Limit("compiled PowerPoint interaction data exceeds 16 MiB".into())); }
            (format!(r#"<we:reference id="{EXPERIMENTAL_COPILOT_ID}" version="1.0.0.0" store="en-US" storeType="sdxcatalog"/>"#),
                format!(r#"<we:property name="contentBlockHtml" value="{}"/>"#,attribute(&encoded)))
        };
        let instance=format!("{{A1511DE0-0000-4000-8000-{sequence:012}}}");
        let extension=format!(r#"<we:webextension xmlns:we="{WE}" xmlns:r="{R}" id="{instance}" frozen="false">{reference}<we:properties>{property}</we:properties><we:bindings/><we:snapshot r:embed="rIdSnapshot"/></we:webextension>"#);
        package.add_part(extension_path.clone(),extension.into_bytes())?;
        package.add_part(relations_path(&extension_path),format!(r#"<Relationships xmlns="{REL}"><Relationship Id="rIdSnapshot" Type="{R}/image" Target="../media/aislide-interactive-{sequence}.png"/></Relationships>"#).into_bytes())?;
        append_relationship(&mut package,&relations_path(&binding.path),&relation,WEB_REL,&format!("../webextensions/webextension{sequence}.xml"))?;
        let emu=|value:f64|(value*9525.0).round().to_string();
        let fallback_group=&xml[node.range()];
        let wrapper=format!(r#"<mc:AlternateContent xmlns:mc="{MC}" xmlns:we="{WE}" xmlns:p="{P}" xmlns:a="{A}" xmlns:r="{R}" xmlns:aislide="{AISLIDE_OFFICE}" mc:Ignorable="aislide" aislide:version="1"><mc:Choice Requires="we"><p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{frame_id}" name="AISlide Interactive {sequence}"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></p:xfrm><a:graphic><a:graphicData uri="{WE}"><we:webextensionref r:id="{relation}"/></a:graphicData></a:graphic></p:graphicFrame></mc:Choice><mc:Fallback>{fallback_group}</mc:Fallback></mc:AlternateContent>"#,emu(x),emu(y),emu(width),emu(height));
        slide_edits.entry(binding.path.clone()).or_default().push((node.range(),wrapper));
        content_types.push_str(&format!(r#"<Override PartName="/{extension_path}" ContentType="{WEB_TYPE}"/><Override PartName="/{image_path}" ContentType="image/png"/>"#));
    }
    for (path,edits) in slide_edits { let xml=apply(package.text(&path)?.to_owned(),edits)?; package.replace_part(&path,xml)?; }
    let xml=package.text("[Content_Types].xml")?;
    let parsed=parse(xml)?;
    let mut edits=Vec::new();
    insert_child(xml,parsed.root_element(),&content_types,None,&mut edits)?;
    package.replace_part("[Content_Types].xml",apply(xml.to_owned(),edits)?)?;
    let bytes=package.save()?;
    if bytes.len()>document.capacity_profile.limits().archive_bytes { return Err(Error::Limit("interactive PPTX exceeds the selected archive byte budget".into())); }
    if own_addin.is_none() {
        return Ok(json!({
            "base64":STANDARD.encode(bytes),"filename":"interactive-copilot-experimental.pptx",
            "catalog_id":EXPERIMENTAL_COPILOT_ID,"store_type":"sdxcatalog","components":managed.len(),
            "experimental":true,"public_authoring_api_verified":false,"powerpoint_playback_verified":false,"javascript_lab_components":labs.len(),
            "warnings":warnings,"limitations":[
                "Experimental reproduction of an observed file format, not a supported Microsoft Copilot authoring API",
                "Requires a PowerPoint host with Microsoft's interactive slide runtime available; host policy, rollout and compatibility still apply",
                "No AISlide HTTPS server, own add-in manifest, trust changes or license bypass are provided by this export",
                "Generated HTML is authored by AISlide, not generated or approved by Microsoft Copilot; imported HTML is never executed",
                "Microsoft's catalog host may reject or change this unpublished property contract; no automatic switch to another runtime",
                "Keep the editable static source; snapshots/native fallback are not proof of active HTML playback",
                "For explicitly selected JavaScript lab components, the native fallback is an explanatory summary, not a live or identical UI"
            ]
        }));
    }
    let (manifest,_)=own_addin.ok_or_else(||Error::Invalid("own add-in configuration missing".into()))?;
    Ok(json!({
        "base64": STANDARD.encode(bytes), "filename":"interactive.pptx","manifest":manifest,"addin_id":ADDIN_ID,"components":managed.len(),
        "powerpoint_playback_verified":false,"requires_addin_installation":true,"requires_https_runtime":true,"warnings":warnings,
        "limitations":[
            "Windows development catalog export; network-share catalogs are not a production deployment method",
            "Each recipient must trust/install this AISlide-owned manifest and reach its trusted HTTPS runtime",
            "This is not Microsoft's Copilot catalog or a promise that arbitrary embedded HTML runs in PowerPoint",
            "Ordinary PPTX/PDF exports remain static; keep the editable static AISlide source for later edits",
            "Office may use its snapshot when an add-in is inactive; MCE-unaware readers use the native editable group fallback",
            "Runtime interaction is transient and resets to the saved initial_state; no Office settings are written",
            "Office playback and visual parity require separate host validation"
        ]
    }))
}

fn attribute(value:&str)->String {
    quick_xml::escape::escape(value).replace('\n',"&#10;").replace('\r',"&#13;").replace('\t',"&#9;")
}

fn append_relationship(package:&mut Package,path:&str,id:&str,kind:&str,target:&str)->Result<()> {
    let fragment=format!(r#"<Relationship Id="{}" Type="{}" Target="{}"/>"#,attribute(id),attribute(kind),attribute(target));
    if package.part_names().contains(path) {
        let xml=package.text(path)?;
        let parsed=parse(xml)?;
        if parsed.descendants().any(|node|node.attribute("Id")==Some(id)) { return Err(Error::Conflict("interactive relationship identity already exists".into())); }
        let mut edits=Vec::new();
        insert_child(xml,parsed.root_element(),&fragment,None,&mut edits)?;
        package.replace_part(path,apply(xml.to_owned(),edits)?)?;
    } else { package.add_part(path.into(),format!(r#"<Relationships xmlns="{REL}">{fragment}</Relationships>"#).into_bytes())?; }
    Ok(())
}

fn snapshot(element:&Element,theme:&Theme)->Result<(Vec<u8>,Vec<crate::export_static::RenderWarning>)> {
    let mut value=serde_json::to_value(element)?;
    value["x"]=json!(0);value["y"]=json!(0);
    let (_,_,_,width,height)=element.bounds();
    let deck=serde_json::from_value(json!({"version":1,"title":"AISlide content snapshot","width":width.ceil() as u32,"height":height.ceil() as u32,
        "design":{"theme":theme,"masters":[{"id":"master","name":"Snapshot","background":"@lt1","elements":[]}],"layouts":[{"id":"blank","name":"Blank","master_id":"master","elements":[]}]},
        "slides":[{"id":"snapshot","title":"Snapshot","background":"@lt1","notes":"","layout_id":"blank","elements":[value]}]}))?;
    let scene=crate::render::render_slide_svg(&deck,0,false)?;
    let tree=resvg::usvg::Tree::from_str(&scene.svg,&resvg::usvg::Options::default()).map_err(|_|Error::Invalid("generated snapshot SVG".into()))?;
    let mut pixmap=resvg::tiny_skia::Pixmap::new(width.ceil() as u32,height.ceil() as u32).ok_or_else(||Error::Limit("interactive snapshot allocation".into()))?;
    resvg::render(&tree,resvg::tiny_skia::Transform::identity(),&mut pixmap.as_mut());
    let png=pixmap.encode_png().map_err(|_|Error::Invalid("interactive snapshot PNG encoding".into()))?;
    Ok((png,scene.warnings))
}
