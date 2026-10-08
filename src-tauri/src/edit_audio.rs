//! Penyuntingan meeting yang sudah selesai (langkah 58, feedback3 C6/C7): gabung dua meeting menjadi satu,
//! dan hapus satu bagian (segment transkrip + audionya dinolkan).

use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use crate::audio::writer::{PartWriter, WAV_SPEC};
use crate::audio::Channel;
use crate::db::repo_meetings::{self, MeetingStatus};
use crate::db::{repo_parts, Db};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::playback::PLAYBACK_FILE;

const SR: i64 = 16;
/// Header WAV hound = 44 byte (diverifikasi langkah 4).
const WAV_HEADER: u64 = 44;

fn bad(msg: &str) -> AppError {
    AppError::with_message(ErrorCode::InvalidState, msg)
}

fn channel_of(s: &str) -> Channel {
    if s == "system" { Channel::System } else { Channel::Mic }
}

/// Meeting `done` sebelum `id` (≤ 12 jam sebelumnya) yang bisa digabung.
pub fn previous_mergeable(db: &Db, id: &str) -> AppResult<Option<repo_meetings::MeetingRow>> {
    let conn = db.conn();
    let m = repo_meetings::get(&conn, id)?;
    let prev: Option<String> = conn
        .query_row(
            "SELECT id FROM meetings WHERE status = 'done' AND id <> ?1 AND started_at < ?2 AND started_at >= ?3
             ORDER BY started_at DESC LIMIT 1",
            rusqlite::params![id, m.started_at, m.started_at - 12 * 3600 * 1000],
            |r| r.get(0),
        )
        .ok();
    prev.map(|p| repo_meetings::get(&conn, &p)).transpose()
}

/// Gabung `b` (lebih akhir) ke `a`: part audio `b` dipindah ke folder `a` setelah audio `a` (channel yang lebih
/// pendek dipad hening), segment/momen digeser durasi `a`, catatan digabung, `b` dihapus. Ringkasan `a` dibuat ulang.
pub fn merge(data_dir: &Path, db: &Db, a_id: &str, b_id: &str) -> AppResult<()> {
    let (a, b) = {
        let conn = db.conn();
        (repo_meetings::get(&conn, a_id)?, repo_meetings::get(&conn, b_id)?)
    };
    if a.status != MeetingStatus::Done || b.status != MeetingStatus::Done {
        return Err(bad("Hanya meeting yang sudah selesai diproses yang bisa digabung."));
    }
    let offset_ms = a.duration_ms;
    let a_dir = data_dir.join("recordings").join(a_id);
    let audio_ok = !a.audio_deleted && !b.audio_deleted;
    if audio_ok {
        std::fs::create_dir_all(&a_dir)?;
        let target = offset_ms * SR;
        let a_parts = repo_parts::list(&db.conn(), a_id)?;
        let b_parts = repo_parts::list(&db.conn(), b_id)?;
        for ch in ["mic", "system"] {
            let mine: Vec<_> = a_parts.iter().filter(|p| p.channel == ch).collect();
            let mut next = mine.iter().map(|p| p.part_index).max().unwrap_or(0) + 1;
            let have: i64 = mine.iter().map(|p| p.samples).sum();
            let theirs: Vec<_> = b_parts.iter().filter(|p| p.channel == ch).collect();
            // Pad hening agar timeline kedua channel tetap sejajar sebelum audio `b`.
            if have < target && !theirs.is_empty() {
                let name = PartWriter::part_file_name(channel_of(ch), next as u32);
                let mut w = hound::WavWriter::create(a_dir.join(&name), WAV_SPEC).map_err(std::io::Error::other)?;
                for _ in 0..(target - have) {
                    w.write_sample(0i16).map_err(std::io::Error::other)?;
                }
                w.finalize().map_err(std::io::Error::other)?;
                let rel = format!("recordings/{a_id}/{name}");
                let conn = db.conn();
                repo_parts::insert_open(&conn, a_id, ch, next, &rel)?;
                repo_parts::mark_finalized(&conn, a_id, ch, next, target - have)?;
                next += 1;
            }
            for p in theirs {
                let name = PartWriter::part_file_name(channel_of(ch), next as u32);
                std::fs::rename(data_dir.join(&p.path), a_dir.join(&name))?;
                let rel = format!("recordings/{a_id}/{name}");
                let conn = db.conn();
                repo_parts::insert_open(&conn, a_id, ch, next, &rel)?;
                repo_parts::mark_finalized(&conn, a_id, ch, next, p.samples)?;
                next += 1;
            }
        }
        let _ = std::fs::remove_file(a_dir.join(PLAYBACK_FILE));
    }
    {
        let mut conn = db.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE transcript_segments SET meeting_id = ?1, start_ms = start_ms + ?3, end_ms = end_ms + ?3 WHERE meeting_id = ?2",
            rusqlite::params![a_id, b_id, offset_ms],
        )?;
        tx.execute(
            "UPDATE bookmarks SET meeting_id = ?1, at_ms = at_ms + ?3 WHERE meeting_id = ?2",
            rusqlite::params![a_id, b_id, offset_ms],
        )?;
        let notes_b: Option<String> =
            tx.query_row("SELECT text FROM meeting_notes WHERE meeting_id = ?1", [b_id], |r| r.get(0)).ok();
        if let Some(nb) = notes_b {
            let na: String = tx.query_row("SELECT text FROM meeting_notes WHERE meeting_id = ?1", [a_id], |r| r.get(0)).unwrap_or_default();
            let joined = if na.trim().is_empty() { nb } else { format!("{na}\n{nb}") };
            tx.execute(
                "INSERT INTO meeting_notes (meeting_id, text, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(meeting_id) DO UPDATE SET text = excluded.text, updated_at = excluded.updated_at",
                rusqlite::params![a_id, joined, crate::db::now_ms()],
            )?;
        }
        tx.execute("DELETE FROM meeting_qa WHERE meeting_id = ?1", [a_id])?;
        tx.execute(
            "UPDATE meetings SET duration_ms = ?2, ended_at = ?3, audio_deleted = ?4, updated_at = ?5 WHERE id = ?1",
            rusqlite::params![a_id, a.duration_ms + b.duration_ms, b.ended_at, !audio_ok, crate::db::now_ms()],
        )?;
        tx.commit()?;
    }
    repo_meetings::delete(&db.conn(), b_id)?;
    let b_dir = data_dir.join("recordings").join(b_id);
    if b_dir.exists() {
        let _ = std::fs::remove_dir_all(&b_dir);
    }
    tracing::info!("meeting {b_id} digabung ke {a_id} (offset {offset_ms} ms)");
    Ok(())
}

