pub mod ai;
pub mod ask;
pub mod audio;
pub mod bridge;
pub mod commands;
pub mod config;
pub mod db;
pub mod desktop;
pub mod edit_audio;
pub mod error;
pub mod events;
pub mod import;
pub mod ai_http;
pub mod llm;
pub mod meeting_watch;
pub mod pipeline;
pub mod playback;
pub mod preprocess;
pub mod queue;
pub mod quick_llm;
pub mod recording;
pub mod secrets;
pub mod stt;
pub mod updater;
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

use crate::bridge::{TauriBridge, TRAY_ID};
use crate::config::providers::ProvidersConfig;
use crate::config::settings;
use crate::db::{now_ms, repo_usage, Db};
use crate::queue::worker::Worker;
use crate::recording::{RecordingService, StopReason};

/// usage_log lebih tua dari ini dihapus saat start (PRD §11).
const USAGE_LOG_RETENTION_MS: i64 = 2 * 24 * 60 * 60 * 1000;

/// ms → "HH:MM:SS" (untuk teks ke LLM).
pub fn format_hhmmss(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

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
    pub bridge: Arc<TauriBridge>,
    pub recording: Arc<RecordingService>,
    /// Membangunkan worker antrean (meeting baru, retry, API key baru).
    pub queue_wake: Arc<Notify>,
    pub worker: Arc<Worker>,
    _log_guard: WorkerGuard,
}

/// Panic di build release langsung abort (`panic = "abort"`) dan log non-blocking bisa hilang:
/// tulis pesan, lokasi, dan backtrace secara sinkron ke `logs/crash-*.txt`.
fn install_panic_hook(log_dir: PathBuf, version: String) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let when = chrono::Local::now();
        let path = log_dir.join(format!("crash-{}.txt", when.format("%Y%m%d-%H%M%S")));
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        let body = format!(
            "Meeting Pake AI {version}\nWaktu: {}\nThread: {thread}\n{info}\n\nBacktrace:\n{}\n",
            when.format("%Y-%m-%d %H:%M:%S %z"),
            std::backtrace::Backtrace::force_capture()
        );
        let _ = std::fs::write(&path, body);
        default_hook(info);
    }));
}

fn init_logging(log_dir: &Path) -> Result<WorkerGuard, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(log_dir)?;
    // Rotasi per jam, simpan 72 file (±3 hari): ukuran log terbatas walau satu hari bermasalah.
    let appender = RollingBuilder::new()
        .rotation(Rotation::HOURLY)
        .filename_prefix("app.log")
        .max_log_files(72)
        .build(log_dir)?;
    // Simpan 20 laporan crash terbaru.
    let mut crashes: Vec<_> = std::fs::read_dir(log_dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("crash-")))
        .collect();
    crashes.sort();
    while crashes.len() > 20 {
        let _ = std::fs::remove_file(crashes.remove(0));
    }
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
    install_panic_hook(data_dir.join("logs"), app.package_info().version.to_string());
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

    match db::repo_search::backfill_if_empty(&mut db.conn()) {
        Ok(n) if n > 0 => tracing::info!("index pencarian dibangun untuk {n} meeting"),
        Ok(_) => {}
        Err(e) => tracing::warn!("index pencarian gagal dibangun: {}", e.message),
    }

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

    let http = ai_http::build_client();
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
        bridge,
        recording,
        queue_wake,
        worker,
        _log_guard: log_guard,
    })
}

pub fn show_main_window(app: &AppHandle) {
    bridge::show_main_window(app);
}

/// Mulai rekam dari tray / shortcut tanpa membuka jendela; gagal → notifikasi Windows.
pub(crate) fn start_recording_in_background(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(state) = app.try_state::<AppState>() else { return };
        if let Err(e) = state.recording.start(None) {
            tracing::warn!("mulai rekam dari tray/shortcut gagal: {}", e.message);
            events::EventSink::notify(state.bridge.as_ref(), "Rekaman gagal dimulai", &e.message);
        }
    });
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

/// Buka jendela main di meeting `done` terbaru (`route = None`) atau di halaman tertentu (langkah 57).
fn open_in_main(app: &AppHandle, route: Option<&str>) {
    let Some(state) = app.try_state::<AppState>() else { return };
    match route {
        Some(r) => state.bridge.set_pending_nav(r),
        None => {
            let latest: Option<String> = state
                .db
                .conn()
                .query_row("SELECT id FROM meetings WHERE status = 'done' ORDER BY started_at DESC LIMIT 1", [], |r| r.get(0))
                .ok();
            if let Some(id) = latest {
                state.bridge.set_pending_meeting(&id);
            }
        }
    }
    show_main_window(app);
    let _ = tauri::Emitter::emit_to(app, "main", bridge::EV_APP_PENDING, ());
}

