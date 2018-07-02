//! # hdlc
//! Rust implementation of a High-level Data Link Control (HDLC) library with support of the
//! IEEE standard.
//!
//! ## Usage
//!
//! ### Encode packet
//! ```rust
//! extern crate hdlc;
//! use hdlc::{SpecialChars, encode};
//!
//! let msg: Vec<u8> = vec![0x01, 0x50, 0x00, 0x00, 0x00, 0x05, 0x80, 0x09];
//! let cmp: Vec<u8> = vec![0x7E, 0x01, 0x50, 0x00, 0x00, 0x00, 0x05, 0x80, 0x09, 0x7E];
//!
//! let result = encode(&msg, SpecialChars::default());
//!
//! assert!(result.is_ok());
//! assert_eq!(result.unwrap(), cmp);
//! ```
//!
//! ### Custom Special Characters
//! ```rust
//! extern crate hdlc;
//! use hdlc::{SpecialChars, encode};
//!
//! let msg: Vec<u8> = vec![0x01, 0x7E, 0x70, 0x50, 0x00, 0x05, 0x80, 0x09];
//! let cmp: Vec<u8> = vec![0x71, 0x01, 0x7E, 0x70, 0x50, 0x50, 0x00, 0x05, 0x80, 0x09, 0x71];
//! let chars = SpecialChars::new(0x71, 0x70, 0x51, 0x50);
//!
//! let result = encode(&msg, chars);
//!
//! assert!(result.is_ok());
//! assert_eq!(result.unwrap(), cmp)
//! ```
//!
//! ### Decode packet
//! ```rust
//! extern crate hdlc;
//! use hdlc::{SpecialChars, decode};
//!
//! let chars = SpecialChars::default();
//! let msg: Vec<u8> = vec![
//!     chars.fend, 0x01, 0x50, 0x00, 0x00, 0x00, 0x05, 0x80, 0x09, chars.fend,
//! ];
//! let cmp: Vec<u8> = vec![0x01, 0x50, 0x00, 0x00, 0x00, 0x05, 0x80, 0x09];
//!
//! let result = decode(&msg, chars);
//!
//! assert!(result.is_ok());
//! assert_eq!(result.unwrap(), cmp);
//! ```

#![deny(missing_docs)]

use std::collections::HashSet;
use std::default::Default;
use std::error::Error;
use std::fmt;

/// Special Character structure for holding the encode and decode values
///
/// # Default
///
/// * **FEND**  = 0x7E;
/// * **FESC**  = 0x7D;
/// * **TFEND** = 0x5E;
/// * **TFESC** = 0x5D;
#[derive(Debug)]
pub struct SpecialChars {
    /// Frame END. Byte that marks the beginning and end of a packet
    pub fend: u8,
    /// Frame ESCape. Byte that marks the start of a swap byte
    pub fesc: u8,
    /// Trade Frame END. Byte that is substituted for the FEND byte
    pub tfend: u8,
    /// Trade Frame ESCape. Byte that is substituted for the FESC byte
    pub tfesc: u8,
}

impl Default for SpecialChars {
    /// Creates the default SpecialChars structure for encoding/decoding a packet
    fn default() -> SpecialChars {
        SpecialChars {
            fend: 0x7E,
            fesc: 0x7D,
            tfend: 0x5E,
            tfesc: 0x5D,
        }
    }
}
impl SpecialChars {
    /// Creates a new SpecialChars structure for encoding/decoding a packet
    pub fn new(fend: u8, fesc: u8, tfend: u8, tfesc: u8) -> SpecialChars {
        SpecialChars {
            fend,
            fesc,
            tfend,
            tfesc,
        }
    }
}

