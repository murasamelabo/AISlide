//! Slide transitions (`p:transition`) and animations (`p:timing`) of the parts of a template or presentation.
//!
//! AISlide neither plays nor edits them, and the preview never shows them. PowerPoint applies the transition set on a master or layout to
//! every slide that inherits it, so they are reported (template inspection and preflight) and can be stripped when a template is imported.
//! Both elements are read with their namespaces, either as a direct child of the part's root or inside `mc:AlternateContent`, where
//! PowerPoint writes the effects it introduced in 2010.

use crate::{native::{relations_path, P, R, REL}, native_save::apply, package::Package, pptx::{parse, resolve}, Error, Result};
use roxmltree::Node;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";

/// The direct settings of a `p:transition`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Transition {
    /// Local name of the effect element (`fade`, `push`, `vortex`, ...); `none` for a transition without one.
    pub kind: String,
    /// The effect is outside the base presentationml namespace, as the effects PowerPoint 2010 and later added are.
    pub extended: bool,
    #[serde(skip_serializing_if = "Option::is_none")] pub speed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub duration_ms: Option<u32>,
    /// The slide advances by itself after this time (`advTm`).
    #[serde(skip_serializing_if = "Option::is_none")] pub advance_after_ms: Option<u32>,
    /// What the transition does with sound (`p:sndAc`): `start` plays one and `stop` ends the one that is playing. A slide show does it when
    /// the slide appears, even without a visual effect.
    #[serde(skip_serializing_if = "Option::is_none")] pub sound: Option<&'static str>,
}

/// What a part sets directly. `transition` is `None` when it has no `p:transition`, which differs from a transition without an effect:
/// the first sets nothing and inherits, the second overrides what the part would inherit.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct Effects {
    pub transition: Option<Transition>,
    pub animations: bool,
}

/// The `p:transition` and `p:timing` elements a part's root holds, directly or in an alternate-content branch, in document order.
fn found<'a>(root: Node<'a, 'a>) -> (Vec<Node<'a, 'a>>, Vec<Node<'a, 'a>>) {
    let (mut transitions, mut timings) = (Vec::new(), Vec::new());
    for node in root.children().filter(|node| node.is_element()) {
        if node.has_tag_name((P, "transition")) { transitions.push(node); }
        else if node.has_tag_name((P, "timing")) { timings.push(node); }
        else if node.has_tag_name((MC, "AlternateContent")) {
            for branch in node.children().filter(|branch| branch.has_tag_name((MC, "Choice")) || branch.has_tag_name((MC, "Fallback"))) {
                for entry in branch.children().filter(|entry| entry.is_element()) {
                    if entry.has_tag_name((P, "transition")) { transitions.push(entry); }
                    else if entry.has_tag_name((P, "timing")) { timings.push(entry); }
                }
            }
        }
    }
    (transitions, timings)
}

fn transition(node: Node<'_, '_>) -> Transition {
    let effect = node.children().find(|entry| entry.is_element() && !(entry.tag_name().namespace() == Some(P) && matches!(entry.tag_name().name(), "sndAc" | "extLst")));
    let (kind, extended) = effect.map_or(("none".to_owned(), false), |effect| (effect.tag_name().name().chars().take(64).collect(), effect.tag_name().namespace() != Some(P)));
    let milliseconds = |value: Option<&str>| value.and_then(|value| value.parse::<u32>().ok());
    Transition {
        kind, extended,
        speed: node.attribute("spd").filter(|value| value.len() <= 16).map(str::to_owned),
        duration_ms: milliseconds(node.attribute((P14, "dur"))),
        advance_after_ms: milliseconds(node.attribute("advTm")),
        sound: node.children().find(|entry| entry.has_tag_name((P, "sndAc"))).and_then(|action| action.children().find(|entry| entry.is_element()))
            .map(|sound| if sound.has_tag_name((P, "endSnd")) { "stop" } else { "start" }),
    }
}

/// Whether a `p:timing` defines an animation. PowerPoint writes an effect as behavior and media nodes, so a timing tree without any, such
/// as an empty element or only its root node, animates nothing.
fn animates(timing: Node<'_, '_>) -> bool {
    timing.descendants().any(|node| node.tag_name().namespace() == Some(P)
        && matches!(node.tag_name().name(), "anim" | "animClr" | "animEffect" | "animMotion" | "animRot" | "animScale" | "set" | "cmd" | "audio" | "video"))
}