/// Nolkan audio rentang `[start_ms, end_ms)` di semua part kedua channel (header 44 byte, PCM16 mono).
fn silence_range(data_dir: &Path, db: &Db, meeting_id: &str, start_ms: i64, end_ms: i64) -> AppResult<()> {
    let parts = repo_parts::list(&db.conn(), meeting_id)?;
    let (from, to) = (start_ms * SR, end_ms * SR);
    for ch in ["mic", "system"] {
        let mut pos = 0i64;
        for p in parts.iter().filter(|p| p.channel == ch) {
            let (s, e) = (from.max(pos), to.min(pos + p.samples));
            if s < e {
                let mut f = std::fs::OpenOptions::new().write(true).open(data_dir.join(&p.path))?;
                f.seek(SeekFrom::Start(WAV_HEADER + ((s - pos) as u64) * 2))?;
                f.write_all(&vec![0u8; ((e - s) as usize) * 2])?;
            }
            pos += p.samples;
        }
    }
    let _ = std::fs::remove_file(data_dir.join("recordings").join(meeting_id).join(PLAYBACK_FILE));
    Ok(())
}

/// Hapus satu baris transkrip beserta audionya (dinolkan). Mengembalikan id meeting.
pub fn delete_segment(data_dir: &Path, db: &Db, segment_id: i64) -> AppResult<String> {
    let (meeting_id, start_ms, end_ms): (String, i64, i64) = db.conn().query_row(
        "SELECT meeting_id, start_ms, end_ms FROM transcript_segments WHERE id = ?1",
        [segment_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let m = repo_meetings::get(&db.conn(), &meeting_id)?;
    if !m.audio_deleted {
        silence_range(data_dir, db, &meeting_id, start_ms, end_ms)?;
    }
    db.conn().execute("DELETE FROM transcript_segments WHERE id = ?1", [segment_id])?;
    Ok(meeting_id)
}
