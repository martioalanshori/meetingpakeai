//! Pencarian teks (F11, FTS5, langkah 28): tabel virtual `search_index` per meeting.
//! Diisi ulang setelah job `done`, edit ringkasan, dan ganti judul; dihapus bersama meeting.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::AppResult;

const MAX_HITS: i64 = 100;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub meeting_id: String,
    pub title: String,
    pub started_at: i64,
    /// `title` | `summary` | `decision` | `action` | `topic` | `transcript`
    pub kind: String,
    /// Potongan teks; kata yang cocok diapit `[` `]`.
    pub snippet: String,
    /// Posisi transkrip (ms) untuk `kind = transcript`.
    pub start_ms: Option<i64>,
}

/// Bangun ulang index satu meeting dari judul, ringkasan, action item, dan transkrip yang tampil.
pub fn reindex(conn: &mut Connection, meeting_id: &str) -> AppResult<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM search_index WHERE meeting_id = ?1", [meeting_id])?;
    tx.execute(
        "INSERT INTO search_index (meeting_id, kind, ref, text) SELECT id, 'title', NULL, title FROM meetings WHERE id = ?1",
        [meeting_id],
    )?;
    tx.execute(
        "INSERT INTO search_index (meeting_id, kind, ref, text)
         SELECT meeting_id, 'summary', NULL, summary FROM summaries WHERE meeting_id = ?1 AND summary IS NOT NULL",
        [meeting_id],
    )?;
    tx.execute(
        "INSERT INTO search_index (meeting_id, kind, ref, text)
         SELECT s.meeting_id, 'decision', NULL, j.value FROM summaries s, json_each(s.decisions) j WHERE s.meeting_id = ?1",
        [meeting_id],
    )?;
    tx.execute(
        "INSERT INTO search_index (meeting_id, kind, ref, text)
         SELECT s.meeting_id, 'topic', NULL, j.value FROM summaries s, json_each(s.topics) j WHERE s.meeting_id = ?1",
        [meeting_id],
    )?;
    tx.execute(
        "INSERT INTO search_index (meeting_id, kind, ref, text)
         SELECT meeting_id, 'action', NULL, task || COALESCE(' — ' || assignee, '') FROM action_items WHERE meeting_id = ?1",
        [meeting_id],
    )?;
    tx.execute(
        "INSERT INTO search_index (meeting_id, kind, ref, text)
         SELECT meeting_id, 'transcript', start_ms, text FROM transcript_segments
         WHERE meeting_id = ?1 AND is_filtered = 0 AND is_duplicate = 0",
        [meeting_id],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn delete(conn: &Connection, meeting_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM search_index WHERE meeting_id = ?1", [meeting_id])?;
    Ok(())
}

/// Index kosong tetapi ada meeting selesai (baru migrasi) → isi semua sekali.
pub fn backfill_if_empty(conn: &mut Connection) -> AppResult<usize> {
    let indexed: i64 = conn.query_row("SELECT COUNT(*) FROM search_index", [], |r| r.get(0))?;
    if indexed > 0 {
        return Ok(0);
    }
    let ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM meetings WHERE status = 'done'")?;
        let rows = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
        rows
    };
    for id in &ids {
        reindex(conn, id)?;
    }
    Ok(ids.len())
}

/// Ubah input pengguna jadi query FTS5 aman: tiap kata jadi `"kata"*` (prefiks), semua kata wajib ada.
fn to_fts_query(input: &str) -> Option<String> {
    let terms: Vec<String> = input
        .split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

pub fn search(conn: &Connection, input: &str) -> AppResult<Vec<SearchHit>> {
    let Some(q) = to_fts_query(input) else { return Ok(Vec::new()) };
    let mut stmt = conn.prepare(
        "SELECT si.meeting_id, m.title, m.started_at, si.kind, snippet(search_index, 3, '[', ']', '…', 14), si.ref
         FROM search_index si JOIN meetings m ON m.id = si.meeting_id
         WHERE search_index MATCH ?1
         ORDER BY rank LIMIT ?2",
    )?;
    let hits = stmt
        .query_map(params![q, MAX_HITS], |r| {
            Ok(SearchHit {
                meeting_id: r.get(0)?,
                title: r.get(1)?,
                started_at: r.get(2)?,
                kind: r.get(3)?,
                snippet: r.get(4)?,
                start_ms: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_fts_aman() {
        assert_eq!(to_fts_query("  harga \"proyek\" OR-"), Some("\"harga\"* \"proyek\"* \"OR\"*".to_string()));
        assert_eq!(to_fts_query("  *** "), None);
    }

    #[test]
    fn fts5_aktif_di_sqlite_bundled() {
        let db = crate::db::Db::open_in_memory().unwrap();
        let conn = db.conn();
        conn.execute(
            "INSERT INTO meetings (id, title, created_at, started_at, language, consent_at, status, updated_at)
             VALUES ('m1', 'Rapat harga', 0, 0, 'id', 0, 'done', 0)",
            [],
        )
        .unwrap();
        drop(conn);
        reindex(&mut db.conn(), "m1").unwrap();
        let hits = search(&db.conn(), "har").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].snippet, "Rapat [harga]");
    }
}
