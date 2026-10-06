//! Groq Chat Completions (PRD §9.3).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Map, Value};

use super::{LlmProvider, LlmRequest, LlmResponse};
use crate::groq::{self, ProviderError};

const TIMEOUT: Duration = Duration::from_secs(90);

pub struct GroqLlm {
    pub http: reqwest::Client,
    pub api_key: String,
    pub model: String,
    /// `llm_extra_body` dari providers.json (mis. mematikan reasoning).
    pub extra_body: Map<String, Value>,
    /// Dimatikan otomatis jika model menolak parameter tambahan (400).
    extras_enabled: AtomicBool,
}

impl GroqLlm {
    pub fn new(http: reqwest::Client, api_key: String, model: String, extra_body: Map<String, Value>) -> Self {
        Self { http, api_key, model, extra_body, extras_enabled: AtomicBool::new(true) }
    }

    fn body(&self, req: &LlmRequest, with_extras: bool) -> Value {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": req.messages,
            "temperature": req.temperature,
            "max_tokens": req.max_tokens,
        });
        if with_extras {
            for (k, v) in &self.extra_body {
                body[k] = v.clone();
            }
            body["response_format"] = serde_json::json!({ "type": "json_object" });
        }
        body
    }

    async fn send(&self, body: &Value) -> Result<LlmResponse, ProviderError> {
        let resp = self
            .http
            .post(format!("{}/chat/completions", groq::BASE_URL))
            .bearer_auth(&self.api_key)
            .timeout(TIMEOUT)
            .json(body)
            .send()
            .await?;
        let parsed: ChatResponse = groq::check_status(resp)
            .await?
            .json()
            .await
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| ProviderError::InvalidResponse("choices kosong".into()))?;
        Ok(LlmResponse {
            content,
            prompt_tokens: parsed.usage.as_ref().map_or(0, |u| u.prompt_tokens),
            completion_tokens: parsed.usage.as_ref().map_or(0, |u| u.completion_tokens),
        })
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: Msg,
}

#[derive(Deserialize)]
struct Msg {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[async_trait]
impl LlmProvider for GroqLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, ProviderError> {
        let with_extras = self.extras_enabled.load(Ordering::Relaxed);
        match self.send(&self.body(&req, with_extras)).await {
            // Model menolak parameter tambahan (reasoning_effort / response_format) → ulang tanpa itu.
            Err(ProviderError::BadRequest(msg))
                if with_extras && (msg.contains("reasoning") || msg.contains("response_format") || msg.contains("json")) =>
            {
                tracing::warn!("model menolak parameter tambahan, ulang tanpa extra body");
                self.extras_enabled.store(false, Ordering::Relaxed);
                self.send(&self.body(&req, false)).await
            }
            other => other,
        }
    }

    fn model(&self) -> &str {
        &self.model
    }
}
