use crate::{Error, Result, package::Package, native::{A, P, R, child}};
use base64::{Engine, engine::general_purpose::STANDARD};
use roxmltree::Node;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, collections::VecDeque};

pub const SVG_NAMESPACE: &str = "http://schemas.microsoft.com/office/drawing/2016/SVG/main";
pub const SVG_EXTENSION: &str = "{96DAC541-7B7A-43D3-8B79-37D633B846F1}";

/// Closed normalized contours, at most 256 commands in total. Coordinates and
/// control points are 0..=1; DrawingML quantizes them to 1/1,000,000.
/// Fill uses nonzero winding in DrawingML and SVG: holes must have the opposite
/// winding to their exterior. Editing retains curves; geometry_ops explicitly
/// flattens boolean operands with a declared tolerance. No arcs or formulas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VectorPath { pub commands: Vec<PathCommand> }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum PathCommand {
    Move { point: [f64; 2] },
    Line { point: [f64; 2] },
    Quadratic { control: [f64; 2], point: [f64; 2] },
    Cubic { control1: [f64; 2], control2: [f64; 2], point: [f64; 2] },
    Close,
}

impl VectorPath {
    /// Polygon.points must equal this list when visual.path is present. This
    /// makes conflicting edits reject instead of silently ignoring point edits.
    pub fn points(&self) -> Vec<[f64; 2]> {
        self.commands.iter().flat_map(|command| match command {
            PathCommand::Move { point } | PathCommand::Line { point } => vec![*point],
            PathCommand::Quadratic { control, point } => vec![*control, *point],
            PathCommand::Cubic { control1, control2, point } => vec![*control1, *control2, *point],
            PathCommand::Close => Vec::new(),
        }).collect()
    }
    pub fn validate(&self) -> Result<()> {
        if !(3..=256).contains(&self.commands.len()) || !matches!(self.commands.first(), Some(PathCommand::Move { .. })) || !matches!(self.commands.last(), Some(PathCommand::Close)) {
            return Err(Error::Invalid("path requires move, 1-254 drawing commands, close".into()));
        }
        let mut active = false;
        let mut count = 0;
        for command in &self.commands {
            match command {
                PathCommand::Move { .. } if !active => { active = true; count = 1; }
                PathCommand::Line { .. } if active => count += 1,
                PathCommand::Quadratic { .. } if active => count += 2,
                PathCommand::Cubic { .. } if active => count += 3,
                PathCommand::Close if active && count >= 3 => active = false,
                _ => return Err(Error::Invalid("each contour requires move, at least three points, and close".into())),
            }
        }
        let points = self.points();
        if points.len() < 3 || points.iter().flatten().any(|value| !value.is_finite() || !(0.0..=1.0).contains(value)) { return Err(Error::Invalid("path needs at least three finite normalized points".into())); }
        if self.commands.iter().filter(|command| matches!(command, PathCommand::Move { .. })).count() > 1 { crate::geometry_ops::validate_compound(self)?; }
        Ok(())
    }

    pub fn requires_override(&self) -> bool {
        self.commands.iter().filter(|command| matches!(command, PathCommand::Move { .. })).count() > 1
            || self.commands.iter().any(|command| matches!(command, PathCommand::Quadratic { .. } | PathCommand::Cubic { .. }))
    }
}

pub fn edit_path(mut element: crate::model::Element, path: VectorPath) -> Result<crate::model::Element> {
    let validate = |element: &crate::model::Element| crate::model::validate_elements(std::slice::from_ref(element), (4096.0, 4096.0), 0, &mut std::collections::BTreeSet::new(), &mut 0, &mut 0);
    validate(&element)?;
    if element.visual().is_some_and(|visual| visual.locked) { return Err(Error::Invalid("locked elements cannot be edited".into())); }
    path.validate()?;
    let crate::model::Element::Polygon { points, visual, .. } = &mut element else { return Err(Error::Unsupported("path editing requires a polygon".into())); };
    *points = path.points();
    if path.requires_override() {
        visual.get_or_insert_with(Default::default).path = Some(path);
    } else if let Some(style) = visual {
        style.path = None;
        if *style == crate::visual::VisualStyle::default() { *visual = None; }
    }
    validate(&element)?;
    Ok(element)
}

