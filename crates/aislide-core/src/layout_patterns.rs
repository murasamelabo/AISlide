use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const VERSION: u32 = 1;
const BODY_TOP: f64 = 120.0;
const BODY_BOTTOM_GAP: f64 = 60.0;
const REFERENCE_BOTTOM_GAP: f64 = 112.0;
const MESSAGE_BAND: f64 = 72.0;
const BODY_SIZE: f64 = 18.0;
const WIDE_ASPECT: f64 = 1.5;

const GUIDANCE: [&str; 8] = [
    "Choose by the information relationship and primary content, not by item count alone.",
    "Resolve the chosen pattern for the actual canvas and use the returned slot frames unchanged as PartSpec.layout or element frames.",
    "When fits is false, use the fallback pattern or split the content; do not shrink text below the body size.",
    "Patterns allocate body regions only; managed parts keep their internal layout. A part listed for a single-slot pattern fills that slot.",
    "Parts draw on a 1152x424 canvas (1152x512 with title). Pass the part preset to rank patterns or resolve per-slot part_fit (distortion, fit, area_used) and apply that fit; small markers and images stay square automatically.",
    "Keep one pattern across parallel pages and change it when the relationship changes; do not vary patterns only for variety.",
    "capacity is an approximate full-width (CJK) character budget at the body size; Latin text fits roughly 1.8 times as many characters. Measure rendered text.",
    "Patterns are deterministic guidance, not a visual-quality certification or Office parity.",
];

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Canvas { pub width: u32, pub height: u32 }

