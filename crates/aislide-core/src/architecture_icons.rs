use crate::{Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs::{self, File, Metadata}, io::Read, path::{Component, Path, PathBuf}, sync::OnceLock};

const CATALOG_BYTES: &[u8] = include_bytes!("architecture-icons.json");
const MAX_ICONS: usize = 60;
const MAX_PNG_BYTES: usize = 1024 * 1024;
const MAX_BATCH_BYTES: usize = 4 * 1024 * 1024;
const MAX_CONSENT_BYTES: usize = 8192;
const SETUP: &str = "Run node tools/cloud-icons-setup.mjs --accept-vendor-terms after reviewing the vendor conditions in docs/authoring/cloud-icons.md. This API never downloads or installs artwork.";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureIconArchive {
    pub id: String,
    pub url: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureIconProvider {
    pub id: String,
    pub name: String,
    pub terms_url: String,
    pub notice: String,
    pub archives: Vec<ArchitectureIconArchive>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureIconSource {
    pub archive_id: String,
    pub entry: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureIconEntry {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub kind: String,
    pub categories: Vec<String>,
    pub aliases: Vec<String>,
    pub source: ArchitectureIconSource,
    pub png_sha256: String,
    pub png_bytes: usize,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize)]
pub struct ArchitectureIconCatalog {
    pub version: u32,
    pub release: String,
    pub configured: bool,
    pub message: String,
    pub providers: Vec<ArchitectureIconProvider>,
    pub icons: Vec<ArchitectureIconEntry>,
}

#[derive(Debug, Serialize)]
pub struct ArchitectureIconAsset {
    pub id: String,
    pub base64: String,
    pub mime_type: &'static str,
    pub alt: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize)]
pub struct ArchitectureIconAssets { pub icons: Vec<ArchitectureIconAsset> }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    release: String,
    providers: Vec<ArchitectureIconProvider>,
    icons: Vec<ArchitectureIconEntry>,
}

struct Compiled { manifest: Manifest, sha256: String }
static COMPILED: OnceLock<std::result::Result<Compiled, String>> = OnceLock::new();

fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn hash_valid(value: &str) -> bool { value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) }
fn text_valid(value: &str, limit: usize) -> bool { !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control) }
fn logical_path(value: &str) -> bool {
    text_valid(value, 1024) && !value.contains(['\\', ':']) && value.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

fn validate_manifest(manifest: &Manifest) -> Result<()> {
    let invalid = || Error::Invalid("compiled architecture icon catalog invariants".into());
    if manifest.version != 1 || manifest.release != "2026-09-20" || manifest.providers.len() != 3 || manifest.icons.len() != 1498 { return Err(invalid()); }
    let mut providers = HashSet::new();
    let mut archives = HashSet::new();
    for provider in &manifest.providers {
        if !["azure", "aws", "gcp"].contains(&provider.id.as_str()) || !providers.insert(provider.id.as_str())
            || !text_valid(&provider.name, 256) || !text_valid(&provider.notice, 4096) || !provider.terms_url.starts_with("https://") || provider.archives.is_empty() { return Err(invalid()); }
        for archive in &provider.archives {
            if !logical_path(&archive.id) || archive.id.contains('/') || !archives.insert((provider.id.as_str(), archive.id.as_str()))
                || !archive.url.starts_with("https://") || !hash_valid(&archive.sha256) || archive.bytes == 0 { return Err(invalid()); }
        }
    }
    let mut ids = HashSet::new();
    for icon in &manifest.icons {
        if !logical_path(&icon.id) || icon.id.len() > 256 || !ids.insert(icon.id.as_str()) || !providers.contains(icon.provider.as_str())
            || !icon.id.starts_with(&format!("{}/", icon.provider)) || !text_valid(&icon.name, 500) || !text_valid(&icon.kind, 80)
            || icon.categories.iter().chain(&icon.aliases).any(|value| !text_valid(value, 512))
            || !archives.contains(&(icon.provider.as_str(), icon.source.archive_id.as_str())) || !logical_path(&icon.source.entry)
            || !hash_valid(&icon.source.sha256) || !hash_valid(&icon.png_sha256) || !(1..=MAX_PNG_BYTES).contains(&icon.png_bytes)
            || !(1..=512).contains(&icon.width) || !(1..=512).contains(&icon.height) { return Err(invalid()); }
    }
    Ok(())
}

fn compiled() -> Result<&'static Compiled> {
    COMPILED.get_or_init(|| {
        let manifest: Manifest = serde_json::from_slice(CATALOG_BYTES).map_err(|_| "invalid compiled architecture icon metadata".to_string())?;
        validate_manifest(&manifest).map_err(|error| error.to_string())?;
        Ok(Compiled { manifest, sha256: sha256(CATALOG_BYTES) })
    }).as_ref().map_err(|message| Error::Invalid(message.clone()))
}

