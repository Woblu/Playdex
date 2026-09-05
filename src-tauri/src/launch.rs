//! Launching games and recording how long they were played.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

use crate::db;
use crate::error::{AppError, Result};
use crate::models::{EmulatorConfig, Game};
use crate::platforms;

/// Split a command template into argv, respecting double quotes.
fn tokenize(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in input.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Where RetroArch keeps its cores, if the user did not say explicitly.
fn default_cores_dir(retroarch_path: &Path) -> Option<PathBuf> {
    let dir = retroarch_path.parent()?;
    let candidate = dir.join("cores");
    if candidate.is_dir() {
        Some(candidate)
    } else {
        None
    }
}

fn core_filename(core: &str) -> String {
    #[cfg(target_os = "windows")]
    let ext = "dll";
    #[cfg(target_os = "macos")]
    let ext = "dylib";
    #[cfg(all(unix, not(target_os = "macos")))]
    let ext = "so";

    if core.ends_with(ext) {
        core.to_string()
    } else {
        format!("{core}.{ext}")
    }
}

/// The emulator configured for a platform, falling back to RetroArch with the
/// platform's preferred core.
pub fn resolve_config(conn: &rusqlite::Connection, platform: &str) -> EmulatorConfig {
    if let Ok(Some(cfg)) = db::get_emulator(conn, platform) {
        let has_core = cfg.core.as_deref().map(|c| !c.is_empty()).unwrap_or(false);
        let has_cmd = cfg
            .custom_command
            .as_deref()
            .map(|c| !c.is_empty())
            .unwrap_or(false);
        if (cfg.mode == "custom" && has_cmd) || (cfg.mode == "retroarch" && has_core) {
            return cfg;
        }
    }
    EmulatorConfig {
        platform: platform.to_string(),
        mode: "retroarch".into(),
        core: platforms::by_slug(platform)
            .and_then(|p| p.cores.first())
            .map(|c| c.to_string()),
        custom_command: None,
    }
}

/// The extensions a core will actually accept, from the `.info` file
/// RetroArch ships beside it.
///
/// Worth reading rather than guessing: Dolphin, for instance, lists
/// `gcm|iso|wbfs|ciso|gcz|elf|dol|dff|tgc|wad|rvz|m3u|wia` and no archive
/// format at all, so handing it a `.7z` fails with nothing useful said.
pub fn core_extensions(ra_path: &Path, core: &str) -> Option<Vec<String>> {
    let stem = core.trim_end_matches(".dll").trim_end_matches(".so");
    let info = ra_path.parent()?.join("info").join(format!("{stem}.info"));
    let text = std::fs::read_to_string(info).ok()?;

    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "supported_extensions" {
            continue;
        }
        return Some(
            value
                .trim()
                .trim_matches('"')
                .split('|')
                .map(|e| e.trim().to_ascii_lowercase())
                .filter(|e| !e.is_empty())
                .collect(),
        );
    }
    None
}

/// Where an archived ROM gets unpacked to, keyed by the archive's own hash so
/// two dumps of the same name cannot collide.
fn cache_dir_for(game: &Game, cache_root: &Path) -> PathBuf {
    let key = game
        .crc32
        .clone()
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| game.id.to_string());
    cache_root.join("extracted").join(key)
}

