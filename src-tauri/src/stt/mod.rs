//! Abstraksi Speech-to-Text (PRD §9.1).

pub mod openai;

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use crate::ai_http::ProviderError;

#[async_trait]
pub trait SttProvider: Send + Sync {
    async fn transcribe(&self, req: SttRequest) -> Result<Vec<SttSegment>, ProviderError>;
}

pub struct SttRequest {
    pub wav_path: PathBuf,
    pub language: Option<String>,
    pub prompt: Option<String>,
    /// `None` = 0 (deterministik); dipakai lebih tinggi saat mencoba ulang bagian ragu (langkah 56).
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttSegment {
    pub start_s: f64,
    pub end_s: f64,
    pub text: String,
    pub no_speech_prob: f64,
    pub avg_logprob: f64,
    pub compression_ratio: f64,
}
