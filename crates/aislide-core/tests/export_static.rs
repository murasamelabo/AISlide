use aislide_core::{
    export_static::{export_static, preview_presentation, ExportFormat, ExportOptions, PreviewFormat, PreviewLayout, PreviewOptions, PreviewOverflow},
    model::Deck,
    render::render_slide_svg,
    Error,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::json;

fn deck() -> Deck {
    serde_json::from_value(json!({
        "version": 1, "title": "Static export", "width": 320, "height": 320,
        "slides": [{"id": "slide-1", "title": "Pixels", "background": "FFFFFF", "notes": "",
            "elements": [{"type": "rect", "id": "red", "x": 20, "y": 30,
                "width": 80, "height": 60, "fill": "FF0000"}]}]
    }))
    .unwrap()
}

#[test]
fn png_has_requested_resolution_real_pixels_and_does_not_mutate_source() {
    let source = deck();
    let original = serde_json::to_vec(&source).unwrap();
    let result = export_static(
        &source,
        &ExportOptions {
            scale: 2.0,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!result.office_parity_verified);
    assert_eq!(result.artifacts.len(), 1);
    let artifact = &result.artifacts[0];
    assert_eq!(artifact.mime_type, "image/png");
    assert_eq!(artifact.page_indices, [0]);
    assert_eq!((artifact.width, artifact.height), (640, 640));
    let pixels = image::load_from_memory(&artifact.bytes)
        .unwrap()
        .into_rgba8();
    assert_eq!(pixels.dimensions(), (640, 640));
    assert_eq!(pixels.get_pixel(80, 100).0, [255, 0, 0, 255]);
    assert_eq!(pixels.get_pixel(0, 0).0, [255, 255, 255, 255]);
    assert_eq!(serde_json::to_vec(&source).unwrap(), original);
}

fn with_elements(elements: serde_json::Value) -> Deck {
    let mut value = serde_json::to_value(deck()).unwrap();
    value["slides"][0]["elements"] = elements;
    serde_json::from_value(value).unwrap()
}

fn noisy_preview_document() -> aislide_core::document::Document {
    let mut state = 0x1234_5678u32;
    let pixels = image::RgbaImage::from_fn(256, 256, |_column, _row| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        image::Rgba([state as u8, (state >> 8) as u8, (state >> 16) as u8, 255])
    });
    let mut encoded = std::io::Cursor::new(Vec::new());
    pixels.write_to(&mut encoded, image::ImageFormat::Png).unwrap();
    let mut source = with_elements(json!([{
        "type":"picture", "id":"noise", "x":0, "y":0, "width":320, "height":320,
        "base64":STANDARD.encode(encoded.into_inner()), "mime_type":"image/png", "alt":"Synthetic noise"
    }, {
        "type":"shape", "id":"approximation", "preset":"hexagon", "x":10, "y":10,
        "width":40, "height":40, "fill":"FFFFFF", "stroke":"FFFFFF", "stroke_width":0,
        "text":"", "font_size":12, "color":"000000", "bold":false
    }]));
    source.slides = (0..3).map(|index| {
        let mut slide = source.slides[0].clone();
        slide.id = format!("noise-{index}");
        slide
    }).collect();
    aislide_core::document::create("preview-noise".into(), source, vec![], vec![], None).unwrap()
}

#[test]
fn preview_default_shrinks_only_output_budget_and_preserves_all_pages() {
    let document = noisy_preview_document();
    let before = serde_json::to_vec(&document).unwrap();
    let selected = vec![2, 0, 1];
    let encoded_size = |scale| export_static(&document.deck, &ExportOptions {
        scale, page_indices: Some(selected.clone()), ..Default::default()
    }).unwrap().artifacts.iter().map(|artifact| artifact.bytes.len()).sum::<usize>();
    let budget = encoded_size(0.75);
    assert!(encoded_size(1.0) > budget);
    let preview = preview_presentation(&document, &PreviewOptions {
        page_indices: Some(selected.clone()), max_dimension: 320, max_output_bytes: budget,
        ..Default::default()
    }).expect("default preview must shrink on encoded output overflow");
    let metadata = serde_json::to_value(&preview).unwrap();
    assert_eq!(metadata["requested_max_dimension"], 320);
    assert_eq!(metadata["actual_max_dimension"], 240);
    assert_eq!(metadata["quality_reduced"], true);
    assert_eq!(preview.pages.iter().map(|page| page.page_index).collect::<Vec<_>>(), selected);
    assert_eq!(preview.images.len(), 3);
    assert!(preview.images.iter().map(|image| image.byte_length).sum::<usize>() <= budget);
    for (index, page) in preview.pages.iter().enumerate() {
        assert_eq!(page.slide_id, document.deck.slides[page.page_index].id);
        assert_eq!((page.image_index, page.x, page.y, page.width, page.height), (index, 0, 0, 240, 240));
        let pixels = image::load_from_memory(&STANDARD.decode(&preview.images[index].base64).unwrap()).unwrap();
        assert_eq!((pixels.width(), pixels.height()), (240, 240));
    }
    let warning = preview.warnings.iter().find(|warning| warning.code == "PREVIEW_DOWNSCALED").unwrap();
    assert!(warning.message.contains("320") && warning.message.contains("240") && warning.message.contains("all"));
    assert_eq!(preview.warnings.iter().filter(|warning| warning.code == "SHAPE_APPROXIMATION").count(), 3);
    assert_eq!(preview.hash, document.hash);
    assert_eq!(serde_json::to_vec(&document).unwrap(), before);
}

fn noisy_preview_size(document: &aislide_core::document::Document, dimension: u32) -> usize {
    export_static(&document.deck, &ExportOptions {
        scale: f64::from(dimension) / 320.0, ..Default::default()
    }).unwrap().artifacts.iter().map(|artifact| artifact.bytes.len()).sum()
}

fn assert_preview_image(image: &aislide_core::export_static::PreviewImage, format: image::ImageFormat) {
    use sha2::{Digest, Sha256};
    let bytes = STANDARD.decode(&image.base64).unwrap();
    assert_eq!(image.byte_length, bytes.len());
    assert_eq!(image.sha256, format!("{:x}", Sha256::digest(&bytes)));
    assert_eq!(image::guess_format(&bytes).unwrap(), format);
    let pixels = image::load_from_memory_with_format(&bytes, format).unwrap();
    assert_eq!((image.width, image.height), (pixels.width(), pixels.height()));
}

#[test]
fn preview_png_success_keeps_existing_bytes_and_effective_dimension() {
    let mut source = deck();
    source.width = 640;
    let document = aislide_core::document::create("preview-identity".into(), source, vec![], vec![], None).unwrap();
    let before = serde_json::to_vec(&document).unwrap();
    let defaults: PreviewOptions = serde_json::from_value(json!({})).unwrap();
    assert!(matches!(defaults.format, PreviewFormat::Png));
    assert!(matches!(defaults.overflow, PreviewOverflow::Shrink));
    for layout in [PreviewLayout::Pages, PreviewLayout::ContactSheet] {
        let montage = matches!(layout, PreviewLayout::ContactSheet);
        let cell = if montage { 1256 } else { 1280 };
        let reference = export_static(&document.deck, &ExportOptions {
            scale: f64::from(cell) / 640.0, ..Default::default()
        }).unwrap();
        let artifact = &reference.artifacts[0];
        let expected = if montage {
            let pixels = image::load_from_memory(&artifact.bytes).unwrap().into_rgba8();
            let mut sheet = image::RgbaImage::from_pixel(artifact.width + 24, artifact.height + 24, image::Rgba([238, 238, 238, 255]));
            image::imageops::replace(&mut sheet, &pixels, 12, 12);
            let mut encoded = std::io::Cursor::new(Vec::new());
            sheet.write_to(&mut encoded, image::ImageFormat::Png).unwrap();
            encoded.into_inner()
        } else { artifact.bytes.clone() };
        let preview = preview_presentation(&document, &PreviewOptions { layout, ..defaults.clone() }).unwrap();
        assert_eq!(STANDARD.decode(&preview.images[0].base64).unwrap(), expected);
        assert_preview_image(&preview.images[0], image::ImageFormat::Png);
        assert_eq!(preview.images[0].mime_type, "image/png");
        assert_eq!((preview.requested_max_dimension, preview.actual_max_dimension, preview.quality_reduced), (1280, 1280, false));
        assert!(!preview.warnings.iter().any(|warning| warning.code == "PREVIEW_DOWNSCALED"));
        assert_eq!((preview.pages[0].width, preview.pages[0].height), (artifact.width, artifact.height));
        assert_eq!((preview.pages[0].x, preview.pages[0].y), if montage { (12, 12) } else { (0, 0) });
        assert_eq!((preview.revision, &preview.hash), (document.revision, &document.hash));
        assert!(serde_json::to_vec(&preview).unwrap().len() <= 4 * 1024 * 1024 - 65536);
    }
    assert_eq!(serde_json::to_vec(&document).unwrap(), before);
}

#[test]
fn preview_strict_overflow_stops_at_requested_size_with_actionable_error() {
    let document = noisy_preview_document();
    let before = serde_json::to_vec(&document).unwrap();
    let budget = noisy_preview_size(&document, 240);
    let options: PreviewOptions = serde_json::from_value(json!({
        "max_dimension":320, "max_output_bytes":budget, "overflow":"error"
    })).unwrap();
    let error = preview_presentation(&document, &options).unwrap_err();
    assert!(matches!(error, Error::Limit(_)));
    let message = error.to_string();
    for expected in ["encoded output byte limit", "1 attempt", "[320]", "fewer pages", "max_dimension", "jpeg", "no partial"] {
        assert!(message.contains(expected), "{message}");
    }
    assert_eq!(serde_json::to_vec(&document).unwrap(), before);
}

#[test]
fn preview_third_attempt_uses_floor_of_original_dimension() {
    let document = noisy_preview_document();
    let budget = noisy_preview_size(&document, 181);
    assert!(noisy_preview_size(&document, 241) > budget);
    let preview = preview_presentation(&document, &PreviewOptions {
        max_dimension: 322, max_output_bytes: budget, ..Default::default()
    }).unwrap();
    assert_eq!((preview.requested_max_dimension, preview.actual_max_dimension, preview.quality_reduced), (322, 181, true));
    assert_eq!(preview.pages.len(), 3);
    assert!(preview.images.iter().all(|image| image.width == 181 && image.height == 181));
}

#[test]
fn preview_exhausts_three_attempts_even_when_a_fourth_size_would_fit() {
    let document = noisy_preview_document();
    let before = serde_json::to_vec(&document).unwrap();
    let budget = noisy_preview_size(&document, 160);
    assert!(noisy_preview_size(&document, 181) > budget);
    let message = preview_presentation(&document, &PreviewOptions {
        max_dimension: 322, max_output_bytes: budget, ..Default::default()
    }).unwrap_err().to_string();
    for expected in ["3 attempt", "[322, 241, 181]", "fewer pages", "max_dimension", "jpeg", "do not guarantee", "no partial"] {
        assert!(message.contains(expected), "{message}");
    }
    assert_eq!(serde_json::to_vec(&document).unwrap(), before);
}

#[test]
fn preview_minimum_dimension_is_never_retried_twice() {
    let document = aislide_core::document::create("preview-minimum".into(), deck(), vec![], vec![], None).unwrap();
    for (dimension, attempts, sizes) in [(160, "1 attempt", "[160]"), (161, "2 attempt", "[161, 160]")] {
        let message = preview_presentation(&document, &PreviewOptions {
            max_dimension: dimension, max_output_bytes: 1, ..Default::default()
        }).unwrap_err().to_string();
        assert!(message.contains(attempts) && message.contains(sizes), "{message}");
    }
}

#[test]
fn preview_invalid_options_and_document_integrity_never_fall_back() {
    let document = aislide_core::document::create("preview-invalid".into(), deck(), vec![], vec![], None).unwrap();
    for options in [
        json!({"page_indices":[]}), json!({"page_indices":[1]}), json!({"page_indices":[0,0]}),
        json!({"page_indices":[0,0,0,0,0,0,0,0,0]}), json!({"max_dimension":159}),
        json!({"max_dimension":1601}), json!({"max_output_bytes":0}), json!({"max_output_bytes":2097153}),
    ] {
        let options: PreviewOptions = serde_json::from_value(options).unwrap();
        let error = preview_presentation(&document, &options).unwrap_err();
        assert!(matches!(error, Error::Invalid(_) | Error::Limit(_)));
        assert!(!error.to_string().contains("attempt"));
    }
    for options in [json!({"format":"pdf"}), json!({"format":"webp"}), json!({"overflow":"ignore"}), json!({"path":"outside"})] {
        assert!(serde_json::from_value::<PreviewOptions>(options).is_err());
    }
    let mut corrupted = document.clone();
    corrupted.hash = "0".repeat(64);
    assert!(matches!(preview_presentation(&corrupted, &PreviewOptions {
        max_output_bytes: 1, ..Default::default()
    }), Err(Error::Conflict(_))));
}

#[test]
fn preview_contact_sheet_shrink_preserves_order_padding_and_all_coordinates() {
    let document = noisy_preview_document();
    let before = serde_json::to_vec(&document).unwrap();
    let selected = vec![2, 0, 1];
    let options = PreviewOptions {
        page_indices: Some(selected.clone()), max_dimension: 480, layout: PreviewLayout::ContactSheet,
        overflow: PreviewOverflow::Error, ..Default::default()
    };
    let reference = preview_presentation(&document, &options).unwrap();
    let budget = reference.images[0].byte_length;
    assert!(preview_presentation(&document, &PreviewOptions {
        max_dimension: 640, max_output_bytes: budget, ..options.clone()
    }).is_err());
    let preview = preview_presentation(&document, &PreviewOptions {
        max_dimension: 640, max_output_bytes: budget, overflow: PreviewOverflow::Shrink, ..options
    }).unwrap();
    assert_eq!((preview.requested_max_dimension, preview.actual_max_dimension, preview.quality_reduced), (640, 480, true));
    assert_eq!(preview.pages.iter().map(|page| page.page_index).collect::<Vec<_>>(), selected);
    assert_eq!(serde_json::to_value(&preview.pages).unwrap(), serde_json::to_value(&reference.pages).unwrap());
    assert_eq!(preview.images[0].base64, reference.images[0].base64);
    assert_eq!(preview.images.len(), 1);
    assert!(preview.images[0].byte_length <= budget);
    assert_preview_image(&preview.images[0], image::ImageFormat::Png);
    let pixels = image::load_from_memory(&STANDARD.decode(&preview.images[0].base64).unwrap()).unwrap().into_rgba8();
    for (index, page) in preview.pages.iter().enumerate() {
        assert_eq!(page.image_index, 0);
        assert_eq!(page.x, 12 + index as u32 % 2 * (page.width + 12));
        assert_eq!(page.y, 12 + index as u32 / 2 * (page.height + 12));
        assert!(page.x + page.width + 12 <= pixels.width());
        assert!(page.y + page.height + 12 <= pixels.height());
        assert_eq!(pixels.get_pixel(page.x - 1, page.y - 1).0, [238, 238, 238, 255]);
    }
    assert_eq!(serde_json::to_vec(&document).unwrap(), before);
}

#[test]
fn preview_jpeg_pages_and_contact_sheet_are_decodable_and_keep_metadata() {
    let document = noisy_preview_document();
    let before = serde_json::to_vec(&document).unwrap();
    let selected = vec![2, 0, 1];
    for (layout, count) in [("pages", 3), ("contact_sheet", 1)] {
        let options: PreviewOptions = serde_json::from_value(json!({
            "page_indices":selected, "max_dimension":320, "layout":layout, "format":"jpeg", "overflow":"error"
        })).unwrap();
        let preview = preview_presentation(&document, &options).unwrap();
        assert_eq!(preview.images.len(), count);
        assert_eq!(preview.pages.iter().map(|page| page.page_index).collect::<Vec<_>>(), selected);
        assert_eq!((preview.requested_max_dimension, preview.actual_max_dimension, preview.quality_reduced), (320, 320, false));
        assert!(preview.images.iter().map(|image| image.byte_length).sum::<usize>() <= options.max_output_bytes);
        for image in &preview.images {
            assert_eq!(image.mime_type, "image/jpeg");
            assert_preview_image(image, image::ImageFormat::Jpeg);
        }
        for page in &preview.pages {
            let image = &preview.images[page.image_index];
            assert!(page.x + page.width <= image.width && page.y + page.height <= image.height);
        }
        if layout == "pages" {
            let reference = export_static(&document.deck, &ExportOptions {
                format: ExportFormat::Jpeg, page_indices: Some(selected.clone()), ..Default::default()
            }).unwrap();
            for (image, artifact) in preview.images.iter().zip(reference.artifacts) {
                assert_eq!(STANDARD.decode(&image.base64).unwrap(), artifact.bytes);
            }
            let budget = preview.images.iter().map(|image| image.byte_length).sum();
            let bounded = PreviewOptions { max_output_bytes: budget, ..options.clone() };
            assert!(preview_presentation(&document, &PreviewOptions { format: PreviewFormat::Png, ..bounded.clone() }).is_err());
            let jpeg = preview_presentation(&document, &bounded).unwrap();
            assert!(!jpeg.quality_reduced);
            assert_eq!(jpeg.actual_max_dimension, 320);
        }
        assert_eq!(preview.hash, document.hash);
        assert!(serde_json::to_vec(&preview).unwrap().len() <= 4 * 1024 * 1024 - 65536);
    }
    assert_eq!(serde_json::to_vec(&document).unwrap(), before);
}

#[test]
fn preview_jpeg_composites_alpha_onto_slide_background() {
    let pixels = image::RgbaImage::from_pixel(16, 16, image::Rgba([0, 0, 255, 128]));
    let mut encoded = std::io::Cursor::new(Vec::new());
    pixels.write_to(&mut encoded, image::ImageFormat::Png).unwrap();
    let mut source = with_elements(json!([{
        "type":"picture", "id":"alpha", "x":80, "y":80, "width":160, "height":160,
        "base64":STANDARD.encode(encoded.into_inner()), "mime_type":"image/png", "alt":"Synthetic alpha"
    }]));
    source.slides[0].background = "00FF00".into();
    let document = aislide_core::document::create("preview-alpha".into(), source, vec![], vec![], None).unwrap();
    for layout in [PreviewLayout::Pages, PreviewLayout::ContactSheet] {
        let preview = preview_presentation(&document, &PreviewOptions {
            max_dimension: 320, format: PreviewFormat::Jpeg, layout, ..Default::default()
        }).unwrap();
        let page = &preview.pages[0];
        let pixels = image::load_from_memory(&STANDARD.decode(&preview.images[0].base64).unwrap()).unwrap().into_rgb8();
        assert!(pixels.get_pixel(page.x + 20, page.y + 20)[1] > 250);
        let mixed = pixels.get_pixel(page.x + page.width / 2, page.y + page.height / 2);
        assert!(mixed[0] < 5 && (i16::from(mixed[1]) - 127).abs() < 5 && (i16::from(mixed[2]) - 128).abs() < 5);
    }
}

#[test]
fn preview_default_two_mib_budget_returns_all_eight_pages() {
    let mut source = noisy_preview_document().deck;
    source.slides = (0..8).map(|index| {
        let mut slide = source.slides[0].clone();
        slide.id = format!("page-{index}");
        slide
    }).collect();
    let document = aislide_core::document::create("preview-eight".into(), source, vec![], vec![], None).unwrap();
    let before = serde_json::to_vec(&document).unwrap();
    assert!(noisy_preview_size(&document, 320) > 2 * 1024 * 1024);
    let preview = preview_presentation(&document, &PreviewOptions { max_dimension: 320, ..Default::default() }).unwrap();
    assert_eq!(preview.pages.iter().map(|page| page.page_index).collect::<Vec<_>>(), (0..8).collect::<Vec<_>>());
    assert_eq!(preview.images.len(), 8);
    assert_eq!(preview.actual_max_dimension, 240);
    assert!(preview.images.iter().map(|image| image.byte_length).sum::<usize>() <= 2 * 1024 * 1024);
    assert!(serde_json::to_vec(&preview).unwrap().len() <= 4 * 1024 * 1024 - 65536);
    assert_eq!(serde_json::to_vec(&document).unwrap(), before);
}

#[test]
fn preview_minimum_jpeg_contact_sheet_resizes_without_alignment_loss() {
    let colors = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0], [0, 255, 255], [255, 0, 255], [255, 255, 255], [0, 0, 0]];
    let mut source = deck();
    source.width = 4096;
    source.height = 4096;
    source.slides = colors.iter().enumerate().map(|(index, color)| {
        let mut slide = source.slides[0].clone();
        slide.id = format!("color-{index}");
        slide.background = format!("{:02X}{:02X}{:02X}", color[0], color[1], color[2]);
        slide.elements.clear();
        slide
    }).collect();
    let document = aislide_core::document::create("preview-jpeg-minimum".into(), source, vec![], vec![], None).unwrap();
    let selected = vec![7, 1, 5, 3, 0, 6, 4, 2];
    let preview = preview_presentation(&document, &PreviewOptions {
        page_indices: Some(selected.clone()), max_dimension: 160, format: PreviewFormat::Jpeg,
        layout: PreviewLayout::ContactSheet, ..Default::default()
    }).unwrap();
    assert_eq!((preview.actual_max_dimension, preview.quality_reduced), (160, false));
    assert_eq!(preview.images.len(), 1);
    assert_eq!((preview.images[0].width, preview.images[0].height), (159, 159));
    assert_preview_image(&preview.images[0], image::ImageFormat::Jpeg);
    let pixels = image::load_from_memory(&STANDARD.decode(&preview.images[0].base64).unwrap()).unwrap().into_rgb8();
    for (index, page) in preview.pages.iter().enumerate() {
        assert_eq!(page.page_index, selected[index]);
        assert_eq!(page.slide_id, document.deck.slides[selected[index]].id);
        assert_eq!((page.image_index, page.width, page.height), (0, 37, 37));
        assert_eq!((page.x, page.y), (12 + index as u32 % 3 * 49, 12 + index as u32 / 3 * 49));
        let color = pixels.get_pixel(page.x + 18, page.y + 18);
        assert!(color.0.iter().zip(colors[page.page_index]).all(|(actual, expected)| (i16::from(*actual) - expected as i16).abs() < 5));
    }
}

