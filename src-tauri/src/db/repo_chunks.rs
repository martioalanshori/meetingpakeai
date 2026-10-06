//! Tabel `upload_chunks` (PRD §8.1–§8.2).

use rusqlite::{params, Connection};

use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct ChunkRow {
    pub id: i64,
    pub channel: String,
    pub idx: i64,
    /// Relatif terhadap app_data_dir.
    pub path: String,
    pub duration_ms: i64,
    pub offset_map_json: String,
    pub stt_status: String,
    pub attempts: i64,
    pub response_json: Option<String>,
}

pub struct NewChunk<'a> {
    pub meeting_id: &'a str,
    pub channel: &'a str,
    pub idx: i64,
    pub path: &'a str,
    pub duration_ms: i64,
    pub offset_map_json: &'a str,
}

pub fn insert(conn: &Connection, c: &NewChunk<'_>) -> AppResult<()> {
    conn.execute(
        "INSERT INTO upload_chunks (meeting_id, channel, idx, path, duration_ms, offset_map_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![c.meeting_id, c.channel, c.idx, c.path, c.duration_ms, c.offset_map_json],
    )?;
    Ok(())
}

pub fn delete_for_meeting(conn: &Connection, meeting_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM upload_chunks WHERE meeting_id = ?1", [meeting_id])?;
    Ok(())
}

/// Semua chunk, urut mic dulu lalu system, lalu `idx` (PRD §8.2).
pub fn list(conn: &Connection, meeting_id: &str) -> AppResult<Vec<ChunkRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, channel, idx, path, duration_ms, offset_map_json, stt_status, attempts, response_json
         FROM upload_chunks WHERE meeting_id = ?1
         ORDER BY CASE channel WHEN 'mic' THEN 0 ELSE 1 END, idx",
    )?;
    let rows = stmt
        .query_map([meeting_id], |r| {
            Ok(ChunkRow {
                id: r.get(0)?,
                channel: r.get(1)?,
                idx: r.get(2)?,
                path: r.get(3)?,
                duration_ms: r.get(4)?,
                offset_map_json: r.get(5)?,
                stt_status: r.get(6)?,
                attempts: r.get(7)?,
                response_json: r.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn mark_done(conn: &Connection, id: i64, response_json: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE upload_chunks SET stt_status = 'done', response_json = ?2, error_message = NULL, attempts = attempts + 1
         WHERE id = ?1",
        params![id, response_json],
    )?;
    Ok(())
}

pub fn mark_failed(conn: &Connection, id: i64, message: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE upload_chunks SET stt_status = 'failed', error_message = ?2, attempts = attempts + 1 WHERE id = ?1",
        params![id, message],
    )?;
    Ok(())
}
