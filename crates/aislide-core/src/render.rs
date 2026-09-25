//! Static scene projection. Text uses the same cosmic-text shaping/wrapping
//! primitives as layout measurement, with installed-font outlines. Preset
//! shapes are preview approximations, not Office geometry or parity evidence.
//! Unsupported effects/chart options fail closed. Returned SVG is generated
//! solely from validated types; there is no arbitrary-SVG rendering endpoint.
//!
//! Shapes: all catalog presets plus can/cloud; nontrivial preset geometry is
//! explicitly approximate. Supports polygons/curves, connectors, groups,
//! rotation/flips, fill opacity, gradients, picture crop/masks, master/layout
//! artwork and inherited backgrounds. Generated shadow/glow/soft-edge filters
//! and mirrored reflections are warned approximations. Eight default WordArt
//! presets warp shaped glyph outlines; PDF semantic text remains searchable.
//! Effect work is preflighted at output scale within 32 MiB and 8192px edges.
//! Rich text supports installed-font shaping, wrap/alignment, run
//! color/size/weight/style, underline/highlight/baseline and paragraph spacing.
//! Tabs and non-Arabic numbering reject; hanging indentation is approximate.
//! Fields use cached display text. Text overflow clips with an explicit warning.
//! Tables use core dimension/merge/padding/style resolution, not equal-cell math.
//! Charts use Plotters primitives for all 24 accepted families. 3D uses a warned
//! 2D projection; combo axes share one plot with independently scaled values.
//! Histogram counts and box quartiles derive from raw samples, never caches.
//! Hierarchies use stable proportional slice-and-dice / angular projections,
//! not squarification; parent labels overlay regions. Preview layout is warned.
//! Cartesian statistics use a shared bounded SVD/axis projection. Unsupported
//! numeric formats warn and use General, without changing native format codes.
//! Fonts are installed or already licensed document-local faces, never fetched.
//! Text is outlined; SVG pictures retain the explicit sanitized raster fallback.
use crate::{
    design::Theme,
    export_static::{RenderWarning, MAX_SVG_BYTES},
    model::{validate_deck, Bullet, Deck, Element, TextAlign, TextFormat, VerticalAlign},
    rich_text::{RichParagraph, RichRun, RunStyle, Spacing},
    visual::{Gradient, VisualStyle},
    Error, Result,
};
use cosmic_text::{
    Align, Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, Weight, Wrap,
};
use std::{
    collections::BTreeSet,
    fmt::Write as _,
    sync::{Mutex, OnceLock},
};
use xmlwriter::{Options, XmlWriter};

static FONTS: OnceLock<Mutex<FontSystem>> = OnceLock::new();
pub struct RenderedSlide {
    pub svg: String,
    pub warnings: Vec<RenderWarning>,
    pub(crate) text: Vec<RenderedText>,
    pub(crate) objects: Vec<(String, String)>,
}

pub(crate) struct RenderedText {
    pub element_id: String,
    pub svg_id: String,
    pub glyphs: Vec<RenderedGlyph>,
}

pub(crate) struct RenderedGlyph {
    pub text: String,
    pub origin: [f64; 2],
    pub size: f64,
    pub width: f64,
}

/// Full-deck validation precedes rendering, even for one selected page.
/// Preview limits: 200000 text scalars, 100000 geometry points, 8 MiB SVG,
/// 32 MiB decoded embedded pixels per selected page. No external references.
pub fn render_slide_svg(
    deck: &Deck,
    page_index: usize,
    transparent: bool,
) -> Result<RenderedSlide> {
    validate_deck(deck)?;
    render_validated(deck, page_index, transparent)
}

pub fn render_element_preview(element: &Element, theme: Option<&Theme>) -> Result<serde_json::Value> {
    if !matches!(element, Element::Chart { .. } | Element::Text { .. } | Element::Shape { .. }) { return Err(Error::Unsupported("preview accepts typed chart/text/shape only".into())); }
    let (_, _, _, width, height) = element.bounds();
    let mut value = serde_json::to_value(element)?;
    value["x"] = serde_json::json!(0); value["y"] = serde_json::json!(0);
    let deck: Deck = serde_json::from_value(serde_json::json!({"version":1,"title":"Read-only preview","width":width.ceil().max(320.0) as u32,"height":height.ceil().max(320.0) as u32,"slides":[{"id":"preview","title":"Preview","background":"FFFFFF","notes":"","elements":[value]}]}))?;
    validate_deck(&deck)?;
    let fallback = Theme::default();
    let theme = theme.unwrap_or(&fallback);
    crate::design::validate_theme(theme)?;
    let mut filter_bytes = 0.0; let mut nodes = 0;
    filter_budget(&deck.slides[0].elements[0], 1.0, 1.0, 1, &mut filter_bytes, &mut nodes)?;
    let mut fonts = FONTS.get_or_init(|| Mutex::new(FontSystem::new())).lock().map_err(|_| Error::Invalid("render font state unavailable".into()))?;
    let mut scene = Scene { writer: XmlWriter::new(Options { indent: xmlwriter::Indent::None, ..Default::default() }), theme, fonts: &mut fonts, page_index: 0, warnings: vec![], sequence: 0, path_bytes: 0, text: vec![], objects: vec![] };
    scene.writer.start_element("svg"); scene.writer.write_attribute("xmlns", "http://www.w3.org/2000/svg"); scene.writer.write_attribute("xmlns:xlink", "http://www.w3.org/1999/xlink");
    scene.writer.write_attribute("width", &width); scene.writer.write_attribute("height", &height); scene.writer.write_attribute("viewBox", &format!("0 0 {width} {height}"));
    scene.element(&deck.slides[0].elements[0])?;
    scene.writer.end_element();
    let svg = scene.writer.end_document();
    if svg.len() > MAX_SVG_BYTES { return Err(Error::Limit("preview SVG > 8 MiB".into())); }
    Ok(serde_json::json!({"svg":svg,"warnings":scene.warnings,"office_parity_verified":false}))
}

pub(crate) fn render_validated(
    deck: &Deck,
    page_index: usize,
    transparent: bool,
) -> Result<RenderedSlide> {
    render_at_scale(deck, page_index, transparent, 1.0)
}

pub(crate) fn render_at_scale(
    deck: &Deck,
    page_index: usize,
    transparent: bool,
    scale: f64,
) -> Result<RenderedSlide> {
    let slide = deck
        .slides
        .get(page_index)
        .ok_or_else(|| Error::Invalid("render page index out of range".into()))?;
    let fallback = Theme::default();
    let theme = crate::design::slide_theme(slide, deck.design.as_ref()).unwrap_or(&fallback);
    let layout = deck.design.as_ref().and_then(|design| {
        slide
            .layout_id
            .as_ref()
            .and_then(|id| design.layouts.iter().find(|layout| &layout.id == id))
            .or_else(|| design.layouts.first())
    });
    let master = layout.and_then(|layout| {
        deck.design
            .as_ref()?
            .masters
            .iter()
            .find(|master| master.id == layout.master_id)
    });
    let layers: Vec<&[Element]> = master
        .filter(|_| !slide.hide_master_graphics)
        .map(|master| master.elements.as_slice())
        .into_iter()
        .chain(layout.map(|layout| layout.elements.as_slice()))
        .chain(std::iter::once(slide.elements.as_slice()))
        .collect();
    let mut characters = 0usize;
    let mut points = 0usize;
    let mut pixels = 0usize;
    let mut filter_bytes = 0.0;
    let mut expanded_nodes = 0;
    for layer in &layers {
        for element in *layer { filter_budget(element, scale, scale, 1, &mut filter_bytes, &mut expanded_nodes)?; }
        for element in crate::model::element_list(layer) {
            match element {
                Element::Text { text, .. } | Element::Shape { text, .. } => {
                    characters += text.chars().count()
                }
                Element::Table { rows, .. } => {
                    characters += rows
                        .iter()
                        .flatten()
                        .map(|text| text.chars().count())
                        .sum::<usize>()
                }
                Element::Polygon {
                    points: vertices, ..
                } => points += vertices.len(),
                Element::Picture {
                    base64,
                    mime_type,
                    svg,
                    ..
                } => {
                    let info = crate::media::inspect_raster(base64, mime_type)?;
                    pixels = pixels.saturating_add(info.width as usize * info.height as usize * 4);
                    if svg.is_some() {
                        pixels = pixels.saturating_add(4 * 1024 * 1024);
                    }
                }
                _ => {}
            }
        }
    }
    if characters > 200_000 || points > 100_000 || pixels > 32 * 1024 * 1024 {
        return Err(Error::Limit(
            "static scene text/geometry/decoded-image budget".into(),
        ));
    }
    let mut fonts = FONTS
        .get_or_init(|| Mutex::new(FontSystem::new()))
        .lock()
        .map_err(|_| Error::Invalid("render font state unavailable".into()))?;
    let mut document_fonts = crate::fonts::document_system(&fonts, deck)?;
    let fonts = document_fonts.as_mut().unwrap_or(&mut fonts);
    let mut scene = Scene {
        writer: XmlWriter::new(Options {
            indent: xmlwriter::Indent::None,
            ..Default::default()
        }),
        theme,
        fonts,
        page_index,
        warnings: Vec::new(),
        sequence: 0,
        path_bytes: 0,
        text: Vec::new(),
        objects: Vec::new(),
    };
    scene.writer.start_element("svg");
    scene
        .writer
        .write_attribute("xmlns", "http://www.w3.org/2000/svg");
    scene
        .writer
        .write_attribute("xmlns:xlink", "http://www.w3.org/1999/xlink");
    scene.writer.write_attribute("width", &deck.width);
    scene.writer.write_attribute("height", &deck.height);
    scene
        .writer
        .write_attribute("viewBox", &format!("0 0 {} {}", deck.width, deck.height));
    if !transparent {
        let background = if slide.inherit_background {
            layout
                .and_then(|layout| layout.background.as_deref())
                .or_else(|| master.map(|master| master.background.as_str()))
                .unwrap_or(&slide.background)
        } else {
            &slide.background
        };
        scene.rectangle(
            [0.0, 0.0, deck.width as f64, deck.height as f64],
            &scene.color(background),
            None,
        );
    }
    for (index, layer) in layers.iter().enumerate() {
        for element in *layer {
            if index + 1 < layers.len()
                && matches!(element,Element::Text {format,..} if format.placeholder.is_some())
            {
                continue;
            }
            scene.element(&crate::fields::display_element(element, page_index + 1))?;
        }
    }
    scene.writer.end_element();
    let svg = scene.writer.end_document();
    if svg.len() > MAX_SVG_BYTES {
        return Err(Error::Limit("render SVG > 8 MiB".into()));
    }
    Ok(RenderedSlide {
        svg,
        warnings: scene.warnings,
        text: scene.text,
        objects: scene.objects,
    })
}

