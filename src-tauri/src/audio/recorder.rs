//! Satu sesi rekaman: dua channel (mic + system), jam bersama, pause/mute/level (PRD §7.2–§7.4).
//! Tidak bergantung pada Tauri; pemanggil menerima `PartEvent` lewat callback.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use super::capture::{self, CaptureHandle, CaptureSink};
use super::level::{LevelMeter, SharedLevel};
use super::timeline::{Aligner, Clock};
use super::writer::{PartEvent, PartWriter};
use super::{AudioError, Channel, SAMPLE_RATE};

pub type PartCallback = Arc<dyn Fn(PartEvent) + Send + Sync>;

const ZEROS: [i16; 1600] = [0; 1600];

/// State yang diubah thread capture dan dibaca saat Stop.
struct ChannelState {
    aligner: Aligner,
    writer: Option<PartWriter>,
    meter: LevelMeter,
}

impl ChannelState {
    fn write(&mut self, samples: &[i16]) {
        if let Some(w) = self.writer.as_mut() {
            if let Err(e) = w.write(samples) {
                tracing::error!("gagal menulis part: {e}");
            }
        }
    }

    fn write_zeros(&mut self, mut n: u64) {
        while n > 0 {
            let take = (ZEROS.len() as u64).min(n) as usize;
            self.write(&ZEROS[..take]);
            n -= take as u64;
        }
    }
}

/// Bagian yang dibagi antara `Recorder` dan thread capture satu channel.
struct ChannelShared {
    channel: Channel,
    state: Mutex<ChannelState>,
    alive: AtomicBool,
    level: SharedLevel,
}

impl ChannelShared {
    fn lock(&self) -> MutexGuard<'_, ChannelState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Kontrol bersama kedua channel.
struct Controls {
    clock: Clock,
    mic_muted: AtomicBool,
    /// Batas sampel saat Stop (u64::MAX selama merekam) agar kedua channel tidak melebihi panjang final.
    limit: AtomicU64,
}

struct Sink {
    shared: Arc<ChannelShared>,
    controls: Arc<Controls>,
}

impl Sink {
    fn publish(&self) -> impl FnMut(f32) + '_ {
        |db| self.shared.level.set(db)
    }
}

impl CaptureSink for Sink {
    fn on_samples(&mut self, packet: &[i16]) {
        // Pause: paket yang datang dibuang (PRD §7.4).
        if self.controls.clock.is_paused() {
            return;
        }
        let expected = self.controls.clock.expected_samples_at(Instant::now());
        let limit = self.controls.limit.load(Ordering::SeqCst);
        let mut st = self.shared.lock();
        let before = st.aligner.written();
        let aligned = st.aligner.align(packet, expected);
        let mut zeros = aligned.zeros_before;
        let mut samples = aligned.samples;
        // Jangan melewati batas final saat Stop.
        let allowed = limit.saturating_sub(before);
        if zeros + samples.len() as u64 > allowed {
            zeros = zeros.min(allowed);
            samples = &samples[..(allowed - zeros) as usize];
            st.aligner.set_written(before + zeros + samples.len() as u64);
        }
        let muted = self.shared.channel == Channel::Mic && self.controls.mic_muted.load(Ordering::Relaxed);
        st.write_zeros(zeros);
        st.meter.push_zeros(zeros, self.publish());
        if muted {
            // Mute: timeline tetap jalan, isinya hening.
            st.write_zeros(samples.len() as u64);
            st.meter.push_zeros(samples.len() as u64, self.publish());
        } else {
            st.write(samples);
            st.meter.push(samples, self.publish());
        }
    }

    fn on_tick(&mut self) {
        if self.controls.clock.is_paused() {
            return;
        }
        let expected = self.controls.clock.expected_samples_at(Instant::now());
        let limit = self.controls.limit.load(Ordering::SeqCst);
        let mut st = self.shared.lock();
        // Loopback tidak mengirim paket saat hening: isi celah agar file di disk tetap mengikuti timeline.
        let zeros = st.aligner.fill_gap(expected.min(limit));
        if zeros > 0 {
            st.write_zeros(zeros);
            st.meter.push_zeros(zeros, self.publish());
        }
    }

    fn on_error(&mut self, err: AudioError) {
        tracing::warn!("channel {} mati: {err}", self.shared.channel.as_str());
        self.shared.alive.store(false, Ordering::SeqCst);
        self.shared.level.set(super::level::SILENCE_DBFS);
    }
}

pub struct StopResult {
    pub duration_ms: i64,
    pub mic_samples: u64,
    pub system_samples: u64,
}

pub struct Recorder {
    dir: PathBuf,
    controls: Arc<Controls>,
    mic: Arc<ChannelShared>,
    system: Arc<ChannelShared>,
    /// Handle capture per channel [mic, system]; diganti saat device dibuka ulang.
    handles: Mutex<[Option<CaptureHandle>; 2]>,
}

