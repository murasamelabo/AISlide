use crate::{model::valid_text, package::Package, Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use calamine::{Data, Reader, Xlsx};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io::Cursor};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_ROWS: usize = 1000;
const MAX_COLUMNS: usize = 32;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat { Csv, Json, Xlsx, Markdown, Text, Pdf, Png, Jpeg }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceInput { pub name: String, pub format: SourceFormat, pub base64: String, #[serde(default)] pub ocr: bool, #[serde(default)] pub ocr_language: Option<String>, #[serde(default)] pub attribution: Option<Attribution> }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attribution { pub citation: String, pub url: String, pub license: String, #[serde(default)] pub derived_from_sha256: Option<String>, #[serde(default)] pub transformation: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceTable { pub name: String, pub columns: Vec<String>, pub rows: Vec<Vec<Value>>, pub locators: Vec<Vec<String>> }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDocument {
    pub version: u32, pub id: String, pub name: String, pub format: SourceFormat,
    pub sha256: String, pub byte_length: usize, pub content_sha256: String,
    pub tables: Vec<SourceTable>, pub text: String, pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raster: Option<crate::media::RasterInfo>,
    #[serde(default)]
    pub pages: Vec<crate::extraction::SourcePage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<Attribution>,
}

fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn content_hash(source: &SourceDocument) -> Result<String> {
    let mut canonical = source.clone(); canonical.content_sha256.clear();
    Ok(hash(&crate::canonical::bytes(&canonical)?))
}

fn utf8(bytes: &[u8]) -> Result<&str> {
    let text = std::str::from_utf8(bytes).map_err(|_| Error::Unsupported("text sources must use UTF-8".into()))?.trim_start_matches('\u{feff}');
    valid_text(text, MAX_BYTES)?;
    Ok(text)
}

fn valid_columns(columns: &[String]) -> Result<()> {
    if columns.is_empty() || columns.len() > MAX_COLUMNS { return Err(Error::Limit("source requires 1-32 columns".into())); }
    let mut seen = BTreeSet::new();
    for column in columns {
        valid_text(column, 80)?;
        if column.trim().is_empty() || !seen.insert(column) { return Err(Error::Invalid("source headers must be nonempty and unique".into())); }
    }
    Ok(())
}

fn scalar(value: &Value) -> Result<()> {
    match value {
        Value::String(text) => valid_text(text, 2000),
        Value::Bool(_) | Value::Null | Value::Number(_) => Ok(()),
        _ => Err(Error::Unsupported("source cells must be scalar values, not nested arrays or objects".into())),
    }
}

fn push_row(table: &mut SourceTable, row: Vec<Value>, locators: Vec<String>) -> Result<()> {
    if table.rows.len() >= MAX_ROWS || row.len() != table.columns.len() || locators.len() != row.len() { return Err(Error::Limit("source table must be rectangular with at most 1000 rows".into())); }
    for value in &row { scalar(value)?; }
    table.rows.push(row); table.locators.push(locators);
    Ok(())
}

