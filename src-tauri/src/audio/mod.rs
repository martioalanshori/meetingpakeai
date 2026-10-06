//! Capture audio (PRD §7). Format internal: 16 kHz, mono, PCM 16-bit.

pub mod capture;
pub mod devices;
pub mod writer;

use serde::{Deserialize, Serialize};

pub const SAMPLE_RATE: u32 = 16_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// Suara pengguna (default capture device).
    Mic,
    /// Audio sistem lewat loopback (peserta lain).
    System,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mic => "mic",
            Self::System => "system",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("tidak ada device input")]
    NoInputDevice,
    #[error("tidak ada device output")]
    NoOutputDevice,
    /// Device dicabut / diganti (AUDCLNT_E_DEVICE_INVALIDATED); bisa dibuka ulang.
    #[error("device tidak valid lagi")]
    DeviceInvalidated,
    #[error("wasapi: {0}")]
    Wasapi(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

const AUDCLNT_E_DEVICE_INVALIDATED: i32 = 0x8889_0004_u32 as i32;
const E_NOTFOUND: i32 = 0x8007_0490_u32 as i32;

impl AudioError {
    pub(crate) fn from_wasapi(e: wasapi::WasapiError, channel: Channel) -> Self {
        if let wasapi::WasapiError::Windows(w) = &e {
            match w.code().0 {
                AUDCLNT_E_DEVICE_INVALIDATED => return Self::DeviceInvalidated,
                E_NOTFOUND => {
                    return match channel {
                        Channel::Mic => Self::NoInputDevice,
                        Channel::System => Self::NoOutputDevice,
                    }
                }
                _ => {}
            }
        }
        Self::Wasapi(e.to_string())
    }
}

impl From<AudioError> for crate::error::AppError {
    fn from(e: AudioError) -> Self {
        use crate::error::{AppError, ErrorCode};
        match e {
            AudioError::NoInputDevice => ErrorCode::NoInputDevice.into(),
            AudioError::NoOutputDevice => ErrorCode::NoOutputDevice.into(),
            other => AppError::internal(format!("audio: {other}")),
        }
    }
}
