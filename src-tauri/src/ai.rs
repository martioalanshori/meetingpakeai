//! Pilihan layanan AI per peran: Transkrip (STT) dan Ringkasan (LLM).
//! Semua penyedia memakai API kompatibel OpenAI; preset hanya mengisi alamat & model default.
//! Konfigurasi disimpan di tabel `settings` (`ai_stt`, `ai_llm`); API key per penyedia di Credential Manager.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config::providers::ProvidersConfig;
use crate::db::repo_settings;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::secrets;

pub const GROQ: &str = "groq";
pub const CUSTOM: &str = "custom";
const KEY_STT: &str = "ai_stt";
const KEY_LLM: &str = "ai_llm";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Stt,
    Llm,
}

/// Preset penyedia. `stt_model`/`llm_model` = `None` → tidak mendukung peran itu.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub base_url: &'static str,
    pub stt_model: Option<&'static str>,
    pub llm_model: Option<&'static str>,
    /// Halaman pembuatan API key.
    pub key_url: Option<&'static str>,
    /// Awalan key untuk deteksi dari clipboard.
    pub key_prefix: Option<&'static str>,
    pub key_required: bool,
}

pub const PRESETS: &[Preset] = &[
    Preset {
        id: GROQ,
        name: "Groq",
        base_url: "https://api.groq.com/openai/v1",
        // Model Groq diambil dari providers.json (lihat `default_model`).
        stt_model: Some(""),
        llm_model: Some(""),
        key_url: Some("https://console.groq.com/keys"),
        key_prefix: Some("gsk_"),
        key_required: true,
    },
    Preset {
        id: "openai",
        name: "OpenAI",
        base_url: "https://api.openai.com/v1",
        // whisper-1: satu-satunya model OpenAI yang mengembalikan segment bertimestamp (verbose_json).
        stt_model: Some("whisper-1"),
        llm_model: Some("gpt-4o-mini"),
        key_url: Some("https://platform.openai.com/api-keys"),
        key_prefix: Some("sk-"),
        key_required: true,
    },
    Preset {
        id: "openrouter",
        name: "OpenRouter",
        base_url: "https://openrouter.ai/api/v1",
        stt_model: None,
        llm_model: Some("openai/gpt-4o-mini"),
        key_url: Some("https://openrouter.ai/keys"),
        key_prefix: Some("sk-or-"),
        key_required: true,
    },
    Preset {
        id: "gemini",
        name: "Google Gemini",
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        stt_model: None,
        llm_model: Some("gemini-2.5-flash"),
        key_url: Some("https://aistudio.google.com/apikey"),
        key_prefix: Some("AIza"),
        key_required: true,
    },
    Preset {
        id: CUSTOM,
        name: "Kustom (kompatibel OpenAI)",
        base_url: "http://localhost:11434/v1",
        stt_model: Some(""),
        llm_model: Some(""),
        key_url: None,
        key_prefix: None,
        key_required: false,
    },
];

pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Penyedia + alamat + model untuk satu peran.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub provider: String,
    pub base_url: String,
    pub model: String,
}

impl Endpoint {
    pub fn is_groq(&self) -> bool {
        self.provider == GROQ
    }
}

fn default_model(role: Role, p: &Preset, cfg: &ProvidersConfig) -> String {
    if p.id == GROQ {
        return match role {
            Role::Stt => cfg.stt_model.clone(),
            Role::Llm => cfg.llm_model.clone(),
        };
    }
    match role {
        Role::Stt => p.stt_model,
        Role::Llm => p.llm_model,
    }
    .unwrap_or_default()
    .to_string()
}

/// Endpoint default sebuah penyedia untuk peran ini (dipakai UI saat penyedia diganti).
pub fn default_endpoint(role: Role, provider: &str, cfg: &ProvidersConfig) -> Option<Endpoint> {
    let p = preset(provider)?;
    let supported = match role {
        Role::Stt => p.stt_model.is_some(),
        Role::Llm => p.llm_model.is_some(),
    };
    supported.then(|| Endpoint { provider: p.id.into(), base_url: p.base_url.into(), model: default_model(role, p, cfg) })
}

