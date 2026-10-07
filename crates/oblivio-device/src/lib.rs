//! Low-level access to storage devices (NVMe and ATA ioctls, SG_IO,
//! O_DIRECT with aligned buffers, sysfs).
//!
//! This is the only crate allowed to talk to a device. It must refuse a
//! PROTECTED drive by construction, including in tests. Hardware tests live
//! behind the `hw-tests` feature and the `OBLIVIO_TEST_DEVICE` variable.
//!
//! Read-only [`inventory`] (sysfs and procfs), pure [`decode`]rs, NVMe admin
//! and ATA pass-through commands ([`nvme`], [`ata`]), firmware [`inspect`]ion,
//! and [`drive`] access for erasure, which refuses protected drives by
//! construction. All `unsafe` code lives in the private `sys` module and in
//! [`aligned`].

pub mod aligned;
pub mod ata;
pub mod block;
pub mod decode;
pub mod drive;
pub mod inspect;
pub mod inventory;
pub mod nvme;
pub mod openers;
pub mod procfs;
pub mod security;
mod sys;
pub mod sysfs;

pub use inventory::{BlockDevice, ScanError, SystemPaths, scan};

/// Why a device operation failed.
#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    /// System call failure.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// The inventory could not be trusted.
    #[error("inventory: {0}")]
    Scan(#[from] ScanError),
    /// Firmware data could not be decoded.
    #[error("{0}")]
    Decode(#[from] decode::DecodeError),
    /// The device rejected or failed a command.
    #[error("{0}")]
    Command(String),
    /// Oblivio refuses to touch this drive.
    #[error("refused: {0}")]
    Refused(String),
    /// Another program uses the drive, a partition or a sibling namespace:
    /// mounted or opened exclusively (`O_EXCL` fails), or held open by a
    /// process (docs/securite.md, S9). The operator can act on it.
    #[error("in use: {0}")]
    InUse(String),
    /// The drive became protected (mounted, swap, volume member, system
    /// drive) between the verdict and the opening for erasure.
    #[error("protected: {0}")]
    Protected(String),
}