struct Scene<'a> {
    writer: XmlWriter,
    theme: &'a Theme,
    fonts: &'a mut FontSystem,
    page_index: usize,
    warnings: Vec<RenderWarning>,
    sequence: usize,
    path_bytes: usize,
    text: Vec<RenderedText>,
    objects: Vec<(String, String)>,
}
impl Scene<'_> {
    fn color(&self, value: &str) -> String {
        if value == "none" {
            return value.into();
        }
        format!(
            "#{}",
            value
                .strip_prefix('@')
                .and_then(|key| self.theme.colors.get(key))
                .map_or(value, String::as_str)
        )
    }
    fn warning(&mut self, id: &str, code: &str, message: impl Into<String>) {
        let message = message.into();
        if !self.warnings.iter().any(|warning| {
            warning.element_id == id && warning.code == code && warning.message == message
        }) {
            self.warnings.push(RenderWarning {
                code: code.into(),
                page_index: self.page_index,
                element_id: id.into(),
                message,
            });
        }
    }
    fn identity(&mut self) -> String {
        self.sequence += 1;
        format!("g35-{}", self.sequence)
    }
    fn rectangle(&mut self, bounds: [f64; 4], fill: &str, outline: Option<(&str, f64)>) {
        self.writer.start_element("rect");
        for (name, value) in ["x", "y", "width", "height"].into_iter().zip(bounds) {
            self.writer.write_attribute(name, &value);
        }
        self.writer.write_attribute("fill", fill);
        if let Some((color, width)) = outline {
            self.writer.write_attribute("stroke", color);
            self.writer.write_attribute("stroke-width", &width);
        }
        self.writer.end_element();
    }
    fn clip(&mut self, width: f64, height: f64) {
        let id = self.identity();
        self.writer.start_element("defs");
        self.writer.start_element("clipPath");
        self.writer.write_attribute("id", &id);
        self.rectangle([0.0, 0.0, width, height], "#FFFFFF", None);
        self.writer.end_element();
        self.writer.end_element();
        self.writer.start_element("g");
        self.writer
            .write_attribute("clip-path", &format!("url(#{id})"));
    }
    fn fill(&mut self, fill: &str, visual: &VisualStyle) -> String {
        let Some(gradient) = &visual.gradient else {
            return self.color(fill);
        };
        let id = self.identity();
        self.writer.start_element("defs");
        match gradient {
            Gradient::Linear { angle, .. } => {
                self.writer.start_element("linearGradient");
                let radians = angle.to_radians();
                for (name, value) in [
                    ("x1", 0.5 - radians.cos() / 2.0),
                    ("y1", 0.5 - radians.sin() / 2.0),
                    ("x2", 0.5 + radians.cos() / 2.0),
                    ("y2", 0.5 + radians.sin() / 2.0),
                ] {
                    self.writer.write_attribute(name, &value);
                }
            }
            Gradient::Radial { center, .. } => {
                self.writer.start_element("radialGradient");
                self.writer.write_attribute("cx", &center[0]);
                self.writer.write_attribute("cy", &center[1]);
            }
        }
        self.writer.write_attribute("id", &id);
        for stop in gradient.stops() {
            self.writer.start_element("stop");
            self.writer.write_attribute("offset", &stop.offset);
            self.writer
                .write_attribute("stop-color", &self.color(&stop.color));
            self.writer.write_attribute("stop-opacity", &stop.opacity);
            self.writer.end_element();
        }
        self.writer.end_element();
        self.writer.end_element();
        format!("url(#{id})")
    }
    fn primitive(&mut self, name: &str, attributes: &[(&str, String)]) {
        self.writer.start_element(name);
        for (name, value) in attributes { self.writer.write_attribute(*name, value); }
        self.writer.end_element();
    }
    fn effect_filter(&mut self, visual: &VisualStyle, bounds: [f64; 4]) -> Option<String> {
        if visual.shadow.is_none() && visual.glow.is_none() && visual.soft_edge.is_none() { return None; }
        let id = self.identity();
        self.writer.start_element("defs");
        self.writer.start_element("filter");
        self.writer.write_attribute("id", &id);
        self.writer.write_attribute("filterUnits", "userSpaceOnUse");
        self.writer.write_attribute("color-interpolation-filters", "sRGB");
        for (name, value) in ["x", "y", "width", "height"].into_iter().zip(bounds) { self.writer.write_attribute(name, &value); }
        let mut results = Vec::new();
        for (name, color, opacity, blur, offset) in visual.glow.iter().map(|glow| ("glow", glow.color.as_str(), glow.opacity, glow.radius, [0.0, 0.0]))
            .chain(visual.shadow.iter().map(|shadow| ("shadow", shadow.color.as_str(), shadow.opacity, shadow.blur, [shadow.distance * shadow.angle.to_radians().cos(), shadow.distance * shadow.angle.to_radians().sin()]))) {
            self.primitive("feGaussianBlur", &[("in", "SourceAlpha".into()), ("stdDeviation", (blur / 2.0).to_string()), ("result", "blurred".into())]);
            self.primitive("feOffset", &[("in", "blurred".into()), ("dx", offset[0].to_string()), ("dy", offset[1].to_string()), ("result", "offset".into())]);
            self.primitive("feFlood", &[("flood-color", self.color(color)), ("flood-opacity", opacity.to_string()), ("result", "color".into())]);
            self.primitive("feComposite", &[("in", "color".into()), ("in2", "offset".into()), ("operator", "in".into()), ("result", name.into())]);
            results.push(name);
        }
        if let Some(radius) = visual.soft_edge {
            self.primitive("feGaussianBlur", &[("in", "SourceGraphic".into()), ("stdDeviation", (radius / 2.0).to_string()), ("result", "soft".into())]);
        }
        results.push(if visual.soft_edge.is_some() { "soft" } else { "SourceGraphic" });
        self.writer.start_element("feMerge");
        for result in results { self.primitive("feMergeNode", &[("in", result.into())]); }
        self.writer.end_element();
        self.writer.end_element();
        self.writer.end_element();
        Some(id)
    }
    fn reflection(&mut self, source_id: &str, reflection: &crate::visual::Reflection, width: f64, height: f64) {
        let gradient = self.identity();
        let mask = self.identity();
        let padding = reflection.blur * 2.0 + 2.0;
        let filter = self.effect_filter(&VisualStyle { soft_edge: Some(reflection.blur), ..Default::default() }, [-padding, -padding, width + padding * 2.0, height + padding * 2.0]);
        let top = height + reflection.distance;
        self.writer.start_element("defs");
        self.writer.start_element("linearGradient");
        self.writer.write_attribute("id", &gradient);
        self.writer.write_attribute("gradientUnits", "userSpaceOnUse");
        for (name, value) in [("x1", 0.0), ("x2", 0.0), ("y1", top), ("y2", top + height)] { self.writer.write_attribute(name, &value); }
        for (offset, opacity) in [(0.0, reflection.start_opacity), (reflection.end_position, reflection.end_opacity)] {
            self.primitive("stop", &[("offset", offset.to_string()), ("stop-color", "#FFFFFF".into()), ("stop-opacity", opacity.to_string())]);
        }
        self.writer.end_element();
        self.writer.start_element("mask");
        self.writer.write_attribute("id", &mask);
        self.writer.write_attribute("maskUnits", "userSpaceOnUse");
        for (name, value) in [("x", 0.0), ("y", top), ("width", width), ("height", height)] { self.writer.write_attribute(name, &value); }
        self.rectangle([0.0, top, width, height], &format!("url(#{gradient})"), None);
        self.writer.end_element();
        self.writer.end_element();
        self.writer.start_element("g");
        self.writer.write_attribute("mask", &format!("url(#{mask})"));
        self.writer.start_element("g");
        self.writer.write_attribute("transform", &format!("translate(0 {}) scale(1 -1)", height * 2.0 + reflection.distance));
        if let Some(filter) = filter { self.writer.write_attribute("filter", &format!("url(#{filter})")); }
        self.primitive("use", &[("xlink:href", format!("#{source_id}"))]);
        self.writer.end_element();
        self.writer.end_element();
    }
    fn element(&mut self, element: &Element) -> Result<()> {
        let (id, x, y, width, height) = element.bounds();
        let default = VisualStyle::default();
        let visual = element.visual().unwrap_or(&default);
        if visual.hidden {
            return Ok(());
        }
        if visual.text_warp.is_some() { self.warning(id, "WORDART_APPROXIMATION", "Shaped glyph outlines warped with kurbo subdivision (0.25 local-pixel target, bounded paths); default adjustments only, not Office visual parity. PDF semantic text retained; search/selection geometry, underline and highlight remain unwarped"); }
        let rotation = match element {
            Element::Shape { rotation, .. } => *rotation,
            _ => visual.rotation.unwrap_or(0.0),
        };
        self.writer.start_element("g");
        self.writer.write_attribute("transform",&format!("translate({x} {y}) translate({} {}) rotate({rotation}) scale({} {}) translate({} {})",width/2.0,height/2.0,if visual.flip_h {-1}else{1},if visual.flip_v {-1}else{1},-width/2.0,-height/2.0));
        let object_id = self.identity();
        self.writer.write_attribute("id", &object_id);
        self.objects.push((id.into(), object_id.clone()));
        let effect_id = self.identity();
        let filter = self.effect_filter(visual, effect_bounds(element));
        if filter.is_some() || visual.reflection.is_some() {
            self.warning(id, "EFFECT_APPROXIMATION", "Generated SVG Gaussian shadow/glow/soft-edge and local mirrored reflection approximate DrawingML; PDF may rasterize only effect groups, not the page");
        }
        self.writer.start_element("g");
        self.writer.write_attribute("id", &effect_id);
        if let Some(filter) = filter { self.writer.write_attribute("filter", &format!("url(#{filter})")); }
        match element {
            Element::Rect { fill, .. } => {
                let paint = self.fill(fill, visual);
                self.writer.start_element("g");
                self.writer
                    .write_attribute("fill-opacity", &visual.opacity.unwrap_or(1.0));
                self.rectangle([0.0, 0.0, width, height], &paint, None);
                self.writer.end_element();
            }
            Element::Polygon {
                points,
                fill,
                stroke,
                stroke_width,
                ..
            } => {
                let path = if let Some(path) = &visual.path {
                    vector_path(path, width, height)
                } else {
                    let mut path = String::new();
                    for (index, point) in points.iter().enumerate() {
                        let _ = write!(
                            path,
                            "{}{} {} ",
                            if index == 0 { "M" } else { "L" },
                            point[0] * width,
                            point[1] * height
                        );
                    }
                    path.push('Z');
                    path
                };
                self.path(&path, fill, stroke, *stroke_width, visual)?;
            }
            Element::Shape {
                preset,
                fill,
                stroke,
                stroke_width,
                text,
                font_size,
                color,
                bold,
                format,
                ..
            } => {
                let path = shape_path(preset, width, height, visual)?;
                if !matches!(
                    preset.as_str(),
                    "rect" | "ellipse" | "flowChartProcess" | "flowChartConnector"
                ) {
                    self.warning(
                        id,
                        "SHAPE_APPROXIMATION",
                        format!("{preset}: normalized preset preview geometry, not Office parity"),
                    );
                }
                self.path(&path, fill, stroke, *stroke_width, visual)?;
                for extra in shape_details(preset, width, height) {
                    self.path(
                        &extra,
                        "none",
                        stroke,
                        *stroke_width,
                        &VisualStyle::default(),
                    )?;
                }
                self.shaped_text(
                    id,
                    text,
                    [6.0, 4.0, (width - 12.0).max(1.0), (height - 8.0).max(1.0)],
                    *font_size,
                    color,
                    *bold,
                    format,
                    visual.text_warp,
                )?;
            }
            Element::Text {
                text,
                font_size,
                color,
                bold,
                format,
                ..
            } => self.shaped_text(
                id,
                text,
                [0.0, 0.0, width, height],
                *font_size,
                color,
                *bold,
                format,
                visual.text_warp,
            )?,
            Element::Table {
                rows,
                font_size,
                format,
                ..
            } => {
                let columns = crate::table_format::tracks(
                    format.column_widths.as_ref(),
                    rows[0].len(),
                    width,
                )?;
                let heights =
                    crate::table_format::tracks(format.row_heights.as_ref(), rows.len(), height)?;
                let mut top = 0.0;
                for (row_index, row) in rows.iter().enumerate() {
                    let mut left = 0.0;
                    for (column_index, text) in row.iter().enumerate() {
                        let merge = format.merge_at(row_index, column_index);
                        if merge.is_some_and(|region| {
                            (region.row, region.column) != (row_index, column_index)
                        }) {
                            left += columns[column_index];
                            continue;
                        }
                        let cell_width = columns[column_index
                            ..column_index + merge.map_or(1, |region| region.col_span)]
                            .iter()
                            .sum::<f64>();
                        let cell_height = heights
                            [row_index..row_index + merge.map_or(1, |region| region.row_span)]
                            .iter()
                            .sum::<f64>();
                        let style = format.cell_style(row_index, column_index);
                        let outline = style.outline.as_ref();
                        self.rectangle(
                            [left, top, cell_width, cell_height],
                            &self.color(
                                style
                                    .fill
                                    .as_deref()
                                    .unwrap_or(crate::table_format::default_fill(row_index)),
                            ),
                            Some((
                                &self.color(outline.map_or("@lt1", |value| value.color.as_str())),
                                outline.map_or(1.0, |value| value.width),
                            )),
                        );
                        let padding =
                            style
                                .padding
                                .clone()
                                .unwrap_or(crate::table_format::CellPadding {
                                    left: 6.0,
                                    right: 6.0,
                                    top: 6.0,
                                    bottom: 6.0,
                                });
                        let mut text_format = style.text(text);
                        text_format.vertical = style.vertical.unwrap_or(VerticalAlign::Top);
                        let text_width = cell_width - padding.left - padding.right;
                        let text_height = cell_height - padding.top - padding.bottom;
                        if !text.is_empty() && (text_width <= 0.0 || text_height <= 0.0) {
                            return Err(Error::Invalid(format!(
                                "static table {id} has no padded text area"
                            )));
                        }
                        self.text(
                            &format!("{id}[{row_index},{column_index}]"),
                            text,
                            [
                                left + padding.left,
                                top + padding.top,
                                text_width.max(1.0),
                                text_height.max(1.0),
                            ],
                            *font_size,
                            crate::table_format::default_color(row_index),
                            row_index == 0,
                            &text_format,
                        )?;
                        left += columns[column_index];
                    }
                    top += heights[row_index];
                }
            }
            Element::Picture {
                base64,
                mime_type,
                crop,
                svg,
                ..
            } => {
                let prepared = svg
                    .as_ref()
                    .map(|svg| crate::vector::prepare_svg(svg))
                    .transpose()?;
                if prepared.is_some() {
                    self.warning(
                        id,
                        "SVG_RASTERIZED",
                        "Sanitized SVG image projected through the core's 1024px PNG renderer",
                    );
                }
                self.clip(width, height);
                if let Some(mask) = visual.picture_mask {
                    let mask_id = self.identity();
                    self.writer.start_element("defs");
                    self.writer.start_element("clipPath");
                    self.writer.write_attribute("id", &mask_id);
                    let path = shape_path(mask.preset(), width, height, &VisualStyle::default())?;
                    self.writer.start_element("path");
                    self.writer.write_attribute("d", &path);
                    self.writer.end_element();
                    self.writer.end_element();
                    self.writer.end_element();
                    self.writer.start_element("g");
                    self.writer
                        .write_attribute("clip-path", &format!("url(#{mask_id})"));
                }
                let visible_width = 1.0 - crop.left - crop.right;
                let visible_height = 1.0 - crop.top - crop.bottom;
                if visible_width < 0.0001 || visible_height < 0.0001 {
                    return Err(Error::Limit(
                        "static crop magnification exceeds 10000".into(),
                    ));
                }
                self.writer.start_element("image");
                self.writer
                    .write_attribute("x", &(-crop.left * width / visible_width));
                self.writer
                    .write_attribute("y", &(-crop.top * height / visible_height));
                self.writer
                    .write_attribute("width", &(width / visible_width));
                self.writer
                    .write_attribute("height", &(height / visible_height));
                self.writer.write_attribute("preserveAspectRatio", "none");
                self.writer
                    .write_attribute("opacity", &visual.opacity.unwrap_or(1.0));
                self.writer.write_attribute(
                    "xlink:href",
                    &format!(
                        "data:{};base64,{}",
                        if prepared.is_some() {
                            "image/png"
                        } else {
                            mime_type
                        },
                        prepared.as_deref().unwrap_or(base64)
                    ),
                );
                self.writer.end_element();
                if visual.picture_mask.is_some() {
                    self.writer.end_element();
                }
                self.writer.end_element();
            }
            Element::Connector {
                color,
                stroke_width,
                arrow,
                flip_v,
                routing,
                ..
            } => {
                let points = routing
                    .as_ref()
                    .map(|route| route.points.clone())
                    .unwrap_or_else(|| {
                        vec![
                            [0.0, if *flip_v { 1.0 } else { 0.0 }],
                            [1.0, if *flip_v { 0.0 } else { 1.0 }],
                        ]
                    });
                let mut path = String::new();
                for (index, point) in points.iter().enumerate() {
                    let _ = write!(
                        path,
                        "{}{} {} ",
                        if index == 0 { "M" } else { "L" },
                        point[0] * width,
                        point[1] * height
                    );
                }
                self.writer.start_element("path");
                self.writer.write_attribute("d", &path);
                self.writer.write_attribute("fill", "none");
                self.writer.write_attribute("stroke", &self.color(color));
                self.writer.write_attribute("stroke-width", stroke_width);
                if routing.as_ref().is_some_and(|route| route.dashed) {
                    self.writer.write_attribute(
                        "stroke-dasharray",
                        &format!("{} {}", stroke_width * 4.0, stroke_width * 3.0),
                    );
                }
                self.writer.end_element();
                if *arrow {
                    self.arrow(
                        points[points.len() - 2],
                        points[points.len() - 1],
                        width,
                        height,
                        *stroke_width,
                        color,
                    )?;
                }
                if routing.as_ref().is_some_and(|route| route.start_arrow) {
                    self.arrow(points[1], points[0], width, height, *stroke_width, color)?;
                }
            }
            Element::Group {
                view_width,
                view_height,
                children,
                ..
            } => {
                self.writer.start_element("g");
                self.writer.write_attribute(
                    "transform",
                    &format!("scale({} {})", width / view_width, height / view_height),
                );
                for child in children {
                    self.element(child)?;
                }
                self.writer.end_element();
            }
            Element::Chart {
                kind,
                categories,
                series,
                options,
                ..
            } => {
                let svg = chart_svg(
                    *kind, categories, series, options, width, height, self.theme,
                )?;
                if chart_projection::required(series, options) && !kind.is_polar() && !kind.is_extended() && !matches!(kind, crate::model::ChartKind::Radar | crate::model::ChartKind::RadarFilled) {
                    for warning in crate::model::chart_format::compute_chart_presentation(*kind, categories, series, options)?.warnings { self.warning(id, "CHART_PRESENTATION", warning); }
                    if options.legend.is_some_and(|legend| !matches!(legend, crate::model::chart_format::LegendPosition::Bottom | crate::model::chart_format::LegendPosition::Hidden)) { self.warning(id, "CHART_LEGEND_LAYOUT", "Statistical chart preview uses a bottom legend; native legend placement is retained"); }
                    if options.data_labels.as_ref().and_then(|labels| labels.position).is_some() { self.warning(id, "CHART_LABEL_LAYOUT", "Statistical chart preview places labels above points; native placement is retained"); }
                }
                self.warning(
                    id,
                    "CHART_PREVIEW",
                    "Plotters chart layout; axis/legend placement is not Office parity",
                );
                if kind.is_3d() {
                    self.warning(id, "CHART_3D_PROJECTED", "Native 3D chart rendered as an explicit 2D data projection; depth and perspective are omitted");
                }
                if matches!(kind, crate::model::ChartKind::Treemap | crate::model::ChartKind::Sunburst) {
                    self.warning(id, "HIERARCHY_LAYOUT", "Stable proportional slice-and-dice treemap / angular sunburst; parent labels overlay data regions, not Office layout");
                }
                self.copy_chart(roxmltree::Document::parse(&svg)?.root_element(), id)?;
            }
        }
        self.writer.end_element();
        if let Some(reflection) = &visual.reflection { self.reflection(&effect_id, reflection, width, height); }
        self.writer.end_element();
        Ok(())
    }
    fn path(
        &mut self,
        path: &str,
        fill: &str,
        stroke: &str,
        stroke_width: f64,
        visual: &VisualStyle,
    ) -> Result<()> {
        self.path_bytes += path.len() + 256;
        if self.path_bytes > MAX_SVG_BYTES / 2 {
            return Err(Error::Limit("static path budget".into()));
        }
        let paint = self.fill(fill, visual);
        self.writer.start_element("path");
        self.writer.write_attribute("d", path);
        self.writer.write_attribute("fill", &paint);
        self.writer
            .write_attribute("fill-opacity", &visual.opacity.unwrap_or(1.0));
        self.writer.write_attribute("stroke", &self.color(stroke));
        self.writer.write_attribute("stroke-width", &stroke_width);
        self.writer.write_attribute("stroke-linejoin", "round");
        self.writer.end_element();
        Ok(())
    }
    fn arrow(
        &mut self,
        from: [f64; 2],
        to: [f64; 2],
        width: f64,
        height: f64,
        stroke: f64,
        color: &str,
    ) -> Result<()> {
        let angle = ((to[1] - from[1]) * height).atan2((to[0] - from[0]) * width);
        let length = stroke * 4.0;
        let end = [to[0] * width, to[1] * height];
        let path = format!(
            "M{} {} L{} {} L{} {} Z",
            end[0],
            end[1],
            end[0] - length * angle.cos() + length * 0.4 * angle.sin(),
            end[1] - length * angle.sin() - length * 0.4 * angle.cos(),
            end[0] - length * angle.cos() - length * 0.4 * angle.sin(),
            end[1] - length * angle.sin() + length * 0.4 * angle.cos()
        );
        self.path(&path, color, color, 0.0, &VisualStyle::default())
    }
    fn family(&self, text: &str, requested: Option<&str>) -> String {
        match requested {
            Some(name) if !name.starts_with('@')=>name.into(),
            _ if text.chars().any(|character|matches!(character,'\u{3000}'..='\u{9fff}'|'\u{ac00}'..='\u{d7ff}'|'\u{f900}'..='\u{faff}'|'\u{20000}'..='\u{3134f}'))=>self.theme.fonts.east_asian.clone(),
            _ if text.chars().any(|character|matches!(character,'\u{0590}'..='\u{08ff}'|'\u{0900}'..='\u{0dff}'))=>self.theme.fonts.complex_script.clone(),
            Some("@major")=>self.theme.fonts.major.clone(),_=>self.theme.fonts.minor.clone(),
        }
    }
    fn text(
        &mut self,
        id: &str,
        text: &str,
        bounds: [f64; 4],
        size: f64,
        color: &str,
        bold: bool,
        format: &TextFormat,
    ) -> Result<()> {
        self.shaped_text(id, text, bounds, size, color, bold, format, None)
    }
    fn shaped_text(
        &mut self,
        id: &str,
        text: &str,
        bounds: [f64; 4],
        size: f64,
        color: &str,
        bold: bool,
        format: &TextFormat,
        warp: Option<crate::visual::TextWarp>,
    ) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        if self.fonts.db().faces().next().is_none() {
            return Err(Error::Unsupported(
                "installed fonts required for static text".into(),
            ));
        }
        let paragraphs = if format.paragraphs.is_empty() {
            text.split('\n')
                .map(|line| RichParagraph {
                    runs: vec![RichRun {
                        text: line.into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .collect()
        } else {
            format.paragraphs.clone()
        };
        let mut glyphs = Vec::new();
        let mut top = 0.0;
        let mut used = BTreeSet::new();
        let mut requested = BTreeSet::new();
        let mut overflow = false;
        for (paragraph_index, paragraph) in paragraphs.iter().enumerate() {
            if !paragraph.tabs.is_empty()
                || paragraph.runs.iter().any(|run| run.text.contains('\t'))
            {
                return Err(Error::Unsupported(format!(
                    "static text {id}: custom/default tab stops"
                )));
            }
            if paragraph.numbering.as_deref().is_some_and(|value| {
                !matches!(
                    value,
                    "arabicPeriod" | "arabicParenR" | "arabicParenBoth" | "arabicPlain"
                )
            }) {
                return Err(Error::Unsupported(format!(
                    "static text {id}: non-Arabic numbering"
                )));
            }
            if paragraph.runs.iter().any(|run| run.field.is_some()) {
                self.warning(id,"FIELD_CACHED_TEXT","Dynamic fields render their stored display text; export does not evaluate date/time fields");
            }
            let mut runs = paragraph.runs.clone();
            let bullet = paragraph.bullet.unwrap_or(format.bullet);
            if bullet != Bullet::None {
                let marker = if bullet == Bullet::Bullet {
                    paragraph
                        .bullet_character
                        .clone()
                        .unwrap_or_else(|| "\u{2022}".into())
                } else {
                    let number = paragraph.number_start.unwrap_or(1) + paragraph_index as u32;
                    match paragraph.numbering.as_deref() {
                        Some("arabicParenR") => format!("{number})"),
                        Some("arabicParenBoth") => format!("({number})"),
                        Some("arabicPlain") => number.to_string(),
                        _ => format!("{number}."),
                    }
                };
                runs.insert(
                    0,
                    RichRun {
                        text: format!("{marker} "),
                        ..Default::default()
                    },
                );
                self.warning(
                    id,
                    "BULLET_LAYOUT_APPROXIMATION",
                    "Bullet uses shaped inline prefix; hanging indent differs from Office",
                );
            }
            let paragraph_text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
            let styles: Vec<_> = runs
                .iter()
                .map(|run| {
                    let mut style = RunStyle::frame(size, color, bold, format);
                    style.overlay(&run.style);
                    style
                })
                .collect();
            let families: Vec<_> = runs
                .iter()
                .zip(&styles)
                .map(|(run, style)| self.family(&run.text, style.font_family.as_deref()))
                .collect();
            requested.extend(families.iter().cloned());
            let maximum = styles
                .iter()
                .filter_map(|style| style.font_size)
                .fold(size, f64::max);
            let line_height = spacing(
                paragraph.line_spacing.unwrap_or(Spacing::Percent(115000)),
                maximum,
            )
            .max(maximum);
            let margin = paragraph
                .margin_left
                .map_or(0.0, |value| value as f64 / 9525.0);
            let indent = paragraph.indent.unwrap_or(0) as f64 / 9525.0;
            if indent != 0.0 || paragraph.level.is_some() {
                self.warning(
                    id,
                    "INDENT_APPROXIMATION",
                    "Paragraph indent applies to all wrapped lines; hierarchy is not auto-indented",
                );
            }
            let inset = (margin + indent).max(0.0);
            let available = bounds[2] - inset;
            if available <= 0.0 {
                return Err(Error::Invalid(format!(
                    "static text {id} has no width after indentation"
                )));
            }
            top += paragraph
                .space_before
                .map_or(0.0, |value| spacing(value, maximum));
            let mut buffer =
                Buffer::new(self.fonts, Metrics::new(maximum as f32, line_height as f32));
            buffer.set_size(Some(available as f32), None);
            buffer.set_wrap(Wrap::WordOrGlyph);
            let attrs = Attrs::new();
            buffer.set_rich_text(
                runs.iter().zip(&styles).zip(&families).enumerate().map(
                    |(index, ((run, style), family))| {
                        (
                            run.text.as_str(),
                            Attrs::new()
                                .family(Family::Name(family))
                                .weight(if style.bold.unwrap_or(bold) {
                                    Weight::BOLD
                                } else {
                                    Weight::NORMAL
                                })
                                .style(if style.italic.unwrap_or(false) {
                                    Style::Italic
                                } else {
                                    Style::Normal
                                })
                                .metrics(Metrics::new(
                                    style.font_size.unwrap_or(size) as f32,
                                    line_height as f32,
                                ))
                                .metadata(index),
                        )
                    },
                ),
                &attrs,
                Shaping::Advanced,
                None,
            );
            let align = match paragraph.alignment.unwrap_or(format.alignment) {
                TextAlign::Left => Align::Left,
                TextAlign::Center => Align::Center,
                TextAlign::Right => Align::Right,
                TextAlign::Justify => Align::Justified,
            };
            for line in &mut buffer.lines {
                line.set_align(Some(align));
            }
            buffer.shape_until_scroll(self.fonts, false);
            let mut height = line_height;
            for run in buffer.layout_runs() {
                height = height.max((run.line_top + run.line_height) as f64);
                overflow |= run.line_w as f64 > available + 1.0;
                for glyph in run.glyphs {
                    if glyphs.len() >= 200_000 {
                        return Err(Error::Limit("static glyph count > 200000".into()));
                    }
                    let fallback = self.fonts.db().face(glyph.font_id).and_then(|face| {
                        for (name, _) in &face.families {
                            used.insert(name.clone());
                        }
                        let requested = &families[glyph.metadata];
                        if face
                            .families
                            .iter()
                            .any(|(name, _)| name.eq_ignore_ascii_case(requested))
                        {
                            None
                        } else {
                            Some(format!(
                                "Requested {requested}; glyphs use {}",
                                face.families
                                    .first()
                                    .map_or("unknown", |(name, _)| name.as_str())
                            ))
                        }
                    });
                    if let Some(message) = fallback {
                        self.warning(id, "FONT_FALLBACK", message);
                    }
                    let whitespace = paragraph_text
                        .get(glyph.start..glyph.end)
                        .unwrap_or("")
                        .chars()
                        .all(char::is_whitespace);
                    glyphs.push((
                        glyph.clone(),
                        top + run.line_y as f64,
                        inset,
                        styles[glyph.metadata].clone(),
                        whitespace,
                        paragraph_text.get(glyph.start..glyph.end).unwrap_or("").to_owned(),
                        (paragraph_index, run.line_i, glyph.start),
                    ));
                }
            }
            top += height
                + paragraph
                    .space_after
                    .map_or(0.0, |value| spacing(value, maximum));
        }
        overflow |= top > bounds[3] + 1.0;
        if overflow {
            self.warning(id,"TEXT_OVERFLOW","Shaped text exceeds its frame and is clipped; shorten content or enlarge the frame");
        }
        for family in requested {
            if !used.iter().any(|name| name.eq_ignore_ascii_case(&family)) {
                self.warning(
                    id,
                    "FONT_FALLBACK",
                    format!(
                        "Requested {family}; used {}",
                        used.iter().cloned().collect::<Vec<_>>().join(", ")
                    ),
                );
            }
        }
        let offset = match format.vertical {
            VerticalAlign::Top => 0.0,
            VerticalAlign::Middle => (bounds[3] - top).max(0.0) / 2.0,
            VerticalAlign::Bottom => (bounds[3] - top).max(0.0),
        };
        self.writer.start_element("g");
        let text_id = self.identity();
        self.writer.write_attribute("id", &text_id);
        self.writer.write_attribute(
            "transform",
            &format!("translate({} {})", bounds[0], bounds[1]),
        );
        self.clip(bounds[2], bounds[3]);
        self.writer.start_element("title");
        self.writer.write_text(&escape_text(text));
        self.writer.end_element();
        let mut semantic = Vec::new();
        for (glyph, baseline, inset, style, whitespace, cluster, logical) in glyphs {
            let shift = style.baseline.unwrap_or(0) as f64 * glyph.font_size as f64 / 100000.0;
            let origin = [
                inset + glyph.x as f64 + (glyph.x_offset * glyph.font_size) as f64,
                offset + baseline + glyph.y as f64
                    - (glyph.y_offset * glyph.font_size) as f64
                    - shift,
            ];
            if origin[0] >= 0.0 && origin[0] < bounds[2] && origin[1] >= 0.0 && origin[1] <= bounds[3] {
                semantic.push((logical, RenderedGlyph {
                    text: cluster,
                    origin,
                    size: glyph.font_size as f64,
                    width: glyph.w as f64,
                }));
            }
            if let Some(highlight) = &style.highlight {
                if highlight != "none" {
                    self.rectangle(
                        [
                            origin[0],
                            origin[1] - glyph.font_size as f64,
                            glyph.w as f64,
                            glyph.font_size as f64 * 1.15,
                        ],
                        &self.color(highlight),
                        None,
                    );
                }
            }
            let outlined = self
                .fonts
                .db()
                .with_face_data(glyph.font_id, |data, index| {
                    let face = ttf_parser::Face::parse(data, index).ok()?;
                    let units = face.units_per_em() as f64;
                    let mut builder = Outline {
                        path: String::new(),
                        scale: glyph.font_size as f64 / units,
                        origin,
                        overflow: false,
                    };
                    let bounds =
                        face.outline_glyph(ttf_parser::GlyphId(glyph.glyph_id), &mut builder);
                    Some((builder, bounds.is_some()))
                })
                .flatten();
            let color = style.color.as_deref().unwrap_or("@dk1");
            match outlined {
                Some((outline, _)) if outline.overflow => {
                    return Err(Error::Limit("static glyph outline > 128 KiB".into()))
                }
                Some((outline, true)) if glyph.glyph_id != 0 => {
                    let path = if let Some(warp) = warp { warp_outline(&outline.path, bounds[2], bounds[3], warp)? } else { outline.path };
                    self.path(&path, color, "none", 0.0, &VisualStyle::default())?
                }
                _ if glyph.glyph_id == 0 => {
                    self.warning(
                        id,
                        "MISSING_GLYPHS",
                        "Unavailable glyph rendered as an outlined replacement box",
                    );
                    self.rectangle(
                        [
                            origin[0],
                            origin[1] - glyph.font_size as f64 * 0.8,
                            (glyph.w as f64).max(4.0),
                            glyph.font_size as f64,
                        ],
                        "none",
                        Some((&self.color(color), 1.0)),
                    );
                }
                _ => {
                    if !whitespace {
                        self.warning(
                            id,
                            "GLYPH_OUTLINE_UNAVAILABLE",
                            "Font has a non-outline glyph; rendered as an outlined replacement box",
                        );
                        self.rectangle(
                            [
                                origin[0],
                                origin[1] - glyph.font_size as f64 * 0.8,
                                (glyph.w as f64).max(4.0),
                                glyph.font_size as f64,
                            ],
                            "none",
                            Some((&self.color(color), 1.0)),
                        );
                    }
                }
            }
            if style.underline.unwrap_or(false) {
                self.rectangle(
                    [
                        origin[0],
                        origin[1] + glyph.font_size as f64 * 0.08,
                        glyph.w as f64,
                        (glyph.font_size as f64 / 16.0).max(1.0),
                    ],
                    &self.color(color),
                    None,
                );
            }
        }
        self.writer.end_element();
        self.writer.end_element();
        semantic.sort_by_key(|(logical, _)| *logical);
        semantic.dedup_by_key(|(logical, _)| *logical);
        self.text.push(RenderedText {
            element_id: id.into(),
            svg_id: text_id,
            glyphs: semantic.into_iter().map(|(_, glyph)| glyph).collect(),
        });
        Ok(())
    }
    fn copy_chart(&mut self, node: roxmltree::Node<'_, '_>, id: &str) -> Result<()> {
        if node.is_text() {
            self.writer
                .write_text(&escape_text(node.text().unwrap_or("")));
            return Ok(());
        }
        if !node.is_element() {
            return Ok(());
        }
        if node.tag_name().name() == "text" {
            let text = node.text().unwrap_or("").trim_matches('\n');
            if text.is_empty() {
                return Ok(());
            }
            let number = |name: &str, default: f64| {
                node.attribute(name)
                    .and_then(|value| value.parse::<f64>().ok())
                    .unwrap_or(default)
            };
            let size = number("font-size", 12.0);
            let horizontal = number("x", 0.0);
            let vertical = number("y", 0.0);
            let family = self.family(text, None);
            let mut buffer = Buffer::new(self.fonts, Metrics::new(size as f32, size as f32 * 1.15));
            buffer.set_size(None, None);
            buffer.set_text(
                text,
                &Attrs::new().family(Family::Name(&family)),
                Shaping::Advanced,
                None,
            );
            buffer.shape_until_scroll(self.fonts, false);
            let extent = buffer
                .layout_runs()
                .map(|run| run.line_w as f64)
                .fold(1.0, f64::max)
                + 1.0;
            let left = horizontal
                - match node.attribute("text-anchor") {
                    Some("end") => extent,
                    Some("middle") => extent / 2.0,
                    _ => 0.0,
                };
            let top = vertical
                - match node.attribute("dy") {
                    Some("0.5ex") => size * 0.5,
                    Some("-0.5ex") => size,
                    _ => 0.0,
                };
            self.writer.start_element("g");
            if let Some(transform) = node.attribute("transform") {
                self.writer.write_attribute("transform", transform);
            }
            self.writer
                .write_attribute("opacity", &number("opacity", 1.0));
            self.text(
                id,
                text,
                [left, top, extent, size * 1.5],
                size,
                node.attribute("fill")
                    .unwrap_or("#000000")
                    .trim_start_matches('#'),
                node.attribute("font-weight") == Some("bold"),
                &TextFormat {
                    font_family: Some(family),
                    ..Default::default()
                },
            )?;
            self.writer.end_element();
            return Ok(());
        }
        self.writer.start_element(node.tag_name().name());
        for attribute in node.attributes() {
            if attribute.name() != "font-family" {
                self.writer
                    .write_attribute(attribute.name(), attribute.value());
            }
        }
        for child in node.children() {
            self.copy_chart(child, id)?;
        }
        self.writer.end_element();
        Ok(())
    }
}

fn effect_bounds(element: &Element) -> [f64; 4] {
    let (_, _, _, width, height) = element.bounds();
    let mut bounds = [0.0_f64, 0.0_f64, width, height];
    if let Element::Group { view_width, view_height, children, .. } = element {
        for child in children {
            if child.visual().is_some_and(|style| style.hidden) { continue; }
            let (_, left, top, child_width, child_height) = child.bounds();
            let [child_left, child_top, extent_width, extent_height] = effect_bounds(child);
            let style = child.visual().cloned().unwrap_or_default();
            let rotation = match child { Element::Shape { rotation, .. } => *rotation, _ => style.rotation.unwrap_or(0.0) }.to_radians();
            for (point_x, point_y) in [(child_left, child_top), (child_left + extent_width, child_top), (child_left, child_top + extent_height), (child_left + extent_width, child_top + extent_height)] {
                let relative_x = (point_x - child_width / 2.0) * if style.flip_h { -1.0 } else { 1.0 };
                let relative_y = (point_y - child_height / 2.0) * if style.flip_v { -1.0 } else { 1.0 };
                let point_x = (left + child_width / 2.0 + relative_x * rotation.cos() - relative_y * rotation.sin()) * width / view_width;
                let point_y = (top + child_height / 2.0 + relative_x * rotation.sin() + relative_y * rotation.cos()) * height / view_height;
                bounds[0] = bounds[0].min(point_x);
                bounds[1] = bounds[1].min(point_y);
                bounds[2] = bounds[2].max(point_x);
                bounds[3] = bounds[3].max(point_y);
            }
        }
    }
    let stroke = match element { Element::Shape { stroke_width, .. } | Element::Polygon { stroke_width, .. } | Element::Connector { stroke_width, .. } => *stroke_width * 4.0, _ => 0.0 };
    let mut padding = stroke + 2.0;
    if let Some(style) = element.visual() {
        padding += style.soft_edge.unwrap_or(0.0) * 2.0;
        padding += style.glow.as_ref().map_or(0.0, |glow| glow.radius * 2.0);
        padding += style.shadow.as_ref().map_or(0.0, |shadow| shadow.blur * 2.0 + shadow.distance);
        if let Some(reflection) = &style.reflection { bounds[3] += height + reflection.distance + reflection.blur * 2.0; }
    }
    [bounds[0] - padding, bounds[1] - padding, bounds[2] - bounds[0] + padding * 2.0, bounds[3] - bounds[1] + padding * 2.0]
}

fn filter_budget(element: &Element, scale_x: f64, scale_y: f64, copies: usize, bytes: &mut f64, expanded_nodes: &mut usize) -> Result<()> {
    if element.visual().is_some_and(|style| style.hidden) { return Ok(()); }
    *expanded_nodes = expanded_nodes.saturating_add(copies);
    if *expanded_nodes > 8192 { return Err(Error::Limit("static reflection expansion exceeds 8192 objects".into())); }
    let style = element.visual().cloned().unwrap_or_default();
    let scale = scale_x.abs().max(scale_y.abs());
    if style.shadow.is_some() || style.glow.is_some() || style.soft_edge.is_some() || style.reflection.is_some() {
        let bounds = effect_bounds(element);
        let rotation = match element { Element::Shape { rotation, .. } => *rotation, _ => style.rotation.unwrap_or(0.0) }.to_radians();
        let extent_width = (bounds[2] * rotation.cos().abs() + bounds[3] * rotation.sin().abs()) * scale;
        let extent_height = (bounds[2] * rotation.sin().abs() + bounds[3] * rotation.cos().abs()) * scale;
        *bytes += extent_width.ceil() * extent_height.ceil() * 4.0 * 8.0 * copies as f64;
        if !bytes.is_finite() || extent_width > 8192.0 || extent_height > 8192.0 || *bytes > 32.0 * 1024.0 * 1024.0 {
            return Err(Error::Limit("static filter working budget exceeds 32 MiB or 8192px".into()));
        }
    }
    if let Element::Group { width, height, view_width, view_height, children, .. } = element {
        for child in children { filter_budget(child, scale * width / view_width, scale * height / view_height, copies * if style.reflection.is_some() { 2 } else { 1 }, bytes, expanded_nodes)?; }
    }
    Ok(())
}

fn escape_text(text: &str) -> String {
    text.replace('&', "&amp;").replace('>', "&gt;")
}
fn spacing(value: Spacing, size: f64) -> f64 {
    match value {
        Spacing::Percent(value) => size * value as f64 / 100000.0,
        Spacing::Points(value) => value as f64 / 75.0,
    }
}
struct Outline {
    path: String,
    scale: f64,
    origin: [f64; 2],
    overflow: bool,
}
impl Outline {
    fn push(&mut self, command: &str, points: &[(f32, f32)]) {
        if self.path.len() > 128 * 1024 {
            self.overflow = true;
            return;
        }
        self.path.push_str(command);
        for (horizontal, vertical) in points {
            let _ = write!(
                self.path,
                "{:.3} {:.3} ",
                self.origin[0] + *horizontal as f64 * self.scale,
                self.origin[1] - *vertical as f64 * self.scale
            );
        }
    }
}
impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, horizontal: f32, vertical: f32) {
        self.push("M", &[(horizontal, vertical)]);
    }
    fn line_to(&mut self, horizontal: f32, vertical: f32) {
        self.push("L", &[(horizontal, vertical)]);
    }
    fn quad_to(&mut self, control_x: f32, control_y: f32, horizontal: f32, vertical: f32) {
        self.push("Q", &[(control_x, control_y), (horizontal, vertical)]);
    }
    fn curve_to(
        &mut self,
        first_x: f32,
        first_y: f32,
        second_x: f32,
        second_y: f32,
        horizontal: f32,
        vertical: f32,
    ) {
        self.push(
            "C",
            &[
                (first_x, first_y),
                (second_x, second_y),
                (horizontal, vertical),
            ],
        );
    }
    fn close(&mut self) {
        self.push("Z", &[]);
    }
}
fn vector_path(path: &crate::vector::VectorPath, width: f64, height: f64) -> String {
    use crate::vector::PathCommand;
    let mut result = String::new();
    for command in &path.commands {
        let (name, points) = match command {
            PathCommand::Move { point } => ("M", vec![point]),
            PathCommand::Line { point } => ("L", vec![point]),
            PathCommand::Quadratic { control, point } => ("Q", vec![control, point]),
            PathCommand::Cubic {
                control1,
                control2,
                point,
            } => ("C", vec![control1, control2, point]),
            PathCommand::Close => ("Z", vec![]),
        };
        result.push_str(name);
        for point in points {
            let _ = write!(result, "{} {} ", point[0] * width, point[1] * height);
        }
    }
    result
}

