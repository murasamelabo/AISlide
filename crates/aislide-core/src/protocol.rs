use crate::{model::{Deck, validate_deck}, package::Package, pptx::{export_pptx, inspect_pptx, patch_text}, report::{ReportInput, compile_report, sample_report}, Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use crate::generation::{CancellationToken, GenerationInput, generate_configured, generate_configured_async, provider_status};

pub const MAX_REQUEST_BYTES: usize = crate::limits::LARGE.request_bytes;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    CapacityProfiles {},
    InspectFont { base64: String },
    InspectPptxFonts { base64: String },
    ListFonts { deck: Deck },
    EmbedFont { document: crate::document::Document, expected_revision: u64, base64: String, license_acknowledged: bool },
    SetFontUsage { document: crate::document::Document, expected_revision: u64, sha256: String, license_acknowledged: bool },
    Sample {},
    ProviderStatus {},
    Generate { input: GenerationInput },
    TextAssist { input: crate::text_assist::Input },
    ApplyTextAssist { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, expected_text: String, candidate: crate::text_assist::Candidate },
    Compile { report: ReportInput },
    CreatePicture { id: String, base64: String, mime_type: String, alt: String },
    InspectRaster { base64: String, mime_type: String },
    CreateAsset { id: String, base64: String, mime_type: String, alt: String, size: f64 },
    CreateDiagram { id: String, steps: Vec<String> },
    GraphCatalog {},
    ArchitectureIcons {},
    ArchitectureIconAssets { ids: Vec<String> },
    CreateGraphIcon { base64: String, mime_type: String, #[serde(default)] alt: String },
    CreateGraph { id: String, spec: crate::graphs::GraphSpec, #[serde(default)] theme: Option<crate::design::Theme>, #[serde(default)] include_diagnostics: bool },
    InsertGraph { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, spec: crate::graphs::GraphSpec },
    UpdateGraph { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, spec: crate::graphs::GraphSpec },
    TransformGraph { spec: crate::graphs::GraphSpec, operations: Vec<crate::graphs::GraphOperation> },
    ApplyGraph { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, operations: Vec<crate::graphs::GraphOperation> },
    Ingest { input: crate::sources::SourceInput },
    OcrStatus {},
    DesignDefaults {},
    DesignPresets {},
    ApplyDesignPreset { deck: Deck, preset_id: String },
    ObjectCatalog {},
    ComputeChartPresentation { kind: crate::model::ChartKind, categories: Vec<String>, series: Vec<crate::model::ChartSeries>, #[serde(default)] options: crate::model::ChartOptions },
    RenderElementPreview { element: crate::model::Element, #[serde(default)] theme: Option<crate::design::Theme> },
    CreatePart { id: String, spec: crate::parts::PartSpec, #[serde(default)] theme: Option<crate::design::Theme> },
    PartCatalog {},
    BestPracticeProfiles {},
    BestPracticeGuide { profile_id: String },
    ValidateGuidedPresentation { input: crate::guided::GuidedInput },
    CreateGuidedPresentation { id: String, input: crate::guided::GuidedInput },
    InsertPart { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, spec: crate::parts::PartSpec },
    UpdatePart { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, spec: crate::parts::PartSpec },
    CreateObject { id: String, kind: crate::objects::ObjectKind, #[serde(default)] preset: Option<String>, #[serde(default)] rows: Option<usize>, #[serde(default)] columns: Option<usize> },
    UpdateDesign { deck: Deck, design: crate::design::Design },
    ApplyTheme { deck: Deck, theme: crate::design::Theme },
    SetMasterTheme { deck: Deck, master_id: String, theme: Option<crate::design::Theme> },
    SetDesignField { deck: Deck, field: crate::fields::DesignField },
    DesignCapabilities {},
    AssignLayout { deck: Deck, slide_id: String, layout_id: String, #[serde(default)] preserve_freeform: bool },
    DataReport { source: crate::sources::SourceDocument, mapping: crate::data_report::DataMapping },
    NewDocument { id: String, deck: Deck, #[serde(default)] sources: Vec<crate::sources::SourceDocument>, #[serde(default)] bindings: Vec<crate::data_report::SourceBinding>, #[serde(default)] report: Option<ReportInput> },
    CreatePresentation { id: String, title: String },
    EditSlides { document: crate::document::Document, expected_revision: u64, operations: Vec<crate::editing::SlideOperation> },
    EditElements { document: crate::document::Document, expected_revision: u64, slide_id: String, operations: Vec<crate::editing::ElementOperation> },
    ApplyOperations { document: crate::document::Document, expected_revision: u64, expected_hash: String, operations: Vec<crate::authoring_batch::Operation> },
    ImportSlides { document: crate::document::Document, expected_revision: u64, expected_hash: String, source: crate::document::Document, source_slide_ids: Vec<String>, prefix: String, #[serde(default)] after: Option<String> },
    SearchText { deck: Deck, options: crate::text_ops::SearchOptions },
    ImportProofingDictionary { language: String, format: crate::proofing::DictionaryFormat, content: String },
    CopyFormat { document: crate::document::Document, slide_id: String, id: String, #[serde(default)] paragraph_index: usize, #[serde(default)] run_index: usize },
    ApplyFormat { document: crate::document::Document, expected_revision: u64, slide_id: String, ids: Vec<String>, style: crate::authoring_ops::FormatSnapshot },
    SetProofingLanguage { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, language: String },
    ProofText { text: String, language: String, #[serde(default)] dictionary: Option<crate::proofing::Dictionary>, #[serde(default)] term: Option<String>, #[serde(default)] target_language: Option<String> },
    ReplaceText { document: crate::document::Document, expected_revision: u64, options: crate::text_ops::ReplaceOptions },
    ReplaceFont { document: crate::document::Document, expected_revision: u64, from: String, to: String },
    AuthoringCapabilities {},
    FormatTextElement { element: crate::model::Element, start: usize, end: usize, style: crate::rich_text::RunStyle },
    ReplaceElementText { element: crate::model::Element, text: String },
    SetTableCellText { element: crate::model::Element, row: usize, column: usize, text: String },
    EditVector { element: crate::model::Element, path: crate::vector::VectorPath },
    SampleSlidePixel { document: crate::document::Document, slide_id: String, x: u32, y: u32 },
    CombineShapes { document: crate::document::Document, expected_revision: u64, slide_id: String, ids: Vec<String>, operation: crate::geometry_ops::BooleanOperation, result_id: String },
    UpdateRichNotes { document: crate::document::Document, expected_revision: u64, slide_id: String, paragraphs: Vec<crate::rich_text::RichParagraph> },
    UpdateAuxiliaryDesign { document: crate::document::Document, expected_revision: u64, design: crate::model::AuxiliaryDesign },
    RefreshFields { document: crate::document::Document, expected_revision: u64, reference_date: String, #[serde(default)] reference_time: Option<String>, #[serde(default)] locale: Option<String> },
    AddComment { document: crate::document::Document, expected_revision: u64, slide_id: String, comment: crate::comments::Comment },
    ModernComment { document: crate::document::Document, expected_revision: u64, slide_id: String, operation: crate::comments::modern::Operation },
    SetTableHeaders { document: crate::document::Document, expected_revision: u64, slide_id: String, element_id: String, policy: crate::review::TableHeaders },
    ReplyComment { document: crate::document::Document, expected_revision: u64, slide_id: String, parent_id: String, comment: crate::comments::Comment },
    ResolveComment { document: crate::document::Document, expected_revision: u64, slide_id: String, comment_id: String, resolved: bool },
    RemoveComment { document: crate::document::Document, expected_revision: u64, slide_id: String, comment_id: String },
    SetAccessibility { document: crate::document::Document, expected_revision: u64, slide_id: String, element_id: String, metadata: Option<crate::review::ElementAccessibility> },
    SetReadingOrder { document: crate::document::Document, expected_revision: u64, slide_id: String, order: Vec<String> },
    CheckAccessibility { document: crate::document::Document },
    InspectDocument { document: crate::document::Document },
    ExportCleanCopy { document: crate::document::Document, options: crate::review::CleanCopyOptions },
    FormatText { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, start: usize, end: usize, style: crate::rich_text::RunStyle },
    ReplaceTextContent { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, text: String },
    UpdateParagraphs { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, paragraphs: Vec<crate::rich_text::RichParagraph> },
    EditSelection { document: crate::document::Document, expected_revision: u64, slide_id: String, operation: crate::selection::SelectionOperation, #[serde(default)] clipboard: Option<crate::selection::ElementBundle> },
    ResizeCanvas { document: crate::document::Document, expected_revision: u64, width: u32, height: u32, mode: crate::canvas::ResizeMode },
    ImportTemplate { id: String, kind: crate::authoring_ops::TemplateKind, base64: String },
    InspectMasterSource { kind: crate::master_import::SourceKind, base64: String },
    PreviewMasterImport { document: crate::document::Document, expected_revision: u64, expected_hash: String, input: crate::master_import::MasterImportInput },
    ImportMasters { document: crate::document::Document, expected_revision: u64, expected_hash: String, input: crate::master_import::MasterImportInput, expected_candidate_hash: String },
    ExportTemplate { document: crate::document::Document, kind: crate::authoring_ops::TemplateKind },
    EditImage { base64: String, mime_type: String, params: crate::image_edit::ImageEditParams },
    SegmentationStatus {},
    SegmentImage { base64: String, mime_type: String },
    ApplyImageEdit { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, image: crate::image_edit::EditedImage },
    EditTable { document: crate::document::Document, expected_revision: u64, slide_id: String, id: String, operations: Vec<crate::authoring_ops::TableOperation> },
    Transaction { document: crate::document::Document, transaction: crate::document::Transaction },
    UndoTransaction { document: crate::document::Document, expected_revision: u64, receipt: crate::document::UndoReceipt },
    ExportProject { document: crate::document::Document },
    OpenProject { base64: String, checkpoint: crate::document::Checkpoint },
    OpenPresentation { id: String, base64: String },
    ExportPresentation { document: crate::document::Document },
    ExportStatic { document: crate::document::Document, options: crate::export_static::ExportOptions },
    PreviewPresentation { document: crate::document::Document, #[serde(default)] options: crate::export_static::PreviewOptions },
    PreflightPresentation { document: crate::document::Document, #[serde(default)] options: crate::authoring_preflight::PreflightOptions },
    PrepareDelivery { document: crate::document::Document, expected_revision: u64, expected_hash: String, #[serde(default)] options: crate::delivery::DeliveryOptions },
    PreviewSlideRevision { document: crate::document::Document, expected_revision: u64, expected_hash: String, slide_id: String, edits: Vec<crate::slide_revision::RevisionEdit>, #[serde(default)] max_dimension: Option<u32> },
    ApplySlideRevision { document: crate::document::Document, expected_revision: u64, expected_hash: String, slide_id: String, edits: Vec<crate::slide_revision::RevisionEdit>, candidate_hash: String },
    VerifyRecovery { document: crate::document::Document },
    VerifySessionRecovery { envelope: crate::document::SessionRecovery },
    PrepareRecovery { state: crate::recovery::State, expected_generation: u64, action: crate::recovery::Action, now_ms: u64 },
    VerifyRecoveryRecord { entry: crate::recovery::Entry, payload: String },
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
    let profile = capacity_profile(&value)?;
    checked_response(execute(parse_request(value, profile, None)?, profile)?, profile)
}

pub async fn execute_request_async(value: Value, cancellation: CancellationToken) -> Result<Value> {
    if cancellation.is_cancelled() { return Err(Error::Generation("operation cancelled".into())); }
    let profile = capacity_profile(&value)?;
    let worker_token = cancellation.clone();
    let result = match parse_request(value, profile, Some(&cancellation))? {
        Request::Generate { input } => Ok(serde_json::to_value(generate_configured_async(&input, cancellation.clone()).await?)?),
        Request::TextAssist { input } => Ok(serde_json::to_value(crate::text_assist::configured_async(&input, cancellation.clone()).await?)?),
        Request::SegmentImage { base64, mime_type } => tokio::task::spawn_blocking(move || {
            Ok(serde_json::to_value(crate::segmentation::segment(&base64, &mime_type, worker_token)?)?)
        }).await.map_err(|_| Error::Generation("segmentation worker failed".into()))?,
        request => tokio::task::spawn_blocking(move || {
            if worker_token.is_cancelled() { return Err(Error::Generation("operation cancelled".into())); }
            execute(request, profile)
        }).await
            .map_err(|_| Error::Generation("core worker failed".into()))?,
    }?;
    if cancellation.is_cancelled() { return Err(Error::Generation("operation cancelled".into())); }
    checked_response(result, profile)
}

fn capacity_profile(value: &Value) -> Result<crate::limits::CapacityProfile> {
    match value.get("capacity_profile") {
        None => Ok(crate::limits::CapacityProfile::Large),
        Some(Value::String(profile)) if profile == "large" => Ok(crate::limits::CapacityProfile::Large),
        Some(Value::String(profile)) if profile == "standard" => Ok(crate::limits::CapacityProfile::Standard),
        Some(Value::String(profile)) if profile == "legacy" => Ok(crate::limits::CapacityProfile::Legacy),
        _ => Err(Error::Unsupported("unknown capacity profile; expected large, standard or legacy".into())),
    }
}

fn checked_response(value: Value, profile: crate::limits::CapacityProfile) -> Result<Value> {
    crate::preflight::value(&value, profile.limits(), None)?;
    crate::preflight::serialized_bytes(&value, profile.limits().request_bytes, "JSON response")?;
    if let Some(encoded) = value.get("base64").and_then(Value::as_str) {
        crate::preflight::archive_admission(encoded, profile.limits().archive_bytes)?;
    }
    Ok(value)
}

fn parse_request(mut value: Value, profile: crate::limits::CapacityProfile, cancellation: Option<&CancellationToken>) -> Result<Request> {
    crate::preflight::value(&value, profile.limits(), cancellation)?;
    crate::preflight::serialized_bytes(&value, profile.limits().request_bytes, "JSON request")?;
    if let Some(encoded) = value.get("base64").and_then(Value::as_str) {
        crate::preflight::archive_admission(encoded, profile.limits().archive_bytes)?;
    }
    if let Some(object) = value.as_object_mut() { object.remove("capacity_profile"); }
    for pointer in ["/document", "/checkpoint/document"] {
        if let Some(document) = value.pointer_mut(pointer).and_then(Value::as_object_mut) {
            document.insert("capacity_profile".into(), serde_json::to_value(profile)?);
        }
    }
    Ok(serde_json::from_value(value)?)
}

fn execute(request: Request, profile: crate::limits::CapacityProfile) -> Result<Value> {
    match request {
        Request::CapacityProfiles {} => Ok(json!({"default":"large", "legacy":crate::limits::LEGACY, "standard":crate::limits::STANDARD, "large":crate::limits::LARGE})),
        Request::InspectFont { base64 } => Ok(serde_json::to_value(crate::fonts::inspect_base64(&base64)?)?),
        Request::InspectPptxFonts { base64 } => Ok(serde_json::json!({"fonts":crate::fonts::inspect_pptx(STANDARD.decode(base64).map_err(|_|Error::Invalid("PPTX base64".into()))?)?,"office_verified":false})),
        Request::ListFonts { deck } => Ok(serde_json::json!({"fonts":crate::fonts::infos(&deck)?,"office_verified":false})),
        Request::EmbedFont { document, expected_revision, base64, license_acknowledged } => Ok(serde_json::to_value(crate::fonts::embed(&document, expected_revision, base64, license_acknowledged)?)?),
        Request::SetFontUsage { document, expected_revision, sha256, license_acknowledged } => Ok(serde_json::to_value(crate::fonts::set_usage(&document, expected_revision, &sha256, license_acknowledged)?)?),
        Request::Sample {} => Ok(serde_json::to_value(sample_report())?),
        Request::ProviderStatus {} => Ok(serde_json::to_value(provider_status())?),
        Request::Generate { input } => Ok(serde_json::to_value(generate_configured(&input)?)?),
        Request::TextAssist { input } => Ok(serde_json::to_value(crate::text_assist::configured(&input, CancellationToken::new())?)?),
        Request::ApplyTextAssist { document, expected_revision, slide_id, id, expected_text, candidate } => Ok(serde_json::to_value(crate::text_assist::apply(&document, expected_revision, &slide_id, &id, &expected_text, &candidate)?)?),
        Request::Compile { report } => Ok(serde_json::to_value(compile_report(&report)?)?),
        Request::CreatePicture { id, base64, mime_type, alt } => Ok(serde_json::to_value(crate::media::create_picture(&id, base64, &mime_type, &alt)?)?),
        Request::InspectRaster { base64, mime_type } => Ok(serde_json::to_value(crate::media::inspect_raster(&base64, &mime_type)?)?),
        Request::CreateAsset { id, base64, mime_type, alt, size } => Ok(serde_json::to_value(crate::media::create_asset(&id, base64, &mime_type, &alt, size)?)?),
        Request::CreateDiagram { id, steps } => Ok(serde_json::to_value(crate::graphics::create_diagram(&id, &steps)?)?),
        Request::GraphCatalog {} => Ok(crate::graphs::catalog()),
        Request::ArchitectureIcons {} => Ok(serde_json::to_value(crate::architecture_icons::catalog()?)?),
        Request::ArchitectureIconAssets { ids } => Ok(serde_json::to_value(crate::architecture_icons::assets(&ids)?)?),
        Request::CreateGraphIcon { base64, mime_type, alt } => Ok(serde_json::to_value(crate::graphs::create_icon(base64, &mime_type, &alt)?)?),
        Request::CreateGraph { id, spec, theme, include_diagnostics } => {
            let theme = theme.unwrap_or_default();
            let element = crate::graphs::create(&id, &spec, &theme)?;
            if include_diagnostics { Ok(serde_json::json!({"diagnostics":crate::graphs::diagnostics(&id, &spec, &element, &theme),"element":element})) }
            else { Ok(serde_json::to_value(element)?) }
        },
        Request::InsertGraph { document, expected_revision, slide_id, id, spec } => Ok(serde_json::to_value(crate::graphs::change(&document, expected_revision, &slide_id, &id, &spec, false)?)?),
        Request::UpdateGraph { document, expected_revision, slide_id, id, spec } => Ok(serde_json::to_value(crate::graphs::change(&document, expected_revision, &slide_id, &id, &spec, true)?)?),
        Request::TransformGraph { spec, operations } => Ok(serde_json::to_value(crate::graphs::transform(&spec, &operations)?)?),
        Request::ApplyGraph { document, expected_revision, slide_id, id, operations } => Ok(serde_json::to_value(crate::graphs::apply(&document, expected_revision, &slide_id, &id, &operations)?)?),
        Request::Ingest { input } => Ok(serde_json::to_value(crate::sources::ingest(input)?)?),
        Request::OcrStatus {} => Ok(serde_json::to_value(crate::extraction::ocr_status())?),
        Request::DesignDefaults {} => Ok(serde_json::to_value(crate::design::Design::default())?),
        Request::DesignPresets {} => Ok(serde_json::to_value(crate::design_presets::catalog()?)?),
        Request::ApplyDesignPreset { deck, preset_id } => Ok(serde_json::to_value(crate::design_presets::apply(deck, &preset_id)?)?),
        Request::ObjectCatalog {} => Ok(crate::objects::catalog()),
        Request::ComputeChartPresentation { kind, categories, series, options } => Ok(serde_json::to_value(crate::model::chart_format::compute_chart_presentation(kind, &categories, &series, &options)?)?),
        Request::RenderElementPreview { element, theme } => crate::render::render_element_preview(&element, theme.as_ref()),
        Request::CreatePart { id, spec, theme } => Ok(serde_json::to_value(crate::parts::create_with_theme(&id, &spec, &theme.unwrap_or_default())?)?),
        Request::PartCatalog {} => Ok(crate::parts::catalog()),
        Request::BestPracticeProfiles {} => Ok(crate::guided::profiles()),
        Request::BestPracticeGuide { profile_id } => crate::guided::guide(&profile_id),
        Request::ValidateGuidedPresentation { input } => Ok(serde_json::to_value(crate::guided::validate(&input))?),
        Request::CreateGuidedPresentation { id, input } => crate::guided::create(&id,&input),
        Request::InsertPart { document, expected_revision, slide_id, id, spec } => Ok(serde_json::to_value(crate::parts::state::change(&document,expected_revision,&slide_id,&id,&spec,false)?)?),
        Request::UpdatePart { document, expected_revision, slide_id, id, spec } => Ok(serde_json::to_value(crate::parts::state::change(&document,expected_revision,&slide_id,&id,&spec,true)?)?),
        Request::CreateObject { id, kind, preset, rows, columns } => Ok(serde_json::to_value(crate::objects::create(id, kind, preset, rows, columns)?)?),
        Request::UpdateDesign { deck, design } => Ok(serde_json::to_value(crate::design::update_design(deck, design)?)?),
        Request::ApplyTheme { deck, theme } => Ok(serde_json::to_value(crate::design::apply_theme(deck, theme)?)?),
        Request::SetMasterTheme { deck, master_id, theme } => Ok(serde_json::to_value(crate::design::set_master_theme(deck, &master_id, theme)?)?),
        Request::SetDesignField { deck, field } => Ok(serde_json::to_value(crate::fields::set_design_field(deck, field)?)?),
        Request::DesignCapabilities {} => Ok(crate::fields::capabilities()),
        Request::AssignLayout { deck, slide_id, layout_id, preserve_freeform } => Ok(serde_json::to_value(crate::design::assign_layout_with_options(deck, &slide_id, &layout_id, preserve_freeform)?)?),
        Request::DataReport { source, mapping } => Ok(serde_json::to_value(crate::data_report::data_report(&source, &mapping)?)?),
        Request::NewDocument { id, deck, sources, bindings, report } => Ok(serde_json::to_value(crate::document::create_with_profile(id, deck, sources, bindings, report, profile)?)?),
        Request::CreatePresentation { id, title } => Ok(serde_json::to_value(crate::editing::create(id, title)?)?),
        Request::EditSlides { document, expected_revision, operations } => Ok(serde_json::to_value(crate::editing::slides(&document, expected_revision, &operations)?)?),
        Request::EditElements { document, expected_revision, slide_id, operations } => Ok(serde_json::to_value(crate::editing::elements(&document, expected_revision, &slide_id, &operations)?)?),
        Request::ApplyOperations { document, expected_revision, expected_hash, operations } => Ok(serde_json::to_value(crate::authoring_batch::apply_operations(&document, expected_revision, &expected_hash, &operations)?)?),
        Request::ImportSlides { document, expected_revision, expected_hash, mut source, source_slide_ids, prefix, after } => {
            source.capacity_profile = profile;
            Ok(serde_json::to_value(crate::slide_import::import(&document, expected_revision, &expected_hash, &source, &source_slide_ids, &prefix, after.as_deref())?)?)
        }
        Request::SearchText { deck, options } => Ok(serde_json::to_value(crate::text_ops::search(&deck, &options)?)?),
        Request::ImportProofingDictionary { language, format, content } => Ok(serde_json::to_value(crate::proofing::import(&language, format, &content)?)?),
        Request::CopyFormat { document, slide_id, id, paragraph_index, run_index } => Ok(serde_json::to_value(crate::authoring_ops::copy_format(&document, &slide_id, &id, paragraph_index, run_index)?)?),
        Request::SampleSlidePixel { document, slide_id, x, y } => crate::visual::sample_slide_pixel(&document,&slide_id,x,y),
        Request::ApplyFormat { document, expected_revision, slide_id, ids, style } => Ok(serde_json::to_value(crate::authoring_ops::apply_format(&document, expected_revision, &slide_id, &ids, &style)?)?),
        Request::SetProofingLanguage { document, expected_revision, slide_id, id, language } => Ok(serde_json::to_value(crate::authoring_ops::set_proofing_language(&document, expected_revision, &slide_id, &id, &language)?)?),
        Request::ProofText { text, language, dictionary, term, target_language } => Ok(serde_json::to_value(crate::proofing::proof(&text, &language, dictionary.as_ref(), term.as_deref(), target_language.as_deref())?)?),
        Request::ReplaceText { document, expected_revision, options } => Ok(serde_json::to_value(crate::authoring_ops::replace_text(&document, expected_revision, &options)?)?),
        Request::ReplaceFont { document, expected_revision, from, to } => Ok(serde_json::to_value(crate::authoring_ops::replace_font(&document, expected_revision, &from, &to)?)?),
        Request::AuthoringCapabilities {} => Ok(crate::authoring_ops::capabilities()),
        Request::FormatTextElement { element, start, end, style } => {
            Ok(serde_json::to_value(crate::authoring_ops::edit_text_element(element, |element| crate::rich_text::apply_range(element, start, end, style))?)?)
        }
        Request::UpdateRichNotes { document, expected_revision, slide_id, paragraphs } => Ok(serde_json::to_value(crate::authoring_ops::update_rich_notes(&document, expected_revision, &slide_id, paragraphs)?)?),
        Request::UpdateAuxiliaryDesign { document, expected_revision, design } => Ok(serde_json::to_value(crate::authoring_ops::update_auxiliary_design(&document, expected_revision, design)?)?),
        Request::RefreshFields { document, expected_revision, reference_date, reference_time, locale } => {
            let locale = locale.as_deref().unwrap_or("en-US");
            let warnings = crate::fields::evaluation_warnings(&document.deck, reference_time.as_deref(), locale);
            let mut result = serde_json::to_value(crate::authoring_ops::refresh_fields_with_options(&document, expected_revision, &reference_date, reference_time.as_deref(), locale)?)?;
            result["warnings"] = serde_json::to_value(warnings)?;
            Ok(result)
        },
        Request::AddComment { document, expected_revision, slide_id, comment } => Ok(serde_json::to_value(crate::authoring_ops::update_review(&document, expected_revision, |deck| crate::comments::add(deck, &slide_id, comment))?)?),
        Request::ModernComment { document, expected_revision, slide_id, operation } => Ok(serde_json::to_value(crate::authoring_ops::update_review(&document, expected_revision, |deck| crate::comments::modern::apply(deck, &slide_id, operation))?)?),
        Request::SetTableHeaders { document, expected_revision, slide_id, element_id, policy } => Ok(serde_json::to_value(crate::authoring_ops::update_review(&document, expected_revision, |deck| crate::review::set_table_headers(deck, &slide_id, &element_id, policy))?)?),
        Request::ReplyComment { document, expected_revision, slide_id, parent_id, comment } => Ok(serde_json::to_value(crate::authoring_ops::update_review(&document, expected_revision, |deck| crate::comments::reply(deck, &slide_id, &parent_id, comment))?)?),
        Request::ResolveComment { document, expected_revision, slide_id, comment_id, resolved } => Ok(serde_json::to_value(crate::authoring_ops::update_review(&document, expected_revision, |deck| crate::comments::resolve(deck, &slide_id, &comment_id, resolved))?)?),
        Request::RemoveComment { document, expected_revision, slide_id, comment_id } => Ok(serde_json::to_value(crate::authoring_ops::update_review(&document, expected_revision, |deck| crate::comments::remove(deck, &slide_id, &comment_id))?)?),
        Request::SetAccessibility { document, expected_revision, slide_id, element_id, metadata } => Ok(serde_json::to_value(crate::authoring_ops::update_review(&document, expected_revision, |deck| crate::review::set_accessibility(deck, &slide_id, &element_id, metadata))?)?),
        Request::SetReadingOrder { document, expected_revision, slide_id, order } => {
            let mut result = serde_json::to_value(crate::authoring_ops::update_review(&document, expected_revision, |deck| crate::review::set_reading_order(deck, &slide_id, order))?)?;
            result["warnings"] = json!(["Reading order also changes native z-order; review overlaps and visual appearance."]);
            Ok(result)
        }
        Request::CheckAccessibility { document } => {
            crate::document::verify(&document)?;
            Ok(serde_json::to_value(crate::review::check_accessibility(&document.deck))?)
        }
        Request::InspectDocument { document } => Ok(serde_json::to_value(crate::review::inspect_document(&document)?)?),
        Request::ExportCleanCopy { document, options } => {
            let clean = crate::review::export_clean_copy(&document, &options)?;
            let mut result = encoded(clean.bytes)?;
            result["document"] = serde_json::to_value(clean.document)?;
            result["inspection"] = serde_json::to_value(clean.inspection)?;
            Ok(result)
        }
        Request::ReplaceElementText { element, text } => {
            Ok(serde_json::to_value(crate::authoring_ops::edit_text_element(element, |element| crate::rich_text::replace_text_content(element, text))?)?)
        }
        Request::SetTableCellText { element, row, column, text } => Ok(serde_json::to_value(crate::authoring_ops::edit_text_element(element, |element| crate::table_format::replace_cell_text(element, row, column, text))?)?),
        Request::EditVector { element, path } => Ok(serde_json::to_value(crate::vector::edit_path(element, path)?)?),
        Request::CombineShapes { document, expected_revision, slide_id, ids, operation, result_id } => Ok(serde_json::to_value(crate::authoring_ops::combine_shapes(&document, expected_revision, &slide_id, &ids, operation, &result_id)?)?),
        Request::FormatText { document, expected_revision, slide_id, id, start, end, style } => Ok(serde_json::to_value(crate::authoring_ops::format_text(&document, expected_revision, &slide_id, &id, start, end, style)?)?),
        Request::ReplaceTextContent { document, expected_revision, slide_id, id, text } => Ok(serde_json::to_value(crate::authoring_ops::replace_text_content(&document, expected_revision, &slide_id, &id, text)?)?),
        Request::UpdateParagraphs { document, expected_revision, slide_id, id, paragraphs } => Ok(serde_json::to_value(crate::authoring_ops::update_paragraphs(&document, expected_revision, &slide_id, &id, paragraphs)?)?),
        Request::EditSelection { document, expected_revision, slide_id, operation, clipboard } => Ok(serde_json::to_value(crate::authoring_ops::edit_selection(&document, expected_revision, &slide_id, &operation, clipboard.as_ref())?)?),
        Request::ResizeCanvas { document, expected_revision, width, height, mode } => Ok(serde_json::to_value(crate::authoring_ops::resize_canvas(&document, expected_revision, width, height, mode)?)?),
        Request::ImportTemplate { id, kind, base64 } => Ok(serde_json::to_value(crate::authoring_ops::import_template(id, kind, decode(&base64)?)?)?),
        Request::InspectMasterSource { kind, base64 } => crate::master_import::inspect(kind, &base64, profile),
        Request::PreviewMasterImport { document, expected_revision, expected_hash, input } => crate::master_import::preview(&document, expected_revision, &expected_hash, &input),
        Request::ImportMasters { document, expected_revision, expected_hash, input, expected_candidate_hash } => Ok(serde_json::to_value(crate::master_import::import(&document, expected_revision, &expected_hash, &input, &expected_candidate_hash)?)?),
        Request::ExportTemplate { document, kind } => {
            let mut result = encoded(crate::authoring_ops::export_template(&document, kind)?)?;
            result["filename"] = json!(match kind { crate::authoring_ops::TemplateKind::Potx => "template.potx", crate::authoring_ops::TemplateKind::Thmx => "template.thmx" });
            Ok(result)
        }
        Request::EditImage { base64, mime_type, params } => Ok(serde_json::to_value(crate::image_edit::edit_image(&base64, &mime_type, &params)?)?),
        Request::SegmentationStatus {} => Ok(serde_json::to_value(crate::segmentation::status())?),
        Request::SegmentImage { base64, mime_type } => Ok(serde_json::to_value(crate::segmentation::segment(&base64, &mime_type, CancellationToken::new())?)?),
        Request::ApplyImageEdit { document, expected_revision, slide_id, id, image } => Ok(serde_json::to_value(crate::authoring_ops::apply_image_edit(&document, expected_revision, &slide_id, &id, image)?)?),
        Request::EditTable { document, expected_revision, slide_id, id, operations } => Ok(serde_json::to_value(crate::authoring_ops::edit_table(&document, expected_revision, &slide_id, &id, operations)?)?),
        Request::Transaction { document, transaction } => Ok(serde_json::to_value(crate::authoring_ops::review_transaction(&document, transaction)?)?),
        Request::UndoTransaction { document, expected_revision, receipt } => Ok(serde_json::to_value(crate::document::undo(&document, expected_revision, receipt)?)?),
        Request::ExportProject { document } => crate::document::export(&document),
        Request::OpenProject { base64, checkpoint } => Ok(serde_json::to_value(crate::document::open(&base64, checkpoint)?)?),
        Request::OpenPresentation { id, base64 } => crate::document::open_presentation_with_profile(id, decode(&base64)?, profile),
        Request::ExportPresentation { document } => crate::document::export_presentation(&document),
        Request::PreviewPresentation { document, options } => Ok(serde_json::to_value(crate::export_static::preview_presentation(&document, &options)?)?),
        Request::PreflightPresentation { document, options } => Ok(serde_json::to_value(crate::authoring_preflight::preflight_presentation(&document, &options)?)?),
        Request::PrepareDelivery { document, expected_revision, expected_hash, options } => Ok(serde_json::to_value(crate::delivery::prepare_delivery(&document, expected_revision, &expected_hash, &options)?)?),
        Request::PreviewSlideRevision { document, expected_revision, expected_hash, slide_id, edits, max_dimension } => Ok(serde_json::to_value(crate::slide_revision::preview_slide_revision(&document, expected_revision, &expected_hash, &slide_id, &edits, max_dimension)?)?),
        Request::ApplySlideRevision { document, expected_revision, expected_hash, slide_id, edits, candidate_hash } => Ok(serde_json::to_value(crate::slide_revision::apply_slide_revision(&document, expected_revision, &expected_hash, &slide_id, &edits, &candidate_hash)?)?),
        Request::VerifyRecovery { document } => {
            crate::document::verify(&document)?;
            Ok(serde_json::to_value(document)?)
        }
        Request::VerifySessionRecovery { envelope } => Ok(serde_json::to_value(crate::document::verify_session_recovery(envelope)?)?),
        Request::PrepareRecovery { state, expected_generation, action, now_ms } => Ok(serde_json::to_value(crate::recovery::prepare(state, expected_generation, action, now_ms)?)?),
        Request::VerifyRecoveryRecord { entry, payload } => Ok(serde_json::to_value(crate::recovery::verify_record(&entry, &payload)?)?),
        Request::ExportStatic { document, options } => {
            crate::document::verify(&document)?;
            if document.bindings.iter().any(|binding| binding.stale) {
                return Err(Error::Conflict("source bindings are stale; undo or rebind before export".into()));
            }
            let output = crate::export_static::export_static(&document.deck, &options)?;
            let total: usize = output.artifacts.iter().map(|artifact| artifact.bytes.len()).sum();
            if total > crate::limits::LEGACY.request_bytes * 3 / 4 - 1024 {
                return Err(Error::Limit("static output bundle too large for the 4 MiB JSON protocol; reduce scale or select fewer pages".into()));
            }
            let files: Vec<Value> = output.artifacts.into_iter().map(|artifact| {
                let filename = match options.format {
                    crate::export_static::ExportFormat::Pdf => "report.pdf".to_string(),
                    format => format!("report-page-{:03}.{}", artifact.page_indices[0] + 1,
                        if format == crate::export_static::ExportFormat::Png { "png" } else { "jpg" }),
                };
                json!({"filename":filename,"byte_length":artifact.bytes.len(),"base64":STANDARD.encode(artifact.bytes),
                    "mime_type":artifact.mime_type,"page_indices":artifact.page_indices,"width":artifact.width,"height":artifact.height})
            }).collect();
            Ok(json!({"files":files,"warnings":output.warnings,"office_parity_verified":output.office_parity_verified,
                "pdf_rasterized":output.pdf_rasterized,"pdf_text_outlined":options.format == crate::export_static::ExportFormat::Pdf,
                "pdf_editable_text":false,"pdf_tagged":options.format == crate::export_static::ExportFormat::Pdf,
                "pdf_searchable_text":options.format == crate::export_static::ExportFormat::Pdf,
                "pdf_selectable_text":options.format == crate::export_static::ExportFormat::Pdf,
                "pdf_semantic_overlay":options.format == crate::export_static::ExportFormat::Pdf,
                "pdf_ua_certified":false}))
        }
        Request::Export { deck } => encoded(export_pptx(&deck)?),
        Request::MeasureLayout { deck } => Ok(serde_json::to_value(crate::layout::measure_layout(&deck)?)?),
        Request::Validate { deck } => {
            validate_deck(&deck)?;
            Ok(json!({"structurally_valid":true,"slides":deck.slides.len(),"deck":deck,"warnings":["Only structure and geometry were checked. Text fit and PowerPoint visual parity require separate validation."]}))
        }
        Request::Inspect { base64 } => Ok(serde_json::to_value(inspect_pptx(decode(&base64)?)?)?),
        Request::ImportPptx { base64 } => Ok(serde_json::to_value(crate::import::import_pptx(decode(&base64)?)?)?),
        Request::ImportDocument { id, base64 } => crate::document::import_document_with_profile(id, decode(&base64)?, profile),
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
    crate::preflight::decode_archive(value, crate::limits::LARGE.archive_bytes)
}

fn encoded(bytes: Vec<u8>) -> Result<Value> {
    if bytes.len() > crate::limits::LARGE.archive_bytes {
        return Err(Error::Limit("archive exceeds the 16 MiB binary protocol budget".into()));
    }
    Ok(json!({"base64":STANDARD.encode(bytes),"filename":"report.pptx"}))
}