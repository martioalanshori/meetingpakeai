//! Label proyek/klien per meeting (langkah 59, feedback3 D4).

use rusqlite::{params, Connection};

use crate::error::AppResult;

/// Batas jumlah & panjang label per meeting.
pub const MAX_TAGS: usize = 6;
pub const MAX_TAG_CHARS: usize = 40;

/// Semua label yang dipakai, terbanyak dulu.
pub fn all(conn: &Connection) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t JOIN meeting_tags mt ON mt.tag_id = t.id
         GROUP BY t.id ORDER BY COUNT(*) DESC, t.name",
    )?;
    let rows = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn for_meeting(conn: &Connection, meeting_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t JOIN meeting_tags mt ON mt.tag_id = t.id WHERE mt.meeting_id = ?1 ORDER BY t.name",
    )?;
    let rows = stmt.query_map([meeting_id], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Ganti label meeting (dibersihkan, tanpa duplikat). Label yang tidak dipakai lagi dibuang.
pub fn set_for_meeting(conn: &mut Connection, meeting_id: &str, names: &[String]) -> AppResult<Vec<String>> {
    let mut clean: Vec<String> = Vec::new();
    for n in names {
        let n: String = n.trim().chars().take(MAX_TAG_CHARS).collect();
        if !n.is_empty() && !clean.iter().any(|c| c.eq_ignore_ascii_case(&n)) {
            clean.push(n);
        }
    }
    clean.truncate(MAX_TAGS);
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM meeting_tags WHERE meeting_id = ?1", [meeting_id])?;
    for n in &clean {
        tx.execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", [n])?;
        tx.execute(
            "INSERT OR IGNORE INTO meeting_tags (meeting_id, tag_id) SELECT ?1, id FROM tags WHERE name = ?2",
            params![meeting_id, n],
        )?;
    }
    tx.execute("DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM meeting_tags)", [])?;
    tx.commit()?;
    for_meeting(conn, meeting_id)
}
