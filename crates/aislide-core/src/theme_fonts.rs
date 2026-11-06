//! Which typefaces of a theme's font scheme the file specifies.
//!
//! AISlide models four effective fonts (`ThemeFonts`: major, minor, East Asian, complex script), which it needs to draw and measure
//! text. A file can leave any of the six typeface slots (major and minor, each Latin, East Asian and complex script) empty or omit
//! it; PowerPoint then chooses a font for the viewer's language and installation. `ThemeFonts::unset` records those slots, so that
//! the stand-in AISlide draws with is not mistaken for the file's own setting, and so that a theme edit writes only what the caller
//! specified, but always writes that, even when it equals the stand-in.

use crate::{design::{Theme, ThemeFonts}, model::valid_text, native::{child, A}, Error, Result};
use roxmltree::Node;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const DEFAULT_LATIN: &str = "Aptos";
pub const DEFAULT_EAST_ASIAN: &str = "Yu Gothic";
pub const DEFAULT_COMPLEX_SCRIPT: &str = "Arial";
const SCRIPT_FONT_LIMIT: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FontGroup { Major, Minor }

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FontScript { Latin, EastAsian, ComplexScript }

impl FontGroup {
    pub const ALL: [FontGroup; 2] = [FontGroup::Major, FontGroup::Minor];
    pub fn name(self) -> &'static str { match self { Self::Major => "major", Self::Minor => "minor" } }
    pub(crate) fn element(self) -> &'static str { match self { Self::Major => "majorFont", Self::Minor => "minorFont" } }
}

impl FontScript {
    pub const ALL: [FontScript; 3] = [FontScript::Latin, FontScript::EastAsian, FontScript::ComplexScript];
    pub fn name(self) -> &'static str { match self { Self::Latin => "latin", Self::EastAsian => "east_asian", Self::ComplexScript => "complex_script" } }
    pub(crate) fn element(self) -> &'static str { match self { Self::Latin => "latin", Self::EastAsian => "ea", Self::ComplexScript => "cs" } }
    pub fn default_font(self) -> &'static str { match self { Self::Latin => DEFAULT_LATIN, Self::EastAsian => DEFAULT_EAST_ASIAN, Self::ComplexScript => DEFAULT_COMPLEX_SCRIPT } }
    fn label(self) -> &'static str { match self { Self::Latin => "Latin", Self::EastAsian => "East Asian", Self::ComplexScript => "complex-script" } }
}

/// How the file leaves a typeface slot unspecified, and whether an edit has named a font for it since.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsetFont {
    /// The element is present with `typeface=""` (or only blanks).
    Empty,
    /// The element is missing from the font collection.
    Absent,
    /// The file leaves the slot unspecified and an edit named a font for it: the name is written when the file is saved, even when it is
    /// the one AISlide showed. Only an edit sets this; reading a file never does.
    Written,
}

impl UnsetFont {
    fn name(self) -> &'static str { match self { Self::Empty => "empty", Self::Absent => "absent", Self::Written => "written" } }
    /// Whether the slot is still unspecified, which `Written` no longer is.
    pub fn is_unspecified(self) -> bool { self != Self::Written }
}

/// The unspecified typeface slots of one font collection (`a:majorFont` or `a:minorFont`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsetFonts {
    #[serde(default, skip_serializing_if = "Option::is_none")] pub latin: Option<UnsetFont>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub east_asian: Option<UnsetFont>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub complex_script: Option<UnsetFont>,
    /// Typefaces of the collection's script-specific `a:font` entries by script tag (`Jpan`, `Hans`, ...). PowerPoint uses them for text of
    /// that script while the matching slot is unspecified, so text they cover is not reported. Informational: edits keep the package's own.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")] pub script_fonts: BTreeMap<String, String>,
}

impl UnsetFonts {
    pub fn is_empty(&self) -> bool { self.latin.is_none() && self.east_asian.is_none() && self.complex_script.is_none() }
    fn get(&self, script: FontScript) -> Option<UnsetFont> {
        match script { FontScript::Latin => self.latin, FontScript::EastAsian => self.east_asian, FontScript::ComplexScript => self.complex_script }
    }
    fn set(&mut self, script: FontScript, state: Option<UnsetFont>) {
        match script { FontScript::Latin => self.latin = state, FontScript::EastAsian => self.east_asian = state, FontScript::ComplexScript => self.complex_script = state }
    }
}

