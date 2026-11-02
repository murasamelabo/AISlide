use crate::{composition::{CompositionBlock, CompositionSpec, ImageFit, PatternChoice}, model::{Deck, Element, valid_text}, parts::{PartData, PartItem, PartMessage, PartSpec, ShiftRow}, rich_text::{RichParagraph, RichRun, RunStyle, Spacing}, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipeRelationship { Layers, EvidenceReading, ActivityOutcome, EntityInstance, Screenshot, Comparison, Peers, Sequence, Milestones, Architecture }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReaderGoal { UnderstandRelationship, TraceFlow, LocateControls, CompareOptions, ScanPeers, PlanActions, ReadEvidence }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDimensions { pub width: u32, pub height: u32 }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeIntent {
    pub relationship: RecipeRelationship,
    pub item_count: usize,
    #[serde(default)] pub has_source_image: bool,
    #[serde(default)] pub has_measurement_conditions: bool,
    #[serde(default)] pub actual_relationship: Option<RecipeRelationship>,
    #[serde(default)] pub reader_goal: Option<ReaderGoal>,
    #[serde(default)] pub source_dimensions: Option<SourceDimensions>,
    #[serde(default)] pub needs_exact_positions: bool,
}

impl Default for RecipeIntent {
    fn default() -> Self { Self { relationship: RecipeRelationship::Layers, item_count: 2, has_source_image: false, has_measurement_conditions: false, actual_relationship: None, reader_goal: None, source_dimensions: None, needs_exact_positions: false } }
}

#[derive(Clone, Debug, Serialize)]
pub struct Recommendation {
    pub recipe_id: String,
    pub variant: String,
    pub reason: String,
    pub missing_inputs: Vec<String>,
    pub fallback: String,
    pub avoid_when: String,
    pub decision_checks: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RejectedRecipe { pub recipe_id: String, pub code: String, pub reason: String, pub alternative: String }

#[derive(Clone, Debug, Serialize)]
pub struct RecommendationReport {
    pub status: String,
    pub candidates: Vec<Recommendation>,
    pub rejected: Vec<RejectedRecipe>,
    pub next_steps: Vec<String>,
    pub semantic_classification_verified: bool,
    pub visual_quality_verified: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeInput {
    pub version: u32,
    pub title: String,
    pub source_caption: String,
    pub takeaway: String,
    #[serde(default)] pub notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub component_profile: Option<crate::design_tokens::ComponentProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub takeaway_emphasis: Option<crate::design_tokens::EmphasisSpec>,
    pub data: RecipeData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecipeData {
    LayerRoleMap { layer_heading: String, role_heading: String, layers: Vec<LayerRole> },
    SourceReading { image: RecipeImage, readings: Vec<Reading> },
    ActivityOutcome { from_label: String, to_label: String, changes: Vec<OutcomeChange>, measurement_conditions: String },
    EntityInstanceMap { definition: Reading, instances: Vec<Reading>, relation_label: String },
    ScreenshotReading { image: RecipeImage, callouts: Vec<crate::parts::Callout> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerRole { pub id: String, pub label: String, pub role: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeImage { pub base64: String, pub mime_type: String, pub alt: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reading { pub label: String, pub detail: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeChange { pub activity: String, pub outcome: String, pub definition: String }

#[derive(Clone, Debug, Serialize)]
pub struct PreparedRecipe {
    pub recipe_id: String,
    pub variant: String,
    composition: CompositionSpec,
    notes: String,
    design_sha256: String,
    pub layout_checked: bool,
    pub semantic_truth_verified: bool,
    pub office_visual_parity_verified: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct RecipePreview {
    pub recipe_id: String,
    pub variant: String,
    pub base_revision: u64,
    pub base_hash: String,
    pub candidate_hash: String,
    pub layout_checked: bool,
    pub semantic_truth_verified: bool,
    pub office_visual_parity_verified: bool,
    pub preview: crate::export_static::PresentationPreview,
}

impl PreparedRecipe {
    pub fn composition(&self) -> &CompositionSpec { &self.composition }
    pub fn notes(&self) -> &str { &self.notes }

    pub fn operations(&self, deck: &Deck, slide_id: &str, id: &str) -> Result<Vec<crate::authoring_batch::Operation>> {
        if design_hash(deck)? != self.design_sha256 { return Err(Error::Conflict("recipe design changed; prepare the recipe again".into())); }
        valid_text(slide_id, 80)?;
        if id.is_empty() || id.len() > 24 || !id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) {
            return Err(Error::Invalid("recipe operation ID requires 1-24 ASCII letters, digits, underscores or hyphens".into()));
        }
        let slide = deck.slides.iter().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("recipe slide not found".into()))?;
        if !slide.elements.is_empty() || !slide.notes.is_empty() || !slide.notes_paragraphs.is_empty() || slide.native_source_id.is_some() { return Err(Error::Conflict("recipes require a new empty slide; existing content is never replaced".into())); }
        let mut operations = vec![crate::authoring_batch::Operation::ComposeSlide { slide_id: slide_id.into(), id: id.into(), spec: self.composition.clone() }];
        for (slot, block) in &self.composition.slots {
            if matches!(block, CompositionBlock::Table { .. }) {
                operations.push(crate::authoring_batch::Operation::SetTableHeaders { slide_id: slide_id.into(), element_id: format!("{id}-{slot}"), policy: crate::review::TableHeaders::FirstRow });
            }
        }
        operations.push(crate::authoring_batch::Operation::UpdateNotes { slide_id: slide_id.into(), notes: self.notes.clone() });
        Ok(operations)
    }
}

fn design_hash(deck: &Deck) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(crate::canonical::bytes(&(deck.width, deck.height, &deck.design, &deck.embedded_fonts))?)))
}

pub fn recommend(intent: &RecipeIntent) -> Result<Vec<Recommendation>> {
    Ok(recommend_report(intent)?.candidates)
}

fn upper_source_is_larger(intent: &RecipeIntent) -> Result<bool> {
    let Some(size) = &intent.source_dimensions else { return Ok(false) };
    let document = crate::editing::create("recipe-selection".into(), "Recipe layout assessment".into())?;
    let tokens = crate::design_tokens::resolve_profile(&document.deck, Some(crate::design_tokens::ComponentProfile::TechnicalBriefing))?;
    let contained_width = |pattern: &str, count| -> Result<f64> {
        let options = crate::layout_patterns::ResolveOptions { message_band: true, count, ..Default::default() };
        let placed = crate::layout_patterns::placement(pattern, tokens.canvas, Some(tokens.frames.body), Some(tokens.gaps), &options)?;
        let primary = placed.iter().find(|slot| slot.id == "primary").ok_or_else(|| Error::Invalid("source pattern requires a primary image slot".into()))?;
        Ok(primary.frame[2].min(primary.frame[3] * size.width as f64 / size.height as f64))
    };
    Ok(contained_width("media/figure-readings", Some(intent.item_count))? >= contained_width("split/2-1", None)? * 1.1)
}

pub fn recommend_report(intent: &RecipeIntent) -> Result<RecommendationReport> {
    let mut rejected = Vec::new();
    if let Some(size) = &intent.source_dimensions {
        if size.width == 0 || size.height == 0 || size.width > 16000 || size.height > 16000 { return Err(Error::Invalid("source dimensions must be 1-16000 pixels".into())); }
    }
    let next_steps = vec!["Record the reader question, actual relationship, primary visual, selected candidate and why the nearest alternative was rejected.".into(), "Resolve the document-wide component profile before composing; preview the actual supplied text and source image before applying.".into(), "Review source-label readability, relationship meaning, necessary conditions and repeated-role consistency; structural fit is not visual-quality verification.".into()];
    if intent.actual_relationship.is_some_and(|actual| actual != intent.relationship) {
        rejected.push(RejectedRecipe { recipe_id: if intent.relationship == RecipeRelationship::Layers { "layer-role-map" } else { "relationship-mismatch" }.into(), code: if intent.relationship == RecipeRelationship::Layers { "not_ordered_layers" } else { "relationship_mismatch" }.into(), reason: "The supplied actual relationship differs from the requested recipe family; do not force concepts into a convenient container.".into(), alternative: "Reclassify the relationship and request suitable candidates; definition-to-instance belongs in an entity-instance graph, not a layer stack.".into() });
        return Ok(RecommendationReport { status: "no_suitable_recipe".into(), candidates: Vec::new(), rejected, next_steps, semantic_classification_verified: false, visual_quality_verified: false });
    }
    let screenshot = intent.relationship == RecipeRelationship::Screenshot || intent.needs_exact_positions || intent.reader_goal == Some(ReaderGoal::LocateControls);
    let (id, maximum) = if screenshot {
        rejected.push(RejectedRecipe { recipe_id: "source-reading".into(), code: "position_anchors_required".into(), reason: "A generic full-image reading guide does not connect explanations to exact controls.".into(), alternative: "Use screenshot-reading with source-relative callout coordinates.".into() });
        ("screenshot-reading", 6)
    } else { match intent.relationship {
        RecipeRelationship::Layers => ("layer-role-map", 5),
        RecipeRelationship::EvidenceReading => ("source-reading", 3),
        RecipeRelationship::ActivityOutcome => ("activity-outcome", 3),
        RecipeRelationship::EntityInstance => ("entity-instance-map", 4),
        RecipeRelationship::Screenshot => unreachable!(),
        relationship => {
            if !(2..=12).contains(&intent.item_count) { return Err(Error::Limit("visual intent requires 2-12 items".into())); }
            let alternative = match relationship {
                RecipeRelationship::Comparison => "Use contrast/panels for two explanations; use a native table only when options share explicit criteria.",
                RecipeRelationship::Peers => "Use icon-columns for short equal peers; use icon-cards when each peer carries multiple attributes.",
                RecipeRelationship::Sequence => "Use open-steps for short ordered actions; use vertical-flow/rail for longer explanations.",
                RecipeRelationship::Milestones => "Use roadmap only for supplied phases, actions and outcomes; do not imply proportional elapsed time.",
                RecipeRelationship::Architecture => "Use a managed graph with supplied subjects, boundaries and labeled relationships; do not force arbitrary architecture into a fixed entity-instance recipe.",
                _ => unreachable!(),
            };
            rejected.push(RejectedRecipe { recipe_id: "initial-recipe-families".into(), code: "no_matching_recipe".into(), reason: "No whole-page recipe in the current catalog matches this relationship.".into(), alternative: alternative.into() });
            return Ok(RecommendationReport { status: "no_suitable_recipe".into(), candidates: Vec::new(), rejected, next_steps, semantic_classification_verified: false, visual_quality_verified: false });
        }
    }};
    let minimum = if screenshot || intent.relationship == RecipeRelationship::EntityInstance { 1 } else { 2 };
    if !(minimum..=maximum).contains(&intent.item_count) { return Err(Error::Limit(format!("recipe intent requires {minimum}-{maximum} items; split the content"))); }
    let mut missing = Vec::new();
    if (screenshot || intent.relationship == RecipeRelationship::EvidenceReading) && !intent.has_source_image { missing.push("image".to_owned()); }
    if screenshot { missing.push("callout_coordinates".to_owned()); }
    if intent.relationship == RecipeRelationship::ActivityOutcome && !intent.has_measurement_conditions { missing.push("measurement_conditions".to_owned()); }
    let catalog = catalog();
    let entry = catalog["recipes"].as_array().and_then(|entries| entries.iter().find(|entry| entry["id"] == id))
        .ok_or_else(|| Error::Invalid("recipe is missing from the catalog".into()))?;
    let variants = entry["variants"].as_array().ok_or_else(|| Error::Invalid("recipe variants are missing".into()))?;
    let mut selected: Vec<_> = variants.iter().collect();
    if id == "source-reading" {
        if intent.reader_goal == Some(ReaderGoal::TraceFlow) && upper_source_is_larger(intent)? {
            selected.sort_by_key(|variant| if variant["id"] == "visual-top" { 0 } else { 1 });
        } else {
            selected.retain(|variant| variant["id"] != "visual-top");
            if intent.source_dimensions.is_some() && intent.reader_goal == Some(ReaderGoal::TraceFlow) {
                rejected.push(RejectedRecipe { recipe_id: "source-reading".into(), code: "upper_source_not_larger".into(), reason: "The resolved upper image is not at least 10 percent wider than the side image on the standard 1280x720 technical profile; source aspect alone is not enough.".into(), alternative: "Use the larger side image or split the complete source and readable detail pages; preview on the actual deck.".into() });
            }
        }
    }
    let candidates = selected.into_iter().take(3).map(|variant| Ok(Recommendation {
        recipe_id: id.into(), variant: field(variant, "id")?, reason: if variant["id"] == "visual-top" { "The supplied wide source figure is at least 10 percent wider above compact readings on the standard 1280x720 technical profile; verify actual contained size and labels in the deck preview.".into() } else { field(variant, "reason")? },
        missing_inputs: missing.clone(), fallback: field(variant, "fallback")?,
        avoid_when: field(entry, "avoid_when")?,
        decision_checks: vec!["Does the selected relationship match the supplied facts rather than the desired shape?".into(), "Can the reader answer the page question from the visual, with conditions still visible?".into(), "Is the nearest alternative worse for this text length, source image aspect and reading order?".into()],
    })).collect::<Result<Vec<_>>>()?;
    Ok(RecommendationReport { status: if missing.is_empty() { "candidates_ready" } else { "missing_inputs" }.into(), candidates, rejected, next_steps, semantic_classification_verified: false, visual_quality_verified: false })
}

fn field(value: &Value, key: &str) -> Result<String> {
    value[key].as_str().map(str::to_owned).ok_or_else(|| Error::Invalid(format!("recipe catalog field {key} is missing")))
}

fn required(value: &str, maximum: usize, name: &str) -> Result<()> {
    valid_text(value, maximum)?;
    if value.trim().is_empty() { return Err(Error::Invalid(format!("recipe {name} requires text"))); }
    if value.contains('\r') { return Err(Error::Invalid(format!("recipe {name} requires LF line breaks"))); }
    Ok(())
}

fn item_count(length: usize, maximum: usize) -> Result<()> {
    if !(2..=maximum).contains(&length) { return Err(Error::Limit(format!("recipe requires 2-{maximum} items; split the content"))); }
    Ok(())
}

fn part(preset: &str, data: PartData) -> CompositionBlock {
    CompositionBlock::Part { spec: PartSpec { version: 1, preset: preset.into(), title: String::new(), subtitle: String::new(), data, layout: None }, show_title: None }
}

fn reading_paragraphs(readings: &[Reading]) -> Vec<RichParagraph> {
    readings.iter().flat_map(|reading| {
        let heading = reading.label.split('\n').map(|text| RichParagraph {
            runs: vec![RichRun { text: text.into(), style: RunStyle { bold: Some(true), color: Some("@accent1".into()), ..Default::default() }, field: None }],
            space_after: Some(Spacing::Points(400)), ..Default::default()
        });
        let detail = reading.detail.split('\n').map(|text| RichParagraph {
            runs: vec![RichRun { text: text.into(), ..Default::default() }],
            space_after: Some(Spacing::Points(600)), ..Default::default()
        });
        heading.chain(detail)
    }).collect()
}

fn body_fonts(element: &Element, expected: &[(&str, f64)]) -> Result<()> {
    let mut rendered = crate::model::element_list(std::slice::from_ref(element)).into_iter().filter_map(|element| match element {
        Element::Text { text, font_size, .. } | Element::Shape { text, font_size, .. } if !text.is_empty() => Some((text, *font_size)),
        _ => None,
    });
    for (text, minimum) in expected {
        let (value, font_size) = rendered.next().ok_or_else(|| Error::Invalid("recipe body text was not rendered".into()))?;
        if value.as_str() != *text { return Err(Error::Invalid("recipe body text order changed".into())); }
        if font_size < *minimum { return Err(Error::Invalid(format!("recipe body would shrink from {minimum}px to {font_size}px; choose another variant or split the content"))); }
    }
    Ok(())
}

pub fn prepare(deck: &Deck, input: &RecipeInput, variant: &str) -> Result<PreparedRecipe> {
    if input.version != 1 { return Err(Error::Unsupported("recipe version".into())); }
    required(&input.title, 120, "title")?;
    required(&input.source_caption, 200, "source_caption")?;
    required(&input.takeaway, 120, "takeaway")?;
    valid_text(&input.notes, 8000)?;
    let mut slots = BTreeMap::new();
    let (recipe_id, pattern_id, message_band) = match &input.data {
        RecipeData::LayerRoleMap { layer_heading, role_heading, layers } => {
            item_count(layers.len(), 5)?;
            required(layer_heading, 24, "layer_heading")?;
            required(role_heading, 24, "role_heading")?;
            let mut ids = BTreeSet::new();
            for layer in layers {
                if layer.id.is_empty() || layer.id.len() > 24 || !layer.id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) || !ids.insert(&layer.id) {
                    return Err(Error::Invalid("recipe layer IDs must be distinct 1-24 ASCII letters, digits, underscores or hyphens".into()));
                }
                required(&layer.label, 48, "layer label")?;
                required(&layer.role, 120, "layer role")?;
            }
            let items = |details: bool| layers.iter().map(|layer| PartItem { label: layer.label.clone(), detail: if details { layer.role.clone() } else { String::new() }, value: None }).collect();
            match variant {
                "diagram-table" => {
                    slots.insert("left".into(), part("layer/balanced", PartData::Items { items: items(false), center: String::new() }));
                    let rows = std::iter::once(vec![layer_heading.clone(), role_heading.clone()])
                        .chain(layers.iter().map(|layer| vec![layer.label.clone(), layer.role.clone()])).collect();
                    slots.insert("right".into(), CompositionBlock::Table { rows });
                    ("layer-role-map", "split/1-1", true)
                }
                "annotated-layers" => {
                    if input.component_profile.is_some() {
                        let rows = layers.iter().map(|layer| crate::parts::IconRow { label: layer.label.clone(), detail: layer.role.clone(), detail_emphasis: Vec::new(), icon: None, accent: None }).collect();
                        slots.insert("primary".into(), part("list/icon-rows", PartData::IconRows { rows, boxed: false, body_size: Some(18.0), message: None }));
                        slots.insert("message".into(), CompositionBlock::Callout { text: format!("{layer_heading} -> {role_heading}\n{}", input.takeaway) });
                        ("layer-role-map", "focus/full", true)
                    } else {
                        slots.insert("image".into(), part("layer/labeled", PartData::Items { items: items(true), center: String::new() }));
                        slots.insert("caption".into(), CompositionBlock::Label { text: format!("{layer_heading} -> {role_heading}\n{}", input.takeaway) });
                        ("layer-role-map", "focus/image-caption", false)
                    }
                }
                _ => return Err(Error::Invalid("layer-role-map requires diagram-table or annotated-layers".into())),
            }
        }
        RecipeData::SourceReading { image, readings } => {
            item_count(readings.len(), 3)?;
            required(&image.alt, 500, "image alt")?;
            crate::media::inspect_raster(&image.base64, &image.mime_type)?;
            for reading in readings { required(&reading.label, 48, "reading label")?; required(&reading.detail, 120, "reading detail")?; }
            let image = CompositionBlock::Image { base64: image.base64.clone(), mime_type: image.mime_type.clone(), alt: image.alt.clone(), fit: ImageFit::Contain };
            let explanation = CompositionBlock::Text { paragraphs: reading_paragraphs(readings) };
            match variant {
                "visual-left" => { slots.insert("primary".into(), image); slots.insert("support".into(), explanation); ("source-reading", "split/2-1", true) }
                "visual-right" => { slots.insert("primary".into(), image); slots.insert("lead".into(), explanation); ("source-reading", "split/1-2", true) }
                "visual-top" => {
                    slots.insert("primary".into(), image);
                    for (index, reading) in readings.iter().enumerate() {
                        slots.insert(format!("insight-{}", index + 1), CompositionBlock::Text { paragraphs: reading_paragraphs(std::slice::from_ref(reading)) });
                    }
                    ("source-reading", "media/figure-readings", true)
                }
                _ => return Err(Error::Invalid("source-reading requires visual-left, visual-right or visual-top".into())),
            }
        }
        RecipeData::ActivityOutcome { from_label, to_label, changes, measurement_conditions } => {
            if variant != "shift-rows" { return Err(Error::Invalid("activity-outcome requires shift-rows".into())); }
            item_count(changes.len(), 3)?;
            required(from_label, 24, "from_label")?; required(to_label, 24, "to_label")?;
            required(measurement_conditions, 160, "measurement_conditions")?;
            for change in changes {
                required(&change.activity, 48, "activity")?; required(&change.outcome, 48, "outcome")?; required(&change.definition, 160, "definition")?;
            }
            let rows = changes.iter().map(|change| ShiftRow { from: change.activity.clone(), to: change.outcome.clone(), caption: String::new(), detail: change.definition.clone() }).collect();
            slots.insert("primary".into(), part("before-after/shift", PartData::ShiftRows {
                from_label: from_label.clone(), to_label: to_label.clone(), rows, accent: None, body_size: Some(18.0),
                message: input.component_profile.is_none().then(|| PartMessage { text: input.takeaway.clone(), detail: measurement_conditions.clone(), fill: None, color: None }),
            }));
            if input.component_profile.is_some() {
                slots.insert("message".into(), CompositionBlock::Callout { text: format!("{}\n{measurement_conditions}", input.takeaway) });
            }
            ("activity-outcome", "focus/full", input.component_profile.is_some())
        }
        RecipeData::EntityInstanceMap { definition, instances, relation_label } => {
            if variant != "definition-instances" { return Err(Error::Invalid("entity-instance-map requires definition-instances".into())); }
            if !(1..=4).contains(&instances.len()) { return Err(Error::Limit("entity-instance-map requires 1-4 instances; split overview and detail".into())); }
            required(&definition.label, 48, "definition label")?;
            valid_text(&definition.detail, 120)?;
            required(relation_label, 24, "relation_label")?;
            let mut nodes = vec![json!({"id": "definition", "label": definition.label, "detail": definition.detail, "x": 48, "y": 180, "width": 320, "height": 152, "font_size": 24, "detail_font_size": 20, "text_align": "left", "fill": "@lt2", "stroke": "@accent1"})];
            let mut edges = Vec::new();
            let pitch = 120.0;
            let height = 108.0;
            let top = (512.0 - (instances.len() as f64 * height + instances.len().saturating_sub(1) as f64 * (pitch - height))) / 2.0;
            for (index, instance) in instances.iter().enumerate() {
                required(&instance.label, 48, "instance label")?;
                valid_text(&instance.detail, 120)?;
                let id = format!("instance-{}", index + 1);
                nodes.push(json!({"id": id, "label": instance.label, "detail": instance.detail, "x": 736, "y": top + index as f64 * pitch, "width": 360, "height": height, "font_size": 24, "detail_font_size": 20, "text_align": "left", "fill": "@lt1", "stroke": "@accent2"}));
                edges.push(json!({"id": format!("relation-{}", index + 1), "source": "definition", "target": id, "source_port": "right", "target_port": "left", "label": relation_label, "label_font_size": 18, "color": "@accent1", "route": "elbow"}));
            }
            let graph = serde_json::from_value(json!({"version": 1, "title": "", "show_title": false, "nodes": nodes, "edges": edges}))?;
            slots.insert("primary".into(), part("diagram/custom", PartData::Diagram { graph }));
            ("entity-instance-map", "focus/full", true)
        }
        RecipeData::ScreenshotReading { image, callouts } => {
            if variant != "position-callouts" { return Err(Error::Invalid("screenshot-reading requires position-callouts".into())); }
            if !(1..=6).contains(&callouts.len()) { return Err(Error::Limit("screenshot-reading requires 1-6 supplied callouts".into())); }
            required(&image.alt, 500, "image alt")?;
            crate::media::inspect_raster(&image.base64, &image.mime_type)?;
            for callout in callouts {
                required(&callout.label, 48, "callout label")?;
                valid_text(&callout.detail, 200)?;
                if ![callout.x, callout.y].iter().all(|value| value.is_finite() && (0.0..=1.0).contains(value)) { return Err(Error::Invalid("callout coordinates must be supplied 0-1 source-image fractions".into())); }
            }
            slots.insert("primary".into(), part("list-enumeration/screenshot-callouts", PartData::ScreenshotCallouts {
                image: crate::graphs::GraphIcon { base64: image.base64.clone(), mime_type: image.mime_type.clone(), alt: image.alt.clone() },
                callouts: callouts.clone(), detail_views: input.component_profile.is_some(), accent: None, body_size: Some(18.0), message: None,
            }));
            ("screenshot-reading", "focus/full", true)
        }
    };
    if message_band || pattern_id == "focus/full-band" { slots.entry("message".into()).or_insert_with(|| CompositionBlock::Callout { text: input.takeaway.clone() }); }
    if let Some(emphasis) = input.takeaway_emphasis {
        let slot = if slots.contains_key("message") { "message" } else { "caption" };
        let value = match slots.get(slot) {
            Some(CompositionBlock::Callout { text } | CompositionBlock::Label { text }) => text.clone(),
            _ => return Err(Error::Invalid("recipe emphasis requires a visible takeaway slot".into())),
        };
        slots.insert(slot.into(), CompositionBlock::Emphasis { paragraphs: crate::composition_pattern::lines(&value), emphasis });
    }
    let choice = PatternChoice { id: pattern_id.into(), count: None, mirror: false, message_band, lead: false };
    let composition = CompositionSpec { title: input.title.clone(), subtitle: String::new(), footer: input.source_caption.clone(), style: Default::default(), component_profile: input.component_profile, blocks: Vec::new(), pattern: Some(choice.clone()), slots };
    let tokens = crate::design_tokens::resolve_profile(deck, input.component_profile)?;
    let theme = deck.design.as_ref().map_or_else(crate::design::Theme::default, |design| {
        let layout = tokens.layout_id.as_ref().and_then(|id| design.layouts.iter().find(|layout| &layout.id == id)).or_else(|| design.layouts.first());
        layout.map_or_else(|| design.theme.clone(), |layout| crate::design::master_theme(design, &layout.master_id).clone())
    });
    let built = crate::composition_pattern::build(&tokens, &theme, "recipe", &crate::composition_pattern::Chrome { title: &input.title, footer: &input.source_caption, page: "" }, &choice, &composition.slots)?;
    for element in &built.elements {
        if let Element::Table { font_size, .. } = element {
            let minimum = if variant == "diagram-table" { 21.0 } else { tokens.type_scale.body };
            if *font_size < minimum { return Err(Error::Invalid("recipe table would shrink; choose annotated-layers or split the content".into())); }
        }
    }
    for (id, spec) in &built.parts {
        let element = crate::parts::create_with_theme(id, spec, &theme)?;
        let expected: Vec<(&str, f64)> = match &input.data {
            RecipeData::LayerRoleMap { .. } if input.component_profile.is_some() && variant == "annotated-layers" => Vec::new(),
            RecipeData::LayerRoleMap { layers, .. } => std::iter::once((spec.title.as_str(), 28.0)).filter(|(title, _minimum)| !title.is_empty()).chain(layers.iter().flat_map(|layer| {
                std::iter::once((layer.label.as_str(), 21.0)).chain((variant == "annotated-layers").then_some((layer.role.as_str(), 16.0)))
            })).collect(),
            RecipeData::ActivityOutcome { from_label, to_label, changes, .. } => [(from_label.as_str(), 13.0), (to_label.as_str(), 13.0)].into_iter().chain(changes.iter().flat_map(|change| [(change.activity.as_str(), 16.0), (change.outcome.as_str(), 17.0), (change.definition.as_str(), 18.0)])).collect(),
            RecipeData::SourceReading { .. } | RecipeData::EntityInstanceMap { .. } | RecipeData::ScreenshotReading { .. } => Vec::new(),
        };
        body_fonts(&element, &expected)?;
    }
    Ok(PreparedRecipe { recipe_id: recipe_id.into(), variant: variant.into(), composition, notes: input.notes.clone(), design_sha256: design_hash(deck)?, layout_checked: true, semantic_truth_verified: false, office_visual_parity_verified: false })
}

fn candidate(document: &crate::document::Document, slide_id: &str, id: &str, input: &RecipeInput, variant: &str) -> Result<(PreparedRecipe, crate::document::TransactionResult)> {
    crate::document::verify(document)?;
    let prepared = prepare(&document.deck, input, variant)?;
    let operations = prepared.operations(&document.deck, slide_id, id)?;
    let result = crate::authoring_batch::apply_operations(document, document.revision, &document.hash, &operations)?;
    Ok((prepared, result))
}

pub fn preview(document: &crate::document::Document, slide_id: &str, id: &str, input: &RecipeInput, variant: &str, max_dimension: Option<u32>) -> Result<RecipePreview> {
    let max_dimension = max_dimension.unwrap_or(1280);
    if !(160..=1600).contains(&max_dimension) { return Err(Error::Limit("recipe preview requires 160-1600 pixels".into())); }
    let (prepared, result) = candidate(document, slide_id, id, input, variant)?;
    let index = result.document.deck.slides.iter().position(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("recipe slide not found".into()))?;
    let preview = crate::export_static::preview_presentation(&result.document, &crate::export_static::PreviewOptions {
        page_indices: Some(vec![index]), max_dimension, overflow: crate::export_static::PreviewOverflow::Error, ..Default::default()
    })?;
    Ok(RecipePreview { recipe_id: prepared.recipe_id, variant: prepared.variant, base_revision: document.revision, base_hash: document.hash.clone(), candidate_hash: result.document.hash.clone(), layout_checked: true, semantic_truth_verified: false, office_visual_parity_verified: false, preview })
}

pub fn apply(document: &crate::document::Document, expected_revision: u64, expected_hash: &str, slide_id: &str, id: &str, input: &RecipeInput, variant: &str, candidate_hash: &str) -> Result<crate::document::TransactionResult> {
    if document.revision != expected_revision || document.hash != expected_hash { return Err(Error::Conflict("recipe base revision or hash changed; preview the recipe again".into())); }
    let (_, result) = candidate(document, slide_id, id, input, variant)?;
    if result.document.hash != candidate_hash { return Err(Error::Conflict("recipe candidate changed; preview the recipe again".into())); }
    Ok(result)
}

pub fn catalog() -> Value {
    json!({"version": 1, "visual_quality_verified": false, "recipes": [
        {
            "id": "layer-role-map",
            "relationship": "layers",
            "use_when": "Ordered layers need an explicit correspondence with their responsibilities. Both views use the same supplied labels and roles.",
            "avoid_when": "Unordered concepts, causal flows or measured quantities; layers are categorical, not proportional.",
            "required": ["title", "source_caption", "takeaway", "layer_heading", "role_heading", "layers"],
            "limits": {"items_min": 2, "items_max": 5, "label_chars": 48, "role_chars": 120},
            "variants": [
                {"id": "diagram-table", "pattern": "split/1-1", "part": "layer/balanced", "reason": "A layer diagram and a responsibility table share one ordered input.", "fallback": "annotated-layers"},
                {"id": "annotated-layers", "pattern": "focus/image-caption", "part": "layer/labeled", "reason": "Each layer carries its supplied responsibility beside the stack; the caption keeps both headings and the takeaway.", "fallback": "split-content"}
            ]
        },
        {
            "id": "source-reading",
            "relationship": "evidence_reading",
            "use_when": "An approved source figure is the primary visual, with a short reading guide and a takeaway. Keep the whole image and its attribution.",
            "avoid_when": "Missing source images, decorative photos, or callouts that must identify exact positions in a screenshot.",
            "required": ["title", "source_caption", "takeaway", "image", "readings"],
            "limits": {"items_min": 2, "items_max": 3, "label_chars": 48, "detail_chars": 120},
            "variants": [
                {"id": "visual-left", "pattern": "split/2-1", "reason": "Read the full source figure before the explanation.", "fallback": "split-content"},
                {"id": "visual-right", "pattern": "split/1-2", "reason": "Read the interpretation before the full source figure.", "fallback": "split-content"},
                {"id": "visual-top", "pattern": "media/figure-readings", "reason": "Keep a wide source figure large above compact, unboxed reading points; verify actual contained dimensions.", "fallback": "split-content"}
            ]
        },
        {
            "id": "activity-outcome",
            "relationship": "activity_outcome",
            "use_when": "Paired activity and outcome measures need their definitions and measurement conditions kept visible. No values or effects are inferred.",
            "avoid_when": "Unpaired alternatives, invented performance claims or measurements without stated conditions.",
            "required": ["title", "source_caption", "takeaway", "from_label", "to_label", "changes", "measurement_conditions"],
            "limits": {"items_min": 2, "items_max": 3, "label_chars": 48, "definition_chars": 160},
            "variants": [
                {"id": "shift-rows", "pattern": "focus/full", "part": "before-after/shift", "reason": "Pair each activity with an outcome and retain the supplied definition and conditions.", "fallback": "split-content"}
            ]
        },
        {
            "id": "entity-instance-map", "relationship": "entity_instance",
            "use_when": "One supplied definition generates or represents one to four instances; keep relationship labels explicit.",
            "avoid_when": "Actual ordered layers, arbitrary architectures, causal sequences or unlabelled associations.",
            "required": ["title", "source_caption", "takeaway", "definition", "instances", "relation_label"],
            "limits": {"items_min": 1, "items_max": 4, "label_chars": 48, "detail_chars": 120},
            "variants": [{"id": "definition-instances", "pattern": "focus/full", "part": "diagram/custom", "reason": "Show the supplied definition-to-instance relationship rather than duplicating labels in a layer stack and table.", "fallback": "split-overview-detail"}]
        },
        {
            "id": "screenshot-reading", "relationship": "screenshot",
            "use_when": "Explain exact controls or states in an approved screenshot using source-relative positions.",
            "avoid_when": "Decorative images, inferred coordinates, whole-diagram overviews or unreadable source pixels.",
            "required": ["title", "source_caption", "takeaway", "image", "callouts"],
            "limits": {"items_min": 1, "items_max": 6, "label_chars": 48, "detail_chars": 200},
            "variants": [{"id": "position-callouts", "pattern": "focus/full", "part": "list-enumeration/screenshot-callouts", "reason": "Connect each explanation to a supplied normalized location on the original screenshot.", "fallback": "split-image-detail"}]
        }
    ]})
}