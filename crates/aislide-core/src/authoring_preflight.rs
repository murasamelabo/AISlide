use crate::{document::Document, model::{Deck, Element, TextFormat}, Error, Result};
use kurbo::{Affine, Point, Rect};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PreflightOptions {
	pub page_indices: Option<Vec<usize>>,
	pub min_font_size: f64,
}

impl Default for PreflightOptions {
	fn default() -> Self { Self { page_indices: None, min_font_size: 16.0 } }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Bounds { pub x: f64, pub y: f64, pub width: f64, pub height: f64 }

impl From<Rect> for Bounds {
	fn from(rect: Rect) -> Self { Self { x: rect.x0, y: rect.y0, width: rect.width(), height: rect.height() } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
	pub code: String,
	pub severity: String,
	pub evidence: String,
	pub page_index: usize,
	pub slide_id: String,
	pub element_ids: Vec<String>,
	pub scopes: Vec<String>,
	pub bounds: Bounds,
	pub message: String,
	pub suggestions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreflightReport {
	pub revision: u64,
	pub hash: String,
	pub page_indices: Vec<usize>,
	pub findings: Vec<Finding>,
	pub checks: Vec<String>,
	pub limitations: Vec<String>,
	pub office_visual_parity: bool,
	pub semantic_truth_verified: bool,
}

struct Object<'a> {
	id: &'a str,
	scope: String,
	bounds: Rect,
	frame: Rect,
	transform: Affine,
	text: bool,
	font_size: Option<f64>,
	characters: usize,
	route: Vec<Point>,
	connections: Vec<&'a str>,
}

fn minimum_font(format: &TextFormat, base: f64) -> f64 {
	format.paragraphs.iter().flat_map(|paragraph| &paragraph.runs)
		.filter(|run| !run.text.trim().is_empty()).map(|run| run.style.font_size.unwrap_or(base))
		.reduce(f64::min).unwrap_or(base)
}

fn collect<'a>(elements: &'a [Element], parent: Affine, scope: &str, objects: &mut Vec<Object<'a>>) -> Result<()> {
	for element in elements {
		if element.visual().is_some_and(|style| style.hidden) { continue; }
		if objects.len() >= 1024 { return Err(Error::Limit("visual preflight supports at most 1024 visible objects per page".into())); }
		let (id, x, y, width, height) = element.bounds();
		let visual = element.visual().cloned().unwrap_or_default();
		let rotation = match element { Element::Shape { rotation, .. } => *rotation, _ => visual.rotation.unwrap_or(0.0) };
		let transform = parent * Affine::translate((x, y)) * Affine::translate((width / 2.0, height / 2.0))
			* Affine::rotate(rotation.to_radians())
			* Affine::scale_non_uniform(if visual.flip_h { -1.0 } else { 1.0 }, if visual.flip_v { -1.0 } else { 1.0 })
			* Affine::translate((-width / 2.0, -height / 2.0));
		let frame = Rect::new(0.0, 0.0, width, height);
		let bounds = transform.transform_rect_bbox(frame);
		let coefficients = transform.as_coeffs();
		let scale = coefficients[0].hypot(coefficients[1]).min(coefficients[2].hypot(coefficients[3]));
		let (text, font_size, characters) = match element {
			Element::Text { text, font_size, format, .. } | Element::Shape { text, font_size, format, .. } =>
				(!text.trim().is_empty(), Some(minimum_font(format, *font_size) * scale), text.chars().count()),
			Element::Table { rows, font_size, format, .. } => {
				let minimum = rows.iter().enumerate().flat_map(|(row, cells)| cells.iter().enumerate().filter_map(move |(column, text)| {
					if text.trim().is_empty() { return None; }
					Some(minimum_font(&format.cell_style(row, column).text(text), *font_size) * scale)
				})).reduce(f64::min);
				(false, minimum, rows.iter().flatten().map(|text| text.chars().count()).sum())
			}
			_ => (false, None, 0),
		};
		let (route, connections) = if let Element::Connector { routing, flip_v, start, end, .. } = element {
			let points = routing.as_ref().map(|route| route.points.clone()).unwrap_or_else(|| vec![[0.0, if *flip_v { 1.0 } else { 0.0 }], [1.0, if *flip_v { 0.0 } else { 1.0 }]]);
			(points.into_iter().map(|point| transform * Point::new(point[0] * width, point[1] * height)).collect(),
				start.iter().chain(end.iter()).map(|connection| connection.element_id.as_str()).collect())
		} else { (Vec::new(), Vec::new()) };
		objects.push(Object { id, scope: scope.into(), bounds, frame, transform, text, font_size, characters, route, connections });
		if let Element::Group { view_width, view_height, children, .. } = element {
			collect(children, transform * Affine::scale_non_uniform(width / view_width, height / view_height), scope, objects)?;
		}
	}
	Ok(())
}

fn page_objects(deck: &Deck, page: usize) -> Result<Vec<Object<'_>>> {
	let slide = &deck.slides[page];
	let mut objects = Vec::new();
	if let Some(design) = &deck.design {
		let layout = slide.layout_id.as_ref().and_then(|id| design.layouts.iter().find(|layout| &layout.id == id)).or_else(|| design.layouts.first());
		if let Some(layout) = layout {
			if !slide.hide_master_graphics {
				if let Some(master) = design.masters.iter().find(|master| master.id == layout.master_id) {
					for element in &master.elements {
						if matches!(element, Element::Text { format, .. } if format.placeholder.is_some()) { continue; }
						collect(std::slice::from_ref(element), Affine::IDENTITY, &format!("master:{}", master.id), &mut objects)?;
					}
				}
			}
			for element in &layout.elements {
				if matches!(element, Element::Text { format, .. } if format.placeholder.is_some()) { continue; }
				collect(std::slice::from_ref(element), Affine::IDENTITY, &format!("layout:{}", layout.id), &mut objects)?;
			}
		}
	}
	collect(&slide.elements, Affine::IDENTITY, "slide", &mut objects)?;
	Ok(objects)
}