fn local_path(path: &Path) -> Result<()> {
    let invalid = || Error::Invalid("architecture icon root must be an absolute local path without traversal, network or device prefixes".into());
    let text = path.to_str().ok_or_else(invalid)?;
    if !path.is_absolute() || text.len() > 4096 || text.starts_with(['\\']) || text.starts_with("//") || text.contains("://")
        || text.chars().any(char::is_control) || text.split(['/', '\\']).any(|part| part == "." || part == "..") { return Err(invalid()); }
    for component in path.components() {
        match component {
            Component::Prefix(prefix) if matches!(prefix.kind(), std::path::Prefix::Disk(_)) => {},
            Component::RootDir => {},
            Component::Normal(part) if part.to_str().is_some_and(|part| !part.contains([':', '\\']) && !part.ends_with(['.', ' '])) => {},
            _ => return Err(invalid()),
        }
    }
    Ok(())
}

fn host_root(release: &str) -> Result<PathBuf> {
    let root = if let Some(root) = std::env::var_os("AISLIDE_ICON_PACK_ROOT") { PathBuf::from(root) } else {
        #[cfg(target_os = "windows")]
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        #[cfg(not(target_os = "windows"))]
        let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
        base.ok_or_else(|| Error::Unsupported("architecture icon host data directory is not configured".into()))?.join("AISlide/icon-packs").join(release)
    };
    local_path(&root)?;
    Ok(root)
}

fn is_link(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    { metadata.file_type().is_symlink() }
}

fn checked_metadata(path: &Path, file: bool) -> Result<Metadata> {
    #[cfg(windows)]
    let classify = aislide_platform::is_local_drive;
    #[cfg(not(windows))]
    let classify = |_: u8| true;
    checked_metadata_with(path, file, classify, |path| fs::symlink_metadata(path))
}

fn checked_metadata_with(
    path: &Path,
    file: bool,
    classify: impl FnOnce(u8) -> bool,
    mut inspect: impl FnMut(&Path) -> std::io::Result<Metadata>,
) -> Result<Metadata> {
    local_path(path)?;
    #[cfg(windows)]
    {
        let local = match path.components().next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                std::path::Prefix::Disk(letter) => classify(letter),
                _ => false,
            },
            _ => false,
        };
        if !local { return Err(Error::Unsupported("architecture icon pack requires a known local Windows drive".into())); }
    }
    #[cfg(not(windows))]
    let _ = classify;
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_)) { continue; }
        let metadata = inspect(&current).map_err(|_| Error::Unsupported("architecture icon pack file or directory is missing or unreadable".into()))?;
        if is_link(&metadata) { return Err(Error::Invalid("architecture icon pack symlinks and reparse points are not allowed".into())); }
        let regular = if current == path && file { metadata.is_file() } else { metadata.is_dir() };
        if !regular { return Err(Error::Invalid("architecture icon pack requires regular files and directories".into())); }
    }
    inspect(path).map_err(|_| Error::Unsupported("architecture icon pack metadata is unreadable".into()))
}

