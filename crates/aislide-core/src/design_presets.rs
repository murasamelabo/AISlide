use crate::{design::{Design, Master, SlideLayout, Theme, ThemeFonts, COLOR_KEYS}, model::{Deck, Element, Placeholder, PlaceholderKind, TextAlign, TextFormat}, Error, Result};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Region { pub layout_id: String, pub name: String, pub x: f64, pub y: f64, pub width: f64, pub height: f64 }
#[derive(Clone, Debug, Serialize)]
pub struct Rules { pub margin: f64, pub gutter: f64, pub heading_size: f64, pub body_size: f64, pub regions: Vec<Region> }
#[derive(Clone, Debug, Serialize)]
pub struct Preset { pub id: String, pub name: String, pub rules: Rules, pub design: Design }

struct Style {
    id: &'static str, name: &'static str, margin: f64, gutter: f64, heading: f64, body: f64,
    major: &'static str, minor: &'static str, east_asian: &'static str, colors: [&'static str; 12],
}

const STYLES: [Style; 7] = [
    Style { id: "public", name: "Civic clarity", margin: 64.0, gutter: 48.0, heading: 42.0, body: 26.0, major: "Noto Sans JP", minor: "Noto Sans JP", east_asian: "Noto Sans JP", colors: ["1A1A1C","FFFFFF","414448","F1F5F9","0017C1","007A4D","B54434","987100","52626C","87536B","0017C1","69469B"] },
    Style { id: "minimal", name: "Minimal space", margin: 96.0, gutter: 56.0, heading: 40.0, body: 24.0, major: "Noto Sans JP", minor: "Noto Sans JP", east_asian: "Noto Sans JP", colors: ["242628","FFFFFF","555A5C","F3F5F5","007A74","AD475A","476C9B","947400","686E56","895D89","0066A3","76518A"] },
    Style { id: "stylish", name: "Studio editorial", margin: 72.0, gutter: 40.0, heading: 48.0, body: 24.0, major: "IBM Plex Sans", minor: "Noto Sans JP", east_asian: "Noto Sans JP", colors: ["202124","FFFFFF","4D5356","EDF3F4","006B68","A83965","325E9A","8C7000","5B6D3E","9C573C","0066A3","76518A"] },
    Style { id: "pop", name: "Color play", margin: 56.0, gutter: 32.0, heading: 48.0, body: 26.0, major: "Noto Sans JP", minor: "Noto Sans JP", east_asian: "Noto Sans JP", colors: ["242735","FFFFFF","535665","FFF2F6","C72B67","006B64","1261A0","947100","6A4DAD","B64722","1261A0","734C8C"] },
    Style { id: "dynamic", name: "Bold momentum", margin: 48.0, gutter: 40.0, heading: 60.0, body: 26.0, major: "IBM Plex Sans", minor: "Noto Sans JP", east_asian: "Noto Sans JP", colors: ["151515","FFFFFF","4F5357","F0F2F4","C92D32","006C67","1A589A","927000","586E34","9B4775","1A589A","76518A"] },
    Style { id: "trust", name: "Trusted report", margin: 72.0, gutter: 48.0, heading: 42.0, body: 26.0, major: "IBM Plex Sans", minor: "Noto Sans JP", east_asian: "Noto Sans JP", colors: ["1D2935","FFFFFF","465567","EDF3F8","1D5093","00786B","B33D4B","9A6600","526D43","815781","1D5093","76518A"] },
    Style { id: "luxury", name: "Quiet prestige", margin: 104.0, gutter: 64.0, heading: 44.0, body: 24.0, major: "Yu Mincho", minor: "Yu Gothic", east_asian: "Yu Mincho", colors: ["20221F","FFFFFF","53594F","F3F4F1","7A6435","355A4A","765465","456B7D","786337","725D53","355A4A","765465"] },
];

fn rectangle(id: &str, bounds: [f64; 4], fill: &str) -> Element {
    Element::Rect { id: id.into(), x: bounds[0], y: bounds[1], width: bounds[2], height: bounds[3], fill: fill.into() }
}

fn text(id: &str, bounds: [f64; 4], size: f64, value: &str, kind: PlaceholderKind, index: u32) -> Element {
    Element::Text { id: id.into(), x: bounds[0], y: bounds[1], width: bounds[2], height: bounds[3], text: value.into(), font_size: size, color: "@dk1".into(), bold: kind == PlaceholderKind::Title,
        format: TextFormat { font_family: Some(if kind == PlaceholderKind::Title { "@major" } else { "@minor" }.into()), placeholder: Some(Placeholder { kind, index }), ..Default::default() } }
}

