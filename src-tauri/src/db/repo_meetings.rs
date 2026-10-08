//! Tabel `meetings` dan `action_items` (operasi dasar). Query lain ditambahkan di langkah terkait.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::db::now_ms;
use crate::error::{AppError, AppResult, ErrorCode};

/// Status meeting/job (PRD §13). String DB = snake_case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeetingStatus {
    Recording,
    Interrupted,
    Queued,
    Preprocessing,
    Transcribing,
    Merging,
    Summarizing,
    Done,
    WaitingQuota,
    WaitingNetwork,
    Failed,
}

impl MeetingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::Interrupted => "interrupted",
            Self::Queued => "queued",
            Self::Preprocessing => "preprocessing",
            Self::Transcribing => "transcribing",
            Self::Merging => "merging",
            Self::Summarizing => "summarizing",
            Self::Done => "done",
            Self::WaitingQuota => "waiting_quota",
            Self::WaitingNetwork => "waiting_network",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "recording" => Self::Recording,
            "interrupted" => Self::Interrupted,
            "queued" => Self::Queued,
            "preprocessing" => Self::Preprocessing,
            "transcribing" => Self::Transcribing,
            "merging" => Self::Merging,
            "summarizing" => Self::Summarizing,
            "done" => Self::Done,
            "waiting_quota" => Self::WaitingQuota,
            "waiting_network" => Self::WaitingNetwork,
            "failed" => Self::Failed,
            _ => return None,
        })
    }

    /// Step pipeline yang sedang berjalan (bisa di-resume).
    pub fn is_running_step(self) -> bool {
        matches!(self, Self::Preprocessing | Self::Transcribing | Self::Merging | Self::Summarizing)
    }
}

impl rusqlite::types::FromSql for MeetingStatus {
    fn column_result(v: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let s = v.as_str()?;
        Self::parse(s).ok_or_else(|| rusqlite::types::FromSqlError::Other(format!("status tidak dikenal: {s}").into()))
    }
}

/// Satu baris tabel `meetings`.
#[derive(Debug, Clone)]
pub struct MeetingRow {
    pub id: String,
    pub title: String,
    pub title_edited: bool,
    pub created_at: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub duration_ms: i64,
    pub language: String,
    pub source_app: Option<String>,
    pub consent_at: i64,
    pub status: MeetingStatus,
    pub failed_step: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub progress_done: i64,
    pub progress_total: i64,
    pub attempts: i64,
    pub next_run_at: Option<i64>,
    pub audio_deleted: bool,
    pub updated_at: i64,
}

const COLS: &str = "id, title, title_edited, created_at, started_at, ended_at, duration_ms, language, \
    source_app, consent_at, status, failed_step, error_code, error_message, progress_done, \
    progress_total, attempts, next_run_at, audio_deleted, updated_at";

fn map_row(r: &Row<'_>) -> rusqlite::Result<MeetingRow> {
    Ok(MeetingRow {
        id: r.get(0)?,
        title: r.get(1)?,
        title_edited: r.get(2)?,
        created_at: r.get(3)?,
        started_at: r.get(4)?,
        ended_at: r.get(5)?,
        duration_ms: r.get(6)?,
        language: r.get(7)?,
        source_app: r.get(8)?,
        consent_at: r.get(9)?,
        status: r.get(10)?,
        failed_step: r.get(11)?,
        error_code: r.get(12)?,
        error_message: r.get(13)?,
        progress_done: r.get(14)?,
        progress_total: r.get(15)?,
        attempts: r.get(16)?,
        next_run_at: r.get(17)?,
        audio_deleted: r.get(18)?,
        updated_at: r.get(19)?,
    })
}

/// Data untuk membuat meeting baru saat Start.
pub struct NewMeeting<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub started_at: i64,
    pub language: &'a str,
    pub source_app: Option<&'a str>,
    pub consent_at: i64,
}

pub fn insert(conn: &Connection, m: &NewMeeting<'_>) -> AppResult<()> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO meetings (id, title, created_at, started_at, language, source_app, consent_at, status, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'recording', ?3)",
        params![m.id, m.title, now, m.started_at, m.language, m.source_app, m.consent_at],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, id: &str) -> AppResult<MeetingRow> {
    conn.query_row(&format!("SELECT {COLS} FROM meetings WHERE id = ?1"), [id], map_row)
        .optional()?
        .ok_or_else(|| AppError::new(ErrorCode::NotFound))
}

/// Urut `started_at` DESC (PRD §12.3 `list_meetings`).
pub fn list(conn: &Connection, limit: i64, offset: i64) -> AppResult<Vec<MeetingRow>> {
    let mut stmt =
        conn.prepare(&format!("SELECT {COLS} FROM meetings ORDER BY started_at DESC LIMIT ?1 OFFSET ?2"))?;
    let rows = stmt.query_map(params![limit, offset], map_row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn list_by_status(conn: &Connection, status: MeetingStatus) -> AppResult<Vec<MeetingRow>> {
    let mut stmt =
        conn.prepare(&format!("SELECT {COLS} FROM meetings WHERE status = ?1 ORDER BY started_at ASC"))?;
    let rows = stmt.query_map([status.as_str()], map_row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Judul dari pengguna: set `title_edited = 1`.
pub fn rename(conn: &Connection, id: &str, title: &str) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE meetings SET title = ?2, title_edited = 1, updated_at = ?3 WHERE id = ?1",
        params![id, title, now_ms()],
    )?;
    if n == 0 {
        return Err(ErrorCode::NotFound.into());
    }
    Ok(())
}

/// Judul dari LLM: hanya jika pengguna belum pernah mengubah judul (PRD §8.6).
pub fn set_generated_title(conn: &Connection, id: &str, title: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE meetings SET title = ?2, updated_at = ?3 WHERE id = ?1 AND title_edited = 0",
        params![id, title, now_ms()],
    )?;
    Ok(())
}

/// Pindah status; progres di-reset, error dibersihkan (kecuali status `failed`, pakai `set_failed`).
pub fn set_status(conn: &Connection, id: &str, status: MeetingStatus) -> AppResult<()> {
    conn.execute(
        "UPDATE meetings SET status = ?2, progress_done = 0, progress_total = 0,
           failed_step = NULL, error_code = NULL, error_message = NULL, next_run_at = NULL, updated_at = ?3
         WHERE id = ?1",
        params![id, status.as_str(), now_ms()],
    )?;
    Ok(())
}

pub fn set_progress(conn: &Connection, id: &str, done: i64, total: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE meetings SET progress_done = ?2, progress_total = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, done, total, now_ms()],
    )?;
    Ok(())
}

