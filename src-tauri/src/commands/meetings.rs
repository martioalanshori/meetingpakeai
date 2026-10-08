use serde::Serialize;
use tauri::State;

use crate::config::settings::{self, DEFAULT_SYSTEM_LABEL};
use crate::db::repo_meetings::{self, MeetingRow, MeetingStatus};
use crate::db::repo_segments::{self, VisibleSegment};
use crate::db::repo_summary::{self, ActionItemView, SummaryEdit, SummaryView};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::events::{self, MeetingUpdated};
use crate::AppState;

const TITLE_MAX_CHARS: usize = 100;
const SPEAKER_NAME_MAX_CHARS: usize = 50;

/// `MeetingListItem` (PRD §12.2).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingListItem {
    pub id: String,
    pub title: String,
    pub started_at: i64,
    pub duration_ms: i64,
    pub status: MeetingStatus,
    pub progress_done: i64,
    pub progress_total: i64,
    pub error_message: Option<String>,
    /// Tambahan: untuk banner "antrean dijeda" di Beranda.
    pub error_code: Option<String>,
}

impl From<&MeetingRow> for MeetingListItem {
    fn from(m: &MeetingRow) -> Self {
        Self {
            id: m.id.clone(),
            title: m.title.clone(),
            started_at: m.started_at,
            duration_ms: m.duration_ms,
            status: m.status,
            progress_done: m.progress_done,
            progress_total: m.progress_total,
            error_message: m.error_message.clone(),
            error_code: m.error_code.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Labels {
    pub mic: String,
    pub system: String,
}

/// `MeetingDetail` (PRD §12.2).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingDetail {
    #[serde(flatten)]
    pub base: MeetingListItem,
    pub ended_at: Option<i64>,
    pub language: String,
    pub failed_step: Option<String>,
    pub audio_deleted: bool,
    pub labels: Labels,
    pub summary: Option<SummaryView>,
    pub action_items: Vec<ActionItemView>,
}

fn emit_updated(state: &AppState, id: &str) {
    events::emit(state.bridge.as_ref(), events::EV_MEETING_UPDATED, &MeetingUpdated { meeting_id: id });
}

#[tauri::command]
pub async fn list_meetings(state: State<'_, AppState>, limit: i64, offset: i64) -> AppResult<Vec<MeetingListItem>> {
    let rows = repo_meetings::list(&state.db.conn(), limit.clamp(1, 500), offset.max(0))?;
    Ok(rows.iter().map(MeetingListItem::from).collect())
}

#[tauri::command]
pub async fn get_meeting(state: State<'_, AppState>, id: String) -> AppResult<MeetingDetail> {
    let conn = state.db.conn();
    let m = repo_meetings::get(&conn, &id)?;
    // Label dihitung saat tampil, bukan disimpan per segment (AC F9.1).
    let labels = Labels {
        mic: settings::load(&conn)?.user_display_name,
        system: repo_summary::system_label(&conn, &id)?.unwrap_or_else(|| DEFAULT_SYSTEM_LABEL.to_string()),
    };
    Ok(MeetingDetail {
        base: MeetingListItem::from(&m),
        ended_at: m.ended_at,
        language: m.language.clone(),
        failed_step: m.failed_step.clone(),
        audio_deleted: m.audio_deleted,
        labels,
        summary: repo_summary::get(&conn, &id)?,
        action_items: repo_summary::action_items(&conn, &id)?,
    })
}

#[tauri::command]
pub async fn get_transcript(state: State<'_, AppState>, id: String) -> AppResult<Vec<VisibleSegment>> {
    let conn = state.db.conn();
    repo_meetings::get(&conn, &id)?;
    repo_segments::list_visible(&conn, &id)
}

#[tauri::command]
pub async fn rename_meeting(state: State<'_, AppState>, id: String, title: String) -> AppResult<()> {
    let title: String = title.trim().chars().take(TITLE_MAX_CHARS).collect();
    if title.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidState));
    }
    repo_meetings::rename(&state.db.conn(), &id, &title)?;
    emit_updated(&state, &id);
    Ok(())
}

/// Tambahan (langkah 26): path WAV campuran mic+sistem untuk diputar (dibuat sekali, dipakai ulang).
#[tauri::command]
pub async fn prepare_playback(state: State<'_, AppState>, id: String) -> AppResult<String> {
    let m = repo_meetings::get(&state.db.conn(), &id)?;
    if m.audio_deleted || m.status == MeetingStatus::Recording {
        return Err(AppError::new(ErrorCode::AudioNotAvailable));
    }
    let (data_dir, db) = (state.data_dir.clone(), state.db.clone());
    let path = tauri::async_runtime::spawn_blocking(move || crate::playback::prepare(&data_dir, &db, &id))
        .await
        .map_err(AppError::internal)??;
    Ok(path.to_string_lossy().into_owned())
}

/// Tambahan (langkah 26): simpan ekspor notulen ke file pilihan pengguna. Dialog dibuka di sini
/// sehingga UI tidak bisa menulis ke path sembarang. `false` jika dialog dibatalkan.
#[tauri::command]
pub async fn save_export(app: tauri::AppHandle, file_name: String, contents: String) -> AppResult<bool> {
    use tauri_plugin_dialog::DialogExt;
    let (label, ext) = if file_name.ends_with(".txt") { ("Teks", "txt") } else { ("Markdown", "md") };
    let picked = app.dialog().file().set_file_name(&file_name).add_filter(label, &[ext]).blocking_save_file();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(false) };
    std::fs::write(&path, contents)?;
    tracing::info!("notulen diekspor ({ext})");
    Ok(true)
}