pub(crate) fn native_requires_override(geometry: Node<'_, '_>) -> bool {
    geometry.descendants().filter(|node| node.has_tag_name((A,"moveTo"))).count() > 1
        || geometry.descendants().any(|node| node.has_tag_name((A,"cubicBezTo")) || node.has_tag_name((A,"quadBezTo")))
}

pub(crate) fn path_xml(path: &VectorPath) -> String {
    let point_xml = |point: &[f64;2]| format!("<a:pt x=\"{}\" y=\"{}\"/>", (point[0]*1000000.0).round(), (point[1]*1000000.0).round());
    let commands: String = path.commands.iter().map(|command| match command {
        PathCommand::Move { point } => format!("<a:moveTo>{}</a:moveTo>", point_xml(point)),
        PathCommand::Line { point } => format!("<a:lnTo>{}</a:lnTo>", point_xml(point)),
        PathCommand::Quadratic { control, point } => format!("<a:quadBezTo>{}{}</a:quadBezTo>", point_xml(control), point_xml(point)),
        PathCommand::Cubic { control1, control2, point } => format!("<a:cubicBezTo>{}{}{}</a:cubicBezTo>", point_xml(control1), point_xml(control2), point_xml(point)),
        PathCommand::Close => "<a:close/>".into(),
    }).collect();
    format!("<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l=\"0\" t=\"0\" r=\"r\" b=\"b\"/><a:pathLst><a:path w=\"1000000\" h=\"1000000\">{commands}</a:path></a:pathLst></a:custGeom>")
}

