//! Firmware inspection of the drives of the inventory: the engine profile.
//!
//! Read-only commands only (Identify, logs, SMART, sanitize status), sent on
//! a node opened read-only and non-exclusively, so mounted drives can be
//! inspected too (they will be PROTECTED). Any failure gives
//! `Inspection::Failed` (N1), never a guess.

use std::fs::{File, OpenOptions};
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};

use disphas_core::profile::{DeviceIdentity, DeviceProfile, Inspection, QuirkFlags, SanitizeState};

use crate::DeviceError;
use crate::ata::AtaDevice;
use crate::decode::{ata as ata_decode, nvme as nvme_decode};
use crate::inventory::{BlockDevice, SystemPaths, Transport};
use crate::nvme::NvmeDevice;

/// A drive of the inventory with its engine profile.
#[derive(Debug, Clone)]
pub struct Inspected {
    /// Kernel name, such as `nvme0n1`.
    pub name: String,
    /// Capacity in bytes, if known.
    pub capacity_bytes: Option<u64>,
    /// Profile for the decision engine.
    pub profile: DeviceProfile,
}

/// Opens a drive node read-only and non-exclusively, for inspection and
/// security state changes (unlock). Block devices only.
pub fn open_read_only(paths: &SystemPaths, name: &str) -> Result<File, DeviceError> {
    let path = paths.dev.join(name);
    if !std::fs::symlink_metadata(&path)?
        .file_type()
        .is_block_device()
    {
        return Err(DeviceError::Refused(format!(
            "{} is not a block device",
            path.display()
        )));
    }
    Ok(OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?)
}

/// Like [`open_read_only`], but exclusive (`O_EXCL`): refused while the drive
/// is mounted or opened for erasure, by this helper or another one. Used for
/// security state changes (unlock, Security Erase recovery).
pub fn open_exclusive(paths: &SystemPaths, name: &str) -> Result<File, DeviceError> {
    // Checks that the node is a block device.
    drop(open_read_only(paths, name)?);
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_EXCL)
        .open(paths.dev.join(name))
        .map_err(|error| match error.raw_os_error() {
            Some(libc::EBUSY) => DeviceError::InUse(format!("{name} is in use (O_EXCL)")),
            _ => DeviceError::Io(error),
        })
}

/// Identity reported by the firmware of an opened drive, built exactly as
/// [`inspect`] builds it: used to check that a node opened for erasure is
/// still the drive the operator confirmed.
pub fn identity_of(file: &File, nvme: bool) -> Result<DeviceIdentity, DeviceError> {
    if nvme {
        let device = NvmeDevice::new(file)?;
        let controller = device.identify_controller()?;
        let (_, namespace) = device.identify_namespace()?;
        Ok(DeviceIdentity {
            model: controller.model,
            serial: controller.serial,
            firmware: controller.firmware,
            wwn: None,
            eui64: namespace.eui64,
            nguid: namespace.nguid,
        })
    } else {
        let identify = AtaDevice::new(file).identify()?;
        Ok(DeviceIdentity {
            model: identify.model,
            serial: identify.serial,
            firmware: identify.firmware,
            wwn: identify.wwn,
            eui64: None,
            nguid: None,
        })
    }
}

fn nvme_profile(file: &File) -> Result<(DeviceIdentity, u64, Inspection), DeviceError> {
    let device = NvmeDevice::new(file)?;
    let controller = device.identify_controller()?;
    let (_, namespace) = device.identify_namespace()?;
    let health = device.health()?;
    // Without Sanitize there is no Sanitize Status log (the controller
    // refuses to read it) and nothing can be in progress or failed.
    let sanitize_state = if controller.sanitize_supported {
        device.sanitize_status()?.0
    } else {
        SanitizeState::Idle
    };
    let identity = DeviceIdentity {
        model: controller.model.clone(),
        serial: controller.serial.clone(),
        firmware: controller.firmware.clone(),
        wwn: None,
        eui64: namespace.eui64.clone(),
        nguid: namespace.nguid.clone(),
    };
    let capacity = namespace.capacity_bytes().unwrap_or_default();
    Ok((
        identity,
        capacity,
        Inspection::Nvme(nvme_decode::profile(&controller, health, sanitize_state)),
    ))
}

fn ata_profile(file: &File) -> Result<(DeviceIdentity, u64, Inspection), DeviceError> {
    let device = AtaDevice::new(file);
    let identify = device.identify()?;
    // SMART may be unsupported or disabled: no attributes is not a failure.
    let smart = device.smart().ok();
    let sanitize = if identify.sanitize_supported {
        device.sanitize_status()?.0
    } else {
        ata_decode::sanitize_status(0, false)
    };
    let profile = ata_decode::profile(&identify, smart, sanitize);
    let identity = DeviceIdentity {
        model: identify.model.clone(),
        serial: identify.serial.clone(),
        firmware: identify.firmware.clone(),
        wwn: identify.wwn.clone(),
        eui64: None,
        nguid: None,
    };
    Ok((identity, identify.capacity_bytes, Inspection::Ata(profile)))
}

/// Inspects one drive of the inventory. NVMe and SATA only (V1).
pub fn inspect(paths: &SystemPaths, drive: &BlockDevice) -> Option<Inspected> {
    let result = match drive.transport {
        Transport::Nvme => open_read_only(paths, &drive.name).and_then(|file| nvme_profile(&file)),
        Transport::Ata => open_read_only(paths, &drive.name).and_then(|file| ata_profile(&file)),
        Transport::Usb | Transport::Other => return None,
    };
    let (identity, capacity, inspection) = match result {
        Ok(found) => found,
        Err(_) => (
            DeviceIdentity {
                model: drive.identity.model.clone().unwrap_or_default(),
                serial: drive.identity.serial.clone().unwrap_or_default(),
                firmware: drive.identity.firmware.clone().unwrap_or_default(),
                wwn: None,
                eui64: None,
                nguid: None,
            },
            drive.size_bytes,
            Inspection::Failed,
        ),
    };
    Some(Inspected {
        name: drive.name.clone(),
        capacity_bytes: Some(capacity).filter(|&bytes| bytes > 0),
        profile: DeviceProfile {
            identity,
            protection: drive.protection,
            inspection,
            failed_unblock_attempts: 0,
            quirks: QuirkFlags::default(),
        },
    })
}