fn build_tray(app: &AppHandle, bridge: &TauriBridge) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Buka Meeting Pake AI", true, None::<&str>)?;
    let record = MenuItem::with_id(app, "record", "Mulai rekam", true, None::<&str>)?;
    // Langkah 57 (feedback3 A4): akses cepat ke hasil tanpa membuka aplikasi dulu.
    let latest = MenuItem::with_id(app, "latest", "Notulen terakhir", true, None::<&str>)?;
    let tasks = MenuItem::with_id(app, "tasks", "Tugas terbuka", true, None::<&str>)?;
    let sep1 = tauri::menu::PredefinedMenuItem::separator(app)?;
    let sep2 = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Keluar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &record, &sep1, &latest, &tasks, &sep2, &quit])?;
    *bridge.record_item.lock().unwrap_or_else(|e| e.into_inner()) = Some(record.clone());

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(bridge::tray_icon_idle())?)
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
                    start_recording_in_background(app);
                }
            }
            "latest" => open_in_main(app, None),
            "tasks" => open_in_main(app, Some("/tasks")),
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let state = match init_state(app.handle()) {
                Ok(s) => s,
                Err(e) => {
                    // Mis. database dari versi yang lebih baru: beri tahu pengguna sebelum keluar.
                    app.dialog()
                        .message(e.to_string())
                        .title("Meeting Pake AI tidak bisa dibuka")
                        .kind(MessageDialogKind::Error)
                        .blocking_show();
                    return Err(e);
                }
            };
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
            let bridge = state.bridge.clone();
            app.manage(state);
            updater::spawn_background(app.handle().clone(), bridge);
            if let Err(e) = desktop::set_shortcut(app.handle(), "", &s.global_shortcut) {
                tracing::warn!("shortcut global tidak aktif: {}", e.message);
            }
            if let Err(e) = desktop::set_shortcut(app.handle(), "", &s.bookmark_shortcut) {
                tracing::warn!("shortcut tandai momen tidak aktif: {}", e.message);
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
            if matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
                bridge::remember_main_geometry(window.app_handle(), false);
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                bridge::remember_main_geometry(window.app_handle(), true);
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
            commands::api_key::get_ai_config,
            commands::api_key::default_ai_endpoint,
            commands::api_key::save_ai_endpoint,
            commands::api_key::delete_ai_key,
            commands::api_key::detect_api_key_in_clipboard,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::check_update,
            commands::settings::get_storage_usage,
            commands::settings::clear_old_audio,
            commands::settings::save_problem_report,
            commands::settings::install_update,
            commands::recording::start_recording,
            commands::recording::pause_recording,
            commands::recording::resume_recording,
            commands::recording::set_mic_muted,
            commands::recording::add_bookmark,
            commands::recording::dismiss_meeting_offer,
            commands::recording::stop_recording,
            commands::recording::get_recording_state,
            commands::recording::respond_auto_stop,
            commands::recording::take_pending_offer,
            commands::recording::take_pending_meeting,
            commands::meetings::list_meetings,
            commands::meetings::search_meetings,
            commands::meetings::list_action_items,
            commands::meetings::get_meeting,
            commands::meetings::get_transcript,
            commands::meetings::rename_meeting,
            commands::meetings::set_action_item_done,
            commands::meetings::update_summary,
            commands::meetings::prepare_playback,
            commands::meetings::save_export,
            commands::meetings::delete_meeting,
            commands::meetings::retry_job,
            commands::meetings::regenerate_summary,
            commands::meetings::generate_follow_up,
            commands::meetings::delete_bookmark,
            commands::meetings::import_recording,
            commands::meetings::summarize_so_far,
            commands::meetings::get_notes,
            commands::meetings::save_notes,
            commands::meetings::toggle_bookmark_at,
            commands::meetings::ask_meeting,
            commands::meetings::list_tags,
            commands::meetings::set_meeting_tags,
            commands::meetings::restore_transcript,
            commands::meetings::previous_mergeable,
            commands::meetings::merge_with_previous,
            commands::meetings::delete_segment,
            commands::meetings::weekly_digest,
            commands::meetings::weekly_summary_text,
            commands::meetings::previous_open_tasks,
            commands::meetings::update_action_item,
            commands::meetings::add_action_item,
            commands::meetings::export_tasks_ics,
            commands::meetings::ask_all_meetings,
            commands::meetings::update_segment,
            commands::meetings::replace_in_meeting,
            commands::meetings::list_meeting_qa,
            commands::meetings::clear_meeting_qa,
            commands::recording::open_notes_window,
            commands::recording::open_meeting_in_main,
            commands::recording::take_pending_nav,
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
