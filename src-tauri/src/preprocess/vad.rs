//! VAD (PRD §8.1 langkah 2–3): webrtc-vad mode Aggressive, frame 30 ms (480 sampel),
//! region = frame bersuara + padding 450 ms kiri-kanan, gabung jika jarak < 1000 ms.

use webrtc_vad::{SampleRate, Vad, VadMode};

use super::reader::PartReader;

pub const FRAME: usize = 480;
/// Langkah 45 (G7): 300 → 450 ms agar awal kata yang diucapkan pelan tidak terpotong.
const PAD_SAMPLES: u64 = 450 * 16;
const MERGE_GAP_SAMPLES: u64 = 1000 * 16;

/// Rentang sampel [start, end).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub start: u64,
    pub end: u64,
}

impl Region {
    pub fn len(&self) -> u64 {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
}

pub struct VadResult {
    pub regions: Vec<Region>,
    /// Energi (jumlah kuadrat) per frame 30 ms, untuk memotong region panjang di titik paling sepi.
    pub frame_energy: Vec<f32>,
    pub total_samples: u64,
}

pub fn detect(reader: &PartReader) -> std::io::Result<VadResult> {
    let mut vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Aggressive);
    let mut voiced: Vec<bool> = Vec::new();
    let mut frame_energy: Vec<f32> = Vec::new();
    reader.for_each_frame(FRAME, |frame| {
        let energy: f64 = frame.iter().map(|&s| (s as f64) * (s as f64)).sum();
        frame_energy.push(energy as f32);
        // Frame nol penuh (hening/mute/padding) tidak perlu masuk VAD.
        let is_voice = energy > 0.0 && vad.is_voice_segment(frame).unwrap_or(false);
        voiced.push(is_voice);
    })?;
    let total = reader.total_samples();

    // Frame bersuara berurutan → region mentah.
    let mut raw: Vec<Region> = Vec::new();
    let mut run_start: Option<usize> = None;
    for (i, &v) in voiced.iter().chain(std::iter::once(&false)).enumerate() {
        match (v, run_start) {
            (true, None) => run_start = Some(i),
            (false, Some(s)) => {
                raw.push(Region { start: (s * FRAME) as u64, end: ((i * FRAME) as u64).min(total) });
                run_start = None;
            }
            _ => {}
        }
    }

    // Padding lalu gabung region yang jaraknya < 1000 ms.
    let mut regions: Vec<Region> = Vec::new();
    for r in raw {
        let padded = Region { start: r.start.saturating_sub(PAD_SAMPLES), end: (r.end + PAD_SAMPLES).min(total) };
        match regions.last_mut() {
            Some(last) if padded.start < last.end + MERGE_GAP_SAMPLES => last.end = last.end.max(padded.end),
            _ => regions.push(padded),
        }
    }
    regions.retain(|r| !r.is_empty());
    Ok(VadResult { regions, frame_energy, total_samples: total })
}
