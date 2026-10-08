//! Panggilan LLM langsung dari command (bukan lewat antrean worker): pesan tindak lanjut, ringkas sejauh
//! ini, tanya meeting, tanya semua meeting. Satu request, keluaran JSON, pemakaian token dicatat ke rate limiter.

use serde_json::Value;

use crate::ai::{self, Role};
use crate::config::providers::ProvidersConfig;
use crate::db::Db;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::llm::openai::OpenAiLlm;
use crate::llm::{parse, ChatMessage, LlmProvider, LlmRequest};

/// Batas kasar konteks yang dikirim (karakter); ±4–5 ribu token agar muat batas menit Groq.
pub const CONTEXT_MAX_CHARS: usize = 18_000;

/// Satu panggilan LLM yang wajib mengembalikan objek JSON.
pub async fn json_call(
    db: &Db,
    providers: &ProvidersConfig,
    http: &reqwest::Client,
    system: &str,
    user: String,
    max_tokens: u32,
) -> AppResult<Value> {
    let llm = {
        let conn = db.conn();
        let e = ai::endpoint(&conn, Role::Llm, providers)?;
        let key = ai::api_key_for(&e)?;
        let extra = if e.is_groq() { providers.llm_extra_body.clone() } else { Default::default() };
        OpenAiLlm::new(http.clone(), e.base_url.clone(), key, e.model.clone(), extra)
    };
    let messages = vec![ChatMessage::system(system), ChatMessage::user(user)];
    let resp = llm
        .complete(LlmRequest { messages, max_tokens, temperature: 0.2 })
        .await
        .map_err(|e| e.to_app_error())?;
    let used = i64::from(resp.prompt_tokens + resp.completion_tokens);
    if used > 0 {
        let cost = crate::queue::rate_limiter::Cost::Llm { tokens: used };
        let _ = crate::queue::rate_limiter::record(&db.conn(), cost, Some(used), crate::db::now_ms());
    }
    parse::extract_json(&resp.content)
        .map_err(|e| AppError::with_message(ErrorCode::Internal, format!("Jawaban AI tidak bisa dibaca: {e}")))
}

/// Field string dari JSON (trim; kosong jika tidak ada).
pub fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).map(str::trim).unwrap_or_default().to_string()
}

/// Field daftar string dari JSON.
pub fn list(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect())
        .unwrap_or_default()
}

/// Potong teks panjang: awal + akhir (bagian tengah diringkas tanda "…").
pub fn clamp_context(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max / 4).collect();
    let tail: String = s.chars().skip(n - (max - max / 4)).collect();
    format!("{head}\n[…]\n{tail}")
}
