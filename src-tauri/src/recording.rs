//! Service rekaman: validasi Start/Stop (§7.7), meeting di DB, monitor (level 10 Hz, auto-stop §7.6,
//! reconnect device §7.5). Tidak bergantung pada Tauri; UI dijangkau lewat `EventSink`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::audio::device_watch::DeviceWatcher;
use crate::audio::recorder::Recorder;
use crate::audio::test_tone::{self, AudioTestResult};
use crate::audio::writer::PartEvent;
use crate::audio::Channel;
use crate::config::providers::RecordingConfig;
use crate::config::settings;
use crate::db::repo_meetings::{self, NewMeeting};
use crate::db::{now_ms, repo_bookmarks, repo_parts, Db};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::events::{self, EventSink, MeetingUpdated};
use crate::windows_integration;

const MIN_FREE_START_BYTES: u64 = 1024 * 1024 * 1024;
const MIN_FREE_RECORDING_BYTES: u64 = 500 * 1024 * 1024;
const MIN_DURATION_MS: i64 = 5_000;
const SILENCE_THRESHOLD_DBFS: f32 = -50.0;
const AUTO_STOP_GRACE: Duration = Duration::from_secs(120);
const DISK_CHECK_EVERY: Duration = Duration::from_secs(30);
const REOPEN_EVERY: Duration = Duration::from_secs(1);
const REOPEN_MAX_ATTEMPTS: u32 = 10;
const MONITOR_TICK: Duration = Duration::from_millis(100);
/// Gagal tulis beruntun sebanyak ini (±beberapa detik audio) → rekaman dihentikan dengan aman.
const MAX_WRITE_FAILURES: u32 = 20;
/// Audio sistem dianggap tidak terdengar di bawah ini (loopback diam / volume 0 / device salah).
const SYSTEM_SILENT_DBFS: f32 = -70.0;
/// Peringatan "audio sistem tidak terdengar" setelah selama ini, jika mic sempat aktif.
const SYSTEM_SILENT_WARN_AFTER: Duration = Duration::from_secs(120);
/// Windows sering mengirim beberapa notifikasi default device beruntun; tunggu sebelum membuka ulang.
const DEVICE_SWITCH_DEBOUNCE: Duration = Duration::from_secs(1);
/// Peringatan sebelum batas durasi rekaman (lalu berlanjut sebagai meeting baru).
const LIMIT_WARN_BEFORE: Duration = Duration::from_secs(600);

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
    MeetingEnded,
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
    /// Alasan peringatan aktif (hening / aplikasi meeting selesai).
    meeting_ended: bool,
}

struct Active {
    meeting_id: String,
    source_app: Option<String>,
    dir: PathBuf,
    recorder: Recorder,
    monitor_stop: Arc<AtomicBool>,
    auto_stop: Mutex<AutoStopState>,
    /// Thread pencatat part ke DB (selesai sendiri setelah semua writer ditutup).
    part_db: Option<JoinHandle<()>>,
}

pub struct RecordingService {
    data_dir: PathBuf,
    db: Arc<Db>,
    events: Arc<dyn EventSink>,
    config: RecordingConfig,
    /// Dipanggil setelah meeting masuk antrean (membangunkan worker).
    on_queued: Box<dyn Fn() + Send + Sync>,
    active: Mutex<Option<Arc<Active>>>,
    /// Tes audio onboarding sedang berjalan (tidak boleh bersamaan dengan rekaman).
    testing: AtomicBool,
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
        Self { data_dir, db, events, config, on_queued, active: Mutex::new(None), testing: AtomicBool::new(false) }
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
        if guard.is_some() || self.testing.load(Ordering::SeqCst) {
            return Err(ErrorCode::AlreadyRecording.into());
        }
        if !crate::ai::keys_ready(&self.db.conn())? {
            return Err(ErrorCode::NoApiKey.into());
        }
        if windows_integration::free_disk_bytes(&self.data_dir).is_some_and(|b| b < MIN_FREE_START_BYTES) {
            return Err(ErrorCode::DiskFull.into());
        }

