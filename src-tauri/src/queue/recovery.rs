//! Recovery saat aplikasi start (PRD §13).
//! 1. Meeting `recording` → repair header part `finalized = 0` → `interrupted`.
//! 2. Step berjalan → dilanjutkan worker. 3. `waiting_*` → dijadwalkan worker sesuai `next_run_at`.

use std::path::Path;

use crate::audio::writer::repair_wav_header;
use crate::audio::SAMPLE_RATE;
use crate::db::repo_meetings::{self, MeetingStatus};
use crate::db::{repo_parts, Db};
use crate::error::AppResult;

pub fn run(data_dir: &Path, db: &Db) -> AppResult<()> {
    let stale = repo_meetings::list_by_status(&db.conn(), MeetingStatus::Recording)?;
    for m in stale {
        let parts = repo_parts::list(&db.conn(), &m.id)?;
        let mut per_channel = [0i64; 2];
        for p in &parts {
            let samples = if p.finalized {
                p.samples
            } else {
                match repair_wav_header(&data_dir.join(&p.path)) {
                    Ok(n) => {
                        let n = n as i64;
                        let _ = repo_parts::mark_finalized(&db.conn(), &m.id, &p.channel, p.part_index, n);
                        n
                    }
                    Err(e) => {
                        tracing::warn!("repair part gagal ({e})");
                        0
                    }
                }
            };
            per_channel[usize::from(p.channel != "mic")] += samples;
        }
        let duration_ms = per_channel.iter().max().copied().unwrap_or(0) * 1000 / i64::from(SAMPLE_RATE);
        let conn = db.conn();
        repo_meetings::set_duration(&conn, &m.id, duration_ms)?;
        repo_meetings::set_status(&conn, &m.id, MeetingStatus::Interrupted)?;
        tracing::info!("meeting {} terputus saat merekam: {} ms audio diselamatkan", m.id, duration_ms);
    }
    Ok(())
}
