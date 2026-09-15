use super::{PartSpec, PartData};
use serde_json::{json, Value};

pub(super) const CATEGORIES: &[(&str, &str, [&str; 3])] = &[
    ("pie-chart","Pie / doughnut",["Doughnut overview","Full pie","Composition ledger"]),
    ("vertical-bar-graph","Column chart",["Balanced columns","Metric focus","Data ledger"]),
    ("add-vertical-bar-graph","Stacked columns",["Stack overview","Series focus","Composition ledger"]),
    ("100-add-vertical-bar-graph","100% stacked columns",["Share overview","Share focus","Raw data ledger"]),
    ("horizontal-bar-graph","Bar chart",["Balanced bars","Metric focus","Data ledger"]),
    ("area-graph","Area chart",["Area overview","Trend focus","Data ledger"]),
    ("water-fall","Waterfall",["Running bridge","Total emphasis","Change ledger"]),
    ("line-graph","Line chart",["Trend overview","Latest value","Data ledger"]),
    ("infographic","Pictogram",["Unit dots","Unit tiles","Progress strips"]),
    ("scatter","Scatter plot",["Numeric axes","Point focus","Coordinate ledger"]),
    ("tree","Tree",["Top-down hierarchy","Left-to-right tree","Hierarchy outline"]),
    ("pyramid","Pyramid",["Tiered pyramid","Inverted funnel","Pyramid annotations"]),
    ("flow","Horizontal flow",["Connected stages","Chevron ribbon","Milestone rail"]),
    ("vertical-flow","Vertical flow",["Downward stages","Timeline spine","Numbered handoffs"]),
    ("cycle","Cycle",["Circular loop","Loop with center","Cycle annotations"]),
    ("before-after","Before / after",["Side-by-side","Transition arrow","Paired rows"]),
    ("map","Geographic map",["Global locations","Proportional markers","Location ledger"]),
    ("puzzle","Puzzle / honeycomb",["Honeycomb row","Hexagon cluster","Interlocking blocks"]),
    ("radiation","Radial",["Hub and spokes","Orbit nodes","Hub annotations"]),
    ("correlation","Relationship",["Circular network","Two-column network","Tiered network"]),
    ("matrix","Matrix",["Labeled grid","Quadrant emphasis","Comparison table"]),
    ("venn","Venn",["Two-set overlap","Three-set overlap","Intersection focus"]),
    ("cross","Formula",["Addition","Multiplication","Equation ledger"]),
    ("set","Small groups",["Group columns","Group rows","Grouped circles"]),
    ("list-set","Many groups",["Category grid","Category lanes","Directory columns"]),
    ("contrast","Item comparison",["Side-by-side rows","Highlighted alternative","Comparison table"]),
    ("scale-contrast","Scale comparison",["Area-scaled circles","Proportional lengths","Area-scaled squares"]),
    ("grow","TAM / SAM / SOM",["Nested circles","Nested rectangles","Shared baseline"]),
    ("layer","Layers",["Stacked bands","Offset planes","Layer annotations"]),
    ("triangle","Triangle",["Three corners","Triangular sectors","Three-sided relationship"]),
    ("step","Steps",["Staircase","Step milestones","Step annotations"]),
    ("gantt-chart","Gantt chart",["Schedule bars","Progress schedule","Milestone schedule"]),
    ("list","Vertical list",["Numbered rows","Accent rail","Two-column list"]),
    ("list-horizontal","Horizontal list",["Editorial columns","Numbered columns","Accent bands"]),
    ("list-enumeration","Enumeration",["Label grid","Indexed grid","Two-column index"]),
    ("ranking","Ranking",["Ranked bars","Podium","Rank ledger"]),
];

pub fn catalog() -> Value {
    let presets: Vec<_> = CATEGORIES.iter().enumerate().flat_map(|(index,(category,name,names))| {
        ["balanced","focus","labeled"].into_iter().enumerate().map(move |(variant,suffix)| {
            let id = format!("{category}/{suffix}");
            let example = PartSpec { version:1,preset:id.clone(),title:(*name).into(),subtitle:"Synthetic example".into(),data:example(category,variant) };
            json!({"id":id,"category":category,"category_name":name,"name":names[variant],"family":if index<10 {"Charts"} else {"Diagrams"},"example":example})
        })
    }).collect();
    json!({"version":1,"presets":presets,"schema":schemars::schema_for!(PartSpec),"style":"Theme-linked minimal modern","default_bounds":{"x":64,"y":144,"width":1152,"height":512}})
}

