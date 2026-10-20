use super::{Item, Scale, Spec, State, Template, visible_items};
use crate::{Error, Result, model::TextAlign, parts::{Drawing, PartSpec}};
use serde_json::{json, Value};

fn placements<'a>(spec:&'a Spec,state:&State,w:f64,h:f64,title:bool)->Result<Vec<(&'a Item,[f64;4])>> {
    let items=visible_items(spec,state);
    let top=if title {148.0} else {108.0};
    let body=h-top-56.0;
    let left=32.0;
    let left_width=w*0.64-left-28.0;
    if spec.template==Template::Map {
        let groups:Vec<_>=spec.groups.iter().filter(|g|items.iter().any(|i|i.group_id.as_ref()==Some(&g.id))).map(|g|Some(g.id.as_str()))
            .chain(items.iter().any(|i|i.group_id.is_none()).then_some(None)).collect();
        let columns=if groups.len()>1 {2} else {1};
        let rows=groups.len().div_ceil(columns);
        let cw=(left_width-16.0*(columns-1) as f64)/columns as f64;
        let rh=(body-16.0*(rows-1) as f64)/rows as f64;
        let mut result=Vec::new();
        for (index,group) in groups.iter().enumerate() {
            let x=left+(index%columns) as f64*(cw+16.0);
            let y=top+(index/columns) as f64*(rh+16.0);
            let peers:Vec<_>=items.iter().filter(|i|i.group_id.as_deref()==*group).collect();
            let pitch=(rh-62.0)/peers.len() as f64;
            if pitch<54.0 {return Err(Error::Invalid("showcase map needs more vertical room per group; enlarge or split the view".into()));}
            for (row,item) in peers.iter().enumerate() {
                result.push((**item,[x+10.0,y+54.0+row as f64*pitch,cw-20.0,pitch-8.0]));
            }
        }
        return Ok(result);
    }
    Ok(items.iter().enumerate().map(|(index,item)| {
        let bounds=match spec.template {
            Template::Summary=>{
                let columns=if items.len()>2 {2} else {1};
                let rows=items.len().div_ceil(columns);
                let cw=(left_width-16.0*(columns-1) as f64)/columns as f64;
                let rh=(body-16.0*(rows-1) as f64)/rows as f64;
                [left+(index%columns) as f64*(cw+16.0),top+(index/columns) as f64*(rh+16.0),cw,rh]
            }
            Template::Dashboard=>{
                let pitch=(body-48.0)/items.len() as f64;
                [left,top+48.0+index as f64*pitch,left_width,pitch-4.0]
            }
            Template::Comparison=>{
                let row=(body-54.0)/items.len() as f64;
                [left,top+54.0+index as f64*row,left_width,row-12.0]
            }
            Template::Timeline=>{
                let step=(w-64.0)/items.len() as f64;
                [left+index as f64*step,top,step-8.0,110.0]
            }
            Template::Map=>unreachable!(),
        };
        (*item,bounds)
    }).collect())
}

pub(super) fn hotspots(part:&PartSpec,spec:&Spec,state:&State,w:f64,h:f64)->Result<Vec<Value>> {
    Ok(placements(spec,state,w,h,part.layout.as_ref().is_none_or(|layout|layout.show_title))?.into_iter()
        .map(|(item,[x,y,width,height])|json!({"item_id":item.id,"x":x,"y":y,"width":width,"height":height})).collect())
}

pub(super) fn controls_frame(part:&PartSpec,w:f64)->Value {
    let y=if part.layout.as_ref().is_none_or(|layout|layout.show_title) {88.0} else {20.0};
    json!({"x":32.0,"y":y,"width":w-64.0,"height":40.0})
}

fn text(d:&mut Drawing,s:&str,b:[f64;4],size:f64,color:&str,bold:bool) {
    d.text(s,b,size,color,bold,TextAlign::Left);
}
fn card(d:&mut Drawing,b:[f64;4],active:bool) {
    d.rect(b,if active {"@accent1"} else {"@lt2"});
}