/// Whether this ROM has to be unpacked before the emulator can open it.
///
/// `accepts` is the core's own extension list where we have one. Without it
/// the rule is just `.7z`: RetroArch unpacks a `.zip` itself for the cores
/// that want one, but a 7z is no use to anything.
fn needs_extracting(game: &Game, accepts: Option<&[String]>) -> bool {
    let ext = Path::new(&game.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if !crate::platforms::ARCHIVE_EXTS.contains(&ext.as_str()) {
        return false;
    }
    match accepts {
        // The core said what it takes. If the archive is not on that list,
        // it has to come out of the archive first.
        Some(list) => !list.iter().any(|e| e == &ext),
        None => ext == "7z",
    }
}

/// Where the ROM *will* be once unpacked, without doing the unpacking — for
/// showing the command before anything runs.
pub fn preview_rom_path(game: &Game, cache_root: &Path, accepts: Option<&[String]>) -> String {
    if !needs_extracting(game, accepts) {
        return game.path.clone();
    }
    let inner = game
        .inner_name
        .as_deref()
        .and_then(|n| Path::new(n).file_name().and_then(|f| f.to_str()))
        .unwrap_or("<rom>");
    cache_dir_for(game, cache_root)
        .join(inner)
        .to_string_lossy()
        .to_string()
}

/// The path to hand the emulator, unpacking the ROM first if it has to be.
pub fn playable_path(game: &Game, cache_root: &Path, accepts: Option<&[String]>) -> Result<String> {
    if !needs_extracting(game, accepts) {
        return Ok(game.path.clone());
    }
    let dir = cache_dir_for(game, cache_root);
    let file = crate::hashing::extract_rom(Path::new(&game.path), &dir)?;
    Ok(file.to_string_lossy().to_string())
}

/// Build the command for a game without running it — also used by the UI to
/// show what will be executed. `rom_path` is what the emulator is handed,
/// which is not the library path when the ROM had to be unpacked.
pub fn build_command(
    conn: &rusqlite::Connection,
    game: &Game,
    rom_path: &str,
) -> Result<(String, Vec<String>)> {
    let cfg = resolve_config(conn, &game.platform);

    if cfg.mode == "custom" {
        let template = cfg
            .custom_command
            .filter(|c| !c.is_empty())
            .ok_or_else(|| AppError::Other(format!("No emulator set for {}", game.platform)))?;
        let filled = template.replace("{rom}", rom_path);
        let mut argv = tokenize(&filled);
        if argv.is_empty() {
            return Err(AppError::Other("Emulator command is empty".into()));
        }
        let exe = argv.remove(0);
        // A template with no {rom} still needs the ROM passed somewhere.
        if !template.contains("{rom}") {
            argv.push(rom_path.to_string());
        }
        return Ok((exe, argv));
    }

    let retroarch = db::get_setting(conn, "retroarch_path")?
        .map(|p| crate::detect::clean_path(&p))
        .filter(|p| !p.is_empty())
        .ok_or_else(|| {
            AppError::Other("RetroArch path is not set — add it in Settings".into())
        })?;
    let ra_path = PathBuf::from(&retroarch);
    if !ra_path.exists() {
        return Err(AppError::Other(format!(
            "RetroArch not found at: {retroarch} — check the path in Settings, \
             or press Detect RetroArch"
        )));
    }

    let core = cfg.core.filter(|c| !c.is_empty()).ok_or_else(|| {
        let name = platforms::display_name(&game.platform);
        // Some systems have no libretro core in existence, so pointing at one
        // is not the fix — a standalone emulator is.
        if platforms::by_slug(&game.platform).map_or(false, |p| p.cores.is_empty()) {
            AppError::Other(format!(
                "{name} has no libretro core. Point it at a standalone emulator                  in Settings -> Emulators."
            ))
        } else {
            AppError::Other(format!("No libretro core set for {name}"))
        }
    })?;

    let cores_dir = db::get_setting(conn, "retroarch_cores_dir")?
        .map(|p| crate::detect::clean_path(&p))
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| default_cores_dir(&ra_path))
        .ok_or_else(|| {
            AppError::Other("RetroArch cores folder is not set — add it in Settings".into())
        })?;

    let core_path = cores_dir.join(core_filename(&core));
    if !core_path.exists() {
        return Err(AppError::Other(format!(
            "Core not found: {}",
            core_path.to_string_lossy()
        )));
    }

    Ok((
        retroarch,
        vec![
            "-L".into(),
            core_path.to_string_lossy().to_string(),
            rom_path.to_string(),
        ],
    ))
}

