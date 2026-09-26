use crate::{document::{self, Document, Transaction, TransactionResult}, model::Deck, text_ops, Error, Result};
use serde_json::json;
use serde::{Deserialize, Serialize};
use crate::{model::{Element, element_list}, rich_text::{RichParagraph, RunStyle}, selection::{ElementBundle, SelectionOperation, SelectionEffects}, table_format::{self, TableFormat, CellStyle, MergeRegion}};

fn check(document: &Document, expected_revision: u64) -> Result<()> {
    document::verify(document)?;
    if document.revision != expected_revision { return Err(Error::Conflict("stale document revision".into())); }
    Ok(())
}

fn replace_deck(document: &Document, expected_revision: u64, deck: Deck) -> Result<TransactionResult> {
    document::transact(document, Transaction { expected_revision, expected_hash: document.hash.clone(),
        operations: serde_json::from_value(json!([{"op":"replace","path":"/deck","value":deck}]))? })
}

pub fn update_review(document: &Document, expected_revision: u64, update: impl FnOnce(&Deck) -> Result<Deck>) -> Result<TransactionResult> {
    check(document, expected_revision)?;
    let next = update(&document.deck)?;
    crate::comments::modern::validate_transition(&document.deck, &next)?;
    replace_deck(document, expected_revision, next)
}

pub fn review_transaction(document: &Document, mut transaction: Transaction) -> Result<TransactionResult> {
    check(document, transaction.expected_revision)?;
    if document.hash != transaction.expected_hash { return Err(Error::Conflict("stale document content hash".into())); }
    if transaction.operations.0.is_empty() || transaction.operations.0.len() > 128 { return Err(Error::Limit("transaction requires 1-128 operations".into())); }
    fn preserve_review(value: &mut serde_json::Value, previous: &serde_json::Value) {
        let Some(threads) = previous.get("modern_threads").filter(|value| value.is_array()) else { return; };
        if value.is_null() { *value = json!({}); }
        if let Some(review) = value.as_object_mut() {
            if review.get("modern_threads").is_none_or(serde_json::Value::is_null) { review.insert("modern_threads".into(), threads.clone()); }
        }
    }
    fn preserve_slide(value: &mut serde_json::Value, slides: &serde_json::Value) {
        let Some(id) = value.get("id").and_then(serde_json::Value::as_str) else { return; };
        let Some(previous) = slides.as_array().and_then(|slides| slides.iter().find(|slide| slide["id"] == id)) else { return; };
        if let Some(slide) = value.as_object_mut() { preserve_review(slide.entry("review").or_insert(serde_json::Value::Null), &previous["review"]); }
    }
    let mut working = serde_json::to_value(document)?;
    for operation in &mut transaction.operations.0 {
        let mut encoded = serde_json::to_value(&*operation)?;
        let path = encoded["path"].as_str().unwrap_or("").to_owned();
        if path.len() > 2048 { return Err(Error::Limit("transaction path > 2048 bytes".into())); }
        let kind = encoded["op"].as_str().unwrap_or("").to_owned();
        if matches!(kind.as_str(), "add" | "replace") {
            let value = &mut encoded["value"];
            if path == "/deck" {
                if let Some(slides) = value.get_mut("slides").and_then(serde_json::Value::as_array_mut) { for slide in slides { preserve_slide(slide, &working["deck"]["slides"]); } }
            } else if path == "/deck/slides" {
                if let Some(slides) = value.as_array_mut() { for slide in slides { preserve_slide(slide, &working["deck"]["slides"]); } }
            } else if let Some(rest) = path.strip_prefix("/deck/slides/") {
                let pieces: Vec<_> = rest.split('/').collect();
                if pieces.len() == 1 { preserve_slide(value, &working["deck"]["slides"]); }
                else if pieces.len() == 2 && pieces[1] == "review" { preserve_review(value, working.pointer(&path).unwrap_or(&serde_json::Value::Null)); }
                else if pieces.len() == 3 && pieces[1..] == ["review", "modern_threads"] && value.is_null() {
                    if let Some(previous) = working.pointer(&path).filter(|value| value.is_array()) { *value = previous.clone(); }
                }
            }
        } else if kind == "remove" && path.starts_with("/deck/slides/") {
            let pieces: Vec<_> = path.split('/').collect();
            let preserved = if pieces.len() == 5 && pieces[4] == "review" { working.pointer(&format!("{path}/modern_threads")).filter(|value| value.is_array()).map(|threads| json!({"modern_threads":threads})) }
                else if pieces.len() == 6 && pieces[4..] == ["review", "modern_threads"] { working.pointer(&path).filter(|value| value.is_array()).cloned() } else { None };
            if let Some(value) = preserved { encoded = json!({"op":"replace","path":path,"value":value}); }
        }
        *operation = serde_json::from_value(encoded)?;
        json_patch::patch(&mut working, &json_patch::Patch(vec![operation.clone()])).map_err(|_| Error::Conflict("transaction precondition or path failed; no changes applied".into()))?;
        crate::preflight::value(&working, document.capacity_profile.limits(), None)?;
        crate::preflight::serialized_bytes(&working, document.capacity_profile.limits().document_bytes, "review transaction document")?;
    }
    let next: Deck = serde_json::from_value(working["deck"].clone())?;
    crate::comments::modern::validate_transition(&document.deck, &next)?;
    document::transact(document, transaction)
}

