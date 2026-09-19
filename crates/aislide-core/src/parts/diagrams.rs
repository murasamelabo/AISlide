use super::{Drawing, PartData, PartItem, accent, count, display};
use crate::{model::{Element, TextAlign}, Error, Result};
use std::f64::consts::PI;

pub(super) fn render(drawing: &mut Drawing, category: &str, variant: usize, data: &PartData) -> Result<()> {
    match category {
        "tree" => tree(drawing,variant,data),
        "correlation" => network(drawing,variant,data),
        "matrix" | "contrast" => matrix(drawing,variant,data),
        "set" | "list-set" => groups(drawing,category,variant,data),
        "gantt-chart" => timeline(drawing,variant,data),
        "map" => map(drawing,variant,data),
        _ => {
            let PartData::Items { items, center } = data else { return Err(Error::Invalid("this diagram requires labeled items".into())); };
            match category {
                "flow" | "vertical-flow" => flow(drawing,items,category=="vertical-flow",variant),
                "cycle" => cycle(drawing,items,center,variant),
                "radiation" => radial(drawing,items,center,false,variant),
                "before-after" => before_after(drawing,items,variant),
                "pyramid" => pyramid(drawing,items,variant),
                "puzzle" => puzzle(drawing,items,variant),
                "venn" => venn(drawing,items,center,variant),
                "cross" => formula(drawing,items,center,variant),
                "scale-contrast" | "grow" | "ranking" => quantities(drawing,items,category,variant),
                "layer" | "step" => layers(drawing,items,category=="step",variant),
                "triangle" => triangle(drawing,items,center,variant),
                "list" | "list-horizontal" | "list-enumeration" => list(drawing,items,category,variant),
                _ => Err(Error::Unsupported("unknown diagram recipe".into())),
            }
        }
    }
}

fn block(drawing: &mut Drawing, item: &PartItem, bounds: [f64;4], index: usize, filled: bool) {
    let [left,top,width,height]=bounds; let shade=accent(index);
    if filled { drawing.rect(bounds,"@lt2"); }
    drawing.rect([left,top,3.0,height],&shade);
    drawing.text(&item.label,[left+16.0,top+12.0,width-30.0,48.0],20.0,"@dk1",true,TextAlign::Left);
    if !item.detail.is_empty() { drawing.text(&item.detail,[left+16.0,top+64.0,width-30.0,(height-72.0).max(24.0)],16.0,"@dk2",false,TextAlign::Left); }
}

fn list(drawing: &mut Drawing, items: &[PartItem], category: &str, variant: usize) -> Result<()> {
    let columns=if category=="list-horizontal" { count(items.len(),2,5)?; items.len() } else if category=="list-enumeration" { if variant==2 {2} else {3} } else if variant==2 {2} else {1};
    let rows=items.len().div_ceil(columns); let cell_width=1104.0/columns as f64; let cell_height=400.0/rows as f64;
    for (index,item) in items.iter().enumerate() {
        let left=24.0+(index%columns) as f64*cell_width; let top=88.0+(index/columns) as f64*cell_height;
        if category=="list" && variant!=2 {
            drawing.text(&format!("{:02}",index+1),[left,top+10.0,56.0,40.0],24.0,&accent(index),true,TextAlign::Left);
            drawing.text(&item.label,[left+80.0,top+8.0,360.0,42.0],20.0,"@dk1",true,TextAlign::Left);
            drawing.text(&item.detail,[left+466.0,top+8.0,cell_width-482.0,cell_height-18.0],16.0,"@dk2",false,TextAlign::Left);
            if variant==1 { drawing.rect([left+64.0,top,3.0,cell_height-12.0],&accent(index)); } else { drawing.rect([left,top+cell_height-12.0,cell_width-16.0,1.0],"@lt2"); }
        } else {
            if variant==1 { drawing.text(&format!("{:02}",index+1),[left+12.0,top,cell_width-40.0,40.0],28.0,&accent(index),true,TextAlign::Left); }
            let offset=if variant==1 {44.0} else {0.0};
            block(drawing,item,[left,top+offset,cell_width-20.0,cell_height-18.0-offset],index,variant==2);
        }
    }
    Ok(())
}

