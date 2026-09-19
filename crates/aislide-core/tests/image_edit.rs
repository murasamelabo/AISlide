use aislide_core::{Error, image_edit::{BackgroundKey, ImageEditParams, ImageOutputFormat, edit_image}};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

fn png(image: &RgbaImage) -> String {
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, ImageFormat::Png).unwrap();
    STANDARD.encode(output.into_inner())
}

fn edited(image: &RgbaImage, params: ImageEditParams) -> RgbaImage {
    let output = edit_image(&png(image), "image/png", &params).unwrap();
    assert_eq!(output.mime_type, "image/png");
    let bytes = STANDARD.decode(&output.base64).unwrap();
    assert!(bytes.len() <= 1024 * 1024);
    assert_eq!(image::guess_format(&bytes).unwrap(), ImageFormat::Png);
    let result = image::load_from_memory(&bytes).unwrap().into_rgba8();
    assert_eq!(result.dimensions(), (output.width, output.height));
    result
}

#[test]
fn defaults_preserve_rgba_pixels_and_source() {
    let source = RgbaImage::from_fn(4, 2, |column, row| {
        Rgba([column as u8 * 60, row as u8 * 100, 37, column as u8 * 85])
    });
    let original = source.clone();
    assert_eq!(edited(&source, ImageEditParams::default()), source);
    assert_eq!(source, original);
}

#[test]
fn brightness_contrast_and_saturation_change_pixels_and_clamp_channels() {
    let source = RgbaImage::from_pixel(1, 1, Rgba([20, 100, 240, 71]));
    let bright = edited(&source, ImageEditParams { brightness: 0.2, ..Default::default() });
    assert_eq!(bright.get_pixel(0, 0).0, [71, 151, 255, 71]);
    let dark = edited(&source, ImageEditParams { brightness: -1.0, ..Default::default() });
    assert_eq!(dark.get_pixel(0, 0).0, [0, 0, 0, 71]);
    let light = edited(&source, ImageEditParams { brightness: 1.0, ..Default::default() });
    assert_eq!(light.get_pixel(0, 0).0, [255, 255, 255, 71]);
    let contrast = edited(&source, ImageEditParams { contrast: 4.0, ..Default::default() });
    assert_eq!(contrast.get_pixel(0, 0).0, [0, 18, 255, 71]);
    let flat = edited(&source, ImageEditParams { contrast: 0.0, ..Default::default() });
    assert_eq!(flat.get_pixel(0, 0).0, [128, 128, 128, 71]);
    let saturated = edited(&source, ImageEditParams { saturation: 4.0, ..Default::default() });
    assert_eq!(saturated.get_pixel(0, 0)[0], 0);
    assert_eq!(saturated.get_pixel(0, 0)[2], 255);
    assert_eq!(saturated.get_pixel(0, 0)[3], 71);
}

#[test]
fn grayscale_and_zero_saturation_use_the_same_luma_and_keep_alpha() {
    let source = RgbaImage::from_pixel(2, 2, Rgba([100, 150, 200, 123]));
    let gray = edited(&source, ImageEditParams { grayscale: true, ..Default::default() });
    assert_eq!(gray.get_pixel(0, 0).0, [143, 143, 143, 123]);
    assert_eq!(gray, edited(&source, ImageEditParams { saturation: 0.0, ..Default::default() }));
}

#[test]
fn explicit_color_key_matches_original_rgb_with_inclusive_channel_tolerance() {
    let source = RgbaImage::from_raw(4, 1, vec![
        10, 200, 20, 255, 15, 195, 25, 120, 16, 200, 20, 80, 10, 200, 20, 0,
    ]).unwrap();
    let result = edited(&source, ImageEditParams {
        background_key: Some(BackgroundKey { color: [10, 200, 20], tolerance: 5.0 }),
        brightness: 0.2,
        ..Default::default()
    });
    assert_eq!(result.pixels().map(|pixel| pixel[3]).collect::<Vec<_>>(), [0, 0, 80, 0]);
    assert_eq!(result.get_pixel(0, 0).0, [61, 251, 71, 0]);
    let exact = edited(&source, ImageEditParams {
        background_key: Some(BackgroundKey { color: [10, 200, 20], tolerance: 0.0 }),
        ..Default::default()
    });
    assert_eq!(exact.get_pixel(1, 0)[3], 120);
    let all = edited(&source, ImageEditParams {
        background_key: Some(BackgroundKey { color: [0, 0, 0], tolerance: 255.0 }),
        ..Default::default()
    });
    assert!(all.pixels().all(|pixel| pixel[3] == 0));
}

