use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FractalError {
    pub code: FractalErrorCode,
    pub message: String,
}

impl FractalError {
    pub fn new(code: FractalErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::InvalidInput, message)
    }

    pub fn invalid_project(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::InvalidProject, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::NotFound, message)
    }

    pub fn already_exists(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::AlreadyExists, message)
    }

    pub fn unsupported_version(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::UnsupportedVersion, message)
    }
}

impl fmt::Display for FractalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for FractalError {}

impl From<&str> for FractalError {
    fn from(message: &str) -> Self {
        Self::invalid_input(message)
    }
}

impl From<String> for FractalError {
    fn from(message: String) -> Self {
        Self::invalid_input(message)
    }
}

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FractalErrorCode {
    AlreadyExists,
    InvalidInput,
    InvalidProject,
    Io,
    Json,
    NotFound,
    Path,
    UnsupportedVersion,
    Utf8,
}
