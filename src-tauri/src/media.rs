//! Downloads and caches artwork under the app data directory.

use std::path::{Path, PathBuf};

use crate::error::Result;

/// Where a game's artwork lives.
pub fn game_media_dir(root: &Path, game_id: i64) -> PathBuf {
    root.join("media").join(game_id.to_string())
}

fn extension_for(url: &str, content_type: Option<&str>) -> &'static str {
    if let Some(ct) = content_type {
        if ct.contains("png") {
            return "png";
        }
        if ct.contains("jpeg") || ct.contains("jpg") {
            return "jpg";
        }
        if ct.contains("webp") {
            return "webp";
        }
        if ct.contains("gif") {
            return "gif";
        }
    }
    let lower = url.to_ascii_lowercase();
    if lower.contains(".png") {
        "png"
    } else if lower.contains(".webp") {
        "webp"
    } else if lower.contains(".gif") {
        "gif"
    } else {
        "jpg"
    }
}

/// Fetch one image into the cache. Returns the file path, or None when the
/// request failed or came back as something that is not an image.
pub async fn download_image(
    client: &reqwest::Client,
    url: &str,
    media_root: &Path,
    game_id: i64,
    kind: &str,
) -> Result<Option<String>> {
    if url.trim().is_empty() {
        return Ok(None);
    }

    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    if !resp.status().is_success() {
        return Ok(None);
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // ScreenScraper answers quota problems with a text/plain body and a 200.
    if let Some(ct) = &content_type {
        if !ct.starts_with("image/") {
            return Ok(None);
        }
    }

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    if bytes.is_empty() {
        return Ok(None);
    }

    let dir = game_media_dir(media_root, game_id);
    std::fs::create_dir_all(&dir)?;
    let ext = extension_for(url, content_type.as_deref());
    let dest = dir.join(format!("{kind}.{ext}"));
    std::fs::write(&dest, &bytes)?;

    Ok(Some(dest.to_string_lossy().to_string()))
}

/// Delete a game's cached artwork.
pub fn clear_game_media(media_root: &Path, game_id: i64) {
    let dir = game_media_dir(media_root, game_id);
    if dir.exists() {
        let _ = std::fs::remove_dir_all(dir);
    }
}
