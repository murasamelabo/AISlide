use crate::{document::{Document, Transaction, TransactionResult}, model::{Deck, Slide, Element, element_list, valid_text}, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum SlideOperation {
    Insert { id: String, #[serde(default)] after: Option<String>, title: String, #[serde(default)] layout_id: Option<String> },
    Duplicate { slide_id: String, id: String },
    Remove { slide_id: String },
    Move { slide_id: String, index: usize },
    Rename { slide_id: String, title: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ElementOperation {
    Duplicate { id: String, new_id: String },
    Remove { id: String },
    Order { id: String, index: usize },
}

pub fn elements(document: &Document, expected_revision: u64, slide_id: &str, operations: &[ElementOperation]) -> Result<TransactionResult> {
    crate::document::verify(document)?;
    if document.revision != expected_revision { return Err(Error::Conflict("stale document revision".into())); }
    if operations.is_empty() || operations.len() > 128 { return Err(Error::Limit("element editing requires 1-128 operations".into())); }
    let mut deck = document.deck.clone(); let mut parts = document.parts.clone(); let mut bindings = document.bindings.clone();
    let slide = deck.slides.iter_mut().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("slide not found".into()))?;
    for operation in operations {
        let id = match operation { ElementOperation::Duplicate { id, .. } | ElementOperation::Remove { id } | ElementOperation::Order { id, .. } => id };
        let index = slide.elements.iter().position(|element| element.bounds().0 == id).ok_or_else(|| Error::Invalid("element not found".into()))?;
        match operation {
            ElementOperation::Remove { .. } => {
                let removed: std::collections::BTreeSet<_> = element_list(&[slide.elements[index].clone()]).iter().map(|element| element.bounds().0.to_owned()).collect();
                slide.elements.retain(|element| element.bounds().0 != id && !matches!(element, Element::Connector { start, end, .. } if [start,end].into_iter().flatten().any(|connection| removed.contains(&connection.element_id))));
                parts.retain(|part| part.slide_id != slide_id || &part.element_id != id);
                bindings.retain(|binding| binding.slide_id != slide_id || !removed.contains(&binding.element_id));
            }
            ElementOperation::Order { index: target, .. } => {
                if *target >= slide.elements.len() { return Err(Error::Invalid("element order is outside the slide".into())); }
                let element = slide.elements.remove(index); slide.elements.insert(*target, element);
            }
            ElementOperation::Duplicate { new_id, .. } => {
                identity(new_id)?; valid_text(new_id, 40)?;
                let source = &slide.elements[index];
                let part = parts.iter().find(|part| part.slide_id == slide_id && &part.element_id == id).cloned();
                if part.as_ref().is_some_and(|part| part.stale) { return Err(Error::Conflict("stale part metadata cannot be duplicated".into())); }
                if part.is_none() { crate::native_save::check_duplicate(document, slide_id, source)?; }
                let mapping: std::collections::BTreeMap<_, _> = element_list(&[source.clone()]).iter().enumerate().map(|(index, element)| (element.bounds().0.to_owned(), if index == 0 { new_id.clone() } else { format!("{new_id}-{index}") })).collect();
                fn rename(value: &mut serde_json::Value, mapping: &std::collections::BTreeMap<String, String>) {
                    if let Some(id) = value.get_mut("id") { if let Some(next) = id.as_str().and_then(|id| mapping.get(id)) { *id = json!(next); } }
                    for field in ["start", "end"] { if let Some(id) = value.get_mut(field).and_then(|connection| connection.get_mut("element_id")) { if let Some(next) = id.as_str().and_then(|id| mapping.get(id)) { *id = json!(next); } } }
                    if let Some(children) = value.get_mut("children").and_then(|children| children.as_array_mut()) { for child in children { rename(child, mapping); } }
                }
                let mut value = serde_json::to_value(source)?; rename(&mut value, &mapping);
                let (_, x, y, width, height) = source.bounds(); value["x"] = json!((x + 24.0).min(1280.0 - width)); value["y"] = json!((y + 24.0).min(720.0 - height));
                let copy: Element = serde_json::from_value(value)?;
                if let Some(mut part) = part { part.element_id = new_id.clone(); part.native_sha256 = None; part.render_sha256 = crate::parts::state::render_hash(&copy)?; parts.push(part); }
                let copied: Vec<_> = bindings.iter().filter(|binding| binding.slide_id == slide_id && mapping.contains_key(&binding.element_id)).map(|binding| { let mut copy = binding.clone(); copy.element_id = mapping[&copy.element_id].clone(); copy }).collect();
                bindings.extend(copied); slide.elements.insert(index + 1, copy);
            }
        }
    }
    crate::document::transact(document, Transaction { expected_revision, expected_hash: document.hash.clone(), operations: serde_json::from_value(json!([
        {"op":"replace","path":"/deck","value":deck}, {"op":"add","path":"/parts","value":parts}, {"op":"replace","path":"/bindings","value":bindings}
    ]))? })
}

fn identity(id: &str) -> Result<()> {
    valid_text(id, 80)?;
    if id.is_empty() { return Err(Error::Invalid("slide ID is required".into())); }
    Ok(())
}

fn blank(id: String, title: String, layout_id: Option<String>) -> Slide {
    Slide { id, title, background: "@lt1".into(), elements: Vec::new(), notes: String::new(), layout_id, inherit_background: false, hide_master_graphics: false, native_source_id: None }
}

pub fn create(id: String, title: String) -> Result<Document> {
    valid_text(&title, 200)?;
    let design = crate::design::Design::default();
    let layout = design.layouts.iter().find(|layout| layout.elements.is_empty()).map(|layout| layout.id.clone());
    let deck = Deck { version: 1, title, width: 1280, height: 720, slides: vec![blank("slide-1".into(), "Slide 1".into(), layout)], design: Some(design) };
    crate::document::create(id, deck, Vec::new(), Vec::new(), None)
}

pub fn slides(document: &Document, expected_revision: u64, operations: &[SlideOperation]) -> Result<TransactionResult> {
    crate::document::verify(document)?;
    if document.revision != expected_revision { return Err(Error::Conflict("stale document revision".into())); }
    if operations.is_empty() || operations.len() > 128 { return Err(Error::Limit("slide editing requires 1-128 operations".into())); }
    let mut deck = document.deck.clone();
    let mut parts = document.parts.clone();
    let mut bindings = document.bindings.clone();
    for operation in operations {
        let locate = |id: &str| deck.slides.iter().position(|slide| slide.id == id).ok_or_else(|| Error::Invalid("slide not found".into()));
        match operation {
            SlideOperation::Insert { id, after, title, layout_id } => {
                identity(id)?; valid_text(title, 200)?;
                if deck.slides.iter().any(|slide| &slide.id == id) { return Err(Error::Conflict("slide ID already exists".into())); }
                let index = after.as_deref().map(locate).transpose()?.map_or(deck.slides.len(), |index| index + 1);
                let layout = layout_id.clone().or_else(|| deck.design.as_ref().and_then(|design| design.layouts.iter().filter(|layout| layout.elements.is_empty()).min_by_key(|layout| usize::from(layout.id != "preset-blank" || layout.master_id != "preset-master")).map(|layout| layout.id.clone())));
                deck.slides.insert(index, blank(id.clone(), title.clone(), layout.clone()));
                if layout_id.is_some() { deck = crate::design::assign_layout(deck, id, layout.as_deref().unwrap_or(""))?; }
            }
            SlideOperation::Duplicate { slide_id, id } => {
                identity(id)?;
                if deck.slides.iter().any(|slide| &slide.id == id) { return Err(Error::Conflict("slide ID already exists".into())); }
                let index = locate(slide_id)?;
                let mut copied = deck.slides[index].clone(); copied.id = id.clone();
                if let Some(origin) = document.origin.as_ref().filter(|origin| origin.native) {
                    let package = crate::package::Package::open(base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &origin.base64).map_err(|_| Error::Invalid("origin base64".into()))?)?;
                    if copied.native_source_id.is_none() && crate::native::read(&package)?.slides.iter().any(|slide| &slide.id == slide_id) { copied.native_source_id = Some(slide_id.clone()); }
                }
                let copied_parts: Vec<_> = parts.iter().filter(|part| &part.slide_id == slide_id).map(|part| {
                    if part.stale { return Err(Error::Conflict("stale part metadata cannot be duplicated; preserve native edits first".into())); }
                    let mut copy = part.clone(); copy.slide_id = id.clone(); copy.native_sha256 = None; Ok(copy)
                }).collect::<Result<_>>()?;
                let copied_bindings: Vec<_> = bindings.iter().filter(|binding| &binding.slide_id == slide_id).map(|binding| { let mut copy = binding.clone(); copy.slide_id = id.clone(); copy }).collect();
                deck.slides.insert(index + 1, copied); parts.extend(copied_parts); bindings.extend(copied_bindings);
            }
            SlideOperation::Remove { slide_id } => {
                let index = locate(slide_id)?;
                if deck.slides.len() == 1 { return Err(Error::Invalid("a presentation must retain at least one slide".into())); }
                deck.slides.remove(index); parts.retain(|part| &part.slide_id != slide_id); bindings.retain(|binding| &binding.slide_id != slide_id);
            }
            SlideOperation::Move { slide_id, index } => {
                let current = locate(slide_id)?;
                if *index >= deck.slides.len() { return Err(Error::Invalid("slide destination is outside the presentation".into())); }
                let slide = deck.slides.remove(current); deck.slides.insert(*index, slide);
            }
            SlideOperation::Rename { slide_id, title } => {
                valid_text(title, 200)?;
                let index = locate(slide_id)?; deck.slides[index].title = title.clone();
            }
        }
        if deck.slides.len() > 32 { return Err(Error::Limit("presentation exceeds 32 slides".into())); }
    }
    crate::document::transact(document, Transaction { expected_revision, expected_hash: document.hash.clone(), operations: serde_json::from_value(json!([
        {"op":"replace","path":"/deck","value":deck}, {"op":"add","path":"/parts","value":parts},
        {"op":"replace","path":"/bindings","value":bindings}, {"op":"add","path":"/report","value":null}
    ]))? })
}