fn read_bounded(path: &Path, maximum: usize, exact: Option<usize>) -> Result<Vec<u8>> {
    let check = |metadata: &Metadata| -> Result<()> {
        if !metadata.is_file() || is_link(metadata) { return Err(Error::Invalid("architecture icon pack requires a regular non-link file".into())); }
        if metadata.len() > maximum as u64 || exact.is_some_and(|size| metadata.len() != size as u64) { return Err(Error::Limit("architecture icon pack file size mismatch or limit".into())); }
        Ok(())
    };
    check(&checked_metadata(path, true)?)?;
    let file = File::open(path).map_err(|_| Error::Unsupported("architecture icon pack file cannot be opened".into()))?;
    check(&file.metadata().map_err(|_| Error::Unsupported("architecture icon pack open-file metadata is unreadable".into()))?)?;
    let mut bytes = Vec::new();
    file.take((maximum + 1) as u64).read_to_end(&mut bytes).map_err(|_| Error::Unsupported("architecture icon pack file read failed".into()))?;
    if bytes.len() > maximum || exact.is_some_and(|size| bytes.len() != size) { return Err(Error::Limit("architecture icon pack file size mismatch or limit".into())); }
    check(&checked_metadata(path, true)?)?;
    Ok(bytes)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Consent { version: u32, catalog_sha256: String, accepted_vendor_terms: bool }

fn check_consent(root: &Path, catalog_sha256: &str) -> Result<()> {
    let bytes = read_bounded(&root.join("consent.json"), MAX_CONSENT_BYTES, None)?;
    let consent: Consent = serde_json::from_slice(&bytes).map_err(|_| Error::Invalid("architecture icon consent format is invalid".into()))?;
    if consent.version != 1 || consent.catalog_sha256 != catalog_sha256 || !consent.accepted_vendor_terms {
        return Err(Error::Unsupported("architecture icon consent version, catalog hash or vendor acknowledgment does not match".into()));
    }
    Ok(())
}

fn catalog_at(compiled: &Compiled, root: Result<PathBuf>) -> ArchitectureIconCatalog {
    let ready = root.and_then(|root| check_consent(&root, &compiled.sha256));
    ArchitectureIconCatalog {
        version: compiled.manifest.version, release: compiled.manifest.release.clone(), configured: ready.is_ok(),
        message: match ready {
            Ok(()) => "Local vendor acknowledgment matches this catalog. Individual images are verified only when requested; this is not a complete-pack integrity check or a grant of redistribution rights.".into(),
            Err(error) => format!("{error}. {SETUP}"),
        },
        providers: compiled.manifest.providers.clone(), icons: compiled.manifest.icons.clone(),
    }
}

pub fn catalog() -> Result<ArchitectureIconCatalog> {
    let compiled = compiled()?;
    Ok(catalog_at(compiled, host_root(&compiled.manifest.release)))
}

fn selected<'catalog>(manifest: &'catalog Manifest, ids: &[String]) -> Result<Vec<&'catalog ArchitectureIconEntry>> {
    if !(1..=MAX_ICONS).contains(&ids.len()) { return Err(Error::Limit("architecture icon assets requires 1..60 IDs".into())); }
    let mut seen = HashSet::new();
    let mut icons = Vec::with_capacity(ids.len());
    for id in ids {
        if id.len() > 256 { return Err(Error::Invalid("architecture icon ID exceeds 256 bytes".into())); }
        if !seen.insert(id) { return Err(Error::Invalid("duplicate architecture icon ID".into())); }
        icons.push(manifest.icons.iter().find(|icon| &icon.id == id).ok_or_else(|| Error::Invalid("unknown architecture icon ID".into()))?);
    }
    let total = icons.iter().try_fold(0usize, |total, icon| total.checked_add(icon.png_bytes)).ok_or_else(|| Error::Limit("architecture icon batch size overflow".into()))?;
    if total > MAX_BATCH_BYTES { return Err(Error::Limit("architecture icon batch exceeds 4 MiB raw PNG budget; request fewer IDs".into())); }
    Ok(icons)
}

