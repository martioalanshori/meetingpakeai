//! API key layanan AI di Windows Credential Manager (PRD §6.1), satu entri per penyedia.
//! Tidak pernah ke DB, file, atau log.

use keyring::{Entry, Error as KeyringError};

use crate::error::{AppError, AppResult};

const SERVICE: &str = "com.meetingpakeai.desktop";

/// Nama entri. Groq memakai nama lama agar key yang sudah tersimpan tetap terbaca.
fn user(provider: &str) -> String {
    if provider == crate::ai::GROQ {
        "groq_api_key".into()
    } else {
        format!("api_key_{provider}")
    }
}

fn entry(provider: &str) -> AppResult<Entry> {
    Entry::new(SERVICE, &user(provider)).map_err(|e| AppError::internal(format!("keyring entry: {e:?}")))
}

pub fn get_key(provider: &str) -> AppResult<Option<String>> {
    match entry(provider)?.get_password() {
        Ok(k) if !k.trim().is_empty() => Ok(Some(k)),
        Ok(_) | Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(AppError::internal(format!("keyring get: {e:?}"))),
    }
}

pub fn set_key(provider: &str, key: &str) -> AppResult<()> {
    entry(provider)?
        .set_password(key)
        .map_err(|e| AppError::internal(format!("keyring set: {e:?}")))
}

pub fn delete_key(provider: &str) -> AppResult<()> {
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(e) => Err(AppError::internal(format!("keyring delete: {e:?}"))),
    }
}
