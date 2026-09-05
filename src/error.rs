use serde::{Deserialize, Serialize};
use std::fmt;

/// An error returned by a Fractal operation.
///
/// `code` is intended for programmatic handling. `message` contains the
/// operation-specific detail intended for logs or users.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FractalError {
    pub code: FractalErrorCode,
    pub message: String,
}

impl FractalError {
    /// Creates an error with an explicit code and message.
    pub fn new(code: FractalErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    /// Creates an [`FractalErrorCode::InvalidInput`] error.
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::InvalidInput, message)
    }
    /// Creates an [`FractalErrorCode::InvalidProject`] error.
    pub fn invalid_project(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::InvalidProject, message)
    }
    /// Creates an [`FractalErrorCode::NotFound`] error.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::NotFound, message)
    }
    /// Creates an [`FractalErrorCode::AlreadyExists`] error.
    pub fn already_exists(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::AlreadyExists, message)
    }
    /// Creates an [`FractalErrorCode::Conflict`] error.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::Conflict, message)
    }
    /// Creates an [`FractalErrorCode::UnsupportedVersion`] error.
    pub fn unsupported_version(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::UnsupportedVersion, message)
    }
    /// Creates an [`FractalErrorCode::RecoveryRequired`] error.
    pub fn recovery_required(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::RecoveryRequired, message)
    }
    /// Creates an [`FractalErrorCode::Indeterminate`] error.
    pub fn indeterminate(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::Indeterminate, message)
    }
}

impl fmt::Display for FractalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for FractalError {}

impl From<std::io::Error> for FractalError {
    fn from(error: std::io::Error) -> Self {
        let code = match error.kind() {
            std::io::ErrorKind::AlreadyExists => FractalErrorCode::AlreadyExists,
            std::io::ErrorKind::NotFound => FractalErrorCode::NotFound,
            _ => FractalErrorCode::Io,
        };
        Self::new(code, error.to_string())
    }
}
impl From<serde_json::Error> for FractalError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(FractalErrorCode::Json, error.to_string())
    }
}
impl From<std::string::FromUtf8Error> for FractalError {
    fn from(error: std::string::FromUtf8Error) -> Self {
        Self::new(FractalErrorCode::Utf8, error.to_string())
    }
}
impl From<std::path::StripPrefixError> for FractalError {
    fn from(error: std::path::StripPrefixError) -> Self {
        Self::new(FractalErrorCode::Path, error.to_string())
    }
}

/// A stable category for a [`FractalError`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FractalErrorCode {
    AlreadyExists,
    Conflict,
    InvalidInput,
    InvalidProject,
    Indeterminate,
    Io,
    Json,
    NotFound,
    Path,
    RecoveryRequired,
    UnsupportedVersion,
    Utf8,
}
