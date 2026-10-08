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

/// Tambahan (langkah 29, C5.2): pemakaian kuota Groq hari ini dari `usage_log`.
#[tauri::command]
pub async fn get_quota_today(state: State<'_, AppState>) -> AppResult<crate::queue::rate_limiter::QuotaToday> {
    let conn = state.db.conn();
    let mut q = crate::queue::rate_limiter::quota_today(&conn, &state.providers.limits, crate::db::now_ms())?;
    // Batas harian hanya diketahui untuk Groq (free tier); penyedia lain mengikuti akun masing-masing.
    q.stt_groq = crate::ai::provider_of(&conn, crate::ai::Role::Stt)? == crate::ai::GROQ;
    q.llm_groq = crate::ai::provider_of(&conn, crate::ai::Role::Llm)? == crate::ai::GROQ;
    Ok(q)
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