fn key(role: Role) -> &'static str {
    match role {
        Role::Stt => KEY_STT,
        Role::Llm => KEY_LLM,
    }
}

/// Endpoint aktif; belum pernah diatur → Groq dengan model dari providers.json (perilaku lama).
pub fn endpoint(conn: &Connection, role: Role, cfg: &ProvidersConfig) -> AppResult<Endpoint> {
    let saved: Option<Endpoint> = repo_settings::get(conn, key(role))?;
    Ok(saved
        .filter(|e| preset(&e.provider).is_some())
        .unwrap_or_else(|| default_endpoint(role, GROQ, cfg).expect("preset groq ada")))
}

/// Validasi lalu simpan. Penyedia bawaan selalu memakai alamat preset-nya; kustom memakai alamat isian.
pub fn set_endpoint(conn: &Connection, role: Role, e: &Endpoint) -> AppResult<Endpoint> {
    let invalid = |msg: &str| AppError::with_message(ErrorCode::InvalidState, msg);
    let p = preset(&e.provider).ok_or_else(|| invalid("Penyedia tidak dikenal."))?;
    let supported = match role {
        Role::Stt => p.stt_model.is_some(),
        Role::Llm => p.llm_model.is_some(),
    };
    if !supported {
        return Err(invalid(&format!("{} tidak menyediakan transkrip audio.", p.name)));
    }
    let model = e.model.trim();
    if model.is_empty() {
        return Err(invalid("Nama model wajib diisi."));
    }
    let base_url = if p.id == CUSTOM { e.base_url.trim().trim_end_matches('/') } else { p.base_url };
    if !(base_url.starts_with("https://") || base_url.starts_with("http://")) {
        return Err(invalid("Alamat API harus diawali http:// atau https://."));
    }
    let clean = Endpoint { provider: p.id.into(), base_url: base_url.into(), model: model.into() };
    repo_settings::set(conn, key(role), &clean)?;
    Ok(clean)
}

/// API key untuk penyedia endpoint ini; `Err(NO_API_KEY)` jika wajib tapi belum ada.
pub fn api_key_for(e: &Endpoint) -> AppResult<Option<String>> {
    let key = secrets::get_key(&e.provider)?;
    let required = preset(&e.provider).is_some_and(|p| p.key_required);
    if required && key.is_none() {
        return Err(ErrorCode::NoApiKey.into());
    }
    Ok(key)
}

/// Penyedia aktif sebuah peran (tanpa perlu providers.json).
pub fn provider_of(conn: &Connection, role: Role) -> AppResult<String> {
    let saved: Option<Endpoint> = repo_settings::get(conn, key(role))?;
    Ok(saved.map(|e| e.provider).filter(|p| preset(p).is_some()).unwrap_or_else(|| GROQ.to_string()))
}

/// Kedua peran punya key yang dibutuhkan (onboarding, mulai rekam).
pub fn keys_ready(conn: &Connection) -> AppResult<bool> {
    for role in [Role::Stt, Role::Llm] {
        let e = Endpoint { provider: provider_of(conn, role)?, base_url: String::new(), model: String::new() };
        match api_key_for(&e) {
            Ok(_) => {}
            Err(err) if err.code == ErrorCode::NoApiKey => return Ok(false),
            Err(err) => return Err(err),
        }
    }
    Ok(true)
}

/// Tebak penyedia dari bentuk key (deteksi clipboard). Awalan terpanjang menang (`sk-or-` sebelum `sk-`).
pub fn provider_for_key(key: &str) -> Option<&'static str> {
    PRESETS
        .iter()
        .filter_map(|p| p.key_prefix.filter(|pre| key.starts_with(pre)).map(|pre| (pre.len(), p.id)))
        .max_by_key(|(len, _)| *len)
        .map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tebak_penyedia_dari_key() {
        assert_eq!(provider_for_key("gsk_abc"), Some(GROQ));
        assert_eq!(provider_for_key("sk-or-v1-abc"), Some("openrouter"));
        assert_eq!(provider_for_key("sk-proj-abc"), Some("openai"));
        assert_eq!(provider_for_key("AIzaSyabc"), Some("gemini"));
        assert_eq!(provider_for_key("halo"), None);
    }
}
