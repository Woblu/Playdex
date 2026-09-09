mod cheats;
mod commands;
mod dats;
mod db;
mod discs;
mod detect;
mod error;
mod hacks;
mod hashing;
mod launch;
mod media;
mod models;
mod patch;
mod platforms;
mod romcheck;
mod saves;
mod scan;
mod signature;
mod scrape;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use tauri::{Manager, State};

pub struct AppState {
    pub client: reqwest::Client,
    pub media_root: PathBuf,
    pub scrape_running: AtomicBool,
    pub scrape_cancel: AtomicBool,
}

/// Turn a cached artwork path into a URL the webview can load.
/// The frontend calls `convertFileSrc(path, 'media')` to build the other half.
fn media_protocol_response(
    media_root: &std::path::Path,
    uri_path: &str,
) -> tauri::http::Response<Vec<u8>> {
    let not_found = || {
        tauri::http::Response::builder()
            .status(404)
            .body(Vec::new())
            .unwrap()
    };

    let decoded = match urlencoding::decode(uri_path.trim_start_matches('/')) {
        Ok(d) => d.into_owned(),
        Err(_) => return not_found(),
    };

    let path = PathBuf::from(decoded);

    // This protocol only ever serves artwork we downloaded ourselves.
    let inside_cache = path
        .canonicalize()
        .ok()
        .zip(media_root.canonicalize().ok())
        .map(|(p, root)| p.starts_with(root))
        .unwrap_or(false);
    if !inside_cache {
        return not_found();
    }

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return not_found(),
    };

    let mime = match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/jpeg",
    };

    tauri::http::Response::builder()
        .header("Content-Type", mime)
        .header("Cache-Control", "max-age=86400")
        .body(bytes)
        .unwrap()
}

#[tauri::command]
fn app_paths(state: State<AppState>) -> serde_json::Value {
    serde_json::json!({
        "mediaRoot": state.media_root.to_string_lossy(),
    })
}

/// The bundle identifier Playdex shipped under, up to and including 0.8.1.
///
/// The app data directory is derived from the identifier, so renaming the app
/// to Romcade moves it. Everything anyone has — the library database, their
/// playtime, the artwork cache and the generated disc playlists — lives in
/// that folder, so the first launch after updating carries it across instead
/// of opening an empty library.
const LEGACY_IDENTIFIER: &str = "com.boazv.playdex";

/// Moves `old` to `new` if, and only if, there is something to move and
/// nothing already at the destination.
///
/// Refusing to run when `new` exists is the whole safety argument: a Romcade
/// library that is already in place can never be overwritten by a stale
/// Playdex one, whatever order things happened in. Returns whether it moved
/// anything.
///
/// Split out from the identifier lookup so it can be tested without standing
/// up a Tauri app handle.
fn move_data_dir(old: &std::path::Path, new: &std::path::Path) -> std::io::Result<bool> {
    if new.exists() || !old.is_dir() {
        return Ok(false);
    }
    if let Some(parent) = new.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(old, new)?;
    Ok(true)
}

