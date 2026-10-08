//! "Tanya meeting ini" (langkah 51, feedback3 C1): jawaban dari transkrip + notulen satu meeting, dengan
//! waktu sumber. Meeting panjang: hanya potongan yang relevan dengan pertanyaan yang dikirim (pencarian
//! kata sederhana, jendela ±2 menit), agar muat batas token.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::db::now_ms;
use crate::db::repo_segments::VisibleSegment;
use crate::error::AppResult;
use crate::pipeline::filter::normalize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QaItem {
    pub id: i64,
    pub question: String,
    pub answer: String,
    /// Ms dari awal meeting.
    pub sources: Vec<i64>,
    pub created_at: i64,
}

pub fn list(conn: &Connection, meeting_id: &str) -> AppResult<Vec<QaItem>> {
    let mut stmt =
        conn.prepare("SELECT id, question, answer, sources, created_at FROM meeting_qa WHERE meeting_id = ?1 ORDER BY id")?;
    let rows = stmt
        .query_map([meeting_id], |r| {
            Ok(QaItem {
                id: r.get(0)?,
                question: r.get(1)?,
                answer: r.get(2)?,
                sources: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                created_at: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn insert(conn: &Connection, meeting_id: &str, question: &str, answer: &str, sources: &[i64]) -> AppResult<QaItem> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO meeting_qa (meeting_id, question, answer, sources, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![meeting_id, question, answer, serde_json::to_string(sources)?, now],
    )?;
    Ok(QaItem {
        id: conn.last_insert_rowid(),
        question: question.to_string(),
        answer: answer.to_string(),
        sources: sources.to_vec(),
        created_at: now,
    })
}

pub fn clear(conn: &Connection, meeting_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM meeting_qa WHERE meeting_id = ?1", [meeting_id])?;
    Ok(())
}

/// Kata umum Indonesia/Inggris yang tidak membantu mencari potongan relevan.
const STOP: [&str; 40] = [
    "yang", "dan", "di", "ke", "dari", "ini", "itu", "apa", "apakah", "siapa", "kapan", "bagaimana", "kenapa", "mengapa",
    "berapa", "untuk", "dengan", "ada", "tidak", "akan", "sudah", "saja", "atau", "pada", "dalam", "the", "a", "an",
    "what", "who", "when", "how", "why", "is", "are", "of", "to", "in", "and", "meeting",
];

fn line(s: &VisibleSegment) -> String {
    format!("[{}] {}", crate::format_hhmmss(s.start_ms), s.text.trim())
}

/// Transkrip untuk konteks: utuh bila muat `budget` karakter; selain itu jendela ±2 menit di sekitar segment
/// dengan kata kunci pertanyaan terbanyak (urut waktu, digabung bila tumpang tindih).
pub fn transcript_context(segs: &[VisibleSegment], question: &str, budget: usize) -> String {
    let full: Vec<String> = segs.iter().map(line).collect();
    let total: usize = full.iter().map(|l| l.chars().count() + 1).sum();
    if total <= budget {
        return full.join("\n");
    }
    let keys: Vec<String> = normalize(question)
        .split_whitespace()
        .filter(|w| w.chars().count() >= 3 && !STOP.contains(w))
        .map(str::to_string)
        .collect();
    let mut scored: Vec<(usize, usize)> = segs
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let t = normalize(&s.text);
            (keys.iter().filter(|k| t.contains(k.as_str())).count(), i)
        })
        .filter(|(n, _)| *n > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut keep = vec![false; segs.len()];
    let mut used = 0usize;
    for (_, i) in scored {
        let center = segs[i].start_ms;
        for (j, s) in segs.iter().enumerate() {
            if !keep[j] && (s.start_ms - center).abs() <= 120_000 {
                keep[j] = true;
                used += full[j].chars().count() + 1;
            }
        }
        if used >= budget {
            break;
        }
    }
    if used == 0 {
        // Tidak ada kata kunci yang cocok: kirim awal + akhir transkrip.
        return crate::quick_llm::clamp_context(&full.join("\n"), budget);
    }
    let mut out = String::new();
    let mut prev_kept = true;
    for (j, l) in full.iter().enumerate() {
        if keep[j] {
            if !prev_kept && !out.is_empty() {
                out.push_str("[…]\n");
            }
            out.push_str(l);
            out.push('\n');
            prev_kept = true;
        } else {
            prev_kept = false;
        }
        if out.chars().count() > budget {
            break;
        }
    }
    out
}

/// Kata kunci pertanyaan (kata umum dibuang), untuk pencarian FTS.
pub fn keywords(question: &str) -> Vec<String> {
    normalize(question)
        .split_whitespace()
        .filter(|w| w.chars().count() >= 3 && !STOP.contains(w))
        .map(str::to_string)
        .collect()
}

/// Rentang waktu yang disebut di pertanyaan ("hari ini", "minggu ini", "bulan lalu", …) → (mulai, akhir) ms.
pub fn time_window(question: &str, now: chrono::DateTime<chrono::Local>) -> Option<(i64, i64)> {
    use chrono::{Datelike, Duration, TimeZone};
    let q = question.to_lowercase();
    let day = |d: chrono::NaiveDate| chrono::Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).single().map(|t| t.timestamp_millis());
    let today = now.date_naive();
    let monday = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
    let first = today.with_day(1)?;
    let (start, end) = if q.contains("hari ini") || q.contains("today") {
        (today, today + Duration::days(1))
    } else if q.contains("kemarin") || q.contains("yesterday") {
        (today - Duration::days(1), today)
    } else if q.contains("minggu lalu") || q.contains("pekan lalu") || q.contains("last week") {
        (monday - Duration::days(7), monday)
    } else if q.contains("minggu ini") || q.contains("pekan ini") || q.contains("this week") {
        (monday, today + Duration::days(1))
    } else if q.contains("bulan lalu") || q.contains("last month") {
        let prev = (first - Duration::days(1)).with_day(1)?;
        (prev, first)
    } else if q.contains("bulan ini") || q.contains("this month") {
        (first, today + Duration::days(1))
    } else {
        return None;
    };
    Some((day(start)?, day(end)?))
}
