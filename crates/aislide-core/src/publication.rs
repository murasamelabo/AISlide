use crate::{document::Document, Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Serialize;
use std::{fs, io::{Read, Write}, path::{Path, PathBuf}};
use sha2::{Digest, Sha256};

#[derive(Serialize)]
pub struct PublishedProject { pub path: PathBuf, pub checkpoint_path: PathBuf, pub bytes: usize }

#[derive(Serialize)]
pub struct PublishedPresentation { pub path: PathBuf, pub bytes: usize, pub sha256: String }

pub fn publish_presentation(document: &Document, destination: &Path) -> Result<PublishedPresentation> {
    if !destination.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| extension.eq_ignore_ascii_case("pptx")) { return Err(Error::Invalid("presentation output must use a .pptx filename".into())); }
    match fs::symlink_metadata(destination) {
        Ok(_) => return Err(Error::Io(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "PPTX already exists; choose a new filename to preserve the original"))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::Io(error)),
    }
    let exported = crate::document::export_presentation(document)?;
    let bytes = STANDARD.decode(exported["base64"].as_str().ok_or_else(|| Error::Invalid("export archive missing".into()))?).map_err(|_| Error::Invalid("export archive encoding".into()))?;
    let parent = destination.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    fs::hard_link(temporary.path(), destination)?;
    let mut persisted = Vec::new();
    fs::File::open(destination)?.take(bytes.len() as u64 + 1).read_to_end(&mut persisted)?;
    if persisted != bytes { return Err(Error::Conflict("PPTX changed during publication, possibly by another application or protection policy. The published file was retained; its protection was not changed.".into())); }
    Ok(PublishedPresentation { path: destination.into(), bytes: bytes.len(), sha256: format!("{:x}", Sha256::digest(&bytes)) })
}

pub fn publish_project(document: &Document, destination: &Path) -> Result<PublishedProject> {
    if !destination.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| extension.eq_ignore_ascii_case("pptx")) { return Err(Error::Invalid("project output must use a .pptx filename".into())); }
    let bundle = crate::document::export(document)?;
    let bytes = STANDARD.decode(bundle["base64"].as_str().ok_or_else(|| Error::Invalid("export archive missing".into()))?).map_err(|_| Error::Invalid("export archive encoding".into()))?;
    let checkpoint = serde_json::to_vec(&bundle["checkpoint"])?;
    let parent = destination.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let checkpoint_path = destination.with_extension("aislide.json");
    for path in [destination, checkpoint_path.as_path()] {
        match fs::symlink_metadata(path) {
            Ok(_) => return Err(Error::Io(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "project output already exists"))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error::Io(error)),
        }
    }
    let mut presentation_file = tempfile::NamedTempFile::new_in(parent)?;
    let mut checkpoint_file = tempfile::NamedTempFile::new_in(parent)?;
    presentation_file.write_all(&bytes)?; presentation_file.as_file().sync_all()?;
    checkpoint_file.write_all(&checkpoint)?; checkpoint_file.as_file().sync_all()?;
    fs::hard_link(presentation_file.path(), destination)?;
    if let Err(error) = fs::hard_link(checkpoint_file.path(), &checkpoint_path) {
        return Err(Error::Io(std::io::Error::new(error.kind(), format!("partial project publication at {}; checkpoint could not be published. No published paths were removed; verify the pair before use", destination.display()))));
    }
    Ok(PublishedProject { path: destination.into(), checkpoint_path, bytes: bytes.len() })
}