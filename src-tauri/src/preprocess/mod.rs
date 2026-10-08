//! Praproses per channel (PRD §8.1): baca part → VAD → region → upload chunk + offset map.

pub mod chunker;
pub mod reader;
pub mod vad;

use serde::{Deserialize, Serialize};

/// Satu entri offset map: posisi di file upload → waktu asli di rekaman (timeline).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OffsetEntry {
    pub file_ms: i64,
    pub orig_ms: i64,
    pub dur_ms: i64,
}

/// Pemetaan waktu (PRD §8.2): cari entri terbesar dengan `file_ms ≤ t`,
/// `orig = orig_ms + min(t − file_ms, dur_ms)`.
pub fn map_time(map: &[OffsetEntry], t_ms: i64) -> i64 {
    let mut chosen = match map.first() {
        Some(e) => e,
        None => return t_ms,
    };
    for e in map {
        if e.file_ms <= t_ms {
            chosen = e;
        } else {
            break;
        }
    }
    chosen.orig_ms + (t_ms - chosen.file_ms).clamp(0, chosen.dur_ms)
}

use std::path::Path;

use crate::audio::Channel;
use crate::db::repo_chunks::{self, NewChunk};
use crate::db::{repo_live, repo_parts, Db};
use crate::error::AppResult;

/// Durasi potongan region asal untuk posisi `t_ms` di file upload (`None` jika map kosong).
pub fn region_duration_at(map: &[OffsetEntry], t_ms: i64) -> Option<i64> {
    map.iter().take_while(|e| e.file_ms <= t_ms).last().or(map.first()).map(|e| e.dur_ms)
}

/// Praproses bertahap: minimal audio baru per channel sebelum dipotong saat merekam (5 menit).
const LIVE_MIN_SAMPLES: u64 = 5 * 60 * 16_000;
/// Region yang berakhir dalam 1,5 dtk terakhir audio final ditunda (kalimat mungkin belum selesai).
const LIVE_HOLD_SAMPLES: u64 = 1_500 * 16;

/// Step `preprocessing` satu meeting setelah Stop. Tanpa progres live: idempoten, `upload/` dan chunk lama
/// dihapus lalu semua audio diproses. Dengan progres live: hanya sisa audio setelah titik terakhir.
/// Mengembalikan jumlah chunk yang dibuat.
pub fn run(data_dir: &Path, db: &Db, meeting_id: &str, chunk_target_sec: u32) -> AppResult<usize> {
    let rel_upload = format!("recordings/{meeting_id}/upload");
    let upload_dir = data_dir.join(&rel_upload);
    let resume = repo_live::has_any(&db.conn(), meeting_id)?;
    if !resume {
        if upload_dir.exists() {
            std::fs::remove_dir_all(&upload_dir)?;
        }
        repo_chunks::delete_for_meeting(&db.conn(), meeting_id)?;
    }
    std::fs::create_dir_all(&upload_dir)?;

    let parts = repo_parts::list(&db.conn(), meeting_id)?;
    let mut count = 0;
    for channel in [Channel::Mic, Channel::System] {
        let paths: Vec<_> = parts
            .iter()
            .filter(|p| p.channel == channel.as_str())
            .map(|p| data_dir.join(&p.path))
            .collect();
        let from = if resume { repo_live::get(&db.conn(), meeting_id, channel.as_str())?.unwrap_or(0) } else { 0 };
        let (n, _) = process_range(data_dir, db, meeting_id, channel, &paths, from, None, chunk_target_sec)?;
        count += n;
    }
    if resume {
        repo_live::clear(&db.conn(), meeting_id)?;
    }
    Ok(count)
}

/// Praproses bertahap selama merekam (langkah 37): hanya part yang sudah final, minimal 5 menit audio
/// baru per channel. Mengembalikan jumlah chunk baru.
pub fn run_live(data_dir: &Path, db: &Db, meeting_id: &str, chunk_target_sec: u32) -> AppResult<usize> {
    std::fs::create_dir_all(data_dir.join(format!("recordings/{meeting_id}/upload")))?;
    let parts = repo_parts::list(&db.conn(), meeting_id)?;
    let mut count = 0;
    for channel in [Channel::Mic, Channel::System] {
        // Part berurutan yang sudah final saja (part yang sedang ditulis tidak dibaca).
        let mut paths = Vec::new();
        let mut final_samples = 0u64;
        for p in parts.iter().filter(|p| p.channel == channel.as_str()) {
            if !p.finalized {
                break;
            }
            paths.push(data_dir.join(&p.path));
            final_samples += p.samples.max(0) as u64;
        }
        let from = repo_live::get(&db.conn(), meeting_id, channel.as_str())?.unwrap_or(0);
        if final_samples < from + LIVE_MIN_SAMPLES {
            continue;
        }
        let (n, processed) =
            process_range(data_dir, db, meeting_id, channel, &paths, from, Some(final_samples), chunk_target_sec)?;
        repo_live::set(&db.conn(), meeting_id, channel.as_str(), processed)?;
        count += n;
    }
    Ok(count)
}

