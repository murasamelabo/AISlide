use crate::{document::{self, Document}, export_static::{self, ExportFormat, ExportOptions, PreviewLayout, PreviewOptions}, Error, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const MAX_DELIVERY_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryPreview { None, Pages, #[default] ContactSheet }

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct DeliveryOptions {
    pub page_indices: Option<Vec<usize>>,
    pub pdf: bool,
    pub preview: DeliveryPreview,
    pub notes: bool,
    pub source_report: bool,
    pub preflight: bool,
    pub max_dimension: u32,
    pub min_font_size: f64,
    pub max_output_bytes: usize,
}

impl Default for DeliveryOptions {
    fn default() -> Self {
        Self { page_indices: None, pdf: false, preview: DeliveryPreview::ContactSheet, notes: false, source_report: false, preflight: true, max_dimension: 1280, min_font_size: 16.0, max_output_bytes: MAX_DELIVERY_BYTES }
    }
}

#[derive(Serialize)]
pub struct DeliveryFile {
    pub kind: String,
    pub suffix: String,
    pub mime_type: String,
    pub page_indices: Vec<usize>,
    pub byte_length: usize,
    pub sha256: String,
    pub base64: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")] pub height: Option<u32>,
}

#[derive(Serialize)]
pub struct PreparedDelivery {
    pub revision: u64,
    pub hash: String,
    pub files: Vec<DeliveryFile>,
    pub manifest: Value,
}

struct Builder { files: Vec<DeliveryFile>, remaining: usize }

impl Builder {
    fn add(&mut self, kind: &str, suffix: String, mime_type: &str, pages: Vec<usize>, bytes: Vec<u8>, dimensions: Option<(u32, u32)>) -> Result<()> {
        self.remaining = self.remaining.checked_sub(bytes.len()).ok_or_else(|| Error::Limit("delivery exceeds combined output byte budget".into()))?;
        self.files.push(DeliveryFile { kind: kind.into(), suffix, mime_type: mime_type.into(), page_indices: pages,
            byte_length: bytes.len(), sha256: format!("{:x}", Sha256::digest(&bytes)), base64: STANDARD.encode(bytes),
            width: dimensions.map(|size| size.0), height: dimensions.map(|size| size.1) });
        Ok(())
    }
}

pub fn prepare_delivery(document: &Document, expected_revision: u64, expected_hash: &str, options: &DeliveryOptions) -> Result<PreparedDelivery> {
    document::verify(document)?;
    if document.revision != expected_revision || document.hash != expected_hash {
        return Err(Error::Conflict("delivery base revision or hash changed; inspect the current deck before exporting".into()));
    }
    if options.max_output_bytes == 0 || options.max_output_bytes > MAX_DELIVERY_BYTES || !(160..=1600).contains(&options.max_dimension)
        || !options.min_font_size.is_finite() || !(8.0..=48.0).contains(&options.min_font_size)
    { return Err(Error::Invalid("delivery requires 1..32MiB output, 160..1600px images and 8..48px font floor".into())); }
    let all_pages: Vec<_> = (0..document.deck.slides.len()).collect();
    let selected = options.page_indices.clone().unwrap_or_else(|| all_pages.clone());
    let visual_requested = options.pdf || options.preflight || !matches!(options.preview, DeliveryPreview::None);
    if selected.is_empty() || selected.iter().any(|index| *index >= all_pages.len()) || selected.iter().collect::<BTreeSet<_>>().len() != selected.len()
        || (visual_requested && selected.len() > 8) || (options.page_indices.is_some() && selected.len() > 8)
    { return Err(Error::Invalid("delivery visual selection requires 1-8 unique existing page indices; PPTX always contains the whole deck".into())); }
    let presentation = document::export_presentation(document)?;
    let base64 = presentation["base64"].as_str().ok_or_else(|| Error::Invalid("PPTX export did not return bytes".into()))?;
    let mut bundle = Builder { files: Vec::new(), remaining: options.max_output_bytes };
    bundle.add("pptx", ".pptx".into(), "application/vnd.openxmlformats-officedocument.presentationml.presentation", all_pages.clone(), STANDARD.decode(base64).map_err(|_| Error::Invalid("PPTX export encoding".into()))?, None)?;

    let preflight = if options.preflight {
        Some(crate::authoring_preflight::preflight_presentation(document, &crate::authoring_preflight::PreflightOptions { page_indices: Some(selected.clone()), min_font_size: options.min_font_size })?)
    } else { None };
    let mut render_warnings = Vec::new();
    let mut preview_pages = Vec::new();
    if options.pdf {
        let output = export_static::export_static(&document.deck, &ExportOptions { format: ExportFormat::Pdf, page_indices: Some(selected.clone()), max_output_bytes: bundle.remaining.min(export_static::MAX_OUTPUT_BYTES), ..Default::default() })?;
        render_warnings.extend(output.warnings);
        for artifact in output.artifacts {
            bundle.add("pdf", ".pdf".into(), &artifact.mime_type, artifact.page_indices, artifact.bytes, None)?;
        }
    }
    if !matches!(options.preview, DeliveryPreview::None) {
        let preview = export_static::preview_presentation(document, &PreviewOptions { page_indices: Some(selected.clone()), max_dimension: options.max_dimension,
            layout: if matches!(options.preview, DeliveryPreview::Pages) { PreviewLayout::Pages } else { PreviewLayout::ContactSheet }, max_output_bytes: bundle.remaining.min(2 * 1024 * 1024), ..Default::default() })?;
        render_warnings.extend(preview.warnings);
        preview_pages = preview.pages;
        for (index, image) in preview.images.into_iter().enumerate() {
            let pages = preview_pages.iter().filter(|page| page.image_index == index).map(|page| page.page_index).collect::<Vec<_>>();
            let suffix = if matches!(options.preview, DeliveryPreview::Pages) { format!("-page-{:03}.png", pages[0] + 1) } else { "-overview.png".into() };
            bundle.add("preview", suffix, &image.mime_type, pages, STANDARD.decode(&image.base64).map_err(|_| Error::Invalid("preview encoding".into()))?, Some((image.width, image.height)))?;
        }
    }
    let mut warning_keys = BTreeSet::new();
    render_warnings.retain(|warning| warning_keys.insert((warning.page_index, warning.element_id.clone(), warning.code.clone(), warning.message.clone())));
    if options.notes {
        let mut notes = format!("\u{feff}{}\n\nDocument: {}\nRevision: {}\nContent SHA-256: {}\n\nSpeaker notes and evidence are user-authored; not independently verified.\n", document.deck.title, document.id, document.revision, document.hash);
        for (index, slide) in document.deck.slides.iter().enumerate() {
            notes.push_str(&format!("\n=== Slide {}: {} ({}) ===\n{}\n", index + 1, slide.title, slide.id, slide.notes));
            if notes.len() > bundle.remaining { return Err(Error::Limit("delivery notes exceed output byte budget".into())); }
        }
        bundle.add("notes", "-notes.txt".into(), "text/plain; charset=utf-8", all_pages.clone(), notes.into_bytes(), None)?;
    }
    if options.source_report {
        let report = json!({"format":"aislide.sources","version":1,"document_hash":document.hash,
            "sources":document.sources.iter().map(|source| json!({"id":source.id,"name":source.name,"format":source.format,"sha256":source.sha256,"content_sha256":source.content_sha256,"byte_length":source.byte_length,"attribution":source.attribution,"warnings":source.warnings})).collect::<Vec<_>>(),
            "bindings":document.bindings.iter().map(|binding| json!({"slide_id":binding.slide_id,"element_id":binding.element_id,"field":binding.field,"source_id":binding.source_id,"source_sha256":binding.source_sha256,"locator":binding.locator,"transform":binding.transform,"stale":binding.stale})).collect::<Vec<_>>(),
            "source_authenticity_verified":false,"source_freshness_verified":false,"semantic_truth_verified":false,
            "limitations":["Source text, table rows, raw values and original bytes are excluded from this report; locators and citation metadata can still be sensitive", "Guided evidence declarations remain in slide notes and are not authenticated source records", "No URL was fetched or revalidated"]});
        bundle.add("source_report", "-sources.json".into(), "application/json", all_pages.clone(), serde_json::to_vec_pretty(&report)?, None)?;
    }
    let files: Vec<_> = bundle.files.iter().map(|file| json!({"kind":file.kind,"suffix":file.suffix,"mime_type":file.mime_type,"page_indices":file.page_indices,"byte_length":file.byte_length,"sha256":file.sha256,"width":file.width,"height":file.height})).collect();
    let manifest = json!({"format":"aislide.delivery","version":1,
        "producer":{"name":"aislide-core","version":env!("CARGO_PKG_VERSION"),"engine":"aislide-core","contract_version":1},
        "document":{"id":document.id,"revision":document.revision,"hash":document.hash,"title":document.deck.title,"slide_count":document.deck.slides.len()},
        "options":options,"files":files,"preview_pages":preview_pages,"multi_file_atomic":false,
        "checks":{"document_verified":true,"pptx_exported":true,"native_preservation_checked":document.origin.is_some(),"source_bindings_current":true,
            "layout_measured":!presentation["layout"].is_null(),"layout_issues":presentation["layout"].get("issues").cloned().unwrap_or(json!([])),
            "layout_scope":if presentation["layout"].is_null() {"not_measured_for_native_origin"} else {"visible_slide_elements_and_used_design"},
            "preflight_page_indices":if options.preflight {selected.clone()} else {Vec::new()},
            "static_render_page_indices":if options.pdf || !matches!(options.preview,DeliveryPreview::None) {selected} else {Vec::new()},
            "office_visual_parity":false,"source_authenticity_verified":false,"source_freshness_verified":false,"semantic_truth_verified":false,"accessibility_checked":false},
        "preflight":preflight,"render_warnings":render_warnings,
        "privacy":{"pptx_includes_notes":true,"pptx_may_include_sources_and_bindings":true,"separate_notes_requested":options.notes,"separate_source_report_requested":options.source_report},
        "limitations":["A completed delivery is not a visual, accessibility or factual approval", "Static rendering and diagnostic scope are the recorded selected pages, not necessarily the complete PPTX", "No Office application, model inference, source URL fetch or freshness verification", "No files are published by core preparation; hosts must publish exclusively and write this manifest last", "Multiple files are not crash-atomic; a manifest identifies hashes, not a cryptographic signature"]});
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?.len();
    if manifest_bytes > bundle.remaining { return Err(Error::Limit("delivery manifest exceeds combined output byte budget".into())); }
    let result = PreparedDelivery { revision: document.revision, hash: document.hash.clone(), files: bundle.files, manifest };
    crate::preflight::serialized_bytes(&result, document.capacity_profile.limits().request_bytes, "delivery response")?;
    Ok(result)
}