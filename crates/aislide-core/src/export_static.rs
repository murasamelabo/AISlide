//! In-memory static export of the validated native scene. No files, Office,
//! browser, network, or source mutations. Scene pixels are 1/96 inch; scale
//! changes raster resolution, not PDF paper size. PDF uses vector paths,
//! including outlined text, and embedded raster pictures. Top-level objects
//! containing generated effects are rasterized at 2x with an explicit warning;
//! other objects remain vector. SVG pictures use the warned PNG fallback.
//! A positioned invisible Unicode layer provides search/selection and basic
//! structure tags; visible text remains outlined, not editable. Figures use
//! tagged nonpainting proxies; visual artwork is an Artifact. No font programs
//! are embedded. This is not PDF/A, PDF/UA or WCAG certification.
//!
//! Limits: 1-32 unique selected pages from a validated deck (up to 128 slides),
//! scale 0.01-16, output edges <=8192, combined
//! encoded artifacts <=32 MiB (caller may lower), cumulative RGBA <=128 MiB.
//! Working raster accounting reserves 64 MiB for scene/media/output and allows
//! three raster buffers in the remaining 64 MiB. These are allocation budgets,
//! not an OS RSS/deadline guarantee: installed-font caches and library overhead
//! are outside that accounting. Hosts needing hard isolation must supply it.
//! Transparent PNG includes RGBA; JPEG composites onto the explicit RGB matte.
//! Warnings may be escalated with deny_warnings; unsupported content always
//! rejects. Source validation, image validation and scene limits are not waived.
use crate::{
    model::{validate_deck, Deck, Element, Slide},
    render, Error, Result,
};
use image::{ExtendedColorType, ImageEncoder};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, io::Write};