fn flow(drawing: &mut Drawing, items: &[PartItem], vertical: bool, variant: usize) -> Result<()> {
    count(items.len(),2,if vertical {5} else {6})?;
    if vertical {
        let row_height=392.0/items.len() as f64;
        for (index,item) in items.iter().enumerate() {
            let top=100.0+index as f64*row_height; let height=row_height-20.0;
            let shade=if variant==0 {"@accent1"} else {"@lt2"};
            drawing.rect([24.0,top,48.0,height],"@accent1");
            if variant==1 {
                drawing.polygon(vec![[88.0,top],[480.0,top],[480.0,top+height-10.0],[284.0,top+height],[88.0,top+height-10.0]],shade,"@accent1");
            } else { drawing.rect([88.0,top,if variant==2 {1040.0} else {392.0},height],shade); }
            if variant!=2 { drawing.rect([496.0,top,632.0,height],"@lt2"); }
            drawing.text(&(index+1).to_string(),[28.0,top+(height-30.0)/2.0,40.0,30.0],20.0,"@lt1",true,TextAlign::Center);
            drawing.text(&item.label,[108.0,top+10.0,350.0,height-24.0],22.0,if variant==0 {"@lt1"} else {"@dk1"},true,TextAlign::Left);
            drawing.text(&item.detail,[516.0,top+10.0,590.0,height-20.0],18.0,"@dk2",false,TextAlign::Left);
            if index+1<items.len() { filled_arrow(drawing,[48.0,top+height+2.0],[48.0,top+row_height-2.0],14.0,"@accent1"); }
        }
    } else {
        let slot=1104.0/items.len() as f64;
        for (index,item) in items.iter().enumerate() {
            let left=24.0+index as f64*slot; let width=slot-20.0; let inset=if variant==1 {32.0} else {18.0};
            if variant==1 {
                drawing.polygon(vec![[left,144.0],[left+width-22.0,144.0],[left+width,234.0],[left+width-22.0,324.0],[left,324.0],[left+22.0,234.0]],"@lt2","@accent1");
            } else { drawing.rect([left,if variant==2 {112.0} else {144.0},width,if variant==2 {368.0} else {180.0}],if variant==0 {"@accent1"} else {"@lt2"}); }
            drawing.text(&format!("{:02}",index+1),[left+inset,164.0,width-inset*2.0,36.0],24.0,if variant==0 {"@lt1"} else {"@accent1"},true,TextAlign::Left);
            drawing.text(&item.label,[left+inset,220.0,width-inset*2.0,76.0],20.0,if variant==0 {"@lt1"} else {"@dk1"},true,TextAlign::Left);
            if variant!=2 { drawing.rect([left,344.0,width,136.0],"@lt2"); }
            drawing.text(&item.detail,[left+14.0,356.0,width-28.0,108.0],18.0,"@dk2",false,TextAlign::Left);
            if index+1<items.len() { filled_arrow(drawing,[left+width+2.0,234.0],[left+slot-2.0,234.0],16.0,"@accent1"); }
        }
    }
    Ok(())
}

fn filled_arrow(drawing: &mut Drawing, start: [f64;2], end: [f64;2], width: f64, color: &str) {
    let distance=((end[0]-start[0]).powi(2)+(end[1]-start[1]).powi(2)).sqrt();
    if distance<1.0 { return; }
    let unit=[(end[0]-start[0])/distance,(end[1]-start[1])/distance]; let normal=[-unit[1],unit[0]];
    let shoulder=[end[0]-unit[0]*(width*0.8).min(distance*0.6),end[1]-unit[1]*(width*0.8).min(distance*0.6)];
    let offset=|point:[f64;2],amount:f64| [point[0]+normal[0]*amount,point[1]+normal[1]*amount];
    drawing.polygon(vec![offset(start,width*0.22),offset(shoulder,width*0.22),offset(shoulder,width*0.5),end,offset(shoulder,-width*0.5),offset(shoulder,-width*0.22),offset(start,-width*0.22)],color,color);
}

fn cycle(drawing: &mut Drawing, items: &[PartItem], center_label: &str, variant: usize) -> Result<()> {
    count(items.len(),3,6)?;
    let center=[if variant==2 {304.0} else {324.0},292.0];
    let outer=if variant==1 {192.0} else {180.0}; let inner=if variant==1 {126.0} else {132.0};
    let point=|angle:f64,radius:f64| [center[0]+radius*angle.cos(),center[1]+radius*angle.sin()];
    let step=2.0*PI/items.len() as f64; let row_height=392.0/items.len() as f64;
    for (index,item) in items.iter().enumerate() {
        let begin=-PI/2.0+index as f64*step+0.04; let end=begin+step-0.1; let shoulder=end-0.16;
        let shade=if index==0 {"@accent1"} else {"@lt2"};
        let mut points:Vec<_>=(0..=24).map(|sample|point(begin+(shoulder-begin)*sample as f64/24.0,outer)).collect();
        points.push(point(end,(outer+inner)/2.0));
        points.extend((0..=24).rev().map(|sample|point(begin+(shoulder-begin)*sample as f64/24.0,inner)));
        drawing.polygon(points,shade,"@accent1");
        let label_point=point((begin+shoulder)/2.0,(inner+outer)/2.0);
        drawing.text(&(index+1).to_string(),[label_point[0]-18.0,label_point[1]-15.0,36.0,30.0],20.0,if index==0 {"@lt1"} else {"@accent1"},true,TextAlign::Center);
        let top=100.0+index as f64*row_height; let left=if variant==2 {556.0} else {588.0}; let height=row_height-12.0;
        drawing.rect([left,top,1128.0-left,height],"@lt2");
        drawing.rect([left,top,4.0,height],"@accent1");
        drawing.text(&format!("{:02}",index+1),[left+14.0,top+8.0,44.0,28.0],20.0,"@accent1",true,TextAlign::Left);
        drawing.text(&item.label,[left+74.0,top+6.0,1028.0-left,28.0],20.0,"@dk1",true,TextAlign::Left);
        drawing.text(&item.detail,[left+74.0,top+36.0,1028.0-left,(height-40.0).max(16.0)],16.0,"@dk2",false,TextAlign::Left);
    }
    drawing.shape("ellipse",[center[0]-90.0,center[1]-70.0,180.0,140.0],if variant==1 {"@lt2"} else {"@lt1"},"@lt2");
    drawing.text(center_label,[center[0]-66.0,center[1]-40.0,132.0,80.0],20.0,"@dk1",true,TextAlign::Center);
    Ok(())
}