        let meeting_id = uuid::Uuid::new_v4().to_string();
        let now = now_ms();
        let language = settings::load(&self.db.conn())?.stt_language;
        // Nama meeting dari jendela Teams / tab Google Meet (langkah 46, A2); judul AI tetap boleh menggantikan.
        let detected = crate::windows_integration::meeting_window::meeting_title(
            &crate::windows_integration::meeting_window::default_meet_patterns(),
        );
        let source_app = source_app.or_else(|| detected.as_ref().map(|(k, _)| k.clone()));
        let title = detected.map_or_else(|| default_title(now), |(_, t)| t.chars().take(100).collect());
        repo_meetings::insert(
            &self.db.conn(),
            &NewMeeting {
                id: &meeting_id,
                title: &title,
                started_at: now,
                language: language.as_str(),
                source_app: source_app.as_deref(),
                // Consent diminta pengguna di luar aplikasi; kolom diisi waktu mulai rekam.
                consent_at: now,
            },
        )?;

        let rel_dir = format!("recordings/{meeting_id}");
        let dir = self.data_dir.join(&rel_dir);
        // Thread capture tidak boleh menunggu kunci DB (worker bisa memegangnya lama):
        // event part dikirim lewat channel ke thread pencatat.
        let (part_tx, part_rx) = mpsc::channel::<PartEvent>();
        let part_db = self.spawn_part_db(meeting_id.clone(), rel_dir, part_rx);
        let on_part = Arc::new(move |ev: PartEvent| {
            let _ = part_tx.send(ev);
        });