/// Carries a pre-rename library across, if one is sitting next door.
///
/// A failure here is reported and then dropped. Being unable to move the old
/// folder is a bad first launch; refusing to start at all is a worse one, and
/// the old directory is left untouched either way, so nothing is lost that a
/// later attempt could not still recover.
fn migrate_legacy_data_dir(new_dir: &std::path::Path) {
    let Some(parent) = new_dir.parent() else {
        return;
    };
    let old_dir = parent.join(LEGACY_IDENTIFIER);
    match move_data_dir(&old_dir, new_dir) {
        Ok(true) => eprintln!(
            "carried the library over from {} to {}",
            old_dir.display(),
            new_dir.display()
        ),
        Ok(false) => {}
        Err(e) => eprintln!("could not carry the old Playdex library across: {e}"),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("media", |ctx, request| {
            let state = ctx.app_handle().state::<AppState>();
            media_protocol_response(&state.media_root, request.uri().path())
        })
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("no app data directory available");
            // Before anything creates the new directory, since the move only
            // happens when there is nothing at the destination.
            migrate_legacy_data_dir(&data_dir);
            std::fs::create_dir_all(&data_dir)?;

            let conn = db::open(&data_dir.join("library.db"))?;

            // Moving the folder is only half of it. Artwork and patched ROMs
            // are recorded as absolute paths into that folder, so they have to
            // be repointed as well or the library comes up with every cover
            // broken. This runs for anyone who took 0.9.0 before the paths
            // were fixed, not only at the moment the folder moves.
            if let Some(current) = data_dir.file_name().and_then(|name| name.to_str()) {
                match db::repoint_data_dir(&conn, LEGACY_IDENTIFIER, current) {
                    Ok(0) => {}
                    Ok(n) => eprintln!("repointed {n} stored paths after the rename"),
                    Err(e) => eprintln!("could not repoint stored paths: {e}"),
                }
            }

            app.manage(db::Db(std::sync::Arc::new(Mutex::new(conn))));

            let media_root = data_dir.clone();
            std::fs::create_dir_all(media_root.join("media"))?;

            let client = reqwest::Client::builder()
                .user_agent(concat!("romcade/", env!("CARGO_PKG_VERSION")))
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("could not build http client");

            app.manage(AppState {
                client,
                media_root,
                scrape_running: AtomicBool::new(false),
                scrape_cancel: AtomicBool::new(false),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_paths,
            commands::add_dropped,
            commands::import_dat,
            commands::dat_status,
            commands::clear_dats,
            commands::verify_game,
            commands::cache_usage,
            commands::clear_cache,
            commands::unpack_in_place,
            commands::list_games,
            commands::get_game,
            commands::list_platforms,
            commands::known_platforms,
            commands::library_stats,
            commands::library_insights,
            commands::set_favorite,
            commands::set_hidden,
            commands::set_game_platform,
            commands::remove_game,
            commands::list_library_folders,
            commands::add_library_folder,
            commands::remove_library_folder,
            commands::pick_folder,
            commands::pick_file,
            commands::scan_library,
            commands::clean_missing,
            commands::scrape_library,
            commands::scrape_one,
            commands::cancel_scrape,
            commands::launch_game,
            commands::preview_launch,
            commands::reveal_game,
            commands::get_settings,
            commands::save_settings,
            commands::list_emulators,
            commands::save_emulator,
            commands::effective_emulator,
            commands::disc_members,
            commands::game_emulator,
            commands::save_game_emulator,
            commands::inspect_patch,
            commands::add_hack,
            commands::apply_catalog_patch,
            commands::import_patches,
            commands::patches_for_game,
            commands::list_patches,
            commands::patch_catalog_size,
            commands::clear_patch_catalog,
            commands::find_cheats,
            commands::list_cheats,
            commands::set_cheat,
            commands::set_all_cheats,
            commands::save_cheats,
            commands::retroarch_cheat_status,
            commands::enable_auto_apply_cheats,
            commands::list_hack_bundles,
            commands::download_hack_bundle,
            commands::list_saves,
            commands::back_up_saves,
            commands::delete_save_state,
            commands::detect_retroarch,
            commands::test_credentials,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


#[cfg(test)]
mod migration_tests {
    use super::move_data_dir;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "romcade-migrate-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn carries_an_old_library_across() {
        let base = scratch("move");
        let old = base.join("com.boazv.playdex");
        let new = base.join("com.boazv.romcade");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("library.db"), b"library").unwrap();

        assert!(move_data_dir(&old, &new).unwrap());
        assert_eq!(std::fs::read(new.join("library.db")).unwrap(), b"library");
        assert!(!old.exists());

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn refuses_to_overwrite_an_existing_library() {
        let base = scratch("keep");
        let old = base.join("com.boazv.playdex");
        let new = base.join("com.boazv.romcade");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("library.db"), b"old").unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("library.db"), b"new").unwrap();

        assert!(!move_data_dir(&old, &new).unwrap());
        assert_eq!(std::fs::read(new.join("library.db")).unwrap(), b"new");
        assert!(old.exists());

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn does_nothing_on_a_fresh_install() {
        let base = scratch("fresh");
        let new = base.join("com.boazv.romcade");
        assert!(!move_data_dir(&base.join("com.boazv.playdex"), &new).unwrap());
        assert!(!new.exists());
    }
}