/// VAD + chunk untuk audio channel mulai sampel `from`. `limit = Some(n)`: audio final hanya sampai `n`;
/// region yang menyentuh ujung ditunda dan batas proses berhenti di awalnya. Chunk dinomori lanjut dari
/// `idx` terbesar. Mengembalikan (jumlah chunk, batas sampel yang sudah diproses).
#[allow(clippy::too_many_arguments)]
fn process_range(
    data_dir: &Path,
    db: &Db,
    meeting_id: &str,
    channel: Channel,
    paths: &[std::path::PathBuf],
    from: u64,
    limit: Option<u64>,
    chunk_target_sec: u32,
) -> AppResult<(usize, u64)> {
    let rel_upload = format!("recordings/{meeting_id}/upload");
    let upload_dir = data_dir.join(&rel_upload);
    let reader = reader::PartReader::open(paths)?;
    let total = limit.unwrap_or_else(|| reader.total_samples()).min(reader.total_samples());
    if total <= from {
        return Ok((0, from));
    }
    let mut vad = vad::detect(&reader)?;

    // Region di [from, total); di mode live region yang berakhir terlalu dekat ujung ditunda.
    let hold = if limit.is_some() { total.saturating_sub(LIVE_HOLD_SAMPLES) } else { u64::MAX };
    let mut processed = if limit.is_some() { hold } else { total };
    let mut selected = Vec::new();
    for r in vad.regions.iter().filter(|r| r.end > from && r.start < total) {
        let clipped = vad::Region { start: r.start.max(from), end: r.end.min(total) };
        if clipped.end > hold {
            processed = processed.min(clipped.start);
            break;
        }
        selected.push(clipped);
    }
    let processed = processed.max(from);
    vad.regions = selected;
    let plans = chunker::plan_chunks(&vad, chunk_target_sec);
    tracing::info!(
        "praproses {meeting_id} {}: sampel {from}–{processed}, {} region, {} chunk",
        channel.as_str(),
        vad.regions.len(),
        plans.len()
    );
    let mut idx = repo_chunks::max_idx(&db.conn(), meeting_id, channel.as_str())?;
    for plan in &plans {
        idx += 1;
        let file = format!("{}_{:03}.wav", channel.as_str(), idx);
        let (duration_ms, map) = chunker::write_chunk(&reader, plan, &upload_dir.join(&file))?;
        let map_json = serde_json::to_string(&map)?;
        repo_chunks::insert(
            &db.conn(),
            &NewChunk {
                meeting_id,
                channel: channel.as_str(),
                idx,
                path: &format!("{rel_upload}/{file}"),
                duration_ms,
                offset_map_json: &map_json,
            },
        )?;
    }
    Ok((plans.len(), processed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> Vec<OffsetEntry> {
        // Dua potongan: file 0–1000 ms = asli 5000–6000; file 1300–3300 ms = asli 20 000–22 000 (jeda 300 ms).
        vec![
            OffsetEntry { file_ms: 0, orig_ms: 5_000, dur_ms: 1_000 },
            OffsetEntry { file_ms: 1_300, orig_ms: 20_000, dur_ms: 2_000 },
        ]
    }

    #[test]
    fn map_time_di_dalam_potongan() {
        assert_eq!(map_time(&map(), 0), 5_000);
        assert_eq!(map_time(&map(), 500), 5_500);
        assert_eq!(map_time(&map(), 1_300), 20_000);
        assert_eq!(map_time(&map(), 2_300), 21_000);
    }

    #[test]
    fn map_time_di_jeda_dijepit_ke_akhir_potongan() {
        assert_eq!(map_time(&map(), 1_200), 6_000);
        assert_eq!(map_time(&map(), 9_999), 22_000);
    }

    #[test]
    fn map_time_map_kosong_identitas() {
        assert_eq!(map_time(&[], 1_234), 1_234);
    }

    #[test]
    fn region_duration_at_memilih_potongan() {
        assert_eq!(region_duration_at(&map(), 100), Some(1_000));
        assert_eq!(region_duration_at(&map(), 1_500), Some(2_000));
        assert_eq!(region_duration_at(&[], 0), None);
    }
}