fn shape_path(preset: &str, width: f64, height: f64, visual: &VisualStyle) -> Result<String> {
    if !visual.connection_sites.is_empty() && preset == "roundRect" {
        let radius = f64::from(visual.adjustments.first().map_or(16667, |adjustment| adjustment.value)) / 100000.0 * width.min(height);
        let right = width - radius; let bottom = height - radius;
        return Ok(format!("M{radius} 0H{right}A{radius} {radius} 0 0 1 {width} {radius}V{bottom}A{radius} {radius} 0 0 1 {right} {height}H{radius}A{radius} {radius} 0 0 1 0 {bottom}V{radius}A{radius} {radius} 0 0 1 {radius} 0Z"));
    }
    let normalized=match preset {
        "rect"|"flowChartProcess"|"flowChartPredefinedProcess"|"flowChartInternalStorage"=>"M0 0H100V100H0Z",
        "roundRect"=>"M17 0H83Q100 0 100 17V83Q100 100 83 100H17Q0 100 0 83V17Q0 0 17 0Z",
        "ellipse"|"flowChartConnector"=>"M0 50A50 50 0 1 0 100 50A50 50 0 1 0 0 50Z",
        "triangle"=>"M50 0L100 100H0Z","rtTriangle"=>"M0 0L100 100H0Z","diamond"|"flowChartDecision"=>"M50 0L100 50 50 100 0 50Z",
        "can"=>"M0 15A50 15 0 0 1 100 15V85A50 15 0 0 1 0 85Z",
        "cloud"=>"M20 78C1 78 0 48 18 42C9 20 36 11 46 25C54 5 86 12 84 35C101 31 108 63 92 72C99 92 72 102 61 86C45 101 17 97 20 78Z",
        "parallelogram"=>"M25 0H100L75 100H0Z","trapezoid"=>"M25 0H75L100 100H0Z",
        "plus"=>"M33 0H67V33H100V67H67V100H33V67H0V33H33Z",
        "heart"=>"M50 100C40 85 0 60 0 28C0 -5 36 -10 50 18C64 -10 100 -5 100 28C100 60 60 85 50 100Z",
        "chevron"=>"M0 0H65L100 50 65 100H0L35 50Z","rightArrow"=>"M0 25H60V0L100 50 60 100V75H0Z",
        "leftArrow"=>"M100 25H40V0L0 50 40 100V75H100Z","upArrow"=>"M25 100V40H0L50 0 100 40H75V100Z",
        "downArrow"=>"M25 0V60H0L50 100 100 60H75V0Z","leftRightArrow"=>"M0 50L25 0V25H75V0L100 50 75 100V75H25V100Z",
        "upDownArrow"=>"M50 0L100 25H75V75H100L50 100 0 75H25V25H0Z","homePlate"=>"M0 0H65L100 50 65 100H0Z",
        "flowChartTerminator"=>"M25 0H75A25 50 0 0 1 75 100H25A25 50 0 0 1 25 0Z",
        "flowChartInputOutput"=>"M20 0H100L80 100H0Z","flowChartDocument"=>"M0 0H100V85C60 65 40 110 0 90Z",
        "flowChartOfflineStorage"=>"M0 0H100L50 100Z","wedgeRectCallout"=>"M0 0H100V80H40L10 100 23 80H0Z",
        "wedgeRoundRectCallout"=>"M15 0H85Q100 0 100 15V65Q100 80 85 80H40L10 100 23 80H15Q0 80 0 65V15Q0 0 15 0Z",
        "wedgeEllipseCallout"=>"M20 76C-20 52 0 0 50 0C115 0 120 76 50 80L10 100Z",
        _=>"",
    };
    let mut path = normalized.to_owned();
    let sides = match preset {
        "pentagon" => Some(5),
        "hexagon" => Some(6),
        "heptagon" => Some(7),
        "octagon" => Some(8),
        "decagon" => Some(10),
        _ => preset
            .strip_prefix("star")
            .and_then(|count| count.parse::<usize>().ok()),
    };
    if let Some(sides) = sides {
        let star = preset.starts_with("star");
        let count = if star { sides * 2 } else { sides };
        path.clear();
        for index in 0..count {
            let angle =
                -std::f64::consts::FRAC_PI_2 + index as f64 * std::f64::consts::TAU / count as f64;
            let radius = if star && index % 2 == 1 {
                if sides == 5 {
                    19.1
                } else {
                    25.0
                }
            } else {
                50.0
            };
            let _ = write!(
                path,
                "{}{} {} ",
                if index == 0 { "M" } else { "L" },
                50.0 + radius * angle.cos(),
                50.0 + radius * angle.sin()
            );
        }
        path.push('Z');
    }
    if let Some(adjustment) = visual.adjustments.first() {
        let value = adjustment.value as f64;
        match preset {
            "triangle" => path = format!("M{} 0L100 100H0Z", value / 1000.0),
            "chevron" => {
                let inset = (value / 100000.0 * width.min(height) / width * 100.0).min(100.0);
                path = format!(
                    "M0 0H{}L100 50 {} 100H0L{inset} 50Z",
                    100.0 - inset,
                    100.0 - inset
                );
            }
            "roundRect" => {
                let horizontal = value / 100000.0 * width.min(height) / width * 100.0;
                let vertical = value / 100000.0 * width.min(height) / height * 100.0;
                path=format!("M{horizontal} 0H{}Q100 0 100 {vertical}V{}Q100 100 {} 100H{horizontal}Q0 100 0 {}V{vertical}Q0 0 {horizontal} 0Z",100.0-horizontal,100.0-vertical,100.0-horizontal,100.0-vertical);
            }
            _ => return Err(Error::Unsupported("static shape adjustment".into())),
        }
    }
    if path.is_empty() {
        return Err(Error::Unsupported(format!("static shape preset {preset}")));
    }
    scale_shape_path(&path, width, height)
}

