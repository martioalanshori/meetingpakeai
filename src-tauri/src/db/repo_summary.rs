//! Tabel `summaries` dan `action_items`.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::now_ms;
use crate::error::AppResult;
use crate::llm::parse::{self, FinalNotes};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryView {
    pub status: String,
    pub summary: Option<String>,
    pub decisions: Vec<String>,
    /// Sejajar `decisions`: ms dari awal meeting tempat keputusan dibahas (langkah 42).
    pub decision_sources: Vec<Option<i64>>,
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
    pub source_ms: Option<i64>,
}

/// `HH:MM:SS` dari LLM → ms; di luar durasi meeting (halusinasi) → `None`.
fn source_ms(sumber: Option<&str>, duration_ms: i64) -> Option<i64> {
    let ms = parse::timestamp_ms(sumber?)?;
    (duration_ms <= 0 || ms <= duration_ms).then_some(ms)
}

/// Simpan hasil ringkasan; row lama (summary + action items) diganti dalam satu transaksi (PRD §11).
/// `notes = None` → `status = 'empty'`.
pub fn save(
    conn: &mut Connection,
    meeting_id: &str,
    notes: Option<&FinalNotes>,
    model: &str,
    duration_ms: i64,
) -> AppResult<()> {
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
            let texts: Vec<&str> = n.keputusan.iter().map(|d| d.teks.as_str()).collect();
            let sources: Vec<Option<i64>> =
                n.keputusan.iter().map(|d| source_ms(d.sumber.as_deref(), duration_ms)).collect();
            tx.execute(
                "INSERT INTO summaries (meeting_id, status, summary, decisions, decision_sources, topics, model, created_at)
                 VALUES (?1, 'ok', ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    meeting_id,
                    n.ringkasan,
                    serde_json::to_string(&texts)?,
                    serde_json::to_string(&sources)?,
                    serde_json::to_string(&n.topik)?,
                    model,
                    now_ms()
                ],
            )?;
            let mut stmt = tx.prepare(
                "INSERT INTO action_items (meeting_id, idx, task, assignee, due, source_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for (i, a) in n.action_items.iter().enumerate() {
                let src = source_ms(a.sumber.as_deref(), duration_ms);
                stmt.execute(params![meeting_id, i as i64, a.tugas, a.penanggung_jawab, a.tenggat, src])?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn get(conn: &Connection, meeting_id: &str) -> AppResult<Option<SummaryView>> {
    let row = conn
        .query_row(
            "SELECT status, summary, decisions, topics, edited, decision_sources FROM summaries WHERE meeting_id = ?1",
            [meeting_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, bool>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?;
    Ok(row.map(|(status, summary, decisions, topics, edited, sources)| {
        let decisions: Vec<String> = serde_json::from_str(&decisions).unwrap_or_default();
        let mut decision_sources: Vec<Option<i64>> = serde_json::from_str(&sources).unwrap_or_default();
        decision_sources.resize(decisions.len(), None);
        SummaryView {
            status,
            summary,
            decisions,
            decision_sources,
            topics: serde_json::from_str(&topics).unwrap_or_default(),
            edited,
        }
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
    // Sumber waktu dipertahankan untuk keputusan/tugas yang teksnya tidak diubah.
    let old = get(conn, meeting_id)?;
    let old_sources: Vec<(String, Option<i64>)> = old
        .map(|o| o.decisions.into_iter().zip(o.decision_sources).collect())
        .unwrap_or_default();
    let sources: Vec<Option<i64>> =
        decisions.iter().map(|d| old_sources.iter().find(|(t, _)| t == d).and_then(|(_, s)| *s)).collect();
    let old_items = action_items(conn, meeting_id)?;
    let tx = conn.transaction()?;
    let changed = tx.execute(
        "UPDATE summaries SET status = 'ok', summary = ?2, decisions = ?3, topics = ?4, decision_sources = ?5, edited = 1
         WHERE meeting_id = ?1",
        params![
            meeting_id,
            clean(&e.summary),
            serde_json::to_string(&decisions)?,
            serde_json::to_string(&topics)?,
            serde_json::to_string(&sources)?
        ],
    )?;
    if changed == 0 {
        return Ok(false);
    }
    tx.execute("DELETE FROM action_items WHERE meeting_id = ?1", [meeting_id])?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO action_items (meeting_id, idx, task, assignee, due, done, source_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        let items = e.action_items.iter().filter_map(|a| clean(&a.task).map(|task| (task, a)));
        for (i, (task, a)) in items.enumerate() {
            let assignee = a.assignee.as_deref().and_then(clean);
            let due = a.due.as_deref().and_then(clean);
            let src = old_items.iter().find(|o| o.task == task).and_then(|o| o.source_ms);
            stmt.execute(params![meeting_id, i as i64, task, assignee, due, a.done, src])?;
        }
    }
    tx.commit()?;
    Ok(true)
}

pub fn action_items(conn: &Connection, meeting_id: &str) -> AppResult<Vec<ActionItemView>> {
    let mut stmt =
        conn.prepare("SELECT id, task, assignee, due, done, source_ms FROM action_items WHERE meeting_id = ?1 ORDER BY idx")?;
    let rows = stmt
        .query_map([meeting_id], |r| {
            Ok(ActionItemView {
                id: r.get(0)?,
                task: r.get(1)?,
                assignee: r.get(2)?,
                due: r.get(3)?,
                done: r.get(4)?,
                source_ms: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Action item lintas meeting (halaman "Tugas", langkah 28).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskView {
    pub id: i64,
    pub meeting_id: String,
    pub meeting_title: String,
    pub started_at: i64,
    pub task: String,
    pub assignee: Option<String>,
    pub due: Option<String>,
    pub done: bool,
}

/// Semua action item meeting `done`: belum selesai dulu, lalu meeting terbaru.
pub fn all_action_items(conn: &Connection) -> AppResult<Vec<TaskView>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.meeting_id, m.title, m.started_at, a.task, a.assignee, a.due, a.done
         FROM action_items a JOIN meetings m ON m.id = a.meeting_id
         WHERE m.status = 'done'
         ORDER BY a.done, m.started_at DESC, a.idx",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(TaskView {
                id: r.get(0)?,
                meeting_id: r.get(1)?,
                meeting_title: r.get(2)?,
                started_at: r.get(3)?,
                task: r.get(4)?,
                assignee: r.get(5)?,
                due: r.get(6)?,
                done: r.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

