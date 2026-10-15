use crate::{document::{self, Document, Transaction, TransactionResult}, model::{element_list, Element}, selection::{Alignment, RelativeTo, SelectionOperation}, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum RevisionEdit {
    Translate { ids: Vec<String>, dx: f64, dy: f64 },
    Align { ids: Vec<String>, alignment: Alignment, relative_to: RelativeTo },
    SetTextFrame { id: String, x: f64, y: f64, width: f64, height: f64 },
    ReplaceText { id: String, text: String },
    UpdatePart { id: String, spec: crate::parts::PartSpec },
    UpdateGraph { id: String, spec: crate::graphs::GraphSpec },
}

#[derive(Serialize)]
pub struct RevisionPreview {
    pub slide_id: String,
    pub base_revision: u64,
    pub base_hash: String,
    pub candidate_hash: String,
    pub affected_ids: Vec<String>,
    pub stale_part_ids: Vec<String>,
    pub source_bindings_stale: bool,
    pub native_preservation_checked: bool,
    pub office_visual_parity: bool,
    pub before: crate::export_static::PresentationPreview,
    pub after: crate::export_static::PresentationPreview,
}

fn target(elements: &[Element], id: &str, ancestor_locked: bool) -> Result<()> {
    for element in elements {
        let locked = ancestor_locked || element.visual().is_some_and(|visual| visual.locked || visual.hidden);
        if element.bounds().0 == id {
            if locked || element_list(std::slice::from_ref(element)).iter().any(|child| child.visual().is_some_and(|visual| visual.locked || visual.hidden)) {
                return Err(Error::Unsupported("show and unlock revision targets before editing".into()));
            }
            return Ok(());
        }
        if let Element::Group { children, .. } = element {
            if element_list(children).iter().any(|child| child.bounds().0 == id) { return target(children, id, locked); }
        }
    }
    Err(Error::Invalid("revision target is not on the selected slide".into()))
}

fn set_frame(elements: &mut [Element], id: &str, frame: [f64; 4]) -> Result<()> {
    for element in elements {
        if element.bounds().0 == id {
            match element {
                Element::Text { x, y, width, height, format, .. } | Element::Shape { x, y, width, height, format, .. } => {
                    [*x, *y, *width, *height] = frame;
                    format.inherit_layout = false;
                    return Ok(());
                }
                _ => return Err(Error::Unsupported("set_text_frame requires a text box or shape".into())),
            }
        }
        if let Element::Group { children, .. } = element {
            if element_list(children).iter().any(|child| child.bounds().0 == id) { return set_frame(children, id, frame); }
        }
    }
    Err(Error::Invalid("revision text frame not found".into()))
}

fn candidate(document: &Document, expected_revision: u64, expected_hash: &str, slide_id: &str, edits: &[RevisionEdit]) -> Result<TransactionResult> {
    document::verify(document)?;
    if document.revision != expected_revision || document.hash != expected_hash { return Err(Error::Conflict("stale revision preview base; preview again".into())); }
    if edits.is_empty() || edits.len() > 16 { return Err(Error::Limit("slide revision requires 1-16 typed edits".into())); }
    crate::preflight::serialized_bytes(&edits, 128 * 1024, "slide revision edits")?;
    let slide_index = document.deck.slides.iter().position(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("revision slide not found".into()))?;
    let mut working = document.clone();
    for edit in edits {
        let ids: Vec<&String> = match edit {
            RevisionEdit::Translate { ids, .. } | RevisionEdit::Align { ids, .. } => ids.iter().collect(),
            RevisionEdit::SetTextFrame { id, .. } | RevisionEdit::ReplaceText { id, .. } | RevisionEdit::UpdatePart { id, .. } | RevisionEdit::UpdateGraph { id, .. } => vec![id],
        };
        if ids.is_empty() || ids.len() > 32 || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() { return Err(Error::Invalid("revision requires 1-32 unique target IDs".into())); }
        for id in ids {
            crate::model::valid_text(id, 80)?;
            target(&working.deck.slides[slide_index].elements, id, false)?;
        }
        let result = match edit {
            RevisionEdit::Translate { ids, dx, dy } => {
                if [*dx, *dy].iter().any(|value| !value.is_finite() || value.abs() > 4096.0) { return Err(Error::Invalid("revision translation exceeds the bounded canvas".into())); }
                crate::authoring_ops::edit_selection(&working, working.revision, slide_id, &SelectionOperation::Translate { ids: ids.clone(), dx: *dx, dy: *dy }, None)?.transaction
            }
            RevisionEdit::Align { ids, alignment, relative_to } => crate::authoring_ops::edit_selection(&working, working.revision, slide_id, &SelectionOperation::Align { ids: ids.clone(), alignment: *alignment, relative_to: *relative_to }, None)?.transaction,
            RevisionEdit::ReplaceText { id, text } => crate::authoring_ops::replace_text_content(&working, working.revision, slide_id, id, text.clone())?,
            RevisionEdit::SetTextFrame { id, x, y, width, height } => {
                let frame = [*x, *y, *width, *height];
                if frame.iter().any(|value| !value.is_finite() || !(0.0..=4096.0).contains(value)) || *width <= 0.0 || *height <= 0.0 {
                    return Err(Error::Invalid("revision frame requires finite positive dimensions within 4096px".into()));
                }
                let mut slide = working.deck.slides[slide_index].clone();
                set_frame(&mut slide.elements, id, frame)?;
                crate::authoring_ops::review_transaction(&working, Transaction { expected_revision: working.revision, expected_hash: working.hash.clone(), operations: serde_json::from_value(json!([
                    {"op":"replace","path":format!("/deck/slides/{slide_index}"),"value":slide}
                ]))? })?
            }
            RevisionEdit::UpdatePart { id, spec } => crate::parts::state::change(&working, working.revision, slide_id, id, spec, true)?,
            RevisionEdit::UpdateGraph { id, spec } => crate::graphs::change(&working, working.revision, slide_id, id, spec, true)?,
        };
        working = result.document;
        working.revision = document.revision;
    }
    for (index, slide) in document.deck.slides.iter().enumerate() {
        if index != slide_index && crate::canonical::bytes(slide)? != crate::canonical::bytes(&working.deck.slides[index])? {
            return Err(Error::Unsupported("revision would modify another slide".into()));
        }
    }
    crate::authoring_ops::review_transaction(document, Transaction { expected_revision, expected_hash: expected_hash.into(), operations: serde_json::from_value(json!([
        {"op":"replace","path":format!("/deck/slides/{slide_index}"),"value":working.deck.slides[slide_index]},
        {"op":"add","path":"/parts","value":working.parts},
        {"op":"replace","path":"/bindings","value":working.bindings}
    ]))? })
}

pub fn preview_slide_revision(document: &Document, expected_revision: u64, expected_hash: &str, slide_id: &str, edits: &[RevisionEdit], max_dimension: Option<u32>) -> Result<RevisionPreview> {
    let candidate = candidate(document, expected_revision, expected_hash, slide_id, edits)?.document;
    let index = document.deck.slides.iter().position(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("revision slide not found".into()))?;
    let old: BTreeMap<_, _> = element_list(&document.deck.slides[index].elements).iter().map(|element| Ok((element.bounds().0.to_owned(), crate::canonical::bytes(element)?))).collect::<Result<_>>()?;
    let new: BTreeMap<_, _> = element_list(&candidate.deck.slides[index].elements).iter().map(|element| Ok((element.bounds().0.to_owned(), crate::canonical::bytes(element)?))).collect::<Result<_>>()?;
    let affected_ids = old.keys().chain(new.keys()).collect::<BTreeSet<_>>().into_iter().filter(|id| old.get(*id) != new.get(*id)).cloned().collect();
    let options = crate::export_static::PreviewOptions { page_indices: Some(vec![index]), max_dimension: max_dimension.unwrap_or(960), max_output_bytes: 1024 * 1024, ..Default::default() };
    let result = RevisionPreview { slide_id: slide_id.into(), base_revision: document.revision, base_hash: document.hash.clone(), candidate_hash: candidate.hash.clone(),
        affected_ids, stale_part_ids: candidate.parts.iter().filter(|part| part.slide_id == slide_id && part.stale).map(|part| part.element_id.clone()).collect(),
        source_bindings_stale: candidate.bindings.iter().any(|binding| binding.stale), native_preservation_checked: document.origin.is_some(), office_visual_parity: false,
        before: crate::export_static::preview_presentation(document, &options)?, after: crate::export_static::preview_presentation(&candidate, &options)?,
    };
    crate::preflight::serialized_bytes(&result, 4 * 1024 * 1024 - 65536, "revision preview response")?;
    Ok(result)
}

pub fn apply_slide_revision(document: &Document, expected_revision: u64, expected_hash: &str, slide_id: &str, edits: &[RevisionEdit], candidate_hash: &str) -> Result<TransactionResult> {
    let result = candidate(document, expected_revision, expected_hash, slide_id, edits)?;
    if result.document.hash != candidate_hash { return Err(Error::Conflict("revision candidate differs from preview; preview again".into())); }
    Ok(result)
}