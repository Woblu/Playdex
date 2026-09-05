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

/// Build the command for a game without running it — also used by the UI to
/// show what will be executed.
pub fn build_command(
    conn: &rusqlite::Connection,
    game: &Game,
) -> Result<(String, Vec<String>)> {
    let cfg = resolve_config(conn, &game.platform);

    if cfg.mode == "custom" {
        let template = cfg
            .custom_command
            .filter(|c| !c.is_empty())
            .ok_or_else(|| AppError::Other(format!("No emulator set for {}", game.platform)))?;
        let filled = template.replace("{rom}", &game.path);
        let mut argv = tokenize(&filled);
        if argv.is_empty() {
            return Err(AppError::Other("Emulator command is empty".into()));
        }
        let exe = argv.remove(0);
        // A template with no {rom} still needs the ROM passed somewhere.
        if !template.contains("{rom}") {
            argv.push(game.path.clone());
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
        AppError::Other(format!(
            "No libretro core set for {}",
            platforms::display_name(&game.platform)
        ))
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
            game.path.clone(),
        ],
    ))
}

/// Start a game and record the session when it exits.
pub fn launch(app: &AppHandle, conn: &Mutex<rusqlite::Connection>, game: &Game) -> Result<()> {
    if !Path::new(&game.path).exists() {
        return Err(AppError::Other(format!("ROM is missing: {}", game.path)));
    }

    let (exe, args) = {
        let guard = conn.lock().unwrap();
        build_command(&guard, game)?
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
