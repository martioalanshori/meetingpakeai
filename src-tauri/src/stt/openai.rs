//! STT kompatibel OpenAI (PRD §9.2): `POST {base}/audio/transcriptions`, verbose_json + segment timestamps.
//! Dipakai untuk Groq, OpenAI (`whisper-1`), dan server lokal (mis. faster-whisper-server).

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use super::{SttProvider, SttRequest, SttSegment};
use crate::ai_http::{self, ProviderError};

const TIMEOUT: Duration = Duration::from_secs(120);
const PROMPT_MAX_CHARS: usize = 800;

pub struct OpenAiStt {
    pub http: reqwest::Client,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
}

#[derive(Deserialize)]
struct VerboseJson {
    #[serde(default)]
    segments: Vec<RawSegment>,
    /// Server yang tidak mengirim `segments`: seluruh teks jadi satu segment sepanjang `duration`.
    #[serde(default)]
    text: String,
    #[serde(default)]
    duration: f64,
}

#[derive(Deserialize)]
struct RawSegment {
    start: f64,
    end: f64,
    #[serde(default)]
    text: String,
    // Field hilang → nilai netral 0 / 0 / 1 (PRD §9.2).
    #[serde(default)]
    no_speech_prob: f64,
    #[serde(default)]
    avg_logprob: f64,
    #[serde(default = "one")]
    compression_ratio: f64,
}

fn one() -> f64 {
    1.0
}

#[async_trait]
impl SttProvider for OpenAiStt {
    async fn transcribe(&self, req: SttRequest) -> Result<Vec<SttSegment>, ProviderError> {
        let bytes = tokio::fs::read(&req.wav_path)
            .await
            .map_err(|e| ProviderError::BadRequest(format!("file chunk tidak bisa dibaca: {e}")))?;
        let file = reqwest::multipart::Part::bytes(bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| ProviderError::BadRequest(e.to_string()))?;
        let mut form = reqwest::multipart::Form::new()
            .part("file", file)
            .text("model", self.model.clone())
            .text("response_format", "verbose_json")
            .text("timestamp_granularities[]", "segment")
            .text("temperature", "0");
        // "auto" → field language tidak dikirim (AC F9.2).
        if let Some(lang) = req.language {
            form = form.text("language", lang);
        }
        if let Some(p) = req.prompt.filter(|p| !p.trim().is_empty()) {
            form = form.text("prompt", p.chars().take(PROMPT_MAX_CHARS).collect::<String>());
        }
        let req = self.http.post(format!("{}/audio/transcriptions", self.base_url.trim_end_matches('/')));
        let resp = ai_http::with_key(req, self.api_key.as_deref()).timeout(TIMEOUT).multipart(form).send().await?;
        let body: VerboseJson = ai_http::check_status(resp)
            .await?
            .json()
            .await
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;
        if body.segments.is_empty() && !body.text.trim().is_empty() {
            return Ok(vec![SttSegment {
                start_s: 0.0,
                end_s: body.duration,
                text: body.text,
                no_speech_prob: 0.0,
                avg_logprob: 0.0,
                compression_ratio: 1.0,
            }]);
        }
        Ok(body
            .segments
            .into_iter()
            .map(|s| SttSegment {
                start_s: s.start,
                end_s: s.end,
                text: s.text,
                no_speech_prob: s.no_speech_prob,
                avg_logprob: s.avg_logprob,
                compression_ratio: s.compression_ratio,
            })
            .collect())
    }
}
