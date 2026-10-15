use super::{Action, State, MAX_ENTRIES, hash, prepare, valid_hash, validate_state, verify_record};
use crate::{generation::CancellationToken, Error, Result};
use std::{fs::{self, File, OpenOptions}, io::{Read, Write}, path::{Path, PathBuf}};

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_SHARE_READ,
    FILE_SHARE_WRITE,
};

pub struct Store { root: PathBuf, _lock: File, _directories: Vec<(PathBuf, File)> }

#[derive(serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    State,
    Transition { expected_generation: u64, action: Action },
    Load { id: String, expected_generation: u64 },
}

pub fn execute(root: &Path, request: serde_json::Value, token: &CancellationToken) -> Result<serde_json::Value> {
    cancelled(token)?;
    crate::preflight::value(&request, &crate::limits::LARGE, Some(token))?;
    crate::preflight::serialized_bytes(&request, crate::limits::LARGE.request_bytes, "native recovery request")?;
    let request: Request = serde_json::from_value(request)?;
    let store = Store::open(root)?;
    let result = match request {
        Request::State => serde_json::to_value(store.state()?)?,
        Request::Load { id, expected_generation } => serde_json::to_value(store.load(&id, expected_generation)?)?,
        Request::Transition { expected_generation, action } => {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|Error::Invalid("recovery clock".into()))?.as_millis();
            serde_json::to_value(store.apply(expected_generation, action, u64::try_from(now).map_err(|_|Error::Limit("recovery clock".into()))?, token)?)?
        }
    };
    cancelled(token)?;
    Ok(result)
}

fn cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() { return Err(Error::Generation("recovery cancelled".into())); }
    Ok(())
}

fn regular(file: &File, directory: bool) -> Result<()> {
    let metadata = file.metadata()?;
    #[cfg(windows)]
    {
        let information = winapi_util::file::information(file)?;
        if information.file_attributes() & u64::from(FILE_ATTRIBUTE_REPARSE_POINT.0) != 0 {
            return Err(Error::Unsupported("recovery reparse points are not allowed".into()));
        }
        if !directory && information.number_of_links() != 1 {
            return Err(Error::Unsupported("recovery files must have one private link".into()));
        }
    }
    if metadata.file_type().is_symlink() || (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        return Err(Error::Unsupported("recovery path is not an ordinary private slot".into()));
    }
    Ok(())
}

#[cfg(windows)]
fn pin_directory(path: &Path, share_writes: bool) -> Result<File> {
    let file = OpenOptions::new().read(true)
        .access_mode((FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES).0)
        .share_mode(if share_writes { (FILE_SHARE_READ | FILE_SHARE_WRITE).0 } else { FILE_SHARE_READ.0 })
        .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).0).open(path)?;
    regular(&file, true)?;
    Ok(file)
}

#[cfg(windows)]
fn same_directory(expected: &File, actual: &File) -> Result<()> {
    let expected = winapi_util::file::information(expected)?;
    let actual = winapi_util::file::information(actual)?;
    if (expected.volume_serial_number(), expected.file_index()) != (actual.volume_serial_number(), actual.file_index()) {
        return Err(Error::Conflict("recovery directory identity changed".into()));
    }
    Ok(())
}