/// Tambahan (langkah 23): simpan ringkasan & action item hasil edit pengguna.
#[tauri::command]
pub async fn update_summary(state: State<'_, AppState>, id: String, edit: SummaryEdit) -> AppResult<()> {
    let m = repo_meetings::get(&state.db.conn(), &id)?;
    if m.status != MeetingStatus::Done {
        return Err(AppError::new(ErrorCode::InvalidState));
    }
    if !repo_summary::update(&mut state.db.conn(), &id, &edit)? {
        return Err(AppError::new(ErrorCode::InvalidState));
    }
    emit_updated(&state, &id);
    Ok(())
}

/// Tambahan (langkah 23, F10): ganti label "Peserta lain" untuk satu meeting; kosong = default.
#[tauri::command]
pub async fn set_speaker_name(state: State<'_, AppState>, id: String, name: String) -> AppResult<()> {
    let conn = state.db.conn();
    repo_meetings::get(&conn, &id)?;
    let name: String = name.trim().chars().take(SPEAKER_NAME_MAX_CHARS).collect();
    repo_summary::set_system_label(&conn, &id, &name)?;
    drop(conn);
    emit_updated(&state, &id);
    Ok(())
}

#[tauri::command]
pub async fn set_action_item_done(state: State<'_, AppState>, id: i64, done: bool) -> AppResult<()> {
    repo_meetings::set_action_item_done(&state.db.conn(), id, done)
}

#[tauri::command]
pub async fn delete_meeting(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let m = repo_meetings::get(&state.db.conn(), &id)?;
    if m.status == MeetingStatus::Recording {
        return Err(AppError::new(ErrorCode::InvalidState));
    }
    // Hentikan request Groq yang sedang berjalan untuk meeting ini sebelum datanya dihapus (A6).
    state.worker.cancel_and_wait(&id).await;
    repo_meetings::delete(&state.db.conn(), &id)?;
    let dir = state.data_dir.join("recordings").join(&id);
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            tracing::warn!("folder audio meeting {id} gagal dihapus: {e}");
        }
    }
    tracing::info!("meeting {id} dihapus");
    emit_updated(&state, &id);
    Ok(())
}

fn requeue(state: &AppState, id: &str, step: MeetingStatus) -> AppResult<()> {
    repo_meetings::set_status(&state.db.conn(), id, step)?;
    state.queue_wake.notify_one();
    emit_updated(state, id);
    Ok(())
}

/// Hanya untuk `failed` / `waiting_*`; lanjut dari `failed_step`.
#[tauri::command]
pub async fn retry_job(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let m = repo_meetings::get(&state.db.conn(), &id)?;
    if !matches!(m.status, MeetingStatus::Failed | MeetingStatus::WaitingQuota | MeetingStatus::WaitingNetwork) {
        return Err(AppError::new(ErrorCode::InvalidState));
    }
    let step = m
        .failed_step
        .as_deref()
        .and_then(MeetingStatus::parse)
        .filter(|s| s.is_running_step())
        .unwrap_or(MeetingStatus::Preprocessing);
    if step == MeetingStatus::Preprocessing && m.audio_deleted {
        return Err(AppError::new(ErrorCode::AudioNotAvailable));
    }
    requeue(&state, &id, step)
}

/// Hanya jika `done` atau `failed` di `summarizing`.
#[tauri::command]
pub async fn regenerate_summary(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let m = repo_meetings::get(&state.db.conn(), &id)?;
    let allowed = m.status == MeetingStatus::Done
        || (m.status == MeetingStatus::Failed && m.failed_step.as_deref() == Some("summarizing"));
    if !allowed {
        return Err(AppError::new(ErrorCode::InvalidState));
    }
    requeue(&state, &id, MeetingStatus::Summarizing)
}

#[tauri::command]
pub async fn retranscribe(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let m = repo_meetings::get(&state.db.conn(), &id)?;
    if m.audio_deleted {
        return Err(AppError::new(ErrorCode::AudioNotAvailable));
    }
    let busy = m.status == MeetingStatus::Recording
        || m.status.is_running_step()
        || m.status == MeetingStatus::Queued
        || state.worker.current_meeting().as_deref() == Some(id.as_str());
    if busy {
        return Err(AppError::new(ErrorCode::InvalidState));
    }
    requeue(&state, &id, MeetingStatus::Preprocessing)
}

#[tauri::command]
pub async fn resolve_interrupted(state: State<'_, AppState>, id: String, action: String) -> AppResult<()> {
    let m = repo_meetings::get(&state.db.conn(), &id)?;
    if m.status != MeetingStatus::Interrupted {
        return Err(AppError::new(ErrorCode::InvalidState));
    }
    match action.as_str() {
        "process" => requeue(&state, &id, MeetingStatus::Queued),
        "discard" => {
            repo_meetings::delete(&state.db.conn(), &id)?;
            let _ = std::fs::remove_dir_all(state.data_dir.join("recordings").join(&id));
            emit_updated(&state, &id);
            Ok(())
        }
        _ => Err(AppError::new(ErrorCode::InvalidState)),
    }
}
