//! Design tokens: one resolved source of frames, spacing, type scale, script metrics and color
//! roles for deterministic composition. Tokens are derived from the deck's design on demand and
//! are never persisted: an unmodified design preset yields its own scale, any other design
//! yields neutral defaults scaled to the canvas.
use crate::{design_presets::Geometry, layout_patterns::{Canvas, Frame, GapScale}, model::Deck, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentProfile { TechnicalBriefing }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmphasisStyle { #[default] Auto, Inline, Rule, Outline, Tint, Closing, Solid }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmphasisStrength { Subtle, #[default] Normal, Strong }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmphasisRelationship { #[default] Supporting, Standalone, Contrast, Conditional }

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EmphasisSpec { pub style: EmphasisStyle, pub strength: EmphasisStrength, pub relationship: EmphasisRelationship }

#[derive(Clone, Debug, Serialize)]
pub struct EmphasisLayout {
    pub style: EmphasisStyle,
    pub frame: Frame,
    pub text_frame: Frame,
    pub surface_frame: Option<Frame>,
    pub separator_frame: Option<Frame>,
    pub fill: Option<&'static str>,
    pub stroke: Option<&'static str>,
    pub stroke_width: f64,
    /// Style-owned text size; `None` keeps the size of the slot that holds the emphasis.
    pub text_size: Option<f64>,
    /// Style-owned text color; `None` keeps the ink role.
    pub text_color: Option<&'static str>,
}

/// Where an emphasis is placed relative to the body it concludes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmphasisPlacement { Bottom, Lead }

pub fn emphasis_style(spec: &EmphasisSpec) -> EmphasisStyle {
    if spec.style != EmphasisStyle::Auto { return spec.style; }
    match (spec.relationship, spec.strength) {
        // A qualifier limits the claim; it must stay visible without outranking the conclusion.
        (EmphasisRelationship::Conditional, EmphasisStrength::Subtle) => EmphasisStyle::Inline,
        (EmphasisRelationship::Conditional, EmphasisStrength::Normal) => EmphasisStyle::Rule,
        (EmphasisRelationship::Conditional, EmphasisStrength::Strong) => EmphasisStyle::Outline,
        (EmphasisRelationship::Standalone | EmphasisRelationship::Contrast, _) => EmphasisStyle::Outline,
        (EmphasisRelationship::Supporting, EmphasisStrength::Subtle) => EmphasisStyle::Inline,
        (EmphasisRelationship::Supporting, EmphasisStrength::Normal) => EmphasisStyle::Rule,
        (EmphasisRelationship::Supporting, EmphasisStrength::Strong) => EmphasisStyle::Tint,
    }
}

pub fn emphasis_layout(tokens: &DesignTokens, outer: Frame, spec: &EmphasisSpec) -> Result<EmphasisLayout> {
    emphasis_layout_at(tokens, outer, spec, EmphasisPlacement::Bottom)
}

/// Layout of one emphasis frame; a lead sits under the title, so its rule closes the frame instead of opening it.
pub fn emphasis_layout_at(tokens: &DesignTokens, outer: Frame, spec: &EmphasisSpec, placement: EmphasisPlacement) -> Result<EmphasisLayout> {
    let style = emphasis_style(spec);
    let vertical = tokens.canvas.height as f64 / 720.0;
    let mut result = EmphasisLayout { style, frame: outer, text_frame: outer, surface_frame: None, separator_frame: None, fill: None, stroke: None, stroke_width: 0.0, text_size: None, text_color: None };
    match style {
        EmphasisStyle::Inline => {},
        EmphasisStyle::Rule => {
            let gap = 12.0 * vertical;
            let (separator_y, text_y) = match placement {
                EmphasisPlacement::Bottom => (outer.y, outer.y + gap),
                EmphasisPlacement::Lead => (outer.y + outer.height - vertical, outer.y),
            };
            result.separator_frame = Some(frame(outer.x, separator_y, outer.width, vertical));
            result.stroke = Some(tokens.colors.muted);
            result.text_frame = frame(outer.x, text_y, outer.width, outer.height - gap);
        }
        EmphasisStyle::Closing => {
            // One closing sentence: a firm accent rule and a larger line, never a surface.
            let rule = 2.0 * vertical;
            let gap = 10.0 * vertical;
            result.separator_frame = Some(frame(outer.x, outer.y, outer.width, rule));
            result.stroke = Some(tokens.colors.accent_alt);
            result.text_frame = frame(outer.x, outer.y + rule + gap, outer.width, outer.height - rule - gap);
            result.text_size = Some(28.0 * vertical);
        }
        EmphasisStyle::Solid => {
            // Reversed text on the ink role keeps the same contrast as body text on the background.
            let horizontal_padding = 24.0 * vertical;
            let vertical_padding = 8.0 * vertical;
            result.surface_frame = Some(outer);
            result.fill = Some(tokens.colors.ink);
            result.stroke = Some(tokens.colors.ink);
            result.text_frame = frame(outer.x + horizontal_padding, outer.y + vertical_padding, outer.width - horizontal_padding * 2.0, outer.height - vertical_padding * 2.0);
            result.text_size = Some(24.0 * vertical);
            result.text_color = Some(tokens.colors.on_accent);
        }
        EmphasisStyle::Outline | EmphasisStyle::Tint => {
            let horizontal_padding = 16.0 * vertical;
            let vertical_padding = 8.0 * vertical;
            result.surface_frame = Some(outer);
            result.fill = (style == EmphasisStyle::Tint).then_some(tokens.colors.surface);
            result.stroke = Some(tokens.colors.accent_alt);
            result.stroke_width = if spec.strength == EmphasisStrength::Strong { 2.0 * vertical } else { vertical };
            result.text_frame = frame(outer.x + horizontal_padding, outer.y + vertical_padding, outer.width - horizontal_padding * 2.0, outer.height - vertical_padding * 2.0);
        }
        EmphasisStyle::Auto => return Err(crate::Error::Invalid("emphasis style was not resolved".into())),
    }
    if result.text_frame.width <= 0.0 || result.text_frame.height <= 0.0 { return Err(crate::Error::Invalid("emphasis frame is too small for its spacing; enlarge it or split the content".into())); }
    Ok(result)
}

pub const VERSION: u32 = 1;
/// Line spacing used by composed text; East Asian text keeps the same value for Office parity.
const LINE_SPACING: u32 = 115;
/// Matches the default visual preflight floor.
const MINIMUM_SIZE: f64 = 16.0;

#[derive(Clone, Debug, Serialize)]
pub struct DesignTokens {
    pub version: u32,
    /// `preset:<id>` for an unmodified design preset, otherwise `default`.
    pub source: String,
    pub canvas: Canvas,
    pub grid: f64,
    pub margin: f64,
    pub gutter: f64,
    pub gaps: GapScale,
    pub frames: Frames,
    pub type_scale: TypeScale,
    pub scripts: Scripts,
    pub colors: ColorRoles,
    pub card: CardStyle,
    /// Layout that composed slides use; `None` keeps the slide's current layout.
    pub layout_id: Option<String>,
    /// Whether composed slides keep master decorations visible.
    pub master_graphics: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Frames { pub title: Frame, pub body: Frame, pub footer: Frame, pub full_page: Frame }

#[derive(Clone, Copy, Debug, Serialize)]
pub struct TypeScale { pub title: f64, pub statement: f64, pub metric: f64, pub body: f64, pub caption: f64, pub minimum: f64, pub line_spacing: u32 }

/// Approximate character budgets for one script at the title size.
#[derive(Clone, Debug, Serialize)]
pub struct Script { pub language: &'static str, pub advance_em: f64, pub reading_measure: u32, pub title_line: u32, pub title_lines: u32 }

#[derive(Clone, Debug, Serialize)]
pub struct Scripts { pub latin: Script, pub east_asian: Script }

/// Theme slot references, so composed content follows theme changes.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ColorRoles { pub ink: &'static str, pub muted: &'static str, pub background: &'static str, pub surface: &'static str, pub accent: &'static str, pub accent_alt: &'static str, pub on_accent: &'static str }

#[derive(Clone, Copy, Debug, Serialize)]
pub struct CardStyle { pub fill: &'static str, pub rule: f64, pub padding: f64, pub band_padding: f64 }

const ROLES: ColorRoles = ColorRoles { ink: "@dk1", muted: "@dk2", background: "@lt1", surface: "@lt2", accent: "@accent1", accent_alt: "@accent2", on_accent: "@lt1" };

fn even(value: f64) -> f64 { (value / 2.0).round() * 2.0 }

fn grid(value: f64, minimum: f64, maximum: f64) -> f64 { ((value / 8.0).round() * 8.0).clamp(minimum, maximum) }

fn frame(x: f64, y: f64, width: f64, height: f64) -> Frame { Frame { x, y, width, height } }

fn scripts(title: Frame, size: f64) -> Scripts {
    let lines = ((title.height / (size * 1.15)).floor() as u32).max(1);
    let script = |language, advance_em: f64, reading_measure| Script { language, advance_em, reading_measure, title_line: (title.width / (size * advance_em)).floor() as u32, title_lines: lines };
    Scripts { latin: script("en-US", 0.55, 70), east_asian: script("ja-JP", 1.0, 40) }
}

/// Tokens for the deck's current design and canvas.
pub fn resolve(deck: &Deck) -> Result<DesignTokens> {
    crate::canvas::validate_size(deck.width, deck.height)?;
    let canvas = Canvas { width: deck.width, height: deck.height };
    let preset = deck.design.as_ref().filter(|_| deck.width == 1280 && deck.height == 720).and_then(crate::design_presets::recognize);
    Ok(match preset { Some(geometry) => preset_tokens(canvas, &geometry), None => default_tokens(canvas, deck) })
}

pub fn resolve_profile(deck: &Deck, profile: Option<ComponentProfile>) -> Result<DesignTokens> {
    let mut tokens = resolve(deck)?;
    if profile == Some(ComponentProfile::TechnicalBriefing) {
        let horizontal = deck.width as f64 / 1280.0;
        let vertical = deck.height as f64 / 720.0;
        tokens.source = "component:technical_briefing".into();
        tokens.margin = 72.0 * horizontal;
        let width = deck.width as f64 - tokens.margin * 2.0;
        tokens.frames.title = frame(tokens.margin, 64.0 * vertical, width, 60.0 * vertical);
        tokens.frames.body = frame(tokens.margin, 144.0 * vertical, width, 496.0 * vertical);
        tokens.frames.footer = frame(tokens.margin, 650.0 * vertical, width, 28.0 * vertical);
        tokens.type_scale.title = 40.0 * vertical;
        tokens.type_scale.body = 18.0 * vertical;
        tokens.type_scale.caption = 14.0 * vertical;
        tokens.type_scale.minimum = 12.0 * vertical;
        tokens.scripts = scripts(tokens.frames.title, tokens.type_scale.title);
        tokens.card.padding = 16.0 * vertical;
        tokens.card.band_padding = 16.0 * vertical;
        tokens.gaps.peer = 24.0 * vertical;
    }
    Ok(tokens)
}

pub fn component_styles(deck: &Deck, profile: ComponentProfile) -> Result<serde_json::Value> {
    let tokens = resolve_profile(deck, Some(profile))?;
    let horizontal = deck.width as f64 / 1280.0;
    let vertical = deck.height as f64 / 720.0;
    let role = |frame: Frame, size: f64, family: &str, alignment: &str, vertical_alignment: &str| serde_json::json!({
        "frame": frame, "font_size": size, "font_family": family, "alignment": alignment, "vertical": vertical_alignment
    });
    let outer = frame(tokens.margin, 568.0 * vertical, tokens.frames.body.width, 72.0 * vertical);
    let takeaway = emphasis_layout(&tokens, outer, &EmphasisSpec::default())?;
    let emphasis_variants = [EmphasisStyle::Inline, EmphasisStyle::Rule, EmphasisStyle::Outline, EmphasisStyle::Tint, EmphasisStyle::Closing, EmphasisStyle::Solid].into_iter().map(|style| {
        let (use_when, avoid_when) = match style {
            EmphasisStyle::Inline => ("A quiet note or qualifier that should stay with the primary content without a boundary.", "A conclusion; it reads as an afterthought."),
            EmphasisStyle::Rule => ("A normal supporting takeaway or qualifier that needs a visible boundary from body and source.", "The one conclusion of a slide; use closing, lead or solid."),
            EmphasisStyle::Outline => ("A standalone or contrasting group, or a strong warning, that needs containment, not a filled card.", "Routine qualifiers; the box outranks the body."),
            EmphasisStyle::Tint => ("A strongly emphasized supporting point that should read as a distinct unit.", "Qualifiers and caveats (use inline, rule or outline) and conclusions (use closing or solid); the surface is only slightly darker than the page."),
            EmphasisStyle::Closing => ("The conclusion read after the body: a decision, next action or one-sentence summary. One closing sentence at 28px, ruled off from the body.", "Qualifiers, or text longer than one line at 28px; shorten it or use lead or rule."),
            _ => ("The single conclusion of a summary, chapter-end or decision slide that needs the strongest contrast. Reversed text on the ink role, one line at 24px.", "Ordinary slides and qualifiers; use at most a few times per deck so it keeps its weight. Mark phrases with bold, not accent colors."),
        };
        Ok(serde_json::json!({"style": style, "use_when": use_when, "avoid_when": avoid_when, "layout": emphasis_layout(&tokens, outer, &EmphasisSpec { style, ..Default::default() })?}))
    }).collect::<Result<Vec<_>>>()?;
    let lead = frame(tokens.margin, tokens.frames.body.y, tokens.frames.body.width, crate::layout_patterns::LEAD_BAND * vertical);
    let lead_layout = emphasis_layout_at(&tokens, lead, &EmphasisSpec::default(), EmphasisPlacement::Lead)?;
    let message_placements = serde_json::json!([
        {"id": "lead", "how": "pattern.lead=true and a `lead-message` slot (emphasis, text, callout or label), directly under the title", "frame": lead, "font_size": 24.0 * vertical,
            "use_when": "The claim or conclusion should be read first: executive and decision material, a chapter claim before a long diagram or table, or a reading guide for a dense visual. One line of about 40 full-width characters.",
            "avoid_when": "Slides whose body already states the claim in its first row, and slides that also carry a closing line or solid band; give each slide one conclusion.",
            "layout": lead_layout},
        {"id": "bottom", "how": "pattern.message_band=true (or a pattern band slot) with an emphasis style: closing, solid, rule, outline or inline", "frame": outer, "font_size": 20.0 * vertical,
            "use_when": "A conclusion that follows from the body (closing or solid) or a qualifier set apart from it (rule or inline).",
            "avoid_when": "Restating what the title or the first row already says; delete it instead."},
        {"id": "fold_into_body", "how": "Write the qualifier into the row it limits and mark the phrase with detail_emphasis (icon_rows) or an emphasized run",
            "use_when": "A qualifier, scope limit or exception that belongs to one row or statement; the reader meets it exactly where it applies.",
            "avoid_when": "A conclusion about the whole slide, or a qualifier that applies to every row."},
        {"id": "none", "how": "Omit the slot; put the claim in the title",
            "use_when": "The title already states the claim and the body is the evidence, or the slide is a diagram, screenshot or table that speaks for itself.",
            "avoid_when": "A summary or decision slide with no stated takeaway."}
    ]);
    Ok(serde_json::json!({"version": 1, "profile": profile, "canvas": tokens.canvas, "roles": {
        "title": role(tokens.frames.title, tokens.type_scale.title, "@major", "left", "top"),
        "body": role(frame(tokens.margin, 144.0 * vertical, tokens.frames.body.width, 400.0 * vertical), tokens.type_scale.body, "@minor", "left", "top"),
        "takeaway": {"frame": takeaway.text_frame, "band_frame": outer, "text_frame": takeaway.text_frame, "separator_frame": takeaway.separator_frame, "font_size": 20.0 * vertical, "font_family": "@minor", "alignment": "left", "vertical": "middle", "accent": tokens.colors.accent_alt, "padding": {"left": 0.0, "right": 0.0, "top": 0.0, "bottom": 0.0}},
        "source": role(tokens.frames.footer, tokens.type_scale.caption, "@minor", "left", "middle"),
        "page": role(frame(deck.width as f64 - 112.0 * horizontal, 650.0 * vertical, 40.0 * horizontal, 28.0 * vertical), tokens.type_scale.caption, "@minor", "right", "middle")
    }, "emphasis_variants": emphasis_variants, "message_placements": message_placements, "emphasis_selection": "auto uses supplied strength and relationship: subtle -> inline; normal supporting or conditional -> rule; strong conditional, standalone or contrast -> outline; strong supporting -> tint. Closing and solid are never chosen automatically: select them explicitly for a conclusion (see message_placements). Explicit style overrides auto. These inputs do not verify semantic truth.", "use": "Use these roles before adding recipe, part or manual content. Keep short readings open, distinguish takeaways from source text, and choose emphasis by strength and relationship rather than always boxing or always removing boxes. Give each slide at most one conclusion and choose its placement from message_placements: lead for a claim read first, closing or solid after the body, a folded phrase for a qualifier. Use supplied meaningful icons for short peers. Resolve each body layout by its information relationship; role consistency does not require identical internal diagrams.", "office_visual_parity": false, "visual_quality_verified": false}))
}

fn preset_tokens(canvas: Canvas, preset: &Geometry) -> DesignTokens {
    let margin = preset.margin;
    let content = canvas.width as f64 - margin * 2.0;
    let title = frame(margin, 64.0, content, 112.0);
    let type_scale = TypeScale {
        title: preset.heading, statement: even(preset.heading * 0.8), metric: even(preset.heading * 1.5).min(96.0),
        body: preset.body, caption: (preset.body - 8.0).max(MINIMUM_SIZE), minimum: MINIMUM_SIZE, line_spacing: LINE_SPACING,
    };
    DesignTokens {
        version: VERSION, source: format!("preset:{}", preset.id), canvas, grid: 8.0, margin, gutter: preset.gutter,
        gaps: GapScale { tight: 16.0, peer: grid(preset.gutter / 2.0, 16.0, 32.0), support: grid(preset.gutter * 2.0 / 3.0, 24.0, 48.0), contrast: grid(preset.gutter, 32.0, 64.0) },
        frames: Frames { title, body: frame(margin, preset.body_top, content, 640.0 - preset.body_top), footer: frame(margin, 644.0, content, 28.0), full_page: frame(margin, 64.0, content, 576.0) },
        type_scale, scripts: scripts(title, type_scale.title), colors: ROLES,
        card: CardStyle { fill: ROLES.surface, rule: preset.rule, padding: 24.0, band_padding: 16.0 },
        layout_id: Some("preset-blank".into()), master_graphics: true,
    }
}

fn default_tokens(canvas: Canvas, deck: &Deck) -> DesignTokens {
    let (width, height) = (canvas.width as f64, canvas.height as f64);
    let wide = width / height >= 1.5;
    let scale = height / 720.0;
    let margin = if wide { 48.0 } else { 40.0 };
    let content = width - margin * 2.0;
    let title = frame(margin, 24.0, content, 88.0);
    let type_scale = TypeScale {
        title: even(36.0 * scale), statement: even(32.0 * scale), metric: even(64.0 * scale), body: even(20.0 * scale),
        caption: even(16.0 * scale).max(MINIMUM_SIZE), minimum: MINIMUM_SIZE, line_spacing: LINE_SPACING,
    };
    let empty_layout = deck.design.as_ref().and_then(|design| design.layouts.iter().find(|layout| layout.elements.is_empty())).map(|layout| layout.id.clone());
    DesignTokens {
        version: VERSION, source: "default".into(), canvas, grid: 8.0, margin, gutter: if wide { 32.0 } else { 24.0 },
        gaps: GapScale::standard(wide),
        frames: Frames { title, body: frame(margin, 120.0, content, height - 180.0), footer: frame(margin, height - 48.0, content, 28.0), full_page: frame(margin, margin, content, height - margin * 2.0) },
        type_scale, scripts: scripts(title, type_scale.title), colors: ROLES,
        card: CardStyle { fill: ROLES.surface, rule: 3.0, padding: if wide { 24.0 } else { 20.0 }, band_padding: 16.0 },
        layout_id: empty_layout, master_graphics: false,
    }
}
