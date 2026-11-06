//! The background behind each text element, for `check_accessibility`.
//!
//! Layers are the master, layout and slide in the order the renderer paints them, so a nested group, an inherited slide
//! background and an inherited graphic are read the way they are drawn. Text gets a background only when the paint behind it
//! is known exactly: the topmost opaque solid shape below it that covers its whole frame, or else the background the renderer
//! paints for the slide. A shape that holds text is its own backdrop only where its outline holds that text: the content
//! frame the text is drawn in lies inside a rectangle, rounded rectangle or ellipse, or else the laid-out text does (centered
//! text in a circle or a pill usually does). The outline is read in the shape's own coordinates, so a turned shape is judged
//! like an upright one. A line is no backdrop of its own: a connector, or the outline of a shape that has no fill, leaves the text
//! unsupported where its stroke crosses the text frame, as wide as it is drawn and with the corners and arrowheads it ends in (these are
//! covered by squares and rectangles a little larger than the paint, so the check errs toward unsupported), while the border of a filled
//! shape is an edge of that fill. The shadow, glow and reflection of a shape reach past it, out to the extent
//! the renderer draws them, and text there is unsupported unless a shape above them covers it. A partial cover, a picture, chart or
//! table, a gradient, transparency, a soft edge, an outline this module cannot measure (a custom path or another preset), or rotation
//! and skew when the background has to be found behind the text is reported with a reason. Nothing is guessed to be white.
use crate::{model::{Deck, Element, TextAlign, VerticalAlign}, visual::VisualStyle};
use kurbo::{Affine, Point, Rect};
use std::collections::BTreeMap;

/// How far, in pixels, a shape may fall short of a text frame and still count as covering it.
const TOLERANCE: f64 = 0.5;

