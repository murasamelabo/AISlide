//! Native visual subset shared by authoring and preservation saves.
//!
//! Transform order is DrawingML's center-based flips/rotation; group children
//! remain in the group's view coordinate space. Rotation is -360..=360 degrees.
//! Hidden maps to cNvPr.hidden; locked sets noMove/noResize/noRot, not an ACL.
//! Gradients have 2-16 ordered stops; base fill equals the first stop color.
//! Linear angles and shadow directions must round below 360 degrees. Radial
//! center [cx,cy] maps to fillToRect [cx,cy,1-cx,1-cy] with path="circle".
//! Distances/radii are pixels (9525 EMU); angles use 60000 units/degree;
//! opacity/stop offsets use 100000 units. Native reopen reflects quantization.
//! Shadow/glow/soft-edge blur/radius is 0..=100px, distance 0..=200px.
//! Reflection is the fixed downward, vertically mirrored bottom-left preset;
//! only blur, distance, start/end alpha and fade end position are adjustable.
//! Text warps are eight validated default-adjustment bodyPr presets;
//! shape effects are not rich-run WordArt fills/outlines or 3D text.
//! Shape adj is limited to roundRect [0,50000], chevron/triangle [0,100000].
//! Picture masks are native prstGeom plus the existing srcRect crop.
//! None/empty visuals preserve authored legacy XML; unsupported native subtrees
//! remain raw and replacements fail closed. Office and UI rendering still need
//! independent qualification. No clipping/boolean geometry engine is provided.
use crate::{model::Element, native::{A, P, child, properties}, Error, Result};
use serde::{Deserialize, Serialize};
use roxmltree::Node;
use std::{collections::BTreeMap, ops::Range};

