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
pub async fn save_api_key(key: String) -> AppResult<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidApiKey));
    }
    secrets::set_api_key(key)?;
    tracing::info!("api key disimpan");
    // TODO langkah 13: lanjutkan antrean yang dijeda karena INVALID_API_KEY.
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