#[test]
fn phase3_chart_statistics_render_without_mutating_native_values() {
    let source = with_elements(json!([{
        "type":"chart","id":"statistics","x":10,"y":10,"width":300,"height":280,
        "kind":"line","categories":["1","2","3"],
        "series":[{"name":"Measured","values":[3,5,7],"color":"087F73",
            "trendline":{"kind":"linear","forward":1,"display_equation":true,"display_r_squared":true},
            "error_bars":{"kind":"fixed_value","value":1}}],
        "options":{"legend":"hidden","primary_axis":{"major_unit":2,"minor_unit":1,"number_format":"0.0"}}
    }]));
    let original = serde_json::to_vec(&source).unwrap();
    let png = export_static(&source, &ExportOptions::default()).unwrap();
    let pixels = image::load_from_memory(&png.artifacts[0].bytes).unwrap().into_rgb8();
    assert!(pixels.pixels().filter(|pixel| pixel[1] > pixel[0].saturating_add(30)).count() > 100);
    let pdf = export_static(&source, &ExportOptions { format: ExportFormat::Pdf, ..Default::default() }).unwrap();
    assert!(pdf.artifacts[0].bytes.starts_with(b"%PDF-"));
    assert_eq!(serde_json::to_vec(&source).unwrap(), original);
}