fn node_positions(length: usize, center: [f64;2], radius: [f64;2]) -> Vec<[f64;2]> {
    (0..length).map(|index| { let angle=-PI/2.0+2.0*PI*index as f64/length as f64; [center[0]+radius[0]*angle.cos(),center[1]+radius[1]*angle.sin()] }).collect()
}

fn radial(drawing: &mut Drawing, items: &[PartItem], center_label: &str, cycle: bool, variant: usize) -> Result<()> {
    count(items.len(),3,6)?;
    let center=if variant==2 {[416.0,286.0]} else {[576.0,286.0]}; let radius=if variant==2 {[274.0,136.0]} else {[380.0,138.0]};
    let positions=node_positions(items.len(),center,radius);
    if variant==1 { drawing.shape("ellipse",[center[0]-radius[0],center[1]-radius[1],radius[0]*2.0,radius[1]*2.0],"none","@lt2"); }
    for (index,position) in positions.iter().enumerate() {
        let target=if cycle {positions[(index+1)%positions.len()]} else {center};
        let from=if cycle {*position} else {center}; let to=if cycle {target} else {*position};
        let distance=((to[0]-from[0]).powi(2)+(to[1]-from[1]).powi(2)).sqrt();
        let unit=[(to[0]-from[0])/distance,(to[1]-from[1])/distance];
        drawing.line([from[0]+unit[0]*62.0,from[1]+unit[1]*42.0],[to[0]-unit[0]*62.0,to[1]-unit[1]*42.0],cycle,"@dk2");
    }
    if !cycle || variant==1 { drawing.shape("ellipse",[center[0]-96.0,center[1]-42.0,192.0,84.0],"@lt1","@accent1"); drawing.text(center_label,[center[0]-80.0,center[1]-24.0,160.0,52.0],18.0,"@dk1",true,TextAlign::Center); }
    for (index,(item,position)) in items.iter().zip(positions.iter()).enumerate() {
        let width=if variant==2 {152.0} else {200.0};
        drawing.rect([position[0]-width/2.0,position[1]-38.0,width,76.0],"@lt1");
        drawing.text(&item.label,[position[0]-width/2.0,position[1]-30.0,width,42.0],18.0,&accent(index),true,TextAlign::Center);
        if variant==2 { drawing.text(&format!("{}. {}",index+1,item.label),[832.0,98.0+index as f64*64.0,280.0,26.0],17.0,"@dk1",true,TextAlign::Left); drawing.text(&item.detail,[832.0,126.0+index as f64*64.0,280.0,34.0],14.0,"@dk2",false,TextAlign::Left); }
        else { drawing.text(&item.detail,[position[0]-width/2.0,position[1]+14.0,width,52.0],14.0,"@dk2",false,TextAlign::Center); }
    }
    Ok(())
}

fn tree(drawing: &mut Drawing, variant: usize, data: &PartData) -> Result<()> {
    let PartData::Tree { nodes }=data else {return Err(Error::Invalid("tree data required".into()));};
    let mut children=vec![Vec::new();nodes.len()]; let mut root=0;
    for (index,node) in nodes.iter().enumerate() { if let Some(parent)=&node.parent { children[nodes.iter().position(|entry|&entry.id==parent).expect("validated tree")].push(index); } else { root=index; } }
    fn leaves(index:usize,children:&[Vec<usize>])->usize { if children[index].is_empty() {1} else {children[index].iter().map(|child|leaves(*child,children)).sum()} }
    fn arrange(index:usize,depth:usize,start:f64,span:f64,children:&[Vec<usize>],positions:&mut [[f64;3]],order:&mut Vec<usize>) {
        positions[index]=[depth as f64,start+span/2.0,span]; order.push(index);
        let total=leaves(index,children) as f64; let mut cursor=start;
        for child in &children[index] { let width=span*leaves(*child,children) as f64/total; arrange(*child,depth+1,cursor,width,children,positions,order);cursor+=width; }
    }
    let mut positions=vec![[0.0;3];nodes.len()]; let mut order=Vec::new();
    arrange(root,0,0.0,1.0,&children,&mut positions,&mut order);
    let levels=positions.iter().map(|position|position[0] as usize).max().unwrap_or(0)+1;
    let mut boxes=vec![[0.0;4];nodes.len()];
    for (row,index) in order.iter().enumerate() {
        let [depth,center,span]=positions[*index];
        boxes[*index]=if variant==2 {let left=32.0+depth*56.0;[left,100.0+row as f64*392.0/nodes.len() as f64,1096.0-left,392.0/nodes.len() as f64-6.0]}
        else if variant==1 {let height=(span*392.0-14.0).min(88.0);if height<36.0 {return Err(Error::Limit("tree has too many sibling branches; use the outline variant or split the tree".into()));}[32.0+depth*1096.0/levels as f64,100.0+center*392.0-height/2.0,(1096.0/levels as f64-44.0).min(232.0),height]}
        else {let width=(span*1096.0-16.0).min(232.0);[32.0+center*1096.0-width/2.0,100.0+depth*392.0/levels as f64,width,(392.0/levels as f64-32.0).min(76.0)]};
    }
    for (parent,descendants) in children.iter().enumerate() { for child in descendants {
        let [left,top,width,height]=boxes[parent]; let [child_left,child_top,child_width,child_height]=boxes[*child];
        if variant==0 {let middle=(top+height+child_top)/2.0;drawing.line([left+width/2.0,top+height],[left+width/2.0,middle],false,"@accent1");drawing.line([left+width/2.0,middle],[child_left+child_width/2.0,middle],false,"@accent1");drawing.line([child_left+child_width/2.0,middle],[child_left+child_width/2.0,child_top],false,"@accent1");}
        else if variant==1 {let middle=(left+width+child_left)/2.0;drawing.line([left+width,top+height/2.0],[middle,top+height/2.0],false,"@accent1");drawing.line([middle,top+height/2.0],[middle,child_top+child_height/2.0],false,"@accent1");drawing.line([middle,child_top+child_height/2.0],[child_left,child_top+child_height/2.0],false,"@accent1");}
        else {drawing.line([child_left-16.0,top+height],[child_left-16.0,child_top+child_height/2.0],false,"@accent1");drawing.line([child_left-16.0,child_top+child_height/2.0],[child_left,child_top+child_height/2.0],false,"@accent1");}
    } }
    for (index,node) in nodes.iter().enumerate() {let [left,top,width,height]=boxes[index];let primary=index==root;
        drawing.rect(boxes[index],if primary {"@accent1"} else {"@lt2"});
        drawing.text(&node.label,[left+12.0,top+if variant==2 {3.0} else {10.0},width-24.0,height-if variant==2 {6.0} else {20.0}],if variant==2 {18.0} else {20.0},if primary {"@lt1"} else {"@dk1"},true,TextAlign::Left);
    }
    Ok(())
}