fn detail(d:&mut Drawing,item:&Item,b:[f64;4]) {
    let [x,y,w,h]=b;
    d.rect(b,"@lt2");
    d.rect([x,y,5.0,h],"@accent1");
    text(d,"SELECTED / 選択中の視点",[x+24.0,y+18.0,w-48.0,22.0],14.0,"@dk2",true);
    text(d,&item.label,[x+24.0,y+52.0,w-48.0,68.0],28.0,"@dk1",true);
    text(d,&item.detail,[x+24.0,y+134.0,w-48.0,110.0],21.0,"@dk1",false);
    if !item.metrics.is_empty() {
        let track=(w-48.0)/item.metrics.len() as f64;
        for (index,metric) in item.metrics.iter().enumerate() {
            let mx=x+24.0+index as f64*track;
            text(d,&metric.label,[mx,y+260.0,track-8.0,24.0],14.0,"@dk2",true);
            text(d,&metric.value,[mx,y+292.0,track-8.0,44.0],29.0,"@accent1",true);
            text(d,&metric.qualifier,[mx,y+344.0,track-8.0,42.0],13.0,"@dk2",false);
        }
    }
    if !item.action.is_empty() {
        let top=y+h-100.0;
        d.rect([x+20.0,top,w-40.0,80.0],"@dk1");
        text(d,&item.action,[x+36.0,top+16.0,w-72.0,50.0],18.0,"@lt1",true);
    }
}

