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
/// Normalisasi level (langkah 45, G7): persentil 99,9% amplitudo diangkat ke ±−3 dBFS, maks +16 dB,
/// tidak pernah dikecilkan. Suara pelan (mic jauh, volume Windows rendah) lebih mudah dikenali STT.
const NORM_TARGET: f32 = 0.7 * 32767.0;
const NORM_MAX_GAIN: f32 = 6.0;

/// Penguatan untuk satu potongan: dari histogram |sampel| (tahan terhadap klik/ketukan sesaat).
fn piece_gain(reader: &PartReader, start: u64, end: u64) -> std::io::Result<f32> {
    let mut hist = vec![0u32; 32769];
    let mut total = 0u64;
    let mut pos = start;
    while pos < end {
        let n = (end - pos).min(10 * SR);
        for s in reader.read_range(pos, n)? {
            hist[(s as i32).unsigned_abs() as usize] += 1;
            total += 1;
        }
        pos += n;
    }
    if total == 0 {
        return Ok(1.0);
    }
    let limit = total - total / 1000;
    let mut acc = 0u64;
    let mut p999 = 0usize;
    for (v, &c) in hist.iter().enumerate() {
        acc += u64::from(c);
        if acc >= limit {
            p999 = v;
            break;
        }
    }
    if p999 < 64 {
        // Praktis hening: jangan mengangkat derau.
        return Ok(1.0);
    }
    Ok((NORM_TARGET / p999 as f32).clamp(1.0, NORM_MAX_GAIN))
}

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

/// Titik potong (ms di file upload) untuk memecah chunk yang ditolak 413: tengah jeda antar-potongan
/// yang paling dekat dengan tengah file; satu potongan saja → tepat di tengah.
pub fn split_point_ms(map: &[OffsetEntry], file_ms: i64) -> i64 {
    let half = file_ms / 2;
    let gap_ms = to_ms(GAP_SAMPLES);
    map.iter()
        .skip(1)
        .map(|e| e.file_ms - gap_ms / 2)
        .min_by_key(|cut| (cut - half).abs())
        .filter(|cut| *cut > 0 && *cut < file_ms)
        .unwrap_or(half)
}

/// Pecah WAV `src` di `cut_ms` menjadi `a` (awal) dan `b` (sisa).
pub fn split_wav(src: &Path, cut_ms: i64, a: &Path, b: &Path) -> std::io::Result<()> {
    let mut r = hound::WavReader::open(src).map_err(std::io::Error::other)?;
    let cut = (cut_ms.max(0) as u64 * SR / 1000) as usize;
    let mut wa = hound::WavWriter::create(a, WAV_SPEC).map_err(std::io::Error::other)?;
    let mut wb = hound::WavWriter::create(b, WAV_SPEC).map_err(std::io::Error::other)?;
    for (i, s) in r.samples::<i16>().enumerate() {
        let s = s.map_err(std::io::Error::other)?;
        let w = if i < cut { &mut wa } else { &mut wb };
        w.write_sample(s).map_err(std::io::Error::other)?;
    }
    wa.finalize().map_err(std::io::Error::other)?;
    wb.finalize().map_err(std::io::Error::other)?;
    Ok(())
}

/// Salin rentang `[start_ms, end_ms)` dari WAV `src` ke `dst` (coba ulang bagian ragu, langkah 56).
pub fn extract_wav(src: &Path, start_ms: i64, end_ms: i64, dst: &Path) -> std::io::Result<()> {
    let mut r = hound::WavReader::open(src).map_err(std::io::Error::other)?;
    let from = (start_ms.max(0) as u64 * SR / 1000) as usize;
    let to = (end_ms.max(0) as u64 * SR / 1000) as usize;
    let mut w = hound::WavWriter::create(dst, WAV_SPEC).map_err(std::io::Error::other)?;
    for s in r.samples::<i16>().skip(from).take(to.saturating_sub(from)) {
        w.write_sample(s.map_err(std::io::Error::other)?).map_err(std::io::Error::other)?;
    }
    w.finalize().map_err(std::io::Error::other)?;
    Ok(())
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
        let gain = piece_gain(reader, piece.start, piece.end)?;
        // Baca per 10 dtk agar memori kecil.
        let mut pos = piece.start;
        while pos < piece.end {
            let n = (piece.end - pos).min(10 * SR);
            for s in reader.read_range(pos, n)? {
                let v = if gain > 1.0 { (f32::from(s) * gain).clamp(-32768.0, 32767.0) as i16 } else { s };
                w.write_sample(v).map_err(std::io::Error::other)?;
            }
            pos += n;
        }
        file_pos += piece.len();
    }
    w.finalize().map_err(std::io::Error::other)?;
    Ok((to_ms(file_pos), map))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vad(regions: &[(u64, u64)], total_sec: u64) -> VadResult {
        let total = total_sec * SR;
        VadResult {
            regions: regions.iter().map(|&(s, e)| Region { start: s * SR, end: e * SR }).collect(),
            frame_energy: vec![1.0; (total / FRAME as u64) as usize + 1],
            total_samples: total,
        }
    }

    #[test]
    fn tanpa_region_tanpa_chunk() {
        assert!(plan_chunks(&vad(&[], 60), 300).is_empty());
    }

    #[test]
    fn region_pendek_jadi_satu_chunk() {
        let plans = plan_chunks(&vad(&[(0, 20), (40, 60)], 60), 300);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].pieces.len(), 2);
    }

    #[test]
    fn dipotong_saat_mencapai_target() {
        // Tiap region 100 dtk; target 300 dtk → chunk ditutup setelah region ke-3.
        let regions: Vec<(u64, u64)> = (0..6).map(|i| (i * 120, i * 120 + 100)).collect();
        let plans = plan_chunks(&vad(&regions, 800), 300);
        assert_eq!(plans.len(), 2);
        assert!(plans.iter().all(|p| p.file_samples() <= MAX_CHUNK_SAMPLES));
    }

    #[test]
    fn region_panjang_dipecah_maks_600_dtk() {
        let plans = plan_chunks(&vad(&[(0, 1_500)], 1_500), 300);
        assert!(plans.len() >= 3);
        assert!(plans.iter().all(|p| p.file_samples() <= MAX_CHUNK_SAMPLES));
        let total: u64 = plans.iter().flat_map(|p| &p.pieces).map(Region::len).sum();
        assert_eq!(total, 1_500 * SR);
    }

    #[test]
    fn split_point_di_jeda_terdekat_tengah() {
        let map = vec![
            OffsetEntry { file_ms: 0, orig_ms: 0, dur_ms: 100_000 },
            OffsetEntry { file_ms: 100_300, orig_ms: 200_000, dur_ms: 100_000 },
            OffsetEntry { file_ms: 200_600, orig_ms: 400_000, dur_ms: 300_000 },
        ];
        // File 500,6 dtk, tengah 250,3 dtk → jeda sebelum potongan ke-3 (200,45 dtk) lebih dekat.
        assert_eq!(split_point_ms(&map, 500_600), 200_450);
        let single = vec![OffsetEntry { file_ms: 0, orig_ms: 0, dur_ms: 600_000 }];
        assert_eq!(split_point_ms(&single, 600_000), 300_000);
    }

    #[test]
    fn sisa_kurang_10_dtk_digabung() {
        // 300 dtk lalu 5 dtk → sisa 5 dtk digabung ke chunk sebelumnya.
        let plans = plan_chunks(&vad(&[(0, 300), (310, 315)], 320), 300);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].pieces.len(), 2);
    }
}