pub(crate) const BACKGROUND_NOT_SOLID: &str = "background_not_solid";
pub(crate) const PARTIAL_COVER: &str = "partial_cover";
pub(crate) const UNSUPPORTED_BACKDROP: &str = "unsupported_backdrop";
pub(crate) const TRANSFORMED_TEXT: &str = "transformed_text";
pub(crate) const TEXT_STYLE: &str = "text_highlight_or_baseline";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Backdrop {
    /// One solid color as written (`@lt1` or `RRGGBB`), behind the whole text frame.
    Solid(String),
    /// Why the paint behind the text is not known exactly.
    Unsupported(&'static str),
}

#[derive(Clone, Debug)]
pub(crate) struct TextBackdrop {
    /// Rendered size over the authored size: the scale the enclosing groups apply.
    pub scale: f64,
    pub backdrop: Backdrop,
}

enum Geometry { Rect(Rect), RoundRect(Rect, f64), Ellipse(Rect), Convex(Vec<Point>), Other }

enum Paint { Solid(String, Geometry), Unknown(&'static str), Line(Vec<[Point; 4]>) }

struct Painter { frame: Rect, paint: Paint }

enum Fill { Nothing, Solid(String), Unknown(&'static str) }

/// What a text-bearing shape lays under its own text. Its outline is kept in the shape's own coordinates, where a turn or a
/// group's scale changes nothing about whether the text sits inside it.
enum OwnPaint<'a> { Nothing, Unknown(&'static str), Solid(Own<'a>) }

struct Own<'a> { color: String, shape: &'a Element, outline: Geometry, frame: Rect }

struct Site<'a> { id: String, before: usize, frame: Rect, scale: f64, aligned: bool, faded: bool, own: OwnPaint<'a> }

struct Context { transform: Affine, faded: bool, hidden: bool }

/// The backdrop of every text-bearing element of the slide at `page`, by element ID.
pub(crate) fn text_backdrops(deck: &Deck, page: usize) -> BTreeMap<String, TextBackdrop> {
    let Some(slide) = deck.slides.get(page) else { return BTreeMap::new(); };
    let layers = crate::render::slide_layers(deck, slide);
    let background = crate::design::rendered_background(slide, deck.design.as_ref());
    let mut painters = Vec::new();
    let mut sites = Vec::new();
    let context = Context { transform: Affine::IDENTITY, faded: false, hidden: false };
    for (index, (_, elements)) in layers.iter().enumerate() { walk(elements, &context, index + 1 == layers.len(), &mut painters, &mut sites); }
    let mut backdrops: Vec<Option<Backdrop>> = sites.iter().map(|site| match &site.own {
        OwnPaint::Nothing => Some(backdrop(site, &painters, background)),
        OwnPaint::Unknown(reason) => Some(Backdrop::Unsupported(reason)),
        OwnPaint::Solid(own) => own.settle(),
    }).collect();
    // A frame that reaches outside its shape's outline is left to the text itself; one pass measures every such shape.
    let open: Vec<usize> = (0..sites.len()).filter(|&index| backdrops[index].is_none()).collect();
    if !open.is_empty() {
        let shapes: Vec<&Element> = open.iter().filter_map(|&index| match &sites[index].own { OwnPaint::Solid(own) => Some(own.shape), _ => None }).collect();
        for (&index, extent) in open.iter().zip(crate::layout::text_extents(deck, slide, &shapes)) {
            if let OwnPaint::Solid(own) = &sites[index].own { backdrops[index] = Some(own.settle_by_text(extent)); }
        }
    }
    sites.into_iter().zip(backdrops).map(|(site, backdrop)| (site.id, TextBackdrop { scale: site.scale, backdrop: backdrop.unwrap_or(Backdrop::Unsupported(UNSUPPORTED_BACKDROP)) })).collect()
}

fn walk<'a>(elements: &'a [Element], parent: &Context, slide_layer: bool, painters: &mut Vec<Painter>, sites: &mut Vec<Site<'a>>) {
    for element in elements {
        let (id, x, y, width, height) = element.bounds();
        let none = VisualStyle::default();
        let style = element.visual().unwrap_or(&none);
        let rotation = match element { Element::Shape { rotation, .. } => *rotation, _ => style.rotation.unwrap_or(0.0) };
        let transform = parent.transform * Affine::translate((x, y)) * Affine::translate((width / 2.0, height / 2.0))
            * Affine::rotate(rotation.to_radians())
            * Affine::scale_non_uniform(if style.flip_h { -1.0 } else { 1.0 }, if style.flip_v { -1.0 } else { 1.0 })
            * Affine::translate((-width / 2.0, -height / 2.0));
        // A group cannot carry an opacity (`visual::validate` rejects it), so only a soft edge fades what a group holds.
        let faded = parent.faded || style.soft_edge.is_some_and(|radius| radius > 0.0);
        let hidden = parent.hidden || style.hidden;
        let frame = transform.transform_rect_bbox(Rect::new(0.0, 0.0, width, height));
        let [a, b, c, d, _, _] = transform.as_coeffs();
        let aligned = b.abs() < 1e-9 && c.abs() < 1e-9 && a.abs() > 1e-9 && d.abs() > 1e-9;
        let fill = fill_of(element, style, faded);
        if slide_layer {
            if let Element::Text { text, .. } | Element::Shape { text, .. } = element {
                if !text.trim().is_empty() {
                    sites.push(Site { id: id.to_owned(), before: painters.len(), frame, scale: (a.hypot(b).min(c.hypot(d)) * 1e6).round() / 1e6, aligned, faded, own: own_paint(element, style, &fill) });
                }
            }
        }
        if !hidden { if let Some(painter) = effect_painter(element, style, transform) { painters.push(painter); } }
        if let Element::Group { view_width, view_height, children, .. } = element {
            if *view_width > 0.0 && *view_height > 0.0 {
                let context = Context { transform: transform * Affine::scale_non_uniform(width / view_width, height / view_height), faded, hidden };
                walk(children, &context, slide_layer, painters, sites);
            }
            continue;
        }
        if hidden { continue; }
        if let Some(painter) = line_painter(element, style, transform, frame, width, height) { painters.push(painter); }
        match fill {
            Fill::Nothing => {}
            Fill::Unknown(reason) => painters.push(Painter { frame, paint: Paint::Unknown(reason) }),
            Fill::Solid(color) => painters.push(Painter { frame, paint: if aligned { Paint::Solid(color, geometry(element, style, transform, frame)) } else { Paint::Unknown(UNSUPPORTED_BACKDROP) } }),
        }
    }
}

/// What an element paints over its area: a plain opaque solid, something that is not one, or nothing.
fn fill_of(element: &Element, style: &VisualStyle, faded: bool) -> Fill {
    let transparent = style.opacity.is_some_and(|opacity| opacity <= 0.0);
    let fill = match element {
        Element::Rect { fill, .. } | Element::Shape { fill, .. } | Element::Polygon { fill, .. } => fill.as_str(),
        Element::Picture { .. } | Element::Chart { .. } | Element::Table { .. } => return if transparent { Fill::Nothing } else { Fill::Unknown(UNSUPPORTED_BACKDROP) },
        _ => return Fill::Nothing,
    };
    if fill == "none" || transparent { Fill::Nothing }
    else if faded || style.opacity.is_some_and(|opacity| opacity < 1.0) || style.gradient.is_some() || style.soft_edge.is_some_and(|radius| radius > 0.0) { Fill::Unknown(BACKGROUND_NOT_SOLID) }
    else { Fill::Solid(fill.to_owned()) }
}

fn geometry(element: &Element, style: &VisualStyle, transform: Affine, frame: Rect) -> Geometry {
    if style.path.is_some() { return Geometry::Other; }
    match element {
        Element::Rect { .. } => Geometry::Rect(frame),
        Element::Shape { preset, width, height, .. } => match preset.as_str() {
            "rect" => Geometry::Rect(frame),
            "roundRect" => {
                let adjustment = style.adjustments.iter().find(|adjustment| adjustment.name == "adj").map_or(16667.0, |adjustment| f64::from(adjustment.value)).clamp(0.0, 50000.0);
                let [a, _, _, d, _, _] = transform.as_coeffs();
                // The larger of the two scales gives the smaller shape, so a text frame is never wrongly counted as covered.
                Geometry::RoundRect(frame, width.min(*height) * adjustment / 100000.0 * a.abs().max(d.abs()))
            }
            "ellipse" => Geometry::Ellipse(frame),
            _ => Geometry::Other,
        },
        Element::Polygon { points, width, height, .. } => {
            let page: Vec<Point> = points.iter().map(|point| transform * Point::new(point[0] * width, point[1] * height)).collect();
            if is_convex(&page) { Geometry::Convex(page) } else { Geometry::Other }
        }
        _ => Geometry::Other,
    }
}

/// The background of text that has no paint of its own: the nearest painter below it, or else the slide's background.
fn backdrop(site: &Site, painters: &[Painter], background: &str) -> Backdrop {
    if !site.aligned { return Backdrop::Unsupported(TRANSFORMED_TEXT); }
    if site.faded { return Backdrop::Unsupported(BACKGROUND_NOT_SOLID); }
    for painter in painters[..site.before].iter().rev() {
        if !overlaps(painter.frame, site.frame) { continue; }
        return match &painter.paint {
            // A line is no backdrop of its own: it matters only where its stroke, width included, crosses the text frame, and then
            // the paint under it is not one color.
            Paint::Line(quads) if !quads.iter().any(|quad| meets(quad, site.frame)) => continue,
            Paint::Line(_) => Backdrop::Unsupported(UNSUPPORTED_BACKDROP),
            Paint::Solid(color, shape) if shape.covers(site.frame) => Backdrop::Solid(color.clone()),
            Paint::Solid(_, Geometry::Other) => Backdrop::Unsupported(UNSUPPORTED_BACKDROP),
            Paint::Solid(..) => Backdrop::Unsupported(PARTIAL_COVER),
            Paint::Unknown(reason) => Backdrop::Unsupported(reason),
        };
    }
    Backdrop::Solid(background.to_owned())
}

fn overlaps(first: Rect, second: Rect) -> bool {
    first.x0 < second.x1 - 0.01 && second.x0 < first.x1 - 0.01 && first.y0 < second.y1 - 0.01 && second.y0 < first.y1 - 0.01
}

// What a line-drawing element strokes, in its own coordinates: a connector, or a shape that has no fill and so draws only its
// outline. The route is None for an outline that is not measured (a custom path, a preset not listed).
struct Stroke { width: f64, route: Option<Vec<[f64; 2]>>, miter: bool, arrowheads: [bool; 2] }

fn stroke_of(element: &Element, style: &VisualStyle, width: f64, height: f64) -> Option<Stroke> {
    match element {
        Element::Connector { color, stroke_width, routing, flip_v, arrow, .. } if color != "none" && *stroke_width > 0.0 => {
            let points = routing.as_ref().map(|route| route.points.clone()).unwrap_or_else(|| vec![[0.0, if *flip_v { 1.0 } else { 0.0 }], [1.0, if *flip_v { 0.0 } else { 1.0 }]]);
            let route = points.iter().map(|point| [point[0] * width, point[1] * height]).collect();
            Some(Stroke { width: *stroke_width, route: Some(route), miter: true, arrowheads: [routing.as_ref().is_some_and(|route| route.start_arrow), *arrow] })
        }
        Element::Shape { preset, stroke, stroke_width, fill, .. } if fill == "none" && stroke != "none" && *stroke_width > 0.0 => {
            Some(Stroke { width: *stroke_width, route: if style.path.is_some() { None } else { outline(preset, style, width, height) }, miter: false, arrowheads: [false; 2] })
        }
        Element::Polygon { points, stroke, stroke_width, fill, .. } if fill == "none" && stroke != "none" && *stroke_width > 0.0 => {
            let route = if style.path.is_some() { None } else { Some(points.iter().chain(points.first()).map(|point| [point[0] * width, point[1] * height]).collect()) };
            Some(Stroke { width: *stroke_width, route, miter: false, arrowheads: [false; 2] })
        }
        _ => None,
    }
}

// The painter a line leaves. A line is no backdrop of its own; it matters where its stroke crosses a text frame. An outline that is
// measured is followed segment by segment (see `quads`); any other covers its whole frame with something that is not one color.
fn line_painter(element: &Element, style: &VisualStyle, transform: Affine, frame: Rect, width: f64, height: f64) -> Option<Painter> {
    let stroke = stroke_of(element, style, width, height)?;
    let Some(route) = &stroke.route else {
        let [a, b, c, d, _, _] = transform.as_coeffs();
        let reach = stroke.width * a.hypot(b).max(c.hypot(d));
        return Some(Painter { frame: frame.inflate(reach, reach), paint: Paint::Unknown(UNSUPPORTED_BACKDROP) });
    };
    let quads = quads(&stroke, route, transform);
    let (mut low, mut high) = (Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
    for corner in quads.iter().flatten() { low = Point::new(low.x.min(corner.x), low.y.min(corner.y)); high = Point::new(high.x.max(corner.x), high.y.max(corner.y)); }
    (!quads.is_empty()).then(|| Painter { frame: Rect::new(low.x, low.y, high.x, high.y), paint: Paint::Line(quads) })
}

// The outline of a shape in its own coordinates, for the presets this module can follow.
fn outline(preset: &str, style: &VisualStyle, width: f64, height: f64) -> Option<Vec<[f64; 2]>> {
    Some(match preset {
        "rect" => vec![[0.0, 0.0], [width, 0.0], [width, height], [0.0, height], [0.0, 0.0]],
        "roundRect" => {
            let adjustment = style.adjustments.iter().find(|adjustment| adjustment.name == "adj").map_or(16667.0, |adjustment| f64::from(adjustment.value)).clamp(0.0, 50000.0);
            let radius = (width.min(height) * adjustment / 100000.0).min(width / 2.0).min(height / 2.0);
            let corners = [([width - radius, radius], -std::f64::consts::FRAC_PI_2), ([width - radius, height - radius], 0.0), ([radius, height - radius], std::f64::consts::FRAC_PI_2), ([radius, radius], std::f64::consts::PI)];
            let mut route: Vec<[f64; 2]> = corners.iter().flat_map(|(center, start)| (0..=8).map(move |step| { let angle = start + std::f64::consts::FRAC_PI_2 * f64::from(step) / 8.0; [center[0] + radius * angle.cos(), center[1] + radius * angle.sin()] })).collect();
            route.push(route[0]);
            route
        }
        "ellipse" => (0..=64).map(|step| { let angle = f64::from(step) / 64.0 * std::f64::consts::TAU; [width / 2.0 * (1.0 + angle.cos()), height / 2.0 * (1.0 + angle.sin())] }).collect(),
        _ => return None,
    })
}

// The stroke as the renderer draws it, in page coordinates, covered by quadrilaterals: a band as wide as the line along each segment, a
// square at every join and a rectangle around each arrowhead. The squares and rectangles hold the paint but are larger than it, so text a
// few pixels from a corner or an arrowhead can be reported although the paint does not reach it. A shape joins round, so its squares reach
// half a line width, the radius of the join. A connector joins with a miter: the renderer keeps SVG's miter limit of 4, which caps the
// miter's length at four line widths, so its tip lies at most two line widths (four half-widths) from the vertex and its squares reach
// that far. The ends of an open route are flat. `the_quads_of_a_line_hold_every_pixel_the_renderer_inks` keeps all of this in step with the renderer.
fn quads(stroke: &Stroke, route: &[[f64; 2]], transform: Affine) -> Vec<[Point; 4]> {
    let half_width = stroke.width / 2.0;
    let closed = route.len() > 2 && route.first() == route.last();
    let page = |corners: [[f64; 2]; 4]| corners.map(|corner| transform * Point::new(corner[0], corner[1]));
    let mut quads: Vec<[Point; 4]> = route.windows(2).filter_map(|pair| {
        let (dx, dy) = (pair[1][0] - pair[0][0], pair[1][1] - pair[0][1]);
        let length = dx.hypot(dy);
        (length > 1e-9).then(|| {
            let (nx, ny) = (-dy / length * half_width, dx / length * half_width);
            page([[pair[0][0] + nx, pair[0][1] + ny], [pair[1][0] + nx, pair[1][1] + ny], [pair[1][0] - nx, pair[1][1] - ny], [pair[0][0] - nx, pair[0][1] - ny]])
        })
    }).collect();
    for (index, point) in route.iter().enumerate() {
        // The ends of an open route are flat, so they add nothing beyond the band; every other vertex is a join.
        if !closed && (index == 0 || index + 1 == route.len()) { continue; }
        let reach = if stroke.miter { half_width * 4.0 } else { half_width };
        quads.push(page([[point[0] - reach, point[1] - reach], [point[0] + reach, point[1] - reach], [point[0] + reach, point[1] + reach], [point[0] - reach, point[1] + reach]]));
    }
    // The renderer draws an arrowhead four line widths long and 3.2 line widths wide, its tip on the end of the route.
    let ends = [(stroke.arrowheads[0], route.get(1), route.first()), (stroke.arrowheads[1], route.len().checked_sub(2).and_then(|index| route.get(index)), route.last())];
    for (drawn, from, tip) in ends {
        let (true, Some(from), Some(tip)) = (drawn, from, tip) else { continue; };
        let (dx, dy) = (tip[0] - from[0], tip[1] - from[1]);
        let distance = dx.hypot(dy);
        if distance < 1e-9 { continue; }
        let (ux, uy) = (dx / distance, dy / distance);
        let (length, half) = (stroke.width * 4.0, stroke.width * 1.6);
        let (nx, ny) = (-uy * half, ux * half);
        let (bx, by) = (tip[0] - ux * length, tip[1] - uy * length);
        quads.push(page([[bx + nx, by + ny], [tip[0] + nx, tip[1] + ny], [tip[0] - nx, tip[1] - ny], [bx - nx, by - ny]]));
    }
    quads
}

// The shadow, glow and reflection of an element reach past its frame, out to the extent the renderer draws them. The paint there is
// not one color, so text over it is unsupported unless a shape above the effect covers the text.
fn effect_painter(element: &Element, style: &VisualStyle, transform: Affine) -> Option<Painter> {
    if style.shadow.is_none() && style.glow.is_none() && style.reflection.is_none() { return None; }
    let [x, y, width, height] = crate::render::effect_bounds(element);
    Some(Painter { frame: transform.transform_rect_bbox(Rect::new(x, y, x + width, y + height)), paint: Paint::Unknown(BACKGROUND_NOT_SOLID) })
}

// Separating-axis test: the quadrilateral and the frame share some area. Touching along an edge or at a corner does not count.
fn meets(quad: &[Point; 4], frame: Rect) -> bool {
    let corners = [Point::new(frame.x0, frame.y0), Point::new(frame.x1, frame.y0), Point::new(frame.x1, frame.y1), Point::new(frame.x0, frame.y1)];
    let normal = |from: Point, to: Point| { let (dx, dy) = (to.x - from.x, to.y - from.y); let length = dx.hypot(dy); (-dy / length, dx / length) };
    let axes = [(1.0, 0.0), (0.0, 1.0), normal(quad[0], quad[1]), normal(quad[1], quad[2])];
    axes.iter().all(|&(x, y)| {
        let span = |points: &[Point]| points.iter().map(|point| point.x * x + point.y * y).fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| (low.min(value), high.max(value)));
        let (frame_low, frame_high) = span(&corners);
        let (quad_low, quad_high) = span(quad);
        frame_high.min(quad_high) - frame_low.max(quad_low) > 0.01
    })
}

fn own_paint<'a>(element: &'a Element, style: &VisualStyle, fill: &Fill) -> OwnPaint<'a> {
    let Element::Shape { width, height, format, .. } = element else { return OwnPaint::Nothing; };
    match fill {
        Fill::Nothing => OwnPaint::Nothing,
        Fill::Unknown(reason) => OwnPaint::Unknown(reason),
        Fill::Solid(color) => {
            // The renderer clips a shape's text to this frame, so no glyph is drawn outside it.
            let [x, y, text_width, text_height] = format.content_frame(*width, *height, true);
            OwnPaint::Solid(Own { color: color.clone(), shape: element, outline: geometry(element, style, Affine::IDENTITY, Rect::new(0.0, 0.0, *width, *height)), frame: Rect::new(x, y, x + text_width, y + text_height) })
        }
    }
}

