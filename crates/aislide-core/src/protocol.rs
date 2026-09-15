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
    CreateAsset { id: String, base64: String, mime_type: String, alt: String, size: f64 },
    CreateDiagram { id: String, steps: Vec<String> },
    GraphCatalog {},
    CreateGraph { id: String, spec: crate::graphs::GraphSpec, #[serde(default)] theme: Option<crate::design::Theme> },
    InsertGraph { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, spec: crate::graphs::GraphSpec },
    UpdateGraph { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, spec: crate::graphs::GraphSpec },
    TransformGraph { spec: crate::graphs::GraphSpec, operations: Vec<crate::graphs::GraphOperation> },
    ApplyGraph { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, operations: Vec<crate::graphs::GraphOperation> },
    Ingest { input: crate::sources::SourceInput },
    OcrStatus {},
    DesignDefaults {},
    ObjectCatalog {},
    CreatePart { id: String, spec: crate::parts::PartSpec, #[serde(default)] theme: Option<crate::design::Theme> },
    PartCatalog {},
    InsertPart { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, spec: crate::parts::PartSpec },
    UpdatePart { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, spec: crate::parts::PartSpec },
    CreateObject { id: String, kind: crate::objects::ObjectKind, #[serde(default)] preset: Option<String>, #[serde(default)] rows: Option<usize>, #[serde(default)] columns: Option<usize> },
    UpdateDesign { deck: Deck, design: crate::design::Design },
    ApplyTheme { deck: Deck, theme: crate::design::Theme },
    AssignLayout { deck: Deck, slide_id: String, layout_id: String },
    DataReport { source: crate::sources::SourceDocument, mapping: crate::data_report::DataMapping },
    NewDocument { id: String, deck: Deck, #[serde(default)] sources: Vec<crate::sources::SourceDocument>, #[serde(default)] bindings: Vec<crate::data_report::SourceBinding>, #[serde(default)] report: Option<ReportInput> },
    CreatePresentation { id: String, title: String },
    EditSlides { document: crate::document::Document, expected_revision: u64, operations: Vec<crate::editing::SlideOperation> },
    EditElements { document: crate::document::Document, expected_revision: u64, slide_id: String, operations: Vec<crate::editing::ElementOperation> },
    Transaction { document: crate::document::Document, transaction: crate::document::Transaction },
    UndoTransaction { document: crate::document::Document, expected_revision: u64, receipt: crate::document::UndoReceipt },
    ExportProject { document: crate::document::Document },
    OpenProject { base64: String, checkpoint: crate::document::Checkpoint },
    OpenPresentation { id: String, base64: String },
    ExportPresentation { document: crate::document::Document },
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
        Request::CreateAsset { id, base64, mime_type, alt, size } => Ok(serde_json::to_value(crate::media::create_asset(&id, base64, &mime_type, &alt, size)?)?),
        Request::CreateDiagram { id, steps } => Ok(serde_json::to_value(crate::graphics::create_diagram(&id, &steps)?)?),
        Request::GraphCatalog {} => Ok(crate::graphs::catalog()),
        Request::CreateGraph { id, spec, theme } => Ok(serde_json::to_value(crate::graphs::create(&id, &spec, &theme.unwrap_or_default())?)?),
        Request::InsertGraph { document, expected_revision, slide_id, id, spec } => Ok(serde_json::to_value(crate::graphs::change(&document, expected_revision, &slide_id, &id, &spec, false)?)?),
        Request::UpdateGraph { document, expected_revision, slide_id, id, spec } => Ok(serde_json::to_value(crate::graphs::change(&document, expected_revision, &slide_id, &id, &spec, true)?)?),
        Request::TransformGraph { spec, operations } => Ok(serde_json::to_value(crate::graphs::transform(&spec, &operations)?)?),
        Request::ApplyGraph { document, expected_revision, slide_id, id, operations } => Ok(serde_json::to_value(crate::graphs::apply(&document, expected_revision, &slide_id, &id, &operations)?)?),
        Request::Ingest { input } => Ok(serde_json::to_value(crate::sources::ingest(input)?)?),
        Request::OcrStatus {} => Ok(serde_json::to_value(crate::extraction::ocr_status())?),
        Request::DesignDefaults {} => Ok(serde_json::to_value(crate::design::Design::default())?),
        Request::ObjectCatalog {} => Ok(crate::objects::catalog()),
        Request::CreatePart { id, spec, theme } => Ok(serde_json::to_value(crate::parts::create_with_theme(&id, &spec, &theme.unwrap_or_default())?)?),
        Request::PartCatalog {} => Ok(crate::parts::catalog()),
        Request::InsertPart { document, expected_revision, slide_id, id, spec } => Ok(serde_json::to_value(crate::parts::state::change(&document,expected_revision,&slide_id,&id,&spec,false)?)?),
        Request::UpdatePart { document, expected_revision, slide_id, id, spec } => Ok(serde_json::to_value(crate::parts::state::change(&document,expected_revision,&slide_id,&id,&spec,true)?)?),
        Request::CreateObject { id, kind, preset, rows, columns } => Ok(serde_json::to_value(crate::objects::create(id, kind, preset, rows, columns)?)?),
        Request::UpdateDesign { deck, design } => Ok(serde_json::to_value(crate::design::update_design(deck, design)?)?),
        Request::ApplyTheme { deck, theme } => Ok(serde_json::to_value(crate::design::apply_theme(deck, theme)?)?),
        Request::AssignLayout { deck, slide_id, layout_id } => Ok(serde_json::to_value(crate::design::assign_layout(deck, &slide_id, &layout_id)?)?),
        Request::DataReport { source, mapping } => Ok(serde_json::to_value(crate::data_report::data_report(&source, &mapping)?)?),
        Request::NewDocument { id, deck, sources, bindings, report } => Ok(serde_json::to_value(crate::document::create(id, deck, sources, bindings, report)?)?),
        Request::CreatePresentation { id, title } => Ok(serde_json::to_value(crate::editing::create(id, title)?)?),
        Request::EditSlides { document, expected_revision, operations } => Ok(serde_json::to_value(crate::editing::slides(&document, expected_revision, &operations)?)?),
        Request::EditElements { document, expected_revision, slide_id, operations } => Ok(serde_json::to_value(crate::editing::elements(&document, expected_revision, &slide_id, &operations)?)?),
        Request::Transaction { document, transaction } => Ok(serde_json::to_value(crate::document::transact(&document, transaction)?)?),
        Request::UndoTransaction { document, expected_revision, receipt } => Ok(serde_json::to_value(crate::document::undo(&document, expected_revision, receipt)?)?),
        Request::ExportProject { document } => crate::document::export(&document),
        Request::OpenProject { base64, checkpoint } => Ok(serde_json::to_value(crate::document::open(&base64, checkpoint)?)?),
        Request::OpenPresentation { id, base64 } => crate::document::open_presentation(id, decode(&base64)?),
        Request::ExportPresentation { document } => crate::document::export_presentation(&document),
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