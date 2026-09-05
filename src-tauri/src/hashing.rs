//! One-pass CRC32 / MD5 / SHA1 hashing.
//!
//! Scrapers and DAT files (No-Intro, Redump) hash the *ROM*, not the archive
//! that wraps it, so for a zip we hash the entry inside rather than the file
//! on disk.

use md5::{Digest, Md5};
use sha1::Sha1;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::error::Result;
use crate::platforms;

#[derive(Debug, Clone, Default)]
pub struct Hashes {
    pub crc32: String,
    pub md5: String,
    pub sha1: String,
    pub size: u64,
    /// Entry name when the hashes came from inside an archive.
    pub inner_name: Option<String>,
}

/// Files larger than this are indexed but not hashed. Disc images run to
/// gigabytes and hashing them all would make a scan take hours for metadata
/// we can still look up by name.
pub const MAX_HASH_BYTES: u64 = 512 * 1024 * 1024;

fn hash_reader<R: Read>(mut r: R) -> Result<Hashes> {
    let mut crc = crc32fast::Hasher::new();
    let mut md5 = Md5::new();
    let mut sha1 = Sha1::new();
    let mut size: u64 = 0;
    let mut buf = vec![0u8; 1024 * 1024];

    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let chunk = &buf[..n];
        crc.update(chunk);
        md5.update(chunk);
        sha1.update(chunk);
        size += n as u64;
    }

    Ok(Hashes {
        crc32: format!("{:08x}", crc.finalize()),
        md5: hex::encode(md5.finalize()),
        sha1: hex::encode(sha1.finalize()),
        size,
        inner_name: None,
    })
}

/// Hash a ROM, transparently descending into a zip when there is one clear
/// ROM inside it.
pub fn hash_rom(path: &Path) -> Result<Option<Hashes>> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_HASH_BYTES {
        return Ok(None);
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if platforms::ARCHIVE_EXTS.contains(&ext.as_str()) {
        return hash_zip_entry(path);
    }

    let f = File::open(path)?;
    Ok(Some(hash_reader(BufReader::new(f))?))
}

/// Pick the most ROM-looking entry in a zip and hash it.
fn hash_zip_entry(path: &Path) -> Result<Option<Hashes>> {
    let f = File::open(path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(f))?;

    // Survey the entries first, so no borrow of `archive` is held while we
    // decide which one to hash.
    let mut candidates: Vec<(usize, u64, bool)> = Vec::new();
    for i in 0..archive.len() {
        let (name, size, is_dir) = {
            let entry = archive.by_index(i)?;
            (entry.name().to_string(), entry.size(), entry.is_dir())
        };
        if is_dir {
            continue;
        }
        let ext = Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !platforms::is_indexable_ext(&ext) {
            continue;
        }
        candidates.push((i, size, platforms::is_unique_ext(&ext)));
    }

    // Prefer an unambiguous ROM extension, then the largest entry.
    candidates.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)));

    let Some(&(idx, size, _)) = candidates.first() else {
        return Ok(None);
    };
    if size > MAX_HASH_BYTES {
        return Ok(None);
    }

    let mut entry = archive.by_index(idx)?;
    let inner_name = entry.name().to_string();
    let mut hashes = hash_reader(&mut entry)?;
    hashes.inner_name = Some(inner_name);
    Ok(Some(hashes))
}

/// Read a ROM's actual bytes, descending into a zip when needed.
/// Returns the bytes and the ROM's own name (the entry name, when zipped).
pub fn read_rom_bytes(path: &Path) -> Result<(Vec<u8>, String)> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let own_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("rom")
        .to_string();

    if !platforms::ARCHIVE_EXTS.contains(&ext.as_str()) {
        return Ok((std::fs::read(path)?, own_name));
    }

    let f = File::open(path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(f))?;

    let mut candidates: Vec<(usize, u64, bool)> = Vec::new();
    for i in 0..archive.len() {
        let (name, size, is_dir) = {
            let entry = archive.by_index(i)?;
            (entry.name().to_string(), entry.size(), entry.is_dir())
        };
        if is_dir {
            continue;
        }
        let e = Path::new(&name)
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !platforms::is_indexable_ext(&e) {
            continue;
        }
        candidates.push((i, size, platforms::is_unique_ext(&e)));
    }
    candidates.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)));

    let Some(&(idx, _, _)) = candidates.first() else {
        return Err(crate::error::AppError::Other(
            "No ROM found inside the archive".into(),
        ));
    };

    let mut entry = archive.by_index(idx)?;
    let inner_name = entry.name().to_string();
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf)?;
    Ok((buf, inner_name))
}

/// The names inside an archive, used for platform detection.
pub fn zip_entry_names(path: &Path) -> Result<Vec<String>> {
    let f = File::open(path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(f))?;
    let mut out = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        if !entry.is_dir() {
            out.push(entry.name().to_string());
        }
    }
    Ok(out)
}
