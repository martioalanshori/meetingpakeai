//! Tabel `recording_parts` (PRD §7.3).

use rusqlite::{params, Connection};

use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct PartRow {
    pub id: i64,
    pub channel: String,
    pub part_index: i64,
    /// Relatif terhadap app_data_dir.
    pub path: String,
    pub samples: i64,
    pub finalized: bool,
}

pub fn insert_open(conn: &Connection, meeting_id: &str, channel: &str, part_index: i64, rel_path: &str) -> AppResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO recording_parts (meeting_id, channel, part_index, path, samples, finalized)
         VALUES (?1, ?2, ?3, ?4, 0, 0)",
        params![meeting_id, channel, part_index, rel_path],
    )?;
    Ok(())
}

pub fn mark_finalized(conn: &Connection, meeting_id: &str, channel: &str, part_index: i64, samples: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE recording_parts SET samples = ?4, finalized = 1
         WHERE meeting_id = ?1 AND channel = ?2 AND part_index = ?3",
        params![meeting_id, channel, part_index, samples],
    )?;
    Ok(())
}

/// Part satu meeting, urut channel lalu indeks.
pub fn list(conn: &Connection, meeting_id: &str) -> AppResult<Vec<PartRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, channel, part_index, path, samples, finalized FROM recording_parts
         WHERE meeting_id = ?1 ORDER BY channel, part_index",
    )?;
    let rows = stmt
        .query_map([meeting_id], |r| {
            Ok(PartRow {
                id: r.get(0)?,
                channel: r.get(1)?,
                part_index: r.get(2)?,
                path: r.get(3)?,
                samples: r.get(4)?,
                finalized: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
