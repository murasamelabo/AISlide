use crate::{Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageFormat, ImageReader, Limits};
use serde::{Serialize, Deserialize};
use sha2::{Digest, Sha256};
use std::io::Cursor;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterInfo { pub width: u32, pub height: u32, pub mime_type: String, pub sha256: String, pub byte_length: usize }

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