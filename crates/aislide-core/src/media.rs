use crate::{Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageFormat, ImageReader, Limits};
use serde::{Serialize, Deserialize};
use sha2::{Digest, Sha256};
use std::io::Cursor;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterInfo { pub width: u32, pub height: u32, pub mime_type: String, pub sha256: String, pub byte_length: usize }

pub fn create_asset(id: &str, base64: String, mime_type: &str, alt: &str, size: f64) -> Result<crate::model::Element> {
    if !size.is_finite() || !(8.0..=640.0).contains(&size) { return Err(Error::Invalid("asset size must be 8-640 pixels".into())); }
    let (data, mime) = if mime_type == "image/svg+xml" { (svg_png(&base64)?, "image/png") } else { (base64, mime_type) };
    let mut picture = create_picture(id, data, mime, alt)?;
    if let crate::model::Element::Picture { width, height, y, .. } = &mut picture {
        let scale = size / width.max(*height); *width *= scale; *height *= scale; *y = 180.0;
    }
    Ok(picture)
}

fn svg_png(encoded: &str) -> Result<String> {
    if encoded.len() > 349528 { return Err(Error::Limit("SVG exceeds 256 KiB".into())); }
    let bytes = STANDARD.decode(encoded).map_err(|_| Error::Invalid("invalid SVG base64".into()))?;
    if bytes.len() > 262144 { return Err(Error::Limit("SVG exceeds 256 KiB".into())); }
    let svg = std::str::from_utf8(&bytes).map_err(|_| Error::Invalid("SVG must be UTF-8".into()))?.trim_start_matches('\u{feff}');
    let document = crate::pptx::parse(svg)?;
    const SVG: &str = "http://www.w3.org/2000/svg";
    if !document.root_element().has_tag_name((SVG, "svg")) { return Err(Error::Invalid("an SVG root with its standard namespace is required".into())); }
    let elements = ["svg", "g", "path", "rect", "circle", "ellipse", "line", "polyline", "polygon", "defs", "linearGradient", "radialGradient", "stop", "clipPath", "mask", "title", "desc"];
    let attributes = ["id", "class", "viewBox", "preserveAspectRatio", "width", "height", "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "rx", "ry", "fx", "fy", "fr", "d", "points", "fill", "fill-rule", "fill-opacity", "stroke", "stroke-width", "stroke-linecap", "stroke-linejoin", "stroke-miterlimit", "stroke-dasharray", "stroke-dashoffset", "stroke-opacity", "opacity", "color", "transform", "clip-path", "clip-rule", "mask", "maskUnits", "maskContentUnits", "gradientUnits", "gradientTransform", "spreadMethod", "offset", "stop-color", "stop-opacity", "href", "version", "role", "aria-hidden", "aria-label", "focusable", "shape-rendering", "vector-effect"];
    let mut ids = std::collections::BTreeSet::new(); let mut count = 0;
    for node in document.descendants().filter(|node| node.is_element()) {
        count += 1;
        if count > 2048 || node.ancestors().count() > 32 { return Err(Error::Limit("SVG exceeds element/depth limits".into())); }
        if node.tag_name().namespace() != Some(SVG) || !elements.contains(&node.tag_name().name()) { return Err(Error::Unsupported("SVG contains unsupported content; export outlined paths or PNG instead".into())); }
        if let Some(id) = node.attribute("id") { if !ids.insert(id) { return Err(Error::Invalid("duplicate SVG identity".into())); } }
        for attribute in node.attributes() {
            if !attributes.contains(&attribute.name()) || attribute.namespace().is_some_and(|namespace| namespace != "http://www.w3.org/1999/xlink") { return Err(Error::Unsupported("SVG attribute is outside the inert icon subset".into())); }
            let value = attribute.value().trim();
            let local_reference = |value: &str| value.strip_prefix('#').is_some_and(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':')));
            if attribute.name() == "href" && !local_reference(value) { return Err(Error::Unsupported("external SVG references are not allowed".into())); }
            if value.to_ascii_lowercase().contains("url(") {
                let target = value.strip_prefix("url(").and_then(|value| value.strip_suffix(')')).map(|value| value.trim().trim_matches(['\'', '"']));
                if !target.is_some_and(local_reference) { return Err(Error::Unsupported("SVG paint references must be local fragment IDs".into())); }
            }
        }
    }
    validate_svg_references(&document)?;
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg, &options).map_err(|_| Error::Invalid("SVG cannot be parsed for rendering".into()))?;
    let dimensions = tree.size();
    if dimensions.width() > 4096.0 || dimensions.height() > 4096.0 { return Err(Error::Limit("SVG viewport exceeds 4096 pixels".into())); }
    let factor = 1024.0 / dimensions.width().max(dimensions.height());
    let width = (dimensions.width() * factor).round().max(1.0) as u32;
    let height = (dimensions.height() * factor).round().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).ok_or_else(|| Error::Limit("SVG raster allocation".into()))?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(factor, factor), &mut pixmap.as_mut());
    if pixmap.pixels().iter().all(|pixel| pixel.alpha() == 0) { return Err(Error::Invalid("SVG has no visible graphics".into())); }
    let png = pixmap.encode_png().map_err(|_| Error::Invalid("SVG PNG encoding failed".into()))?;
    if png.len() > 1024 * 1024 { return Err(Error::Limit("rendered SVG exceeds the 1 MiB image limit".into())); }
    Ok(STANDARD.encode(png))
}