/// Start a game and record the session when it exits.
pub fn launch(
    app: &AppHandle,
    conn: &Mutex<rusqlite::Connection>,
    game: &Game,
    cache_root: &Path,
) -> Result<()> {
    if !Path::new(&game.path).exists() {
        return Err(AppError::Other(format!("ROM is missing: {}", game.path)));
    }

    let (exe, args) = {
        let guard = conn.lock().unwrap();

        // Unpack the ROM first if the emulator cannot read the archive it
        // lives in. Asking the core what it accepts beats guessing: Dolphin
        // takes a .wbfs but not the .7z holding it.
        let accepts = db::get_setting(&guard, "retroarch_path")?
            .map(|p| crate::detect::clean_path(&p))
            .filter(|p| !p.is_empty())
            .and_then(|p| {
                let core = resolve_config(&guard, &game.platform).core?;
                core_extensions(Path::new(&p), &core)
            });
        let rom = playable_path(game, cache_root, accepts.as_deref())?;

        let command = build_command(&guard, game, &rom)?;

        // Put any switched-on cheats where RetroArch will find them, now,
        // while it is definitely not running. Best effort: a game with no
        // cheats, a system with no core, or a RetroArch we cannot find are
        // all ordinary, and none of them is a reason to refuse to play.
        let _ = crate::cheats::sync_to_retroarch(&guard, game);

        command
    };

    let mut cmd = Command::new(&exe);
    cmd.args(&args);
    if let Some(dir) = Path::new(&exe).parent() {
        if dir.is_dir() {
            cmd.current_dir(dir);
        }
    }

    let child = cmd
        .spawn()
        .map_err(|e| AppError::Other(format!("Could not start {exe}: {e}")))?;

    let started_at = db::now();
    let game_id = game.id;
    let title = game.title.clone();
    let app_handle = app.clone();
    // An owned handle, so the waiting thread does not borrow the app.
    let db_handle = app.state::<crate::db::Db>().0.clone();

    let _ = app.emit(
        "game-launched",
        serde_json::json!({ "gameId": game_id, "title": title }),
    );

    // Wait off-thread so the UI stays responsive for the whole session.
    std::thread::spawn(move || {
        let mut child = child;
        let _ = child.wait();
        let seconds = (db::now() - started_at).max(0);

        if let Ok(guard) = db_handle.lock() {
            let _ = db::record_play(&guard, game_id, started_at, seconds);
        }

        let _ = app_handle.emit(
            "game-exited",
            serde_json::json!({
                "gameId": game_id,
                "title": title,
                "seconds": seconds,
            }),
        );
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "playdex-launch-{}-{}-{:?}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn game_at(path: &Path, inner: Option<&str>) -> Game {
        Game {
            id: 1,
            path: path.to_string_lossy().to_string(),
            filename: path.file_name().unwrap().to_string_lossy().to_string(),
            platform: "wii".into(),
            size: 0,
            crc32: Some("8c246e48".into()),
            md5: None,
            sha1: None,
            inner_name: inner.map(|s| s.to_string()),
            ..Default::default()
        }
    }

    /// RetroArch ships the answer next to every core; read it rather than
    /// guessing what the core will take.
    #[test]
    fn reads_a_cores_own_extension_list() {
        let dir = tmp("info");
        let exe = dir.join("retroarch.exe");
        std::fs::write(&exe, b"stub").unwrap();
        std::fs::create_dir_all(dir.join("info")).unwrap();
        std::fs::write(
            dir.join("info").join("dolphin_libretro.info"),
            "display_name = \"Nintendo - GameCube / Wii (Dolphin)\"
             supported_extensions = \"gcm|iso|wbfs|ciso|gcz|elf|dol|dff|tgc|wad|rvz|m3u|wia\"
",
        )
        .unwrap();

        let exts = core_extensions(&exe, "dolphin_libretro").expect("info file should parse");
        assert!(exts.contains(&"wbfs".to_string()));
        assert!(exts.contains(&"rvz".to_string()));
        // The point of the whole exercise: no archive format is on that list.
        assert!(!exts.contains(&"7z".to_string()));
        assert!(!exts.contains(&"zip".to_string()));

        assert!(core_extensions(&exe, "not_a_core").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A 7z of a Wii disc has to come out of the archive; a zip of a NES ROM
    /// does not, because RetroArch unpacks that itself for a core that says
    /// it accepts one.
    #[test]
    fn only_unpacks_what_the_core_cannot_read() {
        let dolphin: Vec<String> = ["gcm", "iso", "wbfs", "rvz"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let nestopia: Vec<String> = ["nes", "fds", "unf", "zip"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let wii = game_at(Path::new("C:/roms/New Super Mario Bros Wii.7z"), None);
        assert!(needs_extracting(&wii, Some(&dolphin)));

        let nes = game_at(Path::new("C:/roms/Super Mario Bros.zip"), None);
        assert!(!needs_extracting(&nes, Some(&nestopia)));

        // A zip handed to a core that does not list zip still has to come out.
        assert!(needs_extracting(&nes, Some(&dolphin)));

        // A plain ROM is never unpacked.
        let plain = game_at(Path::new("C:/roms/Super Mario Bros.nes"), None);
        assert!(!needs_extracting(&plain, Some(&nestopia)));

        // With no info file to consult, only 7z is assumed unreadable.
        assert!(needs_extracting(&wii, None));
        assert!(!needs_extracting(&nes, None));
    }

    /// The whole path: an archived ROM is unpacked once and the emulator is
    /// handed the real file, not the archive.
    #[test]
    fn hands_the_emulator_the_unpacked_rom() {
        let dir = tmp("extract");
        let cache = dir.join("cache");

        let archive = dir.join("New Super Mario Bros Wii [SMNE01].7z");
        let payload: Vec<u8> = (0..8192).map(|i| (i * 31 % 251) as u8).collect();
        {
            let mut w = sevenz_rust2::ArchiveWriter::create(&archive).unwrap();
            w.push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("New Super Mario Bros Wii [SMNE01].wbfs"),
                Some(payload.as_slice()),
            )
            .unwrap();
            w.finish().unwrap();
        }

        let game = game_at(&archive, Some("New Super Mario Bros Wii [SMNE01].wbfs"));
        let dolphin: Vec<String> = ["wbfs", "iso"].iter().map(|s| s.to_string()).collect();

        let out = playable_path(&game, &cache, Some(&dolphin)).unwrap();
        assert!(out.ends_with(".wbfs"), "got {out}");
        assert_eq!(std::fs::read(&out).unwrap(), payload);

        // The preview says the same thing without unpacking anything.
        assert_eq!(preview_rom_path(&game, &cache, Some(&dolphin)), out);

        // Unpacking again is free: the file is already the right size.
        let again = playable_path(&game, &cache, Some(&dolphin)).unwrap();
        assert_eq!(again, out);

        // Nothing is left behind half-written.
        let leftovers: Vec<String> = std::fs::read_dir(Path::new(&out).parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".partial"))
            .collect();
        assert!(leftovers.is_empty(), "partial files left: {leftovers:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}

