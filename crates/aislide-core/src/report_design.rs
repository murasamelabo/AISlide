//! Report compilation with a design preset. Each content section becomes a layout-pattern
//! composition in the preset's design tokens: candidates are tried in a fixed order and the first
//! whose slots fit and whose text fits at token sizes is kept. Covers fill the preset's cover
//! layout placeholders. Without a preset, `report::compile_report` is unchanged.
use crate::{
    composition::{CompositionBlock, PatternChoice},
    composition_pattern::{build, caption_text, lines, Chrome},
    design::{Design, Theme},
    design_tokens::DesignTokens,
    model::{Deck, Element, PlaceholderKind, Slide, TextAlign, validate_deck, validate_rows},
    parts::PartItem,
    report::{CompiledReport, Layout, ReportInput, Section},
    rich_text::Spacing,
    Error, Result,
};
use std::collections::BTreeMap;

type Slots = BTreeMap<String, CompositionBlock>;
type Candidate = (PatternChoice, Slots);

fn candidate(id: &str, count: Option<usize>, entries: Vec<(String, CompositionBlock)>) -> Candidate {
    (PatternChoice { id: id.into(), count, mirror: false, message_band: false }, entries.into_iter().collect())
}

fn banded(mut candidate: Candidate, body: &[String]) -> Candidate {
    if !body.is_empty() {
        candidate.0.message_band = true;
        candidate.1.insert("message".into(), CompositionBlock::Callout { text: body.join("\n") });
    }
    candidate
}

fn slot(id: impl Into<String>, block: CompositionBlock) -> (String, CompositionBlock) { (id.into(), block) }

/// Body strings as paragraphs, with a paragraph gap between strings.
fn reading(body: &[String]) -> CompositionBlock {
    let mut paragraphs = Vec::new();
    for (index, entry) in body.iter().enumerate() {
        let mut block = lines(entry.trim());
        if index + 1 < body.len() { if let Some(last) = block.last_mut() { last.space_after = Some(Spacing::Points(1200)); } }
        paragraphs.extend(block);
    }
    CompositionBlock::Text { paragraphs }
}

/// A body string whose first line is a heading becomes a labelled card.
fn card(entry: &str) -> CompositionBlock {
    let (label, detail) = entry.split_once('\n').map_or(("", entry.trim()), |(label, detail)| (label.trim(), detail.trim()));
    CompositionBlock::Cards { items: vec![PartItem { label: label.into(), detail: detail.into(), value: None }], columns: None }
}

fn label_detail(body: &[String]) -> Option<Candidate> {
    let mut entries = Vec::new();
    for (index, entry) in body.iter().enumerate() {
        let (label, detail) = entry.split_once('\n')?;
        entries.push(slot(format!("label-{}", index + 1), CompositionBlock::Label { text: label.trim().into() }));
        entries.push(slot(format!("detail-{}", index + 1), CompositionBlock::Text { paragraphs: lines(detail.trim()) }));
    }
    Some(candidate("text/label-detail", Some(body.len()), entries))
}

fn grid(prefix: &str, blocks: &[CompositionBlock]) -> Vec<(String, CompositionBlock)> {
    blocks.iter().enumerate().map(|(index, block)| slot(format!("{prefix}-{}-{}", index / 2 + 1, index % 2 + 1), block.clone())).collect()
}

fn numbered(prefix: &str, blocks: &[CompositionBlock]) -> Vec<(String, CompositionBlock)> {
    blocks.iter().enumerate().map(|(index, block)| slot(format!("{prefix}-{}", index + 1), block.clone())).collect()
}