pub fn refresh_fields(document: &Document, expected_revision: u64, reference_date: &str) -> Result<TransactionResult> {
    refresh_fields_with_options(document, expected_revision, reference_date, None, "en-US")
}

pub fn refresh_fields_with_options(document: &Document, expected_revision: u64, reference_date: &str, reference_time: Option<&str>, locale: &str) -> Result<TransactionResult> {
    check(document, expected_revision)?;
    let deck = crate::fields::refresh_with_options(&document.deck, reference_date, reference_time, locale)?;
    if let Some(origin) = &document.origin {
        use base64::{Engine, engine::general_purpose::STANDARD};
        if !origin.native { return Err(Error::Unsupported("field refresh requires an authored or native document".into())); }
        let bytes = STANDARD.decode(&origin.base64).map_err(|_| Error::Invalid("invalid imported origin base64".into()))?;
        let candidate = crate::native::save(bytes, &deck)?;
        if crate::fields::refresh_native_with_options(candidate.clone(), reference_date, reference_time, locale)? != candidate {
            return Err(Error::Unsupported("native field refresh contains unrepresented caches; no changes applied".into()));
        }
    }
    replace_deck(document, expected_revision, deck)
}

pub fn replace_text(document: &Document, expected_revision: u64, options: &text_ops::ReplaceOptions) -> Result<TransactionResult> {
    check(document, expected_revision)?;
    replace_deck(document, expected_revision, text_ops::replace(document.deck.clone(), options)?)
}

pub fn replace_font(document: &Document, expected_revision: u64, from: &str, to: &str) -> Result<TransactionResult> {
    check(document, expected_revision)?;
    replace_deck(document, expected_revision, text_ops::replace_font(document.deck.clone(), from, to)?)
}

fn find_element<'a>(elements: &'a mut [Element], id: &str) -> Option<&'a mut Element> {
    for element in elements {
        if element.bounds().0 == id { return Some(element); }
        if let Element::Group { children, .. } = element {
            if let Some(found) = find_element(children, id) { return Some(found); }
        }
    }
    None
}

fn update_element(document: &Document, expected_revision: u64, slide_id: &str, id: &str, update: impl FnOnce(Element) -> Result<Element>) -> Result<TransactionResult> {
    check(document, expected_revision)?;
    let mut deck = document.deck.clone();
    let slide = deck.slides.iter_mut().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("slide not found".into()))?;
    let element = find_element(&mut slide.elements, id).ok_or_else(|| Error::Invalid("element not found".into()))?;
    *element = update(element.clone())?;
    replace_deck(document, expected_revision, deck)
}