/// Optional DrawingML properties. Angles are degrees, distances are scene pixels,
/// alpha and gradient offsets are 0..=1. Shape.rotation remains authoritative for
/// Shape; Connector routing owns its flips. Opacity affects shape fill or picture
/// pixels, not text, outlines, or an entire group. None preserves legacy output.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VisualStyle {
    #[serde(skip_serializing_if = "Option::is_none")] pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")] pub flip_h: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")] pub flip_v: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")] pub hidden: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")] pub locked: bool,
    #[serde(skip_serializing_if = "Option::is_none")] pub opacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub gradient: Option<Gradient>,
    #[serde(skip_serializing_if = "Option::is_none")] pub shadow: Option<Shadow>,
    #[serde(skip_serializing_if = "Option::is_none")] pub glow: Option<Glow>,
    #[serde(skip_serializing_if = "Option::is_none")] pub soft_edge: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub reflection: Option<Reflection>,
    #[serde(skip_serializing_if = "Option::is_none")] pub text_warp: Option<TextWarp>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub adjustments: Vec<ShapeAdjustment>,
    #[serde(skip_serializing_if = "Option::is_none")] pub picture_mask: Option<PictureMask>,
    #[serde(skip_serializing_if = "Option::is_none")] pub path: Option<crate::vector::VectorPath>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub connection_sites: Vec<ConnectionSite>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionSite { pub x: f64, pub y: f64, pub angle: f64 }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GradientStop { pub offset: f64, pub color: String, pub opacity: f64 }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Gradient {
    Linear { angle: f64, stops: Vec<GradientStop> },
    Radial { center: [f64; 2], stops: Vec<GradientStop> },
}
impl Gradient { pub fn stops(&self) -> &[GradientStop] { match self { Self::Linear { stops, .. } | Self::Radial { stops, .. } => stops } } }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shadow { pub color: String, pub opacity: f64, pub blur: f64, pub distance: f64, pub angle: f64 }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Glow { pub color: String, pub opacity: f64, pub radius: f64 }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reflection { pub blur: f64, pub distance: f64, pub start_opacity: f64, pub end_opacity: f64, pub end_position: f64 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextWarp { ArchUp, ArchDown, Wave1, Wave2, Inflate, Deflate, SlantUp, SlantDown }
impl TextWarp {
    pub const ALL: [Self; 8] = [Self::ArchUp, Self::ArchDown, Self::Wave1, Self::Wave2, Self::Inflate, Self::Deflate, Self::SlantUp, Self::SlantDown];
    pub fn preset(self) -> &'static str { match self { Self::ArchUp => "textArchUp", Self::ArchDown => "textArchDown", Self::Wave1 => "textWave1", Self::Wave2 => "textWave2", Self::Inflate => "textInflate", Self::Deflate => "textDeflate", Self::SlantUp => "textSlantUp", Self::SlantDown => "textSlantDown" } }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShapeAdjustment { pub name: String, pub value: i32 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PictureMask { Ellipse, RoundRect, Diamond, Hexagon }
impl PictureMask { pub fn preset(self) -> &'static str { match self { Self::Ellipse => "ellipse", Self::RoundRect => "roundRect", Self::Diamond => "diamond", Self::Hexagon => "hexagon" } } }

pub fn sample_slide_pixel(document: &crate::document::Document, slide_id: &str, x: u32, y: u32) -> Result<serde_json::Value> {
    crate::document::verify(document)?;
    let deck = &document.deck;
    if x>=deck.width || y>=deck.height { return Err(Error::Invalid("sample coordinates must be inside the slide".into())); }
    let index = deck.slides.iter().position(|slide| slide.id==slide_id).ok_or_else(|| Error::Invalid("sample slide not found".into()))?;
    let rendered = crate::export_static::export_static(deck,&crate::export_static::ExportOptions { page_indices:Some(vec![index]),..Default::default() })?;
    let artifact = rendered.artifacts.first().ok_or_else(|| Error::Invalid("sample render missing".into()))?;
    let image = image::load_from_memory_with_format(&artifact.bytes,image::ImageFormat::Png).map_err(|_| Error::Invalid("sample raster decode failed".into()))?.to_rgba8();
    let pixel = image.get_pixel(x,y).0;
    Ok(serde_json::json!({"x":x,"y":y,"color":format!("{:02X}{:02X}{:02X}",pixel[0],pixel[1],pixel[2]),"rgba":pixel,"warnings":rendered.warnings,"office_parity_verified":false}))
}

impl Element {
    pub fn visual(&self) -> Option<&VisualStyle> { match self {
        Self::Text { visual, .. } | Self::Rect { visual, .. } | Self::Polygon { visual, .. } | Self::Shape { visual, .. }
        | Self::Picture { visual, .. } | Self::Group { visual, .. } | Self::Connector { visual, .. } => visual.as_ref(),
        _ => None,
    } }
    pub fn visual_mut(&mut self) -> Option<&mut Option<VisualStyle>> { match self {
        Self::Text { visual, .. } | Self::Rect { visual, .. } | Self::Polygon { visual, .. } | Self::Shape { visual, .. }
        | Self::Picture { visual, .. } | Self::Group { visual, .. } | Self::Connector { visual, .. } => Some(visual),
        _ => None,
    } }
}

fn bounded(value: f64, minimum: f64, maximum: f64) -> Result<()> {
    if !value.is_finite() || !(minimum..=maximum).contains(&value) { return Err(Error::Invalid(format!("visual value must be finite in {minimum}..={maximum}"))); } Ok(())
}
fn positive_angle(value: f64) -> Result<()> {
    bounded(value, 0.0, 360.0)?;
    if (value * 60000.0).round() >= 21600000.0 { return Err(Error::Invalid("DrawingML positive fixed angle must round below 360 degrees".into())); }
    Ok(())
}
pub fn validate(element: &Element) -> Result<()> {
    let Some(style) = element.visual() else { return Ok(()); };
    if let Some(rotation) = style.rotation { bounded(rotation, -360.0, 360.0)?; if matches!(element, Element::Shape { .. } | Element::Connector { .. }) { return Err(Error::Invalid("use Shape.rotation or connector routing, not visual.rotation".into())); } }
    if matches!(element, Element::Connector { .. }) && (style.flip_h || style.flip_v) { return Err(Error::Invalid("connector flips belong to routing/flip_v".into())); }
    if matches!(element, Element::Text { format, .. } if format.inherit_layout) && style != &VisualStyle::default() { return Err(Error::Unsupported("detach inherited text before applying visuals".into())); }
    let fill = match element { Element::Rect { fill, .. } | Element::Polygon { fill, .. } | Element::Shape { fill, .. } => Some(fill.as_str()), _ => None };
    if let Some(opacity) = style.opacity {
        bounded(opacity, 0.0, 1.0)?;
        if fill == Some("none") || (fill.is_none() && !matches!(element, Element::Picture { .. })) { return Err(Error::Unsupported("opacity requires a filled shape or picture".into())); }
    }
    if let Some(gradient) = &style.gradient {
        let stops = gradient.stops();
        if !(2..=16).contains(&stops.len()) || stops.windows(2).any(|pair| pair[0].offset > pair[1].offset) { return Err(Error::Invalid("gradient needs 2-16 ordered stops".into())); }
        if fill != Some(stops[0].color.as_str()) || style.opacity.is_some() { return Err(Error::Invalid("gradient first color must equal base fill; use stop opacity instead of visual.opacity".into())); }
        for stop in stops { bounded(stop.offset, 0.0, 1.0)?; bounded(stop.opacity, 0.0, 1.0)?; crate::model::valid_color(&stop.color)?; }
        match gradient { Gradient::Linear { angle, .. } => positive_angle(*angle)?, Gradient::Radial { center, .. } => { for value in center { bounded(*value, 0.0, 1.0)?; } } }
    }
    if let Some(shadow) = &style.shadow { crate::model::valid_color(&shadow.color)?; bounded(shadow.opacity, 0.0, 1.0)?; bounded(shadow.blur, 0.0, 100.0)?; bounded(shadow.distance, 0.0, 200.0)?; positive_angle(shadow.angle)?; }
    if let Some(glow) = &style.glow { crate::model::valid_color(&glow.color)?; bounded(glow.opacity, 0.0, 1.0)?; bounded(glow.radius, 0.0, 100.0)?; }
    if let Some(radius) = style.soft_edge { bounded(radius, 0.0, 100.0)?; }
    if let Some(reflection) = &style.reflection { bounded(reflection.blur, 0.0, 100.0)?; bounded(reflection.distance, 0.0, 200.0)?; for value in [reflection.start_opacity, reflection.end_opacity, reflection.end_position] { bounded(value, 0.0, 1.0)?; } }
    if style.text_warp.is_some() && !matches!(element, Element::Text { .. } | Element::Shape { .. }) { return Err(Error::Invalid("text warp requires a text body".into())); }
    if !style.adjustments.is_empty() {
        let Element::Shape { preset, .. } = element else { return Err(Error::Invalid("adjustments require a preset shape".into())); };
        let maximum = match preset.as_str() { "roundRect" => 50000, "chevron" | "triangle" => 100000, _ => return Err(Error::Unsupported("adjustments supported only for roundRect, chevron, triangle".into())) };
        if style.adjustments.len() != 1 || style.adjustments[0].name != "adj" || !(0..=maximum).contains(&style.adjustments[0].value) { return Err(Error::Invalid("preset adjustment must be one bounded adj value".into())); }
    }
    if style.picture_mask.is_some() && !matches!(element, Element::Picture { .. }) { return Err(Error::Invalid("picture mask requires a picture".into())); }
    if !style.connection_sites.is_empty() {
        let Element::Shape { preset, .. } = element else { return Err(Error::Invalid("custom connection sites require a supported semantic Shape".into())); };
        crate::vector::connection_geometry_xml(preset, &style.adjustments, &style.connection_sites)?;
    }
    if let Some(path) = &style.path {
        path.validate()?;
        if !path.requires_override() { return Err(Error::Invalid("single-contour straight polygons use points without visual.path".into())); }
        if !matches!(element, Element::Polygon { points, .. } if *points == path.points()) { return Err(Error::Invalid("visual.path requires Polygon.points equal to its complete control-point list".into())); }
    }
    Ok(())
}

type Edits = Vec<(Range<usize>, String)>;
const ORDER: &[&str] = &["xfrm", "prstGeom", "custGeom", "noFill", "solidFill", "gradFill", "blipFill", "pattFill", "grpFill", "ln", "effectLst", "effectDag", "scene3d", "sp3d", "extLst"];
fn number(value: f64, unit: f64) -> String { (value * unit).round().to_string() }
fn color_xml(color: &str, opacity: f64) -> String {
    let (tag, value) = color.strip_prefix('@').map_or(("srgbClr", color), |slot| ("schemeClr", slot));
    format!("<a:{tag} val=\"{value}\"><a:alpha val=\"{}\"/></a:{tag}>", number(opacity, 100000.0))
}
fn fill_xml(style: &VisualStyle, fill: &str) -> String {
    if let Some(gradient) = &style.gradient {
        let stops: String = gradient.stops().iter().map(|stop| format!("<a:gs pos=\"{}\">{}</a:gs>", number(stop.offset, 100000.0), color_xml(&stop.color, stop.opacity))).collect();
        let geometry = match gradient {
            Gradient::Linear { angle, .. } => format!("<a:lin ang=\"{}\" scaled=\"1\"/>", number(*angle, 60000.0)),
            Gradient::Radial { center, .. } => format!("<a:path path=\"circle\"><a:fillToRect l=\"{}\" t=\"{}\" r=\"{}\" b=\"{}\"/></a:path>", number(center[0], 100000.0), number(center[1], 100000.0), number(1.0-center[0], 100000.0), number(1.0-center[1], 100000.0)),
        };
        format!("<a:gradFill rotWithShape=\"1\"><a:gsLst>{stops}</a:gsLst>{geometry}</a:gradFill>")
    } else { format!("<a:solidFill>{}</a:solidFill>", color_xml(fill, style.opacity.unwrap_or(1.0))) }
}
fn effects_xml(style: &VisualStyle) -> String {
    let mut effects = String::new();
    if let Some(glow) = &style.glow { effects.push_str(&format!("<a:glow rad=\"{}\">{}</a:glow>", number(glow.radius, 9525.0), color_xml(&glow.color, glow.opacity))); }
    if let Some(shadow) = &style.shadow { effects.push_str(&format!("<a:outerShdw blurRad=\"{}\" dist=\"{}\" dir=\"{}\" algn=\"ctr\" rotWithShape=\"1\">{}</a:outerShdw>", number(shadow.blur, 9525.0), number(shadow.distance, 9525.0), number(shadow.angle, 60000.0), color_xml(&shadow.color, shadow.opacity))); }
    if let Some(reflection) = &style.reflection { effects.push_str(&format!("<a:reflection blurRad=\"{}\" stA=\"{}\" stPos=\"0\" endA=\"{}\" endPos=\"{}\" dist=\"{}\" dir=\"5400000\" sy=\"-100000\" algn=\"bl\" rotWithShape=\"0\"/>", number(reflection.blur, 9525.0), number(reflection.start_opacity, 100000.0), number(reflection.end_opacity, 100000.0), number(reflection.end_position, 100000.0), number(reflection.distance, 9525.0))); }
    if let Some(radius) = style.soft_edge { effects.push_str(&format!("<a:softEdge rad=\"{}\"/>", number(radius, 9525.0))); }
    if effects.is_empty() { effects } else { format!("<a:effectLst>{effects}</a:effectLst>") }
}

fn insert(xml: &str, parent: Node<'_, '_>, fragment: &str, order: &[&str], name: &str, edits: &mut Edits) -> Result<()> {
    let rank = order.iter().position(|tag| *tag == name).unwrap_or(0);
    let before = parent.children().find(|node| node.is_element() && order.iter().position(|tag| *tag == node.tag_name().name()).is_some_and(|position| position > rank));
    crate::native_save::insert_child(xml, parent, fragment, before, edits)
}
fn replace(xml: &str, parent: Node<'_, '_>, names: &[&str], fragment: &str, order: &[&str], edits: &mut Edits) -> Result<()> {
    let nodes: Vec<_> = parent.children().filter(|node| node.tag_name().namespace() == Some(A) && names.contains(&node.tag_name().name())).collect();
    if let Some(first) = nodes.first() {
        edits.push((first.range(), fragment.into()));
        for node in nodes.iter().skip(1) { edits.push((node.range(), String::new())); }
    } else if !fragment.is_empty() { insert(xml, parent, fragment, order, names[0], edits)?; }
    Ok(())
}
fn locks<'node>(node: Node<'node, '_>) -> Option<(Node<'node, 'node>, &'static str)> {
    for (parent, name) in [("nvSpPr", "spLocks"), ("nvPicPr", "picLocks"), ("nvGrpSpPr", "grpSpLocks"), ("nvCxnSpPr", "cxnSpLocks")] {
        let property = match parent { "nvSpPr" => "cNvSpPr", "nvPicPr" => "cNvPicPr", "nvGrpSpPr" => "cNvGrpSpPr", _ => "cNvCxnSpPr" };
        if let Some(parent) = child(node, P, parent).and_then(|node| child(node, P, property)) { return Some((parent, name)); }
    }
    None
}

pub(crate) fn decorate(xml: String, elements: &[&Element], ids: &BTreeMap<&str, usize>) -> Result<String> {
    if elements.iter().all(|element| element.visual().is_none() && !matches!(element, Element::Picture { svg: Some(_), .. })) { return Ok(xml); }
    let parsed = crate::pptx::parse(&xml)?; let mut edits = Vec::new();
    for element in elements {
        if element.visual().is_none() && !matches!(element, Element::Picture { svg: Some(_), .. }) { continue; }
        let id = ids[element.bounds().0].to_string();
        let node = parsed.descendants().find(|node| crate::native::is_shape(*node) && properties(*node).and_then(|node| node.attribute("id")) == Some(id.as_str())).ok_or_else(|| Error::Invalid("visual shape missing".into()))?;
        let default = VisualStyle::default(); let style = element.visual().unwrap_or(&default);
        let parent = child(node, P, if matches!(element, Element::Group { .. }) { "grpSpPr" } else { "spPr" }).ok_or_else(|| Error::Invalid("visual shape properties missing".into()))?;
        if let Some(transform) = child(parent, A, "xfrm") {
            for (name, value) in [("rot", style.rotation.map(|angle| number(angle, 60000.0))), ("flipH", style.flip_h.then(|| "1".into())), ("flipV", style.flip_v.then(|| "1".into()))] {
                if let Some(value) = value { crate::native_save::set_attribute(&xml, transform, name, &value, &mut edits)?; }
            }
        }
        if style.hidden { crate::native_save::set_attribute(&xml, properties(node).unwrap(), "hidden", "1", &mut edits)?; }
        if style.locked { if let Some((parent, tag)) = locks(node) {
            if let Some(lock) = child(parent, A, tag) { for name in ["noMove", "noResize", "noRot"] { crate::native_save::set_attribute(&xml, lock, name, "1", &mut edits)?; } }
            else { insert(&xml, parent, &format!("<a:{tag} noMove=\"1\" noResize=\"1\" noRot=\"1\"/>"), &[tag, "stCxn", "endCxn", "extLst"], tag, &mut edits)?; }
        } }
        if style.opacity.is_some() || style.gradient.is_some() { if let Element::Rect { fill, .. } | Element::Shape { fill, .. } | Element::Polygon { fill, .. } = element { replace(&xml, parent, &["solidFill", "noFill", "gradFill"], &fill_xml(style, fill), ORDER, &mut edits)?; } }
        let effects = effects_xml(style); if !effects.is_empty() { replace(&xml, parent, &["effectLst"], &effects, ORDER, &mut edits)?; }
        if let Some(path) = &style.path { replace(&xml, parent, &["custGeom"], &crate::vector::path_xml(path), ORDER, &mut edits)?; }
        if !style.connection_sites.is_empty() {
            let Element::Shape { preset, .. } = element else { return Err(Error::Invalid("connection-site shape missing".into())); };
            replace(&xml, parent, &["prstGeom", "custGeom"], &crate::vector::connection_geometry_xml(preset, &style.adjustments, &style.connection_sites)?, ORDER, &mut edits)?;
        }
        if let Some(warp) = style.text_warp {
            let body = child(node, P, "txBody").and_then(|node| child(node, A, "bodyPr")).ok_or_else(|| Error::Invalid("warp body missing".into()))?;
            insert(&xml, body, &format!("<a:prstTxWarp prst=\"{}\"><a:avLst/></a:prstTxWarp>", warp.preset()), &["prstTxWarp", "noAutofit", "normAutofit", "spAutoFit", "scene3d", "sp3d", "flatTx", "extLst"], "prstTxWarp", &mut edits)?;
        }
        if !style.adjustments.is_empty() && style.connection_sites.is_empty() {
            let geometry = child(parent, A, "prstGeom").ok_or_else(|| Error::Invalid("adjustment geometry missing".into()))?;
            let guides: String = style.adjustments.iter().map(|adjustment| format!("<a:gd name=\"{}\" fmla=\"val {}\"/>", adjustment.name, adjustment.value)).collect();
            replace(&xml, geometry, &["avLst"], &format!("<a:avLst>{guides}</a:avLst>"), &["avLst"], &mut edits)?;
        }
        if let Element::Picture { svg, .. } = element {
            if let Some(mask) = style.picture_mask { crate::native_save::set_attribute(&xml, child(parent, A, "prstGeom").unwrap(), "prst", mask.preset(), &mut edits)?; }
            let blip = child(node, P, "blipFill").and_then(|node| child(node, A, "blip")).ok_or_else(|| Error::Invalid("picture blip missing".into()))?;
            let mut inner = String::new();
            if let Some(opacity) = style.opacity { inner.push_str(&format!("<a:alphaModFix amt=\"{}\"/>", number(opacity, 100000.0))); }
            if svg.is_some() { inner.push_str(&crate::vector::svg_blip_extension(&format!("rIdSvg{id}"))); }
            if !inner.is_empty() { crate::native_save::insert_child(&xml, blip, &inner, None, &mut edits)?; }
        }
    }
    String::from_utf8(crate::native_save::apply(xml, edits)?).map_err(|_| Error::Invalid("visual XML encoding".into()))
}

fn numeric(node: Node<'_, '_>, name: &str, default: f64) -> Result<f64> {
    let value = node.attribute(name).map(|value| value.parse::<f64>().map_err(|_| Error::Unsupported("visual formula is not a literal number".into()))).transpose()?.unwrap_or(default);
    if !value.is_finite() { return Err(Error::Invalid("nonfinite native visual".into())); } Ok(value)
}
fn is_true(node: Node<'_, '_>, name: &str) -> bool { matches!(node.attribute(name), Some("1" | "true")) }
pub(crate) fn read_color(node: Node<'_, '_>) -> Result<(String, f64)> {
    let shade = node.children().find(|node| node.has_tag_name((A, "srgbClr")) || node.has_tag_name((A, "schemeClr"))).ok_or_else(|| Error::Unsupported("visual color type".into()))?;
    let value = shade.attribute("val").ok_or_else(|| Error::Invalid("visual color missing".into()))?;
    let color = if shade.has_tag_name((A, "schemeClr")) { format!("@{value}") } else { value.into() };
    let opacity = child(shade, A, "alpha").map(|node| numeric(node, "val", 100000.0)).transpose()?.unwrap_or(100000.0) / 100000.0;
    Ok((color, opacity))
}

pub(crate) fn read(node: Node<'_, '_>) -> Result<Option<VisualStyle>> {
    let mut style = VisualStyle::default();
    let Some(parent) = child(node, P, if node.has_tag_name((P, "grpSp")) { "grpSpPr" } else { "spPr" }) else { return Ok(None); };
    if let Some(transform) = child(parent, A, "xfrm") {
        if !node.has_tag_name((P, "cxnSp")) {
            let rotation = numeric(transform, "rot", 0.0)? / 60000.0;
            if rotation != 0.0 { style.rotation = Some(rotation); }
            style.flip_h = is_true(transform, "flipH"); style.flip_v = is_true(transform, "flipV");
        }
    }
    style.hidden = properties(node).is_some_and(|node| is_true(node, "hidden"));
    style.locked = locks(node).and_then(|(node, tag)| child(node, A, tag)).is_some_and(|node| ["noMove", "noResize", "noRot"].iter().all(|name| is_true(node, name)));
    if let Some(fill) = child(parent, A, "solidFill") {
        if fill.descendants().any(|node| node.has_tag_name((A, "alpha"))) { style.opacity = Some(read_color(fill)?.1); }
    }
    if let Some(gradient) = child(parent, A, "gradFill") {
        let stops = child(gradient, A, "gsLst").ok_or_else(|| Error::Invalid("gradient stops missing".into()))?.children().filter(|node| node.has_tag_name((A, "gs"))).map(|node| { let (color, opacity) = read_color(node)?; Ok(GradientStop { offset: numeric(node, "pos", 0.0)? / 100000.0, color, opacity }) }).collect::<Result<Vec<_>>>()?;
        style.gradient = Some(if let Some(linear) = child(gradient, A, "lin") { Gradient::Linear { angle: numeric(linear, "ang", 0.0)? / 60000.0, stops } }
        else if let Some(path) = child(gradient, A, "path").filter(|node| node.attribute("path") == Some("circle")) {
            let rect = child(path, A, "fillToRect").ok_or_else(|| Error::Unsupported("radial gradient center".into()))?;
            Gradient::Radial { center: [numeric(rect, "l", 0.0)? / 100000.0, numeric(rect, "t", 0.0)? / 100000.0], stops }
        } else { return Err(Error::Unsupported("gradient geometry".into())); });
    }
    if let Some(effects) = child(parent, A, "effectLst") { for effect in effects.children().filter(|node| node.is_element()) { if effect.tag_name().namespace() != Some(A) { continue; } match effect.tag_name().name() {
        "glow" => { let (color, opacity) = read_color(effect)?; style.glow = Some(Glow { color, opacity, radius: numeric(effect, "rad", 0.0)? / 9525.0 }); }
        "outerShdw" => { let (color, opacity) = read_color(effect)?; style.shadow = Some(Shadow { color, opacity, blur: numeric(effect, "blurRad", 0.0)? / 9525.0, distance: numeric(effect, "dist", 0.0)? / 9525.0, angle: numeric(effect, "dir", 0.0)? / 60000.0 }); }
        "softEdge" => style.soft_edge = Some(numeric(effect, "rad", 0.0)? / 9525.0),
        "reflection" => style.reflection = Some(Reflection { blur: numeric(effect, "blurRad", 0.0)? / 9525.0, distance: numeric(effect, "dist", 0.0)? / 9525.0, start_opacity: numeric(effect, "stA", 100000.0)? / 100000.0, end_opacity: numeric(effect, "endA", 0.0)? / 100000.0, end_position: numeric(effect, "endPos", 100000.0)? / 100000.0 }),
        _ => {}
    } } }
    if let Some(warp) = child(node, P, "txBody").and_then(|node| child(node, A, "bodyPr")).and_then(|node| child(node, A, "prstTxWarp")) {
        if child(warp, A, "avLst").is_some_and(|list| list.children().any(|node| node.is_element())) { return Err(Error::Unsupported("nondefault WordArt adjustments retained as raw XML".into())); }
        style.text_warp = TextWarp::ALL.into_iter().find(|preset| Some(preset.preset()) == warp.attribute("prst"));
    }
    if node.has_tag_name((P, "sp")) {
        if let Some(geometry) = child(parent,A,"custGeom") {
            if let Some((_, adjustments, sites)) = crate::vector::read_connection_geometry(geometry)? {
                style.adjustments = adjustments; style.connection_sites = sites;
            } else if crate::vector::native_requires_override(geometry) { style.path = Some(crate::vector::read_path(geometry)?); }
        }
    }
    if node.has_tag_name((P, "pic")) {
        if child(parent,A,"custGeom").is_some() { return Err(Error::Unsupported("custom picture mask retained as raw XML".into())); }
        style.picture_mask = match child(parent, A, "prstGeom").and_then(|node| node.attribute("prst")) { Some("ellipse") => Some(PictureMask::Ellipse), Some("roundRect") => Some(PictureMask::RoundRect), Some("diamond") => Some(PictureMask::Diamond), Some("hexagon") => Some(PictureMask::Hexagon), Some("rect") | None => None, _ => return Err(Error::Unsupported("picture mask geometry".into())) };
        if let Some(alpha) = child(node, P, "blipFill").and_then(|node| child(node, A, "blip")).and_then(|node| child(node, A, "alphaModFix")) { style.opacity = Some(numeric(alpha, "amt", 100000.0)? / 100000.0); }
    } else if node.has_tag_name((P, "sp")) {
        if let Some(geometry) = child(parent, A, "prstGeom").filter(|node| matches!(node.attribute("prst"), Some("roundRect" | "chevron" | "triangle"))) {
            for guide in child(geometry, A, "avLst").into_iter().flat_map(|node| node.children()).filter(|node| node.is_element()) {
                let name = guide.attribute("name").unwrap_or("");
                let value = guide.attribute("fmla").and_then(|value| value.strip_prefix("val ")).and_then(|value| value.parse().ok());
                if name != "adj" || value.is_none() { return Err(Error::Unsupported("shape adjustment formula".into())); }
                style.adjustments.push(ShapeAdjustment { name: name.into(), value: value.unwrap_or_default() });
            }
        }
    }
    Ok((style != VisualStyle::default()).then_some(style))
}

pub(crate) fn signature(node: Node<'_, '_>, references: bool) -> serde_json::Value {
    let attributes: BTreeMap<_, _> = node.attributes().map(|attribute| {
        let value = if references && attribute.namespace() == Some(crate::native::R) && attribute.name() == "embed" { "resource" } else { attribute.value() };
        (format!("{}:{}", attribute.namespace().unwrap_or(""), attribute.name()), value)
    }).collect();
    let children: Vec<_> = node.children().filter(|node| node.is_element()).map(|node| signature(node, references)).collect();
    serde_json::json!([node.tag_name().namespace(),node.tag_name().name(),attributes,children])
}

fn equivalent(parent: Node<'_, '_>, expected: Node<'_, '_>, names: &[&str], references: bool) -> Result<()> {
    let selected = |parent: Node<'_, '_>| parent.children().filter(|node| node.tag_name().namespace() == Some(A) && names.contains(&node.tag_name().name())).map(|node| signature(node, references)).collect::<Vec<_>>();
    if selected(parent) != selected(expected) { return Err(Error::Unsupported(format!("native {} contains unrepresented formatting; replacement is refused", names.join("/")))); }
    Ok(())
}

fn replace_generated(xml: &str, parent: Node<'_, '_>, generated: &str, next: Node<'_, '_>, previous: Node<'_, '_>, names: &[&str], order: &[&str], edits: &mut Edits) -> Result<()> {
    equivalent(parent, previous, names, false)?;
    let fragment: String = next.children().filter(|node| node.tag_name().namespace() == Some(A) && names.contains(&node.tag_name().name())).map(|node| crate::native_save::fragment(generated, node)).collect();
    replace(xml, parent, names, &fragment, order, edits)
}

fn picture_changed(old: &Element, new: &Element) -> bool {
    matches!((old,new), (Element::Picture { base64, mime_type, svg, .. }, Element::Picture { base64: next, mime_type: next_mime, svg: next_svg, .. }) if base64 != next || mime_type != next_mime || svg != next_svg)
}

/// Patches only represented visual properties, never an entire native shape.
/// Unmodeled XML inside a replaced paint/effect/geometry subtree blocks edits;
/// sibling extensions and unrelated attributes remain byte-for-byte intact.
pub(crate) fn patch(xml: &str, node: Node<'_, '_>, generated: &str, next: Node<'_, '_>, previous: Node<'_, '_>, old: &Element, new: &Element, edits: &mut Edits) -> Result<()> {
    if matches!(new, Element::Table { .. } | Element::Chart { .. }) { return Ok(()); }
    if let (Element::Picture { base64, svg: Some(svg), .. }, Element::Picture { base64: next, svg: Some(next_svg), .. }) = (old,new) {
        if base64 != next && svg == next_svg { return Err(Error::Invalid("raster edits must remove or replace the retained SVG in the same element replacement".into())); }
    }
    let default = VisualStyle::default(); let before = old.visual().unwrap_or(&default); let after = new.visual().unwrap_or(&default);
    let tag = if matches!(new, Element::Group { .. }) { "grpSpPr" } else { "spPr" };
    let parent = child(node, P, tag).ok_or_else(|| Error::Unsupported("native visual properties missing".into()))?;
    let expected = child(previous, P, tag).ok_or_else(|| Error::Invalid("previous visual properties missing".into()))?;
    let next_parent = child(next, P, tag).ok_or_else(|| Error::Invalid("generated visual properties missing".into()))?;
    if let (Element::Group { children: old_children, .. }, Element::Group { children: new_children, .. }) = (old,new) {
        let origin = child(parent,A,"xfrm").and_then(|node| child(node,A,"chOff"));
        if origin.is_some_and(|node| node.attribute("x") != Some("0") || node.attribute("y") != Some("0")) && old_children.iter().map(|element| element.bounds().0).collect::<Vec<_>>() != new_children.iter().map(|element| element.bounds().0).collect::<Vec<_>>() {
            return Err(Error::Unsupported("nonzero native group origins preserve child topology; edit child visuals/positions without reparenting".into()));
        }
    }
    if (before.rotation, before.flip_h, before.flip_v) != (after.rotation, after.flip_h, after.flip_v) {
        let transform = child(parent, A, "xfrm").ok_or_else(|| Error::Unsupported("detach inherited geometry before applying visual transforms".into()))?;
        for (name, changed, value) in [("rot", before.rotation != after.rotation, number(after.rotation.unwrap_or(0.0), 60000.0)), ("flipH", before.flip_h != after.flip_h, u8::from(after.flip_h).to_string()), ("flipV", before.flip_v != after.flip_v, u8::from(after.flip_v).to_string())] {
            if changed { crate::native_save::set_attribute(xml, transform, name, &value, edits)?; }
        }
    }
    if before.hidden != after.hidden { crate::native_save::set_attribute(xml, properties(node).ok_or_else(|| Error::Unsupported("native nonvisual properties missing".into()))?, "hidden", if after.hidden { "1" } else { "0" }, edits)?; }
    if before.locked != after.locked {
        let (parent, tag) = locks(node).ok_or_else(|| Error::Unsupported("native locking properties missing".into()))?;
        if let Some(lock) = child(parent, A, tag) { for name in ["noMove", "noResize", "noRot"] { crate::native_save::set_attribute(xml, lock, name, if after.locked { "1" } else { "0" }, edits)?; } }
        else if after.locked { insert(xml, parent, &format!("<a:{tag} xmlns:a=\"{A}\" noMove=\"1\" noResize=\"1\" noRot=\"1\"/>"), &[tag, "stCxn", "endCxn", "extLst"], tag, edits)?; }
    }
    let fill = |element: &Element| match element { Element::Rect { fill, .. } | Element::Polygon { fill, .. } | Element::Shape { fill, .. } => Some(fill.clone()), _ => None };
    if fill(new).is_some() && (fill(old) != fill(new) || before.opacity != after.opacity || before.gradient != after.gradient) {
        replace_generated(xml, parent, generated, next_parent, expected, &["noFill", "solidFill", "gradFill", "blipFill", "pattFill", "grpFill"], ORDER, edits)?;
    }
    if (&before.shadow, &before.glow, before.soft_edge, &before.reflection) != (&after.shadow, &after.glow, after.soft_edge, &after.reflection) {
        replace_generated(xml, parent, generated, next_parent, expected, &["effectLst", "effectDag"], ORDER, edits)?;
    }
    if before.text_warp != after.text_warp {
        let body = |node| child(node, P, "txBody").and_then(|node| child(node, A, "bodyPr")).ok_or_else(|| Error::Unsupported("native text warp body missing".into()));
        replace_generated(xml, body(node)?, generated, body(next)?, body(previous)?, &["prstTxWarp"], &["prstTxWarp", "noAutofit", "normAutofit", "spAutoFit", "scene3d", "sp3d", "flatTx", "extLst"], edits)?;
    }
    let geometry_changed = match (old, new) {
        (Element::Polygon { points, .. }, Element::Polygon { points: next, .. }) => points != next || before.path != after.path,
        (Element::Shape { preset, .. }, Element::Shape { preset: next, .. }) => preset != next || before.adjustments != after.adjustments || before.connection_sites != after.connection_sites,
        (Element::Connector { routing: Some(before), .. }, Element::Connector { routing: Some(after), .. }) if before.custom && after.custom => before.points != after.points,
        (Element::Picture { .. }, Element::Picture { .. }) => before.picture_mask != after.picture_mask,
        _ => false,
    };
    if geometry_changed { replace_generated(xml, parent, generated, next_parent, expected, &["prstGeom", "custGeom"], ORDER, edits)?; }
    let outline_changed = matches!((old,new), (Element::Shape { stroke, stroke_width, .. }, Element::Shape { stroke: next, stroke_width: next_width, .. }) | (Element::Polygon { stroke, stroke_width, .. }, Element::Polygon { stroke: next, stroke_width: next_width, .. }) if stroke != next || stroke_width != next_width);
    if outline_changed { replace_generated(xml, parent, generated, next_parent, expected, &["ln"], ORDER, edits)?; }
    if let (Element::Connector { routing: Some(before), color, stroke_width, arrow, .. }, Element::Connector { routing: Some(after), color: next_color, stroke_width: next_width, arrow: next_arrow, .. }) = (old, new) {
        if before.custom && after.custom && (before.points != after.points || before.start_arrow != after.start_arrow || before.dashed != after.dashed || color != next_color || stroke_width != next_width || arrow != next_arrow) {
            replace_generated(xml, parent, generated, next_parent, expected, &["ln"], ORDER, edits)?;
        }
    }
    if matches!(new, Element::Picture { .. }) && (picture_changed(old,new) || before.opacity != after.opacity) {
        let fill = child(node, P, "blipFill").ok_or_else(|| Error::Unsupported("native picture fill missing".into()))?;
        let previous_fill = child(previous, P, "blipFill").ok_or_else(|| Error::Invalid("previous picture fill missing".into()))?;
        equivalent(fill, previous_fill, &["blip"], true)?;
        let next_blip = child(next, P, "blipFill").and_then(|node| child(node, A, "blip")).ok_or_else(|| Error::Invalid("generated picture blip missing".into()))?;
        let mut fragment = crate::native_save::fragment(generated, next_blip);
        if !picture_changed(old,new) {
            let mut mapping = BTreeMap::new();
            for next in next_blip.descendants().filter(|node| node.is_element()) {
                if let Some(reference) = next.attribute((crate::native::R,"embed")) {
                    let actual = fill.descendants().find(|node| node.tag_name() == next.tag_name()).and_then(|node| node.attribute((crate::native::R,"embed"))).ok_or_else(|| Error::Invalid("existing picture reference missing".into()))?;
                    mapping.insert(reference.to_owned(), actual.to_owned());
                }
            }
            fragment = crate::native_save::remap_resources(fragment, &mapping)?;
        }
        replace(xml, fill, &["blip"], &fragment, &["blip", "srcRect", "tile", "stretch"], edits)?;
    }
    Ok(())
}