pub fn connection_geometry_xml(preset: &str, adjustments: &[crate::visual::ShapeAdjustment], sites: &[crate::visual::ConnectionSite]) -> Result<String> {
    if !matches!(preset, "rect" | "roundRect" | "ellipse" | "diamond") {
        return Err(Error::Unsupported(format!("faithful custom connection-site geometry is not implemented for {preset}")));
    }
    if sites.is_empty() || sites.len() > 128 { return Err(Error::Invalid("custom connection sites require 1-128 entries".into())); }
    if !adjustments.is_empty() && (preset != "roundRect" || adjustments.len() != 1 || adjustments[0].name != "adj" || !(0..=50000).contains(&adjustments[0].value)) {
        return Err(Error::Invalid("custom connection geometry supports only roundRect's bounded adj".into()));
    }
    let marker = format!("<a:gd name=\"aislideConnectionSitesV1_{preset}\" fmla=\"val 0\"/>");
    let adjustment = adjustments.first().map(|adjustment| format!("<a:gd name=\"adj\" fmla=\"val {}\"/>", adjustment.value)).unwrap_or_default();
    let mut guides = String::new();
    let mut connections = String::new();
    for (index, site) in sites.iter().enumerate() {
        if !site.x.is_finite() || !site.y.is_finite() || !site.angle.is_finite() || !(0.0..=1.0).contains(&site.x) || !(0.0..=1.0).contains(&site.y) || !(0.0..=360.0).contains(&site.angle) {
            return Err(Error::Invalid("connection sites require finite normalized positions and angles in 0..=360 degrees".into()));
        }
        for (axis, dimension, value) in [("x", "w", site.x), ("y", "h", site.y)] {
            guides.push_str(&format!("<a:gd name=\"site{index}{axis}\" fmla=\"*/ {dimension} {} 1000000\"/>", (value * 1e6).round()));
        }
        connections.push_str(&format!("<a:cxn ang=\"{}\"><a:pos x=\"site{index}x\" y=\"site{index}y\"/></a:cxn>", (site.angle * 60000.0).round()));
    }
    let commands = match preset {
        "rect" | "roundRect" if preset == "rect" || adjustments.first().is_some_and(|adjustment| adjustment.value == 0) => "<a:moveTo><a:pt x=\"l\" y=\"t\"/></a:moveTo><a:lnTo><a:pt x=\"r\" y=\"t\"/></a:lnTo><a:lnTo><a:pt x=\"r\" y=\"b\"/></a:lnTo><a:lnTo><a:pt x=\"l\" y=\"b\"/></a:lnTo><a:close/>",
        "diamond" => "<a:moveTo><a:pt x=\"hc\" y=\"t\"/></a:moveTo><a:lnTo><a:pt x=\"r\" y=\"vc\"/></a:lnTo><a:lnTo><a:pt x=\"hc\" y=\"b\"/></a:lnTo><a:lnTo><a:pt x=\"l\" y=\"vc\"/></a:lnTo><a:close/>",
        "ellipse" => "<a:moveTo><a:pt x=\"l\" y=\"vc\"/></a:moveTo><a:arcTo wR=\"wd2\" hR=\"hd2\" stAng=\"10800000\" swAng=\"10800000\"/><a:arcTo wR=\"wd2\" hR=\"hd2\" stAng=\"0\" swAng=\"10800000\"/><a:close/>",
        "roundRect" => {
            let radius = if adjustments.is_empty() { "16667" } else { "adj" };
            guides.push_str(&format!("<a:gd name=\"radius\" fmla=\"*/ ss {radius} 100000\"/><a:gd name=\"rightInset\" fmla=\"+- r 0 radius\"/><a:gd name=\"bottomInset\" fmla=\"+- b 0 radius\"/>"));
            "<a:moveTo><a:pt x=\"radius\" y=\"t\"/></a:moveTo><a:lnTo><a:pt x=\"rightInset\" y=\"t\"/></a:lnTo><a:arcTo wR=\"radius\" hR=\"radius\" stAng=\"16200000\" swAng=\"5400000\"/><a:lnTo><a:pt x=\"r\" y=\"bottomInset\"/></a:lnTo><a:arcTo wR=\"radius\" hR=\"radius\" stAng=\"0\" swAng=\"5400000\"/><a:lnTo><a:pt x=\"radius\" y=\"b\"/></a:lnTo><a:arcTo wR=\"radius\" hR=\"radius\" stAng=\"5400000\" swAng=\"5400000\"/><a:lnTo><a:pt x=\"l\" y=\"radius\"/></a:lnTo><a:arcTo wR=\"radius\" hR=\"radius\" stAng=\"10800000\" swAng=\"5400000\"/><a:close/>"
        }
        _ => unreachable!(),
    };
    Ok(format!("<a:custGeom><a:avLst>{marker}{adjustment}</a:avLst><a:gdLst>{guides}</a:gdLst><a:ahLst/><a:cxnLst>{connections}</a:cxnLst><a:rect l=\"l\" t=\"t\" r=\"r\" b=\"b\"/><a:pathLst><a:path>{commands}</a:path></a:pathLst></a:custGeom>"))
}

