//! Worker tunggal (PRD §13): `preprocessing → transcribing → merging → summarizing → done`.
//! Setiap step membaca status dari DB sehingga bisa dilanjutkan setelah restart.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use chrono::{Local, TimeZone};
use serde::Serialize;
use tokio::sync::Notify;

use crate::config::providers::ProvidersConfig;
use crate::config::settings::{self, AudioRetention, DEFAULT_SYSTEM_LABEL};
use crate::db::repo_meetings::{self, MeetingRow, MeetingStatus};
use crate::db::{now_ms, repo_chunks, repo_segments, repo_summary, Db};
use crate::error::{AppError, ErrorCode};
use crate::events::{self, EventSink, MeetingUpdated};
use crate::ai::{self, Endpoint, Role};
use crate::ai_http::ProviderError;
use crate::llm::openai::OpenAiLlm;
use crate::llm::{ChatMessage, LlmProvider, LlmRequest};
use crate::pipeline::summarize::{self, LlmCaller, SummarizeInput, SummaryOutcome};
use crate::pipeline::{dedup, merge};
use crate::queue::rate_limiter::{self, Admission, Cost};
use crate::queue::state::{StepError, StepResult};
use crate::stt::openai::OpenAiStt;
use crate::stt::{SttProvider, SttRequest, SttSegment};
use crate::preprocess;

const RATE_MAX_ATTEMPTS: u32 = 6;
const NET_MAX_ATTEMPTS: u32 = 5;
const QUOTA_RETRY_MS: i64 = 15 * 60 * 1000;
const NETWORK_RETRY_MS: i64 = 60 * 1000;
const IDLE_MAX_SLEEP: Duration = Duration::from_secs(60);
/// Satu transkripsi chunk yang berjalan (lihat `step_transcribe`).
/// Opsi STT per meeting (bahasa + prompt glosarium).
struct SttOpts {
    language: Option<String>,
    prompt: Option<String>,
}

type ChunkJob<'a> = Pin<Box<dyn Future<Output = (&'a repo_chunks::ChunkRow, StepResult<Vec<SttSegment>>)> + Send + 'a>>;

/// Chunk STT yang dikirim bersamaan (urutan hasil tidak penting: merge mengurutkan menurut waktu).
const STT_CONCURRENCY: usize = 3;
const STT_CONCURRENCY_GROQ: usize = 2;
/// Retensi "7 hari": audio meeting selesai yang lebih tua dari ini dihapus.
const AUDIO_KEEP_MS: i64 = 7 * 24 * 60 * 60 * 1000;
const RETENTION_SWEEP_EVERY: Duration = Duration::from_secs(60 * 60);
/// Jeda antar-putaran transkripsi bertahap selama merekam.
const LIVE_EVERY: Duration = Duration::from_secs(60);
/// Rekaman terputus yang tidak pernah diproses: audionya dibuang setelah 30 hari.
const INTERRUPTED_KEEP_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// Chunk yang ditolak 413 dipecah dua, maksimal sedalam ini (≤ 4 bagian).
const MAX_SPLIT_DEPTH: u32 = 2;
/// `delete_meeting` menunggu worker melepas meeting yang dibatalkan maksimal selama ini.
const CANCEL_WAIT: Duration = Duration::from_secs(5);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JobProgress<'a> {
    meeting_id: &'a str,
    status: &'a str,
    progress_done: i64,
    progress_total: i64,
}

pub struct Worker {
    data_dir: PathBuf,
    db: Arc<Db>,
    providers: ProvidersConfig,
    http: reqwest::Client,
    events: Arc<dyn EventSink>,
    wake: Arc<Notify>,
    /// Antrean dijeda karena API key tidak valid (PRD §9.5).
    paused: AtomicBool,
    /// Meeting yang sedang diproses.
    current: Mutex<Option<String>>,
    /// Pembatal job yang sedang berjalan (meeting dihapus): `notify_one` menghentikan step di await berikutnya.
    cancel: Mutex<Option<(String, Arc<Notify>)>>,
    /// Meeting yang transkripsi bertahapnya dimatikan (key tidak valid / kuota habis); diproses setelah Stop.
    live_off: Mutex<Option<String>>,
}

/// Backoff `2s × 2^n` + jitter 0–1 dtk.
fn backoff(attempt: u32) -> Duration {
    let jitter_ms = u64::from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_millis()));
    Duration::from_secs(2u64.saturating_mul(1 << attempt.min(6))) + Duration::from_millis(jitter_ms)
}

