//! Tabel `summaries` dan `action_items`.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::now_ms;
use crate::error::AppResult;
use crate::llm::parse::FinalNotes;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryView {
    pub status: String,
    pub summary: Option<String>,
    pub decisions: Vec<String>,
    pub topics: Vec<String>,
    /// Sudah diubah pengguna (langkah 23).
    pub edited: bool,
}

/// Isi edit pengguna (`update_summary`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryEdit {
    pub summary: String,
    pub decisions: Vec<String>,
    pub topics: Vec<String>,
    pub action_items: Vec<ActionItemEdit>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionItemEdit {
    pub task: String,
    pub assignee: Option<String>,
    pub due: Option<String>,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionItemView {
    pub id: i64,
    pub task: String,
    pub assignee: Option<String>,
    pub due: Option<String>,
    pub done: bool,
}

/// Simpan hasil ringkasan; row lama (summary + action items) diganti dalam satu transaksi (PRD §11).
/// `notes = None` → `status = 'empty'`.
pub fn save(conn: &mut Connection, meeting_id: &str, notes: Option<&FinalNotes>, model: &str) -> AppResult<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM summaries WHERE meeting_id = ?1", [meeting_id])?;
    tx.execute("DELETE FROM action_items WHERE meeting_id = ?1", [meeting_id])?;
    match notes {
        None => {
            tx.execute(
                "INSERT INTO summaries (meeting_id, status, summary, model, created_at) VALUES (?1, 'empty', NULL, ?2, ?3)",
                params![meeting_id, model, now_ms()],
            )?;
        }
        Some(n) => {
            tx.execute(
                "INSERT INTO summaries (meeting_id, status, summary, decisions, topics, model, created_at)
                 VALUES (?1, 'ok', ?2, ?3, ?4, ?5, ?6)",
                params![
                    meeting_id,
                    n.ringkasan,
                    serde_json::to_string(&n.keputusan)?,
                    serde_json::to_string(&n.topik)?,
                    model,
                    now_ms()
                ],
            )?;
            let mut stmt = tx.prepare(
                "INSERT INTO action_items (meeting_id, idx, task, assignee, due) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (i, a) in n.action_items.iter().enumerate() {
                stmt.execute(params![meeting_id, i as i64, a.tugas, a.penanggung_jawab, a.tenggat])?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn get(conn: &Connection, meeting_id: &str) -> AppResult<Option<SummaryView>> {
    let row = conn
        .query_row(
            "SELECT status, summary, decisions, topics, edited FROM summaries WHERE meeting_id = ?1",
            [meeting_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, bool>(4)?,
                ))
            },
        )
        .optional()?;
    Ok(row.map(|(status, summary, decisions, topics, edited)| SummaryView {
        status,
        summary,
        decisions: serde_json::from_str(&decisions).unwrap_or_default(),
        topics: serde_json::from_str(&topics).unwrap_or_default(),
        edited,
    }))
}

fn clean(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// Ganti ringkasan & action item dengan versi pengguna (`edited = 1`). Ringkasan harus sudah ada.
/// Mengembalikan `false` jika meeting belum punya ringkasan.
pub fn update(conn: &mut Connection, meeting_id: &str, e: &SummaryEdit) -> AppResult<bool> {
    let decisions: Vec<String> = e.decisions.iter().filter_map(|d| clean(d)).collect();
    let topics: Vec<String> = e.topics.iter().filter_map(|t| clean(t)).collect();
    let tx = conn.transaction()?;
    let changed = tx.execute(
        "UPDATE summaries SET status = 'ok', summary = ?2, decisions = ?3, topics = ?4, edited = 1 WHERE meeting_id = ?1",
        params![meeting_id, clean(&e.summary), serde_json::to_string(&decisions)?, serde_json::to_string(&topics)?],
    )?;
    if changed == 0 {
        return Ok(false);
    }
    tx.execute("DELETE FROM action_items WHERE meeting_id = ?1", [meeting_id])?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO action_items (meeting_id, idx, task, assignee, due, done) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        let items = e.action_items.iter().filter_map(|a| clean(&a.task).map(|task| (task, a)));
        for (i, (task, a)) in items.enumerate() {
            let assignee = a.assignee.as_deref().and_then(clean);
            let due = a.due.as_deref().and_then(clean);
            stmt.execute(params![meeting_id, i as i64, task, assignee, due, a.done])?;
        }
    }
    tx.commit()?;
    Ok(true)
}

/// Nama "Peserta lain" per meeting (F10); kosong → kembali ke default.
pub fn set_system_label(conn: &Connection, meeting_id: &str, name: &str) -> AppResult<()> {
    match clean(name) {
        Some(n) => conn.execute(
            "INSERT INTO meeting_speaker_names (meeting_id, channel, display_name) VALUES (?1, 'system', ?2)
             ON CONFLICT(meeting_id, channel) DO UPDATE SET display_name = excluded.display_name",
            params![meeting_id, n],
        )?,
        None => conn.execute("DELETE FROM meeting_speaker_names WHERE meeting_id = ?1", [meeting_id])?,
    };
    Ok(())
}

pub fn action_items(conn: &Connection, meeting_id: &str) -> AppResult<Vec<ActionItemView>> {
    let mut stmt =
        conn.prepare("SELECT id, task, assignee, due, done FROM action_items WHERE meeting_id = ?1 ORDER BY idx")?;
    let rows = stmt
        .query_map([meeting_id], |r| {
            Ok(ActionItemView { id: r.get(0)?, task: r.get(1)?, assignee: r.get(2)?, due: r.get(3)?, done: r.get(4)? })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Label "Peserta lain" per meeting (Beta F10; tabel sudah ada sejak MVP).
pub fn system_label(conn: &Connection, meeting_id: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT display_name FROM meeting_speaker_names WHERE meeting_id = ?1 AND channel = 'system'",
            [meeting_id],
            |r| r.get(0),
        )
        .optional()?)
}