fn candidates(section: &Section, index: usize) -> Result<Vec<Candidate>> {
    let path = format!("report.sections[{index}]");
    let body = &section.body;
    Ok(match section.layout {
        Layout::Cover => Vec::new(),
        Layout::Statement => {
            if body.iter().all(|entry| entry.trim().is_empty()) { return Err(Error::Invalid(format!("{path}.body: a statement requires text"))); }
            vec![
                candidate("focus/statement", None, vec![slot("statement", CompositionBlock::Statement { text: body.join("\n") })]),
                candidate("text/reading", None, vec![slot("body", reading(body))]),
            ]
        }
        Layout::Columns => {
            if body.is_empty() { return Err(Error::Invalid("columns layout requires body blocks".into())); }
            let cards: Vec<CompositionBlock> = body.iter().map(|entry| card(entry)).collect();
            let mut list = match body.len() {
                1 => vec![candidate("text/reading", None, vec![slot("body", reading(body))])],
                2 => vec![
                    candidate("split/1-1", None, vec![slot("left", cards[0].clone()), slot("right", cards[1].clone())]),
                    candidate("text/two-column", None, numbered("column", &body.iter().map(|entry| reading(std::slice::from_ref(entry))).collect::<Vec<_>>())),
                ],
                3 => vec![candidate("columns/3", None, numbered("column", &cards))],
                _ => vec![candidate("grid/2x2", None, grid("cell", &cards)), candidate("columns/4", None, numbered("column", &cards))],
            };
            if body.len() > 2 { list.extend(label_detail(body)); }
            list
        }
        Layout::Metrics => {
            if section.metrics.is_empty() { return Err(Error::Invalid("metrics layout requires metrics".into())); }
            let metrics: Vec<CompositionBlock> = section.metrics.iter().map(|metric| CompositionBlock::Metric { value: metric.value.clone(), label: metric.label.clone(), detail: String::new() }).collect();
            match metrics.len() {
                1 => match body.len() {
                    0 => return Err(Error::Invalid(format!("{path}.body: a single metric needs a body string with its definition or evidence"))),
                    1 => vec![candidate("split/1-1", None, vec![slot("left", metrics[0].clone()), slot("right", reading(body))])],
                    _ => vec![
                        candidate("focus/kpi-hero", None, vec![slot("metric", metrics[0].clone()), slot("context", reading(&body[..1])), slot("evidence", reading(&body[1..]))]),
                        candidate("split/1-1", None, vec![slot("left", metrics[0].clone()), slot("right", reading(body))]),
                    ],
                },
                2 => vec![banded(candidate("split/1-1", None, vec![slot("left", metrics[0].clone()), slot("right", metrics[1].clone())]), body)],
                3 => vec![banded(candidate("columns/3", None, numbered("column", &metrics)), body)],
                _ => vec![banded(candidate("grid/2x2", None, grid("cell", &metrics)), body), banded(candidate("columns/4", None, numbered("column", &metrics)), body)],
            }
        }
        Layout::Table => {
            validate_rows(&section.rows)?;
            let table = CompositionBlock::Table { rows: section.rows.clone() };
            if body.is_empty() { vec![candidate("focus/full", None, vec![slot("primary", table)])] }
            else { vec![candidate("compare/table-full", None, vec![slot("table", table), slot("decision", CompositionBlock::Callout { text: body.join("\n") })])] }
        }
        Layout::Chart => {
            let chart = section.chart.as_ref().ok_or_else(|| Error::Invalid("chart layout requires chart data".into()))?;
            crate::model::chart_format::validate(chart.kind, &chart.categories, &chart.series, &chart.options)?;
            let mut themed = chart.clone();
            for (position, series) in themed.series.iter_mut().enumerate() { series.color = format!("@accent{}", position % 6 + 1); }
            if themed.options.legend.is_none() { themed.options.legend = Some(crate::model::chart_format::LegendPosition::Top); }
            let primary = CompositionBlock::Chart { chart: themed };
            if body.is_empty() { vec![candidate("focus/full", None, vec![slot("primary", primary)])] }
            else {
                let mut list = vec![candidate("split/2-1", None, vec![slot("primary", primary.clone()), slot("support", reading(body))])];
                if (2..=4).contains(&body.len()) {
                    let mut entries = vec![slot("primary", primary.clone())];
                    entries.extend(numbered("insight", &body.iter().map(|entry| card(entry)).collect::<Vec<_>>()));
                    list.push(candidate("stack/2-1", Some(body.len()), entries));
                }
                list.push(candidate("split/3-2", None, vec![slot("primary", primary), slot("support", reading(body))]));
                list
            }
        }
        Layout::Process => {
            let steps: Vec<CompositionBlock> = body.iter().map(|entry| CompositionBlock::Cards { items: vec![PartItem { label: entry.trim().into(), detail: String::new(), value: None }], columns: None }).collect();
            let rows: Vec<CompositionBlock> = body.iter().map(|entry| CompositionBlock::Text { paragraphs: lines(entry.trim()) }).collect();
            vec![candidate("sequence/steps-h", Some(body.len()), numbered("step", &steps)), candidate("sequence/steps-v", Some(body.len()), numbered("step", &rows))]
        }
    })
}