fn network(drawing: &mut Drawing, variant: usize, data: &PartData) -> Result<()> {
    let PartData::Network { nodes, edges }=data else {return Err(Error::Invalid("network data required".into()));};
    let positions=if variant==0 {node_positions(nodes.len(),[576.0,292.0],[376.0,140.0])} else {(0..nodes.len()).map(|index| if variant==1 {[if index%2==0 {260.0} else {884.0},148.0+(index/2) as f64*140.0]} else {[200.0+(index%3) as f64*376.0,156.0+(index/3) as f64*260.0]}).collect()};
    let mut occupied:Vec<_>=positions.iter().map(|position|[position[0]-106.0,position[1]-48.0,212.0,138.0]).collect();
    let mut labels=Vec::new();
    for edge in edges {
        let from=positions[edge.from];let to=positions[edge.to];drawing.line(from,to,false,"@dk2");
        if edge.label.is_empty() {labels.push(None);continue;}
        let bounds=(0..=30).map(|index|0.5+if index%2==0 {index as f64*0.01} else {-(index as f64+1.0)*0.01}).map(|fraction|[from[0]+(to[0]-from[0])*fraction-64.0,from[1]+(to[1]-from[1])*fraction-13.0,128.0,28.0]).find(|candidate| {
            !occupied.iter().any(|other|candidate[0]<other[0]+other[2]+4.0 && other[0]<candidate[0]+candidate[2]+4.0 && candidate[1]<other[1]+other[3]+4.0 && other[1]<candidate[1]+candidate[3]+4.0)
        }).ok_or_else(||Error::Limit("relationship labels need more space; use fewer links or another variant".into()))?;
        occupied.push(bounds);labels.push(Some(bounds));
    }
    for (index,(edge,bounds)) in edges.iter().zip(labels).enumerate() {if let Some([left,top,width,height])=bounds {drawing.rect([left,top,width,height],"@lt1");drawing.text(&edge.label,[left+2.0,top+2.0,width-4.0,height-4.0],13.0,&accent(index),false,TextAlign::Center);}}
    for (index,(node,position)) in nodes.iter().zip(&positions).enumerate() { drawing.shape(if variant==0 {"ellipse"} else {"roundRect"},[position[0]-100.0,position[1]-44.0,200.0,88.0],"@lt1",&accent(index)); drawing.text(&node.label,[position[0]-80.0,position[1]-24.0,160.0,52.0],20.0,"@dk1",true,TextAlign::Center); if !node.detail.is_empty() {drawing.text(&node.detail,[position[0]-100.0,position[1]+48.0,200.0,40.0],14.0,"@dk2",false,TextAlign::Center);} }
    Ok(())
}

fn pyramid(drawing: &mut Drawing, items: &[PartItem], variant: usize) -> Result<()> {
    count(items.len(),2,5)?; let tier_height=384.0/items.len() as f64; let center=380.0;
    for (index,item) in items.iter().enumerate() {
        let top=96.0+index as f64*tier_height; let fraction=index as f64/items.len() as f64; let next=(index+1) as f64/items.len() as f64;
        let (upper,lower)=if variant==1 {((1.0-fraction)*660.0,(1.0-next)*660.0)} else {((fraction*660.0).max(0.0),next*660.0)};
        drawing.polygon(vec![[center-upper/2.0,top],[center+upper/2.0,top],[center+lower/2.0,top+tier_height-8.0],[center-lower/2.0,top+tier_height-8.0]],if variant==2 {"@lt2"} else {"@lt1"},&accent(index));
        drawing.text(&format!("{:02}",index+1),[center-40.0,top+tier_height*0.4,80.0,32.0],22.0,&accent(index),true,TextAlign::Center);
        drawing.text(&item.label,[776.0,top+8.0,336.0,40.0],22.0,"@dk1",true,TextAlign::Left);
        drawing.text(&item.detail,[776.0,top+52.0,336.0,(tier_height-56.0).max(28.0)],16.0,"@dk2",false,TextAlign::Left);
        if variant==2 { drawing.line([center+lower/2.0,top+tier_height/2.0],[748.0,top+tier_height/2.0],false,"@dk2"); }
    }
    Ok(())
}

