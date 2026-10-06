//! Upload chunk (PRD §8.1 langkah 4–6): akumulasi region sampai ≈ target (default 300 dtk), maks 600 dtk,
//! potong hanya di antara region; region > 600 dtk dipotong di frame energi terendah pada jendela 280–320 dtk;
//! sisa < 10 dtk digabung ke chunk sebelumnya. Region disambung dengan hening 300 ms.

use std::path::Path;

use super::reader::PartReader;
use super::vad::{Region, VadResult, FRAME};
use super::OffsetEntry;
use crate::audio::writer::WAV_SPEC;
use crate::audio::SAMPLE_RATE;

const SR: u64 = SAMPLE_RATE as u64;
const MAX_CHUNK_SAMPLES: u64 = 600 * SR;
const MIN_LAST_CHUNK_SAMPLES: u64 = 10 * SR;
const SPLIT_WINDOW: (u64, u64) = (280 * SR, 320 * SR);
const GAP_SAMPLES: u64 = 300 * 16;

/// Rencana satu chunk: potongan audio asli berurutan.
#[derive(Debug, Clone, Default)]
pub struct ChunkPlan {
    pub pieces: Vec<Region>,
}

impl ChunkPlan {
    /// Panjang file upload (potongan + hening 300 ms di antaranya).
    pub fn file_samples(&self) -> u64 {
        let audio: u64 = self.pieces.iter().map(Region::len).sum();
        audio + GAP_SAMPLES * self.pieces.len().saturating_sub(1) as u64
    }

    fn file_samples_with(&self, extra: &Region) -> u64 {
        if self.pieces.is_empty() {
            extra.len()
        } else {
            self.file_samples() + GAP_SAMPLES + extra.len()
        }
    }
}

/// Potong region > 600 dtk di frame energi terendah pada jendela 280–320 dtk dari awal region.
fn split_long(region: Region, energy: &[f32]) -> Vec<Region> {
    let mut out = Vec::new();
    let mut cur = region;
    while cur.len() > MAX_CHUNK_SAMPLES {
        let lo_frame = ((cur.start + SPLIT_WINDOW.0) / FRAME as u64) as usize;
        let hi_frame = (((cur.start + SPLIT_WINDOW.1) / FRAME as u64) as usize).min(energy.len().saturating_sub(1));
        let cut_frame = (lo_frame..=hi_frame.max(lo_frame))
            .min_by(|&a, &b| {
                let ea = energy.get(a).copied().unwrap_or(f32::MAX);
                let eb = energy.get(b).copied().unwrap_or(f32::MAX);
                ea.total_cmp(&eb)
            })
            .unwrap_or(lo_frame);
        let cut = ((cut_frame * FRAME) as u64).clamp(cur.start + 1, cur.end - 1);
        out.push(Region { start: cur.start, end: cut });
        cur = Region { start: cut, end: cur.end };
    }
    out.push(cur);
    out
}

pub fn plan_chunks(vad: &VadResult, target_sec: u32) -> Vec<ChunkPlan> {
    let target = u64::from(target_sec) * SR;
    let pieces: Vec<Region> = vad.regions.iter().flat_map(|&r| split_long(r, &vad.frame_energy)).collect();

    let mut chunks: Vec<ChunkPlan> = Vec::new();
    let mut cur = ChunkPlan::default();
    for p in pieces {
        if !cur.pieces.is_empty() && cur.file_samples_with(&p) > MAX_CHUNK_SAMPLES {
            chunks.push(std::mem::take(&mut cur));
        }
        cur.pieces.push(p);
        if cur.file_samples() >= target {
            chunks.push(std::mem::take(&mut cur));
        }
    }
    if !cur.pieces.is_empty() {
        // Sisa < 10 dtk digabung ke chunk sebelumnya (Groq menagih minimal 10 dtk per request).
        match chunks.last_mut() {
            Some(prev) if cur.file_samples() < MIN_LAST_CHUNK_SAMPLES => prev.pieces.extend(cur.pieces),
            _ => chunks.push(cur),
        }
    }
    chunks
}

fn to_ms(samples: u64) -> i64 {
    (samples * 1000 / SR) as i64
}

/// Tulis WAV chunk dan kembalikan (durasi ms, offset map).
pub fn write_chunk(reader: &PartReader, plan: &ChunkPlan, path: &Path) -> std::io::Result<(i64, Vec<OffsetEntry>)> {
    let mut w = hound::WavWriter::create(path, WAV_SPEC).map_err(std::io::Error::other)?;
    let mut map = Vec::with_capacity(plan.pieces.len());
    let mut file_pos = 0u64;
    for (i, piece) in plan.pieces.iter().enumerate() {
        if i > 0 {
            for _ in 0..GAP_SAMPLES {
                w.write_sample(0i16).map_err(std::io::Error::other)?;
            }
            file_pos += GAP_SAMPLES;
        }
        map.push(OffsetEntry { file_ms: to_ms(file_pos), orig_ms: to_ms(piece.start), dur_ms: to_ms(piece.len()) });
        // Baca per 10 dtk agar memori kecil.
        let mut pos = piece.start;
        while pos < piece.end {
            let n = (piece.end - pos).min(10 * SR);
            for s in reader.read_range(pos, n)? {
                w.write_sample(s).map_err(std::io::Error::other)?;
            }
            pos += n;
        }
        file_pos += piece.len();
    }
    w.finalize().map_err(std::io::Error::other)?;
    Ok((to_ms(file_pos), map))
}
