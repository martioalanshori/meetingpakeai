//! Implementasi `EventSink` untuk Tauri: emit event, notifikasi Windows, tray & jendela widget rekaman.
//! Demi RAM kecil (PRD §17), widget hanya ada selama merekam dan jendela main dihancurkan saat ditutup ke tray.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::menu::MenuItem;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_notification::NotificationExt;

use crate::config::settings::{self, MainGeometry, WindowPosition};
use crate::db::Db;
use crate::events::EventSink;

pub const TRAY_ID: &str = "main-tray";
/// Ikon tray dari logo: [taskbar terang, taskbar gelap]. Kotak sudut hitam tidak terlihat di taskbar gelap,
/// jadi varian gelap memakai kotak sudut putih.
const TRAY_IDLE: [&[u8]; 2] = [include_bytes!("../icons/tray-idle.png"), include_bytes!("../icons/tray-idle-dark.png")];
const TRAY_RECORDING: [&[u8]; 2] =
    [include_bytes!("../icons/tray-recording.png"), include_bytes!("../icons/tray-recording-dark.png")];
const TRAY_PROCESSING: [&[u8]; 2] =
    [include_bytes!("../icons/tray-processing.png"), include_bytes!("../icons/tray-processing-dark.png")];

/// Varian ikon sesuai tema taskbar Windows saat ini.
fn themed(icons: [&'static [u8]; 2]) -> &'static [u8] {
    icons[usize::from(!crate::windows_integration::taskbar_is_light())]
}

pub fn tray_icon_idle() -> &'static [u8] {
    themed(TRAY_IDLE)
}
const RECORDER_LABEL: &str = "recorder";
/// Jendela tawaran rekam (langkah 46): terlihat walau Do Not Disturb menyala.
const OFFER_LABEL: &str = "offer";
/// Meeting dari notifikasi "Notulen siap" dibuka jika jendela main dibuka dalam waktu ini.
const PENDING_MEETING_TTL: Duration = Duration::from_secs(60 * 60);
/// Tawaran "Meeting terdeteksi" berlaku selama ini (lewat dari itu banner tidak ditampilkan).
const PENDING_OFFER_TTL: Duration = Duration::from_secs(5 * 60);
/// Ada hal tertunda untuk jendela main yang sedang fokus (meeting selesai / tawaran rekam).
pub const EV_APP_PENDING: &str = "app://pending";
const RECORDER_WIDTH: f64 = 380.0;
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

/// Simpan ukuran jendela main paling sering tiap 500 ms (event Moved/Resized beruntun saat menyeret).
const GEOMETRY_SAVE_EVERY: Duration = Duration::from_millis(500);
static LAST_GEOMETRY_SAVE: Mutex<Option<Instant>> = Mutex::new(None);

/// Titik (+ ruang untuk diklik) berada di area kerja salah satu monitor.
fn point_on_screen(app: &AppHandle, x: i32, y: i32) -> bool {
    const GRAB: i64 = 40;
    let Ok(monitors) = app.available_monitors() else { return true };
    monitors.iter().any(|m| {
        let wa = m.work_area();
        let (x0, y0) = (i64::from(wa.position.x), i64::from(wa.position.y));
        let (x1, y1) = (x0 + i64::from(wa.size.width), y0 + i64::from(wa.size.height));
        let (px, py) = (i64::from(x), i64::from(y));
        px + GRAB >= x0 && px + GRAB <= x1 && py >= y0 && py + GRAB <= y1
    })
}

/// Widget tawaran "Zoom terdeteksi · Rekam · Abaikan" (atau hitung mundur rekam otomatis), langkah 46.
/// Fungsi bebas agar bisa dijalankan di main thread tanpa memegang `TauriBridge`.
fn open_offer(app: &AppHandle, db: &Db, kind: &str, auto: bool) {
    if let Some(w) = app.get_webview_window(OFFER_LABEL) {
        let _ = w.destroy();
    }
    let url = format!("recorder/offer?kind={kind}&auto={}", u8::from(auto));
    let b = WebviewWindowBuilder::new(app, OFFER_LABEL, WebviewUrl::App(url.into()))
        .title("Meeting Pake AI - Meeting terdeteksi")
        .inner_size(RECORDER_WIDTH, 92.0)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false);
    let pos = settings::recorder_position(&db.conn())
        .ok()
        .flatten()
        .filter(|p| point_on_screen(app, p.x, p.y))
        .or_else(|| default_widget_position(app));
    match b.build() {
        Ok(w) => {
            if let Some(pos) = pos {
                let _ = w.set_position(PhysicalPosition::new(pos.x, pos.y));
            }
        }
        Err(e) => tracing::error!("widget tawaran gagal dibuat: {e}"),
    }
}

/// Pojok kanan atas monitor utama (margin 16 px).
fn default_widget_position(app: &AppHandle) -> Option<WindowPosition> {
    let m = app.primary_monitor().ok().flatten()?;
    let wa = m.work_area();
    let scale = m.scale_factor();
    let width = ((RECORDER_WIDTH + RECORDER_MARGIN) * scale) as i32;
    let margin = (RECORDER_MARGIN * scale) as i32;
    Some(WindowPosition { x: wa.position.x + wa.size.width as i32 - width, y: wa.position.y + margin })
}