fn layout(id: &str, name: &str, background: Option<&str>, elements: Vec<Element>) -> SlideLayout {
    SlideLayout { id: format!("preset-{id}"), name: name.into(), master_id: "preset-master".into(), background: background.map(str::to_owned), elements }
}

fn build(style: &Style) -> Result<Preset> {
    let margin = style.margin;
    let width = 1280.0 - margin * 2.0;
    let column = (width - style.gutter) / 2.0;
    let body_top = if style.id == "dynamic" { 208.0 } else { 192.0 };
    let body_height = 640.0 - body_top;
    let title = || text("title", [margin, 64.0, width, 112.0], style.heading, "Key message", PlaceholderKind::Title, 0);
    let mut master = Master { id: "preset-master".into(), name: style.name.into(), background: "@lt1".into(), elements: Vec::new() };
    match style.id {
        "minimal" => master.elements.push(rectangle("preset-footer-rule", [margin, 681.0, 48.0, 2.0], "@accent1")),
        "stylish" => {
            master.elements.push(rectangle("preset-side-rule", [32.0, 64.0, 6.0, 112.0], "@accent1"));
            master.elements.push(rectangle("preset-footer-rule", [margin, 680.0, width, 2.0], "@dk1"));
        }
        "pop" => {
            for (index, color) in ["@accent1", "@accent2", "@accent3"].iter().enumerate() {
                master.elements.push(rectangle(&format!("preset-color-{index}"), [margin + index as f64 * 40.0, 681.0, 32.0, 6.0], color));
            }
        }
        "dynamic" => master.elements.push(Element::Polygon { id: "preset-corner".into(), x: 1152.0, y: 0.0, width: 128.0, height: 64.0, points: vec![[0.0,0.0],[1.0,0.0],[1.0,1.0]], fill: "@accent1".into(), stroke: "@dk1".into(), stroke_width: 0.0 }),
        "luxury" => {
            master.elements.push(rectangle("preset-footer-rule", [margin, 676.0, width, 1.0], "@accent1"));
            master.elements.push(rectangle("preset-footer-accent", [628.0, 688.0, 24.0, 2.0], "@accent1"));
        }
        _ => {
            master.elements.push(rectangle("preset-top-rule", [margin, 40.0, if style.id == "public" { 72.0 } else { width }, 4.0], "@accent1"));
            master.elements.push(rectangle("preset-footer-rule", [margin, 680.0, width, 2.0], "@lt2"));
        }
    }
    let cover_width = if style.id == "dynamic" { width * 0.75 } else { width };
    let mut cover = text("title", [margin, 180.0, cover_width, 168.0], style.heading + 16.0, "Presentation title", PlaceholderKind::Title, 0);
    let mut subtitle = text("subtitle", [margin, 384.0, width, 120.0], style.body, "Subtitle", PlaceholderKind::Subtitle, 1);
    if style.id == "luxury" {
        for element in [&mut cover, &mut subtitle] { if let Element::Text { format, .. } = element { format.alignment = TextAlign::Center; } }
    }
    let mut cover_graphics = Vec::new();
    let cover_background = match style.id {
        "stylish" => {
            if let Element::Text { width, color, .. } = &mut cover { *width = 760.0; *color = "@lt1".into(); }
            if let Element::Text { width, color, .. } = &mut subtitle { *width = 760.0; *color = "@lt1".into(); }
            cover_graphics.push(rectangle("preset-cover-panel", [940.0, 128.0, 268.0, 440.0], "@accent1"));
            Some("@dk1")
        }
        "dynamic" => {
            if let Element::Text { color, font_size, .. } = &mut cover { *color = "@lt1".into(); *font_size = 84.0; }
            if let Element::Text { color, .. } = &mut subtitle { *color = "@lt1".into(); }
            cover_graphics.push(rectangle("preset-cover-block", [960.0, 568.0, 272.0, 72.0], "@dk1"));
            Some("@accent1")
        }
        "pop" => {
            if let Element::Text { x, width, .. } = &mut cover { *x += 32.0; *width -= 32.0; }
            if let Element::Text { x, width, .. } = &mut subtitle { *x += 32.0; *width -= 32.0; }
            cover_graphics.push(rectangle("preset-cover-line", [margin, 180.0, 12.0, 312.0], "@accent1"));
            for (index, color) in ["@accent1", "@accent2", "@accent3"].iter().enumerate() {
                cover_graphics.push(rectangle(&format!("preset-cover-tile-{index}"), [margin + index as f64 * 112.0, 552.0, 96.0, 64.0], color));
            }
            Some("@lt2")
        }
        "trust" => {
            if let Element::Text { color, .. } = &mut cover { *color = "@accent1".into(); }
            cover_graphics.push(rectangle("preset-cover-rule", [margin, 356.0, width, 4.0], "@accent1"));
            None
        }
        "luxury" => {
            cover_graphics.push(rectangle("preset-cover-rule", [512.0, 144.0, 256.0, 1.0], "@accent1"));
            None
        }
        _ => None,
    };
    cover_graphics.extend([cover, subtitle]);
    let mut section = text("title", [margin, 244.0, width, 220.0], style.heading + 12.0, "Section title", PlaceholderKind::Title, 0);
    if let Element::Text { color, .. } = &mut section { *color = "@lt1".into(); }
    let card_width = (width - 2.0 * style.gutter) / 3.0;
    let mut cards = vec![title()];
    for index in 0..3 {
        let horizontal = margin + index as f64 * (card_width + style.gutter);
        cards.push(rectangle(&format!("preset-panel-{index}"), [horizontal, body_top, card_width, body_height], "@lt2"));
        cards.push(rectangle(&format!("preset-panel-rule-{index}"), [horizontal, body_top, card_width, if style.id == "pop" { 8.0 } else { 3.0 }], ["@accent1", "@accent2", "@accent3"][index]));
        cards.push(text(&format!("body-{index}"), [horizontal + 20.0, body_top + 32.0, card_width - 40.0, body_height - 56.0], style.body, ["First point", "Second point", "Third point"][index], PlaceholderKind::Body, index as u32 + 1));
    }
    let visual_x = margin + column + style.gutter;
    let layouts = vec![
        layout("blank", "Blank", None, Vec::new()),
        layout("cover", "Cover", cover_background, cover_graphics),
        layout("title-content", "Title and content", None, vec![title(), text("body", [margin, body_top, width, body_height], style.body, "Supporting content", PlaceholderKind::Body, 1)]),
        layout("two-columns", "Two columns", None, vec![title(), text("body-left", [margin, body_top, column, body_height], style.body, "Left content", PlaceholderKind::Body, 1), text("body-right", [visual_x, body_top, column, body_height], style.body, "Right content", PlaceholderKind::Body, 2)]),
        layout("section", "Section header", Some("@accent1"), vec![section]),
        layout("three-cards", "Three points", None, cards),
        layout("visual-content", "Text and visual", None, vec![title(), text("body", [margin, body_top, column, body_height], style.body, "Key insight", PlaceholderKind::Body, 1), rectangle("preset-visual-region", [visual_x, body_top, column, body_height], "@lt2")]),
    ];
    let design = Design { theme: Theme { name: style.name.into(), colors: COLOR_KEYS.into_iter().zip(style.colors).map(|(key, value)| (key.into(), value.into())).collect(), fonts: ThemeFonts { major: style.major.into(), minor: style.minor.into(), east_asian: style.east_asian.into(), complex_script: style.minor.into() } }, masters: vec![master], layouts };
    crate::design::validate_design(&design)?;
    let mut regions = Vec::new();
    for target in ["blank", "title-content", "two-columns", "three-cards", "visual-content"] {
        let count = if target == "two-columns" { 2 } else if target == "three-cards" { 3 } else { 1 };
        for index in 0..count {
            let region_width = if target == "visual-content" { column } else { (width - (count - 1) as f64 * style.gutter) / count as f64 };
            let horizontal = if target == "visual-content" { visual_x } else { margin + index as f64 * (region_width + style.gutter) };
            regions.push(Region { layout_id: format!("preset-{target}"), name: if target == "visual-content" { "Visual / chart / part".into() } else { format!("Content {}", index + 1) }, x: horizontal, y: body_top, width: region_width, height: body_height });
        }
    }
    Ok(Preset { id: style.id.into(), name: style.name.into(), rules: Rules { margin, gutter: style.gutter, heading_size: style.heading, body_size: style.body, regions }, design })
}

