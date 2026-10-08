//! Tabel `bookmarks`: momen penting yang ditandai saat merekam (langkah 44).

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::now_ms;
use crate::error::AppResult;

pub fn insert(conn: &Connection, meeting_id: &str, at_ms: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO bookmarks (meeting_id, at_ms, created_at) VALUES (?1, ?2, ?3)",
        params![meeting_id, at_ms, now_ms()],
    )?;
    Ok(())
}

/// Waktu (ms) semua tanda satu meeting, urut.
pub fn list(conn: &Connection, meeting_id: &str) -> AppResult<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT at_ms FROM bookmarks WHERE meeting_id = ?1 ORDER BY at_ms")?;
    let rows = stmt.query_map([meeting_id], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn last(conn: &Connection, meeting_id: &str) -> AppResult<Option<i64>> {
    Ok(conn
        .query_row("SELECT MAX(at_ms) FROM bookmarks WHERE meeting_id = ?1", [meeting_id], |r| r.get(0))
        .optional()?
        .flatten())
}

pub fn count(conn: &Connection, meeting_id: &str) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM bookmarks WHERE meeting_id = ?1", [meeting_id], |r| r.get(0))?)
}

pub fn delete(conn: &Connection, meeting_id: &str, at_ms: i64) -> AppResult<()> {
    conn.execute("DELETE FROM bookmarks WHERE meeting_id = ?1 AND at_ms = ?2", params![meeting_id, at_ms])?;
    Ok(())
}
