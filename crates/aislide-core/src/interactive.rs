//! Typed, local interactive views. Imported HTML is never interpreted or executed.
use crate::{Error, Result, design::Theme, model::{Element, TextAlign}, parts::{Drawing, PartData, PartSpec}};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
pub mod office;
pub mod javascript_lab;
mod showcase;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all="snake_case")]
pub enum Template { Summary, Dashboard, Timeline, Comparison, Map }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all="snake_case")]
pub enum Appearance { #[default] Plain, Showcase }
impl Appearance { fn is_plain(&self)->bool { *self==Self::Plain } }

impl Template {
    pub fn name(self) -> &'static str {
        match self { Self::Summary=>"summary", Self::Dashboard=>"dashboard", Self::Timeline=>"timeline", Self::Comparison=>"comparison", Self::Map=>"map" }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all="snake_case")]
pub enum Scale { #[default] Linear, Logarithmic }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub selected_id: String,
    #[serde(default, skip_serializing_if="Option::is_none")] pub group_id: Option<String>,
    #[serde(default)] pub dimension: usize,
    #[serde(default)] pub scale: Scale,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Group { #[schemars(length(min = 1, max = 32))] pub id: String, #[schemars(length(min = 1, max = 32))] pub label: String }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Metric { #[schemars(length(min = 1, max = 32))] pub label: String, #[schemars(length(min = 1, max = 24))] pub value: String, #[schemars(length(min = 1, max = 80))] pub qualifier: String }

/// `id` is 1-32 ASCII letters, digits, `-` or `_`. `values` holds one text per dimension of the view.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Item {
    #[schemars(length(min = 1, max = 32))] pub id: String,
    #[schemars(length(min = 1, max = 48))] pub label: String,
    #[schemars(length(min = 1, max = 300))] pub detail: String,
    #[serde(default)] #[schemars(length(max = 160))] pub action: String,
    #[serde(default, skip_serializing_if="Option::is_none")] pub group_id: Option<String>,
    #[serde(default, skip_serializing_if="Option::is_none")] #[schemars(range(min = 0, max = 1e15))] pub value: Option<f64>,
    #[serde(default)] #[schemars(length(max = 16))] pub unit: String,
    #[serde(default)] #[schemars(length(max = 4), inner(length(min = 1, max = 96)))] pub values: Vec<String>,
    #[serde(default)] #[schemars(length(max = 3))] pub metrics: Vec<Metric>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    #[schemars(range(min = 1, max = 1))] pub version: u32,
    pub template: Template,
    #[serde(default,skip_serializing_if="Appearance::is_plain")] pub appearance: Appearance,
    #[schemars(length(min = 1, max = 240))] pub source: String,
    #[serde(default)] #[schemars(length(max = 4))] pub groups: Vec<Group>,
    #[serde(default)] #[schemars(length(max = 4), inner(length(min = 1, max = 32)))] pub dimensions: Vec<String>,
    #[schemars(length(min = 2, max = 8))] pub items: Vec<Item>,
    pub initial_state: State,
}

fn field(value: &str, maximum: usize, name: &str, required: bool) -> Result<()> {
    crate::model::valid_text(value, maximum).map_err(|error| Error::Invalid(format!("{name}: {error}")))?;
    if value.contains('\r') || required && value.trim().is_empty() { return Err(Error::Invalid(format!("{name}: nonempty LF text is required"))); }
    Ok(())
}

fn identity(value: &str, name: &str) -> Result<()> {
    if value.is_empty() || value.len()>32 || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b==b'-' || b==b'_') {
        return Err(Error::Invalid(format!("{name}: use 1-32 ASCII letters, digits, hyphens or underscores")));
    }
    Ok(())
}

