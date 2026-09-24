use crate::{document::Document, model::{Deck, Element, TextAlign, TextFormat, valid_text}, parts::{PartSpec, state::PartInstance}, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const COMMON: &str = include_str!("../../../docs/authoring/common.md");
const PATTERNS: &str = include_str!("../../../docs/authoring/consulting-patterns.json");
const DEFAULT_SLIDE_LIMIT: usize = 32;
const MAX_SLIDE_LIMIT: usize = 128;
const PROFILES: &[(&str, &str, &str)] = &[
    ("consulting-decision", "Consulting and decision meetings", include_str!("../../../docs/authoring/consulting-decision.md")),
    ("technical-explainer", "Technical explanation", include_str!("../../../docs/authoring/technical-explainer.md")),
    ("event-talk", "Event presentation", include_str!("../../../docs/authoring/event-talk.md")),
    ("status-report", "Reports and operational updates", include_str!("../../../docs/authoring/status-report.md")),
];

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuidedInput {
    pub version: u32, pub profile_id: String, pub title: String, pub audience: String, pub purpose: String,
    pub governing_message: String, pub language: String,
    #[serde(default)] pub brand_color: Option<String>,
    #[serde(default, skip_serializing_if="Option::is_none")] pub authoring: Option<Authoring>,
    pub evidence: Vec<Evidence>,
    #[serde(default)] pub issues: Vec<DecisionIssue>,
    pub slides: Vec<GuidedSlide>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Evidence { pub id: String, pub kind: EvidenceKind, pub reference: String, pub statement: String }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind { Source, Assumption, Unknown }

#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Authoring {
    pub context: Option<AuthoringContext>,
    pub density: Option<AuthoringDensity>,
    pub spacing: Option<AuthoringSpacing>,
    #[schemars(range(min=12, max=40))] pub body_font_min: Option<f64>,
    #[schemars(range(min=28, max=64))] pub headline_font_size: Option<f64>,
    #[schemars(length(min=1, max=100))] pub font_family: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub headline_style: Option<HeadlineStyle>,
    #[serde(skip_serializing_if="Option::is_none")]
    #[schemars(range(min=32, max=128))] pub slide_limit: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all="snake_case")]
pub enum HeadlineStyle { #[default] Sentence, Keyword }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all="snake_case")]
pub enum AuthoringContext { Reading, Projection }

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all="snake_case")]
pub enum AuthoringDensity { #[default] Comfortable, Compact }

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all="snake_case")]
pub enum AuthoringSpacing { #[default] Standard, Relaxed }

struct ResolvedAuthoring<'a> {
    body_floor: f64, headline_size: f64, margin: f64, gap: f64,
    line_spacing: u32, paragraph_spacing: u32, font_family: Option<&'a str>,
}

impl Authoring {
    fn has_typography(&self) -> bool {
        self.context.is_some() || self.density.is_some() || self.spacing.is_some()
            || self.body_font_min.is_some() || self.headline_font_size.is_some() || self.font_family.is_some()
            || (self.headline_style.is_none() && self.slide_limit.is_none())
    }

