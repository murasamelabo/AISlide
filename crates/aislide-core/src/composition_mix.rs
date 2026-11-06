//! A deck's mix of managed part presets: how many slides use each, the longest runs of one preset and how many slides draw a relationship.
//!
//! It is information for a review, never a target: it counts managed parts only, so hand-drawn shapes, imported slides and text-only slides
//! are reported as holding no part rather than classified. A part edited by hand after it was created (stale) is counted under the preset
//! recorded for it, and `stale_parts` says how many there are, since what is drawn may no longer match.

use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The presets that draw relations between named things; a trailing `*` matches the whole category.
pub(crate) const RELATIONSHIP_PRESETS: [&str; 5] = ["diagram/custom", "correlation/*", "radiation/*", "tree/*", "flow/swimlane"];

const LISTED_PRESETS: usize = 12;
const MINIMUM_RUN: usize = 3;
const LISTED_RUNS: usize = 5;
const NOTE: &str = "Information for review, not a target or a quota. Counts cover managed parts only: slides without one (text-only, hand-drawn, imported) are counted in without_parts, not classified. Equal peers belong in a list; when many slides repeat one preset while the content relates, bounds or hands off named things, reread it for a relationship diagram. An explicit no-diagram request wins.";

fn relationship(preset: &str) -> bool {
    RELATIONSHIP_PRESETS.iter().any(|entry| entry.strip_suffix('*').map_or(*entry == preset, |prefix| preset.starts_with(prefix)))
}

/// One slide, in deck order: its ID and the presets of the managed parts it holds (none when it holds no part).
pub(crate) struct SlideParts { pub id: String, pub presets: Vec<String> }

/// `stale_parts` is how many of the counted parts were edited by hand after they were created; it is reported only when it is not zero.
pub(crate) fn mix(slides: &[SlideParts], stale_parts: usize) -> Value {
    let mut by_preset: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for slide in slides {
        let mut seen = BTreeSet::new();
        for preset in &slide.presets {
            let entry = by_preset.entry(preset.as_str()).or_default();
            entry.1 += 1;
            if seen.insert(preset.as_str()) { entry.0 += 1; }
        }
    }
    let mut parts: Vec<(&str, usize, usize)> = by_preset.iter().map(|(preset, (slides, instances))| (*preset, *slides, *instances)).collect();
    parts.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));
    let other = parts.len().saturating_sub(LISTED_PRESETS);
    // The longest run of consecutive slides that hold the preset, for each preset.
    let mut runs: Vec<(&str, usize, usize)> = Vec::new();
    for (preset, _, _) in &parts {
        let (mut best, mut start, mut length) = ((0, 0), 0, 0);
        for (index, slide) in slides.iter().enumerate() {
            if slide.presets.iter().any(|candidate| candidate == preset) {
                if length == 0 { start = index; }
                length += 1;
                if length > best.1 { best = (start, length); }
            } else { length = 0; }
        }
        if best.1 >= MINIMUM_RUN { runs.push((*preset, best.0, best.1)); }
    }
    runs.sort_by(|left, right| right.2.cmp(&left.2).then(left.0.cmp(right.0)));
    let longest_runs: Vec<Value> = runs.into_iter().take(LISTED_RUNS).map(|(preset, start, length)| json!({
        "preset": preset, "slides": length, "first_slide_id": slides[start].id, "last_slide_id": slides[start + length - 1].id,
    })).collect();
    let mut value = json!({
        "slides": slides.len(),
        "parts": parts.iter().take(LISTED_PRESETS).map(|(preset, slides, instances)| json!({"preset": preset, "slides": slides, "instances": instances})).collect::<Vec<_>>(),
        "longest_runs": longest_runs,
        "relationship_diagrams": slides.iter().filter(|slide| slide.presets.iter().any(|preset| relationship(preset))).count(),
        "relationship_presets": RELATIONSHIP_PRESETS,
        "without_parts": slides.iter().filter(|slide| slide.presets.is_empty()).count(),
        "note": NOTE,
    });
    if other > 0 { value["other_presets"] = json!(other); }
    if stale_parts > 0 {
        value["stale_parts"] = json!(stale_parts);
        value["note"] = json!(format!("{NOTE} {stale_parts} of the counted parts are stale: they were edited by hand after they were created, so the preset recorded for them may no longer describe what is drawn."));
    }
    value
}