pub fn validate(spec: &Spec) -> Result<()> {
    if spec.version!=1 || !(2..=8).contains(&spec.items.len()) || spec.groups.len()>4 || spec.dimensions.len()>4 {
        return Err(Error::Invalid("interactive version 1 requires 2-8 items, at most 4 groups and 4 dimensions".into()));
    }
    field(&spec.source,240,"interactive.source",true)?;
    let mut groups=BTreeSet::new();
    for group in &spec.groups {
        identity(&group.id,"interactive.groups.id")?;
        field(&group.label,32,"interactive.groups.label",true)?;
        if !groups.insert(group.id.as_str()) { return Err(Error::Invalid("duplicate interactive group ID".into())); }
    }
    for dimension in &spec.dimensions { field(dimension,32,"interactive.dimensions",true)?; }
    if spec.template==Template::Comparison && spec.dimensions.is_empty() { return Err(Error::Invalid("comparison requires named dimensions".into())); }
    let mut ids=BTreeSet::new();
    for (index,item) in spec.items.iter().enumerate() {
        let path=format!("interactive.items[{index}]");
        identity(&item.id,&format!("{path}.id"))?;
        if !ids.insert(&item.id) { return Err(Error::Invalid(format!("{path}.id: duplicate"))); }
        field(&item.label,48,&format!("{path}.label"),true)?;
        field(&item.detail,300,&format!("{path}.detail"),true)?;
        field(&item.action,160,&format!("{path}.action"),false)?;
        field(&item.unit,16,&format!("{path}.unit"),false)?;
        if item.group_id.as_ref().is_some_and(|id| !groups.contains(id.as_str())) { return Err(Error::Invalid(format!("{path}.group_id: unknown group"))); }
        if item.value.is_some_and(|value| !value.is_finite() || !(0.0..=1e15).contains(&value)) { return Err(Error::Invalid(format!("{path}.value: requires a finite nonnegative value <= 1e15"))); }
        if spec.template==Template::Dashboard && item.value.is_none() { return Err(Error::Invalid(format!("{path}.value: dashboard value is required"))); }
        if item.values.len()!=spec.dimensions.len() { return Err(Error::Invalid(format!("{path}.values: count must match dimensions"))); }
        for value in &item.values { field(value,96,&format!("{path}.values"),true)?; }
        if item.metrics.len()>3 { return Err(Error::Limit(format!("{path}.metrics: at most 3"))); }
        for metric in &item.metrics {
            field(&metric.label,32,&format!("{path}.metrics.label"),true)?;
            field(&metric.value,24,&format!("{path}.metrics.value"),true)?;
            field(&metric.qualifier,80,&format!("{path}.metrics.qualifier"),true)?;
        }
    }
    validate_state(spec,&spec.initial_state)
}

pub fn validate_state(spec: &Spec, state: &State) -> Result<()> {
    let selected=spec.items.iter().find(|item| item.id==state.selected_id).ok_or_else(||Error::Invalid("interactive.state.selected_id: unknown item".into()))?;
    if let Some(id)=&state.group_id {
        if !spec.groups.iter().any(|group| &group.id==id) || selected.group_id.as_ref()!=Some(id) { return Err(Error::Invalid("interactive.state.group_id: selected item must belong to the selected group".into())); }
    }
    if state.dimension>=spec.dimensions.len().max(1) { return Err(Error::Invalid("interactive.state.dimension: unknown dimension".into())); }
    Ok(())
}

pub fn visible_items<'a>(spec: &'a Spec, state: &State) -> Vec<&'a Item> {
    spec.items.iter().filter(|item| state.group_id.is_none() || item.group_id==state.group_id).collect()
}

