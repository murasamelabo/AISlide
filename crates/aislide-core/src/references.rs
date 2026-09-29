use crate::{document::{Document, Transaction, TransactionResult}, model::{Deck, Element, Slide, TextFormat, valid_text}, Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Placement { #[default] Auto, Footnotes, Appendix, AppendixOnly }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub id: String,
    pub name: String,
    pub url: String,
    pub slide_ids: Vec<String>,
    #[serde(default)]
    pub publish: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReferenceOptions {
    pub entries: Vec<Reference>,
    pub placement: Placement,
    pub title: String,
    pub font_size: f64,
}

impl Default for ReferenceOptions {
    fn default() -> Self { Self { entries: Vec::new(), placement: Placement::Auto, title: "References".into(), font_size: 16.0 } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedElement { pub slide_id: String, pub id: String, pub sha256: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceState {
    pub options: ReferenceOptions,
    pub elements: Vec<OwnedElement>,
    pub slides: Vec<String>,
    pub page_sha256: BTreeMap<String, String>,
}

#[derive(Serialize)]
pub struct Publication { pub supplied: usize, pub published: usize, pub excluded: usize }

#[derive(Serialize)]
pub struct ReferenceResult {
    #[serde(flatten)]
    pub transaction: TransactionResult,
    pub publication: Publication,
}

fn fingerprint(element: &Element) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(crate::canonical::bytes(element)?)))
}

fn page_fingerprint(slide: &Slide) -> Result<String> {
    let mut value = serde_json::to_value(slide)?;
    let object = value.as_object_mut().ok_or_else(|| Error::Invalid("reference page metadata".into()))?;
    object.remove("elements"); object.remove("native_source_id");
    Ok(format!("{:x}", Sha256::digest(crate::canonical::bytes(&value)?)))
}

pub(crate) fn exported_state(state: &ReferenceState, authored: &Deck, emitted: &Deck) -> Result<ReferenceState> {
    let mut result = state.clone();
    for owned in &mut result.elements {
        let Some(page) = authored.slides.iter().position(|slide| slide.id == owned.slide_id) else { continue; };
        let index = authored.slides[page].elements.iter().position(|element| element.bounds().0 == owned.id)
            .ok_or_else(|| Error::Conflict("managed reference is missing during export".into()))?;
        let element = emitted.slides.get(page).and_then(|slide| slide.elements.get(index))
            .ok_or_else(|| Error::Conflict("managed reference cannot be reopened".into()))?;
        let mut value = serde_json::to_value(element)?;
        value["id"] = json!(owned.id);
        owned.sha256 = fingerprint(&serde_json::from_value(value)?)?;
    }
    for id in &state.slides {
        let page = authored.slides.iter().position(|slide| &slide.id == id).ok_or_else(|| Error::Conflict("reference page missing during export".into()))?;
        let mut slide = emitted.slides.get(page).cloned().ok_or_else(|| Error::Conflict("reference page cannot be reopened".into()))?;
        slide.id = id.clone();
        result.page_sha256.insert(id.clone(), page_fingerprint(&slide)?);
    }
    Ok(result)
}

fn validate_options(options: &ReferenceOptions) -> Result<()> {
    valid_text(&options.title, 120)?;
    if options.title.trim().is_empty() || options.entries.len() > 64 || !options.font_size.is_finite() || !(16.0..=32.0).contains(&options.font_size) {
        return Err(Error::Invalid("references require a title, at most 64 entries and a 16-32px font".into()));
    }
    let mut ids = BTreeSet::new();
    for entry in &options.entries {
        valid_text(&entry.id, 40)?; valid_text(&entry.name, 200)?;
        if entry.id.is_empty() || !entry.id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) || !ids.insert(&entry.id)
            || entry.name.trim().is_empty() || entry.slide_ids.is_empty() || entry.slide_ids.len() > 128 || !entry.publish {
            return Err(Error::Invalid("references require unique simple IDs, names, slide IDs and explicit publication approval".into()));
        }
        let mut slides = BTreeSet::new();
        for id in &entry.slide_ids { valid_text(id, 80)?; if !slides.insert(id) { return Err(Error::Invalid("duplicate reference slide ID".into())); } }
        crate::model::validate_hyperlink(&entry.url)?;
        let url = reqwest::Url::parse(&entry.url).map_err(|_| Error::Invalid("invalid reference URL".into()))?;
        let host = url.host_str().unwrap_or("").trim_end_matches('.');
        if !matches!(url.scheme(), "https" | "http") || host.is_empty() || host == "localhost" || host.ends_with(".localhost") || !host.contains('.')
            || host.parse::<std::net::IpAddr>().is_ok() || host.starts_with('[') || entry.url.chars().any(char::is_whitespace) {
            return Err(Error::Invalid("public references require an approved HTTP(S) URL with a domain name, not local paths or IP addresses".into()));
        }
    }
    Ok(())
}

pub(crate) fn validate(state: &ReferenceState) -> Result<()> {
    validate_options(&state.options)?;
    if state.elements.len() > 1024 || state.slides.len() > 64 { return Err(Error::Limit("generated reference resource budget".into())); }
    let mut ids = BTreeSet::new();
    for owned in &state.elements {
        valid_text(&owned.slide_id, 80)?; valid_text(&owned.id, 80)?;
        if !owned.id.starts_with("aislide-ref-") || !ids.insert((&owned.slide_id, &owned.id)) || owned.sha256.len() != 64 {
            return Err(Error::Invalid("invalid generated reference ownership".into()));
        }
    }
    if state.slides.iter().any(|id| !id.starts_with("aislide-ref-page-")) || state.slides.iter().collect::<BTreeSet<_>>().len() != state.slides.len() {
        return Err(Error::Invalid("invalid reference appendix identity".into()));
    }
    if state.page_sha256.len() != state.slides.len() || state.slides.iter().any(|id| state.page_sha256.get(id).is_none_or(|hash| hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()))) {
        return Err(Error::Invalid("invalid reference page fingerprint".into()));
    }
    Ok(())
}