fn scale_shape_path(path: &str, width: f64, height: f64) -> Result<String> {
    use resvg::tiny_skia::PathSegment;
    let svg=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"100\"><path d=\"{path}\"/></svg>");
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())
        .map_err(|error| Error::Invalid(format!("static preset path: {error}")))?;
    let Some(resvg::usvg::Node::Path(path)) = tree.root().children().first() else {
        return Err(Error::Invalid("static preset has no path".into()));
    };
    let transformed = path
        .data()
        .clone()
        .transform(resvg::tiny_skia::Transform::from_scale(
            width as f32 / 100.0,
            height as f32 / 100.0,
        ))
        .ok_or_else(|| Error::Limit("static preset transform".into()))?;
    let mut result = String::new();
    for segment in transformed.segments() {
        match segment {
            PathSegment::MoveTo(point) => {
                let _ = write!(result, "M{} {} ", point.x, point.y);
            }
            PathSegment::LineTo(point) => {
                let _ = write!(result, "L{} {} ", point.x, point.y);
            }
            PathSegment::QuadTo(control, point) => {
                let _ = write!(
                    result,
                    "Q{} {} {} {} ",
                    control.x, control.y, point.x, point.y
                );
            }
            PathSegment::CubicTo(first, second, point) => {
                let _ = write!(
                    result,
                    "C{} {} {} {} {} {} ",
                    first.x, first.y, second.x, second.y, point.x, point.y
                );
            }
            PathSegment::Close => result.push('Z'),
        }
    }
    Ok(result)
}

