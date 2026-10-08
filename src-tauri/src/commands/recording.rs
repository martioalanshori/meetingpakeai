use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::recording::{RecordingState, StopReason};
use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingIdResult {
    pub meeting_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartResult {
    pub meeting_id: String,
}

/// `consentConfirmed` wajib `true` (PRD §12.3, F5).
#[tauri::command]
pub async fn start_recording(
    state: State<'_, AppState>,
    consent_confirmed: bool,
    source_app: Option<String>,
) -> AppResult<StartResult> {
    if !consent_confirmed {
        return Err(AppError::new(ErrorCode::InvalidState));
    }
    let svc = state.recording.clone();
    // Membuka device WASAPI bisa memblok sebentar; jalankan di thread blocking.
    let meeting_id = tauri::async_runtime::spawn_blocking(move || svc.start(source_app))
        .await
        .map_err(AppError::internal)??;
    Ok(StartResult { meeting_id })
}

#[tauri::command]
pub async fn pause_recording(state: State<'_, AppState>) -> AppResult<RecordingState> {
    state.recording.pause()
}

#[tauri::command]
pub async fn resume_recording(state: State<'_, AppState>) -> AppResult<RecordingState> {
    state.recording.resume()
}

#[tauri::command]
pub async fn set_mic_muted(state: State<'_, AppState>, muted: bool) -> AppResult<RecordingState> {
    state.recording.set_mic_muted(muted)
}

#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>) -> AppResult<MeetingIdResult> {
    let svc = state.recording.clone();
    let meeting_id = tauri::async_runtime::spawn_blocking(move || svc.stop(StopReason::Manual))
        .await
        .map_err(AppError::internal)??;
    Ok(MeetingIdResult { meeting_id })
}

#[tauri::command]
pub async fn get_recording_state(state: State<'_, AppState>) -> AppResult<RecordingState> {
    Ok(state.recording.state())
}

#[tauri::command]
pub async fn respond_auto_stop(state: State<'_, AppState>, continue_recording: bool) -> AppResult<()> {
    let svc = state.recording.clone();
    tauri::async_runtime::spawn_blocking(move || svc.respond_auto_stop(continue_recording))
        .await
        .map_err(AppError::internal)?
}

/// Tambahan: jendela main yang mendapat fokus menanyakan meeting dari notifikasi "Notulen siap".
#[tauri::command]
pub async fn take_pending_meeting(state: State<'_, AppState>) -> AppResult<Option<String>> {
    Ok(state.bridge.take_pending_meeting())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingConsent {
    pub open: bool,
    /// Jenis aplikasi dari deteksi meeting (`zoom` / `teams` / `browser`).
    pub source_app: Option<String>,
}

/// Tambahan: jendela main (baru dibuat / mendapat fokus) menanyakan apakah popup consent perlu dibuka:
/// dari menu tray / shortcut "Mulai rekam", atau dari notifikasi "Meeting terdeteksi".
#[tauri::command]
pub async fn take_pending_consent(state: State<'_, AppState>) -> AppResult<PendingConsent> {
    let from_tray = state.pending_consent.swap(false, std::sync::atomic::Ordering::SeqCst);
    let source_app = state.bridge.take_pending_offer();
    Ok(PendingConsent { open: from_tray || source_app.is_some(), source_app })
}
