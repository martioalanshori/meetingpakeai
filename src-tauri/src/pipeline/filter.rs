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
///
/// Langkah 45 (feedback3 G1): dianggap hening hanya jika `no_speech_prob` tinggi **dan** `avg_logprob`
/// rendah (aturan Whisper). Segment yang sekadar ragu tetap disimpan (ditandai lewat `avg_logprob` di UI);
/// `compression_ratio` tinggi (pengulangan) ditangani `collapse_repeats`, bukan dibuang.
pub fn is_hallucination(seg: &SttSegment, cfg: &PipelineConfig, region_ms: Option<i64>) -> bool {
    if seg.no_speech_prob > cfg.no_speech_prob_max && seg.avg_logprob < cfg.avg_logprob_min {
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

/// Rapikan pengulangan khas Whisper ("oke oke oke oke …", frasa yang sama berulang): rangkaian kata
/// 1–8 kata yang muncul ≥ 3 kali berturut-turut disisakan satu.
pub fn collapse_repeats(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let key = |w: &str| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    let mut out: Vec<&str> = Vec::with_capacity(words.len());
    let mut i = 0;
    'outer: while i < words.len() {
        for n in 1..=8usize {
            if i + n * 3 > words.len() {
                break;
            }
            let same = |a: usize, b: usize| (0..n).all(|k| key(words[a + k]) == key(words[b + k]));
            let mut reps = 1;
            while i + (reps + 1) * n <= words.len() && same(i, i + reps * n) {
                reps += 1;
            }
            if reps >= 3 {
                out.extend_from_slice(&words[i + (reps - 1) * n..i + reps * n]);
                i += reps * n;
                continue 'outer;
            }
        }
        out.push(words[i]);
        i += 1;
    }
    out.join(" ")
}

/// Teks (sudah dinormalisasi) ≤ 3 kata dan sama persis dengan salah satu frasa pendek.
fn is_short_phrase(norm: &str, cfg: &PipelineConfig) -> bool {
    norm.split(' ').count() <= SHORT_PHRASE_MAX_WORDS
        && cfg.short_hallucination_phrases.iter().any(|p| normalize(p) == norm)
}
