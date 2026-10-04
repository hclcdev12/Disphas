//! What this build trusts: root keys and revoked licenses. Shared by the
//! history, the verifier and PDF rendering so they never disagree.

use oblivio_attestation::Revocation;
use oblivio_attestation::ed25519_dalek::VerifyingKey;

/// Root keys (official first) and revocations.
#[derive(Debug, Clone)]
pub struct Trust {
    /// Trusted roots, the official one first.
    pub roots: Vec<VerifyingKey>,
    /// The official root, if this build has one.
    pub official: Option<VerifyingKey>,
    /// Revoked licenses.
    pub revocations: Vec<Revocation>,
}

impl Trust {
    /// The roots and the revocation list shipped with this version. Fails
    /// when the shipped list does not verify: nothing is then called
    /// authentic (fail closed).
    pub fn shipped() -> Result<Self, String> {
        let official = oblivio_attestation::trust::official_root_key();
        let mut roots = oblivio_license::trusted_roots();
        roots.sort_by_key(|root| Some(*root) != official);
        let revocations = oblivio_license::trusted_revocations()
            .map_err(|error| format!("revocation list: {error}"))?;
        Ok(Self {
            roots,
            official,
            revocations,
        })
    }
}