impl Own<'_> {
    /// The backdrop when the text frame alone decides it: the outline holds the whole frame, or it cannot be measured at all.
    /// `None` when only the laid-out text can tell.
    fn settle(&self) -> Option<Backdrop> {
        if self.outline.covers(self.frame) { Some(Backdrop::Solid(self.color.clone())) }
        else if matches!(self.outline, Geometry::Other) { Some(Backdrop::Unsupported(UNSUPPORTED_BACKDROP)) }
        else { None }
    }

    /// The frame reaches outside the outline, as it does for centered text in a circle or a pill, so the fill is the backdrop
    /// only if the text as it is laid out stays inside.
    fn settle_by_text(&self, extent: Option<(f64, f64)>) -> Backdrop {
        match extent.and_then(|(width, height)| laid_out_text(self.shape, self.frame, width, height)) {
            Some(text) if self.outline.covers(text) => Backdrop::Solid(self.color.clone()),
            _ => Backdrop::Unsupported(PARTIAL_COVER),
        }
    }
}

/// Where the text of a shape is drawn inside its content frame, from the size it measures and how it is aligned. `None` when
/// that is not known for sure: a warped shape, or paragraphs that carry tabs.
fn laid_out_text(shape: &Element, frame: Rect, width: f64, height: f64) -> Option<Rect> {
    let Element::Shape { text, font_size, visual, format, .. } = shape else { return None; };
    if visual.as_ref().is_some_and(|style| style.text_warp.is_some()) { return None; }
    let (mirrored_x, mirrored_y) = visual.as_ref().map_or((false, false), |style| (style.flip_h, style.flip_v));
    let paragraphs = crate::rich_text::paragraphs(text, format);
    if paragraphs.iter().any(|paragraph| !paragraph.tabs.is_empty() || paragraph.runs.iter().any(|run| run.text.contains('\t'))) { return None; }
    let alignment = paragraphs.first()?.alignment.unwrap_or(format.alignment);
    // Lines of one alignment and no indent share one edge or center; anything else may use the frame's whole width. A mirrored
    // shape draws its text mirrored too, so along the mirrored axis the whole frame is allowed.
    let shared = paragraphs.iter().all(|paragraph| paragraph.alignment.unwrap_or(format.alignment) == alignment && crate::rich_text::paragraph_insets(paragraph, format, *font_size) == (0.0, 0.0));
    let (width, height) = (width.min(frame.width()), height.min(frame.height()));
    let (left, right) = match alignment {
        _ if !shared || mirrored_x => (frame.x0, frame.x1),
        TextAlign::Left => (frame.x0, frame.x0 + width),
        TextAlign::Center => (frame.x0 + (frame.width() - width) / 2.0, frame.x0 + (frame.width() + width) / 2.0),
        TextAlign::Right => (frame.x1 - width, frame.x1),
        TextAlign::Justify => (frame.x0, frame.x1),
    };
    let (top, bottom) = match format.vertical {
        _ if mirrored_y => (frame.y0, frame.y1),
        VerticalAlign::Top => (frame.y0, frame.y0 + height),
        VerticalAlign::Middle => (frame.y0 + (frame.height() - height) / 2.0, frame.y0 + (frame.height() + height) / 2.0),
        VerticalAlign::Bottom => (frame.y1 - height, frame.y1),
    };
    // A pixel of slack absorbs the difference between measuring and drawing.
    Some(Rect::new(left, top, right, bottom).inflate(1.0, 1.0).intersect(frame))
}

