//! Service rekaman: validasi Start/Stop (§7.7), meeting di DB, monitor (level 10 Hz, auto-stop §7.6,
//! reconnect device §7.5). Tidak bergantung pada Tauri; UI dijangkau lewat `EventSink`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::audio::recorder::Recorder;
use crate::audio::writer::PartEvent;
use crate::audio::Channel;
use crate::config::providers::RecordingConfig;
use crate::config::settings;
use crate::db::repo_meetings::{self, NewMeeting};
use crate::db::{now_ms, repo_parts, Db};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::events::{self, EventSink, MeetingUpdated};
use crate::{secrets, windows_integration};

const MIN_FREE_START_BYTES: u64 = 1024 * 1024 * 1024;
const MIN_FREE_RECORDING_BYTES: u64 = 500 * 1024 * 1024;
const MIN_DURATION_MS: i64 = 5_000;
const SILENCE_THRESHOLD_DBFS: f32 = -50.0;
const AUTO_STOP_GRACE: Duration = Duration::from_secs(120);
const DISK_CHECK_EVERY: Duration = Duration::from_secs(30);
const REOPEN_EVERY: Duration = Duration::from_secs(1);
const REOPEN_MAX_ATTEMPTS: u32 = 10;
const MONITOR_TICK: Duration = Duration::from_millis(100);

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des"];

/// `RecordingState` (PRD §12.2).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingState {
    pub status: &'static str,
    pub meeting_id: Option<String>,
    pub elapsed_ms: i64,
    pub mic_muted: bool,
    pub mic_alive: bool,
    pub system_alive: bool,
}

