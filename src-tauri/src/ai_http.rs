//! HTTP bersama untuk layanan AI kompatibel OpenAI (Groq, OpenAI, OpenRouter, Gemini, server lokal):
//! client, `ProviderError` (PRD §9.1), pemetaan status HTTP, daftar model.
//! Implementasi STT/LLM ada di `stt/openai.rs` dan `llm/openai.rs`; preset penyedia di `ai.rs`.

use std::time::Duration;

use reqwest::{Response, StatusCode};
use serde::Deserialize;

use crate::error::{AppError, ErrorCode};

const MODELS_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub enum ProviderError {
    Unauthorized,                                  // 401
    RateLimited { retry_after: Option<Duration> }, // 429
    PayloadTooLarge,                               // 413
    BadRequest(String),                            // 400/404/422
    Server(u16),                                   // 5xx
    Network(String),                               // timeout, DNS, koneksi
    InvalidResponse(String),                       // body tidak bisa di-parse
}

impl ProviderError {
    /// Untuk ditampilkan langsung ke UI (bukan untuk antrean; antrean menangani retry sendiri).
    pub fn to_app_error(&self) -> AppError {
        match self {
            Self::Unauthorized => ErrorCode::InvalidApiKey.into(),
            Self::RateLimited { .. } => ErrorCode::RateLimited.into(),
            Self::Network(_) | Self::Server(_) => ErrorCode::Network.into(),
            Self::PayloadTooLarge | Self::BadRequest(_) | Self::InvalidResponse(_) => {
                AppError::internal(format!("layanan AI: {self:?}"))
            }
        }
    }
}

impl From<reqwest::Error> for ProviderError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_decode() {
            Self::InvalidResponse(e.to_string())
        } else {
            // Pesan reqwest berisi URL, bukan header; aman untuk log.
            Self::Network(e.to_string())
        }
    }
}

pub fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("MeetingPakeAI/", env!("CARGO_PKG_VERSION")))
        // Gagal sambung cepat diketahui; durasi total diatur per request (unggahan besar butuh lama).
        .connect_timeout(Duration::from_secs(15))
        .build()
        .expect("reqwest client")
}

/// Catat header rate limit ke log debug (PRD §9.5). [VERIFIKASI] nama header.
pub fn log_rate_limit_headers(resp: &Response) {
    if tracing::enabled!(tracing::Level::DEBUG) {
        for (name, value) in resp.headers() {
            let n = name.as_str();
            if n.starts_with("x-ratelimit-") || n == "retry-after" {
                tracing::debug!("{n}: {}", value.to_str().unwrap_or("?"));
            }
        }
    }
}

/// Lolos jika 2xx; selain itu dipetakan ke `ProviderError`.
pub async fn check_status(resp: Response) -> Result<Response, ProviderError> {
    log_rate_limit_headers(&resp);
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map(Duration::from_secs_f64);
    let body = resp.text().await.unwrap_or_default();
    tracing::warn!("layanan AI HTTP {}", status.as_u16());
    Err(match status {
        StatusCode::UNAUTHORIZED => ProviderError::Unauthorized,
        StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimited { retry_after },
        StatusCode::PAYLOAD_TOO_LARGE => ProviderError::PayloadTooLarge,
        s if s.is_server_error() => ProviderError::Server(s.as_u16()),
        s => ProviderError::BadRequest(format!("HTTP {}: {}", s.as_u16(), truncate(&body, 300))),
    })
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

#[derive(Deserialize)]
struct ModelList {
    data: Vec<ModelInfo>,
}

#[derive(Deserialize)]
struct ModelInfo {
    id: String,
}

/// Tambah `Authorization: Bearer` hanya jika ada key (server lokal sering tanpa key).
pub fn with_key(req: reqwest::RequestBuilder, api_key: Option<&str>) -> reqwest::RequestBuilder {
    match api_key {
        Some(k) if !k.is_empty() => req.bearer_auth(k),
        _ => req,
    }
}

/// `GET {base}/models` → daftar id model yang tersedia untuk key ini (AC F2.2).
pub async fn list_models(client: &reqwest::Client, base_url: &str, api_key: Option<&str>) -> Result<Vec<String>, ProviderError> {
    let req = client.get(format!("{}/models", base_url.trim_end_matches('/'))).timeout(MODELS_TIMEOUT);
    let resp = with_key(req, api_key).send().await?;
    let list: ModelList = check_status(resp)
        .await?
        .json()
        .await
        .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;
    Ok(list.data.into_iter().map(|m| m.id).collect())
}