fn crosses_frame(start: Point, end: Point, rect: Rect) -> bool {
	let mut low = 0.0_f64;
	let mut high = 1.0_f64;
	for (position, delta, minimum, maximum) in [(start.x, end.x - start.x, rect.x0, rect.x1), (start.y, end.y - start.y, rect.y0, rect.y1)] {
		if delta.abs() < 1e-9 {
			if position <= minimum || position >= maximum { return false; }
		} else {
			let first = (minimum - position) / delta;
			let last = (maximum - position) / delta;
			low = low.max(first.min(last));
			high = high.min(first.max(last));
			if high - low <= 1e-9 { return false; }
		}
	}
	true
}

fn push_finding(report: &mut PreflightReport, page: usize, slide: &str, objects: &[&Object<'_>], code: &str, severity: &str, evidence: &str, message: &str, suggestions: &[&str]) -> Result<()> {
	if report.findings.len() >= 256 { return Err(Error::Limit("visual preflight exceeds 256 findings; select fewer pages or resolve reported density".into())); }
	let bounds = objects.iter().map(|object| object.bounds).reduce(|first, next| first.union(next)).unwrap_or(Rect::ZERO);
	report.findings.push(Finding { code: code.into(), severity: severity.into(), evidence: evidence.into(), page_index: page, slide_id: slide.into(),
		element_ids: objects.iter().map(|object| object.id.into()).collect(), scopes: objects.iter().map(|object| object.scope.clone()).collect(), bounds: bounds.into(),
		message: message.into(), suggestions: suggestions.iter().map(|suggestion| (*suggestion).into()).collect() });
	Ok(())
}

pub fn preflight_presentation(document: &Document, options: &PreflightOptions) -> Result<PreflightReport> {
	crate::document::verify(document)?;
	let deck = &document.deck;
	let selected = options.page_indices.clone().unwrap_or_else(|| (0..deck.slides.len()).collect());
	if selected.is_empty() || selected.len() > 8 || selected.iter().any(|index| *index >= deck.slides.len()) || selected.iter().collect::<BTreeSet<_>>().len() != selected.len() {
		return Err(Error::Invalid("visual preflight requires 1-8 unique existing page indices".into()));
	}
	if !options.min_font_size.is_finite() || !(8.0..=48.0).contains(&options.min_font_size) {
		return Err(Error::Invalid("visual preflight minimum font size must be in 8..=48 pixels".into()));
	}
	let mut report = PreflightReport { revision: document.revision, hash: document.hash.clone(), page_indices: selected.clone(), findings: Vec::new(),
		checks: ["renderer_warnings", "off_slide", "text_overlap", "connector_label_interference", "small_text", "density"].map(String::from).to_vec(),
		limitations: ["Static renderer, not Office visual parity or semantic truth verification", "Text overlap uses transformed frame bounds, not glyph intersection; intentional overlapping text needs human review", "Connector checks exclude attached endpoint nodes; backgrounds and nontext containment are not collisions", "Density and font floors are heuristics; charts, orphan lines, contrast and full accessibility require separate review", "Unsupported renderer content fails closed; no partial all-clear report"].map(String::from).to_vec(),
		office_visual_parity: false, semantic_truth_verified: false };
	for page in selected {
		let slide = &deck.slides[page];
		let objects = page_objects(deck, page)?;
		let rendered = crate::render::render_slide_svg(deck, page, false)?;
		for warning in rendered.warnings {
			let object = objects.iter().rev().find(|object| warning.element_id == object.id || warning.element_id.starts_with(&format!("{}[", object.id)));
			let evidence = object.into_iter().collect::<Vec<_>>();
			let severity = if matches!(warning.code.as_str(), "TEXT_OVERFLOW" | "MISSING_GLYPHS") { "error" } else { "warning" };
			let suggestions: &[&str] = match warning.code.as_str() {
				"TEXT_OVERFLOW" => &["Enlarge or reflow the text frame", "Shorten or split content only with author approval"],
				"MISSING_GLYPHS" | "FONT_FALLBACK" => &["Select an available font or explicitly supply a licensed document font"],
				_ => &["Inspect the preview and compare in Office when visual parity matters"],
			};
			push_finding(&mut report, page, &slide.id, &evidence, &warning.code, severity, "renderer", &warning.message, suggestions)?;
			if object.is_none() {
				if let Some(finding) = report.findings.last_mut() { finding.element_ids.push(warning.element_id); }
			}
		}
		for (index, object) in objects.iter().enumerate() {
			if object.bounds.x0 < -0.5 || object.bounds.y0 < -0.5 || object.bounds.x1 > f64::from(deck.width) + 0.5 || object.bounds.y1 > f64::from(deck.height) + 0.5 {
				push_finding(&mut report, page, &slide.id, &[object], "OFF_SLIDE", "warning", "geometry", "Transformed object extends beyond the slide", &["Move or resize the object within the slide"])?;
			}
			if object.characters > 0 && object.font_size.is_some_and(|size| size + 0.01 < options.min_font_size) {
				push_finding(&mut report, page, &slide.id, &[object], "SMALL_TEXT", "warning", "heuristic", &format!("Effective font is below the requested {}px floor", options.min_font_size), &["Increase font size and enlarge or reflow the frame", "Split dense content into another slide"])?;
			}
			if !object.text { continue; }
			for other in &objects[index + 1..] {
				let overlap = object.bounds.intersect(other.bounds);
				if other.text && overlap.width() > 2.0 && overlap.height() > 2.0 {
					push_finding(&mut report, page, &slide.id, &[object, other], "TEXT_OVERLAP", "warning", "geometry", "Visible text frames overlap; confirm whether intentional", &["Move, align or reflow the text frames", "Inspect rotated text before applying a repair"])?;
				}
			}
			for connector in objects.iter().filter(|candidate| !candidate.route.is_empty()) {
				if connector.scope == object.scope && connector.connections.contains(&object.id) { continue; }
				let inverse = object.transform.inverse();
				if connector.route.windows(2).any(|segment| crosses_frame(inverse * segment[0], inverse * segment[1], object.frame)) {
					push_finding(&mut report, page, &slide.id, &[object, connector], "CONNECTOR_LABEL_INTERFERENCE", "warning", "geometry", "A connector crosses a text or label frame; a label fill may hide the line", &["Place the label above, below or beside the route", "Use a transparent label and keep text clear of the connector"])?;
				}
			}
		}
		if objects.iter().map(|object| object.characters).sum::<usize>() > 1200 || objects.iter().filter(|object| object.text).count() > 80 {
			push_finding(&mut report, page, &slide.id, &[], "DENSE_SLIDE", "warning", "heuristic", "Slide exceeds 1200 text scalars or 80 text frames", &["Split content into overview and detail slides", "Move supporting narrative to speaker notes"])?;
		}
		if document.bindings.iter().any(|binding| binding.stale) {
			push_finding(&mut report, page, &slide.id, &[], "SOURCE_BINDINGS_STALE", "warning", "document", "Document has stale source bindings; this report does not verify the claims", &["Undo or rebind before export"])?;
		}
	}
	Ok(report)
}