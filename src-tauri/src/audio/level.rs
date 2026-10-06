//! Level meter (PRD §7.4): RMS per 100 ms → dBFS (−90 jika hening).

use std::sync::atomic::{AtomicU32, Ordering};

use super::SAMPLE_RATE;

pub const SILENCE_DBFS: f32 = -90.0;
const WINDOW_SAMPLES: u32 = SAMPLE_RATE / 10;

/// Nilai dBFS terakhir, dibaca thread lain (event 10 Hz, auto-stop).
pub struct SharedLevel(AtomicU32);

impl Default for SharedLevel {
    fn default() -> Self {
        Self(AtomicU32::new(SILENCE_DBFS.to_bits()))
    }
}

impl SharedLevel {
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    pub fn set(&self, dbfs: f32) {
        self.0.store(dbfs.to_bits(), Ordering::Relaxed);
    }
}

/// Akumulator RMS per jendela 100 ms; juga mencatat puncak (untuk tes 5 detik).
#[derive(Default)]
pub struct LevelMeter {
    sum_sq: f64,
    count: u32,
    peak: i16,
}

impl LevelMeter {
    /// Masukkan sampel; panggil `publish` untuk setiap jendela 100 ms yang penuh.
    pub fn push(&mut self, samples: &[i16], mut publish: impl FnMut(f32)) {
        for &s in samples {
            let v = s as f64 / 32768.0;
            self.sum_sq += v * v;
            self.count += 1;
            self.peak = self.peak.max(s.saturating_abs());
            if self.count >= WINDOW_SAMPLES {
                publish(rms_dbfs(self.sum_sq, self.count));
                self.sum_sq = 0.0;
                self.count = 0;
            }
        }
    }

    /// Masukkan `n` sampel nol tanpa alokasi.
    pub fn push_zeros(&mut self, mut n: u64, mut publish: impl FnMut(f32)) {
        while n > 0 {
            let room = (WINDOW_SAMPLES - self.count) as u64;
            let take = room.min(n);
            self.count += take as u32;
            n -= take;
            if self.count >= WINDOW_SAMPLES {
                publish(rms_dbfs(self.sum_sq, self.count));
                self.sum_sq = 0.0;
                self.count = 0;
            }
        }
    }

    pub fn peak_dbfs(&self) -> f32 {
        if self.peak == 0 {
            SILENCE_DBFS
        } else {
            (20.0 * (self.peak as f64 / 32768.0).log10()) as f32
        }
    }
}

fn rms_dbfs(sum_sq: f64, count: u32) -> f32 {
    if count == 0 || sum_sq <= 0.0 {
        return SILENCE_DBFS;
    }
    let rms = (sum_sq / count as f64).sqrt();
    ((20.0 * rms.log10()) as f32).max(SILENCE_DBFS)
}