fn shape_details(preset: &str, width: f64, height: f64) -> Vec<String> {
    match preset {
        "flowChartPredefinedProcess" => vec![format!(
            "M{} 0V{height}M{} 0V{height}",
            width * 0.13,
            width * 0.87
        )],
        "flowChartInternalStorage" => vec![format!(
            "M{} 0V{height}M0 {}H{width}",
            width * 0.15,
            height * 0.15
        )],
        "can" => vec![format!(
            "M0 {}A{} {} 0 0 0 {width} {}",
            height * 0.15,
            width / 2.0,
            height * 0.15,
            height * 0.15
        )],
        _ => vec![],
    }
}

fn warp_outline(source: &str, width: f64, height: f64, warp: crate::visual::TextWarp) -> Result<String> {
    use crate::visual::TextWarp;
    use kurbo::{BezPath, PathEl, Point};
    if width < 8.0 || height < 8.0 || !(1.0 / 32.0..=32.0).contains(&(width / height)) { return Err(Error::Limit("WordArt needs an 8px frame and aspect ratio within 1:32..32:1".into())); }
    let path = BezPath::from_svg(source).map_err(|_| Error::Invalid("glyph path could not be parsed".into()))?;
    let transform = |point: Point| {
        let fraction = point.x / width;
        let vertical = point.y / height;
        let sine = (fraction * std::f64::consts::PI).sin();
        let mapped = match warp {
            TextWarp::ArchUp => vertical * 0.65 + 0.3 * (1.0 - sine),
            TextWarp::ArchDown => vertical * 0.65 + 0.3 * sine,
            TextWarp::Wave1 => vertical * 0.65 + 0.175 + 0.15 * (fraction * std::f64::consts::TAU).sin(),
            TextWarp::Wave2 => vertical * 0.65 + 0.175 - 0.15 * (fraction * std::f64::consts::TAU).sin(),
            TextWarp::Inflate => 0.5 + (vertical - 0.5) * (0.6 + 0.4 * sine),
            TextWarp::Deflate => 0.5 + (vertical - 0.5) * (1.0 - 0.4 * sine),
            TextWarp::SlantUp => vertical * 0.7 + 0.3 * (1.0 - fraction),
            TextWarp::SlantDown => vertical * 0.7 + 0.3 * fraction,
        };
        Point::new(point.x, mapped * height)
    };
    let mut result = BezPath::new();
    let mut previous = Point::ZERO;
    let mut beginning = Point::ZERO;
    let mut overflow = false;
    let tolerance = 0.025 / (1.0 + 4.0 * height / width);
    kurbo::flatten(path.iter(), tolerance, |element| {
        if overflow { return; }
        match element {
            PathEl::MoveTo(point) => { previous = point; beginning = point; result.move_to(transform(point)); }
            PathEl::LineTo(_) | PathEl::ClosePath => {
                let point = if matches!(element, PathEl::ClosePath) { beginning } else { match element { PathEl::LineTo(point) => point, _ => previous } };
                let count = ((point.x - previous.x).abs() / width * (height * 160.0).sqrt().max(128.0)).ceil().max(1.0) as usize;
                if result.elements().len().saturating_add(count) > 8192 { overflow = true; return; }
                for index in 1..=count { result.line_to(transform(previous.lerp(point, index as f64 / count as f64))); }
                previous = point;
                if matches!(element, PathEl::ClosePath) { result.close_path(); }
            }
            _ => { overflow = true; }
        }
    });
    if overflow { return Err(Error::Limit("WordArt glyph subdivision > 8192 points".into())); }
    let output = result.to_svg();
    if output.len() > 128 * 1024 || output.contains("NaN") || output.contains("inf") { return Err(Error::Limit("WordArt glyph outline budget/finite guard".into())); }
    Ok(output)
}