/// Reads what the part at `path` sets. The first transition in document order names the effect, which is the richer alternate-content
/// choice when PowerPoint wrote both it and a fallback.
pub(crate) fn read(package: &Package, path: &str) -> Result<Effects> {
    let parsed = parse(package.text(path)?)?;
    let (transitions, timings) = found(parsed.root_element());
    Ok(Effects { transition: transitions.first().map(|node| transition(*node)), animations: timings.iter().any(|timing| animates(*timing)) })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Removed { pub transitions: usize, pub animations: usize }

/// Removes the transitions and/or animations the part at `path` sets directly. An alternate-content element is removed whole when everything
/// in its branches is the element to remove, and counts once however many branches repeat it; one that mixes it with other markup is not
/// guessed at and fails. Only a `p:timing` that defines an animation counts as an animation removed.
pub(crate) fn strip(package: &mut Package, path: &str, transitions: bool, animations: bool) -> Result<Removed> {
    if !transitions && !animations { return Ok(Removed::default()); }
    let xml = package.text(path)?.to_owned();
    let parsed = parse(&xml)?;
    let (mut edits, mut removed) = (Vec::new(), Removed::default());
    for node in parsed.root_element().children().filter(|node| node.is_element()) {
        if node.has_tag_name((P, "transition")) && transitions { edits.push((node.range(), String::new())); removed.transitions += 1; }
        else if node.has_tag_name((P, "timing")) && animations { edits.push((node.range(), String::new())); removed.animations += usize::from(animates(node)); }
        else if node.has_tag_name((MC, "AlternateContent")) {
            let entries: Vec<Node<'_, '_>> = node.children().filter(|branch| branch.has_tag_name((MC, "Choice")) || branch.has_tag_name((MC, "Fallback")))
                .flat_map(|branch| branch.children().filter(|entry| entry.is_element())).collect();
            for (name, requested) in [("transition", transitions), ("timing", animations)] {
                if !requested || !entries.iter().any(|entry| entry.has_tag_name((P, name))) { continue; }
                if entries.iter().any(|entry| !entry.has_tag_name((P, name))) {
                    return Err(Error::Unsupported(format!("{path} wraps a p:{name} in alternate content together with other markup; remove it in PowerPoint before importing")));
                }
                edits.push((node.range(), String::new()));
                if name == "transition" { removed.transitions += 1; } else { removed.animations += usize::from(entries.iter().any(|entry| animates(*entry))); }
            }
        }
    }
    if edits.is_empty() { return Ok(removed); }
    // A single alternate-content element can be listed twice only when it mixes both kinds, which the check above rejects.
    package.replace_part(path, apply(xml, edits)?)?;
    Ok(removed)
}

/// The internal image parts a part references through its relationships. External references are never fetched (template inspection rejects
/// them before this is called), so they are not counted.
pub(crate) fn images(package: &Package, path: &str) -> Result<BTreeSet<String>> {
    let relations = relations_path(path);
    let mut parts = BTreeSet::new();
    if !package.part_names().contains(relations.as_str()) { return Ok(parts); }
    let parsed = parse(package.text(&relations)?)?;
    for node in parsed.root_element().children().filter(|node| node.has_tag_name((REL, "Relationship")) && node.attribute("Type") == Some(format!("{R}/image").as_str())) {
        if node.attribute("TargetMode").is_some_and(|mode| !mode.eq_ignore_ascii_case("Internal")) { continue; }
        let target = node.attribute("Target").ok_or_else(|| Error::Invalid("image relationship target missing".into()))?;
        parts.insert(resolve(path, target)?);
    }
    Ok(parts)
}

/// The count and uncompressed size of a set of image parts; the same part counts once.
pub(crate) fn image_report(package: &Package, parts: &BTreeSet<String>) -> Value {
    json!({"count": parts.len(), "bytes": parts.iter().filter_map(|part| package.part(part).ok()).map(<[u8]>::len).sum::<usize>()})
}

pub(crate) fn transition_json(transition: &Option<Transition>) -> Value { serde_json::to_value(transition).unwrap_or(Value::Null) }
