pub mod commands;
pub mod config;
pub mod db;
pub mod error;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder as RollingBuilder, Rotation};

use crate::config::providers::ProvidersConfig;
use crate::config::settings;
use crate::db::{now_ms, repo_usage, Db};

const TRAY_ID: &str = "main-tray";
const TRAY_ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.png");
const TRAY_ICON_RECORDING: &[u8] = include_bytes!("../icons/tray-recording.png");

/// usage_log lebih tua dari ini dihapus saat start (PRD §11).
const USAGE_LOG_RETENTION_MS: i64 = 2 * 24 * 60 * 60 * 1000;

/// State global aplikasi. Field lain ditambahkan di langkah berikutnya.
pub struct AppState {
    /// Root data: `%APPDATA%\com.meetingpakeai.desktop\`.
    pub data_dir: PathBuf,
    pub db: Db,
    pub providers: ProvidersConfig,
    /// Cermin setting `minimize_to_tray` agar handler close tidak perlu query DB.
    pub minimize_to_tray: AtomicBool,
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

    let db = Db::open(&data_dir.join("db").join("app.sqlite"))?;
    let providers = ProvidersConfig::load(&data_dir.join("providers.json"));
    let minimize_to_tray = {
        let conn = db.conn();
        let removed = repo_usage::delete_older_than(&conn, now_ms() - USAGE_LOG_RETENTION_MS)?;
        if removed > 0 {
            tracing::info!("usage_log lama dihapus: {removed} baris");
        }
        settings::load(&conn)?.minimize_to_tray
    };

    Ok(AppState {
        data_dir,
        db,
        providers,
        minimize_to_tray: AtomicBool::new(minimize_to_tray),
        _log_guard: log_guard,
    })
}

fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Ganti ikon tray normal / merah (dipakai saat rekaman mulai/berhenti).
pub fn set_tray_recording(app: &AppHandle, recording: bool) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let bytes = if recording { TRAY_ICON_RECORDING } else { TRAY_ICON_IDLE };
        tray.set_icon(Some(Image::from_bytes(bytes)?))?;
    }
    Ok(())
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Buka Meeting Pake AI", true, None::<&str>)?;
    // TODO langkah 6: label berganti "Stop rekam" saat merekam; mulai rekam selalu lewat popup consent.
    let record = MenuItem::with_id(app, "record", "Mulai rekam", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Keluar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &record, &quit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(TRAY_ICON_IDLE)?)
        .tooltip("Meeting Pake AI")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" | "record" => show_main_window(app),
            // TODO langkah 6: konfirmasi "Rekaman sedang berjalan. Stop dan keluar?" saat merekam.
            "quit" => app.exit(0),
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
        .setup(|app| {
            let state = init_state(app.handle())?;
            app.manage(state);
            build_tray(app.handle())?;
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
                if to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::update_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
