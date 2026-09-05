//! Filesystem scanning: find ROMs, work out what platform they belong to,
//! hash them, and record them in the library.

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

use crate::db;
use crate::error::Result;
use crate::hashing;
use crate::models::ScanProgress;
use crate::romcheck;
use crate::platforms;

/// Strip the extension and the usual No-Intro / TOSEC bracket tags to get
/// something presentable before a scraper has run.
pub fn clean_title(filename: &str) -> String {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);

    let mut out = String::new();
    let mut depth_paren = 0i32;
    let mut depth_brack = 0i32;
    for ch in stem.chars() {
        match ch {
            '(' => depth_paren += 1,
            ')' => depth_paren = (depth_paren - 1).max(0),
            '[' => depth_brack += 1,
            ']' => depth_brack = (depth_brack - 1).max(0),
            _ if depth_paren == 0 && depth_brack == 0 => out.push(ch),
            _ => {}
        }
    }

    let out = out.replace(['_', '.'], " ");
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let out = out.trim_matches(|c: char| c == '-' || c.is_whitespace()).to_string();

    // "Legend of Zelda, The" -> "The Legend of Zelda"
    if let Some((head, tail)) = out.rsplit_once(", ") {
        let t = tail.trim();
        if matches!(
            t.to_ascii_lowercase().as_str(),
            "the" | "a" | "an" | "le" | "la" | "les" | "der" | "die" | "das"
        ) {
            return format!("{t} {head}");
        }
    }

    if out.is_empty() {
        stem.to_string()
    } else {
        out
    }
}

/// Pull a region out of the filename tags, e.g. "Sonic (USA, Europe).md".
pub fn guess_region(filename: &str) -> Option<String> {
    const REGIONS: &[&str] = &[
        "USA", "Europe", "Japan", "World", "Australia", "Germany", "France", "Spain", "Italy",
        "Korea", "Brazil", "Canada", "China", "Netherlands", "Sweden", "Asia", "Taiwan",
    ];
    let lower = filename.to_ascii_lowercase();
    let mut found: Vec<&str> = Vec::new();
    for r in REGIONS {
        if lower.contains(&r.to_ascii_lowercase()) {
            found.push(r);
        }
    }
    if found.is_empty() {
        None
    } else {
        Some(found.join(", "))
    }
}

