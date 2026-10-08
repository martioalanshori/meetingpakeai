//! Filter halusinasi (PRD §8.3) dan normalisasi teks (dipakai filter & dedup).

use crate::config::providers::PipelineConfig;

/// Frasa pendek umum ("terima kasih", "ya") hanya dibuang jika region VAD-nya lebih pendek dari ini
/// (noise singkat seperti ketukan/batuk yang ditranskrip Whisper menjadi frasa umum).
const SHORT_REGION_MS: i64 = 1_000;
const SHORT_PHRASE_MAX_WORDS: usize = 3;
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
/// `region_ms` = durasi region VAD asal segment (`None` jika tidak diketahui).
pub fn is_hallucination(seg: &SttSegment, cfg: &PipelineConfig, region_ms: Option<i64>) -> bool {
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
    let long_phrase = cfg
        .hallucination_phrases
        .iter()
        .map(|p| normalize(p))
        .any(|p| !p.is_empty() && norm.contains(&p));
    long_phrase || (region_ms.is_some_and(|ms| ms < SHORT_REGION_MS) && is_short_phrase(&norm, cfg))
}

/// Teks (sudah dinormalisasi) ≤ 3 kata dan sama persis dengan salah satu frasa pendek.
fn is_short_phrase(norm: &str, cfg: &PipelineConfig) -> bool {
    norm.split(' ').count() <= SHORT_PHRASE_MAX_WORDS
        && cfg.short_hallucination_phrases.iter().any(|p| normalize(p) == norm)
}
