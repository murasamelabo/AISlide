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
        let position = CATEGORIES.iter().position(|entry| entry.0 == category).expect("built-in briefing category");
        let category_name = CATEGORIES[position].1;
        let example = PartSpec { version: 1, preset: preset.id.into(), title: preset.name.into(), subtitle: "Synthetic example".into(), data: briefing_example(preset.kind), layout: None };
        let mut entry = json!({"id":preset.id,"category":category,"category_name":category_name,"name":preset.name,"family":if position < 10 {"Charts"} else {"Diagrams"},"recommended":true,"use_when":preset.use_when,"avoid_when":preset.avoid_when,"example":example});
        if preset.specialized { entry["specialized"] = json!(true); }
        entry
    }));
    json!({"version":1,"presets":presets,"schema":schemars::schema_for!(PartSpec),"style":"Theme-linked minimal modern","default_bounds":{"x":64,"y":144,"width":1152,"height":512}})
}

fn briefing_example(kind: &str) -> PartData {
    let value = match kind {
        "open_steps" | "rail_steps" => json!({"kind":kind,"steps":[
            {"label":"Collect","detail":"Gather the approved evidence."},
            {"label":"Assess","detail":"Choose the next action."},
            {"label":"Respond","detail":"Record the outcome."}
        ]}),
        "roadmap" => json!({"kind":"roadmap","phases":[
            {"period":"Now","label":"Measure the baseline","detail":"Start with the highest-impact gaps.","points":["Confirm owners","Check exposure"],"outcome":"Baseline agreed"},
            {"period":"Next quarter","label":"Build the capability","detail":"Extend the same controls to more teams.","points":["Deploy controls","Rehearse recovery"],"outcome":"Repeatable operations"},
            {"period":"Later","label":"Make improvement routine","detail":"Review changes with leadership.","points":["Track evidence","Revisit priorities"],"outcome":"Continuous improvement"}
        ]}),
        "icon_columns" => json!({"kind":"icon_columns","items":[
            {"label":"Exposure","detail":"Understand the assets and gaps."},
            {"label":"Identity","detail":"Protect access at every boundary."},
            {"label":"Recovery","detail":"Practice restoring service safely."}
        ]}),
        "fact_columns" => json!({"kind":"fact_columns","items":[
            {"value":"24","unit":"teams","label":"Coverage","detail":"Teams included in this synthetic example.","qualifier":"Not a measured production value."},
            {"value":"85","unit":"%","label":"Completion","detail":"A separate illustrative measure.","qualifier":"Its denominator differs from coverage."},
            {"value":"12","unit":"days","label":"Elapsed time","detail":"An independently supplied duration.","qualifier":"Not visually compared with the other facts."}
        ]}),
        "image_columns" => {
            let PartData::ScreenshotCallouts { image, .. } = briefing_example("screenshot_callouts") else { unreachable!("built-in image example") };
            json!({"kind":"image_columns","items":[
                {"image":image,"label":"Evidence A","detail":"Approved source image kept in full.","caption":"Synthetic sample image"},
                {"image":image,"label":"Evidence B","detail":"A separate observation, not a stage.","caption":"Synthetic sample image"},
                {"image":image,"label":"Evidence C","detail":"Explain the visible evidence.","caption":"Synthetic sample image"}
            ]})
        },
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
        "screenshot_callouts" => json!({"kind":"screenshot_callouts","image":{"base64":placeholder_screenshot(),"mime_type":"image/png","alt":"Synthetic settings screen"},"callouts":[
            {"x":0.30,"y":0.34,"label":"Turn on the method","detail":"Enable the setting before choosing targets."},
            {"x":0.12,"y":0.50,"label":"Choose targets","detail":"Include groups; exclusions take precedence."},
            {"x":0.62,"y":0.70,"label":"Assign a profile","detail":"Pick the profile applied to each target."}
        ]}),
        "kpi_cards" => json!({"kind":"kpi_cards","message":{"text":"Revenue and margin beat plan; churn needs attention."},"cards":[
            {"label":"Revenue","value":"12.4","unit":"B","delta":"+8.2% YoY","status":"good","comparison":"Plan 12.0"},
            {"label":"Operating margin","value":"14.8","unit":"%","delta":"+1.1 pt","status":"good","comparison":"Plan 14.0%"},
            {"label":"Monthly churn","value":"2.9","unit":"%","delta":"+0.4 pt","status":"bad","comparison":"Target 2.5%"},
            {"label":"Active customers","value":"48.2k","delta":"+3.1k QoQ","comparison":"Target 50k"}
        ]}),
        "bullet_graphs" => json!({"kind":"bullet_graphs","rows":[
            {"label":"Revenue","actual":12.4,"target":12.0,"ranges":[9,11,14],"unit":"B","note":"Ahead of plan"},
            {"label":"Gross margin","actual":38,"target":40,"ranges":[30,36,45],"unit":"%","note":"Mix shift toward services"},
            {"label":"Net promoter score","actual":42,"target":45,"ranges":[20,35,60]},
            {"label":"Lead time","actual":6.5,"target":5,"ranges":[4,7,10],"unit":"d","lower_is_better":true,"note":"Lower is better"}
        ]}),
        "variance" => json!({"kind":"variance","unit":"M","total_label":"Operating profit","message":{"text":"Sales and SG&A savings offset higher cost of sales."},"rows":[
            {"label":"Product sales","plan":820,"actual":865},
            {"label":"Services","plan":310,"actual":296},
            {"label":"Cost of sales","plan":-540,"actual":-561},
            {"label":"SG&A","plan":-380,"actual":-352}
        ]}),
        "harvey_matrix" => json!({"kind":"harvey_matrix","columns":["Speed","Cost","Control","Scalability"],"legend":["None","Low","Partial","High","Full"],
            "message":{"text":"The partner platform balances speed and scalability."},"rows":[
            {"label":"Build in-house","levels":[1,2,4,3]},
            {"label":"Buy a package","levels":[4,3,2,2]},
            {"label":"Partner platform","levels":[3,3,3,4]}
        ]}),
        "heatmap" => json!({"kind":"heatmap","unit":"%","columns":["Jan","Feb","Mar","Apr","May","Jun"],"rows":[
            {"label":"North","values":[92,94,95,91,96,97]},
            {"label":"East","values":[88,86,84,83,85,87]},
            {"label":"West","values":[95,96,94,97,98,97]},
            {"label":"South","values":[81,79,82,84,86,88]}
        ]}),
        "raci" => json!({"kind":"raci","roles":["Product","Engineering","Security","Support"],"legend":["Responsible","Accountable","Consulted","Informed"],"tasks":[
            {"label":"Define requirements","assignments":["A/R","C","C","I"]},
            {"label":"Build and test","assignments":["C","A/R","C",""]},
            {"label":"Security review","assignments":["I","R","A",""]},
            {"label":"Release communication","assignments":["A","I","I","R"]}
        ]}),
        "risk_matrix" => json!({"kind":"risk_matrix","likelihood_label":"Likelihood","impact_label":"Impact","zone_labels":["Critical","High","Medium","Low"],"risks":[
            {"id":"R1","label":"Key supplier delay","likelihood":4,"impact":5,"action":"Qualify a second supplier by Q3"},
            {"id":"R2","label":"Data migration errors","likelihood":3,"impact":4,"action":"Run two rehearsal migrations"},
            {"id":"R3","label":"Adoption below plan","likelihood":3,"impact":3,"action":"Train champions in each team"},
            {"id":"R4","label":"Budget overrun","likelihood":2,"impact":3,"action":"Review costs monthly"},
            {"id":"R5","label":"Regulatory change","likelihood":1,"impact":5,"action":"Monitor consultation papers"}
        ]}),
        "pareto" => json!({"kind":"pareto","value_label":"Defects","cumulative_label":"Cumulative share","message":{"text":"Two causes explain 69% of defects."},"items":[
            {"label":"Labeling","value":42},{"label":"Packaging","value":27},{"label":"Scratches","value":14},
            {"label":"Missing parts","value":9},{"label":"Color","value":5},{"label":"Other","value":3}
        ]}),
        "control_chart" => json!({"kind":"control_chart","series_label":"Cycle time (min)",
            "labels":["W1","W2","W3","W4","W5","W6","W7","W8","W9","W10","W11","W12","W13","W14","W15","W16"],
            "values":[12.1,11.8,12.4,12.0,11.6,12.3,12.8,12.2,11.9,12.5,13.9,12.1,11.7,12.0,12.4,12.2]}),
        "fishbone" => json!({"kind":"fishbone","effect":"Late deliveries","message":{"text":"Handovers and manual entry are the first two fixes."},"categories":[
            {"label":"People","causes":[{"text":"New staff onboarding"},{"text":"Shift handover gaps","focus":true}]},
            {"label":"Process","causes":[{"text":"Manual order entry","focus":true},{"text":"Unclear escalation"}]},
            {"label":"Systems","causes":[{"text":"Overnight batch updates"},{"text":"No stock alerts"}]},
            {"label":"Suppliers","causes":[{"text":"Variable lead times"},{"text":"Single-source parts"}]}
        ]}),
        "swimlane" => json!({"kind":"swimlane","lanes":["Customer","Sales","Operations"],"steps":[
            {"id":"order","label":"Place order","lane":0,"shape":"event"},
            {"id":"check","label":"Check credit","lane":1},
            {"id":"approve","label":"Approved?","lane":1,"shape":"decision"},
            {"id":"ship","label":"Pick and ship","lane":2},
            {"id":"receive","label":"Receive goods","lane":0,"shape":"event"}
        ],"flows":[
            {"from":"order","to":"check"},{"from":"check","to":"approve"},{"from":"approve","to":"ship","label":"Yes"},
            {"from":"ship","to":"receive"},{"from":"approve","to":"order","label":"No","exception":true}
        ]}),
        "sankey" => json!({"kind":"sankey","unit":"k","nodes":[
            {"id":"search","label":"Search"},{"id":"ads","label":"Ads"},{"id":"referral","label":"Referral"},{"id":"visit","label":"Site visits"},
            {"id":"trial","label":"Trials"},{"id":"left","label":"Left"},{"id":"paid","label":"Paid"},{"id":"lapsed","label":"Lapsed"}
        ],"links":[
            {"from":"search","to":"visit","value":48},{"from":"ads","to":"visit","value":32},{"from":"referral","to":"visit","value":20},
            {"from":"visit","to":"trial","value":35},{"from":"visit","to":"left","value":65},
            {"from":"trial","to":"paid","value":14},{"from":"trial","to":"lapsed","value":21}
        ]}),
        "journey" => json!({"kind":"journey","stages":["Discover","Compare","Sign up","Onboard","Renew"],"emotions":[1,0,-2,-1,2],
            "emotion_notes":["Curious","Unsure","Form too long","Needs setup help","Clear value"],"highlight":2,"rows":[
            {"label":"Actions","cells":["Reads reviews","Compares plans","Fills in the form","Imports data","Reviews usage"]},
            {"label":"Touchpoints","cells":["Search and blog","Pricing page","Sign-up form","Setup wizard","Account team"]},
            {"label":"Opportunities","boxed":true,"cells":["Customer stories","Plan finder","Three-field sign-up","Guided import","Usage digest"]}
        ]}),
        "architecture" => json!({"kind":"architecture","system":"Order platform","elements":[
            {"id":"customer","label":"Customer","kind":"person","detail":"Places and tracks orders"},
            {"id":"web","label":"Web app","kind":"container","detail":"Storefront UI"},
            {"id":"api","label":"Order API","kind":"container","detail":"Validates orders"},
            {"id":"worker","label":"Fulfilment","kind":"container","detail":"Queues shipments"},
            {"id":"db","label":"Order database","kind":"database","detail":"Relational store"},
            {"id":"payments","label":"Payment provider","kind":"external","detail":"Card authorization"}
        ],"relations":[
            {"from":"customer","to":"web","label":"Uses"},{"from":"web","to":"api","label":"HTTPS"},{"from":"api","to":"db","label":"Reads and writes"},
            {"from":"api","to":"payments","label":"Authorizes"},{"from":"api","to":"worker","label":"Enqueues"}
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

fn placeholder_screenshot() -> String {
    use base64::Engine;
    let mut pixels = image::RgbImage::from_pixel(640, 400, image::Rgb([246, 247, 249]));
    for (x0, y0, x1, y1, shade) in [(0, 0, 640, 44, 228u8), (24, 70, 360, 92, 214), (24, 124, 150, 146, 222), (24, 180, 600, 204, 232), (24, 216, 600, 240, 238), (380, 260, 560, 290, 214)] {
        for y in y0..y1 { for x in x0..x1 { pixels.put_pixel(x, y, image::Rgb([shade, shade, shade.saturating_add(4)])); } }
    }
    let mut output = std::io::Cursor::new(Vec::new());
    pixels.write_to(&mut output, image::ImageFormat::Png).expect("in-memory PNG encoding");
    base64::engine::general_purpose::STANDARD.encode(output.into_inner())
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