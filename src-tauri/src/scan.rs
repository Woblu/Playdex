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
/// platform, then to what is inside the archive, then to directory names, and
/// finally to the ROM's own name.
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

    // 2. For an archive, look at what is inside.
    let mut inner_names: Vec<String> = Vec::new();
    if platforms::ARCHIVE_EXTS.contains(&ext.as_str()) {
        if let Ok(names) = hashing::archive_entry_names(path) {
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
            inner_names = names;
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

    // 4. The ROM's own name, and the names inside the archive. Dumps are
    //    routinely called "Mario Kart Wii" or "Sonic (Mega Drive)", which is
    //    the last real evidence available for a container extension like .7z
    //    that belongs to no system at all. Matched on whole words only, so a
    //    title is not mistaken for a system it merely contains the letters of.
    let own_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    for name in std::iter::once(own_name).chain(inner_names.iter().map(|s| s.as_str())) {
        if let Some(p) = platforms::match_alias_word(name) {
            return p.slug.to_string();
        }
    }

    // 5. Extension shared by a few systems and nothing else to go on: take the
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
    /// Entries already in the library that were corrected on this pass.
    pub reindexed: usize,
    /// Entries dropped because they are no longer recognised as games. The
    /// file on disk is never touched.
    pub dropped: usize,
    /// Files that were not games at all.
    pub ignored: usize,
    reasons: Vec<String>,
}

/// Whether an entry already in the library was indexed under a rule we have
/// since fixed, and so deserves another look.
///
/// Two cases, both cheap to spot: a platform we never worked out, and an
/// archive we recorded without ever opening — its stored hashes are the
/// container's, which match nothing in a scraper or a DAT.
fn needs_reindex(state: &db::IndexState, ext: &str) -> bool {
    state.platform == "unknown"
        || (platforms::ARCHIVE_EXTS.contains(&ext) && state.inner_name.is_none())
}

impl ScanTally {
    /// The one-line summary of a finished scan, so the progress event and the
    /// command's return value can never drift apart.
    pub fn message(&self) -> String {
        let mut out = format!(
            "Added {}, skipped {}, ignored {}",
            self.added, self.skipped, self.ignored
        );
        if self.reindexed > 0 {
            out.push_str(&format!(", corrected {}", self.reindexed));
        }
        if self.dropped > 0 {
            out.push_str(&format!(", dropped {}", self.dropped));
        }
        out
    }

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
            corrected: tally.reindexed,
            dropped: tally.dropped,
            ignored: tally.ignored,
            ignored_summary: tally.summary(),
            done,
        },
    );
}

/// What happened to one file.
pub enum Indexed {
    Added(i64),
    AlreadyKnown,
    NotRom(String),
}

/// Index a single file into the library.
///
/// Shared by the folder scan and by dropping a ROM onto the window, so a game
/// added either way ends up described identically - same platform detection,
/// same rejection rules, same hashing, same cleaned-up title. A second path
/// for "quick add" would be a second place for those rules to drift.
///
/// `root` is the folder platform detection is allowed to reason about. For a
/// scan that is the library folder; for a dropped file it is whatever folder
/// the file came out of, which is still worth reading since people keep ROMs
/// in folders named after the system.
pub fn index_file(
    conn: &rusqlite::Connection,
    path: &Path,
    root: &Path,
    override_platform: Option<&str>,
) -> Result<Indexed> {
    let path_str = path.to_string_lossy().to_string();
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let platform = detect_platform(path, root, override_platform);

    // Reject anything that is plainly not a game before spending the time to
    // hash it.
    match romcheck::inspect(path, &platform) {
        romcheck::Verdict::NotRom(reason) => return Ok(Indexed::NotRom(reason)),
        // Odd, but indexed anyway - losing a real game is worse.
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

    let inserted = db::insert_game(
        conn,
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
        Ok(Indexed::Added(
            conn.query_row(
                "SELECT id FROM games WHERE path = ?1",
                rusqlite::params![path_str],
                |r| r.get(0),
            )
            .unwrap_or(0),
        ))
    } else {
        Ok(Indexed::AlreadyKnown)
    }
}

/// What came of dropping things onto the window.
#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DropTally {
    pub added: usize,
    /// Already in the library.
    pub skipped: usize,
    /// Inspected and found not to be a game.
    pub ignored: usize,
    /// Folders added to the library, which then get scanned.
    pub folders: usize,
    /// The last game added, so the UI can select it.
    pub last_id: Option<i64>,
    pub reasons: Vec<String>,
}

