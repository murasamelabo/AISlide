use crate::{Error, Result, generation::CancellationToken, image_edit::{self, EditedImage, ImageOutputFormat}};
use image::{GrayImage, RgbaImage, imageops::{resize, FilterType}};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs::File, io::{Cursor, Read}, path::PathBuf, sync::Mutex, time::Instant};
use tract_onnx::prelude::*;

pub const MODEL_SHA256: &str = "309c8469258dda742793dce0ebea8e6dd393174f89934733ecc8b14c76f4ddd8";
const MODEL_BYTES: usize = 4_574_861;
const SIDE: usize = 320;
const MAP_SIZE: usize = SIDE * SIDE;
const TENSOR_BUDGET: usize = 768 * 1024 * 1024;
static INFERENCE: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Consent { sha256: String, accepted_dataset_terms: bool }

#[derive(Serialize)]
pub struct Status {
    pub configured: bool,
    pub model: &'static str,
    pub sha256: &'static str,
    pub runtime: &'static str,
    pub remote: bool,
    pub tensor_budget_bytes: usize,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SegmentedImage { pub image: EditedImage, pub provenance: SegmentationProvenance }

#[derive(Debug, Serialize)]
pub struct SegmentationProvenance {
    pub mode: &'static str,
    pub model: &'static str,
    pub model_sha256: &'static str,
    pub source_sha256: String,
    pub runtime: &'static str,
    pub remote: bool,
    pub elapsed_ms: u64,
    pub planned_tensor_bytes: usize,
    pub verified: bool,
}

fn model_bytes() -> Result<Vec<u8>> {
    let supplied = std::env::var_os("AISLIDE_U2NETP_MODEL");
    let path = if let Some(path) = supplied.as_ref() { PathBuf::from(path) } else {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or_else(|| Error::Unsupported("set AISLIDE_U2NETP_MODEL to the verified local model".into()))?).join("AISlide/models/u2netp.onnx")
    };
    validate_local_path(&path)?;
    let accepted = if supplied.is_some() { std::env::var("AISLIDE_SEGMENTATION_LICENSE_ACCEPTED").as_deref() == Ok("1") } else {
        let consent = File::open(path.with_file_name("u2netp-consent.json")).ok().and_then(|file| {
            let mut bytes = Vec::new(); file.take(8193).read_to_end(&mut bytes).ok()?;
            if bytes.len() > 8192 { return None; }
            serde_json::from_slice::<Consent>(&bytes).ok()
        });
        consent.is_some_and(|consent| consent.sha256 == MODEL_SHA256 && consent.accepted_dataset_terms)
    };
    if !accepted { return Err(Error::Unsupported("U2NetP requires explicit model and DUTS dataset terms acknowledgement. Run node tools/local-image-model-setup.mjs --accept-model-and-dataset-terms; see docs/authoring/local-ai.md.".into())); }
    let file = File::open(&path).map_err(|_| Error::Unsupported("U2NetP is not installed or readable. Use the explicit local model setup or AISLIDE_U2NETP_MODEL.".into()))?;
    let mut bytes = Vec::new();
    file.take((MODEL_BYTES + 1) as u64).read_to_end(&mut bytes).map_err(|_| Error::Unsupported("local model read failed".into()))?;
    if bytes.len() != MODEL_BYTES || format!("{:x}", Sha256::digest(&bytes)) != MODEL_SHA256 { return Err(Error::Unsupported("unsupported U2NetP bytes: model SHA-256 or size mismatch; external data and custom models are not accepted".into())); }
    Ok(bytes)
}

pub fn status() -> Status {
    let checked = model_bytes();
    Status { configured: checked.is_ok(), model: "u2netp", sha256: MODEL_SHA256, runtime: "tract-onnx-cpu-0.23.7", remote: false, tensor_budget_bytes: TENSOR_BUDGET,
        message: match checked { Ok(_) => "Model bytes verified; inference and image quality have not been tested by this status check. Native inference is non-preemptible; cancelled results are discarded.".into(), Err(error) => error.to_string() } }
}

fn cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() { Err(Error::Generation("segmentation cancelled; no image applied".into())) } else { Ok(()) }
}

fn inference_boundary<Value>(token: &CancellationToken, run: impl FnOnce() -> Result<Value>) -> Result<Value> {
    cancelled(token)?;
    let result = run()?;
    cancelled(token)?;
    Ok(result)
}

fn validate_local_path(path: &std::path::Path) -> Result<()> {
    let value = path.to_string_lossy();
    if !path.is_absolute() || value.starts_with("\\\\") || value.starts_with("//") || value.contains("://") {
        return Err(Error::Invalid("host model path must be an absolute local path, not a network or device path".into()));
    }
    Ok(())
}

