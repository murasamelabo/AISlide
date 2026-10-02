use super::{PartFit, PartSpec};
use crate::{model::Element, Error, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

const CANVAS_WIDTH: f64 = 1152.0;
const LOCAL_AREA: f64 = 0.03;
const SQUARE: f64 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity { Free, Tolerant, Strict }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoding { None, Area, Count, Geography }

/// Every category declares whether its data meaning lives in shape, area, count or geography.
const ENCODINGS: [(&str, Encoding); 36] = [
    ("pie-chart", Encoding::None), ("vertical-bar-graph", Encoding::None), ("add-vertical-bar-graph", Encoding::None), ("100-add-vertical-bar-graph", Encoding::None),
    ("horizontal-bar-graph", Encoding::None), ("area-graph", Encoding::None), ("water-fall", Encoding::None), ("line-graph", Encoding::None),
    ("infographic", Encoding::Count), ("scatter", Encoding::None), ("tree", Encoding::None), ("pyramid", Encoding::None), ("flow", Encoding::None),
    ("vertical-flow", Encoding::None), ("cycle", Encoding::None), ("before-after", Encoding::None), ("map", Encoding::Geography), ("puzzle", Encoding::None),
    ("radiation", Encoding::None), ("correlation", Encoding::None), ("matrix", Encoding::None), ("venn", Encoding::None), ("cross", Encoding::None),
    ("set", Encoding::None), ("list-set", Encoding::None), ("contrast", Encoding::None), ("scale-contrast", Encoding::Area), ("grow", Encoding::Area),
    ("layer", Encoding::None), ("triangle", Encoding::None), ("step", Encoding::None), ("gantt-chart", Encoding::None), ("list", Encoding::None),
    ("list-horizontal", Encoding::None), ("list-enumeration", Encoding::None), ("ranking", Encoding::None),
];

#[derive(Clone, Debug, Serialize)]
pub struct AspectProfile {
    pub layout: &'static str,
    pub canvas: [f64; 2],
    pub sensitivity: Sensitivity,
    pub encoding: Encoding,
    pub markers: usize,
    pub tolerance: Option<f64>,
    pub reasons: Vec<String>,
}

fn encoding(category: &str) -> Result<Encoding> {
    if category == "diagram" { return Ok(Encoding::None); }
    ENCODINGS.iter().find(|entry| entry.0 == category).map(|entry| entry.1)
        .ok_or_else(|| Error::Invalid(format!("part category {category} must declare its aspect encoding")))
}

fn frame(element: &Element) -> [f64; 4] {
    match element {
        Element::Text { x, y, width, height, .. } | Element::Rect { x, y, width, height, .. } | Element::Polygon { x, y, width, height, .. }
        | Element::Shape { x, y, width, height, .. } | Element::Table { x, y, width, height, .. } | Element::Chart { x, y, width, height, .. }
        | Element::Picture { x, y, width, height, .. } | Element::Connector { x, y, width, height, .. } | Element::Group { x, y, width, height, .. } => [*x, *y, *width, *height],
    }
}

fn square(width: f64, height: f64) -> bool { width >= 8.0 && (width - height).abs() <= width.max(height) * SQUARE }

fn diagonal(points: &[[f64; 2]], closed: bool) -> bool {
    let edges = if closed { points.len() } else { points.len().saturating_sub(1) };
    (0..edges).any(|index| { let (a, b) = (points[index], points[(index + 1) % points.len()]); (a[0] - b[0]).abs() > 1e-6 && (a[1] - b[1]).abs() > 1e-6 })
}

/// Rendered geometry, not the category, decides how an element tolerates per-axis stretching.
fn classify(element: &Element) -> (Sensitivity, &'static str) {
    match element {
        Element::Picture { .. } => (Sensitivity::Strict, "images"),
        Element::Rect { width, height, .. } if square(*width, *height) => (Sensitivity::Strict, "square tiles"),
        Element::Shape { width, height, rotation, preset, .. } => {
            if square(*width, *height) { (Sensitivity::Strict, "circles or square markers") }
            else if *rotation != 0.0 { (Sensitivity::Tolerant, "rotated shapes") }
            else if matches!(preset.as_str(), "rect" | "roundRect") { (Sensitivity::Free, "") }
            else { (Sensitivity::Tolerant, "arrows, chevrons or ovals") }
        }
        Element::Polygon { points, .. } if points.len() > 8 => (Sensitivity::Strict, "curved outlines"),
        Element::Polygon { points, .. } if diagonal(points, true) => (Sensitivity::Tolerant, "diagonal edges"),
        Element::Connector { width, height, routing, .. } => {
            let straight = [[0.0, 0.0], [1.0, 1.0]];
            let points = routing.as_ref().map_or(&straight[..], |routing| routing.points.as_slice());
            if *width > 1.0 && *height > 1.0 && diagonal(points, false) { (Sensitivity::Tolerant, "diagonal connectors") } else { (Sensitivity::Free, "") }
        }
        Element::Group { children, .. } => children.iter().map(classify).max_by_key(|entry| entry.0).unwrap_or((Sensitivity::Free, "")),
        _ => (Sensitivity::Free, ""),
    }
}

fn local(element: &Element, canvas_area: f64) -> bool { let [_, _, width, height] = frame(element); width * height <= canvas_area * LOCAL_AREA }

/// Small square markers and images are re-squared after stretching, so they never distort.
pub(crate) fn is_marker(element: &Element, canvas_area: f64) -> bool {
    matches!(element, Element::Picture { .. } | Element::Rect { .. } | Element::Shape { .. })
        && classify(element).0 == Sensitivity::Strict && local(element, canvas_area)
}

pub(crate) fn restore_marker(element: &mut Element, horizontal: f64, vertical: f64) {
    let scale = horizontal.min(vertical);
    if let Element::Picture { x, y, width, height, .. } | Element::Rect { x, y, width, height, .. } | Element::Shape { x, y, width, height, .. } = element {
        let (fitted_width, fitted_height) = (*width * scale / horizontal, *height * scale / vertical);
        *x += (*width - fitted_width) / 2.0; *y += (*height - fitted_height) / 2.0;
        *width = fitted_width; *height = fitted_height;
    }
}

/// Height of the canvas region mapped into a part layout frame.
pub fn content_height(spec: &PartSpec) -> f64 {
    let show_title = spec.layout.as_ref().is_none_or(|layout| layout.show_title);
    if show_title { 512.0 } else if spec.preset == "venn/focus" { 432.0 } else { 424.0 }
}

pub fn profile(spec: &PartSpec) -> Result<AspectProfile> {
    let category = spec.preset.split('/').next().unwrap_or_default();
    let encoding = encoding(category)?;
    let canvas = [CANVAS_WIDTH, content_height(spec)];
    if super::briefing::kind(&spec.data).is_some() {
        return Ok(AspectProfile { layout: "native", canvas, sensitivity: Sensitivity::Free, encoding, markers: 0, tolerance: None, reasons: vec!["lays out natively inside the frame".into()] });
    }
    let mut natural = spec.clone();
    natural.layout = None;
    let element = super::create_with_theme("aspect-probe", &natural, &crate::design::Theme::default())?;
    let Element::Group { view_width, view_height, children, .. } = &element else { return Err(Error::Invalid("part aspect requires a group".into())); };
    let area = view_width * view_height;
    let (mut sensitivity, mut markers, mut reasons) = (Sensitivity::Free, 0, BTreeSet::new());
    for child in children {
        let (class, reason) = classify(child);
        if class == Sensitivity::Free || (class == Sensitivity::Tolerant && local(child, area)) { continue; }
        if is_marker(child, area) { markers += 1; continue; }
        sensitivity = sensitivity.max(class);
        reasons.insert(reason.to_owned());
    }
    if encoding != Encoding::None {
        sensitivity = Sensitivity::Strict;
        reasons.insert(format!("{} encodes data", match encoding { Encoding::Area => "area", Encoding::Count => "unit count", Encoding::Geography => "geography", Encoding::None => "" }));
    }
    let tolerance = match sensitivity { Sensitivity::Free => None, Sensitivity::Tolerant => Some(1.15), Sensitivity::Strict => Some(if encoding == Encoding::None { 1.05 } else { 1.02 }) };
    Ok(AspectProfile { layout: "canvas", canvas, sensitivity, encoding, markers, tolerance, reasons: reasons.into_iter().collect() })
}

/// How a part with this profile fills a frame: per-axis distortion, recommended fit and share of the frame used.
pub fn fit_in(profile: &AspectProfile, width: f64, height: f64) -> Value {
    if profile.layout == "native" { return json!({"distortion": 1.0, "fit": "native", "area_used": 1.0, "tolerance": null}); }
    let (horizontal, vertical) = (width / profile.canvas[0], height / profile.canvas[1]);
    let distortion = horizontal.max(vertical) / horizontal.min(vertical);
    let stretch = profile.tolerance.is_none_or(|tolerance| distortion <= tolerance);
    let area_used = if stretch { 1.0 } else { horizontal.min(vertical).powi(2) * profile.canvas[0] * profile.canvas[1] / (width * height) };
    json!({"distortion": (distortion * 100.0).round() / 100.0, "fit": if stretch { "stretch" } else { "contain" }, "area_used": (area_used * 100.0).round() / 100.0, "tolerance": profile.tolerance})
}

/// Profile of a catalog preset's example, as placed without (default) or with its title band.
pub fn profile_for_preset(preset: &str, show_title: bool) -> Result<AspectProfile> {
    let catalog = super::catalog();
    let example = catalog["presets"].as_array().and_then(|presets| presets.iter().find(|entry| entry["id"] == preset))
        .ok_or_else(|| Error::Invalid(format!("unknown part preset {preset}; call part_catalog for preset IDs")))?["example"].clone();
    let mut spec: PartSpec = serde_json::from_value(example)?;
    spec.layout = Some(super::PartLayout { x: 0.0, y: 0.0, width: CANVAS_WIDTH, height: 424.0, show_title, fit: PartFit::Stretch });
    profile(&spec)
}

/// Distortion of an explicit stretched layout that exceeds the part's tolerance.
pub fn excessive_distortion(spec: &PartSpec) -> Result<Option<(f64, AspectProfile)>> {
    let Some(layout) = &spec.layout else { return Ok(None) };
    if layout.fit == PartFit::Contain { return Ok(None); }
    let profile = profile(spec)?;
    let Some(tolerance) = profile.tolerance else { return Ok(None) };
    let (horizontal, vertical) = (layout.width / profile.canvas[0], layout.height / profile.canvas[1]);
    let distortion = horizontal.max(vertical) / horizontal.min(vertical);
    Ok((distortion > tolerance).then_some((distortion, profile)))
}

#[cfg(test)]
mod tests {
    use crate::parts::catalog::CATEGORIES;

    #[test]
    fn every_catalog_category_declares_an_encoding() {
        for (category, ..) in CATEGORIES { super::encoding(category).unwrap(); }
        assert_eq!(super::ENCODINGS.len(), CATEGORIES.len());
    }
}