#[test]
fn phase3_wordart_shapes_glyphs_and_preserves_searchable_pdf_text() {
    for warp in ["arch_up", "arch_down", "wave1", "wave2", "inflate", "deflate", "slant_up", "slant_down"] {
        let source = with_elements(json!([{"type":"text","id":"warped","x":10,"y":20,"width":290,"height":200,"text":"office affinity","font_size":32,"color":"087F73","bold":true,"visual":{"text_warp":warp}}]));
        let before = serde_json::to_vec(&source).unwrap();
        let rendered = render_slide_svg(&source, 0, true).unwrap();
        assert!(rendered.warnings.iter().any(|warning| warning.code == "WORDART_APPROXIMATION"));
        assert!(!rendered.svg.contains("NaN"));
        let png = export_static(&source, &ExportOptions::default()).unwrap();
        let pixels = image::load_from_memory(&png.artifacts[0].bytes).unwrap().into_rgb8();
        assert!(pixels.pixels().filter(|pixel| pixel[1] > pixel[0].saturating_add(30)).count() > 100, "{warp}");
        let output = export_static(&source, &ExportOptions { format: ExportFormat::Pdf, ..Default::default() }).unwrap();
        let document = lopdf::Document::load_mem(&output.artifacts[0].bytes).unwrap();
        assert!(document.extract_text(&[1]).unwrap().contains("office affinity"), "{warp}");
        assert_eq!(serde_json::to_vec(&source).unwrap(), before);
    }
}

#[test]
fn transparent_png_and_jpeg_matte_preserve_crop_and_alpha() {
    let source = image::RgbaImage::from_fn(32, 16, |column, _row| {
        image::Rgba(if column < 16 {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 128]
        })
    });
    let mut encoded = std::io::Cursor::new(Vec::new());
    source
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let deck = with_elements(
        json!([{"type":"picture","id":"crop","x":32,"y":32,"width":128,"height":128,
        "base64":STANDARD.encode(encoded.into_inner()),"mime_type":"image/png","alt":"test","crop":{"left":0.5}}]),
    );
    let output = export_static(
        &deck,
        &ExportOptions {
            transparent: true,
            ..Default::default()
        },
    )
    .unwrap();
    let pixels = image::load_from_memory(&output.artifacts[0].bytes)
        .unwrap()
        .into_rgba8();
    assert_eq!(pixels.get_pixel(0, 0)[3], 0);
    assert_eq!(pixels.get_pixel(96, 96).0, [0, 0, 255, 128]);
    let output = export_static(
        &deck,
        &ExportOptions {
            format: ExportFormat::Jpeg,
            transparent: true,
            jpeg_matte: [0, 255, 0],
            jpeg_quality: 100,
            ..Default::default()
        },
    )
    .unwrap();
    let pixels = image::load_from_memory(&output.artifacts[0].bytes)
        .unwrap()
        .into_rgb8();
    assert!(pixels.get_pixel(0, 0)[1] > 250);
    assert!((i16::from(pixels.get_pixel(96, 96)[1]) - 127).abs() < 4);
    assert!((i16::from(pixels.get_pixel(96, 96)[2]) - 128).abs() < 4);
}

#[test]
fn rejects_extreme_budgets_selection_and_invalid_unselected_pages() {
    for scale in [f64::NAN, f64::INFINITY, 0.0, -1.0, 1000000.0] {
        assert!(export_static(
            &deck(),
            &ExportOptions {
                scale,
                ..Default::default()
            }
        )
        .is_err());
    }
    for page_indices in [vec![], vec![1], vec![0, 0]] {
        assert!(export_static(
            &deck(),
            &ExportOptions {
                page_indices: Some(page_indices),
                ..Default::default()
            }
        )
        .is_err());
    }
    assert!(export_static(
        &deck(),
        &ExportOptions {
            max_output_bytes: 8,
            ..Default::default()
        }
    )
    .is_err());
    let mut source = deck();
    source.width = 4096;
    source.height = 4096;
    assert!(matches!(
        export_static(&source, &ExportOptions::default()),
        Err(Error::Limit(_))
    ));
    let mut invalid = source.slides[0].clone();
    invalid.id = "unselected".into();
    invalid.background = "bad".into();
    source.slides.push(invalid);
    assert!(export_static(
        &source,
        &ExportOptions {
            scale: 0.25,
            page_indices: Some(vec![0]),
            ..Default::default()
        }
    )
    .is_err());
}