fn preprocess(pixels: &RgbaImage) -> Vec<f32> {
    let rgb = image::RgbImage::from_fn(pixels.width(), pixels.height(), |column, row| { let pixel = pixels.get_pixel(column, row); image::Rgb([pixel[0], pixel[1], pixel[2]]) });
    let resized = resize(&rgb, SIDE as u32, SIDE as u32, FilterType::Lanczos3);
    let maximum = f32::from(*resized.as_raw().iter().max().unwrap_or(&0)).max(f32::EPSILON);
    let mut input = vec![0.0; MAP_SIZE * 3];
    for (index, pixel) in resized.pixels().enumerate() {
        for channel in 0..3 { input[channel * MAP_SIZE + index] = (f32::from(pixel[channel]) / maximum - [0.485, 0.456, 0.406][channel]) / [0.229, 0.224, 0.225][channel]; }
    }
    input
}
#[test]
fn model_configuration_rejects_relative_network_and_device_paths() {
    for path in ["model.onnx", "\\\\server\\share\\model.onnx", "\\\\?\\C:\\model.onnx", "//server/share/model.onnx", "https://example.invalid/model.onnx"] { assert!(validate_local_path(std::path::Path::new(path)).is_err()); }
}

fn apply_mask(mut pixels: RgbaImage, output: &[f32]) -> Result<RgbaImage> {
    if output.len() != MAP_SIZE || output.iter().any(|value| !value.is_finite()) { return Err(Error::Unsupported("segmentation output must be one finite 320x320 saliency map".into())); }
    let minimum = output.iter().copied().fold(f32::INFINITY, f32::min);
    let maximum = output.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if maximum - minimum <= f32::EPSILON { return Err(Error::Unsupported("segmentation returned a constant mask; no candidate created".into())); }
    let values = output.iter().map(|value| (((value - minimum) / (maximum - minimum)) * 255.0).round().clamp(0.0,255.0) as u8).collect();
    let mask = GrayImage::from_raw(SIDE as u32, SIDE as u32, values).ok_or_else(|| Error::Unsupported("invalid saliency dimensions".into()))?;
    let mask = resize(&mask, pixels.width(), pixels.height(), FilterType::Lanczos3);
    for (pixel, mask) in pixels.pixels_mut().zip(mask.pixels()) { pixel[3] = ((u32::from(pixel[3]) * u32::from(mask[0]) + 127) / 255) as u8; }
    Ok(pixels)
}