/// The typeface slots a theme's font scheme leaves empty or omits, and the ones an edit has named since. Empty (and omitted from JSON) when
/// the file specifies all six.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsetFontSlots {
    #[serde(default, skip_serializing_if = "UnsetFonts::is_empty")] pub major: UnsetFonts,
    #[serde(default, skip_serializing_if = "UnsetFonts::is_empty")] pub minor: UnsetFonts,
}

impl UnsetFontSlots {
    pub fn is_empty(&self) -> bool { self.major.is_empty() && self.minor.is_empty() }
    pub fn group(&self, group: FontGroup) -> &UnsetFonts { match group { FontGroup::Major => &self.major, FontGroup::Minor => &self.minor } }
    fn group_mut(&mut self, group: FontGroup) -> &mut UnsetFonts { match group { FontGroup::Major => &mut self.major, FontGroup::Minor => &mut self.minor } }
    /// What the file does with the slot, including `Written` once an edit has named it; `None` for a slot the file specifies.
    pub fn state(&self, group: FontGroup, script: FontScript) -> Option<UnsetFont> { self.group(group).get(script) }
    /// The state of a slot that is still unspecified: `None` for a specified slot and for one an edit has named.
    pub fn unspecified(&self, group: FontGroup, script: FontScript) -> Option<UnsetFont> { self.state(group, script).filter(|state| state.is_unspecified()) }
    /// Script fonts only matter where a slot is unspecified.
    fn tidy(&mut self) {
        for group in FontGroup::ALL { if self.group(group).is_empty() { self.group_mut(group).script_fonts.clear(); } }
    }
}

impl ThemeFonts {
    pub fn new(major: impl Into<String>, minor: impl Into<String>, east_asian: impl Into<String>, complex_script: impl Into<String>) -> Self {
        Self { major: major.into(), minor: minor.into(), east_asian: east_asian.into(), complex_script: complex_script.into(), unset: Box::default() }
    }

    /// The font AISlide draws a slot with. East Asian and complex-script text use one font for both collections.
    pub fn family(&self, group: FontGroup, script: FontScript) -> &str {
        match (group, script) {
            (FontGroup::Major, FontScript::Latin) => &self.major,
            (FontGroup::Minor, FontScript::Latin) => &self.minor,
            (_, FontScript::EastAsian) => &self.east_asian,
            (_, FontScript::ComplexScript) => &self.complex_script,
        }
    }

    /// The fonts a theme edit leaves in the document. A slot the file leaves unspecified stays so only while the edit lists it in `unset`
    /// and its font name is unchanged. Naming a font for it, including the one AISlide showed, or leaving the entry out specifies it: the slot
    /// becomes `Written`, and the name is saved even when it equals the stand-in. A written slot stays written, and a slot the file
    /// specifies is a plain edit whatever `unset` says. Script fonts are facts of the package.
    pub(crate) fn after_edit(&mut self, previous: &ThemeFonts) {
        let mut kept = UnsetFontSlots::default();
        for group in FontGroup::ALL {
            for script in FontScript::ALL {
                let state = match previous.unset.state(group, script) {
                    None => None,
                    Some(UnsetFont::Written) => Some(UnsetFont::Written),
                    // Sent back as unspecified with the name unchanged; a `written` marker, like a new name or no marker, names the slot.
                    Some(unset) if self.unset.unspecified(group, script).is_some() && self.family(group, script) == previous.family(group, script) => Some(unset),
                    Some(_) => Some(UnsetFont::Written),
                };
                kept.group_mut(group).set(script, state);
            }
            kept.group_mut(group).script_fonts = previous.unset.group(group).script_fonts.clone();
        }
        kept.tidy();
        self.unset = Box::new(kept);
    }

