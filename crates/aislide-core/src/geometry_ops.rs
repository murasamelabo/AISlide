use crate::{model::{Deck, Element, element_list, validate_deck}, selection::{ClipboardFormat, SelectionOperation, SelectionResult}, vector::{PathCommand, VectorPath}, visual::VisualStyle, Error, Result};
use geo::{Area, BooleanOps, BoundingRect, Contains, Intersects, LineString, MultiPolygon, Polygon, Validation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_SHAPES: usize = 32;
pub const MAX_VERTICES: usize = 4096;
pub const MAX_EDGE_PAIRS: usize = 262144;
pub const FLATTEN_TOLERANCE: f64 = 0.25;
pub const MAX_FRAGMENTS: usize = 128;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BooleanOperation { Union, Intersect, Subtract, Xor, Fragment }

pub fn capabilities() -> serde_json::Value {
    serde_json::json!({"operations":["union","intersect","subtract","xor","fragment"],"max_shapes":MAX_SHAPES,"max_vertices":MAX_VERTICES,"max_path_commands":256,"max_edge_pairs":MAX_EDGE_PAIRS,"max_fragments":MAX_FRAGMENTS,"native_coordinate_units":1000000,"fill_rule":"nonzero_opposite_winding","clipping_fill_rule":"even_odd","curve_flattening":true,"flatten_tolerance_px":FLATTEN_TOLERANCE,"flatten_max_depth":20,"curve_output":"polygon","empty_result":"reject_without_changes","touching_contours":"reject","style_reference":"first_supplied_id","backend":"geo/i_overlay","office_visual_parity":false})
}

fn flatten(control: &[[f64; 2]], tolerance: f64, depth: usize, points: &mut Vec<(f64,f64)>) -> Result<()> {
    if points.len() >= MAX_VERTICES { return Err(Error::Limit("curve flattening exceeds 4096 vertices".into())); }
    let start = control[0]; let end = control[control.len()-1];
    let delta = [end[0]-start[0],end[1]-start[1]];
    let length_squared = delta[0]*delta[0]+delta[1]*delta[1];
    let flat = control[1..control.len()-1].iter().all(|point| {
        let projection = if length_squared == 0.0 { 0.0 } else { (((point[0]-start[0])*delta[0]+(point[1]-start[1])*delta[1])/length_squared).clamp(0.0,1.0) };
        (point[0]-start[0]-projection*delta[0]).hypot(point[1]-start[1]-projection*delta[1]) <= tolerance
    });
    if flat { points.push((end[0],end[1])); return Ok(()); }
    if depth >= 20 { return Err(Error::Limit("curve cannot meet flattening tolerance within 20 subdivisions".into())); }
    let mut level = control.to_vec();
    let mut left = vec![start]; let mut right = vec![end];
    while level.len()>1 {
        level = level.windows(2).map(|pair| [(pair[0][0]+pair[1][0])/2.0,(pair[0][1]+pair[1][1])/2.0]).collect();
        left.push(level[0]); right.push(level[level.len()-1]);
    }
    right.reverse();
    flatten(&left,tolerance,depth+1,points)?;
    flatten(&right,tolerance,depth+1,points)
}

fn rings(path: &VectorPath, tolerance: f64) -> Result<Vec<LineString<f64>>> {
    let mut contours = Vec::new();
    let mut points = Vec::new();
    let mut total = 0;
    for command in &path.commands {
        match command {
            PathCommand::Move { point } | PathCommand::Line { point } => points.push((point[0], point[1])),
            PathCommand::Quadratic { control, point } => {
                let start = points.last().ok_or_else(|| Error::Invalid("curve without move".into()))?;
                flatten(&[[start.0,start.1],*control,*point],tolerance,0,&mut points)?;
            }
            PathCommand::Cubic { control1, control2, point } => {
                let start = points.last().ok_or_else(|| Error::Invalid("curve without move".into()))?;
                flatten(&[[start.0,start.1],*control1,*control2,*point],tolerance,0,&mut points)?;
            }
            PathCommand::Close => {
                total += points.len();
                if total > MAX_VERTICES { return Err(Error::Limit("flattened contours exceed 4096 vertices".into())); }
                let mut ring = LineString::from(std::mem::take(&mut points));
                ring.close();
                contours.push(ring);
            }
        }
    }
    Ok(contours)
}

fn topology(rings: Vec<LineString<f64>>) -> Result<MultiPolygon<f64>> {
    let contours: Vec<_> = rings.into_iter().map(|ring| Polygon::new(ring, vec![])).collect();
    for contour in &contours {
        contour.check_validation().map_err(|error| Error::Invalid(format!("invalid boolean contour: {error}")))?;
        if contour.unsigned_area() <= 0.0 { return Err(Error::Invalid("zero-area boolean contour".into())); }
    }
    let mut parents = vec![None; contours.len()];
    for (index, contour) in contours.iter().enumerate() {
        for (other_index, other) in contours.iter().enumerate() {
            if index == other_index { continue; }
            if contour.exterior().intersects(other.exterior()) { return Err(Error::Unsupported("touching or crossing compound contours are not supported".into())); }
            if other.contains(contour) && parents[index].is_none_or(|parent: usize| other.unsigned_area() < contours[parent].unsigned_area()) {
                parents[index] = Some(other_index);
            }
        }
    }
    let mut depths = vec![0; contours.len()];
    for index in 0..contours.len() {
        let mut parent = parents[index];
        while let Some(ancestor) = parent {
            depths[index] += 1;
            if depths[index] > contours.len() { return Err(Error::Invalid("cyclic contour containment".into())); }
            parent = parents[ancestor];
        }
        if let Some(parent) = parents[index] {
            if contours[index].signed_area().is_sign_positive() == contours[parent].signed_area().is_sign_positive() {
                return Err(Error::Unsupported("nested contours require opposite winding for native nonzero fill".into()));
            }
        }
    }
    let polygons = contours.iter().enumerate().filter(|(index,_)| depths[*index] % 2 == 0).map(|(index,contour)| {
        Polygon::new(contour.exterior().clone(), contours.iter().enumerate().filter(|(child,_)| parents[*child] == Some(index)).map(|(_,hole)| hole.exterior().clone()).collect())
    }).collect();
    let geometry = MultiPolygon(polygons);
    geometry.check_validation().map_err(|error| Error::Invalid(format!("invalid boolean geometry: {error}")))?;
    Ok(geometry)
}

pub(crate) fn validate_compound(path: &VectorPath) -> Result<()> {
    topology(rings(path,0.00001)?)?;
    Ok(())
}

fn geometry(element: &Element) -> Result<MultiPolygon<f64>> {
    let default = VisualStyle::default();
    let visual = element.visual().unwrap_or(&default);
    let supported = VisualStyle { opacity: visual.opacity, path: visual.path.clone(), ..Default::default() };
    if *visual != supported { return Err(Error::Unsupported("boolean geometry does not preserve transforms, hidden/locked shapes, gradients, or effects".into())); }
    let (points, fill) = match element {
        Element::Rect { fill, .. } => (vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]], fill),
        Element::Polygon { points, fill, .. } => (points.clone(), fill),
        Element::Shape { preset, rotation, text, format, fill, .. } if preset == "rect" && *rotation == 0.0 && text.is_empty() && *format == crate::model::TextFormat::default() => (vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]], fill),
        _ => return Err(Error::Unsupported("boolean operands must be top-level rectangles or polygons without text".into())),
    };
    if fill == "none" { return Err(Error::Unsupported("boolean geometry requires filled shapes".into())); }
    let (_, x, y, width, height) = element.bounds();
    let contours = if let Some(path) = &visual.path { rings(path,FLATTEN_TOLERANCE/width.max(height))? } else {
        let mut ring = LineString::from(points.into_iter().map(|point| (point[0],point[1])).collect::<Vec<_>>());
        ring.close(); vec![ring]
    };
    topology(contours.into_iter().map(|mut ring| {
        for point in &mut ring.0 { point.x = x + point.x * width; point.y = y + point.y * height; }
        ring
    }).collect())
}

