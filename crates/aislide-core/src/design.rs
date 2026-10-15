use crate::{model::{Deck, Element, Slide, TextFormat, Placeholder, PlaceholderKind, valid_text, validate_deck}, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const COLOR_KEYS: [&str; 12] = ["dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink", "folHlink"];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeFonts { pub major: String, pub minor: String, pub east_asian: String, pub complex_script: String }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme { pub name: String, pub colors: BTreeMap<String, String>, pub fonts: ThemeFonts }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Master { pub id: String, pub name: String, pub background: String, pub elements: Vec<Element>, #[serde(default, skip_serializing_if = "Option::is_none")] pub theme: Option<Theme> }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlideLayout { pub id: String, pub name: String, pub master_id: String, #[serde(default)] pub background: Option<String>, pub elements: Vec<Element> }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Design { pub theme: Theme, pub masters: Vec<Master>, pub layouts: Vec<SlideLayout> }

impl Default for Theme {
    fn default() -> Self {
        Self { name: "AISlide Report".into(), colors: COLOR_KEYS.into_iter().zip(["202525", "FFFFFF", "586563", "EDF3F0", "087F73", "CF5847", "416CA5", "C68B19", "677C54", "9A506B", "0066CC", "734C8C"]).map(|(key, value)| (key.into(), value.into())).collect(),
            fonts: ThemeFonts { major: "Aptos".into(), minor: "Aptos".into(), east_asian: "Yu Gothic".into(), complex_script: "Arial".into() } }
    }
}

fn placeholder(id: &str, kind: PlaceholderKind, index: u32, bounds: [f64; 4], size: f64, text: &str) -> Element {
    Element::Text { visual: None, id: id.into(), x: bounds[0], y: bounds[1], width: bounds[2], height: bounds[3], text: text.into(), font_size: size, color: "@dk1".into(), bold: kind == PlaceholderKind::Title,
        format: TextFormat { font_family: Some(if kind == PlaceholderKind::Title { "@major" } else { "@minor" }.into()), placeholder: Some(Placeholder { kind, index }), ..Default::default() } }
}

impl Default for Design {
    fn default() -> Self {
        let title = || placeholder("title", PlaceholderKind::Title, 0, [64.0, 80.0, 1152.0, 120.0], 42.0, "Title");
        Self { theme: Theme::default(), masters: vec![Master { id: "master-1".into(), name: "Main master".into(), background: "@lt1".into(), elements: Vec::new(), theme: None }], layouts: vec![
            SlideLayout { id: "blank".into(), name: "Blank".into(), master_id: "master-1".into(), background: None, elements: Vec::new() },
            SlideLayout { id: "title-content".into(), name: "Title and content".into(), master_id: "master-1".into(), background: None, elements: vec![title(), placeholder("body", PlaceholderKind::Body, 1, [64.0, 230.0, 1152.0, 370.0], 28.0, "Content")] },
            SlideLayout { id: "two-columns".into(), name: "Two columns".into(), master_id: "master-1".into(), background: None, elements: vec![title(), placeholder("body-left", PlaceholderKind::Body, 1, [64.0, 230.0, 548.0, 370.0], 26.0, "Left content"), placeholder("body-right", PlaceholderKind::Body, 2, [668.0, 230.0, 548.0, 370.0], 26.0, "Right content")] },
            SlideLayout { id: "section".into(), name: "Section header".into(), master_id: "master-1".into(), background: Some("@lt2".into()), elements: vec![placeholder("title", PlaceholderKind::Title, 0, [100.0, 250.0, 1080.0, 200.0], 56.0, "Section")] },
        ] }
    }
}

pub fn validate_theme(theme: &Theme) -> Result<()> {
    valid_text(&theme.name, 80)?;
    if theme.colors.len() != COLOR_KEYS.len() || COLOR_KEYS.iter().any(|key| !theme.colors.contains_key(*key)) { return Err(Error::Invalid("theme requires the twelve OOXML color slots".into())); }
    for value in theme.colors.values() { if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) { return Err(Error::Invalid("theme slots must contain six-digit RGB colors".into())); } }
    for font in [&theme.fonts.major, &theme.fonts.minor, &theme.fonts.east_asian, &theme.fonts.complex_script] {
        valid_text(font, 100)?; if font.trim().is_empty() || font.chars().any(char::is_control) { return Err(Error::Invalid("theme font names must be nonempty and contain no control characters".into())); }
    }
    Ok(())
}

pub fn validate_design(design: &Design) -> Result<()> {
    validate_design_on_canvas(design, 1280, 720)
}

pub fn validate_design_on_canvas(design: &Design, width: u32, height: u32) -> Result<()> {
    crate::canvas::validate_size(width, height)?;
    validate_theme(&design.theme)?;
    if design.masters.is_empty() || design.masters.len() > 8 || design.layouts.is_empty() || design.layouts.len() > 32 { return Err(Error::Limit("design requires 1-8 masters and 1-32 layouts".into())); }
    for elements in design.masters.iter().map(|master| master.elements.as_slice()).chain(design.layouts.iter().map(|layout| layout.elements.as_slice())) {
        if crate::model::element_list(elements).iter().any(|element| matches!(element, Element::Text { format, .. } if format.inherit_layout)) { return Err(Error::Invalid("master/layout templates cannot use slide-only inherit_layout".into())); }
    }
    let mut ids = BTreeSet::new(); let mut total = 0; let mut images = 0;
    for master in &design.masters {
        if let Some(theme) = &master.theme { validate_theme(theme)?; }
        valid_text(&master.id, 80)?; valid_text(&master.name, 100)?; crate::model::valid_color(&master.background)?;
        if master.id.is_empty() || !ids.insert(master.id.clone()) || !design.layouts.iter().any(|layout| layout.master_id == master.id) { return Err(Error::Invalid("master IDs must be unique and each master must own a layout".into())); }
        crate::model::validate_elements(&master.elements, (f64::from(width), f64::from(height)), 0, &mut BTreeSet::new(), &mut total, &mut images)?;
    }
    let mut layouts = BTreeSet::new();
    for layout in &design.layouts {
        valid_text(&layout.id, 80)?; valid_text(&layout.name, 100)?;
        if layout.id.is_empty() || !layouts.insert(&layout.id) || !ids.contains(&layout.master_id) { return Err(Error::Invalid("layout ID or master reference is invalid".into())); }
        if let Some(background) = &layout.background { crate::model::valid_color(background)?; }
        crate::model::validate_elements(&layout.elements, (f64::from(width), f64::from(height)), 0, &mut BTreeSet::new(), &mut total, &mut images)?;
        let mut placeholders = BTreeSet::new();
        for element in &layout.elements { if let Element::Text { format, .. } = element { if let Some(placeholder) = &format.placeholder { if !placeholders.insert(placeholder.index) { return Err(Error::Invalid("layout placeholder indices must be unique".into())); } } } }
    }
    Ok(())
}

fn inherit(existing: &Element, template: &Element) -> Element {
    match (existing, template) {
        (Element::Text { id, text, format: existing_format, .. }, Element::Text { .. }) => {
            let mut result = template.clone();
            if let Element::Text { id: new_id, text: new_text, format, .. } = &mut result {
                *new_id = id.clone(); *new_text = text.clone(); format.inherit_layout = true;
                format.paragraphs = existing_format.paragraphs.clone();
            }
            result
        }
        _ => existing.clone(),
    }
}

pub(crate) fn validate_inheritance(slide: &Slide, design: Option<&Design>) -> Result<()> {
    let layout = design.and_then(|design| slide.layout_id.as_ref().and_then(|id| design.layouts.iter().find(|layout| &layout.id == id)).or_else(|| design.layouts.first()));
    let mut placeholders = BTreeSet::new();
    for element in &slide.elements {
        if let Element::Text { format, .. } = element {
            if let Some(placeholder) = &format.placeholder { if !placeholders.insert(placeholder.index) { return Err(Error::Invalid("slide placeholder indices must be unique".into())); } }
            if format.inherit_layout {
                let template = layout.and_then(|layout| layout.elements.iter().find(|entry| matches!(entry, Element::Text { format: other, .. } if format.placeholder.is_some() && other.placeholder == format.placeholder)))
                    .ok_or_else(|| Error::Invalid("inherited text requires a matching layout placeholder".into()))?;
                if crate::canonical::bytes(element)? != crate::canonical::bytes(&inherit(element, template))? { return Err(Error::Invalid("detach inherit_layout before individual geometry/style edits, or use update_design/assign_layout".into())); }
            }
        }
    }
    Ok(())
}

pub fn update_design(mut deck: Deck, design: Design) -> Result<Deck> {
    validate_design_on_canvas(&design, deck.width, deck.height)?;
    let previous = deck.design.replace(design);
    crate::fields::propagate_defaults(previous.as_ref(), &mut deck)?;
    let design = deck.design.take().ok_or_else(|| Error::Invalid("design missing".into()))?;
    for slide in &mut deck.slides {
        if let Some(layout) = slide.layout_id.as_ref().and_then(|id| design.layouts.iter().find(|layout| &layout.id == id)) {
            for element in &mut slide.elements {
                if let Element::Text { format, .. } = element {
                    if format.inherit_layout {
                        if let Some(template) = layout.elements.iter().find(|template| matches!(template, Element::Text { format: other, .. } if other.placeholder == format.placeholder && other.placeholder.is_some())) { *element = inherit(element, template); }
                        else { format.inherit_layout = false; format.placeholder = None; }
                    }
                }
            }
        }
    }
    deck.design = Some(design); crate::fields::materialize(&mut deck)?; validate_deck(&deck)?; Ok(deck)
}

pub fn assign_layout(deck: Deck, slide_id: &str, layout_id: &str) -> Result<Deck> {
    assign_layout_with_options(deck, slide_id, layout_id, false)
}

pub fn assign_layout_with_options(mut deck: Deck, slide_id: &str, layout_id: &str, preserve_freeform: bool) -> Result<Deck> {
    let design = deck.design.get_or_insert_with(Design::default);
    let layout = design.layouts.iter().find(|layout| layout.id == layout_id).ok_or_else(|| Error::Invalid("layout not found".into()))?.clone();
    let slide = deck.slides.iter_mut().find(|slide| slide.id == slide_id).ok_or_else(|| Error::Invalid("slide not found".into()))?;
    slide.layout_id = Some(layout.id); slide.inherit_background = true;
    for template in &layout.elements {
        if let Element::Text { format, .. } = template {
            if format.placeholder.is_none() { continue; }
            if let Some(existing) = slide.elements.iter_mut().find(|existing| matches!(existing, Element::Text { format: other, .. } if (!preserve_freeform && existing.bounds().0 == template.bounds().0) || other.placeholder == format.placeholder)) { *existing = inherit(existing, template); }
            else {
                let mut added = inherit(template, template);
                let used: BTreeSet<_> = crate::model::element_list(&slide.elements).into_iter().map(|element| element.bounds().0).collect();
                if used.contains(added.bounds().0) {
                    let id = (1..=257).map(|number| format!("layout-placeholder-{number}")).find(|id| !used.contains(id.as_str())).ok_or_else(|| Error::Limit("layout placeholder IDs exhausted".into()))?;
                    if let Element::Text { id: target, .. } = &mut added { *target = id; }
                }
                slide.elements.push(added);
            }
        }
    }
    for element in &mut slide.elements {
        if let Element::Text { format, .. } = element {
            if format.placeholder.is_some() && !layout.elements.iter().any(|template| matches!(template, Element::Text { format: other, .. } if other.placeholder == format.placeholder)) { format.inherit_layout = false; format.placeholder = None; }
        }
    }
    validate_deck(&deck)?; Ok(deck)
}

pub fn apply_theme(mut deck: Deck, theme: Theme) -> Result<Deck> {
    validate_theme(&theme)?;
    let previous = deck.design.as_ref().map(|design| design.theme.clone()).unwrap_or_default();
    let bind = |color: &mut String| { if let Some((key, _)) = previous.colors.iter().find(|(_, value)| value.eq_ignore_ascii_case(color)) { *color = format!("@{key}"); } };
    fn visit(elements: &mut [Element], bind: &impl Fn(&mut String)) {
        for element in elements { match element {
            Element::Text { color, format, bold, .. } => { bind(color); if format.font_family.is_none() { format.font_family = Some(if *bold { "@major" } else { "@minor" }.into()); } }
            Element::Rect { fill, .. } => bind(fill),
            Element::Polygon { fill, stroke, .. } => { bind(fill); bind(stroke); }
            Element::Shape { fill, stroke, color, format, .. } => { bind(fill); bind(stroke); bind(color); if format.font_family.is_none() { format.font_family = Some("@minor".into()); } }
            Element::Chart { series, .. } => { for entry in series { bind(&mut entry.color); } }
            Element::Connector { color, .. } => bind(color),
            Element::Group { children, .. } => visit(children, bind),
            _ => {}
        } }
    }
    let explicit_layouts: BTreeSet<_> = deck.design.as_ref().into_iter().flat_map(|design| design.layouts.iter().filter(|layout| design.masters.iter().any(|master| master.id == layout.master_id && master.theme.is_some())).map(|layout| layout.id.clone())).collect();
    let default_layout = deck.design.as_ref().and_then(|design| design.layouts.first()).map(|layout| layout.id.clone());
    for slide in &mut deck.slides {
        if slide.layout_id.as_ref().or(default_layout.as_ref()).is_some_and(|id| explicit_layouts.contains(id)) { continue; }
        bind(&mut slide.background); visit(&mut slide.elements, &bind);
    }
    let design = deck.design.get_or_insert_with(Design::default); design.theme = theme;
    for master in design.masters.iter_mut().filter(|master| master.theme.is_none()) { bind(&mut master.background); visit(&mut master.elements, &bind); }
    for layout in design.layouts.iter_mut().filter(|layout| !explicit_layouts.contains(&layout.id)) { if let Some(background) = &mut layout.background { bind(background); } visit(&mut layout.elements, &bind); }
    validate_deck(&deck)?; Ok(deck)
}

pub fn slide_background<'a>(slide: &'a Slide, design: Option<&'a Design>) -> &'a str {
    if slide.inherit_background { if let Some(design) = design { if let Some(layout) = slide.layout_id.as_ref().and_then(|id| design.layouts.iter().find(|layout| &layout.id == id)) { return layout.background.as_deref().or_else(|| design.masters.iter().find(|master| master.id == layout.master_id).map(|master| master.background.as_str())).unwrap_or(&slide.background); } } }
    &slide.background
}

pub fn resolve_color(value: &str, theme: Option<&Theme>) -> String {
    value.strip_prefix('@').and_then(|key| theme.and_then(|theme| theme.colors.get(key)).cloned().or_else(|| Theme::default().colors.get(key).cloned())).unwrap_or_else(|| value.into())
}

pub fn master_theme<'a>(design: &'a Design, master_id: &str) -> &'a Theme {
    design.masters.iter().find(|master| master.id == master_id).and_then(|master| master.theme.as_ref()).unwrap_or(&design.theme)
}

pub fn slide_theme<'a>(slide: &Slide, design: Option<&'a Design>) -> Option<&'a Theme> {
    design.map(|design| {
        let layout = slide.layout_id.as_ref().and_then(|id| design.layouts.iter().find(|layout| &layout.id == id)).or_else(|| design.layouts.first());
        layout.map_or(&design.theme, |layout| master_theme(design, &layout.master_id))
    })
}

pub fn set_master_theme(mut deck: Deck, master_id: &str, theme: Option<Theme>) -> Result<Deck> {
    if let Some(theme) = &theme { validate_theme(theme)?; }
    let master = deck.design.as_mut().and_then(|design| design.masters.iter_mut().find(|master| master.id == master_id)).ok_or_else(|| Error::Invalid("master not found".into()))?;
    master.theme = theme;
    validate_deck(&deck)?;
    Ok(deck)
}