impl Geometry {
    // Every corner of the frame, pulled in by the tolerance, lies inside the shape. These shapes are convex, so the corners decide.
    fn covers(&self, text: Rect) -> bool {
        let (shrink_x, shrink_y) = (TOLERANCE.min(text.width() / 2.0), TOLERANCE.min(text.height() / 2.0));
        let inner = Rect::new(text.x0 + shrink_x, text.y0 + shrink_y, text.x1 - shrink_x, text.y1 - shrink_y);
        [Point::new(inner.x0, inner.y0), Point::new(inner.x1, inner.y0), Point::new(inner.x1, inner.y1), Point::new(inner.x0, inner.y1)].into_iter().all(|corner| match self {
            Self::Rect(frame) => corner.x >= frame.x0 && corner.x <= frame.x1 && corner.y >= frame.y0 && corner.y <= frame.y1,
            Self::RoundRect(frame, radius) => {
                if corner.x < frame.x0 || corner.x > frame.x1 || corner.y < frame.y0 || corner.y > frame.y1 { return false; }
                let radius = radius.min(frame.width() / 2.0).min(frame.height() / 2.0);
                let nearest = Point::new(corner.x.clamp(frame.x0 + radius, frame.x1 - radius), corner.y.clamp(frame.y0 + radius, frame.y1 - radius));
                (corner.x - nearest.x).hypot(corner.y - nearest.y) <= radius
            }
            Self::Ellipse(frame) => {
                let (radius_x, radius_y) = (frame.width() / 2.0, frame.height() / 2.0);
                radius_x > 0.0 && radius_y > 0.0 && ((corner.x - frame.x0 - radius_x) / radius_x).powi(2) + ((corner.y - frame.y0 - radius_y) / radius_y).powi(2) <= 1.0
            }
            Self::Convex(points) => {
                let orientation = signed_area(points).signum();
                (0..points.len()).all(|index| {
                    let (start, end) = (points[index], points[(index + 1) % points.len()]);
                    ((end.x - start.x) * (corner.y - start.y) - (end.y - start.y) * (corner.x - start.x)) * orientation >= 0.0
                })
            }
            Self::Other => false,
        })
    }
}

