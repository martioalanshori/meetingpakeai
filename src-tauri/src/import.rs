//! Impor rekaman yang sudah ada (langkah 47, feedback3 A1): mp3/m4a/mp4/wav/ogg/flac/webm/mkv →
//! 16 kHz mono PCM16 → part channel `mic` (format sama dengan rekaman langsung) → masuk antrean biasa.
//! Decode memakai `symphonia` (Rust murni, tanpa ffmpeg).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::audio::writer::{PartEvent, PartWriter};
use crate::audio::{Channel, SAMPLE_RATE};
use crate::error::{AppError, AppResult, ErrorCode};

/// Ekstensi yang ditawarkan di dialog pilih file.
pub const EXTENSIONS: [&str; 11] = ["mp3", "m4a", "mp4", "aac", "wav", "ogg", "oga", "flac", "webm", "mkv", "mov"];

/// Hasil decode: total sampel 16 kHz + kejadian part (untuk dicatat ke `recording_parts`).
pub struct Decoded {
    pub samples: u64,
    pub parts: Vec<PartEvent>,
}

fn bad(msg: impl Into<String>) -> AppError {
    AppError::with_message(ErrorCode::InvalidState, msg.into())
}

/// Resampler streaming ke 16 kHz: low-pass FIR (windowed-sinc Blackman, cutoff 7,2 kHz) di laju asal
/// lalu interpolasi linear. Cukup untuk ucapan; memori tetap kecil untuk file berjam-jam.
struct Resampler {
    step: f64,
    taps: Vec<f32>,
    hist: Vec<f32>,
    hist_pos: usize,
    filtered: Vec<f32>,
    /// Posisi output berikutnya, dalam indeks sampel `filtered` (relatif ke `filtered[0]`).
    pos: f64,
}

impl Resampler {
    fn new(src_rate: u32) -> Self {
        let step = f64::from(src_rate) / f64::from(SAMPLE_RATE);
        // Tanpa filter jika laju asal ≤ 16 kHz (hanya interpolasi naik).
        let taps = if src_rate > SAMPLE_RATE {
            let n = 63usize;
            let fc = 7_200.0 / f64::from(src_rate);
            let m = (n - 1) as f64;
            let mut t: Vec<f32> = (0..n)
                .map(|i| {
                    let x = i as f64 - m / 2.0;
                    let sinc = if x == 0.0 { 2.0 * fc } else { (2.0 * std::f64::consts::PI * fc * x).sin() / (std::f64::consts::PI * x) };
                    let w = 0.42 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / m).cos()
                        + 0.08 * (4.0 * std::f64::consts::PI * i as f64 / m).cos();
                    (sinc * w) as f32
                })
                .collect();
            let sum: f32 = t.iter().sum();
            t.iter_mut().for_each(|v| *v /= sum);
            t
        } else {
            vec![1.0]
        };
        let len = taps.len();
        Self { step, taps, hist: vec![0.0; len], hist_pos: 0, filtered: Vec::new(), pos: 0.0 }
    }

    fn push(&mut self, input: &[f32], out: &mut Vec<i16>) {
        let n = self.taps.len();
        for &x in input {
            self.hist[self.hist_pos] = x;
            self.hist_pos = (self.hist_pos + 1) % n;
            let mut acc = 0.0f32;
            for (k, &tap) in self.taps.iter().enumerate() {
                acc += tap * self.hist[(self.hist_pos + k) % n];
            }
            self.filtered.push(acc);
        }
        while self.pos + 1.0 < self.filtered.len() as f64 {
            let i = self.pos.floor() as usize;
            let frac = (self.pos - i as f64) as f32;
            let v = self.filtered[i] * (1.0 - frac) + self.filtered[i + 1] * frac;
            out.push((v * 32767.0).clamp(-32768.0, 32767.0) as i16);
            self.pos += self.step;
        }
        // Buang sampel yang sudah terlewati (sisakan satu untuk interpolasi).
        let keep_from = (self.pos.floor() as usize).min(self.filtered.len().saturating_sub(1));
        if keep_from > 0 {
            self.filtered.drain(..keep_from);
            self.pos -= keep_from as f64;
        }
    }
}

/// Decode `src` dan tulis part WAV channel `mic` ke `dir`.
pub fn decode_to_parts(src: &Path, dir: &Path) -> AppResult<Decoded> {
    let file = std::fs::File::open(src).map_err(|e| bad(format!("File tidak bisa dibuka: {e}")))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = src.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|_| bad("Format file tidak didukung. Gunakan mp3, m4a, mp4, wav, ogg, flac, atau webm."))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| bad("File tidak berisi audio."))?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .cloned()
        .ok_or_else(|| bad("Codec audio di file ini tidak didukung."))?;
    let src_rate = params.sample_rate.ok_or_else(|| bad("Laju sampel audio tidak diketahui."))?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|_| bad("Codec audio di file ini tidak didukung (mis. Opus belum didukung)."))?;

    let events: Arc<Mutex<Vec<PartEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let mut writer = PartWriter::new(
        dir,
        Channel::Mic,
        Box::new(move |ev| sink.lock().unwrap_or_else(|e| e.into_inner()).push(ev)),
    )
    .map_err(AppError::from)?;
    let mut rs = Resampler::new(src_rate);
    let mut planes: Vec<Vec<f32>> = Vec::new();
    let mut mono: Vec<f32> = Vec::new();
    let mut out: Vec<i16> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            // Aliran OGG berantai / berkas terpotong di akhir: anggap selesai.
            Err(SymError::ResetRequired) | Err(SymError::IoError(_)) => break,
            Err(e) => return Err(bad(format!("File rusak: {e}"))),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymError::DecodeError(_)) | Err(SymError::IoError(_)) => continue,
            Err(e) => return Err(bad(format!("Audio tidak bisa dibaca: {e}"))),
        };
        decoded.copy_to_vecs_planar::<f32>(&mut planes);
        let ch = planes.len().max(1);
        let frames = planes.first().map_or(0, Vec::len);
        mono.clear();
        mono.extend((0..frames).map(|i| planes.iter().map(|p| p[i]).sum::<f32>() / ch as f32));
        out.clear();
        rs.push(&mono, &mut out);
        writer.write(&out).map_err(AppError::from)?;
    }
    let samples = writer.finish().map_err(AppError::from)?;
    let parts = std::mem::take(&mut *events.lock().unwrap_or_else(|e| e.into_inner()));
    Ok(Decoded { samples, parts })
}

/// Folder rekaman meeting impor (relatif ke data dir) — sama dengan rekaman langsung.
pub fn rel_dir(meeting_id: &str) -> PathBuf {
    PathBuf::from("recordings").join(meeting_id)
}