#[cfg(windows)]
fn pin_root(root: &Path) -> Result<(PathBuf, Vec<(PathBuf, File)>)> {
    use std::path::{Component, Prefix};
    let mut components = root.components();
    let prefix = match components.next() {
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) => prefix,
        _ => return Err(Error::Unsupported("recovery requires an absolute local Windows directory".into())),
    };
    if components.next() != Some(Component::RootDir) {
        return Err(Error::Unsupported("recovery requires an absolute local Windows directory".into()));
    }
    let names: Vec<_> = components.map(|component| match component {
        Component::Normal(name) if !name.to_string_lossy().ends_with(['.', ' '])
            && !name.to_string_lossy().chars().any(|character| character.is_control() || "<>:\"|?*".contains(character)) => Ok(name),
        _ => Err(Error::Unsupported("recovery directory contains an ambiguous component".into())),
    }).collect::<Result<_>>()?;
    if names.is_empty() { return Err(Error::Unsupported("recovery cannot use a volume root".into())); }
    let mut path = PathBuf::from(prefix.as_os_str());
    path.push("\\");
    let volume = pin_directory(&path, false)?;
    let mut directories = vec![(path.clone(), volume)];
    for name in names {
        path.push(name);
        let directory = match pin_directory(&path, false) {
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::create_dir(&path) {
                    Ok(()) => {},
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {},
                    Err(error) => return Err(error.into()),
                }
                pin_directory(&path, false)?
            }
            result => result?,
        };
        directories.push((path.clone(), directory));
    }
    let canonical = fs::canonicalize(&path)?;
    let confirmed = pin_directory(&canonical, false)?;
    same_directory(&directories.last().ok_or_else(||Error::Invalid("recovery directory guard".into()))?.1, &confirmed)?;
    directories.push((canonical.clone(), confirmed));
    Ok((canonical, directories))
}

#[cfg(not(windows))]
fn pin_root(_root: &Path) -> Result<(PathBuf, Vec<(PathBuf, File)>)> {
    Err(Error::Unsupported("confined native recovery storage is currently Windows-only".into()))
}

fn slot_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0).share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE).0);
    options
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let file = slot_options().open(path)?;
    regular(&file, false)?;
    if file.metadata()?.len() > maximum as u64 { return Err(Error::Limit("recovery file byte limit".into())); }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > maximum { return Err(Error::Limit("recovery file byte limit".into())); }
    Ok(bytes)
}