#[test]
fn resize_sets_longest_side_keeps_aspect_ratio_and_alpha() {
    let source = RgbaImage::from_fn(4, 2, |column, _row| Rgba([20, 40, 60, column as u8 * 85]));
    let enlarged = edited(&source, ImageEditParams { resize_longest_side: Some(8), ..Default::default() });
    assert_eq!(enlarged.dimensions(), (8, 4));
    assert_eq!(enlarged.get_pixel(0, 0)[3], 0);
    assert_eq!(enlarged.get_pixel(7, 3)[3], 255);
    let reduced = edited(&source, ImageEditParams { resize_longest_side: Some(2), ..Default::default() });
    assert_eq!(reduced.dimensions(), (2, 1));
    let narrow = RgbaImage::from_pixel(1, 8, Rgba([1, 2, 3, 42]));
    let minimum = edited(&narrow, ImageEditParams { resize_longest_side: Some(1), ..Default::default() });
    assert_eq!(minimum.dimensions(), (1, 1));
    assert_eq!(minimum.get_pixel(0, 0)[3], 42);
}

#[test]
fn jpeg_requires_matte_for_partial_or_fully_transparent_pixels_and_color_key() {
    let params = ImageEditParams {
        format: ImageOutputFormat::Jpeg { quality: 100, matte: None },
        ..Default::default()
    };
    for alpha in [0, 128, 254] {
        let source = png(&RgbaImage::from_pixel(2, 2, Rgba([200, 0, 0, alpha])));
        assert!(matches!(edit_image(&source, "image/png", &params), Err(Error::Invalid(message)) if message.contains("matte")));
    }
    let opaque = png(&RgbaImage::from_pixel(2, 2, Rgba([10, 200, 20, 255])));
    let keyed = ImageEditParams {
        background_key: Some(BackgroundKey { color: [10, 200, 20], tolerance: 0.0 }),
        ..params.clone()
    };
    assert!(matches!(edit_image(&opaque, "image/png", &keyed), Err(Error::Invalid(message)) if message.contains("matte")));
    assert!(edit_image(&opaque, "image/png", &params).is_ok());
    let mut sparse = RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]));
    sparse.put_pixel(0, 0, Rgba([10, 20, 30, 0]));
    let resized = ImageEditParams { resize_longest_side: Some(1), ..params };
    assert!(matches!(edit_image(&png(&sparse), "image/png", &resized), Err(Error::Invalid(message)) if message.contains("matte")));
}

#[test]
fn jpeg_composites_explicit_matte_and_can_be_read_back_as_input() {
    for (alpha, expected) in [(0, [0, 100, 200]), (128, [100, 50, 100]), (255, [200, 0, 0])] {
        let source = png(&RgbaImage::from_pixel(8, 8, Rgba([200, 0, 0, alpha])));
        let output = edit_image(&source, "image/png", &ImageEditParams {
            format: ImageOutputFormat::Jpeg { quality: 100, matte: Some([0, 100, 200]) },
            ..Default::default()
        }).unwrap();
        assert_eq!(output.mime_type, "image/jpeg");
        assert_eq!((output.width, output.height), (8, 8));
        let bytes = STANDARD.decode(&output.base64).unwrap();
        assert_eq!(image::guess_format(&bytes).unwrap(), ImageFormat::Jpeg);
        assert!(matches!(edit_image(&output.base64, "image/png", &ImageEditParams::default()), Err(Error::Invalid(message)) if message.contains("media type")));
        let decoded = image::load_from_memory(&bytes).unwrap().into_rgb8();
        for (actual, expected) in decoded.get_pixel(4, 4).0.into_iter().zip(expected) {
            assert!(actual.abs_diff(expected) <= 3, "actual={actual}, expected={expected}");
        }
        let converted = edit_image(&output.base64, "image/jpeg", &ImageEditParams::default()).unwrap();
        assert_eq!(converted.mime_type, "image/png");
        let png_bytes = STANDARD.decode(converted.base64).unwrap();
        assert!(image::load_from_memory(&png_bytes).unwrap().into_rgba8().pixels().all(|pixel| pixel[3] == 255));
    }
}

