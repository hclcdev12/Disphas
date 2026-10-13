//! The disphas root key file: Ed25519 seed encrypted under a passphrase.
//!
//! Layout (`OBR1`), little-endian:
//!
//! ```text
//! magic "OBR1" | m_cost u32 | t_cost u32 | p_cost u32 | salt [16] | nonce [12]
//!   | public key [32] | ciphertext of the seed [32 + 16-byte tag]
//! ```
//!
//! The key-encryption key is Argon2id(passphrase, salt). The seed is sealed
//! with ChaCha20-Poly1305; everything before the ciphertext is authenticated
//! as associated data, so the parameters and public key cannot be altered.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use disphas_attestation::ed25519_dalek::{SigningKey, VerifyingKey};
use zeroize::Zeroizing;

const MAGIC: &[u8; 4] = b"OBR1";
const HEADER_LEN: usize = 4 + 12 + 16 + 12 + 32;
const FILE_LEN: usize = HEADER_LEN + 32 + 16;

/// Shortest accepted passphrase, in characters.
pub const MIN_PASSPHRASE_CHARS: usize = 12;

/// Argon2id cost parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cost {
    /// Memory in KiB.
    pub memory_kib: u32,
    /// Iterations.
    pub iterations: u32,
    /// Lanes.
    pub lanes: u32,
}

impl Cost {
    /// Default for a root key: 256 MiB, 3 passes, 1 lane.
    pub const ROOT: Self = Self {
        memory_kib: 256 * 1024,
        iterations: 3,
        lanes: 1,
    };
}

/// Why a root key file cannot be created or opened.
#[derive(Debug, thiserror::Error)]
pub enum RootKeyError {
    /// File error.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// The file already exists: a root key is never overwritten.
    #[error("the root key file already exists")]
    Exists,
    /// Passphrase too short.
    #[error("the passphrase needs at least {MIN_PASSPHRASE_CHARS} characters")]
    WeakPassphrase,
    /// Not an disphas root key file.
    #[error("not an disphas root key file")]
    Malformed,
    /// Wrong passphrase, or the file was altered.
    #[error("wrong passphrase or altered file")]
    Unlock,
    /// Random generator or key derivation failure.
    #[error("{0}")]
    Crypto(String),
}

fn kek(passphrase: &str, salt: &[u8], cost: Cost) -> Result<Zeroizing<[u8; 32]>, RootKeyError> {
    let params = Params::new(cost.memory_kib, cost.iterations, cost.lanes, Some(32))
        .map_err(|error| RootKeyError::Crypto(error.to_string()))?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, key.as_mut())
        .map_err(|error| RootKeyError::Crypto(error.to_string()))?;
    Ok(key)
}

fn cipher(key: &[u8; 32]) -> Result<ChaCha20Poly1305, RootKeyError> {
    ChaCha20Poly1305::new_from_slice(key).map_err(|error| RootKeyError::Crypto(error.to_string()))
}

fn random<const N: usize>() -> Result<[u8; N], RootKeyError> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|error| RootKeyError::Crypto(error.to_string()))?;
    Ok(bytes)
}

/// Generates a root key, writes it encrypted to `path` (mode 0600, never
/// overwritten) and returns its public half.
pub fn create(path: &Path, passphrase: &str, cost: Cost) -> Result<VerifyingKey, RootKeyError> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(RootKeyError::WeakPassphrase);
    }
    let seed = Zeroizing::new(random::<32>()?);
    let public = SigningKey::from_bytes(&seed).verifying_key();
    let salt = random::<16>()?;
    let nonce = random::<12>()?;

    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&cost.memory_kib.to_le_bytes());
    header.extend_from_slice(&cost.iterations.to_le_bytes());
    header.extend_from_slice(&cost.lanes.to_le_bytes());
    header.extend_from_slice(&salt);
    header.extend_from_slice(&nonce);
    header.extend_from_slice(public.as_bytes());

    let key = kek(passphrase, &salt, cost)?;
    let sealed = cipher(&key)?
        .encrypt(
            &Nonce::from(nonce),
            Payload {
                msg: seed.as_ref(),
                aad: &header,
            },
        )
        .map_err(|_| RootKeyError::Crypto("encryption failed".to_owned()))?;

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => RootKeyError::Exists,
            _ => RootKeyError::Io(error),
        })?;
    file.write_all(&header)?;
    file.write_all(&sealed)?;
    file.sync_all()?;
    Ok(public)
}