fn vertex_count(geometry: &MultiPolygon<f64>) -> usize {
    geometry.0.iter().flat_map(|polygon| std::iter::once(polygon.exterior()).chain(polygon.interiors())).map(|ring| ring.0.len().saturating_sub(1)).sum()
}

fn bounded(geometry: &MultiPolygon<f64>) -> Result<()> {
    if vertex_count(geometry) > MAX_VERTICES { return Err(Error::Limit("boolean geometry exceeds 4096 vertices".into())); }
    Ok(())
}

fn output(geometry: &MultiPolygon<f64>, reference: &Element, id: &str) -> Result<Element> {
    bounded(geometry)?;
    if geometry.0.is_empty() || geometry.unsigned_area() <= 0.0 { return Err(Error::Unsupported("boolean result is empty; no changes applied".into())); }
    let bounds = geometry.bounding_rect().ok_or_else(|| Error::Invalid("boolean result has no bounds".into()))?;
    let width = bounds.width(); let height = bounds.height();
    if width <= 0.0 || height <= 0.0 { return Err(Error::Invalid("boolean result has zero dimensions".into())); }
    let mut commands = Vec::new();
    for polygon in &geometry.0 {
        for (index, ring) in std::iter::once(polygon.exterior()).chain(polygon.interiors()).enumerate() {
            let mut points: Vec<_> = ring.0.iter().take(ring.0.len().saturating_sub(1)).map(|point| [
                (((point.x-bounds.min().x)/width)*1e6).round()/1e6,
                (((point.y-bounds.min().y)/height)*1e6).round()/1e6,
            ]).collect();
            if points.iter().flatten().any(|value| !value.is_finite() || !(0.0..=1.0).contains(value)) { return Err(Error::Invalid("boolean output outside normalized bounds".into())); }
            if Polygon::new(ring.clone(),vec![]).signed_area().is_sign_positive() != (index == 0) { points.reverse(); }
            for (point_index, point) in points.into_iter().enumerate() {
                commands.push(if point_index == 0 { PathCommand::Move { point } } else { PathCommand::Line { point } });
            }
            commands.push(PathCommand::Close);
        }
    }
    let path = VectorPath { commands };
    let quantized = topology(rings(&path,0.00001)?)?;
    if quantized.0.len() != geometry.0.len() || quantized.0.iter().map(|polygon| polygon.interiors().len()).sum::<usize>() != geometry.0.iter().map(|polygon| polygon.interiors().len()).sum::<usize>() { return Err(Error::Unsupported("boolean output topology changed at native precision".into())); }
    let (fill, stroke, stroke_width) = match reference {
        Element::Rect { fill, .. } => (fill.clone(), "@dk1".into(), 0.0),
        Element::Polygon { fill, stroke, stroke_width, .. } | Element::Shape { fill, stroke, stroke_width, .. } => (fill.clone(),stroke.clone(),*stroke_width),
        _ => return Err(Error::Unsupported("boolean reference style".into())),
    };
    let mut visual = VisualStyle { opacity: reference.visual().and_then(|style| style.opacity), ..Default::default() };
    let points = path.points();
    if path.requires_override() { path.validate()?; visual.path = Some(path); }
    Ok(Element::Polygon { id:id.into(), x:bounds.min().x, y:bounds.min().y, width, height, points, fill, stroke, stroke_width, visual:(visual != VisualStyle::default()).then_some(visual) })
}

