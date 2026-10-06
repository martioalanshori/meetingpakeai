//! Dedup echo (PRD §8.5): salinan di channel `mic` dari suara speaker ditandai duplikat.

use strsim::normalized_levenshtein;

use super::filter::normalize;
use super::merge::Segment;

const WINDOW_MS: i64 = 1000;

/// Tandai `is_duplicate` pada segment mic yang mirip (≥ `similarity`) dengan segment system yang beririsan
/// dengan `[m.start − 1000, m.end + 1000]`. Mengembalikan jumlah duplikat.
pub fn mark_duplicates(segments: &mut [Segment], similarity: f64) -> usize {
    let systems: Vec<(i64, i64, String)> = segments
        .iter()
        .filter(|s| s.channel == "system" && !s.is_filtered)
        .map(|s| (s.start_ms, s.end_ms, normalize(&s.text)))
        .collect();
    let mut count = 0;
    for m in segments.iter_mut().filter(|s| s.channel == "mic" && !s.is_filtered) {
        let (lo, hi) = (m.start_ms - WINDOW_MS, m.end_ms + WINDOW_MS);
        let norm_m = normalize(&m.text);
        let dup = systems
            .iter()
            .filter(|(s, e, _)| *s < hi && *e > lo)
            .any(|(_, _, t)| normalized_levenshtein(&norm_m, t) >= similarity);
        if dup {
            m.is_duplicate = true;
            count += 1;
        }
    }
    count
}