fn validate_svg_references(document: &roxmltree::Document<'_>) -> Result<()> {
    let nodes: Vec<_> = document.descendants().filter(|node| node.is_element()).collect();
    let positions: std::collections::BTreeMap<_, _> = nodes.iter().enumerate().map(|(index, node)| (node.range().start, index)).collect();
    let identities: std::collections::BTreeMap<_, _> = nodes.iter().enumerate().filter_map(|(index, node)| node.attribute("id").map(|id| (id, index))).collect();
    let mut edges = vec![Vec::new(); nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        edges[index].extend(node.children().filter(|child| child.is_element()).map(|child| positions[&child.range().start]));
        for attribute in node.attributes() {
            let value = attribute.value().trim();
            let target = if attribute.name() == "href" { value.strip_prefix('#') }
                else { value.strip_prefix("url(").and_then(|value| value.strip_suffix(')')).map(|value| value.trim().trim_matches(['\'', '"'])).and_then(|value| value.strip_prefix('#')) };
            let Some(target) = target else { continue };
            let target = *identities.get(target).ok_or_else(|| Error::Invalid("SVG fragment target is missing".into()))?;
            let target_kind = nodes[target].tag_name().name();
            let supported = match attribute.name() {
                "mask" => target_kind == "mask",
                "clip-path" => target_kind == "clipPath",
                "fill" | "stroke" => ["linearGradient", "radialGradient"].contains(&target_kind),
                "href" => ["linearGradient", "radialGradient"].contains(&node.tag_name().name()) && ["linearGradient", "radialGradient"].contains(&target_kind),
                _ => false,
            };
            if !supported { return Err(Error::Unsupported("SVG fragment target has an unsupported type".into())); }
            edges[index].push(target);
        }
    }
    fn visit(index: usize, level: usize, edges: &[Vec<usize>], active: &mut [bool], memo: &mut [Option<(usize, usize)>]) -> Result<(usize, usize)> {
        if level > 48 { return Err(Error::Limit("SVG reference depth exceeds the icon budget".into())); }
        if active[index] { return Err(Error::Invalid("cyclic SVG references are not supported".into())); }
        if let Some(result) = memo[index] { return Ok(result); }
        active[index] = true;
        let mut cost: usize = 1; let mut depth = 1;
        for target in &edges[index] {
            let (child_cost, child_depth) = visit(*target, level + 1, edges, active, memo)?;
            cost = cost.saturating_add(child_cost); depth = depth.max(child_depth + 1);
            if cost > 4096 || depth > 48 { return Err(Error::Limit("SVG reference expansion exceeds the icon budget".into())); }
        }
        active[index] = false; memo[index] = Some((cost, depth)); Ok((cost, depth))
    }
    let mut active = vec![false; nodes.len()]; let mut memo = vec![None; nodes.len()];
    for index in 0..nodes.len() { visit(index, 1, &edges, &mut active, &mut memo)?; }
    Ok(())
}

pub fn create_picture(id: &str, base64: String, mime_type: &str, alt: &str) -> Result<crate::model::Element> {
    crate::model::valid_text(id, 80)?;
    crate::model::valid_text(alt, 500)?;
    if id.is_empty() { return Err(Error::Invalid("picture ID is required".into())); }
    let info = inspect_raster(&base64, mime_type)?;
    let scale = (800.0 / info.width as f64).min(350.0 / info.height as f64);
    Ok(crate::model::Element::Picture { id: id.into(), x: 100.0, y: 230.0, width: info.width as f64 * scale, height: info.height as f64 * scale,
        base64, mime_type: mime_type.into(), alt: alt.into(), crop: Default::default() })
}