fn puzzle(drawing: &mut Drawing, items: &[PartItem], variant: usize) -> Result<()> {
    count(items.len(),2,6)?;
    for (index,item) in items.iter().enumerate() {
        let columns=if variant==0 {items.len().min(4)} else {3}; let row=index/columns; let column=index%columns;
        let width=if variant==0 {1080.0/columns as f64} else {300.0}; let left=if variant==0 {32.0+column as f64*width} else {100.0+column as f64*300.0+row as f64*70.0}; let top=112.0+row as f64*188.0;
        if variant==2 { drawing.polygon(vec![[left,top],[left+width-28.0,top],[left+width,top+34.0],[left+width-28.0,top+68.0],[left+width-28.0,top+164.0],[left,top+164.0],[left+24.0,top+82.0]],"@lt2",&accent(index)); }
        else { drawing.shape("hexagon",[left,top,width-14.0,164.0],"@lt2",&accent(index)); }
        drawing.text(&item.label,[left+42.0,top+38.0,width-98.0,50.0],20.0,"@dk1",true,TextAlign::Center);
        drawing.text(&item.detail,[left+42.0,top+92.0,width-98.0,58.0],14.0,"@dk2",false,TextAlign::Center);
    }
    Ok(())
}

fn before_after(drawing: &mut Drawing, items: &[PartItem], variant: usize) -> Result<()> {
    count(items.len(),2,2)?;
    if variant==2 { for (index,item) in items.iter().enumerate() {let top=112.0+index as f64*180.0; drawing.text(if index==0 {"Before"} else {"After"},[32.0,top+20.0,180.0,48.0],26.0,&accent(index),true,TextAlign::Left); block(drawing,item,[240.0,top,860.0,140.0],index,true);} }
    else { for (index,item) in items.iter().enumerate() {let left=40.0+index as f64*600.0; drawing.text(if index==0 {"Before"} else {"After"},[left,112.0,460.0,46.0],26.0,&accent(index),true,TextAlign::Left); block(drawing,item,[left,180.0,460.0,270.0],index,variant==0);} if variant==1 {drawing.shape("rightArrow",[522.0,280.0,92.0,60.0],"@accent1","@accent1");} }
    Ok(())
}

fn venn(drawing: &mut Drawing, items: &[PartItem], center: &str, variant: usize) -> Result<()> {
    count(items.len(),if variant==1 {3} else {2},if variant==1 {3} else {2})?;
    let positions=if variant==1 {vec![[452.0,228.0],[652.0,228.0],[552.0,352.0]]} else {vec![[424.0,286.0],[696.0,286.0]]};
    for (index,position) in positions.iter().enumerate() {drawing.shape("ellipse",[position[0]-176.0,position[1]-148.0,352.0,296.0],"none",&accent(index));}
    for (index,item) in items.iter().enumerate() {
        let (label,detail)=if variant==1 {
            if index==2 {([476.0,398.0,152.0,38.0],[472.0,442.0,160.0,36.0])}
            else {let left=if index==0 {310.0} else {642.0};([left,146.0,152.0,36.0],[left,188.0,152.0,34.0])}
        } else {let position=positions[index];let label_x=position[0]+if index==0 {-70.0} else {70.0};([label_x-100.0,position[1]-30.0,200.0,48.0],[label_x-100.0,position[1]+22.0,200.0,50.0])};
        drawing.text(&item.label,label,22.0,&accent(index),true,TextAlign::Center);
        drawing.text(&item.detail,detail,14.0,"@dk2",false,TextAlign::Center);
    }
    if variant==2 {drawing.rect([500.0,254.0,120.0,66.0],"@lt2");}
    drawing.text(center,if variant==1 {[504.0,258.0,96.0,42.0]} else {[500.0,260.0,120.0,72.0]},16.0,"@dk1",true,TextAlign::Center);
    Ok(())
}

fn groups(drawing: &mut Drawing, category: &str, variant: usize, data: &PartData) -> Result<()> {
    let PartData::Groups {groups}=data else {return Err(Error::Invalid("group data required".into()));}; count(groups.len(),2,if category=="set" {3} else {6})?;
    let columns=if variant==1 {1} else if variant==2 {groups.len()} else {groups.len().min(3)}; let rows=groups.len().div_ceil(columns); let width=1104.0/columns as f64; let height=400.0/rows as f64;
    for (index,group) in groups.iter().enumerate() {let left=24.0+(index%columns) as f64*width; let top=90.0+(index/columns) as f64*height;
        drawing.rect([left,top,width-20.0,2.0],&accent(index));
        if variant==1 {drawing.text(&group.label,[left+8.0,top+12.0,210.0,height-18.0],20.0,&accent(index),true,TextAlign::Left); let slot=(width-250.0)/group.items.len() as f64; for (item_index,item) in group.items.iter().enumerate() {drawing.text(item,[left+240.0+item_index as f64*slot,top+12.0,slot-18.0,height-18.0],17.0,"@dk1",false,TextAlign::Left);} }
        else {drawing.text(&group.label,[left+8.0,top+14.0,width-36.0,44.0],20.0,&accent(index),true,TextAlign::Left); let slot=(height-72.0)/group.items.len() as f64; for (item_index,item) in group.items.iter().enumerate() { if variant==2 && category=="set" {drawing.shape("ellipse",[left+8.0,top+72.0+item_index as f64*slot,24.0,24.0],"none",&accent(index));} drawing.text(item,[left+if variant==2 {42.0} else {12.0},top+70.0+item_index as f64*slot,width-62.0,slot-4.0],16.0,"@dk1",false,TextAlign::Left);} }
    }
    Ok(())
}

