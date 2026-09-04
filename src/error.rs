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
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::Conflict, message)
    }
    /// Creates an error indicating that a version is unsupported.
    ///
    /// # Examples
    ///
    /// ```
    /// let error = FractalError::unsupported_version("Version 2 is unsupported");
    /// assert_eq!(error.to_string(), "Version 2 is unsupported");
    /// ```
    pub fn unsupported_version(message: impl Into<String>) -> Self {
    pub fn unsupported_version(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::UnsupportedVersion, message)
    }
    /// Creates an error indicating that recovery is required.
    ///
    /// # Examples
    ///
    /// ```
    /// let error = FractalError::recovery_required("Restore the previous state");
    /// assert_eq!(error.to_string(), "Restore the previous state");
    /// ```
    pub fn recovery_required(message: impl Into<String>) -> Self {
        Self::new(FractalErrorCode::RecoveryRequired, message)
    }
    /// Creates an indeterminate-outcome error with the specified message.
    ///
    /// # Examples
    ///
    /// ```
    /// let error = FractalError::indeterminate("The result could not be determined.");
    /// assert_eq!(error.to_string(), "The result could not be determined.");
    /// ```
    ///
    /// # Arguments
    ///
    /// * `message` - A description of why the outcome could not be determined.
    ///
    /// # Returns
    ///
    /// An error categorized as [`FractalErrorCode::Indeterminate`].
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
