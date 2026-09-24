use crate::{Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageFormat, ImageReader, Limits};
use serde::{Serialize, Deserialize};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, collections::VecDeque};
use std::io::Cursor;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterInfo { pub width: u32, pub height: u32, pub mime_type: String, pub sha256: String, pub byte_length: usize }

const RASTER_INFO_CACHE_CAPACITY: usize = 512;

thread_local! {
    static RASTER_INFO_CACHE: RefCell<VecDeque<RasterInfo>> = const { RefCell::new(VecDeque::new()) };
}

#[cfg(test)]
thread_local! {
    static RASTER_DECODE_ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub fn create_asset(id: &str, base64: String, mime_type: &str, alt: &str, size: f64) -> Result<crate::model::Element> {
    if !size.is_finite() || !(8.0..=640.0).contains(&size) { return Err(Error::Invalid("asset size must be 8-640 pixels".into())); }
    if matches!(mime_type,"image/emf" | "image/wmf") {
        let svg = crate::vector::metafile_svg(&base64,mime_type)?;
        return create_asset(id,STANDARD.encode(svg),"image/svg+xml",alt,size);
    }
    let svg = (mime_type == "image/svg+xml").then(|| base64.clone());
    let (data, mime) = if mime_type == "image/svg+xml" { (crate::vector::prepare_svg(&base64)?, "image/png") } else { (base64, mime_type) };
    let mut picture = create_picture(id, data, mime, alt)?;
    if let crate::model::Element::Picture { width, height, y, svg: retained, .. } = &mut picture {
        *retained = svg;
        let scale = size / width.max(*height); *width *= scale; *height *= scale; *y = 180.0;
    }
    Ok(picture)
}

pub(crate) fn icon_raster(base64: String, mime_type: &str) -> Result<(String, String)> {
    if matches!(mime_type,"image/emf" | "image/wmf") {
        return icon_raster(STANDARD.encode(crate::vector::metafile_svg(&base64,mime_type)?),"image/svg+xml");
    }
    if mime_type == "image/svg+xml" {
        let data = svg_png(&base64, 256)?;
        inspect_raster(&data, "image/png")?;
        return Ok((data, "image/png".into()));
    }
    let info = inspect_raster(&base64, mime_type)?;
    if info.width <= 256 && info.height <= 256 { return Ok((base64, mime_type.into())); }
    let bytes = STANDARD.decode(base64).map_err(|_| Error::Invalid("invalid image base64".into()))?;
    let format = if mime_type == "image/png" { ImageFormat::Png } else { ImageFormat::Jpeg };
    let image = decode_image(&bytes, format)?.thumbnail(256, 256);
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, format).map_err(|_| Error::Invalid("graph icon encoding failed".into()))?;
    let data = STANDARD.encode(output.into_inner());
    inspect_raster(&data, mime_type)?;
    Ok((data, mime_type.into()))
}

