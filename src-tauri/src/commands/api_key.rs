use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::{groq, secrets, AppState};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestApiKeyResult {
    pub ok: bool,
    pub missing_models: Vec<String>,
}

#[tauri::command]
pub async fn save_api_key(state: State<'_, AppState>, key: String) -> AppResult<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidApiKey));
    }
    secrets::set_api_key(key)?;
    tracing::info!("api key disimpan");
    // Antrean yang dijeda karena key tidak valid dilanjutkan (PRD §9.5, AC F7.5).
    state.worker.resume_after_new_key();
    Ok(())
}

/// Tanpa `key` → pakai key tersimpan. Key ditolak Groq → error `INVALID_API_KEY`.
#[tauri::command]
pub async fn test_api_key(state: State<'_, AppState>, key: Option<String>) -> AppResult<TestApiKeyResult> {
    let key = match key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        Some(k) => k,
        None => secrets::get_api_key()?.ok_or_else(|| AppError::new(ErrorCode::NoApiKey))?,
    };
    let models = groq::list_models(&state.http, &key).await.map_err(|e| e.to_app_error())?;
    let missing_models: Vec<String> = [&state.providers.stt_model, &state.providers.llm_model]
        .into_iter()
        .filter(|m| !models.iter().any(|id| id == *m))
        .cloned()
        .collect();
    if !missing_models.is_empty() {
        tracing::warn!("model tidak tersedia di akun: {missing_models:?}");
    }
    Ok(TestApiKeyResult { ok: true, missing_models })
}

#[tauri::command]
pub async fn delete_api_key() -> AppResult<()> {
    secrets::delete_api_key()?;
    tracing::info!("api key dihapus");
    Ok(())
}

/// Tambahan (langkah 24): API key Groq (`gsk_…`) di clipboard, untuk ditawarkan ke pengguna.
/// Teks clipboard lain tidak pernah dikembalikan ke UI.
#[tauri::command]
pub async fn detect_api_key_in_clipboard() -> AppResult<Option<String>> {
    let text = crate::windows_integration::clipboard_text().unwrap_or_default();
    let key = text.trim();
    let looks_like_key = key.len() >= 24
        && key.len() <= 200
        && key.starts_with("gsk_")
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    Ok(looks_like_key.then(|| key.to_string()))
}