fn matrix(drawing: &mut Drawing, variant: usize, data: &PartData) -> Result<()> {
    let PartData::Matrix { rows,columns,cells }=data else {return Err(Error::Invalid("matrix data required".into()));};
    if variant==2 {let mut table=vec![std::iter::once(String::new()).chain(columns.iter().cloned()).collect::<Vec<_>>()];for (label,cells) in rows.iter().zip(cells) {table.push(std::iter::once(label.clone()).chain(cells.iter().cloned()).collect());} drawing.elements.push(Element::Table { id:drawing.id(),x:32.0,y:110.0,width:1088.0,height:350.0,rows:table,font_size:20.0,format:Default::default() });return Ok(());}
    let width=880.0/columns.len() as f64; let height=320.0/rows.len() as f64;
    for (index,column) in columns.iter().enumerate() {drawing.text(column,[228.0+index as f64*width,90.0,width-16.0,48.0],22.0,&accent(index),true,TextAlign::Center);}
    for (row_index,row) in rows.iter().enumerate() {let top=150.0+row_index as f64*height; drawing.text(row,[24.0,top+40.0,180.0,height-52.0],20.0,"@dk1",true,TextAlign::Left);for (column_index,cell) in cells[row_index].iter().enumerate() {let left=224.0+column_index as f64*width; drawing.rect([left,top,width-12.0,height-12.0],if variant==1 && column_index==columns.len()-1 {"@accent1"} else {"@lt2"}); drawing.text(cell,[left+22.0,top+30.0,width-56.0,height-58.0],22.0,if variant==1 && column_index==columns.len()-1 {"@lt1"} else {"@dk1"},false,TextAlign::Center);}}
    Ok(())
}

fn formula(drawing: &mut Drawing, items: &[PartItem], center: &str, variant: usize) -> Result<()> {
    count(items.len(),2,4)?;
    if items.iter().any(|item|item.value.is_some()) && items.iter().any(|item|item.value.is_none()) {return Err(Error::Invalid("formula values must be supplied for every term or none".into()));}
    let operator=if variant==1 {"x"} else {"+"}; let slot=1088.0/items.len() as f64;
    for (index,item) in items.iter().enumerate() {let left=32.0+index as f64*slot;let top=if variant==2 {100.0+index as f64*76.0} else {152.0};let bounds=if variant==2 {[220.0,top,800.0,60.0]} else {[left,top,slot-40.0,200.0]};
        if variant==2 {drawing.text(operator,[164.0,top,36.0,40.0],28.0,"@accent1",true,TextAlign::Center);drawing.text(&item.label,[220.0,top,300.0,46.0],22.0,"@dk1",true,TextAlign::Left);drawing.text(&item.detail,[540.0,top,480.0,54.0],17.0,"@dk2",false,TextAlign::Left);}
        else {block(drawing,item,bounds,index,true);if index+1<items.len() {drawing.text(operator,[left+slot-38.0,220.0,36.0,46.0],30.0,"@dk2",true,TextAlign::Center);}}
        if let Some(value)=item.value {drawing.text(&display(value),[bounds[0]+16.0,if variant==2 {top+36.0} else {top+140.0},bounds[2]-32.0,40.0],24.0,&accent(index),true,TextAlign::Left);}
    }
    let result=if items[0].value.is_some() {let value=if variant==1 {items.iter().map(|item|item.value.unwrap_or(0.0)).product()} else {items.iter().map(|item|item.value.unwrap_or(0.0)).sum()};super::finite(value)?;format!("{} = {}",center,display(value))} else {center.into()};
    drawing.text(&result,[120.0,436.0,912.0,64.0],28.0,"@accent1",true,TextAlign::Center); Ok(())
}

