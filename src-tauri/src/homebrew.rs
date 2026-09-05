//! Homebrew browser: search the Internet Archive's homebrew collections and
//! pull games straight into the library.
//!
//! The search is deliberately scoped to an allowlist of collections that hold
//! hobbyist work published freely by its authors. The Internet Archive also
//! hosts emulated *commercial* libraries — "Console Living Room", the MS-DOS
//! software library and so on — and those are intentionally not reachable from
//! here. Widening this list is a licensing decision, not a convenience one.

use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::error::{AppError, Result};
use crate::models::{HomebrewFile, HomebrewItem};
use crate::platforms;

const SEARCH: &str = "https://archive.org/advancedsearch.php";
const METADATA: &str = "https://archive.org/metadata";
const DOWNLOAD: &str = "https://archive.org/download";

/// Collection identifier, friendly name, and the platform its items are for
/// when the collection is single-system.
///
/// This list is short on purpose. Several Internet Archive collections are
/// named "homebrew" but are not, and were checked and rejected:
///
/// - `psp-homebrew-library` — 3,950 items, but the popular ones are fan ports
///   of commercial games and it carries a copy of Sony's PSP BIOS.
/// - `the-homebrew-cloud` — Switch custom firmware and piracy tooling.
/// - `atari_7800_homebrew` — overwhelmingly "(Hack)" entries, which are
///   derivative works built on commercial ROMs.
/// - `ps2-homebrew-library` — loaders and cheat devices rather than games.
/// - `psx-homebrew-library` — real Net Yaroze homebrew mixed with a BIOS
///   dumper.
///
/// Adding a collection here is a licensing decision. Check what is actually
/// inside it first; the name is not evidence.
const COLLECTIONS: &[(&str, &str, Option<&str>)] = &[
    ("spahomebrew", "Spanish Homebrew Archive", None),
    ("doshaven-homebrew", "DOS Haven", Some("dos")),
];

/// Anything above this is not a homebrew ROM and we refuse to fetch it.
const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;

pub fn collection_names() -> Vec<String> {
    COLLECTIONS.iter().map(|(_, name, _)| name.to_string()).collect()
}

fn platform_for_collection(collection: &str) -> Option<&'static str> {
    COLLECTIONS
        .iter()
        .find(|(id, _, _)| *id == collection)
        .and_then(|(_, _, slug)| *slug)
}