pub fn catalog() -> Result<Vec<Preset>> { STYLES.iter().map(build).collect() }

fn same_value(actual: &serde_json::Value, expected: &serde_json::Value) -> bool {
    match (actual, expected) {
        (serde_json::Value::Number(left), serde_json::Value::Number(right)) => left.as_f64().zip(right.as_f64()).is_some_and(|(left, right)| (left - right).abs() < 0.0001),
        (serde_json::Value::Array(left), serde_json::Value::Array(right)) => left.len() == right.len() && left.iter().zip(right).all(|(left, right)| same_value(left, right)),
        (serde_json::Value::Object(left), serde_json::Value::Object(right)) => left.len() == right.len() && right.iter().all(|(key, value)| left.get(key).is_some_and(|actual| same_value(actual, value))),
        _ => actual == expected,
    }
}

fn contains_templates(actual: &[Element], templates: &[Element]) -> Result<bool> {
    for template in templates {
        let Some(element) = actual.iter().find(|entry| entry.bounds().0 == template.bounds().0) else { return Ok(false); };
        if !same_value(&serde_json::to_value(element)?, &serde_json::to_value(template)?) { return Ok(false); }
    }
    Ok(true)
}

fn identify(design: &Design) -> Result<Option<Preset>> {
    let Some(master) = design.masters.iter().find(|master| master.id == "preset-master") else { return Ok(None); };
    for preset in catalog()? {
        if master.background != preset.design.masters[0].background || !contains_templates(&master.elements, &preset.design.masters[0].elements)? { continue; }
        let mut matching = true;
        for template in &preset.design.layouts {
            let Some(layout) = design.layouts.iter().find(|layout| layout.id == template.id && layout.master_id == master.id) else { matching = false; break; };
            if layout.background != template.background || !contains_templates(&layout.elements, &template.elements)? { matching = false; break; }
        }
        if matching { return Ok(Some(preset)); }
    }
    Err(Error::Conflict("preset master is foreign or its templates were edited; retain or duplicate it before replacing the preset".into()))
}

