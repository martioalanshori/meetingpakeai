//! Tabel `meeting_notes`: catatan pribadi pengguna per meeting (langkah 49).

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::now_ms;
use crate::error::AppResult;

/// Batas panjang catatan (karakter).
pub const NOTES_MAX_CHARS: usize = 5_000;

pub fn get(conn: &Connection, meeting_id: &str) -> AppResult<String> {
    Ok(conn
        .query_row("SELECT text FROM meeting_notes WHERE meeting_id = ?1", [meeting_id], |r| r.get(0))
        .optional()?
        .unwrap_or_default())
}

/// Simpan (kosong → baris dihapus).
pub fn set(conn: &Connection, meeting_id: &str, text: &str) -> AppResult<()> {
    let text: String = text.chars().take(NOTES_MAX_CHARS).collect();
    if text.trim().is_empty() {
        conn.execute("DELETE FROM meeting_notes WHERE meeting_id = ?1", [meeting_id])?;
        return Ok(());
    }
    conn.execute(
        "INSERT INTO meeting_notes (meeting_id, text, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(meeting_id) DO UPDATE SET text = excluded.text, updated_at = excluded.updated_at",
        params![meeting_id, text, now_ms()],
    )?;
    Ok(())
}