fn example(category: &str, variant: usize) -> PartData {
    let value = match category {
        "tree" => json!({"kind":"tree","nodes":[{"id":"root","label":"Strategy"},{"id":"one","label":"Product","parent":"root"},{"id":"two","label":"Operations","parent":"root"},{"id":"three","label":"Experience","parent":"one"},{"id":"four","label":"Platform","parent":"one"}]}),
        "correlation" => json!({"kind":"network","nodes":[{"label":"Customers"},{"label":"Platform"},{"label":"Partners"},{"label":"Operations"}],"edges":[{"from":0,"to":1,"label":"Request"},{"from":1,"to":2,"label":"Connect"},{"from":2,"to":3,"label":"Deliver"},{"from":3,"to":0,"label":"Support"}]}),
        "matrix" | "contrast" => json!({"kind":"matrix","rows":["Speed","Control"],"columns":["Option A","Option B"],"cells":[["High","Medium"],["Shared","Dedicated"]]}),
        "set" | "list-set" => { let length = if category=="set" {3} else {6}; json!({"kind":"groups","groups":(0..length).map(|index| json!({"label":format!("Domain {}",index+1),"items":["Plan","Build","Measure"]})).collect::<Vec<_>>()}) }
        "gantt-chart" => json!({"kind":"timeline","periods":["Jan","Feb","Mar","Apr","May","Jun"],"tasks":[{"label":"Discover","start":0,"end":2,"progress":1},{"label":"Design","start":1,"end":3,"progress":0.7},{"label":"Build","start":2,"end":5,"progress":0.35},{"label":"Launch","start":4,"end":6,"progress":0}]}),
        "water-fall" => json!({"kind":"waterfall","unit":"Units","steps":[{"label":"Start","value":100,"total":true},{"label":"Growth","value":30},{"label":"Cost","value":-20},{"label":"Mix","value":10},{"label":"Finish","value":120,"total":true}]}),
        "map" => json!({"kind":"map","points":[{"label":"Tokyo","longitude":139.69,"latitude":35.68,"value":40},{"label":"London","longitude":-0.12,"latitude":51.5,"value":25},{"label":"New York","longitude":-74.0,"latitude":40.71,"value":35}]}),
        "grow" => json!({"kind":"items","items":[{"label":"TAM","detail":"Total addressable","value":100},{"label":"SAM","detail":"Serviceable","value":45},{"label":"SOM","detail":"Obtainable","value":12}]}),
        "scale-contrast" | "ranking" | "infographic" => json!({"kind":"items","items":[{"label":"Segment A","value":70,"detail":"Illustrative value"},{"label":"Segment B","value":40,"detail":"Illustrative value"},{"label":"Segment C","value":25,"detail":"Illustrative value"}]}),
        "pie-chart"|"vertical-bar-graph"|"add-vertical-bar-graph"|"100-add-vertical-bar-graph"|"horizontal-bar-graph"|"area-graph"|"line-graph"|"scatter" => {
            let mut series = vec![json!({"name":"Primary","values":[12,24,18,30]})];
            if category.contains("add-vertical") { series.push(json!({"name":"Secondary","values":[18,12,24,15]})); }
            json!({"kind":"chart","categories":if category=="scatter" {vec!["10","20","30","40"]} else {vec!["Q1","Q2","Q3","Q4"]},"series":series,"x_axis":if category=="scatter" {"Input"} else if category=="horizontal-bar-graph" {"Units"} else {"Quarter"},"y_axis":if category=="horizontal-bar-graph" {"Quarter"} else {"Units"}})
        }
        _ => { let length = if category=="before-after" || (category=="venn" && variant!=1) {2} else if category=="triangle" || category=="cross" || category=="pyramid" || category=="venn" {3} else {4}; json!({"kind":"items","center":"Shared outcome","items":(0..length).map(|index| json!({"label":(["Discover","Design","Deliver","Improve"][index%4]),"detail":"A clear, concise description"})).collect::<Vec<_>>()}) }
    };
    serde_json::from_value(value).expect("built-in part data is valid")
}