/// Pesan untuk pengguna (bahasa sehari-hari + langkah perbaikan); detail teknis hanya di log.
/// Pesan yang menyebut "Pengaturan" membuat UI menampilkan tombol "Buka Pengaturan".
fn provider_failure(e: &ProviderError) -> AppError {
    tracing::warn!("provider gagal: {e:?}");
    let msg = match e {
        ProviderError::PayloadTooLarge => {
            "File audio terlalu besar untuk layanan transkrip. Coba lagi; aplikasi akan memecahnya menjadi bagian lebih kecil."
                .to_string()
        }
        ProviderError::BadRequest(m) => {
            let lower = m.to_lowercase();
            let model_problem = lower.contains("model")
                && ["not found", "does not exist", "decommissioned", "not supported", "invalid", "unknown"]
                    .iter()
                    .any(|k| lower.contains(k));
            if model_problem {
                "Model tidak dikenali oleh penyedia AI. Periksa nama model di Pengaturan → Layanan AI.".to_string()
            } else if m.starts_with("HTTP 404") {
                "Alamat atau model tidak ditemukan di penyedia AI. Periksa Pengaturan → Layanan AI.".to_string()
            } else if m.starts_with("HTTP 403") {
                "Akun penyedia AI tidak punya akses ke layanan ini. Periksa API key dan model di Pengaturan → Layanan AI."
                    .to_string()
            } else {
                "Penyedia AI menolak permintaan. Coba lagi; jika terus terjadi, periksa model di Pengaturan → Layanan AI."
                    .to_string()
            }
        }
        ProviderError::InvalidResponse(_) => {
            "Penyedia AI mengirim jawaban yang tidak bisa dibaca. Coba lagi beberapa saat lagi.".to_string()
        }
        _ => "Layanan AI sedang bermasalah. Coba lagi beberapa saat lagi.".to_string(),
    };
    AppError::with_message(ErrorCode::Internal, msg)
}