pub const MAX_DIMENSION: u32 = 8192;
pub const MAX_RENDER_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_SVG_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PreviewLayout {
    #[default]
    Pages,
    ContactSheet,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PreviewFormat {
    #[default]
    Png,
    Jpeg,
}

impl PreviewFormat {
    fn image_format(self) -> image::ImageFormat {
        match self {
            Self::Png => image::ImageFormat::Png,
            Self::Jpeg => image::ImageFormat::Jpeg,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PreviewOverflow {
    #[default]
    Shrink,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PreviewOptions {
    pub page_indices: Option<Vec<usize>>,
    pub max_dimension: u32,
    pub layout: PreviewLayout,
    pub max_output_bytes: usize,
    pub format: PreviewFormat,
    pub overflow: PreviewOverflow,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self { page_indices: None, max_dimension: 1280, layout: PreviewLayout::Pages, max_output_bytes: 2 * 1024 * 1024,
            format: PreviewFormat::Png, overflow: PreviewOverflow::Shrink }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewImage {
    pub base64: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub byte_length: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewPage {
    pub page_index: usize,
    pub slide_id: String,
    pub image_index: usize,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresentationPreview {
    pub revision: u64,
    pub hash: String,
    pub requested_max_dimension: u32,
    pub actual_max_dimension: u32,
    pub quality_reduced: bool,
    pub pages: Vec<PreviewPage>,
    pub images: Vec<PreviewImage>,
    pub warnings: Vec<RenderWarning>,
    pub office_visual_parity: bool,
}

pub fn preview_presentation(document: &crate::document::Document, options: &PreviewOptions) -> Result<PresentationPreview> {
    crate::document::verify(document)?;
    let deck = &document.deck;
    let selected = options.page_indices.clone().unwrap_or_else(|| (0..deck.slides.len()).collect());
    if selected.is_empty() || selected.len() > 8 || selected.iter().any(|index| *index >= deck.slides.len())
        || selected.iter().collect::<BTreeSet<_>>().len() != selected.len()
    {
        return Err(Error::Invalid("preview requires 1-8 unique existing page indices; select a smaller batch".into()));
    }
    if !(160..=1600).contains(&options.max_dimension) || options.max_output_bytes == 0 || options.max_output_bytes > 2 * 1024 * 1024 {
        return Err(Error::Limit("preview requires 160-1600 pixels and at most 2 MiB encoded images".into()));
    }
    let mut attempted = Vec::new();
    for dimension in [options.max_dimension, (options.max_dimension * 3 / 4).max(160), (options.max_dimension * 9 / 16).max(160)] {
        if attempted.contains(&dimension) { continue; }
        attempted.push(dimension);
        match preview_attempt(document, options, &selected, dimension) {
            Ok(preview) => return Ok(preview),
            Err(StaticExportError::OutputBudget) => {
                if matches!(options.overflow, PreviewOverflow::Error) { break; }
            }
            Err(StaticExportError::Other(error)) => return Err(error),
        }
    }
    Err(Error::Limit(format!(
        "preview encoded output byte limit exceeded after {} attempt(s) at max_dimension {attempted:?}; select fewer pages, reduce max_dimension, or try format:'jpeg' (JPEG). These may reduce output size but do not guarantee it fits; no partial pages returned",
        attempted.len()
    )))
}

fn preview_attempt(
    document: &crate::document::Document, options: &PreviewOptions, selected: &[usize], max_dimension: u32,
) -> std::result::Result<PresentationPreview, StaticExportError> {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let deck = &document.deck;
    let montage = matches!(options.layout, PreviewLayout::ContactSheet);
    let columns = if montage { (selected.len() as f64).sqrt().ceil() as u32 } else { 1 };
    let rows = if montage { (selected.len() as u32).div_ceil(columns) } else { 1 };
    let gap = if montage { 12 } else { 0 };
    let cell_width = (max_dimension - gap * (columns + 1)) / columns;
    let cell_height = (max_dimension - gap * (rows + 1)) / rows;
    let scale = (f64::from(cell_width) / f64::from(deck.width)).min(f64::from(cell_height) / f64::from(deck.height));
    let output = export_static_inner(deck, &ExportOptions {
        format: match options.format { PreviewFormat::Png => ExportFormat::Png, PreviewFormat::Jpeg => ExportFormat::Jpeg },
        page_indices: Some(selected.to_vec()), scale: scale.max(0.01), max_output_bytes: options.max_output_bytes, ..Default::default()
    })?;
    let mut pages = Vec::new();
    let mut artifacts = output.artifacts;
    let mut remaining = options.max_output_bytes;
    for artifact in &mut artifacts {
        if artifact.width > cell_width || artifact.height > cell_height {
            let pixels = image::load_from_memory_with_format(&artifact.bytes, options.format.image_format())
                .map_err(|error| Error::Invalid(format!("preview image: {error}")))?.into_rgba8();
            artifact.width = (f64::from(deck.width) * scale).floor().max(1.0) as u32;
            artifact.height = (f64::from(deck.height) * scale).floor().max(1.0) as u32;
            let resized = image::imageops::resize(&pixels, artifact.width, artifact.height, image::imageops::FilterType::Lanczos3);
            artifact.bytes = encode_preview_image(&resized, options.format, remaining)?;
        }
        remaining = remaining.checked_sub(artifact.bytes.len()).ok_or(StaticExportError::OutputBudget)?;
    }
    for (index, artifact) in artifacts.iter().enumerate() {
        let page_index = artifact.page_indices[0];
        pages.push(PreviewPage {
            page_index, slide_id: deck.slides[page_index].id.clone(), image_index: if montage { 0 } else { index },
            x: if montage { gap + index as u32 % columns * (artifact.width + gap) } else { 0 },
            y: if montage { gap + index as u32 / columns * (artifact.height + gap) } else { 0 },
            width: artifact.width, height: artifact.height,
        });
    }
    if montage {
        let width = artifacts[0].width * columns + gap * (columns + 1);
        let height = artifacts[0].height * rows + gap * (rows + 1);
        let mut sheet = image::RgbaImage::from_pixel(width, height, image::Rgba([238, 238, 238, 255]));
        for (artifact, page) in artifacts.iter().zip(&pages) {
            let pixels = image::load_from_memory_with_format(&artifact.bytes, options.format.image_format())
                .map_err(|error| Error::Invalid(format!("preview image: {error}")))?.into_rgba8();
            image::imageops::replace(&mut sheet, &pixels, i64::from(page.x), i64::from(page.y));
        }
        let bytes = encode_preview_image(&sheet, options.format, options.max_output_bytes)?;
        artifacts = vec![ExportArtifact { bytes, mime_type: artifacts[0].mime_type.clone(), page_indices: pages.iter().map(|page| page.page_index).collect(), width, height }];
    }
    let images = artifacts.into_iter().map(|artifact| PreviewImage {
        sha256: format!("{:x}", Sha256::digest(&artifact.bytes)), byte_length: artifact.bytes.len(),
        base64: base64::engine::general_purpose::STANDARD.encode(artifact.bytes),
        mime_type: artifact.mime_type, width: artifact.width, height: artifact.height,
    }).collect();
    let mut warnings = output.warnings;
    if document.bindings.iter().any(|binding| binding.stale) {
        warnings.push(RenderWarning { code: "SOURCE_BINDINGS_STALE".into(), page_index: pages[0].page_index, element_id: String::new(), message: "Preview includes stale source bindings; undo or rebind before export".into() });
    }
    let quality_reduced = max_dimension < options.max_dimension;
    if quality_reduced {
        warnings.push(RenderWarning { code: "PREVIEW_DOWNSCALED".into(), page_index: pages[0].page_index, element_id: String::new(),
            message: format!("Preview max_dimension reduced from requested {} to actual {max_dimension} pixels to fit the encoded image budget; all {} selected pages are preserved in their requested order", options.max_dimension, pages.len()) });
    }
    let result = PresentationPreview { revision: document.revision, hash: document.hash.clone(),
        requested_max_dimension: options.max_dimension, actual_max_dimension: max_dimension, quality_reduced,
        pages, images, warnings, office_visual_parity: false };
    if serde_json::to_vec(&result).map_err(Error::from)?.len() > 4 * 1024 * 1024 - 65536 {
        return Err(Error::Limit("preview response exceeds 4 MiB; select fewer pages, reduce max_dimension, or try format:'jpeg' without a guarantee of fitting".into()).into());
    }
    Ok(result)
}

fn encode_preview_image(
    pixels: &image::RgbaImage, format: PreviewFormat, limit: usize,
) -> std::result::Result<Vec<u8>, StaticExportError> {
    let mut output = BoundedBytes::new(limit);
    let encoded = match format {
        PreviewFormat::Png => image::codecs::png::PngEncoder::new(&mut output)
            .write_image(pixels.as_raw(), pixels.width(), pixels.height(), ExtendedColorType::Rgba8),
        PreviewFormat::Jpeg => {
            let rgb: Vec<u8> = pixels.pixels().flat_map(|pixel| [pixel[0], pixel[1], pixel[2]]).collect();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, ExportOptions::default().jpeg_quality)
                .write_image(&rgb, pixels.width(), pixels.height(), ExtendedColorType::Rgb8)
        }
    };
    output.check_encoding(encoded, "preview image encoding")?;
    Ok(output.bytes)
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    #[default]
    Png,
    Jpeg,
    Pdf,
}

/// Zero-based page selection in output order; None selects all slides. Empty
/// selections and duplicates reject. Transparency omits only page background;
/// JPEG always composites against jpeg_matte. Limits may only be lowered.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub page_indices: Option<Vec<usize>>,
    pub scale: f64,
    pub transparent: bool,
    pub jpeg_quality: u8,
    pub jpeg_matte: [u8; 3],
    pub max_output_bytes: usize,
    pub deny_warnings: bool,
}
impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Png,
            page_indices: None,
            scale: 1.0,
            transparent: false,
            jpeg_quality: 90,
            jpeg_matte: [255; 3],
            max_output_bytes: MAX_OUTPUT_BYTES,
            deny_warnings: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RenderWarning {
    pub code: String,
    pub page_index: usize,
    pub element_id: String,
    pub message: String,
}

#[derive(Debug)]
/// One image per selected page, or one multipage PDF. width/height are the
/// requested preview pixel dimensions, also for PDF. PDF MediaBox dimensions
/// are always deck.width * 0.75 and deck.height * 0.75 points, independent of scale.
pub struct ExportArtifact {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub page_indices: Vec<usize>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub struct StaticExport {
    pub artifacts: Vec<ExportArtifact>,
    pub warnings: Vec<RenderWarning>,
    pub office_parity_verified: bool,
    /// False means pages were not flattened. Individual pictures (including
    /// sanitized SVG assets rasterized by core) and warned effect-bearing
    /// top-level objects can still be embedded rasters.
    pub pdf_rasterized: bool,
}

pub(crate) struct BoundedBytes {
    pub bytes: Vec<u8>,
    pub limit: usize,
    exceeded: bool,
}

enum StaticExportError {
    OutputBudget,
    Other(Error),
}

impl From<Error> for StaticExportError {
    fn from(error: Error) -> Self { Self::Other(error) }
}

impl StaticExportError {
    fn into_error(self) -> Error {
        match self {
            Self::OutputBudget => Error::Limit("static image encoding: static export output byte limit".into()),
            Self::Other(error) => error,
        }
    }
}

impl BoundedBytes {
    fn new(limit: usize) -> Self { Self { bytes: Vec::new(), limit, exceeded: false } }

    fn check_encoding(&self, encoded: image::ImageResult<()>, context: &str) -> std::result::Result<(), StaticExportError> {
        encoded.map_err(|error| {
            if self.exceeded { StaticExportError::OutputBudget }
            else { Error::Limit(format!("{context}: {error}")).into() }
        })
    }
}
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(std::io::Error::other("static export output byte limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn preflight(deck: &Deck, options: &ExportOptions) -> Result<(Vec<usize>, u32, u32)> {
    if !options.scale.is_finite() || !(0.01..=16.0).contains(&options.scale) {
        return Err(Error::Limit(
            "static export scale must be finite in 0.01..=16".into(),
        ));
    }
    if options.max_output_bytes == 0
        || options.max_output_bytes > MAX_OUTPUT_BYTES
        || !(1..=100).contains(&options.jpeg_quality)
    {
        return Err(Error::Limit(
            "static export output cap or JPEG quality out of range".into(),
        ));
    }
    let width = (f64::from(deck.width) * options.scale).ceil();
    let height = (f64::from(deck.height) * options.scale).ceil();
    if width < 1.0
        || height < 1.0
        || width > f64::from(MAX_DIMENSION)
        || height > f64::from(MAX_DIMENSION)
    {
        return Err(Error::Limit(
            "static export dimensions must be 1..=8192".into(),
        ));
    }
    let selected_count = options.page_indices.as_ref().map_or(deck.slides.len(), Vec::len);
    if selected_count == 0 || selected_count > 32 {
        return Err(Error::Limit("static export requires 1-32 pages".into()));
    }
    let pages = options
        .page_indices
        .clone()
        .unwrap_or_else(|| (0..deck.slides.len()).collect());
    if pages.is_empty()
        || pages.len() > 32
        || pages.iter().any(|index| *index >= deck.slides.len())
        || pages.iter().collect::<BTreeSet<_>>().len() != pages.len()
    {
        return Err(Error::Invalid(
            "static export requires 1-32 unique existing page indices".into(),
        ));
    }
    let raw = width as u64 * height as u64 * 4;
    if raw * 3 + 64 * 1024 * 1024 > MAX_RENDER_BYTES as u64
        || raw * pages.len() as u64 > MAX_RENDER_BYTES as u64
    {
        return Err(Error::Limit(
            "static export working/cumulative raster budget exceeds 128 MiB".into(),
        ));
    }
    validate_deck(deck)?;
    Ok((pages, width as u32, height as u32))
}

/// All-or-error: no partial artifact list escapes on unsupported content,
/// validation failure, or budget exhaustion. Caller owns publication/printing.
pub fn export_static(deck: &Deck, options: &ExportOptions) -> Result<StaticExport> {
    export_static_inner(deck, options).map_err(StaticExportError::into_error)
}

fn export_static_inner(deck: &Deck, options: &ExportOptions) -> std::result::Result<StaticExport, StaticExportError> {
    let (pages, width, height) = preflight(deck, options)?;
    if options.format == ExportFormat::Pdf {
        return export_pdf(deck, options, pages, width, height).map_err(StaticExportError::from);
    }
    let mut result = StaticExport {
        artifacts: Vec::new(),
        warnings: Vec::new(),
        office_parity_verified: false,
        pdf_rasterized: false,
    };
    let mut remaining = options.max_output_bytes;
    for page_index in pages {
        let scene = render::render_at_scale(deck, page_index, options.transparent, options.scale)?;
        if options.deny_warnings && !scene.warnings.is_empty() {
            return Err(Error::Unsupported(
                "static export has render warnings".into(),
            ).into());
        }
        result.warnings.extend(scene.warnings);
        let tree = resvg::usvg::Tree::from_str(&scene.svg, &resvg::usvg::Options::default())
            .map_err(|error| Error::Invalid(format!("internal render SVG: {error}")))?;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
            .ok_or_else(|| Error::Limit("static raster allocation failed".into()))?;
        if options.format == ExportFormat::Jpeg {
            pixmap.fill(resvg::tiny_skia::Color::from_rgba8(
                options.jpeg_matte[0],
                options.jpeg_matte[1],
                options.jpeg_matte[2],
                255,
            ));
        }
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(
                width as f32 / deck.width as f32,
                height as f32 / deck.height as f32,
            ),
            &mut pixmap.as_mut(),
        );
        let mut output = BoundedBytes::new(remaining);
        let encoded = if options.format == ExportFormat::Png {
            let pixels: Vec<u8> = pixmap
                .pixels()
                .iter()
                .flat_map(|pixel| {
                    let color = pixel.demultiply();
                    [color.red(), color.green(), color.blue(), color.alpha()]
                })
                .collect();
            image::codecs::png::PngEncoder::new(&mut output).write_image(
                &pixels,
                width,
                height,
                ExtendedColorType::Rgba8,
            )
        } else {
            let pixels: Vec<u8> = pixmap
                .pixels()
                .iter()
                .flat_map(|pixel| [pixel.red(), pixel.green(), pixel.blue()])
                .collect();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, options.jpeg_quality)
                .write_image(&pixels, width, height, ExtendedColorType::Rgb8)
        };
        output.check_encoding(encoded, "static image encoding")?;
        remaining -= output.bytes.len();
        result.artifacts.push(ExportArtifact {
            bytes: output.bytes,
            mime_type: if options.format == ExportFormat::Png {
                "image/png"
            } else {
                "image/jpeg"
            }
            .into(),
            page_indices: vec![page_index],
            width,
            height,
        });
    }
    Ok(result)
}

fn export_pdf(
    deck: &Deck,
    options: &ExportOptions,
    pages: Vec<usize>,
    width: u32,
    height: u32,
) -> Result<StaticExport> {
    use lopdf::{dictionary, Document, Object};
    let mut document = Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let structure_id = document.new_object_id();
    let logical_id = document.new_object_id();
    let mut sections = Vec::new();
    let mut parent_numbers = Vec::new();
    let mut children = Vec::new();
    let mut warnings = Vec::new();
    let mut total_svg = 0usize;
    let mut intermediate_bytes = 0usize;
    let mut semantic_bytes = 0usize;
    for page_index in &pages {
        let scene = render::render_at_scale(deck, *page_index, options.transparent, 2.0)?;
        let text_tree = svg2pdf::usvg::Tree::from_str(&scene.svg, &svg2pdf::usvg::Options::default())
            .map_err(|error| Error::Invalid(format!("internal text SVG: {error}")))?;
        semantic_bytes += scene.text.iter().map(|span| 512 + span.glyphs.iter().map(|glyph| 128 + glyph.text.len() * 4).sum::<usize>()).sum::<usize>();
        if semantic_bytes > MAX_SVG_BYTES {
            return Err(Error::Limit("PDF semantic text working budget exceeds 8 MiB".into()));
        }
        if options.deny_warnings && !scene.warnings.is_empty() {
            return Err(Error::Unsupported(
                "static export has render warnings".into(),
            ));
        }
        total_svg += scene.svg.len();
        if total_svg > MAX_OUTPUT_BYTES {
            return Err(Error::Limit("PDF cumulative SVG exceeds 32 MiB".into()));
        }
        warnings.extend(scene.warnings);
        let (svg, effect_count) = pdf_effects(&scene.svg)?;
        if effect_count > 0 {
            warnings.push(RenderWarning { code: "PDF_EFFECT_RASTERIZED".into(), page_index: *page_index, element_id: String::new(), message: format!("{effect_count} top-level effect objects rendered by resvg at 2x; other page objects remain vector") });
        }
        let tree = svg2pdf::usvg::Tree::from_str(&svg, &svg2pdf::usvg::Options::default())
            .map_err(|error| Error::Invalid(format!("internal PDF SVG: {error}")))?;
        let bytes = svg2pdf::to_pdf(
            &tree,
            svg2pdf::ConversionOptions {
                embed_text: false,
                ..Default::default()
            },
            svg2pdf::PageOptions { dpi: 96.0 },
        )
        .map_err(|error| Error::Unsupported(format!("static SVG to PDF: {error}")))?;
        intermediate_bytes += bytes.len();
        if intermediate_bytes > options.max_output_bytes {
            return Err(Error::Limit("PDF intermediate output byte cap".into()));
        }
        let mut page_document = Document::load_mem(&bytes)
            .map_err(|error| Error::Invalid(format!("internal generated PDF: {error}")))?;
        page_document.renumber_objects_with(document.max_id + 1);
        document.max_id = page_document.max_id;
        let page_id = *page_document
            .get_pages()
            .values()
            .next()
            .ok_or_else(|| Error::Invalid("PDF converter returned no page".into()))?;
        page_document
            .get_object_mut(page_id)
            .and_then(Object::as_dict_mut)
            .map_err(|error| Error::Invalid(format!("PDF page dictionary: {error}")))?
            .set("Parent", pages_id);
        for (object_id, object) in page_document.objects {
            let kind = object
                .as_dict()
                .ok()
                .and_then(|dictionary| dictionary.get(b"Type").ok())
                .and_then(|kind| kind.as_name().ok());
            if matches!(kind, Some(b"Catalog" | b"Pages")) {
                continue;
            }
            document.objects.insert(object_id, object);
        }
        let section_id = document.new_object_id();
        let mut content = Vec::new();
        let slide = &deck.slides[*page_index];
        let ids: BTreeSet<_> = crate::model::element_list(&slide.elements).into_iter().map(|element| element.bounds().0.to_owned()).collect();
        let mut tags = Vec::new();
        for span in scene.text.iter().filter(|span| !ids.iter().any(|id| span.element_id == *id || span.element_id.starts_with(&format!("{id}[")))) {
            let tag = pdf_tag(&mut document, section_id, "P");
            content.push(PdfContent { tag, role: "P", span: Some(span), bounds: None });
            tags.push(Object::Reference(tag));
        }
        let ordered: Vec<_> = if let Some(review) = slide.review.as_ref().filter(|review| !review.reading_order.is_empty()) {
            review.reading_order.iter().filter_map(|id| slide.elements.iter().find(|element| element.bounds().0 == id)).collect()
        } else { slide.elements.iter().collect() };
        for element in ordered {
            if let Some(tag) = pdf_element_structure(&mut document, section_id, element, slide, &scene.text, &mut content)? {
                tags.push(Object::Reference(tag));
            }
        }
        document.objects.insert(section_id, Object::Dictionary(dictionary! {"Type"=>"StructElem", "S"=>"Sect", "P"=>logical_id, "Pg"=>page_id, "T"=>pdf_unicode(&slide.title), "K"=>tags}));
        let parent_index = sections.len() as i64;
        let parents = pdf_text(&mut document, page_id, &content, &text_tree, &scene.objects, deck.height as f64)?;
        document.get_object_mut(page_id).and_then(Object::as_dict_mut).map_err(pdf_error)?.set("StructParents", parent_index);
        document.get_object_mut(page_id).and_then(Object::as_dict_mut).map_err(pdf_error)?.set("Tabs", Object::Name(b"S".to_vec()));
        sections.push(Object::Reference(section_id));
        parent_numbers.extend([Object::Integer(parent_index), Object::Array(parents)]);
        children.push(Object::Reference(page_id));
    }
    document.objects.insert(
        pages_id,
        Object::Dictionary(
            dictionary! {"Type"=>"Pages","Count"=>children.len() as i64,"Kids"=>children},
        ),
    );
    let parent_tree = document.add_object(dictionary! {"Nums"=>parent_numbers});
    document.objects.insert(logical_id, Object::Dictionary(dictionary! {"Type"=>"StructElem", "S"=>"Document", "P"=>structure_id, "K"=>sections}));
    document.objects.insert(structure_id, Object::Dictionary(dictionary! {"Type"=>"StructTreeRoot", "K"=>vec![Object::Reference(logical_id)], "ParentTree"=>parent_tree, "ParentTreeNextKey"=>pages.len() as i64}));
    document.objects.insert(
        catalog_id,
        Object::Dictionary(dictionary! {"Type"=>"Catalog","Pages"=>pages_id,"StructTreeRoot"=>structure_id,"MarkInfo"=>dictionary! {"Marked"=>true}}),
    );
    document.trailer.set("Root", catalog_id);
    let mut output = BoundedBytes::new(options.max_output_bytes);
    document
        .save_to(&mut output)
        .map_err(|error| Error::Limit(format!("PDF output byte cap: {error}")))?;
    Ok(StaticExport {
        artifacts: vec![ExportArtifact {
            bytes: output.bytes,
            mime_type: "application/pdf".into(),
            page_indices: pages,
            width,
            height,
        }],
        warnings,
        office_parity_verified: false,
        pdf_rasterized: false,
    })
}

fn pdf_error(error: lopdf::Error) -> Error {
    Error::Invalid(format!("PDF semantic structure: {error}"))
}

fn pdf_unicode(value: &str) -> lopdf::Object {
    lopdf::Object::String([vec![0xfe, 0xff], value.encode_utf16().flat_map(u16::to_be_bytes).collect()].concat(), lopdf::StringFormat::Hexadecimal)
}

struct PdfContent<'a> {
    tag: lopdf::ObjectId,
    role: &'static str,
    span: Option<&'a render::RenderedText>,
    bounds: Option<(String, [f64; 4])>,
}

fn pdf_tag(document: &mut lopdf::Document, parent: lopdf::ObjectId, role: &str) -> lopdf::ObjectId {
    use lopdf::dictionary;
    document.add_object(dictionary! {"Type"=>"StructElem", "S"=>lopdf::Object::Name(role.as_bytes().to_vec()), "P"=>parent, "K"=>Vec::<lopdf::Object>::new()})
}

fn pdf_element_structure<'a>(
    document: &mut lopdf::Document,
    parent: lopdf::ObjectId,
    element: &Element,
    slide: &Slide,
    spans: &'a [render::RenderedText],
    content: &mut Vec<PdfContent<'a>>,
) -> Result<Option<lopdf::ObjectId>> {
    use lopdf::{dictionary, Object};
    let (id, x, y, width, height) = element.bounds();
    let metadata = slide.review.as_ref().and_then(|review| review.accessibility.get(id));
    if element.visual().is_some_and(|visual| visual.hidden) || metadata.is_some_and(|value| value.decorative == Some(true)) { return Ok(None); }
    let tag = match element {
        Element::Group { children, .. } => {
            let tag = pdf_tag(document, parent, "Sect");
            let mut tags = Vec::new();
            for child in children {
                if let Some(child) = pdf_element_structure(document, tag, child, slide, spans, content)? { tags.push(Object::Reference(child)); }
            }
            document.get_object_mut(tag).and_then(Object::as_dict_mut).map_err(pdf_error)?.set("K", tags);
            tag
        }
        Element::Table { rows, format, .. } => {
            use crate::review::TableHeaders;
            let policy = slide.review.as_ref().and_then(|review| review.table_headers.get(id)).copied().unwrap_or_default();
            let table = pdf_tag(document, parent, "Table");
            let mut row_tags = Vec::new();
            for (row_index, row) in rows.iter().enumerate() {
                let row_tag = pdf_tag(document, table, "TR");
                row_tags.push(Object::Reference(row_tag));
                let mut cells = Vec::new();
                for (column_index, _) in row.iter().enumerate() {
                    let merge = format.merge_at(row_index, column_index);
                    if merge.is_some_and(|merge| (merge.row, merge.column) != (row_index, column_index)) { continue; }
                    let column_header = row_index == 0 && matches!(policy, TableHeaders::FirstRow | TableHeaders::Both);
                    let row_header = column_index == 0 && matches!(policy, TableHeaders::FirstColumn | TableHeaders::Both);
                    let role = if column_header || row_header { "TH" } else { "TD" };
                    let cell = pdf_tag(document, row_tag, role);
                    let mut attributes = dictionary! {"O"=>"Table", "RowSpan"=>merge.map_or(1, |merge| merge.row_span) as i64, "ColSpan"=>merge.map_or(1, |merge| merge.col_span) as i64};
                    if column_header || row_header {
                        let scope = if column_header && row_header { "Both" } else if column_header { "Column" } else { "Row" };
                        attributes.set("Scope", Object::Name(scope.as_bytes().to_vec()));
                    }
                    document.get_object_mut(cell).and_then(Object::as_dict_mut).map_err(pdf_error)?.set("A", attributes);
                    if let Some(span) = spans.iter().find(|span| span.element_id == format!("{id}[{row_index},{column_index}]")) {
                        content.push(PdfContent { tag: cell, role, span: Some(span), bounds: None });
                    }
                    cells.push(Object::Reference(cell));
                }
                document.get_object_mut(row_tag).and_then(Object::as_dict_mut).map_err(pdf_error)?.set("K", cells);
            }
            document.get_object_mut(table).and_then(Object::as_dict_mut).map_err(pdf_error)?.set("K", row_tags);
            table
        }
        _ => {
            let text: Vec<_> = spans.iter().filter(|span| span.element_id == id).collect();
            let description = metadata.and_then(|value| value.description.as_deref().or(value.title.as_deref()))
                .or_else(|| match element { Element::Picture { alt, .. } => Some(alt.as_str()), _ => None });
            if !text.is_empty() {
                let tag = pdf_tag(document, parent, if matches!(element, Element::Chart { .. }) { "Figure" } else { "Div" });
                if let Some(description) = description.filter(|description| !description.is_empty()) {
                    document.get_object_mut(tag).and_then(Object::as_dict_mut).map_err(pdf_error)?.set("Alt", pdf_unicode(description));
                }
                let mut paragraphs = Vec::new();
                for span in text {
                    let paragraph = pdf_tag(document, tag, "P");
                    paragraphs.push(Object::Reference(paragraph));
                    content.push(PdfContent { tag: paragraph, role: "P", span: Some(span), bounds: None });
                }
                document.get_object_mut(tag).and_then(Object::as_dict_mut).map_err(pdf_error)?.set("K", paragraphs);
                tag
            } else if description.is_some() || matches!(element, Element::Picture { .. } | Element::Chart { .. }) {
                let tag = pdf_tag(document, parent, "Figure");
                if let Some(description) = description.filter(|description| !description.is_empty()) {
                    document.get_object_mut(tag).and_then(Object::as_dict_mut).map_err(pdf_error)?.set("Alt", pdf_unicode(description));
                }
                content.push(PdfContent { tag, role: "Figure", span: None, bounds: Some((id.into(), [x, y, width, height])) });
                tag
            } else { return Ok(None); }
        }
    };
    if let Some(title) = metadata.and_then(|value| value.title.as_deref()) {
        document.get_object_mut(tag).and_then(Object::as_dict_mut).map_err(pdf_error)?.set("T", pdf_unicode(title));
    }
    Ok(Some(tag))
}

fn pdf_text(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    content: &[PdfContent<'_>],
    tree: &svg2pdf::usvg::Tree,
    objects: &[(String, String)],
    height: f64,
) -> Result<Vec<lopdf::Object>> {
    use lopdf::{content::{Content, Operation}, dictionary, Object, Stream, StringFormat};
    let failure = |error| Error::Invalid(format!("PDF semantic text: {error}"));
    let mut fonts = lopdf::Dictionary::new();
    let mut operations = vec![Operation::new("q", vec![])];
    let mut parents = Vec::new();
    for item in content {
        let mcid = parents.len() as i64;
        parents.push(Object::Reference(item.tag));
        let tag = document.get_object_mut(item.tag).and_then(Object::as_dict_mut).map_err(failure)?;
        tag.set("Pg", page_id);
        tag.set("K", mcid);
        operations.push(Operation::new("BDC", vec![Object::Name(item.role.as_bytes().to_vec()), Object::Dictionary(dictionary! {"MCID"=>mcid})]));
        let Some(span) = item.span else {
            if let Some((element_id, fallback)) = &item.bounds {
                let [x, y, width, box_height] = objects.iter().rev().find(|(id, _)| id == element_id)
                    .and_then(|(_, id)| tree.node_by_id(id)).and_then(|node| node.abs_layer_bounding_box())
                    .map(|bounds| [bounds.x() as f64, bounds.y() as f64, bounds.width() as f64, bounds.height() as f64]).unwrap_or(*fallback);
                let bounds: Vec<Object> = vec![(x * 0.75).into(), ((height - y - box_height) * 0.75).into(), ((x + width) * 0.75).into(), ((height - y) * 0.75).into()];
                document.get_object_mut(item.tag).and_then(Object::as_dict_mut).map_err(failure)?.set("A", dictionary! {"O"=>"Layout", "BBox"=>bounds});
                operations.push(Operation::new("re", vec![(x * 0.75).into(), ((height - y - box_height) * 0.75).into(), (width * 0.75).into(), (box_height * 0.75).into()]));
                operations.push(Operation::new("n", vec![]));
            }
            operations.push(Operation::new("EMC", vec![]));
            continue;
        };
        if span.glyphs.is_empty() {
            operations.push(Operation::new("EMC", vec![]));
            continue;
        }
        let node = tree.node_by_id(&span.svg_id).ok_or_else(|| Error::Invalid(format!("PDF text group missing: {} ({})", span.svg_id, span.element_id)))?;
        let transform = node.abs_transform();
        for chunk in span.glyphs.chunks(255) {
            let mut cmap = String::from("/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /AISlideUnicode def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange\n");
            let mut widths = Vec::new();
            for batch in chunk.chunks(100).enumerate() {
                cmap.push_str(&format!("{} beginbfchar\n", batch.1.len()));
                for (offset, glyph) in batch.1.iter().enumerate() {
                    let code = batch.0 * 100 + offset + 1;
                    let unicode = glyph.text.encode_utf16().map(|unit| format!("{unit:04X}")).collect::<String>();
                    cmap.push_str(&format!("<{code:04X}> <{unicode}>\n"));
                    let advance = (glyph.width / glyph.size * 1000.0) as f32;
                    widths.push(Object::Real(advance));
                }
                cmap.push_str("endbfchar\n");
            }
            cmap.push_str("endcmap CMapName currentdict /CMap defineresource pop end end\n");
            let unicode_id = document.add_object(Stream::new(dictionary! {}, cmap.into_bytes()));
            let descriptor = document.add_object(dictionary! {
                "Type"=>"FontDescriptor", "FontName"=>"AISlideSemantic", "Flags"=>4,
                "FontBBox"=>vec![0.into(), (-250).into(), 2000.into(), 1000.into()],
                "ItalicAngle"=>0, "Ascent"=>1000, "Descent"=>-250, "CapHeight"=>750, "StemV"=>80,
            });
            let descendant = document.add_object(dictionary! {
                "Type"=>"Font", "Subtype"=>"CIDFontType2", "BaseFont"=>"AISlideSemantic",
                "CIDSystemInfo"=>dictionary! {"Registry"=>Object::string_literal("Adobe"), "Ordering"=>Object::string_literal("Identity"), "Supplement"=>0},
                "FontDescriptor"=>descriptor, "CIDToGIDMap"=>"Identity", "DW"=>1000,
                "W"=>vec![Object::Integer(1), Object::Array(widths)],
            });
            let font_id = document.add_object(dictionary! {
                "Type"=>"Font", "Subtype"=>"Type0", "BaseFont"=>"AISlideSemantic",
                "Encoding"=>"Identity-H", "DescendantFonts"=>vec![Object::Reference(descendant)], "ToUnicode"=>unicode_id,
            });
            let name = format!("ASem{}", fonts.len());
            fonts.set(name.clone(), font_id);
            operations.push(Operation::new("BT", vec![]));
            operations.push(Operation::new("Tr", vec![3.into()]));
            for (index, glyph) in chunk.iter().enumerate() {
                let mut point = svg2pdf::usvg::tiny_skia_path::Point::from_xy(glyph.origin[0] as f32, glyph.origin[1] as f32);
                transform.map_point(&mut point);
                operations.push(Operation::new("Tf", vec![Object::Name(name.as_bytes().to_vec()), (glyph.size * 0.75).into()]));
                operations.push(Operation::new("Tm", vec![transform.sx.into(), (-transform.ky).into(), (-transform.kx).into(), transform.sy.into(), (point.x as f64 * 0.75).into(), ((height - point.y as f64) * 0.75).into()]));
                operations.push(Operation::new("Tj", vec![Object::String(((index + 1) as u16).to_be_bytes().to_vec(), StringFormat::Hexadecimal)]));
            }
            operations.push(Operation::new("ET", vec![]));
        }
        operations.push(Operation::new("EMC", vec![]));
    }
    operations.push(Operation::new("Q", vec![]));
    let resources = document.get_object(page_id).and_then(Object::as_dict).and_then(|page| page.get(b"Resources")).map_err(failure)?.clone();
    let mut resources = match resources { Object::Reference(id) => document.get_object(id).and_then(Object::as_dict).map_err(failure)?.clone(), Object::Dictionary(value) => value, _ => return Err(Error::Invalid("PDF resources dictionary".into())) };
    resources.set("Font", fonts);
    document.get_object_mut(page_id).and_then(Object::as_dict_mut).map_err(failure)?.set("Resources", resources);
    let original = document.get_object(page_id).and_then(Object::as_dict).and_then(|page| page.get(b"Contents")).map_err(failure)?.clone();
    let prefix = document.add_object(Stream::new(dictionary! {}, b"q\n/Artifact BMC\n".to_vec()));
    let suffix = document.add_object(Stream::new(dictionary! {}, b"EMC\nQ\n".to_vec()));
    let mut streams = vec![Object::Reference(prefix)];
    match original { Object::Array(items) => streams.extend(items), item => streams.push(item) }
    streams.push(Object::Reference(suffix));
    document.get_object_mut(page_id).and_then(Object::as_dict_mut).map_err(failure)?.set("Contents", streams);
    let bytes = Content { operations }.encode().map_err(failure)?;
    document.add_page_contents(page_id, bytes).map_err(failure)?;
    Ok(parents)
}

fn pdf_effects(svg: &str) -> Result<(String, usize)> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let parsed = roxmltree::Document::parse(svg)?;
    let effects: Vec<_> = parsed.root_element().children().filter(|node| node.is_element()
        && node.descendants().any(|child| child.attribute("filter").is_some())).collect();
    if effects.is_empty() { return Ok((svg.into(), 0)); }
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .map_err(|error| Error::Invalid(format!("internal effect SVG: {error}")))?;
    let mut bytes = 0u64;
    let mut prepared = Vec::new();
    for element in &effects {
        let node = element.attribute("id").and_then(|id| tree.node_by_id(id))
            .ok_or_else(|| Error::Unsupported("generated effect object could not be resolved".into()))?;
        let bounds = node.abs_layer_bounding_box().ok_or_else(|| Error::Unsupported("effect object has no renderable bounds".into()))?;
        let width = (bounds.width() * 2.0).ceil() as u32;
        let height = (bounds.height() * 2.0).ceil() as u32;
        bytes = bytes.saturating_add(u64::from(width) * u64::from(height) * 4 * 8);
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION || bytes > 32 * 1024 * 1024 {
            return Err(Error::Limit("PDF effect raster working budget".into()));
        }
        prepared.push((element.range(), node, bounds, width, height));
    }
    let mut replacements = Vec::new();
    let mut encoded_bytes = 0;
    for (range, node, bounds, width, height) in prepared {
        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).ok_or_else(|| Error::Limit("PDF effect allocation".into()))?;
        resvg::render_node(node, resvg::tiny_skia::Transform::from_scale(2.0, 2.0), &mut pixmap.as_mut())
            .ok_or_else(|| Error::Unsupported("PDF effect rendering failed".into()))?;
        let pixels: Vec<u8> = pixmap.pixels().iter().flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        }).collect();
        let mut output = BoundedBytes::new(MAX_SVG_BYTES.saturating_sub(encoded_bytes) / 2);
        image::codecs::png::PngEncoder::new(&mut output).write_image(&pixels, width, height, ExtendedColorType::Rgba8)
            .map_err(|error| Error::Limit(format!("PDF effect encoding: {error}")))?;
        let image = format!("<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" xlink:href=\"data:image/png;base64,{}\"/>", bounds.x(), bounds.y(), width as f32 / 2.0, height as f32 / 2.0, STANDARD.encode(output.bytes));
        encoded_bytes += image.len();
        if encoded_bytes + svg.len() > MAX_SVG_BYTES { return Err(Error::Limit("PDF effect SVG byte budget".into())); }
        replacements.push((range, image));
    }
    let mut result = String::with_capacity(svg.len() + encoded_bytes);
    let mut cursor = 0;
    for (range, image) in replacements {
        result.push_str(&svg[cursor..range.start]);
        result.push_str(&image);
        cursor = range.end;
    }
    result.push_str(&svg[cursor..]);
    Ok((result, effects.len()))
}
