//! Access to drives, as seen by the helper.
//!
//! The real implementation will live on top of `oblivio-device`. Until then
//! the binary runs with [`NoDevices`], which sees no drive at all.

use oblivio_core::profile::DeviceProfile;
use oblivio_protocol::command::DeviceIdentity;

use crate::erasure::ErasureTarget;

/// Why a drive could not be opened for erasure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// Another program uses the drive (`DEVICE_IN_USE`): the operator can
    /// close it and try again.
    InUse(String),
    /// The drive became protected since the verdict (`PROTECTED`).
    Protected(String),
    /// Any other failure (`INTERNAL`).
    Other(String),
}

impl From<String> for OpenError {
    fn from(detail: String) -> Self {
        Self::Other(detail)
    }
}

impl From<&str> for OpenError {
    fn from(detail: &str) -> Self {
        Self::Other(detail.to_owned())
    }
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InUse(detail) | Self::Protected(detail) | Self::Other(detail) => {
                f.write_str(detail)
            }
        }
    }
}

/// A drive found by the backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// Current path, such as `/dev/nvme0n1`: informative only.
    pub node_hint: Option<String>,
    /// Capacity in bytes, if known.
    pub capacity_bytes: Option<u64>,
    /// Hardware profile, read just now.
    pub profile: DeviceProfile,
}

/// Source of drive information.
pub trait DeviceBackend {
    /// Every drive currently attached.
    fn list(&mut self) -> Vec<Device>;

    /// The drive with exactly this identity, read again from the hardware.
    /// Never looked up by path.
    fn find(&mut self, identity: &DeviceIdentity) -> Option<Device>;

    /// Opens the drive with this identity for erasure. Called only once every
    /// Sanitize check passed. With `whole_controller`, the other namespaces
    /// of its NVMe controller are held too. Refuses by default.
    fn open(
        &mut self,
        _identity: &DeviceIdentity,
        _whole_controller: bool,
    ) -> Result<Box<dyn ErasureTarget>, OpenError> {
        Err("this backend cannot erase drives".into())
    }

    /// Unlocks a Locked ATA drive (B2) with the operator's password, then
    /// removes that password so the drive can be erased. Refuses by default.
    fn unlock(
        &mut self,
        _identity: &DeviceIdentity,
        _master: bool,
        _password: &[u8; 32],
    ) -> Result<(), String> {
        Err("this backend cannot unlock drives".to_owned())
    }
}

/// Backend that sees no drive. Used until `oblivio-device` is wired in.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoDevices;

impl DeviceBackend for NoDevices {
    fn list(&mut self) -> Vec<Device> {
        Vec::new()
    }

    fn find(&mut self, _identity: &DeviceIdentity) -> Option<Device> {
        None
    }
}
