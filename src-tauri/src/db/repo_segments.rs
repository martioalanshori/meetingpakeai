//! Tabel `transcript_segments`.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::AppResult;
use crate::pipeline::merge::Segment;

/// `TranscriptSegment` di UI (PRD §12.2).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisibleSegment {
    pub id: i64,
    pub channel: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    /// Whisper ragu (audio kurang jelas): ditandai di UI, langkah 56 (E1).
    pub low_confidence: bool,
}

/// Ambang "ragu" untuk penanda kualitas (lebih longgar dari filter).
pub const LOW_CONFIDENCE_LOGPROB: f64 = -0.85;

pub fn replace_all(conn: &mut Connection, meeting_id: &str, segments: &[Segment]) -> AppResult<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM transcript_segments WHERE meeting_id = ?1", [meeting_id])?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO transcript_segments
               (meeting_id, channel, start_ms, end_ms, text, no_speech_prob, avg_logprob, compression_ratio, is_filtered, is_duplicate)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )?;
        for s in segments {
            stmt.execute(params![
                meeting_id,
                s.channel,
                s.start_ms,
                s.end_ms,
                s.text,
                s.no_speech_prob,
                s.avg_logprob,
                s.compression_ratio,
                s.is_filtered,
                s.is_duplicate
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn delete_for_meeting(conn: &Connection, meeting_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM transcript_segments WHERE meeting_id = ?1", [meeting_id])?;
    Ok(())
}

/// Hanya segment yang ditampilkan: `is_filtered = 0 AND is_duplicate = 0`.
/// Ubah teks satu segment (langkah 52); mengembalikan id meeting-nya.
pub fn update_text(conn: &Connection, segment_id: i64, text: &str) -> AppResult<String> {
    let meeting_id: String =
        conn.query_row("SELECT meeting_id FROM transcript_segments WHERE id = ?1", [segment_id], |r| r.get(0))?;
    conn.execute("UPDATE transcript_segments SET text = ?2 WHERE id = ?1", params![segment_id, text])?;
    Ok(meeting_id)
}

pub fn list_visible(conn: &Connection, meeting_id: &str) -> AppResult<Vec<VisibleSegment>> {
    let mut stmt = conn.prepare(
        "SELECT id, channel, start_ms, end_ms, text, avg_logprob, no_speech_prob FROM transcript_segments
         WHERE meeting_id = ?1 AND is_filtered = 0 AND is_duplicate = 0
         ORDER BY start_ms, CASE channel WHEN 'mic' THEN 0 ELSE 1 END, id",
    )?;
    let rows = stmt
        .query_map([meeting_id], |r| {
            let (logprob, nsp): (Option<f64>, Option<f64>) = (r.get(5)?, r.get(6)?);
            Ok(VisibleSegment {
                id: r.get(0)?,
                channel: r.get(1)?,
                start_ms: r.get(2)?,
                end_ms: r.get(3)?,
                text: r.get(4)?,
                low_confidence: logprob.is_some_and(|l| l < LOW_CONFIDENCE_LOGPROB) || nsp.is_some_and(|n| n > 0.5),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
