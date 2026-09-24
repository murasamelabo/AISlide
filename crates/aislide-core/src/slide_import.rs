use crate::{
    design::{Design, Master, SlideLayout, Theme},
    document::{self, Document, Transaction, TransactionResult},
    model::{Deck, Slide},
    Error, Result,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

fn identity(id: &str) -> Result<()> {
    crate::model::valid_text(id, 80)?;
    if id.trim().is_empty() || id.chars().any(char::is_control) {
        return Err(Error::Invalid("slide import IDs must be nonempty and contain no control characters".into()));
    }
    Ok(())
}

fn fallback_design() -> Design {
    let mut design = Design::default();
    design.layouts.truncate(1);
    design.masters[0].theme = Some(design.theme.clone());
    design
}

fn reserve_ids(deck: &Deck, used: &mut BTreeSet<String>) {
    for slide in &deck.slides {
        used.insert(slide.id.clone());
        used.extend(slide.native_source_id.iter().cloned());
    }
    if let Some(design) = &deck.design {
        used.extend(design.masters.iter().map(|master| master.id.clone()));
        used.extend(design.layouts.iter().map(|layout| layout.id.clone()));
    }
}

fn next_id(used: &mut BTreeSet<String>, prefix: &str, kind: &str) -> Result<String> {
    for index in 1..=used.len() + 1 {
        let id = format!("{prefix}-{kind}{index}");
        if used.insert(id.clone()) { return Ok(id); }
    }
    Err(Error::Limit("slide import identities exhausted".into()))
}

fn master_key(master: &Master, theme: &Theme) -> Result<Vec<u8>> {
    crate::canonical::bytes(&json!({
        "background": master.background,
        "elements": master.elements,
        "theme": {"colors": theme.colors, "fonts": theme.fonts},
    }))
}

fn layout_key(layout: &SlideLayout) -> Result<Vec<u8>> {
    crate::canonical::bytes(&json!({"background": layout.background, "elements": layout.elements}))
}

fn import_layout(
    design: &mut Design,
    source: &Design,
    layout: &SlideLayout,
    used: &mut BTreeSet<String>,
    prefix: &str,
) -> Result<String> {
    let master = source.masters.iter().find(|master| master.id == layout.master_id)
        .ok_or_else(|| Error::Invalid("source layout master missing".into()))?;
    let theme = crate::design::master_theme(source, &master.id);
    let key = master_key(master, theme)?;
    let source_layout_key = layout_key(layout)?;
    let mut master_id = None;
    for existing in &design.masters {
        if master_key(existing, crate::design::master_theme(design, &existing.id))? == key {
            for existing_layout in design.layouts.iter().filter(|layout| layout.master_id == existing.id) {
                if layout_key(existing_layout)? == source_layout_key { return Ok(existing_layout.id.clone()); }
            }
            if master_id.is_none() { master_id = Some(existing.id.clone()); }
        }
    }
    let master_id = match master_id {
        Some(id) => id,
        None => {
            if design.masters.len() >= 8 { return Err(Error::Limit("slide import exceeds 8 masters".into())); }
            let mut copied = master.clone();
            copied.id = next_id(used, prefix, "m")?;
            copied.theme = Some(theme.clone());
            let id = copied.id.clone();
            design.masters.push(copied);
            id
        }
    };
    if design.layouts.len() >= 32 { return Err(Error::Limit("slide import exceeds 32 layouts".into())); }
    let mut copied = layout.clone();
    copied.id = next_id(used, prefix, "l")?;
    copied.master_id = master_id;
    let id = copied.id.clone();
    design.layouts.push(copied);
    Ok(id)
}

fn check_slide(slide: &Slide) -> Result<()> {
    if slide.native_source_id.is_some() {
        return Err(Error::Unsupported("authored slide import cannot copy native slide references".into()));
    }
    if slide.review.as_ref().is_some_and(|review| review.modern_threads.is_some()) {
        return Err(Error::Unsupported("authored slide import does not remap modern comment identities or anchors".into()));
    }
    Ok(())
}

fn merge_fonts(deck: &mut Deck, source: &Deck) -> Result<()> {
    for font in &source.embedded_fonts {
        if !font.license_acknowledged {
            return Err(Error::Unsupported("slide import requires explicit embedding and editing license consent for every source font".into()));
        }
        if let Some(existing) = deck.embedded_fonts.iter().find(|existing|
            existing.family.eq_ignore_ascii_case(&font.family) && existing.style == font.style)
        {
            if !existing.license_acknowledged
                || crate::fonts::inspect_base64(&existing.base64)?.sha256 != crate::fonts::inspect_base64(&font.base64)?.sha256
            {
                return Err(Error::Conflict("slide import font family/style has different bytes or license consent".into()));
            }
        } else {
            deck.embedded_fonts.push(font.clone());
        }
    }
    crate::fonts::validate(&deck.embedded_fonts)
}

pub fn import(
    document: &Document,
    expected_revision: u64,
    expected_hash: &str,
    source: &Document,
    source_slide_ids: &[String],
    prefix: &str,
    after: Option<&str>,
) -> Result<TransactionResult> {
    document::verify(document)?;
    document::verify(source)?;
    if document.revision != expected_revision || document.hash != expected_hash {
        return Err(Error::Conflict("stale slide import document revision or content hash".into()));
    }
    if source.origin.is_some() {
        return Err(Error::Unsupported("import_slides supports authored source documents only (origin must be None); native PPTX cross-package copying is unsupported".into()));
    }
    if source_slide_ids.is_empty() || source_slide_ids.len() > 128 {
        return Err(Error::Limit("slide import requires 1-128 selected slides".into()));
    }
    let mut selected_ids = BTreeSet::new();
    for id in source_slide_ids {
        identity(id)?;
        if !selected_ids.insert(id.as_str()) { return Err(Error::Invalid("slide import selection contains duplicate IDs".into())); }
    }
    if prefix.is_empty() || prefix.len() > 24
        || !prefix.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(Error::Invalid("slide import prefix must be 1-24 ASCII letters, digits, hyphens or underscores".into()));
    }
    if (document.deck.width, document.deck.height) != (source.deck.width, source.deck.height) {
        return Err(Error::Unsupported("slide import requires matching source and target canvas dimensions".into()));
    }
    if document.deck.slides.len() + source_slide_ids.len() > document.capacity_profile.limits().slides {
        return Err(Error::Limit("slide import exceeds the target slide capacity".into()));
    }
    let insertion = match after {
        Some(id) => {
            identity(id)?;
            document.deck.slides.iter().position(|slide| slide.id == id)
                .ok_or_else(|| Error::Invalid("slide import insertion anchor not found".into()))? + 1
        }
        None => document.deck.slides.len(),
    };
    let selected = source_slide_ids.iter().map(|id| {
        let slide = source.deck.slides.iter().find(|slide| &slide.id == id)
            .ok_or_else(|| Error::Invalid("selected source slide not found".into()))?;
        check_slide(slide)?;
        Ok(slide)
    }).collect::<Result<Vec<_>>>()?;
    if source.deck.auxiliary_design.is_some()
        && crate::canonical::bytes(&source.deck.auxiliary_design)? != crate::canonical::bytes(&document.deck.auxiliary_design)?
    {
        return Err(Error::Unsupported("slide import cannot replace the target notes or handout masters".into()));
    }
    let mut used = BTreeSet::new();
    reserve_ids(&document.deck, &mut used);
    used.extend(document.parts.iter().map(|part| part.slide_id.clone()));
    used.extend(document.bindings.iter().map(|binding| binding.slide_id.clone()));
    if let Some(origin) = document.origin.as_ref().filter(|origin| origin.native) {
        let bytes = STANDARD.decode(&origin.base64).map_err(|_| Error::Invalid("target origin base64".into()))?;
        let package = crate::package::Package::open(bytes)?;
        reserve_ids(&crate::native::read(&package)?.deck, &mut used);
    }
    let mut deck = document.deck.clone();
    merge_fonts(&mut deck, &source.deck)?;
    let fallback = fallback_design();
    let source_design = source.deck.design.as_ref().unwrap_or(&fallback);
    if deck.design.is_none() {
        let mut design = fallback_design();
        design.masters[0].id = next_id(&mut used, prefix, "m")?;
        design.layouts[0].id = next_id(&mut used, prefix, "l")?;
        design.layouts[0].master_id = design.masters[0].id.clone();
        deck.design = Some(design);
    }
    let design = deck.design.as_mut().ok_or_else(|| Error::Invalid("target import design missing".into()))?;
    let mut slide_mapping: BTreeMap<String, String> = BTreeMap::new();
    let mut layout_mapping: BTreeMap<String, String> = BTreeMap::new();
    let mut imported = Vec::with_capacity(selected.len());
    for slide in selected {
        let layout = match &slide.layout_id {
            Some(id) => source_design.layouts.iter().find(|layout| &layout.id == id),
            None => source_design.layouts.first(),
        }.ok_or_else(|| Error::Invalid("selected source slide layout missing".into()))?;
        let layout_id = match layout_mapping.get(&layout.id) {
            Some(id) => id.clone(),
            None => {
                let id = import_layout(design, source_design, layout, &mut used, prefix)?;
                layout_mapping.insert(layout.id.clone(), id.clone());
                id
            }
        };
        let mut copied = slide.clone();
        copied.id = next_id(&mut used, prefix, "s")?;
        copied.layout_id = Some(layout_id);
        if slide.layout_id.is_none() {
            copied.background = crate::design::slide_background(slide, source.deck.design.as_ref()).into();
            copied.inherit_background = false;
        }
        if let Some(review) = &mut copied.review {
            for comment in &mut review.comments {
                comment.native_author_id = None;
                comment.native_index = None;
            }
        }
        slide_mapping.insert(slide.id.clone(), copied.id.clone());
        imported.push(copied);
    }
    deck.slides.splice(insertion..insertion, imported);
    let mut parts = document.parts.clone();
    for part in &source.parts {
        let Some(slide_id) = slide_mapping.get(&part.slide_id) else { continue; };
        if part.stale || part.native_sha256.is_some() {
            return Err(Error::Conflict("stale or native source part metadata cannot be imported".into()));
        }
        let mut copied = part.clone();
        copied.slide_id = slide_id.clone();
        copied.native_sha256 = None;
        parts.push(copied);
    }
    let mut sources = document.sources.clone();
    let mut bindings = document.bindings.clone();
    for binding in &source.bindings {
        let Some(slide_id) = slide_mapping.get(&binding.slide_id) else { continue; };
        if binding.stale { return Err(Error::Conflict("stale source bindings cannot be imported".into())); }
        let record = source.sources.iter().find(|record| record.id == binding.source_id)
            .ok_or_else(|| Error::Conflict("source binding record missing".into()))?;
        if let Some(existing) = sources.iter().find(|existing| existing.id == record.id) {
            if existing.sha256 != record.sha256 {
                return Err(Error::Conflict("slide import source ID has a different SHA-256".into()));
            }
        } else {
            sources.push(record.clone());
        }
        let mut copied = binding.clone();
        copied.slide_id = slide_id.clone();
        bindings.push(copied);
    }
    document::transact(document, Transaction {
        expected_revision,
        expected_hash: expected_hash.into(),
        operations: serde_json::from_value(json!([
            {"op":"replace", "path":"/deck", "value":deck},
            {"op":"add", "path":"/parts", "value":parts},
            {"op":"replace", "path":"/sources", "value":sources},
            {"op":"replace", "path":"/bindings", "value":bindings},
            {"op":"add", "path":"/report", "value":null},
        ]))?,
    })
}