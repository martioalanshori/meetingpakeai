use std::sync::atomic::Ordering;

use tauri::State;

use crate::config::settings::{self, Settings, SettingsPatch};
use crate::error::AppResult;
use crate::AppState;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    settings::load(&state.db.conn())
}

#[tauri::command]
pub async fn update_settings(state: State<'_, AppState>, patch: SettingsPatch) -> AppResult<Settings> {
    let updated = settings::apply_patch(&state.db.conn(), patch)?;
    state.minimize_to_tray.store(updated.minimize_to_tray, Ordering::Relaxed);
    Ok(updated)
}