#[path = "render_chart.rs"]
mod chart_projection;

fn chart_svg(
    kind: crate::model::ChartKind,
    categories: &[String],
    series: &[crate::model::ChartSeries],
    options: &crate::model::ChartOptions,
    width: f64,
    height: f64,
    theme: &Theme,
) -> Result<String> {
    use crate::model::{
        chart_format::{ChartAxis, LabelPosition, LegendPosition},
        ChartKind,
    };
    use plotters::prelude::*;
    if chart_projection::required(series, options) && !kind.is_polar() && !kind.is_extended() && !matches!(kind, ChartKind::Radar | ChartKind::RadarFilled) {
        return chart_projection::svg(kind, categories, series, options, width, height, theme);
    }
    if kind.is_extended() || matches!(kind, ChartKind::Radar | ChartKind::RadarFilled) {
        return projected_chart_svg(kind, categories, series, options, width, height, theme);
    }
    let kind = match kind {
        ChartKind::Column3d => ChartKind::Column,
        ChartKind::Bar3d => ChartKind::Bar,
        ChartKind::Pie3d => ChartKind::Pie,
        other => other,
    };
    let unsupported = || {
        Error::Unsupported(format!(
            "static chart {kind:?}: family/options are not supported by the Plotters adapter"
        ))
    };
    if !matches!(
        kind,
        ChartKind::Column
            | ChartKind::Bar
            | ChartKind::Line
            | ChartKind::Area
            | ChartKind::Scatter
            | ChartKind::StackedColumn
            | ChartKind::StackedBar
            | ChartKind::PercentStackedColumn
            | ChartKind::PercentStackedBar
            | ChartKind::Pie
            | ChartKind::Doughnut
            | ChartKind::Combo
            | ChartKind::Bubble
    ) || series.iter().any(|entry| {
        entry.trendline.is_some()
            || entry.error_bars.is_some()
    }) || [&options.primary_axis, &options.secondary_axis].iter().any(|axis| {
        axis.major_unit.is_some() || axis.minor_unit.is_some() || axis.log_base.is_some()
            || axis.reverse || axis.number_format.is_some()
    })
        || options.category_axis != Default::default()
        || options.data_labels.as_ref().is_some_and(|labels| {
            labels.number_format.is_some()
        })
        || width < 160.0
        || height < 120.0
    {
        return Err(unsupported());
    }
    let rgb = |value: &str| -> RGBColor {
        let value = value
            .strip_prefix('@')
            .and_then(|key| theme.colors.get(key))
            .map_or(value, String::as_str);
        let channel = |start| u8::from_str_radix(&value[start..start + 2], 16).unwrap_or(0);
        RGBColor(channel(0), channel(2), channel(4))
    };
    let error = |value| Error::Invalid(format!("static Plotters chart: {value:?}"));
    let mut svg = String::new();
    {
        let root = SVGBackend::with_string(&mut svg, (width.ceil() as u32, height.ceil() as u32))
            .into_drawing_area();
        if matches!(kind, ChartKind::Pie | ChartKind::Doughnut) {
            if options.primary_axis != Default::default() {
                return Err(unsupported());
            }
            use plotters::style::text_anchor::{HPos, Pos, VPos};
            let legend_style = TextStyle::from(("sans-serif", 13).into_font()).pos(Pos::new(HPos::Left, VPos::Top));
            let legend = options.legend.unwrap_or(LegendPosition::Bottom);
            let mut plot = [0.0, 0.0, width, height];
            let mut legend_items = Vec::new();
            if legend != LegendPosition::Hidden {
                let sizes = categories.iter().map(|label| root.estimate_text_size(label, &legend_style).map_err(error)).collect::<Result<Vec<_>>>()?;
                let item_width = f64::from(sizes.iter().map(|size| size.0).max().unwrap_or(0)) + 30.0;
                let row_height = (f64::from(sizes.iter().map(|size| size.1).max().unwrap_or(0)) + 6.0).max(18.0);
                let vertical = matches!(legend, LegendPosition::Left | LegendPosition::Right | LegendPosition::TopRight);
                let (columns, rows) = if vertical {
                    let rows = (((height - 16.0) / row_height).floor() as usize).max(1).min(categories.len());
                    (categories.len().div_ceil(rows), rows)
                } else {
                    let columns = (((width - 16.0) / item_width).floor() as usize).max(1).min(categories.len());
                    (columns, categories.len().div_ceil(columns))
                };
                let legend_width = columns as f64 * item_width;
                let legend_height = rows as f64 * row_height;
                let (left, top) = if vertical {
                    plot[2] -= legend_width + 16.0;
                    if legend == LegendPosition::Left { plot[0] = legend_width + 16.0; }
                    (if legend == LegendPosition::Left { 8.0 } else { width - legend_width - 8.0 },
                     if legend == LegendPosition::TopRight { 8.0 } else { (height - legend_height) / 2.0 })
                } else {
                    plot[3] -= legend_height + 16.0;
                    if legend == LegendPosition::Top { plot[1] = legend_height + 16.0; }
                    ((width - legend_width) / 2.0, if legend == LegendPosition::Top { 8.0 } else { height - legend_height - 8.0 })
                };
                if plot[2] < 100.0 || plot[3] < 80.0 || legend_width > width - 16.0 || legend_height > height - 16.0 {
                    return Err(Error::Unsupported("static pie legend does not fit; enlarge the chart or hide its legend".into()));
                }
                for index in 0..categories.len() {
                    let (column, row) = if vertical { (index / rows, index % rows) } else { (index % columns, index / columns) };
                    legend_items.push(((left + column as f64 * item_width) as i32, (top + row as f64 * row_height) as i32));
                }
            }
            let center = ((plot[0] + plot[2] / 2.0) as i32, (plot[1] + plot[3] / 2.0) as i32);
            let radius = plot[2].min(plot[3]) * 0.30;
            let values = &series[0].values;
            let colors: Vec<_> = (0..values.len())
                .map(|index| rgb(&format!("@accent{}", index % 6 + 1)))
                .collect();
            let labels: Vec<_> = categories
                .iter()
                .enumerate()
                .map(|(index, category)| {
                    let mut parts = Vec::new();
                    if options
                        .data_labels
                        .as_ref()
                        .is_none_or(|labels| labels.show_category_name)
                    {
                        parts.push(category.clone());
                    }
                    if options
                        .data_labels
                        .as_ref()
                        .is_some_and(|labels| labels.show_value)
                    {
                        parts.push(values[index].to_string());
                    }
                    if options
                        .data_labels
                        .as_ref()
                        .is_some_and(|labels| labels.show_series_name)
                    {
                        parts.push(series[0].name.clone());
                    }
                    if options
                        .data_labels
                        .as_ref()
                        .is_some_and(|labels| labels.show_percent)
                    {
                        parts.push(format!(
                            "{:.1}%",
                            values[index] / values.iter().sum::<f64>() * 100.0
                        ));
                    }
                    parts.join(" ")
                })
                .collect();
            let mut pie = Pie::new(&center, &radius, values, &colors, &labels);
            pie.label_style(("sans-serif", 14).into_font());
            if kind == ChartKind::Doughnut {
                pie.donut_hole(radius * 0.5);
            }
            root.draw(&pie).map_err(error)?;
            for (index, (left, top)) in legend_items.into_iter().enumerate() {
                root.draw(&Rectangle::new([(left, top + 2), (left + 12, top + 14)], colors[index].filled())).map_err(error)?;
                root.draw(&Text::new(categories[index].as_str(), (left + 18, top), legend_style.clone())).map_err(error)?;
            }
        } else {
            let horizontal = kind.is_horizontal();
            let stacked = matches!(
                kind,
                ChartKind::StackedColumn
                    | ChartKind::StackedBar
                    | ChartKind::PercentStackedColumn
                    | ChartKind::PercentStackedBar
            );
            let mut values: Vec<Vec<f64>> =
                series.iter().map(|entry| entry.values.clone()).collect();
            if kind.is_percent() {
                for index in 0..categories.len() {
                    let total = values.iter().map(|values| values[index]).sum::<f64>();
                    if total <= 0.0 {
                        return Err(unsupported());
                    }
                    for values in &mut values {
                        values[index] /= total;
                    }
                }
            }
            let mut minimum = 0.0f64;
            let mut maximum = 0.0f64;
            for index in 0..categories.len() {
                if stacked {
                    minimum = minimum.min(values.iter().map(|values| values[index].min(0.0)).sum());
                    maximum = maximum.max(values.iter().map(|values| values[index].max(0.0)).sum());
                } else {
                    for (entry, values) in series.iter().zip(&values) {
                        if entry.axis != Some(ChartAxis::Secondary) {
                            minimum = minimum.min(values[index]);
                            maximum = maximum.max(values[index]);
                        }
                    }
                }
            }
            let span = (maximum - minimum).max(1.0);
            let lower = options.primary_axis.min.unwrap_or(if minimum < 0.0 {
                minimum - span * 0.15
            } else {
                0.0
            });
            let upper = options.primary_axis.max.unwrap_or(if kind.is_percent() { 1.0 } else { maximum + span * 0.15 });
            if lower >= upper || upper - lower < 1e-9 {
                return Err(unsupported());
            }
            let category_end = categories.len() as f64;
            let secondary = series.iter().any(|entry| entry.axis == Some(ChartAxis::Secondary));
            let secondary_values: Vec<_> = series.iter().filter(|entry| entry.axis == Some(ChartAxis::Secondary))
                .flat_map(|entry| entry.values.iter().copied()).collect();
            let secondary_min = secondary_values.iter().copied().fold(0.0, f64::min);
            let secondary_max = secondary_values.iter().copied().fold(0.0, f64::max);
            let secondary_span = (secondary_max - secondary_min).max(1.0);
            let secondary_lower = options.secondary_axis.min.unwrap_or(if secondary_min < 0.0 { secondary_min - secondary_span * 0.15 } else { 0.0 });
            let secondary_upper = options.secondary_axis.max.unwrap_or(secondary_max + secondary_span * 0.15);
            if secondary && secondary_lower >= secondary_upper { return Err(unsupported()); }
            let project_secondary = |value: f64| lower + (value - secondary_lower) / (secondary_upper - secondary_lower) * (upper - lower);
            let (x_range, y_range) = if kind.is_xy() {
                let xs: Vec<f64> = categories
                    .iter()
                    .map(|value| value.parse::<f64>().unwrap_or(0.0))
                    .collect();
                let low = xs.iter().copied().fold(f64::INFINITY, f64::min);
                let high = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let pad = ((high - low) * 0.05).max(1.0);
                (low - pad..high + pad, lower..upper)
            } else if horizontal {
                (lower..upper, 0.0..category_end)
            } else {
                (0.0..category_end, lower..upper)
            };
            let mut chart = ChartBuilder::on(&root)
                .margin(12)
                .x_label_area_size(32)
                .y_label_area_size(if horizontal { 70 } else { 42 })
                .right_y_label_area_size(if secondary { 64 } else { 0 })
                .build_cartesian_2d(x_range, y_range)
                .map_err(error)?;
            let mut mesh = chart.configure_mesh();
            mesh.label_style(("sans-serif", 13)).disable_mesh();
            if !kind.is_xy() {
                if horizontal {
                    mesh.y_labels(0);
                } else {
                    mesh.x_labels(0);
                }
            }
            mesh.draw().map_err(error)?;
            if secondary {
                for step in 0..=4 {
                    let value = secondary_lower + (secondary_upper - secondary_lower) * step as f64 / 4.0;
                    let (left, top) = chart.backend_coord(&(category_end, project_secondary(value)));
                    root.draw(&Text::new(value.to_string(), (left + 5, top), ("sans-serif", 12).into_font())).map_err(error)?;
                }
            }
            if !kind.is_xy() {
                use plotters::style::text_anchor::{HPos, Pos, VPos};
                for (index, category) in categories.iter().enumerate() {
                    let coordinate = if horizontal {
                        (lower, index as f64 + 0.5)
                    } else {
                        (index as f64 + 0.5, lower)
                    };
                    let (left, top) = chart.backend_coord(&coordinate);
                    let position = if horizontal {
                        (left - 5, top)
                    } else {
                        (left, top + 8)
                    };
                    let style =
                        TextStyle::from(("sans-serif", 13).into_font()).pos(if horizontal {
                            Pos::new(HPos::Right, VPos::Center)
                        } else {
                            Pos::new(HPos::Center, VPos::Top)
                        });
                    root.draw(&Text::new(category.as_str(), position, style))
                        .map_err(error)?;
                }
            }
            let mut positive = vec![0.0; categories.len()];
            let mut negative = vec![0.0; categories.len()];
            for (series_index, entry) in series.iter().enumerate() {
                let color = rgb(&entry.color);
                let projected: Vec<f64> = values[series_index].iter().map(|value| if entry.axis == Some(ChartAxis::Secondary) { project_secondary(*value) } else { *value }).collect();
                let data = &projected;
                let series_kind = if kind == ChartKind::Combo { entry.kind.unwrap_or(ChartKind::Column) } else { kind };
                let baseline = if entry.axis == Some(ChartAxis::Secondary) { project_secondary(0.0) } else { 0.0 };
                if matches!(series_kind, ChartKind::Line | ChartKind::Area | ChartKind::Scatter | ChartKind::Bubble) {
                    let coordinates: Vec<_> = data
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            (
                                if kind.is_xy() {
                                    categories[index].parse::<f64>().unwrap_or(0.0)
                                } else {
                                    index as f64 + 0.5
                                },
                                *value,
                            )
                        })
                        .collect();
                    match series_kind {
                        ChartKind::Line => {
                            chart
                                .draw_series(LineSeries::new(
                                    coordinates.clone(),
                                    color.stroke_width(2),
                                ))
                                .map_err(error)?
                                .label(&entry.name)
                                .legend(move |(left, top)| {
                                    PathElement::new(vec![(left, top), (left + 14, top)], color)
                                });
                        }
                        ChartKind::Area => {
                            chart
                                .draw_series(
                                    AreaSeries::new(coordinates.clone(), baseline, color.mix(0.4))
                                        .border_style(color),
                                )
                                .map_err(error)?
                                .label(&entry.name)
                                .legend(move |(left, top)| {
                                    Rectangle::new(
                                        [(left, top - 3), (left + 14, top + 3)],
                                        color.filled(),
                                    )
                                });
                        }
                        _ => {
                            let maximum_size = series.iter().filter_map(|entry| entry.bubble_sizes.as_ref()).flatten().copied().fold(1.0, f64::max);
                            chart
                                .draw_series(
                                    coordinates.iter().enumerate().map(|(index, coordinate)| {
                                        let radius = entry.bubble_sizes.as_ref().map_or(4.0, |sizes| 24.0 * (sizes[index] / maximum_size).sqrt());
                                        Circle::new(*coordinate, radius, color.filled())
                                    }),
                                )
                                .map_err(error)?
                                .label(&entry.name)
                                .legend(move |(left, top)| {
                                    Circle::new((left + 7, top), 4, color.filled())
                                });
                        }
                    }
                    for (index, coordinate) in coordinates.iter().enumerate() {
                        if let Some(label) = chart_label(
                            options,
                            &entry.name,
                            &categories[index],
                            entry.values[index],
                            data[index],
                            kind.is_percent(),
                        ) {
                            chart
                                .draw_series(std::iter::once(Text::new(
                                    label,
                                    (coordinate.0, coordinate.1 + span * 0.03),
                                    ("sans-serif", 13).into_font(),
                                )))
                                .map_err(error)?;
                        }
                    }
                } else {
                    for (index, value) in data.iter().enumerate() {
                        let start = if stacked {
                            if *value >= 0.0 {
                                positive[index]
                            } else {
                                negative[index]
                            }
                        } else {
                            baseline
                        };
                        let end = if stacked { start + value } else { *value };
                        if stacked {
                            if *value >= 0.0 {
                                positive[index] = end;
                            } else {
                                negative[index] = end;
                            }
                        }
                        let fraction = if stacked {
                            0.8
                        } else {
                            0.8 / series.len() as f64
                        };
                        let left = index as f64
                            + 0.1
                            + if stacked {
                                0.0
                            } else {
                                series_index as f64 * fraction
                            };
                        let right = left + fraction;
                        let bounds = if horizontal {
                            [(start, left), (end, right)]
                        } else {
                            [(left, start), (right, end)]
                        };
                        let annotation = chart
                            .draw_series(std::iter::once(Rectangle::new(bounds, color.filled())))
                            .map_err(error)?;
                        if index == 0 {
                            annotation.label(&entry.name).legend(move |(left, top)| {
                                Rectangle::new(
                                    [(left, top - 3), (left + 14, top + 3)],
                                    color.filled(),
                                )
                            });
                        }
                        if let Some(label) = chart_label(
                            options,
                            &entry.name,
                            &categories[index],
                            entry.values[index],
                            *value,
                            kind.is_percent(),
                        ) {
                            let label_value = match options.data_labels.as_ref().and_then(|labels| labels.position) {
                                Some(LabelPosition::Center) => (start + end) / 2.0,
                                Some(LabelPosition::InsideEnd) => end - value.signum() * span * 0.03,
                                _ => end + value.signum() * span * 0.02,
                            };
                            let position = if horizontal { (label_value, (left + right) / 2.0) } else { ((left + right) / 2.0, label_value) };
                            chart
                                .draw_series(std::iter::once(Text::new(
                                    label,
                                    position,
                                    ("sans-serif", 13).into_font(),
                                )))
                                .map_err(error)?;
                        }
                    }
                }
            }
            if options.legend != Some(LegendPosition::Hidden) {
                let position = match options.legend.unwrap_or(LegendPosition::Bottom) {
                    LegendPosition::Bottom => SeriesLabelPosition::LowerMiddle,
                    LegendPosition::Top => SeriesLabelPosition::UpperMiddle,
                    LegendPosition::Left => SeriesLabelPosition::MiddleLeft,
                    LegendPosition::Right => SeriesLabelPosition::MiddleRight,
                    _ => SeriesLabelPosition::UpperRight,
                };
                chart
                    .configure_series_labels()
                    .position(position)
                    .label_font(("sans-serif", 13))
                    .background_style(WHITE.mix(0.8))
                    .draw()
                    .map_err(error)?;
            }
        }
        root.present().map_err(error)?;
    }
    if svg.len() > 1024 * 1024 {
        return Err(Error::Limit("static chart SVG > 1 MiB".into()));
    }
    Ok(svg)
}