    fn resolve(&self, profile: &str) -> ResolvedAuthoring<'_> {
        let projection = self.context.unwrap_or(if profile=="event-talk" {AuthoringContext::Projection} else {AuthoringContext::Reading})==AuthoringContext::Projection;
        let compact = self.density.unwrap_or_default()==AuthoringDensity::Compact;
        let relaxed = self.spacing.unwrap_or_default()==AuthoringSpacing::Relaxed;
        ResolvedAuthoring {
            body_floor: self.body_font_min.unwrap_or(if projection {24.0} else {16.0}),
            headline_size: self.headline_font_size.unwrap_or(if projection {40.0} else {34.0}),
            margin: if compact {48.0} else {64.0}, gap: if relaxed {24.0} else {12.0},
            line_spacing: if relaxed {130000} else {115000},
            paragraph_spacing: if compact {0} else {20000}, font_family: self.font_family.as_deref(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuidedSlide {
    pub id: String, pub section: String, pub headline: String, pub sentence_form: String,
    pub pattern_id: String, pub question: String, pub parent_message: String, pub transition: String,
    pub parallel_basis: String,
    #[serde(default)] pub part: Option<PartSpec>,
    pub support: Vec<ClauseSupport>,
    #[serde(default)] pub numbers: Vec<NumberEvidence>,
    #[serde(default, skip_serializing_if="Option::is_none")]
    #[schemars(length(max=4000))] pub speaker_notes: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClauseSupport { pub clause: String, pub body_paths: Vec<String>, pub evidence_ids: Vec<String> }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NumberEvidence { pub path: String, pub value: Value, pub evidence_id: String }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionIssue {
    pub id: String, pub question: String, pub requested_decision: String, pub criterion: String,
    pub owner: String, pub due: String, pub evidence_ids: Vec<String>, pub analysis_slide_ids: Vec<String>,
}

#[derive(Serialize)]
pub struct Review {
    pub ready: bool, pub issues: Vec<String>, pub review_required: Vec<String>,
    pub semantic_truth_verified: bool, pub office_visual_parity: bool,
}

pub fn profiles() -> Value {
    json!({"version":1,"profiles":PROFILES.iter().map(|(id,title,_)|json!({"id":id,"title":title,"language":"en","default_primary":"1976D2"})).collect::<Vec<_>>()})
}

pub fn guide(id: &str) -> Result<Value> {
    let (_,title,content)=PROFILES.iter().find(|entry|entry.0==id).ok_or_else(||Error::Unsupported("unknown authoring profile".into()))?;
    let patterns:Value=if id=="consulting-decision" {serde_json::from_str(PATTERNS)?} else {json!([])};
    let automatic_patterns=if id=="consulting-decision" {vec!["native-part","C02","C03"]} else {vec!["native-part"]};
    Ok(json!({"version":1,"profile_id":id,"title":title,"language":"en","markdown":format!("{}\n\n{}\n\nAuthoring overrides: authoring.slide_limit explicitly selects {DEFAULT_SLIDE_LIMIT}..{MAX_SLIDE_LIMIT} slides (default {DEFAULT_SLIDE_LIMIT}); resource capacity limits still apply. authoring.headline_style defaults to sentence. keyword waives Japanese decision headline length and adjacent sentence-form variation only; supported sentence_form names, logical support, evidence and numeric declarations remain required. These validation-only options do not activate typography overrides.",COMMON.trim_start_matches('\u{feff}'),content.trim_start_matches('\u{feff}')),"patterns":patterns,"input_schema":schemars::schema_for!(GuidedInput),"automatic_patterns":automatic_patterns,"semantic_truth_verified":false,"limits":{"slides":DEFAULT_SLIDE_LIMIT,"maximum_slides":MAX_SLIDE_LIMIT,"evidence":64,"issues":6},"creation":"Supply a structured, evidence-linked input; validate_guided_presentation before create_guided_presentation. No model call or file write."}))
}

fn required(value:&str, maximum:usize, field:&str, issues:&mut Vec<String>) {
    if value.trim().is_empty() || valid_text(value,maximum).is_err() {issues.push(format!("{field}: nonempty text within {maximum} characters is required"));}
}

fn numeric_paths(value:&Value, path:&str, output:&mut Vec<(String,Value)>) {
    match value {
        Value::Number(_) => output.push((path.into(),value.clone())),
        Value::Array(values) => for (index,item) in values.iter().enumerate() {numeric_paths(item,&format!("{path}/{index}"),output);},
        Value::Object(values) => for (key,item) in values {if !["x","y","width","height","font_size","detail_font_size","stroke_width","version","from","to","start","end","longitude","latitude"].contains(&key.as_str()) {numeric_paths(item,&format!("{path}/{}",key.replace('~',"~0").replace('/',"~1")),output);}},
        _ => (),
    }
}

fn checks(input:&GuidedInput)->Vec<String> {
    let mut issues=Vec::new();
    if input.version!=1 || !PROFILES.iter().any(|entry|entry.0==input.profile_id) {issues.push("Unknown profile or input version".into());}
    if !["en","ja"].contains(&input.language.as_str()) {issues.push("language must be en or ja".into());}
    required(&input.title,120,"title",&mut issues);required(&input.audience,240,"audience",&mut issues);
    required(&input.purpose,600,"purpose",&mut issues);required(&input.governing_message,600,"governing_message",&mut issues);
    let slide_limit=input.authoring.as_ref().and_then(|settings|settings.slide_limit).unwrap_or(DEFAULT_SLIDE_LIMIT);
    if !(DEFAULT_SLIDE_LIMIT..=MAX_SLIDE_LIMIT).contains(&slide_limit) {issues.push(format!("authoring.slide_limit must be {DEFAULT_SLIDE_LIMIT}..{MAX_SLIDE_LIMIT}"));return issues;}
    if !(1..=slide_limit).contains(&input.slides.len()) || input.evidence.len()>64 || input.issues.len()>6 {issues.push(format!("Guided input exceeds slide/evidence/decision limits: 1..{slide_limit} slides, at most 64 evidence entries and 6 decision issues"));return issues;}
    let sentence_headlines=input.authoring.as_ref().and_then(|settings|settings.headline_style).unwrap_or_default()==HeadlineStyle::Sentence;
    if let Some(color)=&input.brand_color {if color.len()!=6 || !color.bytes().all(|byte|byte.is_ascii_hexdigit()) {issues.push("brand_color must be a six-digit RGB value".into());}}
    if let Some(authoring)=&input.authoring {
        for (name,value,min,max) in [("body_font_min",authoring.body_font_min,12.0,40.0),("headline_font_size",authoring.headline_font_size,28.0,64.0)] {
            if value.is_some_and(|value|!value.is_finite() || !(min..=max).contains(&value)) {issues.push(format!("authoring.{name} must be finite and in {min}..{max}"));}
        }
        if let Some(family)=&authoring.font_family {
            required(family,100,"authoring.font_family",&mut issues);
            if family.chars().any(char::is_control) {issues.push("authoring.font_family cannot contain control characters".into());}
        }
    }
    let mut evidence=BTreeMap::new();
    for entry in &input.evidence {
        required(&entry.id,40,"evidence id",&mut issues);required(&entry.reference,600,"evidence reference",&mut issues);required(&entry.statement,1200,"evidence statement",&mut issues);
        if evidence.insert(entry.id.as_str(),entry).is_some() {issues.push("Duplicate evidence id".into());}
    }
    let mut slide_ids=BTreeSet::new();
    for slide in &input.slides {if slide.id=="governing" || !slide_ids.insert(slide.id.as_str()) {issues.push("Duplicate or reserved slide id".into());}}
    let mut decision_ids=BTreeSet::new();
    for issue in &input.issues {
        for (name,value,maximum) in [("issue id",&issue.id,32),("decision question",&issue.question,100),("requested decision",&issue.requested_decision,120),("criterion",&issue.criterion,120),("owner",&issue.owner,48),("due",&issue.due,48)] {required(value,maximum,name,&mut issues);}
        if !decision_ids.insert(&issue.id) {issues.push("Duplicate decision issue id".into());}
        if issue.evidence_ids.is_empty() || issue.evidence_ids.len()>8 || issue.evidence_ids.iter().any(|id|!evidence.contains_key(id.as_str())) {issues.push(format!("{}: unknown or missing decision evidence",issue.id));}
        if issue.analysis_slide_ids.is_empty() || issue.analysis_slide_ids.len()>12 || issue.analysis_slide_ids.iter().any(|id|!input.slides.iter().any(|slide|&slide.id==id && slide.pattern_id=="native-part" && slide.part.is_some())) {issues.push(format!("{}: unknown or missing analysis slide; summary and closing pages are not analysis evidence",issue.id));}
    }
    if input.profile_id=="consulting-decision" && input.slides.len()>1 {
        if !(3..=6).contains(&input.issues.len()) {issues.push("Multi-page decision decks require 3-6 decision issues".into());}
        if input.slides.first().is_none_or(|slide|slide.pattern_id!="C02") || input.slides.last().is_none_or(|slide|slide.pattern_id!="C03") {issues.push("Decision decks must open with C02 and close with C03".into());}
    }
    let mut prior=BTreeSet::from(["governing"]);
    for (index,slide) in input.slides.iter().enumerate() {
        if let Some(notes)=&slide.speaker_notes {if valid_text(notes,4000).is_err() {issues.push(format!("{}: speaker_notes must contain at most 4000 valid Unicode scalars",slide.id));}}
        for (name,value,maximum) in [("slide id",&slide.id,80),("section",&slide.section,80),("headline",&slide.headline,240),("question",&slide.question,240),("parent_message",&slide.parent_message,80),("transition",&slide.transition,80),("parallel_basis",&slide.parallel_basis,80)] {required(value,maximum,name,&mut issues);}
        if !prior.contains(slide.parent_message.as_str()) {issues.push(format!("{}: parent must be governing or an earlier slide",slide.id));}prior.insert(&slide.id);
        if !["causal","conditional","contrast","causal-focus","evaluation","proposal","explanation","comparison","outcome"].contains(&slide.sentence_form.as_str()) {issues.push(format!("{}: unknown sentence form",slide.id));}
        if sentence_headlines && input.profile_id=="consulting-decision" && index>=2 && input.slides[index-1].sentence_form==slide.sentence_form && input.slides[index-2].sentence_form==slide.sentence_form {issues.push(format!("{}: vary sentence forms across adjacent pages",slide.id));}
        if slide.headline.contains(['\u{2014}','\u{2015}']) || ["本ページ","この1枚","this slide"].iter().any(|word|slide.headline.to_lowercase().contains(word)) {issues.push(format!("{}: headline contains a forbidden dash or self-reference",slide.id));}
        if input.profile_id=="consulting-decision" {
            let mut count=0;let mut in_number=false;
            for character in slide.headline.chars() {if character.is_ascii_digit() || ('\u{ff10}'..='\u{ff19}').contains(&character) {if !in_number {count+=1;}in_number=true;} else if !['.',','].contains(&character) {in_number=false;}}
            if count>2 {issues.push(format!("{}: the headline contains more than two numeric claims",slide.id));}
        }
        if sentence_headlines && input.language=="ja" && input.profile_id=="consulting-decision" {
            let length=slide.headline.chars().filter(|character|!character.is_whitespace()).count();
            if !(30..=56).contains(&length) {issues.push(format!("{}: Japanese decision headline must contain 30-56 characters",slide.id));}
        }
        let body=match (&slide.pattern_id[..],&slide.part) {
            ("native-part",Some(part))=>serde_json::to_value(part).unwrap_or(Value::Null),
            ("C02"|"C03",None) if input.profile_id=="consulting-decision" && (3..=6).contains(&input.issues.len())=>json!({"issues":input.issues}),
            _=>{issues.push(format!("{}: pattern requires manual composition, is unsupported, or has the wrong body",slide.id));Value::Null},
        };
        if slide.support.is_empty() || slide.support.len()>8 {issues.push(format!("{}: every headline clause needs body and evidence support",slide.id));}
        let joined=slide.support.iter().map(|support|support.clause.as_str()).collect::<String>();
        let normalized=|value:&str|value.chars().filter(|character|!character.is_whitespace()).collect::<String>();
        if normalized(&joined)!=normalized(&slide.headline) {issues.push(format!("{}: ordered support clauses must reconstruct the whole headline",slide.id));}
        for support in &slide.support {
            let root=if slide.pattern_id=="native-part" {"/data"} else {"/issues"};
            if support.clause.is_empty() || support.body_paths.is_empty() || support.body_paths.len()>16 || support.body_paths.iter().any(|path|path.len()>512 || (path!=root && !path.starts_with(&format!("{root}/"))) || body.pointer(path).is_none_or(|value|value.is_null() || value.as_str().is_some_and(|text|text.trim().is_empty()) || value.as_array().is_some_and(Vec::is_empty) || value.as_object().is_some_and(serde_json::Map::is_empty))) {issues.push(format!("{}: clause refers to missing body evidence; use populated data, not titles",slide.id));}
            if support.evidence_ids.is_empty() || support.evidence_ids.len()>16 || support.evidence_ids.iter().any(|id|!evidence.contains_key(id.as_str())) {issues.push(format!("{}: clause refers to missing evidence id",slide.id));}
        }
        let mut numbers=Vec::new();if let Some(data)=body.get("data") {numeric_paths(data,"/data",&mut numbers);}
        if slide.numbers.len()>256 {issues.push(format!("{}: too many number declarations",slide.id));}
        let mut declared=BTreeSet::new();
        for number in &slide.numbers {
            if !declared.insert(&number.path) || number.path.len()>512 || !body.pointer(&number.path).is_some_and(|value|crate::canonical::equal(value,&number.value)) {issues.push(format!("{}: numeric evidence path/value mismatch",slide.id));}
            if evidence.get(number.evidence_id.as_str()).is_none_or(|entry|entry.kind==EvidenceKind::Unknown && number.value!=json!("xx")) {issues.push(format!("{}: numeric evidence is missing or unknown",slide.id));}
        }
        for (path,value) in numbers {if !slide.numbers.iter().any(|entry|entry.path==path && crate::canonical::equal(&entry.value,&value)) {issues.push(format!("{}: number at {path} has no source or assumption",slide.id));}}
        if input.profile_id=="consulting-decision" && slide.part.as_ref().is_some_and(|part|part.preset.starts_with("pie-chart/")) {issues.push(format!("{}: use a comparison chart; majority-only pie exceptions require manual review",slide.id));}
    }
    issues
}

fn text(id:&str,value:&str,bounds:[f64;4],size:f64,color:&str,bold:bool)->Element {
    let [x,y,width,height]=bounds;
    Element::Text{visual:None,id:id.into(),x,y,width,height,text:value.into(),font_size:size,color:color.into(),bold,format:TextFormat{font_family:Some("@minor".into()),alignment:TextAlign::Left,..TextFormat::default()}}
}

fn decision_table(rows:&[Vec<String>],top:f64,height:f64,closing:bool)->Element {
    let widths=if closing {[180.0,360.0,180.0,432.0]} else {[180.0,300.0,530.0,142.0]};
    let mut drawing=crate::parts::Drawing::new("decision-cell");
    let row_height=(height-44.0)/(rows.len()-1) as f64;
    for (row_index,row) in rows.iter().enumerate() {
        let row_top=if row_index==0 {0.0} else {44.0+(row_index-1) as f64*row_height};
        let current_height=if row_index==0 {44.0} else {row_height};
        drawing.rect([0.0,row_top,1152.0,current_height],if row_index%2==0 {"@lt2"} else {"@lt1"});
        drawing.rect([0.0,row_top+current_height-1.0,1152.0,1.0],"D9DEE3");
        let mut left=0.0;
        for (column,value) in row.iter().enumerate() {
            drawing.text(value,[left+14.0,row_top+if row_index==0 {2.0} else {7.0},widths[column]-28.0,current_height-if row_index==0 {4.0} else {12.0}],if row_index==0 {16.0} else {18.0},if row_index==0 || column==0 {"@accent1"} else {"@dk1"},row_index==0 || column==0,TextAlign::Left);
            left+=widths[column];
        }
    }
    Element::Group{visual:None,id:"decision-table".into(),x:64.0,y:top,width:1152.0,height,view_width:1152.0,view_height:height,children:drawing.elements}
}

fn author_text(text: &str, format: &mut TextFormat, settings: &ResolvedAuthoring<'_>, floor: f64) {
    use crate::rich_text::{RichParagraph, RichRun, Spacing};
    if let Some(family)=settings.font_family {format.font_family=Some(family.into());}
    if format.paragraphs.is_empty() {
        format.paragraphs=text.split('\n').map(|line|RichParagraph {runs:vec![RichRun {text:line.into(),..Default::default()}],..Default::default()}).collect();
    }
    let last=format.paragraphs.len().saturating_sub(1);
    for (index,paragraph) in format.paragraphs.iter_mut().enumerate() {
        paragraph.line_spacing=Some(Spacing::Percent(settings.line_spacing));
        paragraph.space_after=Some(Spacing::Percent(if index==last {0} else {settings.paragraph_spacing}));
        for run in &mut paragraph.runs {
            if let Some(size)=&mut run.style.font_size {*size=size.max(floor);}
            if let Some(family)=settings.font_family {run.style.font_family=Some(family.into());}
        }
    }
}

fn author_body(element: &mut Element, settings: &ResolvedAuthoring<'_>, scale: f64, available_height: f64) {
    let floor=settings.body_floor/scale;
    match element {
        Element::Text {text,font_size,format,..}|Element::Shape {text,font_size,format,..} => {
            *font_size=font_size.max(floor);
            author_text(text,format,settings,floor);
        }
        Element::Table {rows,font_size,format,..} => {
            *font_size=font_size.max(floor);
            for (row,values) in rows.iter().enumerate() {for (column,text) in values.iter().enumerate() {
                let mut style=format.cell_style(row,column);
                let run=style.text_style.get_or_insert_with(Default::default);
                run.font_size=Some(run.font_size.unwrap_or(*font_size).max(floor));
                if let Some(family)=settings.font_family {run.font_family=Some(family.into());}
                author_text(text,style.text_format.get_or_insert_with(Default::default),settings,floor);
                if let Some(cell)=format.cells.iter_mut().find(|cell|cell.row==row && cell.column==column) {cell.style=style;}
                else {format.cells.push(crate::table_format::CellFormat {row,column,style});}
            }}
        }
        Element::Group {width,height,view_width,view_height,children,..} => {
            let child_scale=scale*(*width / *view_width).min(*height / *view_height);
            let limits:Vec<_>=children.iter().map(|child| {
                let (_,left,top,width,height)=child.bounds();
                children.iter().filter(|peer|matches!(peer,Element::Text{..}|Element::Shape{..}|Element::Table{..}|Element::Chart{..}|Element::Group{..}))
                    .map(Element::bounds).filter(|(_,peer_left,peer_top,peer_width,_)|*peer_top>=top+height && *peer_left<left+width && peer_left+peer_width>left)
                    .map(|(_,_,peer_top,_,_)|peer_top-top).fold(*view_height-top,f64::min)
            }).collect();
            for (child,limit) in children.iter_mut().zip(limits) {author_body(child,settings,child_scale,limit);}
        }
        _ => (),
    }
    if let Element::Text {text,font_size,height,format,..}=element {if !text.is_empty() {
        let size=format.paragraphs.iter().flat_map(|paragraph|&paragraph.runs).filter_map(|run|run.style.font_size).fold(*font_size,f64::max);
        *height=height.max((size*settings.line_spacing as f64/100000.0).min(available_height));
    }}
}

fn measurement_cells(element: &mut Element) -> Result<()> {
    match element {
        Element::Group {children,..} => for child in children {measurement_cells(child)?;},
        Element::Table {id,x,y,width,height,rows,font_size,format} => {
            let widths=crate::table_format::tracks(format.column_widths.as_ref(),rows[0].len(),*width)?;
            let heights=crate::table_format::tracks(format.row_heights.as_ref(),rows.len(),*height)?;
            let mut children=Vec::new();
            for (row,values) in rows.iter().enumerate() {for (column,value) in values.iter().enumerate() {
                let merged=format.merge_at(row,column);
                if merged.is_some_and(|region|region.row!=row || region.column!=column) {continue;}
                let style=format.cell_style(row,column);
                let padding=style.padding.unwrap_or(crate::table_format::CellPadding {left:6.0,right:6.0,top:6.0,bottom:6.0});
                let row_span=merged.map_or(1,|region|region.row_span);let col_span=merged.map_or(1,|region|region.col_span);
                let cell_width=widths[column..column+col_span].iter().sum::<f64>()-padding.left-padding.right;
                let cell_height=heights[row..row+row_span].iter().sum::<f64>()-padding.top-padding.bottom;
                if cell_width<=0.0 || cell_height<=0.0 {return Err(Error::Invalid("guided table padding leaves no text area".into()));}
                let run=style.text_style.unwrap_or_default();
                let mut text_format=style.text_format.unwrap_or_default();
                if let Some(family)=run.font_family {text_format.font_family=Some(family);}
                if let Some(italic)=run.italic {text_format.italic=italic;}
                if let Some(underline)=run.underline {text_format.underline=underline;}
                children.push(Element::Text {
                    id:format!("{id}-cell-{row}-{column}"),x:widths[..column].iter().sum::<f64>()+padding.left,
                    y:heights[..row].iter().sum::<f64>()+padding.top,width:cell_width,height:cell_height,
                    text:value.clone(),font_size:run.font_size.unwrap_or(*font_size),color:run.color.unwrap_or_else(||"@dk1".into()),
                    bold:run.bold.unwrap_or(row==0),format:text_format,visual:None,
                });
            }}
            *element=Element::Group {id:id.clone(),x:*x,y:*y,width:*width,height:*height,view_width:*width,view_height:*height,children,visual:None};
        }
        _ => (),
    }
    Ok(())
}

fn build(input:&GuidedInput,id:&str)->Result<Document> {
    let authoring=input.authoring.as_ref().filter(|settings|settings.has_typography()).map(|settings|settings.resolve(&input.profile_id));
    let primary=input.brand_color.as_deref().unwrap_or("1976D2");
    let tint=|weight:f64|->String {(0..3).map(|index|{let channel=u8::from_str_radix(&primary[index*2..index*2+2],16).unwrap_or(0) as f64;format!("{:02X}",(channel+(255.0-channel)*weight).round() as u8)}).collect()};
    let mut design=crate::design::Design::default();
    design.theme.name=format!("Guided {}",input.profile_id);
    for (slot,value) in [("accent1",primary.to_owned()),("accent2",tint(0.28)),("accent3",tint(0.52)),("accent4",tint(0.75)),("accent5",primary.into()),("accent6","222222".into()),("dk1","222222".into()),("dk2","6B7C85".into()),("lt1","FFFFFF".into()),("lt2","EEF3F6".into())] {design.theme.colors.insert(slot.into(),value);}
    design.theme.fonts.major="Yu Gothic".into();design.theme.fonts.minor="Yu Gothic".into();design.theme.fonts.east_asian="Yu Gothic".into();
    if let Some(family)=authoring.as_ref().and_then(|settings|settings.font_family) {
        design.theme.fonts.major=family.into();design.theme.fonts.minor=family.into();
        design.theme.fonts.east_asian=family.into();design.theme.fonts.complex_script=family.into();
    }
    let design_value=json!({"theme":design.theme,"masters":[{"id":"guided-master","name":"Evidence-led briefing","background":"@lt1","elements":[]}],"layouts":[{"id":"guided-content","name":"Claim and evidence","master_id":"guided-master","background":null,"elements":[]}]});
    let mut slides=Vec::new();let mut parts=Vec::new();
    for (index,slide) in input.slides.iter().enumerate() {
        let double=if input.language=="ja" {slide.headline.chars().count()>36} else {slide.headline.chars().count()>80};
        let mut body_top=if double {168.0} else {144.0};let mut body_height=if double {474.0} else {498.0};
        let citations:Vec<_>=input.evidence.iter().filter(|entry|slide.support.iter().any(|support|support.evidence_ids.contains(&entry.id))).collect();
        let citation=citations.iter().map(|entry|format!("[{}] {}",entry.id,entry.reference)).collect::<Vec<_>>().join("; ");
        let mut elements=vec![text("section",&slide.section,[64.0,24.0,1152.0,28.0],18.0,"@accent1",true),text("headline",&slide.headline,[64.0,64.0,1152.0,if double {86.0} else {52.0}],if double {29.0} else {32.0},"@dk1",true),text("source",&citation,[64.0,658.0,1052.0,48.0],12.0,"@dk2",false),text("page",&format!("{} / {}",index+1,input.slides.len()),[1132.0,676.0,84.0,26.0],12.0,"@dk2",false)];
        if let Some(settings)=&authoring {
            if let Element::Text {x,width,height,font_size,format,..}=&mut elements[1] {
                *x=settings.margin;*width=1280.0-2.0*settings.margin;*font_size=settings.headline_size;
                *height=settings.headline_size*(2.0*settings.line_spacing as f64+settings.paragraph_spacing as f64)/100000.0+8.0;
                if let Some(family)=settings.font_family {format.font_family=Some(family.into());}
                body_top=(64.0+*height+settings.gap).max(144.0);
                body_height=642.0-body_top;
            }
        }
        if let Some(spec)=&slide.part {
            let element_id=format!("guided-part-{}",index+1);
            let mut element=crate::parts::create_with_theme(&element_id,spec,&design.theme)?;
            let margin=authoring.as_ref().map_or(64.0,|settings|settings.margin);
            let body_width=1280.0-2.0*margin;
            if let Element::Group{x,y,width,height,..}=&mut element {*x=margin;*y=body_top;*width=body_width;*height=body_height;}
            crate::parts::state::resize_canvas(&mut element,body_width,body_height)?;
            fn minimum_text(element:&mut Element) {match element {Element::Text{font_size,..}|Element::Shape{font_size,..}|Element::Table{font_size,..}=>*font_size=font_size.max(12.0),Element::Group{children,..}=>children.iter_mut().for_each(minimum_text),_=>()}}
            minimum_text(&mut element);
            if authoring.is_none() && input.profile_id=="event-talk" {
                let mut encoded=serde_json::to_value(&element)?;
                if let Some(children)=encoded["children"].as_array_mut() {for child in children {if child["type"]=="text" && child["y"].as_f64().is_some_and(|value|value>70.0) && child["height"].as_f64().is_some_and(|value|value>=40.0) {child["font_size"]=json!(child["font_size"].as_f64().unwrap_or(22.0).max(22.0));}}}
                element=serde_json::from_value(encoded)?;
            }
            if let Some(settings)=&authoring {author_body(&mut element,settings,1.0,body_height);}
            parts.push(PartInstance{slide_id:slide.id.clone(),element_id,spec:spec.clone(),render_sha256:crate::parts::state::render_hash(&element)?,native_sha256:None,stale:false});
            elements.push(element);
        } else {
            let closing=slide.pattern_id=="C03";
            let headers=if input.language=="ja" {if closing {vec!["論点","決定事項・責任者","日程","判断基準"]} else {vec!["論点","求める決定","根拠","参照ページ"]}} else if closing {vec!["Issue","Decision / owner","Timing","Criterion"]} else {vec!["Issue","Requested decision","Evidence","Analysis pages"]};
            let mut rows=vec![headers.into_iter().map(String::from).collect()];
            for issue in &input.issues {rows.push(if closing {vec![format!("{}: {}",issue.id,issue.question),format!("{}\n{}",issue.requested_decision,issue.owner),issue.due.clone(),issue.criterion.clone()]} else {vec![format!("{}: {}",issue.id,issue.question),issue.requested_decision.clone(),issue.evidence_ids.iter().filter_map(|id|input.evidence.iter().find(|entry|&entry.id==id)).map(|entry|entry.statement.clone()).collect::<Vec<_>>().join("\n"),issue.analysis_slide_ids.iter().filter_map(|id|input.slides.iter().position(|slide|&slide.id==id)).map(|index|(index+1).to_string()).collect::<Vec<_>>().join(", ")]});}
            let gap=authoring.as_ref().map_or(12.0,|settings|settings.gap);
            let margin=authoring.as_ref().map_or(64.0,|settings|settings.margin);
            let body_width=1280.0-2.0*margin;
            let panel_height=authoring.as_ref().map_or(44.0,|settings|(settings.body_floor.max(14.0)*(2.0*settings.line_spacing as f64+settings.paragraph_spacing as f64)/100000.0+8.0).max(44.0));
            let title_height=authoring.as_ref().map_or(28.0,|settings|(settings.body_floor.max(20.0)*settings.line_spacing as f64/100000.0+4.0).max(28.0));
            let panel_top=if authoring.is_some() {642.0-panel_height} else {body_top+434.0};
            let schedule_top=if authoring.is_some() {panel_top-gap-title_height} else {body_top+400.0};
            let table_top=body_top+gap;
            let table_height=if authoring.is_some() {if closing {schedule_top-gap-table_top} else {642.0-table_top}} else if closing {382.0} else {452.0};
            if table_height<=44.0 {return Err(Error::Invalid("guided decision body has insufficient space; reduce density or headline size".into()));}
            let mut table=decision_table(&rows,table_top,table_height,closing);
            if let Some(settings)=&authoring {
                if let Element::Group{x,width,..}=&mut table {*x=settings.margin;*width=body_width;}
                crate::parts::state::resize_canvas(&mut table,body_width,table_height)?;
            }
            elements.push(table);
            if closing {
                elements.push(text("schedule-title",if input.language=="ja" {"直近の実行日程"} else {"Immediate execution schedule"},[margin,schedule_top,body_width,title_height],20.0,"@dk1",true));
                let slot=body_width/input.issues.len() as f64;
                for (offset,issue) in input.issues.iter().enumerate() {elements.push(Element::Rect{visual:None,id:format!("schedule-panel-{offset}"),x:margin+offset as f64*slot,y:panel_top,width:slot-12.0,height:panel_height,fill:"@lt2".into()});elements.push(text(&format!("schedule-{offset}"),&format!("{} / {}\n{}",issue.id,issue.due,issue.owner),[margin+10.0+offset as f64*slot,panel_top+4.0,slot-32.0,panel_height-8.0],14.0,"@dk1",false));}
            }
        }
        if slide.part.is_none() {if let Some(settings)=&authoring {
            for element in &mut elements[4..] {let bounds=element.bounds();author_body(element,settings,1.0,bounds.4);}
        }}
        let mut ledger=slide.clone();ledger.speaker_notes=None;
        let mut notes=format!("Profile: {}\nAudience: {}\nPurpose: {}\nGoverning message: {}\nLedger: {}\nEvidence: {}\nSemantic truth and Office parity require human review.",input.profile_id,input.audience,input.purpose,input.governing_message,serde_json::to_string(&ledger)?,serde_json::to_string(&citations)?);
        if let Some(speaker_notes)=&slide.speaker_notes {notes.push_str("\nSpeaker notes:\n");notes.push_str(speaker_notes);}
        valid_text(&notes,8000).map_err(|_|Error::Limit(format!("{}: combined ledger, evidence and speaker notes exceed 8000 valid Unicode scalars",slide.id)))?;
        slides.push(json!({"id":slide.id,"title":slide.headline,"background":"@lt1","layout_id":"guided-content","inherit_background":true,"elements":elements,"notes":notes}));
    }
    let deck:Deck=serde_json::from_value(json!({"version":1,"title":input.title,"width":1280,"height":720,"design":design_value,"slides":slides}))?;
    let measured=if authoring.is_some() {
        crate::model::validate_deck(&deck)?;
        let mut measurement=deck.clone();
        for slide in &mut measurement.slides {for element in &mut slide.elements {measurement_cells(element)?;}}
        crate::layout::measure_layout(&measurement)?
    } else {crate::layout::measure_layout(&deck)?};
    let measured=serde_json::to_value(measured)?;
    let errors:Vec<_>=measured["issues"].as_array().unwrap().iter().filter(|issue|issue["severity"]=="error").collect();
    if !errors.is_empty() {return Err(Error::Invalid(format!("guided layout does not fit: {}",serde_json::to_string(&errors)?)));}
    let document=crate::document::create(id.into(),deck,Vec::new(),Vec::new(),None)?;
    if parts.is_empty() {return Ok(document);}
    Ok(crate::document::transact(&document,crate::document::Transaction{expected_revision:document.revision,expected_hash:document.hash.clone(),operations:serde_json::from_value(json!([{"op":"add","path":"/parts","value":parts}]))?})?.document)
}

fn evaluate(input:&GuidedInput,id:&str)->(Option<Document>,Review) {
    let mut issues=checks(input);
    let document=if issues.is_empty() {match build(input,id) {Ok(document)=>Some(document),Err(error)=>{issues.push(error.to_string());None}}} else {None};
    let review=Review{ready:document.is_some(),issues,review_required:vec!["Verify source authenticity, assumptions and headline entailment; declared references are not proof.".into(),"Review material quantities embedded in prose, arithmetic, qualitative scores, and logical completeness.".into(),"Inspect actual Office rendering, chart scales, figure conventions, shape-label collisions and orphan lines.".into(),"Native parts are primitives, not implementations of every consulting compound pattern.".into()],semantic_truth_verified:false,office_visual_parity:false};
    (document,review)
}

pub fn validate(input:&GuidedInput)->Review {evaluate(input,"guided-validation").1}

pub fn create(id:&str,input:&GuidedInput)->Result<Value> {
    valid_text(id,80)?;if id.is_empty() {return Err(Error::Invalid("document id is empty".into()));}
    let (document,review)=evaluate(input,id);
    let document=document.ok_or_else(||Error::Invalid(format!("guided input is not ready: {}",review.issues.join("; "))))?;
    Ok(json!({"document":document,"validation":review,"profile_id":input.profile_id,"model_inference":false}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guided_authoring_nested_rich_and_table_floors_survive_group_scaling() {
        let rich=json!({"type":"text","id":"rich","x":0,"y":0,"width":1500,"height":150,"text":"First\nSecond","font_size":12,"color":"@dk1","bold":false,"format":{"paragraphs":[{"runs":[{"text":"First","style":{"font_size":10,"font_family":"Old","italic":true}}]},{"runs":[{"text":"Second","style":{"bold":true}}]}]}});
        let table=json!({"type":"table","id":"table","x":0,"y":200,"width":1500,"height":500,"rows":[["Cell"]],"font_size":12,"format":{"cells":[{"row":0,"column":0,"style":{"fill":"@accent1","text_style":{"font_size":8,"font_family":"Old"},"text_format":{"paragraphs":[{"runs":[{"text":"Cell","style":{"font_size":9,"font_family":"Old","underline":true}}]}]}}}]}});
        let inner=json!({"type":"group","id":"inner","x":0,"y":0,"width":800,"height":400,"view_width":1600,"view_height":800,"children":[rich,table]});
        let mut element:Element=serde_json::from_value(json!({"type":"group","id":"outer","x":0,"y":0,"width":800,"height":400,"view_width":1600,"view_height":800,"children":[inner]})).unwrap();
        let settings:Authoring=serde_json::from_value(json!({"body_font_min":24,"font_family":"Arial","spacing":"relaxed"})).unwrap();
        author_body(&mut element,&settings.resolve("status-report"),1.0,400.0);
        let encoded=serde_json::to_value(&element).unwrap();
        let rich=&encoded["children"][0]["children"][0];
        assert_eq!(rich["font_size"],96.0);
        assert_eq!(rich["format"]["paragraphs"][0]["runs"][0]["style"]["font_size"],96.0);
        assert_eq!(rich["format"]["paragraphs"][0]["runs"][0]["style"]["font_family"],"Arial");
        assert_eq!(rich["format"]["paragraphs"][0]["runs"][0]["style"]["italic"],true);
        assert_eq!(rich["text"],"First\nSecond");
        assert_eq!(rich["format"]["paragraphs"][0]["space_after"]["value"],20000);
        assert_eq!(rich["format"]["paragraphs"][1]["space_after"]["value"],0);
        let table=&encoded["children"][0]["children"][1];
        assert_eq!(table["font_size"],96.0);
        let style=&table["format"]["cells"][0]["style"];
        assert_eq!(style["text_style"]["font_size"],96.0);
        assert_eq!(style["text_format"]["paragraphs"][0]["runs"][0]["style"]["font_size"],96.0);
        assert_eq!(style["text_format"]["paragraphs"][0]["runs"][0]["style"]["font_family"],"Arial");
        assert_eq!(style["text_format"]["paragraphs"][0]["runs"][0]["style"]["underline"],true);
        assert_eq!(style["fill"],"@accent1");
        assert_eq!(table["rows"],json!([["Cell"]]));
    }
}