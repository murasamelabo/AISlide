//! Design tokens: one resolved source of frames, spacing, type scale, script metrics and color
//! roles for deterministic composition. Tokens are derived from the deck's design on demand and
//! are never persisted: an unmodified design preset yields its own scale, any other design
//! yields neutral defaults scaled to the canvas.
use crate::{design_presets::Geometry, layout_patterns::{Canvas, Frame, GapScale}, model::Deck, Result};
use serde::Serialize;

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