#[test]
fn invalid_parameters_fail_before_base64_decoding_or_pixel_allocation() {
    let mut invalid = Vec::new();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.01, 1.01] {
        invalid.push(("brightness", ImageEditParams { brightness: value, ..Default::default() }));
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01, 4.01] {
        invalid.push(("contrast", ImageEditParams { contrast: value, ..Default::default() }));
        invalid.push(("saturation", ImageEditParams { saturation: value, ..Default::default() }));
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01, 255.01] {
        invalid.push(("tolerance", ImageEditParams {
            background_key: Some(BackgroundKey { color: [0, 0, 0], tolerance: value }),
            ..Default::default()
        }));
    }
    for value in [0, 4097, u32::MAX] {
        invalid.push(("resize", ImageEditParams { resize_longest_side: Some(value), ..Default::default() }));
    }
    for value in [0, 101, u8::MAX] {
        invalid.push(("quality", ImageEditParams {
            format: ImageOutputFormat::Jpeg { quality: value, matte: None },
            ..Default::default()
        }));
    }
    for (field, params) in invalid {
        assert!(matches!(edit_image("not base64!", "image/png", &params), Err(Error::Invalid(message)) if message.contains(field)), "{field}: {params:?}");
    }
}

#[test]
fn input_validation_rejects_header_mismatch_truncation_paths_and_excessive_sizes() {
    let params = ImageEditParams::default();
    let source = png(&RgbaImage::from_pixel(2, 2, Rgba([20, 40, 60, 255])));
    edit_image(&source, "image/png", &params).unwrap();
    assert!(matches!(edit_image(&source, "image/jpeg", &params), Err(Error::Invalid(message)) if message.contains("media type")));
    assert!(matches!(edit_image(&source, "image/svg+xml", &params), Err(Error::Unsupported(_))));
    let truncated = STANDARD.decode(source).unwrap();
    assert!(edit_image(&STANDARD.encode(&truncated[..20]), "image/png", &params).is_err());
    assert!(edit_image(&STANDARD.encode(&truncated[..truncated.len() / 2]), "image/png", &params).is_err());
    for invalid in ["", "not base64!", "file:///private.png", "https://example.com/image.png", "C:\\private.png"] {
        assert!(edit_image(invalid, "image/png", &params).is_err());
    }
    assert!(matches!(edit_image(&"A".repeat(1_398_105), "image/png", &params), Err(Error::Limit(_))));
    for (width, height) in [(4097, 1), (1, 4097)] {
        let source = png(&RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255])));
        assert!(edit_image(&source, "image/png", &params).is_err());
    }
}

#[test]
fn jpeg_quality_controls_encoding_and_accepts_both_bounds() {
    let source = png(&RgbaImage::from_fn(16, 16, |column, row| {
        Rgba([column as u8 * 16, row as u8 * 16, (column + row) as u8 * 8, 255])
    }));
    let encode = |quality| edit_image(&source, "image/png", &ImageEditParams {
        format: ImageOutputFormat::Jpeg { quality, matte: None },
        ..Default::default()
    }).unwrap();
    let lowest = encode(1);
    let highest = encode(100);
    assert_ne!(lowest.base64, highest.base64);
    assert!(lowest.base64.len() < highest.base64.len());
    aislide_core::media::inspect_raster(&lowest.base64, "image/jpeg").unwrap();
    aislide_core::media::inspect_raster(&highest.base64, "image/jpeg").unwrap();
}

#[test]
fn encoding_more_than_one_mib_returns_a_limit_error_without_a_result() {
    let mut seed = 0x1234_5678_u32;
    let source = RgbaImage::from_fn(768, 768, |_column, _row| {
        let mut color = [0_u8; 4];
        for channel in &mut color[..3] {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *channel = seed as u8;
        }
        color[3] = 255;
        Rgba(color)
    });
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 50).encode_image(&source).unwrap();
    assert!(bytes.len() < 1024 * 1024, "synthetic JPEG must fit the input limit");
    let decoded = image::load_from_memory(&bytes).unwrap().into_rgba8();
    let expected_png = STANDARD.decode(png(&decoded)).unwrap();
    assert!(expected_png.len() > 1024 * 1024, "synthetic PNG must exercise the output limit");
    assert!(matches!(
        edit_image(&STANDARD.encode(bytes), "image/jpeg", &ImageEditParams::default()),
        Err(Error::Limit(message)) if message.contains("output limit")
    ));
}

#[test]
fn owned_image_edit_sources_have_exactly_one_utf8_bom() {
    for bytes in [include_bytes!("../src/image_edit.rs").as_slice(), include_bytes!("image_edit.rs").as_slice()] {
        assert!(bytes.starts_with(&[0xef, 0xbb, 0xbf]));
        let text = std::str::from_utf8(bytes).unwrap();
        assert_eq!(text.chars().filter(|character| *character == '\u{feff}').count(), 1);
    }
}