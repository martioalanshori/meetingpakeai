//! Hasil step job yang tidak sukses (PRD §9.5, §13).

use crate::error::AppError;

#[derive(Debug)]
pub enum StepError {
    /// Job `failed` dengan error ini.
    Failed(AppError),
    /// 401 / API key hilang: job `failed` + seluruh antrean dijeda.
    Unauthorized(AppError),
    /// `waiting_quota` sampai epoch ms ini.
    WaitingQuota(i64),
    /// `waiting_network` sampai epoch ms ini.
    WaitingNetwork(i64),
    /// Meeting dihapus saat diproses.
    Cancelled,
    /// File ditolak Groq karena terlalu besar (413); step transkripsi memecahnya.
    TooLarge,
}

impl From<AppError> for StepError {
    fn from(e: AppError) -> Self {
        if e.code == crate::error::ErrorCode::NotFound {
            Self::Cancelled
        } else {
            Self::Failed(e)
        }
    }
}

impl From<std::io::Error> for StepError {
    fn from(e: std::io::Error) -> Self {
        Self::Failed(AppError::from(e))
    }
}

pub type StepResult<T> = Result<T, StepError>;
