//! The real backend: inventory, read-only firmware inspection, and erasure
//! through firmware commands or O_DIRECT writes (`oblivio-device`).
//!
//! Used by release builds only, or by development builds with the
//! `real-devices` feature, on a test bench, as root (CLAUDE.md §2).

use std::path::PathBuf;
use std::time::Duration;

use oblivio_core::method::Method;
use oblivio_core::profile::{AtaSmart, SanitizeState};
use oblivio_device::ata::{AtaDevice, Identifier, SanitizeOperation};
use oblivio_device::block::BlockIo;
use oblivio_device::drive::ErasableDrive;
use oblivio_device::inspect::{inspect, open_exclusive};
use oblivio_device::nvme::{NvmeDevice, SanitizeAction, SecureErase};
use oblivio_device::security::{self, Password, SecurityDrive, SecurityState};
use oblivio_device::{SystemPaths, scan};
use oblivio_protocol::command::DeviceIdentity;

use crate::backend::{Device, DeviceBackend};
use crate::convert;
use crate::erasure::ErasureTarget;
use crate::recovery::{self, FileRecord};

/// Interval between two sanitize status reads.
const POLL: Duration = Duration::from_secs(1);

/// Backend over the drives of this machine.
#[derive(Debug)]
pub struct SystemBackend {
    paths: SystemPaths,
    /// Where Security Erase recovery records live (`<state>/recovery`).
    recovery: PathBuf,
}

impl SystemBackend {
    /// Backend keeping Security Erase recovery records in `recovery`.
    pub fn new(recovery: PathBuf) -> Self {
        Self {
            paths: SystemPaths::default(),
            recovery,
        }
    }

    /// Restores the drives left with Oblivio's password by an interrupted
    /// Security Erase, using their recovery records. Records of drives not
    /// found, or not restored, are kept. Returns a line per record.
    pub fn restore_pending(&mut self) -> Vec<String> {
        let drives = self.inspected();
        recovery::pending(&self.recovery)
            .into_iter()
            .map(|mut pending| {
                let Some((name, _)) = drives
                    .iter()
                    .find(|(_, device)| device.profile.identity.serial == pending.serial)
                else {
                    return format!("drive {} absent, recovery record kept", pending.serial);
                };
                let outcome = open_exclusive(&self.paths, name).and_then(|file| {
                    let mut drive = AtaSecurity {
                        ata: AtaDevice::new(&file),
                    };
                    security::restore(&mut drive, &pending.password)
                });
                match outcome {
                    Ok(true) => match security::RecoveryRecord::clear(&mut pending.record) {
                        Ok(()) => format!("drive {} restored", pending.serial),
                        Err(error) => format!(
                            "drive {} restored, record not deleted: {error}",
                            pending.serial
                        ),
                    },
                    Ok(false) => format!("drive {} still protected, record kept", pending.serial),
                    Err(error) => format!("drive {}: {error}, record kept", pending.serial),
                }
            })
            .collect()
    }

    fn inspected(&self) -> Vec<(String, Device)> {
        let drives = match scan(&self.paths) {
            Ok(drives) => drives,
            Err(error) => {
                // Without a trustworthy inventory, protections are unknown:
                // show nothing rather than a drive that could be protected.
                eprintln!("oblivio-helper: inventory failed: {error}");
                return Vec::new();
            }
        };
        drives
            .iter()
            .filter_map(|drive| inspect(&self.paths, drive))
            .map(|inspected| {
                let device = Device {
                    node_hint: Some(format!("/dev/{}", inspected.name)),
                    capacity_bytes: inspected.capacity_bytes,
                    profile: inspected.profile,
                };
                (inspected.name, device)
            })
            .collect()
    }
}

impl DeviceBackend for SystemBackend {
    fn list(&mut self) -> Vec<Device> {
        self.inspected()
            .into_iter()
            .map(|(_, device)| device)
            .collect()
    }

    fn find(&mut self, identity: &DeviceIdentity) -> Option<Device> {
        self.inspected()
            .into_iter()
            .map(|(_, device)| device)
            .find(|device| convert::identity(&device.profile.identity) == *identity)
    }

