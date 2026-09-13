use crate::{model::valid_text, Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRegion { pub text: String, pub x: f32, pub y: f32, pub width: f32, pub height: f32 }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePage { pub page: u32, pub locator: String, pub text: String, pub method: String, #[serde(default)] pub regions: Vec<TextRegion> }
#[derive(Serialize)]
pub struct OcrStatus { pub available: bool, pub remote: bool, pub languages: Vec<String>, pub message: String }

pub fn pdf_text(bytes: &[u8]) -> Result<Vec<SourcePage>> {
    let document = lopdf::Document::load_mem_with_options(bytes, lopdf::LoadOptions { max_decompressed_size: Some(4 * 1024 * 1024), strict: true, ..Default::default() }).map_err(|_| Error::Invalid("PDF could not be parsed within the stream limits".into()))?;
    if document.is_encrypted() { return Err(Error::Unsupported("encrypted PDF source".into())); }
    if document.objects.len() > 10000 { return Err(Error::Limit("PDF object count > 10000".into())); }
    let pages = document.get_pages();
    if pages.is_empty() || pages.len() > 32 { return Err(Error::Limit("PDF requires 1-32 pages".into())); }
    let mut result = Vec::new(); let mut count = 0;
    for page in pages.keys() {
        let text = document.extract_text_with_limit(&[*page], 4 * 1024 * 1024).map_err(|_| Error::Invalid("PDF text could not be extracted within the stream limits".into()))?;
        count += text.chars().count();
        if count > 24000 { return Err(Error::Limit("extracted PDF text > 24000 characters".into())); }
        valid_text(&text, 24000)?;
        result.push(SourcePage { page: *page, locator: format!("page:{page}"), text, method: "pdf_text".into(), regions: Vec::new() });
    }
    Ok(result)
}

#[cfg(not(target_os = "windows"))]
pub fn ocr_status() -> OcrStatus { OcrStatus { available: false, remote: false, languages: Vec::new(), message: "Local OCR currently requires Windows with an installed OCR language.".into() } }

#[cfg(target_os = "windows")]
pub fn ocr_status() -> OcrStatus {
    let result = windows::Media::Ocr::OcrEngine::AvailableRecognizerLanguages().and_then(|languages| languages.into_iter().map(|language| language.LanguageTag().map(|tag| tag.to_string())).collect::<windows::core::Result<Vec<_>>>());
    match result {
        Ok(languages) => OcrStatus { available: !languages.is_empty(), remote: false, languages, message: "Windows local OCR; recognition requires manual review.".into() },
        Err(_) => OcrStatus { available: false, remote: false, languages: Vec::new(), message: "Windows OCR is not available in this process. Check installed language capabilities.".into() },
    }
}

#[cfg(not(target_os = "windows"))]
pub fn image_text(_bytes: &[u8], _language: Option<&str>) -> Result<SourcePage> { Err(Error::Unsupported("local OCR requires Windows with an installed language".into())) }

#[cfg(target_os = "windows")]
pub fn image_text(bytes: &[u8], language: Option<&str>) -> Result<SourcePage> {
    use windows::{core::HSTRING, Globalization::Language, Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap}, Media::Ocr::OcrEngine, Storage::Streams::DataWriter};
    let pixels = image::load_from_memory(bytes).map_err(|_| Error::Invalid("OCR raster decoding failed".into()))?.into_luma8();
    let recognized = (|| -> windows::core::Result<_> {
        let engine = match language { Some(language) => OcrEngine::TryCreateFromLanguage(&Language::CreateLanguage(&HSTRING::from(language))?)?, None => OcrEngine::TryCreateFromUserProfileLanguages()? };
        let writer = DataWriter::new()?;
        writer.WriteBytes(pixels.as_raw())?;
        let buffer = writer.DetachBuffer()?;
        let bitmap = SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Gray8, pixels.width() as i32, pixels.height() as i32)?;
        let result = engine.RecognizeAsync(&bitmap)?.join()?;
        let text = result.Text()?.to_string();
        let mut regions = Vec::new();
        for line in result.Lines()? {
            for word in line.Words()? {
                let bounds = word.BoundingRect()?;
                regions.push(TextRegion { text: word.Text()?.to_string(), x: bounds.X, y: bounds.Y, width: bounds.Width, height: bounds.Height });
            }
        }
        Ok((text, regions, engine.RecognizerLanguage()?.LanguageTag()?.to_string()))
    })().map_err(|_| Error::Unsupported("Windows OCR failed or the selected language is not installed".into()))?;
    valid_text(&recognized.0, 24000)?;
    if recognized.1.len() > 4000 { return Err(Error::Limit("OCR word regions > 4000".into())); }
    Ok(SourcePage { page: 1, locator: "image:1".into(), text: recognized.0, method: format!("windows_ocr:{}", recognized.2), regions: recognized.1 })
}