pub(crate) fn svg_png(encoded: &str, resolution: u32) -> Result<String> {
    if encoded.len() > 349528 { return Err(Error::Limit("SVG exceeds 256 KiB".into())); }
    let bytes = STANDARD.decode(encoded).map_err(|_| Error::Invalid("invalid SVG base64".into()))?;
    if bytes.len() > 262144 { return Err(Error::Limit("SVG exceeds 256 KiB".into())); }
    let svg = std::str::from_utf8(&bytes).map_err(|_| Error::Invalid("SVG must be UTF-8".into()))?.trim_start_matches('\u{feff}');
    let document = crate::pptx::parse(svg)?;
    if document.descendants().any(|node| node.is_pi()) { return Err(Error::Unsupported("SVG processing instructions are not allowed".into())); }
    const SVG: &str = "http://www.w3.org/2000/svg";
    if !document.root_element().has_tag_name((SVG, "svg")) { return Err(Error::Invalid("an SVG root with its standard namespace is required".into())); }
    let elements = ["svg", "g", "path", "rect", "circle", "ellipse", "line", "polyline", "polygon", "defs", "linearGradient", "radialGradient", "stop", "clipPath", "mask", "title", "desc", "text", "tspan", "image"];
    let attributes = ["id", "class", "viewBox", "preserveAspectRatio", "width", "height", "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "rx", "ry", "fx", "fy", "fr", "d", "points", "fill", "fill-rule", "fill-opacity", "stroke", "stroke-width", "stroke-linecap", "stroke-linejoin", "stroke-miterlimit", "stroke-dasharray", "stroke-dashoffset", "stroke-opacity", "opacity", "color", "transform", "clip-path", "clip-rule", "mask", "maskUnits", "maskContentUnits", "gradientUnits", "gradientTransform", "spreadMethod", "offset", "stop-color", "stop-opacity", "href", "version", "role", "aria-hidden", "aria-label", "focusable", "shape-rendering", "vector-effect"];
    let text_attributes = ["font-family","font-size","font-weight","font-style","text-anchor","dominant-baseline","dx","dy","letter-spacing","word-spacing","text-decoration"];
    let mut ids = std::collections::BTreeSet::new(); let mut count = 0;
    let mut image_count = 0; let mut image_pixels = 0u64; let mut characters = 0usize; let mut has_text = false;
    for node in document.descendants().filter(|node| node.is_element()) {
        count += 1;
        if count > 2048 || node.ancestors().count() > 32 { return Err(Error::Limit("SVG exceeds element/depth limits".into())); }
        if node.tag_name().namespace() != Some(SVG) || !elements.contains(&node.tag_name().name()) { return Err(Error::Unsupported("SVG contains unsupported content; export outlined paths or PNG instead".into())); }
        if node.has_tag_name((SVG,"text")) || node.has_tag_name((SVG,"tspan")) {
            has_text = true;
            characters += node.children().filter(|child| child.is_text()).map(|child| child.text().unwrap_or_default().chars().count()).sum::<usize>();
            if characters>16000 { return Err(Error::Limit("SVG text exceeds 16000 characters".into())); }
        }
        if node.has_tag_name((SVG,"image")) {
            image_count += 1;
            if image_count>8 { return Err(Error::Limit("SVG exceeds eight embedded raster images".into())); }
            let references: Vec<_> = node.attributes().filter(|attribute| attribute.name()=="href").collect();
            if references.len()!=1 { return Err(Error::Invalid("SVG image requires exactly one embedded data reference".into())); }
            let (mime,data) = embedded_raster(references[0].value())?;
            let info = inspect_raster(data,mime)?;
            image_pixels += u64::from(info.width)*u64::from(info.height);
            if image_pixels>16*1024*1024 { return Err(Error::Limit("SVG embedded rasters exceed 16 million decoded pixels".into())); }
        }
        if let Some(id) = node.attribute("id") { if !ids.insert(id) { return Err(Error::Invalid("duplicate SVG identity".into())); } }
        for attribute in node.attributes() {
            if (!attributes.contains(&attribute.name()) && !text_attributes.contains(&attribute.name())) || attribute.namespace().is_some_and(|namespace| namespace != "http://www.w3.org/1999/xlink" || attribute.name()!="href") { return Err(Error::Unsupported("SVG attribute is outside the inert icon subset".into())); }
            let value = attribute.value().trim();
            if node.has_tag_name((SVG,"image")) && attribute.name()=="href" { continue; }
            let local_reference = |value: &str| value.strip_prefix('#').is_some_and(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':')));
            if attribute.name() == "href" && !local_reference(value) { return Err(Error::Unsupported("external SVG references are not allowed".into())); }
            if value.to_ascii_lowercase().contains("url(") {
                let target = value.strip_prefix("url(").and_then(|value| value.strip_suffix(')')).map(|value| value.trim().trim_matches(['\'', '"']));
                if !target.is_some_and(local_reference) { return Err(Error::Unsupported("SVG paint references must be local fragment IDs".into())); }
            }
        }
    }
    validate_svg_references(&document)?;
    let mut options = resvg::usvg::Options::default();
    options.image_href_resolver.resolve_string = Box::new(|_,_| None);
    options.image_href_resolver.resolve_data = Box::new(|mime,data,_| match mime {
        "image/png" => Some(resvg::usvg::ImageKind::PNG(data)),
        "image/jpeg" => Some(resvg::usvg::ImageKind::JPEG(data)),
        _ => None,
    });
    if has_text {
        static FONTS: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = std::sync::OnceLock::new();
        options.fontdb = FONTS.get_or_init(|| {
            let mut database = resvg::usvg::fontdb::Database::new();
            database.load_system_fonts();
            std::sync::Arc::new(database)
        }).clone();
    }
    let tree = resvg::usvg::Tree::from_str(svg, &options).map_err(|_| Error::Invalid("SVG cannot be parsed for rendering".into()))?;
    let dimensions = tree.size();
    if dimensions.width() > 4096.0 || dimensions.height() > 4096.0 { return Err(Error::Limit("SVG viewport exceeds 4096 pixels".into())); }
    let factor = resolution as f32 / dimensions.width().max(dimensions.height());
    let width = (dimensions.width() * factor).round().max(1.0) as u32;
    let height = (dimensions.height() * factor).round().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).ok_or_else(|| Error::Limit("SVG raster allocation".into()))?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(factor, factor), &mut pixmap.as_mut());
    if pixmap.pixels().iter().all(|pixel| pixel.alpha() == 0) { return Err(Error::Invalid("SVG has no visible graphics".into())); }
    let png = pixmap.encode_png().map_err(|_| Error::Invalid("SVG PNG encoding failed".into()))?;
    if png.len() > 1024 * 1024 { return Err(Error::Limit("rendered SVG exceeds the 1 MiB image limit".into())); }
    Ok(STANDARD.encode(png))
}