fn strip(deck: &mut Deck, state: &ReferenceState) -> Result<()> {
    validate(state)?;
    for slide in deck.slides.iter().filter(|slide| state.slides.contains(&slide.id)) {
        if state.page_sha256.get(&slide.id) != Some(&page_fingerprint(slide)?) {
            return Err(Error::Conflict("managed reference page was edited; change references through set_references".into()));
        }
    }
    for owned in &state.elements {
        let Some(slide) = deck.slides.iter().find(|slide| slide.id == owned.slide_id) else { continue; };
        let element = slide.elements.iter().find(|element| element.bounds().0 == owned.id)
            .ok_or_else(|| Error::Conflict("managed reference was removed; change references through set_references".into()))?;
        if fingerprint(element)? != owned.sha256 { return Err(Error::Conflict("managed reference was edited; change references through set_references".into())); }
    }
    for slide in &mut deck.slides {
        slide.elements.retain(|element| !state.elements.iter().any(|owned| owned.slide_id == slide.id && owned.id == element.bounds().0));
        if state.slides.contains(&slide.id) && (!slide.elements.is_empty() || !slide.notes.is_empty() || !slide.notes_paragraphs.is_empty()) {
            return Err(Error::Conflict("reference appendix contains authored content; refusing to replace it".into()));
        }
    }
    deck.slides.retain(|slide| !state.slides.contains(&slide.id));
    Ok(())
}

fn text_element(id: String, text: String, x: f64, y: f64, width: f64, height: f64, size: f64, color: &str, link: Option<String>) -> Element {
    Element::Text { id, x, y, width, height, text, font_size: size, color: color.into(), bold: false,
        format: TextFormat { hyperlink: link, ..TextFormat::default() }, visual: None }
}

fn add(slide: &mut Slide, state: &mut ReferenceState, element: Element) -> Result<()> {
    let id = element.bounds().0;
    if crate::model::element_list(&slide.elements).iter().any(|existing| existing.bounds().0 == id) {
        return Err(Error::Conflict("reference element ID collides with existing content".into()));
    }
    state.elements.push(OwnedElement { slide_id: slide.id.clone(), id: id.into(), sha256: fingerprint(&element)? });
    slide.elements.push(element);
    Ok(())
}

fn text_height(text: &str, width: f64, size: f64, theme: &crate::design::Theme) -> Result<f64> {
    let (measurement, _) = crate::layout::graph_text_metrics(text, width - 16.0, 10000.0, size, false, theme)?;
    Ok((f64::from(measurement.measured_height) * 1.2 + 16.0).ceil())
}

