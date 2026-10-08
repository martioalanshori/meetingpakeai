//! Rate limiter (PRD §9.5): jendela 60 dtk, 3600 dtk, dan hari kalender lokal, dihitung dari `usage_log`.
//! Memakai `batas × safety_factor`.

use std::time::Duration;

use chrono::{Days, Local, NaiveTime, TimeZone};
use rusqlite::Connection;

use crate::config::providers::Limits;
use crate::db::repo_usage::{self, UsageKind};
use crate::error::AppResult;

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 3_600_000;
/// Jeda cek ulang saat menunggu jendela menit/jam.
const POLL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Admission {
    Go,
    /// Tidak muat di jendela menit/jam: tunggu lalu cek lagi.
    Wait(Duration),
    /// Tidak muat di jendela harian: lanjut pada epoch ms ini (00:05 lokal besok).
    NextDay(i64),
}

/// Biaya satu request.
#[derive(Debug, Clone, Copy)]
pub enum Cost {
    /// Detik audio (minimal 10).
    Stt { audio_sec: f64 },
    /// Estimasi token input + max_tokens.
    Llm { tokens: i64 },
}

fn local_midnight_ms(now_ms: i64) -> i64 {
    let now = Local.timestamp_millis_opt(now_ms).single().unwrap_or_else(Local::now);
    now.date_naive()
        .and_time(NaiveTime::MIN)
        .and_local_timezone(Local)
        .earliest()
        .map_or(now_ms - 86_400_000, |t| t.timestamp_millis())
}

/// 00:05 waktu lokal hari berikutnya (PRD §9.5).
pub fn next_day_0005_ms(now_ms: i64) -> i64 {
    let now = Local.timestamp_millis_opt(now_ms).single().unwrap_or_else(Local::now);
    let tomorrow = now.date_naive().checked_add_days(Days::new(1)).unwrap_or(now.date_naive());
    tomorrow
        .and_time(NaiveTime::from_hms_opt(0, 5, 0).unwrap_or(NaiveTime::MIN))
        .and_local_timezone(Local)
        .earliest()
        .map_or(now_ms + 86_400_000, |t| t.timestamp_millis())
}

/// Muat jika pemakaian + biaya ≤ batas × safety. Jendela kosong selalu boleh (agar request besar tidak macet).
fn fits(used: f64, cost: f64, limit: u32, safety: f64) -> bool {
    used <= 0.0 || used + cost <= f64::from(limit) * safety
}

pub fn check(conn: &Connection, limits: &Limits, cost: Cost, now_ms: i64) -> AppResult<Admission> {
    let sf = limits.safety_factor;
    let kind = match cost {
        Cost::Stt { .. } => UsageKind::Stt,
        Cost::Llm { .. } => UsageKind::Llm,
    };
    let minute = repo_usage::totals_since(conn, kind, now_ms - MINUTE_MS)?;
    let hour = repo_usage::totals_since(conn, kind, now_ms - HOUR_MS)?;
    let day = repo_usage::totals_since(conn, kind, local_midnight_ms(now_ms))?;

    let (day_ok, short_ok) = match cost {
        Cost::Stt { audio_sec } => {
            let a = audio_sec.max(10.0);
            (
                fits(day.requests as f64, 1.0, limits.stt_rpd, sf) && fits(day.audio_sec, a, limits.stt_audio_sec_per_day, sf),
                fits(minute.requests as f64, 1.0, limits.stt_rpm, sf)
                    && fits(hour.audio_sec, a, limits.stt_audio_sec_per_hour, sf),
            )
        }
        Cost::Llm { tokens } => {
            let t = tokens as f64;
            (
                fits(day.requests as f64, 1.0, limits.llm_rpd, sf) && fits(day.tokens as f64, t, limits.llm_tpd, sf),
                fits(minute.requests as f64, 1.0, limits.llm_rpm, sf) && fits(minute.tokens as f64, t, limits.llm_tpm, sf),
            )
        }
    };
    Ok(if !day_ok {
        Admission::NextDay(next_day_0005_ms(now_ms))
    } else if !short_ok {
        Admission::Wait(POLL)
    } else {
        Admission::Go
    })
}

/// Pemakaian hari ini (sejak 00:00 lokal) vs batas × safety (langkah 29, estimasi kuota di Pengaturan).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaToday {
    pub stt_audio_sec_used: f64,
    pub stt_audio_sec_limit: f64,
    pub stt_requests_used: i64,
    pub stt_requests_limit: f64,
    pub llm_tokens_used: i64,
    pub llm_tokens_limit: f64,
    pub llm_requests_used: i64,
    pub llm_requests_limit: f64,
}

pub fn quota_today(conn: &Connection, limits: &Limits, now_ms: i64) -> AppResult<QuotaToday> {
    let since = local_midnight_ms(now_ms);
    let stt = repo_usage::totals_since(conn, UsageKind::Stt, since)?;
    let llm = repo_usage::totals_since(conn, UsageKind::Llm, since)?;
    let sf = limits.safety_factor;
    Ok(QuotaToday {
        stt_audio_sec_used: stt.audio_sec,
        stt_audio_sec_limit: f64::from(limits.stt_audio_sec_per_day) * sf,
        stt_requests_used: stt.requests,
        stt_requests_limit: f64::from(limits.stt_rpd) * sf,
        llm_tokens_used: llm.tokens,
        llm_tokens_limit: f64::from(limits.llm_tpd) * sf,
        llm_requests_used: llm.requests,
        llm_requests_limit: f64::from(limits.llm_rpd) * sf,
    })
}

/// Catat pemakaian setelah request selesai (sukses maupun gagal yang sudah terkirim).
pub fn record(conn: &Connection, cost: Cost, actual_tokens: Option<i64>, now_ms: i64) -> AppResult<()> {
    match cost {
        Cost::Stt { audio_sec } => repo_usage::insert(conn, UsageKind::Stt, now_ms, audio_sec.max(10.0), 0),
        Cost::Llm { tokens } => repo_usage::insert(conn, UsageKind::Llm, now_ms, 0.0, actual_tokens.unwrap_or(tokens)),
    }
}