pub fn segment(base64: &str, mime_type: &str, token: CancellationToken) -> Result<SegmentedImage> {
    cancelled(&token)?;
    let _guard = INFERENCE.try_lock().map_err(|_| Error::Unsupported("segmentation is busy; only one inference can run at a time".into()))?;
    let started = Instant::now();
    let pixels = image_edit::decode_rgba(base64, mime_type)?;
    let bytes = model_bytes()?;
    cancelled(&token)?;
    let model = tract_onnx::onnx().model_for_read(&mut Cursor::new(bytes))
        .and_then(|model| model.with_input_fact(0, f32::fact([1,3,SIDE,SIDE]).into()))
        .and_then(|model| model.into_optimized())
        .map_err(|_| Error::Unsupported("the pinned U2NetP graph is unsupported by this CPU runtime".into()))?;
    let runnable = model.into_runnable().map_err(|_| Error::Unsupported("segmentation plan could not be initialized".into()))?;
    let model = runnable.model();
    let order = runnable.order_without_consts();
    let mut sizes = vec![0usize; model.nodes().len()];
    let mut last_use = vec![0usize; sizes.len()];
    let mut scheduled = vec![false; sizes.len()];
    for (step, node_id) in order.iter().enumerate() {
        scheduled[*node_id] = true;
        last_use[*node_id] = step;
        for input in &model.node(*node_id).inputs { last_use[input.node] = step; }
    }
    for output in model.output_outlets().map_err(|_| Error::Unsupported("invalid segmentation outputs".into()))? { last_use[output.node] = order.len(); }
    for node in model.nodes() {
        for outlet in &node.outputs {
            let shape = outlet.fact.shape.as_concrete().ok_or_else(|| Error::Unsupported("segmentation requires fixed tensor shapes".into()))?;
            let bytes = shape.iter().try_fold(outlet.fact.datum_type.size_of(), |size, dimension| size.checked_mul(*dimension))
                .ok_or_else(|| Error::Limit("segmentation tensor size overflow".into()))?;
            sizes[node.id] = sizes[node.id].checked_add(bytes).filter(|size| *size <= TENSOR_BUDGET)
                .ok_or_else(|| Error::Limit("segmentation exceeds its independent 768 MiB planned tensor budget".into()))?;
        }
    }
    let mut live: usize = sizes.iter().enumerate().filter(|(index, _)| !scheduled[*index]).map(|(_, size)| *size).sum();
    let mut planned_tensor_bytes = live;
    for (step, node_id) in order.iter().enumerate() {
        live = live.checked_add(sizes[*node_id]).filter(|size| *size <= TENSOR_BUDGET)
            .ok_or_else(|| Error::Limit("segmentation exceeds its independent 768 MiB live tensor budget".into()))?;
        planned_tensor_bytes = planned_tensor_bytes.max(live);
        for (index, last) in last_use.iter().enumerate() { if scheduled[index] && *last == step { live = live.saturating_sub(sizes[index]); } }
    }
    let tensor = Tensor::from_shape(&[1,3,SIDE,SIDE], &preprocess(&pixels)).map_err(|_| Error::Unsupported("segmentation input tensor invalid".into()))?;
    let outputs = inference_boundary(&token, || runnable.run(tvec!(tensor.into())).map_err(|_| Error::Unsupported("U2NetP CPU inference failed".into())))?;
    let output = outputs.first().ok_or_else(|| Error::Unsupported("U2NetP returned no saliency map".into()))?;
    if output.shape() != [1,1,SIDE,SIDE] { return Err(Error::Unsupported("U2NetP output shape is unsupported".into())); }
    let view = output.to_plain_array_view::<f32>().map_err(|_| Error::Unsupported("U2NetP output is not float32".into()))?;
    let values = view.as_slice().ok_or_else(|| Error::Unsupported("U2NetP output is not contiguous".into()))?;
    let pixels = apply_mask(pixels, values)?;
    let image = image_edit::encode_pixels(&pixels, ImageOutputFormat::Png)?;
    cancelled(&token)?;
    use base64::Engine;
    let source = base64::engine::general_purpose::STANDARD.decode(base64).map_err(|_| Error::Invalid("image base64".into()))?;
    Ok(SegmentedImage { image, provenance: SegmentationProvenance { mode: "model", model: "u2netp", model_sha256: MODEL_SHA256,
        source_sha256: format!("{:x}",Sha256::digest(source)), runtime: "tract-onnx-cpu-0.23.7", remote: false, elapsed_ms: started.elapsed().as_millis() as u64, planned_tensor_bytes, verified: false } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocessing_is_finite_nchw_and_uses_global_rgb_maximum() {
        let black = RgbaImage::from_pixel(2,2,image::Rgba([0,0,0,255]));
        let output = preprocess(&black);
        assert_eq!(output.len(),3 * MAP_SIZE);
        assert!(output.iter().all(|value| value.is_finite()));
        assert!((output[0] + 0.485 / 0.229).abs() < 1e-5);
        let red = preprocess(&RgbaImage::from_pixel(2,2,image::Rgba([128,0,0,255])));
        assert!((red[0] - (1.0 - 0.485) / 0.229).abs() < 1e-5);
        assert!((red[MAP_SIZE] + 0.456 / 0.224).abs() < 1e-5);
    }

    #[test]
    fn masks_reject_nonfinite_constant_and_multiply_existing_alpha() {
        let pixels = RgbaImage::from_pixel(SIDE as u32,SIDE as u32,image::Rgba([200,100,50,128]));
        for mask in [vec![0.5;MAP_SIZE],vec![f32::NAN;MAP_SIZE],vec![0.0;10]] { assert!(apply_mask(pixels.clone(),&mask).is_err()); }
        let mut mask = vec![0.0;MAP_SIZE]; mask[1] = 1.0; mask[2] = 0.5;
        let result = apply_mask(pixels,&mask).unwrap();
        assert_eq!(result.get_pixel(0,0).0,[200,100,50,0]);
        assert_eq!(result.get_pixel(1,0)[3],128);
        assert_eq!(result.get_pixel(2,0)[3],64);
    }

    #[test]
    fn early_cancel_does_not_decode_or_load_models() {
        let token = CancellationToken::new(); token.cancel();
        assert!(segment("invalid","image/png",token).unwrap_err().to_string().contains("cancelled"));
    }

    #[test]
    fn nonpreemptible_inference_discards_late_result_and_skips_early_work() {
        let token = CancellationToken::new();
        let mut completed = false;
        let result = inference_boundary(&token, || { completed = true; token.cancel(); Ok(42) });
        assert!(completed);
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        assert!(inference_boundary(&token, || -> Result<()> { panic!("early cancellation started inference") }).is_err());
    }

    #[test]
    fn concurrent_segmentation_fails_before_decoding() {
        let _guard = INFERENCE.lock().unwrap();
        assert!(segment("invalid","image/png",CancellationToken::new()).unwrap_err().to_string().contains("busy"));
    }
}