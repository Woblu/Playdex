//! SQLite-backed library storage.

use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;
use std::sync::Mutex;

use crate::error::Result;
use crate::models::*;
use crate::platforms;

pub struct Db(pub std::sync::Arc<Mutex<Connection>>);

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS folders (
    id                INTEGER PRIMARY KEY,
    path              TEXT    NOT NULL UNIQUE,
    platform_override TEXT,
    added_at          INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS games (
    id              INTEGER PRIMARY KEY,
    path            TEXT    NOT NULL UNIQUE,
    filename        TEXT    NOT NULL,
    platform        TEXT    NOT NULL DEFAULT 'unknown',
    size            INTEGER NOT NULL DEFAULT 0,
    crc32           TEXT,
    md5             TEXT,
    sha1            TEXT,
    inner_name      TEXT,
    title           TEXT    NOT NULL,
    description     TEXT,
    developer       TEXT,
    publisher       TEXT,
    genre           TEXT,
    release_date    TEXT,
    players         TEXT,
    rating          REAL,
    region          TEXT,
    cover_path      TEXT,
    screenshot_path TEXT,
    logo_path       TEXT,
    scrape_status   TEXT    NOT NULL DEFAULT 'pending',
    scrape_source   TEXT,
    favorite        INTEGER NOT NULL DEFAULT 0,
    hidden          INTEGER NOT NULL DEFAULT 0,
    play_count      INTEGER NOT NULL DEFAULT 0,
    play_seconds    INTEGER NOT NULL DEFAULT 0,
    last_played     INTEGER,
    added_at        INTEGER NOT NULL,
    base_game_id    INTEGER REFERENCES games(id) ON DELETE CASCADE,
    patch_path      TEXT,
    patch_format    TEXT
);

CREATE INDEX IF NOT EXISTS idx_games_platform ON games(platform);
CREATE INDEX IF NOT EXISTS idx_games_title    ON games(title);
CREATE INDEX IF NOT EXISTS idx_games_status   ON games(scrape_status);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS emulators (
    platform       TEXT PRIMARY KEY,
    mode           TEXT NOT NULL DEFAULT 'retroarch',
    core           TEXT,
    custom_command TEXT
);

CREATE TABLE IF NOT EXISTS patches (
    id          INTEGER PRIMARY KEY,
    path        TEXT    NOT NULL UNIQUE,
    name        TEXT    NOT NULL,
    format      TEXT    NOT NULL,
    source_crc  TEXT,
    system_hint TEXT,
    target_hint TEXT,
    origin      TEXT,
    added_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_patches_crc  ON patches(source_crc);
CREATE INDEX IF NOT EXISTS idx_patches_name ON patches(name);

CREATE TABLE IF NOT EXISTS cheats (
    id          INTEGER PRIMARY KEY,
    game_id     INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    idx         INTEGER NOT NULL,
    description TEXT    NOT NULL,
    code        TEXT    NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 0,
    UNIQUE(game_id, idx)
);

CREATE TABLE IF NOT EXISTS play_sessions (
    id         INTEGER PRIMARY KEY,
    game_id    INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    seconds    INTEGER NOT NULL DEFAULT 0
);
"#;

const GAME_COLS: &str = "id, path, filename, platform, size, crc32, md5, sha1, inner_name, \
     title, description, developer, publisher, genre, release_date, players, rating, region, \
     cover_path, screenshot_path, logo_path, scrape_status, scrape_source, favorite, hidden, \
     play_count, play_seconds, last_played, added_at, base_game_id, patch_path";

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(SCHEMA)?;
    migrate(&conn)?;
    Ok(conn)
}

/// Add columns introduced after a user's database was first created.
fn migrate(conn: &Connection) -> Result<()> {
    for (column, decl) in [
        ("base_game_id", "INTEGER REFERENCES games(id) ON DELETE CASCADE"),
        ("patch_path", "TEXT"),
        ("patch_format", "TEXT"),
    ] {
        let mut stmt = conn.prepare("PRAGMA table_info(games)")?;
        let existing: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if !existing.iter().any(|c| c == column) {
            conn.execute(&format!("ALTER TABLE games ADD COLUMN {column} {decl}"), [])?;
        }
    }
    Ok(())
}

/// Record a patched ROM as its own game, linked back to the ROM it came from.
#[allow(clippy::too_many_arguments)]
pub fn insert_hack(
    conn: &Connection,
    path: &str,
    filename: &str,
    platform: &str,
    title: &str,
    size: i64,
    crc32: Option<&str>,
    base_game_id: i64,
    patch_path: &str,
    patch_format: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO games
         (path, filename, platform, title, size, crc32, added_at,
          base_game_id, patch_path, patch_format)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            path, filename, platform, title, size, crc32, now(),
            base_game_id, patch_path, patch_format
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn row_to_game(row: &Row) -> rusqlite::Result<Game> {
    Ok(Game {
        id: row.get(0)?,
        path: row.get(1)?,
        filename: row.get(2)?,
        platform: row.get(3)?,
        size: row.get(4)?,
        crc32: row.get(5)?,
        md5: row.get(6)?,
        sha1: row.get(7)?,
        inner_name: row.get(8)?,
        title: row.get(9)?,
        description: row.get(10)?,
        developer: row.get(11)?,
        publisher: row.get(12)?,
        genre: row.get(13)?,
        release_date: row.get(14)?,
        players: row.get(15)?,
        rating: row.get(16)?,
        region: row.get(17)?,
        cover_path: row.get(18)?,
        screenshot_path: row.get(19)?,
        logo_path: row.get(20)?,
        scrape_status: row.get(21)?,
        scrape_source: row.get(22)?,
        favorite: row.get::<_, i64>(23)? != 0,
        hidden: row.get::<_, i64>(24)? != 0,
        play_count: row.get(25)?,
        play_seconds: row.get(26)?,
        last_played: row.get(27)?,
        added_at: row.get(28)?,
        base_game_id: row.get(29)?,
        patch_path: row.get(30)?,
    })
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------- games

/// Insert a newly scanned ROM. Returns false when the path was already known.
#[allow(clippy::too_many_arguments)]
pub fn insert_game(
    conn: &Connection,
    path: &str,
    filename: &str,
    platform: &str,
    title: &str,
    region: Option<&str>,
    size: i64,
    crc32: Option<&str>,
    md5: Option<&str>,
    sha1: Option<&str>,
    inner_name: Option<&str>,
) -> Result<bool> {
    let changed = conn.execute(
        "INSERT OR IGNORE INTO games
         (path, filename, platform, title, region, size, crc32, md5, sha1, inner_name, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            path,
            filename,
            platform,
            title,
            region,
            size,
            crc32,
            md5,
            sha1,
            inner_name,
            now()
        ],
    )?;
    Ok(changed > 0)
}

pub fn list_games(conn: &Connection, filter: &GameFilter) -> Result<Vec<Game>> {
    let mut sql = format!("SELECT {GAME_COLS} FROM games WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if !filter.show_hidden.unwrap_or(false) {
        sql.push_str(" AND hidden = 0");
    }
    if let Some(p) = filter.platform.as_deref().filter(|p| !p.is_empty()) {
        sql.push_str(" AND platform = ?");
        args.push(Box::new(p.to_string()));
    }
    if let Some(q) = filter.search.as_deref().filter(|q| !q.trim().is_empty()) {
        sql.push_str(" AND (title LIKE ? OR filename LIKE ?)");
        let like = format!("%{}%", q.trim());
        args.push(Box::new(like.clone()));
        args.push(Box::new(like));
    }
    if filter.favorites_only.unwrap_or(false) {
        sql.push_str(" AND favorite = 1");
    }
    if filter.unscraped_only.unwrap_or(false) {
        sql.push_str(" AND scrape_status != 'ok'");
    }

    sql.push_str(match filter.sort.as_deref() {
        Some("recent") => " ORDER BY last_played IS NULL, last_played DESC, title COLLATE NOCASE",
        Some("played") => " ORDER BY play_seconds DESC, title COLLATE NOCASE",
        Some("added") => " ORDER BY added_at DESC, title COLLATE NOCASE",
        Some("rating") => " ORDER BY rating IS NULL, rating DESC, title COLLATE NOCASE",
        _ => " ORDER BY title COLLATE NOCASE",
    });

    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), row_to_game)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_game(conn: &Connection, id: i64) -> Result<Option<Game>> {
    let sql = format!("SELECT {GAME_COLS} FROM games WHERE id = ?1");
    Ok(conn
        .query_row(&sql, params![id], row_to_game)
        .optional()?)
}

/// Games that still need metadata, oldest first.
pub fn games_needing_scrape(conn: &Connection, platform: Option<&str>) -> Result<Vec<Game>> {
    let mut sql = format!(
        "SELECT {GAME_COLS} FROM games WHERE scrape_status IN ('pending', 'error')"
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(p) = platform.filter(|p| !p.is_empty()) {
        sql.push_str(" AND platform = ?");
        args.push(Box::new(p.to_string()));
    }
    sql.push_str(" ORDER BY id");
    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), row_to_game)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub struct Metadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub release_date: Option<String>,
    pub players: Option<String>,
    pub rating: Option<f64>,
    pub cover_path: Option<String>,
    pub screenshot_path: Option<String>,
    pub logo_path: Option<String>,
    pub source: String,
}

pub fn apply_metadata(conn: &Connection, id: i64, m: &Metadata) -> Result<()> {
    conn.execute(
        "UPDATE games SET
            title           = COALESCE(?2, title),
            description     = COALESCE(?3, description),
            developer       = COALESCE(?4, developer),
            publisher       = COALESCE(?5, publisher),
            genre           = COALESCE(?6, genre),
            release_date    = COALESCE(?7, release_date),
            players         = COALESCE(?8, players),
            rating          = COALESCE(?9, rating),
            cover_path      = COALESCE(?10, cover_path),
            screenshot_path = COALESCE(?11, screenshot_path),
            logo_path       = COALESCE(?12, logo_path),
            scrape_status   = 'ok',
            scrape_source   = ?13
         WHERE id = ?1",
        params![
            id,
            m.title,
            m.description,
            m.developer,
            m.publisher,
            m.genre,
            m.release_date,
            m.players,
            m.rating,
            m.cover_path,
            m.screenshot_path,
            m.logo_path,
            m.source
        ],
    )?;
    Ok(())
}

pub fn set_scrape_status(conn: &Connection, id: i64, status: &str) -> Result<()> {
    conn.execute(
        "UPDATE games SET scrape_status = ?2 WHERE id = ?1",
        params![id, status],
    )?;
    Ok(())
}

pub fn set_favorite(conn: &Connection, id: i64, value: bool) -> Result<()> {
    conn.execute(
        "UPDATE games SET favorite = ?2 WHERE id = ?1",
        params![id, if value { 1 } else { 0 }],
    )?;
    Ok(())
}

pub fn set_hidden(conn: &Connection, id: i64, value: bool) -> Result<()> {
    conn.execute(
        "UPDATE games SET hidden = ?2 WHERE id = ?1",
        params![id, if value { 1 } else { 0 }],
    )?;
    Ok(())
}

pub fn set_platform(conn: &Connection, id: i64, platform: &str) -> Result<()> {
    conn.execute(
        "UPDATE games SET platform = ?2, scrape_status = 'pending' WHERE id = ?1",
        params![id, platform],
    )?;
    Ok(())
}

/// Remove a game from the library. Never touches the ROM on disk.
pub fn remove_game(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM games WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn record_play(conn: &Connection, id: i64, started_at: i64, seconds: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO play_sessions (game_id, started_at, seconds) VALUES (?1, ?2, ?3)",
        params![id, started_at, seconds],
    )?;
    conn.execute(
        "UPDATE games SET play_count = play_count + 1,
                          play_seconds = play_seconds + ?2,
                          last_played = ?3
         WHERE id = ?1",
        params![id, seconds, started_at + seconds],
    )?;
    Ok(())
}

pub fn game_id_by_path(conn: &Connection, path: &str) -> Result<Option<i64>> {
    Ok(conn
        .query_row("SELECT id FROM games WHERE path = ?1", params![path], |r| {
            r.get(0)
        })
        .optional()?)
}

pub fn all_paths(conn: &Connection) -> Result<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare("SELECT path FROM games")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<std::collections::HashSet<_>>>()?)
}