fn content(report: &ReportInput, section: &Section, index: usize, preset: &str, tokens: &DesignTokens, theme: &Theme) -> Result<Vec<Element>> {
    let page = format!("{:02}", index + 1);
    let chrome = Chrome { title: &section.title, footer: &report.title, page: &page };
    let mut last = None;
    for (pattern, slots) in candidates(section, index)? {
        match build(tokens, theme, &format!("s{}", index + 1), &chrome, &pattern, &slots) {
            Ok(built) if built.parts.is_empty() => return Ok(built.elements),
            Ok(_) => last = Some(format!("{} would need managed parts", pattern.id)),
            Err(error) => last = Some(format!("{}: {error}", pattern.id)),
        }
    }
    Err(Error::Invalid(format!("report.sections[{index}] does not fit the {preset} design at its type sizes ({}); shorten the text or split the section", last.unwrap_or_default())))
}

fn set_text(element: &mut Element, value: &str) -> Result<()> {
    if let Element::Text { text, format, .. } = element {
        format.paragraphs = lines(value);
        *text = crate::rich_text::plain_text(&format.paragraphs);
    }
    crate::rich_text::validate_element(element)
}

fn placeholder(slide: &Slide, kind: PlaceholderKind) -> Option<usize> {
    slide.elements.iter().position(|element| matches!(element, Element::Text { format, .. } if format.placeholder.as_ref().is_some_and(|placeholder| placeholder.kind == kind)))
}

fn text_style(element: &Element) -> ([f64; 4], String, TextAlign) {
    let (_, x, y, width, height) = element.bounds();
    match element { Element::Text { color, format, .. } => ([x, y, width, height], color.clone(), format.alignment), _ => ([x, y, width, height], "@dk1".into(), TextAlign::Left) }
}

