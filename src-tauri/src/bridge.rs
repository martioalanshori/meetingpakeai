//! Implementasi `EventSink` untuk Tauri: emit event, notifikasi Windows, tray & jendela widget rekaman.
//! Demi RAM kecil (PRD §17), widget hanya ada selama merekam dan jendela main dihancurkan saat ditutup ke tray.

use std::sync::{Arc, Mutex};

use tauri::image::Image;
use tauri::menu::MenuItem;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_notification::NotificationExt;

use crate::config::settings::{self, WindowPosition};
use crate::db::Db;
use crate::events::EventSink;

pub const TRAY_ID: &str = "main-tray";
pub const TRAY_ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.png");
pub const TRAY_ICON_RECORDING: &[u8] = include_bytes!("../icons/tray-recording.png");
const RECORDER_LABEL: &str = "recorder";

pub struct TauriBridge {
    app: AppHandle,
    db: Arc<Db>,
    /// Item menu tray "Mulai rekam" / "Stop rekam" (diisi setelah tray dibuat).
    pub record_item: Mutex<Option<MenuItem<Wry>>>,
}

/// Tampilkan jendela main; dibuat ulang dari konfigurasi jika sudah dihancurkan.
/// `open_consent` → popup consent langsung dibuka (menu tray "Mulai rekam").
pub fn show_main_window(app: &AppHandle, open_consent: bool) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        if open_consent {
            let _ = app.emit_to("main", crate::EV_TRAY_START_RECORDING, ());
        }
        return;
    }
    let Some(cfg) = app.config().app.windows.iter().find(|w| w.label == "main").cloned() else { return };
    if open_consent {
        if let Some(state) = app.try_state::<crate::AppState>() {
            state.pending_consent.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    match WebviewWindowBuilder::from_config(app, &cfg).and_then(|b| b.build()) {
        Ok(w) => {
            let _ = w.set_focus();
        }
        Err(e) => tracing::error!("jendela main gagal dibuat: {e}"),
    }
}

impl TauriBridge {
    pub fn new(app: AppHandle, db: Arc<Db>) -> Self {
        Self { app, db, record_item: Mutex::new(None) }
    }

    fn open_recorder(&self) {
        if self.app.get_webview_window(RECORDER_LABEL).is_some() {
            return;
        }
        let mut b = WebviewWindowBuilder::new(&self.app, RECORDER_LABEL, WebviewUrl::App("recorder".into()))
            .title("Meeting Pake AI - Rekaman")
            .inner_size(300.0, 64.0)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false);
        let saved = settings::recorder_position(&self.db.conn()).ok().flatten();
        if saved.is_none() {
            b = b.center();
        }
        match b.build() {
            Ok(w) => {
                if let Some(pos) = saved {
                    let _ = w.set_position(PhysicalPosition::new(pos.x, pos.y));
                }
            }
            Err(e) => tracing::error!("widget rekaman gagal dibuat: {e}"),
        }
    }

    fn close_recorder(&self) {
        let Some(w) = self.app.get_webview_window(RECORDER_LABEL) else { return };
        if let Ok(p) = w.outer_position() {
            let _ = settings::set_recorder_position(&self.db.conn(), WindowPosition { x: p.x, y: p.y });
        }
        let _ = w.destroy();
    }
}

impl EventSink for TauriBridge {
    fn emit_json(&self, event: &str, payload: serde_json::Value) {
        if let Err(e) = self.app.emit(event, payload) {
            tracing::warn!("emit {event} gagal: {e}");
        }
    }

    fn notify(&self, title: &str, body: &str) {
        if let Err(e) = self.app.notification().builder().title(title).body(body).show() {
            tracing::warn!("notifikasi gagal: {e}");
        }
    }

    fn recording_changed(&self, recording: bool) {
        let app = self.app.clone();
        let item = self.record_item.lock().unwrap_or_else(|e| e.into_inner()).clone();
        // Operasi tray dijalankan di main thread.
        let _ = self.app.run_on_main_thread(move || {
            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                let bytes = if recording { TRAY_ICON_RECORDING } else { TRAY_ICON_IDLE };
                if let Ok(img) = Image::from_bytes(bytes) {
                    let _ = tray.set_icon(Some(img));
                }
                let _ = tray.set_tooltip(Some(if recording { "Meeting Pake AI — merekam" } else { "Meeting Pake AI" }));
            }
            if let Some(item) = item {
                let _ = item.set_text(if recording { "Stop rekam" } else { "Mulai rekam" });
            }
        });
        if recording {
            self.open_recorder();
        } else {
            self.close_recorder();
        }
    }
}
