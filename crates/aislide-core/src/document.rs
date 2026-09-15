use crate::{data_report::SourceBinding, model::{Deck, element_list, valid_text, validate_deck}, report::ReportInput, sources::{SourceDocument, validate_source}, Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use json_patch::Patch;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
const MAX_REVISION: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub version: u32, pub id: String, pub revision: u64, pub hash: String,
    pub deck: Deck, pub sources: Vec<SourceDocument>, pub bindings: Vec<SourceBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<crate::parts::state::PartInstance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<ReportInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<ImportedOrigin>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedOrigin { pub base64: String, pub sha256: String, #[serde(default, skip_serializing_if = "std::ops::Not::not")] pub native: bool }

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Content { deck: Deck, sources: Vec<SourceDocument>, bindings: Vec<SourceBinding>, #[serde(default, skip_serializing_if = "Vec::is_empty")] parts: Vec<crate::parts::state::PartInstance>, #[serde(default, skip_serializing_if = "Option::is_none")] report: Option<ReportInput>, #[serde(default, skip_serializing_if = "Option::is_none")] origin: Option<ImportedOrigin> }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transaction { pub expected_revision: u64, pub expected_hash: String, pub operations: Patch }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UndoReceipt { pub document_id: String, pub after_hash: String, pub inverse: Patch }

#[derive(Serialize)]
pub struct TransactionResult { pub document: Document, pub receipt: Option<UndoReceipt>, pub changes: Vec<String> }

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint { pub format: String, pub version: u32, pub pptx_sha256: String, pub document: Document }

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn content(document: &Document) -> Content { Content { deck: document.deck.clone(), sources: document.sources.clone(), bindings: document.bindings.clone(), parts: document.parts.clone(), report: document.report.clone(), origin: document.origin.clone() } }

fn seal(document: &mut Document) -> Result<()> {
    valid_text(&document.id, 80)?;
    if document.version != 1 || document.id.is_empty() || document.revision > MAX_REVISION || document.sources.len() > 8 || document.bindings.len() > 4096 { return Err(Error::Invalid("document version, identity, revision or resource count is invalid".into())); }
    if serde_json::to_vec(&content(document))?.len() > MAX_DOCUMENT_BYTES { return Err(Error::Limit("document content > 2 MiB".into())); }
    validate_deck(&document.deck)?;
    crate::parts::state::refresh(&mut document.parts,&document.deck,document.origin.as_ref())?;
    if let Some(origin) = &document.origin {
        let bytes = STANDARD.decode(&origin.base64).map_err(|_| Error::Invalid("invalid imported origin base64".into()))?;
        if digest(&bytes) != origin.sha256 { return Err(Error::Conflict("import origin hash mismatch".into())); }
        if origin.native { crate::native::save(bytes, &document.deck)?; } else { crate::import::save_import(bytes, &document.deck)?; }
    }
    if let Some(report) = &document.report { crate::report::compile_report(report)?; }
    let mut ids = BTreeSet::new();
    let mut cells = BTreeMap::new();
    for source in &document.sources {
        validate_source(source)?;
        if !ids.insert(&source.id) { return Err(Error::Invalid("duplicate source identity".into())); }
        for table in &source.tables {
            for (row, locators) in table.rows.iter().zip(&table.locators) {
                for (value, locator) in row.iter().zip(locators) { cells.insert((source.id.as_str(), locator.as_str()), value); }
            }
        }
    }
    let mut elements = BTreeMap::new();
    for slide in &document.deck.slides { for element in element_list(&slide.elements) { elements.insert((slide.id.as_str(), element.bounds().0), serde_json::to_value(element)?); } }
    for binding in &mut document.bindings {
        valid_text(&binding.field, 512)?;
        let source = document.sources.iter().find(|source| source.id == binding.source_id && source.sha256 == binding.source_sha256).ok_or_else(|| Error::Conflict("source binding refers to an unknown source".into()))?;
        let raw = cells.get(&(source.id.as_str(), binding.locator.as_str())).ok_or_else(|| Error::Conflict("source binding locator does not exist".into()))?;
        if !crate::canonical::equal(raw, &binding.raw_value) { return Err(Error::Conflict("source binding no longer matches its recorded cell".into())); }
        let expected = match binding.transform.as_str() {
            "strict_numeric" => json!(crate::data_report::number(raw)?),
            "display_scalar" => json!(crate::data_report::display(raw)),
            _ => return Err(Error::Unsupported("unrecognized source binding transform".into())),
        };
        if !crate::canonical::equal(&expected, &binding.value) { return Err(Error::Conflict("source binding transform does not match its value".into())); }
        binding.stale = !elements.get(&(binding.slide_id.as_str(), binding.element_id.as_str())).and_then(|element| element.pointer(&binding.field)).is_some_and(|value| crate::canonical::equal(value, &binding.value));
    }
    document.hash = digest(&crate::canonical::bytes(&content(document))?);
    Ok(())
}

pub fn verify(document: &Document) -> Result<()> {
    let mut checked = document.clone(); seal(&mut checked)?;
    if checked.hash != document.hash { return Err(Error::Conflict("document content has changed outside a transaction".into())); }
    Ok(())
}

pub fn create(id: String, deck: Deck, sources: Vec<SourceDocument>, bindings: Vec<SourceBinding>, report: Option<ReportInput>) -> Result<Document> {
    let mut document = Document { version: 1, id, revision: 0, hash: String::new(), deck, sources, bindings, parts: Vec::new(), report, origin: None };
    seal(&mut document)?; Ok(document)
}

pub fn import_document(id: String, bytes: Vec<u8>) -> Result<Value> {
    let imported = crate::import::import_pptx(bytes.clone())?;
    let mut document = Document { version: 1, id, revision: 0, hash: String::new(), deck: imported.deck, sources: Vec::new(), bindings: Vec::new(), parts: Vec::new(), report: None,
        origin: Some(ImportedOrigin { base64: STANDARD.encode(&bytes), sha256: digest(&bytes), native: false }) };
    seal(&mut document)?;
    Ok(json!({"document":document,"warnings":imported.warnings,"objects":imported.objects}))
}

pub fn open_presentation(id: String, bytes: Vec<u8>) -> Result<Value> {
    let package = crate::package::Package::open(bytes.clone())?;
    let imported = crate::native::read(&package)?;
    let (sources, bindings, parts) = imported.metadata.map(|metadata| (metadata.sources, metadata.bindings, metadata.parts)).unwrap_or_default();
    let mut document = Document { version: 1, id, revision: 0, hash: String::new(), deck: imported.deck, sources, bindings, parts, report: None,
        origin: Some(ImportedOrigin { base64: STANDARD.encode(&bytes), sha256: digest(&bytes), native: true }) };
    seal(&mut document)?;
    Ok(json!({"document":document,"warnings":imported.warnings,"objects":[],"format":"open_xml_pptx"}))
}

pub fn transact(document: &Document, transaction: Transaction) -> Result<TransactionResult> {
    verify(document)?;
    if document.revision != transaction.expected_revision || document.hash != transaction.expected_hash { return Err(Error::Conflict("stale document revision or content hash".into())); }
    if transaction.operations.0.is_empty() || transaction.operations.0.len() > 128 { return Err(Error::Limit("transaction requires 1-128 operations".into())); }
    let original = serde_json::to_value(content(document))?;
    let mut working = original.clone();
    for operation in &transaction.operations.0 {
        let encoded = serde_json::to_value(operation)?;
        for field in ["path", "from"] { if encoded.get(field).and_then(Value::as_str).is_some_and(|path| path.len() > 2048) { return Err(Error::Limit("transaction path > 2048 bytes".into())); } }
        json_patch::patch(&mut working, &Patch(vec![operation.clone()])).map_err(|_| Error::Conflict("transaction precondition or path failed; no changes applied".into()))?;
        if serde_json::to_vec(&working)?.len() > MAX_DOCUMENT_BYTES { return Err(Error::Limit("transaction would exceed the 2 MiB document budget".into())); }
    }
    let next: Content = serde_json::from_value(working)?;
    let mut updated = Document { version: document.version, id: document.id.clone(), revision: document.revision, hash: String::new(), deck: next.deck, sources: next.sources, bindings: next.bindings, parts: next.parts, report: next.report, origin: next.origin };
    if document.origin.is_some() && crate::canonical::bytes(&document.origin)? != crate::canonical::bytes(&updated.origin)? { return Err(Error::Unsupported("import origin cannot be detached or replaced inside an edit transaction".into())); }
    updated.parts.retain(|part| {
        let contains = |deck: &Deck| deck.slides.iter().any(|slide| slide.id == part.slide_id && slide.elements.iter().any(|element| element.bounds().0 == part.element_id));
        contains(&updated.deck) || !contains(&document.deck)
    });
    seal(&mut updated)?;
    if updated.hash == document.hash { return Ok(TransactionResult { document: document.clone(), receipt: None, changes: Vec::new() }); }
    updated.revision = document.revision.checked_add(1).filter(|revision| *revision <= MAX_REVISION).ok_or_else(|| Error::Limit("document revision exhausted".into()))?;
    let updated_content = serde_json::to_value(content(&updated))?;
    let mut inverse = json_patch::diff(&updated_content, &original);
    if inverse.0.len() > 128 { inverse = serde_json::from_value(json!([{"op":"replace","path":"","value":original}]))?; }
    let changes = serde_json::to_value(&transaction.operations)?.as_array().into_iter().flatten().filter_map(|operation| operation["path"].as_str().map(String::from)).collect();
    let receipt = UndoReceipt { document_id: updated.id.clone(), after_hash: updated.hash.clone(), inverse };
    Ok(TransactionResult { document: updated, receipt: Some(receipt), changes })
}

pub fn undo(document: &Document, expected_revision: u64, receipt: UndoReceipt) -> Result<TransactionResult> {
    if document.id != receipt.document_id || document.hash != receipt.after_hash { return Err(Error::Conflict("undo receipt does not match current document content".into())); }
    transact(document, Transaction { expected_revision, expected_hash: receipt.after_hash, operations: receipt.inverse })
}

fn presentation(document: &Document) -> Result<Vec<u8>> {
    if let Some(origin) = &document.origin {
        let bytes = STANDARD.decode(&origin.base64).map_err(|_| Error::Invalid("invalid imported origin".into()))?;
        if origin.native { crate::native::save(bytes, &document.deck) } else { crate::import::save_import(bytes, &document.deck) }
    }
    else { crate::pptx::export_pptx(&document.deck) }
}

pub fn export(document: &Document) -> Result<Value> {
    verify(document)?;
    if document.bindings.iter().any(|binding| binding.stale) { return Err(Error::Conflict("source bindings are stale; undo data edits or explicitly remove/rebind affected citations before export".into())); }
    let layout = if document.origin.is_none() { Some(crate::layout::measure_layout(&document.deck)?) } else { None };
    if let Some(issue) = layout.as_ref().and_then(|report| report.issues.iter().find(|issue| issue.severity == "error")) {
        return Err(Error::Invalid(format!("project export blocked by {}: {}", issue.code, issue.message)));
    }
    let bytes = presentation(document)?;
    let checkpoint = Checkpoint { format: "aislide.project".into(), version: 1, pptx_sha256: digest(&bytes), document: document.clone() };
    Ok(json!({"base64":STANDARD.encode(bytes),"filename":"report.pptx","checkpoint":checkpoint,"checkpoint_filename":"report.aislide.json","layout":layout}))
}

pub fn open(base64: &str, checkpoint: Checkpoint) -> Result<Document> {
    if checkpoint.format != "aislide.project" || checkpoint.version != 1 { return Err(Error::Unsupported("project checkpoint version".into())); }
    let bytes = STANDARD.decode(base64).map_err(|_| Error::Invalid("invalid project PPTX base64".into()))?;
    if digest(&bytes) != checkpoint.pptx_sha256 { return Err(Error::Conflict("PPTX does not match this checkpoint".into())); }
    verify(&checkpoint.document)?;
    if presentation(&checkpoint.document)? != bytes { return Err(Error::Conflict("checkpoint scene does not reproduce its bound PPTX".into())); }
    Ok(checkpoint.document)
}

pub fn export_presentation(document: &Document) -> Result<Value> {
    let mut result = export(document)?;
    let bytes = STANDARD.decode(result["base64"].as_str().ok_or_else(|| Error::Invalid("export bytes missing".into()))?).map_err(|_| Error::Invalid("export base64".into()))?;
    result["base64"] = json!(STANDARD.encode(crate::provenance::attach(bytes, document)?));
    if let Some(value) = result.as_object_mut() { value.remove("checkpoint"); value.remove("checkpoint_filename"); }
    Ok(result)
}