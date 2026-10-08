//! Auto-update (langkah 25): `tauri-plugin-updater` + `latest.json` di GitHub Releases.
//! Kunci publik & endpoint disematkan saat build lewat env `MPA_UPDATER_PUBKEY` dan
//! `MPA_UPDATER_ENDPOINT`; tanpa keduanya updater nonaktif (build dev / build lokal).

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Updater, UpdaterExt};

use crate::error::{AppError, AppResult, ErrorCode};
use crate::events::EventSink;
use crate::AppState;

const PUBKEY: Option<&str> = option_env!("MPA_UPDATER_PUBKEY");
const ENDPOINT: Option<&str> = option_env!("MPA_UPDATER_ENDPOINT");
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(60);
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

/// Versi terakhir yang sudah diberitahukan (notifikasi sekali per versi).
static NOTIFIED: Mutex<Option<String>> = Mutex::new(None);

pub fn configured() -> bool {
    PUBKEY.is_some_and(|k| !k.is_empty()) && ENDPOINT.is_some_and(|e| !e.is_empty())
}

fn build(app: &AppHandle) -> AppResult<Updater> {
    let (Some(pubkey), Some(endpoint)) = (PUBKEY, ENDPOINT) else {
        return Err(AppError::with_message(ErrorCode::InvalidState, "Pembaruan otomatis tidak aktif di build ini."));
    };
    let url = endpoint.parse().map_err(AppError::internal)?;
    app.updater_builder()
        .pubkey(pubkey)
        .endpoints(vec![url])
        .map_err(AppError::internal)?
        .build()
        .map_err(AppError::internal)
}

/// Cek versi baru; `None` jika sudah terbaru. Updater nonaktif → error dengan pesan.
pub async fn check(app: &AppHandle) -> AppResult<Option<UpdateInfo>> {
    let update = build(app)?.check().await.map_err(|e| {
        tracing::warn!("cek pembaruan gagal: {e}");
        AppError::new(ErrorCode::Network)
    })?;
    Ok(update.map(|u| UpdateInfo { version: u.version, notes: u.body }))
}

/// Unduh & pasang. Ditolak saat merekam atau memproses meeting (installer menutup aplikasi).
pub async fn install(app: &AppHandle) -> AppResult<()> {
    if let Some(state) = app.try_state::<AppState>() {
        if state.recording.is_recording() || state.worker.current_meeting().is_some() {
            return Err(AppError::with_message(
                ErrorCode::InvalidState,
                "Tunggu sampai rekaman dan pemrosesan selesai sebelum memasang pembaruan.",
            ));
        }
    }
    let Some(update) = build(app)?.check().await.map_err(|_| AppError::new(ErrorCode::Network))? else {
        return Ok(());
    };
    tracing::info!("mengunduh pembaruan {}", update.version);
    let failed = |e: tauri_plugin_updater::Error| {
        tracing::error!("pasang pembaruan gagal: {e}");
        AppError::with_message(ErrorCode::Internal, "Pembaruan gagal dipasang. Detail tersimpan di log.")
    };
    let bytes = update.download(|_, _| {}, || {}).await.map_err(failed)?;
    // Rekaman bisa dimulai selama unduhan: installer menutup aplikasi, jadi cek ulang tepat sebelum memasang.
    if let Some(state) = app.try_state::<AppState>() {
        if state.recording.is_recording() || state.worker.current_meeting().is_some() {
            return Err(AppError::with_message(
                ErrorCode::InvalidState,
                "Pembaruan sudah diunduh, tetapi rekaman atau pemrosesan sedang berjalan. Coba pasang lagi setelah selesai.",
            ));
        }
    }
    update.install(bytes).map_err(failed)?;
    app.restart();
}

/// Cek di latar belakang: 1 menit setelah start, lalu tiap 24 jam. Hanya memberi tahu, tidak memasang.
pub fn spawn_background(app: AppHandle, events: std::sync::Arc<dyn EventSink>) {
    if !configured() {
        tracing::info!("updater nonaktif (pubkey/endpoint tidak disematkan saat build)");
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_AFTER).await;
        loop {
            if let Ok(Some(info)) = check(&app).await {
                let mut last = NOTIFIED.lock().unwrap_or_else(|e| e.into_inner());
                if last.as_deref() != Some(info.version.as_str()) {
                    *last = Some(info.version.clone());
                    events.notify(
                        "Pembaruan tersedia",
                        &format!("Versi {} siap dipasang. Buka Pengaturan → Tentang untuk memasang.", info.version),
                    );
                }
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}
