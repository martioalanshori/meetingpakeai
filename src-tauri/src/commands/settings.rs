use std::sync::atomic::Ordering;

use tauri::{AppHandle, State};

use crate::config::settings::{self, Settings, SettingsPatch};
use crate::desktop;
use crate::error::AppResult;
use crate::AppState;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    settings::load(&state.db.conn())
}

#[tauri::command]
pub async fn update_settings(app: AppHandle, state: State<'_, AppState>, patch: SettingsPatch) -> AppResult<Settings> {
    let current = settings::load(&state.db.conn())?;
    // Efek ke sistem dulu; gagal → setting tidak disimpan.
    if let Some(sc) = &patch.global_shortcut {
        let sc = sc.trim();
        if sc != current.global_shortcut {
            desktop::set_shortcut(&app, &current.global_shortcut, sc)?;
        }
    }
    if let Some(v) = patch.autostart {
        if v != current.autostart {
            desktop::set_autostart(&app, v)?;
        }
    }
    let updated = settings::apply_patch(&state.db.conn(), patch)?;
    state.minimize_to_tray.store(updated.minimize_to_tray, Ordering::Relaxed);
    Ok(updated)
}

/// Tambahan (langkah 25): versi baru dari endpoint updater; `null` jika terbaru / updater nonaktif.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> AppResult<Option<crate::updater::UpdateInfo>> {
    crate::updater::check(&app).await
}

/// Tambahan (langkah 25): unduh, pasang, dan mulai ulang aplikasi.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> AppResult<()> {
    crate::updater::install(&app).await
}