fn read_icon(root: &Path, icon: &ArchitectureIconEntry) -> Result<ArchitectureIconAsset> {
    let bytes = read_bounded(&root.join(&icon.provider).join(format!("{}.png", icon.png_sha256)), icon.png_bytes, Some(icon.png_bytes))?;
    if sha256(&bytes) != icon.png_sha256 { return Err(Error::Invalid("architecture icon PNG SHA-256 mismatch".into())); }
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR"
        || bytes[16..20] != icon.width.to_be_bytes() || bytes[20..24] != icon.height.to_be_bytes() {
        return Err(Error::Invalid("architecture icon PNG header or dimensions mismatch".into()));
    }
    let base64 = STANDARD.encode(bytes);
    let info = crate::media::inspect_raster(&base64, "image/png")?;
    if info.width != icon.width || info.height != icon.height { return Err(Error::Invalid("architecture icon decoded dimensions mismatch".into())); }
    Ok(ArchitectureIconAsset { id: icon.id.clone(), base64, mime_type: "image/png", alt: icon.name.clone(), width: info.width, height: info.height })
}

fn assets_at(compiled: &Compiled, ids: &[String], root: impl FnOnce() -> Result<PathBuf>) -> Result<ArchitectureIconAssets> {
    let icons = selected(&compiled.manifest, ids)?;
    let root = root()?;
    check_consent(&root, &compiled.sha256).map_err(|error| Error::Unsupported(format!("{error}. {SETUP}")))?;
    let icons = icons.into_iter().map(|icon| read_icon(&root, icon)).collect::<Result<Vec<_>>>()?;
    Ok(ArchitectureIconAssets { icons })
}