impl Worker {
    pub fn new(
        data_dir: PathBuf,
        db: Arc<Db>,
        providers: ProvidersConfig,
        http: reqwest::Client,
        events: Arc<dyn EventSink>,
        wake: Arc<Notify>,
    ) -> Self {
        let paused = repo_meetings::has_invalid_key_failure(&db.conn()).unwrap_or(false);
        Self {
            data_dir,
            db,
            providers,
            http,
            events,
            wake,
            paused: AtomicBool::new(paused),
            current: Mutex::new(None),
            cancel: Mutex::new(None),
            live_off: Mutex::new(None),
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// API key baru lolos uji: meeting yang gagal karena key → antre lagi, antrean lanjut.
    pub fn resume_after_new_key(&self) {
        match repo_meetings::requeue_invalid_key(&self.db.conn()) {
            Ok(n) if n > 0 => tracing::info!("{n} meeting diantrekan lagi setelah API key baru"),
            Ok(_) => {}
            Err(e) => tracing::warn!("requeue gagal: {}", e.message),
        }
        self.paused.store(false, Ordering::SeqCst);
        self.wake.notify_one();
        events::emit(self.events.as_ref(), events::EV_MEETING_UPDATED, &MeetingUpdated { meeting_id: "" });
    }

    pub fn current_meeting(&self) -> Option<String> {
        self.current.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Batalkan job meeting `id` jika sedang diproses, lalu tunggu worker melepasnya (request Groq
    /// yang sedang berjalan dihentikan sehingga kuota tidak terbuang dan tidak ada error palsu di log).
    pub async fn cancel_and_wait(&self, id: &str) {
        let token = {
            let guard = self.cancel.lock().unwrap_or_else(|e| e.into_inner());
            guard.as_ref().filter(|(cid, _)| cid == id).map(|(_, n)| n.clone())
        };
        let Some(token) = token else { return };
        token.notify_one();
        let start = std::time::Instant::now();
        while self.current_meeting().as_deref() == Some(id) && start.elapsed() < CANCEL_WAIT {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Hapus folder `path` di thread blocking (folder rekaman bisa berisi ratusan MB).
    async fn remove_dir(&self, path: PathBuf) {
        let result = tokio::task::spawn_blocking(move || if path.exists() { std::fs::remove_dir_all(&path) } else { Ok(()) }).await;
        if let Ok(Err(e)) = result {
            tracing::warn!("hapus folder gagal: {e}");
        }
    }

    async fn delete_audio(&self, id: &str) -> Result<(), AppError> {
        self.remove_dir(self.data_dir.join("recordings").join(id)).await;
        repo_meetings::set_audio_deleted(&self.db.conn(), id)
    }

    /// Retensi: "7 hari" untuk meeting selesai/gagal; rekaman terputus yang tidak diproses dibuang setelah
    /// 30 hari (apa pun pilihan retensi) dan ditandai gagal agar bannernya hilang.
    async fn sweep_retention(&self) {
        let retention = settings::load(&self.db.conn()).map(|s| s.audio_retention);
        if retention.ok() == Some(AudioRetention::Days7) {
            let expired = repo_meetings::audio_expired(&self.db.conn(), now_ms() - AUDIO_KEEP_MS).unwrap_or_default();
            for id in expired {
                match self.delete_audio(&id).await {
                    Ok(()) => {
                        tracing::info!("audio meeting {id} dihapus (retensi 7 hari)");
                        events::emit(self.events.as_ref(), events::EV_MEETING_UPDATED, &MeetingUpdated { meeting_id: &id });
                    }
                    Err(e) => tracing::warn!("retensi {id} gagal: {}", e.message),
                }
            }
        }
        let stale = repo_meetings::interrupted_older_than(&self.db.conn(), now_ms() - INTERRUPTED_KEEP_MS).unwrap_or_default();
        for id in stale {
            if self.delete_audio(&id).await.is_ok() {
                let err = AppError::with_message(
                    ErrorCode::AudioNotAvailable,
                    "Rekaman terputus ini tidak diproses selama 30 hari, jadi audionya dihapus otomatis.",
                );
                let _ = repo_meetings::set_failed(&self.db.conn(), &id, MeetingStatus::Preprocessing, &err);
                tracing::info!("audio rekaman terputus {id} dihapus (30 hari)");
                events::emit(self.events.as_ref(), events::EV_MEETING_UPDATED, &MeetingUpdated { meeting_id: &id });
            }
        }
    }

    pub async fn run(self: Arc<Self>) {
        tracing::info!("worker antrean mulai");
        let mut last_sweep: Option<std::time::Instant> = None;
        let mut last_live = std::time::Instant::now();
        loop {
            if last_sweep.is_none_or(|t| t.elapsed() >= RETENTION_SWEEP_EVERY) {
                last_sweep = Some(std::time::Instant::now());
                self.sweep_retention().await;
            }
            if last_live.elapsed() >= LIVE_EVERY {
                last_live = std::time::Instant::now();
                self.live_tick().await;
            }
            let next = if self.is_paused() { Ok(None) } else { repo_meetings::next_job(&self.db.conn(), now_ms()) };
            match next {
                Ok(Some(m)) => {
                    *self.current.lock().unwrap_or_else(|e| e.into_inner()) = Some(m.id.clone());
                    let token = Arc::new(Notify::new());
                    *self.cancel.lock().unwrap_or_else(|e| e.into_inner()) = Some((m.id.clone(), token.clone()));
                    self.events.processing_changed(true);
                    let id = m.id.clone();
                    tokio::select! {
                        _ = self.process(m) => {}
                        _ = token.notified() => tracing::info!("meeting {id} dibatalkan (dihapus) saat diproses"),
                    }
                    *self.cancel.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    *self.current.lock().unwrap_or_else(|e| e.into_inner()) = None;
                }
                Ok(None) => {
                    self.events.processing_changed(false);
                    let sleep = repo_meetings::earliest_waiting(&self.db.conn())
                        .ok()
                        .flatten()
                        .map(|t| Duration::from_millis((t - now_ms()).max(1000) as u64))
                        .unwrap_or(IDLE_MAX_SLEEP)
                        .min(IDLE_MAX_SLEEP);
                    tokio::select! {
                        _ = self.wake.notified() => {}
                        _ = tokio::time::sleep(sleep) => {}
                    }
                }
                Err(e) => {
                    tracing::error!("worker: query gagal: {}", e.message);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    fn emit_progress(&self, id: &str, status: MeetingStatus, done: i64, total: i64) {
        events::emit(
            self.events.as_ref(),
            events::EV_JOB_PROGRESS,
            &JobProgress { meeting_id: id, status: status.as_str(), progress_done: done, progress_total: total },
        );
    }

    fn enter_step(&self, id: &str, step: MeetingStatus, total: i64) -> StepResult<()> {
        let conn = self.db.conn();
        repo_meetings::get(&conn, id)?; // dihapus → Cancelled
        repo_meetings::set_status(&conn, id, step)?;
        repo_meetings::set_progress(&conn, id, 0, total)?;
        drop(conn);
        self.emit_progress(id, step, 0, total);
        Ok(())
    }

    fn set_progress(&self, id: &str, step: MeetingStatus, done: i64, total: i64) {
        if let Err(e) = repo_meetings::set_progress(&self.db.conn(), id, done, total) {
            tracing::warn!("progres gagal disimpan: {}", e.message);
        }
        self.emit_progress(id, step, done, total);
    }

    async fn process(&self, m: MeetingRow) {
        let id = m.id.clone();
        let mut step = match m.status {
            MeetingStatus::Queued => MeetingStatus::Preprocessing,
            MeetingStatus::WaitingQuota | MeetingStatus::WaitingNetwork => m
                .failed_step
                .as_deref()
                .and_then(MeetingStatus::parse)
                .filter(|s| s.is_running_step())
                .unwrap_or(MeetingStatus::Preprocessing),
            s => s,
        };
        tracing::info!("proses meeting {id} mulai di step {}", step.as_str());
        let started = std::time::Instant::now();
        loop {
            let result = match step {
                MeetingStatus::Preprocessing => self.step_preprocess(&id).await,
                MeetingStatus::Transcribing => self.step_transcribe(&m).await,
                MeetingStatus::Merging => self.step_merge(&id).await,
                MeetingStatus::Summarizing => self.step_summarize(&m).await,
                _ => Ok(()),
            };
            if let Err(e) = result {
                self.handle_error(&id, step, e);
                return;
            }
            step = match step {
                MeetingStatus::Preprocessing => MeetingStatus::Transcribing,
                MeetingStatus::Transcribing => MeetingStatus::Merging,
                MeetingStatus::Merging => MeetingStatus::Summarizing,
                _ => break,
            };
        }
        if repo_meetings::set_status(&self.db.conn(), &id, MeetingStatus::Done).is_ok() {
            if let Err(e) = crate::db::repo_search::reindex(&mut self.db.conn(), &id) {
                tracing::warn!("index pencarian {id} gagal: {}", e.message);
            }
            let row = repo_meetings::get(&self.db.conn(), &id).ok();
            // Metrik "Stop → Notulen siap" (feedback §E) dibaca dari log ini.
            let since_stop = row.as_ref().and_then(|r| r.ended_at).map_or(-1, |t| (now_ms() - t) / 1000);
            tracing::info!("meeting {id} selesai dalam {} dtk ({since_stop} dtk sejak Stop)", started.elapsed().as_secs());
            self.emit_progress(&id, MeetingStatus::Done, 0, 0);
            events::emit(self.events.as_ref(), events::EV_MEETING_UPDATED, &MeetingUpdated { meeting_id: &id });
            if let Some(r) = row {
                self.events.meeting_done(&id, &r.title);
                // Audio masih disimpan → siapkan file putar sekarang agar klik pertama di transkrip langsung jalan.
                if !r.audio_deleted {
                    let (data_dir, db, mid) = (self.data_dir.clone(), self.db.clone(), id.clone());
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = crate::playback::prepare(&data_dir, &db, &mid) {
                            tracing::warn!("audio putar {mid} gagal disiapkan: {}", e.message);
                        }
                    });
                }
            }
        }
    }

    fn handle_error(&self, id: &str, step: MeetingStatus, e: StepError) {
        let conn = self.db.conn();
        let status = match e {
            StepError::Cancelled => {
                tracing::info!("meeting {id} dihapus saat diproses");
                return;
            }
            StepError::TooLarge => {
                let err = provider_failure(&ProviderError::PayloadTooLarge);
                tracing::warn!("meeting {id} gagal di {}: file terlalu besar", step.as_str());
                let _ = repo_meetings::set_failed(&conn, id, step, &err);
                MeetingStatus::Failed
            }
            StepError::Failed(err) => {
                tracing::warn!("meeting {id} gagal di {}: {}", step.as_str(), err.code.as_str());
                let _ = repo_meetings::set_failed(&conn, id, step, &err);
                MeetingStatus::Failed
            }
            StepError::Unauthorized(err) => {
                tracing::warn!("meeting {id}: API key tidak valid, antrean dijeda");
                let _ = repo_meetings::set_failed(&conn, id, step, &err);
                self.paused.store(true, Ordering::SeqCst);
                self.events.notify("API key layanan AI tidak valid", err.code.message());
                MeetingStatus::Failed
            }
            StepError::WaitingQuota(at) => {
                let _ = repo_meetings::set_waiting(&conn, id, MeetingStatus::WaitingQuota, step, at);
                MeetingStatus::WaitingQuota
            }
            StepError::WaitingNetwork(at) => {
                let _ = repo_meetings::set_waiting(&conn, id, MeetingStatus::WaitingNetwork, step, at);
                MeetingStatus::WaitingNetwork
            }
        };
        drop(conn);
        self.emit_progress(id, status, 0, 0);
        events::emit(self.events.as_ref(), events::EV_MEETING_UPDATED, &MeetingUpdated { meeting_id: id });
    }

    /// Penyedia + key untuk satu peran. Key wajib belum ada → antrean dijeda seperti key tidak valid.
    fn endpoint(&self, role: Role) -> StepResult<(Endpoint, Option<String>)> {
        let e = ai::endpoint(&self.db.conn(), role, &self.providers)?;
        match ai::api_key_for(&e) {
            Ok(k) => Ok((e, k)),
            Err(err) if err.code == ErrorCode::NoApiKey => Err(StepError::Unauthorized(err)),
            Err(err) => Err(StepError::Failed(err)),
        }
    }

    /// Tunggu sampai rate limiter mengizinkan (PRD §9.5). Batas di providers.json adalah batas free tier
    /// Groq, jadi hanya berlaku jika peran ini memakai Groq; penyedia lain cukup ditangani lewat 429.
    async fn admit(&self, cost: Cost) -> StepResult<()> {
        let role = match cost {
            Cost::Stt { .. } => Role::Stt,
            Cost::Llm { .. } => Role::Llm,
        };
        if ai::provider_of(&self.db.conn(), role)? != ai::GROQ {
            return Ok(());
        }
        loop {
            let adm = rate_limiter::check(&self.db.conn(), &self.providers.limits, cost, now_ms())?;
            match adm {
                Admission::Go => return Ok(()),
                Admission::Wait(d) => tokio::time::sleep(d).await,
                Admission::NextDay(at) => return Err(StepError::WaitingQuota(at)),
            }
        }
    }

    /// Jalankan request provider dengan aturan retry PRD §9.5.
    async fn with_retry<T, F, Fut>(&self, cost: Cost, mut f: F) -> StepResult<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, ProviderError>>,
    {
        let mut rate_attempts = 0u32;
        let mut net_attempts = 0u32;
        loop {
            self.admit(cost).await?;
            match f().await {
                Ok(v) => return Ok(v),
                Err(ProviderError::RateLimited { retry_after }) => {
                    rate_attempts += 1;
                    if rate_attempts >= RATE_MAX_ATTEMPTS {
                        return Err(StepError::WaitingQuota(now_ms() + QUOTA_RETRY_MS));
                    }
                    let wait = retry_after.unwrap_or_else(|| backoff(rate_attempts));
                    tracing::info!("429, tunggu {} ms (percobaan {rate_attempts})", wait.as_millis());
                    tokio::time::sleep(wait).await;
                }
                Err(ProviderError::Server(code)) => {
                    net_attempts += 1;
                    tracing::info!("server error {code} (percobaan {net_attempts})");
                    if net_attempts >= NET_MAX_ATTEMPTS {
                        return Err(StepError::WaitingNetwork(now_ms() + NETWORK_RETRY_MS));
                    }
                    tokio::time::sleep(backoff(net_attempts)).await;
                }
                Err(ProviderError::Network(_)) => {
                    net_attempts += 1;
                    tracing::info!("jaringan gagal (percobaan {net_attempts})");
                    if net_attempts >= NET_MAX_ATTEMPTS {
                        return Err(StepError::WaitingNetwork(now_ms() + NETWORK_RETRY_MS));
                    }
                    tokio::time::sleep(backoff(net_attempts)).await;
                }
                Err(ProviderError::Unauthorized) => {
                    return Err(StepError::Unauthorized(ErrorCode::InvalidApiKey.into()));
                }
                Err(ProviderError::PayloadTooLarge) => return Err(StepError::TooLarge),
                Err(e) => return Err(StepError::Failed(provider_failure(&e))),
            }
        }
    }

    async fn step_preprocess(&self, id: &str) -> StepResult<()> {
        self.enter_step(id, MeetingStatus::Preprocessing, 1)?;
        let (dir, db, mid) = (self.data_dir.clone(), self.db.clone(), id.to_string());
        let target = self.providers.pipeline.stt_chunk_target_sec;
        let n = tokio::task::spawn_blocking(move || preprocess::run(&dir, &db, &mid, target))
            .await
            .map_err(|e| StepError::Failed(AppError::internal(e)))??;
        tracing::info!("meeting {id}: {n} upload chunk");
        self.set_progress(id, MeetingStatus::Preprocessing, 1, 1);
        Ok(())
    }

    async fn step_transcribe(&self, m: &MeetingRow) -> StepResult<()> {
        let id = &m.id;
        let chunks = repo_chunks::list(&self.db.conn(), id)?;
        let total = chunks.len() as i64;
        self.enter_step(id, MeetingStatus::Transcribing, total)?;
        let mut done = chunks.iter().filter(|c| c.stt_status == "done").count() as i64;
        self.set_progress(id, MeetingStatus::Transcribing, done, total);
        if done == total {
            return Ok(());
        }
        let (e, api_key) = self.endpoint(Role::Stt)?;
        // Beberapa chunk sekaligus (mic & sistem paralel). Groq dibatasi lebih ketat karena kuota per menit/jam.
        let concurrency = if e.is_groq() { STT_CONCURRENCY_GROQ } else { STT_CONCURRENCY };
        let stt = OpenAiStt { http: self.http.clone(), base_url: e.base_url, api_key, model: e.model };
        let opts = self.stt_opts(m);
        // Future dibuat lebih dulu (Vec) agar tidak ada closure tersimpan di state future worker (`Send`).
        let mut jobs: Vec<ChunkJob<'_>> = Vec::new();
        for c in chunks.iter().filter(|c| c.stt_status != "done") {
            jobs.push(Box::pin(self.transcribe_chunk(&stt, &opts, c)));
        }
        let mut results = futures_util::stream::iter(jobs).buffer_unordered(concurrency);
        while let Some((c, segments)) = results.next().await {
            let cost = Cost::Stt { audio_sec: c.duration_ms as f64 / 1000.0 };
            let segments = match segments {
                Ok(s) => s,
                Err(e) => {
                    if let StepError::Failed(err) = &e {
                        let _ = repo_chunks::mark_failed(&self.db.conn(), c.id, &err.message);
                    }
                    return Err(e);
                }
            };
            {
                let conn = self.db.conn();
                rate_limiter::record(&conn, cost, None, now_ms())?;
                repo_chunks::mark_done(&conn, c.id, &serde_json::to_string(&segments).map_err(AppError::from)?)?;
            }
            done += 1;
            self.set_progress(id, MeetingStatus::Transcribing, done, total);
        }
        Ok(())
    }

    /// Transkripsi bertahap (langkah 37): selama merekam, audio final yang terkumpul (≥ 5 menit per channel)
    /// dipraproses dan ditranskrip, sehingga setelah Stop hanya sisa terakhir + ringkasan yang dikerjakan.
    /// Gagal apa pun dilewati diam-diam; semuanya tetap diproses lewat jalur biasa setelah Stop.
    async fn live_tick(&self) {
        let recording = repo_meetings::list_by_status(&self.db.conn(), MeetingStatus::Recording).unwrap_or_default();
        let Some(m) = recording.into_iter().next() else { return };
        let id = m.id.clone();
        if self.live_off.lock().unwrap_or_else(|e| e.into_inner()).as_deref() == Some(id.as_str()) {
            return;
        }
        let Ok((e, api_key)) = self.endpoint(Role::Stt) else { return };

        let (dir, db, mid) = (self.data_dir.clone(), self.db.clone(), id.clone());
        let target = self.providers.pipeline.stt_chunk_target_sec;
        match tokio::task::spawn_blocking(move || preprocess::run_live(&dir, &db, &mid, target)).await {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => {
                tracing::warn!("praproses bertahap {id} gagal: {}", err.message);
                return;
            }
            Err(_) => return,
        }

        let chunks = repo_chunks::list(&self.db.conn(), &id).unwrap_or_default();
        if chunks.iter().all(|c| c.stt_status == "done") {
            return;
        }
        let concurrency = if e.is_groq() { STT_CONCURRENCY_GROQ } else { STT_CONCURRENCY };
        let stt = OpenAiStt { http: self.http.clone(), base_url: e.base_url, api_key, model: e.model };
        let opts = self.stt_opts(&m);
        let mut jobs: Vec<ChunkJob<'_>> = Vec::new();
        for c in chunks.iter().filter(|c| c.stt_status != "done") {
            jobs.push(Box::pin(self.transcribe_chunk(&stt, &opts, c)));
        }
        let mut results = futures_util::stream::iter(jobs).buffer_unordered(concurrency);
        let mut done = 0;
        while let Some((c, result)) = results.next().await {
            match result {
                Ok(segments) => {
                    let conn = self.db.conn();
                    let cost = Cost::Stt { audio_sec: c.duration_ms as f64 / 1000.0 };
                    let _ = rate_limiter::record(&conn, cost, None, now_ms());
                    if let Ok(json) = serde_json::to_string(&segments) {
                        let _ = repo_chunks::mark_done(&conn, c.id, &json);
                        done += 1;
                    }
                }
                Err(StepError::Unauthorized(_) | StepError::WaitingQuota(_)) => {
                    tracing::info!("transkripsi bertahap {id} dihentikan (key/kuota); dilanjutkan setelah Stop");
                    *self.live_off.lock().unwrap_or_else(|e| e.into_inner()) = Some(id.clone());
                    break;
                }
                Err(_) => break,
            }
        }
        drop(results);
        if done > 0 {
            let conn = self.db.conn();
            let processed = ["mic", "system"]
                .iter()
                .filter_map(|ch| crate::db::repo_live::get(&conn, &id, ch).ok().flatten())
                .max()
                .unwrap_or(0);
            drop(conn);
            tracing::info!("transkripsi bertahap {id}: {done} chunk, audio s.d. {} dtk", processed / 16_000);
            self.events.emit_json(events::EV_RECORDING_LIVE, serde_json::json!({ "transcribedSec": processed / 16_000 }));
        }
    }

    /// Bahasa + prompt STT: judul meeting (jika bermakna), glosarium pengguna, lalu nama PJ yang sering
    /// muncul 30 hari terakhir. Ditulis sebagai kalimat Indonesia rapi karena Whisper meniru gaya prompt
    /// (ejaan, kapital, tanda baca) — langkah 45, feedback3 G2.
    fn stt_opts(&self, m: &MeetingRow) -> SttOpts {
        let language = (m.language == "id").then(|| "id".to_string());
        let conn = self.db.conn();
        let glossary = settings::load(&conn).map(|s| s.stt_glossary).unwrap_or_default();
        let mut terms = settings::glossary_terms(&glossary);
        let since = now_ms() - 30 * 24 * 3600 * 1000;
        for name in repo_meetings::recent_assignees(&conn, since, 20).unwrap_or_default() {
            if !terms.iter().any(|t| t.eq_ignore_ascii_case(&name)) {
                terms.push(name);
            }
        }
        drop(conn);
        let mut prompt = String::new();
        if m.title_edited || !is_default_title(&m.title) {
            prompt.push_str(m.title.trim().trim_end_matches('.'));
            prompt.push_str(". ");
        }
        // Daftar istilah dipotong per istilah (bukan di tengah kata) agar muat batas prompt.
        let mut list = String::new();
        let budget = settings::GLOSSARY_MAX_CHARS.saturating_sub(prompt.chars().count() + 24);
        for t in &terms {
            let add = t.chars().count() + if list.is_empty() { 0 } else { 2 };
            if list.chars().count() + add > budget {
                break;
            }
            if !list.is_empty() {
                list.push_str(", ");
            }
            list.push_str(t);
        }
        if !list.is_empty() {
            prompt.push_str(&format!("Nama dan istilah yang dibahas: {list}."));
        }
        let prompt = prompt.trim().to_string();
        SttOpts { language, prompt: (!prompt.is_empty()).then_some(prompt) }
    }

    async fn transcribe_chunk<'a>(
        &'a self,
        stt: &'a OpenAiStt,
        opts: &'a SttOpts,
        c: &'a repo_chunks::ChunkRow,
    ) -> (&'a repo_chunks::ChunkRow, StepResult<Vec<SttSegment>>) {
        let path = self.data_dir.join(&c.path);
        let map: Vec<preprocess::OffsetEntry> = serde_json::from_str(&c.offset_map_json).unwrap_or_default();
        let result = self.transcribe_file(stt, opts, &path, c.duration_ms, &map, 0).await;
        (c, result)
    }

    /// Transkrip satu file chunk. Ditolak 413 → pecah dua di jeda antar-region (offset map) dan
    /// transkrip tiap bagian; waktu segmen bagian kedua digeser sehingga tetap waktu file asli.
    fn transcribe_file<'a>(
        &'a self,
        stt: &'a OpenAiStt,
        opts: &'a SttOpts,
        path: &'a Path,
        duration_ms: i64,
        map: &'a [preprocess::OffsetEntry],
        depth: u32,
    ) -> Pin<Box<dyn Future<Output = StepResult<Vec<SttSegment>>> + Send + 'a>> {
        Box::pin(async move {
            let cost = Cost::Stt { audio_sec: duration_ms as f64 / 1000.0 };
            let res = self
                .with_retry(cost, || {
                    stt.transcribe(SttRequest {
                        wav_path: path.to_path_buf(),
                        language: opts.language.clone(),
                        prompt: opts.prompt.clone(),
                    })
                })
                .await;
            if !matches!(res, Err(StepError::TooLarge)) || depth >= MAX_SPLIT_DEPTH {
                return res;
            }
            let cut_ms = preprocess::chunker::split_point_ms(map, duration_ms);
            let (a, b) = (path.with_extension(format!("{depth}a.wav")), path.with_extension(format!("{depth}b.wav")));
            tracing::info!("chunk terlalu besar (413): dipecah di {} dtk", cut_ms / 1000);
            {
                let (src, a, b) = (path.to_path_buf(), a.clone(), b.clone());
                tokio::task::spawn_blocking(move || preprocess::chunker::split_wav(&src, cut_ms, &a, &b))
                    .await
                    .map_err(|e| StepError::Failed(AppError::internal(e)))??;
            }
            // Map untuk tiap bagian dalam waktu file bagian itu (hanya dipakai memilih titik potong berikutnya).
            let map_a: Vec<_> = map.iter().copied().filter(|e| e.file_ms < cut_ms).collect();
            let map_b: Vec<_> = map
                .iter()
                .filter(|e| e.file_ms >= cut_ms)
                .map(|e| preprocess::OffsetEntry { file_ms: e.file_ms - cut_ms, ..*e })
                .collect();
            let parts = async {
                let first = self.transcribe_file(stt, opts, &a, cut_ms, &map_a, depth + 1).await?;
                let second = self.transcribe_file(stt, opts, &b, duration_ms - cut_ms, &map_b, depth + 1).await?;
                Ok::<_, StepError>((first, second))
            }
            .await;
            let _ = std::fs::remove_file(&a);
            let _ = std::fs::remove_file(&b);
            let (mut out, second) = parts?;
            let shift = cut_ms as f64 / 1000.0;
            out.extend(second.into_iter().map(|mut s| {
                s.start_s += shift;
                s.end_s += shift;
                s
            }));
            Ok(out)
        })
    }

    async fn step_merge(&self, id: &str) -> StepResult<()> {
        self.enter_step(id, MeetingStatus::Merging, 1)?;
        let cfg = &self.providers.pipeline;
        let chunks = repo_chunks::list(&self.db.conn(), id)?;
        let mut segments = merge::build_segments(&chunks, cfg);
        let dups = dedup::mark_duplicates(&mut segments, cfg.dedup_similarity);
        let filtered = segments.iter().filter(|s| s.is_filtered).count();
        repo_segments::replace_all(&mut self.db.conn(), id, &segments)?;
        tracing::info!("meeting {id}: {} segment, {filtered} difilter, {dups} duplikat", segments.len());

        // Chunk unggah tidak dibutuhkan lagi setelah transkrip tersusun (transkrip ulang membuatnya kembali).
        self.remove_dir(self.data_dir.join("recordings").join(id).join("upload")).await;

        // Retensi (PRD §8.7): hapus audio setelah merging sukses (opsi "setelah transkrip").
        if settings::load(&self.db.conn())?.audio_retention == AudioRetention::AfterTranscript {
            self.delete_audio(id).await?;
        }
        self.set_progress(id, MeetingStatus::Merging, 1, 1);
        Ok(())
    }

    async fn step_summarize(&self, m: &MeetingRow) -> StepResult<()> {
        let id = &m.id;
        self.enter_step(id, MeetingStatus::Summarizing, 1)?;
        let (llm_endpoint, llm_key) = self.endpoint(Role::Llm)?;
        let (input, model) = {
            let conn = self.db.conn();
            let label_saya = settings::load(&conn)?.user_display_name;
            let (instruksi, bahasa_meeting) = repo_meetings::summary_prefs(&conn, id)?;
            let bahasa = bahasa_meeting.unwrap_or_else(|| settings::load(&conn).map(|s| s.notes_language).unwrap_or_default());
            // LLM selalu melihat label default; nama hasil kenali (1:1) disimpan setelahnya.
            let label_peserta = DEFAULT_SYSTEM_LABEL.to_string();
            let segments = repo_segments::list_visible(&conn, id)?;
            let word_count = segments.iter().map(|s| s.text.split_whitespace().count()).sum();
            let lines = segments
                .iter()
                .map(|s| {
                    let label = if s.channel == "mic" { &label_saya } else { &label_peserta };
                    format!("[{}] {label}: {}", hhmmss(s.start_ms), s.text)
                })
                .collect();
            let tanggal = Local
                .timestamp_millis_opt(m.started_at)
                .single()
                .map_or_else(String::new, |t| t.format("%Y-%m-%d").to_string());
            (
                SummarizeInput {
                    lines,
                    word_count,
                    label_saya,
                    label_peserta,
                    tanggal,
                    ejaan: settings::glossary_terms(&settings::load(&conn)?.stt_glossary),
                    momen: crate::db::repo_bookmarks::list(&conn, id)?.into_iter().map(hhmmss).collect(),
                    catatan: crate::db::repo_notes::get(&conn, id)?,
                    bahasa: bahasa.clone(),
                    instruksi: instruksi.clone().unwrap_or_default(),
                },
                llm_endpoint.model.clone(),
            )
        };

        let outcome = if input.word_count < 20 {
            SummaryOutcome::Empty
        } else {
            // `llm_extra_body` (mis. reasoning_effort) khusus model Groq di providers.json.
            let extra = if llm_endpoint.is_groq() { self.providers.llm_extra_body.clone() } else { Default::default() };
            let llm = OpenAiLlm::new(self.http.clone(), llm_endpoint.base_url.clone(), llm_key, model.clone(), extra);
            let caller = Caller { worker: self, llm, meeting_id: id };
            summarize::summarize(&caller, &input, self.providers.pipeline.llm_chunk_max_tokens as usize).await?
        };
        let mut conn = self.db.conn();
        match outcome {
            SummaryOutcome::Empty => repo_summary::save(&mut conn, id, None, &model, m.duration_ms)?,
            SummaryOutcome::Notes(notes) => {
                repo_summary::save(&mut conn, id, Some(&notes), &model, m.duration_ms)?;
                repo_meetings::set_generated_title(&conn, id, &notes.judul)?;
            }
        }
        Ok(())
    }
}

/// Judul bawaan "Meeting 8 Okt 2026 09.00" / "Meeting" (belum bermakna untuk prompt STT).
fn is_default_title(title: &str) -> bool {
    let t = title.trim();
    t == "Meeting" || (t.starts_with("Meeting ") && t.ends_with(|c: char| c.is_ascii_digit()))
}

fn hhmmss(ms: i64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// `LlmCaller` untuk summarize: rate limiter + retry + pencatatan token.
struct Caller<'a> {
    worker: &'a Worker,
    llm: OpenAiLlm,
    meeting_id: &'a str,
}

#[async_trait]
impl LlmCaller for Caller<'_> {
    async fn call(&self, messages: Vec<ChatMessage>, max_tokens: u32) -> StepResult<String> {
        let input_text: String = messages.iter().map(|m| m.content.as_str()).collect();
        let est = summarize::estimate_tokens(&input_text) as i64 + i64::from(max_tokens);
        let cost = Cost::Llm { tokens: est };
        let req = LlmRequest { messages, max_tokens, temperature: 0.2 };
        let resp = self.worker.with_retry(cost, || self.llm.complete(req.clone())).await?;
        let used = i64::from(resp.prompt_tokens + resp.completion_tokens);
        rate_limiter::record(&self.worker.db.conn(), cost, (used > 0).then_some(used), now_ms())?;
        Ok(resp.content)
    }

    fn progress(&self, done: usize, total: usize) {
        self.worker.set_progress(self.meeting_id, MeetingStatus::Summarizing, done as i64, total as i64);
    }
}
