//! License state as seen by the helper: status, activation, installation.
//!
//! Licenses are checked against `disphas_license::trusted_roots()`: the
//! official root key, plus the public development root in debug builds.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use disphas_attestation::Revocation;
use disphas_attestation::ed25519_dalek::VerifyingKey;
use disphas_license::{InstalledLicense, LicenseStatus, LicenseStore, WorkstationKey};
use disphas_protocol::response::{ErrorBody, ErrorCode, LicenseState, LicenseStatusResult};

/// License operations needed by the session.
pub trait Licensing: std::fmt::Debug {
    /// Current license status.
    fn status(&self) -> LicenseStatusResult;
    /// Creates the workstation key if needed and returns the activation request.
    fn activation_request(&self) -> Result<String, ErrorBody>;
    /// Verifies and installs a license text, then returns the new status.
    fn install(&self, text: &str) -> Result<LicenseStatusResult, ErrorBody>;
    /// Workstation key and signed license, to sign an attestation. Only
    /// available while the license is valid.
    fn signer(&self) -> Result<Signer, ErrorBody>;
}

/// What signs an attestation: the workstation key and its license.
#[derive(Debug)]
pub struct Signer {
    /// Workstation key, wiped from memory on drop.
    pub key: WorkstationKey,
    /// Signed license (root or shop key), embedded in the attestation.
    pub license: Vec<u8>,
}

/// Where keys and licenses are stored: see [`disphas_license::state_dir`].
pub fn state_dir() -> PathBuf {
    disphas_license::state_dir()
}

/// Licensing backed by the on-disk store and the system clock.
#[derive(Debug)]
pub struct StoreLicensing {
    store: LicenseStore,
    roots: Vec<VerifyingKey>,
    /// Revoked licenses, or why the shipped list does not verify.
    revocations: Result<Vec<Revocation>, String>,
}

impl StoreLicensing {
    /// Licensing over `store`, accepting licenses signed by any of `roots`
    /// (see `disphas_license::trusted_roots`).
    /// Revocations come from the official list shipped with this version.
    pub fn new(store: LicenseStore, roots: Vec<VerifyingKey>) -> Self {
        Self {
            store,
            roots,
            revocations: disphas_license::trusted_revocations(),
        }
    }

    /// Replaces the revocation list (tests).
    pub fn with_revocations(mut self, revocations: Result<Vec<Revocation>, String>) -> Self {
        self.revocations = revocations;
        self
    }