fn caption_metrics(width: f64, size: f64, theme: &crate::design::Theme) -> Result<(usize, f64)> {
    let (prefix, _) = crate::layout::graph_text_metrics("Slides: ", 10000.0, 10000.0, size, false, theme)?;
    let mut slot_width = 0.0f64;
    let mut line_height = f64::from(prefix.measured_height);
    for page in 1..=crate::limits::LARGE.slides {
        let (measurement, _) = crate::layout::graph_text_metrics(&format!("{page}, "), 10000.0, 10000.0, size, false, theme)?;
        if measurement.missing_glyphs > 0 || prefix.missing_glyphs > 0 { return Err(Error::Unsupported("reference page captions require readable page-number glyphs".into())); }
        slot_width = slot_width.max(f64::from(measurement.measured_width));
        line_height = line_height.max(f64::from(measurement.measured_height));
    }
    let per_line = ((width - 16.0 - f64::from(prefix.measured_width)) / (slot_width + 2.0)).floor();
    if per_line < 1.0 { return Err(Error::Limit("reference page caption cannot fit a maximum-width page number; use a wider appendix canvas".into())); }
    Ok((per_line as usize, line_height))
}

fn page_numbers(deck: &Deck) -> Result<BTreeMap<String, usize>> {
    if deck.slides.len() > crate::limits::LARGE.slides { return Err(Error::Limit("reference page captions exceed the supported page capacity".into())); }
    Ok(deck.slides.iter().enumerate().map(|(index, slide)| (slide.id.clone(), index + 1)).collect())
}

fn page_caption(entry: &Reference, pages: &BTreeMap<String, usize>, per_line: usize) -> String {
    let mut numbers: Vec<_> = entry.slide_ids.iter().filter_map(|id| pages.get(id).copied()).collect();
    numbers.sort_unstable();
    let numbers: Vec<_> = numbers.into_iter().map(|number| number.to_string()).collect();
    format!("Slides: {}", numbers.chunks(per_line).map(|chunk| chunk.join(", ")).collect::<Vec<_>>().join(",\n"))
}

fn refresh_captions(deck: &mut Deck, state: &mut ReferenceState) -> Result<()> {
    if state.options.placement != Placement::AppendixOnly { return Ok(()); }
    let pages = page_numbers(deck)?;
    for owned in &mut state.elements {
        let Some(id) = owned.id.strip_prefix("aislide-ref-pages-") else { continue; };
        let entry = state.options.entries.iter().find(|entry| entry.id == id).ok_or_else(|| Error::Conflict("managed page caption has no reference".into()))?;
        let slide = deck.slides.iter_mut().find(|slide| slide.id == owned.slide_id).ok_or_else(|| Error::Conflict("managed page caption has no appendix page".into()))?;
        let mut theme = crate::design::slide_theme(slide, deck.design.as_ref()).cloned().unwrap_or_default();
        let element = slide.elements.iter_mut().find(|element| element.bounds().0 == owned.id).ok_or_else(|| Error::Conflict("managed page caption is missing".into()))?;
        let Element::Text { text, width, height, font_size, format, .. } = element else { return Err(Error::Conflict("managed page caption is not text".into())); };
        if let Some(family) = &format.font_family {
            theme.fonts.minor = match family.as_str() { "@major" => theme.fonts.major.clone(), "@minor" => theme.fonts.minor.clone(), _ => family.clone() };
        }
        let frame = format.content_frame(*width, *height, false);
        let available_width = frame[2].min(*width - 16.0);
        let available_height = frame[3].min(*height - 16.0);
        let (per_line, _) = caption_metrics(available_width + 16.0, *font_size, &theme)?;
        let caption = page_caption(entry, &pages, per_line);
        let (measurement, _) = crate::layout::graph_text_metrics(&caption, available_width, available_height, *font_size, false, &theme)?;
        if measurement.overflow || measurement.missing_glyphs > 0 { return Err(Error::Limit("reference page caption does not fit its reserved frame; explicitly retry set_references".into())); }
        *text = caption;
        owned.sha256 = fingerprint(element)?;
    }
    Ok(())
}

fn readable_links(theme: &crate::design::Theme, background: &str) -> Result<bool> {
    for slot in ["@hlink", "@folHlink"] {
        if crate::review::contrast_ratio(&crate::design::resolve_color(slot, Some(theme)), background)? < 4.5 { return Ok(false); }
    }
    Ok(true)
}

