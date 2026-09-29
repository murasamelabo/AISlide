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
    ("tree","Tree",["Hierarchy blocks","Branch columns","Hierarchy outline"]),
    ("pyramid","Pyramid",["Tiered pyramid","Inverted funnel","Pyramid annotations"]),
    ("flow","Horizontal flow",["Stage panels","Chevron ribbon","Process columns"]),
    ("vertical-flow","Vertical flow",["Downward panels","Arrow stages","Numbered lanes"]),
    ("cycle","Cycle",["Segmented cycle","Cycle with hub","Cycle ledger"]),
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

pub(super) const OPEN_LISTS: &[(&str, &str, &str, &str, &str)] = &[
    ("list", "rows", "Open text rows", "2-8 equal-status topics with a heading and explanation; read down a shared heading/detail alignment.", "Ordered stages, rankings or measured comparisons; use a flow, ranking or comparison instead."),
    ("list-horizontal", "columns", "Open editorial columns", "2-4 parallel concepts with comparable detail; read across aligned headings without enclosing cards.", "A sequence, unequal priorities or more than four topics; do not imply chronology from left-to-right placement."),
    ("list-enumeration", "grid", "Open text grid", "2-8 unordered peers with short labels and descriptions; group by whitespace, not item colors.", "Steps, ranks or categories that need an explicit relationship; choose the matching diagram instead."),
];

pub fn catalog() -> Value {
    let mut presets: Vec<_> = CATEGORIES.iter().enumerate().flat_map(|(index,(category,name,names))| {
        ["balanced","focus","labeled"].into_iter().enumerate().map(move |(variant,suffix)| {
            let id = format!("{category}/{suffix}");
            let example = PartSpec { version:1,preset:id.clone(),title:(*name).into(),subtitle:"Synthetic example".into(),data:example(category,variant),layout:None };
            json!({"id":id,"category":category,"category_name":name,"name":names[variant],"family":if index<10 {"Charts"} else {"Diagrams"},"example":example})
        })
    }).collect();
    presets.extend(OPEN_LISTS.iter().map(|(category, variant, name, use_when, avoid_when)| {
        let id = format!("{category}/{variant}");
        let category_name = CATEGORIES.iter().find(|entry| entry.0 == *category).expect("built-in list category").1;
        let example = PartSpec { version:1,preset:id.clone(),title:(*name).into(),subtitle:"Synthetic example".into(),data:PartData::Items {
            center:String::new(), items:[("Collection","Bring relevant signals together."),("Coordination","Connect decisions across teams."),("Judgment","Keep priorities and outcomes explicit."),("Learning","Use feedback to improve the next response.")]
                .into_iter().map(|(label,detail)| super::PartItem { label:label.into(),detail:detail.into(),value:None }).collect(),
        },layout:None };
        json!({"id":id,"category":category,"category_name":category_name,"name":name,"family":"Diagrams","recommended":true,"use_when":use_when,"avoid_when":avoid_when,"example":example})
    }));
    let example = PartSpec { version: 1, preset: "contrast/panels".into(), title: "Paired explanatory panels".into(), subtitle: "Synthetic example".into(), data: example("contrast", 3), layout: None };
    presets.push(json!({"id":"contrast/panels","category":"contrast","category_name":"Item comparison","name":"Paired explanatory panels","family":"Diagrams","recommended":true,
        "use_when":"Two alternatives explained through 1-5 paired statements. Tint each panel, use supplied meaningful icons, and show a transition arrow only when the relationship warrants it. Fixed typography rejects overflow.",
        "avoid_when":"Three or more alternatives, numeric evaluation matrices or dense tabular data. Do not convert every comparison or every slide into cards.","example":example}));
    presets.extend(super::briefing::PRESETS.iter().map(|preset| {
        let category = preset.id.split_once('/').expect("briefing preset category").0;
        let category_name = CATEGORIES.iter().find(|entry| entry.0 == category).expect("built-in briefing category").1;
        let example = PartSpec { version: 1, preset: preset.id.into(), title: preset.name.into(), subtitle: "Synthetic example".into(), data: briefing_example(preset.kind), layout: None };
        json!({"id":preset.id,"category":category,"category_name":category_name,"name":preset.name,"family":"Diagrams","recommended":true,"use_when":preset.use_when,"avoid_when":preset.avoid_when,"example":example})
    }));
    json!({"version":1,"presets":presets,"schema":schemars::schema_for!(PartSpec),"style":"Theme-linked minimal modern","default_bounds":{"x":64,"y":144,"width":1152,"height":512}})
}

fn briefing_example(kind: &str) -> PartData {
    let value = match kind {
        "icon_cards" => json!({"kind":"icon_cards","numbered":true,"message":{"text":"People set direction; automation extends reach."},"cards":[
            {"label":"Unified operations","caption":"Shared platform","detail":"Signals, context and controls are managed together.","tag":"Part 2"},
            {"label":"Continuous protection","caption":"Closed loop","detail":"Detection, response and prevention reinforce each other.","tag":"Part 3"},
            {"label":"Agent collaboration","caption":"Operating model","detail":"Agents coordinate routine work while people set priorities.","tag":"Part 4"}
        ]}),
        "icon_rows" => json!({"kind":"icon_rows","rows":[
            {"label":"Faster discovery","detail":"Exposure is mapped continuously and prioritized by impact."},
            {"label":"Machine-speed response","detail":"Routine containment runs within approved policy boundaries."},
            {"label":"Wider scope for small teams","detail":"Specialists supervise many parallel investigations."}
        ]}),
        "shift_rows" => json!({"kind":"shift_rows","from_label":"Today","to_label":"Next","rows":[
            {"from":"Manage tasks","to":"Govern outcomes","caption":"Outcome owner","detail":"Measure risk removed instead of work completed."},
            {"from":"Respond at human speed","to":"Defend at machine speed","caption":"Policy-bound response","detail":"Contain routine threats within approved policy."},
            {"from":"Count processed alerts","to":"Measure reduced risk","caption":"Outcome metrics","detail":"Track exposure closed and attack paths blocked."}
        ]}),
        "step_cards" => json!({"kind":"step_cards","steps":[
            {"label":"Establish the foundation","detail":"Connect identities, endpoints, cloud and data into a shared context.","outcome":"Shared view before orchestration"},
            {"label":"Embed agents in workflows","detail":"Start with well-defined investigation and triage tasks.","outcome":"Capacity before autonomy"},
            {"label":"Expand to a system","detail":"Coordinate agents toward protection goals across services.","outcome":"Specialists with shared context"}
        ]}),
        _ => json!({"kind":"agenda","items":[
            {"label":"Background and direction","detail":"Why the operating model is changing","meta":"10 min"},
            {"label":"Platform overview","detail":"Shared context, controls and data","meta":"15 min"},
            {"label":"Operating model","detail":"Roles, metrics and collaboration","meta":"15 min"},
            {"label":"Adoption path","detail":"Three steps and review points","meta":"10 min"}
        ]}),
    };
    serde_json::from_value(value).expect("built-in briefing data is valid")
}

fn example(category: &str, variant: usize) -> PartData {
    let value = match category {
        "contrast" if variant == 3 => json!({"kind":"comparison_panels","transition":false,"panels":[
            {"label":"Separate workflows","items":[{"text":"Signals are collected separately."},{"text":"Context is rebuilt at each handoff."},{"text":"Actions require coordination."},{"text":"Ownership is distributed."}]},
            {"label":"Shared workflow","items":[{"text":"Signals are available together."},{"text":"Context follows the investigation."},{"text":"Actions share the same context."},{"text":"Ownership remains explicit."}]}
        ]}),
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