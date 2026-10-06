//! Tabel `settings`: key → nilai JSON.

use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::AppResult;

/// Baca setting; `None` jika belum pernah disimpan atau JSON-nya rusak (dicatat ke log).
pub fn get<T: DeserializeOwned>(conn: &Connection, key: &str) -> AppResult<Option<T>> {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
        .optional()?;
    Ok(raw.and_then(|s| match serde_json::from_str(&s) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("setting {key} rusak, pakai default: {e}");
            None
        }
    }))
}

pub fn set<T: Serialize>(conn: &Connection, key: &str, value: &T) -> AppResult<()> {
    let json = serde_json::to_string(value)?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, json],
    )?;
    Ok(())
}