fn read_optional(path: &Path, maximum: usize) -> Result<Option<Vec<u8>>> {
    match read_bounded(path, maximum) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        let (root, mut directories) = pin_root(root)?;
        #[cfg(all(test, windows))]
        tests::after_directory_check();
        let path = root.join("store.lock");
        let lock = slot_options().create(true).truncate(false).write(true).open(path)?;
        regular(&lock, false)?;
        lock.try_lock().map_err(|_|Error::Conflict("recovery store busy in another window; retry".into()))?;
        #[cfg(windows)]
        for (path, held) in &mut directories {
            let shared = pin_directory(path, true)?;
            same_directory(held, &shared)?;
            *held = shared;
        }
        Ok(Self { root, _lock: lock, _directories: directories })
    }

    pub fn state(&self) -> Result<State> {
        let path = self.root.join("index.json");
        let Some(bytes) = read_optional(&path, 64 * 1024)? else { return Ok(State::default()); };
        let state: State = serde_json::from_slice(&bytes)?;
        validate_state(&state)?;
        Ok(state)
    }

    pub fn load(&self, id: &str, expected_generation: u64) -> Result<crate::document::SessionRecovery> {
        let state = self.state()?;
        if state.generation != expected_generation { return Err(Error::Conflict("recovery selection changed; refresh".into())); }
        let entry = state.entries.iter().find(|entry|entry.id == id).ok_or_else(||Error::Conflict("recovery selection no longer exists".into()))?;
        let bytes = read_bounded(&self.slot(&entry.integrity)?, crate::document::MAX_RECOVERY_BYTES)?;
        let payload = std::str::from_utf8(&bytes).map_err(|_|Error::Invalid("recovery payload UTF-8".into()))?;
        verify_record(entry, payload)
    }

    fn slot(&self, integrity: &str) -> Result<PathBuf> {
        if !valid_hash(integrity) { return Err(Error::Invalid("recovery slot identity".into())); }
        Ok(self.root.join(format!("{integrity}.json")))
    }

    fn cleanup(&self, state: &State) -> Result<()> {
        if !state.enabled { return Ok(()); }
        let mut slots = Vec::new();
        for item in fs::read_dir(&self.root)?.take(129) {
            let item = item?;
            let name = item.file_name().to_string_lossy().into_owned();
            if let Some(integrity) = name.strip_suffix(".json").filter(|value|valid_hash(value)) {
                if !state.entries.iter().any(|entry|entry.integrity == integrity) { slots.push((item.path(), integrity.to_string())); }
            }
        }
        if slots.len() > MAX_ENTRIES * 4 { return Err(Error::Limit("too many orphan recovery slots; manual review required".into())); }
        for (path, integrity) in slots {
            let bytes = read_bounded(&path, crate::document::MAX_RECOVERY_BYTES)?;
            if hash(&bytes) == integrity { fs::remove_file(path)?; }
        }
        Ok(())
    }

    pub fn apply(&self, expected_generation: u64, action: Action, now_ms: u64, token: &CancellationToken) -> Result<State> {
        cancelled(token)?;
        let previous = self.state()?;
        let result = prepare(previous.clone(), expected_generation, action, now_ms)?;
        cancelled(token)?;
        self.cleanup(&previous)?;
        cancelled(token)?;
        if result.state.generation == previous.generation { self.cleanup(&result.state)?; return Ok(result.state); }
        let mut new_slot = None;
        if let Some(record) = result.write {
            let slot = self.slot(&record.entry.integrity)?;
            if let Some(bytes) = read_optional(&slot, crate::document::MAX_RECOVERY_BYTES)? {
                if bytes != record.payload.as_bytes() { return Err(Error::Conflict("recovery slot changed; good checkpoint retained".into())); }
            } else {
                let mut temporary = tempfile::Builder::new().prefix("checkpoint-").suffix(".tmp").tempfile_in(&self.root)?;
                regular(temporary.as_file(), false)?;
                temporary.write_all(record.payload.as_bytes())?;
                temporary.as_file().sync_all()?;
                cancelled(token)?;
                temporary.persist_noclobber(&slot).map_err(|error|Error::Io(error.error))?;
                new_slot = Some(slot);
            }
        }
        let commit = || -> Result<()> {
            let mut temporary = tempfile::Builder::new().prefix("index-").suffix(".tmp").tempfile_in(&self.root)?;
            regular(temporary.as_file(), false)?;
            serde_json::to_writer(&mut temporary, &result.state)?;
            temporary.as_file().sync_all()?;
            cancelled(token)?;
            let index = self.root.join("index.json");
            match slot_options().open(&index) {
                Ok(file) => regular(&file, false)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
                Err(error) => return Err(error.into()),
            }
            temporary.persist(index).map_err(|error|Error::Io(error.error))?;
            Ok(())
        };
        if let Err(error) = commit() {
            if let Some(path) = new_slot { let _ = fs::remove_file(path); }
            return Err(error);
        }
        for integrity in result.deleted {
            let path = self.slot(&integrity)?;
            if read_bounded(&path, crate::document::MAX_RECOVERY_BYTES).is_ok_and(|bytes|hash(&bytes) == integrity) {
                fs::remove_file(path)?;
            }
        }
        self.cleanup(&result.state)?;
        Ok(result.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use serde_json::json;

    #[cfg(windows)]
    std::thread_local! {
        static AFTER_DIRECTORY_CHECK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = std::cell::RefCell::new(None);
    }

    #[cfg(windows)]
    pub(super) fn after_directory_check() {
        if let Some(hook) = AFTER_DIRECTORY_CHECK.with(|hook| hook.borrow_mut().take()) { hook(); }
    }

    #[cfg(windows)]
    fn junction(link: &Path, target: &Path) {
        let output = std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"]).arg(link).arg(target).output().unwrap();
        assert!(output.status.success(), "junction fixture: {}", String::from_utf8_lossy(&output.stderr));
    }

    #[cfg(windows)]
    struct TestJunction(PathBuf);

    #[cfg(windows)]
    impl Drop for TestJunction {
        fn drop(&mut self) { fs::remove_dir(&self.0).unwrap(); }
    }

    #[cfg(windows)]
    #[test]
    fn root_swap_after_validation_cannot_redirect_index_write() {
        directory_swap_after_validation(false);
    }

    #[cfg(windows)]
    #[test]
    fn ancestor_swap_after_validation_cannot_redirect_index_write() {
        directory_swap_after_validation(true);
    }

    #[cfg(windows)]
    fn directory_swap_after_validation(ancestor: bool) {
        let owned = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(owned.path().join("test-owned"), b"AISlide storage race fixture").unwrap();
        fs::write(outside.path().join("test-owned"), b"AISlide storage outside fixture").unwrap();
        let parent = owned.path().join("parent");
        let root = parent.join("recovery-v2");
        let attacked = if ancestor { parent } else { root.clone() };
        let outside_root = if ancestor { outside.path().join("recovery-v2") } else { outside.path().to_path_buf() };
        fs::create_dir_all(&outside_root).unwrap();
        let moved = owned.path().join("moved");
        let sentinel = serde_json::to_vec(&State::default()).unwrap();
        let outside_index = outside_root.join("index.json");
        fs::write(&outside_index, &sentinel).unwrap();
        let (checked, ready) = std::sync::mpsc::sync_channel(0);
        let (attempted, resume) = std::sync::mpsc::sync_channel(0);
        AFTER_DIRECTORY_CHECK.with(|hook| *hook.borrow_mut() = Some(Box::new(move || {
            checked.send(()).unwrap();
            resume.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        })));
        let attacker = {
            let attacked = attacked.clone();
            let target = outside.path().to_path_buf();
            std::thread::spawn(move || {
                ready.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
                assert!(OpenOptions::new().write(true).custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0).open(&attacked).is_err());
                let rename = fs::rename(&attacked, moved);
                if rename.is_ok() { junction(&attacked, &target); }
                attempted.send(()).unwrap();
                rename
            })
        };
        let opened = Store::open(&root);
        let rename = attacker.join().unwrap();
        let link = rename.is_ok().then(|| TestJunction(attacked.clone()));
        let applied = opened.and_then(|store| store.apply(0, Action::Configure { enabled: true }, 1000, &CancellationToken::new()));
        drop(link);
        assert_eq!(fs::read(&outside_index).unwrap(), sentinel, "outside index was overwritten through the swapped root");
        assert_eq!(fs::read_dir(&outside_root).unwrap().count(), if ancestor { 1 } else { 2 });
        assert!(rename.is_err() || applied.is_err(), "root replacement must be blocked or the write rejected");
        if rename.is_err() {
            assert!(applied.unwrap().enabled);
            assert!(Store::open(&root).unwrap().state().unwrap().enabled);
            fs::rename(&attacked, owned.path().join("released")).unwrap();
            junction(&attacked, outside.path());
            drop(TestJunction(attacked));
        }
    }

    #[cfg(windows)]
    #[test]
    fn reparse_ancestor_is_rejected_before_creating_missing_descendants() {
        let owned = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let sentinel = outside.path().join("original.pptx");
        fs::write(&sentinel, b"untouched original").unwrap();
        let alias = owned.path().join("alias");
        junction(&alias, outside.path());
        let link = TestJunction(alias.clone());
        assert!(Store::open(&alias.join("missing").join("recovery-v2")).is_err());
        assert_eq!(fs::read(&sentinel).unwrap(), b"untouched original");
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 1);
        drop(link);
        let safe = owned.path().join("new-parent").join("recovery-v2");
        assert!(!Store::open(&safe).unwrap().state().unwrap().enabled);
        assert!(Store::open(&owned.path().join("never-create").join("..").join("recovery-v2")).is_err());
        assert!(!owned.path().join("never-create").exists());
    }

    #[cfg(windows)]
    #[test]
    fn opened_slots_reject_links_and_enforce_byte_bounds_without_changing_originals() {
        let owned = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = owned.path().join("recovery-v2");
        fs::create_dir(&root).unwrap();
        let sentinel = serde_json::to_vec(&State::default()).unwrap();
        let original = outside.path().join("original.pptx");
        fs::write(&original, &sentinel).unwrap();
        let lock = root.join("store.lock");
        fs::hard_link(&original, &lock).unwrap();
        assert!(matches!(Store::open(&root), Err(Error::Unsupported(_))));
        fs::remove_file(&lock).unwrap();
        let store = Store::open(&root).unwrap();
        assert!(fs::remove_file(&lock).is_err());
        let index = root.join("index.json");
        fs::hard_link(&original, &index).unwrap();
        assert!(matches!(store.state(), Err(Error::Unsupported(_))));
        fs::remove_file(&index).unwrap();
        let mut bounded = sentinel.clone();
        bounded.resize(64 * 1024, b' ');
        fs::write(&index, &bounded).unwrap();
        assert!(!store.state().unwrap().enabled);
        bounded.push(b' ');
        fs::write(&index, &bounded).unwrap();
        assert!(matches!(store.state(), Err(Error::Limit(_))));
        fs::remove_file(&index).unwrap();
        junction(&index, outside.path());
        let link = TestJunction(index.clone());
        assert!(store.state().is_err());
        drop(link);
        let token = CancellationToken::new();
        let first = store.apply(0, Action::Configure { enabled: true }, 1000, &token).unwrap();
        let second = store.apply(first.generation, Action::Configure { enabled: false }, 1001, &token).unwrap();
        assert_eq!(store.state().unwrap().generation, second.generation);
        assert_eq!(fs::read(original).unwrap(), sentinel);
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 1);
    }

    #[cfg(not(windows))]
    #[test]
    fn native_storage_fails_closed_without_a_confined_platform_implementation() {
        let owned = tempfile::tempdir().unwrap();
        let root = owned.path().join("recovery-v2");
        assert!(matches!(Store::open(&root), Err(Error::Unsupported(_))));
        assert!(!root.exists());
    }

    #[cfg(windows)]
    #[test]
    fn private_store_restart_cas_cancel_and_corruption_preserve_good_checkpoint() {
        let directory = tempfile::tempdir().unwrap();
        let original = directory.path().join("original.pptx");
        fs::write(&original, b"untouched original").unwrap();
        let root = directory.path().join("recovery-v2");
        let token = CancellationToken::new();
        let document = crate::execute_request(json!({"op":"create_presentation","id":"durable","title":"Synthetic"})).unwrap();
        let change = |document: &serde_json::Value, title: &str| crate::execute_request(json!({"op":"transaction","document":document,"transaction":{
            "expected_revision":document["revision"],"expected_hash":document["hash"],"operations":[{"op":"replace","path":"/deck/title","value":title}]
        }})).unwrap();
        let first = change(&document, "First");
        let second = change(&first["document"], "Second");
        let undone = crate::execute_request(json!({"op":"undo_transaction","document":second["document"],"expected_revision":2,"receipt":second["receipt"]})).unwrap();
        let save = || serde_json::from_value(json!({"op":"save","filename":"C:\\private\\file.pptx","envelope":{
            "format":"aislide.session","version":1,"capacity_profile":"large","document":undone["document"],"past":[first["receipt"]],"future":[undone["receipt"]],"history_boundary":null
        }})).unwrap();
        let state = {
            let store = Store::open(&root).unwrap();
            assert!(Store::open(&root).is_err());
            let enabled = store.apply(0, Action::Configure { enabled: true }, 1000, &token).unwrap();
            store.apply(enabled.generation, save(), 1001, &token).unwrap()
        };
        let restarted = Store::open(&root).unwrap();
        let loaded = restarted.load("durable", state.generation).unwrap();
        assert_eq!(loaded.document.id, "durable");
        assert_eq!(crate::document::undo(&loaded.document, loaded.document.revision, loaded.past[0].clone()).unwrap().document.deck.title, "Synthetic");
        assert_eq!(crate::document::undo(&loaded.document, loaded.document.revision, loaded.future[0].clone()).unwrap().document.deck.title, "Second");
        assert_eq!(state.entries[0].filename, "file.pptx");
        assert!(restarted.apply(1, save(), 1002, &token).is_err());
        token.cancel();
        assert!(restarted.apply(state.generation, Action::Clear, 1003, &token).is_err());
        assert_eq!(restarted.state().unwrap().generation, state.generation);
        fs::write(restarted.slot(&state.entries[0].integrity).unwrap(), b"corrupt").unwrap();
        assert!(restarted.load("durable", state.generation).is_err());
        assert_eq!(fs::read(original).unwrap(), b"untouched original");
    }
}