#[test]
fn multipage_pdf_has_order_header_and_canvas_paper_dimensions() {
    let mut source = deck();
    source.width = 640;
    let mut second = source.slides[0].clone();
    second.id = "second".into();
    second.elements.clear();
    source.slides.push(second);
    let output = export_static(
        &source,
        &ExportOptions {
            format: ExportFormat::Pdf,
            scale: 2.0,
            page_indices: Some(vec![1, 0]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(output.artifacts.len(), 1);
    assert!(!output.pdf_rasterized);
    let artifact = &output.artifacts[0];
    assert_eq!(artifact.page_indices, [1, 0]);
    assert_eq!(artifact.mime_type, "application/pdf");
    assert!(artifact.bytes.starts_with(b"%PDF-"));
    let document = lopdf::Document::load_mem(&artifact.bytes).unwrap();
    let pages = document.get_pages();
    assert_eq!(pages.len(), 2);
    for page in pages.values() {
        let bounds = document
            .get_object(*page)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"MediaBox")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(
            bounds
                .iter()
                .map(|value| value.as_float().unwrap())
                .collect::<Vec<_>>(),
            [0.0, 0.0, 480.0, 240.0]
        );
    }
    assert_eq!(
        serde_json::to_value(&source).unwrap()["slides"][0]["elements"][0]["fill"],
        "FF0000"
    );
}

#[test]
fn pdf_keeps_searchable_unicode_text_without_rasterizing_the_page() {
    let mut source = with_elements(json!([{
        "type":"text","id":"searchable","x":24,"y":36,"width":560,"height":96,
        "text":"Selectable PDF \u{65e5}\u{672c}\u{8a9e}","font_size":24,"color":"111111","bold":false
    }]));
    source.width = 640;
    let output = export_static(&source, &ExportOptions { format: ExportFormat::Pdf, ..Default::default() }).unwrap();
    assert!(!output.pdf_rasterized);
    let document = lopdf::Document::load_mem(&output.artifacts[0].bytes).unwrap();
    let extracted = document.extract_text(&[1]).unwrap();
    assert!(extracted.contains("Selectable PDF"), "missing searchable Latin text: {extracted:?}");
    assert!(extracted.contains("\u{65e5}\u{672c}\u{8a9e}"), "missing searchable Japanese text: {extracted:?}");
}

#[test]
fn selection_from_128_slides_retains_the_32_output_page_limit() {
    let mut source = deck();
    source.slides = (0..128)
        .map(|index| {
            let mut slide = source.slides[0].clone();
            slide.id = format!("page-{index}");
            slide.elements.clear();
            slide
        })
        .collect();
    let options = ExportOptions {
        format: ExportFormat::Pdf,
        page_indices: Some(vec![127, 64, 0]),
        ..Default::default()
    };
    let output = export_static(&source, &options).unwrap();
    assert_eq!(output.artifacts[0].page_indices, [127, 64, 0]);
    assert_eq!(lopdf::Document::load_mem(&output.artifacts[0].bytes).unwrap().get_pages().len(), 3);
    for page_indices in [None, Some((0..33).collect())] {
        assert!(matches!(export_static(&source, &ExportOptions {
            page_indices, ..options.clone()
        }), Err(Error::Limit(_))));
    }
    source.slides[12].background = "invalid".into();
    assert!(export_static(&source, &options).is_err());
}

#[test]
fn pdf_table_headers_follow_explicit_semantic_policy() {
    for (policy, expected) in [
        ("unknown", vec![]), ("none", vec![]),
        ("first_row", vec!["Column", "Column"]),
        ("first_column", vec!["Row", "Row"]),
        ("both", vec!["Both", "Column", "Row"]),
    ] {
        let mut source = with_elements(json!([{
            "type":"table","id":"table","x":20,"y":20,"width":280,"height":180,
            "font_size":18,"rows":[["Name","Value"],["First","42"]]
        }]));
        source.slides[0].review = Some(serde_json::from_value(json!({"table_headers":{"table":policy}})).unwrap());
        let original = serde_json::to_vec(&source).unwrap();
        let output = export_static(&source, &ExportOptions { format: ExportFormat::Pdf, ..Default::default() }).unwrap();
        let document = lopdf::Document::load_mem(&output.artifacts[0].bytes).unwrap();
        let mut scopes = Vec::new();
        let mut cells = 0;
        for dictionary in document.objects.values().filter_map(|object| object.as_dict().ok()) {
            let role = dictionary.get(b"S").ok().and_then(|value| value.as_name().ok());
            if role == Some(b"TH") {
                let scope = dictionary.get(b"A").unwrap().as_dict().unwrap().get(b"Scope").unwrap().as_name().unwrap();
                scopes.push(std::str::from_utf8(scope).unwrap());
                cells += 1;
            } else if role == Some(b"TD") {
                assert!(dictionary.get(b"A").unwrap().as_dict().unwrap().get(b"Scope").is_err());
                cells += 1;
            }
        }
        scopes.sort();
        assert_eq!(scopes, expected, "{policy}");
        assert_eq!(cells, 4);
        assert!(document.extract_text(&[1]).unwrap().contains("42"));
        assert_eq!(serde_json::to_vec(&source).unwrap(), original);
    }
}

#[test]
fn pdf_structure_tracks_reading_order_rich_cells_figures_and_decoration() {
    let mut source = with_elements(json!([
        {"type":"text","id":"later","x":20,"y":20,"width":260,"height":40,"text":"Second","font_size":20,"color":"000000","bold":false},
        {"type":"table","id":"table","x":20,"y":80,"width":280,"height":120,"font_size":18,
         "rows":[["Name","Value"],["日本語","Rich value"]],"format":{"cells":[{"row":1,"column":1,"style":{"text_format":{"paragraphs":[{"runs":[{"text":"Rich ","style":{"bold":true}},{"text":"value","style":{"italic":true}}]}]}}}]}},
        {"type":"text","id":"decorative","x":10,"y":260,"width":280,"height":30,"text":"Do not read","font_size":16,"color":"000000","bold":false},
        {"type":"rect","id":"figure","x":240,"y":220,"width":40,"height":30,"fill":"FF0000"}
    ]));
    source.slides[0].review = Some(serde_json::from_value(json!({
        "reading_order":["table","figure","later","decorative"],
        "table_headers":{"table":"first_row"},
        "accessibility":{"figure":{"description":"赤い図"},"decorative":{"decorative":true}}
    })).unwrap());
    let original = serde_json::to_vec(&source).unwrap();
    let output = export_static(&source, &ExportOptions { format: ExportFormat::Pdf, ..Default::default() }).unwrap();
    let document = lopdf::Document::load_mem(&output.artifacts[0].bytes).unwrap();
    let extracted = document.extract_text(&[1]).unwrap();
    assert!(extracted.find("Name").unwrap() < extracted.find("Second").unwrap(), "{extracted}");
    assert!(extracted.contains("日本語") && extracted.contains("Rich value"), "{extracted}");
    assert!(!extracted.contains("Do not read"));
    let catalog = document.catalog().unwrap();
    assert!(catalog.get(b"MarkInfo").unwrap().as_dict().unwrap().get(b"Marked").unwrap().as_bool().unwrap());
    let root_id = catalog.get(b"StructTreeRoot").unwrap().as_reference().unwrap();
    let root = document.get_dictionary(root_id).unwrap();
    assert!(root.get(b"ParentTree").is_ok());
    let roles: Vec<_> = document.objects.values().filter_map(|object| object.as_dict().ok())
        .filter_map(|dict| dict.get(b"S").ok()?.as_name().ok()).collect();
    for role in [b"Document".as_slice(), b"Sect", b"Table", b"TR", b"TH", b"TD", b"Figure", b"P"] {
        assert!(roles.contains(&role), "missing role {role:?}");
    }
    let figure = document.objects.values().filter_map(|object| object.as_dict().ok())
        .find(|dict| dict.get(b"S").ok().and_then(|role| role.as_name().ok()) == Some(b"Figure")).unwrap();
    assert_eq!(figure.get(b"Alt").unwrap().as_str().unwrap(), [vec![0xfe,0xff], "赤い図".encode_utf16().flat_map(u16::to_be_bytes).collect()].concat());
    let page_id = document.get_pages()[&1];
    let page = document.get_dictionary(page_id).unwrap();
    assert_eq!(page.get(b"StructParents").unwrap().as_i64().unwrap(), 0);
    let parent_tree = document.get_dictionary(root.get(b"ParentTree").unwrap().as_reference().unwrap()).unwrap();
    let numbers = parent_tree.get(b"Nums").unwrap().as_array().unwrap();
    assert_eq!(numbers[0].as_i64().unwrap(), 0);
    let parents = numbers[1].as_array().unwrap();
    let content = lopdf::content::Content::decode(&document.get_page_content(page_id)).unwrap();
    let marked: Vec<_> = content.operations.iter().filter(|operation| operation.operator == "BDC").collect();
    assert_eq!(marked.len(), parents.len());
    for operation in marked {
        let mcid = operation.operands[1].as_dict().unwrap().get(b"MCID").unwrap().as_i64().unwrap() as usize;
        let tag = document.get_dictionary(parents[mcid].as_reference().unwrap()).unwrap();
        assert_eq!(tag.get(b"Pg").unwrap().as_reference().unwrap(), page_id);
        assert_eq!(tag.get(b"K").unwrap().as_i64().unwrap(), mcid as i64);
    }
    assert_eq!(serde_json::to_vec(&source).unwrap(), original);
}

#[test]
fn rich_text_is_escaped_shaped_colored_and_reports_missing_fonts() {
    let source = with_elements(
        json!([{"type":"text","id":"rich","x":10,"y":10,"width":290,"height":140,
        "text":"Red <&> Blue 日本語","font_size":24,"color":"000000","bold":false,
        "format":{"font_family":"NoSuchFont-G35","paragraphs":[{"runs":[{"text":"Red <&> ","style":{"color":"FF0000","bold":true}},
            {"text":"Blue 日本語","style":{"color":"0000FF","font_size":28,"underline":true}}]}]}}]),
    );
    let svg = render_slide_svg(&source, 0, false).unwrap();
    roxmltree::Document::parse(&svg.svg).unwrap();
    assert!(svg.svg.contains("#FF0000"));
    assert!(svg.svg.contains("#0000FF"));
    assert!(svg
        .warnings
        .iter()
        .any(|warning| warning.code == "FONT_FALLBACK"));
    let output = export_static(&source, &ExportOptions::default()).unwrap();
    let pixels = image::load_from_memory(&output.artifacts[0].bytes)
        .unwrap()
        .into_rgba8();
    assert!(
        pixels
            .pixels()
            .filter(|pixel| pixel[0] > 150 && pixel[1] < 100 && pixel[2] < 100)
            .count()
            > 30
    );
    assert!(
        pixels
            .pixels()
            .filter(|pixel| pixel[2] > 150 && pixel[0] < 100 && pixel[1] < 100)
            .count()
            > 30
    );
    assert!(export_static(
        &source,
        &ExportOptions {
            deny_warnings: true,
            ..Default::default()
        }
    )
    .is_err());
}

#[test]
fn pdf_semantics_handle_multiline_whitespace_and_reject_invalid_or_oversized_input() {
    let source = with_elements(json!([
        {"type":"text","id":"multiline","x":12,"y":12,"width":280,"height":120,"text":"Alpha\n日本語\nOmega","font_size":20,"color":"111111","bold":false,
         "format":{"paragraphs":[{"runs":[{"text":"Alpha"}]},{"runs":[{"text":"日本語"}]},{"runs":[{"text":"Omega"}]}]}},
        {"type":"text","id":"space","x":12,"y":160,"width":200,"height":40,"text":"   ","font_size":20,"color":"111111","bold":false}
    ]));
    let options = ExportOptions { format: ExportFormat::Pdf, ..Default::default() };
    let output = export_static(&source, &options).unwrap();
    let document = lopdf::Document::load_mem(&output.artifacts[0].bytes).unwrap();
    let extracted = document.extract_text(&[1]).unwrap();
    for value in ["Alpha", "日本語", "Omega"] { assert_eq!(extracted.matches(value).count(), 1, "{extracted}"); }
    assert!(document.objects.values().filter_map(|object| object.as_dict().ok()).all(|dict| !dict.has(b"FontFile") && !dict.has(b"FontFile2") && !dict.has(b"FontFile3")));
    assert!(matches!(export_static(&source, &ExportOptions { max_output_bytes: 64, ..options.clone() }), Err(Error::Limit(_))));
    let mut invalid = source.clone();
    invalid.slides[0].review = Some(serde_json::from_value(json!({"reading_order":["missing"]})).unwrap());
    assert!(export_static(&invalid, &options).is_err());
    invalid = source.clone();
    invalid.embedded_fonts.push(aislide_core::fonts::EmbeddedFont { family: "Invalid".into(), style: aislide_core::fonts::FontStyle::Regular, base64: STANDARD.encode(b"not a font"), license_acknowledged: true });
    assert!(export_static(&invalid, &options).is_err());
}

#[test]
fn pdf_grouped_figure_uses_transformed_bounds_and_alt_description() {
    let mut source = with_elements(json!([{"type":"group","id":"group","x":40,"y":60,"width":200,"height":100,"view_width":100,"view_height":100,
        "children":[{"type":"rect","id":"figure","x":10,"y":20,"width":20,"height":30,"fill":"00AA00"}]}]));
    source.slides[0].review = Some(serde_json::from_value(json!({"accessibility":{"figure":{"description":"Grouped figure"}}})).unwrap());
    let output = export_static(&source, &ExportOptions { format: ExportFormat::Pdf, ..Default::default() }).unwrap();
    let document = lopdf::Document::load_mem(&output.artifacts[0].bytes).unwrap();
    let figure = document.objects.values().filter_map(|object| object.as_dict().ok())
        .find(|dict| dict.get(b"S").ok().and_then(|role| role.as_name().ok()) == Some(b"Figure")).unwrap();
    let bounds = figure.get(b"A").unwrap().as_dict().unwrap().get(b"BBox").unwrap().as_array().unwrap();
    assert_eq!(bounds.iter().map(|number| number.as_float().unwrap()).collect::<Vec<_>>(), [45.0, 157.5, 75.0, 180.0]);
}

#[test]
fn table_merges_tracks_padding_and_rich_styles_render() {
    let source = with_elements(
        json!([{"type":"table","id":"table","x":10,"y":10,"width":300,"height":200,
        "font_size":20,"rows":[["Merged 42",""],["Wide","Thin"]],"format":{
            "column_widths":{"unit":"relative","values":[2,1]},"row_heights":{"unit":"relative","values":[1,3]},
            "merges":[{"row":0,"column":0,"row_span":1,"col_span":2}],
            "cells":[{"row":0,"column":0,"style":{"fill":"00FF00","text_style":{"color":"FF0000","bold":true}}}]}}]),
    );
    let output = export_static(&source, &ExportOptions::default()).unwrap();
    let pixels = image::load_from_memory(&output.artifacts[0].bytes)
        .unwrap()
        .into_rgba8();
    assert_eq!(pixels.get_pixel(250, 40).0, [0, 255, 0, 255]);
    assert!(
        pixels
            .enumerate_pixels()
            .filter(|(column, row, pixel)| *column < 200
                && *row < 60
                && pixel[0] > 150
                && pixel[1] < 100)
            .count()
            > 20
    );
}

#[test]
fn grouped_geometry_rotation_and_fill_only_opacity_render() {
    let source = with_elements(
        json!([{"type":"group","id":"group","x":100,"y":100,"width":100,"height":100,"view_width":200,"view_height":200,
        "visual":{"rotation":90},"children":[{"type":"rect","id":"child","x":0,"y":0,"width":80,"height":80,"fill":"FF0000","visual":{"opacity":0.5}}]}]),
    );
    let output = export_static(
        &source,
        &ExportOptions {
            transparent: true,
            ..Default::default()
        },
    )
    .unwrap();
    let pixels = image::load_from_memory(&output.artifacts[0].bytes)
        .unwrap()
        .into_rgba8();
    assert_eq!(pixels.get_pixel(180, 120).0, [255, 0, 0, 128]);
    assert_eq!(pixels.get_pixel(120, 120)[3], 0);
}

#[test]
fn unsafe_svg_and_unknown_chart_modes_fail_instead_of_rendering_substitutes() {
    let mut source = deck();
    let picture = json!({"type":"picture","id":"unsafe","x":0,"y":0,"width":100,"height":100,"base64":"","mime_type":"image/png","alt":"",
        "svg":STANDARD.encode("<svg xmlns='http://www.w3.org/2000/svg'><image href='https://example.invalid/private'/></svg>")});
    source.slides[0].elements = vec![serde_json::from_value(picture).unwrap()];
    assert!(export_static(&source, &ExportOptions::default()).is_err());
    let source = with_elements(
        json!([{"type":"chart","id":"unsupported","x":0,"y":0,"width":300,"height":300,
        "kind":"column","categories":["A","B"],"series":[{"name":"Values","values":[10,20],"color":"FF0000"}],
        "options":{"primary_axis":{"number_format":"unsupported-custom-format"}}}]),
    );
    assert!(
        export_static(&source,&ExportOptions::default()).unwrap().warnings.iter().any(|warning| warning.message.contains("Unsupported number format"))
    );
}

#[test]
fn chart_renders_actual_values_categories_and_labels() {
    let source = with_elements(
        json!([{"type":"chart","id":"chart","x":0,"y":0,"width":320,"height":300,
        "kind":"column","categories":["Low","High"],"series":[{"name":"Measured","values":[12,42],"color":"FF0000"}],
        "options":{"data_labels":{"show_value":true}}}]),
    );
    let svg = render_slide_svg(&source, 0, false).unwrap();
    assert!(svg.svg.contains("42"));
    assert!(svg.svg.contains("Measured"));
    assert!(svg.svg.contains("High"));
    assert!(!svg.warnings.iter().any(|warning| warning.code == "TEXT_OVERFLOW"), "{:?}", svg.warnings);
    let output = export_static(&source, &ExportOptions::default()).unwrap();
    let pixels = image::load_from_memory(&output.artifacts[0].bytes)
        .unwrap()
        .into_rgba8();
    let count = |left: u32, right: u32| {
        pixels
            .enumerate_pixels()
            .filter(|(column, _row, pixel)| {
                *column >= left
                    && *column < right
                    && pixel[0] > 150
                    && pixel[1] < 80
                    && pixel[2] < 80
            })
            .count()
    };
    assert!(count(160, 310) > count(50, 160) * 2);
}

#[test]
fn rich_paragraph_whitespace_never_turns_into_replacement_boxes() {
    let source = with_elements(
        json!([{"type":"text","id":"paragraphs","x":10,"y":10,"width":280,"height":250,"font_size":24,"color":"000000","bold":false,
        "text":"First\n A","format":{"paragraphs":[{"runs":[{"text":"First"}]},{"runs":[{"text":" A"}],"alignment":"right","space_before":{"kind":"points","value":750}}]}}]),
    );
    let rendered = render_slide_svg(&source, 0, false).unwrap();
    assert!(
        !rendered
            .warnings
            .iter()
            .any(|warning| warning.code == "GLYPH_OUTLINE_UNAVAILABLE"),
        "{:?}",
        rendered.warnings
    );
}

#[test]
fn every_preset_shape_renders_and_has_explicit_approximation_policy() {
    for preset in aislide_core::objects::SHAPES
        .iter()
        .map(|(preset, _)| *preset)
        .chain(["can", "cloud"])
    {
        let element = aislide_core::objects::create(
            "shape".into(),
            aislide_core::objects::ObjectKind::Shape,
            Some(preset.into()),
            None,
            None,
        )
        .unwrap();
        let mut source = deck();
        source.width = 1280;
        source.height = 720;
        source.slides[0].elements = vec![element];
        let output = export_static(
            &source,
            &ExportOptions {
                scale: 0.25,
                ..Default::default()
            },
        )
        .unwrap_or_else(|error| panic!("{preset}: {error}"));
        let pixels = image::load_from_memory(&output.artifacts[0].bytes)
            .unwrap()
            .into_rgb8();
        assert!(
            pixels.pixels().filter(|pixel| pixel.0 != [255; 3]).count() > 100,
            "{preset} was blank"
        );
        if !matches!(
            preset,
            "rect" | "ellipse" | "flowChartProcess" | "flowChartConnector"
        ) {
            assert!(
                output
                    .warnings
                    .iter()
                    .any(|warning| warning.code == "SHAPE_APPROXIMATION"),
                "{preset}"
            );
        }
    }
}

#[test]
fn all_24_chart_families_render_png_pdf_without_mutating_data() {
    assert_eq!(aislide_core::model::chart_format::KINDS.len(), 24);
    for kind in aislide_core::model::chart_format::KINDS {
        let preset = serde_json::to_value(kind)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let element = aislide_core::objects::create(
            "chart".into(),
            aislide_core::objects::ObjectKind::Chart,
            Some(preset),
            None,
            None,
        )
        .unwrap();
        let mut source = deck();
        source.width = 1280;
        source.height = 720;
        source.slides[0].elements = vec![element];
        let before = serde_json::to_value(&source).unwrap();
        let result = export_static(
            &source,
            &ExportOptions {
                scale: 1.0,
                ..Default::default()
            },
        );
            let output = result.unwrap_or_else(|error| panic!("{kind:?}: {error}"));
            let pixels = image::load_from_memory(&output.artifacts[0].bytes)
                .unwrap()
                .into_rgb8();
            assert!(
                pixels.pixels().filter(|pixel| pixel.0.iter().max().unwrap() - pixel.0.iter().min().unwrap() > 35).count() > 100,
                "{kind:?} was blank"
            );
            assert!(output
                .warnings
                .iter()
                .any(|warning| warning.code == "CHART_PREVIEW"));
        assert_eq!(output.warnings.iter().any(|warning| warning.code == "CHART_3D_PROJECTED"), kind.is_3d());
        let pdf = export_static(&source, &ExportOptions { format: ExportFormat::Pdf, scale: 0.25, ..Default::default() })
            .unwrap_or_else(|error| panic!("{kind:?} PDF: {error}"));
        assert!(!pdf.pdf_rasterized);
        let document = lopdf::Document::load_mem(&pdf.artifacts[0].bytes).unwrap();
        assert_eq!(document.get_pages().len(), 1);
        assert!(document.get_page_content(document.get_pages()[&1]).len() > 100);
        assert_eq!(serde_json::to_value(&source).unwrap(), before);
        assert!(export_static(&source, &ExportOptions { deny_warnings: true, scale: 0.25, ..Default::default() }).is_err());
    }
}

#[test]
fn master_layout_theme_layers_and_hidden_master_graphics_are_resolved() {
    use aislide_core::design::{Design, Master, SlideLayout, Theme};
    let mut source = deck();
    source.slides[0].elements.clear();
    source.slides[0].layout_id = Some("layout".into());
    source.slides[0].inherit_background = true;
    let rect = |id: &str, left: f64, fill: &str| {
        serde_json::from_value(
            json!({"type":"rect","id":id,"x":left,"y":0,"width":40,"height":40,"fill":fill}),
        )
        .unwrap()
    };
    let mut theme = Theme::default();
    theme.colors.insert("accent1".into(), "FF0000".into());
    source.design = Some(Design {
        theme,
        masters: vec![Master {
            id: "master".into(),
            theme: None,
            name: "master".into(),
            background: "00FF00".into(),
            elements: vec![rect("master-art", 0.0, "@accent1")],
        }],
        layouts: vec![SlideLayout {
            id: "layout".into(),
            name: "layout".into(),
            master_id: "master".into(),
            background: Some("0000FF".into()),
            elements: vec![rect("layout-art", 40.0, "FFFF00")],
        }],
    });
    let sample = |deck: &Deck| {
        image::load_from_memory(
            &export_static(deck, &ExportOptions::default())
                .unwrap()
                .artifacts[0]
                .bytes,
        )
        .unwrap()
        .into_rgba8()
    };
    let pixels = sample(&source);
    assert_eq!(pixels.get_pixel(10, 10).0, [255, 0, 0, 255]);
    assert_eq!(pixels.get_pixel(50, 10).0, [255, 255, 0, 255]);
    assert_eq!(pixels.get_pixel(200, 200).0, [0, 0, 255, 255]);
    source.slides[0].hide_master_graphics = true;
    let pixels = sample(&source);
    assert_eq!(pixels.get_pixel(10, 10).0, [0, 0, 255, 255]);
    assert_eq!(pixels.get_pixel(50, 10).0, [255, 255, 0, 255]);
}

fn chart_deck(kind: &str, categories: serde_json::Value, values: serde_json::Value, options: serde_json::Value) -> Deck {
    let mut source = with_elements(json!([{"type":"chart","id":"chart","x":0,"y":0,"width":640,"height":400,
        "kind":kind,"categories":categories,"series":[{"name":"Observed","values":values,"color":"FF0000"}],"options":options}]));
    source.width = 640;
    source.height = 400;
    source
}

#[test]
fn histogram_width_overflow_labels_match_counts() {
    for (closed, expected) in [
        ("right", ["[0, 4]: 2", "(4, 5]: 1", ">5: 1"]),
        ("left", ["[0, 4): 1", "[4, 5]: 2", ">5: 1"]),
    ] {
        let source = chart_deck("histogram", json!([]), json!([]), json!({"histogram":{
            "samples":[0,4,5,6],"binning":{"rule":"width","width":4},"interval_closed":closed,"overflow":5}}));
        let rendered = render_slide_svg(&source, 0, false).unwrap();
        let parsed = roxmltree::Document::parse(&rendered.svg).unwrap();
        let labels: Vec<_> = parsed.descendants().filter(|node| node.has_tag_name("title")).filter_map(|node| node.text()).map(str::trim).collect();
        for expected in expected { assert!(labels.contains(&expected), "missing {expected}: {labels:?}"); }
        assert!(!labels.iter().any(|label| label.contains(", 8]")));
    }
}

#[test]
fn pie_category_legend_is_rendered_unless_hidden() {
    for kind in ["pie", "pie3d", "doughnut"] {
        for legend in ["left", "right", "top", "bottom", "top_right", "hidden"] {
            let source = chart_deck(kind, json!(["Alpha", "Beta"]), json!([1,3]), json!({"data_labels":{},"legend":legend}));
            let rendered = render_slide_svg(&source, 0, false).unwrap();
            let parsed = roxmltree::Document::parse(&rendered.svg).unwrap();
            let labels: Vec<_> = parsed.descendants().filter(|node| node.has_tag_name("title")).filter_map(|node| node.text()).map(str::trim).collect();
            for category in ["Alpha", "Beta"] { assert_eq!(labels.contains(&category), legend != "hidden", "{kind} {legend}: {labels:?}"); }
        }
    }
}

#[test]
fn statistical_geometry_uses_raw_samples_thresholds_quartiles_and_outliers() {
    for (closed, expected) in [("left", ["<=0: 2", "(0, 2): 1", "[2, 4]: 3", ">4: 1"]),
        ("right", ["<=0: 2", "(0, 2]: 2", "(2, 4]: 2", ">4: 1"])] {
        let source = chart_deck("histogram", json!([]), json!([]), json!({"histogram":{
            "samples":[-1,0,1,2,3,4,5],"binning":{"rule":"count","count":2},"interval_closed":closed,"underflow":0,"overflow":4}}));
        let before = serde_json::to_value(&source).unwrap();
        let rendered = render_slide_svg(&source, 0, false).unwrap();
        let parsed = roxmltree::Document::parse(&rendered.svg).unwrap();
        let titles: Vec<_> = parsed.descendants().filter(|node| node.has_tag_name("title")).filter_map(|node| node.text()).map(str::trim).collect();
        for expected in expected { assert!(titles.contains(&expected), "missing {expected}: {titles:?}"); }
        assert_eq!(serde_json::to_value(&source).unwrap(), before);
    }
    for (method, quartiles) in [("inclusive", "median 4.5; Q1 2.75; Q3 6.25; whiskers 1..7"),
        ("exclusive", "median 4.5; Q1 2.25; Q3 6.75; whiskers 1..7")] {
        let source = chart_deck("box_whisker", json!(["Group"]), json!([]), json!({"box_whisker":{
            "samples":[[1,2,3,4,5,6,7,100]],"quartile_method":method,"mean_line":false,"mean_marker":false,"nonoutliers":false,"outliers":true}}));
        let rendered = render_slide_svg(&source, 0, false).unwrap();
        assert!(rendered.svg.contains(quartiles));
        let parsed = roxmltree::Document::parse(&rendered.svg).unwrap();
        assert_eq!(parsed.descendants().filter(|node| node.has_tag_name("circle")).count(), 1);
    }
}

#[test]
fn sparse_statistical_and_hierarchy_inputs_do_not_invent_data() {
    let single = chart_deck("histogram", json!([]), json!([]), json!({"histogram":{
        "samples":[7],"binning":{"rule":"count","count":1},"interval_closed":"left"}}));
    assert!(render_slide_svg(&single, 0, false).unwrap().svg.contains("[7, 8]: 1"));
    let equal = chart_deck("box_whisker", json!(["Equal"]), json!([]), json!({"box_whisker":{
        "samples":[[7,7,7,7]],"quartile_method":"exclusive","mean_line":true,"mean_marker":true,"nonoutliers":true,"outliers":true}}));
    assert!(render_slide_svg(&equal, 0, false).unwrap().svg.contains("median 7; Q1 7; Q3 7; whiskers 7..7"));
    for source in [single, equal] {
        let mut invalid = serde_json::to_value(source).unwrap();
        let options = &mut invalid["slides"][0]["elements"][0]["options"];
        if options.get("histogram").is_some() { options["histogram"]["samples"] = json!([]); }
        else { options["box_whisker"]["samples"] = json!([[7]]); }
        assert!(export_static(&serde_json::from_value(invalid).unwrap(), &ExportOptions::default()).is_err());
    }
    for kind in ["treemap", "sunburst"] {
        let source = chart_deck(kind, json!(["Single"]), json!([9]), json!({"hierarchy":{"paths":[["Parent","Single"]]}}));
        let rendered = render_slide_svg(&source, 0, false).unwrap();
        assert!(rendered.svg.contains("Parent: 9") && rendered.svg.contains("Single: 9"));
    }
}

#[test]
fn floating_waterfall_and_hierarchy_geometry_preserve_values_and_parent_paths() {
    let waterfall = chart_deck("waterfall", json!(["Start","Loss","Gain","Total"]), json!([100,-30,20,90]), json!({"waterfall_totals":[0,3]}));
    let rendered = render_slide_svg(&waterfall, 0, false).unwrap();
    let parsed = roxmltree::Document::parse(&rendered.svg).unwrap();
    let titles: Vec<_> = parsed.descendants().filter(|node| node.has_tag_name("title")).filter_map(|node| node.text()).map(str::trim).collect();
    for value in ["0 -> 100", "100 -> 70", "70 -> 90", "0 -> 90"] { assert!(titles.contains(&value)); }
    let source = chart_deck("treemap", json!(["First","Second","Third"]), json!([1,3,4]), json!({"hierarchy":{"paths":[["ParentA","First"],["ParentA","Second"],["ParentB","Third"]],"parent_labels":"none"}}));
    let rendered = render_slide_svg(&source, 0, false).unwrap();
    assert!(!rendered.svg.contains("ParentA:"));
    for value in ["First: 1", "Second: 3", "Third: 4"] { assert!(rendered.svg.contains(value)); }
    let parsed = roxmltree::Document::parse(&rendered.svg).unwrap();
    let first_color = format!("#{}", aislide_core::design::Theme::default().colors["accent1"]);
    let areas: Vec<f64> = parsed.descendants().filter(|node| node.has_tag_name("rect") && node.attribute("fill") == Some(first_color.as_str()))
        .map(|node| node.attribute("width").unwrap().parse::<f64>().unwrap() * node.attribute("height").unwrap().parse::<f64>().unwrap()).collect();
    assert_eq!(areas.len(), 3, "parent and two children: {areas:?}");
    assert!((areas[1] / areas[2] - 1.0 / 3.0).abs() < 0.015, "leaf area ratio: {areas:?}");
    assert_eq!(render_slide_svg(&source, 0, false).unwrap().svg, rendered.svg);
}

#[test]
fn bubble_area_and_combo_secondary_axis_share_correct_coordinates() {
    let mut bubble = chart_deck("bubble", json!(["1","2"]), json!([10,20]), json!({"legend":"hidden"}));
    if let aislide_core::model::Element::Chart { series, .. } = &mut bubble.slides[0].elements[0] { series[0].bubble_sizes = Some(vec![1.0,4.0]); }
    let rendered = render_slide_svg(&bubble, 0, false).unwrap();
    let parsed = roxmltree::Document::parse(&rendered.svg).unwrap();
    let radii: Vec<f64> = parsed.descendants().filter(|node| node.has_tag_name("circle")).map(|node| node.attribute("r").unwrap().parse().unwrap()).collect();
    assert_eq!(radii, [12.0,24.0]);
    let mut combo = serde_json::to_value(chart_deck("column", json!(["A","B"]), json!([10,20]), json!({}))).unwrap();
    let chart = &mut combo["slides"][0]["elements"][0];
    chart["kind"] = json!("combo");
    chart["series"] = json!([
        {"name":"Primary","values":[10,20],"color":"FF0000","kind":"column","axis":"primary"},
        {"name":"Secondary","values":[1000,2000],"color":"0000FF","kind":"line","axis":"secondary"}]);
    chart["options"] = json!({"primary_axis":{"min":0,"max":40},"secondary_axis":{"min":0,"max":4000},"legend":"hidden"});
    let rendered = render_slide_svg(&serde_json::from_value(combo).unwrap(), 0, false).unwrap();
    let parsed = roxmltree::Document::parse(&rendered.svg).unwrap();
    let tops: Vec<i32> = parsed.descendants().filter(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#FF0000"))
        .map(|node| node.attribute("y").unwrap().parse().unwrap()).collect();
    let line = parsed.descendants().find(|node| node.has_tag_name("polyline") && node.attribute("stroke") == Some("#0000FF")).unwrap();
    let line_tops: Vec<i32> = line.attribute("points").unwrap().split_whitespace().map(|point| point.split(',').nth(1).unwrap().parse().unwrap()).collect();
    assert_eq!(tops, line_tops);
    assert!(rendered.svg.contains("4000"));
}

#[test]
fn gradients_curves_connectors_and_picture_masks_have_visible_geometry() {
    let source = with_elements(json!([
        {"type":"rect","id":"gradient","x":0,"y":0,"width":100,"height":80,"fill":"FF0000","visual":{"gradient":{"kind":"linear","angle":0,"stops":[{"offset":0,"color":"FF0000","opacity":1},{"offset":1,"color":"0000FF","opacity":1}]}}},
        {"type":"polygon","id":"curve","x":120,"y":0,"width":100,"height":100,"fill":"00FF00","stroke":"000000","stroke_width":1,
            "points":[[0,0],[1,0],[1,1],[0,1]],"visual":{"path":{"commands":[{"op":"move","point":[0,0]},{"op":"cubic","control1":[1,0],"control2":[1,1],"point":[0,1]},{"op":"close"}]}}},
        {"type":"connector","id":"arrow","x":10,"y":150,"width":200,"height":60,"color":"000000","stroke_width":4,"arrow":true,"routing":{"points":[[0,0],[1,0],[1,1]],"dashed":true,"start_arrow":true}}
    ]));
    let result = export_static(&source, &ExportOptions::default()).unwrap();
    let pixels = image::load_from_memory(&result.artifacts[0].bytes)
        .unwrap()
        .into_rgb8();
    assert!(pixels.get_pixel(10, 30)[0] > 200);
    assert!(pixels.get_pixel(90, 30)[2] > 200);
    assert!(pixels.get_pixel(140, 50)[1] > 200);
    assert!(
        pixels
            .enumerate_pixels()
            .filter(|(_column, row, pixel)| *row > 145 && pixel.0 == [0; 3])
            .count()
            > 100
    );
    let raster = image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    raster
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    for mask in ["ellipse", "round_rect", "diamond", "hexagon"] {
        let source = with_elements(
            json!([{"type":"picture","id":"masked","x":10,"y":10,"width":100,"height":100,"base64":STANDARD.encode(bytes.get_ref()),"mime_type":"image/png","alt":"mask","visual":{"picture_mask":mask}}]),
        );
        let result = export_static(
            &source,
            &ExportOptions {
                transparent: true,
                ..Default::default()
            },
        )
        .unwrap();
        let pixels = image::load_from_memory(&result.artifacts[0].bytes)
            .unwrap()
            .into_rgba8();
        assert_eq!(pixels.get_pixel(60, 60).0, [255, 0, 0, 255], "{mask}");
        assert_eq!(pixels.get_pixel(10, 10)[3], 0, "{mask}");
    }
}

#[test]
fn missing_unicode_and_unsupported_effects_are_not_silently_omitted() {
    let source = with_elements(
        json!([{"type":"text","id":"missing","x":10,"y":10,"width":200,"height":100,"text":"\u{10ffff}","font_size":24,"color":"000000","bold":false}]),
    );
    let result = export_static(&source, &ExportOptions::default()).unwrap();
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.code == "MISSING_GLYPHS"));
    let source = with_elements(
        json!([{"type":"text","id":"effect","x":10,"y":10,"width":100,"height":100,"text":"Warp","font_size":20,"color":"FF0000","bold":false,"visual":{"text_warp":"wave1"}}]),
    );
    assert!(
        export_static(&source,&ExportOptions::default()).unwrap().warnings.iter().any(|warning| warning.code == "WORDART_APPROXIMATION")
    );
}

#[test]
fn effects_render_alpha_reflection_and_pdf_with_explicit_approximation_warnings() {
    for visual in [
        json!({"shadow":{"color":"0000FF","opacity":0.7,"blur":4,"distance":20,"angle":0}}),
        json!({"glow":{"color":"0000FF","opacity":0.7,"radius":10}}),
        json!({"soft_edge":8}),
        json!({"reflection":{"blur":1,"distance":10,"start_opacity":0.8,"end_opacity":0,"end_position":1}}),
    ] {
        let source = with_elements(json!([{"type":"rect","id":"effect","x":80,"y":40,"width":80,"height":60,"fill":"FF0000","visual":visual}]));
        let before = serde_json::to_value(&source).unwrap();
        let rendered = render_slide_svg(&source, 0, true).unwrap();
        assert!(rendered.warnings.iter().any(|warning| warning.code == "EFFECT_APPROXIMATION"));
        let output = export_static(&source, &ExportOptions { transparent:true, ..Default::default() }).unwrap();
        let pixels = image::load_from_memory(&output.artifacts[0].bytes).unwrap().into_rgba8();
        assert!(pixels.get_pixel(120, 70)[0] > 200);
        assert!(pixels.pixels().any(|pixel| pixel[3] > 0 && pixel[3] < 250));
        if visual.get("reflection").is_some() { assert!(pixels.get_pixel(120, 125)[3] > 30); }
        let pdf = export_static(&source, &ExportOptions { format:ExportFormat::Pdf, ..Default::default() }).unwrap();
        assert!(!pdf.pdf_rasterized);
        let document = lopdf::Document::load_mem(&pdf.artifacts[0].bytes).unwrap();
        assert_eq!(document.get_pages().len(), 1);
        assert!(document.objects.values().any(|object| object.as_stream().ok().map(|stream| &stream.dict)
            .and_then(|dictionary| dictionary.get(b"Subtype").ok()).and_then(|value| value.as_name().ok()) == Some(b"Image")), "effect vanished from PDF: {visual}");
        assert!(pdf.warnings.iter().any(|warning| warning.code == "PDF_EFFECT_RASTERIZED"));
        assert!(export_static(&source, &ExportOptions { deny_warnings:true, ..Default::default() }).is_err());
        assert_eq!(serde_json::to_value(&source).unwrap(), before);
    }
}

#[test]
fn hidden_group_effects_emit_nothing_and_filter_budgets_reject_before_output() {
    let source = with_elements(json!([{"type":"group","id":"group","x":0,"y":0,"width":100,"height":100,"view_width":100,"view_height":100,
        "visual":{"hidden":true,"soft_edge":10},"children":[{"type":"rect","id":"child","x":0,"y":0,"width":80,"height":80,"fill":"FF0000","visual":{"glow":{"color":"0000FF","opacity":1,"radius":100}}}]}]));
    let output = export_static(&source, &ExportOptions { transparent:true, deny_warnings:true, ..Default::default() }).unwrap();
    let pixels = image::load_from_memory(&output.artifacts[0].bytes).unwrap().into_rgba8();
    assert!(pixels.pixels().all(|pixel| pixel[3] == 0));
    let mut source = with_elements(json!([{"type":"rect","id":"effect","x":0,"y":0,"width":320,"height":320,"fill":"FF0000","visual":{"soft_edge":100}}]));
    source.slides[0].elements = (0..64).map(|index| {
        let mut element = serde_json::to_value(&source.slides[0].elements[0]).unwrap();
        element["id"] = json!(format!("effect-{index}"));
        serde_json::from_value(element).unwrap()
    }).collect();
    assert!(matches!(export_static(&source, &ExportOptions::default()), Err(Error::Limit(message)) if message.contains("filter")));
}

#[test]
fn wide_effects_fit_pdf_budget_and_compound_holes_survive_group_filters() {
    let mut wide = with_elements(json!([{"type":"rect","id":"wide","x":40,"y":40,"width":600,"height":200,"fill":"FF0000",
        "visual":{"shadow":{"color":"000000","opacity":0.5,"blur":4,"distance":10,"angle":0}}}]));
    wide.width = 1280;
    wide.height = 720;
    assert!(export_static(&wide, &ExportOptions { format:ExportFormat::Pdf, ..Default::default() }).is_ok());
    let source = with_elements(json!([{"type":"group","id":"group","x":20,"y":20,"width":160,"height":160,"view_width":160,"view_height":160,
        "visual":{"shadow":{"color":"0000FF","opacity":0.7,"blur":2,"distance":8,"angle":0}},
        "children":[{"type":"polygon","id":"hole","x":0,"y":0,"width":160,"height":160,"fill":"FF0000","stroke":"000000","stroke_width":0,
            "points":[[0,0],[1,0],[1,1],[0,1],[0.25,0.25],[0.25,0.75],[0.75,0.75],[0.75,0.25]],
            "visual":{"path":{"commands":[{"op":"move","point":[0,0]},{"op":"line","point":[1,0]},{"op":"line","point":[1,1]},{"op":"line","point":[0,1]},{"op":"close"},
                {"op":"move","point":[0.25,0.25]},{"op":"line","point":[0.25,0.75]},{"op":"line","point":[0.75,0.75]},{"op":"line","point":[0.75,0.25]},{"op":"close"}]}}}]}]));
    let output = export_static(&source, &ExportOptions { transparent:true, ..Default::default() }).unwrap();
    let pixels = image::load_from_memory(&output.artifacts[0].bytes).unwrap().into_rgba8();
    assert_eq!(pixels.get_pixel(100,100)[3], 0);
    assert!(pixels.get_pixel(35,35)[0] > 240);
    let pdf = export_static(&source, &ExportOptions { format:ExportFormat::Pdf, transparent:true, ..Default::default() }).unwrap();
    let document = lopdf::Document::load_mem(&pdf.artifacts[0].bytes).unwrap();
    let image = document.objects.values().filter_map(|object| object.as_stream().ok())
        .find(|stream| stream.dict.get(b"ColorSpace").ok().and_then(|value| value.as_name().ok()) == Some(b"DeviceRGB")).unwrap();
    let samples = image.decompressed_content().unwrap();
    assert!(samples.chunks_exact(3).any(|pixel| pixel[0] > 240 && pixel[1] < 10 && pixel[2] < 10));
    let mask = document.get_object(image.dict.get(b"SMask").unwrap().as_reference().unwrap()).unwrap().as_stream().unwrap();
    let alpha = mask.decompressed_content().unwrap();
    assert!(alpha.contains(&0) && alpha.contains(&255));
}

#[test]
fn grouped_rich_runs_keep_margin_colors_wrapping_and_scale() {
    let source = with_elements(json!([{"type":"group","id":"group","x":0,"y":0,"width":160,"height":160,"view_width":320,"view_height":320,
        "children":[{"type":"text","id":"rich","x":0,"y":0,"width":300,"height":280,"text":"Margin Bold Mixed Wrap Words","font_size":12,"color":"000000","bold":false,
            "format":{"paragraphs":[{"margin_left":381000,"runs":[{"text":"Margin Bold ","style":{"font_size":28,"bold":true,"color":"FF0000"}},
                {"text":"Mixed Wrap Words","style":{"font_size":40,"color":"0000FF","underline":true}}]}]}}]}]));
    let output = export_static(&source, &ExportOptions { transparent:true, ..Default::default() }).unwrap();
    let pixels = image::load_from_memory(&output.artifacts[0].bytes).unwrap().into_rgba8();
    let visible: Vec<_> = pixels.enumerate_pixels().filter(|(_,_,pixel)| pixel[3] > 20).collect();
    assert!(visible.iter().all(|(column,row,_)| *column >= 19 && *column < 160 && *row < 160));
    assert!(visible.iter().any(|(_,_,pixel)| pixel[0] > 200 && pixel[2] < 50));
    assert!(visible.iter().any(|(_,_,pixel)| pixel[2] > 200 && pixel[0] < 50));
    assert!(!output.warnings.iter().any(|warning| warning.code == "TEXT_OVERFLOW"));
}

#[test]
fn reflection_reference_expansion_is_bounded_before_svg_conversion() {
    let mut children = (0..128).map(|index| json!({"type":"rect","id":format!("leaf-{index}"),"x":0,"y":0,"width":1,"height":1,"fill":"FF0000"})).collect::<Vec<_>>();
    for depth in 0..7 {
        children = vec![json!({"type":"group","id":format!("level-{depth}"),"x":0,"y":0,"width":1,"height":1,"view_width":1,"view_height":1,
            "visual":{"reflection":{"blur":0,"distance":0,"start_opacity":1,"end_opacity":0,"end_position":1}},"children":children})];
    }
    let source = with_elements(json!(children));
    let failure = render_slide_svg(&source, 0, true).err();
    assert!(matches!(failure, Some(Error::Limit(message)) if message.contains("expansion")), "reflection expansion must reject before rendering");
}

#[test]
fn pdf_text_is_vector_outlines_and_ordered_pages_have_real_content() {
    let mut source = with_elements(
        json!([{"type":"text","id":"text","x":10,"y":10,"width":200,"height":100,"text":"Vector 42","font_size":24,"color":"000000","bold":false}]),
    );
    let mut empty = source.slides[0].clone();
    empty.id = "empty".into();
    empty.elements.clear();
    source.slides.push(empty);
    let output = export_static(
        &source,
        &ExportOptions {
            format: ExportFormat::Pdf,
            transparent: true,
            page_indices: Some(vec![1, 0]),
            ..Default::default()
        },
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&output.artifacts[0].bytes).unwrap();
    let pages = document.get_pages();
    let first = document.get_page_content(pages[&1]);
    let second = document.get_page_content(pages[&2]);
    assert!(second.len() > first.len() + 100);
    assert!(!output.pdf_rasterized);
    assert!(!document.objects.values().any(|object| object
        .as_dict()
        .ok()
        .and_then(|dictionary| dictionary.get(b"Subtype").ok())
        .and_then(|value| value.as_name().ok())
        == Some(b"Image")));
}

#[test]
fn options_schema_and_deserialization_preserve_the_bounded_contract() {
    let schema = serde_json::to_value(schemars::schema_for!(ExportOptions)).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        schema["$defs"]["ExportFormat"]["enum"],
        json!(["png", "jpeg", "pdf"])
    );
    assert!(
        serde_json::from_value::<ExportOptions>(json!({"filename":"not-permitted.png"})).is_err()
    );
    let options: ExportOptions = serde_json::from_value(
        json!({"format":"jpeg","page_indices":[0],"transparent":true,"jpeg_matte":[10,20,30]}),
    )
    .unwrap();
    assert_eq!(options.scale, 1.0);
    assert_eq!(options.jpeg_matte, [10, 20, 30]);
}