    fn open(&mut self, identity: &DeviceIdentity) -> Result<Box<dyn ErasureTarget>, String> {
        let name = self
            .inspected()
            .into_iter()
            .find(|(_, device)| convert::identity(&device.profile.identity) == *identity)
            .map(|(name, _)| name)
            .ok_or("drive not found")?;
        let identity = self
            .inspected()
            .into_iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, device)| device.profile.identity)
            .ok_or("drive not found")?;
        // Opening re-checks protections on a fresh inventory and uses O_EXCL.
        let drive = ErasableDrive::open(&self.paths, &name).map_err(|error| error.to_string())?;
        Ok(Box::new(DriveTarget {
            drive,
            record: FileRecord::new(&self.recovery, &identity),
        }))
    }

    fn unlock(&mut self, identity: &DeviceIdentity, password: &[u8; 32]) -> Result<(), String> {
        let name = self
            .inspected()
            .into_iter()
            .find(|(_, device)| convert::identity(&device.profile.identity) == *identity)
            .map(|(name, _)| name)
            .ok_or("drive not found")?;
        if !name.starts_with("sd") {
            return Err("only SATA drives have ATA passwords".to_owned());
        }
        let file = open_exclusive(&self.paths, &name).map_err(|error| error.to_string())?;
        let ata = AtaDevice::new(&file);
        // Each failed attempt counts towards the drive's limit (B6).
        ata.unlock(Identifier::User, password)
            .map_err(|_| "wrong password, or the drive refused it".to_owned())?;
        ata.disable_password(Identifier::User, password)
            .map_err(|error| format!("unlocked, but the password could not be removed: {error}"))
    }
}

/// The security commands of a SATA drive.
struct AtaSecurity<'a> {
    ata: AtaDevice<'a>,
}

impl SecurityDrive for AtaSecurity<'_> {
    fn state(&mut self, enhanced: bool) -> Result<SecurityState, oblivio_device::DeviceError> {
        let identify = self.ata.identify()?;
        let security = identify.security;
        Ok(SecurityState {
            supported: security.supported,
            enabled: security.enabled,
            locked: security.locked,
            frozen: security.frozen,
            attempts_expired: security.attempts_expired,
            enhanced_supported: security.enhanced_erase,
            erase_minutes: if enhanced {
                identify.enhanced_erase_minutes
            } else {
                identify.security_erase_minutes
            },
        })
    }

    fn set_password(&mut self, password: &Password) -> Result<(), oblivio_device::DeviceError> {
        self.ata.set_user_password(password.bytes())
    }

    fn erase(
        &mut self,
        password: &Password,
        enhanced: bool,
        timeout_ms: u32,
    ) -> Result<(), oblivio_device::DeviceError> {
        self.ata.erase_unit(password.bytes(), enhanced, timeout_ms)
    }

    fn unlock(&mut self, password: &Password) -> Result<(), oblivio_device::DeviceError> {
        self.ata.unlock(Identifier::User, password.bytes())
    }

    fn disable(&mut self, password: &Password) -> Result<(), oblivio_device::DeviceError> {
        self.ata
            .disable_password(Identifier::User, password.bytes())
    }
}

/// A real drive opened for erasure.
struct DriveTarget {
    drive: ErasableDrive,
    /// Recovery record used by Security Erase.
    record: FileRecord,
}

/// Polls a background sanitize until it ends. After a successful start the
/// controller reports the operation in progress until it completes, so a
/// completed or idle state read afterwards is this operation's completion.
fn wait_for_sanitize(
    mut status: impl FnMut() -> Result<(SanitizeState, u16), String>,
    progress: &mut dyn FnMut(u8),
) -> Result<bool, String> {
    loop {
        let (state, done) = status()?;
        match state {
            SanitizeState::InProgress => {
                progress((u32::from(done) * 100 / 65_536) as u8);
                std::thread::sleep(POLL);
            }
            SanitizeState::Idle => {
                progress(100);
                return Ok(true);
            }
            SanitizeState::Failed => return Ok(false),
        }
    }
}

impl ErasureTarget for DriveTarget {
    fn io(&mut self) -> &mut dyn BlockIo {
        &mut self.drive
    }