fn signed_area(points: &[Point]) -> f64 {
    (0..points.len()).map(|index| { let (start, end) = (points[index], points[(index + 1) % points.len()]); start.x * end.y - end.x * start.y }).sum::<f64>() / 2.0
}

// All turns go the same way and the outline winds once, so a bow-tie or a star is not treated as convex.
fn is_convex(points: &[Point]) -> bool {
    let count = points.len();
    if count < 3 || signed_area(points).abs() < 1e-9 { return false; }
    let mut turn = 0.0;
    let mut total = 0.0;
    for index in 0..count {
        let (first, second, third) = (points[index], points[(index + 1) % count], points[(index + 2) % count]);
        let (before, after) = ((second.x - first.x, second.y - first.y), (third.x - second.x, third.y - second.y));
        let cross = before.0 * after.1 - before.1 * after.0;
        if cross.abs() > 1e-9 {
            if turn != 0.0 && cross.signum() != turn { return false; }
            turn = cross.signum();
        }
        total += cross.atan2(before.0 * after.0 + before.1 * after.1);
    }
    (total.abs() - std::f64::consts::TAU).abs() < 1e-6
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    // The quads follow the renderer's stroke by hand: SVG's miter limit, which the renderer leaves at its default of 4, and the size of an
    // arrowhead. Keep them in step: every pixel the renderer inks must lie in a quad, at corners sharp enough for the renderer to cut the
    // miter off and at corners that are not, and in a group that scales unevenly and turns.
    #[test]
    fn the_quads_of_a_line_hold_every_pixel_the_renderer_inks() {
        let connector = |id: &str, bounds: [f64; 4], width: f64, route: &[[f64; 2]]| json!({"type":"connector","id":id,"x":bounds[0],"y":bounds[1],"width":bounds[2],"height":bounds[3],
            "color":"000000","stroke_width":width,"arrow":true,"routing":{"points":route,"start_arrow":true,"custom":true}});
        // Two arms of 90 px meet at the apex (100, 20) of a box 200 px square, at the angle given, and each ends in an arrowhead.
        let mut elements: Vec<Value> = [10.0_f64, 25.0, 30.0, 60.0, 90.0, 150.0].iter().enumerate().map(|(index, angle)| {
            let (sin, cos) = angle.to_radians().sin_cos();
            let route: Vec<[f64; 2]> = [[10.0, 20.0], [100.0, 20.0], [100.0 - 90.0 * cos, 20.0 + 90.0 * sin]].iter().map(|[x, y]| [x / 200.0, y / 200.0]).collect();
            connector(&format!("angle-{angle}"), [40.0 + 420.0 * (index % 3) as f64, 30.0 + 240.0 * (index / 3) as f64, 200.0, 200.0], 10.0, &route)
        }).collect();
        // The group draws what it holds 2.5 times as wide and 1.5 times as high as authored, and turns it 20 degrees.
        elements.push(json!({"type":"group","id":"uneven","x":60,"y":520,"width":250,"height":150,"view_width":100,"view_height":100,"visual":{"rotation":20},
            "children":[connector("inner", [10.0, 10.0, 80.0, 80.0], 4.0, &[[0.0, 0.1], [1.0, 0.5], [0.0, 0.9]])]}));
        let deck: Deck = serde_json::from_value(json!({"version":1,"title":"Synthetic","width":1280,"height":720,
            "slides":[{"id":"s1","title":"Synthetic","background":"FFFFFF","notes":"","elements":elements}]})).unwrap();
        crate::model::validate_deck(&deck).unwrap();
        let mut painters = Vec::new();
        walk(&deck.slides[0].elements, &Context { transform: Affine::IDENTITY, faded: false, hidden: false }, true, &mut painters, &mut Vec::new());
        let quads: Vec<&[Point; 4]> = painters.iter().filter_map(|painter| match &painter.paint { Paint::Line(quads) => Some(quads), _ => None }).flatten().collect();
        let svg = crate::render::render_validated(&deck, 0, false).unwrap().svg;
        let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(deck.width, deck.height).unwrap();
        pixmap.fill(resvg::tiny_skia::Color::WHITE);
        resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pixmap.as_mut());
        // The lines are black on white, so a pixel darker than 192 has a quarter or more of its area under a line, and a quad that holds the line meets it.
        let ink: Vec<(u32, u32)> = (0..deck.height).flat_map(|y| (0..deck.width).map(move |x| (x, y))).filter(|&(x, y)| pixmap.pixel(x, y).is_some_and(|pixel| pixel.red() < 192)).collect();
        assert!(ink.len() > 5_000, "only {} pixels are inked", ink.len());
        // The corner of 30 degrees at (980, 50) is mitered, its tip near (998.6, 45); a bevel would leave (994, 46) empty.
        assert!(ink.contains(&(994, 46)), "the renderer no longer miters a corner of 30 degrees");
        let outside: Vec<&(u32, u32)> = ink.iter().filter(|&&(x, y)| {
            let pixel = Rect::new(f64::from(x), f64::from(y), f64::from(x) + 1.0, f64::from(y) + 1.0);
            !quads.iter().any(|quad| meets(quad, pixel))
        }).collect();
        assert!(outside.is_empty(), "{} inked pixels lie outside every quad, the first at {:?}", outside.len(), outside.first());
    }

    // The text block is worked out from the measured size and the alignment; the renderer alone decides where glyphs land. Keep the
    // two in step: every glyph the renderer draws must lie inside the block, give or take the em-box proxy `occupied_bounds` uses.
    #[test]
    fn the_laid_out_text_holds_every_glyph_the_renderer_draws() {
        let padding = |left: f64, top: f64| json!({"left":left,"right":left,"top":top,"bottom":top});
        let cases: Vec<(String, Value)> = vec![
            ("12".into(), json!({"alignment":"center","vertical":"middle","padding":padding(0.0, 0.0)})),
            ("Synthetic".into(), json!({})),
            ("Synthetic text".into(), json!({"alignment":"right","vertical":"bottom","padding":padding(10.0, 6.0)})),
            ("First line\nSecond".into(), json!({"alignment":"center","vertical":"middle"})),
            ("Synthetic ".repeat(12), json!({"alignment":"center","vertical":"top"})),
            ("日本語の見出しです".into(), json!({"alignment":"center","vertical":"middle"})),
            ("Short\nA much longer second line of text".into(), json!({"alignment":"left","vertical":"middle","padding":padding(8.0, 4.0)})),
        ];
        for (text, format) in cases {
            let shape = json!({"type":"shape","id":"label","x":100,"y":100,"width":320,"height":120,"preset":"rect","fill":"0B1F3A","stroke":"0B1F3A","stroke_width":1,
                "text":text,"font_size":18,"color":"FFFFFF","bold":false,"format":format});
            let deck: Deck = serde_json::from_value(json!({"version":1,"title":"Synthetic","width":1280,"height":720,
                "slides":[{"id":"s1","title":"Synthetic","background":"FFFFFF","notes":"","elements":[shape]}]})).unwrap();
            crate::model::validate_deck(&deck).unwrap();
            let slide = &deck.slides[0];
            let element = &slide.elements[0];
            let Element::Shape { width, height, format, .. } = element else { unreachable!() };
            let [x, y, frame_width, frame_height] = format.content_frame(*width, *height, true);
            let frame = Rect::new(x, y, x + frame_width, y + frame_height);
            let drawn = crate::render::render_validated(&deck, 0, false).unwrap().text.iter()
                .find(|item| item.element_id == "label").and_then(|item| item.occupied_bounds()).expect("the renderer draws the text");
            let (measured_width, measured_height) = crate::layout::text_extents(&deck, slide, &[element])[0].expect("installed fonts measure the text");
            let block = laid_out_text(element, frame, measured_width, measured_height).expect("plain aligned text has a known block");
            let slack = 0.2 * 18.0;
            assert!(block.x0 - 1.0 <= drawn.x0 && drawn.x1 <= block.x1 + 1.0 && block.y0 - slack <= drawn.y0 && drawn.y1 <= block.y1 + slack, "{text:?}: drawn {drawn:?} outside block {block:?}");
        }
    }
}