fn footer_clear(deck: &Deck, page: usize, height: f64) -> Result<bool> {
    let footer = kurbo::Rect::new(32.0, f64::from(deck.height) - 24.0 - height, f64::from(deck.width) - 32.0, f64::from(deck.height) - 24.0);
    Ok(crate::authoring_preflight::visible_bounds(deck, page)?.iter().all(|bounds| {
        let overlap = bounds.intersect(footer);
        overlap.width() <= 0.0 || overlap.height() <= 0.0
    }))
}

fn marker_top(deck: &Deck, page: usize, height: f64) -> Result<f64> {
    let bounds = crate::authoring_preflight::reference_obstacles(deck, page)?;
    let maximum = f64::from(deck.height) - 24.0 - height;
    let minimum = f64::from(deck.height) * 0.75;
    let width = f64::from(deck.width) - 64.0;
    let mut candidates = vec![maximum];
    candidates.extend(bounds.iter().map(|(_, bounds)| bounds.y0 - 8.0 - height));
    candidates.sort_by(|left, right| right.total_cmp(left));
    for top in candidates.into_iter().filter(|top| *top >= minimum && *top <= maximum) {
        let frame = kurbo::Rect::new(32.0, top, f64::from(deck.width) - 32.0, top + height);
        if bounds.iter().all(|(_, bounds)| { let overlap = bounds.inflate(8.0, 8.0).intersect(frame); overlap.width() <= 0.0 || overlap.height() <= 0.0 }) { return Ok(top); }
    }
    let frame = kurbo::Rect::new(32.0, maximum, f64::from(deck.width) - 32.0, maximum + height);
    let ids: Vec<_> = bounds.iter().filter(|(_, bounds)| { let overlap = bounds.intersect(frame); overlap.width() > 0.0 && overlap.height() > 0.0 }).map(|(id, _)| id.as_str()).collect();
    Err(Error::Conflict(format!("slide {} has no free reference marker band in the bottom quarter with 8px clearance; candidate frame (x=32, y={maximum}, width={width}, height={height}); blocking elements: {}; reserve a band or explicitly select placement=appendix_only and retry set_references", deck.slides[page].id, if ids.is_empty() { "none (clearance or bottom-quarter constraint)".into() } else { ids.join(", ") })))
}

pub(crate) fn refresh(document: &mut Document) -> Result<()> {
    let Some(mut state) = document.references.take() else { return Ok(()); };
    let mut checked = document.deck.clone();
    strip(&mut checked, &state)?;
    let active: BTreeSet<_> = state.options.entries.iter().filter(|entry| checked.slides.iter().any(|slide| entry.slide_ids.contains(&slide.id)))
        .flat_map(|entry| [format!("aislide-ref-entry-{}", entry.id), format!("aislide-ref-url-{}", entry.id), format!("aislide-ref-pages-{}", entry.id)]).collect();
    if !active.is_empty() && state.slides.iter().any(|id| !document.deck.slides.iter().any(|slide| &slide.id == id)) {
        return Err(Error::Conflict("managed reference appendix was removed; use set_references to change or clear it".into()));
    }
    for slide in document.deck.slides.iter_mut().filter(|slide| state.slides.contains(&slide.id)) {
        slide.elements.retain(|element| !(element.bounds().0.starts_with("aislide-ref-entry-") || element.bounds().0.starts_with("aislide-ref-url-") || element.bounds().0.starts_with("aislide-ref-pages-")) || active.contains(element.bounds().0));
    }
    document.deck.slides.retain(|slide| !state.slides.contains(&slide.id) || slide.elements.iter().any(|element| active.contains(element.bounds().0)));
    state.elements.retain(|owned| document.deck.slides.iter().any(|slide| slide.id == owned.slide_id && slide.elements.iter().any(|element| element.bounds().0 == owned.id)));
    state.slides.retain(|id| document.deck.slides.iter().any(|slide| &slide.id == id));
    state.page_sha256.retain(|id, _| state.slides.contains(id));
    refresh_captions(&mut document.deck, &mut state)?;
    document.references = Some(state);
    Ok(())
}

