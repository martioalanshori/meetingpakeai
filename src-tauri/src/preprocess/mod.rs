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
use crate::db::{repo_parts, Db};
use crate::error::AppResult;

/// Step `preprocessing` satu meeting. Idempoten: `upload/` dan row `upload_chunks` lama dihapus dulu.
/// Mengembalikan jumlah chunk yang dibuat.
pub fn run(data_dir: &Path, db: &Db, meeting_id: &str, chunk_target_sec: u32) -> AppResult<usize> {
    let rel_upload = format!("recordings/{meeting_id}/upload");
    let upload_dir = data_dir.join(&rel_upload);
    if upload_dir.exists() {
        std::fs::remove_dir_all(&upload_dir)?;
    }
    repo_chunks::delete_for_meeting(&db.conn(), meeting_id)?;
    std::fs::create_dir_all(&upload_dir)?;

    let parts = repo_parts::list(&db.conn(), meeting_id)?;
    let mut count = 0;
    for channel in [Channel::Mic, Channel::System] {
        let paths: Vec<_> = parts
            .iter()
            .filter(|p| p.channel == channel.as_str())
            .map(|p| data_dir.join(&p.path))
            .collect();
        let reader = reader::PartReader::open(&paths)?;
        let vad = vad::detect(&reader)?;
        let plans = chunker::plan_chunks(&vad, chunk_target_sec);
        tracing::info!(
            "praproses {meeting_id} {}: {} dtk audio, {} region, {} chunk",
            channel.as_str(),
            reader.total_samples() / 16_000,
            vad.regions.len(),
            plans.len()
        );
        // Channel tanpa region → tidak ada request STT.
        for (i, plan) in plans.iter().enumerate() {
            let idx = i as i64 + 1;
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
            count += 1;
        }
    }
    Ok(count)
}
