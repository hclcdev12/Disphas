//! Access to drives, as seen by the helper.
//!
//! The real implementation will live on top of `oblivio-device`. Until then
//! the binary runs with [`NoDevices`], which sees no drive at all.

use oblivio_core::profile::DeviceProfile;
use oblivio_protocol::command::DeviceIdentity;

use crate::erasure::ErasureTarget;

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
    /// Sanitize check passed. Refuses by default: no backend erases yet.
    fn open(&mut self, _identity: &DeviceIdentity) -> Result<Box<dyn ErasureTarget>, String> {
        Err("this backend cannot erase drives".to_owned())
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