/// Produces unescaped (decoded) message without `FEND` characters.
///
/// # Inputs
/// * **Vec<u8>**: A vector of the bytes you want to decode
/// * **SpecialChars**: The special characters you want to swap
///
/// # Output
///
/// * **Result<Vec<u8>>**: Decoded output message
///
/// # Error
///
/// * **HDLCError::DuplicateSpecialChar**: Checks special characters for duplicates, if any of
/// the `SpecialChars` are duplicate, throw an error.  Displays "Duplicate special character".
/// * **HDLCError::SyncCharInData**: Checks to make sure the full decoded message is the full
/// length.  Found the `SpecialChars::fend` inside the message.
/// * **HDLCError::MissingTradeChar**: Checks to make sure every frame escape character `fesc`
/// is followed by either a `tfend` or a `tfesc`.
/// * **HDLCError::MissingFinalFEND**: Input vector is missing a final `SpecialChars::fend`
///
/// # Todo
///
/// Catch more errors, like an incomplete packet
///
/// # Example
/// ```rust
/// extern crate hdlc;
/// let chars = hdlc::SpecialChars::default();
/// let input: Vec<u8> = vec![ 0x7E, 0x01, 0x50, 0x00, 0x00, 0x00, 0x05, 0x80, 0x09, 0x7E];
/// let op_vec = hdlc::decode(&input.to_vec(), chars);
/// ```
pub fn decode(input: &Vec<u8>, s_chars: SpecialChars) -> Result<Vec<u8>, HDLCError> {
    let mut set = HashSet::new();
    if !set.insert(s_chars.fend)
        || !set.insert(s_chars.fesc)
        || !set.insert(s_chars.tfend)
        || !set.insert(s_chars.tfesc)
    {
        return Err(
            HDLCError::DuplicateSpecialChar,
        );
    }

    let mut sync = 0;
    let mut last_was_fesc = 0;
    let input_length = input.len();
    let mut output: Vec<u8> = Vec::with_capacity(input_length);

    for byte in input {
        // Handle the special escape characters
        if last_was_fesc > 0 {
            if *byte == s_chars.tfesc {
                output.push(s_chars.fesc);
            } else if *byte == s_chars.tfend {
                output.push(s_chars.fend);
            } else {
                return Err(HDLCError::MissingTradeChar);
            }
            last_was_fesc = 0
        } else {
            // Match based on the special characters, but struct fields are not patterns and cant match
            if *byte == s_chars.fend {
                // If we are already synced, this is the closing sync char
                if sync > 0 {
                    // Check to make sure the full message was decoded
                    if output.len() < ((input.len() / 2) - 1) {
                        return Err(
                            HDLCError::SyncCharInData,
                        );
                    }
                    return Ok(output);

                // Todo: Maybe save for a 2nd message?  I currently throw an error above
                } else {
                    sync = 1;
                }
            } else if *byte == s_chars.fesc {
                last_was_fesc = 1;
            } else {
                if sync > 0 {
                    output.push(*byte);
                }
            }
        }
    }

    // Missing a final sync character
    return Err(
        HDLCError::MissingFinalFEND,
    );
}

/// Produces escaped (encoded) message surrounded with `FEND`
///
/// # Inputs
/// * **Vec<u8>**: A vector of the bytes you want to encode
/// * **SpecialChars**: The special characters you want to swap
///
/// # Output
///
/// * **Result<Vec<u8>>**: Encoded output message
///
/// # Error
///
/// * **HDLCError::DuplicateSpecialChar**: Checks special characters for duplicates, if any of
/// the `SpecialChars` are duplicate, throw an error.  Displays "Duplicate special character".
///
/// # Todo
///
/// Catch more errors, like an incomplete packet
///
/// # Example
/// ```rust
/// extern crate hdlc;
/// let chars = hdlc::SpecialChars::default();
/// let input: Vec<u8> = vec![0x01, 0x50, 0x00, 0x00, 0x00, 0x05, 0x80, 0x09];
/// let op_vec = hdlc::encode(&input.to_vec(), chars);
/// ```
pub fn encode(data: &Vec<u8>, s_chars: SpecialChars) -> Result<Vec<u8>, HDLCError> {
    // Safety check to make sure the special character values are all unique
    let mut set = HashSet::new();
    if !set.insert(s_chars.fend)
        || !set.insert(s_chars.fesc)
        || !set.insert(s_chars.tfend)
        || !set.insert(s_chars.tfesc)
    {
        return Err(
            HDLCError::DuplicateSpecialChar
        );
    }

    let mut output = Vec::with_capacity(data.len() * 2); // *2 is the max size it can be if EVERY char is swapped

    output.push(s_chars.fend);

    // As of 4/24/18 Stuct fields are not patterns and cannot be match arms.
    for i in data {
        if *i == s_chars.fend {
            output.push(s_chars.fesc);
            output.push(s_chars.tfend);
        } else if *i == s_chars.fesc {
            output.push(s_chars.fesc);
            output.push(s_chars.tfesc);
        } else {
            output.push(*i);
        }
    }

    // Wrap the message in FENDs and return
    output.push(s_chars.fend);
    Ok(output)
}

#[derive(Debug, PartialEq)]
/// Common error for HDLC actions.
pub enum HDLCError {
    /// Catches duplicate special characters.
    DuplicateSpecialChar,
    /// Catches a random sync char in the data.
    SyncCharInData,
    /// Catches a random swap char, `fesc`, in the data with no `tfend` or `tfesc`.
    MissingTradeChar,
    /// No final fend on the message.
    MissingFinalFEND,
}

impl fmt::Display for HDLCError {
    /// Formats the output for the error using the given formatter.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            HDLCError::DuplicateSpecialChar => write!(f, "Caught a duplicate special character."),
            HDLCError::SyncCharInData => write!(f, "Caught a random sync char in the data."),
            HDLCError::MissingTradeChar => write!(f, "Caught a random swap char in the data."),
            HDLCError::MissingFinalFEND => write!(f, "Missing final FEND character."),
        }
    }
}

impl Error for HDLCError {
    /// Returns a short description of the error.
    fn description(&self) -> &str {
        match *self {
            HDLCError::DuplicateSpecialChar => "Caught a duplicate special character.",
            HDLCError::SyncCharInData => "Caught a random sync char in the data.",
            HDLCError::MissingTradeChar => "Caught a random swap char in the data.",
            HDLCError::MissingFinalFEND => "Missing final FEND character.",
        }
    }
}