pub(crate) fn read_connection_geometry(geometry: Node<'_, '_>) -> Result<Option<(String, Vec<crate::visual::ShapeAdjustment>, Vec<crate::visual::ConnectionSite>)>> {
    let Some(marker) = child(geometry, A, "avLst").and_then(|list| list.children().find(|node| node.has_tag_name((A, "gd")) && node.attribute("name").is_some_and(|name| name.starts_with("aislideConnectionSitesV1_")))) else { return Ok(None); };
    let preset = marker.attribute("name").unwrap_or_default().trim_start_matches("aislideConnectionSitesV1_");
    let invalid = || Error::Unsupported("unrepresented native custom connection-site geometry".into());
    let mut adjustments = Vec::new();
    if let Some(guide) = child(geometry, A, "avLst").and_then(|list| list.children().find(|node| node.has_tag_name((A, "gd")) && node.attribute("name") == Some("adj"))) {
        let value = guide.attribute("fmla").and_then(|value| value.strip_prefix("val ")).and_then(|value| value.parse().ok()).ok_or_else(invalid)?;
        adjustments.push(crate::visual::ShapeAdjustment { name: "adj".into(), value });
    }
    let mut sites = Vec::new();
    let list = child(geometry, A, "cxnLst").ok_or_else(invalid)?;
    for (index, connection) in list.children().filter(|node| node.is_element()).enumerate() {
        if index >= 128 { return Err(Error::Limit("native connection sites exceed 128 entries".into())); }
        let coordinate = |axis, dimension| -> Result<f64> {
            let name = format!("site{index}{axis}");
            let guide = child(geometry, A, "gdLst").and_then(|list| list.children().find(|node| node.has_tag_name((A, "gd")) && node.attribute("name") == Some(name.as_str()))).ok_or_else(invalid)?;
            guide.attribute("fmla").and_then(|value| value.strip_prefix(&format!("*/ {dimension} "))).and_then(|value| value.strip_suffix(" 1000000")).and_then(|value| value.parse::<u32>().ok()).map(|value| f64::from(value) / 1e6).ok_or_else(invalid)
        };
        let angle = connection.attribute("ang").and_then(|value| value.parse::<u32>().ok()).ok_or_else(invalid)?;
        sites.push(crate::visual::ConnectionSite { x: coordinate("x", "w")?, y: coordinate("y", "h")?, angle: f64::from(angle) / 60000.0 });
    }
    let expected = format!("<root xmlns:a=\"{A}\">{}</root>", connection_geometry_xml(preset, &adjustments, &sites)?);
    let parsed = crate::pptx::parse(&expected)?;
    let expected = child(parsed.root_element(), A, "custGeom").ok_or_else(invalid)?;
    if crate::visual::signature(geometry, false) != crate::visual::signature(expected, false) { return Err(invalid()); }
    Ok(Some((preset.into(), adjustments, sites)))
}

pub(crate) fn read_path(geometry: Node<'_, '_>) -> Result<VectorPath> {
    let paths: Vec<_> = child(geometry, A, "pathLst").into_iter().flat_map(|node| node.children()).filter(|node| node.is_element()).collect();
    if paths.len() != 1 || !paths[0].has_tag_name((A,"path")) { return Err(Error::Unsupported("multiple native paths".into())); }
    let path = paths[0];
    let dimension = |name| path.attribute(name).and_then(|value| value.parse::<f64>().ok()).filter(|value| value.is_finite() && *value > 0.0 && *value <= 1e12).ok_or_else(|| Error::Unsupported("native path coordinate space".into()));
    let width = dimension("w")?; let height = dimension("h")?;
    let mut commands = Vec::new();
    for command in path.children().filter(|node| node.is_element()) {
        if commands.len() >= 256 { return Err(Error::Limit("path exceeds 256 commands".into())); }
        if command.tag_name().namespace() != Some(A) { return Err(Error::Unsupported("native path extension".into())); }
        let points = command.children().filter(|node| node.is_element()).map(|node| {
            if !node.has_tag_name((A,"pt")) { return Err(Error::Unsupported("native path point extension".into())); }
            let coordinate = |name, unit| node.attribute(name).and_then(|value| value.parse::<f64>().ok()).map(|value| value / unit).ok_or_else(|| Error::Unsupported("native path formula".into()));
            Ok([coordinate("x",width)?,coordinate("y",height)?])
        }).collect::<Result<Vec<_>>>()?;
        commands.push(match (command.tag_name().name(), points.as_slice()) {
            ("moveTo",[point]) => PathCommand::Move { point:*point },
            ("lnTo",[point]) => PathCommand::Line { point:*point },
            ("quadBezTo",[control,point]) => PathCommand::Quadratic { control:*control,point:*point },
            ("cubicBezTo",[control1,control2,point]) => PathCommand::Cubic { control1:*control1,control2:*control2,point:*point },
            ("close",[]) => PathCommand::Close,
            _ => return Err(Error::Unsupported("native path command or arity".into())),
        });
    }
    let result = VectorPath { commands }; result.validate()?; Ok(result)
}

