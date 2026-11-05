//! The background behind each text element, for `check_accessibility`.
//!
//! Layers are the master, layout and slide in the order the renderer paints them, so a nested group, an inherited slide
//! background and an inherited graphic are read the way they are drawn. Text gets a background only when the paint behind it
//! is known exactly: its own solid fill, the topmost opaque solid shape below it that covers its whole frame, or else the
//! background the renderer paints for the slide. A partial cover, a picture, chart or table, a gradient, transparency, a soft
//! edge, a shape this module cannot measure, or rotation and skew when the background has to be found behind the text is
//! reported with a reason. A shape's own fill sits behind its text however the shape turns, so a turned shape keeps it.
//! Nothing is guessed to be white.
use crate::{model::{Deck, Element}, visual::VisualStyle};
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

enum Paint { Solid(String, Geometry), Unknown(&'static str) }

struct Painter { frame: Rect, paint: Paint }

#[derive(Clone)]
enum Fill { Nothing, Solid(String), Unknown(&'static str) }

struct Site { id: String, before: usize, frame: Rect, scale: f64, aligned: bool, faded: bool, own: Fill }

struct Context { transform: Affine, faded: bool, hidden: bool }

/// The backdrop of every text-bearing element of the slide at `page`, by element ID.
pub(crate) fn text_backdrops(deck: &Deck, page: usize) -> BTreeMap<String, TextBackdrop> {
    let Some(slide) = deck.slides.get(page) else { return BTreeMap::new(); };
    let Ok(layers) = crate::render::render_layers(deck, page) else { return BTreeMap::new(); };
    let background = crate::design::rendered_background(slide, deck.design.as_ref());
    let mut painters = Vec::new();
    let mut sites = Vec::new();
    let context = Context { transform: Affine::IDENTITY, faded: false, hidden: false };
    for (index, (_, elements)) in layers.iter().enumerate() { walk(elements, &context, index + 1 == layers.len(), &mut painters, &mut sites); }
    sites.into_iter().map(|site| {
        let backdrop = backdrop(&site, &painters, background);
        (site.id, TextBackdrop { scale: site.scale, backdrop })
    }).collect()
}

fn walk(elements: &[Element], parent: &Context, slide_layer: bool, painters: &mut Vec<Painter>, sites: &mut Vec<Site>) {
    for element in elements {
        let (id, x, y, width, height) = element.bounds();
        let none = VisualStyle::default();
        let style = element.visual().unwrap_or(&none);
        let rotation = match element { Element::Shape { rotation, .. } => *rotation, _ => style.rotation.unwrap_or(0.0) };
        let transform = parent.transform * Affine::translate((x, y)) * Affine::translate((width / 2.0, height / 2.0))
            * Affine::rotate(rotation.to_radians())
            * Affine::scale_non_uniform(if style.flip_h { -1.0 } else { 1.0 }, if style.flip_v { -1.0 } else { 1.0 })
            * Affine::translate((-width / 2.0, -height / 2.0));
        let faded = parent.faded || style.soft_edge.is_some_and(|radius| radius > 0.0);
        let hidden = parent.hidden || style.hidden;
        let frame = transform.transform_rect_bbox(Rect::new(0.0, 0.0, width, height));
        let [a, b, c, d, _, _] = transform.as_coeffs();
        let aligned = b.abs() < 1e-9 && c.abs() < 1e-9 && a.abs() > 1e-9 && d.abs() > 1e-9;
        let fill = fill_of(element, style, faded);
        if slide_layer {
            if let Element::Text { text, .. } | Element::Shape { text, .. } = element {
                if !text.trim().is_empty() {
                    sites.push(Site { id: id.to_owned(), before: painters.len(), frame, scale: (a.hypot(b).min(c.hypot(d)) * 1e6).round() / 1e6, aligned, faded,
                        own: if matches!(element, Element::Shape { .. }) { fill.clone() } else { Fill::Nothing } });
                }
            }
        }
        if let Element::Group { view_width, view_height, children, .. } = element {
            if *view_width > 0.0 && *view_height > 0.0 {
                let context = Context { transform: transform * Affine::scale_non_uniform(width / view_width, height / view_height), faded, hidden };
                walk(children, &context, slide_layer, painters, sites);
            }
            continue;
        }
        if hidden { continue; }
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

fn backdrop(site: &Site, painters: &[Painter], background: &str) -> Backdrop {
    match &site.own {
        Fill::Solid(color) => return Backdrop::Solid(color.clone()),
        Fill::Unknown(reason) => return Backdrop::Unsupported(reason),
        Fill::Nothing => {}
    }
    if !site.aligned { return Backdrop::Unsupported(TRANSFORMED_TEXT); }
    if site.faded { return Backdrop::Unsupported(BACKGROUND_NOT_SOLID); }
    for painter in painters[..site.before].iter().rev() {
        if !overlaps(painter.frame, site.frame) { continue; }
        return match &painter.paint {
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
