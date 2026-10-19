// src-tauri/src/error.rs
//! Erreurs contextualisées (PB-023) : un code stable + un détail technique.
//! L'interface associe à chaque code une explication lisible et une action proposée.

use serde::Serialize;
use std::fmt;

pub mod codes {
    pub const INVALID_ARGUMENT: &str = "PB-E001";
    pub const INTERNAL: &str = "PB-E002";
    pub const BACKEND_UNAVAILABLE: &str = "PB-E101";
    pub const BACKEND_SESSION: &str = "PB-E102";
    pub const DEVICE_NOT_FOUND: &str = "PB-E201";
    pub const DEVICE_UNREADABLE: &str = "PB-E202";
    pub const REPORT_INVALID: &str = "PB-E203";
    pub const PROFILE_INVALID: &str = "PB-E301";
    pub const PROFILE_NOT_FOUND: &str = "PB-E302";
    pub const PROFILE_CORRUPT: &str = "PB-E303";
    pub const STORAGE: &str = "PB-E304";
    pub const EXPORT_FAILED: &str = "PB-E401";
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: &'static str,
    pub detail: String,
}

impl AppError {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub fn storage(err: std::io::Error) -> Self {
        Self::new(codes::STORAGE, err.to_string())
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for AppError {}

pub type AppResult<T> = Result<T, AppError>;
