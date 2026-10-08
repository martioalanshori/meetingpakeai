//! Deteksi meeting (F12, PRD §14.8) dan tawaran Stop saat meeting selesai (feedback C2.1).
//! Polling registry ConsentStore; tidak bergantung pada Tauri (UI lewat `EventSink`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::providers::MeetingDetectionConfig;
use crate::config::settings;
use crate::db::Db;
use crate::events::EventSink;
use crate::recording::RecordingService;
use crate::windows_integration::meeting_detect::active_meeting_apps;

pub fn spawn(db: Arc<Db>, recording: Arc<RecordingService>, events: Arc<dyn EventSink>, cfg: MeetingDetectionConfig) {
    let spawned = std::thread::Builder::new().name("meeting-watch".into()).spawn(move || {
        Watch {
            db,
            recording,
            events,
            cfg,
            offered: BTreeSet::new(),
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
    /// Sesi pemakaian mic yang sudah ditawari / sudah direkam (sekali per sesi).
    offered: BTreeSet<String>,
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
            let enabled = {
                let conn = self.db.conn();
                let onboarded = settings::onboarding_completed(&conn).unwrap_or(false);
                onboarded && settings::load(&conn).map(|s| s.meeting_detection).unwrap_or(false)
            };
            if !enabled {
                self.offered.clear();
                self.reset_recording_state();
                continue;
            }
            let active = active_meeting_apps(&self.cfg.apps);
            self.tick(&active);
        }
    }

    fn reset_recording_state(&mut self) {
        self.seen_while_recording.clear();
        self.ended_since = None;
        self.stop_offered = false;
    }

    fn tick(&mut self, active: &BTreeMap<String, String>) {
        // Sesi mic yang sudah berakhir boleh ditawari lagi di sesi berikutnya.
        self.offered.retain(|k| active.contains_key(k));

        if !self.recording.is_recording() {
            self.reset_recording_state();
            let fresh = active.iter().find(|(k, _)| !self.offered.contains(*k)).map(|(_, kind)| kind.clone());
            self.offered.extend(active.keys().cloned());
            if let Some(kind) = fresh {
                tracing::info!("meeting terdeteksi: {kind}");
                self.events.meeting_detected(&kind);
            }
            return;
        }

        // Merekam: jangan tawarkan Start; pantau apakah aplikasi meeting selesai memakai mic.
        self.offered.extend(active.keys().cloned());
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
