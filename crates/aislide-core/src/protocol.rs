use crate::{model::{Deck, validate_deck}, package::Package, pptx::{export_pptx, inspect_pptx, patch_text}, report::{ReportInput, compile_report, sample_report}, Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use crate::generation::{CancellationToken, GenerationInput, generate_configured, generate_configured_async, provider_status};

pub const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Sample {},
    ProviderStatus {},
    Generate { input: GenerationInput },
    Compile { report: ReportInput },
    Export { deck: Deck },
    Validate { deck: Deck },
    Inspect { base64: String },
    Roundtrip { base64: String },
    PatchText { base64: String, part: String, shape_id: String, run_index: usize, expected: String, text: String },
}

pub fn execute_request(value: Value) -> Result<Value> {
    execute(parse_request(value)?)
}

pub async fn execute_request_async(value: Value, cancellation: CancellationToken) -> Result<Value> {
    if cancellation.is_cancelled() { return Err(Error::Generation("operation cancelled".into())); }
    match parse_request(value)? {
        Request::Generate { input } => Ok(serde_json::to_value(generate_configured_async(&input, cancellation).await?)?),
        request => tokio::task::spawn_blocking(move || execute(request)).await
            .map_err(|_| Error::Generation("core worker failed".into()))?,
    }
}

fn parse_request(value: Value) -> Result<Request> {
    if serde_json::to_vec(&value)?.len() > MAX_REQUEST_BYTES {
        return Err(Error::Limit("JSON request > 4 MiB".into()));
    }
    Ok(serde_json::from_value(value)?)
}

fn execute(request: Request) -> Result<Value> {
    match request {
        Request::Sample {} => Ok(serde_json::to_value(sample_report())?),
        Request::ProviderStatus {} => Ok(serde_json::to_value(provider_status())?),
        Request::Generate { input } => Ok(serde_json::to_value(generate_configured(&input)?)?),
        Request::Compile { report } => Ok(serde_json::to_value(compile_report(&report)?)?),
        Request::Export { deck } => encoded(export_pptx(&deck)?),
        Request::Validate { deck } => {
            validate_deck(&deck)?;
            Ok(json!({"structurally_valid":true,"slides":deck.slides.len(),"warnings":["Only structure and geometry were checked. Text fit and PowerPoint visual parity require separate validation."]}))
        }
        Request::Inspect { base64 } => Ok(serde_json::to_value(inspect_pptx(decode(&base64)?)?)?),
        Request::Roundtrip { base64 } => {
            let bytes = decode(&base64)?;
            inspect_pptx(bytes.clone())?;
            encoded(Package::open(bytes)?.save()?)
        }
        Request::PatchText { base64, part, shape_id, run_index, expected, text } => {
            encoded(patch_text(decode(&base64)?, &part, &shape_id, run_index, &expected, &text)?)
        }
    }
}

fn decode(value: &str) -> Result<Vec<u8>> {
    if value.len() > MAX_REQUEST_BYTES { return Err(Error::Limit("encoded archive budget".into())); }
    STANDARD.decode(value).map_err(|_| Error::Invalid("invalid base64".into()))
}

fn encoded(bytes: Vec<u8>) -> Result<Value> {
    if bytes.len() > MAX_REQUEST_BYTES * 3 / 4 - 1024 {
        return Err(Error::Limit("archive too large for the 4 MiB JSON protocol".into()));
    }
    Ok(json!({"base64":STANDARD.encode(bytes),"filename":"report.pptx"}))
}