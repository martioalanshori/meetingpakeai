//! Filter halusinasi (PRD §8.3) dan normalisasi teks (dipakai filter & dedup).

use crate::config::providers::PipelineConfig;
use crate::stt::SttSegment;

/// lowercase, hapus tanda baca, spasi berulang → satu spasi, trim.
pub fn normalize(text: &str) -> String {
    let lowered = text.to_lowercase();
    let cleaned: String = lowered
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `true` jika segment harus dibuang (`is_filtered = 1`).
pub fn is_hallucination(seg: &SttSegment, cfg: &PipelineConfig) -> bool {
    if seg.no_speech_prob > cfg.no_speech_prob_max
        || seg.avg_logprob < cfg.avg_logprob_min
        || seg.compression_ratio > cfg.compression_ratio_max
    {
        return true;
    }
    let norm = normalize(&seg.text);
    if norm.is_empty() {
        return true;
    }
    cfg.hallucination_phrases
        .iter()
        .map(|p| normalize(p))
        .any(|p| !p.is_empty() && norm.contains(&p))
}