    /// Brings the markers of a deck's theme in line with the file the deck was opened from, before a native save compares the two. A slot
    /// keeps what the file has unless an edit named it (`Written`) or its name changed, so a deck saved before markers existed, which has
    /// none, leaves the file's empty slots alone, and a marker on a slot the file specifies cannot empty it.
    fn align_with_file(&mut self, file: &ThemeFonts) {
        let mut aligned = UnsetFontSlots::default();
        for group in FontGroup::ALL {
            for script in FontScript::ALL {
                let in_file = file.unset.state(group, script);
                let state = match self.unset.state(group, script) {
                    Some(UnsetFont::Written) if in_file.is_some() => Some(UnsetFont::Written),
                    _ if self.family(group, script) == file.family(group, script) => in_file,
                    _ => None,
                };
                aligned.group_mut(group).set(script, state);
            }
            aligned.group_mut(group).script_fonts = file.unset.group(group).script_fonts.clone();
        }
        aligned.tidy();
        self.unset = Box::new(aligned);
    }

    pub(crate) fn validate(&self) -> Result<()> {
        for group in FontGroup::ALL {
            let entries = &self.unset.group(group).script_fonts;
            if entries.len() > SCRIPT_FONT_LIMIT { return Err(Error::Limit(format!("theme fonts unset.{}.script_fonts allows at most {SCRIPT_FONT_LIMIT} entries", group.name()))); }
            for (script, typeface) in entries {
                if script.is_empty() || script.len() > 15 || !script.bytes().all(|byte| byte.is_ascii_alphanumeric()) { return Err(Error::Invalid("theme font script tags are 1-15 ASCII letters or digits".into())); }
                valid_text(typeface, 100)?;
                if typeface.trim().is_empty() || typeface.chars().any(char::is_control) { return Err(Error::Invalid("theme script font names must be nonempty and contain no control characters".into())); }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
struct Slot { state: Option<UnsetFont>, typeface: String }

/// What a font scheme says for each of the six slots, before AISlide substitutes its defaults for the ones it leaves unspecified.
pub(crate) struct Scheme { slots: [[Slot; 3]; 2], script_fonts: [BTreeMap<String, String>; 2] }

fn index(group: FontGroup) -> usize { group as usize }
fn script_index(script: FontScript) -> usize { script as usize }

impl Scheme {
    /// Reads an `a:fontScheme`; a theme without one leaves every slot omitted.
    pub(crate) fn read(scheme: Option<Node<'_, '_>>) -> Scheme {
        let mut result = Scheme { slots: Default::default(), script_fonts: Default::default() };
        for group in FontGroup::ALL {
            let collection = scheme.and_then(|scheme| child(scheme, A, group.element()));
            for script in FontScript::ALL {
                let slot = &mut result.slots[index(group)][script_index(script)];
                match collection.and_then(|collection| child(collection, A, script.element())) {
                    None => slot.state = Some(UnsetFont::Absent),
                    Some(font) => match font.attribute("typeface") {
                        Some(typeface) if !typeface.trim().is_empty() => slot.typeface = typeface.to_owned(),
                        _ => slot.state = Some(UnsetFont::Empty),
                    },
                }
            }
            for font in collection.into_iter().flat_map(|collection| collection.children()).filter(|node| node.has_tag_name((A, "font"))) {
                let (Some(script), Some(typeface)) = (font.attribute("script"), font.attribute("typeface")) else { continue };
                if !typeface.trim().is_empty() && result.script_fonts[index(group)].len() < SCRIPT_FONT_LIMIT { result.script_fonts[index(group)].entry(script.to_owned()).or_insert_with(|| typeface.to_owned()); }
            }
        }
        result
    }

    fn slot(&self, group: FontGroup, script: FontScript) -> &Slot { &self.slots[index(group)][script_index(script)] }

    /// The font AISlide draws a slot with: the file's own, else its default.
    fn font(&self, group: FontGroup, script: FontScript) -> &str {
        let slot = self.slot(group, script);
        if slot.state.is_none() { &slot.typeface } else { script.default_font() }
    }

    /// The effective fonts of the model: Latin per collection, East Asian and complex script from the minor collection.
    pub(crate) fn fonts(&self) -> ThemeFonts {
        let mut fonts = ThemeFonts::new(self.font(FontGroup::Major, FontScript::Latin), self.font(FontGroup::Minor, FontScript::Latin),
            self.font(FontGroup::Minor, FontScript::EastAsian), self.font(FontGroup::Minor, FontScript::ComplexScript));
        for group in FontGroup::ALL {
            for script in FontScript::ALL { fonts.unset.group_mut(group).set(script, self.slot(group, script).state); }
            fonts.unset.group_mut(group).script_fonts = self.script_fonts[index(group)].clone();
        }
        fonts.unset.tidy();
        fonts
    }

    /// The report of `inspect_template`: every slot with the file's own typeface.
    pub(crate) fn report(&self, name: &str) -> Value {
        let group = |group: FontGroup| {
            let mut value = serde_json::Map::new();
            for script in FontScript::ALL { value.insert(script.name().into(), slot_report(self.slot(group, script).state, self.font(group, script))); }
            if self.slots[index(group)].iter().any(|slot| slot.state.is_some()) && !self.script_fonts[index(group)].is_empty() { value.insert("script_fonts".into(), json!(self.script_fonts[index(group)])); }
            Value::Object(value)
        };
        with_notes(json!({"name": name, "major": group(FontGroup::Major), "minor": group(FontGroup::Minor)}), |group, script| self.slot(group, script).state.map(|state| (state, self.font(group, script).to_owned())))
    }
}

fn slot_report(state: Option<UnsetFont>, font: &str) -> Value {
    json!({"state": state.map_or("specified", UnsetFont::name), "font": font, "installed": crate::layout::font_installed(font)})
}

fn with_notes(mut report: Value, unset: impl Fn(FontGroup, FontScript) -> Option<(UnsetFont, String)>) -> Value {
    let notes: Vec<String> = FontGroup::ALL.into_iter().flat_map(|group| FontScript::ALL.into_iter().map(move |script| (group, script)))
        .filter_map(|(group, script)| unset(group, script).map(|(state, font)| format!("{}.{} is {} in the file: PowerPoint chooses a font for the viewer's language and installation; AISlide draws it with {font}", group.name(), script.name(), state.name())))
        .collect();
    report["notes"] = json!(notes);
    report
}

/// The report of `design_tokens` and `import_template` for a document's themes: the fonts AISlide draws with and the slots the file
/// leaves unspecified. The model keeps one East Asian and one complex-script font (from the minor collection); `inspect_template`
/// reads each collection's own typefaces from the package.
pub fn report(design: &crate::design::Design) -> Value {
    let one = |scope: &str, theme: &Theme| {
        let fonts = &theme.fonts;
        let group = |group: FontGroup| {
            let mut value = serde_json::Map::new();
            for script in FontScript::ALL { value.insert(script.name().into(), slot_report(fonts.unset.unspecified(group, script), fonts.family(group, script))); }
            if !fonts.unset.group(group).script_fonts.is_empty() { value.insert("script_fonts".into(), json!(fonts.unset.group(group).script_fonts)); }
            Value::Object(value)
        };
        with_notes(json!({"scope": scope, "name": theme.name, "major": group(FontGroup::Major), "minor": group(FontGroup::Minor)}),
            |group, script| fonts.unset.unspecified(group, script).map(|state| (state, fonts.family(group, script).to_owned())))
    };
    let mut themes = vec![one("default", &design.theme)];
    themes.extend(design.masters.iter().filter_map(|master| master.theme.as_ref().map(|theme| one(&master.id, theme))));
    json!(themes)
}

fn is_east_asian(character: char) -> bool {
    matches!(character, '\u{3000}'..='\u{9fff}' | '\u{ac00}'..='\u{d7ff}' | '\u{f900}'..='\u{faff}' | '\u{ff00}'..='\u{ffef}' | '\u{20000}'..='\u{3134f}')
}

fn is_complex_script(character: char) -> bool { matches!(character, '\u{0590}'..='\u{08ff}' | '\u{0900}'..='\u{0dff}') }

/// The script tag (ISO 15924) under which a font scheme lists a font for a complex-script character, when the scheme has tags for it.
fn complex_script_tag(character: char) -> Option<&'static str> {
    Some(match character {
        '\u{0590}'..='\u{05ff}' => "Hebr",
        '\u{0600}'..='\u{06ff}' | '\u{0750}'..='\u{077f}' | '\u{08a0}'..='\u{08ff}' => "Arab",
        '\u{0700}'..='\u{074f}' => "Syrc",
        '\u{0780}'..='\u{07bf}' => "Thaa",
        '\u{0900}'..='\u{097f}' => "Deva",
        '\u{0980}'..='\u{09ff}' => "Beng",
        '\u{0a00}'..='\u{0a7f}' => "Guru",
        '\u{0a80}'..='\u{0aff}' => "Gujr",
        '\u{0b00}'..='\u{0b7f}' => "Orya",
        '\u{0b80}'..='\u{0bff}' => "Taml",
        '\u{0c00}'..='\u{0c7f}' => "Telu",
        '\u{0c80}'..='\u{0cff}' => "Knda",
        '\u{0d00}'..='\u{0d7f}' => "Mlym",
        '\u{0d80}'..='\u{0dff}' => "Sinh",
        _ => return None,
    })
}

/// The script whose theme font draws a run of text.
pub(crate) fn script_of(text: &str) -> FontScript {
    if text.chars().any(is_east_asian) { FontScript::EastAsian }
    else if text.chars().any(is_complex_script) { FontScript::ComplexScript }
    else { FontScript::Latin }
}

/// Whether the collection's script-specific `a:font` entries give text of this script a font while its slot is unspecified. Complex-script
/// text is covered when the scheme lists every script the text uses; a script the scheme has no tag for is never covered.
fn script_fonts_cover(text: &str, script: FontScript, entries: &BTreeMap<String, String>) -> bool {
    let has = |tags: &[&str]| tags.iter().any(|tag| entries.contains_key(*tag));
    match script {
        FontScript::Latin => false,
        FontScript::EastAsian => {
            let kana = text.chars().any(|character| matches!(character, '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}' | '\u{ff66}'..='\u{ff9f}'));
            let hangul = text.chars().any(|character| matches!(character, '\u{1100}'..='\u{11ff}' | '\u{3130}'..='\u{318f}' | '\u{ac00}'..='\u{d7af}'));
            // Kana needs the Japanese entry and Hangul the Korean one, each when the text has it; Han characters alone take any Han entry.
            if kana || hangul { (!kana || has(&["Jpan"])) && (!hangul || has(&["Hang"])) } else { has(&["Jpan", "Hans", "Hant"]) }
        }
        FontScript::ComplexScript => {
            let used: std::collections::BTreeSet<Option<&str>> = text.chars().filter(|character| is_complex_script(*character)).map(complex_script_tag).collect();
            !used.is_empty() && used.iter().all(|tag| tag.is_some_and(|tag| entries.contains_key(tag)))
        }
    }
}

/// The unspecified theme slot a run of text depends on, if any. `None` for text with its own font, for specified slots (including one an
/// edit has named), and for text whose script the font scheme covers with a script-specific font.
pub(crate) fn unset_slot(text: &str, requested: Option<&str>, theme: &Theme) -> Option<(FontGroup, FontScript)> {
    let (group, script) = crate::fonts::theme_slot(text, requested)?;
    theme.fonts.unset.unspecified(group, script)?;
    if script_fonts_cover(text, script, &theme.fonts.unset.group(group).script_fonts) { return None; }
    Some((group, script))
}

/// A replaced auxiliary design (the notes and handout masters) keeps a font slot unspecified only as `ThemeFonts::after_edit` allows, against the
/// master it replaces. A master the design did not have comes with its own theme, whose markers stay as given.
pub(crate) fn after_auxiliary_edit(previous: Option<&crate::model::AuxiliaryDesign>, design: &mut crate::model::AuxiliaryDesign) {
    let Some(previous) = previous else { return };
    for (master, before) in [(design.notes_master.as_mut(), previous.notes_master.as_ref()), (design.handout_master.as_mut(), previous.handout_master.as_ref())] {
        if let (Some(master), Some(before)) = (master, before) { master.theme.fonts.after_edit(&before.theme.fonts); }
    }
}

/// Brings the font markers of a deck's themes in line with the file the deck was opened from, so that a native save sees only what an edit
/// changed: the default theme, each master's own theme and the notes and handout masters' themes.
pub(crate) fn align_with_file(deck: &mut crate::model::Deck, file: &crate::model::Deck) {
    if let (Some(design), Some(file_design)) = (deck.design.as_mut(), file.design.as_ref()) {
        design.theme.fonts.align_with_file(&file_design.theme.fonts);
        for master in &mut design.masters {
            let (Some(theme), Some(file_master)) = (master.theme.as_mut(), file_design.masters.iter().find(|entry| entry.id == master.id)) else { continue };
            theme.fonts.align_with_file(&file_master.theme.as_ref().unwrap_or(&file_design.theme).fonts);
        }
    }
    if let (Some(auxiliary), Some(file_auxiliary)) = (deck.auxiliary_design.as_mut(), file.auxiliary_design.as_ref()) {
        for (master, file_master) in [(&mut auxiliary.notes_master, &file_auxiliary.notes_master), (&mut auxiliary.handout_master, &file_auxiliary.handout_master)] {
            if let (Some(master), Some(file_master)) = (master.as_mut(), file_master.as_ref()) { master.theme.fonts.align_with_file(&file_master.theme.fonts); }
        }
    }
}

/// The THEME_FONT_UNSET warning for the slots a text element depends on.
pub(crate) fn unset_message(slots: &std::collections::BTreeSet<(FontGroup, FontScript)>, theme: &Theme) -> String {
    let described = slots.iter().map(|(group, script)| format!("{} {}", group.name(), script.label())).collect::<Vec<_>>().join(", ");
    let drawn = slots.iter().map(|(group, script)| theme.fonts.family(*group, *script)).collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>().join(", ");
    format!("The template leaves its {described} theme font empty, so PowerPoint chooses a font for the viewer's language and installation, while AISlide draws text that uses it with {drawn}; line breaks and appearance can differ. Name the font with apply_theme or set_master_theme (an explicit value is always saved), or give the text its own font_family")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fonts(unset: &[(FontGroup, FontScript, UnsetFont)]) -> ThemeFonts {
        let mut fonts = ThemeFonts::new("Major", "Minor", "Yu Gothic", "Arial");
        for (group, script, state) in unset { fonts.unset.group_mut(*group).set(*script, Some(*state)); }
        fonts
    }

    fn entries(tags: &[&str]) -> BTreeMap<String, String> { tags.iter().map(|tag| ((*tag).to_owned(), "Font".to_owned())).collect() }

    #[test]
    fn an_edit_takes_a_slot_from_unspecified_to_written_and_never_back() {
        use super::{FontGroup::Minor, FontScript::EastAsian};
        let previous = fonts(&[(Minor, EastAsian, UnsetFont::Empty)]);
        let mut sent_back = previous.clone();
        sent_back.after_edit(&previous);
        assert_eq!(sent_back.unset.state(Minor, EastAsian), Some(UnsetFont::Empty), "sent back unchanged");
        let mut named = fonts(&[]);
        named.after_edit(&previous);
        assert_eq!(named.unset.state(Minor, EastAsian), Some(UnsetFont::Written), "entry left out");
        let mut renamed = previous.clone();
        renamed.east_asian = "Meiryo".into();
        renamed.after_edit(&previous);
        assert_eq!(renamed.unset.state(Minor, EastAsian), Some(UnsetFont::Written), "name changed under its marker");
        let mut again = fonts(&[(Minor, EastAsian, UnsetFont::Empty)]);
        again.after_edit(&named);
        assert_eq!(again.unset.state(Minor, EastAsian), Some(UnsetFont::Written), "a marker cannot unspecify a written slot");
        let mut bare = fonts(&[]);
        bare.after_edit(&named);
        assert_eq!(bare.unset.state(Minor, EastAsian), Some(UnsetFont::Written));
        let mut injected = fonts(&[(Minor, EastAsian, UnsetFont::Empty)]);
        injected.after_edit(&fonts(&[]));
        assert_eq!(injected.unset.state(Minor, EastAsian), None, "a marker on a specified slot means nothing");
        assert_eq!(named.unset.unspecified(Minor, EastAsian), None);
        // An explicit written marker names the slot even with the name AISlide showed, and applying an edit twice changes nothing more.
        let mut explicit = fonts(&[(Minor, EastAsian, UnsetFont::Written)]);
        explicit.after_edit(&previous);
        assert_eq!(explicit.unset.state(Minor, EastAsian), Some(UnsetFont::Written));
        let mut twice = named.clone();
        twice.after_edit(&previous);
        assert_eq!(twice.unset.state(Minor, EastAsian), Some(UnsetFont::Written));
    }

    #[test]
    fn alignment_with_the_file_ignores_what_a_build_without_markers_did_not_record() {
        use super::{FontGroup::Minor, FontScript::EastAsian};
        let file = fonts(&[(Minor, EastAsian, UnsetFont::Empty)]);
        let mut legacy = fonts(&[]);
        legacy.align_with_file(&file);
        assert_eq!(legacy.unset.state(Minor, EastAsian), Some(UnsetFont::Empty));
        let mut written = fonts(&[(Minor, EastAsian, UnsetFont::Written)]);
        written.align_with_file(&file);
        assert_eq!(written.unset.state(Minor, EastAsian), Some(UnsetFont::Written));
        let mut renamed = fonts(&[(Minor, EastAsian, UnsetFont::Empty)]);
        renamed.east_asian = "Meiryo".into();
        renamed.align_with_file(&file);
        assert_eq!(renamed.unset.state(Minor, EastAsian), None, "a changed name is written by name");
        let mut injected = fonts(&[(Minor, EastAsian, UnsetFont::Empty)]);
        injected.align_with_file(&fonts(&[]));
        assert_eq!(injected.unset.state(Minor, EastAsian), None, "the file specifies the slot");
    }

    #[test]
    fn complex_script_text_is_covered_only_by_a_font_for_its_own_script() {
        let (arabic, devanagari, mixed) = ("مرحبا", "नमस्ते", "مرحبا नमस्ते");
        assert!(script_fonts_cover(arabic, FontScript::ComplexScript, &entries(&["Arab"])));
        assert!(!script_fonts_cover(devanagari, FontScript::ComplexScript, &entries(&["Arab"])));
        assert!(script_fonts_cover(devanagari, FontScript::ComplexScript, &entries(&["Arab", "Deva"])));
        assert!(!script_fonts_cover(mixed, FontScript::ComplexScript, &entries(&["Arab"])));
        assert!(script_fonts_cover(mixed, FontScript::ComplexScript, &entries(&["Arab", "Deva"])));
        assert!(!script_fonts_cover(arabic, FontScript::ComplexScript, &entries(&["Jpan", "Hang"])), "East Asian script fonts do not cover complex-script text");
        assert!(!script_fonts_cover(arabic, FontScript::ComplexScript, &BTreeMap::new()));
    }

    #[test]
    fn east_asian_text_needs_an_entry_for_each_of_the_scripts_it_uses() {
        let (kana, hangul, mixed, han) = ("かな", "한글", "かな한글", "漢字");
        assert!(script_fonts_cover(kana, FontScript::EastAsian, &entries(&["Jpan"])));
        assert!(!script_fonts_cover(kana, FontScript::EastAsian, &entries(&["Hang"])));
        assert!(script_fonts_cover(hangul, FontScript::EastAsian, &entries(&["Hang"])));
        assert!(!script_fonts_cover(mixed, FontScript::EastAsian, &entries(&["Jpan"])));
        assert!(!script_fonts_cover(mixed, FontScript::EastAsian, &entries(&["Hang"])));
        assert!(script_fonts_cover(mixed, FontScript::EastAsian, &entries(&["Jpan", "Hang"])));
        assert!(script_fonts_cover(han, FontScript::EastAsian, &entries(&["Hans"])));
        assert!(!script_fonts_cover(han, FontScript::EastAsian, &entries(&["Hang"])));
    }
}