#[test]
fn mixed_script_fallback_is_reported_even_when_requested_font_handles_latin() {
    let source = with_elements(
        json!([{"type":"text","id":"mixed","x":10,"y":10,"width":290,"height":100,"text":"Latin 日本語","font_size":24,"color":"000000","bold":false,"format":{"font_family":"Segoe UI"}}]),
    );
    let output = export_static(&source, &ExportOptions::default()).unwrap();
    assert!(
        output
            .warnings
            .iter()
            .any(|warning| warning.code == "FONT_FALLBACK"),
        "{:?}",
        output.warnings
    );
}

#[test]
fn sanitized_svg_renders_instead_of_stale_fallback_and_external_svg_is_rejected() {
    let fallback = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 255, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    fallback
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let safe = STANDARD.encode("<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><rect width='16' height='16' fill='red'/></svg>");
    let mut source = with_elements(
        json!([{"type":"picture","id":"svg","x":10,"y":10,"width":100,"height":100,
        "base64":STANDARD.encode(bytes.get_ref()),"mime_type":"image/png","alt":"SVG","svg":safe}]),
    );
    let output = export_static(&source, &ExportOptions::default()).unwrap();
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.code == "SVG_RASTERIZED"));
    let pixels = image::load_from_memory(&output.artifacts[0].bytes)
        .unwrap()
        .into_rgba8();
    assert_eq!(pixels.get_pixel(60, 60).0, [255, 0, 0, 255]);
    if let aislide_core::model::Element::Picture { svg, .. } = &mut source.slides[0].elements[0] {
        *svg = Some(STANDARD.encode("<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><image href='https://example.invalid/private'/></svg>"));
    }
    assert!(
        matches!(export_static(&source, &ExportOptions::default()), Err(Error::Unsupported(message)) if message.contains("SVG") || message.contains("external"))
    );
}