impl DropTally {
    pub fn message(&self) -> String {
        let mut parts = Vec::new();
        if self.folders > 0 {
            parts.push(format!(
                "Added {} folder{}",
                self.folders,
                if self.folders == 1 { "" } else { "s" }
            ));
        }
        if self.added > 0 {
            parts.push(format!(
                "Added {} game{}",
                self.added,
                if self.added == 1 { "" } else { "s" }
            ));
        }
        if self.skipped > 0 {
            parts.push(format!("{} already in your library", self.skipped));
        }
        if self.ignored > 0 {
            parts.push(format!("{} not a game", self.ignored));
        }
        if parts.is_empty() {
            "Nothing to add".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Take files and folders dropped onto the window and put them in the library.
///
/// A dropped file is indexed where it lies rather than copied anywhere. It is
/// someone's own ROM in their own place, and quietly duplicating gigabytes
/// into the app data folder to make the bookkeeping tidier is not a trade
/// worth making. The consequence is that the file has to stay where it is,
/// which is the same rule the rest of the library already follows.
///
/// A dropped folder becomes a library folder, because that is plainly what
/// someone means by dropping one, and it is the same thing Settings does.
pub fn add_paths(
    db_handle: &std::sync::Mutex<rusqlite::Connection>,
    paths: &[PathBuf],
) -> Result<DropTally> {
    let mut tally = DropTally::default();

    for path in paths {
        if path.is_dir() {
            let conn = db_handle.lock().unwrap();
            db::add_folder(&conn, &path.to_string_lossy(), None)?;
            tally.folders += 1;
            continue;
        }
        if !path.is_file() {
            continue;
        }

        // Skipped before inspection so a dropped folder of mixed junk does not
        // report a wall of reasons for files that were never candidates.
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext.is_empty() || !platforms::is_indexable_ext(&ext) {
            tally.ignored += 1;
            tally.reasons.push(format!("not a ROM format (.{ext})"));
            continue;
        }

        let root = path.parent().unwrap_or(path);
        let conn = db_handle.lock().unwrap();
        match index_file(&conn, path, root, None)? {
            Indexed::Added(id) => {
                tally.added += 1;
                tally.last_id = Some(id);
            }
            Indexed::AlreadyKnown => tally.skipped += 1,
            Indexed::NotRom(reason) => {
                tally.ignored += 1;
                tally.reasons.push(reason);
            }
        }
    }

    Ok(tally)
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
        (db::list_folders(&conn)?, db::index_state(&conn)?)
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
                let hidden = e
                    .file_name()
                    .to_str()
                    .map(|s| s.starts_with('.') || s.eq_ignore_ascii_case("$RECYCLE.BIN"))
                    .unwrap_or(false);
                if hidden {
                    return false;
                }
                // An emulator unpacked into the library is not a ROM folder.
                // Skipping it at its root drops its Sys and data directories
                // with it, which is where the firmware and fonts that read as
                // ROMs actually live. Never applied to the library root
                // itself, which the user chose deliberately.
                if e.depth() > 0 && e.file_type().is_dir() && romcheck::holds_a_program(e.path()) {
                    return false;
                }
                true
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

        if let Some(state) = known.get(&path_str) {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();

            // An entry the scanner itself added and now recognises as a
            // mistake — an emulator's firmware, an application archive — is
            // dropped from the library. The file on disk is untouched, and
            // anything you have played, favourited or built a hack on is left
            // alone however it now reads: a heuristic is not allowed to throw
            // away something you have shown you care about.
            if !state.precious {
                if let romcheck::Verdict::NotRom(reason) = romcheck::inspect(path, &state.platform)
                {
                    let conn = db_handle.lock().unwrap();
                    db::remove_game(&conn, state.id)?;
                    drop(conn);
                    tally.dropped += 1;
                    tally.reasons.push(reason);
                    if i % 5 == 0 || i + 1 == total {
                        emit(app, "hashing", i + 1, total, &filename, &tally, false);
                    }
                    continue;
                }
            }

            if needs_reindex(state, &ext) {
                let platform = detect_platform(path, root, override_platform.as_deref());
                let hashes = hashing::hash_rom(path).ok().flatten();
                let size = hashes
                    .as_ref()
                    .map(|h| h.size as i64)
                    .or_else(|| std::fs::metadata(path).ok().map(|m| m.len() as i64))
                    .unwrap_or(0);

                let conn = db_handle.lock().unwrap();
                db::reindex_game(
                    &conn,
                    state.id,
                    &platform,
                    size,
                    hashes.as_ref().map(|h| h.crc32.as_str()),
                    hashes.as_ref().map(|h| h.md5.as_str()),
                    hashes.as_ref().map(|h| h.sha1.as_str()),
                    hashes.as_ref().and_then(|h| h.inner_name.as_deref()),
                )?;
                tally.reindexed += 1;
            } else {
                tally.skipped += 1;
            }
        } else {
            let conn = db_handle.lock().unwrap();
            match index_file(&conn, path, root, override_platform.as_deref())? {
                Indexed::Added(_) => tally.added += 1,
                Indexed::AlreadyKnown => tally.skipped += 1,
                Indexed::NotRom(reason) => {
                    drop(conn);
                    tally.ignored += 1;
                    tally.reasons.push(reason);
                    if i % 5 == 0 || i + 1 == total {
                        emit(app, "hashing", i + 1, total, &filename, &tally, false);
                    }
                    continue;
                }
            }
        }

        if i % 5 == 0 || i + 1 == total {
            emit(app, "hashing", i + 1, total, &filename, &tally, false);
        }
    }

    let message = tally.message();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Dropping a ROM on the window indexes it where it lies, and dropping a
    /// folder makes it a library folder.
    #[test]
    fn drops_index_files_and_adopt_folders() {
        let dir = std::env::temp_dir().join(format!(
            "playdex-drop-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let conn = db::open(&dir.join("library.db")).unwrap();
        let db_handle = std::sync::Mutex::new(conn);

        // A real-enough NES ROM: iNES header, past the size floor.
        let rom = dir.join("Super Mario Bros. (World).nes");
        let mut bytes = b"NES".to_vec();
        bytes.push(0x1a);
        bytes.resize(40_976, 0);
        std::fs::write(&rom, &bytes).unwrap();

        // Something that is plainly not a game.
        let junk = dir.join("box-scan.png");
        let mut png = vec![0x89u8, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        png.resize(9000, 0);
        std::fs::write(&junk, &png).unwrap();

        // And a folder to adopt.
        let folder = dir.join("More ROMs");
        std::fs::create_dir_all(&folder).unwrap();

        let tally = add_paths(
            &db_handle,
            &[rom.clone(), junk.clone(), folder.clone()],
        )
        .unwrap();

        assert_eq!(tally.added, 1, "the ROM should be added");
        assert_eq!(tally.folders, 1, "the folder should be adopted");
        assert_eq!(tally.ignored, 1, "the PNG should be rejected");
        assert!(tally.last_id.is_some());

        {
            let conn = db_handle.lock().unwrap();
            let game = db::get_game(&conn, tally.last_id.unwrap()).unwrap().unwrap();
            // Indexed in place: the path is where the user left it.
            assert_eq!(game.path, rom.to_string_lossy());
            assert_eq!(game.platform, "nes");
            // clean_title drops the region and the trailing full stop.
            assert_eq!(game.title, "Super Mario Bros");
            assert!(game.crc32.is_some(), "it should have been hashed");

            // The folder really joined the library.
            let folders = db::list_folders(&conn).unwrap();
            assert!(folders.iter().any(|f| f.path == folder.to_string_lossy()));
        }

        // Dropping the same ROM again is recognised rather than duplicated.
        let again = add_paths(&db_handle, &[rom]).unwrap();
        assert_eq!(again.added, 0);
        assert_eq!(again.skipped, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn folder_assignment_beats_everything() {
        let root = Path::new("C:/roms");
        let p = Path::new("C:/roms/Wii/Mario Kart Wii.iso");
        assert_eq!(detect_platform(p, root, Some("gamecube")), "gamecube");
    }

    #[test]
    fn a_unique_extension_settles_it() {
        let root = Path::new("C:/roms");
        let p = Path::new("C:/roms/anything/Super Metroid.sfc");
        assert_eq!(detect_platform(p, root, None), "snes");
    }

    #[test]
    fn a_directory_named_after_a_system_wins_over_the_title() {
        let root = Path::new("C:/roms");
        // The file says Wii; the folder the user made says GameCube. Their
        // own filing is the better evidence.
        let p = Path::new("C:/roms/GameCube/Wii Play.iso");
        assert_eq!(detect_platform(p, root, None), "gamecube");
    }

    /// The case this was written for: a 7z belongs to no system by extension,
    /// sits in a folder named nothing in particular, and the only thing left
    /// saying "Wii" is the filename.
    #[test]
    fn falls_back_to_the_roms_own_name() {
        let root = Path::new("C:/roms");
        let p = Path::new("C:/roms/Retro Folders/New Super Mario Bros Wii [SMNE01].7z");
        assert_eq!(detect_platform(p, root, None), "wii");
    }

    #[test]
    fn a_nameless_container_is_still_unidentified() {
        let root = Path::new("C:/roms");
        let p = Path::new("C:/roms/Retro Folders/Some Game.7z");
        assert_eq!(detect_platform(p, root, None), "unknown");
    }

    #[test]
    fn an_ambiguous_extension_takes_its_first_candidate() {
        let root = Path::new("C:/roms");
        // rvz is shared by GameCube and Wii; with nothing else to go on it
        // lands on the first rather than being dropped.
        let p = Path::new("C:/roms/discs/Some Game.rvz");
        assert_eq!(detect_platform(p, root, None), "gamecube");
        // ...but the name is enough to tell them apart.
        let p = Path::new("C:/roms/discs/Wii Sports Resort.rvz");
        assert_eq!(detect_platform(p, root, None), "wii");
    }
}


