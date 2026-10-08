use std::sync::atomic::Ordering;

use tauri::{AppHandle, State};

use crate::config::settings::{self, Settings, SettingsPatch};
use crate::desktop;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::AppState;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    settings::load(&state.db.conn())
}

#[tauri::command]
pub async fn update_settings(app: AppHandle, state: State<'_, AppState>, patch: SettingsPatch) -> AppResult<Settings> {
    let current = settings::load(&state.db.conn())?;
    // Efek ke sistem dulu; gagal → setting tidak disimpan.
    let same = |a: &str, b: &str| !a.is_empty() && a.eq_ignore_ascii_case(b);
    if let Some(sc) = &patch.global_shortcut {
        let sc = sc.trim();
        if same(sc, &current.bookmark_shortcut) {
            return Err(AppError::with_message(ErrorCode::InvalidState, "Shortcut ini sudah dipakai untuk tandai momen."));
        }
        if sc != current.global_shortcut {
            desktop::set_shortcut(&app, &current.global_shortcut, sc)?;
        }
    }
    if let Some(sc) = &patch.bookmark_shortcut {
        let sc = sc.trim();
        if same(sc, &current.global_shortcut) {
            return Err(AppError::with_message(ErrorCode::InvalidState, "Shortcut ini sudah dipakai untuk mulai/hentikan rekaman."));
        }
        if sc != current.bookmark_shortcut {
            desktop::set_shortcut(&app, &current.bookmark_shortcut, sc)?;
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

/// Ukuran maksimal log yang dimasukkan ke laporan (bagian akhir file terbaru).
const REPORT_MAX_LOG_BYTES: usize = 4 * 1024 * 1024;

/// Tambahan (langkah 29, C5.5): simpan laporan masalah (versi, OS, pengaturan non-rahasia, log) ke
/// file pilihan pengguna. Tanpa audio, transkrip, ringkasan, atau API key. `false` jika dibatalkan.
#[tauri::command]
pub async fn save_problem_report(app: AppHandle, state: State<'_, AppState>) -> AppResult<bool> {
    use tauri_plugin_dialog::DialogExt;
    let mut report = String::new();
    report.push_str(&format!("Meeting Pake AI {}\n", app.package_info().version));
    report.push_str(&format!("OS: {} {}\n", std::env::consts::OS, std::env::consts::ARCH));
    report.push_str(&format!("Dibuat: {}\n\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S %z")));
    let mut s = settings::load(&state.db.conn())?;
    s.user_display_name = "(disembunyikan)".into();
    report.push_str(&format!("Pengaturan: {}\n", serde_json::to_string(&s)?));
    report.push_str(&format!("Antrean dijeda: {}\n", state.worker.is_paused()));
    for (name, role) in [("Transkrip", crate::ai::Role::Stt), ("Ringkasan", crate::ai::Role::Llm)] {
        let e = crate::ai::endpoint(&state.db.conn(), role, &state.providers)?;
        report.push_str(&format!("{name}: {} / {}\n", e.provider, e.model));
    }
    report.push('\n');

    let mut logs: Vec<_> = std::fs::read_dir(state.data_dir.join("logs"))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    // Crash terbaru (dari panic hook) ditaruh paling atas.
    let mut crashes: Vec<_> =
        logs.iter().filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("crash-"))).cloned().collect();
    crashes.sort();
    if let Some(latest) = crashes.last() {
        report.push_str(&format!("===== {} =====\n{}\n", latest.display(), std::fs::read_to_string(latest).unwrap_or_default()));
    }
    logs.retain(|p| !crashes.contains(p));
    logs.sort();
    let mut budget = REPORT_MAX_LOG_BYTES;
    let mut parts = Vec::new();
    // File terbaru dulu sampai anggaran habis, lalu ditulis urut waktu.
    for path in logs.iter().rev() {
        if budget == 0 {
            break;
        }
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let start = text.len().saturating_sub(budget);
        let start = (start..text.len()).find(|&i| text.is_char_boundary(i)).unwrap_or(text.len());
        budget -= text.len() - start;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        parts.push(format!("===== {name} =====\n{}\n", &text[start..]));
    }
    for p in parts.iter().rev() {
        report.push_str(p);
    }

    let file_name = format!("laporan-meeting-pake-ai-{}.txt", chrono::Local::now().format("%Y%m%d-%H%M"));
    let picked = app.dialog().file().set_file_name(&file_name).add_filter("Teks", &["txt"]).blocking_save_file();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(false) };
    std::fs::write(&path, report)?;
    tracing::info!("laporan masalah disimpan");
    Ok(true)
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

fn dir_size(path: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else { return 0 };
    entries
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    /// Total ukuran folder rekaman (byte).
    pub recordings_bytes: u64,
    /// Jumlah meeting selesai/gagal yang audionya masih disimpan.
    pub clearable_meetings: usize,
}

/// Tambahan (langkah 35): ukuran audio tersimpan di disk.
#[tauri::command]
pub async fn get_storage_usage(state: State<'_, AppState>) -> AppResult<StorageUsage> {
    let dir = state.data_dir.join("recordings");
    let recordings_bytes = tauri::async_runtime::spawn_blocking(move || dir_size(&dir)).await.map_err(crate::error::AppError::internal)?;
    let clearable_meetings = crate::db::repo_meetings::audio_clearable(&state.db.conn())?.len();
    Ok(StorageUsage { recordings_bytes, clearable_meetings })
}

/// Tambahan (langkah 35): hapus audio semua meeting selesai/gagal (transkrip & notulen tetap ada).
#[tauri::command]
pub async fn clear_old_audio(state: State<'_, AppState>) -> AppResult<usize> {
    let ids = crate::db::repo_meetings::audio_clearable(&state.db.conn())?;
    let root = state.data_dir.join("recordings");
    for id in &ids {
        let dir = root.join(id);
        let _ = tauri::async_runtime::spawn_blocking(move || if dir.exists() { std::fs::remove_dir_all(&dir) } else { Ok(()) }).await;
        crate::db::repo_meetings::set_audio_deleted(&state.db.conn(), id)?;
    }
    tracing::info!("audio {} meeting dihapus oleh pengguna", ids.len());
    crate::events::emit(state.bridge.as_ref(), crate::events::EV_MEETING_UPDATED, &crate::events::MeetingUpdated { meeting_id: "" });
    Ok(ids.len())
}