impl RecordingState {
    fn idle() -> Self {
        Self { status: "idle", meeting_id: None, elapsed_ms: 0, mic_muted: false, mic_alive: true, system_alive: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Manual,
    Silence,
    MaxDuration,
    DiskFull,
    DeviceLost,
}

#[derive(Default)]
struct ChannelHealth {
    attempts: u32,
    last_try: Option<Instant>,
    given_up: bool,
}

#[derive(Default)]
struct AutoStopState {
    silent_since: Option<Instant>,
    warning_deadline: Option<Instant>,
}

struct Active {
    meeting_id: String,
    dir: PathBuf,
    recorder: Recorder,
    monitor_stop: Arc<AtomicBool>,
    auto_stop: Mutex<AutoStopState>,
}

pub struct RecordingService {
    data_dir: PathBuf,
    db: Arc<Db>,
    events: Arc<dyn EventSink>,
    config: RecordingConfig,
    /// Dipanggil setelah meeting masuk antrean (membangunkan worker).
    on_queued: Box<dyn Fn() + Send + Sync>,
    active: Mutex<Option<Arc<Active>>>,
}

fn default_title(started_at_ms: i64) -> String {
    use chrono::{Datelike, Local, TimeZone, Timelike};
    match Local.timestamp_millis_opt(started_at_ms).single() {
        Some(t) => format!(
            "Meeting {} {} {} {:02}.{:02}",
            t.day(),
            MONTHS[t.month0() as usize],
            t.year(),
            t.hour(),
            t.minute()
        ),
        None => "Meeting".to_string(),
    }
}

impl RecordingService {
    pub fn new(
        data_dir: PathBuf,
        db: Arc<Db>,
        events: Arc<dyn EventSink>,
        config: RecordingConfig,
        on_queued: Box<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self { data_dir, db, events, config, on_queued, active: Mutex::new(None) }
    }

    fn lock_active(&self) -> MutexGuard<'_, Option<Arc<Active>>> {
        self.active.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn current(&self) -> Option<Arc<Active>> {
        self.lock_active().clone()
    }

    pub fn is_recording(&self) -> bool {
        self.lock_active().is_some()
    }

    pub fn state(&self) -> RecordingState {
        match self.current() {
            Some(a) => state_of(&a),
            None => RecordingState::idle(),
        }
    }

    fn emit_state(&self) {
        events::emit(self.events.as_ref(), events::EV_RECORDING_STATE, &self.state());
    }

    pub fn start(self: &Arc<Self>, source_app: Option<String>) -> AppResult<String> {
        let mut guard = self.lock_active();
        if guard.is_some() {
            return Err(ErrorCode::AlreadyRecording.into());
        }
        if secrets::get_api_key()?.is_none() {
            return Err(ErrorCode::NoApiKey.into());
        }
        if windows_integration::free_disk_bytes(&self.data_dir).is_some_and(|b| b < MIN_FREE_START_BYTES) {
            return Err(ErrorCode::DiskFull.into());
        }

        let meeting_id = uuid::Uuid::new_v4().to_string();
        let now = now_ms();
        let language = settings::load(&self.db.conn())?.stt_language;
        repo_meetings::insert(
            &self.db.conn(),
            &NewMeeting {
                id: &meeting_id,
                title: &default_title(now),
                started_at: now,
                language: language.as_str(),
                source_app: source_app.as_deref(),
                consent_at: now,
            },
        )?;

        let rel_dir = format!("recordings/{meeting_id}");
        let dir = self.data_dir.join(&rel_dir);
        let db = self.db.clone();
        let mid = meeting_id.clone();
        let on_part = Arc::new(move |ev: PartEvent| {
            let conn = db.conn();
            let res = match ev {
                PartEvent::Opened { channel, part_index, .. } => {
                    let rel = format!("{rel_dir}/{}", crate::audio::writer::PartWriter::part_file_name(channel, part_index));
                    repo_parts::insert_open(&conn, &mid, channel.as_str(), part_index as i64, &rel)
                }
                PartEvent::Finalized { channel, part_index, samples } => {
                    repo_parts::mark_finalized(&conn, &mid, channel.as_str(), part_index as i64, samples as i64)
                }
            };
            if let Err(e) = res {
                tracing::error!("catat part gagal: {}", e.message);
            }
        });

        let recorder = match Recorder::start(&dir, on_part) {
            Ok(r) => r,
            Err(e) => {
                let _ = repo_meetings::delete(&self.db.conn(), &meeting_id);
                let _ = std::fs::remove_dir_all(&dir);
                tracing::warn!("start rekaman gagal: {e}");
                return Err(AppError::from(e));
            }
        };
        let active = Arc::new(Active {
            meeting_id: meeting_id.clone(),
            dir,
            recorder,
            monitor_stop: Arc::new(AtomicBool::new(false)),
            auto_stop: Mutex::new(AutoStopState::default()),
        });
        *guard = Some(active.clone());
        drop(guard);
        tracing::info!("rekaman dimulai: meeting {meeting_id}");

        self.spawn_monitor(active);
        self.events.recording_changed(true);
        self.emit_state();
        events::emit(self.events.as_ref(), events::EV_MEETING_UPDATED, &MeetingUpdated { meeting_id: &meeting_id });
        Ok(meeting_id)
    }

    fn with_active<T>(&self, f: impl FnOnce(&Active) -> T) -> AppResult<T> {
        let a = self.current().ok_or_else(|| AppError::new(ErrorCode::NotRecording))?;
        Ok(f(&a))
    }

    pub fn pause(&self) -> AppResult<RecordingState> {
        self.with_active(|a| a.recorder.pause())?;
        self.emit_state();
        Ok(self.state())
    }

    pub fn resume(&self) -> AppResult<RecordingState> {
        self.with_active(|a| {
            a.recorder.resume();
            // Hening selama pause tidak dihitung untuk auto-stop.
            a.auto_stop.lock().unwrap_or_else(|e| e.into_inner()).silent_since = None;
        })?;
        self.emit_state();
        Ok(self.state())
    }

    pub fn set_mic_muted(&self, muted: bool) -> AppResult<RecordingState> {
        self.with_active(|a| a.recorder.set_mic_muted(muted))?;
        self.emit_state();
        Ok(self.state())
    }

    pub fn respond_auto_stop(&self, continue_recording: bool) -> AppResult<()> {
        if continue_recording {
            self.with_active(|a| {
                let mut st = a.auto_stop.lock().unwrap_or_else(|e| e.into_inner());
                st.warning_deadline = None;
                st.silent_since = Some(Instant::now());
            })?;
            Ok(())
        } else {
            self.stop(StopReason::Manual).map(|_| ())
        }
    }

    /// Stop. `Ok(None)` jika durasi < 5 detik (meeting & file dibuang).
    pub fn stop(&self, reason: StopReason) -> AppResult<Option<String>> {
        let active = self.lock_active().take().ok_or_else(|| AppError::new(ErrorCode::NotRecording))?;
        active.monitor_stop.store(true, Ordering::SeqCst);
        let ended_at = now_ms();
        // Monitor memegang Arc juga; ambil Recorder lewat try_unwrap setelah monitor lepas.
        let active = wait_unique(active);
        let Active { meeting_id, dir, recorder, .. } = active;
        let result = recorder.stop();
        self.events.recording_changed(false);
        self.emit_state();

        let result = match result {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("stop rekaman gagal: {e}");
                // Tetap antrekan; recovery/preprocess memakai part yang ada.
                crate::audio::recorder::StopResult { duration_ms: 0, mic_samples: 0, system_samples: 0 }
            }
        };
        tracing::info!("rekaman berhenti ({reason:?}): meeting {meeting_id}, {} ms", result.duration_ms);

        if result.duration_ms < MIN_DURATION_MS {
            repo_meetings::delete(&self.db.conn(), &meeting_id)?;
            let _ = std::fs::remove_dir_all(&dir);
            events::emit(self.events.as_ref(), events::EV_MEETING_UPDATED, &MeetingUpdated { meeting_id: &meeting_id });
            return Ok(None);
        }
        repo_meetings::finish_recording(&self.db.conn(), &meeting_id, ended_at, result.duration_ms)?;
        events::emit(self.events.as_ref(), events::EV_MEETING_UPDATED, &MeetingUpdated { meeting_id: &meeting_id });
        (self.on_queued)();

        let note = match reason {
            StopReason::Manual => None,
            StopReason::Silence => Some("Tidak ada suara terdeteksi. Rekaman dihentikan otomatis dan sedang diproses."),
            StopReason::MaxDuration => Some("Durasi maksimal rekaman tercapai. Rekaman dihentikan dan sedang diproses."),
            StopReason::DiskFull => Some("Ruang disk hampir habis. Rekaman dihentikan dan sedang diproses."),
            StopReason::DeviceLost => Some("Perangkat audio tidak tersedia. Rekaman dihentikan dan sedang diproses."),
        };
        if let Some(body) = note {
            self.events.notify("Rekaman dihentikan", body);
        }
        Ok(Some(meeting_id))
    }

    fn spawn_monitor(self: &Arc<Self>, active: Arc<Active>) {
        let svc = Arc::clone(self);
        let spawned = std::thread::Builder::new().name("recording-monitor".into()).spawn(move || {
            svc.monitor_loop(active);
        });
        if let Err(e) = spawned {
            tracing::error!("monitor rekaman gagal dibuat: {e}");
        }
    }

    fn monitor_loop(&self, active: Arc<Active>) {
        let silence_limit = Duration::from_secs(u64::from(self.config.auto_stop_silence_min) * 60);
        let max_duration = Duration::from_secs(u64::from(self.config.max_recording_hours) * 3600);
        let mut health = [ChannelHealth::default(), ChannelHealth::default()];
        let mut last_disk_check = Instant::now();
        let mut last_alive = [true, true];

        let stop_reason = loop {
            if active.monitor_stop.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(MONITOR_TICK);
            let rec = &active.recorder;
            let now = Instant::now();

            // Level 10 Hz (widget rekaman terbuka selama merekam).
            let (mic_db, sys_db) = rec.levels();
            self.events.emit_json(
                events::EV_RECORDING_LEVEL,
                serde_json::json!({ "micDbfs": mic_db, "systemDbfs": sys_db }),
            );

            // Reconnect device (§7.5).
            for (i, channel) in [Channel::Mic, Channel::System].into_iter().enumerate() {
                let h = &mut health[i];
                if rec.alive(channel) {
                    h.attempts = 0;
                    continue;
                }
                if h.given_up || h.last_try.is_some_and(|t| now.duration_since(t) < REOPEN_EVERY) {
                    continue;
                }
                h.last_try = Some(now);
                h.attempts += 1;
                match rec.reopen(channel) {
                    Ok(()) => h.attempts = 0,
                    Err(e) => {
                        tracing::warn!("buka ulang {} gagal ({}x): {e}", channel.as_str(), h.attempts);
                        if h.attempts >= REOPEN_MAX_ATTEMPTS {
                            h.given_up = true;
                            self.events.emit_json(
                                events::EV_RECORDING_WARNING,
                                serde_json::json!({ "code": "device_lost", "channel": channel.as_str() }),
                            );
                        }
                    }
                }
            }
            let alive_now = [rec.alive(Channel::Mic), rec.alive(Channel::System)];
            if alive_now != last_alive {
                last_alive = alive_now;
                self.emit_state();
            }
            if health[0].given_up && health[1].given_up {
                break StopReason::DeviceLost;
            }

            // Durasi maksimal.
            if rec.elapsed() >= max_duration {
                break StopReason::MaxDuration;
            }

            // Disk < 500 MB.
            if now.duration_since(last_disk_check) >= DISK_CHECK_EVERY {
                last_disk_check = now;
                if windows_integration::free_disk_bytes(&self.data_dir).is_some_and(|b| b < MIN_FREE_RECORDING_BYTES) {
                    break StopReason::DiskFull;
                }
            }

            // Hening 10 menit → peringatan; tanpa respons 2 menit → stop.
            let mut st = active.auto_stop.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(deadline) = st.warning_deadline {
                if now >= deadline {
                    break StopReason::Silence;
                }
                continue;
            }
            if rec.is_paused() {
                st.silent_since = None;
                continue;
            }
            let silent = mic_db < SILENCE_THRESHOLD_DBFS && sys_db < SILENCE_THRESHOLD_DBFS;
            if !silent {
                st.silent_since = None;
                continue;
            }
            let since = *st.silent_since.get_or_insert(now);
            if now.duration_since(since) >= silence_limit {
                st.warning_deadline = Some(now + AUTO_STOP_GRACE);
                self.events.emit_json(
                    events::EV_AUTO_STOP_WARNING,
                    serde_json::json!({ "reason": "silence", "secondsLeft": AUTO_STOP_GRACE.as_secs() }),
                );
            }
        };
        drop(active);
        if let Err(e) = self.stop(stop_reason) {
            tracing::warn!("auto-stop gagal: {}", e.message);
        }
    }
}

fn state_of(a: &Active) -> RecordingState {
    let r = &a.recorder;
    RecordingState {
        status: if r.is_paused() { "paused" } else { "recording" },
        meeting_id: Some(a.meeting_id.clone()),
        elapsed_ms: r.elapsed().as_millis() as i64,
        mic_muted: r.mic_muted(),
        mic_alive: r.alive(Channel::Mic),
        system_alive: r.alive(Channel::System),
    }
}

/// Tunggu sampai monitor melepas Arc-nya (≤ 1 tick), lalu ambil isinya.
fn wait_unique(mut a: Arc<Active>) -> Active {
    loop {
        match Arc::try_unwrap(a) {
            Ok(inner) => return inner,
            Err(back) => {
                a = back;
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