// ------------------------------------------------------------ platforms

pub fn platform_counts(conn: &Connection) -> Result<Vec<PlatformInfo>> {
    let mut stmt = conn.prepare(
        "SELECT platform, COUNT(*) FROM games WHERE hidden = 0 GROUP BY platform ORDER BY 2 DESC",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;

    let mut out = Vec::new();
    for row in rows {
        let (slug, count) = row?;
        let def = platforms::by_slug(&slug);
        out.push(PlatformInfo {
            name: platforms::display_name(&slug),
            slug,
            extensions: def
                .map(|d| d.exts.iter().map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            cores: def
                .map(|d| d.cores.iter().map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            game_count: count,
        });
    }
    Ok(out)
}

pub fn stats(conn: &Connection) -> Result<LibraryStats> {
    Ok(LibraryStats {
        total_games: conn.query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))?,
        total_platforms: conn.query_row(
            "SELECT COUNT(DISTINCT platform) FROM games",
            [],
            |r| r.get(0),
        )?,
        scraped: conn.query_row(
            "SELECT COUNT(*) FROM games WHERE scrape_status = 'ok'",
            [],
            |r| r.get(0),
        )?,
        total_play_seconds: conn
            .query_row("SELECT COALESCE(SUM(play_seconds), 0) FROM games", [], |r| {
                r.get(0)
            })?,
    })
}

// -------------------------------------------------------------- folders

pub fn add_folder(conn: &Connection, path: &str, platform: Option<&str>) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO folders (path, platform_override, added_at)
         VALUES (?1, ?2, COALESCE((SELECT added_at FROM folders WHERE path = ?1), ?3))",
        params![path, platform, now()],
    )?;
    Ok(())
}