pub fn ingest(input: SourceInput) -> Result<SourceDocument> {
    valid_text(&input.name, 200)?;
    if input.name.trim().is_empty() || input.base64.len() > MAX_BYTES.div_ceil(3) * 4 { return Err(Error::Limit("named source must be at most 2 MiB".into())); }
    let bytes = STANDARD.decode(&input.base64).map_err(|_| Error::Invalid("invalid source base64".into()))?;
    if bytes.is_empty() || bytes.len() > MAX_BYTES { return Err(Error::Limit("source must contain 1 byte to 2 MiB".into())); }
    if let Some(language) = &input.ocr_language { valid_text(language, 35)?; }
    if input.ocr && !matches!(input.format, SourceFormat::Png | SourceFormat::Jpeg) { return Err(Error::Unsupported("OCR is available for PNG/JPEG sources only; scanned PDF pages must be supplied as images".into())); }
    let digest = hash(&bytes);
    let mut source = SourceDocument { version: 1, id: format!("sha256:{digest}"), name: input.name, format: input.format, sha256: digest,
        byte_length: bytes.len(), content_sha256: String::new(), tables: Vec::new(), text: String::new(),
        warnings: vec!["Parsed source data is not independently fact-checked.".into()], raster: None, pages: Vec::new(), attribution: input.attribution };
    match input.format {
        SourceFormat::Csv => {
            let mut reader = csv::ReaderBuilder::new().from_reader(utf8(&bytes)?.as_bytes());
            let columns: Vec<String> = reader.headers().map_err(|_| Error::Invalid("invalid CSV header".into()))?.iter().map(String::from).collect();
            valid_columns(&columns)?;
            let mut table = SourceTable { name: "CSV".into(), columns, rows: Vec::new(), locators: Vec::new() };
            for (index, record) in reader.records().enumerate() {
                let record = record.map_err(|_| Error::Invalid("CSV record has invalid syntax or column count".into()))?;
                let locators = (0..record.len()).map(|column| format!("record:{}/column:{}", index + 2, column + 1)).collect();
                push_row(&mut table, record.iter().map(|value| Value::String(value.into())).collect(), locators)?;
            }
            source.tables.push(table);
        }
        SourceFormat::Json => {
            let value: Value = serde_json::from_str(utf8(&bytes)?)?;
            let rows = value.as_array().filter(|rows| !rows.is_empty()).ok_or_else(|| Error::Invalid("JSON source must be a nonempty array of objects or a header/row matrix".into()))?;
            let objects = rows[0].is_object();
            let columns = if let Some(first) = rows[0].as_object() { first.keys().cloned().collect() }
                else { rows[0].as_array().ok_or_else(|| Error::Invalid("invalid JSON table".into()))?.iter().map(|value| value.as_str().map(String::from).ok_or_else(|| Error::Invalid("JSON matrix headers must be strings".into()))).collect::<Result<Vec<_>>>()? };
            valid_columns(&columns)?;
            let mut table = SourceTable { name: "JSON".into(), columns, rows: Vec::new(), locators: Vec::new() };
            for (index, entry) in rows.iter().enumerate().skip(if objects { 0 } else { 1 }) {
                if objects {
                    let object = entry.as_object().ok_or_else(|| Error::Invalid("JSON records must all be objects".into()))?;
                    if object.keys().any(|key| !table.columns.contains(key)) { return Err(Error::Invalid("JSON records have inconsistent fields".into())); }
                    let values = table.columns.iter().map(|column| object.get(column).cloned().unwrap_or(Value::Null)).collect();
                    let locators = table.columns.iter().map(|column| format!("/{index}/{}", column.replace('~', "~0").replace('/', "~1"))).collect();
                    push_row(&mut table, values, locators)?;
                } else {
                    let values = entry.as_array().ok_or_else(|| Error::Invalid("JSON matrix rows must be arrays".into()))?.clone();
                    let locators = (0..values.len()).map(|column| format!("/{index}/{column}")).collect();
                    push_row(&mut table, values, locators)?;
                }
            }
            source.warnings.push("Missing JSON fields remain null; no numeric imputation is performed.".into());
            source.tables.push(table);
        }
        SourceFormat::Xlsx => {
            preflight_workbook(&bytes)?;
            let mut workbook = Xlsx::new(Cursor::new(bytes)).map_err(|_| Error::Invalid("invalid XLSX workbook".into()))?;
            let names = workbook.sheet_names().to_vec();
            if names.len() > 8 { return Err(Error::Limit("at most 8 worksheets are accepted".into())); }
            for name in names {
                let range = workbook.worksheet_range(&name).map_err(|_| Error::Invalid("worksheet could not be read".into()))?;
                let Some(first) = range.rows().next() else { continue };
                let columns = first.iter().map(ToString::to_string).collect::<Vec<_>>(); valid_columns(&columns)?;
                let (first_row, first_column) = range.start().unwrap_or((0, 0));
                let mut table = SourceTable { name: name.clone(), columns, rows: Vec::new(), locators: Vec::new() };
                for (index, row) in range.rows().enumerate().skip(1) {
                    let values = row.iter().map(|cell| match cell {
                        Data::Int(value) => json!(value), Data::Float(value) => json!(value), Data::Bool(value) => json!(value), Data::Empty => Value::Null,
                        Data::String(value) | Data::DateTimeIso(value) | Data::DurationIso(value) => Value::String(value.clone()),
                        Data::DateTime(value) => Value::String(value.to_string()), Data::Error(value) => Value::String(format!("Spreadsheet error: {value}")),
                    }).collect();
                    let locators = (0..row.len()).map(|column| format!("{name}!{}", cell_address(first_row as usize + index, first_column as usize + column))).collect();
                    push_row(&mut table, values, locators)?;
                }
                source.tables.push(table);
            }
            source.warnings.push("XLSX formulas are never evaluated; cached values and date representations require review.".into());
        }
        SourceFormat::Markdown | SourceFormat::Text => { source.text = utf8(&bytes)?.into(); valid_text(&source.text, 24000)?; }
        SourceFormat::Pdf => {
            source.pages = crate::extraction::pdf_text(&bytes)?;
            source.text = source.pages.iter().map(|page| format!("[{}]\n{}", page.locator, page.text)).collect::<Vec<_>>().join("\n\n");
            source.warnings.push("PDF text order and numeric claims require manual review. Images, actions and links are not executed; scanned pages require a separate image OCR step.".into());
        }
        SourceFormat::Png | SourceFormat::Jpeg => {
            source.raster = Some(crate::media::inspect_raster(&input.base64, if matches!(input.format, SourceFormat::Png) { "image/png" } else { "image/jpeg" })?);
            if input.ocr {
                let page = crate::extraction::image_text(&bytes, input.ocr_language.as_deref())?;
                source.text = page.text.clone(); source.pages.push(page);
                source.warnings.push("Local OCR text and word regions are unverified. Check every numerical claim against the original image before use.".into());
            } else { source.warnings.push("Image metadata was read; OCR has not been performed and no textual claims were extracted.".into()); }
        }
    }
    source.content_sha256 = content_hash(&source)?;
    validate_source(&source)?;
    Ok(source)
}

