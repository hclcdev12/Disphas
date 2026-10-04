//! The Oblivio root of trust: the official root public key, the only key
//! certificates and licenses ultimately chain to (`docs/certificat.md` §4).
//!
//! Generated offline with `oblivio-issuer keygen`; only the public half is
//! published here. The official revocation list, signed by that key, ships
//! with every version.

use ed25519_dalek::VerifyingKey;

use crate::revocation::{RevocationError, RevocationList};
use crate::verify::Revocation;

/// The official root public key, generated offline on 2026-10-04.
pub const OFFICIAL_ROOT_KEY: Option<[u8; 32]> = Some([
    0x90, 0x24, 0x19, 0x25, 0x11, 0x14, 0x69, 0xb6, 0x86, 0x76, 0xf8, 0x04, 0x15, 0x78, 0x35, 0x5a,
    0x9f, 0x3e, 0x0b, 0xf1, 0x4d, 0x84, 0xaf, 0xd8, 0x25, 0xf2, 0x47, 0x82, 0xc3, 0x50, 0x8a, 0x0b,
]);

/// The official root public key, if this version has one.
pub fn official_root_key() -> Option<VerifyingKey> {
    OFFICIAL_ROOT_KEY.and_then(|bytes| VerifyingKey::from_bytes(&bytes).ok())
}

/// The official revocation list shipped with this version (`OBR1:` text,
/// produced by `oblivio-issuer revoke`), or `None` while no license has ever
/// been revoked. To ship a new list: save it as `revocations.obr` next to
/// `Cargo.toml` and set `Some(include_str!("../revocations.obr"))`.
pub const OFFICIAL_REVOCATIONS: Option<&str> = None;

/// Licenses revoked by the official list of this version. An error means the
/// shipped list does not verify: callers must fail closed (never sign, never
/// call a certificate authentic) rather than ignore revocations.
pub fn official_revocations() -> Result<Vec<Revocation>, RevocationError> {
    let Some(text) = OFFICIAL_REVOCATIONS else {
        return Ok(Vec::new());
    };
    let root = official_root_key().ok_or(RevocationError::Format)?;
    Ok(RevocationList::decode(text, &root)?.entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_revocation_list_verifies() {
        assert!(official_revocations().is_ok());
    }

    #[test]
    fn official_root_key_is_a_valid_ed25519_point() {
        assert!(official_root_key().is_some());
    }
}