fn arrange(document: &mut Document) -> Result<()> {
    let Some(mut state) = document.references.take() else { return Ok(()); };
    strip(&mut document.deck, &state)?;
    state.elements.clear(); state.slides.clear(); state.page_sha256.clear();
    let entries = state.options.entries.clone();
    let active: Vec<_> = entries.iter().collect();
    let width = f64::from(document.deck.width) - 64.0;
    let height = f64::from(document.deck.height);
    if width < 240.0 || height < 240.0 { return Err(Error::Invalid("references require a canvas at least 304x240 pixels".into())); }
    let size = state.options.font_size;
    let appendix_layout = document.deck.design.as_ref().and_then(|design| design.layouts.iter().find(|layout| layout.elements.is_empty())).cloned();
    let appendix_theme = document.deck.design.as_ref().zip(appendix_layout.as_ref()).map(|(design, layout)| crate::design::master_theme(design, &layout.master_id).clone()).unwrap_or_default();
    let theme_color = crate::design::resolve_color("@dk1", Some(&appendix_theme));
    let appendix_color = if crate::review::contrast_ratio(&theme_color, "FFFFFF")? >= 4.5 { theme_color } else { "000000".into() };
    let heading_height = text_height(&state.options.title, width, 28.0, &appendix_theme)?.max(64.0);
    let entry_top = 40.0 + heading_height;
    let mut appendix = BTreeSet::new();
    for page in 0..document.deck.slides.len() {
        let slide = &document.deck.slides[page];
        let cited: Vec<_> = active.iter().enumerate().filter(|(_, entry)| entry.slide_ids.contains(&slide.id)).collect();
        if cited.is_empty() { continue; }
        if state.options.placement == Placement::AppendixOnly { appendix.extend(cited.iter().map(|(number, _)| *number)); continue; }
        let theme = crate::design::slide_theme(slide, document.deck.design.as_ref()).cloned().unwrap_or_default();
        let background = crate::design::resolve_color(crate::design::slide_background(slide, document.deck.design.as_ref()), Some(&theme));
        let color = if crate::review::contrast_ratio("000000", &background)? >= crate::review::contrast_ratio("FFFFFF", &background)? { "000000" } else { "FFFFFF" };
        let mut footnotes = Vec::new();
        let mut total = 0.0;
        for (number, entry) in &cited {
            let text = format!("[{}] {}", number + 1, entry.name);
            let label_height = text_height(&text, width, size, &theme)?;
            let url_height = text_height(&entry.url, width, size, &theme)?;
            total += label_height + url_height;
            footnotes.push((text, label_height, url_height, *entry));
        }
        let use_footer = state.options.placement != Placement::Appendix && total <= height * 0.22 && readable_links(&theme, &background)? && footer_clear(&document.deck, page, total)?;
        if use_footer {
            let mut top = height - 24.0 - total;
            for (index, (text, label_height, url_height, entry)) in footnotes.into_iter().enumerate() {
                add(&mut document.deck.slides[page], &mut state, text_element(format!("aislide-ref-footnote-{index}"), text, 32.0, top, width, label_height, size, color, None))?;
                top += label_height;
                add(&mut document.deck.slides[page], &mut state, text_element(format!("aislide-ref-footnote-url-{index}"), entry.url.clone(), 32.0, top, width, url_height, size, color, Some(entry.url.clone())))?;
                top += url_height;
            }
        } else {
            appendix.extend(cited.iter().map(|(number, _)| *number));
            let text = format!("{}: {}", state.options.title.lines().collect::<Vec<_>>().join(" "), cited.iter().map(|(number, _)| format!("[{}]", number + 1)).collect::<Vec<_>>().join(" "));
            let needed = text_height(&text, width, size, &theme)?;
            let top = marker_top(&document.deck, page, needed)?;
            add(&mut document.deck.slides[page], &mut state, text_element("aislide-ref-marker".into(), text, 32.0, top, width, needed, size, color, None))?;
        }
    }
    if !appendix.is_empty() && !readable_links(&appendix_theme, "FFFFFF")? {
        return Err(Error::Unsupported("reference appendix requires theme hyperlink colors readable on white; adjust the chosen empty layout's theme".into()));
    }
    let caption_layout = if state.options.placement == Placement::AppendixOnly { Some(caption_metrics(width, size, &appendix_theme)?) } else { None };
    let pages = page_numbers(&document.deck)?;
    let mut top = height;
    for number in appendix {
        let entry = active[number];
        let text = format!("[{}] {}", number + 1, entry.name);
        let label_height = text_height(&text, width, size, &appendix_theme)?;
        let url_height = text_height(&entry.url, width, size, &appendix_theme)?;
        let caption_height = caption_layout.map_or(0.0, |(per_line, line_height)| (entry.slide_ids.len().div_ceil(per_line) as f64 * line_height * 1.2 + 16.0).ceil());
        let needed = label_height + caption_height + url_height;
        if needed > height - entry_top - 24.0 { return Err(Error::Limit("reference page caption, URL and title do not fit one appendix page at the requested font size".into())); }
        if top + needed > height - 24.0 {
            let id = format!("aislide-ref-page-{}", state.slides.len() + 1);
            if document.deck.slides.iter().any(|slide| slide.id == id) { return Err(Error::Conflict("reference appendix ID already exists".into())); }
            let title = state.options.title.clone();
            let mut slide: Slide = serde_json::from_value(json!({"id":id,"title":title,"background":"FFFFFF","notes":"","elements":[],"hide_master_graphics":true}))?;
            if document.deck.design.is_some() {
                slide.layout_id = Some(appendix_layout.as_ref().ok_or_else(|| Error::Unsupported("reference appendix requires an existing empty layout".into()))?.id.clone());
            }
            add(&mut slide, &mut state, text_element("aislide-ref-heading".into(), title, 32.0, 24.0, width, heading_height, 28.0, &appendix_color, None))?;
            state.page_sha256.insert(id.clone(), page_fingerprint(&slide)?);
            state.slides.push(id);
            document.deck.slides.push(slide);
            top = entry_top;
        }
        let page = document.deck.slides.len() - 1;
        add(&mut document.deck.slides[page], &mut state, text_element(format!("aislide-ref-entry-{}", entry.id), text, 32.0, top, width, label_height, size, &appendix_color, None))?;
        if let Some((per_line, _)) = caption_layout {
            add(&mut document.deck.slides[page], &mut state, text_element(format!("aislide-ref-pages-{}", entry.id), page_caption(entry, &pages, per_line), 32.0, top + label_height, width, caption_height, size, &appendix_color, None))?;
        }
        add(&mut document.deck.slides[page], &mut state, text_element(format!("aislide-ref-url-{}", entry.id), entry.url.clone(), 32.0, top + label_height + caption_height, width, url_height, size, &appendix_color, Some(entry.url.clone())))?;
        top += needed + 12.0;
    }
    document.references = Some(state);
    Ok(())
}