pub fn validate_source(source: &SourceDocument) -> Result<()> {
    if source.version != 1 || source.byte_length > MAX_BYTES || source.sha256.len() != 64 || !source.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) || source.id != format!("sha256:{}", source.sha256) { return Err(Error::Invalid("invalid source descriptor".into())); }
    valid_text(&source.name, 200)?; valid_text(&source.text, 24000)?;
    if let Some(attribution) = &source.attribution {
        valid_text(&attribution.citation, 1000)?; valid_text(&attribution.url, 2048)?; valid_text(&attribution.license, 500)?; valid_text(&attribution.transformation, 2000)?;
        if attribution.derived_from_sha256.as_ref().is_some_and(|hash| hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())) { return Err(Error::Invalid("invalid upstream source hash".into())); }
    }
    if source.tables.len() > 8 || source.warnings.len() > 20 || source.pages.len() > 32 { return Err(Error::Limit("source metadata limits".into())); }
    for page in &source.pages {
        valid_text(&page.text, 24000)?; valid_text(&page.locator, 300)?; valid_text(&page.method, 80)?;
        if page.regions.len() > 4000 { return Err(Error::Limit("source word regions > 4000".into())); }
        for region in &page.regions {
            valid_text(&region.text, 2000)?;
            if [region.x, region.y, region.width, region.height].iter().any(|value| !value.is_finite() || *value < 0.0 || *value > 4096.0) { return Err(Error::Invalid("OCR region outside image budget".into())); }
        }
    }
    let mut cells = 0;
    for table in &source.tables {
        valid_text(&table.name, 200)?; valid_columns(&table.columns)?;
        if table.rows.len() > MAX_ROWS || table.rows.len() != table.locators.len() { return Err(Error::Invalid("invalid source row count or locators".into())); }
        for (row, locators) in table.rows.iter().zip(&table.locators) {
            cells += row.len();
            if row.len() != table.columns.len() || locators.len() != row.len() || cells > 16000 { return Err(Error::Limit("source must be rectangular and contain at most 16000 cells".into())); }
            for value in row { scalar(value)?; }
            for locator in locators { valid_text(locator, 300)?; }
        }
    }
    if source.content_sha256 != content_hash(source)? { return Err(Error::Conflict("source content has changed since ingestion".into())); }
    Ok(())
}

fn cell_address(row: usize, column: usize) -> String {
    let mut number = column + 1; let mut letters = Vec::new();
    while number > 0 { letters.push(char::from(b'A' + ((number - 1) % 26) as u8)); number = (number - 1) / 26; }
    format!("{}{}", letters.iter().rev().collect::<String>(), row + 1)
}

fn check_address(address: &str) -> Result<()> {
    for cell in address.split(':') {
        let mut column = 0usize; let mut digits = String::new();
        for character in cell.chars() {
            if character.is_ascii_uppercase() && digits.is_empty() { column = column.saturating_mul(26).saturating_add(character as usize - 'A' as usize + 1); }
            else if character.is_ascii_digit() { digits.push(character); }
            else { return Err(Error::Invalid("unsupported worksheet cell address".into())); }
        }
        let row = digits.parse::<usize>().map_err(|_| Error::Invalid("invalid worksheet row".into()))?;
        if !(1..=MAX_COLUMNS).contains(&column) || !(1..=MAX_ROWS + 1).contains(&row) { return Err(Error::Limit("worksheet cell range exceeds 1001 rows or 32 columns".into())); }
    }
    Ok(())
}

fn preflight_workbook(bytes: &[u8]) -> Result<()> {
    let package = Package::open(bytes.to_vec())?;
    for (name, data) in package.parts() {
        if name.contains("vbaProject") || name.starts_with("xl/externalLinks/") { return Err(Error::Unsupported("active or externally linked workbook content".into())); }
        if !name.ends_with(".xml") && !name.ends_with(".rels") { continue; }
        let document = crate::pptx::parse(std::str::from_utf8(data).map_err(|_| Error::Unsupported("workbook XML must use UTF-8".into()))?)?;
        if name.ends_with(".rels") && document.descendants().any(|node| node.attribute("TargetMode") == Some("External")) { return Err(Error::Unsupported("external workbook relationships are not loaded".into())); }
        if name.starts_with("xl/worksheets/") {
            for node in document.descendants() {
                if node.tag_name().name() == "c" { check_address(node.attribute("r").ok_or_else(|| Error::Invalid("worksheet cell address required".into()))?)?; }
                if node.tag_name().name() == "dimension" { if let Some(reference) = node.attribute("ref") { check_address(reference)?; } }
            }
        }
    }
    Ok(())
}