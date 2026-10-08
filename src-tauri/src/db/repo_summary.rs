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
    /// Intisari 3 poin (langkah 50).
    pub key_points: Vec<String>,
    /// Belum diputuskan / pertanyaan terbuka + sumber waktunya (langkah 50).
    pub open_questions: Vec<String>,
    pub open_question_sources: Vec<Option<i64>>,
    pub topics: Vec<String>,
    /// Sudah diubah pengguna (langkah 23).
    pub edited: bool,
    /// Draf pesan tindak lanjut (langkah 43).
    pub follow_up: Option<FollowUp>,
    /// Status tugas meeting sebelumnya (langkah 55).
    pub followup_status: Vec<FollowupItem>,
    /// Meeting sebelumnya (id, judul) bila ada status tindak lanjut.
    pub followup_from: Option<FollowupFrom>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowupItem {
    pub item_id: i64,
    pub task: String,
    /// `selesai` | `dibahas` | `belum_disebut`
    pub status: String,
    pub note: String,
    /// Tugas lama sudah dicentang (dibaca saat tampil).
    #[serde(default)]
    pub done: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowupFrom {
    pub meeting_id: String,
    pub title: String,
}

/// Simpan status tindak lanjut (langkah 55): nomor dari LLM dipetakan ke tugas lama.
pub fn save_followup(
    conn: &Connection,
    meeting_id: &str,
    prev_id: &str,
    prev_tasks: &[(i64, String)],
    statuses: &[(usize, String, String)],
) -> AppResult<()> {
    let items: Vec<FollowupItem> = prev_tasks
        .iter()
        .enumerate()
        .map(|(i, (item_id, task))| {
            let found = statuses.iter().find(|(no, _, _)| *no == i + 1);
            let status = found
                .map(|(_, s, _)| s.to_lowercase())
                .filter(|s| matches!(s.as_str(), "selesai" | "dibahas" | "belum_disebut"))
                .unwrap_or_else(|| "belum_disebut".into());
            FollowupItem { item_id: *item_id, task: task.clone(), status, note: found.map(|f| f.2.clone()).unwrap_or_default(), done: false }
        })
        .collect();
    conn.execute(
        "UPDATE summaries SET followup_status = ?2, followup_from = ?3 WHERE meeting_id = ?1",
        params![meeting_id, serde_json::to_string(&items)?, prev_id],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUp {
    pub subject: String,
    pub body: String,
    /// `id` / `en`.
    pub lang: String,
}

pub fn set_follow_up(conn: &Connection, meeting_id: &str, f: &FollowUp) -> AppResult<()> {
    conn.execute(
        "UPDATE summaries SET follow_up = ?2 WHERE meeting_id = ?1",
        params![meeting_id, serde_json::to_string(f)?],
    )?;
    Ok(())
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

/// Tanggal `YYYY-MM-DD` pertama di teks tenggat (mis. "Jumat depan (2026-10-16)"), langkah 54.
pub fn due_date_of(due: Option<&str>) -> Option<String> {
    let s = due?;
    let b = s.as_bytes();
    (0..b.len().saturating_sub(9)).find_map(|i| {
        let w = &b[i..i + 10];
        let ok = w.iter().enumerate().all(|(k, c)| if k == 4 || k == 7 { *c == b'-' } else { c.is_ascii_digit() });
        ok.then(|| s[i..i + 10].to_string())
    })
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
            let open: Vec<&str> = n.pertanyaan_terbuka.iter().map(|d| d.teks.as_str()).collect();
            let open_sources: Vec<Option<i64>> =
                n.pertanyaan_terbuka.iter().map(|d| source_ms(d.sumber.as_deref(), duration_ms)).collect();
            tx.execute(
                "INSERT INTO summaries (meeting_id, status, summary, decisions, decision_sources, topics, model, created_at,
                                        key_points, open_questions, open_question_sources)
                 VALUES (?1, 'ok', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    meeting_id,
                    n.ringkasan,
                    serde_json::to_string(&texts)?,
                    serde_json::to_string(&sources)?,
                    serde_json::to_string(&n.topik)?,
                    model,
                    now_ms(),
                    serde_json::to_string(&n.intisari)?,
                    serde_json::to_string(&open)?,
                    serde_json::to_string(&open_sources)?
                ],
            )?;
            let mut stmt = tx.prepare(
                "INSERT INTO action_items (meeting_id, idx, task, assignee, due, source_ms, due_date)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for (i, a) in n.action_items.iter().enumerate() {
                let src = source_ms(a.sumber.as_deref(), duration_ms);
                let due_date = due_date_of(a.tenggat.as_deref());
                stmt.execute(params![meeting_id, i as i64, a.tugas, a.penanggung_jawab, a.tenggat, src, due_date])?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn get(conn: &Connection, meeting_id: &str) -> AppResult<Option<SummaryView>> {
    let row = conn
        .query_row(
            "SELECT status, summary, decisions, topics, edited, decision_sources, follow_up, key_points, open_questions,
                    open_question_sources, followup_status, followup_from
             FROM summaries WHERE meeting_id = ?1",
            [meeting_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, bool>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    (r.get::<_, String>(7)?, r.get::<_, String>(8)?, r.get::<_, String>(9)?),
                    (r.get::<_, String>(10)?, r.get::<_, Option<String>>(11)?),
                ))
            },
        )
        .optional()?;
    let row = row.map(|(status, summary, decisions, topics, edited, sources, follow_up, kp, (fs, ff))| {
        let mut items: Vec<FollowupItem> = serde_json::from_str(&fs).unwrap_or_default();
        for it in &mut items {
            it.done = conn
                .query_row("SELECT done FROM action_items WHERE id = ?1", [it.item_id], |r| r.get::<_, bool>(0))
                .unwrap_or(false);
        }
        let from = ff.and_then(|mid| {
            conn.query_row("SELECT title FROM meetings WHERE id = ?1", [&mid], |r| r.get::<_, String>(0))
                .ok()
                .map(|title| FollowupFrom { meeting_id: mid, title })
        });
        (status, summary, decisions, topics, edited, sources, follow_up, kp, items, from)
    });
    Ok(row.map(|(status, summary, decisions, topics, edited, sources, follow_up, (key_points, open, open_src), followup_status, followup_from)| {
        let decisions: Vec<String> = serde_json::from_str(&decisions).unwrap_or_default();
        let mut decision_sources: Vec<Option<i64>> = serde_json::from_str(&sources).unwrap_or_default();
        decision_sources.resize(decisions.len(), None);
        let open_questions: Vec<String> = serde_json::from_str(&open).unwrap_or_default();
        let mut open_question_sources: Vec<Option<i64>> = serde_json::from_str(&open_src).unwrap_or_default();
        open_question_sources.resize(open_questions.len(), None);
        SummaryView {
            key_points: serde_json::from_str(&key_points).unwrap_or_default(),
            open_questions,
            open_question_sources,
            status,
            summary,
            decisions,
            decision_sources,
            topics: serde_json::from_str(&topics).unwrap_or_default(),
            edited,
            follow_up: follow_up.and_then(|f| serde_json::from_str(&f).ok()),
            followup_status,
            followup_from,
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
            "INSERT INTO action_items (meeting_id, idx, task, assignee, due, done, source_ms, due_date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        let items = e.action_items.iter().filter_map(|a| clean(&a.task).map(|task| (task, a)));
        for (i, (task, a)) in items.enumerate() {
            let assignee = a.assignee.as_deref().and_then(clean);
            let due = a.due.as_deref().and_then(clean);
            let src = old_items.iter().find(|o| o.task == task).and_then(|o| o.source_ms);
            let due_date = due_date_of(due.as_deref());
            stmt.execute(params![meeting_id, i as i64, task, assignee, due, a.done, src, due_date])?;
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
    /// Tenggat terstruktur `YYYY-MM-DD` (langkah 54).
    pub due_date: Option<String>,
}

/// Semua action item meeting `done`: belum selesai dulu, lalu meeting terbaru.
pub fn all_action_items(conn: &Connection) -> AppResult<Vec<TaskView>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.meeting_id, m.title, m.started_at, a.task, a.assignee, a.due, a.done, a.due_date
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
                due_date: r.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}


/// Ubah satu tugas dari halaman Tugas (langkah 54). Field `None` tidak diubah; string kosong → NULL.
pub fn update_action_item(
    conn: &Connection,
    item_id: i64,
    task: Option<&str>,
    assignee: Option<&str>,
    due_date: Option<&str>,
) -> AppResult<()> {
    if let Some(t) = task.map(str::trim).filter(|t| !t.is_empty()) {
        conn.execute("UPDATE action_items SET task = ?2 WHERE id = ?1", params![item_id, t])?;
    }
    if let Some(a) = assignee {
        conn.execute("UPDATE action_items SET assignee = ?2 WHERE id = ?1", params![item_id, clean(a)])?;
    }
    if let Some(d) = due_date {
        let d = due_date_of(Some(d));
        // Teks tenggat ikut tanggal yang dipilih agar tampilan, salin, dan ekspor konsisten.
        conn.execute("UPDATE action_items SET due_date = ?2, due = ?2 WHERE id = ?1", params![item_id, d])?;
    }
    Ok(())
}

/// Tambah tugas manual ke sebuah meeting (urutan terakhir).
pub fn add_action_item(
    conn: &Connection,
    meeting_id: &str,
    task: &str,
    assignee: Option<&str>,
    due_date: Option<&str>,
) -> AppResult<i64> {
    let idx: i64 =
        conn.query_row("SELECT COALESCE(MAX(idx), -1) + 1 FROM action_items WHERE meeting_id = ?1", [meeting_id], |r| r.get(0))?;
    let due_date = due_date_of(due_date);
    conn.execute(
        "INSERT INTO action_items (meeting_id, idx, task, assignee, due, due_date) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![meeting_id, idx, task.trim(), assignee.and_then(clean), due_date],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Jumlah tugas terbuka yang tenggatnya hari ini / sudah lewat (untuk pengingat harian).
pub fn due_counts(conn: &Connection, today: &str) -> AppResult<(i64, i64)> {
    let q = |sql: &str| -> AppResult<i64> {
        Ok(conn.query_row(sql, [today], |r| r.get(0))?)
    };
    let base = "SELECT COUNT(*) FROM action_items a JOIN meetings m ON m.id = a.meeting_id
                WHERE m.status = 'done' AND a.done = 0 AND a.due_date IS NOT NULL AND a.due_date ";
    Ok((q(&format!("{base} = ?1"))?, q(&format!("{base} < ?1"))?))
}