fn embedded_raster(value: &str) -> Result<(&str,&str)> {
    for mime in ["image/png","image/jpeg"] {
        if let Some(data) = value.strip_prefix(&format!("data:{mime};base64,")) { return Ok((mime,data)); }
    }
    Err(Error::Unsupported("SVG images require embedded base64 PNG or JPEG; no external or nested SVG references".into()))
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
        base64, mime_type: mime_type.into(), alt: alt.into(), crop: Default::default(), visual: None, svg: None })
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
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    if let Some(info) = RASTER_INFO_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let index = cache.iter().position(|info| info.sha256 == sha256 && info.mime_type == mime_type
            && info.byte_length == bytes.len() && (1..=4096).contains(&info.width) && (1..=4096).contains(&info.height))?;
        let info = cache.remove(index)?;
        cache.push_back(info.clone());
        Some(info)
    }) { return Ok(info); }
    let info = decode_raster(&bytes, actual, mime_type, sha256)?;
    RASTER_INFO_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= RASTER_INFO_CACHE_CAPACITY { cache.pop_front(); }
        cache.push_back(info.clone());
    });
    Ok(info)
}

fn decode_raster(bytes: &[u8], format: ImageFormat, mime_type: &str, sha256: String) -> Result<RasterInfo> {
    let image = decode_image(bytes, format)?;
    Ok(RasterInfo { width: image.width(), height: image.height(), mime_type: mime_type.into(), sha256, byte_length: bytes.len() })
}