fn quantities(drawing: &mut Drawing, items: &[PartItem], category: &str, variant: usize) -> Result<()> {
    count(items.len(),if category=="grow" {3} else {2},if category=="grow" || (category=="ranking" && variant==1) {3} else {6})?;
    if items.iter().any(|item|item.value.is_none_or(|value|value<0.0)) {return Err(Error::Invalid("scale diagrams require nonnegative values".into()));}
    let mut items=items.to_vec(); if category=="ranking" {items.sort_by(|left,right|right.value.unwrap_or(0.0).total_cmp(&left.value.unwrap_or(0.0)));}
    let maximum=items.iter().filter_map(|item|item.value).fold(0.0,f64::max); if maximum<=0.0 {return Err(Error::Invalid("scale diagrams require a positive maximum".into()));}
    if category=="grow" {if items.windows(2).any(|pair|pair[0].value<pair[1].value) {return Err(Error::Invalid("TAM >= SAM >= SOM is required".into()));} return market(drawing,&items,variant,maximum);}
    let slot=1080.0/items.len() as f64;
    for (index,item) in items.iter().enumerate() {let value=item.value.unwrap_or(0.0);let ratio=value/maximum;let shade=accent(index);let left=36.0+index as f64*slot;
        if (category=="scale-contrast" && variant==1) || (category=="ranking" && variant!=1) {let top=100.0+index as f64*376.0/items.len() as f64;
            let rank=items.iter().position(|candidate|candidate.value==item.value).unwrap_or(index)+1;let label=if category=="ranking" {format!("{rank:02}  {}",item.label)} else {item.label.clone()}; drawing.text(&label,[28.0,top,240.0,46.0],19.0,"@dk1",true,TextAlign::Left);
            if variant!=2 {drawing.rect([288.0,top+8.0,700.0*ratio,28.0],&shade);}else{drawing.rect([288.0,top+48.0,700.0,1.0],"@lt2");drawing.text(&item.detail,[288.0,top,660.0,44.0],17.0,"@dk2",false,TextAlign::Left);}
            drawing.text(&display(value),[1004.0,top,116.0,44.0],24.0,&shade,true,TextAlign::Right);
        } else {
            let size=(slot-44.0).min(256.0)*ratio.sqrt();let top=414.0-size;
            if category=="ranking" {let height=250.0*ratio;let rank=items.iter().position(|candidate|candidate.value==item.value).unwrap_or(index)+1;drawing.rect([left+24.0,414.0-height,slot-48.0,height],"@lt2");drawing.text(&format!("#{rank:02}"),[left,382.0-height,slot,30.0],22.0,&shade,true,TextAlign::Center);}
            else if size>0.0 {drawing.shape(if variant==2 {"rect"} else {"ellipse"},[left+(slot-size)/2.0,top,size,size],"@lt2",&shade);}
            drawing.text(&display(value),[left,420.0,slot,36.0],24.0,&shade,true,TextAlign::Center);drawing.text(&item.label,[left+8.0,458.0,slot-16.0,42.0],18.0,"@dk1",true,TextAlign::Center);
        }
    }
    Ok(())
}

fn market(drawing: &mut Drawing, items: &[PartItem], variant: usize, maximum: f64) -> Result<()> {
    for (index,item) in items.iter().enumerate() {let value=item.value.unwrap_or(0.0);let shade=accent(index);
        if variant==2 {let top=112.0+index as f64*120.0;drawing.text(&item.label,[24.0,top,164.0,42.0],22.0,&shade,true,TextAlign::Left);drawing.rect([220.0,top+8.0,716.0*value/maximum,30.0],&shade);drawing.text(&display(value),[960.0,top,160.0,42.0],28.0,"@dk1",true,TextAlign::Right);drawing.text(&item.detail,[220.0,top+54.0,716.0,36.0],17.0,"@dk2",false,TextAlign::Left);}
        else {let size=360.0*(value/maximum).sqrt();if size>0.0 {drawing.shape(if variant==0 {"ellipse"} else {"rect"},[360.0-size/2.0,472.0-size,size,size],"@lt1",&shade);}let top=116.0+index as f64*120.0;drawing.text(&format!("{}  {}",item.label,display(value)),[764.0,top,340.0,50.0],26.0,&shade,true,TextAlign::Left);drawing.text(&item.detail,[764.0,top+54.0,340.0,48.0],18.0,"@dk2",false,TextAlign::Left);}
    }
    Ok(())
}

fn layers(drawing: &mut Drawing, items: &[PartItem], steps: bool, variant: usize) -> Result<()> {
    count(items.len(),2,5)?;
    if steps {let slot=1080.0/items.len() as f64;for (index,item) in items.iter().enumerate() {let left=32.0+index as f64*slot;let top=376.0-index as f64*54.0;
        if variant==0 {drawing.rect([left,top,slot-10.0,100.0+index as f64*54.0],"@lt2");} else {drawing.line([left,top+48.0],[left+slot-10.0,top+48.0],false,&accent(index));if variant==1 {drawing.shape("ellipse",[left+12.0,top+28.0,40.0,40.0],"@lt1",&accent(index));}}
        drawing.text(&item.label,[left+14.0,top-70.0,slot-36.0,64.0],20.0,"@dk1",true,TextAlign::Left);drawing.text(&item.detail,[left+14.0,top+74.0,slot-36.0,50.0],14.0,"@dk2",false,TextAlign::Left);drawing.text(&(index+1).to_string(),[left+16.0,top+8.0,slot-36.0,36.0],24.0,&accent(index),true,TextAlign::Left);
    }} else {let row=380.0/items.len() as f64;for (index,item) in items.iter().enumerate() {let top=100.0+index as f64*row;let left=32.0+if variant==1 {index as f64*26.0} else {0.0};let width=if variant==2 {664.0} else {1000.0};
        if variant==1 {drawing.polygon(vec![[left,top+16.0],[left+width-32.0,top],[left+width,top+row-18.0],[left+32.0,top+row-4.0]],"@lt2",&accent(index));} else {drawing.rect([left,top,width,row-12.0],"@lt2");drawing.rect([left,top,4.0,row-12.0],&accent(index));}
        drawing.text(&item.label,[left+22.0,top+14.0,300.0,row-28.0],21.0,"@dk1",true,TextAlign::Left);drawing.text(&item.detail,[if variant==2 {744.0} else {left+360.0},top+16.0,if variant==2 {376.0} else {600.0},row-28.0],16.0,"@dk2",false,TextAlign::Left);
    }} Ok(())
}