fn retain_additions(target: &mut Vec<Element>, existing: &[Element], previous: &[Element]) -> Result<()> {
    for element in existing.iter().filter(|element| !previous.iter().any(|template| template.bounds().0 == element.bounds().0)) {
        if target.iter().any(|template| template.bounds().0 == element.bounds().0) { return Err(Error::Conflict("custom element ID conflicts with the selected preset".into())); }
        target.push(element.clone());
    }
    Ok(())
}

pub fn apply(deck: Deck, preset_id: &str) -> Result<Deck> {
    crate::model::validate_deck(&deck)?;
    let style = STYLES.iter().find(|style| style.id == preset_id).ok_or_else(|| Error::Invalid("design preset not found".into()))?;
    let mut preset = build(style)?;
    let previous_design = deck.design.clone().unwrap_or_default();
    let previous_preset = identify(&previous_design)?;
    let retained_layouts = previous_design.layouts.iter().filter(|layout| !preset.design.layouts.iter().any(|entry| entry.id == layout.id)).count();
    if previous_design.masters.len() + usize::from(previous_preset.is_none()) > 8 || retained_layouts + preset.design.layouts.len() > 32 { return Err(Error::Limit("insufficient design capacity for the preset master and seven layouts".into())); }
    if previous_design.layouts.iter().any(|layout| layout.master_id != "preset-master" && preset.design.layouts.iter().any(|entry| entry.id == layout.id)) { return Err(Error::Conflict("preset layout ID is already in use".into())); }
    let mut deck = crate::design::apply_theme(deck, preset.design.theme.clone())?;
    let mut design = deck.design.clone().unwrap_or_default();
    if let Some(previous) = previous_preset {
        if let Some(master) = design.masters.iter().find(|master| master.id == "preset-master") {
            retain_additions(&mut preset.design.masters[0].elements, &master.elements, &previous.design.masters[0].elements)?;
        }
        for template in &mut preset.design.layouts {
            if let Some(existing) = design.layouts.iter().find(|layout| layout.id == template.id) {
                let original = previous.design.layouts.iter().find(|layout| layout.id == template.id).ok_or_else(|| Error::Conflict("preset layout identity changed".into()))?;
                retain_additions(&mut template.elements, &existing.elements, &original.elements)?;
            }
        }
    }
    design.masters.retain(|master| master.id != "preset-master");
    design.layouts.retain(|layout| !preset.design.layouts.iter().any(|entry| entry.id == layout.id));
    design.theme = preset.design.theme;
    design.masters.extend(preset.design.masters);
    design.layouts.extend(preset.design.layouts.clone());
    for slide in &mut deck.slides {
        let previous = slide.layout_id.as_deref().unwrap_or("blank");
        let role = previous.strip_prefix("preset-").unwrap_or(previous);
        if let Some(layout) = preset.design.layouts.iter().find(|layout| layout.id == format!("preset-{role}")) {
            slide.layout_id = Some(layout.id.clone());
            slide.inherit_background = true;
        }
    }
    crate::design::update_design(deck, design)
}