//! Audio untuk diputar di Detail meeting (langkah 26): mic + sistem dicampur ke satu WAV
//! `recordings/<id>/playback.wav`, dibuat sekali saat pertama diminta lalu dipakai ulang.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::audio::writer::WAV_SPEC;
use crate::db::{repo_parts, Db};
use crate::error::{AppError, AppResult};
use crate::preprocess::reader::PartReader;

const BLOCK_SAMPLES: u64 = 10 * 16_000;
pub const PLAYBACK_FILE: &str = "playback.wav";

/// Satu pembuatan sekaligus: pembuatan di latar belakang (setelah job selesai) dan klik pengguna
/// tidak boleh menulis file sementara yang sama bersamaan.
static BUILD_LOCK: Mutex<()> = Mutex::new(());

/// Path absolut file playback; dibuat jika belum ada.
pub fn prepare(data_dir: &Path, db: &Db, meeting_id: &str) -> AppResult<PathBuf> {
    let out = data_dir.join("recordings").join(meeting_id).join(PLAYBACK_FILE);
    if out.exists() {
        return Ok(out);
    }
    let _guard = BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if out.exists() {
        return Ok(out);
    }
    let parts = repo_parts::list(&db.conn(), meeting_id)?;
    let paths = |channel: &str| -> Vec<PathBuf> {
        parts.iter().filter(|p| p.channel == channel).map(|p| data_dir.join(&p.path)).collect()
    };
    let mic = PartReader::open(&paths("mic"))?;
    let sys = PartReader::open(&paths("system"))?;
    let total = mic.total_samples().max(sys.total_samples());

    // Tulis ke file sementara lalu rename, agar file setengah jadi tidak pernah dipakai.
    let tmp = out.with_extension("wav.tmp");
    let mut w = hound::WavWriter::create(&tmp, WAV_SPEC).map_err(AppError::internal)?;
    let mut pos = 0u64;
    while pos < total {
        let n = BLOCK_SAMPLES.min(total - pos);
        let a = mic.read_range(pos, n)?;
        let b = sys.read_range(pos, n)?;
        for i in 0..n as usize {
            let s = i32::from(a.get(i).copied().unwrap_or(0)) + i32::from(b.get(i).copied().unwrap_or(0));
            w.write_sample(s.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16).map_err(AppError::internal)?;
        }
        pos += n;
    }
    w.finalize().map_err(AppError::internal)?;
    std::fs::rename(&tmp, &out)?;
    tracing::info!("audio playback {meeting_id} dibuat ({} dtk)", total / 16_000);
    Ok(out)
}
