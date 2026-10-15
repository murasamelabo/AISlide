use crate::{Error, Result, media};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ExtendedColorType, ImageEncoder, ImageFormat, ImageReader, Limits, RgbaImage};
use image::codecs::{jpeg::JpegEncoder, png::PngEncoder};
use image::imageops::{FilterType, resize};
use std::io::{self, Cursor, Write};
use serde::{Deserialize, Serialize};

const MAX_DIMENSION: u32 = 4096;
const MAX_DECODE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PIXELS: u64 = MAX_DECODE_BYTES / 4;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackgroundKey {
    pub color: [u8; 3],
    pub tolerance: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ImageOutputFormat {
    #[default]
    Png,
    Jpeg { quality: u8, matte: Option<[u8; 3]> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImageEditParams {
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub grayscale: bool,
    pub background_key: Option<BackgroundKey>,
    pub resize_longest_side: Option<u32>,
    pub format: ImageOutputFormat,
}

impl Default for ImageEditParams {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            grayscale: false,
            background_key: None,
            resize_longest_side: None,
            format: ImageOutputFormat::Png,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditedImage {
    pub base64: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
}

/// Locally edits an in-memory PNG/JPEG after shared media validation.
///
/// Keying first sets alpha to zero when every original RGB channel is within
/// the inclusive tolerance (0..=255) of the explicit key. This is not segmentation.
/// Contrast (0..=4, identity 1) scales RGB about 127.5; brightness (-1..=1,
/// identity 0) then adds 255 times its value. Saturation (0..=4, identity 1)
/// scales distance from Rec.709 luma in encoded RGB space; grayscale overrides
/// saturation with zero. Channels are finally rounded and clamped to 0..=255.
///
/// Resize sets the longest side to 1..=4096, rounding the other side to at
/// least one pixel. Nearest-neighbor sampling preserves unassociated alpha.
/// PNG retains alpha; JPEG quality is 1..=100 and any nonopaque pixel after
/// keying requires an explicit matte, even if resizing would omit that pixel.
/// Matte compositing uses rounded encoded-RGB arithmetic after color edits.
///
/// Parameters are checked before decoding. Media input limits remain 1 MiB,
/// 4096px per side and 64 MiB per decode; RGBA buffers are checked against
/// 16,777,216 pixels. Encoded output is capped at 1 MiB (base64 at 1,398,104
/// characters). Output is 8-bit without source metadata or orientation edits.
/// No paths, files, external fetches, models, or document mutations are used.
pub fn edit_image(base64: &str, mime_type: &str, params: &ImageEditParams) -> Result<EditedImage> {
    validate_params(params)?;
    let mut pixels = decode_rgba(base64, mime_type)?;
    let (width, height) = resized_dimensions(pixels.width(), pixels.height(), params.resize_longest_side)?;
    adjust_pixels(&mut pixels, params);
    if let ImageOutputFormat::Jpeg { matte, .. } = params.format {
        if pixels.pixels().any(|pixel| pixel[3] != 255) && matte.is_none() {
            return Err(Error::Invalid("JPEG with nonopaque alpha requires an explicit matte color".into()));
        }
        if let Some(matte) = matte { apply_matte(&mut pixels, matte); }
    }
    if pixels.dimensions() != (width, height) {
        pixels = resize(&pixels, width, height, FilterType::Nearest);
    }
    encode_pixels(&pixels, params.format)
}

pub(crate) fn decode_rgba(base64: &str, mime_type: &str) -> Result<RgbaImage> {
    let info = media::inspect_raster(base64, mime_type)?;
    checked_rgba_bytes(info.width, info.height)?;
    let bytes = STANDARD.decode(base64).map_err(|_| Error::Invalid("invalid image base64".into()))?;
    let format = match mime_type {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        _ => return Err(Error::Unsupported("only PNG and JPEG raster images are accepted".into())),
    };
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|_| Error::Invalid("image cannot be decoded within the 4096px/64MiB limits".into()))?;
    if decoded.width() != info.width || decoded.height() != info.height {
        return Err(Error::Invalid("image dimensions changed after validation".into()));
    }
    Ok(decoded.into_rgba8())
}

pub(crate) fn encode_pixels(pixels: &RgbaImage, format: ImageOutputFormat) -> Result<EditedImage> {
    let (width, height) = pixels.dimensions();
    let mut output = BoundedOutput::default();
    let (encoded, mime_type) = match format {
        ImageOutputFormat::Png => (
            PngEncoder::new(&mut output).write_image(pixels.as_raw(), width, height, ExtendedColorType::Rgba8),
            "image/png",
        ),
        ImageOutputFormat::Jpeg { quality, .. } => (
            JpegEncoder::new_with_quality(&mut output, quality).encode_image(pixels),
            "image/jpeg",
        ),
    };
    if output.exceeded { return Err(Error::Limit("edited image exceeds the 1 MiB output limit".into())); }
    encoded.map_err(|_| Error::Invalid("edited image encoding failed".into()))?;
    Ok(EditedImage { base64: STANDARD.encode(output.bytes), mime_type: mime_type.into(), width, height })
}

fn validate_params(params: &ImageEditParams) -> Result<()> {
    for (name, value, minimum, maximum) in [
        ("brightness", params.brightness, -1.0, 1.0),
        ("contrast", params.contrast, 0.0, 4.0),
        ("saturation", params.saturation, 0.0, 4.0),
    ] {
        if !value.is_finite() || !(minimum..=maximum).contains(&value) {
            return Err(Error::Invalid(format!("{name} must be finite and within {minimum}..={maximum}")));
        }
    }
    if let Some(key) = params.background_key {
        if !key.tolerance.is_finite() || !(0.0..=255.0).contains(&key.tolerance) {
            return Err(Error::Invalid("background key tolerance must be finite and within 0..=255".into()));
        }
    }
    if params.resize_longest_side.is_some_and(|side| !(1..=MAX_DIMENSION).contains(&side)) {
        return Err(Error::Invalid("resize longest side must be 1..=4096 pixels".into()));
    }
    if let ImageOutputFormat::Jpeg { quality, .. } = params.format {
        if !(1..=100).contains(&quality) { return Err(Error::Invalid("JPEG quality must be 1..=100".into())); }
    }
    Ok(())
}

fn checked_rgba_bytes(width: u32, height: u32) -> Result<usize> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(Error::Limit("image dimensions must be 1..=4096 pixels".into()));
    }
    let pixels = u64::from(width).checked_mul(u64::from(height))
        .filter(|pixels| *pixels <= MAX_PIXELS)
        .ok_or_else(|| Error::Limit("image exceeds the total pixel limit".into()))?;
    let bytes = pixels.checked_mul(4).filter(|bytes| *bytes <= MAX_DECODE_BYTES)
        .ok_or_else(|| Error::Limit("image exceeds the RGBA allocation limit".into()))?;
    usize::try_from(bytes).map_err(|_| Error::Limit("image allocation size is not representable".into()))
}

fn resized_dimensions(width: u32, height: u32, longest_side: Option<u32>) -> Result<(u32, u32)> {
    checked_rgba_bytes(width, height)?;
    let Some(longest_side) = longest_side else { return Ok((width, height)); };
    if !(1..=MAX_DIMENSION).contains(&longest_side) {
        return Err(Error::Invalid("resize longest side must be 1..=4096 pixels".into()));
    }
    let longest = u64::from(width.max(height));
    let scaled = |side: u32| {
        ((u64::from(side) * u64::from(longest_side) + longest / 2) / longest).max(1) as u32
    };
    let dimensions = (scaled(width), scaled(height));
    checked_rgba_bytes(dimensions.0, dimensions.1)?;
    Ok(dimensions)
}

fn adjust_pixels(pixels: &mut RgbaImage, params: &ImageEditParams) {
    let saturation = if params.grayscale { 0.0 } else { params.saturation };
    for pixel in pixels.pixels_mut() {
        if let Some(key) = params.background_key {
            if pixel.0[..3].iter().zip(key.color).all(|(channel, key_channel)| f64::from(channel.abs_diff(key_channel)) <= key.tolerance) {
                pixel[3] = 0;
            }
        }
        let adjusted = [pixel[0], pixel[1], pixel[2]].map(|channel| {
            (f64::from(channel) - 127.5) * params.contrast + 127.5 + params.brightness * 255.0
        });
        let luma = adjusted[0] * 0.2126 + adjusted[1] * 0.7152 + adjusted[2] * 0.0722;
        for (channel, adjusted) in pixel.0[..3].iter_mut().zip(adjusted) {
            *channel = (luma + (adjusted - luma) * saturation).round().clamp(0.0, 255.0) as u8;
        }
    }
}

fn apply_matte(pixels: &mut RgbaImage, matte: [u8; 3]) {
    for pixel in pixels.pixels_mut() {
        let alpha = u32::from(pixel[3]);
        for (channel, background) in pixel.0[..3].iter_mut().zip(matte) {
            *channel = ((u32::from(*channel) * alpha + u32::from(background) * (255 - alpha) + 127) / 255) as u8;
        }
        pixel[3] = 255;
    }
}

#[derive(Default)]
struct BoundedOutput {
    bytes: Vec<u8>,
    exceeded: bool,
}

impl Write for BoundedOutput {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.exceeded || buffer.len() > MAX_OUTPUT_BYTES.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(io::Error::other("edited image exceeds the 1 MiB output limit"));
        }
        let required = self.bytes.len() + buffer.len();
        if required > self.bytes.capacity() {
            let capacity = required.max(self.bytes.capacity().saturating_mul(2)).min(MAX_OUTPUT_BYTES);
            self.bytes.try_reserve_exact(capacity - self.bytes.len()).map_err(io::Error::other)?;
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_edit_dimensions_are_checked_without_allocating_pixels() {
        assert_eq!(checked_rgba_bytes(4096, 4096).unwrap(), 64 * 1024 * 1024);
        for (width, height) in [(0, 1), (1, 0), (4097, 1), (1, 4097), (u32::MAX, u32::MAX)] {
            assert!(matches!(checked_rgba_bytes(width, height), Err(Error::Limit(_))));
            assert!(resized_dimensions(width, height, Some(1)).is_err());
        }
        assert_eq!(resized_dimensions(3, 2, Some(4096)).unwrap(), (4096, 2731));
        assert!(resized_dimensions(1, 1, Some(u32::MAX)).is_err());
    }

    #[test]
    fn image_edit_output_writer_rejects_growth_before_allocation() {
        let mut output = BoundedOutput::default();
        let oversized = vec![0; MAX_OUTPUT_BYTES + 1];
        assert!(output.write_all(&oversized).is_err());
        assert!(output.bytes.is_empty());
        assert_eq!(output.bytes.capacity(), 0);
        let mut output = BoundedOutput::default();
        output.write_all(&oversized[..MAX_OUTPUT_BYTES]).unwrap();
        let capacity = output.bytes.capacity();
        assert!(output.write_all(&[1]).is_err());
        assert!(output.exceeded);
        assert_eq!(output.bytes.len(), MAX_OUTPUT_BYTES);
        assert_eq!(output.bytes.capacity(), capacity);
    }
}