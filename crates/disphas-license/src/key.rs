//! The workstation key: generated once, kept in a root-only file, never exported.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use disphas_attestation::cose::{self, CoseError};
use disphas_attestation::ed25519_dalek::{SigningKey, VerifyingKey};
use zeroize::Zeroizing;

/// Magic header of the key file, version 1.
const MAGIC: &[u8; 4] = b"OBK1";
const FILE_LEN: usize = MAGIC.len() + 32;

/// Why the key file cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    /// I/O failure.
    #[error("key file: {0}")]
    Io(#[from] io::Error),
    /// The key file already exists: it is never overwritten.
    #[error("a workstation key already exists")]
    AlreadyExists,
    /// The operating system random generator failed.
    #[error("random generator unavailable")]
    Random,
    /// Readable or writable by someone else than its owner, or owned by
    /// another user than the current one.
    #[error("key file permissions are too open or its owner is wrong")]
    InsecurePermissions,
    /// Not an disphas key file.
    #[error("malformed key file")]
    Malformed,
}

/// The workstation signing key. Wiped from memory on drop; no method gives
/// its secret bytes back.
pub struct WorkstationKey(SigningKey);

impl WorkstationKey {
    /// Generates a new key from the operating system random generator and
    /// writes it to `path` (mode 0600). Fails if `path` already exists.
    pub fn generate(path: &Path) -> Result<Self, KeyError> {
        let mut secret = Zeroizing::new([0u8; 32]);
        getrandom::fill(secret.as_mut()).map_err(|_| KeyError::Random)?;
        let key = SigningKey::from_bytes(&secret);

        let mut content = Zeroizing::new(Vec::with_capacity(FILE_LEN));
        content.extend_from_slice(MAGIC);
        content.extend_from_slice(secret.as_ref());
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| match error.kind() {
                io::ErrorKind::AlreadyExists => KeyError::AlreadyExists,
                _ => KeyError::Io(error),
            })?;
        file.write_all(&content)?;
        file.sync_all()?;
        sync_parent(path)?;
        Ok(Self(key))
    }

    /// Loads the key from `path`, or `None` if there is none. Refuses a file
    /// that someone else than its owner could read or write.
    pub fn load(path: &Path) -> Result<Option<Self>, KeyError> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(KeyError::Io(error)),
        };
        let metadata = file.metadata()?;
        if metadata.permissions().mode() & 0o077 != 0 || metadata.uid() != expected_owner(path)? {
            return Err(KeyError::InsecurePermissions);
        }
        let mut content = Zeroizing::new(Vec::with_capacity(FILE_LEN + 1));
        Read::by_ref(&mut file)
            .take(FILE_LEN as u64 + 1)
            .read_to_end(&mut content)?;
        if content.len() != FILE_LEN || &content[..MAGIC.len()] != MAGIC {
            return Err(KeyError::Malformed);
        }
        let mut secret = Zeroizing::new([0u8; 32]);
        secret.copy_from_slice(&content[MAGIC.len()..]);
        Ok(Some(Self(SigningKey::from_bytes(&secret))))
    }

    /// The public half, shared in the activation request and the license.
    pub fn public_key(&self) -> VerifyingKey {
        self.0.verifying_key()
    }

    /// Signs `payload` into a COSE_Sign1 message with this key.
    pub fn sign(&self, payload: Vec<u8>) -> Result<Vec<u8>, CoseError> {
        cose::sign(payload, &self.0)
    }
}

impl core::fmt::Debug for WorkstationKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("WorkstationKey")
            .field(&self.public_key())
            .finish()
    }
}

/// Owner expected for the key file: the owner of its directory (root for the
/// helper, the current user in tests), which avoids an `unsafe` getuid. The
/// directory itself must be closed to everyone else, or the check means nothing.
fn expected_owner(path: &Path) -> Result<u32, KeyError> {
    let parent = fs::metadata(path.parent().unwrap_or(Path::new(".")))?;
    if parent.permissions().mode() & 0o077 != 0 {
        return Err(KeyError::InsecurePermissions);
    }
    Ok(parent.uid())
}

/// Makes a rename or creation in the parent directory durable.
pub(crate) fn sync_parent(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(parent) => File::open(parent)?.sync_all(),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temporary directory closed to other users, as the store creates it.
    fn private_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temporary directory");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).expect("chmod");
        dir
    }

    #[test]
    fn generates_then_loads_the_same_key() {
        let dir = private_dir();
        let path = dir.path().join("workstation.key");
        let generated = WorkstationKey::generate(&path).expect("generated");
        let mode = fs::metadata(&path).expect("metadata").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let loaded = WorkstationKey::load(&path)
            .expect("readable")
            .expect("present");
        assert_eq!(loaded.public_key(), generated.public_key());
    }

    #[test]
    fn never_overwrites_an_existing_key() {
        let dir = private_dir();
        let path = dir.path().join("workstation.key");
        let first = WorkstationKey::generate(&path).expect("generated");
        assert!(matches!(
            WorkstationKey::generate(&path),
            Err(KeyError::AlreadyExists)
        ));
        let kept = WorkstationKey::load(&path)
            .expect("readable")
            .expect("present");
        assert_eq!(kept.public_key(), first.public_key());
    }

    #[test]
    fn two_generated_keys_differ() {
        let dir = private_dir();
        let a = WorkstationKey::generate(&dir.path().join("a")).expect("generated");
        let b = WorkstationKey::generate(&dir.path().join("b")).expect("generated");
        assert_ne!(a.public_key(), b.public_key());
    }

    #[test]
    fn missing_key_is_none() {
        let dir = private_dir();
        assert!(
            WorkstationKey::load(&dir.path().join("absent"))
                .expect("not an error")
                .is_none()
        );
    }

    #[test]
    fn readable_by_others_is_refused() {
        let dir = private_dir();
        let path = dir.path().join("workstation.key");
        WorkstationKey::generate(&path).expect("generated");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");
        assert!(matches!(
            WorkstationKey::load(&path),
            Err(KeyError::InsecurePermissions)
        ));
    }

    #[test]
    fn open_directory_is_refused() {
        let dir = private_dir();
        let path = dir.path().join("workstation.key");
        WorkstationKey::generate(&path).expect("generated");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).expect("chmod");
        assert!(matches!(
            WorkstationKey::load(&path),
            Err(KeyError::InsecurePermissions)
        ));
    }

    #[test]
    fn malformed_file_is_refused() {
        let dir = private_dir();
        let path = dir.path().join("workstation.key");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .expect("created");
        file.write_all(b"OBK1short").expect("written");
        assert!(matches!(
            WorkstationKey::load(&path),
            Err(KeyError::Malformed)
        ));
    }

    #[test]
    fn debug_shows_only_the_public_key() {
        let dir = private_dir();
        let key = WorkstationKey::generate(&dir.path().join("k")).expect("generated");
        let text = format!("{key:?}");
        assert!(text.starts_with("WorkstationKey(VerifyingKey"));
        assert!(!text.contains("secret"));
    }
}
