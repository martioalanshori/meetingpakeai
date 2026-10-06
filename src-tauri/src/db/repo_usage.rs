//! Tabel `usage_log`: dasar perhitungan rate limiter (PRD §9.5).

use rusqlite::{params, Connection};

use crate::error::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageKind {
    Stt,
    Llm,
}

impl UsageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stt => "stt",
            Self::Llm => "llm",
        }
    }
}

/// Total pemakaian dalam satu jendela waktu.
#[derive(Debug, Clone, Copy, Default)]
pub struct UsageTotals {
    pub requests: i64,
    pub audio_sec: f64,
    pub tokens: i64,
}

pub fn insert(conn: &Connection, kind: UsageKind, ts: i64, audio_sec: f64, tokens: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO usage_log (ts, kind, audio_sec, tokens) VALUES (?1, ?2, ?3, ?4)",
        params![ts, kind.as_str(), audio_sec, tokens],
    )?;
    Ok(())
}

/// Jumlah pemakaian dengan `ts >= since_ms`.
pub fn totals_since(conn: &Connection, kind: UsageKind, since_ms: i64) -> AppResult<UsageTotals> {
    Ok(conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(audio_sec), 0), COALESCE(SUM(tokens), 0)
         FROM usage_log WHERE kind = ?1 AND ts >= ?2",
        params![kind.as_str(), since_ms],
        |r| Ok(UsageTotals { requests: r.get(0)?, audio_sec: r.get(1)?, tokens: r.get(2)? }),
    )?)
}

/// Hapus log lebih tua dari `before_ms` (dipanggil saat start: lebih tua dari 2 hari).
pub fn delete_older_than(conn: &Connection, before_ms: i64) -> AppResult<usize> {
    Ok(conn.execute("DELETE FROM usage_log WHERE ts < ?1", [before_ms])?)
}
