//! AppError + ErrorCode (PRD §12.1) dengan pesan UI dari §15.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    NoApiKey,
    InvalidApiKey,
    Network,
    RateLimited,
    QuotaExhausted,
    MicPermissionDenied,
    NoInputDevice,
    NoOutputDevice,
    AlreadyRecording,
    NotRecording,
    DiskFull,
    NotFound,
    InvalidState,
    AudioNotAvailable,
    LlmInvalidOutput,
    Internal,
}

impl ErrorCode {
    /// Kode persis seperti di DB / TS (`meetings.error_code`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoApiKey => "NO_API_KEY",
            Self::InvalidApiKey => "INVALID_API_KEY",
            Self::Network => "NETWORK",
            Self::RateLimited => "RATE_LIMITED",
            Self::QuotaExhausted => "QUOTA_EXHAUSTED",
            Self::MicPermissionDenied => "MIC_PERMISSION_DENIED",
            Self::NoInputDevice => "NO_INPUT_DEVICE",
            Self::NoOutputDevice => "NO_OUTPUT_DEVICE",
            Self::AlreadyRecording => "ALREADY_RECORDING",
            Self::NotRecording => "NOT_RECORDING",
            Self::DiskFull => "DISK_FULL",
            Self::NotFound => "NOT_FOUND",
            Self::InvalidState => "INVALID_STATE",
            Self::AudioNotAvailable => "AUDIO_NOT_AVAILABLE",
            Self::LlmInvalidOutput => "LLM_INVALID_OUTPUT",
            Self::Internal => "INTERNAL",
        }
    }

    /// Pesan untuk pengguna (PRD §15).
    pub fn message(self) -> &'static str {
        match self {
            Self::NoApiKey => "API key layanan AI belum diatur. Buka Pengaturan untuk menambahkannya.",
            Self::InvalidApiKey => "API key layanan AI tidak valid atau sudah dicabut. Perbarui di Pengaturan.",
            Self::Network => "Tidak bisa terhubung ke layanan AI. Periksa koneksi internet atau alamat API.",
            Self::RateLimited => "Batas kecepatan layanan AI tercapai. Proses akan dilanjutkan otomatis.",
            Self::QuotaExhausted => "Kuota harian layanan AI habis. Proses dilanjutkan otomatis besok.",
            Self::MicPermissionDenied => "Akses mikrofon diblokir Windows. Izinkan di Pengaturan Privasi.",
            Self::NoInputDevice => "Mikrofon tidak ditemukan.",
            Self::NoOutputDevice => "Perangkat audio output tidak ditemukan.",
            Self::AlreadyRecording => "Rekaman lain sedang berjalan.",
            Self::NotRecording => "Tidak ada rekaman yang sedang berjalan.",
            Self::DiskFull => "Ruang disk tidak cukup (minimal 1 GB).",
            Self::NotFound => "Meeting tidak ditemukan.",
            Self::InvalidState => "Aksi ini tidak bisa dilakukan pada status meeting saat ini.",
            Self::AudioNotAvailable => "Audio meeting ini sudah dihapus.",
            Self::LlmInvalidOutput => "Gagal membuat ringkasan. Coba buat ulang ringkasan.",
            Self::Internal => "Terjadi kesalahan. Detail tersimpan di log.",
        }
    }
}

/// Error yang dikirim ke UI: `{ code, message }`, message sudah Bahasa Indonesia.
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
}

impl AppError {
    pub fn new(code: ErrorCode) -> Self {
        Self { code, message: code.message().to_string() }
    }

    /// Error dengan pesan khusus (mis. pesan error dari provider untuk `failed`).
    pub fn with_message(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    /// INTERNAL: detail ditulis ke log, pengguna hanya melihat pesan umum.
    pub fn internal(detail: impl std::fmt::Display) -> Self {
        tracing::error!("internal error: {detail}");
        Self::new(ErrorCode::Internal)
    }
}

impl From<ErrorCode> for AppError {
    fn from(code: ErrorCode) -> Self {
        Self::new(code)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        Self::internal(format!("db: {e}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::internal(format!("io: {e}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::internal(format!("json: {e}"))
    }
}

pub type AppResult<T> = Result<T, AppError>;