pub fn set_references(document: &Document, expected_revision: u64, expected_hash: &str, mut options: ReferenceOptions) -> Result<ReferenceResult> {
    crate::document::verify(document)?;
    if document.revision != expected_revision || document.hash != expected_hash { return Err(Error::Conflict("stale document revision or content hash".into())); }
    if options.entries.len() > 64 { return Err(Error::Limit("at most 64 references".into())); }
    let supplied = options.entries.len();
    options.entries.retain(|entry| entry.publish);
    let publication = Publication { supplied, published: options.entries.len(), excluded: supplied - options.entries.len() };
    validate_options(&options)?;
    let mut working = document.clone();
    if let Some(state) = working.references.take() { strip(&mut working.deck, &state)?; }
    for entry in &options.entries {
        if entry.slide_ids.iter().any(|id| !working.deck.slides.iter().any(|slide| &slide.id == id)) { return Err(Error::Invalid("reference target slide does not exist".into())); }
    }
    working.references = if options.entries.is_empty() { None } else { Some(ReferenceState { options, elements: Vec::new(), slides: Vec::new(), page_sha256: BTreeMap::new() }) };
    arrange(&mut working)?;
    let mut operations = vec![json!({"op":"replace","path":"/deck","value":working.deck})];
    if let Some(state) = working.references { operations.push(json!({"op":"add","path":"/references","value":state})); }
    else if document.references.is_some() { operations.push(json!({"op":"remove","path":"/references"})); }
    let transaction = crate::document::transact(document, Transaction { expected_revision, expected_hash: expected_hash.into(), operations: serde_json::from_value(json!(operations))? })?;
    Ok(ReferenceResult { transaction, publication })
}

fn normalized_url(value: &str) -> Option<String> {
    crate::model::validate_hyperlink(value).ok()?;
    reqwest::Url::parse(value).ok().map(|url| url.to_string())
}