thread_local! {
    static SVG_CACHE: RefCell<VecDeque<([u8;32], String)>> = const { RefCell::new(VecDeque::new()) };
}

/// Retains exact inert SVG bytes. Preflight rejects rather than strips active or
/// external content. Literal text uses local fonts; embedded PNG/JPEG are bounded.
/// The PNG is rendered locally at a 1024px longest side using resvg. Only
/// successful renders are cached, by content hash, at most eight entries/4 MiB.
pub fn prepare_svg(base64: &str) -> Result<String> {
    if base64.len() > 349528 { return Err(Error::Limit("SVG exceeds 256 KiB".into())); }
    let hash: [u8;32] = Sha256::digest(base64.as_bytes()).into();
    if let Some(png) = SVG_CACHE.with(|cache| cache.borrow().iter().find(|(key,_)| *key == hash).map(|(_,png)| png.clone())) { return Ok(png); }
    let png = crate::media::svg_png(base64, 1024)?;
    SVG_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        while cache.len() >= 8 || cache.iter().map(|(_,png)| png.len()).sum::<usize>() + png.len() > 4 * 1024 * 1024 { cache.pop_front(); }
        cache.push_back((hash,png.clone()));
    });
    Ok(png)
}

pub(crate) fn svg_blip_extension(id: &str) -> String {
    format!("<a:extLst><a:ext uri=\"{SVG_EXTENSION}\"><asvg:svgBlip xmlns:asvg=\"{SVG_NAMESPACE}\" r:embed=\"{id}\"/></a:ext></a:extLst>")
}

pub(crate) fn read_svg(package: &Package, part: &str, node: Node<'_, '_>) -> Result<Option<String>> {
    let Some(blip) = child(node, P, "blipFill").and_then(|node| child(node, A, "blip")) else { return Ok(None); };
    let matches: Vec<_> = blip.descendants().filter(|node| node.has_tag_name((SVG_NAMESPACE, "svgBlip"))).collect();
    if matches.len() > 1 { return Err(Error::Unsupported("multiple SVG references".into())); }
    let Some(svg) = matches.first() else { return Ok(None); };
    if svg.parent().and_then(|node| node.attribute("uri")) != Some(SVG_EXTENSION) { return Err(Error::Unsupported("SVG extension identity".into())); }
    let id = svg.attribute((R, "embed")).ok_or_else(|| Error::Unsupported("external or missing SVG".into()))?;
    let targets = crate::pptx::relationship_targets(package, part, "image")?;
    let path = targets.get(id).ok_or_else(|| Error::Invalid("internal SVG relationship missing".into()))?;
    let encoded = STANDARD.encode(package.part(path)?);
    prepare_svg(&encoded)?;
    Ok(Some(encoded))
}

#[derive(Clone)]
enum MetafileObject { Brush(String), Pen(String,i32) }

fn meta_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let data = bytes.get(offset..offset+2).ok_or_else(|| Error::Invalid("truncated metafile word".into()))?;
    Ok(u16::from_le_bytes([data[0],data[1]]))
}
fn meta_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let data = bytes.get(offset..offset+4).ok_or_else(|| Error::Invalid("truncated metafile integer".into()))?;
    Ok(u32::from_le_bytes([data[0],data[1],data[2],data[3]]))
}
fn meta_color(value: u32) -> Result<String> {
    if value>0xffffff { return Err(Error::Unsupported("metafile palette colors are not supported".into())); }
    Ok(format!("#{:02X}{:02X}{:02X}",value&255,(value>>8)&255,(value>>16)&255))
}