#[derive(Default)]
struct HierarchyNode {
    name: String,
    value: f64,
    children: Vec<HierarchyNode>,
}

fn hierarchy_tree(paths: &[Vec<String>], values: &[f64]) -> HierarchyNode {
    let mut root = HierarchyNode::default();
    for (path, value) in paths.iter().zip(values) {
        let mut parent = &mut root;
        parent.value += value;
        for name in path {
            let position = parent.children.iter().position(|child| &child.name == name)
                .unwrap_or_else(|| {
                    parent.children.push(HierarchyNode { name: name.clone(), ..Default::default() });
                    parent.children.len() - 1
                });
            parent = &mut parent.children[position];
            parent.value += value;
        }
    }
    root
}

fn histogram_bins(options: &crate::model::chart_format::HistogramOptions) -> Vec<(String, f64)> {
    use crate::model::chart_format::{HistogramBinning, IntervalClosed};
    let minimum = options.samples.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = options.samples.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let lower = options.underflow.unwrap_or(minimum);
    let upper = options.overflow.unwrap_or(maximum).max(lower);
    let count = match options.binning {
        HistogramBinning::Count { count } => count as usize,
        HistogramBinning::Width { width } => ((upper - lower) / width).ceil().max(1.0) as usize,
    };
    let width = match options.binning {
        HistogramBinning::Width { width } => width,
        HistogramBinning::Count { .. } => if upper > lower { (upper - lower) / count as f64 } else { 1.0 },
    };
    let mut bins: Vec<_> = (0..count).map(|index| {
        let start = lower + index as f64 * width;
        let end = options.overflow.map_or(lower + (index + 1) as f64 * width, |threshold| (lower + (index + 1) as f64 * width).min(threshold));
        let left = if index == 0 { if options.underflow.is_some() { '(' } else { '[' } }
            else if options.interval_closed == IntervalClosed::Left { '[' } else { '(' };
        let right = if index + 1 == count || options.interval_closed == IntervalClosed::Right { ']' } else { ')' };
        (format!("{left}{start}, {end}{right}"), 0.0)
    }).collect();
    let mut underflow = 0.0;
    let mut overflow = 0.0;
    for sample in &options.samples {
        if options.underflow.is_some_and(|threshold| *sample <= threshold) { underflow += 1.0; continue; }
        if options.overflow.is_some_and(|threshold| *sample > threshold) { overflow += 1.0; continue; }
        let position = (sample - lower) / width;
        let index = match options.interval_closed {
            IntervalClosed::Left => position.floor(),
            IntervalClosed::Right => position.ceil() - 1.0,
        }.max(0.0) as usize;
        bins[index.min(count - 1)].1 += 1.0;
    }
    if let Some(threshold) = options.underflow { bins.insert(0, (format!("<={threshold}"), underflow)); }
    if let Some(threshold) = options.overflow { bins.push((format!(">{threshold}"), overflow)); }
    bins
}

fn sample_quantile(sorted: &[f64], fraction: f64, method: crate::model::chart_format::QuartileMethod) -> f64 {
    use crate::model::chart_format::QuartileMethod;
    let position = match method {
        QuartileMethod::Inclusive => (sorted.len() - 1) as f64 * fraction,
        QuartileMethod::Exclusive => (sorted.len() + 1) as f64 * fraction - 1.0,
    }.clamp(0.0, (sorted.len() - 1) as f64);
    let lower = position.floor() as usize;
    let upper = (lower + 1).min(sorted.len() - 1);
    sorted[lower] + (sorted[upper] - sorted[lower]) * (position - lower as f64)
}

