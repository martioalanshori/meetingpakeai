//! Merge (PRD §8.4): segment kedua channel → waktu asli lewat offset map → urut `start_ms` (seri: mic dulu).

use crate::config::providers::PipelineConfig;
use crate::db::repo_chunks::ChunkRow;
use crate::preprocess::{map_time, region_duration_at, OffsetEntry};
use crate::stt::SttSegment;

use super::filter::is_hallucination;

#[derive(Debug, Clone)]
pub struct Segment {
    pub channel: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub no_speech_prob: f64,
    pub avg_logprob: f64,
    pub compression_ratio: f64,
    pub is_filtered: bool,
    pub is_duplicate: bool,
}

/// Bangun segment dari semua chunk yang sudah `done`. Chunk dengan JSON rusak dilewati (dicatat ke log).
pub fn build_segments(chunks: &[ChunkRow], cfg: &PipelineConfig) -> Vec<Segment> {
    let mut out = Vec::new();
    for c in chunks.iter().filter(|c| c.stt_status == "done") {
        let Some(raw) = c.response_json.as_deref() else { continue };
        let segs: Vec<SttSegment> = match serde_json::from_str(raw) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("respons chunk {} tidak bisa dibaca: {e}", c.id);
                continue;
            }
        };
        let map: Vec<OffsetEntry> = serde_json::from_str(&c.offset_map_json).unwrap_or_default();
        for s in segs {
            let file_start = (s.start_s * 1000.0).round() as i64;
            let start = map_time(&map, file_start);
            let region_ms = region_duration_at(&map, file_start);
            let end = map_time(&map, (s.end_s * 1000.0).round() as i64).max(start);
            out.push(Segment {
                channel: c.channel.clone(),
                start_ms: start,
                end_ms: end,
                text: s.text.trim().to_string(),
                no_speech_prob: s.no_speech_prob,
                avg_logprob: s.avg_logprob,
                compression_ratio: s.compression_ratio,
                is_filtered: is_hallucination(&s, cfg, region_ms),
                is_duplicate: false,
            });
        }
    }
    out.sort_by(|a, b| {
        a.start_ms
            .cmp(&b.start_ms)
            .then_with(|| (a.channel != "mic").cmp(&(b.channel != "mic")))
    });
    out
}