pub fn assets(ids: &[String]) -> Result<ArchitectureIconAssets> {
    let compiled = compiled()?;
    assets_at(compiled, ids, || host_root(&compiled.manifest.release))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    fn fixture() -> (tempfile::TempDir, Compiled, Vec<u8>) {
        let directory = tempfile::tempdir().unwrap();
        let mut manifest: Manifest = serde_json::from_slice(CATALOG_BYTES).unwrap();
        let mut output = Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(512, 128, image::Rgba([17u8, 88, 160, 255])).write_to(&mut output, image::ImageFormat::Png).unwrap();
        let bytes = output.into_inner();
        manifest.icons.truncate(2);
        for icon in &mut manifest.icons {
            icon.provider = "aws".into(); icon.png_sha256 = sha256(&bytes); icon.png_bytes = bytes.len(); icon.width = 512; icon.height = 128;
        }
        fs::create_dir(directory.path().join("aws")).unwrap();
        fs::write(directory.path().join("aws").join(format!("{}.png", sha256(&bytes))), &bytes).unwrap();
        let compiled = Compiled { manifest, sha256: sha256(CATALOG_BYTES) };
        consent(directory.path(), json!({"version":1,"catalog_sha256":compiled.sha256,"accepted_vendor_terms":true}));
        (directory, compiled, bytes)
    }

    fn consent(root: &Path, value: serde_json::Value) { fs::write(root.join("consent.json"), serde_json::to_vec(&value).unwrap()).unwrap(); }
    fn ids(compiled: &Compiled) -> Vec<String> { compiled.manifest.icons.iter().rev().map(|icon| icon.id.clone()).collect() }
    fn png_path(root: &Path, compiled: &Compiled) -> PathBuf { root.join("aws").join(format!("{}.png", compiled.manifest.icons[0].png_sha256)) }

    #[cfg(windows)]
    #[test]
    fn architecture_icons_denied_drive_precedes_every_metadata_probe() {
        use std::cell::Cell;
        for (path, file) in [
            ("Z:/pack", false),
            ("Z:/pack/consent.json", true),
            ("Z:/pack/aws/image.png", true),
            ("Z:/Users/test/AppData/Local/AISlide/icon-packs/2026-09-20/consent.json", true),
        ] {
            let classifications = Cell::new(0);
            let inspections = Cell::new(0);
            let result = checked_metadata_with(Path::new(path), file, |letter| {
                assert_eq!(letter, b'Z');
                classifications.set(classifications.get() + 1);
                false
            }, |_| {
                inspections.set(inspections.get() + 1);
                Err(std::io::Error::other("synthetic metadata probe"))
            });
            assert_eq!(inspections.get(), 0, "metadata preceded rejection for {path}");
            assert_eq!(classifications.get(), 1);
            assert!(result.unwrap_err().to_string().contains("known local Windows drive"));
        }
    }

    #[cfg(windows)]
    #[test]
    fn architecture_icons_local_drive_classification_precedes_ancestor_walk() {
        use std::cell::RefCell;
        let directory = tempfile::tempdir().unwrap();
        let metadata = fs::symlink_metadata(directory.path()).unwrap();
        let events = RefCell::new(Vec::new());
        checked_metadata_with(Path::new("Z:/pack"), false, |letter| {
            assert_eq!(letter, b'Z');
            events.borrow_mut().push("classify".to_string());
            true
        }, |path| {
            events.borrow_mut().push(path.to_string_lossy().replace('\\', "/"));
            Ok(metadata.clone())
        }).unwrap();
        assert_eq!(*events.borrow(), ["classify", "Z:/", "Z:/pack", "Z:/pack"]);
    }

    #[test]
    fn architecture_icons_lexical_rejection_precedes_drive_and_metadata_queries() {
        for path in ["relative", "C:relative", "\\\\server\\share", "//server/share", "\\\\?\\C:\\pack", "\\\\.\\C:\\pack", "C:/pack/../other", "C:/pack/./other"] {
            assert!(checked_metadata_with(Path::new(path), false,
                |_| panic!("invalid path must not query a drive"),
                |_| panic!("invalid path must not inspect metadata"),
            ).is_err(), "{path}");
        }
    }

    #[test]
    fn architecture_icons_original_png_order_and_full_labels_survive() {
        let (directory, compiled, bytes) = fixture();
        let requested = ids(&compiled);
        let response = assets_at(&compiled, &requested, || Ok(directory.path().into())).unwrap();
        assert_eq!(response.icons.iter().map(|icon| &icon.id).collect::<Vec<_>>(), requested.iter().collect::<Vec<_>>());
        for asset in response.icons {
            assert_eq!(STANDARD.decode(asset.base64).unwrap(), bytes);
            assert_eq!((asset.width, asset.height, asset.mime_type), (512, 128, "image/png"));
            assert_eq!(asset.alt, compiled.manifest.icons.iter().find(|icon| icon.id == asset.id).unwrap().name);
        }
    }

    #[test]
    fn architecture_icons_consent_is_not_a_complete_pack_claim() {
        let (directory, compiled, _) = fixture();
        fs::remove_file(png_path(directory.path(), &compiled)).unwrap();
        let catalog = catalog_at(&compiled, Ok(directory.path().into()));
        assert!(catalog.configured);
        assert!(catalog.message.contains("not a complete-pack"));
        assert!(assets_at(&compiled, &ids(&compiled), || Ok(directory.path().into())).is_err());
        fs::remove_file(directory.path().join("consent.json")).unwrap();
        let catalog = catalog_at(&compiled, Ok(directory.path().into()));
        assert!(!catalog.configured);
        assert_eq!(catalog.icons.len(), 2);
        assert!(catalog.message.contains("tools/cloud-icons-setup.mjs"));
        assert!(!catalog.message.contains(directory.path().to_str().unwrap()));
    }

    #[test]
    fn architecture_icons_consent_rejects_wrong_hash_version_acceptance_and_unknown_fields() {
        let (directory, compiled, _) = fixture();
        for invalid in [json!({"version":2,"catalog_sha256":compiled.sha256,"accepted_vendor_terms":true}),
            json!({"version":1,"catalog_sha256":"0".repeat(64),"accepted_vendor_terms":true}),
            json!({"version":1,"catalog_sha256":compiled.sha256,"accepted_vendor_terms":false}),
            json!({"version":1,"catalog_sha256":compiled.sha256,"accepted_vendor_terms":true,"root":"other"})] {
            consent(directory.path(), invalid);
            assert!(!catalog_at(&compiled, Ok(directory.path().into())).configured);
            assert!(assets_at(&compiled, &ids(&compiled), || Ok(directory.path().into())).is_err());
        }
        for bytes in [b"{".to_vec(), vec![b' '; MAX_CONSENT_BYTES + 1]] {
            fs::write(directory.path().join("consent.json"), bytes).unwrap();
            assert!(!catalog_at(&compiled, Ok(directory.path().into())).configured);
        }
    }

    #[test]
    fn architecture_icons_request_validation_precedes_host_or_file_access() {
        let (_, mut compiled, _) = fixture();
        let requested = ids(&compiled);
        for invalid in [vec![], vec!["../consent.json".into()], vec![requested[0].clone(), "unknown".into()], vec![requested[0].clone(); 2], vec![requested[0].clone(); 61], vec!["a".repeat(257)]] {
            assert!(assets_at(&compiled, &invalid, || panic!("host root must not be resolved")).is_err());
        }
        let template = compiled.manifest.icons[0].clone();
        compiled.manifest.icons = (0..5).map(|index| ArchitectureIconEntry { id: format!("aws/test/{index}"), png_bytes: MAX_PNG_BYTES, ..template.clone() }).collect();
        let all = ids(&compiled);
        assert!(selected(&compiled.manifest, &all[..4]).is_ok());
        assert!(assets_at(&compiled, &all, || panic!("budget must precede files")).unwrap_err().to_string().contains("4 MiB"));
    }

    #[test]
    fn architecture_icons_corrupt_truncated_oversized_and_late_batch_failure_are_atomic() {
        let (directory, mut compiled, bytes) = fixture();
        let file = png_path(directory.path(), &compiled);
        let mut corrupt = bytes.clone(); corrupt[30] ^= 1;
        for invalid in [bytes[..bytes.len()-1].to_vec(), [bytes.as_slice(), &[0]].concat(), vec![0; MAX_PNG_BYTES+1], corrupt] {
            fs::write(&file, invalid).unwrap();
            assert!(assets_at(&compiled, &ids(&compiled), || Ok(directory.path().into())).is_err());
        }
        fs::write(file, bytes).unwrap();
        compiled.manifest.icons[0].png_sha256 = "0".repeat(64);
        assert!(assets_at(&compiled, &ids(&compiled), || Ok(directory.path().into())).is_err());
    }

    #[test]
    fn architecture_icons_hash_precedes_decode_and_pinned_header_still_requires_decode() {
        let (directory, mut compiled, bytes) = fixture();
        let mut icon = compiled.manifest.icons[0].clone();
        fs::write(png_path(directory.path(), &compiled), vec![0; bytes.len()]).unwrap();
        assert!(read_icon(directory.path(), &icon).unwrap_err().to_string().contains("SHA-256"));
        icon.width = 256;
        fs::write(png_path(directory.path(), &compiled), &bytes).unwrap();
        assert!(read_icon(directory.path(), &icon).unwrap_err().to_string().contains("dimensions"));
        let invalid = bytes[..24].to_vec();
        for entry in &mut compiled.manifest.icons { entry.png_sha256 = sha256(&invalid); entry.png_bytes = invalid.len(); }
        fs::write(png_path(directory.path(), &compiled), invalid).unwrap();
        assert!(assets_at(&compiled, &ids(&compiled), || Ok(directory.path().into())).unwrap_err().to_string().contains("decoded"));
    }

    #[test]
    fn architecture_icons_rejects_nonregular_files_and_root_traversal() {
        let (directory, compiled, _) = fixture();
        for root in ["relative", "C:drive-relative", "\\\\server\\share", "//server/share", "\\\\?\\C:\\pack", "\\\\.\\C:\\pack", "https://example.invalid/pack", "/tmp/../pack", "/tmp/./pack"] { assert!(local_path(Path::new(root)).is_err(), "{root}"); }
        assert!(local_path(&directory.path().join(".." ).join("pack")).is_err());
        let path = png_path(directory.path(), &compiled);
        fs::remove_file(&path).unwrap(); fs::create_dir(&path).unwrap();
        assert!(assets_at(&compiled, &ids(&compiled), || Ok(directory.path().into())).is_err());
        fs::remove_file(directory.path().join("consent.json")).unwrap(); fs::create_dir(directory.path().join("consent.json")).unwrap();
        assert!(!catalog_at(&compiled, Ok(directory.path().into())).configured);
    }

    #[test]
    fn architecture_icons_compiled_metadata_rejects_unsafe_invariants() {
        let baseline: serde_json::Value = serde_json::from_slice(CATALOG_BYTES).unwrap();
        for (pointer, value) in [("/icons/0/provider", json!("../aws")), ("/icons/0/id", json!("aws/../secret")), ("/icons/0/png_sha256", json!("../secret")),
            ("/icons/0/source/entry", json!("../secret")), ("/icons/0/source/archive_id", json!("unknown")), ("/icons/0/png_bytes", json!(MAX_PNG_BYTES+1)),
            ("/icons/0/width", json!(513)), ("/icons/0/height", json!(0)), ("/providers/0/id", json!("other")), ("/icons/0/id", baseline["icons"][1]["id"].clone())] {
            let mut changed = baseline.clone(); *changed.pointer_mut(pointer).unwrap() = value;
            assert!(validate_manifest(&serde_json::from_value(changed).unwrap()).is_err(), "{pointer}");
        }
        assert!(compiled().is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn architecture_icons_rejects_symlink_root_ancestor_provider_consent_and_png() {
        use std::os::unix::fs::symlink;
        let (directory, compiled, _) = fixture();
        let links = tempfile::tempdir().unwrap();
        let root = links.path().join("linked"); symlink(directory.path(), &root).unwrap();
        assert!(!catalog_at(&compiled, Ok(root.clone())).configured);
        assert!(checked_metadata(&root.join("missing/child"), false).unwrap_err().to_string().contains("symlinks"));
        let provider = directory.path().join("aws"); let moved = directory.path().join("moved"); fs::rename(&provider, &moved).unwrap(); symlink(&moved, &provider).unwrap();
        assert!(assets_at(&compiled, &ids(&compiled), || Ok(directory.path().into())).is_err());
        fs::remove_file(&provider).unwrap(); fs::rename(moved, provider).unwrap();
        for file in [directory.path().join("consent.json"), png_path(directory.path(), &compiled)] {
            let moved = file.with_extension("original"); fs::rename(&file, &moved).unwrap(); symlink(&moved, &file).unwrap();
            assert!(assets_at(&compiled, &ids(&compiled), || Ok(directory.path().into())).is_err());
            fs::remove_file(&file).unwrap(); fs::rename(moved, file).unwrap();
        }
    }

    #[cfg(windows)]
    #[test]
    fn architecture_icons_rejects_windows_junction_root_ancestor_and_provider() {
        let (directory, compiled, _) = fixture();
        let links = tempfile::tempdir().unwrap();
        let link = links.path().join("linked");
        let result = std::process::Command::new("cmd.exe").args(["/D", "/C", "mklink", "/J"]).arg(&link).arg(directory.path()).output().unwrap();
        assert!(result.status.success(), "junction fixture creation failed");
        assert!(!catalog_at(&compiled, Ok(link.clone())).configured);
        assert!(checked_metadata(&link.join("missing/child"), false).unwrap_err().to_string().contains("reparse"));
        fs::remove_dir(&link).unwrap();
        let provider = directory.path().join("aws"); let moved = directory.path().join("moved"); fs::rename(&provider, &moved).unwrap();
        let result = std::process::Command::new("cmd.exe").args(["/D", "/C", "mklink", "/J"]).arg(&provider).arg(&moved).output().unwrap();
        assert!(result.status.success(), "provider junction fixture creation failed");
        assert!(assets_at(&compiled, &ids(&compiled), || Ok(directory.path().into())).is_err());
        fs::remove_dir(provider).unwrap();
    }
}