use std::io::{Read, Write};
use crate::errors::FracFormatError;

use crate::frac::IntermediateRep;

fn write_frac(ir: &IntermediateRep, path: &str) -> Result<Vec<u8>, FracFormatError> {
    
    Err(FracFormatError::InvalidData("unimplemented".to_string()))
}

fn read_frac(path: &str) -> Result<IntermediateRep, FracFormatError> {

    Err(FracFormatError::InvalidData("unimplemented".to_string()))
}
