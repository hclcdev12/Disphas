//! Secrets carried by the protocol.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroizing;

/// Longest ATA password, in bytes (ATA Security feature set).
pub const ATA_PASSWORD_MAX_BYTES: usize = 32;

/// ATA user or master password, wiped from memory on drop and never printed.
///
/// Deserialization moves the decoded string into the wrapper without copying
/// it. The caller remains responsible for wiping the line it decoded from.
#[derive(Clone, PartialEq, Eq)]
pub struct AtaPassword(Zeroizing<String>);

impl AtaPassword {
    /// Wraps `password`, rejecting it if longer than [`ATA_PASSWORD_MAX_BYTES`].
    pub fn new(password: String) -> Result<Self, PasswordTooLong> {
        let password = Zeroizing::new(password);
        if password.len() > ATA_PASSWORD_MAX_BYTES {
            return Err(PasswordTooLong);
        }
        Ok(Self(password))
    }

    /// The password itself. Never log it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

/// The password exceeds [`ATA_PASSWORD_MAX_BYTES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("ATA password longer than {ATA_PASSWORD_MAX_BYTES} bytes")]
pub struct PasswordTooLong;

impl core::fmt::Debug for AtaPassword {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("AtaPassword(<redacted>)")
    }
}

impl Serialize for AtaPassword {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for AtaPassword {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_password() {
        let password = AtaPassword::new("hunter2".to_owned()).expect("short password");
        assert!(!format!("{password:?}").contains("hunter2"));
    }

    #[test]
    fn password_length_is_bounded() {
        assert!(AtaPassword::new("x".repeat(ATA_PASSWORD_MAX_BYTES)).is_ok());
        assert_eq!(
            AtaPassword::new("x".repeat(ATA_PASSWORD_MAX_BYTES + 1)),
            Err(PasswordTooLong)
        );
    }
}