pub(crate) fn metafile_svg(encoded: &str, mime: &str) -> Result<String> {
    if encoded.len()>349528 { return Err(Error::Limit("metafile exceeds 256 KiB".into())); }
    let bytes = STANDARD.decode(encoded).map_err(|_| Error::Invalid("invalid metafile base64".into()))?;
    if bytes.len()>262144 { return Err(Error::Limit("metafile exceeds 256 KiB".into())); }
    let emf = mime=="image/emf";
    let (bounds,mut offset,handles,expected_records) = if emf {
        if meta_u32(&bytes,0)?!=1 || meta_u32(&bytes,4)?!=88 || meta_u32(&bytes,40)?!=0x464d4520 || meta_u32(&bytes,44)?!=0x10000 || meta_u32(&bytes,48)? as usize != bytes.len()
            || meta_u16(&bytes,58)?!=0 || meta_u32(&bytes,60)?!=0 || meta_u32(&bytes,64)?!=0 || meta_u32(&bytes,68)?!=0 {
            return Err(Error::Unsupported("EMF requires an 88-byte v1 header without descriptions or palettes".into()));
        }
        let bounds = [meta_u32(&bytes,8)? as i32,meta_u32(&bytes,12)? as i32,meta_u32(&bytes,16)? as i32,meta_u32(&bytes,20)? as i32];
        (bounds,88,usize::from(meta_u16(&bytes,56)?),Some(meta_u32(&bytes,52)? as usize))
    } else {
        if mime!="image/wmf" || meta_u32(&bytes,0)?!=0x9ac6cdd7 || meta_u16(&bytes,4)?!=0 || meta_u32(&bytes,16)?!=0 || meta_u16(&bytes,14)?==0 {
            return Err(Error::Unsupported("WMF requires a bounded placeable header".into()));
        }
        let mut checksum = 0;
        for offset in (0..20).step_by(2) { checksum ^= meta_u16(&bytes,offset)?; }
        if checksum!=meta_u16(&bytes,20)? || !matches!(meta_u16(&bytes,22)?,1|2) || meta_u16(&bytes,24)?!=9 || meta_u16(&bytes,26)?!=0x300 || u64::from(meta_u32(&bytes,28)?)*2+22 != bytes.len() as u64 || meta_u16(&bytes,38)?!=0 {
            return Err(Error::Invalid("invalid WMF header length, version or checksum".into()));
        }
        let bounds = [meta_u16(&bytes,6)? as i16 as i32,meta_u16(&bytes,8)? as i16 as i32,meta_u16(&bytes,10)? as i16 as i32,meta_u16(&bytes,12)? as i16 as i32];
        (bounds,40,usize::from(meta_u16(&bytes,32)?),None)
    };
    let width = i64::from(bounds[2])-i64::from(bounds[0]); let height = i64::from(bounds[3])-i64::from(bounds[1]);
    if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) || !(1..=256).contains(&handles) || bounds.iter().any(|value| value.unsigned_abs()>1_000_000) { return Err(Error::Limit("metafile bounds or handles exceed limits".into())); }
    let mut objects = vec![None;handles];
    let mut brush = "#FFFFFF".to_owned(); let mut pen = "#000000".to_owned(); let mut pen_width = 1;
    let mut position = [bounds[0],bounds[1]]; let mut fill_rule = "evenodd";
    let mut records = usize::from(emf); let mut vertices = 0; let mut maximum_record = 0; let mut ended = false;
    let mut svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"{} {} {width} {height}\">",bounds[0],bounds[1]);
    while offset<bytes.len() {
        records+=1;
        if records>1024 { return Err(Error::Limit("metafile exceeds 1024 records".into())); }
        let (kind,length,header) = if emf { (meta_u32(&bytes,offset)?,meta_u32(&bytes,offset+4)? as usize,8) }
            else { (u32::from(meta_u16(&bytes,offset+4)?),usize::try_from(u64::from(meta_u32(&bytes,offset)?)*2).map_err(|_|Error::Limit("metafile record size".into()))?,6) };
        if length<header || length%if emf {4}else{2}!=0 || length>bytes.len()-offset { return Err(Error::Invalid("invalid metafile record length".into())); }
        maximum_record = maximum_record.max(length);
        let record = &bytes[offset..offset+length]; offset+=length;
        let exact = |expected| if length==expected { Ok(()) } else { Err(Error::Invalid("metafile record has unexpected payload".into())) };
        let coordinate = |at| -> Result<i32> { if emf { Ok(meta_u32(record,at)? as i32) } else { Ok(meta_u16(record,at)? as i16 as i32) } };
        let check_point = |point: [i32;2]| -> Result<[i32;2]> {
            if point[0]<bounds[0] || point[0]>bounds[2] || point[1]<bounds[1] || point[1]>bounds[3] { return Err(Error::Unsupported("metafile coordinates must remain within declared bounds".into())); } Ok(point)
        };
        let attributes = format!("fill=\"{brush}\" fill-rule=\"{fill_rule}\" stroke=\"{pen}\" stroke-width=\"{pen_width}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"");
        match (emf,kind) {
            (true,14) | (false,0) => {
                exact(if emf {20}else{6})?;
                if emf && (meta_u32(record,8)?!=0 || meta_u32(record,12)?!=0 || meta_u32(record,16)?!=20) { return Err(Error::Unsupported("EMF EOF palette".into())); }
                if offset!=bytes.len() { return Err(Error::Invalid("trailing metafile bytes".into())); }
                ended = true; break;
            }
            (true,37) | (false,0x012d) => {
                exact(if emf {12}else{8})?;
                let handle = if emf {meta_u32(record,8)?} else {u32::from(meta_u16(record,6)?)};
                let object = if emf && handle&0x80000000!=0 {
                    match handle&0x7fffffff {
                        0 => MetafileObject::Brush("#FFFFFF".into()),4 => MetafileObject::Brush("#000000".into()),5 => MetafileObject::Brush("none".into()),
                        6 => MetafileObject::Pen("#FFFFFF".into(),1),7 => MetafileObject::Pen("#000000".into(),1),8 => MetafileObject::Pen("none".into(),1),
                        _ => return Err(Error::Unsupported("unsupported metafile stock object".into())),
                    }
                } else { objects.get(handle as usize).and_then(Clone::clone).ok_or_else(|| Error::Invalid("metafile object not defined".into()))? };
                match object { MetafileObject::Brush(color) => brush=color,MetafileObject::Pen(color,width) => { pen=color;pen_width=width; } }
            }
            (true,38|39) | (false,0x02fa|0x02fc) => {
                let is_pen = kind==38 || kind==0x02fa;
                exact(if emf {if is_pen {28}else{24}}else if is_pen {16}else{14})?;
                let index = if emf {meta_u32(record,8)? as usize} else {objects.iter().position(Option::is_none).ok_or_else(|| Error::Limit("WMF object table full".into()))?};
                let style = if emf {meta_u32(record,12)?}else{u32::from(meta_u16(record,6)?)};
                let color = meta_color(meta_u32(record,if emf {if is_pen {24}else{16}}else if is_pen {12}else{8})?)?;
                let object = if is_pen {
                    let width = coordinate(if emf {16}else{8})?;
                    if !matches!(style,0|5) || !(0..=100).contains(&width) || coordinate(if emf {20}else{10})?!=0 { return Err(Error::Unsupported("metafile pen must be solid/null and at most 100 units wide".into())); }
                    MetafileObject::Pen(if style==5 {"none".into()}else{color},width.max(1))
                } else {
                    if !matches!(style,0|1) || (if emf {meta_u32(record,20)?}else{u32::from(meta_u16(record,12)?)})!=0 { return Err(Error::Unsupported("metafile brush must be solid or null".into())); }
                    MetafileObject::Brush(if style==1 {"none".into()}else{color})
                };
                let slot = objects.get_mut(index).ok_or_else(|| Error::Invalid("metafile object handle out of range".into()))?;
                if slot.is_some() { return Err(Error::Invalid("metafile object redefinition".into())); } *slot=Some(object);
            }
            (true,40) | (false,0x01f0) => {
                exact(if emf {12}else{8})?;
                let index = if emf {meta_u32(record,8)? as usize}else{usize::from(meta_u16(record,6)?)};
                if objects.get_mut(index).and_then(Option::take).is_none() { return Err(Error::Invalid("deleting undefined metafile object".into())); }
            }
            (true,43|42) | (false,0x041b|0x0418) => {
                exact(if emf {24}else{14})?;
                let (start,end) = if emf {([coordinate(8)?,coordinate(12)?],[coordinate(16)?,coordinate(20)?])} else {([coordinate(12)?,coordinate(10)?],[coordinate(8)?,coordinate(6)?])};
                check_point(start)?;check_point(end)?;
                let width = end[0]-start[0]; let height = end[1]-start[1];
                if width<=0 || height<=0 { return Err(Error::Invalid("degenerate metafile shape".into())); }
                if kind==43 || kind==0x041b { svg.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"{width}\" height=\"{height}\" {attributes}/>",start[0],start[1])); }
                else { svg.push_str(&format!("<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" {attributes}/>",f64::from(start[0])+f64::from(width)/2.0,f64::from(start[1])+f64::from(height)/2.0,f64::from(width)/2.0,f64::from(height)/2.0)); }
            }
            (true,27|54) | (false,0x0214|0x0213) => {
                exact(if emf {16}else{10})?;
                let next = check_point(if emf {[coordinate(8)?,coordinate(12)?]}else{[coordinate(8)?,coordinate(6)?]})?;
                if kind==54 || kind==0x0213 { svg.push_str(&format!("<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{pen}\" stroke-width=\"{pen_width}\" stroke-linecap=\"round\"/>",position[0],position[1],next[0],next[1])); }
                position=next;
            }
            (true,3|4) | (false,0x0324|0x0325) => {
                let count = if emf {meta_u32(record,24)? as usize}else{usize::from(meta_u16(record,6)?)};
                vertices+=count;
                if count<2 || vertices>4096 { return Err(Error::Limit("metafile point budget".into())); }
                let stride = if emf {8}else{4}; let start = if emf {28}else{8}; exact(start+count*stride)?;
                let mut points = String::new();
                for index in 0..count { let point = check_point([coordinate(start+index*stride)?,coordinate(start+index*stride+stride/2)?])?; points.push_str(&format!("{},{} ",point[0],point[1])); }
                if kind==3 || kind==0x0324 {
                    if count<3 { return Err(Error::Invalid("polygon requires three vertices".into())); }
                    svg.push_str(&format!("<polygon points=\"{points}\" {attributes}/>"));
                } else { svg.push_str(&format!("<polyline points=\"{points}\" fill=\"none\" stroke=\"{pen}\" stroke-width=\"{pen_width}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>")); }
            }
            (true,19) | (false,0x0106) => {
                exact(if emf {12}else{8})?;
                fill_rule = match coordinate(header)? {1=>"evenodd",2=>"nonzero",_=>return Err(Error::Unsupported("metafile fill mode".into()))};
            }
            _ => return Err(Error::Unsupported(format!("metafile record {kind:#x} is not in the inert vector conversion subset"))),
        }
        if svg.len()>250000 { return Err(Error::Limit("converted metafile SVG exceeds budget".into())); }
    }
    if !ended || expected_records.is_some_and(|count| count!=records) || (!emf && meta_u32(&bytes,34)? as usize!=maximum_record/2) { return Err(Error::Invalid("metafile EOF or record-count mismatch".into())); }
    svg.push_str("</svg>");
    Ok(svg)
}