pub(super) fn draw(d:&mut Drawing,part:&PartSpec,spec:&Spec,state:&State,w:f64,h:f64)->Result<()> {
    if w<1200.0 || h<720.0 { return Err(Error::Invalid("showcase appearance requires at least 1200x720; enlarge the native frame rather than shrinking text".into())); }
    let items=visible_items(spec,state);
    let selected=items.iter().find(|item|item.id==state.selected_id).ok_or_else(||Error::Invalid("showcase selection missing".into()))?;
    let title=part.layout.as_ref().is_none_or(|layout|layout.show_title);
    let top=if title {148.0} else {108.0};
    let body=h-top-56.0;
    let left=32.0;
    let right=w*0.64;
    let left_width=right-left-28.0;
    let right_width=w-right-32.0;
    let targets=placements(spec,state,w,h,title)?;
    d.rect([0.0,0.0,w,h],"@lt1");
    if title {
        d.rect([0.0,0.0,w,120.0],"@dk1");
        d.rect([32.0,24.0,42.0,5.0],"@accent1");
        text(d,&part.subtitle,[88.0,16.0,w-200.0,26.0],16.0,"@lt2",true);
        text(d,&part.title,[32.0,42.0,w-160.0,42.0],34.0,"@lt1",true);
    }
    match spec.template {
        Template::Summary=>{
            for (index,(item,[x,y,cw,rh])) in targets.iter().copied().enumerate() {
                let active=item.id==state.selected_id;
                let ink=if active {"@lt1"} else {"@dk1"};
                card(d,[x,y,cw,rh],active);
                text(d,&format!("{:02}",index+1),[x+20.0,y+16.0,cw-40.0,26.0],16.0,ink,true);
                text(d,&item.label,[x+20.0,y+54.0,cw-40.0,rh-106.0],24.0,ink,true);
                let group=spec.groups.iter().find(|g|item.group_id.as_ref()==Some(&g.id)).map_or("選択して詳細を見る",|g|g.label.as_str());
                text(d,group,[x+20.0,y+rh-36.0,cw-40.0,24.0],14.0,ink,false);
            }
            detail(d,selected,[right,top,right_width,body]);
        }
        Template::Dashboard=>{
            text(d,match state.scale {Scale::Linear=>"件数 / 線形スケール",Scale::Logarithmic=>"件数 / 対数スケール log(1 + 値)"},[left,top,left_width,30.0],18.0,"@dk1",true);
            let pitch=(body-48.0)/items.len() as f64;
            let maximum=items.iter().filter_map(|item|item.value).fold(0.0_f64,f64::max);
            for (item,[_,y,_,_]) in targets.iter().copied() {
                let active=item.id==state.selected_id;
                let value=item.value.ok_or_else(||Error::Invalid("showcase dashboard value missing".into()))?;
                let ratio=if maximum==0.0 {0.0} else {match state.scale {Scale::Linear=>value/maximum,Scale::Logarithmic=>value.ln_1p()/maximum.ln_1p()}};
                let start=left+left_width*0.34;
                let bw=left_width*0.43;
                if active { d.rect([left,y,left_width,pitch-4.0],"@lt2"); }
                text(d,&item.label,[left+12.0,y+6.0,left_width*0.32-18.0,pitch-12.0],18.0,"@dk1",active);
                d.rect([start,y+14.0,bw,18.0],"@lt2");
                if ratio>0.0 {d.rect([start,y+14.0,bw*ratio,18.0],if active {"@accent1"} else {"@accent3"});}
                text(d,&format!("{} {}",crate::parts::display(value),item.unit),[start+bw+12.0,y+6.0,left_width*0.23-12.0,pitch-12.0],18.0,"@dk1",true);
            }
            detail(d,selected,[right,top,right_width,body]);
        }
        Template::Comparison=>{
            text(d,&format!("比較軸：{}",spec.dimensions[state.dimension]),[left,top,left_width,30.0],22.0,"@dk1",true);
            for (item,[_,y,_,rh]) in targets.iter().copied() {
                let active=item.id==state.selected_id;
                card(d,[left,y,left_width,rh],active);
                let ink=if active {"@lt1"} else {"@dk1"};
                text(d,&item.label,[left+18.0,y+18.0,left_width*0.33-28.0,rh-30.0],23.0,ink,true);
                text(d,&item.values[state.dimension],[left+left_width*0.34,y+18.0,left_width*0.66-22.0,rh-30.0],20.0,ink,false);
            }
            detail(d,selected,[right,top,right_width,body]);
        }
        Template::Timeline=>{
            let count=items.len();
            let track=w-64.0;
            let step=track/count as f64;
            let current=items.iter().position(|item|item.id==state.selected_id).unwrap_or(0);
            for (index,(item,[x,_,_,_])) in targets.iter().copied().enumerate() {
                d.rect([x,top,step-8.0,5.0],if index<=current {"@accent1"} else {"@lt2"});
                text(d,&format!("{:02}",index+1),[x,top+16.0,step-12.0,28.0],18.0,"@accent1",true);
                text(d,&item.label,[x,top+54.0,step-14.0,54.0],19.0,"@dk1",index==current);
            }
            let y=top+134.0;
            let bh=body-134.0;
            d.rect([left,y,left_width,bh],"@dk1");
            text(d,&format!("PHASE {:02} / {:02}",current+1,count),[left+24.0,y+20.0,left_width-48.0,28.0],16.0,"@lt2",true);
            text(d,&selected.label,[left+24.0,y+68.0,left_width-48.0,54.0],36.0,"@lt1",true);
            text(d,&selected.detail,[left+24.0,y+146.0,left_width-48.0,bh-174.0],23.0,"@lt1",false);
            let metrics=selected.metrics.iter().map(|m|format!("{}  {}  / {}",m.label,m.value,m.qualifier)).collect::<Vec<_>>().join("\n");
            text(d,"この段階で確かめること",[right,y+10.0,right_width,30.0],20.0,"@dk1",true);
            text(d,&selected.action,[right,y+60.0,right_width,116.0],23.0,"@dk1",true);
            if !metrics.is_empty() {
                d.rect([right,y+200.0,right_width,bh-200.0],"@lt2");
                text(d,&metrics,[right+18.0,y+218.0,right_width-36.0,bh-236.0],18.0,"@dk1",false);
            }
        }
        Template::Map=>{
            let groups:Vec<_>=spec.groups.iter().filter(|g|items.iter().any(|i|i.group_id.as_ref()==Some(&g.id))).map(|g|(Some(g.id.as_str()),g.label.as_str()))
                .chain(items.iter().any(|i|i.group_id.is_none()).then_some((None,"未分類"))).collect();
            let columns=if groups.len()>1 {2} else {1};
            let rows=groups.len().div_ceil(columns);
            let cw=(left_width-16.0*(columns-1) as f64)/columns as f64;
            let rh=(body-16.0*(rows-1) as f64)/rows as f64;
            for (index,(group,label)) in groups.iter().enumerate() {
                let x=left+(index%columns) as f64*(cw+16.0);
                let y=top+(index/columns) as f64*(rh+16.0);
                d.rect([x,y,cw,rh],"@lt2");
                text(d,label,[x+16.0,y+14.0,cw-32.0,30.0],18.0,"@dk1",true);
                for (item,[ix,iy,iw,ih]) in targets.iter().copied().filter(|(item,_)|item.group_id.as_deref()==*group) {
                    let active=item.id==state.selected_id;
                    d.rect([ix,iy,iw,ih],if active {"@accent1"} else {"@lt1"});
                    text(d,&item.label,[ix+12.0,iy+12.0,iw-24.0,ih-22.0],20.0,if active {"@lt1"} else {"@dk1"},active);
                }
            }
            detail(d,selected,[right,top,right_width,body]);
        }
    }
    text(d,&spec.source,[32.0,h-34.0,w-64.0,24.0],14.0,"@dk2",false);
    Ok(())
}