fn cover(slide: &mut Slide, design: &Design, report: &ReportInput, section: &Section, index: usize, preset: &str, tokens: &DesignTokens, theme: &Theme) -> Result<()> {
    let probe = Deck { version: 1, title: report.title.clone(), width: 1280, height: 720, slides: vec![slide.clone()], design: Some(design.clone()), embedded_fonts: Vec::new(), auxiliary_design: None };
    *slide = crate::design::assign_layout(probe, &slide.id, "preset-cover")?.slides.remove(0);
    let layout = design.layouts.iter().find(|layout| layout.id == "preset-cover").ok_or_else(|| Error::Invalid("preset cover layout is missing".into()))?;
    let title_index = placeholder(slide, PlaceholderKind::Title).ok_or_else(|| Error::Invalid("preset cover has no title placeholder".into()))?;
    set_text(&mut slide.elements[title_index], &section.title)?;
    let label = section.body.first().map(|entry| entry.trim()).filter(|entry| !entry.is_empty());
    if report.subtitle.trim().is_empty() {
        if let Some(index) = placeholder(slide, PlaceholderKind::Subtitle) { slide.elements.remove(index); }
    } else if let Some(index) = placeholder(slide, PlaceholderKind::Subtitle) {
        set_text(&mut slide.elements[index], report.subtitle.trim())?;
        if crate::layout::first_overflow(std::slice::from_ref(&slide.elements[index]), theme)?.is_some() {
            return Err(Error::Invalid(format!("report.subtitle does not fit the {preset} cover at {}px; shorten it", tokens.type_scale.body)));
        }
    }
    let title_index = placeholder(slide, PlaceholderKind::Title).unwrap_or(title_index);
    if crate::layout::first_overflow(std::slice::from_ref(&slide.elements[title_index]), theme)?.is_some() {
        let ([x, y, width, _], _, _) = text_style(&slide.elements[title_index]);
        let below = |frame: [f64; 4]| frame[1] > y && frame[0] < x + width && frame[0] + frame[2] > x;
        let decorations = layout.elements.iter().filter(|element| !matches!(element, Element::Text { format, .. } if format.placeholder.is_some())).map(|element| { let (_, x, y, width, height) = element.bounds(); [x, y, width, height] });
        let subtitle = placeholder(slide, PlaceholderKind::Subtitle).map(|index| text_style(&slide.elements[index]).0);
        let limit = decorations.chain(subtitle).chain(label.map(|_| [x, 512.0, width, 32.0])).filter(|frame| below(*frame)).map(|frame| frame[1]).fold(640.0, f64::min) - 8.0;
        if let Element::Text { height, format, .. } = &mut slide.elements[title_index] {
            if limit - y > *height { *height = limit - y; format.inherit_layout = false; }
        }
        if crate::layout::first_overflow(std::slice::from_ref(&slide.elements[title_index]), theme)?.is_some() {
            return Err(Error::Invalid(format!("report.sections[{index}].title does not fit the {preset} cover title; shorten it")));
        }
    }
    let (title_frame, title_color, title_align) = text_style(&slide.elements[title_index]);
    if !report.period.trim().is_empty() {
        slide.elements.push(caption_text("s1-period".into(), [title_frame[0], 104.0, title_frame[2], 32.0], report.period.trim(), tokens.type_scale.caption, &title_color, true, title_align)?);
    }
    if let Some(label) = label {
        let (frame, color, align) = placeholder(slide, PlaceholderKind::Subtitle).map_or_else(|| (title_frame, title_color.clone(), title_align), |index| text_style(&slide.elements[index]));
        slide.elements.push(caption_text("s1-label".into(), [frame[0], 512.0, frame[2], 32.0], label, tokens.type_scale.caption, &color, false, align)?);
    }
    if let Some(element) = crate::layout::first_overflow(&slide.elements, theme)? {
        return Err(Error::Invalid(format!("report.sections[{index}] cover text {element} does not fit the {preset} cover; shorten it")));
    }
    Ok(())
}

pub(crate) fn compile(report: &ReportInput, preset: &str) -> Result<CompiledReport> {
    crate::report::validate_header(report)?;
    let design = crate::design_presets::preset(preset)?.design;
    let theme = design.theme.clone();
    let mut deck = Deck { version: 1, title: report.title.clone(), width: 1280, height: 720, slides: Vec::new(), design: Some(design.clone()), embedded_fonts: Vec::new(), auxiliary_design: None };
    let tokens = crate::design_tokens::resolve(&deck)?;
    for (index, section) in report.sections.iter().enumerate() {
        crate::report::validate_section(index, section)?;
        let mut slide = Slide { id: format!("slide-{}", index + 1), title: section.title.clone(), background: "@lt1".into(), elements: Vec::new(), notes: format!("{}\n\nSource: {}", section.body.join("\n"), report.source), notes_paragraphs: Vec::new(), layout_id: tokens.layout_id.clone(), inherit_background: true, hide_master_graphics: false, native_source_id: None, review: None };
        if section.layout == Layout::Cover { cover(&mut slide, &design, report, section, index, preset, &tokens, &theme)?; }
        else { slide.elements = content(report, section, index, preset, &tokens, &theme)?; }
        deck.slides.push(slide);
    }
    validate_deck(&deck)?;
    Ok(CompiledReport { deck, issues: vec![crate::report::font_parity_issue()] })
}
