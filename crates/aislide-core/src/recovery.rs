use crate::{document::{SessionRecovery, MAX_RECOVERY_BYTES, verify_session_recovery}, Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const MAX_ENTRIES: usize = 5;
pub const MAX_TOTAL_BYTES: usize = 100 * crate::limits::MIB;
pub const RETENTION_MS: u64 = 7 * 24 * 60 * 60 * 1000;
const MAX_GENERATION: u64 = 9_007_199_254_740_991;

pub mod storage;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub filename: String,
    pub hash: String,
    pub revision: u64,
    pub saved_at: u64,
    pub byte_length: usize,
    pub integrity: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub version: u32,
    pub enabled: bool,
    pub generation: u64,
    pub consent_epoch: u64,
    pub entries: Vec<Entry>,
}

impl Default for State {
    fn default() -> Self { Self { version: 2, enabled: false, generation: 0, consent_epoch: 0, entries: Vec::new() } }
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Open,
    Configure { enabled: bool },
    Save { envelope: SessionRecovery, filename: String },
    Remove { id: String },
    Clear,
}

#[derive(Serialize)]
pub struct Record { pub entry: Entry, pub payload: String }

#[derive(Serialize)]
pub struct Transition {
    pub state: State,
    pub write: Option<Record>,
    pub deleted: Vec<String>,
}

pub fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

pub fn valid_hash(value: &str) -> bool { value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) }

pub fn validate_state(state: &State) -> Result<()> {
    if state.version != 2 { return Err(Error::Unsupported("recovery policy v2 consent is required; legacy copies remain unchanged".into())); }
    if state.generation > MAX_GENERATION || state.consent_epoch > state.generation || state.entries.len() > MAX_ENTRIES {
        return Err(Error::Limit("recovery generation or entry count".into()));
    }
    let mut ids = BTreeSet::new();
    let mut total = 0usize;
    for entry in &state.entries {
        crate::model::valid_text(&entry.id, 80)?;
        if entry.id.is_empty() || !ids.insert(&entry.id) || !valid_hash(&entry.hash) || !valid_hash(&entry.integrity)
            || entry.revision > MAX_GENERATION || entry.saved_at > MAX_GENERATION || entry.byte_length > MAX_RECOVERY_BYTES || entry.byte_length == 0 {
            return Err(Error::Invalid("recovery entry metadata".into()));
        }
        if basename(&entry.filename)? != entry.filename { return Err(Error::Invalid("recovery filename must be a basename".into())); }
        total = total.checked_add(entry.byte_length).ok_or_else(||Error::Limit("recovery bytes overflow".into()))?;
    }
    if total > MAX_TOTAL_BYTES { return Err(Error::Limit("recovery aggregate exceeds 100 MiB".into())); }
    Ok(())
}

fn basename(value: &str) -> Result<String> {
    let name: String = value.rsplit(['/', '\\']).next().unwrap_or("").chars()
        .filter(|character| !character.is_control() && !"<>:\"|?*".contains(*character) && !('\u{202a}'..='\u{202e}').contains(character) && !('\u{2066}'..='\u{2069}').contains(character)).take(180).collect();
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." { return Ok("Untitled.pptx".into()); }
    Ok(name.into())
}

pub fn verify_record(entry: &Entry, payload: &str) -> Result<SessionRecovery> {
    if payload.len() != entry.byte_length || hash(payload.as_bytes()) != entry.integrity {
        return Err(Error::Conflict("recovery payload integrity mismatch".into()));
    }
    if payload.len() > MAX_RECOVERY_BYTES { return Err(Error::Limit("recovery entry exceeds 48 MiB".into())); }
    let envelope = verify_session_recovery(serde_json::from_value(crate::preflight::json(payload.as_bytes())?)?)?;
    if envelope.document.id != entry.id || envelope.document.hash != entry.hash || envelope.document.revision != entry.revision {
        return Err(Error::Conflict("recovery document identity mismatch".into()));
    }
    Ok(envelope)
}

pub fn prepare(mut state: State, expected_generation: u64, action: Action, now_ms: u64) -> Result<Transition> {
    validate_state(&state)?;
    if state.generation != expected_generation { return Err(Error::Conflict("recovery changed in another window; refresh before saving or deleting".into())); }
    if now_ms > MAX_GENERATION { return Err(Error::Invalid("recovery clock".into())); }
    let before: Vec<_> = state.entries.iter().map(|entry|entry.integrity.clone()).collect();
    let mut write = None;
    let mut changed = true;
    match action {
        Action::Open => {
            if state.enabled { state.entries.retain(|entry| now_ms.saturating_sub(entry.saved_at) < RETENTION_MS); }
            changed = state.entries.len() != before.len();
        }
        Action::Configure { enabled } => { state.enabled = enabled; state.consent_epoch = state.consent_epoch.checked_add(1).ok_or_else(||Error::Limit("recovery consent epoch".into()))?; }
        Action::Save { envelope, filename } => {
            if !state.enabled { return Err(Error::Conflict("recovery is disabled; explicit v2 consent required".into())); }
            let envelope = verify_session_recovery(envelope)?;
            if state.entries.iter().any(|entry| entry.id == envelope.document.id && entry.revision > envelope.document.revision) {
                return Err(Error::Conflict("an older session cannot overwrite a newer recovery revision".into()));
            }
            let payload = serde_json::to_string(&envelope)?;
            if payload.len() > MAX_RECOVERY_BYTES { return Err(Error::Limit("recovery entry exceeds 48 MiB".into())); }
            let saved_at = state.entries.iter().map(|entry|entry.saved_at.saturating_add(1)).max().unwrap_or(0).max(now_ms);
            let entry = Entry { id: envelope.document.id, hash: envelope.document.hash, revision: envelope.document.revision,
                filename: basename(&filename)?, saved_at, byte_length: payload.len(), integrity: hash(payload.as_bytes()) };
            state.entries.retain(|candidate| candidate.id != entry.id);
            state.entries.sort_by_key(|candidate|candidate.saved_at);
            while state.entries.len() >= MAX_ENTRIES || state.entries.iter().map(|candidate|candidate.byte_length).sum::<usize>() + entry.byte_length > MAX_TOTAL_BYTES {
                if state.entries.is_empty() { return Err(Error::Limit("recovery aggregate exceeds 100 MiB".into())); }
                state.entries.remove(0);
            }
            state.entries.push(entry.clone());
            write = Some(Record { entry, payload });
        }
        Action::Remove { id } => {
            if !state.entries.iter().any(|entry|entry.id == id) { return Err(Error::Conflict("recovery selection no longer exists".into())); }
            state.entries.retain(|entry|entry.id != id);
        }
        Action::Clear => state.entries.clear(),
    }
    if changed { state.generation = state.generation.checked_add(1).filter(|value|*value<=MAX_GENERATION).ok_or_else(||Error::Limit("recovery generation exhausted".into()))?; }
    validate_state(&state)?;
    let deleted = before.into_iter().filter(|integrity| !state.entries.iter().any(|entry|&entry.integrity == integrity)).collect();
    Ok(Transition { state, write, deleted })
}