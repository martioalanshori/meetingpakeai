//! Pengaturan pengguna (PRD §11.1), disimpan di tabel `settings` (key snake_case → JSON).
//! API key TIDAK pernah disimpan di sini (lihat `secrets.rs`).

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::repo_settings;
use crate::error::AppResult;

pub const KEY_ONBOARDING_COMPLETED: &str = "onboarding_completed";
pub const KEY_USER_DISPLAY_NAME: &str = "user_display_name";
pub const KEY_STT_LANGUAGE: &str = "stt_language";
/// Lama (MVP): bool. Dibaca hanya untuk migrasi ke `audio_retention`.
pub const KEY_DELETE_AUDIO: &str = "delete_audio_after_transcript";
pub const KEY_AUDIO_RETENTION: &str = "audio_retention";
pub const KEY_MINIMIZE_TO_TRAY: &str = "minimize_to_tray";
pub const KEY_RECORDER_POSITION: &str = "recorder_position";
pub const KEY_MAIN_GEOMETRY: &str = "main_window_geometry";
pub const KEY_GLOBAL_SHORTCUT: &str = "global_shortcut";
pub const KEY_AUTOSTART: &str = "autostart";
pub const KEY_MEETING_DETECTION: &str = "meeting_detection";
pub const KEY_STT_GLOSSARY: &str = "stt_glossary";
pub const KEY_BOOKMARK_SHORTCUT: &str = "bookmark_shortcut";
pub const DEFAULT_BOOKMARK_SHORTCUT: &str = "Ctrl+Alt+B";
/// Batas glosarium (= batas prompt STT).
pub const GLOSSARY_MAX_CHARS: usize = 800;

pub const DEFAULT_USER_DISPLAY_NAME: &str = "Saya";
pub const DEFAULT_SYSTEM_LABEL: &str = "Peserta lain";
pub const DEFAULT_GLOBAL_SHORTCUT: &str = "Ctrl+Alt+R";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SttLanguage {
    Id,
    Auto,
}

impl SttLanguage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Auto => "auto",
        }
    }
}

/// Retensi audio (F13, langkah 26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioRetention {
    /// Hapus setelah transkrip selesai (perilaku MVP).
    AfterTranscript,
    /// Simpan 7 hari setelah meeting (bisa diputar & ditranskrip ulang), lalu hapus.
    Days7,
    Forever,
}

/// Tipe `Settings` di UI (PRD §12.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub user_display_name: String,
    pub stt_language: SttLanguage,
    pub audio_retention: AudioRetention,
    pub minimize_to_tray: bool,
    /// Shortcut global Mulai/Stop rekam; kosong = mati.
    pub global_shortcut: String,
    /// Jalankan tersembunyi di tray saat Windows menyala.
    pub autostart: bool,
    /// Tawarkan rekam saat Zoom/Teams/browser memakai mic, dan tawarkan Stop saat selesai.
    pub meeting_detection: bool,
    /// Nama & istilah yang sering muncul, satu per baris (membantu ejaan transkrip & notulen).
    pub stt_glossary: String,
    /// Shortcut global tandai momen penting saat merekam; kosong = mati.
    pub bookmark_shortcut: String,
}

/// `Partial<Settings>` dari `update_settings`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub user_display_name: Option<String>,
    pub stt_language: Option<SttLanguage>,
    pub audio_retention: Option<AudioRetention>,
    pub minimize_to_tray: Option<bool>,
    pub global_shortcut: Option<String>,
    pub autostart: Option<bool>,
    pub meeting_detection: Option<bool>,
    pub stt_glossary: Option<String>,
    pub bookmark_shortcut: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

