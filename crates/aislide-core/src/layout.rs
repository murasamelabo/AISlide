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
    if !format.paragraphs.is_empty() { return measure_rich(fonts, slide, id, text, width, height, size, bold, format, theme); }
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

fn measure_rich(fonts: &mut FontSystem, slide: &str, id: &str, text: &str, width: f64, height: f64, size: f64, bold: bool, format: &TextFormat, theme: &Theme) -> Measurement {
    use crate::rich_text::{RunStyle, Spacing};
    let mut measured_width = 0.0f32; let mut measured_height = 0.0f32; let mut lines = 0; let mut missing = 0;
    let mut used = BTreeSet::new(); let mut requested = String::new();
    let spacing_pixels = |spacing: Spacing, size: f64| match spacing { Spacing::Percent(value) => size * value as f64 / 100000.0, Spacing::Points(value) => value as f64 / 75.0 };
    for paragraph in &format.paragraphs {
        let styles: Vec<_> = paragraph.runs.iter().map(|run| { let mut style = RunStyle::frame(size, "@dk1", bold, format); style.overlay(&run.style); style }).collect();
        let families: Vec<_> = paragraph.runs.iter().zip(&styles).map(|(run, style)| match style.font_family.as_deref() {
            Some("@major") if !run.text.chars().any(|character| character >= '\u{3000}') => theme.fonts.major.clone(),
            None | Some("@minor" | "@major") if run.text.chars().any(|character| character >= '\u{3000}') => theme.fonts.east_asian.clone(),
            None | Some("@minor" | "@major") => theme.fonts.minor.clone(),
            Some(family) => family.to_owned(),
        }).collect();
        if requested.is_empty() { requested = families.first().cloned().unwrap_or_else(|| theme.fonts.minor.clone()); }
        let maximum_size = styles.iter().filter_map(|style| style.font_size).reduce(f64::max).unwrap_or(size);
        let line_height = spacing_pixels(paragraph.line_spacing.unwrap_or(Spacing::Percent(115000)), maximum_size).max(maximum_size);
        let margin = paragraph.margin_left.map(|value| value as f64 / 9525.0).unwrap_or(if paragraph.bullet.unwrap_or(format.bullet) == Bullet::None { 0.0 } else { size });
        let positive_indent = paragraph.indent.unwrap_or(0).max(0) as f64 / 9525.0;
        let tab_reserve = if paragraph.runs.iter().any(|run| run.text.contains('\t')) { paragraph.tabs.last().map_or(maximum_size * 4.0, |tab| tab.position as f64 / 9525.0) } else { 0.0 };
        let inset = margin + positive_indent + tab_reserve;
        let default_attrs = Attrs::new().family(Family::Name(&requested));
        let mut buffer = Buffer::new(fonts, Metrics::new(maximum_size as f32, line_height as f32));
        buffer.set_size(Some((width - inset).max(1.0) as f32), None); buffer.set_wrap(Wrap::WordOrGlyph);
        buffer.set_rich_text(paragraph.runs.iter().zip(&styles).zip(&families).map(|((run, style), family)| {
            let attrs = Attrs::new().family(Family::Name(family)).weight(if style.bold.unwrap_or(bold) { Weight::BOLD } else { Weight::NORMAL }).style(if style.italic.unwrap_or(false) { Style::Italic } else { Style::Normal }).metrics(Metrics::new(style.font_size.unwrap_or(size) as f32, line_height as f32));
            (run.text.as_str(), attrs)
        }), &default_attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(fonts, false);
        let mut paragraph_height = line_height as f32;
        for run in buffer.layout_runs() {
            measured_width = measured_width.max(run.line_w + inset as f32); paragraph_height = paragraph_height.max(run.line_top + run.line_height); lines += 1;
            for glyph in run.glyphs {
                if glyph.glyph_id == 0 { missing += 1; }
                if let Some(face) = fonts.db().face(glyph.font_id) { if let Some((name, _)) = face.families.first() { used.insert(name.clone()); } }
            }
        }
        let raise = styles.iter().map(|style| style.baseline.unwrap_or(0).max(0) as f64 * style.font_size.unwrap_or(size) / 100000.0).fold(0.0, f64::max);
        let lower = styles.iter().map(|style| -style.baseline.unwrap_or(0).min(0) as f64 * style.font_size.unwrap_or(size) / 100000.0).fold(0.0, f64::max);
        let before = paragraph.space_before.map_or(0.0, |spacing| spacing_pixels(spacing, maximum_size));
        let after = paragraph.space_after.map_or(0.0, |spacing| spacing_pixels(spacing, maximum_size));
        measured_height += paragraph_height + (raise + lower + before + after) as f32;
    }
    Measurement { slide_id: slide.into(), element_id: id.into(), width, height, measured_width, measured_height, lines,
        overflow: !text.is_empty() && (measured_width as f64 > width + 1.0 || measured_height as f64 > height + 1.0), missing_glyphs: missing, requested_family: requested, fonts: used.into_iter().collect() }
}

fn visit(fonts: &mut FontSystem, slide: &str, elements: &[Element], measurements: &mut Vec<Measurement>, characters: &mut usize, theme: &Theme) -> Result<()> {
    for element in elements {
        let (id, _, _, width, height) = element.bounds();
        match element {
            Element::Text { text, font_size, bold, format, .. } | Element::Shape { text, font_size, bold, format, .. } => {
                crate::rich_text::validate_element(element)?;
                *characters += text.chars().count();
                if *characters > 200_000 { return Err(Error::Limit("layout measurement > 200000 characters".into())); }
                let (width, height) = if matches!(element, Element::Shape { .. }) { ((width - 12.0).max(1.0), (height - 8.0).max(1.0)) } else { (width, height) };
                measurements.push(measure(fonts, slide, id, text, width, height, *font_size, *bold, format, theme));
            }
            Element::Table { rows, font_size, format, .. } => {
                let columns = crate::table_format::tracks(format.column_widths.as_ref(), rows[0].len(), width)?;
                let heights = crate::table_format::tracks(format.row_heights.as_ref(), rows.len(), height)?;
                for (row_index, row) in rows.iter().enumerate() { for (column, text) in row.iter().enumerate() {
                    let merge = format.merge_at(row_index, column);
                    if merge.is_some_and(|region| (region.row, region.column) != (row_index, column)) { continue; }
                    let cell_width = columns[column..column + merge.map_or(1, |region| region.col_span)].iter().sum::<f64>();
                    let cell_height = heights[row_index..row_index + merge.map_or(1, |region| region.row_span)].iter().sum::<f64>();
                    let style = format.cell_style(row_index, column);
                    let padding = style.padding.clone().unwrap_or(crate::table_format::CellPadding { left: 6.0, right: 6.0, top: 6.0, bottom: 6.0 });
                    let text_width = cell_width - padding.left - padding.right;
                    let text_height = cell_height - padding.top - padding.bottom;
                    *characters += text.chars().count();
                    if *characters > 200_000 { return Err(Error::Limit("layout measurement > 200000 characters".into())); }
                    let mut measurement = measure(fonts, slide, &format!("{id}[{},{}]", row_index + 1, column + 1), text, text_width, text_height, *font_size, row_index == 0, &style.text(text), theme);
                    if !text.is_empty() && (text_width <= 0.0 || text_height <= 0.0) { measurement.overflow = true; }
                    measurements.push(measurement);
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
    let mut document_fonts = crate::fonts::document_system(&fonts, deck)?;
    let fonts = document_fonts.as_mut().unwrap_or(&mut fonts);
    if fonts.db().faces().next().is_none() { return Err(Error::Unsupported("no installed fonts available for measurement".into())); }
    let mut measurements = Vec::new(); let mut characters = 0;
    let fallback = Theme::default();
    for slide in &deck.slides { visit(fonts, &slide.id, &slide.elements, &mut measurements, &mut characters, crate::design::slide_theme(slide, deck.design.as_ref()).unwrap_or(&fallback))?; }
    if let Some(design) = &deck.design {
        for master in &design.masters { visit(fonts, &format!("master:{}", master.id), &master.elements, &mut measurements, &mut characters, crate::design::master_theme(design, &master.id))?; }
        for layout in &design.layouts { visit(fonts, &format!("layout:{}", layout.id), &layout.elements, &mut measurements, &mut characters, crate::design::master_theme(design, &layout.master_id))?; }
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
    fit_part_text_with_small_annotations(elements, theme, &BTreeSet::new())
}

pub(crate) fn fit_part_text_with_small_annotations(elements: &mut [Element], theme: &Theme, small_annotations: &BTreeSet<String>) -> Result<()> {
    let mut fonts=FONTS.get_or_init(||Mutex::new(FontSystem::new())).lock().map_err(|_|Error::Invalid("font measurement state unavailable".into()))?;
    if fonts.db().faces().next().is_none() {return Err(Error::Unsupported("installed fonts are required for part fitting".into()));}
    fn fit(fonts:&mut FontSystem,elements:&mut [Element],theme:&Theme,small_annotations:&BTreeSet<String>)->Result<()> {
        for element in elements {
            match element {
                Element::Text {id,text,width,height,font_size,bold,format,..} | Element::Shape {id,text,width,height,font_size,bold,format,..} => {
                    let mut size=*font_size;
                    let minimum=if small_annotations.contains(id) {8.0} else {12.0};
                    loop {
                        let result=measure(fonts,"part",id,text,*width,*height,size,*bold,format,theme);
                        if result.missing_glyphs>0 {return Err(Error::Invalid(format!("part text {id} contains unavailable glyphs")));}
                        if !result.overflow {*font_size=size;break;}
                        size-=1.0;
                        if size<minimum {return Err(Error::Invalid(format!("part text {id} does not fit at {minimum}px; shorten the label or reduce item count")));}
                    }
                }
                Element::Group {children,..} => fit(fonts,children,theme,small_annotations)?,
                _ => {}
            }
        }
        Ok(())
    }
    fit(&mut fonts,elements,theme,small_annotations)
}