    fn firmware(&mut self, method: Method, progress: &mut dyn FnMut(u8)) -> Result<bool, String> {
        let file = self.drive.file();
        let text = |error: oblivio_device::DeviceError| error.to_string();
        match method {
            Method::NvmeSanitizeBlockErase | Method::NvmeSanitizeCryptoErase => {
                let nvme = NvmeDevice::new(file).map_err(text)?;
                let action = if method == Method::NvmeSanitizeBlockErase {
                    SanitizeAction::BlockErase
                } else {
                    SanitizeAction::CryptoErase
                };
                nvme.start_sanitize(action).map_err(text)?;
                wait_for_sanitize(|| nvme.sanitize_status().map_err(text), progress)
            }
            Method::NvmeFormatCryptoErase | Method::NvmeFormatUserDataErase => {
                let nvme = NvmeDevice::new(file).map_err(text)?;
                let ses = if method == Method::NvmeFormatCryptoErase {
                    SecureErase::Cryptographic
                } else {
                    SecureErase::UserData
                };
                nvme.format(ses).map_err(text)?;
                progress(100);
                Ok(true)
            }
            Method::AtaSanitizeBlockErase
            | Method::AtaSanitizeCryptoScramble
            | Method::AtaSanitizeOverwrite => {
                let ata = AtaDevice::new(file);
                let operation = match method {
                    Method::AtaSanitizeBlockErase => SanitizeOperation::BlockErase,
                    Method::AtaSanitizeCryptoScramble => SanitizeOperation::CryptoScramble,
                    _ => SanitizeOperation::Overwrite,
                };
                ata.start_sanitize(operation).map_err(text)?;
                wait_for_sanitize(
                    || {
                        ata.sanitize_status()
                            .map(|(status, done)| (status.state, done))
                            .map_err(text)
                    },
                    progress,
                )
            }
            Method::AtaSecurityEraseEnhanced | Method::AtaSecurityEraseNormal => {
                // SECURITY ERASE UNIT blocks until done: no progress until then.
                progress(0);
                let mut drive = AtaSecurity {
                    ata: AtaDevice::new(file),
                };
                let erased = security::security_erase(
                    &mut drive,
                    &mut self.record,
                    method == Method::AtaSecurityEraseEnhanced,
                )
                .map_err(text)?;
                progress(100);
                Ok(erased)
            }
            Method::HostOverwrite => Err("a host overwrite is not a firmware method".to_owned()),
        }
    }

    fn resume_firmware(
        &mut self,
        method: Method,
        progress: &mut dyn FnMut(u8),
    ) -> Result<bool, String> {
        // The drive carries a Sanitize on across resets and power cycles:
        // only wait for its outcome, never start it again here.
        let file = self.drive.file();
        let text = |error: oblivio_device::DeviceError| error.to_string();
        match method {
            Method::NvmeSanitizeBlockErase | Method::NvmeSanitizeCryptoErase => {
                let nvme = NvmeDevice::new(file).map_err(text)?;
                wait_for_sanitize(|| nvme.sanitize_status().map_err(text), progress)
            }
            Method::AtaSanitizeBlockErase
            | Method::AtaSanitizeCryptoScramble
            | Method::AtaSanitizeOverwrite => {
                let ata = AtaDevice::new(file);
                wait_for_sanitize(
                    || {
                        ata.sanitize_status()
                            .map(|(status, done)| (status.state, done))
                            .map_err(text)
                    },
                    progress,
                )
            }
            _ => Err(format!("{method:?} cannot be resumed")),
        }
    }

    fn smart_after(&mut self) -> Option<AtaSmart> {
        if self.drive.interface() != oblivio_device::drive::Interface::Ata {
            return None;
        }
        let (failing, attributes) = AtaDevice::new(self.drive.file()).smart().ok()?;
        Some(AtaSmart {
            failing,
            reallocated_sectors: attributes.reallocated_sectors,
            reported_uncorrectable: attributes.reported_uncorrectable,
            pending_sectors: attributes.pending_sectors,
            offline_uncorrectable: attributes.offline_uncorrectable,
        })
    }

    fn simulated(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_wait_reports_progress_until_completion() {
        let states = std::cell::RefCell::new(vec![
            (SanitizeState::Idle, 0),
            (SanitizeState::InProgress, 32_768),
            (SanitizeState::InProgress, 0),
        ]);
        let mut seen = Vec::new();
        let outcome = wait_for_sanitize(
            || {
                states
                    .borrow_mut()
                    .pop()
                    .ok_or_else(|| "no state".to_owned())
            },
            &mut |percent| seen.push(percent),
        );
        assert_eq!(outcome, Ok(true));
        assert_eq!(seen, [0, 50, 100]);
    }

    #[test]
    fn failed_sanitize_is_reported() {
        let outcome = wait_for_sanitize(|| Ok((SanitizeState::Failed, 0)), &mut |_| {});
        assert_eq!(outcome, Ok(false));
    }
}
