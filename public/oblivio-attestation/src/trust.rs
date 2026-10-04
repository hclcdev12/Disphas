//! The Oblivio root of trust: the official root public key, the only key
//! certificates and licenses ultimately chain to (`docs/certificat.md` §4).
//!
//! Generated offline with `oblivio-issuer keygen`; only the public half is
//! published here.

use ed25519_dalek::VerifyingKey;

/// The official root public key, generated offline on 2026-10-04.
pub const OFFICIAL_ROOT_KEY: Option<[u8; 32]> = Some([
    0x90, 0x24, 0x19, 0x25, 0x11, 0x14, 0x69, 0xb6, 0x86, 0x76, 0xf8, 0x04, 0x15, 0x78, 0x35, 0x5a,
    0x9f, 0x3e, 0x0b, 0xf1, 0x4d, 0x84, 0xaf, 0xd8, 0x25, 0xf2, 0x47, 0x82, 0xc3, 0x50, 0x8a, 0x0b,
]);

/// The official root public key, if this version has one.
pub fn official_root_key() -> Option<VerifyingKey> {
    OFFICIAL_ROOT_KEY.and_then(|bytes| VerifyingKey::from_bytes(&bytes).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_root_key_is_a_valid_ed25519_point() {
        assert!(official_root_key().is_some());
    }
}
