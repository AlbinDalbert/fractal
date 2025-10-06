use std::io::{Read, Write, Seek, Cursor};
use crate::frac::*;
use crate::errors::FracFormatError;


// -- CORE SERIALIZATION LOGIC --
// -- Phase One --
//
pub fn serialize_ir(ir: &IntermediateRep) -> Result<Vec<u8>, FracFormatError> {
    // Calculate Body and Footnote content into temporary buffers


    // Calculate offsets and checksum


    // Assemble Header and write everything to a final Vec<u8>
    return Err(FracFormatError::InvalidData("unimplemented or unexpected".to_string()))
}


// -- CORE DESERIALIZATION LOGIC --
// -- Phase Two --
//
pub fn deserialize_ir(data: &[u8]) -> Result<IntermediateRep, FracFormatError> {
    let mut cursor = Cursor::new(data);
    // Read and validate Header


    // Jump to Body Offset, deserialize DocElm sequence


    // Jump to Footnotes Offset, deserialize footnotes
    return Err(FracFormatError::InvalidData("unimplemented or unexpected".to_string()))
}
