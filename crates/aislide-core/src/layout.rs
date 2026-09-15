use crate::{model::{Deck, Element, Issue, TextFormat, Bullet, validate_deck}, design::Theme, Error, Result};
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, Weight, Wrap};
use serde::Serialize;
use std::{collections::BTreeSet, sync::{Mutex, OnceLock}};

static FONTS: OnceLock<Mutex<FontSystem>> = OnceLock::new();

#[derive(Serialize)]
pub struct Measurement {
    pub slide_id: String, pub element_id: String, pub width: f64, pub height: f64,
    pub measured_width: f32, pub measured_height: f32, pub lines: usize, pub overflow: bool,
    pub missing_glyphs: usize, pub requested_family: String, pub fonts: Vec<String>,
}

#[derive(Serialize)]
pub struct LayoutReport { pub engine: String, pub office_parity_verified: bool, pub measurements: Vec<Measurement>, pub fonts: Vec<String>, pub issues: Vec<Issue> }

fn measure(fonts: &mut FontSystem, slide: &str, id: &str, text: &str, width: f64, height: f64, size: f64, bold: bool, format: &TextFormat, theme: &Theme) -> Measurement {
    let family = if text.chars().any(|character| character >= '\u{3000}') { &theme.fonts.east_asian } else { match format.font_family.as_deref() { Some("@major") => &theme.fonts.major, None | Some("@minor") => &theme.fonts.minor, Some(family) => family } };
    let available_width = width - if format.bullet == Bullet::None { 0.0 } else { size };
    let mut buffer = Buffer::new(fonts, Metrics::new(size as f32, (size * 1.15) as f32));
    buffer.set_size(Some(available_width.max(1.0) as f32), None);
    buffer.set_wrap(Wrap::WordOrGlyph);
    buffer.set_text(text, &Attrs::new().family(Family::Name(family)).weight(if bold { Weight::BOLD } else { Weight::NORMAL }).style(if format.italic { Style::Italic } else { Style::Normal }), Shaping::Advanced, None);
    buffer.shape_until_scroll(fonts, false);
    let mut measured_width = 0.0f32; let mut measured_height = 0.0f32; let mut lines = 0; let mut missing = 0;
    let mut used = BTreeSet::new();
    for run in buffer.layout_runs() {
        measured_width = measured_width.max(run.line_w); measured_height = measured_height.max(run.line_top + run.line_height); lines += 1;
        for glyph in run.glyphs {
            if glyph.glyph_id == 0 { missing += 1; }
            if let Some(face) = fonts.db().face(glyph.font_id) { if let Some((name, _)) = face.families.first() { used.insert(name.clone()); } }
        }
    }
    Measurement { slide_id: slide.into(), element_id: id.into(), width, height, measured_width, measured_height, lines,
        overflow: !text.is_empty() && (measured_width as f64 > width + 1.0 || measured_height as f64 > height + 1.0), missing_glyphs: missing, requested_family: family.into(), fonts: used.into_iter().collect() }
}

fn visit(fonts: &mut FontSystem, slide: &str, elements: &[Element], measurements: &mut Vec<Measurement>, characters: &mut usize, theme: &Theme) -> Result<()> {
    for element in elements {
        let (id, _, _, width, height) = element.bounds();
        match element {
            Element::Text { text, font_size, bold, format, .. } | Element::Shape { text, font_size, bold, format, .. } => {
                *characters += text.chars().count();
                if *characters > 200_000 { return Err(Error::Limit("layout measurement > 200000 characters".into())); }
                measurements.push(measure(fonts, slide, id, text, width, height, *font_size, *bold, format, theme));
            }
            Element::Table { rows, font_size, .. } => {
                let cell_width = width / rows[0].len() as f64 - 12.0;
                let cell_height = height / rows.len() as f64 - 12.0;
                for (row_index, row) in rows.iter().enumerate() { for (column, text) in row.iter().enumerate() {
                    *characters += text.chars().count();
                    if *characters > 200_000 { return Err(Error::Limit("layout measurement > 200000 characters".into())); }
                    measurements.push(measure(fonts, slide, &format!("{id}[{},{}]", row_index + 1, column + 1), text, cell_width, cell_height, *font_size, row_index == 0, &TextFormat::default(), theme));
                } }
            }
            Element::Group { children, .. } => visit(fonts, slide, children, measurements, characters, theme)?,
            _ => {}
        }
    }
    Ok(())
}