/// Catat ukuran/posisi/maximize jendela main. `force` = abaikan pembatas (saat jendela ditutup).
pub fn remember_main_geometry(app: &AppHandle, force: bool) {
    {
        let mut last = LAST_GEOMETRY_SAVE.lock().unwrap_or_else(|e| e.into_inner());
        if !force && last.is_some_and(|t| t.elapsed() < GEOMETRY_SAVE_EVERY) {
            return;
        }
        *last = Some(Instant::now());
    }
    let Some(w) = app.get_webview_window("main") else { return };
    if w.is_minimized().unwrap_or(false) {
        return;
    }
    let maximized = w.is_maximized().unwrap_or(false);
    let (Ok(pos), Ok(size)) = (w.outer_position(), w.inner_size()) else { return };
    let Some(state) = app.try_state::<crate::AppState>() else { return };
    let conn = state.db.conn();
    // Saat maximize, pertahankan ukuran normal terakhir agar "restore" kembali ke ukuran itu.
    let previous = settings::main_geometry(&conn).ok().flatten();
    let g = match (maximized, previous) {
        (true, Some(p)) => MainGeometry { maximized: true, ..p },
        _ => MainGeometry { x: pos.x, y: pos.y, width: size.width, height: size.height, maximized },
    };
    if let Err(e) = settings::set_main_geometry(&conn, g) {
        tracing::warn!("ukuran jendela gagal disimpan: {}", e.message);
    }
}

/// Tampilkan jendela main; dibuat ulang dari konfigurasi jika sudah dihancurkan (dengan ukuran terakhir).
pub fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let Some(cfg) = app.config().app.windows.iter().find(|w| w.label == "main").cloned() else { return };
    let saved = app.try_state::<crate::AppState>().and_then(|s| settings::main_geometry(&s.db.conn()).ok().flatten());
    match WebviewWindowBuilder::from_config(app, &cfg).and_then(|b| b.build()) {
        Ok(w) => {
            if let Some(g) = saved {
                if g.width >= 640 && g.height >= 480 {
                    let _ = w.set_size(tauri::PhysicalSize::new(g.width, g.height));
                }
                if point_on_screen(app, g.x, g.y) {
                    let _ = w.set_position(PhysicalPosition::new(g.x, g.y));
                }
                if g.maximized {
                    let _ = w.maximize();
                }
            }
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
        point_on_screen(&self.app, p.x, p.y)
    }

    fn default_recorder_position(&self) -> Option<WindowPosition> {
        default_widget_position(&self.app)
    }

    /// Ikon & tooltip tray: merekam > memproses > idle; label menu "Mulai/Stop rekam".
    fn update_tray(&self) {
        let recording = self.recording.load(Ordering::SeqCst);
        let processing = self.processing.load(Ordering::SeqCst);
        let app = self.app.clone();
        let item = self.record_item.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let watching = settings::load(&self.db.conn()).is_ok_and(|s| s.meeting_detection);
        // Operasi tray dijalankan di main thread.
        let _ = self.app.run_on_main_thread(move || {
            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                let (bytes, tip) = if recording {
                    (themed(TRAY_RECORDING), "Meeting Pake AI — merekam")
                } else if processing {
                    (themed(TRAY_PROCESSING), "Meeting Pake AI — memproses notulen")
                } else {
                    (
                        themed(TRAY_IDLE),
                        if watching { "Meeting Pake AI — siap, deteksi meeting aktif" } else { "Meeting Pake AI" },
                    )
                };
                if let Ok(img) = Image::from_bytes(bytes) {
                    let _ = tray.set_icon(Some(img));
                }
                let _ = tray.set_tooltip(Some(tip));
            }
            if let Some(item) = item {
                let _ = item.set_text(if recording { "Hentikan rekaman" } else { "Mulai rekam" });
            }
        });
    }

    fn close_offer(&self) {
        if let Some(w) = self.app.get_webview_window(OFFER_LABEL) {
            let _ = w.destroy();
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

    fn meeting_detected(&self, kind: &str, auto: bool) {
        *self.pending_offer.lock().unwrap_or_else(|e| e.into_inner()) = Some((kind.to_string(), Instant::now()));
        let name = match kind {
            "zoom" => "Zoom",
            "teams" => "Microsoft Teams",
            "meet" => "Google Meet",
            "browser" => "browser",
            other => other,
        };
        // Widget di atas semua jendela: tetap terlihat saat Do Not Disturb menahan notifikasi.
        let (app, db, kind_s) = (self.app.clone(), self.db.clone(), kind.to_string());
        let _ = self.app.run_on_main_thread(move || open_offer(&app, &db, &kind_s, auto));
        if self.main_focused() {
            let _ = self.app.emit_to("main", EV_APP_PENDING, ());
            return;
        }
        if auto {
            self.notify(&format!("Meeting terdeteksi ({name})"), "Rekaman dimulai otomatis dalam 10 detik. Batalkan dari widget.");
        } else {
            self.notify(&format!("Meeting terdeteksi ({name})"), "Mulai rekam dari widget, atau tekan shortcut rekam.");
        }
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
            self.close_offer();
            self.open_recorder();
        } else {
            self.close_recorder();
        }
    }
}
