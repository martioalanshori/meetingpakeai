//! Implementasi `EventSink` untuk Tauri: emit event, notifikasi Windows, tray & jendela widget rekaman.
//! Demi RAM kecil (PRD §17), widget hanya ada selama merekam dan jendela main dihancurkan saat ditutup ke tray.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
pub const TRAY_ICON_PROCESSING: &[u8] = include_bytes!("../icons/tray-processing.png");
const RECORDER_LABEL: &str = "recorder";
/// Meeting dari notifikasi "Notulen siap" dibuka jika jendela main dibuka dalam waktu ini.
const PENDING_MEETING_TTL: Duration = Duration::from_secs(60 * 60);
/// Tawaran "Meeting terdeteksi" berlaku selama ini (lewat dari itu popup consent tidak dibuka otomatis).
const PENDING_OFFER_TTL: Duration = Duration::from_secs(5 * 60);
/// Ada hal tertunda untuk jendela main yang sedang fokus (meeting selesai / tawaran rekam).
pub const EV_APP_PENDING: &str = "app://pending";
const RECORDER_WIDTH: f64 = 300.0;
/// Jarak widget dari tepi layar (logical px).
const RECORDER_MARGIN: f64 = 16.0;

pub struct TauriBridge {
    app: AppHandle,
    db: Arc<Db>,
    /// Item menu tray "Mulai rekam" / "Stop rekam" (diisi setelah tray dibuat).
    pub record_item: Mutex<Option<MenuItem<Wry>>>,
    /// Notifikasi desktop tidak punya handler klik: meeting terakhir yang selesai dibuka
    /// saat jendela main berikutnya mendapat fokus (klik notifikasi / tray).
    pending_meeting: Mutex<Option<(String, Instant)>>,
    /// Tawaran rekam dari deteksi meeting: jenis aplikasi (`source_app`).
    pending_offer: Mutex<Option<(String, Instant)>>,
    recording: AtomicBool,
    processing: AtomicBool,
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
        Self {
            app,
            db,
            record_item: Mutex::new(None),
            pending_meeting: Mutex::new(None),
            pending_offer: Mutex::new(None),
            recording: AtomicBool::new(false),
            processing: AtomicBool::new(false),
        }
    }

    /// Tawaran rekam dari notifikasi "Meeting terdeteksi" (sekali ambil): jenis aplikasi.
    pub fn take_pending_offer(&self) -> Option<String> {
        let taken = self.pending_offer.lock().unwrap_or_else(|e| e.into_inner()).take();
        taken.filter(|(_, at)| at.elapsed() < PENDING_OFFER_TTL).map(|(kind, _)| kind)
    }

    fn main_focused(&self) -> bool {
        self.app.get_webview_window("main").is_some_and(|w| w.is_focused().unwrap_or(false))
    }

    /// Meeting yang menunggu dibuka dari notifikasi "Notulen siap" (sekali ambil).
    pub fn take_pending_meeting(&self) -> Option<String> {
        let taken = self.pending_meeting.lock().unwrap_or_else(|e| e.into_inner()).take();
        taken.filter(|(_, at)| at.elapsed() < PENDING_MEETING_TTL).map(|(id, _)| id)
    }

    fn open_recorder(&self) {
        if self.app.get_webview_window(RECORDER_LABEL).is_some() {
            return;
        }
        let mut b = WebviewWindowBuilder::new(&self.app, RECORDER_LABEL, WebviewUrl::App("recorder".into()))
            .title("Meeting Pake AI - Rekaman")
            .inner_size(RECORDER_WIDTH, 64.0)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false);
        // Posisi tersimpan bisa berada di monitor yang sudah dilepas → pakai pojok kanan atas monitor utama.
        let saved = settings::recorder_position(&self.db.conn()).ok().flatten().filter(|p| self.on_screen(*p));
        let pos = saved.or_else(|| self.default_recorder_position());
        if pos.is_none() {
            b = b.center();
        }
        match b.build() {
            Ok(w) => {
                if let Some(pos) = pos {
                    let _ = w.set_position(PhysicalPosition::new(pos.x, pos.y));
                }
            }
            Err(e) => tracing::error!("widget rekaman gagal dibuat: {e}"),
        }
    }

    /// Pojok kiri-atas widget (+ sedikit ruang untuk diklik) ada di area kerja salah satu monitor.
    fn on_screen(&self, p: WindowPosition) -> bool {
        const GRAB: i64 = 40;
        let Ok(monitors) = self.app.available_monitors() else { return true };
        monitors.iter().any(|m| {
            let wa = m.work_area();
            let (x0, y0) = (i64::from(wa.position.x), i64::from(wa.position.y));
            let (x1, y1) = (x0 + i64::from(wa.size.width), y0 + i64::from(wa.size.height));
            let (px, py) = (i64::from(p.x), i64::from(p.y));
            px + GRAB >= x0 && px + GRAB <= x1 && py >= y0 && py + GRAB <= y1
        })
    }

    fn default_recorder_position(&self) -> Option<WindowPosition> {
        let m = self.app.primary_monitor().ok().flatten()?;
        let wa = m.work_area();
        let scale = m.scale_factor();
        let width = ((RECORDER_WIDTH + RECORDER_MARGIN) * scale) as i32;
        let margin = (RECORDER_MARGIN * scale) as i32;
        Some(WindowPosition { x: wa.position.x + wa.size.width as i32 - width, y: wa.position.y + margin })
    }

    /// Ikon & tooltip tray: merekam > memproses > idle; label menu "Mulai/Stop rekam".
    fn update_tray(&self) {
        let recording = self.recording.load(Ordering::SeqCst);
        let processing = self.processing.load(Ordering::SeqCst);
        let app = self.app.clone();
        let item = self.record_item.lock().unwrap_or_else(|e| e.into_inner()).clone();
        // Operasi tray dijalankan di main thread.
        let _ = self.app.run_on_main_thread(move || {
            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                let (bytes, tip) = if recording {
                    (TRAY_ICON_RECORDING, "Meeting Pake AI — merekam")
                } else if processing {
                    (TRAY_ICON_PROCESSING, "Meeting Pake AI — memproses notulen")
                } else {
                    (TRAY_ICON_IDLE, "Meeting Pake AI")
                };
                if let Ok(img) = Image::from_bytes(bytes) {
                    let _ = tray.set_icon(Some(img));
                }
                let _ = tray.set_tooltip(Some(tip));
            }
            if let Some(item) = item {
                let _ = item.set_text(if recording { "Stop rekam" } else { "Mulai rekam" });
            }
        });
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

    fn meeting_detected(&self, kind: &str) {
        *self.pending_offer.lock().unwrap_or_else(|e| e.into_inner()) = Some((kind.to_string(), Instant::now()));
        if self.main_focused() {
            let _ = self.app.emit_to("main", EV_APP_PENDING, ());
            return;
        }
        let name = match kind {
            "zoom" => "Zoom",
            "teams" => "Microsoft Teams",
            "browser" => "browser",
            other => other,
        };
        self.notify(&format!("Meeting terdeteksi ({name})"), "Mulai rekam? Klik untuk membuka Meeting Pake AI.");
    }

    fn meeting_done(&self, meeting_id: &str, title: &str) {
        // Pengguna sedang melihat aplikasi: status di layar sudah cukup.
        if self.main_focused() {
            return;
        }
        *self.pending_meeting.lock().unwrap_or_else(|e| e.into_inner()) = Some((meeting_id.to_string(), Instant::now()));
        self.notify("Notulen siap", &format!("{title}
Klik untuk membuka notulen."));
    }

    fn processing_changed(&self, processing: bool) {
        if self.processing.swap(processing, Ordering::SeqCst) != processing {
            self.update_tray();
        }
    }

    fn recording_changed(&self, recording: bool) {
        self.recording.store(recording, Ordering::SeqCst);
        self.update_tray();
        if recording {
            self.open_recorder();
        } else {
            self.close_recorder();
        }
    }
}
