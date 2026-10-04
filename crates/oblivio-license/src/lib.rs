//! Offline license handling: workstation key, activation request, `.lic`
//! installation and status (`docs/cadre_projet.md` §3, `docs/certificat.md` §4).
//!
//! ```text
//! workstation ──OBA1: activation request (public key, self-signed)──► Oblivio
//! workstation ◄──────────────OBL1: license signed by the root key──── Oblivio
//! ```
//!
//! Layout under the state directory (normally `/var/lib/oblivio`, root only):
//! `keys/workstation.key` (0600) and `license/oblivio.lic`.

pub mod key;

/// The state directory: keys, license, jobs and reports
/// (`docs/protocole_ipc.md` §10). Shared by the helper and the app, which
/// reads `reports/` for the history.
///
/// Release builds always use `/var/lib/oblivio` and ignore the environment,
/// which pkexec clears anyway. Development builds run as the current user:
/// `OBLIVIO_STATE_DIR`, else `~/.local/state/oblivio-dev`.
pub fn state_dir() -> std::path::PathBuf {
    #[cfg(debug_assertions)]
    {
        if let Some(dir) = std::env::var_os("OBLIVIO_STATE_DIR") {
            return std::path::PathBuf::from(dir);
        }
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home).join(".local/state/oblivio-dev");
        }
    }
    std::path::PathBuf::from("/var/lib/oblivio")
}

/// Root keys licenses and certificates may chain to: the official root key;
/// in development builds also the public development root, so licenses of
/// both origins work there. Release builds trust the official key alone.
pub fn trusted_roots() -> Vec<VerifyingKey> {
    #[allow(unused_mut)]
    let mut roots: Vec<VerifyingKey> = oblivio_attestation::trust::official_root_key()
        .into_iter()
        .collect();
    #[cfg(debug_assertions)]
    roots.push(dev_root::verifying_key());
    roots
}

/// Public development root key: debug builds only, never in a release.
#[cfg(debug_assertions)]
pub mod dev_root;

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use oblivio_attestation::License;
use oblivio_attestation::base45;
use oblivio_attestation::cose::Unverified;
use oblivio_attestation::ed25519_dalek::VerifyingKey;
use oblivio_attestation::model::ActivationRequest;

pub use key::{KeyError, WorkstationKey};

/// Prefix of an activation request text.
pub const REQUEST_PREFIX: &str = "OBA1:";
/// Prefix of a license text, as stored in a `.lic` file.
pub const LICENSE_PREFIX: &str = "OBL1:";
/// Largest license or request text accepted.
pub const MAX_TEXT_BYTES: usize = 4096;

/// Why a license operation failed.
#[derive(Debug, thiserror::Error)]
pub enum LicenseError {
    /// Workstation key problem.
    #[error(transparent)]
    Key(#[from] KeyError),
    /// I/O failure.
    #[error("license storage: {0}")]
    Io(#[from] io::Error),
    /// No workstation key: create an activation request first.
    #[error("no workstation key: create an activation request first")]
    NoWorkstationKey,
    /// Not an Oblivio license or request text.
    #[error("not an Oblivio {0} text")]
    Format(&'static str),
    /// Invalid signature.
    #[error("invalid signature")]
    Signature,
    /// Malformed content.
    #[error("malformed content")]
    Content,
    /// The license was issued for another workstation.
    #[error("license issued for another workstation")]
    WrongWorkstation,
    /// The license has already expired.
    #[error("license expired")]
    Expired,
}

/// An installed license, with the signed form to embed in attestations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledLicense {
    /// Decoded license.
    pub license: License,
    /// The license as signed by the root key (COSE_Sign1), for `lic`.
    pub signed: Vec<u8>,
}

/// State of the installed license.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LicenseStatus {
    /// No license installed.
    Missing,
    /// Valid now.
    Valid(InstalledLicense),
    /// Not valid yet.
    NotYetValid(InstalledLicense),
    /// No longer valid.
    Expired(InstalledLicense),
    /// Installed file unusable: corrupted, wrong root, or wrong workstation.
    Invalid(String),
}

/// License and key storage under one state directory.
#[derive(Debug, Clone)]
pub struct LicenseStore {
    dir: PathBuf,
}

impl LicenseStore {
    /// Storage under `dir`, normally `/var/lib/oblivio`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn key_path(&self) -> PathBuf {
        self.dir.join("keys/workstation.key")
    }

    fn license_path(&self) -> PathBuf {
        self.dir.join("license/oblivio.lic")
    }

    /// The workstation key, if one was created.
    pub fn workstation_key(&self) -> Result<Option<WorkstationKey>, LicenseError> {
        Ok(WorkstationKey::load(&self.key_path())?)
    }

