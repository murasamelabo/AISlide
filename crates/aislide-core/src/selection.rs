//! Pure structural selection mechanics, not a native XML clipboard or a transaction.
//! `apply` validates input and a cloned candidate; callers own revision/history, part
//! metadata, bindings, and native preservation. Never commit without reviewing effects.
//! Resize uses independent coordinate scales and their minimum for fonts/strokes.
//! Ungroup removes one selected group level and normalizes descendant view spaces.
//! Paste offsets are relative to the original slide coordinates, never clamped.
//! Generate the operation contract with `schemars::schema_for!(SelectionOperation)`;
//! the separate serde `ElementBundle` carries structural elements and source theme.

use crate::{design::{Theme, resolve_color}, model::{Deck, Element, TextFormat, element_list, validate_deck, valid_text}, Error, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Alignment { Left, Center, Right, Top, Middle, Bottom }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RelativeTo { Selection, Page }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Axis { Horizontal, Vertical }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardFormat { KeepSourceFormatting }

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum SelectionOperation {
    Translate { ids: Vec<String>, dx: f64, dy: f64 },
    Resize { ids: Vec<String>, x: f64, y: f64, width: f64, height: f64 },
    Align { ids: Vec<String>, alignment: Alignment, relative_to: RelativeTo },
    Distribute { ids: Vec<String>, axis: Axis, relative_to: RelativeTo },
    Group { ids: Vec<String>, group_id: String },
    Ungroup { ids: Vec<String> },
    Copy { ids: Vec<String>, format: ClipboardFormat },
    Cut { ids: Vec<String>, format: ClipboardFormat },
    Paste { id_prefix: String, dx: f64, dy: f64 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElementBundle {
    pub version: u32,
    pub format: ClipboardFormat,
    pub source_slide_id: String,
    pub source_theme: Theme,
    pub elements: Vec<Element>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_document: Option<ClipboardSource>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipboardSource {
    pub id: String,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_sha256: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectionEffects {
    pub affected_ids: Vec<String>,
    pub added_ids: Vec<String>,
    pub removed_ids: Vec<String>,
    pub reparented_ids: Vec<String>,
    pub id_map: BTreeMap<String, String>,
    pub metadata_review_required: bool,
    pub native_validation_required: bool,
    pub raw_native_preserved: bool,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SelectionResult {
    pub deck: Deck,
    pub clipboard: Option<ElementBundle>,
    pub effects: SelectionEffects,
}

/// Select only unique top-level IDs in one slide. All operations are atomic.
/// Copy/cut emit a detached structural bundle. Paste requires that bundle as a
/// separate argument; other operations reject an unexpected clipboard argument.
/// Effects include descendants, so the parent can reject/detach/rebuild partial
/// managed parts and reconcile source bindings in the same transaction.
pub fn apply(deck: &Deck, slide_id: &str, operation: &SelectionOperation, clipboard: Option<&ElementBundle>) -> Result<SelectionResult> {
    validate_deck(deck)?;
    let slide_index = deck.slides.iter().position(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("selection slide not found".into()))?;
    let original = &deck.slides[slide_index].elements;
    let mut candidate = deck.clone();
    let elements = &mut candidate.slides[slide_index].elements;
    let mut effects = SelectionEffects::default();
    let mut output_clipboard = None;
    if let SelectionOperation::Paste { id_prefix, dx, dy } = operation {
        finite(&[*dx, *dy])?;
        identity(id_prefix, 60)?;
        let bundle = clipboard.ok_or_else(|| Error::Invalid("paste requires an element bundle".into()))?;
        validate_bundle(bundle)?;
        let theme = deck.design.as_ref().map(|design| design.theme.clone()).unwrap_or_default();
        compatible_theme(&bundle.elements, &bundle.source_theme, &theme)?;
        let mut copied = bundle.elements.clone();
        freeze_formatting(&mut copied, &bundle.source_theme)?;
        let mut used: BTreeSet<String> = deck.slides.iter().flat_map(|slide| element_list(&slide.elements)).map(|element| element.bounds().0.to_owned()).collect();
        used.extend(element_list(&bundle.elements).iter().map(|element| element.bounds().0.to_owned()));
        let mut serial = 1usize;
        for element in element_list(&copied) {
            let next = loop {
                let next = format!("{id_prefix}-{serial}");
                serial = serial.checked_add(1).ok_or_else(|| Error::Limit("clipboard IDs exhausted".into()))?;
                if used.insert(next.clone()) { break next; }
            };
            effects.id_map.insert(element.bounds().0.into(), next);
        }
        remap(&mut copied, &effects.id_map)?;
        for element in &mut copied { translate(element, *dx, *dy)?; }
        elements.extend(copied);
    } else {
        if clipboard.is_some() { return Err(Error::Invalid("only paste accepts a clipboard bundle".into())); }
        let ids = match operation {
            SelectionOperation::Translate { ids, .. } | SelectionOperation::Resize { ids, .. }
            | SelectionOperation::Align { ids, .. } | SelectionOperation::Distribute { ids, .. }
            | SelectionOperation::Group { ids, .. } | SelectionOperation::Ungroup { ids }
            | SelectionOperation::Copy { ids, .. } | SelectionOperation::Cut { ids, .. } => ids,
            SelectionOperation::Paste { .. } => unreachable!(),
        };
        let selected = select(original, ids)?;
        let bounds = selection_bounds(original, &selected);
        effects.affected_ids = selected.iter().flat_map(|index| element_list(&original[*index..=*index])).map(|element| element.bounds().0.into()).collect();
        match operation {
            SelectionOperation::Translate { dx, dy, .. } => {
                finite(&[*dx, *dy])?;
                for index in selected { translate(&mut elements[index], *dx, *dy)?; }
            }
            SelectionOperation::Resize { x, y, width, height, .. } => {
                finite(&[*x, *y, *width, *height])?;
                if *x < 0.0 || *y < 0.0 || *width <= 0.0 || *height <= 0.0 { return Err(Error::Invalid("resize requires positive size and nonnegative origin".into())); }
                let scale = [width / bounds[2], height / bounds[3]];
                for index in selected { transform(&mut elements[index], [bounds[0], bounds[1]], [*x, *y], scale)?; }
            }
            SelectionOperation::Align { alignment, relative_to, .. } => {
                let frame = reference(bounds, *relative_to, deck);
                for index in selected {
                    let (_, x, y, width, height) = elements[index].bounds();
                    let (next_x, next_y) = match alignment {
                        Alignment::Left => (frame[0], y),
                        Alignment::Center => (frame[0] + (frame[2] - width) / 2.0, y),
                        Alignment::Right => (frame[0] + frame[2] - width, y),
                        Alignment::Top => (x, frame[1]),
                        Alignment::Middle => (x, frame[1] + (frame[3] - height) / 2.0),
                        Alignment::Bottom => (x, frame[1] + frame[3] - height),
                    };
                    translate(&mut elements[index], next_x - x, next_y - y)?;
                }
            }
            SelectionOperation::Distribute { axis, relative_to, .. } => {
                if selected.len() < 3 { return Err(Error::Invalid("distribution requires at least three elements".into())); }
                let frame = reference(bounds, *relative_to, deck);
                let horizontal = matches!(axis, Axis::Horizontal);
                let coordinate = |element: &Element| { let (_, x, y, width, height) = element.bounds(); if horizontal { (x, width) } else { (y, height) } };
                let mut ordered = selected;
                ordered.sort_by(|left, right| coordinate(&elements[*left]).0.total_cmp(&coordinate(&elements[*right]).0));
                let total: f64 = ordered.iter().map(|index| coordinate(&elements[*index]).1).sum();
                let span = if horizontal { frame[2] } else { frame[3] };
                if total > span { return Err(Error::Invalid("distribution frame cannot fit nonoverlapping elements".into())); }
                let gap = (span - total) / (ordered.len() - 1) as f64;
                let mut cursor = if horizontal { frame[0] } else { frame[1] };
                for index in ordered {
                    let (position, size) = coordinate(&elements[index]);
                    translate(&mut elements[index], if horizontal { cursor - position } else { 0.0 }, if horizontal { 0.0 } else { cursor - position })?;
                    cursor += size + gap;
                }
            }
            SelectionOperation::Group { group_id, .. } => {
                identity(group_id, 80)?;
                if selected.len() < 2 { return Err(Error::Invalid("group requires at least two elements".into())); }
                if element_list(original).iter().any(|element| element.bounds().0 == group_id) { return Err(Error::Conflict("group ID already exists".into())); }
                let mut children: Vec<_> = selected.iter().map(|index| original[*index].clone()).collect();
                for child in &mut children { translate(child, -bounds[0], -bounds[1])?; detach(child); }
                let group = Element::Group { visual: None, id: group_id.clone(), x: bounds[0], y: bounds[1], width: bounds[2], height: bounds[3], view_width: bounds[2], view_height: bounds[3], children };
                let first = selected[0];
                for index in selected.iter().rev() { elements.remove(*index); }
                elements.insert(first, group);
            }
            SelectionOperation::Ungroup { .. } => {
                for index in selected.into_iter().rev() {
                    let Element::Group { x, y, width, height, view_width, view_height, children, .. } = &elements[index] else {
                        return Err(Error::Invalid("ungroup requires only group IDs".into()));
                    };
                    let mut flattened = children.clone();
                    for child in &mut flattened {
                        transform(child, [0.0, 0.0], [*x, *y], [width / view_width, height / view_height])?;
                        detach(child);
                    }
                    elements.splice(index..=index, flattened);
                }
            }
            SelectionOperation::Copy { format, .. } | SelectionOperation::Cut { format, .. } => {
                let source_theme = deck.design.as_ref().map(|design| design.theme.clone()).unwrap_or_default();
                let mut copied: Vec<_> = selected.iter().map(|index| original[*index].clone()).collect();
                freeze_formatting(&mut copied, &source_theme)?;
                let bundle = ElementBundle { version: 1, format: *format, source_slide_id: slide_id.into(), source_theme, elements: copied, source_document: None };
                validate_bundle(&bundle)?;
                output_clipboard = Some(bundle);
                if matches!(operation, SelectionOperation::Cut { .. }) {
                    for index in selected.into_iter().rev() { elements.remove(index); }
                }
            }
            SelectionOperation::Paste { .. } => unreachable!(),
        }
    }
    validate_deck(&candidate)?;
    let before = containers(original);
    let after = containers(&candidate.slides[slide_index].elements);
    effects.added_ids = after.keys().filter(|id| !before.contains_key(*id)).cloned().collect();
    effects.removed_ids = before.keys().filter(|id| !after.contains_key(*id)).cloned().collect();
    effects.reparented_ids = before.iter().filter(|(id, parent)| after.get(*id).is_some_and(|next| next != *parent)).map(|(id, _)| id.clone()).collect();
    if matches!(operation, SelectionOperation::Paste { .. }) { effects.affected_ids = effects.added_ids.clone(); }
    effects.metadata_review_required = true;
    effects.native_validation_required = true;
    effects.warnings.push("Structural model only: parent must check native preservation, native numeric ID collisions, managed parts (including partial selections), source bindings, and history before committing.".into());
    if output_clipboard.is_some() || matches!(operation, SelectionOperation::Paste { .. }) {
        effects.warnings.push("Keep-source-formatting resolves representable colors and Latin fonts; implicit script fonts and table/chart styles require a compatible destination theme. Bundles do not contain raw XML or part/binding metadata.".into());
    }
    if matches!(operation, SelectionOperation::Resize { .. } | SelectionOperation::Ungroup { .. }) {
        effects.warnings.push("Fonts and strokes scale by the smaller axis ratio without clamping; implicit chart label sizes and rotated-shape visual bounds are not modeled.".into());
    }
    Ok(SelectionResult { deck: candidate, clipboard: output_clipboard, effects })
}

/// Parent-side gate for a complete numeric ID allocation before native writes.
/// Includes nested IDs and checks the entire allocation (also retained/deleted
/// native nodes). Root ID 1 is reserved. This cannot inspect unmodeled raw XML;
/// callers must supply every reserved native ID and run their preservation gate.
pub fn validate_native_ids(elements: &[Element], allocation: &BTreeMap<String, u32>) -> Result<()> {
    validate_native_ids_on_canvas(elements, allocation, 1280, 720)
}

pub fn validate_native_ids_on_canvas(elements: &[Element], allocation: &BTreeMap<String, u32>, width: u32, height: u32) -> Result<()> {
    crate::canvas::validate_size(width, height)?;
    crate::model::validate_elements(elements, (f64::from(width), f64::from(height)), 0, &mut BTreeSet::new(), &mut 0, &mut 0)?;
    let mut numbers = BTreeSet::new();
    for number in allocation.values() {
        if *number <= 1 || !numbers.insert(*number) { return Err(Error::Conflict("native shape IDs collide or use a reserved root ID".into())); }
    }
    let mut names = BTreeSet::new();
    for element in element_list(elements) {
        let id = element.bounds().0;
        if !names.insert(id) || !allocation.contains_key(id) { return Err(Error::Conflict("native ID allocation must cover unique recursive element IDs".into())); }
    }
    Ok(())
}

fn select(elements: &[Element], ids: &[String]) -> Result<Vec<usize>> {
    if ids.is_empty() || ids.len() > 256 { return Err(Error::Limit("selection requires 1-256 IDs".into())); }
    let unique: BTreeSet<_> = ids.iter().map(String::as_str).collect();
    if unique.len() != ids.len() { return Err(Error::Invalid("duplicate selection ID".into())); }
    let selected: Vec<_> = elements.iter().enumerate().filter(|(_, element)| unique.contains(element.bounds().0)).map(|(index, _)| index).collect();
    if selected.len() != ids.len() { return Err(Error::Invalid("selection IDs must all exist at the top level of the same slide".into())); }
    Ok(selected)
}

fn selection_bounds(elements: &[Element], selected: &[usize]) -> [f64; 4] {
    let mut left = f64::INFINITY; let mut top = f64::INFINITY;
    let mut right = f64::NEG_INFINITY; let mut bottom = f64::NEG_INFINITY;
    for index in selected {
        let (_, x, y, width, height) = elements[*index].bounds();
        left = left.min(x); top = top.min(y); right = right.max(x + width); bottom = bottom.max(y + height);
    }
    [left, top, right - left, bottom - top]
}

fn reference(bounds: [f64; 4], relative_to: RelativeTo, deck: &Deck) -> [f64; 4] {
    match relative_to { RelativeTo::Selection => bounds, RelativeTo::Page => [0.0, 0.0, deck.width as f64, deck.height as f64] }
}

fn finite(values: &[f64]) -> Result<()> {
    if values.iter().any(|value| !value.is_finite()) { return Err(Error::Invalid("selection geometry must be finite".into())); }
    Ok(())
}

fn identity(id: &str, limit: usize) -> Result<()> {
    valid_text(id, limit)?;
    if id.trim().is_empty() { return Err(Error::Invalid("selection identity cannot be empty".into())); }
    Ok(())
}

fn geometry_mut(element: &mut Element) -> (&mut String, &mut f64, &mut f64, &mut f64, &mut f64) {
    match element {
        Element::Text { id, x, y, width, height, .. } | Element::Rect { id, x, y, width, height, .. }
        | Element::Polygon { id, x, y, width, height, .. } | Element::Shape { id, x, y, width, height, .. }
        | Element::Table { id, x, y, width, height, .. } | Element::Chart { id, x, y, width, height, .. }
        | Element::Picture { id, x, y, width, height, .. } | Element::Connector { id, x, y, width, height, .. }
        | Element::Group { id, x, y, width, height, .. } => (id, x, y, width, height),
    }
}

fn detach(element: &mut Element) {
    if let Element::Text { format, .. } | Element::Shape { format, .. } = element {
        format.placeholder = None; format.inherit_layout = false;
    }
}

fn translate(element: &mut Element, dx: f64, dy: f64) -> Result<()> {
    let (_, x, y, _, _) = geometry_mut(element);
    *x += dx; *y += dy;
    finite(&[*x, *y])?;
    if *x < 0.0 || *y < 0.0 { return Err(Error::Invalid("selection would have negative coordinates".into())); }
    if dx != 0.0 || dy != 0.0 { detach(element); }
    Ok(())
}

pub(crate) fn transform(element: &mut Element, origin: [f64; 2], destination: [f64; 2], scale: [f64; 2]) -> Result<()> {
    finite(&scale)?;
    if scale.iter().any(|value| *value <= 0.0) { return Err(Error::Invalid("selection scale must be positive".into())); }
    let (_, x, y, width, height) = geometry_mut(element);
    *x = destination[0] + (*x - origin[0]) * scale[0];
    *y = destination[1] + (*y - origin[1]) * scale[1];
    *width *= scale[0]; *height *= scale[1];
    finite(&[*x, *y, *width, *height])?;
    if *x < 0.0 || *y < 0.0 { return Err(Error::Invalid("selection would have negative coordinates".into())); }
    let scalar = scale[0].min(scale[1]);
    match element {
        Element::Group { width, height, view_width, view_height, children, .. } => {
            let child_scale = [*width / *view_width, *height / *view_height];
            for child in children { transform(child, [0.0, 0.0], [0.0, 0.0], child_scale)?; }
            *view_width = *width; *view_height = *height;
        }
        Element::Text { font_size, format, .. } => { *font_size *= scalar; scale_paragraphs(format, scale)?; }
        Element::Table { font_size, .. } => *font_size *= scalar,
        Element::Shape { font_size, stroke_width, format, .. } => { *font_size *= scalar; *stroke_width *= scalar; scale_paragraphs(format, scale)?; }
        Element::Polygon { stroke_width, .. } | Element::Connector { stroke_width, .. } => *stroke_width *= scalar,
        Element::Rect { .. } | Element::Chart { .. } | Element::Picture { .. } => {}
    }
    detach(element);
    Ok(())
}

fn scale_paragraphs(format: &mut TextFormat, scale: [f64; 2]) -> Result<()> {
    let scalar = scale[0].min(scale[1]);
    let integer = |value: i32, factor: f64| -> Result<i32> {
        let result = (f64::from(value) * factor).round();
        if !result.is_finite() || result < i32::MIN as f64 || result > i32::MAX as f64 { return Err(Error::Limit("scaled paragraph geometry overflows".into())); }
        Ok(result as i32)
    };
    for paragraph in &mut format.paragraphs {
        for run in &mut paragraph.runs {
            if let Some(size) = &mut run.style.font_size {
                *size *= scalar;
                if !size.is_finite() || !(8.0..=120.0).contains(size) { return Err(Error::Invalid("scaled run font size must be 8-120 pixels".into())); }
            }
        }
        for value in [&mut paragraph.margin_left, &mut paragraph.indent].into_iter().flatten() { *value = integer(*value, scale[0])?; }
        for tab in &mut paragraph.tabs { tab.position = integer(tab.position, scale[0])?; }
        for spacing in [&mut paragraph.line_spacing, &mut paragraph.space_before, &mut paragraph.space_after].into_iter().flatten() {
            if let crate::rich_text::Spacing::Points(value) = spacing {
                let result = (f64::from(*value) * scalar).round();
                if !result.is_finite() || !(0.0..=158400.0).contains(&result) { return Err(Error::Limit("scaled paragraph spacing out of range".into())); }
                *value = result as u32;
            }
        }
    }
    Ok(())
}

fn containers(elements: &[Element]) -> BTreeMap<String, Option<String>> {
    fn visit(elements: &[Element], parent: Option<&str>, result: &mut BTreeMap<String, Option<String>>) {
        for element in elements {
            let id = element.bounds().0;
            result.insert(id.into(), parent.map(str::to_owned));
            if let Element::Group { children, .. } = element { visit(children, Some(id), result); }
        }
    }
    let mut result = BTreeMap::new(); visit(elements, None, &mut result); result
}

fn validate_bundle(bundle: &ElementBundle) -> Result<()> {
    if bundle.version != 1 { return Err(Error::Unsupported("clipboard bundle version".into())); }
    if let Some(source) = &bundle.source_document {
        identity(&source.id, 80)?;
        for digest in std::iter::once(&source.hash).chain(source.origin_sha256.iter()) {
            if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) { return Err(Error::Invalid("clipboard source digest must be SHA-256".into())); }
        }
    }
    identity(&bundle.source_slide_id, 80)?;
    crate::design::validate_theme(&bundle.source_theme)?;
    if bundle.elements.is_empty() { return Err(Error::Invalid("clipboard bundle must not be empty".into())); }
    crate::model::validate_elements(&bundle.elements, (4096.0, 4096.0), 0, &mut BTreeSet::new(), &mut 0, &mut 0)?;
    if element_list(&bundle.elements).iter().any(|element| matches!(element, Element::Text { format, .. } if format.inherit_layout || format.placeholder.is_some())) {
        return Err(Error::Invalid("clipboard text must be detached from layout placeholders".into()));
    }
    Ok(())
}

fn explicit_font(family: Option<&str>, theme: &Theme) -> Result<String> {
    let family = match family {
        Some("@major") => theme.fonts.major.clone(),
        None | Some("@minor") => theme.fonts.minor.clone(),
        Some(family) => family.into(),
    };
    if family.starts_with('@') || family.starts_with("+mj-") || family.starts_with("+mn-") { return Err(Error::Unsupported("clipboard font cannot be resolved explicitly".into())); }
    Ok(family)
}

fn freeze_font(format: &mut TextFormat, theme: &Theme) -> Result<()> {
    format.font_family = Some(explicit_font(format.font_family.as_deref(), theme)?);
    format.placeholder = None; format.inherit_layout = false;
    for paragraph in &mut format.paragraphs {
        for run in &mut paragraph.runs {
            if run.style.font_family.is_some() { run.style.font_family = Some(explicit_font(run.style.font_family.as_deref(), theme)?); }
            for color in [&mut run.style.color, &mut run.style.highlight].into_iter().flatten() { *color = resolve_color(color, Some(theme)); }
        }
    }
    Ok(())
}

fn freeze_formatting(elements: &mut [Element], theme: &Theme) -> Result<()> {
    let resolve = |color: &mut String| { *color = resolve_color(color, Some(theme)); };
    for element in elements {
        match element {
            Element::Text { color, format, .. } => { resolve(color); freeze_font(format, theme)?; }
            Element::Rect { fill, .. } => resolve(fill),
            Element::Polygon { fill, stroke, .. } => { resolve(fill); resolve(stroke); }
            Element::Shape { fill, stroke, color, format, .. } => { resolve(fill); resolve(stroke); resolve(color); freeze_font(format, theme)?; }
            Element::Chart { series, .. } => { for series in series { resolve(&mut series.color); } }
            Element::Connector { color, .. } => resolve(color),
            Element::Group { children, .. } => freeze_formatting(children, theme)?,
            Element::Table { .. } | Element::Picture { .. } => {}
        }
    }
    Ok(())
}

fn compatible_theme(elements: &[Element], source: &Theme, target: &Theme) -> Result<()> {
    let scripts_match = source.fonts.east_asian == target.fonts.east_asian && source.fonts.complex_script == target.fonts.complex_script;
    let colors_match = |keys: &[&str]| keys.iter().all(|key| source.colors[*key].eq_ignore_ascii_case(&target.colors[*key]));
    for element in element_list(elements) {
        let compatible = match element {
            Element::Text { .. } | Element::Shape { .. } => scripts_match,
            Element::Table { .. } => scripts_match && source.fonts.minor == target.fonts.minor && colors_match(&["lt1", "dk1", "accent1", "lt2"]),
            Element::Chart { .. } => scripts_match && source.fonts.major == target.fonts.major && source.fonts.minor == target.fonts.minor && colors_match(&crate::design::COLOR_KEYS),
            _ => true,
        };
        if !compatible { return Err(Error::Unsupported(format!("keep-source-formatting cannot represent implicit script fonts or table/chart theme styles for {}; parent must preserve native styling", element.bounds().0))); }
    }
    Ok(())
}

fn remap(elements: &mut [Element], mapping: &BTreeMap<String, String>) -> Result<()> {
    for element in elements {
        let (id, _, _, _, _) = geometry_mut(element);
        *id = mapping.get(id).ok_or_else(|| Error::Invalid("clipboard ID mapping missing".into()))?.clone();
        match element {
            Element::Connector { start, end, .. } => {
                for connection in [start, end].into_iter().flatten() {
                    connection.element_id = mapping.get(&connection.element_id).ok_or_else(|| Error::Invalid("clipboard connector target is outside the bundle".into()))?.clone();
                }
            }
            Element::Group { children, .. } => remap(children, mapping)?,
            _ => {}
        }
    }
    Ok(())
}