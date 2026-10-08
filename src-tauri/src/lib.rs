pub mod audio;
pub mod bridge;
pub mod commands;
pub mod config;
pub mod db;
pub mod desktop;
pub mod error;
pub mod events;
pub mod groq;
pub mod llm;
pub mod meeting_watch;
pub mod pipeline;
pub mod preprocess;
pub mod queue;
pub mod recording;
pub mod secrets;
pub mod stt;
pub mod windows_integration;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tokio::sync::Notify;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder as RollingBuilder, Rotation};

use crate::bridge::{TauriBridge, TRAY_ICON_IDLE, TRAY_ID};
use crate::config::providers::ProvidersConfig;
use crate::config::settings;
use crate::db::{now_ms, repo_usage, Db};
use crate::queue::worker::Worker;
use crate::recording::{RecordingService, StopReason};

/// usage_log lebih tua dari ini dihapus saat start (PRD §11).
const USAGE_LOG_RETENTION_MS: i64 = 2 * 24 * 60 * 60 * 1000;

/// Event ke jendela main: menu tray "Mulai rekam" → buka popup consent (tidak ada Start tanpa consent).
pub const EV_TRAY_START_RECORDING: &str = "tray://start-recording";

/// State global aplikasi.
pub struct AppState {
    /// Root data: `%APPDATA%\com.meetingpakeai.desktop\`.
    pub data_dir: PathBuf,
    pub db: Arc<Db>,
    pub providers: ProvidersConfig,
    /// HTTP client bersama (koneksi di-reuse).
    pub http: reqwest::Client,
    /// Cermin setting `minimize_to_tray` agar handler close tidak perlu query DB.
    pub minimize_to_tray: AtomicBool,
    /// Jendela main baru dibuat dari menu tray "Mulai rekam": popup consent dibuka setelah halaman siap.
    pub pending_consent: AtomicBool,
    pub bridge: Arc<TauriBridge>,
    pub recording: Arc<RecordingService>,
    /// Membangunkan worker antrean (meeting baru, retry, API key baru).
    pub queue_wake: Arc<Notify>,
    pub worker: Arc<Worker>,
    _log_guard: WorkerGuard,
}

fn init_logging(log_dir: &Path) -> Result<WorkerGuard, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(log_dir)?;
    // Nama file: app.log.YYYY-MM-DD, rotasi harian, simpan 7 file (PRD §6.1, §6.3).
    let appender = RollingBuilder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("app.log")
        .max_log_files(7)
        .build(log_dir)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).with_writer(writer).with_ansi(false).init();
    Ok(guard)
}

fn init_state(app: &AppHandle) -> Result<AppState, Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let log_guard = init_logging(&data_dir.join("logs"))?;
    tracing::info!("app start, versi {}", app.package_info().version);

    let db = Arc::new(Db::open(&data_dir.join("db").join("app.sqlite"))?);
    let providers = ProvidersConfig::load(&data_dir.join("providers.json"));
    let minimize_to_tray = {
        let conn = db.conn();
        let removed = repo_usage::delete_older_than(&conn, now_ms() - USAGE_LOG_RETENTION_MS)?;
        if removed > 0 {
            tracing::info!("usage_log lama dihapus: {removed} baris");
        }
        settings::load(&conn)?.minimize_to_tray
    };

    // Recovery sebelum worker jalan (PRD §13).
    if let Err(e) = queue::recovery::run(&data_dir, &db) {
        tracing::error!("recovery gagal: {}", e.message);
    }

    let bridge = Arc::new(TauriBridge::new(app.clone(), db.clone()));
    let queue_wake = Arc::new(Notify::new());
    let wake = queue_wake.clone();
    let recording = Arc::new(RecordingService::new(
        data_dir.clone(),
        db.clone(),
        bridge.clone(),
        providers.recording.clone(),
        Box::new(move || wake.notify_one()),
    ));

    let http = groq::build_client();
    let worker = Arc::new(Worker::new(
        data_dir.clone(),
        db.clone(),
        providers.clone(),
        http.clone(),
        bridge.clone(),
        queue_wake.clone(),
    ));

    Ok(AppState {
        data_dir,
        db,
        providers,
        http,
        minimize_to_tray: AtomicBool::new(minimize_to_tray),
        pending_consent: AtomicBool::new(false),
        bridge,
        recording,
        queue_wake,
        worker,
        _log_guard: log_guard,
    })
}

pub fn show_main_window(app: &AppHandle) {
    bridge::show_main_window(app, false);
}

