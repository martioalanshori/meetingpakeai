//! Integrasi desktop: shortcut global Mulai/Stop rekam dan autostart Windows (langkah 21).

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

use crate::error::{AppError, AppResult, ErrorCode};
use crate::AppState;

/// Argumen yang dipakai entri autostart: app jalan tersembunyi di tray.
pub const ARG_MINIMIZED: &str = "--minimized";

/// Handler semua shortcut global (hanya satu yang didaftarkan).
/// Idle → langsung mulai rekam (tanpa membuka jendela); merekam → Stop.
pub fn on_shortcut(app: &AppHandle, _shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed {
        return;
    }
    let recording = app.try_state::<AppState>().is_some_and(|s| s.recording.is_recording());
    if recording {
        crate::stop_recording_in_background(app, false);
    } else {
        crate::start_recording_in_background(app);
    }
}

/// Ganti shortcut terdaftar. `new` kosong → shortcut dimatikan.
pub fn set_shortcut(app: &AppHandle, old: &str, new: &str) -> AppResult<()> {
    let gs = app.global_shortcut();
    if !old.is_empty() {
        let _ = gs.unregister(old);
    }
    if new.is_empty() {
        return Ok(());
    }
    if new.parse::<Shortcut>().is_err() {
        return Err(AppError::with_message(ErrorCode::InvalidState, format!("Shortcut \"{new}\" tidak dikenali.")));
    }
    gs.register(new).map_err(|e| {
        tracing::warn!("shortcut {new} gagal didaftarkan: {e}");
        // Kembalikan shortcut lama agar pengguna tidak kehilangan shortcut sama sekali.
        if !old.is_empty() {
            let _ = gs.register(old);
        }
        AppError::with_message(ErrorCode::InvalidState, format!("Shortcut \"{new}\" sudah dipakai aplikasi lain."))
    })
}

pub fn set_autostart(app: &AppHandle, enabled: bool) -> AppResult<()> {
    let al = app.autolaunch();
    let res = if enabled { al.enable() } else { al.disable() };
    res.map_err(|e| AppError::internal(format!("autostart: {e}")))
}