impl Default for Canvas {
    fn default() -> Self { Self { width: 1280, height: 720 } }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame { pub x: f64, pub y: f64, pub width: f64, pub height: f64 }

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResolveOptions {
    pub mirror: bool,
    pub message_band: bool,
    pub reference_band: bool,
    pub count: Option<usize>,
    pub body_size: Option<f64>,
    pub part: Option<String>,
    pub part_title: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind { Visual, Thumb, Panel, Card, Text, Statement, Band, Tile, Label, Marker }

const KINDS: [Kind; 10] = [Kind::Visual, Kind::Thumb, Kind::Panel, Kind::Card, Kind::Text, Kind::Statement, Kind::Band, Kind::Tile, Kind::Label, Kind::Marker];

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Visual => "visual", Kind::Thumb => "thumbnail", Kind::Panel => "panel", Kind::Card => "card", Kind::Text => "text",
            Kind::Statement => "statement", Kind::Band => "band", Kind::Tile => "tile", Kind::Label => "label", Kind::Marker => "marker",
        }
    }
    fn accepts(self) -> &'static [&'static str] {
        match self {
            Kind::Visual => &["diagram", "graph", "chart", "table", "screenshot", "image", "part"],
            Kind::Thumb => &["image", "screenshot", "icon"],
            Kind::Panel => &["part", "card", "text", "diagram", "chart", "table", "image"],
            Kind::Card => &["card", "text", "icon", "part"],
            Kind::Text => &["text", "rich_text", "list"],
            Kind::Statement => &["statement", "quote", "title"],
            Kind::Band => &["message", "callout", "caption"],
            Kind::Tile => &["metric", "icon", "short_label"],
            Kind::Label => &["label", "heading", "caption", "number"],
            Kind::Marker => &["arrow", "number", "icon"],
        }
    }
    fn min(self) -> [f64; 2] {
        match self {
            Kind::Visual => [320.0, 200.0], Kind::Thumb => [200.0, 150.0], Kind::Panel => [260.0, 160.0], Kind::Card => [240.0, 120.0],
            Kind::Text => [260.0, 64.0], Kind::Statement => [400.0, 96.0], Kind::Band => [400.0, 56.0], Kind::Tile => [160.0, 96.0],
            Kind::Label => [120.0, 40.0], Kind::Marker => [32.0, 32.0],
        }
    }
    fn padding(self, wide: bool) -> Option<f64> {
        match self {
            Kind::Panel | Kind::Card => Some(if wide { 24.0 } else { 20.0 }),
            Kind::Band | Kind::Tile => Some(16.0),
            Kind::Text | Kind::Statement | Kind::Label => Some(0.0),
            Kind::Visual | Kind::Thumb | Kind::Marker => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Gap { None, Tight, Peer, Support, Contrast }

/// Pixel distances for the four spacing relationships between slots.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct GapScale { pub tight: f64, pub peer: f64, pub support: f64, pub contrast: f64 }

impl GapScale {
    /// Pattern defaults: wide canvases separate supporting and contrasting regions further.
    pub fn standard(wide: bool) -> Self {
        Self { tight: 16.0, peer: 24.0, support: if wide { 32.0 } else { 24.0 }, contrast: if wide { 48.0 } else { 40.0 } }
    }
}

impl Gap {
    fn px(self, scale: &GapScale) -> f64 {
        match self {
            Gap::None => 0.0, Gap::Tight => scale.tight, Gap::Peer => scale.peer,
            Gap::Support => scale.support, Gap::Contrast => scale.contrast,
        }
    }
}

#[derive(Clone, Copy)]
enum Size { Weight(f64), Fixed(f64) }

#[derive(Clone, Copy)]
enum Count { Fixed(usize), Variable }

#[derive(Clone)]
enum Node {
    Slot(&'static str, Kind),
    Space,
    Split { vertical: bool, gap: Gap, children: Vec<(Size, Node)> },
    Repeat { vertical: bool, gap: Gap, count: Count, marker: Option<f64>, child: Box<Node> },
}

fn slot(id: &'static str, kind: Kind) -> Node { Node::Slot(id, kind) }
fn space() -> Node { Node::Space }
fn w(weight: f64, node: Node) -> (Size, Node) { (Size::Weight(weight), node) }
fn fx(size: f64, node: Node) -> (Size, Node) { (Size::Fixed(size), node) }
fn row(gap: Gap, children: Vec<(Size, Node)>) -> Node { Node::Split { vertical: false, gap, children } }
fn col(gap: Gap, children: Vec<(Size, Node)>) -> Node { Node::Split { vertical: true, gap, children } }
fn across(gap: Gap, count: Count, child: Node) -> Node { Node::Repeat { vertical: false, gap, count, marker: None, child: Box::new(child) } }
fn down(gap: Gap, count: Count, child: Node) -> Node { Node::Repeat { vertical: true, gap, count, marker: None, child: Box::new(child) } }
fn flow(gap: Gap, marker: f64, child: Node) -> Node { Node::Repeat { vertical: false, gap, count: Count::Variable, marker: Some(marker), child: Box::new(child) } }

const VAR: Count = Count::Variable;
const TWO: Count = Count::Fixed(2);
const THREE: Count = Count::Fixed(3);
const FOUR: Count = Count::Fixed(4);
const FIVE: Count = Count::Fixed(5);

struct Pattern {
    id: &'static str,
    name: &'static str,
    ratio: &'static str,
    relationships: &'static [&'static str],
    use_when: &'static str,
    avoid_when: &'static str,
    parts: &'static [&'static str],
    fallback: Option<&'static str>,
    count: Option<[usize; 3]>,
    full_page: bool,
    band: bool,
    root: Node,
}

impl Pattern {
    fn parts(mut self, parts: &'static [&'static str]) -> Self { self.parts = parts; self }
    fn fallback(mut self, id: &'static str) -> Self { self.fallback = Some(id); self }
    fn count(mut self, minimum: usize, maximum: usize, default: usize) -> Self { self.count = Some([minimum, maximum, default]); self }
    fn full_page(mut self) -> Self { self.full_page = true; self.band = false; self }
    fn no_band(mut self) -> Self { self.band = false; self }
    fn family(&self) -> &'static str { self.id.split('/').next().unwrap_or(self.id) }
}

fn p(id: &'static str, name: &'static str, ratio: &'static str, relationships: &'static [&'static str], use_when: &'static str, avoid_when: &'static str, root: Node) -> Pattern {
    Pattern { id, name, ratio, relationships, use_when, avoid_when, parts: &[], fallback: None, count: None, full_page: false, band: true, root }
}

fn patterns() -> Vec<Pattern> {
    use Gap::{Contrast, Peer, Support, Tight};
    use Kind::{Band, Card, Label, Marker, Panel, Statement, Text, Thumb, Tile, Visual};
    vec![
        p("focus/full", "Full body focus", "1", &["overview", "architecture", "single-visual"],
            "One dominant diagram, chart, table or image that needs the whole body, such as a system overview.",
            "Explanations of three or more lines; pair the visual with split/2-1 or add message_band.",
            slot("primary", Visual)).parts(&["add_graph", "chart parts", "matrix/*", "contrast/panels"]),
        p("focus/full-band", "Focus with takeaway band", "1 + band", &["overview", "message", "single-visual"],
            "One dominant visual plus a one-sentence takeaway or caveat.",
            "Several findings or a takeaway longer than one sentence; use compound/main-stack.",
            col(Peer, vec![w(1.0, slot("primary", Visual)), fx(MESSAGE_BAND, slot("message", Band))])).no_band(),
        p("focus/statement", "Centered statement", "70% width", &["statement", "message", "quote"],
            "A key message, question or transition that should be read alone.",
            "Bullets, evidence or several points.",
            col(Gap::None, vec![w(1.0, space()), w(1.4, row(Gap::None, vec![w(0.15, space()), w(0.7, slot("statement", Statement)), w(0.15, space())])), w(1.0, space())])).no_band(),
        p("focus/kpi-hero", "Headline metric", "2:1", &["metric"],
            "One material number with its definition, period and evidence.",
            "Several comparable metrics; use compound/kpi-chart.",
            row(Support, vec![w(2.0, col(Tight, vec![w(3.0, slot("metric", Tile)), w(1.0, slot("context", Text))])), w(1.0, slot("evidence", Text))])),
        p("focus/image-caption", "Image with caption", "1 + caption", &["image", "caption", "screenshot"],
            "A product screen or photo that needs only a caption.",
            "Numbered UI explanations; use media/screenshot-callouts.",
            col(Tight, vec![w(1.0, slot("image", Visual)), fx(48.0, slot("caption", Label))])).no_band(),
        p("focus/centered-diagram", "Centered square visual", "80% width", &["single-visual", "cycle", "overview"],
            "Square or portrait diagrams that would look stretched across the full width.",
            "Wide diagrams and timelines.",
            row(Gap::None, vec![w(0.1, space()), w(0.8, slot("primary", Visual)), w(0.1, space())])),
        p("split/1-1", "Equal halves", "1:1", &["contrast", "alternatives", "before-after"],
            "Two alternatives, states or approaches explained with the same criteria.",
            "Unequal content (one side more than twice the other) or a directional change; use split/2-1 or split/1-arrow-1.",
            row(Contrast, vec![w(1.0, slot("left", Panel)), w(1.0, slot("right", Panel))])).parts(&["list/icon-rows", "list/rows"]),
        p("split/2-1", "Primary with explanation", "2:1", &["image-explanation", "visual-evidence", "screenshot", "chart"],
            "A main image, diagram or chart with a shorter explanation read after it.",
            "Explanations that matter more than the visual, or numbered UI callouts (media/screenshot-callouts).",
            row(Support, vec![w(2.0, slot("primary", Visual)), w(1.0, slot("support", Text))])),
        p("split/1-2", "Lead-in then visual", "1:2", &["lead-in", "image-explanation"],
            "A premise, claim or reading guide that should be read before the visual.",
            "Visuals that need no introduction.",
            row(Support, vec![w(1.0, slot("lead", Text)), w(2.0, slot("primary", Visual))])),
        p("split/3-2", "Visual with substantial text", "3:2", &["image-explanation", "visual-evidence"],
            "A visual with 3-5 explanatory points that need more width than 2:1 allows.",
            "One or two short lines; use split/2-1 or focus/full-band.",
            row(Support, vec![w(3.0, slot("primary", Visual)), w(2.0, slot("support", Text))])),
        p("split/3-1", "Primary with sidebar", "3:1", &["sidebar", "legend", "navigation", "metric"],
            "A wide primary visual with a narrow legend, key figures or navigation.",
            "Paragraphs in the sidebar.",
            row(Support, vec![w(3.0, slot("primary", Visual)), w(1.0, slot("sidebar", Text))])).fallback("split/2-1"),
        p("split/1-3", "Label rail with content", "1:3", &["lead-in", "definition", "section"],
            "A short label, question or key point beside wide content.",
            "Long text in the rail.",
            row(Support, vec![w(1.0, slot("rail", Text)), w(3.0, slot("content", Panel))])).fallback("split/1-2"),
        p("split/1-arrow-1", "Before and after with arrow", "1:arrow:1", &["change", "before-after", "transition"],
            "A directional change from a current state to a target state.",
            "Symmetric alternatives without direction; use split/1-1.",
            row(Tight, vec![w(1.0, slot("before", Panel)), fx(64.0, slot("arrow", Marker)), w(1.0, slot("after", Panel))])).parts(&["before-after/shift"]),
        p("stack/1-1", "Two wide bands", "1:1 vertical", &["stacked", "timeline", "comparison"],
            "Two wide elements that share the horizontal axis, such as a timeline over a table.",
            "Content that needs height, such as tall diagrams.",
            col(Peer, vec![w(1.0, slot("top", Panel)), w(1.0, slot("bottom", Panel))])),
        p("stack/2-1", "Wide visual with insights", "2:1 vertical", &["trend", "insights", "chart"],
            "A wide chart or timeline with 2-4 short findings below.",
            "Tall or square visuals.",
            col(Peer, vec![w(2.0, slot("primary", Visual)), w(1.0, across(Peer, VAR, slot("insight", Card)))])).count(2, 4, 3),
        p("stack/1-2", "Summary above detail", "1:2 vertical", &["summary", "detail"],
            "A short summary, question or condition that frames the detailed content below.",
            "Summaries longer than two lines.",
            col(Peer, vec![w(1.0, slot("summary", Text)), w(2.0, slot("detail", Panel))])),
        p("columns/3", "Three columns", "1:1:1", &["peers", "pillars", "features"],
            "Three equal-status concepts with comparable detail.",
            "Ordered steps or unequal priorities.",
            across(Support, THREE, slot("column", Card))).parts(&["list-horizontal/icon-cards", "list-horizontal/columns"]),
        p("columns/4", "Four columns", "1:1:1:1", &["peers", "categories", "features"],
            "Four short parallel items.",
            "A paragraph per item.",
            across(Peer, FOUR, slot("column", Card))).fallback("grid/2x2").parts(&["list-horizontal/icon-cards", "list-horizontal/columns"]),
        p("columns/5-compact", "Five compact items", "5 equal", &["icons", "peers"],
            "Five icon-and-label items without explanations.",
            "Any sentence-length detail.",
            across(Tight, FIVE, slot("item", Tile))),
        p("grid/2x2", "Two-by-two grid", "2x2", &["quadrant", "categories", "peers"],
            "Four categories or quadrants of similar weight.",
            "Ordered items; use a flow.",
            down(Peer, TWO, across(Peer, TWO, slot("cell", Card)))).parts(&["list-enumeration/grid", "list-horizontal/icon-cards"]),
        p("grid/3x2", "Three-by-two grid", "3x2", &["peers", "features", "categories"],
            "Six equal-status items with a short explanation each.",
            "Items needing more than two sentences.",
            down(Peer, TWO, across(Peer, THREE, slot("cell", Card)))).parts(&["list-horizontal/icon-cards", "list-enumeration/grid"]),
        p("grid/4x2", "Four-by-two grid", "4x2", &["icons", "features"],
            "Eight icon-and-label items.",
            "Explanatory text.",
            down(Peer, TWO, across(Peer, FOUR, slot("cell", Tile)))),
        p("compound/main-stack", "Primary with stacked insights", "2:1", &["insights", "dashboard", "chart"],
            "A main chart or diagram with 2-3 findings read beside it.",
            "Findings longer than about three lines each.",
            row(Support, vec![w(2.0, slot("primary", Visual)), w(1.0, down(Peer, VAR, slot("insight", Card)))])).count(2, 3, 3),
        p("compound/stack-main", "Stacked points then primary", "1:2", &["insights", "lead-in"],
            "Two or three premises or questions read before the main visual.",
            "Conclusions drawn from the visual; use compound/main-stack.",
            row(Support, vec![w(1.0, down(Peer, VAR, slot("point", Card))), w(2.0, slot("primary", Visual))])).count(2, 3, 3),
        p("compound/hero-columns", "Overview with detail columns", "3:2 vertical", &["overview", "detail", "architecture"],
            "An overall diagram followed by 2-4 details.",
            "Details that need tall space.",
            col(Peer, vec![w(3.0, slot("overview", Visual)), w(2.0, across(Peer, VAR, slot("detail", Card)))])).count(2, 4, 3),
        p("compound/master-detail", "Index with detail", "1:2", &["index", "navigation", "detail"],
            "Walking through the items of one list across consecutive slides; keep the index in place and highlight the current entry.",
            "Single slides without a repeated index.",
            row(Support, vec![w(1.0, down(Tight, VAR, slot("entry", Label))), w(2.0, slot("detail", Panel))])).count(3, 7, 5),
        p("compound/kpi-chart", "Metrics above chart", "tiles + chart", &["metric", "dashboard", "trend"],
            "2-4 headline metrics with the chart that explains them.",
            "Metrics unrelated to the chart.",
            col(Peer, vec![fx(120.0, across(Peer, VAR, slot("metric", Tile))), w(1.0, slot("chart", Visual))])).count(2, 4, 3),
        p("compound/bento-1-2", "One large, two small", "2:1", &["features", "overview"],
            "One main capability with two supporting ones.",
            "Equal-status items; use columns/3.",
            row(Peer, vec![w(2.0, slot("feature", Panel)), w(1.0, down(Peer, TWO, slot("minor", Card)))])),
        p("compound/bento-1-4", "One large, four small", "1:1", &["features", "overview"],
            "One main capability with four supporting ones.",
            "Detailed supporting text.",
            row(Peer, vec![w(1.0, slot("feature", Panel)), w(1.0, down(Peer, TWO, across(Peer, TWO, slot("minor", Card))))])).fallback("compound/bento-1-2"),
        p("compound/center-corners", "Central visual with side notes", "1:2:1", &["annotation", "features", "image"],
            "A central product image or diagram with four short notes around it.",
            "Notes that need leader lines crossing the visual.",
            row(Peer, vec![w(1.0, down(Peer, TWO, slot("note-left", Card))), w(2.0, slot("center", Visual)), w(1.0, down(Peer, TWO, slot("note-right", Card)))])).fallback("compound/main-stack"),
        p("sequence/steps-h", "Horizontal steps", "n equal + arrows", &["sequence", "process", "steps"],
            "2-5 ordered steps read left to right; draw arrows in the connector slots.",
            "Steps with long explanations; use sequence/steps-v.",
            flow(Tight, 32.0, slot("step", Card))).count(2, 5, 4).fallback("sequence/steps-v").parts(&["flow/cards"]),
        p("sequence/steps-v", "Vertical steps", "numbered rows", &["sequence", "process", "steps"],
            "2-6 ordered steps with longer explanations.",
            "Unordered items.",
            down(Tight, VAR, row(Peer, vec![fx(64.0, slot("marker", Marker)), w(1.0, slot("step", Text))]))).count(2, 6, 4),
        p("sequence/timeline", "Timeline", "above : axis : below", &["timeline", "sequence"],
            "Dated events or milestones along one time axis; alternate items above and below the axis.",
            "Undated steps.",
            col(Tight, vec![w(1.0, across(Peer, VAR, slot("above", Card))), fx(48.0, slot("axis", Marker)), w(1.0, across(Peer, VAR, slot("below", Card)))])).count(2, 6, 4).fallback("sequence/steps-v"),
        p("sequence/swimlane", "Swimlanes", "labels + lanes", &["actors", "process", "sequence"],
            "Handoffs between 2-5 actors; align a managed graph in the canvas slot with the lane labels.",
            "A single actor; use steps.",
            row(Tight, vec![fx(160.0, down(Tight, VAR, slot("lane", Label))), w(1.0, slot("canvas", Visual))])).count(2, 5, 3).parts(&["add_graph"]),
        p("sequence/cycle", "Cycle with explanation", "3:2", &["cycle", "process"],
            "A repeating loop with an explanation of its stages.",
            "Linear processes.",
            row(Support, vec![w(3.0, slot("cycle", Visual)), w(2.0, slot("explanation", Text))])).parts(&["cycle/*"]),
        p("compare/table-full", "Full-width table with decision", "table + band", &["comparison", "criteria", "options"],
            "Three or more alternatives compared on shared criteria, with the conclusion below.",
            "Two alternatives with narrative explanations; use split/1-1.",
            col(Peer, vec![w(1.0, slot("table", Visual)), fx(MESSAGE_BAND, slot("decision", Band))])).no_band().parts(&["matrix/*", "table"]),
        p("compare/options-3", "Three options with decision", "1:1:1 + band", &["options", "decision", "alternatives"],
            "Three options explained side by side with the recommendation stated below.",
            "Numeric scoring on many criteria; use compare/table-full.",
            col(Peer, vec![w(1.0, across(Support, THREE, slot("option", Card))), fx(MESSAGE_BAND, slot("decision", Band))])).no_band(),
        p("compare/quadrant", "Quadrant with reading", "3:1", &["quadrant", "positioning"],
            "A 2x2 positioning chart with a short interpretation.",
            "Positioning without two defined axes.",
            row(Support, vec![w(3.0, slot("quadrant", Visual)), w(1.0, slot("reading", Text))])).fallback("split/2-1").parts(&["matrix/*"]),
        p("compare/screens-1-1", "Two screens compared", "1:1 + captions", &["ui-compare", "before-after", "screenshot"],
            "Two screenshots at the same scale, such as before and after a setting change.",
            "Screens of different aspect ratios that are not cropped consistently.",
            col(Tight, vec![w(1.0, row(Contrast, vec![w(1.0, slot("before", Visual)), w(1.0, slot("after", Visual))])), fx(40.0, row(Contrast, vec![w(1.0, slot("before-caption", Label)), w(1.0, slot("after-caption", Label))]))])),
        p("text/reading", "Reading column", "65% width", &["reading", "long-text"],
            "Explanatory prose that needs a comfortable line length (about 40 full-width characters).",
            "Bullets that fit a list part.",
            row(Gap::None, vec![w(0.65, slot("body", Text)), w(0.35, space())])),
        p("text/two-column", "Two text columns", "1:1", &["long-text", "reading"],
            "Longer reference text split into two balanced columns.",
            "Two contrasting positions; use split/1-1.",
            across(Contrast, TWO, slot("column", Text))),
        p("text/label-detail", "Label and detail rows", "1:3 rows", &["definition", "faq"],
            "Definitions, FAQs or requirement rows with a short label and a longer detail.",
            "Equal-status topics with icons; use list/icon-rows.",
            down(Tight, VAR, row(Support, vec![w(1.0, slot("label", Label)), w(3.0, slot("detail", Text))]))).count(2, 6, 4).parts(&["list/rows"]),
        p("text/quote", "Quotation", "80% width", &["quote", "statement"],
            "A source-attributed quotation.",
            "Paraphrases presented as quotations.",
            col(Gap::None, vec![w(1.0, space()), w(3.0, row(Gap::None, vec![w(0.1, space()), w(0.8, col(Tight, vec![w(1.0, slot("quote", Statement)), fx(40.0, slot("attribution", Label))])), w(0.1, space())])), w(1.0, space())])).no_band(),
        p("text/takeaways-3", "Three takeaways", "3 numbered rows", &["takeaways", "summary"],
            "Three numbered conclusions.",
            "More than three points; use text/label-detail.",
            down(Peer, THREE, row(Peer, vec![fx(56.0, slot("number", Marker)), w(1.0, slot("takeaway", Text))]))),
        p("media/screenshot-callouts", "Screenshot with numbered callouts", "2:1 (managed by the part)", &["screenshot", "annotation", "ui"],
            "A UI screenshot explained by numbered callouts; the part anchors badges to the image and spaces the legend uniformly.",
            "Hand-placed numbers or legends.",
            slot("part", Panel)).no_band().parts(&["list-enumeration/screenshot-callouts"]),
        p("media/gallery-3", "Image gallery", "n equal + captions", &["gallery", "image"],
            "2-4 images of equal importance with captions.",
            "Images needing detailed explanation.",
            across(Peer, VAR, col(Tight, vec![w(1.0, slot("image", Thumb)), fx(48.0, slot("caption", Label))]))).count(2, 4, 3),
        p("media/image-steps", "Screens as steps", "n equal + arrows", &["steps", "screenshot", "process"],
            "2-4 ordered screens, each with a short instruction.",
            "More than four screens; split the slide.",
            flow(Tight, 32.0, col(Tight, vec![w(3.0, slot("image", Thumb)), w(2.0, slot("step", Card))]))).count(2, 4, 3).parts(&["flow/cards"]),
        p("media/image-overlay-band", "Image with bottom band", "image + band", &["hero-image", "message"],
            "A large image whose message sits in a band directly below it.",
            "Text placed over busy image areas.",
            col(Gap::None, vec![w(1.0, slot("image", Visual)), fx(96.0, slot("caption", Band))])).no_band(),
        p("structure/cover", "Cover", "72% text column", &["cover"],
            "A title page with title, subtitle and optional date or presenter.",
            "Content slides.",
            row(Gap::None, vec![w(0.72, col(Peer, vec![w(1.0, space()), w(1.3, slot("title", Statement)), w(0.8, slot("subtitle", Text)), w(0.9, space())])), w(0.28, space())])).full_page(),
        p("structure/section", "Section divider", "centered band", &["section"],
            "Chapter transitions with a section number and title.",
            "Pages with body content.",
            col(Tight, vec![w(1.5, space()), fx(48.0, slot("number", Label)), w(1.5, slot("title", Statement)), w(1.5, space())])).full_page(),
        p("structure/agenda", "Agenda", "1", &["agenda"],
            "A chapter list with durations.",
            "Detailed content.",
            slot("part", Panel)).no_band().parts(&["list/agenda"]),
        p("structure/summary", "Summary points with message", "n equal + band", &["summary", "takeaways"],
            "2-4 summary points with the overall message below.",
            "New evidence introduced at the end.",
            col(Peer, vec![w(1.0, across(Support, VAR, slot("point", Card))), fx(MESSAGE_BAND, slot("message", Band))])).count(2, 4, 3).no_band(),
        p("structure/closing", "Closing and next steps", "1:2 vertical", &["next-steps", "decision"],
            "A closing message with 2-4 next actions or owners.",
            "Unsupported commitments or dates.",
            col(Peer, vec![w(1.0, slot("message", Statement)), w(2.0, across(Peer, VAR, slot("action", Card)))])).count(2, 4, 3).no_band(),
        p("structure/appendix-refs", "Reference appendix", "1:1", &["references"],
            "Source lists and supplementary references in two columns.",
            "Primary evidence that belongs on the content slide.",
            across(Contrast, TWO, slot("column", Text))).no_band(),
    ]
}

struct Placed { id: String, kind: Kind, frame: [f64; 4] }

struct Context { gaps: GapScale, count: usize }

fn place(node: &Node, frame: [f64; 4], context: &Context, suffix: &str, out: &mut Vec<Placed>) -> Result<()> {
    match node {
        Node::Space => Ok(()),
        Node::Slot(id, kind) => { out.push(Placed { id: format!("{id}{suffix}"), kind: *kind, frame }); Ok(()) }
        Node::Split { vertical, gap, children } => {
            let items: Vec<(Size, &Node, String)> = children.iter().map(|(size, child)| (*size, child, suffix.to_owned())).collect();
            divide(frame, *vertical, gap.px(&context.gaps), &items, context, out)
        }
        Node::Repeat { vertical, gap, count, marker, child } => {
            let total = match count { Count::Fixed(value) => *value, Count::Variable => context.count };
            let connector = slot("connector", Kind::Marker);
            let mut items = Vec::new();
            for index in 1..=total {
                if let (Some(size), true) = (marker, index > 1) { items.push((Size::Fixed(*size), &connector, format!("{suffix}-{}", index - 1))); }
                items.push((Size::Weight(1.0), child.as_ref(), format!("{suffix}-{index}")));
            }
            divide(frame, *vertical, gap.px(&context.gaps), &items, context, out)
        }
    }
}

fn divide(frame: [f64; 4], vertical: bool, gap: f64, items: &[(Size, &Node, String)], context: &Context, out: &mut Vec<Placed>) -> Result<()> {
    let (start, length) = if vertical { (frame[1], frame[3]) } else { (frame[0], frame[2]) };
    let fixed = items.iter().map(|(size, ..)| match size { Size::Fixed(value) => *value, Size::Weight(_) => 0.0 }).sum::<f64>() + gap * items.len().saturating_sub(1) as f64;
    let weights: f64 = items.iter().map(|(size, ..)| match size { Size::Weight(value) => *value, Size::Fixed(_) => 0.0 }).sum();
    let free = length - fixed;
    if free < 0.0 || (weights > 0.0 && free < 1.0) {
        return Err(Error::Invalid("layout pattern does not fit the body frame; use a larger body or another pattern".into()));
    }
    let mut position = start;
    for (size, child, suffix) in items {
        let extent = match size { Size::Fixed(value) => *value, Size::Weight(value) => free * value / weights };
        // Integer gaps keep rounded boundaries exactly one gap apart.
        let (from, to) = (position.round(), (position + extent).round());
        let bounds = if vertical { [frame[0], from, frame[2], to - from] } else { [from, frame[1], to - from, frame[3]] };
        place(child, bounds, context, suffix, out)?;
        position += extent + gap;
    }
    Ok(())
}

fn find(pattern_id: &str) -> Result<Pattern> {
    patterns().into_iter().find(|pattern| pattern.id == pattern_id)
        .ok_or_else(|| Error::Invalid(format!("unknown layout pattern {pattern_id}; call layout_patterns for pattern IDs")))
}

fn frame_json(frame: [f64; 4]) -> Value { json!({"x": frame[0], "y": frame[1], "width": frame[2], "height": frame[3]}) }

fn default_body(canvas: Canvas, full_page: bool, reference_band: bool) -> [f64; 4] {
    let (width, height) = (canvas.width as f64, canvas.height as f64);
    let margin = if width / height >= WIDE_ASPECT { 48.0 } else { 40.0 };
    if full_page { return [margin, margin, width - margin * 2.0, height - margin * 2.0]; }
    let bottom = height - if reference_band { REFERENCE_BOTTOM_GAP } else { BODY_BOTTOM_GAP };
    [margin, BODY_TOP, width - margin * 2.0, bottom - BODY_TOP]
}

fn summary(pattern: &Pattern) -> Value {
    json!({
        "id": pattern.id, "family": pattern.family(), "name": pattern.name, "ratio": pattern.ratio,
        "relationships": pattern.relationships, "use_when": pattern.use_when, "avoid_when": pattern.avoid_when,
        "parts": pattern.parts, "fallback": pattern.fallback, "full_page": pattern.full_page, "message_band": pattern.band,
        "count": pattern.count.map(|[minimum, maximum, default]| json!({"min": minimum, "max": maximum, "default": default})),
    })
}

fn capacity(kind: Kind, frame: [f64; 4], wide: bool, body_size: f64) -> Value {
    let Some(padding) = kind.padding(wide) else { return Value::Null };
    let font_size = if kind == Kind::Statement { (body_size * 14.0 / 9.0).round() } else { body_size };
    let per_line = ((frame[2] - padding * 2.0) / font_size).floor().max(0.0) as u32;
    let lines = ((frame[3] - padding * 2.0) / (font_size * 1.5)).floor().max(0.0) as u32;
    json!({"font_size": font_size, "padding": padding, "cjk_chars_per_line": per_line, "lines": lines})
}

fn arrange(pattern: &Pattern, body: [f64; 4], gaps: GapScale, count: usize, message_band: bool) -> Result<Vec<Placed>> {
    let root = if message_band { col(Gap::Peer, vec![w(1.0, pattern.root.clone()), fx(MESSAGE_BAND, slot("message", Kind::Band))]) } else { pattern.root.clone() };
    let mut placed = Vec::new();
    place(&root, body, &Context { gaps, count }, "", &mut placed)?;
    Ok(placed)
}

pub fn catalog() -> Result<Value> {
    let canvas = Canvas::default();
    let entries = patterns().iter().map(|pattern| {
        let count = pattern.count.map_or(0, |[_, _, default]| default);
        let placed = arrange(pattern, default_body(canvas, pattern.full_page, false), GapScale::standard(true), count, false)?;
        let mut entry = summary(pattern);
        entry["slots"] = placed.iter().map(|slot| json!({"id": slot.id, "kind": slot.kind.name()})).collect();
        Ok(entry)
    }).collect::<Result<Vec<_>>>()?;
    let minimums: serde_json::Map<String, Value> = KINDS.iter().map(|kind| (kind.name().to_owned(), json!({"width": kind.min()[0], "height": kind.min()[1], "accepts": kind.accepts()}))).collect();
    Ok(json!({
        "version": VERSION,
        "canvas_default": canvas,
        "tokens": {
            "grid": 8, "margin": {"wide": 48, "standard": 40}, "wide_aspect_min": WIDE_ASPECT,
            "body_top": BODY_TOP, "body_bottom_gap": BODY_BOTTOM_GAP, "reference_band_bottom_gap": REFERENCE_BOTTOM_GAP,
            "gaps": {"tight": 16, "peer": 24, "support": {"wide": 32, "standard": 24}, "contrast": {"wide": 48, "standard": 40}},
            "message_band_height": MESSAGE_BAND, "body_size": BODY_SIZE, "slot_kinds": minimums,
        },
        "guidance": GUIDANCE,
        "patterns": entries,
    }))
}

struct Prepared { pattern: Pattern, wide: bool, body: [f64; 4], count: usize, gaps: GapScale, body_size: f64, placed: Vec<Placed> }

fn prepare(pattern_id: &str, canvas: Canvas, body: Option<Frame>, gaps: Option<GapScale>, options: &ResolveOptions) -> Result<Prepared> {
    let pattern = find(pattern_id)?;
    crate::canvas::validate_size(canvas.width, canvas.height)?;
    let (width, height) = (canvas.width as f64, canvas.height as f64);
    let wide = width / height >= WIDE_ASPECT;
    let body_size = options.body_size.unwrap_or(BODY_SIZE);
    if !body_size.is_finite() || !(12.0..=40.0).contains(&body_size) { return Err(Error::Invalid("layout body_size must be 12..40".into())); }
    if options.reference_band && (body.is_some() || pattern.full_page) {
        return Err(Error::Invalid("reference_band applies only to the default body of a content page".into()));
    }
    if options.message_band && !pattern.band {
        return Err(Error::Invalid(format!("{} has its own band or page structure; message_band is not available", pattern.id)));
    }
    let body = match body {
        Some(frame) => {
            let values = [frame.x, frame.y, frame.width, frame.height];
            if values.iter().any(|value| !value.is_finite()) { return Err(Error::Invalid("layout body must be finite".into())); }
            let [x, y, frame_width, frame_height] = values.map(f64::round);
            if x < 0.0 || y < 0.0 || frame_width < 1.0 || frame_height < 1.0 || x + frame_width > width || y + frame_height > height {
                return Err(Error::Invalid("layout body must be a positive frame inside the canvas".into()));
            }
            [x, y, frame_width, frame_height]
        }
        None => default_body(canvas, pattern.full_page, options.reference_band),
    };
    if body[2] < 1.0 || body[3] < 1.0 { return Err(Error::Invalid("canvas is too small for the default layout body".into())); }
    let count = match (pattern.count, options.count) {
        (None, Some(_)) => return Err(Error::Invalid(format!("{} has a fixed item count", pattern.id))),
        (None, None) => 0,
        (Some([minimum, maximum, default]), requested) => {
            let value = requested.unwrap_or(default);
            if !(minimum..=maximum).contains(&value) { return Err(Error::Invalid(format!("{} count must be {minimum}..{maximum}", pattern.id))); }
            value
        }
    };
    let gaps = gaps.unwrap_or_else(|| GapScale::standard(wide));
    let mut placed = arrange(&pattern, body, gaps, count, options.message_band)?;
    if options.mirror {
        for slot in &mut placed { slot.frame[0] = body[0] * 2.0 + body[2] - slot.frame[0] - slot.frame[2]; }
    }
    Ok(Prepared { pattern, wide, body, count, gaps, body_size, placed })
}

pub fn resolve(pattern_id: &str, canvas: Option<Canvas>, body: Option<Frame>, options: &ResolveOptions) -> Result<Value> {
    let canvas = canvas.unwrap_or_default();
    let Prepared { pattern, wide, body, count, gaps, body_size, placed } = prepare(pattern_id, canvas, body, None, options)?;
    let profile = options.part.as_deref().map(|preset| crate::parts::aspect::profile_for_preset(preset, options.part_title)).transpose()?;
    let mut issues = Vec::new();
    let slots: Vec<Value> = placed.iter().map(|slot| {
        let [min_width, min_height] = slot.kind.min();
        let fits = slot.frame[2] >= min_width && slot.frame[3] >= min_height;
        if !fits {
            issues.push(format!("{} is {}x{}px; a {} slot needs at least {min_width}x{min_height}px", slot.id, slot.frame[2], slot.frame[3], slot.kind.name()));
        }
        json!({
            "id": slot.id, "kind": slot.kind.name(), "frame": frame_json(slot.frame), "accepts": slot.kind.accepts(),
            "min": {"width": min_width, "height": min_height}, "fits": fits, "capacity": capacity(slot.kind, slot.frame, wide, body_size),
            "part_fit": profile.as_ref().filter(|_| slot.kind.accepts().contains(&"part")).map(|profile| crate::parts::aspect::fit_in(profile, slot.frame[2], slot.frame[3])),
        })
    }).collect();
    let fits = issues.is_empty();
    Ok(json!({
        "version": VERSION, "pattern": summary(&pattern), "canvas": canvas, "body": frame_json(body),
        "options": {"mirror": options.mirror, "message_band": options.message_band, "reference_band": options.reference_band, "count": pattern.count.map(|_| count), "body_size": body_size},
        "gaps": {"tight": gaps.tight, "peer": gaps.peer, "support": gaps.support, "contrast": gaps.contrast},
        "slots": slots, "fits": fits, "issues": issues,
        "fallback": if fits { None } else { pattern.fallback },
        "part": profile,
    }))
}

/// Static traits of a pattern that decide how a caller frames it before placement.
pub(crate) struct Traits { pub full_page: bool, pub count: Option<[usize; 3]>, pub fallback: Option<&'static str> }

pub(crate) fn traits(pattern_id: &str) -> Result<Traits> {
    let pattern = find(pattern_id)?;
    Ok(Traits { full_page: pattern.full_page, count: pattern.count, fallback: pattern.fallback })
}

/// A slot frame resolved for deterministic composition.
pub(crate) struct PlacedSlot { pub id: String, pub kind: &'static str, pub accepts: &'static [&'static str], pub frame: [f64; 4], pub min: [f64; 2] }

impl PlacedSlot {
    pub(crate) fn fits(&self) -> bool { self.frame[2] >= self.min[0] && self.frame[3] >= self.min[1] }
}

/// Typed placement shared with composition; frames are identical to `resolve` for the same inputs.
pub(crate) fn placement(pattern_id: &str, canvas: Canvas, body: Option<Frame>, gaps: Option<GapScale>, options: &ResolveOptions) -> Result<Vec<PlacedSlot>> {
    let prepared = prepare(pattern_id, canvas, body, gaps, options)?;
    Ok(prepared.placed.into_iter().map(|slot| PlacedSlot { id: slot.id, kind: slot.kind.name(), accepts: slot.kind.accepts(), frame: slot.frame, min: slot.kind.min() }).collect())
}

/// Patterns ordered by how well their best part slot holds the preset without distortion or wasted area.
pub fn rank(part: &str, canvas: Option<Canvas>, part_title: bool) -> Result<Value> {
    let canvas = canvas.unwrap_or_default();
    crate::canvas::validate_size(canvas.width, canvas.height)?;
    let wide = canvas.width as f64 / canvas.height as f64 >= WIDE_ASPECT;
    let profile = crate::parts::aspect::profile_for_preset(part, part_title)?;
    let mut ranked = Vec::new();
    for pattern in patterns().iter().filter(|pattern| !pattern.full_page) {
        let count = pattern.count.map_or(0, |[_, _, default]| default);
        let Ok(placed) = arrange(pattern, default_body(canvas, false, false), GapScale::standard(wide), count, false) else { continue };
        let best = placed.iter().filter(|slot| slot.kind.accepts().contains(&"part") && slot.frame[2] >= slot.kind.min()[0] && slot.frame[3] >= slot.kind.min()[1])
            .map(|slot| (slot, crate::parts::aspect::fit_in(&profile, slot.frame[2], slot.frame[3])))
            .max_by(|(left_slot, left), (right_slot, right)| {
                let key = |slot: &Placed, fit: &Value| (fit["area_used"].as_f64().unwrap_or(0.0) * slot.frame[2] * slot.frame[3], -fit["distortion"].as_f64().unwrap_or(f64::MAX));
                key(left_slot, left).partial_cmp(&key(right_slot, right)).unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some((slot, fit)) = best {
            ranked.push(json!({"pattern_id": pattern.id, "name": pattern.name, "slot": slot.id, "frame": frame_json(slot.frame), "fit": fit["fit"], "distortion": fit["distortion"], "area_used": fit["area_used"],
                "slot_share": ((fit["area_used"].as_f64().unwrap_or(0.0) * slot.frame[2] * slot.frame[3] / (default_body(canvas, false, false)[2] * default_body(canvas, false, false)[3])) * 100.0).round() / 100.0}));
        }
    }
    ranked.sort_by(|left, right| right["slot_share"].as_f64().partial_cmp(&left["slot_share"].as_f64()).unwrap_or(std::cmp::Ordering::Equal)
        .then(left["distortion"].as_f64().partial_cmp(&right["distortion"].as_f64()).unwrap_or(std::cmp::Ordering::Equal)));
    Ok(json!({"version": VERSION, "part": profile, "canvas": canvas, "patterns": ranked}))
}