pub fn format_text(document: &Document, expected_revision: u64, slide_id: &str, id: &str, start: usize, end: usize, style: RunStyle) -> Result<TransactionResult> {
    update_element(document, expected_revision, slide_id, id, |element| crate::rich_text::apply_range(element, start, end, style))
}

pub use crate::proofing::FormatSnapshot;

pub fn copy_format(document: &Document, slide_id: &str, id: &str, paragraph_index: usize, run_index: usize) -> Result<FormatSnapshot> {
    document::verify(document)?;
    check_copy(document, slide_id, &[id.to_owned()])?;
    let element = document.deck.slides.iter().find(|slide| slide.id == slide_id)
        .and_then(|slide| slide.elements.iter().find(|element| element.bounds().0 == id))
        .ok_or_else(|| Error::Invalid("format source not found".into()))?;
    crate::proofing::copy_format(element, paragraph_index, run_index)
}

pub fn apply_format(document: &Document, expected_revision: u64, slide_id: &str, ids: &[String], style: &FormatSnapshot) -> Result<TransactionResult> {
    check(document, expected_revision)?;
    replace_deck(document, expected_revision, crate::proofing::apply_format(document.deck.clone(), slide_id, ids, style)?)
}

pub fn set_proofing_language(document: &Document, expected_revision: u64, slide_id: &str, id: &str, language: &str) -> Result<TransactionResult> {
    check(document, expected_revision)?;
    crate::proofing::validate_language(language)?;
    if !document.deck.slides.iter().any(|slide| slide.id == slide_id && slide.elements.iter().any(|element| element.bounds().0 == id)) {
        return Err(Error::Unsupported("proofing language requires a top-level text box or shape".into()));
    }
    update_element(document, expected_revision, slide_id, id, |element| {
        let length = match &element {
            Element::Text { text, visual, .. } | Element::Shape { text, visual, .. } => {
                if visual.as_ref().is_some_and(|visual| visual.locked || visual.hidden) { return Err(Error::Unsupported("unlock and show the proofing target before editing".into())); }
                text.chars().count()
            },
            _ => return Err(Error::Unsupported("proofing language requires a text box or shape".into())),
        };
        if length == 0 { return Err(Error::Unsupported("enter text before setting proofing language".into())); }
        crate::rich_text::apply_range(element, 0, length, RunStyle { language: Some(language.into()), ..Default::default() })
    })
}

pub fn replace_text_content(document: &Document, expected_revision: u64, slide_id: &str, id: &str, text: String) -> Result<TransactionResult> {
    update_element(document, expected_revision, slide_id, id, |element| crate::rich_text::replace_text_content(element, text))
}

