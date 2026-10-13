//! State directory of the signing service (`docs/licences_en_ligne.md` §6.2):
//! `keys/` shop keys (same secure file as the workstation key: 0600, owner
//! checked, wiped from memory), `issuers/` installed certificates, and the
//! register. Everything is closed to other users.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use disphas_attestation::Issuer;
use disphas_attestation::ed25519_dalek::VerifyingKey;
use disphas_license::{WorkstationKey, decode_issuer};

/// A shop key with the certificate the root signed for it.
#[derive(Debug)]
pub struct Certified {
    /// The issuer certificate.
    pub issuer: Issuer,
    /// Its signed form, embedded in every license (`iss`).
    pub signed: Vec<u8>,
    /// The shop key it certifies.
    pub key: WorkstationKey,
}

/// The state directory.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

fn hex(key: &VerifyingKey) -> String {
    key.as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

impl Store {
    /// The state under `dir`, normally `/var/lib/disphas-signer`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Path of the register.
    pub fn register_path(&self) -> PathBuf {
        self.dir.join("register.csv")
    }

    fn keys(&self) -> PathBuf {
        self.dir.join("keys")
    }

    fn issuers(&self) -> PathBuf {
        self.dir.join("issuers")
    }

    /// Creates the directories, closed to other users.
    pub fn prepare(&self) -> io::Result<()> {
        private_dir(&self.dir)?;
        private_dir(&self.keys())?;
        private_dir(&self.issuers())
    }

    /// Generates a new shop key and returns its public half, to certify.
    pub fn keygen(&self) -> Result<VerifyingKey, String> {
        self.prepare().map_err(|error| error.to_string())?;
        let mut name = [0u8; 8];
        getrandom::fill(&mut name).map_err(|error| error.to_string())?;
        let name: String = name.iter().map(|byte| format!("{byte:02x}")).collect();
        let pending = self.keys().join(format!("new-{name}.key"));
        let key = WorkstationKey::generate(&pending).map_err(|error| error.to_string())?;
        let public = key.public_key();
        fs::rename(&pending, self.key_path(&public)).map_err(|error| error.to_string())?;
        Ok(public)
    }

    fn key_path(&self, public: &VerifyingKey) -> PathBuf {
        self.keys().join(format!("{}.key", hex(public)))
    }

    /// Verifies an issuer certificate (`DSI1:…`) against `roots` and installs
    /// it, if it certifies a shop key held here. Returns the certificate.
    pub fn install(&self, text: &str, roots: &[VerifyingKey], now: u64) -> Result<Issuer, String> {
        let (issuer, signed) = roots
            .iter()
            .find_map(|root| decode_issuer(text, root).ok())
            .ok_or("not an issuer certificate signed by a trusted root key")?;
        if issuer.expires <= now {
            return Err(format!("{} has already expired", issuer.id));
        }
        let public = VerifyingKey::from_bytes(&issuer.key)
            .map_err(|_| "the certified key is not an Ed25519 key".to_owned())?;
        if WorkstationKey::load(&self.key_path(&public))
            .map_err(|error| error.to_string())?
            .is_none()
        {
            return Err(format!(
                "{} certifies the key {}, which is not held here (disphas-signer keygen)",
                issuer.id,
                hex(&public)
            ));
        }
        self.prepare().map_err(|error| error.to_string())?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.issuers().join(format!("{}.obi", issuer.id)))
            .map_err(|error| match error.kind() {
                io::ErrorKind::AlreadyExists => format!("{} is already installed", issuer.id),
                _ => error.to_string(),
            })?;
        writeln!(file, "{}", disphas_license::issuer_text(&signed))
            .and_then(|()| file.sync_all())
            .map_err(|error| error.to_string())?;
        Ok(issuer)
    }

    /// Every installed certificate with its key, each verified again
    /// against `roots`. A certificate without its key is an error.
    pub fn certified(&self, roots: &[VerifyingKey]) -> Result<Vec<Certified>, String> {
        let entries = match fs::read_dir(self.issuers()) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.to_string()),
        };
        let mut certified = Vec::new();
        for entry in entries {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("obi") {
                continue;
            }
            let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            let (issuer, signed) = roots
                .iter()
                .find_map(|root| decode_issuer(&text, root).ok())
                .ok_or_else(|| format!("{}: not signed by a trusted root key", path.display()))?;
            let public = VerifyingKey::from_bytes(&issuer.key)
                .map_err(|_| format!("{}: invalid key", path.display()))?;
            let key = WorkstationKey::load(&self.key_path(&public))
                .map_err(|error| format!("{}: {error}", path.display()))?
                .ok_or_else(|| format!("{}: its key is missing", path.display()))?;
            certified.push(Certified {
                issuer,
                signed,
                key,
            });
        }
        certified.sort_by_key(|entry| entry.issuer.expires);
        Ok(certified)
    }
}
