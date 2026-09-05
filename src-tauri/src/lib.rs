mod cheats;
mod commands;
mod db;
mod detect;
mod error;
mod hacks;
mod homebrew;
mod hashing;
mod launch;
mod media;
mod models;
mod patch;
mod platforms;
mod romcheck;
mod saves;
mod scan;
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
            std::fs::create_dir_all(&data_dir)?;

            let conn = db::open(&data_dir.join("library.db"))?;
            app.manage(db::Db(std::sync::Arc::new(Mutex::new(conn))));

            let media_root = data_dir.clone();
            std::fs::create_dir_all(media_root.join("media"))?;

            let client = reqwest::Client::builder()
                .user_agent(concat!("playdex/", env!("CARGO_PKG_VERSION")))
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
            commands::inspect_patch,
            commands::add_hack,
            commands::apply_catalog_patch,
            commands::import_patches,
            commands::patches_for_game,
            commands::list_patches,
            commands::patch_catalog_size,
            commands::clear_patch_catalog,
            commands::search_homebrew,
            commands::homebrew_collections,
            commands::homebrew_files,
            commands::install_homebrew,
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
