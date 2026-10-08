//! Tes rekam 5 detik onboarding (PRD §14.1, AC F2.4): rekam mic + loopback sambil memutar nada tes.
//! Nada: sinus 1 kHz, −12 dBFS, 3 detik (keputusan di CLAUDE.md).

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::capture::{self, CaptureSink};
use super::level::{LevelMeter, SharedLevel, SILENCE_DBFS};
use super::writer::WAV_SPEC;
use super::{AudioError, Channel, SAMPLE_RATE};

pub const TEST_DURATION: Duration = Duration::from_secs(5);
/// Mic dinilai hanya sebelum nada diputar: nada dari speaker yang tertangkap mic tidak boleh meloloskan tes.
const MIC_WINDOW: Duration = Duration::from_millis(2500);
const TONE_SECS: u32 = 3;
const TONE_HZ: f64 = 1000.0;
const TONE_DBFS: f64 = -12.0;
/// Ambang lolos: puncak > −40 dBFS.
pub const PASS_DBFS: f32 = -40.0;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTestResult {
    pub mic_ok: bool,
    pub mic_peak_dbfs: f32,
    pub system_ok: bool,
    pub system_peak_dbfs: f32,
}

/// Tulis file WAV nada tes (16 kHz mono PCM16).
pub fn write_tone(path: &Path) -> std::io::Result<()> {
    let amp = 32767.0 * 10f64.powf(TONE_DBFS / 20.0);
    let mut w = hound::WavWriter::create(path, WAV_SPEC).map_err(std::io::Error::other)?;
    for i in 0..(SAMPLE_RATE * TONE_SECS) {
        let t = i as f64 / SAMPLE_RATE as f64;
        // Fade 20 ms di awal/akhir agar tidak "klik".
        let edge = (t / 0.02).min((TONE_SECS as f64 - t) / 0.02).min(1.0);
        let s = (amp * edge * (2.0 * std::f64::consts::PI * TONE_HZ * t).sin()) as i16;
        w.write_sample(s).map_err(std::io::Error::other)?;
    }
    w.finalize().map_err(std::io::Error::other)
}

struct PeakSink {
    meter: Arc<Mutex<LevelMeter>>,
    level: Arc<SharedLevel>,
}

impl CaptureSink for PeakSink {
    fn on_samples(&mut self, samples: &[i16]) {
        let level = &self.level;
        self.meter.lock().unwrap_or_else(|e| e.into_inner()).push(samples, |db| level.set(db));
    }
    fn on_error(&mut self, err: AudioError) {
        tracing::warn!("tes audio: capture berhenti: {err}");
    }
}

/// Jalankan tes; `on_level(mic_db, system_db)` dipanggil ±10 kali per detik.
/// 0–2,5 dtk: pengguna bicara, puncak mic diambil. Lalu `play_tone` dipanggil sekali untuk tes loopback.
pub fn run(play_tone: impl FnOnce(), mut on_level: impl FnMut(f32, f32)) -> Result<AudioTestResult, AudioError> {
    let make = || (Arc::new(Mutex::new(LevelMeter::default())), Arc::new(SharedLevel::default()));
    let (mic_meter, mic_level) = make();
    let (sys_meter, sys_level) = make();
    let mic = capture::spawn(Channel::Mic, PeakSink { meter: mic_meter.clone(), level: mic_level.clone() })?;
    // Sistem gagal dibuka (mis. tidak ada output) → tetap lanjut; hasil system = gagal.
    let sys = capture::spawn(Channel::System, PeakSink { meter: sys_meter.clone(), level: sys_level.clone() });
    if let Err(e) = &sys {
        tracing::warn!("tes audio: loopback tidak bisa dibuka: {e}");
    }
    let peak = |m: &Arc<Mutex<LevelMeter>>| m.lock().unwrap_or_else(|e| e.into_inner()).peak_dbfs();
    let start = Instant::now();
    let mut play_tone = Some(play_tone);
    let mut mic_peak = SILENCE_DBFS;
    while start.elapsed() < TEST_DURATION {
        std::thread::sleep(Duration::from_millis(100));
        on_level(mic_level.get(), sys_level.get());
        if start.elapsed() >= MIC_WINDOW {
            if let Some(play) = play_tone.take() {
                mic_peak = peak(&mic_meter);
                play();
            }
        }
    }
    mic.stop();
    if let Ok(s) = sys {
        s.stop();
    }
    on_level(SILENCE_DBFS, SILENCE_DBFS);
    let sys_peak = peak(&sys_meter);
    Ok(AudioTestResult {
        mic_ok: mic_peak > PASS_DBFS,
        mic_peak_dbfs: mic_peak,
        system_ok: sys_peak > PASS_DBFS,
        system_peak_dbfs: sys_peak,
    })
}
