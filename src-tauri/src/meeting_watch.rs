//! Deteksi meeting (F12, PRD §14.8) dan tawaran Stop saat meeting selesai (feedback C2.1).
//! Polling registry ConsentStore; tidak bergantung pada Tauri (UI lewat `EventSink`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::providers::MeetingDetectionConfig;
use crate::config::settings;
use crate::db::Db;
use crate::events::EventSink;
use crate::recording::RecordingService;
use crate::windows_integration::meeting_detect::active_meeting_apps;
use crate::windows_integration::meeting_window;

/// Tawaran diabaikan lagi setelah selama ini jika aplikasi meeting masih memakai mic (langkah 46, A0.3).
const REMIND_AFTER: Duration = Duration::from_secs(180);
/// Jenis yang boleh direkam otomatis (browser umum tidak: bisa voice note / tes mic).
const AUTO_KINDS: [&str; 3] = ["zoom", "teams", "meet"];

/// Pengguna menekan "Abaikan" pada tawaran terakhir → tidak diingatkan ulang untuk sesi mic itu.
static DISMISSED: AtomicBool = AtomicBool::new(false);

pub fn dismiss_offer() {
    DISMISSED.store(true, Ordering::SeqCst);
}

pub fn spawn(db: Arc<Db>, recording: Arc<RecordingService>, events: Arc<dyn EventSink>, cfg: MeetingDetectionConfig) {
    let spawned = std::thread::Builder::new().name("meeting-watch".into()).spawn(move || {
        Watch {
            db,
            recording,
            events,
            cfg,
            offered: BTreeMap::new(),
            seen_while_recording: BTreeSet::new(),
            ended_since: None,
            stop_offered: false,
        }
        .run();
    });
    if let Err(e) = spawned {
        tracing::error!("deteksi meeting gagal dimulai: {e}");
    }
}

struct Watch {
    db: Arc<Db>,
    recording: Arc<RecordingService>,
    events: Arc<dyn EventSink>,
    cfg: MeetingDetectionConfig,
    /// Sesi pemakaian mic yang sudah ditawari / sudah direkam: waktu tawaran terakhir + jumlah tawaran.
    offered: BTreeMap<String, (Instant, u8)>,
    /// Aplikasi meeting yang memakai mic selama rekaman berjalan.
    seen_while_recording: BTreeSet<String>,
    /// Sejak kapan semua aplikasi meeting itu berhenti memakai mic.
    ended_since: Option<Instant>,
    stop_offered: bool,
}

impl Watch {
    fn run(mut self) {
        let poll = Duration::from_secs(u64::from(self.cfg.poll_sec.max(2)));
        loop {
            std::thread::sleep(poll);
            let (enabled, auto) = {
                let conn = self.db.conn();
                let onboarded = settings::onboarding_completed(&conn).unwrap_or(false);
                let s = settings::load(&conn).ok();
                (onboarded && s.as_ref().is_some_and(|s| s.meeting_detection), s.is_some_and(|s| s.auto_record))
            };
            if !enabled {
                self.offered.clear();
                self.reset_recording_state();
                continue;
            }
            let mut active = active_meeting_apps(&self.cfg.apps);
            // Browser yang memakai mic dengan tab Google Meet terbuka → "meet" (boleh direkam otomatis).
            if active.values().any(|k| k == "browser") {
                let patterns = if self.cfg.meet_title_patterns.is_empty() {
                    meeting_window::default_meet_patterns()
                } else {
                    self.cfg.meet_title_patterns.clone()
                };
                if meeting_window::meet_open(&patterns) {
                    for k in active.values_mut().filter(|k| *k == "browser") {
                        *k = "meet".into();
                    }
                }
            }
            self.tick(&active, auto);
        }
    }

    fn reset_recording_state(&mut self) {
        self.seen_while_recording.clear();
        self.ended_since = None;
        self.stop_offered = false;
    }

    fn tick(&mut self, active: &BTreeMap<String, String>, auto: bool) {
        // Sesi mic yang sudah berakhir boleh ditawari lagi di sesi berikutnya.
        self.offered.retain(|k, _| active.contains_key(k));
        let now = Instant::now();
        if !self.recording.is_recording() {
            self.reset_recording_state();
            // Sesi baru → tawaran pertama; tawaran pertama diabaikan diam-diam → diingatkan sekali lagi.
            let mut pick: Option<(String, String, bool)> = None;
            for (k, kind) in active {
                match self.offered.get(k) {
                    None => {
                        pick = Some((k.clone(), kind.clone(), false));
                        break;
                    }
                    Some((at, 1)) if now.duration_since(*at) >= REMIND_AFTER && !DISMISSED.load(Ordering::SeqCst) => {
                        pick.get_or_insert((k.clone(), kind.clone(), true));
                    }
                    _ => {}
                }
            }
            for k in active.keys() {
                self.offered.entry(k.clone()).or_insert((now, 1));
            }
            if let Some((key, kind, reminder)) = pick {
                if reminder {
                    self.offered.insert(key, (now, 2));
                } else {
                    DISMISSED.store(false, Ordering::SeqCst);
                }
                let auto = auto && !reminder && AUTO_KINDS.contains(&kind.as_str());
                tracing::info!("meeting terdeteksi: {kind} (otomatis: {auto}, pengingat: {reminder})");
                self.events.meeting_detected(&kind, auto);
            }
            return;
        }
        // Merekam: jangan tawarkan Start; pantau apakah aplikasi meeting selesai memakai mic.
        for k in active.keys() {
            self.offered.entry(k.clone()).or_insert((now, 2));
        }
        self.seen_while_recording.extend(active.keys().cloned());
        if active.is_empty() && !self.seen_while_recording.is_empty() {
            let since = *self.ended_since.get_or_insert_with(Instant::now);
            let grace = Duration::from_secs(u64::from(self.cfg.stop_grace_sec));
            if !self.stop_offered && since.elapsed() >= grace {
                self.stop_offered = true;
                tracing::info!("aplikasi meeting berhenti memakai mic: tawarkan Stop");
                self.recording.offer_stop_meeting_ended(Duration::from_secs(u64::from(self.cfg.stop_countdown_sec)));
            }
        } else {
            self.ended_since = None;
            if !active.is_empty() {
                self.stop_offered = false;
            }
        }
    }
}