struct Parsed {
    cost: Cost,
    salt: [u8; 16],
    nonce: [u8; 12],
    public: VerifyingKey,
    header: Vec<u8>,
    sealed: Vec<u8>,
}

fn parse(path: &Path) -> Result<Parsed, RootKeyError> {
    let bytes = std::fs::read(path)?;
    if bytes.len() != FILE_LEN || &bytes[..4] != MAGIC {
        return Err(RootKeyError::Malformed);
    }
    let u32_at = |offset: usize| {
        let mut word = [0u8; 4];
        word.copy_from_slice(&bytes[offset..offset + 4]);
        u32::from_le_bytes(word)
    };
    let mut salt = [0u8; 16];
    salt.copy_from_slice(&bytes[16..32]);
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&bytes[32..44]);
    let mut public = [0u8; 32];
    public.copy_from_slice(&bytes[44..76]);
    Ok(Parsed {
        cost: Cost {
            memory_kib: u32_at(4),
            iterations: u32_at(8),
            lanes: u32_at(12),
        },
        salt,
        nonce,
        public: VerifyingKey::from_bytes(&public).map_err(|_| RootKeyError::Malformed)?,
        header: bytes[..HEADER_LEN].to_vec(),
        sealed: bytes[HEADER_LEN..].to_vec(),
    })
}

/// Public key stored in the file, readable without the passphrase. It is
/// only trustworthy once `unlock` succeeded, which authenticates it.
pub fn public_key(path: &Path) -> Result<VerifyingKey, RootKeyError> {
    Ok(parse(path)?.public)
}

/// Decrypts the root signing key.
pub fn unlock(path: &Path, passphrase: &str) -> Result<SigningKey, RootKeyError> {
    let parsed = parse(path)?;
    let key = kek(passphrase, &parsed.salt, parsed.cost)?;
    let seed = Zeroizing::new(
        cipher(&key)?
            .decrypt(
                &Nonce::from(parsed.nonce),
                Payload {
                    msg: &parsed.sealed,
                    aad: &parsed.header,
                },
            )
            .map_err(|_| RootKeyError::Unlock)?,
    );
    let seed: [u8; 32] = seed
        .as_slice()
        .try_into()
        .map_err(|_| RootKeyError::Malformed)?;
    let signing = SigningKey::from_bytes(&seed);
    if signing.verifying_key() != parsed.public {
        return Err(RootKeyError::Unlock);
    }
    Ok(signing)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cheap parameters: the tests check the format, not the cost.
    const FAST: Cost = Cost {
        memory_kib: 64,
        iterations: 1,
        lanes: 1,
    };
    const PASS: &str = "correct horse battery staple";

    #[test]
    fn creates_then_unlocks_the_same_key() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("root.key");
        let public = create(&path, PASS, FAST).expect("created");
        assert_eq!(public_key(&path).expect("public"), public);
        assert_eq!(
            unlock(&path, PASS).expect("unlocked").verifying_key(),
            public
        );
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn wrong_passphrase_is_refused() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("root.key");
        create(&path, PASS, FAST).expect("created");
        assert!(matches!(
            unlock(&path, "wrong passphrase!!"),
            Err(RootKeyError::Unlock)
        ));
    }

    #[test]
    fn never_overwrites_and_refuses_weak_passphrases() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("root.key");
        assert!(matches!(
            create(&path, "short", FAST),
            Err(RootKeyError::WeakPassphrase)
        ));
        create(&path, PASS, FAST).expect("created");
        assert!(matches!(
            create(&path, PASS, FAST),
            Err(RootKeyError::Exists)
        ));
    }

    #[test]
    fn any_altered_byte_is_detected() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("root.key");
        create(&path, PASS, FAST).expect("created");
        let original = std::fs::read(&path).expect("read");
        // Skip the cost fields: altering them only changes the derived key,
        // which the authenticated decryption then rejects as well.
        for index in [4, 20, 36, 50, 80, 110] {
            let mut altered = original.clone();
            altered[index] ^= 0x01;
            std::fs::write(&path, &altered).expect("write");
            assert!(unlock(&path, PASS).is_err(), "byte {index}");
        }
    }
}
