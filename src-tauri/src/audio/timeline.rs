//! Timeline & sinkronisasi channel (PRD §7.2).
//! Satu `Clock` dibagi kedua channel; tiap channel punya `Aligner` yang menjaga
//! jumlah sampel tertulis tetap ≈ `expected_samples` (sisip nol / buang kelebihan).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::SAMPLE_RATE;

/// Toleransi selisih: 200 ms = 3200 sampel.
pub const TOLERANCE_SAMPLES: u64 = 3200;

struct ClockInner {
    t0: Instant,
    total_pause: Duration,
    paused_since: Option<Instant>,
}

/// Jam timeline: waktu sejak Start, tidak termasuk pause.
pub struct Clock {
    inner: Mutex<ClockInner>,
}

impl Clock {
    pub fn start() -> Self {
        Self { inner: Mutex::new(ClockInner { t0: Instant::now(), total_pause: Duration::ZERO, paused_since: None }) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ClockInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Durasi timeline pada `now` (saat pause: dibekukan di awal pause).
    pub fn elapsed_at(&self, now: Instant) -> Duration {
        let c = self.lock();
        let end = c.paused_since.unwrap_or(now);
        end.saturating_duration_since(c.t0).saturating_sub(c.total_pause)
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed_at(Instant::now())
    }

    pub fn expected_samples_at(&self, now: Instant) -> u64 {
        duration_to_samples(self.elapsed_at(now))
    }

    pub fn is_paused(&self) -> bool {
        self.lock().paused_since.is_some()
    }

    pub fn pause(&self) {
        let mut c = self.lock();
        if c.paused_since.is_none() {
            c.paused_since = Some(Instant::now());
        }
    }

    pub fn resume(&self) {
        let mut c = self.lock();
        if let Some(since) = c.paused_since.take() {
            c.total_pause += since.elapsed();
        }
    }
}

pub fn duration_to_samples(d: Duration) -> u64 {
    (d.as_secs_f64() * SAMPLE_RATE as f64) as u64
}

/// Hasil penyelarasan satu paket.
pub struct Aligned<'a> {
    /// Jumlah sampel nol yang harus ditulis sebelum `samples`.
    pub zeros_before: u64,
    /// Bagian paket yang ditulis (awal paket bisa dibuang).
    pub samples: &'a [i16],
}

/// Penyelaras per channel: melacak sampel yang sudah ditulis.
#[derive(Default)]
pub struct Aligner {
    written: u64,
}

impl Aligner {
    pub fn written(&self) -> u64 {
        self.written
    }

    /// Koreksi jumlah tertulis (dipakai saat paket dipotong oleh batas Stop).
    pub fn set_written(&mut self, written: u64) {
        self.written = written;
    }

    /// Paket diterima (PRD §7.2):
    /// - tertinggal > 200 ms → sisip nol sampai `written = expected − panjang_paket`;
    /// - kelebihan > 200 ms → buang sampel berlebih dari awal paket.
    pub fn align<'a>(&mut self, packet: &'a [i16], expected: u64) -> Aligned<'a> {
        let len = packet.len() as u64;
        let mut zeros_before = 0;
        let mut samples = packet;
        if self.written + TOLERANCE_SAMPLES < expected {
            let target = expected.saturating_sub(len);
            zeros_before = target.saturating_sub(self.written);
        } else if self.written > expected + TOLERANCE_SAMPLES {
            let excess = (self.written - expected).min(len) as usize;
            samples = &packet[excess..];
        }
        self.written += zeros_before + samples.len() as u64;
        Aligned { zeros_before, samples }
    }

    /// Tanpa paket (loopback diam / device mati): jika tertinggal > 200 ms, isi nol sampai `expected`.
    /// Mengembalikan jumlah nol yang harus ditulis.
    pub fn fill_gap(&mut self, expected: u64) -> u64 {
        if self.written + TOLERANCE_SAMPLES < expected {
            let zeros = expected - self.written;
            self.written = expected;
            zeros
        } else {
            0
        }
    }

    /// Saat Stop: pad nol sampai tepat `expected` (agar kedua channel sama panjang).
    pub fn pad_to(&mut self, expected: u64) -> u64 {
        let zeros = expected.saturating_sub(self.written);
        self.written += zeros;
        zeros
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_tepat_waktu_ditulis_apa_adanya() {
        let mut a = Aligner::default();
        let packet = [1i16; 160];
        let r = a.align(&packet, 160);
        assert_eq!(r.zeros_before, 0);
        assert_eq!(r.samples.len(), 160);
        assert_eq!(a.written(), 160);
    }

    #[test]
    fn align_tertinggal_lebih_dari_toleransi_disisipi_nol() {
        let mut a = Aligner::default();
        let packet = [1i16; 160];
        // Diharapkan 10 000 sampel, baru 0 tertulis → nol sampai expected − panjang paket.
        let r = a.align(&packet, 10_000);
        assert_eq!(r.zeros_before, 10_000 - 160);
        assert_eq!(r.samples.len(), 160);
        assert_eq!(a.written(), 10_000);
    }

    #[test]
    fn align_dalam_toleransi_tidak_dikoreksi() {
        let mut a = Aligner::default();
        let packet = [1i16; 160];
        let r = a.align(&packet, TOLERANCE_SAMPLES);
        assert_eq!(r.zeros_before, 0);
        assert_eq!(a.written(), 160);
    }

    #[test]
    fn align_kelebihan_membuang_awal_paket() {
        let mut a = Aligner::default();
        a.set_written(10_000);
        let packet = [1i16; 1_000];
        // Tertulis 10 000, diharapkan 5 000 → kelebihan 5 000 ≥ panjang paket → seluruh paket dibuang.
        let r = a.align(&packet, 5_000);
        assert_eq!(r.samples.len(), 0);
        assert_eq!(a.written(), 10_000);
    }

    #[test]
    fn fill_gap_dan_pad_to() {
        let mut a = Aligner::default();
        assert_eq!(a.fill_gap(TOLERANCE_SAMPLES), 0);
        assert_eq!(a.fill_gap(TOLERANCE_SAMPLES + 1), TOLERANCE_SAMPLES + 1);
        assert_eq!(a.pad_to(TOLERANCE_SAMPLES + 101), 100);
        assert_eq!(a.pad_to(10), 0);
    }
}
