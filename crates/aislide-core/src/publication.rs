use crate::{document::Document, Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Serialize;
use std::{fs, io::Write, path::{Path, PathBuf}};

#[derive(Serialize)]
pub struct PublishedProject { pub path: PathBuf, pub checkpoint_path: PathBuf, pub bytes: usize }

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