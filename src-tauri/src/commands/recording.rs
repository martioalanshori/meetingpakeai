use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};
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

/// Mulai rekam langsung. Popup consent dihapus atas keputusan pemilik (consent diminta di luar aplikasi).
#[tauri::command]
pub async fn start_recording(state: State<'_, AppState>, source_app: Option<String>) -> AppResult<StartResult> {
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

/// Tambahan (langkah 46): tawaran rekam diabaikan → tidak diingatkan ulang untuk sesi mic ini.
#[tauri::command]
pub async fn dismiss_meeting_offer(state: State<'_, AppState>) -> AppResult<()> {
    crate::meeting_watch::dismiss_offer();
    let _ = state.bridge.take_pending_offer();
    Ok(())
}

/// Tambahan (langkah 44): tandai momen penting saat merekam. Mengembalikan posisi (ms).
#[tauri::command]
pub async fn add_bookmark(state: State<'_, AppState>) -> AppResult<i64> {
    state.recording.bookmark()
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

/// Tambahan: tawaran dari notifikasi "Meeting terdeteksi" (jenis aplikasi) untuk banner "Mulai rekam?"
/// di jendela main. Tidak langsung merekam agar membuka jendela tidak memulai rekaman tanpa sengaja.
#[tauri::command]
pub async fn take_pending_offer(state: State<'_, AppState>) -> AppResult<Option<String>> {
    Ok(state.bridge.take_pending_offer())
}