fn triangle(drawing: &mut Drawing, items: &[PartItem], center: &str, variant: usize) -> Result<()> {
    count(items.len(),3,3)?;let corners=[[576.0,110.0],[984.0,432.0],[168.0,432.0]];let middle=[576.0,320.0];
    if variant==1 {for index in 0..3 {drawing.polygon(vec![middle,corners[index],corners[(index+1)%3]],"@lt2",&accent(index));}}
    else {for index in 0..3 {drawing.line(corners[index],corners[(index+1)%3],variant==2,&accent(index));}}
    for (index,item) in items.iter().enumerate() {let corner=corners[index];let top=if index==0 {90.0} else {380.0};drawing.rect([corner[0]-132.0,top,264.0,112.0],"@lt1");drawing.text(&item.label,[corner[0]-116.0,top+4.0,232.0,40.0],22.0,&accent(index),true,TextAlign::Center);drawing.text(&item.detail,[corner[0]-116.0,top+48.0,232.0,62.0],16.0,"@dk2",false,TextAlign::Center);}
    drawing.text(center,[440.0,282.0,272.0,62.0],22.0,"@dk1",true,TextAlign::Center); Ok(())
}

fn timeline(drawing: &mut Drawing, variant: usize, data: &PartData) -> Result<()> {
    let PartData::Timeline { periods,tasks }=data else {return Err(Error::Invalid("timeline data required".into()));};let column=832.0/periods.len() as f64;let row=328.0/tasks.len() as f64;
    for (index,period) in periods.iter().enumerate() {let left=288.0+index as f64*column;drawing.text(period,[left,98.0,column,36.0],16.0,"@dk2",false,TextAlign::Center);drawing.rect([left,146.0,1.0,328.0],"@lt2");}
    for (index,task) in tasks.iter().enumerate() {let top=148.0+index as f64*row;let left=288.0+task.start as f64*column;let width=(task.end-task.start) as f64*column;let shade=accent(index);drawing.text(&task.label,[24.0,top+8.0,238.0,row-8.0],18.0,"@dk1",true,TextAlign::Left);drawing.rect([left,top+10.0,width,row-20.0],if variant==1 {"@lt2"} else {&shade});
        if variant==1 {drawing.rect([left,top+10.0,width*task.progress,row-20.0],&accent(index));drawing.text(&format!("{}%",display(task.progress*100.0)),[left+8.0,top+16.0,width-16.0,row-26.0],14.0,"@dk1",true,TextAlign::Center);}
        if variant==2 {drawing.shape("diamond",[left+width-12.0,top+row/2.0-12.0,24.0,24.0],"@lt1",&accent(index));}
    } Ok(())
}

fn map(drawing: &mut Drawing, variant: usize, data: &PartData) -> Result<()> {
    let PartData::Map { points }=data else {return Err(Error::Invalid("map data required".into()));};
    let land:serde_json::Value=serde_json::from_str(include_str!("world-land.geojson"))?;
    let width=if variant==2 {736.0} else {768.0};let left=if variant==2 {24.0} else {160.0};let top=96.0;
    let project=|longitude:f64,latitude:f64| [left+(longitude+180.0)/360.0*width,top+(90.0-latitude)/180.0*(width/2.0)];
    let features=land["features"].as_array().ok_or_else(||Error::Invalid("bundled land features".into()))?;
    for feature in features {let geometry=&feature["geometry"];let coordinates=geometry["coordinates"].as_array().ok_or_else(||Error::Invalid("bundled land coordinates".into()))?;
        let polygons:Vec<_>=if geometry["type"]=="Polygon" {vec![&geometry["coordinates"]]} else {coordinates.iter().collect()};
        for polygon in polygons {let ring=polygon[0].as_array().ok_or_else(||Error::Invalid("bundled land polygon".into()))?;let ring=ring.iter().map(|point| {let longitude=point[0].as_f64().ok_or_else(||Error::Invalid("bundled longitude".into()))?;let latitude=point[1].as_f64().ok_or_else(||Error::Invalid("bundled latitude".into()))?;Ok(project(longitude,latitude))}).collect::<Result<Vec<_>>>()?;drawing.polygon(ring,"@lt2","@lt2");}
    }
    let maximum=points.iter().filter_map(|point|point.value).fold(0.0,f64::max).max(1.0);
    for (index,point) in points.iter().enumerate() {let position=project(point.longitude,point.latitude);let size=if variant==1 {48.0*(point.value.ok_or_else(||Error::Invalid("proportional markers require values".into()))?/maximum).sqrt()} else {12.0};if size>0.0 {drawing.shape("ellipse",[position[0]-size/2.0,position[1]-size/2.0,size,size],"@lt1",&accent(index));}
        if variant==2 {let row=96.0+index as f64*46.0;drawing.text(&format!("{}  {}",index+1,point.label),[824.0,row,296.0,40.0],18.0,&accent(index),true,TextAlign::Left);drawing.text(&(index+1).to_string(),[position[0]-12.0,position[1]-12.0,24.0,26.0],14.0,"@dk1",true,TextAlign::Center);}
        else {drawing.text(&point.label,[(position[0]-60.0).clamp(24.0,992.0),position[1]+size/2.0+8.0,136.0,34.0],16.0,&accent(index),true,TextAlign::Center);}
    }
    drawing.text("Natural Earth 1:110m / equirectangular / land outlines",[24.0,484.0,1096.0,24.0],12.0,"@dk2",false,TextAlign::Left);Ok(())
}