impl Recorder {
    /// Mulai merekam kedua channel ke `dir`. Gagal membuka salah satu device → semua dihentikan.
    pub fn start(dir: &Path, on_part: PartCallback) -> Result<Self, AudioError> {
        let controls = Arc::new(Controls {
            clock: Clock::start(),
            mic_muted: AtomicBool::new(false),
            limit: AtomicU64::new(u64::MAX),
        });
        let make = |channel: Channel| -> Result<Arc<ChannelShared>, AudioError> {
            let cb = on_part.clone();
            let writer = PartWriter::new(dir, channel, Box::new(move |ev| cb(ev)))?;
            Ok(Arc::new(ChannelShared {
                channel,
                state: Mutex::new(ChannelState { aligner: Aligner::default(), writer: Some(writer), meter: LevelMeter::default() }),
                alive: AtomicBool::new(true),
                level: SharedLevel::default(),
            }))
        };
        let mic = make(Channel::Mic)?;
        let system = make(Channel::System)?;

        let mic_handle = capture::spawn(Channel::Mic, Sink { shared: mic.clone(), controls: controls.clone() })?;
        // Jika system gagal, `mic_handle` di-drop → thread mic berhenti.
        let system_handle = capture::spawn(Channel::System, Sink { shared: system.clone(), controls: controls.clone() })?;

        Ok(Self {
            dir: dir.to_path_buf(),
            controls,
            mic,
            system,
            handles: Mutex::new([Some(mic_handle), Some(system_handle)]),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn elapsed(&self) -> Duration {
        self.controls.clock.elapsed()
    }

    pub fn is_paused(&self) -> bool {
        self.controls.clock.is_paused()
    }

    pub fn pause(&self) {
        self.controls.clock.pause();
        self.mic.level.set(super::level::SILENCE_DBFS);
        self.system.level.set(super::level::SILENCE_DBFS);
    }

    pub fn resume(&self) {
        self.controls.clock.resume();
    }

    pub fn mic_muted(&self) -> bool {
        self.controls.mic_muted.load(Ordering::Relaxed)
    }

    pub fn set_mic_muted(&self, muted: bool) {
        self.controls.mic_muted.store(muted, Ordering::Relaxed);
    }

    pub fn alive(&self, channel: Channel) -> bool {
        self.shared(channel).alive.load(Ordering::SeqCst)
    }

    /// dBFS terakhir per channel (mic, system).
    pub fn levels(&self) -> (f32, f32) {
        (self.mic.level.get(), self.system.level.get())
    }

    /// Buka ulang default device untuk channel (PRD §7.5): channel mati, atau default device diganti.
    /// Celah terisi nol oleh timeline. Gagal → channel dianggap mati (monitor mencoba lagi).
    pub fn reopen(&self, channel: Channel) -> Result<(), AudioError> {
        let shared = self.shared(channel).clone();
        let mut handles = self.handles.lock().unwrap_or_else(|e| e.into_inner());
        let slot = &mut handles[channel_index(channel)];
        // Hentikan thread lama (jika masih jalan) dan join agar resource WASAPI lepas.
        if let Some(old) = slot.take() {
            shared.alive.store(false, Ordering::SeqCst);
            old.stop();
        }
        let handle = capture::spawn(channel, Sink { shared: shared.clone(), controls: self.controls.clone() })?;
        *slot = Some(handle);
        shared.alive.store(true, Ordering::SeqCst);
        tracing::info!("channel {} dibuka ulang", channel.as_str());
        Ok(())
    }

    fn shared(&self, channel: Channel) -> &Arc<ChannelShared> {
        match channel {
            Channel::Mic => &self.mic,
            Channel::System => &self.system,
        }
    }

    /// Stop: hentikan capture, pad kedua channel sampai panjang final yang sama, tutup part.
    pub fn stop(self) -> Result<StopResult, AudioError> {
        self.controls.clock.pause();
        let final_samples = self.controls.clock.expected_samples_at(Instant::now());
        self.controls.limit.store(final_samples, Ordering::SeqCst);
        let handles = std::mem::take(&mut *self.handles.lock().unwrap_or_else(|e| e.into_inner()));
        for h in handles.into_iter().flatten() {
            h.stop();
        }
        let mut totals = [0u64; 2];
        for (i, shared) in [&self.mic, &self.system].into_iter().enumerate() {
            let mut st = shared.lock();
            let zeros = st.aligner.pad_to(final_samples);
            st.write_zeros(zeros);
            if let Some(w) = st.writer.take() {
                totals[i] = w.finish()?;
            }
        }
        Ok(StopResult {
            duration_ms: (final_samples * 1000 / SAMPLE_RATE as u64) as i64,
            mic_samples: totals[0],
            system_samples: totals[1],
        })
    }
}

fn channel_index(channel: Channel) -> usize {
    match channel {
        Channel::Mic => 0,
        Channel::System => 1,
    }
}