pub fn combine(deck: &Deck, slide_id: &str, ids: &[String], operation: BooleanOperation, result_id: &str) -> Result<SelectionResult> {
    validate_deck(deck)?;
    if !(2..=MAX_SHAPES).contains(&ids.len()) { return Err(Error::Limit("combine_shapes requires 2-32 shapes".into())); }
    if deck.slides.iter().flat_map(|slide| element_list(&slide.elements)).any(|element| element.bounds().0 == result_id) { return Err(Error::Conflict("boolean result ID must be new".into())); }
    let slide = deck.slides.iter().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("boolean slide not found".into()))?;
    let operands = ids.iter().map(|id| slide.elements.iter().find(|element| element.bounds().0 == id).ok_or_else(|| Error::Unsupported("boolean operands must be existing top-level shapes".into()))).collect::<Result<Vec<_>>>()?;
    let mut total = 0usize;
    for element in &operands {
        total += match element { Element::Polygon { points, .. } => points.len(), _ => 4 };
    }
    if total > MAX_VERTICES { return Err(Error::Limit("boolean inputs exceed 4096 total vertices".into())); }
    let shapes = operands.iter().map(|element| geometry(element)).collect::<Result<Vec<_>>>()?;
    if shapes.iter().map(vertex_count).sum::<usize>() > MAX_VERTICES { return Err(Error::Limit("flattened inputs exceed 4096 total vertices".into())); }
    let mut regions = vec![shapes[0].clone()];
    let mut work = 0;
    for shape in &shapes[1..] {
        if matches!(operation,BooleanOperation::Fragment) {
            let mut next = Vec::new();
            let mut remaining = shape.clone();
            for region in &regions {
                for (left,right) in [(region,shape),(&remaining,region)] { charge(left,right,&mut work)?; }
                charge(region,shape,&mut work)?;
                next.extend([region.difference(shape),region.intersection(shape)].into_iter().filter(|part| !part.0.is_empty()));
                remaining = remaining.difference(region);
                bounded(&remaining)?;
            }
            if !remaining.0.is_empty() { next.push(remaining); }
            regions = next;
        } else {
            charge(&regions[0],shape,&mut work)?;
            regions[0] = match operation {
                BooleanOperation::Union => regions[0].union(shape),
                BooleanOperation::Intersect => regions[0].intersection(shape),
                BooleanOperation::Subtract => regions[0].difference(shape),
                BooleanOperation::Xor => regions[0].xor(shape),
                BooleanOperation::Fragment => unreachable!(),
            };
        }
        if regions.iter().map(|region| region.0.len()).sum::<usize>() > MAX_FRAGMENTS { return Err(Error::Limit("boolean result exceeds 128 regions".into())); }
        for region in &regions { bounded(region)?; }
    }
    if matches!(operation,BooleanOperation::Fragment) { regions = regions.into_iter().flat_map(|region| region.0.into_iter().map(|polygon| MultiPolygon(vec![polygon]))).collect(); }
    if regions.iter().map(vertex_count).sum::<usize>() > MAX_VERTICES { return Err(Error::Limit("boolean outputs exceed 4096 total vertices".into())); }
    let elements = regions.iter().enumerate().map(|(index,region)| output(region,operands[0],&if index == 0 { result_id.into() } else { format!("{result_id}-{}",index+1) })).collect::<Result<Vec<_>>>()?;
    let added_ids: Vec<_> = elements.iter().map(|element| element.bounds().0.to_owned()).collect();
    if deck.slides.iter().flat_map(|slide| element_list(&slide.elements)).any(|element| added_ids.iter().any(|id| id == element.bounds().0)) { return Err(Error::Conflict("boolean output ID already exists".into())); }
    let index = slide.elements.iter().position(|element| ids.contains(&element.bounds().0.to_owned())).ok_or_else(|| Error::Invalid("boolean selection empty".into()))?;
    let mut result = crate::selection::apply(deck,slide_id,&SelectionOperation::Cut { ids:ids.to_vec(), format:ClipboardFormat::KeepSourceFormatting },None)?;
    let target = result.deck.slides.iter_mut().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("boolean target missing".into()))?;
    target.elements.splice(index..index,elements);
    result.clipboard = None;
    result.effects.added_ids = added_ids;
    result.effects.warnings = vec![format!("Combined {} shapes using '{}' as the style and subtraction reference. Curves flatten to polygons with maximum 0.25 slide-pixel deviation before clipping; native coordinates additionally round to 1/1,000,000 of the result bounds.", ids.len(), ids[0])];
    validate_deck(&result.deck)?;
    Ok(result)
}

fn charge(left: &MultiPolygon<f64>, right: &MultiPolygon<f64>, work: &mut usize) -> Result<()> {
    *work += vertex_count(left) * vertex_count(right);
    if *work > MAX_EDGE_PAIRS { return Err(Error::Limit("boolean clipping work budget exceeds 262144 edge pairs".into())); }
    Ok(())
}