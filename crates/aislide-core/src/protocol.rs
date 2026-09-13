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
    CreatePicture { id: String, base64: String, mime_type: String, alt: String },
    CreateDiagram { id: String, steps: Vec<String> },
    Ingest { input: crate::sources::SourceInput },
    OcrStatus {},
    DataReport { source: crate::sources::SourceDocument, mapping: crate::data_report::DataMapping },
    NewDocument { id: String, deck: Deck, #[serde(default)] sources: Vec<crate::sources::SourceDocument>, #[serde(default)] bindings: Vec<crate::data_report::SourceBinding>, #[serde(default)] report: Option<ReportInput> },
    Transaction { document: crate::document::Document, transaction: crate::document::Transaction },
    UndoTransaction { document: crate::document::Document, expected_revision: u64, receipt: crate::document::UndoReceipt },
    ExportProject { document: crate::document::Document },
    OpenProject { base64: String, checkpoint: crate::document::Checkpoint },
    Export { deck: Deck },
    Validate { deck: Deck },
    MeasureLayout { deck: Deck },
    Inspect { base64: String },
    ImportPptx { base64: String },
    ImportDocument { id: String, base64: String },
    SaveImport { base64: String, deck: Deck },
    Roundtrip { base64: String },
    PackageManifest { base64: String },
    PatchText { base64: String, part: String, shape_id: String, run_index: usize, expected: String, text: String },
}

pub fn execute_request(value: Value) -> Result<Value> {
    checked_response(execute(parse_request(value)?)?)
}

pub async fn execute_request_async(value: Value, cancellation: CancellationToken) -> Result<Value> {
    if cancellation.is_cancelled() { return Err(Error::Generation("operation cancelled".into())); }
    let result = match parse_request(value)? {
        Request::Generate { input } => Ok(serde_json::to_value(generate_configured_async(&input, cancellation).await?)?),
        request => tokio::task::spawn_blocking(move || execute(request)).await
            .map_err(|_| Error::Generation("core worker failed".into()))?,
    }?;
    checked_response(result)
}

fn checked_response(value: Value) -> Result<Value> {
    if serde_json::to_vec(&value)?.len() > MAX_REQUEST_BYTES { return Err(Error::Limit("JSON response > 4 MiB; reduce document or source size".into())); }
    Ok(value)
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
        Request::CreatePicture { id, base64, mime_type, alt } => Ok(serde_json::to_value(crate::media::create_picture(&id, base64, &mime_type, &alt)?)?),
        Request::CreateDiagram { id, steps } => Ok(serde_json::to_value(crate::graphics::create_diagram(&id, &steps)?)?),
        Request::Ingest { input } => Ok(serde_json::to_value(crate::sources::ingest(input)?)?),
        Request::OcrStatus {} => Ok(serde_json::to_value(crate::extraction::ocr_status())?),
        Request::DataReport { source, mapping } => Ok(serde_json::to_value(crate::data_report::data_report(&source, &mapping)?)?),
        Request::NewDocument { id, deck, sources, bindings, report } => Ok(serde_json::to_value(crate::document::create(id, deck, sources, bindings, report)?)?),
        Request::Transaction { document, transaction } => Ok(serde_json::to_value(crate::document::transact(&document, transaction)?)?),
        Request::UndoTransaction { document, expected_revision, receipt } => Ok(serde_json::to_value(crate::document::undo(&document, expected_revision, receipt)?)?),
        Request::ExportProject { document } => crate::document::export(&document),
        Request::OpenProject { base64, checkpoint } => Ok(serde_json::to_value(crate::document::open(&base64, checkpoint)?)?),
        Request::Export { deck } => encoded(export_pptx(&deck)?),
        Request::MeasureLayout { deck } => Ok(serde_json::to_value(crate::layout::measure_layout(&deck)?)?),
        Request::Validate { deck } => {
            validate_deck(&deck)?;
            Ok(json!({"structurally_valid":true,"slides":deck.slides.len(),"deck":deck,"warnings":["Only structure and geometry were checked. Text fit and PowerPoint visual parity require separate validation."]}))
        }
        Request::Inspect { base64 } => Ok(serde_json::to_value(inspect_pptx(decode(&base64)?)?)?),
        Request::ImportPptx { base64 } => Ok(serde_json::to_value(crate::import::import_pptx(decode(&base64)?)?)?),
        Request::ImportDocument { id, base64 } => crate::document::import_document(id, decode(&base64)?),
        Request::SaveImport { base64, deck } => encoded(crate::import::save_import(decode(&base64)?, &deck)?),
        Request::Roundtrip { base64 } => {
            let bytes = decode(&base64)?;
            inspect_pptx(bytes.clone())?;
            encoded(Package::open(bytes)?.save()?)
        }
        Request::PackageManifest { base64 } => {
            use sha2::{Digest, Sha256};
            let package = Package::open(decode(&base64)?)?;
            let parts = package.parts().iter().map(|(path, bytes)| json!({"path":path,"bytes":bytes.len(),"sha256":format!("{:x}", Sha256::digest(bytes))})).collect::<Vec<_>>();
            Ok(json!({"parts":parts}))
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