pub(crate) fn stop_recording_in_background(app: &AppHandle, then_exit: bool) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Some(state) = app.try_state::<AppState>() {
            if let Err(e) = state.recording.stop(StopReason::Manual) {
                tracing::warn!("stop dari tray gagal: {}", e.message);
            }
        }
        if then_exit {
            app.exit(0);
        }
    });
}

/// Keluar penuh; saat merekam minta konfirmasi dulu (PRD §6.4).
fn request_quit(app: &AppHandle) {
    let recording = app.try_state::<AppState>().is_some_and(|s| s.recording.is_recording());
    if !recording {
        app.exit(0);
        return;
    }
    let handle = app.clone();
    app.dialog()
        .message("Rekaman sedang berjalan. Stop dan keluar?")
        .title("Meeting Pake AI")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("Stop dan keluar".into(), "Batal".into()))
        .show(move |ok| {
            if ok {
                stop_recording_in_background(&handle, true);
            }
        });
}

fn build_tray(app: &AppHandle, bridge: &TauriBridge) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Buka Meeting Pake AI", true, None::<&str>)?;
    let record = MenuItem::with_id(app, "record", "Mulai rekam", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Keluar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &record, &quit])?;
    *bridge.record_item.lock().unwrap_or_else(|e| e.into_inner()) = Some(record.clone());

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(TRAY_ICON_IDLE)?)
        .tooltip("Meeting Pake AI")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "record" => {
                let recording = app.try_state::<AppState>().is_some_and(|s| s.recording.is_recording());
                if recording {
                    stop_recording_in_background(app, false);
                } else {
                    // Start selalu lewat popup consent di jendela main.
                    bridge::show_main_window(app, true);
                }
            }
            "quit" => request_quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Harus plugin pertama (dokumentasi single-instance).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::Builder::new().args([desktop::ARG_MINIMIZED]).build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(desktop::on_shortcut).build())
        .setup(|app| {
            let state = init_state(app.handle())?;
            build_tray(app.handle(), &state.bridge)?;
            tauri::async_runtime::spawn(state.worker.clone().run());
            meeting_watch::spawn(
                state.db.clone(),
                state.recording.clone(),
                state.bridge.clone(),
                state.providers.meeting_detection.clone(),
            );
            let (s, onboarded) = {
                let conn = state.db.conn();
                (settings::load(&conn)?, settings::onboarding_completed(&conn)?)
            };
            app.manage(state);
            if let Err(e) = desktop::set_shortcut(app.handle(), "", &s.global_shortcut) {
                tracing::warn!("shortcut global tidak aktif: {}", e.message);
            }
            // Jendela main dibuat manual ("create": false): dilewati saat start dari autostart.
            let minimized = std::env::args().any(|a| a == desktop::ARG_MINIMIZED);
            if !minimized || !onboarded {
                show_main_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let to_tray = window
                    .try_state::<AppState>()
                    .is_none_or(|s| s.minimize_to_tray.load(Ordering::Relaxed));
                // Ke tray: jendela dihancurkan (WebView2 dilepas, RAM kecil); app tetap hidup lewat tray.
                if !to_tray {
                    api.prevent_close();
                    request_quit(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::onboarding::get_onboarding_status,
            commands::onboarding::complete_onboarding,
            commands::onboarding::check_mic_permission,
            commands::onboarding::open_mic_settings,
            commands::onboarding::run_audio_test,
            commands::onboarding::open_log_folder,
            commands::api_key::save_api_key,
            commands::api_key::test_api_key,
            commands::api_key::delete_api_key,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::recording::start_recording,
            commands::recording::pause_recording,
            commands::recording::resume_recording,
            commands::recording::set_mic_muted,
            commands::recording::stop_recording,
            commands::recording::get_recording_state,
            commands::recording::respond_auto_stop,
            commands::recording::take_pending_consent,
            commands::recording::take_pending_meeting,
            commands::meetings::list_meetings,
            commands::meetings::get_meeting,
            commands::meetings::get_transcript,
            commands::meetings::rename_meeting,
            commands::meetings::set_action_item_done,
            commands::meetings::update_summary,
            commands::meetings::set_speaker_name,
            commands::meetings::delete_meeting,
            commands::meetings::retry_job,
            commands::meetings::regenerate_summary,
            commands::meetings::retranscribe,
            commands::meetings::resolve_interrupted,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // Semua jendela tertutup (code = None) → tetap hidup di tray; keluar hanya lewat app.exit().
            if let RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}
