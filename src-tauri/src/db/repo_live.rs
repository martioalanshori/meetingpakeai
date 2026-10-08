//! Tabel `live_progress` (langkah 37): sampai sampel ke berapa audio tiap channel sudah dipraproses
//! selama merekam. Tanpa baris → meeting diproses penuh dari awal setelah Stop.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;

pub fn get(conn: &Connection, meeting_id: &str, channel: &str) -> AppResult<Option<u64>> {
    let v: Option<i64> = conn
        .query_row(
            "SELECT processed_samples FROM live_progress WHERE meeting_id = ?1 AND channel = ?2",
            params![meeting_id, channel],
            |r| r.get(0),
        )
        .optional()?;
    Ok(v.map(|n| n.max(0) as u64))
}

pub fn has_any(conn: &Connection, meeting_id: &str) -> AppResult<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM live_progress WHERE meeting_id = ?1", [meeting_id], |r| r.get(0))?;
    Ok(n > 0)
}

pub fn set(conn: &Connection, meeting_id: &str, channel: &str, processed_samples: u64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO live_progress (meeting_id, channel, processed_samples) VALUES (?1, ?2, ?3)
         ON CONFLICT(meeting_id, channel) DO UPDATE SET processed_samples = excluded.processed_samples",
        params![meeting_id, channel, processed_samples as i64],
    )?;
    Ok(())
}

pub fn clear(conn: &Connection, meeting_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM live_progress WHERE meeting_id = ?1", [meeting_id])?;
    Ok(())
}