pub fn preview(id: &str, part: &PartSpec, state: Option<State>, theme: Option<Theme>) -> Result<Value> {
    crate::parts::validate_spec(part)?;
    let PartData::Interactive { spec }=&part.data else { return Err(Error::Invalid("preview_interactive requires interactive part data".into())); };
    let state=state.unwrap_or_else(||spec.initial_state.clone());
    validate_state(spec,&state)?;
    let theme=theme.unwrap_or_default();
    let element=create(id,part,spec,&state,&theme)?;
    Ok(json!({"element":element,"state":state,"visible_ids":visible_items(spec,&state).iter().map(|item|&item.id).collect::<Vec<_>>(),
        "powerpoint_interactive":false,"executes_imported_html":false,"transient":true,
        "limitations":["Prototype: PPTX/PDF contains a native editable static view of initial_state, not executable HTML",
            "Selection does not modify a document; explicitly update initial_state through update_part to persist",
            "Static layout and available fonts require preview; Office visual parity is not verified"]}))
}

pub(crate) fn create(id: &str, part: &PartSpec, spec: &Spec, state: &State, theme: &Theme) -> Result<Element> {
    validate(spec)?; validate_state(spec,state)?;
    field(id,40,"interactive part ID",true)?;
    crate::design::validate_theme(theme)?;
    let frame=part.layout.as_ref().map_or([64.0,144.0,1152.0,512.0],|l|[l.x,l.y,l.width,l.height]);
    let show_title=part.layout.as_ref().is_none_or(|l|l.show_title);
    // The 80px title band is not drawn when hidden, so the body keeps the same 340px at either minimum.
    let (min_height,requirement)=if show_title {(480.0,"960x480 (960x400 when layout.show_title is false)")} else {(400.0,"960x400 (layout.show_title is false)")};
    if frame[2]<960.0 || frame[3]<min_height { return Err(Error::Invalid(format!("interactive part requires a frame of at least {requirement}; enlarge it rather than shrinking text"))); }
    let width=frame[2]; let height=frame[3]; let top=if show_title {80.0} else {0.0};
    let bottom=height-48.0; let body_height=bottom-top-12.0;
    let mut drawing=Drawing::new(&format!("{id}-interactive"));
    if spec.appearance==Appearance::Showcase {
        showcase::draw(&mut drawing,part,spec,state,width,height)?;
    } else {
    if top>0.0 {
        drawing.text(&part.title,[0.0,0.0,width,38.0],28.0,"@dk1",true,TextAlign::Left);
        drawing.text(&part.subtitle,[0.0,42.0,width,28.0],16.0,"@dk2",false,TextAlign::Left);
    }
    let selected=spec.items.iter().find(|item|item.id==state.selected_id).ok_or_else(||Error::Invalid("interactive selected item missing".into()))?;
    let items=visible_items(spec,state);
    let left_width=width*0.58; let right=left_width+28.0; let right_width=width-right;
    let text = |d:&mut Drawing,s:&str,rect:[f64;4],size:f64,bold:bool,color:&str| d.text(s,rect,size,color,bold,TextAlign::Left);
    match spec.template {
        Template::Comparison => {
            let column=left_width/items.len() as f64;
            text(&mut drawing,&spec.dimensions[state.dimension],[0.0,top,left_width,30.0],20.0,true,"@dk2");
            for (index,item) in items.iter().enumerate() {
                let x=index as f64*column;
                let active=item.id==state.selected_id;
                drawing.rect([x,top+40.0,column-12.0,4.0],if active {"@accent1"} else {"@lt2"});
                text(&mut drawing,&item.label,[x,top+58.0,column-20.0,64.0],22.0,true,if active {"@accent1"} else {"@dk1"});
                text(&mut drawing,&item.values[state.dimension],[x,top+134.0,column-20.0,body_height-150.0],18.0,false,"@dk1");
            }
        }
        Template::Timeline => {
            let slot=width/items.len() as f64;
            for (index,item) in items.iter().enumerate() {
                let x=index as f64*slot;
                drawing.rect([x,top+6.0,slot-8.0,4.0],if item.id==state.selected_id {"@accent1"} else {"@lt2"});
                text(&mut drawing,&format!("{}  {}",index+1,item.label),[x,top+22.0,slot-12.0,72.0],20.0,true,"@dk1");
            }
            text(&mut drawing,&selected.label,[0.0,top+110.0,width,36.0],24.0,true,"@accent1");
            text(&mut drawing,&selected.detail,[0.0,top+158.0,left_width,body_height-164.0],20.0,false,"@dk1");
            let remaining=body_height-164.0;
            let action_height=if selected.metrics.is_empty() {remaining} else {remaining*0.5};
            text(&mut drawing,&selected.action,[right,top+158.0,right_width,action_height],20.0,true,"@dk1");
            if !selected.metrics.is_empty() {
                let metrics=selected.metrics.iter().map(|m|format!("{} {} ({})",m.label,m.value,m.qualifier)).collect::<Vec<_>>().join("\n");
                text(&mut drawing,&metrics,[right,top+166.0+action_height,right_width,remaining-action_height-8.0],16.0,false,"@dk2");
            }
        }
        _ => {
            let row_height=body_height/items.len() as f64;
            let maximum=items.iter().filter_map(|i|i.value).fold(0.0_f64,f64::max);
            for (index,item) in items.iter().enumerate() {
                let y=top+index as f64*row_height;
                let active=item.id==state.selected_id;
                if spec.template==Template::Map {
                    let columns=(spec.groups.len()+usize::from(items.iter().any(|candidate|candidate.group_id.is_none()))).max(1);
                    let column=item.group_id.as_ref().and_then(|id|spec.groups.iter().position(|g|&g.id==id)).unwrap_or(spec.groups.len());
                    let peers:Vec<_>=items.iter().filter(|candidate|candidate.group_id==item.group_id).collect();
                    if (body_height-50.0)/(peers.len() as f64)<88.0 {
                        return Err(Error::Invalid("interactive map group needs a taller frame or fewer items; tile text must not overlap".into()));
                    }
                    let row=peers.iter().position(|candidate|candidate.id==item.id).unwrap_or(0);
                    let column_width=left_width/columns as f64;
                    let x=column as f64*column_width;
                    let top=top+50.0+row as f64*((body_height-50.0)/peers.len() as f64);
                    if row==0 {
                        let label=spec.groups.get(column).map_or("Ungrouped",|group|group.label.as_str());
                        text(&mut drawing,label,[x,top-50.0,column_width-12.0,40.0],16.0,true,"@dk2");
                    }
                    drawing.rect([x,top,column_width-12.0,3.0],if active {"@accent1"} else {"@lt2"});
                    text(&mut drawing,&item.label,[x,top+12.0,column_width-16.0,64.0],18.0,active,if active {"@accent1"} else {"@dk1"});
                } else {
                    text(&mut drawing,&item.label,[0.0,y,left_width*0.48,row_height-6.0],18.0,active,if active {"@accent1"} else {"@dk1"});
                }
                if spec.template==Template::Dashboard {
                    let value=item.value.ok_or_else(||Error::Invalid("dashboard value missing".into()))?;
                    let ratio=if maximum==0.0 {0.0} else {match state.scale { Scale::Linear=>value/maximum,Scale::Logarithmic=>value.ln_1p()/maximum.ln_1p() }};
                    let bar_width=left_width*0.26*ratio;
                    if bar_width>0.0 { drawing.rect([left_width*0.5,y+9.0,bar_width,16.0],"@accent1"); }
                    text(&mut drawing,&format!("{} {}",super::parts::display(value),item.unit),[left_width*0.78,y,left_width*0.22,row_height-6.0],16.0,false,"@dk1");
                } else if spec.template!=Template::Map {
                    if let Some(group)=spec.groups.iter().find(|g|Some(&g.id)==item.group_id.as_ref()) {
                        text(&mut drawing,&group.label,[left_width*0.5,y,left_width*0.5,row_height-6.0],16.0,false,"@dk2");
                    }
                }
            }
        }
    }
    if spec.template!=Template::Timeline {
        text(&mut drawing,&selected.label,[right,top,right_width,58.0],24.0,true,"@accent1");
        let detail_height=if selected.metrics.is_empty() {body_height*0.42} else {body_height*0.24};
        text(&mut drawing,&selected.detail,[right,top+70.0,right_width,detail_height],20.0,false,"@dk1");
        text(&mut drawing,&selected.action,[right,top+body_height*0.64,right_width,body_height*0.34],18.0,true,"@dk1");
        if !selected.metrics.is_empty() {
            let detail=selected.metrics.iter().map(|m|format!("{} {} ({})",m.label,m.value,m.qualifier)).collect::<Vec<_>>().join("\n");
            text(&mut drawing,&detail,[right,top+body_height*0.45,right_width,body_height*0.18],16.0,false,"@dk2");
        }
    }
    let selection=spec.groups.iter().find(|g|Some(&g.id)==state.group_id.as_ref()).map_or("All",|g|g.label.as_str());
    let mode=if spec.template==Template::Dashboard {match state.scale {Scale::Linear=>"linear",Scale::Logarithmic=>"log(1 + value)"}} else {"selection"};
    text(&mut drawing,&format!("{selection} | {mode} | {}",spec.source),[0.0,height-38.0,width,34.0],14.0,false,"@dk2");
    }
    // Filtering must not reuse a native text identity for a bar, or vice versa.
    let (mut texts,mut rectangles)=(0,0);
    for element in &mut drawing.elements {
        match element {
            Element::Text{id:child,..}=>{*child=format!("{id}-interactive-text-{texts}");texts+=1;}
            Element::Rect{id:child,..}=>{*child=format!("{id}-interactive-rect-{rectangles}");rectangles+=1;}
            _=>return Err(Error::Invalid("interactive projection contains an unsupported element".into())),
        }
    }
    let result=Element::Group{visual:None,id:id.into(),x:frame[0],y:frame[1],width,height,view_width:width,view_height:height,children:drawing.elements};
    if let Some((text,missing))=crate::parts::layout_overflow(&result,theme)? {
        return Err(Error::Invalid(format!("interactive content \"{text}\" {}; enlarge the frame, shorten text or split the view",if missing {"has missing glyphs"} else {"does not fit without reducing font size"})));
    }
    Ok(result)
}

