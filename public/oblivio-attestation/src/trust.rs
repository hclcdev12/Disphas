//! The Oblivio root of trust: the official root public key, the only key
//! certificates and licenses ultimately chain to (`docs/certificat.md` §4).
//!
//! Generated offline with `oblivio-issuer keygen`; only the public half is
//! published here. Until it exists, release builds trust no root at all.

use ed25519_dalek::VerifyingKey;

/// The official root public key. Not generated yet.
pub const OFFICIAL_ROOT_KEY: Option<[u8; 32]> = None;

/// The official root public key, if this version has one.
pub fn official_root_key() -> Option<VerifyingKey> {
    OFFICIAL_ROOT_KEY.and_then(|bytes| VerifyingKey::from_bytes(&bytes).ok())
}