fn decode_image(bytes: &[u8], format: ImageFormat) -> Result<image::DynamicImage> {
    #[cfg(test)]
    RASTER_DECODE_ATTEMPTS.with(|count| count.set(count.get() + 1));
    let mut limits = Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let image = reader.decode().map_err(|_| Error::Invalid("image cannot be decoded within the 4096px/64MiB limits".into()))?;
    if image.width() == 0 || image.height() == 0 { return Err(Error::Invalid("image has empty dimensions".into())); }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reset_raster_cache() {
        RASTER_INFO_CACHE.with(|cache| cache.borrow_mut().clear());
        RASTER_DECODE_ATTEMPTS.with(|count| count.set(0));
    }

    fn raster_decode_attempts() -> usize {
        RASTER_DECODE_ATTEMPTS.with(|count| count.get())
    }

    fn cache_test_png(width: u32, seed: u32) -> String {
        let color = seed.to_le_bytes();
        let image = image::RgbImage::from_pixel(width, 2, image::Rgb([color[0], color[1], color[2]]));
        let mut output = Cursor::new(Vec::new());
        image.write_to(&mut output, ImageFormat::Png).unwrap();
        STANDARD.encode(output.into_inner())
    }

    #[test]
    fn raster_metadata_cache_reuses_191_unique_images_on_second_pass() {
        reset_raster_cache();
        let images: Vec<_> = (0..191).map(|seed| cache_test_png(2, seed)).collect();
        let first: Vec<_> = images.iter().map(|encoded| inspect_raster(encoded, "image/png").unwrap()).collect();
        let unique: std::collections::BTreeSet<_> = first.iter().map(|info| &info.sha256).collect();
        assert_eq!(unique.len(), 191);
        let cold_decodes = raster_decode_attempts();
        assert_eq!(cold_decodes, 191);
        for (encoded, expected) in images.iter().zip(&first) {
            let cached = inspect_raster(encoded, "image/png").unwrap();
            assert_eq!(cached.sha256, expected.sha256);
            assert_eq!(cached.mime_type, expected.mime_type);
            assert_eq!(cached.byte_length, expected.byte_length);
            assert_eq!((cached.width, cached.height), (2, 2));
        }
        let second_pass_decodes = raster_decode_attempts() - cold_decodes;
        eprintln!("raster cache: unique_images=191 cold_decodes={cold_decodes} second_pass_decodes={second_pass_decodes}");
        assert_eq!(second_pass_decodes, 0);
        assert_eq!(RASTER_INFO_CACHE.with(|cache| cache.borrow().len()), 191);
        reset_raster_cache();
    }

    #[test]
    fn raster_metadata_cache_is_bounded_and_refreshes_lru_hits() {
        reset_raster_cache();
        for seed in 0..512 { inspect_raster(&cache_test_png(2, seed), "image/png").unwrap(); }
        assert_eq!(raster_decode_attempts(), 512);
        assert_eq!(RASTER_INFO_CACHE.with(|cache| cache.borrow().len()), 512);
        let oldest = cache_test_png(2, 0);
        let refreshed = inspect_raster(&oldest, "image/png").unwrap();
        assert_eq!(raster_decode_attempts(), 512);
        let evicted_sha256 = format!("{:x}", Sha256::digest(STANDARD.decode(cache_test_png(2, 1)).unwrap()));
        inspect_raster(&cache_test_png(2, 512), "image/png").unwrap();
        assert_eq!(raster_decode_attempts(), 513);
        RASTER_INFO_CACHE.with(|cache| {
            let cache = cache.borrow();
            assert_eq!(cache.len(), 512);
            let retained_bytes = cache.capacity() * std::mem::size_of::<RasterInfo>()
                + cache.iter().map(|info| info.mime_type.capacity() + info.sha256.capacity()).sum::<usize>();
            assert!(retained_bytes <= 200 * 1024);
            assert!(cache.iter().any(|info| info.sha256 == refreshed.sha256));
            assert!(!cache.iter().any(|info| info.sha256 == evicted_sha256));
        });
        inspect_raster(&oldest, "image/png").unwrap();
        assert_eq!(raster_decode_attempts(), 513);
        inspect_raster(&cache_test_png(2, 1), "image/png").unwrap();
        assert_eq!(raster_decode_attempts(), 514);
        for seed in 513..1025 {
            inspect_raster(&cache_test_png(2, seed), "image/png").unwrap();
            assert_eq!(RASTER_INFO_CACHE.with(|cache| cache.borrow().len()), 512);
        }
        assert_eq!(raster_decode_attempts(), 1026);
        reset_raster_cache();
    }

    #[test]
    fn raster_metadata_cache_preserves_content_mime_and_size_checks() {
        reset_raster_cache();
        let encoded = cache_test_png(4, 1);
        let first = inspect_raster(&encoded, "image/png").unwrap();
        assert_eq!((first.width, first.height), (4, 2));
        assert_eq!(inspect_raster(&encoded, "image/png").unwrap().sha256, first.sha256);
        assert_eq!(raster_decode_attempts(), 1);
        assert_eq!(RASTER_INFO_CACHE.with(|cache| cache.borrow().len()), 1);
        assert!(inspect_raster(&encoded, "image/jpeg").is_err());
        assert!(inspect_raster(&encoded, "image/svg+xml").is_err());
        assert!(inspect_raster("!!!!", "image/png").is_err());
        assert!(inspect_raster(&STANDARD.encode(b"not an image"), "image/png").is_err());
        assert!(inspect_raster(&"A".repeat(1_398_105), "image/png").is_err());
        let mut oversized = STANDARD.decode(&encoded).unwrap();
        oversized.resize(1024 * 1024 + 1, 0);
        let oversized = STANDARD.encode(oversized);
        assert_eq!(oversized.len(), 1_398_104);
        assert!(inspect_raster(&oversized, "image/png").is_err());
        assert_eq!(raster_decode_attempts(), 1);
        let bytes = STANDARD.decode(&encoded).unwrap();
        let truncated = STANDARD.encode(&bytes[..20]);
        let too_wide = cache_test_png(4097, 2);
        for rejected in [&truncated, &too_wide] {
            for _ in 0..2 {
                let before = raster_decode_attempts();
                assert!(inspect_raster(rejected, "image/png").is_err());
                assert_eq!(raster_decode_attempts(), before + 1);
                assert_eq!(RASTER_INFO_CACHE.with(|cache| cache.borrow().len()), 1);
            }
        }
        assert_eq!(RASTER_INFO_CACHE.with(|cache| cache.borrow().len()), 1);
        let before = raster_decode_attempts();
        assert_eq!(inspect_raster(&encoded, "image/png").unwrap().sha256, first.sha256);
        assert_eq!(raster_decode_attempts(), before);
        let changed = inspect_raster(&cache_test_png(8, 1), "image/png").unwrap();
        assert_eq!((changed.width, changed.height), (8, 2));
        assert_ne!(changed.sha256, first.sha256);
        assert_eq!(raster_decode_attempts(), before + 1);
        assert_eq!(RASTER_INFO_CACHE.with(|cache| cache.borrow().len()), 2);
        reset_raster_cache();
    }

    #[test]
    fn raster_metadata_cache_reuses_jpeg_and_reset_forces_redecode() {
        reset_raster_cache();
        let image = image::RgbImage::from_pixel(4, 2, image::Rgb([16u8, 32, 160]));
        let mut output = Cursor::new(Vec::new());
        image.write_to(&mut output, ImageFormat::Jpeg).unwrap();
        let encoded = STANDARD.encode(output.into_inner());
        let first = inspect_raster(&encoded, "image/jpeg").unwrap();
        assert_eq!((first.width, first.height), (4, 2));
        assert_eq!(first.mime_type, "image/jpeg");
        assert_eq!(inspect_raster(&encoded, "image/jpeg").unwrap().sha256, first.sha256);
        assert!(inspect_raster(&encoded, "image/png").is_err());
        assert_eq!(raster_decode_attempts(), 1);
        reset_raster_cache();
        assert_eq!(RASTER_INFO_CACHE.with(|cache| cache.borrow().len()), 0);
        assert_eq!(raster_decode_attempts(), 0);
        assert_eq!(inspect_raster(&encoded, "image/jpeg").unwrap().sha256, first.sha256);
        assert_eq!(raster_decode_attempts(), 1);
        reset_raster_cache();
    }

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
        for body in ["<script>alert(1)</script>", "<foreignObject/>", "<image href='file:///secret.png'/>", "<image href='https://example.com/image.png'/>", "<path onload='alert(1)' d='M0 0h24v24z'/>", "<path fill='url(https://example.com/paint)' d='M0 0h24v24z'/>", "<style>@import url(https://example.com/font.css);</style>", "<text><textPath href='https://example.invalid'>Hello</textPath></text>", "<filter id='blur'><feGaussianBlur stdDeviation='200'/></filter>"] {
            let svg = format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'>{body}</svg>");
            assert!(crate::execute_request(json!({"op":"create_asset","id":"unsafe","base64":STANDARD.encode(svg),"mime_type":"image/svg+xml","alt":"Unsafe","size":96})).is_err(), "{body}");
        }
        let oversized = format!("<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'>{}</svg>", " ".repeat(262144));
        assert!(crate::execute_request(json!({"op":"create_asset","id":"huge","base64":STANDARD.encode(oversized),"mime_type":"image/svg+xml","alt":"Huge","size":96})).is_err());
    }

    #[test]
    fn phase2_svg_literal_text_and_embedded_rasters_are_inert_and_visible() {
        let mut output = Cursor::new(Vec::new());
        image::RgbImage::from_pixel(2,2,image::Rgb([0u8,180,60])).write_to(&mut output,ImageFormat::Png).unwrap();
        let png = STANDARD.encode(output.into_inner());
        for body in ["<text x='2' y='22' font-family='sans-serif' font-size='20' fill='#0033aa'>Hello &amp; SVG</text>".into(),format!("<image width='120' height='30' href='data:image/png;base64,{png}'/>")] {
            let svg = format!("<svg xmlns='http://www.w3.org/2000/svg' width='120' height='30'>{body}</svg>");
            let encoded = STANDARD.encode(&svg);
            let asset = create_asset("literal",encoded.clone(),"image/svg+xml","Literal SVG",120.0).unwrap();
            let crate::model::Element::Picture { base64,svg:retained,.. } = asset else { panic!("picture required") };
            assert_eq!(retained,Some(encoded));
            let image = image::load_from_memory(&STANDARD.decode(base64).unwrap()).unwrap().to_rgba8();
            assert!(image.pixels().filter(|pixel| pixel[3]>0).count()>100);
        }
        for target in [format!("data:image/jpeg;base64,{png}"),"data:image/svg+xml;base64,PHN2Zy8+".into(),"data:text/html;base64,PHNjcmlwdC8+".into(),"file:///never-read.png".into()] {
            let svg = format!("<svg xmlns='http://www.w3.org/2000/svg' width='120' height='30'><image width='120' height='30' href='{target}'/></svg>");
            assert!(svg_png(&STANDARD.encode(svg),256).is_err());
        }
    }

    #[test]
    fn phase2_metafiles_convert_explicit_records_and_fail_closed() {
        let mut emf = vec![0u8;88];
        for (offset,value) in [(0,1u32),(4,88),(16,100),(20,100),(40,0x464d4520),(44,0x10000),(48,144),(52,4),(56,1),(72,100),(76,100),(80,25),(84,25)] { emf[offset..offset+4].copy_from_slice(&value.to_le_bytes()); }
        for value in [37u32,12,0x80000004,43,24,10,10,90,90,14,20,0,0,20] { emf.extend(value.to_le_bytes()); }
        let mut wmf = Vec::new();
        for value in [0x9ac6cdd7u32] { wmf.extend(value.to_le_bytes()); }
        for value in [0u16,0,0,100,100,1440,0,0] { wmf.extend(value.to_le_bytes()); }
        let checksum = wmf.chunks_exact(2).map(|chunk| u16::from_le_bytes([chunk[0],chunk[1]])).fold(0,|sum,value| sum^value);
        wmf.extend(checksum.to_le_bytes());
        for value in [1u16,9,0x300,30,0,1,7,0,0] { wmf.extend(value.to_le_bytes()); }
        for record in [vec![7u16,0,0x02fc,0,0x00ff,0,0],vec![4,0,0x012d,0],vec![7,0,0x041b,90,90,10,10],vec![3,0,0]] { for value in record { wmf.extend(value.to_le_bytes()); } }
        for (mime,bytes) in [("image/emf",emf),("image/wmf",wmf)] {
            let asset = create_asset("metafile",STANDARD.encode(&bytes),mime,"Converted vector",100.0).unwrap();
            let crate::model::Element::Picture { svg:Some(svg),base64,.. } = asset else { panic!("converted SVG picture required") };
            assert!(String::from_utf8(STANDARD.decode(svg).unwrap()).unwrap().contains("<rect"));
            let image = image::load_from_memory(&STANDARD.decode(base64).unwrap()).unwrap().to_rgba8();
            assert_eq!(image.get_pixel(512,512)[3],255);
            let mut unknown = bytes.clone();
            if mime=="image/emf" { unknown[88..92].copy_from_slice(&70u32.to_le_bytes()); } else { unknown[44..46].copy_from_slice(&0x0626u16.to_le_bytes()); }
            assert!(create_asset("unsafe",STANDARD.encode(unknown),mime,"",100.0).is_err());
            assert!(create_asset("truncated",STANDARD.encode(&bytes[..bytes.len()-1]),mime,"",100.0).is_err());
        }
    }

    #[test]
    fn svg_reference_graph_rejects_cycles_missing_targets_and_expansion() {
        let cycle = "<defs><mask id='first'><rect width='24' height='24' mask='url(#second)'/></mask><mask id='second'><rect width='24' height='24' mask='url(#first)'/></mask></defs>";
        let mut expansion = String::from("<defs>");
        for index in 0..10 { expansion.push_str(&format!("<mask id='mask{index}'><rect width='24' height='24' mask='url(#mask{})'/><rect width='24' height='24' mask='url(#mask{})'/></mask>", index + 1, index + 1)); }
        expansion.push_str("<mask id='mask10'><rect width='24' height='24'/></mask></defs>");
        for fragment in [cycle, expansion.as_str(), "<defs><mask id='test'><path d='M0 0h24v24z' fill='url(#missing)'/></mask></defs>", "<path id='target' d='M0 0h24v24z'/><rect width='24' height='24' mask='url(#target)'/>"] {
            let svg = format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'>{fragment}<rect width='24' height='24' fill='#0017c1'/></svg>");
            assert!(svg_png(&STANDARD.encode(svg), 1024).is_err(), "unsafe SVG reference graph was accepted");
        }
    }
}