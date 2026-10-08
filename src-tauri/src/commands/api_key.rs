//! Command layanan AI: penyedia + model per peran (Transkrip / Ringkasan) dan API key per penyedia.

use serde::Serialize;
use tauri::State;

use crate::ai::{self, Endpoint, Preset, Role};
use crate::ai_http::{self, ProviderError};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::{secrets, AppState};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleConfig {
    #[serde(flatten)]
    pub endpoint: Endpoint,
    /// Key penyedia ini sudah tersimpan.
    pub key_set: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub stt: RoleConfig,
    pub llm: RoleConfig,
    pub presets: &'static [Preset],
    /// Penyedia yang API key-nya sudah tersimpan.
    pub keys_set: Vec<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAiResult {
    /// `false` jika server tidak menyediakan daftar model (`/models`), jadi model tidak bisa dicek.
    pub verified: bool,
    /// Model tidak ada di daftar model akun ini.
    pub model_missing: bool,
}

fn role_config(state: &AppState, role: Role) -> AppResult<RoleConfig> {
    let endpoint = ai::endpoint(&state.db.conn(), role, &state.providers)?;
    let key_set = secrets::get_key(&endpoint.provider)?.is_some();
    Ok(RoleConfig { endpoint, key_set })
}

/// Tambahan: penyedia aktif kedua peran + daftar preset.
#[tauri::command]
pub async fn get_ai_config(state: State<'_, AppState>) -> AppResult<AiConfig> {
    let mut keys_set = Vec::new();
    for p in ai::PRESETS {
        if secrets::get_key(p.id)?.is_some() {
            keys_set.push(p.id);
        }
    }
    Ok(AiConfig {
        stt: role_config(&state, Role::Stt)?,
        llm: role_config(&state, Role::Llm)?,
        presets: ai::PRESETS,
        keys_set,
    })
}

/// Tambahan: alamat & model default saat pengguna memilih penyedia lain.
#[tauri::command]
pub async fn default_ai_endpoint(state: State<'_, AppState>, role: Role, provider: String) -> AppResult<Endpoint> {
    ai::default_endpoint(role, &provider, &state.providers).ok_or_else(|| AppError::new(ErrorCode::InvalidState))
}

/// Tambahan: uji koneksi lalu simpan penyedia/model peran ini (+ key baru jika diisi).
/// Key ditolak (401) → `INVALID_API_KEY`, tidak ada yang disimpan.
#[tauri::command]
pub async fn save_ai_endpoint(
    state: State<'_, AppState>,
    role: Role,
    endpoint: Endpoint,
    key: Option<String>,
) -> AppResult<SaveAiResult> {
    let new_key = key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
    let provider = ai::preset(&endpoint.provider).ok_or_else(|| AppError::new(ErrorCode::InvalidState))?;
    let base_url = if provider.id == ai::CUSTOM { endpoint.base_url.trim().to_string() } else { provider.base_url.to_string() };
    let key = match &new_key {
        Some(k) => Some(k.clone()),
        None => secrets::get_key(provider.id)?,
    };
    if provider.key_required && key.is_none() {
        return Err(ErrorCode::NoApiKey.into());
    }

    let (verified, model_missing) = match ai_http::list_models(&state.http, &base_url, key.as_deref()).await {
        Ok(models) => (true, !models.iter().any(|m| m == endpoint.model.trim())),
        // Server tanpa `/models` (sebagian server lokal): simpan tanpa cek model.
        Err(ProviderError::BadRequest(_)) => (false, false),
        Err(e) => return Err(e.to_app_error()),
    };

    let saved = ai::set_endpoint(&state.db.conn(), role, &Endpoint { base_url, ..endpoint })?;
    if let Some(k) = &new_key {
        secrets::set_key(provider.id, k)?;
    }
    tracing::info!("layanan AI {:?}: {} / {} (diverifikasi: {verified})", role, saved.provider, saved.model);
    if model_missing {
        tracing::warn!("model {} tidak ada di daftar model {}", saved.model, saved.provider);
    }
    // Antrean yang dijeda karena key tidak valid dilanjutkan (PRD §9.5, AC F7.5).
    state.worker.resume_after_new_key();
    Ok(SaveAiResult { verified, model_missing })
}

/// Tambahan: hapus API key satu penyedia.
#[tauri::command]
pub async fn delete_ai_key(provider: String) -> AppResult<()> {
    if ai::preset(&provider).is_none() {
        return Err(AppError::new(ErrorCode::InvalidState));
    }
    secrets::delete_key(&provider)?;
    tracing::info!("api key {provider} dihapus");
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedKey {
    pub provider: &'static str,
    pub key: String,
}

/// Tambahan (langkah 24, diperluas): API key yang dikenali bentuknya di clipboard (Groq, OpenAI,
/// OpenRouter, Gemini). Teks clipboard lain tidak pernah dikembalikan ke UI.
#[tauri::command]
pub async fn detect_api_key_in_clipboard() -> AppResult<Option<DetectedKey>> {
    let text = crate::windows_integration::clipboard_text().unwrap_or_default();
    let key = text.trim();
    let shape_ok = (20..=300).contains(&key.len())
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !shape_ok {
        return Ok(None);
    }
    Ok(ai::provider_for_key(key).map(|provider| DetectedKey { provider, key: key.to_string() }))
}