fn projected_chart_svg(
    kind: crate::model::ChartKind,
    categories: &[String],
    series: &[crate::model::ChartSeries],
    options: &crate::model::ChartOptions,
    width: f64,
    height: f64,
    theme: &Theme,
) -> Result<String> {
    use crate::model::{ChartKind, chart_format::{LegendPosition, ParentLabelLayout}};
    use plotters::{coord::Shift, prelude::*};
    let unsupported = || Error::Unsupported(format!("static chart {kind:?}: unsupported projection options"));
    if width < 160.0 || height < 120.0 || series.iter().any(|entry| entry.trendline.is_some() || entry.error_bars.is_some())
        || [&options.primary_axis, &options.secondary_axis, &options.category_axis].iter().any(|axis| axis.major_unit.is_some() || axis.minor_unit.is_some() || axis.log_base.is_some() || axis.reverse || axis.number_format.is_some())
        || options.data_labels.as_ref().is_some_and(|labels| labels.number_format.is_some()) {
        return Err(unsupported());
    }
    let rgb = |value: &str| {
        let value = value.strip_prefix('@').and_then(|key| theme.colors.get(key)).map_or(value, String::as_str);
        let channel = |start| u8::from_str_radix(&value[start..start + 2], 16).unwrap_or(0);
        RGBColor(channel(0), channel(2), channel(4))
    };
    let palette: Vec<_> = (1..=6).map(|index| rgb(&format!("@accent{index}"))).collect();
    let ink = rgb("@dk1");
    let shade = rgb(&series[0].color);
    let error = |value| Error::Invalid(format!("static Plotters projection: {value:?}"));
    let mut svg = String::new();
    {
        let root = SVGBackend::with_string(&mut svg, (width.ceil() as u32, height.ceil() as u32)).into_drawing_area();
        let text = |label: String, position: (i32, i32), size: u32| -> Result<()> {
            root.draw(&Text::new(label, position, ("sans-serif", size).into_font().color(&ink))).map_err(error)
        };
        let plot = [44.0, 24.0, width - 70.0, height - 84.0];
        if options.legend != Some(LegendPosition::Hidden) {
            let label = series.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>().join(" / ");
            let position = match options.legend.unwrap_or(LegendPosition::Bottom) {
                LegendPosition::Top | LegendPosition::TopRight => (44, 4),
                LegendPosition::Left => (0, 4),
                LegendPosition::Right => ((width - 120.0) as i32, 4),
                _ => (44, (height - 18.0) as i32),
            };
            text(label, position, 12)?;
        }
        match kind {
            ChartKind::Radar | ChartKind::RadarFilled => {
                let lower = options.primary_axis.min.unwrap_or(series.iter().flat_map(|entry| entry.values.iter()).copied().fold(0.0, f64::min));
                let upper = options.primary_axis.max.unwrap_or(series.iter().flat_map(|entry| entry.values.iter()).copied().fold(1.0, f64::max));
                if lower >= upper { return Err(unsupported()); }
                let center = (width / 2.0, height / 2.0);
                let radius = width.min(height) * 0.32;
                let point = |index: usize, value: f64| {
                    let angle = -std::f64::consts::FRAC_PI_2 + index as f64 * std::f64::consts::TAU / categories.len() as f64;
                    let distance = radius * ((value - lower) / (upper - lower)).clamp(0.0, 1.0);
                    ((center.0 + distance * angle.cos()) as i32, (center.1 + distance * angle.sin()) as i32)
                };
                for level in 1..=4 {
                    let value = lower + (upper - lower) * level as f64 / 4.0;
                    let mut points: Vec<_> = (0..categories.len()).map(|index| point(index, value)).collect();
                    points.push(points[0]);
                    root.draw(&PathElement::new(points, ink.mix(0.25))).map_err(error)?;
                    text(value.to_string(), point(0, value), 10)?;
                }
                for (index, category) in categories.iter().enumerate() {
                    root.draw(&PathElement::new(vec![(center.0 as i32, center.1 as i32), point(index, upper)], ink.mix(0.25))).map_err(error)?;
                    text(category.clone(), point(index, upper), 11)?;
                }
                for entry in series {
                    let color = rgb(&entry.color);
                    let mut points: Vec<_> = entry.values.iter().enumerate().map(|(index, value)| point(index, *value)).collect();
                    if kind == ChartKind::RadarFilled { root.draw(&Polygon::new(points.clone(), color.mix(0.35).filled())).map_err(error)?; }
                    points.push(points[0]);
                    root.draw(&PathElement::new(points, color.stroke_width(2))).map_err(error)?;
                    for (index, value) in entry.values.iter().enumerate() {
                        root.draw(&Circle::new(point(index, *value), 3, color.filled())).map_err(error)?;
                        if let Some(label) = chart_label(options, &entry.name, &categories[index], *value, *value, false) { text(label, point(index, *value), 11)?; }
                    }
                }
            }
            ChartKind::Funnel => {
                let values = &series[0].values;
                let maximum = values.iter().copied().fold(0.0, f64::max);
                let step = plot[3] / values.len() as f64;
                for (index, value) in values.iter().enumerate() {
                    let top_width = plot[2] * value / maximum;
                    let bottom_width = plot[2] * values.get(index + 1).unwrap_or(value) / maximum;
                    let top = plot[1] + index as f64 * step;
                    let center = plot[0] + plot[2] / 2.0;
                    let points = vec![((center - top_width / 2.0) as i32, top as i32), ((center + top_width / 2.0) as i32, top as i32), ((center + bottom_width / 2.0) as i32, (top + step) as i32), ((center - bottom_width / 2.0) as i32, (top + step) as i32)];
                    root.draw(&Polygon::new(points.clone(), shade.mix(0.7).filled())).map_err(error)?;
                    root.draw(&PathElement::new(vec![points[0], points[1]], ink.mix(0.3))).map_err(error)?;
                    text(format!("{}: {value}", categories[index]), (plot[0] as i32, (top + step / 2.0) as i32), 12)?;
                }
            }
            ChartKind::Waterfall | ChartKind::Histogram => {
                let mut accumulator = 0.0;
                let bars: Vec<(String, f64, f64, f64)> = if let Some(histogram) = &options.histogram {
                    histogram_bins(histogram).into_iter().map(|(label, count)| (label, 0.0, count, count)).collect()
                } else {
                    categories.iter().zip(&series[0].values).enumerate().map(|(index, (label, value))| {
                        let total = options.waterfall_totals.as_ref().is_some_and(|indices| indices.contains(&(index as u32)));
                        let start = if total { 0.0 } else { accumulator };
                        let end = if total { *value } else { accumulator + value };
                        accumulator = end;
                        (label.clone(), start, end, *value)
                    }).collect()
                };
                let minimum = bars.iter().flat_map(|entry| [entry.1, entry.2]).fold(0.0, f64::min);
                let maximum = bars.iter().flat_map(|entry| [entry.1, entry.2]).fold(1.0, f64::max);
                let span = (maximum - minimum).max(1.0);
                let vertical = |value| (plot[1] + (maximum - value) / span * plot[3]) as i32;
                let step = plot[2] / bars.len() as f64;
                root.draw(&PathElement::new(vec![(plot[0] as i32, vertical(0.0)), ((plot[0] + plot[2]) as i32, vertical(0.0))], ink)).map_err(error)?;
                for (index, (label, start, end, value)) in bars.iter().enumerate() {
                    let left = plot[0] + index as f64 * step;
                    root.draw(&Rectangle::new([((left + step * 0.1) as i32, vertical(start.max(*end))), ((left + step * 0.9) as i32, vertical(start.min(*end)))], shade.filled())).map_err(error)?;
                    text(format!("{label}: {value}"), (left as i32, (plot[1] + plot[3] + 8.0) as i32), 10)?;
                    text(format!("{start} -> {end}"), (left as i32, vertical(start.max(*end)) - 12), 10)?;
                }
            }
            ChartKind::BoxWhisker => {
                let statistics = options.box_whisker.as_ref().ok_or_else(unsupported)?;
                let minimum = statistics.samples.iter().flatten().copied().fold(f64::INFINITY, f64::min);
                let maximum = statistics.samples.iter().flatten().copied().fold(f64::NEG_INFINITY, f64::max);
                let padding = ((maximum - minimum) * 0.1).max(1.0);
                let vertical = |value| (plot[1] + (maximum + padding - value) / (maximum - minimum + 2.0 * padding) * plot[3]) as i32;
                let step = plot[2] / statistics.samples.len() as f64;
                let mut means = Vec::new();
                for (index, samples) in statistics.samples.iter().enumerate() {
                    let mut sorted = samples.clone();
                    sorted.sort_by(f64::total_cmp);
                    let first = sample_quantile(&sorted, 0.25, statistics.quartile_method);
                    let median = sample_quantile(&sorted, 0.5, statistics.quartile_method);
                    let third = sample_quantile(&sorted, 0.75, statistics.quartile_method);
                    let fence = (third - first) * 1.5;
                    let inside: Vec<_> = sorted.iter().copied().filter(|value| *value >= first - fence && *value <= third + fence).collect();
                    let low = inside.first().copied().unwrap_or(median);
                    let high = inside.last().copied().unwrap_or(median);
                    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
                    let center = (plot[0] + step * (index as f64 + 0.5)) as i32;
                    let half = (step * 0.25) as i32;
                    means.push((center, vertical(mean)));
                    root.draw(&PathElement::new(vec![(center, vertical(low)), (center, vertical(high))], ink)).map_err(error)?;
                    for value in [low, high] { root.draw(&PathElement::new(vec![(center - half / 2, vertical(value)), (center + half / 2, vertical(value))], ink)).map_err(error)?; }
                    root.draw(&Rectangle::new([(center - half, vertical(third)), (center + half, vertical(first).max(vertical(third) + 1))], shade.mix(0.45).filled())).map_err(error)?;
                    root.draw(&Rectangle::new([(center - half, vertical(third)), (center + half, vertical(first).max(vertical(third) + 1))], ink)).map_err(error)?;
                    root.draw(&PathElement::new(vec![(center - half, vertical(median)), (center + half, vertical(median))], ink.stroke_width(2))).map_err(error)?;
                    if statistics.mean_marker { root.draw(&Cross::new((center, vertical(mean)), 4, ink)).map_err(error)?; }
                    for value in &sorted {
                        let outside = *value < first - fence || *value > third + fence;
                        if outside && statistics.outliers || !outside && statistics.nonoutliers { root.draw(&Circle::new((center, vertical(*value)), 2, ink.filled())).map_err(error)?; }
                    }
                    text(format!("{}: median {median}; Q1 {first}; Q3 {third}; whiskers {low}..{high}", categories[index]), (center - half, (plot[1] + plot[3] + 8.0) as i32), 10)?;
                }
                if statistics.mean_line { root.draw(&PathElement::new(means, shade.stroke_width(2))).map_err(error)?; }
            }
            ChartKind::Treemap | ChartKind::Sunburst => {
                let hierarchy = options.hierarchy.as_ref().ok_or_else(unsupported)?;
                let tree = hierarchy_tree(&hierarchy.paths, &series[0].values);
                struct HierarchyPainter<'area, 'svg> {
                    root: &'area DrawingArea<SVGBackend<'svg>, Shift>,
                    palette: &'area [RGBColor],
                    ink: RGBColor,
                    labels: Option<ParentLabelLayout>,
                    sunburst: bool,
                    center: (f64, f64),
                    ring: f64,
                }
                impl HierarchyPainter<'_, '_> {
                    fn paint(&self, parent: &HierarchyNode, bounds: [f64; 4], angles: [f64; 2], depth: usize, shade: usize) -> Result<()> {
                        let error = |value| Error::Invalid(format!("static hierarchy: {value:?}"));
                        let mut offset = 0.0;
                        for (index, node) in parent.children.iter().enumerate() {
                            let fraction = node.value / parent.value;
                            let color_index = if depth == 0 { index } else { shade };
                            let color = self.palette[color_index % self.palette.len()];
                            let split_width = bounds[2] >= bounds[3];
                            let child_bounds = if split_width { [bounds[0] + bounds[2] * offset, bounds[1], bounds[2] * fraction, bounds[3]] } else { [bounds[0], bounds[1] + bounds[3] * offset, bounds[2], bounds[3] * fraction] };
                            let start = angles[0] + (angles[1] - angles[0]) * offset;
                            let end = start + (angles[1] - angles[0]) * fraction;
                            let position;
                            if self.sunburst {
                                let inner = self.ring * depth as f64 + 12.0;
                                let outer = inner + self.ring;
                                let segments = (((end - start).abs() * 40.0).ceil() as usize).max(2);
                                let point = |radius: f64, angle: f64| ((self.center.0 + radius * angle.cos()) as i32, (self.center.1 + radius * angle.sin()) as i32);
                                let mut points: Vec<_> = (0..=segments).map(|step| point(outer, start + (end - start) * step as f64 / segments as f64)).collect();
                                points.extend((0..=segments).rev().map(|step| point(inner, start + (end - start) * step as f64 / segments as f64)));
                                self.root.draw(&Polygon::new(points.clone(), color.mix(1.0 - depth as f64 * 0.15).filled())).map_err(error)?;
                                points.push(points[0]);
                                self.root.draw(&PathElement::new(points, WHITE)).map_err(error)?;
                                position = point((inner + outer) / 2.0, (start + end) / 2.0);
                            } else {
                                let [left, top, width, height] = child_bounds;
                                self.root.draw(&Rectangle::new([(left as i32, top as i32), ((left + width) as i32, (top + height) as i32)], color.filled())).map_err(error)?;
                                position = (left as i32 + 3, top as i32 + 3);
                            }
                            self.paint(node, child_bounds, [start, end], depth + 1, color_index)?;
                            if !self.sunburst {
                                let [left, top, width, height] = child_bounds;
                                self.root.draw(&Rectangle::new([(left as i32, top as i32), ((left + width) as i32, (top + height) as i32)], WHITE)).map_err(error)?;
                            }
                            if node.children.is_empty() || self.sunburst || self.labels != Some(ParentLabelLayout::None) {
                                if !self.sunburst && !node.children.is_empty() && self.labels == Some(ParentLabelLayout::Banner) {
                                    self.root.draw(&Rectangle::new([(position.0 - 2, position.1 - 2), ((child_bounds[0] + child_bounds[2]) as i32, position.1 + 12)], WHITE.mix(0.8).filled())).map_err(error)?;
                                }
                                self.root.draw(&Text::new(format!("{}: {}", node.name, node.value), position, ("sans-serif", 10).into_font().color(&self.ink))).map_err(error)?;
                            }
                            offset += fraction;
                        }
                        Ok(())
                    }
                }
                HierarchyPainter { root: &root, palette: &palette, ink, labels: hierarchy.parent_labels, sunburst: kind == ChartKind::Sunburst, center: (width / 2.0, height / 2.0), ring: (width.min(height) * 0.4 - 12.0) / hierarchy.paths[0].len() as f64 }
                    .paint(&tree, plot, [-std::f64::consts::FRAC_PI_2, std::f64::consts::TAU - std::f64::consts::FRAC_PI_2], 0, 0)?;
            }
            _ => return Err(unsupported()),
        }
        root.present().map_err(error)?;
    }
    if svg.len() > 1024 * 1024 { return Err(Error::Limit("static chart SVG > 1 MiB".into())); }
    Ok(svg)
}

fn chart_label(
    options: &crate::model::ChartOptions,
    series: &str,
    category: &str,
    value: f64,
    plotted: f64,
    percent: bool,
) -> Option<String> {
    let labels = options.data_labels.as_ref()?;
    let mut parts = Vec::new();
    if labels.show_series_name {
        parts.push(series.into());
    }
    if labels.show_category_name {
        parts.push(category.into());
    }
    if labels.show_value {
        parts.push(value.to_string());
    }
    if labels.show_percent && percent {
        parts.push(format!("{plotted:.1}%"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}
