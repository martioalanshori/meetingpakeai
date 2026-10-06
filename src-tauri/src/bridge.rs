//! Implementasi `EventSink` untuk Tauri: emit event, notifikasi Windows, tray & jendela widget rekaman.

use std::sync::{Arc, Mutex};

use tauri::image::Image;
use tauri::menu::MenuItem;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Wry};
use tauri_plugin_notification::NotificationExt;

use crate::config::settings::{self, WindowPosition};
use crate::db::Db;
use crate::events::EventSink;

pub const TRAY_ID: &str = "main-tray";
pub const TRAY_ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.png");
pub const TRAY_ICON_RECORDING: &[u8] = include_bytes!("../icons/tray-recording.png");

pub struct TauriBridge {
    app: AppHandle,
    db: Arc<Db>,
    /// Item menu tray "Mulai rekam" / "Stop rekam" (diisi setelah tray dibuat).
    pub record_item: Mutex<Option<MenuItem<Wry>>>,
}

impl TauriBridge {
    pub fn new(app: AppHandle, db: Arc<Db>) -> Self {
        Self { app, db, record_item: Mutex::new(None) }
    }

    fn show_recorder(&self) {
        let Some(w) = self.app.get_webview_window("recorder") else { return };
        if let Ok(Some(pos)) = settings::recorder_position(&self.db.conn()) {
            let _ = w.set_position(PhysicalPosition::new(pos.x, pos.y));
        }
        let _ = w.show();
    }

    fn hide_recorder(&self) {
        let Some(w) = self.app.get_webview_window("recorder") else { return };
        if let Ok(p) = w.outer_position() {
            let _ = settings::set_recorder_position(&self.db.conn(), WindowPosition { x: p.x, y: p.y });
        }
        let _ = w.hide();
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
        // Operasi tray/jendela dijalankan di main thread.
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
            self.show_recorder();
        } else {
            self.hide_recorder();
        }
    }
}
