//! TheGamesDB API v1 — the fallback provider.
//!
//! It matches on name rather than hash, so results are ranked against the
//! cleaned-up title and, where possible, checked against the platform we
//! detected. Its free key has a monthly allowance, reported on every response.

use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::OnceCell;

use super::{Credentials, GameMeta, Outcome};
use crate::models::Game;
use crate::platforms;

const SEARCH: &str = "https://api.thegamesdb.net/v1.1/Games/ByGameName";
const BASE_V1: &str = "https://api.thegamesdb.net/v1";

static DEVELOPERS: OnceCell<HashMap<i64, String>> = OnceCell::const_new();
static PUBLISHERS: OnceCell<HashMap<i64, String>> = OnceCell::const_new();
static GENRES: OnceCell<HashMap<i64, String>> = OnceCell::const_new();
static TGDB_PLATFORMS: OnceCell<HashMap<i64, String>> = OnceCell::const_new();

/// Fetch one of TheGamesDB's id→name tables. These are small and change
/// rarely, so they are fetched once per app run.
async fn fetch_id_map(
    client: &reqwest::Client,
    key: &str,
    endpoint: &str,
    node: &str,
) -> HashMap<i64, String> {
    let url = format!("{BASE_V1}/{endpoint}");
    let Ok(resp) = client.get(&url).query(&[("apikey", key)]).send().await else {
        return HashMap::new();
    };
    let Ok(json) = resp.json::<Value>().await else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    if let Some(obj) = json
        .get("data")
        .and_then(|d| d.get(node))
        .and_then(|g| g.as_object())
    {
        for (id, entry) in obj {
            if let (Ok(id), Some(name)) = (
                id.parse::<i64>(),
                entry.get("name").and_then(|n| n.as_str()),
            ) {
                out.insert(id, name.to_string());
            }
        }
    }
    out
}

async fn developers(client: &reqwest::Client, key: &str) -> &'static HashMap<i64, String> {
    DEVELOPERS
        .get_or_init(|| fetch_id_map(client, key, "Developers", "developers"))
        .await
}

async fn publishers(client: &reqwest::Client, key: &str) -> &'static HashMap<i64, String> {
    PUBLISHERS
        .get_or_init(|| fetch_id_map(client, key, "Publishers", "publishers"))
        .await
}

async fn genres(client: &reqwest::Client, key: &str) -> &'static HashMap<i64, String> {
    GENRES
        .get_or_init(|| fetch_id_map(client, key, "Genres", "genres"))
        .await
}

async fn tgdb_platforms(client: &reqwest::Client, key: &str) -> &'static HashMap<i64, String> {
    TGDB_PLATFORMS
        .get_or_init(|| fetch_id_map(client, key, "Platforms", "platforms"))
        .await
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Higher is better. Exact title match on the right platform wins.
fn score(candidate: &str, wanted: &str, platform_matches: bool) -> i32 {
    let c = normalize(candidate);
    let w = normalize(wanted);
    let mut s = if c == w {
        100
    } else if c.starts_with(&w) || w.starts_with(&c) {
        70
    } else if c.contains(&w) || w.contains(&c) {
        45
    } else {
        0
    };
    if platform_matches {
        s += 30;
    }
    s
}

fn names_for(ids: Option<&Value>, table: &HashMap<i64, String>) -> Option<String> {
    let arr = ids?.as_array()?;
    let names: Vec<String> = arr
        .iter()
        .filter_map(|v| v.as_i64())
        .filter_map(|id| table.get(&id).cloned())
        .collect();
    if names.is_empty() {
        None
    } else {
        Some(names.join(", "))
    }
}

pub async fn lookup(client: &reqwest::Client, creds: &Credentials, game: &Game) -> Outcome {
    let wanted = game.title.clone();
    if wanted.trim().is_empty() {
        return Outcome::NotFound;
    }

    let resp = match client
        .get(SEARCH)
        .query(&[
            ("apikey", creds.tgdb_key.as_str()),
            ("name", wanted.as_str()),
            ("fields", "players,publishers,genres,overview,rating"),
            ("include", "boxart"),
        ])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return Outcome::Error(format!("request failed: {e}")),
    };

    let status = resp.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Outcome::Error("API key rejected".into());
    }
    if status.as_u16() == 429 {
        return Outcome::Quota("rate limited".into());
    }

    let json: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return Outcome::Error(format!("unreadable response: {e}")),
    };

    if json
        .get("remaining_monthly_allowance")
        .and_then(|v| v.as_i64())
        == Some(0)
    {
        return Outcome::Quota("monthly allowance exhausted".into());
    }

    let Some(games) = json
        .get("data")
        .and_then(|d| d.get("games"))
        .and_then(|g| g.as_array())
    else {
        return Outcome::NotFound;
    };
    if games.is_empty() {
        return Outcome::NotFound;
    }

    let plat_table = tgdb_platforms(client, &creds.tgdb_key).await;

    // Rank candidates by title closeness, boosted when the platform agrees.
    let mut best: Option<(&Value, i32)> = None;
    for g in games {
        let title = g.get("game_title").and_then(|t| t.as_str()).unwrap_or("");
        let platform_matches = g
            .get("platform")
            .and_then(|p| p.as_i64())
            .and_then(|id| plat_table.get(&id))
            .and_then(|name| platforms::match_alias(name))
            .map(|p| p.slug == game.platform)
            .unwrap_or(false);
        let s = score(title, &wanted, platform_matches);
        if best.map_or(true, |(_, bs)| s > bs) {
            best = Some((g, s));
        }
    }

    let Some((hit, best_score)) = best else {
        return Outcome::NotFound;
    };
    // A name-only provider guessing wildly is worse than no metadata.
    if best_score < 45 {
        return Outcome::NotFound;
    }

    let devs = developers(client, &creds.tgdb_key).await;
    let pubs = publishers(client, &creds.tgdb_key).await;
    let gens = genres(client, &creds.tgdb_key).await;

    let game_id = hit.get("id").and_then(|i| i.as_i64());
    let cover_url = game_id.and_then(|id| boxart_url(&json, id));

    let meta = GameMeta {
        title: hit
            .get("game_title")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string()),
        description: hit
            .get("overview")
            .and_then(|t| t.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        developer: names_for(hit.get("developers"), devs),
        publisher: names_for(hit.get("publishers"), pubs),
        genre: names_for(hit.get("genres"), gens),
        release_date: hit
            .get("release_date")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string()),
        players: hit.get("players").and_then(|p| p.as_i64()).map(|p| p.to_string()),
        // TheGamesDB's "rating" is an age rating, not a score.
        rating: None,
        cover_url,
        screenshot_url: None,
        logo_url: None,
    };

    Outcome::Found(Box::new(meta))
}

/// Build the front boxart URL out of the `include.boxart` block.
fn boxart_url(json: &Value, game_id: i64) -> Option<String> {
    let boxart = json.get("include")?.get("boxart")?;
    let base = boxart
        .get("base_url")?
        .get("original")
        .and_then(|b| b.as_str())?;
    let list = boxart
        .get("data")?
        .get(game_id.to_string())?
        .as_array()?;

    let front = list
        .iter()
        .find(|i| {
            i.get("side").and_then(|s| s.as_str()) == Some("front")
                && i.get("type").and_then(|t| t.as_str()) == Some("boxart")
        })
        .or_else(|| list.first())?;

    let filename = front.get("filename").and_then(|f| f.as_str())?;
    Some(format!("{}{}", base.trim_end_matches('/'), format!("/{}", filename.trim_start_matches('/'))))
}