pub fn set_failed(conn: &Connection, id: &str, step: MeetingStatus, err: &AppError) -> AppResult<()> {
    conn.execute(
        "UPDATE meetings SET status = 'failed', failed_step = ?2, error_code = ?3, error_message = ?4, updated_at = ?5
         WHERE id = ?1",
        params![id, step.as_str(), err.code.as_str(), err.message, now_ms()],
    )?;
    Ok(())
}

/// `waiting_quota` / `waiting_network` dengan jadwal lanjut; step semula disimpan di `failed_step`.
pub fn set_waiting(conn: &Connection, id: &str, status: MeetingStatus, step: MeetingStatus, next_run_at: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE meetings SET status = ?2, failed_step = ?3, next_run_at = ?4, updated_at = ?5 WHERE id = ?1",
        params![id, status.as_str(), step.as_str(), next_run_at, now_ms()],
    )?;
    Ok(())
}

pub fn finish_recording(conn: &Connection, id: &str, ended_at: i64, duration_ms: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE meetings SET status = 'queued', ended_at = ?2, duration_ms = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, ended_at, duration_ms, now_ms()],
    )?;
    Ok(())
}

/// Durasi audio yang terselamatkan; `ended_at` diperkirakan `started_at + durasi` (meeting terputus).
pub fn set_duration(conn: &Connection, id: &str, duration_ms: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE meetings SET duration_ms = ?2, ended_at = COALESCE(ended_at, started_at + ?2), updated_at = ?3 WHERE id = ?1",
        params![id, duration_ms, now_ms()],
    )?;
    Ok(())
}

/// Meeting yang audionya masih ada dan berakhir sebelum `cutoff` (retensi 7 hari).
/// Hanya status akhir (`done` / `failed`); meeting yang masih diproses tidak disentuh.
pub fn audio_expired(conn: &Connection, cutoff: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM meetings WHERE audio_deleted = 0 AND status IN ('done', 'failed')
         AND COALESCE(ended_at, started_at) < ?1",
    )?;
    let ids = stmt.query_map([cutoff], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
    Ok(ids)
}

pub fn set_audio_deleted(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE meetings SET audio_deleted = 1, updated_at = ?2 WHERE id = ?1",
        params![id, now_ms()],
    )?;
    Ok(())
}

/// Hapus meeting; tabel anak ikut terhapus lewat ON DELETE CASCADE.
pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    crate::db::repo_search::delete(conn, id)?;
    conn.execute("DELETE FROM meetings WHERE id = ?1", [id])?;
    Ok(())
}

pub fn set_action_item_done(conn: &Connection, item_id: i64, done: bool) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE action_items SET done = ?2 WHERE id = ?1",
        params![item_id, done],
    )?;
    if n == 0 {
        return Err(ErrorCode::NotFound.into());
    }
    Ok(())
}

/// Ada meeting gagal karena API key tidak valid → antrean dijeda (keputusan di CLAUDE.md).
pub fn has_invalid_key_failure(conn: &Connection) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM meetings WHERE status = 'failed' AND error_code = 'INVALID_API_KEY')",
        [],
        |r| r.get(0),
    )?)
}

/// Meeting berikutnya untuk worker (FIFO `started_at`): step berjalan/antre, atau `waiting_*` yang jadwalnya tiba.
pub fn next_job(conn: &Connection, now: i64) -> AppResult<Option<MeetingRow>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {COLS} FROM meetings
                 WHERE status IN ('queued','preprocessing','transcribing','merging','summarizing')
                    OR (status IN ('waiting_quota','waiting_network') AND COALESCE(next_run_at, 0) <= ?1)
                 ORDER BY started_at ASC LIMIT 1"
            ),
            [now],
            map_row,
        )
        .optional()?)
}

/// Jadwal `waiting_*` paling awal (untuk tidur worker).
pub fn earliest_waiting(conn: &Connection) -> AppResult<Option<i64>> {
    Ok(conn.query_row(
        "SELECT MIN(next_run_at) FROM meetings WHERE status IN ('waiting_quota','waiting_network')",
        [],
        |r| r.get(0),
    )?)
}

/// Meeting yang gagal karena API key tidak valid → kembali ke step semula (setelah key baru tersimpan).
pub fn requeue_invalid_key(conn: &Connection) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE meetings SET status = COALESCE(failed_step, 'queued'), failed_step = NULL, error_code = NULL,
           error_message = NULL, progress_done = 0, progress_total = 0, updated_at = ?1
         WHERE status = 'failed' AND error_code IN ('INVALID_API_KEY', 'NO_API_KEY')",
        [now_ms()],
    )?)
}
