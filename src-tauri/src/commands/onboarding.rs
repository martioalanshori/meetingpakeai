use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::audio::test_tone::AudioTestResult;
use crate::config::settings;
use crate::error::{AppError, AppResult};
use crate::windows_integration::mic_permission;
use crate::{secrets, AppState};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingStatus {
    pub completed: bool,
    pub api_key_set: bool,
    /// 'allowed' | 'denied' | 'unknown'
    pub mic_permission: &'static str,
}

#[tauri::command]
pub async fn get_onboarding_status(state: State<'_, AppState>) -> AppResult<OnboardingStatus> {
    let completed = settings::onboarding_completed(&state.db.conn())?;
    Ok(OnboardingStatus {
        completed,
        api_key_set: secrets::get_api_key()?.is_some(),
        mic_permission: mic_permission::check().as_str(),
    })
}

#[tauri::command]
pub async fn complete_onboarding(state: State<'_, AppState>) -> AppResult<()> {
    settings::set_onboarding_completed(&state.db.conn())
}

#[tauri::command]
pub async fn check_mic_permission() -> AppResult<&'static str> {
    Ok(mic_permission::check().as_str())
}

#[tauri::command]
pub async fn open_mic_settings(app: AppHandle) -> AppResult<()> {
    app.opener()
        .open_url("ms-settings:privacy-microphone", None::<&str>)
        .map_err(AppError::internal)
}

#[tauri::command]
pub async fn run_audio_test(state: State<'_, AppState>) -> AppResult<AudioTestResult> {
    let svc = state.recording.clone();
    tauri::async_runtime::spawn_blocking(move || svc.run_audio_test())
        .await
        .map_err(AppError::internal)?
}

/// Pengaturan: tombol "Buka folder log" (PRD §14.6).
#[tauri::command]
pub async fn open_log_folder(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let dir = state.data_dir.join("logs");
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(AppError::internal)
}