pub fn load(conn: &Connection) -> AppResult<Settings> {
    Ok(Settings {
        user_display_name: repo_settings::get(conn, KEY_USER_DISPLAY_NAME)?
            .unwrap_or_else(|| DEFAULT_USER_DISPLAY_NAME.to_string()),
        stt_language: repo_settings::get(conn, KEY_STT_LANGUAGE)?.unwrap_or(SttLanguage::Id),
        audio_retention: match repo_settings::get(conn, KEY_AUDIO_RETENTION)? {
            Some(r) => r,
            // Pilihan eksplisit dari versi lama dipertahankan; default baru = 7 hari.
            None => match repo_settings::get::<bool>(conn, KEY_DELETE_AUDIO)? {
                Some(true) => AudioRetention::AfterTranscript,
                Some(false) => AudioRetention::Forever,
                None => AudioRetention::Days7,
            },
        },
        minimize_to_tray: repo_settings::get(conn, KEY_MINIMIZE_TO_TRAY)?.unwrap_or(true),
        global_shortcut: repo_settings::get(conn, KEY_GLOBAL_SHORTCUT)?
            .unwrap_or_else(|| DEFAULT_GLOBAL_SHORTCUT.to_string()),
        autostart: repo_settings::get(conn, KEY_AUTOSTART)?.unwrap_or(false),
        meeting_detection: repo_settings::get(conn, KEY_MEETING_DETECTION)?.unwrap_or(true),
        stt_glossary: repo_settings::get(conn, KEY_STT_GLOSSARY)?.unwrap_or_default(),
        bookmark_shortcut: repo_settings::get(conn, KEY_BOOKMARK_SHORTCUT)?
            .unwrap_or_else(|| DEFAULT_BOOKMARK_SHORTCUT.to_string()),
    })
}

/// Simpan field yang dikirim saja. Teks di-trim; teks kosong → kembali ke default.
pub fn apply_patch(conn: &Connection, patch: SettingsPatch) -> AppResult<Settings> {
    if let Some(name) = patch.user_display_name {
        let name = name.trim();
        let name = if name.is_empty() { DEFAULT_USER_DISPLAY_NAME } else { name };
        repo_settings::set(conn, KEY_USER_DISPLAY_NAME, &name)?;
    }
    if let Some(lang) = patch.stt_language {
        repo_settings::set(conn, KEY_STT_LANGUAGE, &lang)?;
    }
    if let Some(v) = patch.audio_retention {
        repo_settings::set(conn, KEY_AUDIO_RETENTION, &v)?;
    }
    if let Some(v) = patch.minimize_to_tray {
        repo_settings::set(conn, KEY_MINIMIZE_TO_TRAY, &v)?;
    }
    // Shortcut sudah divalidasi & didaftarkan pemanggil (command); kosong = mati.
    if let Some(sc) = patch.global_shortcut {
        repo_settings::set(conn, KEY_GLOBAL_SHORTCUT, &sc.trim())?;
    }
    if let Some(v) = patch.autostart {
        repo_settings::set(conn, KEY_AUTOSTART, &v)?;
    }
    if let Some(v) = patch.meeting_detection {
        repo_settings::set(conn, KEY_MEETING_DETECTION, &v)?;
    }
    if let Some(sc) = patch.bookmark_shortcut {
        repo_settings::set(conn, KEY_BOOKMARK_SHORTCUT, &sc.trim())?;
    }
    if let Some(v) = patch.stt_glossary {
        // Baris kosong dibuang; dipotong ke batas karakter.
        let lines: Vec<&str> = v.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let v: String = lines.join("\n").chars().take(GLOSSARY_MAX_CHARS).collect();
        repo_settings::set(conn, KEY_STT_GLOSSARY, &v)?;
    }
    load(conn)
}

/// Istilah glosarium (satu per baris, tanpa duplikat).
pub fn glossary_terms(glossary: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in glossary.lines().map(str::trim).filter(|t| !t.is_empty()) {
        if !out.iter().any(|o| o.eq_ignore_ascii_case(t)) {
            out.push(t.to_string());
        }
    }
    out
}

pub fn onboarding_completed(conn: &Connection) -> AppResult<bool> {
    Ok(repo_settings::get(conn, KEY_ONBOARDING_COMPLETED)?.unwrap_or(false))
}

pub fn set_onboarding_completed(conn: &Connection) -> AppResult<()> {
    repo_settings::set(conn, KEY_ONBOARDING_COMPLETED, &true)
}

/// Ukuran & posisi jendela main terakhir (piksel fisik), dipulihkan saat jendela dibuat ulang.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MainGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

pub fn main_geometry(conn: &Connection) -> AppResult<Option<MainGeometry>> {
    repo_settings::get(conn, KEY_MAIN_GEOMETRY)
}

pub fn set_main_geometry(conn: &Connection, g: MainGeometry) -> AppResult<()> {
    repo_settings::set(conn, KEY_MAIN_GEOMETRY, &g)
}

pub fn recorder_position(conn: &Connection) -> AppResult<Option<WindowPosition>> {
    repo_settings::get(conn, KEY_RECORDER_POSITION)
}

pub fn set_recorder_position(conn: &Connection, pos: WindowPosition) -> AppResult<()> {
    repo_settings::set(conn, KEY_RECORDER_POSITION, &pos)
}