    /// Status of the installed license under the first root that verifies it.
    /// `None` when no root does (or this build trusts none).
    fn installed(&self) -> Option<LicenseStatus> {
        // A shipped list that does not verify: fail closed, never sign.
        let revocations = match &self.revocations {
            Ok(revocations) => revocations,
            Err(error) => {
                return Some(LicenseStatus::Invalid(format!("revocation list: {error}")));
            }
        };
        self.roots
            .iter()
            .find_map(|root| match self.store.status(root, now(), revocations) {
                Ok(LicenseStatus::Invalid(_)) | Err(_) => None,
                Ok(status) => Some(status),
            })
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn result(state: LicenseState, installed: Option<&InstalledLicense>) -> LicenseStatusResult {
    LicenseStatusResult {
        state,
        offer: installed.map(|installed| installed.license.offer.clone()),
        license_id: installed.map(|installed| installed.license.id.clone()),
        valid_until: installed.and_then(|installed| installed.license.expires),
        // Filled in by the session, which holds the entitlement table.
        entitlements: None,
    }
}

fn invalid(detail: impl Into<String>) -> ErrorBody {
    ErrorBody {
        code: ErrorCode::LicenseInvalid,
        detail: Some(detail.into()),
    }
}

impl Licensing for StoreLicensing {
    fn status(&self) -> LicenseStatusResult {
        match self.installed() {
            // Without a root key nothing can be verified: never Valid.
            None if self.roots.is_empty() => result(LicenseState::Missing, None),
            None | Some(LicenseStatus::Invalid(_)) => result(LicenseState::Invalid, None),
            Some(LicenseStatus::Missing) => result(LicenseState::Missing, None),
            Some(LicenseStatus::Valid(installed)) => result(LicenseState::Valid, Some(&installed)),
            Some(LicenseStatus::NotYetValid(installed)) => {
                result(LicenseState::NotYetValid, Some(&installed))
            }
            Some(LicenseStatus::Expired(installed)) => {
                result(LicenseState::Expired, Some(&installed))
            }
            Some(LicenseStatus::Revoked(installed, _)) => {
                result(LicenseState::Revoked, Some(&installed))
            }
        }
    }

    fn activation_request(&self) -> Result<String, ErrorBody> {
        self.store
            .create_activation_request(now())
            .map_err(|error| ErrorBody {
                code: ErrorCode::Internal,
                detail: Some(error.to_string()),
            })
    }

    fn install(&self, text: &str) -> Result<LicenseStatusResult, ErrorBody> {
        let mut last = invalid("this build has no root key to verify licenses");
        for root in &self.roots {
            match self.store.install(text, root, now()) {
                Ok(_) => return Ok(self.status()),
                Err(error) => last = invalid(error.to_string()),
            }
        }
        Err(last)
    }

    fn signer(&self) -> Result<Signer, ErrorBody> {
        let required = |detail: &str| ErrorBody {
            code: ErrorCode::LicenseRequired,
            detail: Some(detail.to_owned()),
        };
        let Some(LicenseStatus::Valid(installed)) = self.installed() else {
            return Err(required("no valid license installed"));
        };
        let key = self
            .store
            .workstation_key()
            .ok()
            .flatten()
            .ok_or_else(|| required("workstation key unavailable"))?;
        Ok(Signer {
            key,
            license: installed.signed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn licensing() -> (tempfile::TempDir, StoreLicensing) {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
            .expect("chmod");
        let licensing = StoreLicensing::new(
            LicenseStore::new(dir.path()),
            disphas_license::trusted_roots(),
        );
        (dir, licensing)
    }

    #[test]
    fn development_activation_round_trip() {
        let (_dir, licensing) = licensing();
        assert_eq!(licensing.status().state, LicenseState::Missing);
        let request = licensing.activation_request().expect("request");
        let license = disphas_license::dev_root::issue(&request, now(), 30).expect("issued");
        let status = licensing.install(&license).expect("installed");
        assert_eq!(status.state, LicenseState::Valid);
        assert_eq!(status.offer.as_deref(), Some("Dev"));
        assert_eq!(licensing.status(), status);
    }

    #[test]
    fn shop_license_installs_and_signs() {
        use disphas_attestation::ed25519_dalek::SigningKey;
        use disphas_attestation::{Issuer, License, cose};
        let (_dir, licensing) = licensing();
        let request = disphas_license::read_activation_request(
            &licensing.activation_request().expect("request"),
        )
        .expect("valid request");
        let shop = SigningKey::from_bytes(&[5; 32]);
        let issuer = Issuer {
            id: "ISS-DEV".to_owned(),
            key: shop.verifying_key().to_bytes(),
            not_before: now() - 86_400,
            expires: now() + 365 * 86_400,
            offers: vec!["Pro".to_owned()],
            max_duration: 90 * 86_400,
        };
        let license = License {
            id: "DSL-DEV-SHOP".to_owned(),
            customer: "Atelier XYZ".to_owned(),
            offer: "Pro".to_owned(),
            not_before: now() - 60,
            expires: Some(now() + 37 * 86_400),
            workstation_key: request.workstation_key,
            issuer: Some(
                cose::sign(issuer.to_cbor(), &disphas_license::dev_root::signing_key())
                    .expect("signed issuer"),
            ),
        };
        let signed = cose::sign(license.to_cbor(), &shop).expect("signed license");
        let status = licensing
            .install(&disphas_license::license_text(&signed))
            .expect("installed");
        assert_eq!(status.state, LicenseState::Valid);
        assert_eq!(status.license_id.as_deref(), Some("DSL-DEV-SHOP"));
        // The attestation will carry the shop-signed license as is.
        assert_eq!(licensing.signer().expect("signer").license, signed);
    }

    #[test]
    fn revoked_license_cannot_sign() {
        let (_dir, licensing) = licensing();
        let request = licensing.activation_request().expect("request");
        let license = disphas_license::dev_root::issue(&request, now(), 30).expect("issued");
        let id = licensing
            .install(&license)
            .expect("installed")
            .license_id
            .expect("id");
        let licensing = licensing.with_revocations(Ok(vec![Revocation {
            license_id: id,
            revoked_at: now() - 60,
        }]));
        assert_eq!(licensing.status().state, LicenseState::Revoked);
        assert_eq!(
            licensing.signer().expect_err("refused").code,
            ErrorCode::LicenseRequired
        );
    }

    #[test]
    fn unverifiable_revocation_list_fails_closed() {
        let (_dir, licensing) = licensing();
        let request = licensing.activation_request().expect("request");
        let license = disphas_license::dev_root::issue(&request, now(), 30).expect("issued");
        licensing.install(&license).expect("installed");
        let licensing = licensing.with_revocations(Err("bad signature".to_owned()));
        assert_eq!(licensing.status().state, LicenseState::Invalid);
        assert!(licensing.signer().is_err());
    }

    #[test]
    fn garbage_license_is_rejected_as_invalid() {
        let (_dir, licensing) = licensing();
        licensing.activation_request().expect("request");
        let error = licensing
            .install("DSL1:not a license")
            .expect_err("rejected");
        assert_eq!(error.code, ErrorCode::LicenseInvalid);
    }

    #[test]
    fn license_for_another_workstation_is_rejected() {
        let (_dir, licensing) = licensing();
        licensing.activation_request().expect("request");
        let (_other_dir, other) = self::licensing();
        let foreign = disphas_license::dev_root::issue(
            &other.activation_request().expect("request"),
            now(),
            30,
        )
        .expect("issued");
        assert_eq!(
            licensing.install(&foreign).expect_err("rejected").code,
            ErrorCode::LicenseInvalid
        );
    }

    #[test]
    fn without_root_key_nothing_is_valid() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let licensing = StoreLicensing::new(LicenseStore::new(dir.path()), Vec::new());
        assert_eq!(licensing.status().state, LicenseState::Missing);
        assert_eq!(
            licensing.install("DSL1:x").expect_err("rejected").code,
            ErrorCode::LicenseInvalid
        );
    }
}