pub fn inspect_raster(base64: &str, mime_type: &str) -> Result<RasterInfo> {
    if base64.len() > 1_398_104 { return Err(Error::Limit("image > 1 MiB".into())); }
    let bytes = STANDARD.decode(base64).map_err(|_| Error::Invalid("invalid image base64".into()))?;
    if bytes.len() > 1024 * 1024 { return Err(Error::Limit("image > 1 MiB".into())); }
    let expected = match mime_type {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        _ => return Err(Error::Unsupported("only PNG and JPEG raster images are accepted".into())),
    };
    let actual = image::guess_format(&bytes).map_err(|_| Error::Invalid("unrecognized raster image".into()))?;
    if actual != expected { return Err(Error::Invalid("image content does not match its media type".into())); }
    let mut limits = Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    let mut reader = ImageReader::with_format(Cursor::new(&bytes), actual);
    reader.limits(limits);
    let image = reader.decode().map_err(|_| Error::Invalid("image cannot be decoded within the 4096px/64MiB limits".into()))?;
    if image.width() == 0 || image.height() == 0 { return Err(Error::Invalid("image has empty dimensions".into())); }
    Ok(RasterInfo { width: image.width(), height: image.height(), mime_type: mime_type.into(), sha256: format!("{:x}", Sha256::digest(&bytes)), byte_length: bytes.len() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn svg_assets_render_to_bounded_transparent_png() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><defs><linearGradient id="paint"><stop stop-color="#0017c1"/><stop offset="1" stop-color="#007f73"/></linearGradient></defs><path d="M4 4h16v16H4z" fill="url(#paint)"/></svg>"##;
        let asset = crate::execute_request(json!({"op":"create_asset","id":"icon","base64":STANDARD.encode(svg),"mime_type":"image/svg+xml","alt":"Figma icon","size":96})).unwrap();
        assert_eq!(asset["type"], "picture");
        assert_eq!(asset["mime_type"], "image/png");
        assert_eq!(asset["width"], 96.0);
        let bytes = STANDARD.decode(asset["base64"].as_str().unwrap()).unwrap();
        let image = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(image.width(), 1024);
        assert_eq!(image.get_pixel(0, 0)[3], 0);
        assert_eq!(image.get_pixel(512, 512)[3], 255);
        assert!(image.get_pixel(512, 512)[2] > 80);
    }

    #[test]
    fn svg_assets_reject_active_external_and_excessive_content() {
        for body in ["<script>alert(1)</script>", "<foreignObject/>", "<image href='file:///secret.png'/>", "<image href='https://example.com/image.png'/>", "<path onload='alert(1)' d='M0 0h24v24z'/>", "<path fill='url(https://example.com/paint)' d='M0 0h24v24z'/>", "<style>@import url(https://example.com/font.css);</style>", "<text>Hello</text>", "<filter id='blur'><feGaussianBlur stdDeviation='200'/></filter>"] {
            let svg = format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'>{body}</svg>");
            assert!(crate::execute_request(json!({"op":"create_asset","id":"unsafe","base64":STANDARD.encode(svg),"mime_type":"image/svg+xml","alt":"Unsafe","size":96})).is_err(), "{body}");
        }
        let oversized = format!("<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'>{}</svg>", " ".repeat(262144));
        assert!(crate::execute_request(json!({"op":"create_asset","id":"huge","base64":STANDARD.encode(oversized),"mime_type":"image/svg+xml","alt":"Huge","size":96})).is_err());
    }

    #[test]
    fn svg_reference_graph_rejects_cycles_missing_targets_and_expansion() {
        let cycle = "<defs><mask id='first'><rect width='24' height='24' mask='url(#second)'/></mask><mask id='second'><rect width='24' height='24' mask='url(#first)'/></mask></defs>";
        let mut expansion = String::from("<defs>");
        for index in 0..10 { expansion.push_str(&format!("<mask id='mask{index}'><rect width='24' height='24' mask='url(#mask{})'/><rect width='24' height='24' mask='url(#mask{})'/></mask>", index + 1, index + 1)); }
        expansion.push_str("<mask id='mask10'><rect width='24' height='24'/></mask></defs>");
        for fragment in [cycle, expansion.as_str(), "<defs><mask id='test'><path d='M0 0h24v24z' fill='url(#missing)'/></mask></defs>", "<path id='target' d='M0 0h24v24z'/><rect width='24' height='24' mask='url(#target)'/>"] {
            let svg = format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'>{fragment}<rect width='24' height='24' fill='#0017c1'/></svg>");
            assert!(svg_png(&STANDARD.encode(svg)).is_err(), "unsafe SVG reference graph was accepted");
        }
    }
}