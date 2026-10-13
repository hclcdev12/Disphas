//! Pure decoders for the data returned by drive firmware.
//!
//! Bytes in, engine types out: no I/O. A buffer that is too short or fails an
//! integrity check is rejected, never interpreted.

pub mod ata;
pub mod nvme;

use thiserror::Error;

/// A firmware data structure cannot be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DecodeError {
    /// The buffer is shorter than the structure.
    #[error("{structure}: {actual} bytes, {expected} expected")]
    TooShort {
        /// Structure name.
        structure: &'static str,
        /// Required length.
        expected: usize,
        /// Received length.
        actual: usize,
    },
    /// The content is inconsistent.
    #[error("{structure}: {reason}")]
    Invalid {
        /// Structure name.
        structure: &'static str,
        /// What is wrong.
        reason: &'static str,
    },
}

fn require(data: &[u8], expected: usize, structure: &'static str) -> Result<(), DecodeError> {
    if data.len() < expected {
        return Err(DecodeError::TooShort {
            structure,
            expected,
            actual: data.len(),
        });
    }
    Ok(())
}

/// ASCII field, trimmed of padding; non-ASCII bytes are replaced.
fn ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| {
            if byte.is_ascii_graphic() || byte == b' ' {
                char::from(byte)
            } else {
                '?'
            }
        })
        .collect::<String>()
        .trim_matches(|c| c == ' ' || c == '?')
        .to_owned()
}

/// Identifier in lowercase hexadecimal, `None` when all zero.
fn hex_id(bytes: &[u8]) -> Option<String> {
    bytes
        .iter()
        .any(|&byte| byte != 0)
        .then(|| bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn le_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn le_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn le_u64(data: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&data[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

fn le_u128(data: &[u8], offset: usize) -> u128 {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&data[offset..offset + 16]);
    u128::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_trims_padding_and_replaces_garbage() {
        assert_eq!(ascii(b"  SN123   "), "SN123");
        assert_eq!(ascii(b"AB\x00\x00"), "AB");
        assert_eq!(ascii(b"A\xffB"), "A?B");
    }

    #[test]
    fn zero_identifiers_are_absent() {
        assert_eq!(hex_id(&[0; 8]), None);
        assert_eq!(hex_id(&[0, 0xab]).as_deref(), Some("00ab"));
    }
}