fn urls(text: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let mut offset = 0;
    while offset < text.len() {
        let Some(relative) = [text[offset..].find("https://"), text[offset..].find("http://")].into_iter().flatten().min() else { break; };
        let start = offset + relative;
        let tail = &text[start..];
        let opener = text[..start].chars().next_back();
        let closer = match opener {
            Some('(') => Some(')'), Some('[') => Some(']'), Some('<') => Some('>'),
            Some('\u{ff08}') => Some('\u{ff09}'), Some('\u{300c}') => Some('\u{300d}'), Some('\u{300e}') => Some('\u{300f}'),
            Some('"') => Some('"'), Some('\'') => Some('\''), _ => None,
        };
        let explicit_end = closer.and_then(|close| {
            let mut depth = 0;
            for (index, character) in tail.char_indices() {
                if Some(character) == opener && Some(character) != closer { depth += 1; }
                if character == close { if depth == 0 { return Some(index); } depth -= 1; }
                if character.is_whitespace() { break; }
            }
            None
        });
        let end = explicit_end.unwrap_or_else(|| {
            tail.char_indices().find(|(_, character)| character.is_whitespace() || matches!(character, '<' | '>' | '"' | '\'' | '\u{2e80}'..='\u{9fff}' | '\u{ac00}'..='\u{d7af}' | '\u{ff00}'..='\u{ffef}')).map_or(tail.len(), |(index, _)| index)
        });
        let mut value = tail[..end].trim_end_matches(['.', ',', ';']);
        for (open, close) in [('(', ')'), ('[', ']'), ('{', '}')] {
            while value.ends_with(close) && value.chars().filter(|character| *character == close).count() > value.chars().filter(|character| *character == open).count() {
                value = &value[..value.len() - close.len_utf8()];
            }
        }
        if let Some(url) = normalized_url(value) { result.insert(url); }
        offset = start + end.max("http://".len());
    }
    result
}

fn visible_urls(elements: &[Element], ids: &BTreeSet<String>, result: &mut BTreeSet<String>) {
    for element in elements {
        if element.visual().is_some_and(|visual| visual.hidden || visual.opacity == Some(0.0)) { continue; }
        match element {
            Element::Text { id, text, .. } | Element::Shape { id, text, .. } if ids.contains(id) => result.extend(urls(text)),
            Element::Table { id, rows, .. } if ids.contains(id) => { for text in rows.iter().flatten() { result.extend(urls(text)); } }
            Element::Group { children, .. } => visible_urls(children, ids, result),
            _ => {}
        }
    }
}

pub(crate) fn notes_only_count(document: &Document, page: usize) -> Result<usize> {
    let slide = &document.deck.slides[page];
    let mut notes = urls(&slide.notes);
    for paragraph in &slide.notes_paragraphs {
        let text: String = paragraph.runs.iter().map(|run| run.text.as_str()).collect();
        notes.extend(urls(&text));
    }
    let mut visible = BTreeSet::new();
    visible_urls(&slide.elements, &crate::authoring_preflight::visible_reference_ids(&document.deck, page)?, &mut visible);
    if let Some(state) = &document.references {
        let marker = state.elements.iter().any(|owned| owned.slide_id == slide.id && owned.id == "aislide-ref-marker" && slide.elements.iter().any(|element| element.bounds().0 == owned.id && fingerprint(element).ok().as_ref() == Some(&owned.sha256)));
        if marker || state.options.placement == Placement::AppendixOnly {
            for (page_index, page) in document.deck.slides.iter().enumerate().filter(|(_, page)| state.slides.contains(&page.id)) {
                let ids = crate::authoring_preflight::visible_reference_ids(&document.deck, page_index)?;
                let mut appendix = BTreeSet::new(); visible_urls(&page.elements, &ids, &mut appendix);
                visible.extend(state.options.entries.iter().filter(|entry| entry.slide_ids.contains(&slide.id)).filter(|entry| {
                    marker || state.elements.iter().any(|owned| owned.slide_id == page.id && owned.id == format!("aislide-ref-pages-{}", entry.id) && ids.contains(&owned.id)
                        && page.elements.iter().any(|element| element.bounds().0 == owned.id && fingerprint(element).ok().as_ref() == Some(&owned.sha256)))
                }).filter_map(|entry| normalized_url(&entry.url)).filter(|url| appendix.contains(url)));
            }
        }
    }
    Ok(notes.difference(&visible).count())
}