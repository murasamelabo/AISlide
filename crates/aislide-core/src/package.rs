use crate::{Error, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Write};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

pub const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_PART_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_PARTS: usize = 4096;

#[derive(Clone, Debug)]
pub struct Package {
    original: Vec<u8>,
    parts: BTreeMap<String, Vec<u8>>,
    changes: BTreeMap<String, Vec<u8>>,
}

impl Package {
    pub fn open(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() > MAX_ARCHIVE_BYTES {
            return Err(Error::Limit("compressed archive > 64 MiB".into()));
        }
        let entry_count = preflight_directory(&bytes)?;
        let mut archive = ZipArchive::new(Cursor::new(&bytes))?;
        if archive.len() != entry_count {
            return Err(Error::Invalid("duplicate or inconsistent ZIP directory entries".into()));
        }
        if archive.len() > MAX_PARTS {
            return Err(Error::Limit("archive entries > 4096".into()));
        }
        let mut parts = BTreeMap::new();
        let mut names = BTreeSet::new();
        let mut total = 0usize;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let name = entry.name().to_string();
            validate_name(name.trim_end_matches('/'))?;
            if !names.insert(name.to_ascii_lowercase()) {
                return Err(Error::Invalid("duplicate archive part".into()));
            }
            if entry.encrypted() || entry.is_symlink() {
                return Err(Error::Unsupported("encrypted or symlink entry".into()));
            }
            if entry.is_dir() {
                continue;
            }
            if entry.size() > MAX_PART_BYTES as u64 {
                return Err(Error::Limit("archive part > 16 MiB".into()));
            }
            let mut data = Vec::new();
            Read::by_ref(&mut entry)
                .take(MAX_PART_BYTES as u64 + 1)
                .read_to_end(&mut data)?;
            total += data.len();
            if data.len() > MAX_PART_BYTES || total > MAX_TOTAL_BYTES {
                return Err(Error::Limit("expanded archive budget".into()));
            }
            if data.len() as u64 != entry.size() {
                return Err(Error::Invalid("inconsistent archive size".into()));
            }
            parts.insert(name, data);
        }
        Ok(Self { original: bytes, parts, changes: BTreeMap::new() })
    }

    pub fn from_parts(parts: BTreeMap<String, Vec<u8>>) -> Result<Self> {
        if parts.len() > MAX_PARTS {
            return Err(Error::Limit("too many parts".into()));
        }
        let mut total = 0;
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in parts {
            validate_name(&name)?;
            total += data.len();
            if data.len() > MAX_PART_BYTES || total > MAX_TOTAL_BYTES {
                return Err(Error::Limit("package payload budget".into()));
            }
            writer.start_file(name, options())?;
            writer.write_all(&data)?;
        }
        Self::open(writer.finish()?.into_inner())
    }

    pub fn save(&self) -> Result<Vec<u8>> {
        if self.changes.is_empty() {
            return Ok(self.original.clone());
        }
        let mut original = ZipArchive::new(Cursor::new(&self.original))?;
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer.set_raw_comment(original.comment().to_vec().into_boxed_slice());
        for index in 0..original.len() {
            let entry = original.by_index(index)?;
            if let Some(replacement) = self.changes.get(entry.name()) {
                writer.start_file(entry.name(), entry.options())?;
                writer.write_all(replacement)?;
            } else {
                writer.raw_copy_file(entry)?;
            }
        }
        let bytes = writer.finish()?.into_inner();
        if bytes.len() > MAX_ARCHIVE_BYTES {
            return Err(Error::Limit("saved archive > 64 MiB".into()));
        }
        Ok(bytes)
    }

    pub fn parts(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.parts
    }

    pub fn part(&self, name: &str) -> Result<&[u8]> {
        self.changes.get(name).or_else(|| self.parts.get(name))
            .map(Vec::as_slice)
            .ok_or_else(|| Error::Invalid(format!("missing part {name}")))
    }

    pub fn text(&self, name: &str) -> Result<&str> {
        std::str::from_utf8(self.part(name)?)
            .map_err(|_| Error::Unsupported("non-UTF-8 XML part".into()))
    }

    pub fn changed_parts(&self) -> Vec<String> {
        self.changes.keys().cloned().collect()
    }

    pub fn replace_part(&mut self, name: &str, bytes: Vec<u8>) -> Result<()> {
        let original = self.parts.get(name)
            .ok_or_else(|| Error::Invalid(format!("missing part {name}")))?;
        let total: usize = self.parts.iter().map(|(part_name, data)| {
            if part_name == name { bytes.len() }
            else { self.changes.get(part_name).unwrap_or(data).len() }
        }).sum();
        if bytes.len() > MAX_PART_BYTES || total > MAX_TOTAL_BYTES {
            return Err(Error::Limit("replacement payload budget".into()));
        }
        if original == &bytes {
            self.changes.remove(name);
        } else {
            self.changes.insert(name.to_string(), bytes);
        }
        Ok(())
    }
}

fn options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 512 || name.starts_with('/')
        || name.contains(['\\', ':', '\0', '?', '#'])
        || name.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return Err(Error::Invalid("unsafe archive part name".into()));
    }
    Ok(())
}

fn preflight_directory(bytes: &[u8]) -> Result<usize> {
    if bytes.len() < 22 { return Err(Error::Invalid("truncated ZIP".into())); }
    let word = |index: usize| u16::from_le_bytes([bytes[index], bytes[index + 1]]) as usize;
    let dword = |index: usize| u32::from_le_bytes([bytes[index], bytes[index + 1], bytes[index + 2], bytes[index + 3]]) as usize;
    let start = bytes.len().saturating_sub(65_557);
    let end = (start..=bytes.len() - 22).rev().find(|&index| {
        bytes[index..index + 4] == [0x50, 0x4b, 0x05, 0x06] && index + 22 + word(index + 20) == bytes.len()
    }).ok_or_else(|| Error::Invalid("ZIP end record not found".into()))?;
    let entries = word(end + 10);
    if word(end + 4) != 0 || word(end + 6) != 0 || word(end + 8) != entries || entries == 65_535 {
        return Err(Error::Unsupported("ZIP64 or multi-disk archives".into()));
    }
    if entries > MAX_PARTS { return Err(Error::Limit("archive entries > 4096".into())); }
    let mut cursor = dword(end + 16);
    let size = dword(end + 12);
    if cursor.checked_add(size) != Some(end) { return Err(Error::Invalid("ZIP directory bounds".into())); }
    for _entry in 0..entries {
        if cursor + 46 > end || bytes[cursor..cursor + 4] != [0x50, 0x4b, 0x01, 0x02] {
            return Err(Error::Invalid("truncated ZIP directory entry".into()));
        }
        if dword(cursor + 20) == u32::MAX as usize || dword(cursor + 24) == u32::MAX as usize || dword(cursor + 42) == u32::MAX as usize {
            return Err(Error::Unsupported("ZIP64 entry".into()));
        }
        cursor += 46 + word(cursor + 28) + word(cursor + 30) + word(cursor + 32);
    }
    if cursor != end { return Err(Error::Invalid("ZIP directory size/count mismatch".into())); }
    Ok(entries)
}