        let recorder = match Recorder::start(&dir, on_part) {
            Ok(r) => r,
            Err(e) => {
                // Semua writer sudah di-drop → thread pencatat selesai; tunggu sebelum menghapus meeting.
                if let Some(j) = part_db {
                    let _ = j.join();
                }
                let _ = repo_meetings::delete(&self.db.conn(), &meeting_id);
                let _ = std::fs::remove_dir_all(&dir);
                tracing::warn!("start rekaman gagal: {e}");
                return Err(AppError::from(e));
            }
        };
        let active = Arc::new(Active {
            meeting_id: meeting_id.clone(),
            source_app,
            dir,
            recorder,
            monitor_stop: Arc::new(AtomicBool::new(false)),
            auto_stop: Mutex::new(AutoStopState::default()),
            part_db,
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

    /// Tes 5 detik onboarding: rekam mic + loopback sambil memutar nada tes, emit level 10 Hz.
    pub fn run_audio_test(&self) -> AppResult<AudioTestResult> {
        if self.is_recording() || self.testing.swap(true, Ordering::SeqCst) {
            return Err(ErrorCode::AlreadyRecording.into());
        }
        let result = (|| {
            let tone = self.data_dir.join("test_tone.wav");
            if !tone.exists() {
                test_tone::write_tone(&tone)?;
            }
            let sink = self.events.clone();
            test_tone::run(
                || {
                    if !windows_integration::play_wav_async(&tone) {
                        tracing::warn!("nada tes gagal diputar");
                    }
                },
                |mic, sys| sink.emit_json(events::EV_RECORDING_LEVEL, serde_json::json!({ "micDbfs": mic, "systemDbfs": sys })),
            )
            .map_err(AppError::from)
        })();
        self.testing.store(false, Ordering::SeqCst);
        if let Ok(r) = &result {
            tracing::info!("tes audio: mic {:.1} dBFS, sistem {:.1} dBFS", r.mic_peak_dbfs, r.system_peak_dbfs);
        }
        result
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
                st.meeting_ended = false;
                st.silent_since = Some(Instant::now());
            })?;
            Ok(())
        } else {
            self.stop(StopReason::Manual).map(|_| ())
        }
    }

    /// Tandai momen penting di posisi rekaman saat ini (langkah 44). Tekan beruntun < 3 dtk dihitung sekali.
    pub fn bookmark(&self) -> AppResult<i64> {
        let (meeting_id, at_ms) = self.with_active(|a| (a.meeting_id.clone(), a.recorder.elapsed().as_millis() as i64))?;
        let conn = self.db.conn();
        if repo_bookmarks::last(&conn, &meeting_id)?.is_none_or(|last| at_ms - last >= 3_000) {
            repo_bookmarks::insert(&conn, &meeting_id, at_ms)?;
        }
        let count = repo_bookmarks::count(&conn, &meeting_id)?;
        drop(conn);
        self.events.emit_json(events::EV_RECORDING_BOOKMARK, serde_json::json!({ "atMs": at_ms, "count": count }));
        Ok(at_ms)
    }

    /// Aplikasi meeting selesai memakai mic: peringatan + hitung mundur Stop (bisa dibatalkan "Lanjut").
    pub fn offer_stop_meeting_ended(&self, countdown: Duration) {
        let Some(a) = self.current() else { return };
        {
            let mut st = a.auto_stop.lock().unwrap_or_else(|e| e.into_inner());
            if st.warning_deadline.is_some() {
                return;
            }
            st.warning_deadline = Some(Instant::now() + countdown);
            st.meeting_ended = true;
        }
        self.events.emit_json(
            events::EV_AUTO_STOP_WARNING,
            serde_json::json!({ "reason": "meeting_ended", "secondsLeft": countdown.as_secs() }),
        );
        self.events.notify(
            "Meeting sudah selesai?",
            &format!("Rekaman berhenti otomatis dalam {} detik. Pilih Lanjut di widget untuk terus merekam.", countdown.as_secs()),
        );
    }

    /// Stop. `Ok(None)` jika durasi < 5 detik (meeting & file dibuang).
    pub fn stop(&self, reason: StopReason) -> AppResult<Option<String>> {
        let active = self.lock_active().take().ok_or_else(|| AppError::new(ErrorCode::NotRecording))?;
        active.monitor_stop.store(true, Ordering::SeqCst);
        let ended_at = now_ms();
        // Monitor memegang Arc juga; ambil Recorder lewat try_unwrap setelah monitor lepas.
        let active = wait_unique(active);
        let Active { meeting_id, dir, recorder, part_db, .. } = active;
        let result = recorder.stop();
        // Writer sudah ditutup → pencatat part selesai; tunggu agar status part final sebelum masuk antrean.
        if let Some(j) = part_db {
            let _ = j.join();
        }
        self.events.recording_changed(false);
        self.emit_state();

        let result = match result {
            Ok(r) => r,
            Err(e) => {
                // Jangan pernah menghapus rekaman karena Stop gagal: selamatkan part yang ada di disk.
                tracing::error!("stop rekaman gagal: {e}");
                let duration_ms = crate::queue::recovery::salvage_parts(&self.data_dir, &self.db, &meeting_id).unwrap_or(0);
                let conn = self.db.conn();
                repo_meetings::set_duration(&conn, &meeting_id, duration_ms)?;
                repo_meetings::set_status(&conn, &meeting_id, repo_meetings::MeetingStatus::Interrupted)?;
                drop(conn);
                tracing::warn!("meeting {meeting_id} ditandai terputus: {duration_ms} ms audio diselamatkan");
                events::emit(self.events.as_ref(), events::EV_MEETING_UPDATED, &MeetingUpdated { meeting_id: &meeting_id });
                self.events.notify(
                    "Rekaman tidak tersimpan sempurna",
                    "Audio yang sempat terekam diselamatkan. Buka aplikasi lalu pilih Proses pada rekaman yang terputus.",
                );
                return Ok(Some(meeting_id));
            }
        };
        tracing::info!("rekaman berhenti ({reason:?}): meeting {meeting_id}, {} ms", result.duration_ms);

        // Hanya rekaman yang benar-benar tersimpan lengkap dan < 5 dtk yang dibuang.
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
            // Pemberitahuan dikirim oleh `continue_after_limit` (rekaman berlanjut sebagai meeting baru).
            StopReason::MaxDuration => None,
            StopReason::DiskFull => Some("Ruang disk hampir habis. Rekaman dihentikan dan sedang diproses."),
            StopReason::DeviceLost => Some("Perangkat audio tidak tersedia. Rekaman dihentikan dan sedang diproses."),
            StopReason::MeetingEnded => Some("Meeting sudah selesai. Rekaman dihentikan otomatis dan sedang diproses."),
        };
        if let Some(body) = note {
            self.events.notify("Rekaman dihentikan", body);
        }
        Ok(Some(meeting_id))
    }

    fn spawn_part_db(&self, meeting_id: String, rel_dir: String, rx: mpsc::Receiver<PartEvent>) -> Option<JoinHandle<()>> {
        let db = self.db.clone();
        let spawned = std::thread::Builder::new().name("part-db".into()).spawn(move || {
            for ev in rx {
                let conn = db.conn();
                let res = match ev {
                    PartEvent::Opened { channel, part_index, .. } => {
                        let rel = format!("{rel_dir}/{}", crate::audio::writer::PartWriter::part_file_name(channel, part_index));
                        repo_parts::insert_open(&conn, &meeting_id, channel.as_str(), part_index as i64, &rel)
                    }
                    PartEvent::Finalized { channel, part_index, samples } => {
                        repo_parts::mark_finalized(&conn, &meeting_id, channel.as_str(), part_index as i64, samples as i64)
                    }
                };
                if let Err(e) = res {
                    tracing::error!("catat part gagal: {}", e.message);
                }
            }
        });
        match spawned {
            Ok(j) => Some(j),
            Err(e) => {
                tracing::error!("thread pencatat part gagal dibuat: {e}");
                None
            }
        }
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

    fn monitor_loop(self: &Arc<Self>, active: Arc<Active>) {
        let silence_limit = Duration::from_secs(u64::from(self.config.auto_stop_silence_min) * 60);
        let max_duration = Duration::from_secs(u64::from(self.config.max_recording_hours) * 3600);
        let mut health = [ChannelHealth::default(), ChannelHealth::default()];
        let mut last_disk_check = Instant::now();
        let mut last_alive = [true, true];
        let watcher = DeviceWatcher::start();
        let mut system_health = SystemHealth::default();
        let mut switch_at: [Option<Instant>; 2] = [None, None];
        let mut limit_warned = false;

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

            // Default device diganti di Windows → pindah ke device baru (stream lama tidak ikut pindah).
            if let Some(w) = &watcher {
                for channel in w.changes() {
                    switch_at[channel_idx(channel)] = Some(now + DEVICE_SWITCH_DEBOUNCE);
                }
            }
            for (i, channel) in [Channel::Mic, Channel::System].into_iter().enumerate() {
                if switch_at[i].is_none_or(|t| now < t) {
                    continue;
                }
                switch_at[i] = None;
                match rec.reopen(channel) {
                    Ok(()) => health[i] = ChannelHealth::default(),
                    // Channel ditandai mati; reconnect di bawah mencoba lagi.
                    Err(e) => tracing::warn!("pindah device {} gagal: {e}", channel.as_str()),
                }
            }

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

            // Audio sistem tidak terdengar padahal mic aktif → beri tahu saat meeting, bukan setelahnya.
            if let Some(warn) = system_health.update(now, rec.is_paused(), rec.mic_muted(), mic_db, sys_db) {
                let code = if warn { "system_silent" } else { "system_ok" };
                self.events.emit_json(
                    events::EV_RECORDING_WARNING,
                    serde_json::json!({ "code": code, "channel": "system", "minutes": SYSTEM_SILENT_WARN_AFTER.as_secs() / 60 }),
                );
            }

            // Disk penuh / file terkunci: berhenti dengan aman sebelum audio bolong makin banyak.
            if rec.write_failures() >= MAX_WRITE_FAILURES {
                self.events.emit_json(events::EV_RECORDING_WARNING, serde_json::json!({ "code": "write_failed", "channel": "mic" }));
                break StopReason::DiskFull;
            }

            // Durasi maksimal: peringatan 10 menit sebelumnya, lalu berlanjut sebagai meeting baru.
            let elapsed = rec.elapsed();
            if !limit_warned && elapsed + LIMIT_WARN_BEFORE >= max_duration {
                limit_warned = true;
                let minutes = max_duration.saturating_sub(elapsed).as_secs().div_ceil(60);
                self.events.emit_json(
                    events::EV_RECORDING_WARNING,
                    serde_json::json!({ "code": "limit_soon", "channel": "mic", "minutes": minutes }),
                );
                self.events.notify(
                    "Batas durasi rekaman sebentar lagi",
                    &format!(
                        "Dalam {minutes} menit rekaman ini ditutup lalu langsung berlanjut sebagai meeting baru \"(lanjutan)\"."
                    ),
                );
            }
            if elapsed >= max_duration {
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
                    break if st.meeting_ended { StopReason::MeetingEnded } else { StopReason::Silence };
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
                    serde_json::json!({
                        "reason": "silence",
                        "secondsLeft": AUTO_STOP_GRACE.as_secs(),
                        "silenceMin": self.config.auto_stop_silence_min,
                    }),
                );
            }
        };
        let source_app = active.source_app.clone();
        let prev_id = active.meeting_id.clone();
        drop(active);
        if let Err(e) = self.stop(stop_reason) {
            tracing::warn!("auto-stop gagal: {}", e.message);
        }
        if stop_reason == StopReason::MaxDuration {
            self.continue_after_limit(&prev_id, source_app);
        }
    }

    /// Batas durasi tercapai: rekaman langsung dilanjutkan sebagai meeting baru "<judul> (lanjutan)".
    fn continue_after_limit(self: &Arc<Self>, prev_id: &str, source_app: Option<String>) {
        let prev_title = repo_meetings::get(&self.db.conn(), prev_id).map(|m| m.title).ok();
        match self.start(source_app) {
            Ok(new_id) => {
                let base = prev_title.unwrap_or_else(|| default_title(now_ms()));
                let base = base.trim_end_matches(" (lanjutan)");
                if let Err(e) = repo_meetings::rename(&self.db.conn(), &new_id, &format!("{base} (lanjutan)")) {
                    tracing::warn!("judul lanjutan gagal disimpan: {}", e.message);
                }
                events::emit(self.events.as_ref(), events::EV_MEETING_UPDATED, &MeetingUpdated { meeting_id: &new_id });
                tracing::info!("rekaman berlanjut setelah batas durasi: {prev_id} → {new_id}");
                self.events.notify(
                    "Rekaman berlanjut",
                    "Batas durasi tercapai. Bagian sebelumnya sedang diproses; rekaman berlanjut sebagai meeting baru.",
                );
            }
            Err(e) => {
                tracing::warn!("lanjutan rekaman gagal dimulai: {}", e.message);
                self.events.notify(
                    "Rekaman dihentikan",
                    "Durasi maksimal rekaman tercapai dan rekaman tidak bisa dilanjutkan. Rekaman sebelumnya sedang diproses.",
                );
            }
        }
    }
}

/// Pelacak "audio sistem tidak terdengar" (langkah 24).
#[derive(Default)]
struct SystemHealth {
    silent_since: Option<Instant>,
    mic_active: bool,
    warned: bool,
}

impl SystemHealth {
    /// `Some(true)` = mulai peringatan, `Some(false)` = peringatan selesai, `None` = tidak berubah.
    fn update(&mut self, now: Instant, paused: bool, mic_muted: bool, mic_db: f32, sys_db: f32) -> Option<bool> {
        if paused {
            self.silent_since = None;
            self.mic_active = false;
            return None;
        }
        if sys_db >= SYSTEM_SILENT_DBFS {
            self.silent_since = None;
            self.mic_active = false;
            return std::mem::take(&mut self.warned).then_some(false);
        }
        let since = *self.silent_since.get_or_insert(now);
        if !mic_muted && mic_db >= SILENCE_THRESHOLD_DBFS {
            self.mic_active = true;
        }
        if !self.warned && self.mic_active && now.duration_since(since) >= SYSTEM_SILENT_WARN_AFTER {
            self.warned = true;
            return Some(true);
        }
        None
    }
}

fn channel_idx(channel: Channel) -> usize {
    match channel {
        Channel::Mic => 0,
        Channel::System => 1,
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
