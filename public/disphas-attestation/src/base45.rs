//! Base45 (RFC 9285), the QR-friendly encoding of the attestation.

use thiserror::Error;

const ALPHABET: &[u8; 45] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";

/// Invalid Base45 text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum Base45Error {
    /// A character outside the alphabet.
    #[error("invalid Base45 character")]
    InvalidCharacter,
    /// A length that no encoding can produce, or a group out of range.
    #[error("invalid Base45 group")]
    InvalidGroup,
}

/// Encodes `data` in Base45.
pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(2) * 3);
    let mut push = |value: usize| out.push(char::from(ALPHABET[value % 45]));
    for chunk in data.chunks(2) {
        match *chunk {
            [first, second] => {
                let value = usize::from(first) * 256 + usize::from(second);
                push(value);
                push(value / 45);
                push(value / (45 * 45));
            }
            [single] => {
                let value = usize::from(single);
                push(value);
                push(value / 45);
            }
            _ => {}
        }
    }
    out
}

/// Decodes Base45 text.
pub fn decode(text: &str) -> Result<Vec<u8>, Base45Error> {
    let digits = text
        .bytes()
        .map(|byte| {
            ALPHABET
                .iter()
                .position(|&symbol| symbol == byte)
                .ok_or(Base45Error::InvalidCharacter)
        })
        .collect::<Result<Vec<usize>, _>>()?;
    let mut out = Vec::with_capacity(digits.len() / 3 * 2 + 1);
    for group in digits.chunks(3) {
        match *group {
            [c, d, e] => {
                let value = c + d * 45 + e * 45 * 45;
                let pair = u16::try_from(value).map_err(|_| Base45Error::InvalidGroup)?;
                out.extend_from_slice(&pair.to_be_bytes());
            }
            [c, d] => {
                let value = c + d * 45;
                out.push(u8::try_from(value).map_err(|_| Base45Error::InvalidGroup)?);
            }
            _ => return Err(Base45Error::InvalidGroup),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test vectors of RFC 9285 §4.
    const VECTORS: &[(&str, &str)] = &[
        ("AB", "BB8"),
        ("Hello!!", "%69 VD92EX0"),
        ("base-45", "UJCLQE7W581"),
        ("ietf!", "QED8WEX0"),
    ];

    #[test]
    fn matches_rfc_9285_vectors() {
        for (plain, encoded) in VECTORS {
            assert_eq!(encode(plain.as_bytes()), *encoded);
            assert_eq!(decode(encoded).as_deref(), Ok(plain.as_bytes()));
        }
    }

    #[test]
    fn round_trips_every_byte_value() {
        let data: Vec<u8> = (0..=255).chain([0, 255, 7]).collect();
        assert_eq!(decode(&encode(&data)), Ok(data));
    }

    #[test]
    fn rejects_invalid_input() {
        assert_eq!(decode("GGW"), Err(Base45Error::InvalidGroup)); // 65535 + 1
        assert_eq!(decode("A"), Err(Base45Error::InvalidGroup));
        assert_eq!(decode("ab"), Err(Base45Error::InvalidCharacter));
    }
}