pub fn remove_folder(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM folders WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn list_folders(conn: &Connection) -> Result<Vec<LibraryFolder>> {
    let mut stmt =
        conn.prepare("SELECT id, path, platform_override, added_at FROM folders ORDER BY path")?;
    let rows = stmt.query_map([], |r| {
        Ok(LibraryFolder {
            id: r.get(0)?,
            path: r.get(1)?,
            platform_override: r.get(2)?,
            added_at: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

// ------------------------------------------------------------- settings

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn all_settings(conn: &Connection) -> Result<std::collections::HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<std::collections::HashMap<_, _>>>()?)
}

// ------------------------------------------------------------ emulators

pub fn get_emulator(conn: &Connection, platform: &str) -> Result<Option<EmulatorConfig>> {
    Ok(conn
        .query_row(
            "SELECT platform, mode, core, custom_command FROM emulators WHERE platform = ?1",
            params![platform],
            |r| {
                Ok(EmulatorConfig {
                    platform: r.get(0)?,
                    mode: r.get(1)?,
                    core: r.get(2)?,
                    custom_command: r.get(3)?,
                })
            },
        )
        .optional()?)
}

pub fn set_emulator(conn: &Connection, cfg: &EmulatorConfig) -> Result<()> {
    conn.execute(
        "INSERT INTO emulators (platform, mode, core, custom_command) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(platform) DO UPDATE SET
            mode = excluded.mode,
            core = excluded.core,
            custom_command = excluded.custom_command",
        params![cfg.platform, cfg.mode, cfg.core, cfg.custom_command],
    )?;
    Ok(())
}

pub fn list_emulators(conn: &Connection) -> Result<Vec<EmulatorConfig>> {
    let mut stmt =
        conn.prepare("SELECT platform, mode, core, custom_command FROM emulators ORDER BY platform")?;
    let rows = stmt.query_map([], |r| {
        Ok(EmulatorConfig {
            platform: r.get(0)?,
            mode: r.get(1)?,
            core: r.get(2)?,
            custom_command: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

// ------------------------------------------------------- patch catalog

const PATCH_COLS: &str =
    "id, path, name, format, source_crc, system_hint, target_hint, origin, added_at";

fn row_to_patch(row: &Row) -> rusqlite::Result<PatchEntry> {
    Ok(PatchEntry {
        id: row.get(0)?,
        path: row.get(1)?,
        name: row.get(2)?,
        format: row.get(3)?,
        source_crc: row.get(4)?,
        system_hint: row.get(5)?,
        target_hint: row.get(6)?,
        origin: row.get(7)?,
        added_at: row.get(8)?,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn insert_patch(
    conn: &Connection,
    path: &str,
    name: &str,
    format: &str,
    source_crc: Option<&str>,
    system_hint: Option<&str>,
    target_hint: Option<&str>,
    origin: &str,
) -> Result<bool> {
    let changed = conn.execute(
        "INSERT OR IGNORE INTO patches
         (path, name, format, source_crc, system_hint, target_hint, origin, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![path, name, format, source_crc, system_hint, target_hint, origin, now()],
    )?;
    Ok(changed > 0)
}

/// Catalogued patches built against exactly this ROM.
pub fn patches_for_crc(conn: &Connection, crc: &str) -> Result<Vec<PatchEntry>> {
    let sql = format!(
        "SELECT {PATCH_COLS} FROM patches WHERE source_crc = ?1 COLLATE NOCASE          ORDER BY name COLLATE NOCASE"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![crc], row_to_patch)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn list_patches(conn: &Connection, search: Option<&str>, limit: i64) -> Result<Vec<PatchEntry>> {
    let mut sql = format!("SELECT {PATCH_COLS} FROM patches WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(q) = search.filter(|q| !q.trim().is_empty()) {
        sql.push_str(" AND name LIKE ?");
        args.push(Box::new(format!("%{}%", q.trim())));
    }
    sql.push_str(" ORDER BY name COLLATE NOCASE LIMIT ?");
    args.push(Box::new(limit));

    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), row_to_patch)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_patch(conn: &Connection, id: i64) -> Result<Option<PatchEntry>> {
    let sql = format!("SELECT {PATCH_COLS} FROM patches WHERE id = ?1");
    Ok(conn.query_row(&sql, params![id], row_to_patch).optional()?)
}

pub fn patch_catalog_size(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM patches", [], |r| r.get(0))?)
}

pub fn clear_patch_catalog(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM patches", [])?;
    Ok(())
}

// -------------------------------------------------------------- cheats

/// Replace a game's cheat list, keeping whatever was already switched on.
pub fn replace_cheats(conn: &Connection, game_id: i64, cheats: &[Cheat]) -> Result<()> {
    let previously_on: Vec<i64> = {
        let mut stmt =
            conn.prepare("SELECT idx FROM cheats WHERE game_id = ?1 AND enabled = 1")?;
        let rows = stmt.query_map(params![game_id], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };

    conn.execute("DELETE FROM cheats WHERE game_id = ?1", params![game_id])?;
    for cheat in cheats {
        let on = cheat.enabled || previously_on.contains(&cheat.index);
        conn.execute(
            "INSERT OR REPLACE INTO cheats (game_id, idx, description, code, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                game_id,
                cheat.index,
                cheat.description,
                cheat.code,
                if on { 1 } else { 0 }
            ],
        )?;
    }
    Ok(())
}

pub fn list_cheats(conn: &Connection, game_id: i64) -> Result<Vec<Cheat>> {
    let mut stmt = conn.prepare(
        "SELECT idx, description, code, enabled FROM cheats WHERE game_id = ?1 ORDER BY idx",
    )?;
    let rows = stmt.query_map(params![game_id], |r| {
        Ok(Cheat {
            index: r.get(0)?,
            description: r.get(1)?,
            code: r.get(2)?,
            enabled: r.get::<_, i64>(3)? != 0,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Switch every cheat for a game at once. One statement rather than a
/// round trip per cheat — some games have several hundred.
pub fn set_all_cheats_enabled(conn: &Connection, game_id: i64, on: bool) -> Result<usize> {
    let changed = conn.execute(
        "UPDATE cheats SET enabled = ?2 WHERE game_id = ?1",
        params![game_id, if on { 1 } else { 0 }],
    )?;
    Ok(changed)
}

pub fn set_cheat_enabled(conn: &Connection, game_id: i64, idx: i64, on: bool) -> Result<()> {
    conn.execute(
        "UPDATE cheats SET enabled = ?3 WHERE game_id = ?1 AND idx = ?2",
        params![game_id, idx, if on { 1 } else { 0 }],
    )?;
    Ok(())
}

// ------------------------------------------------------------ insights

/// Games with a play history, most recent first.
pub fn recently_played(conn: &Connection, limit: i64) -> Result<Vec<Game>> {
    let sql = format!(
        "SELECT {GAME_COLS} FROM games
         WHERE last_played IS NOT NULL AND hidden = 0
         ORDER BY last_played DESC LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![limit], row_to_game)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Games ranked by time spent in them.
pub fn most_played(conn: &Connection, limit: i64) -> Result<Vec<Game>> {
    let sql = format!(
        "SELECT {GAME_COLS} FROM games
         WHERE play_seconds > 0 AND hidden = 0
         ORDER BY play_seconds DESC LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![limit], row_to_game)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn session_count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM play_sessions", [], |r| r.get(0))?)
}

pub fn longest_session(conn: &Connection) -> Result<i64> {
    Ok(conn
        .query_row("SELECT COALESCE(MAX(seconds), 0) FROM play_sessions", [], |r| {
            r.get(0)
        })?)
}