/// Strip characters that would change the meaning of a Lucene query. Users
/// type game names here, not query syntax.
fn sanitize_query(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            ':' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | '^' | '~' | '\\' | '/' | '+' | '!'
            | '?' | '*' | '|' | '&' => ' ',
            c => c,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn string_field(doc: &Value, key: &str) -> Option<String> {
    match doc.get(key)? {
        Value::String(s) => Some(s.clone()),
        // Several fields come back as arrays when an item has more than one.
        Value::Array(a) => a.first().and_then(|v| v.as_str()).map(|s| s.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Which collection from our allowlist this item belongs to.
fn allowlisted_collection(doc: &Value) -> Option<String> {
    let ids: Vec<String> = match doc.get("collection") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.to_string())
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    };
    ids.into_iter()
        .find(|c| COLLECTIONS.iter().any(|(id, _, _)| id == c))
}

/// Titles in some collections are tagged with the system, e.g. "Cray 5 [ZX]".
fn platform_from_title(title: &str) -> Option<String> {
    let start = title.rfind('[')?;
    let end = title[start..].find(']')? + start;
    let tag = &title[start + 1..end];
    platforms::match_alias(tag).map(|p| p.slug.to_string())
}

pub async fn search(
    client: &reqwest::Client,
    query: &str,
    page: i64,
) -> Result<Vec<HomebrewItem>> {
    let scope = COLLECTIONS
        .iter()
        .map(|(id, _, _)| format!("collection:({id})"))
        .collect::<Vec<_>>()
        .join(" OR ");

    let cleaned = sanitize_query(query);
    let q = if cleaned.is_empty() {
        format!("({scope})")
    } else {
        format!("({scope}) AND ({cleaned})")
    };

    let params: Vec<(&str, String)> = vec![
        ("q", q),
        ("fl[]", "identifier".into()),
        ("fl[]", "title".into()),
        ("fl[]", "creator".into()),
        ("fl[]", "description".into()),
        ("fl[]", "year".into()),
        ("fl[]", "licenseurl".into()),
        ("fl[]", "downloads".into()),
        ("fl[]", "collection".into()),
        ("sort[]", "downloads desc".into()),
        ("rows", "48".into()),
        ("page", page.max(1).to_string()),
        ("output", "json".into()),
    ];

    let resp = client
        .get(SEARCH)
        .query(&params)
        .send()
        .await
        .map_err(|e| AppError::Other(format!("Search failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::Other(format!(
            "Internet Archive returned {}",
            resp.status()
        )));
    }

    let json: Value = resp
        .json()
        .await
        .map_err(|e| AppError::Other(format!("Unreadable search response: {e}")))?;

    let docs = json
        .get("response")
        .and_then(|r| r.get("docs"))
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::new();
    for doc in docs {
        // Belt and braces: never surface an item outside the allowlist, even
        // if the query somehow matched one.
        let Some(collection) = allowlisted_collection(&doc) else {
            continue;
        };
        let Some(identifier) = string_field(&doc, "identifier") else {
            continue;
        };
        let title = string_field(&doc, "title").unwrap_or_else(|| identifier.clone());

        let platform = platform_from_title(&title)
            .or_else(|| platform_for_collection(&collection).map(|s| s.to_string()));

        out.push(HomebrewItem {
            identifier,
            title,
            creator: string_field(&doc, "creator"),
            description: string_field(&doc, "description"),
            year: string_field(&doc, "year"),
            license: string_field(&doc, "licenseurl"),
            collection,
            platform,
        });
    }

    Ok(out)
}

/// The downloadable ROM files in an item, plus a screenshot to use as art.
pub async fn item_files(
    client: &reqwest::Client,
    identifier: &str,
) -> Result<(Vec<HomebrewFile>, Option<String>)> {
    let url = format!("{METADATA}/{identifier}");
    let json: Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::Other(format!("Could not read item: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Other(format!("Unreadable item metadata: {e}")))?;

    let files = json
        .get("files")
        .and_then(|f| f.as_array())
        .cloned()
        .unwrap_or_default();

    let mut roms = Vec::new();
    let mut best_image: Option<(String, u64)> = None;

    for f in &files {
        let Some(name) = f.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        let ext = Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let size = f
            .get("size")
            .and_then(|s| s.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        // The archive's own `format` field is unreliable — it labels .nes
        // files as audio — so go by extension against our platform table.
        if platforms::is_indexable_ext(&ext) {
            roms.push(HomebrewFile {
                name: name.to_string(),
                size: size as i64,
                url: format!("{DOWNLOAD}/{identifier}/{}", urlencoding::encode(name)),
            });
        } else if matches!(ext.as_str(), "jpg" | "jpeg" | "png")
            && !name.contains("_thumb")
            && !name.starts_with("__ia")
        {
            if best_image.as_ref().map_or(true, |(_, s)| size > *s) {
                best_image = Some((name.to_string(), size));
            }
        }
    }

    let image = best_image
        .map(|(name, _)| format!("{DOWNLOAD}/{identifier}/{}", urlencoding::encode(&name)));

    Ok((roms, image))
}

/// Fetch one file into `dest_dir`, returning where it landed.
pub async fn download_file(
    client: &reqwest::Client,
    url: &str,
    file_name: &str,
    dest_dir: &Path,
) -> Result<PathBuf> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Other(format!("Download failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(AppError::Other(format!(
            "Download failed with {}",
            resp.status()
        )));
    }

    if let Some(len) = resp.content_length() {
        if len > MAX_DOWNLOAD_BYTES {
            return Err(AppError::Other(
                "That file is far larger than a homebrew ROM should be".into(),
            ));
        }
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::Other(format!("Download interrupted: {e}")))?;

    if bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
        return Err(AppError::Other("Downloaded file is too large".into()));
    }

    std::fs::create_dir_all(dest_dir)?;
    let safe = sanitize_file_name(file_name);
    let dest = dest_dir.join(safe);
    std::fs::write(&dest, &bytes)?;
    Ok(dest)
}

/// Archive file names are remote input, so flatten them to a single component.
fn sanitize_file_name(name: &str) -> String {
    let base = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("download")
        .trim()
        .trim_matches('.');
    let cleaned: String = base
        .chars()
        .map(|c| if r#":*?"<>|"#.contains(c) { '_' } else { c })
        .collect();
    if cleaned.is_empty() {
        "download".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_query_syntax() {
        assert_eq!(sanitize_query("  mario   world "), "mario world");
        assert_eq!(sanitize_query("collection:(evil) OR x"), "collection evil OR x");
        assert_eq!(sanitize_query("a\"b\\c"), "a b c");
    }

    #[test]
    fn reads_platform_tag_from_title() {
        assert_eq!(platform_from_title("Cheril Writer [NES]").as_deref(), Some("nes"));
        assert_eq!(platform_from_title("InterNestor Suite [MSX]").as_deref(), Some("msx"));
        assert_eq!(platform_from_title("No tag here"), None);
    }

    #[test]
    fn maps_single_system_collections() {
        assert_eq!(platform_for_collection("doshaven-homebrew"), Some("dos"));
        // Multi-system collections fall back to the title tag instead.
        assert_eq!(platform_for_collection("spahomebrew"), None);
    }

    #[test]
    fn only_allowlisted_collections_pass() {
        let good = serde_json::json!({ "collection": ["spahomebrew", "vintagesoftware"] });
        assert_eq!(allowlisted_collection(&good).as_deref(), Some("spahomebrew"));

        // Neither the emulated commercial libraries nor the "homebrew"
        // collections that were checked and rejected may be surfaced.
        let bad = serde_json::json!({ "collection": ["gamegear_library", "softwarelibrary"] });
        assert_eq!(allowlisted_collection(&bad), None);

        let rejected = serde_json::json!({
            "collection": ["psp-homebrew-library", "the-homebrew-cloud", "emulation"]
        });
        assert_eq!(allowlisted_collection(&rejected), None);
    }

    #[test]
    fn file_names_cannot_escape_the_destination() {
        assert_eq!(sanitize_file_name("../../evil.nes"), "evil.nes");
        assert_eq!(sanitize_file_name("a/b/rom.gba"), "rom.gba");
        assert_eq!(sanitize_file_name(""), "download");
    }
}
