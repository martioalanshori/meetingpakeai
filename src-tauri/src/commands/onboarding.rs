use serde::Serialize;
use tauri::State;

use crate::config::settings;
use crate::error::AppResult;
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
        // TODO langkah 8: baca registry izin mikrofon.
        mic_permission: "unknown",
    })
}

#[tauri::command]
pub async fn complete_onboarding(state: State<'_, AppState>) -> AppResult<()> {
    settings::set_onboarding_completed(&state.db.conn())
}
