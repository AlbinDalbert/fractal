use std::fmt;
use std::error::Error;

#[derive(Debug)]
pub enum FracFormatError { 
    InvalidData(String),
    Io(std::io::Error),
    MissingField(String),
    InvalidHeader(String),
    UnsupportedFeature(String),
}

impl fmt::Display for FracFormatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FracFormatError::InvalidData(msg) => write!(f, "Invalid data for format: {}", msg),
            FracFormatError::Io(err) => write!(f, "Serialization I/O error: {}", err),
            FracFormatError::MissingField(field) => write!(f, "Required field is missing: {}", field),
            FracFormatError::InvalidHeader(msg) => write!(f, "Invalid Header: {}", msg),
            FracFormatError::UnsupportedFeature(msg) => write!(f, "Unsupported feature: {}", msg),
        }
    }
}


impl Error for FracFormatError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            FracFormatError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for FracFormatError {
    fn from(err: std::io::Error) -> Self {
        FracFormatError::Io(err)
    }
}