pub fn update_paragraphs(document: &Document, expected_revision: u64, slide_id: &str, id: &str, paragraphs: Vec<RichParagraph>) -> Result<TransactionResult> {
    update_element(document, expected_revision, slide_id, id, |mut element| {
        crate::rich_text::validate_paragraphs(&paragraphs)?;
        let (text, format) = match &mut element {
            Element::Text { text, format, .. } | Element::Shape { text, format, .. } => (text, format),
            _ => return Err(Error::Unsupported("paragraph editing requires a text box or shape".into())),
        };
        *text = crate::rich_text::plain_text(&paragraphs);
        format.paragraphs = paragraphs;
        format.inherit_layout = false;
        crate::rich_text::validate_element(&element)?;
        Ok(element)
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum TableOperation {
    UpdateFormat { format: TableFormat },
    SetCellStyle { row: usize, column: usize, style: CellStyle },
    SetCellText { row: usize, column: usize, text: String },
    Merge { region: MergeRegion },
    Split { row: usize, column: usize },
    InsertRow { index: usize, values: Vec<String> },
    InsertColumn { index: usize, values: Vec<String> },
    RemoveRow { index: usize },
    RemoveColumn { index: usize },
}

pub fn edit_table(document: &Document, expected_revision: u64, slide_id: &str, id: &str, operations: Vec<TableOperation>) -> Result<TransactionResult> {
    update_element(document, expected_revision, slide_id, id, |mut element| {
        if operations.is_empty() || operations.len() > 128 { return Err(Error::Limit("table editing requires 1-128 operations".into())); }
        for operation in operations {
            element = match operation {
                TableOperation::UpdateFormat { format } => table_format::update_format(element, format)?,
                TableOperation::SetCellStyle { row, column, style } => table_format::set_cell_style(element, row, column, style)?,
                TableOperation::SetCellText { row, column, text } => table_format::replace_cell_text(element, row, column, text)?,
                TableOperation::Merge { region } => table_format::merge_cells(element, region)?,
                TableOperation::Split { row, column } => table_format::split_cell(element, row, column)?,
                TableOperation::InsertRow { index, values } => table_format::insert_row(element, index, values)?,
                TableOperation::InsertColumn { index, values } => table_format::insert_column(element, index, values)?,
                TableOperation::RemoveRow { index } => table_format::remove_row(element, index)?,
                TableOperation::RemoveColumn { index } => table_format::remove_column(element, index)?,
            };
        }
        Ok(element)
    })
}

pub fn resize_canvas(document: &Document, expected_revision: u64, width: u32, height: u32, mode: crate::canvas::ResizeMode) -> Result<TransactionResult> {
    check(document, expected_revision)?;
    replace_deck(document, expected_revision, crate::canvas::resize(document.deck.clone(), width, height, mode)?)
}

pub fn apply_image_edit(document: &Document, expected_revision: u64, slide_id: &str, id: &str, image: crate::image_edit::EditedImage) -> Result<TransactionResult> {
    update_element(document, expected_revision, slide_id, id, |mut element| {
        let info = crate::media::inspect_raster(&image.base64, &image.mime_type)?;
        if info.width != image.width || info.height != image.height { return Err(Error::Invalid("prepared image dimensions do not match its bytes".into())); }
        let Element::Picture { base64, mime_type, svg, .. } = &mut element else { return Err(Error::Unsupported("image application requires a picture".into())); };
        *base64 = image.base64; *mime_type = image.mime_type; *svg = None;
        Ok(element)
    })
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateKind { Potx, Thmx }

pub fn import_template(id: String, kind: TemplateKind, bytes: Vec<u8>) -> Result<Document> {
    match kind {
        TemplateKind::Potx => crate::templates::load_potx(id, bytes),
        TemplateKind::Thmx => {
            let theme = crate::templates::load_thmx(bytes)?;
            let mut deck = crate::editing::create(id.clone(), theme.name.clone())?.deck;
            deck.design.as_mut().ok_or_else(|| Error::Invalid("blank design missing".into()))?.theme = theme;
            document::create(id, deck, Vec::new(), Vec::new(), None)
        }
    }
}

pub fn export_template(document: &Document, kind: TemplateKind) -> Result<Vec<u8>> {
    document::verify(document)?;
    match kind {
        TemplateKind::Potx => crate::templates::export_potx(document),
        TemplateKind::Thmx => crate::templates::export_thmx(&document.deck.design.as_ref().ok_or_else(|| Error::Unsupported("document has no theme".into()))?.theme),
    }
}

#[derive(Serialize)]
pub struct SelectionTransaction {
    pub transaction: TransactionResult,
    pub clipboard: Option<ElementBundle>,
    pub effects: SelectionEffects,
}

fn check_copy(document: &Document, slide_id: &str, ids: &[String]) -> Result<()> {
    if document.origin.as_ref().is_some_and(|origin| !origin.native) { return Err(Error::Unsupported("structural clipboard is unavailable for approximate imported documents".into())); }
    let slide = document.deck.slides.iter().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("clipboard source slide not found".into()))?;
    for id in ids {
        let element = slide.elements.iter().find(|element| element.bounds().0 == id).ok_or_else(|| Error::Invalid("clipboard source element not found".into()))?;
        crate::native_save::check_duplicate(document, slide_id, element)?;
        let descendants = element_list(std::slice::from_ref(element));
        if document.parts.iter().any(|part| part.slide_id == slide_id && part.stale && descendants.iter().any(|element| element.bounds().0 == part.element_id)) {
            return Err(Error::Conflict("stale managed part cannot be copied; retain manual edits or detach metadata explicitly".into()));
        }
    }
    Ok(())
}

pub fn edit_selection(document: &Document, expected_revision: u64, slide_id: &str, operation: &SelectionOperation, clipboard: Option<&ElementBundle>) -> Result<SelectionTransaction> {
    check(document, expected_revision)?;
    if let SelectionOperation::Copy { ids, .. } | SelectionOperation::Cut { ids, .. } = operation { check_copy(document, slide_id, ids)?; }
    let mut result = crate::selection::apply(&document.deck, slide_id, operation, clipboard)?;
    if let Some(bundle) = &mut result.clipboard {
        bundle.source_document = Some(crate::selection::ClipboardSource { id: document.id.clone(), hash: document.hash.clone(), origin_sha256: document.origin.as_ref().map(|origin| origin.sha256.clone()) });
    }
    commit_selection(document, expected_revision, slide_id, operation, clipboard, result)
}

pub fn combine_shapes(document: &Document, expected_revision: u64, slide_id: &str, ids: &[String], operation: crate::geometry_ops::BooleanOperation, result_id: &str) -> Result<SelectionTransaction> {
    check(document, expected_revision)?;
    let result = crate::geometry_ops::combine(&document.deck, slide_id, ids, operation, result_id)?;
    check_copy(document, slide_id, ids)?;
    commit_selection(document, expected_revision, slide_id, &SelectionOperation::Cut { ids: ids.to_vec(), format: crate::selection::ClipboardFormat::KeepSourceFormatting }, None, result)
}

fn commit_selection(document: &Document, expected_revision: u64, slide_id: &str, operation: &SelectionOperation, clipboard: Option<&ElementBundle>, mut result: crate::selection::SelectionResult) -> Result<SelectionTransaction> {
    if let Some(origin) = document.origin.as_ref().filter(|origin| origin.native) {
        use base64::Engine;
        let package = crate::package::Package::open(base64::engine::general_purpose::STANDARD.decode(&origin.base64).map_err(|_| Error::Invalid("origin base64".into()))?)?;
        let native = crate::native::read(&package)?;
        let slide = result.deck.slides.iter().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("selection slide missing".into()))?;
        let binding = native.slides.iter().find(|part| part.id == slide.native_source_id.as_deref().unwrap_or(slide_id));
        let mut allocation: std::collections::BTreeMap<String, u32> = binding.map(|binding| binding.nodes.iter().map(|(id, value)| value.parse().map(|number| (id.clone(), number)).map_err(|_| Error::Invalid("native selection numeric ID".into()))).collect::<Result<_>>()).transpose()?.unwrap_or_default();
        let mut maximum = allocation.values().copied().max().unwrap_or(1);
        for element in element_list(&slide.elements) {
            if !allocation.contains_key(element.bounds().0) {
                maximum = maximum.checked_add(1).ok_or_else(|| Error::Limit("native selection IDs exhausted".into()))?;
                allocation.insert(element.bounds().0.into(), maximum);
            }
        }
        crate::selection::validate_native_ids_on_canvas(&slide.elements, &allocation, result.deck.width, result.deck.height)?;
    }
    let mut parts = document.parts.clone();
    let mut bindings = document.bindings.clone();
    let before_parts = parts.len(); let before_bindings = bindings.len();
    parts.retain(|part| part.slide_id != slide_id || (!result.effects.removed_ids.contains(&part.element_id) && !result.effects.reparented_ids.contains(&part.element_id)));
    bindings.retain(|binding| binding.slide_id != slide_id || !result.effects.removed_ids.contains(&binding.element_id));
    if before_parts != parts.len() || before_bindings != bindings.len() {
        result.effects.warnings.push(format!("Detached {} managed part roots and {} removed-element bindings in this transaction; Undo restores them. Structural clipboard carries no detached metadata.", before_parts - parts.len(), before_bindings - bindings.len()));
    }
    if let (SelectionOperation::Paste { .. }, Some(bundle)) = (operation, clipboard) {
        let ids: Vec<_> = bundle.elements.iter().map(|element| element.bounds().0.to_owned()).collect();
        let same_document = bundle.source_document.as_ref().is_some_and(|source| source.id == document.id && source.origin_sha256.as_deref() == document.origin.as_ref().map(|origin| origin.sha256.as_str()));
        let available = same_document && document.deck.slides.iter().find(|slide| slide.id == bundle.source_slide_id)
            .is_some_and(|slide| ids.iter().all(|id| slide.elements.iter().any(|element| element.bounds().0 == id)));
        if available {
            check_copy(document, &bundle.source_slide_id, &ids)?;
            let mut expected = crate::selection::apply(&document.deck, &bundle.source_slide_id, &SelectionOperation::Copy { ids, format: bundle.format }, None)?.clipboard;
            if let Some(expected) = &mut expected { expected.source_document = bundle.source_document.clone(); }
            if crate::canonical::bytes(&expected)? != crate::canonical::bytes(&Some(bundle))? { return Err(Error::Conflict("clipboard source changed; copy again before pasting".into())); }
            let target = result.deck.slides.iter().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("paste target missing".into()))?;
            let metadata_current = bundle.source_document.as_ref().is_some_and(|source| source.hash == document.hash);
            if !metadata_current { result.effects.warnings.push("Clipboard source document changed; matching semantic objects can be pasted, but part/binding metadata from a different document revision is not copied.".into()); }
            for part in document.parts.iter().filter(|part| metadata_current && part.slide_id == bundle.source_slide_id && result.effects.id_map.contains_key(&part.element_id)) {
                let mut copied = part.clone(); copied.slide_id = slide_id.into(); copied.element_id = result.effects.id_map[&part.element_id].clone(); copied.native_sha256 = None;
                let element = target.elements.iter().find(|element| element.bounds().0 == copied.element_id).ok_or_else(|| Error::Conflict("copied part root missing".into()))?;
                copied.render_sha256 = crate::parts::state::render_hash(element)?; parts.push(copied);
            }
            for binding in document.bindings.iter().filter(|binding| metadata_current && binding.slide_id == bundle.source_slide_id && result.effects.id_map.contains_key(&binding.element_id)) {
                let mut copied = binding.clone(); copied.slide_id = slide_id.into(); copied.element_id = result.effects.id_map[&binding.element_id].clone(); bindings.push(copied);
            }
        } else {
            result.effects.warnings.push("External or cut clipboard pasted as representable semantic content only, not lossless native XML; no managed-part or source-binding metadata was available to map.".into());
        }
    }
    let transaction = document::transact(document, Transaction { expected_revision, expected_hash: document.hash.clone(), operations: serde_json::from_value(json!([
        {"op":"replace","path":"/deck","value":result.deck}, {"op":"add","path":"/parts","value":parts}, {"op":"replace","path":"/bindings","value":bindings}
    ]))? })?;
    if matches!(operation, SelectionOperation::Group { .. } | SelectionOperation::Ungroup { .. }) && document.origin.as_ref().is_some_and(|origin| origin.native) {
        result.effects.raw_native_preserved = true;
        result.effects.warnings.push("Native group/ungroup moved original child XML with adjusted transforms; only affected managed roots were detached. Unsupported group content or cross-container references reject the whole transaction.".into());
    }
    result.effects.warnings.push("The document transaction checked current native preservation when an origin exists; structural validation is not Office visual validation.".into());
    Ok(SelectionTransaction { transaction, clipboard: result.clipboard, effects: result.effects })
}

pub fn capabilities() -> serde_json::Value {
    json!({"version":1,"operations":["search_text","replace_text","replace_font","format_text","replace_text_content","update_paragraphs","edit_selection","combine_shapes","resize_canvas","import_template","export_template","inspect_master_source","preview_master_import","import_masters","edit_image","apply_image_edit","edit_table","format_text_element","replace_element_text","refresh_fields","add_comment","reply_comment","resolve_comment","remove_comment","modern_comment","set_table_headers","set_accessibility","set_reading_order","check_accessibility","inspect_document","export_clean_copy","preview_presentation","preflight_presentation","preview_slide_revision","apply_slide_revision","prepare_delivery","apply_operations","import_slides"],
        "typed_authoring":{"batch_limit":128,"element_roots_per_add":128,"operations":["add_elements","set_frame","set_text_style","set_slide_background","set_connector","set_picture_crop","set_hyperlink","set_shape_adjustment","add_picture","add_part","update_part","add_graph","update_graph","update_notes","set_table_headers","set_accessibility"],"managed_parts_limit":128,"managed_metadata_persisted":true,"scale_fonts":false,"scale_strokes":false,"one_undo":true},
        "slide_import":{"source":"authored_document_only","selected_slides":128,"same_canvas":true,"design_deduplication":true,"office_visual_parity":false},
        "master_import":{"formats":["pptx","potx"],"modes":["masters","slides"],"selection_limit":8,"masters_limit":8,"layouts_limit":32,"same_canvas_required":true,"append_only":true,"automatic_assignment":false,"source_fonts_imported":false,"unsupported_content":"rejected before apply","office_visual_parity":false},
        "delivery":{"visual_pages":8,"output_bytes":crate::delivery::MAX_DELIVERY_BYTES,"mcp_response_bytes":4194304,"mcp_max_files_including_manifest":13,"options_schema":schemars::schema_for!(crate::delivery::DeliveryOptions),"notes_opt_in":true,"source_report_opt_in":true,"multi_file_atomic":false},
        "visual_authoring":{"preview_pages":8,"preview_max_dimension":1600,"preview_encoded_bytes":2097152,"preview_wire_bytes":4194304,"preflight_findings":256,"preflight_objects_per_page":1024,"revision_edits":16,"revision_targets_per_edit":32,"revision_edit_bytes":131072,"preview_schema":schemars::schema_for!(crate::export_static::PreviewOptions),"preflight_schema":schemars::schema_for!(crate::authoring_preflight::PreflightOptions),"revision_edit_schema":schemars::schema_for!(crate::slide_revision::RevisionEdit),"office_visual_parity":false,"semantic_truth_verified":false},
        "local_ai":{"operations":["text_assist","apply_text_assist","segmentation_status","segment_image"],"local_only":true,"automatic_application":false,"text_characters":8000,"text_tasks":["proofread","translate"],"text_preserves_paragraph_count":true,"field_replacement":false,"segmentation_model":"u2netp","model_sha256":crate::segmentation::MODEL_SHA256,"optional_model_bytes":4574861,"weights_bundled":false,"inference_downloads":false,"segmentation_hard_cancellation":false,"model_quality_verified":false},
        "geometry":crate::geometry_ops::capabilities(),
        "capacity_profiles":{"default":"large","legacy":crate::limits::LEGACY,"standard":crate::limits::STANDARD,"large":crate::limits::LARGE},
        "font_limits":{"face_bytes":crate::fonts::MAX_FONT_BYTES,"total_bytes":crate::fonts::MAX_TOTAL_FONT_BYTES,"faces":8,"subsetting":false,"variable_fonts":false,"collections":false},
        "limits":{"request_bytes":crate::protocol::MAX_REQUEST_BYTES,"document_bytes":crate::limits::LARGE.document_bytes,"operations":128,"canvas_min":320,"canvas_max":4096,"search_matches":text_ops::MAX_MATCHES,"selection_ids":256,"table_rows":12,"table_columns":8,"image_bytes":1048576,"image_dimension":4096},
        "selection_schema":schemars::schema_for!(SelectionOperation),"search_schema":schemars::schema_for!(text_ops::SearchOptions),"replace_schema":schemars::schema_for!(text_ops::ReplaceOptions),
        "charts":crate::model::chart_format::catalog(),"templates":["potx","thmx"],"office_visual_parity":false,"raw_clipboard_xml":false,
        "review":{"field_kinds":["slidenum","datetime1"],"field_reference_date":"caller supplied YYYY-MM-DD","native_field_refresh":true,"comments":"legacy API unchanged; separate native modern threads with nonrecursive replies and active/resolved/closed status","authenticated_authors":false,"modern_powerpoint_threads":true,"reading_order_changes_z_order":true,"complete_personal_data_detection":false,"wcag_certified":false,"clean_copy":"confirmed explicit categories; new document ID; source and protection unchanged","limitations":["Native field refresh uses the origin-preserving helper; unrepresented caches and unsupported field edits fail closed.","Unknown native field kinds are preserved without evaluation.","Inspection returns masked candidates and numeric locations, not matched values or paths; manual review required.","Clean copy refuses protected, labelled and signed packages; candidates are not deletion categories; history retains prior content.","Modern authors are local offline GUID identities, not authenticated users, notifications or service mentions.","New modern anchors require explicit unknown; existing opaque anchors are preserved; modern slide copies reject until remapping is supported.","Unknown rich body XML permits status-only changes but rejects body replacement; schema validity does not prove Office interoperability.","Explicit semantic table header policies are independent of styling; merged associations and PDF header tagging require separate verification.","Reading order also changes native z-order; review overlaps and visual appearance."]},
        "limitations":["Native preservation is checked by document transactions and may reject group/ungroup topology or imported rich/table changes.","Copy does not change document revision or history. Bundles exclude raw XML and metadata; only verified current-document sources map part/source bindings on paste.","Canvas scale may mark managed part metadata stale; existing refresh remains authoritative.","Search and font replacement use the current text_ops support contract; no alternate rich-text replacement engine is used.","Image preparation is local and separate from applying pixels; background keying is not semantic segmentation.","Template imports create a new document; macros, signed and protected conversions are rejected."]})
}

pub fn edit_text_element(element: Element, update: impl FnOnce(Element) -> Result<Element>) -> Result<Element> {
    let validate = |element: &Element| crate::model::validate_elements(std::slice::from_ref(element), (4096.0, 4096.0), 0, &mut std::collections::BTreeSet::new(), &mut 0, &mut 0);
    validate(&element)?;
    let element = update(element)?;
    validate(&element)?;
    Ok(element)
}

pub fn update_rich_notes(document: &Document, expected_revision: u64, slide_id: &str, paragraphs: Vec<RichParagraph>) -> Result<TransactionResult> {
    check(document, expected_revision)?;
    let paragraphs = if paragraphs.is_empty() { vec![RichParagraph { runs: vec![crate::rich_text::RichRun::default()], ..Default::default() }] } else { paragraphs };
    crate::rich_text::validate_paragraphs_with_limit(&paragraphs, 8000)?;
    let mut deck = document.deck.clone();
    let slide = deck.slides.iter_mut().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("slide not found".into()))?;
    slide.notes = crate::rich_text::plain_text(&paragraphs);
    slide.notes_paragraphs = paragraphs;
    replace_deck(document, expected_revision, deck)
}

pub fn update_auxiliary_design(document: &Document, expected_revision: u64, design: crate::model::AuxiliaryDesign) -> Result<TransactionResult> {
    check(document, expected_revision)?;
    let mut deck = document.deck.clone();
    deck.auxiliary_design = Some(design);
    replace_deck(document, expected_revision, deck)
}