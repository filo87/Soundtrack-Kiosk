use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::fmt;

pub const AUTH_REQUIRED: &str = "AUTH_REQUIRED";
pub const OFFLINE: &str = "OFFLINE";
pub const QUOTA: &str = "QUOTA";
pub const RATE_LIMITED: &str = "RATE_LIMITED";
pub const NOT_FOUND: &str = "NOT_FOUND";
pub const PRINT_FAILED: &str = "PRINT_FAILED";
pub const ADMIN_LOCKED: &str = "ADMIN_LOCKED";
pub const INVALID_INPUT: &str = "INVALID_INPUT";
pub const INTERNAL: &str = "INTERNAL";

/// Serializable error surface shared with the webview: `{ code, message }`.
#[derive(Debug, Clone)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn auth_required(message: impl Into<String>) -> Self {
        Self::new(AUTH_REQUIRED, message)
    }

    pub fn offline(message: impl Into<String>) -> Self {
        Self::new(OFFLINE, message)
    }

    pub fn quota(message: impl Into<String>) -> Self {
        Self::new(QUOTA, message)
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::new(RATE_LIMITED, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(NOT_FOUND, message)
    }

    pub fn print_failed(message: impl Into<String>) -> Self {
        Self::new(PRINT_FAILED, message)
    }

    pub fn admin_locked(message: impl Into<String>) -> Self {
        Self::new(ADMIN_LOCKED, message)
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(INVALID_INPUT, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(INTERNAL, message)
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", self.code)?;
        state.serialize_field("message", &self.message)?;
        state.end()
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::internal(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::internal(value.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(value: tauri::Error) -> Self {
        Self::internal(value.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        if value.is_connect() || value.is_timeout() || value.is_request() {
            Self::offline(value.to_string())
        } else {
            Self::internal(value.to_string())
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