pub(crate) fn presets() -> Vec<Value> {
    [Template::Summary,Template::Dashboard,Template::Timeline,Template::Comparison,Template::Map].iter().map(|template|{
        let data=Spec {version:1,template:*template,appearance:Appearance::Plain,source:"Synthetic example; values are not measured".into(),
            groups:vec![Group{id:"before".into(),label:"Before".into()},Group{id:"after".into(),label:"After".into()}],
            dimensions:vec!["Purpose".into(),"Control".into()],
            items:[("identity","Identity","before",120.0),("resource","Resource","after",12.0),("recovery","Recovery","after",4.0)].iter().map(|(id,label,group,value)|Item{
                id:(*id).into(),label:(*label).into(),detail:format!("Review {label} with its owner and approved evidence."),action:"Confirm scope and record the next action.".into(),
                group_id:Some((*group).into()),value:Some(*value),unit:"events".into(),values:vec![format!("{label} purpose"),format!("{label} control")],metrics:vec![],
            }).collect(),initial_state:State{selected_id:"identity".into(),group_id:None,dimension:0,scale:Scale::Linear}};
        let preset=format!("interactive/{}",template.name());
        json!({"id":preset,"category":"interactive","category_name":"Interactive prototypes","name":template.name(),"family":"Diagrams",
            "use_when":"Explore approved data using selection, grouping or staged detail in AISlide. The frame needs at least 960x480, or 960x400 when layout.show_title is false; a JavaScript lab export needs 960x480 either way.",
            "avoid_when":"PowerPoint interactivity or arbitrary HTML/JavaScript execution is required.",
            "example":{"version":1,"preset":preset,"title":format!("Interactive {}",template.name()),"subtitle":"Synthetic example",
                "data":{"kind":"interactive","spec":data}}})
    }).collect()
}