pub fn measure_layout(deck: &Deck) -> Result<LayoutReport> {
    validate_deck(deck)?;
    let mut fonts = FONTS.get_or_init(|| Mutex::new(FontSystem::new())).lock().map_err(|_| Error::Invalid("font measurement state unavailable".into()))?;
    if fonts.db().faces().next().is_none() { return Err(Error::Unsupported("no installed fonts available for measurement".into())); }
    let mut measurements = Vec::new(); let mut characters = 0;
    let fallback = Theme::default(); let theme = deck.design.as_ref().map(|design| &design.theme).unwrap_or(&fallback);
    for slide in &deck.slides { visit(&mut fonts, &slide.id, &slide.elements, &mut measurements, &mut characters, theme)?; }
    if let Some(design) = &deck.design {
        for master in &design.masters { visit(&mut fonts, &format!("master:{}", master.id), &master.elements, &mut measurements, &mut characters, theme)?; }
        for layout in &design.layouts { visit(&mut fonts, &format!("layout:{}", layout.id), &layout.elements, &mut measurements, &mut characters, theme)?; }
    }
    let mut issues = vec![Issue { code: "OFFICE_PARITY_UNVERIFIED".into(), severity: "warning".into(), message: "Measurements use installed fonts and cosmic-text, not Office's text engine. Charts and images require separate visual review.".into() }];
    let mut used = BTreeSet::new();
    for measurement in &measurements {
        used.extend(measurement.fonts.iter().cloned());
        if measurement.overflow { issues.push(Issue { code: "TEXT_OVERFLOW".into(), severity: "error".into(), message: format!("{} / {} needs {:.1}px height in a {:.1}px frame", measurement.slide_id, measurement.element_id, measurement.measured_height, measurement.height) }); }
        if measurement.missing_glyphs > 0 { issues.push(Issue { code: "MISSING_GLYPHS".into(), severity: "error".into(), message: format!("{} / {} has {} unresolved glyphs", measurement.slide_id, measurement.element_id, measurement.missing_glyphs) }); }
        if !measurement.fonts.is_empty() && !measurement.fonts.contains(&measurement.requested_family) { issues.push(Issue { code: "FONT_FALLBACK".into(), severity: "warning".into(), message: format!("{} / {} requested {}, used {}", measurement.slide_id, measurement.element_id, measurement.requested_family, measurement.fonts.join(", ")) }); }
    }
    Ok(LayoutReport { engine: "cosmic-text".into(), office_parity_verified: false, measurements, fonts: used.into_iter().collect(), issues })
}

pub(crate) fn fit_metric_size(text: &str, width: f64, height: f64) -> Result<f64> {
    let mut fonts = FONTS.get_or_init(|| Mutex::new(FontSystem::new())).lock().map_err(|_| Error::Invalid("font measurement state unavailable".into()))?;
    if fonts.db().faces().next().is_none() { return Err(Error::Unsupported("installed fonts are required for metric fitting".into())); }
    for size in [56.0, 52.0, 48.0, 44.0, 40.0, 36.0, 32.0, 28.0, 24.0] {
        let measured = measure(&mut fonts, "metric", "value", text, width, height, size, true, &TextFormat::default(), &Theme::default());
        if !measured.overflow { return Ok(size); }
    }
    Err(Error::Invalid("metric value does not fit its minimum font size".into()))
}

pub(crate) fn fit_part_text(elements: &mut [Element], theme: &Theme) -> Result<()> {
    let mut fonts=FONTS.get_or_init(||Mutex::new(FontSystem::new())).lock().map_err(|_|Error::Invalid("font measurement state unavailable".into()))?;
    if fonts.db().faces().next().is_none() {return Err(Error::Unsupported("installed fonts are required for part fitting".into()));}
    fn fit(fonts:&mut FontSystem,elements:&mut [Element],theme:&Theme)->Result<()> {
        for element in elements {
            match element {
                Element::Text {id,text,width,height,font_size,bold,format,..} | Element::Shape {id,text,width,height,font_size,bold,format,..} => {
                    let mut size=*font_size;
                    loop {
                        let result=measure(fonts,"part",id,text,*width,*height,size,*bold,format,theme);
                        if result.missing_glyphs>0 {return Err(Error::Invalid(format!("part text {id} contains unavailable glyphs")));}
                        if !result.overflow {*font_size=size;break;}
                        size-=1.0;
                        if size<12.0 {return Err(Error::Invalid(format!("part text {id} does not fit at 12px; shorten the label or reduce item count")));}
                    }
                }
                Element::Group {children,..} => fit(fonts,children,theme)?,
                _ => {}
            }
        }
        Ok(())
    }
    fit(&mut fonts,elements,theme)
}