/// Decide which platform a ROM belongs to.
///
/// Extensions alone are not enough — `.bin`, `.cue`, `.iso` and `.zip` are
/// shared across a dozen systems — so this falls back to the folder's assigned
/// platform, then to directory names, then to what is inside the archive.
pub fn detect_platform(path: &Path, root: &Path, folder_override: Option<&str>) -> String {
    if let Some(p) = folder_override.filter(|p| !p.is_empty()) {
        return p.to_string();
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // 1. An extension only one system uses settles it.
    let candidates = platforms::candidates_for_ext(&ext);
    if candidates.len() == 1 {
        return candidates[0].slug.to_string();
    }

    // 2. For a zip, look at what is inside.
    if platforms::ARCHIVE_EXTS.contains(&ext.as_str()) {
        if let Ok(names) = hashing::zip_entry_names(path) {
            for name in &names {
                let inner_ext = Path::new(name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let inner = platforms::candidates_for_ext(&inner_ext);
                if inner.len() == 1 {
                    return inner[0].slug.to_string();
                }
            }
        }
    }

    // 3. Directory names between the library root and the file, nearest first.
    let mut dirs: Vec<String> = Vec::new();
    let mut cur = path.parent();
    while let Some(d) = cur {
        if d == root {
            if let Some(name) = d.file_name().and_then(|n| n.to_str()) {
                dirs.push(name.to_string());
            }
            break;
        }
        if let Some(name) = d.file_name().and_then(|n| n.to_str()) {
            dirs.push(name.to_string());
        }
        cur = d.parent();
    }
    for d in &dirs {
        if let Some(p) = platforms::match_alias(d) {
            return p.slug.to_string();
        }
    }

    // 4. Extension shared by a few systems and nothing else to go on: take the
    //    first candidate rather than dropping the ROM entirely.
    if let Some(first) = candidates.first() {
        return first.slug.to_string();
    }

    "unknown".to_string()
}

#[derive(Default)]
pub struct ScanTally {
    pub added: usize,
    pub skipped: usize,
    /// Files that were not games at all.
    pub ignored: usize,
    reasons: Vec<String>,
}

impl ScanTally {
    pub fn summary(&self) -> String {
        if self.reasons.is_empty() {
            String::new()
        } else {
            romcheck::summarise(&self.reasons)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit(
    app: &AppHandle,
    phase: &str,
    current: usize,
    total: usize,
    message: &str,
    tally: &ScanTally,
    done: bool,
) {
    let _ = app.emit(
        "scan-progress",
        ScanProgress {
            phase: phase.to_string(),
            current,
            total,
            message: message.to_string(),
            added: tally.added,
            skipped: tally.skipped,
            ignored: tally.ignored,
            ignored_summary: tally.summary(),
            done,
        },
    );
}

/// Walk every library folder and add anything new.
///
/// Takes the mutex rather than a connection so the lock can be released
/// between inserts — a scan of a large library would otherwise block every
/// query the UI makes for its whole duration.
pub fn scan_all(
    app: &AppHandle,
    db_handle: &std::sync::Mutex<rusqlite::Connection>,
) -> Result<ScanTally> {
    let (folders, known) = {
        let conn = db_handle.lock().unwrap();
        (db::list_folders(&conn)?, db::all_paths(&conn)?)
    };

    let mut tally = ScanTally::default();
    emit(app, "discovering", 0, 0, "Looking for ROMs…", &tally, false);

    // Pass 1 — discover candidate files.
    let mut candidates: Vec<(PathBuf, PathBuf, Option<String>)> = Vec::new();
    for folder in &folders {
        let root = PathBuf::from(&folder.path);
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                !e.file_name()
                    .to_str()
                    .map(|s| s.starts_with('.') || s.eq_ignore_ascii_case("$RECYCLE.BIN"))
                    .unwrap_or(false)
            })
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ext.is_empty() || !platforms::is_indexable_ext(&ext) {
                continue;
            }
            candidates.push((
                path.to_path_buf(),
                root.clone(),
                folder.platform_override.clone(),
            ));
        }
    }

    // Pass 2 — check, hash and record.
    let total = candidates.len();

    for (i, (path, root, override_platform)) in candidates.iter().enumerate() {
        let path_str = path.to_string_lossy().to_string();
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if known.contains(&path_str) {
            tally.skipped += 1;
        } else {
            let platform = detect_platform(path, root, override_platform.as_deref());

            // Reject anything that is plainly not a game before spending the
            // time to hash it.
            match romcheck::inspect(path, &platform) {
                romcheck::Verdict::NotRom(reason) => {
                    tally.ignored += 1;
                    tally.reasons.push(reason);
                    if i % 5 == 0 || i + 1 == total {
                        emit(app, "hashing", i + 1, total, &filename, &tally, false);
                    }
                    continue;
                }
                // Odd, but indexed anyway — losing a real game is worse.
                romcheck::Verdict::Suspect(_) | romcheck::Verdict::Rom => {}
            }

            let hashes = hashing::hash_rom(path).ok().flatten();
            let size = hashes
                .as_ref()
                .map(|h| h.size as i64)
                .or_else(|| std::fs::metadata(path).ok().map(|m| m.len() as i64))
                .unwrap_or(0);

            // A zipped ROM's real name is the entry inside, which is what the
            // scrapers index.
            let name_for_title = hashes
                .as_ref()
                .and_then(|h| h.inner_name.clone())
                .unwrap_or_else(|| filename.clone());

            let conn = db_handle.lock().unwrap();
            let inserted = db::insert_game(
                &conn,
                &path_str,
                &filename,
                &platform,
                &clean_title(&name_for_title),
                guess_region(&name_for_title).as_deref(),
                size,
                hashes.as_ref().map(|h| h.crc32.as_str()),
                hashes.as_ref().map(|h| h.md5.as_str()),
                hashes.as_ref().map(|h| h.sha1.as_str()),
                hashes.as_ref().and_then(|h| h.inner_name.as_deref()),
            )?;
            if inserted {
                tally.added += 1;
            } else {
                tally.skipped += 1;
            }
        }

        if i % 5 == 0 || i + 1 == total {
            emit(app, "hashing", i + 1, total, &filename, &tally, false);
        }
    }

    let message = format!(
        "Added {}, skipped {}, ignored {}",
        tally.added, tally.skipped, tally.ignored
    );
    emit(app, "done", total, total, &message, &tally, true);

    Ok(tally)
}

/// Drop library entries whose file is gone. Does not delete anything on disk.
pub fn remove_missing(conn: &rusqlite::Connection) -> Result<usize> {
    let paths = db::all_paths(conn)?;
    let mut removed = 0;
    for p in paths {
        if !Path::new(&p).exists() {
            conn.execute("DELETE FROM games WHERE path = ?1", rusqlite::params![p])?;
            removed += 1;
        }
    }
    Ok(removed)
}
