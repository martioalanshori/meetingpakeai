//! API key Groq di Windows Credential Manager (PRD §6.1). Tidak pernah ke DB, file, atau log.

use keyring::{Entry, Error as KeyringError};

use crate::error::{AppError, AppResult};

const SERVICE: &str = "com.meetingpakeai.desktop";
const USER: &str = "groq_api_key";

fn entry() -> AppResult<Entry> {
    Entry::new(SERVICE, USER).map_err(|e| AppError::internal(format!("keyring entry: {e:?}")))
}

pub fn get_api_key() -> AppResult<Option<String>> {
    match entry()?.get_password() {
        Ok(k) if !k.trim().is_empty() => Ok(Some(k)),
        Ok(_) | Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(AppError::internal(format!("keyring get: {e:?}"))),
    }
}

pub fn set_api_key(key: &str) -> AppResult<()> {
    entry()?
        .set_password(key)
        .map_err(|e| AppError::internal(format!("keyring set: {e:?}")))
}

pub fn delete_api_key() -> AppResult<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(e) => Err(AppError::internal(format!("keyring delete: {e:?}"))),
    }
}
