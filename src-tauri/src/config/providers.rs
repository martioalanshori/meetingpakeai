//! `providers.json` (PRD §9.6): model, batas free tier, parameter pipeline & rekaman.
//! File di app data dibaca saat start; key yang hilang diisi dari default; JSON rusak → default + warning.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Isi default, disalin ke app data saat pertama jalan.
pub const DEFAULT_JSON: &str = include_str!("../../resources/providers.default.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersConfig {
    pub version: u32,
    pub stt_model: String,
    pub llm_model: String,
    /// Field tambahan untuk body chat completions (mis. mematikan reasoning).
    pub llm_extra_body: Map<String, Value>,
    pub limits: Limits,
    pub pipeline: PipelineConfig,
    pub recording: RecordingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limits {
    pub stt_rpm: u32,
    pub stt_rpd: u32,
    pub stt_audio_sec_per_hour: u32,
    pub stt_audio_sec_per_day: u32,
    pub llm_rpm: u32,
    pub llm_rpd: u32,
    pub llm_tpm: u32,
    pub llm_tpd: u32,
    pub safety_factor: f64,
    pub stt_max_file_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub stt_chunk_target_sec: u32,
    pub llm_chunk_max_tokens: u32,
    pub no_speech_prob_max: f64,
    pub avg_logprob_min: f64,
    pub compression_ratio_max: f64,
    pub dedup_similarity: f64,
    pub hallucination_phrases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    pub auto_stop_silence_min: u32,
    pub max_recording_hours: u32,
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        serde_json::from_str(DEFAULT_JSON).expect("providers.default.json harus valid")
    }
}

impl ProvidersConfig {
    /// Muat dari `path`; buat file dari default jika belum ada.
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            if let Err(e) = std::fs::write(path, DEFAULT_JSON) {
                tracing::warn!("gagal menulis providers.json default: {e}");
            }
            return Self::default();
        }
        let user: Value = match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("providers.json rusak, pakai default: {e}");
                return Self::default();
            }
        };
        let mut merged: Value = serde_json::from_str(DEFAULT_JSON).expect("default valid");
        // llm_extra_body diganti utuh (bukan digabung) agar pengguna bisa menghapus parameter default.
        let mut user = user;
        if let Some(extra) = user.as_object_mut().and_then(|o| o.remove("llm_extra_body")) {
            merged["llm_extra_body"] = extra;
        }
        merge(&mut merged, user);
        serde_json::from_value(merged).unwrap_or_else(|e| {
            tracing::warn!("providers.json tidak sesuai skema, pakai default: {e}");
            Self::default()
        })
    }
}

/// Timpa `base` dengan nilai dari `over` secara rekursif (objek digabung, nilai lain diganti).
fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(slot) => merge(slot, v),
                    None => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (slot, v) => *slot = v,
    }
}