    /// Creates the workstation key if needed and returns the activation
    /// request text (`OBA1:…`) to send to Oblivio. An existing key is reused.
    pub fn create_activation_request(&self, now: u64) -> Result<String, LicenseError> {
        let key = match self.workstation_key()? {
            Some(key) => key,
            None => {
                private_dir(&self.dir.join("keys"))?;
                WorkstationKey::generate(&self.key_path())?
            }
        };
        let request = ActivationRequest {
            workstation_key: key.public_key().to_bytes(),
            issued_at: now,
        };
        let signed = key
            .sign(request.to_cbor())
            .map_err(|_| LicenseError::Signature)?;
        Ok(format!("{REQUEST_PREFIX}{}", base45::encode(&signed)))
    }

    /// Verifies and installs a license text (`OBL1:…`): signed by `root`,
    /// issued for this workstation, not expired at `now`.
    pub fn install(
        &self,
        text: &str,
        root: &VerifyingKey,
        now: u64,
    ) -> Result<InstalledLicense, LicenseError> {
        let key = self
            .workstation_key()?
            .ok_or(LicenseError::NoWorkstationKey)?;
        let installed = decode_license(text, root)?;
        if installed.license.workstation_key != key.public_key().to_bytes() {
            return Err(LicenseError::WrongWorkstation);
        }
        if installed
            .license
            .expires
            .is_some_and(|expires| expires < now)
        {
            return Err(LicenseError::Expired);
        }
        private_dir(&self.dir.join("license"))?;
        write_atomically(
            &self.license_path(),
            license_text(&installed.signed).as_bytes(),
        )?;
        Ok(installed)
    }

    /// State of the installed license at `now`, checked against `root` and
    /// the workstation key every time: a tampered file is never trusted.
    pub fn status(&self, root: &VerifyingKey, now: u64) -> Result<LicenseStatus, LicenseError> {
        let text = match fs::read_to_string(self.license_path()) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(LicenseStatus::Missing);
            }
            Err(error) => return Err(LicenseError::Io(error)),
        };
        let installed = match decode_license(&text, root) {
            Ok(installed) => installed,
            Err(error) => return Ok(LicenseStatus::Invalid(error.to_string())),
        };
        let key = self.workstation_key()?;
        if key.is_none_or(|key| installed.license.workstation_key != key.public_key().to_bytes()) {
            return Ok(LicenseStatus::Invalid(
                LicenseError::WrongWorkstation.to_string(),
            ));
        }
        Ok(if now < installed.license.not_before {
            LicenseStatus::NotYetValid(installed)
        } else if installed.license.is_valid_at(now) {
            LicenseStatus::Valid(installed)
        } else {
            LicenseStatus::Expired(installed)
        })
    }
}

/// Text form of a signed license, as stored in a `.lic` file.
pub fn license_text(signed: &[u8]) -> String {
    format!("{LICENSE_PREFIX}{}", base45::encode(signed))
}

/// Decodes a license text and checks its root signature.
pub fn decode_license(text: &str, root: &VerifyingKey) -> Result<InstalledLicense, LicenseError> {
    let signed = unarmor(text, LICENSE_PREFIX, "license")?;
    let envelope = Unverified::parse(&signed).map_err(|_| LicenseError::Content)?;
    let payload = envelope.verify(root).map_err(|_| LicenseError::Signature)?;
    let license = License::from_cbor(payload).map_err(|_| LicenseError::Content)?;
    Ok(InstalledLicense { license, signed })
}

/// Decodes an activation request and checks that it is signed by the key it
/// carries (proof of possession). Used by the license issuer.
pub fn read_activation_request(text: &str) -> Result<ActivationRequest, LicenseError> {
    let signed = unarmor(text, REQUEST_PREFIX, "activation request")?;
    let envelope = Unverified::parse(&signed).map_err(|_| LicenseError::Content)?;
    let claimed = ActivationRequest::from_cbor(envelope.untrusted_payload())
        .map_err(|_| LicenseError::Content)?;
    let key =
        VerifyingKey::from_bytes(&claimed.workstation_key).map_err(|_| LicenseError::Content)?;
    envelope.verify(&key).map_err(|_| LicenseError::Signature)?;
    Ok(claimed)
}

fn unarmor(text: &str, prefix: &str, what: &'static str) -> Result<Vec<u8>, LicenseError> {
    let text = text.trim();
    if text.len() > MAX_TEXT_BYTES {
        return Err(LicenseError::Format(what));
    }
    let encoded = text
        .strip_prefix(prefix)
        .ok_or(LicenseError::Format(what))?;
    base45::decode(encoded).map_err(|_| LicenseError::Format(what))
}

/// Creates `dir` if needed and restricts it to its owner.
fn private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

/// Writes `content` to `path` through a temporary file and a rename, so a
/// crash leaves either the old file or the new one, never a partial one.
fn write_atomically(path: &Path, content: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(content)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    key::sync_parent(path)
}

#[cfg(test)]
mod tests;
