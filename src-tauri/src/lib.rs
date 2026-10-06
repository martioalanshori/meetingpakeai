use std::sync::atomic::{AtomicBool, Ordering};

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};

const TRAY_ID: &str = "main-tray";
const TRAY_ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.png");
#[allow(dead_code)] // dipakai saat rekaman (langkah 6)
const TRAY_ICON_RECORDING: &[u8] = include_bytes!("../icons/tray-recording.png");

/// State global aplikasi. Field lain ditambahkan di langkah berikutnya.
pub struct AppState {
    /// Cermin setting `minimize_to_tray` (dibaca dari DB mulai langkah 2).
    pub minimize_to_tray: AtomicBool,
}

impl Default for AppState {
    fn default() -> Self {
        Self { minimize_to_tray: AtomicBool::new(true) }
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Ganti ikon tray normal / merah (dipakai saat rekaman mulai/berhenti).
#[allow(dead_code)]
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
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(AppState::default())
        .setup(|app| {
            build_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if state.minimize_to_tray.load(Ordering::Relaxed) {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
