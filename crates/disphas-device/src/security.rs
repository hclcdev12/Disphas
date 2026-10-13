//! ATA Security Erase, safely (decided with the owner on 2026-10-04).
//!
//! The drive accepts SECURITY ERASE UNIT only once a user password is set;
//! on success it clears the password itself. If anything fails in between,
//! the drive stays locked with that password. So:
//!
//! 1. a random password (32 alphanumeric characters) is generated per erase;
//! 2. it is **recorded durably before** being set, with the drive identity;
//! 3. it is set at security level High, so the master password still works;
//! 4. after the erase, the security state is read again: it must be off;
//! 5. on any failure the password is removed (unlock, disable) and the
//!    record is kept until it is; at the next start, leftover records are
//!    used to restore the drives ([`restore`]).
//!
//! The sequence is written against two traits, so every failure path is
//! tested without hardware.

use zeroize::Zeroizing;

use crate::DeviceError;

/// A Security Erase password: 32 ASCII alphanumeric characters, wiped on drop.
pub struct Password(Zeroizing<[u8; 32]>);

impl Password {
    /// A new random password from the operating system generator.
    pub fn generate() -> Result<Self, DeviceError> {
        const ALPHABET: &[u8; 62] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut password = Zeroizing::new([0u8; 32]);
        let mut filled = 0;
        while filled < 32 {
            let mut random = Zeroizing::new([0u8; 64]);
            getrandom::fill(random.as_mut())
                .map_err(|error| DeviceError::Command(format!("random generator: {error}")))?;
            // Rejection sampling: 248 = 4 × 62 keeps the choice uniform.
            for &byte in random.iter().filter(|&&byte| byte < 248) {
                if filled == 32 {
                    break;
                }
                password[filled] = ALPHABET[usize::from(byte % 62)];
                filled += 1;
            }
        }
        Ok(Self(password))
    }

    /// A password read back from a recovery record.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// A password copied straight from `bytes` into its wiped buffer, with
    /// no intermediate copy; `None` unless exactly 32 bytes.
    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        let mut password = Zeroizing::new([0u8; 32]);
        if bytes.len() != password.len() {
            return None;
        }
        password.copy_from_slice(bytes);
        Some(Self(password))
    }

    /// The 32 bytes, for the drive and the recovery record only.
    pub fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Password(<redacted>)")
    }
}

/// Security state from IDENTIFY DEVICE word 128 (and words 89/90).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecurityState {
    /// Security feature set supported.
    pub supported: bool,
    /// A user password is set.
    pub enabled: bool,
    /// Locked.
    pub locked: bool,
    /// Frozen.
    pub frozen: bool,
    /// Password attempt counter expired.
    pub attempts_expired: bool,
    /// Enhanced erase supported.
    pub enhanced_supported: bool,
    /// Estimated erase time for the requested mode, in minutes.
    pub erase_minutes: Option<u32>,
}

/// The security commands of a drive.
pub trait SecurityDrive {
    /// Reads the security state (IDENTIFY DEVICE). `enhanced` selects which
    /// erase time estimate is returned.
    fn state(&mut self, enhanced: bool) -> Result<SecurityState, DeviceError>;
    /// SECURITY SET PASSWORD (user, level High).
    fn set_password(&mut self, password: &Password) -> Result<(), DeviceError>;
    /// SECURITY ERASE PREPARE then SECURITY ERASE UNIT.
    fn erase(
        &mut self,
        password: &Password,
        enhanced: bool,
        timeout_ms: u32,
    ) -> Result<(), DeviceError>;
    /// SECURITY UNLOCK (user).
    fn unlock(&mut self, password: &Password) -> Result<(), DeviceError>;
    /// SECURITY DISABLE PASSWORD (user).
    fn disable(&mut self, password: &Password) -> Result<(), DeviceError>;
}

/// Durable storage of the password while it is set on the drive.
pub trait RecoveryRecord {
    /// Writes the record and makes it durable (fsync) before returning.
    fn save(&mut self, password: &Password) -> Result<(), DeviceError>;
    /// Deletes the record, once the drive no longer has the password.
    fn clear(&mut self) -> Result<(), DeviceError>;
}

/// Shortest timeout for SECURITY ERASE UNIT, whatever the drive estimates.
const MIN_TIMEOUT_MINUTES: u32 = 60;

/// Timeout of the erase: twice the drive's estimate, one hour at least.
pub fn erase_timeout_ms(estimate_minutes: Option<u32>) -> u32 {
    let minutes = estimate_minutes
        .map_or(MIN_TIMEOUT_MINUTES, |estimate| estimate.saturating_mul(2))
        .max(MIN_TIMEOUT_MINUTES);
    minutes.saturating_mul(60_000)
}

/// Removes our password from a drive: unlock if locked, then disable. True
/// when the drive is back to security disabled.
pub fn restore(drive: &mut dyn SecurityDrive, password: &Password) -> Result<bool, DeviceError> {
    let state = drive.state(false)?;
    if !state.enabled {
        return Ok(true);
    }
    if state.locked {
        drive.unlock(password)?;
    }
    drive.disable(password)?;
    Ok(!drive.state(false)?.enabled)
}

/// Runs a Security Erase. `Ok(true)`: the drive reports the erase completed
/// and its security is off again. `Ok(false)`: the erase failed or could not
/// be confirmed, the password was removed. `Err`: the drive was refused, or
/// is left with the password (the record is then kept for [`restore`]).
pub fn security_erase(
    drive: &mut dyn SecurityDrive,
    record: &mut dyn RecoveryRecord,
    enhanced: bool,
) -> Result<bool, DeviceError> {
    let state = drive.state(enhanced)?;
    let refuse = |reason: &str| Err(DeviceError::Refused(format!("Security Erase: {reason}")));
    if !state.supported {
        return refuse("not supported");
    }
    if state.frozen {
        return refuse("security frozen (B1)");
    }
    if state.locked || state.attempts_expired {
        return refuse("drive locked (B2/B6)");
    }
    if state.enabled {
        return refuse("a password is already set by someone else (B7)");
    }
    if enhanced && !state.enhanced_supported {
        return refuse("enhanced erase not supported");
    }

    let password = Password::generate()?;
    // Durable before the drive knows the password: never a password we lose.
    record.save(&password)?;
    if let Err(error) = drive.set_password(&password) {
        // The password may or may not have been applied: check and clean up.
        return match restore(drive, &password) {
            Ok(true) => {
                record.clear()?;
                Err(error)
            }
            _ => Err(DeviceError::Command(format!(
                "{error}; the drive may keep the recorded password"
            ))),
        };
    }
    if !drive.state(enhanced)?.enabled {
        record.clear()?;
        return Err(DeviceError::Command(
            "the drive did not apply the password".to_owned(),
        ));
    }

    let erased = drive.erase(&password, enhanced, erase_timeout_ms(state.erase_minutes));
    let after = drive.state(enhanced);
    if erased.is_ok() && after.as_ref().is_ok_and(|state| !state.enabled) {
        // Success: the drive cleared the password itself.
        record.clear()?;
        return Ok(true);
    }
    // Failed, or the password is still set: remove it, keep the record if
    // that fails too.
    match restore(drive, &password) {
        Ok(true) => {
            record.clear()?;
            Ok(false)
        }
        Ok(false) | Err(_) => Err(DeviceError::Command(
            "Security Erase failed and the password could not be removed; \
             the recovery record is kept"
                .to_owned(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A simulated drive following the ATA security state machine.
    #[derive(Default)]
    struct FakeDrive {
        enabled: bool,
        locked: bool,
        frozen: bool,
        password: Option<[u8; 32]>,
        fail_set: bool,
        apply_set: bool,
        fail_erase: bool,
        erase_keeps_password: bool,
        fail_disable: bool,
        erased: bool,
        log: Vec<&'static str>,
        saw_record: bool,
        record_saved: std::rc::Rc<std::cell::Cell<bool>>,
    }

    impl FakeDrive {
        fn new(record: &Record) -> Self {
            Self {
                apply_set: true,
                record_saved: std::rc::Rc::clone(&record.saved),
                ..Self::default()
            }
        }
    }

    impl SecurityDrive for FakeDrive {
        fn state(&mut self, _: bool) -> Result<SecurityState, DeviceError> {
            Ok(SecurityState {
                supported: true,
                enabled: self.enabled,
                locked: self.locked,
                frozen: self.frozen,
                attempts_expired: false,
                enhanced_supported: true,
                erase_minutes: Some(2),
            })
        }

        fn set_password(&mut self, password: &Password) -> Result<(), DeviceError> {
            self.log.push("set");
            self.saw_record = self.record_saved.get();
            if self.apply_set {
                self.enabled = true;
                self.password = Some(*password.bytes());
            }
            if self.fail_set {
                return Err(DeviceError::Command("set failed".to_owned()));
            }
            Ok(())
        }

        fn erase(&mut self, password: &Password, _: bool, _: u32) -> Result<(), DeviceError> {
            self.log.push("erase");
            assert_eq!(
                Some(*password.bytes()),
                self.password,
                "erase with our password"
            );
            if self.fail_erase {
                // Interrupted erase: the drive stays locked with the password.
                self.locked = true;
                return Err(DeviceError::Command("erase aborted".to_owned()));
            }
            self.erased = true;
            if !self.erase_keeps_password {
                self.enabled = false;
                self.password = None;
            }
            Ok(())
        }

        fn unlock(&mut self, password: &Password) -> Result<(), DeviceError> {
            self.log.push("unlock");
            if Some(*password.bytes()) == self.password {
                self.locked = false;
            }
            Ok(())
        }

        fn disable(&mut self, password: &Password) -> Result<(), DeviceError> {
            self.log.push("disable");
            if self.fail_disable {
                return Err(DeviceError::Command("disable failed".to_owned()));
            }
            if !self.locked && Some(*password.bytes()) == self.password {
                self.enabled = false;
                self.password = None;
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct Record {
        saved: std::rc::Rc<std::cell::Cell<bool>>,
        password: Option<[u8; 32]>,
    }

    impl RecoveryRecord for Record {
        fn save(&mut self, password: &Password) -> Result<(), DeviceError> {
            self.saved.set(true);
            self.password = Some(*password.bytes());
            Ok(())
        }

        fn clear(&mut self) -> Result<(), DeviceError> {
            self.saved.set(false);
            self.password = None;
            Ok(())
        }
    }

    #[test]
    fn passwords_are_random_alphanumeric_and_redacted() {
        let a = Password::generate().expect("random");
        let b = Password::generate().expect("random");
        assert!(a.bytes().iter().all(u8::is_ascii_alphanumeric));
        assert_ne!(a.bytes(), b.bytes());
        assert_eq!(format!("{a:?}"), "Password(<redacted>)");
    }

    #[test]
    fn successful_erase_records_before_setting_and_clears_after() {
        let mut record = Record::default();
        let mut drive = FakeDrive::new(&record);
        assert!(security_erase(&mut drive, &mut record, true).expect("erase"));
        assert!(
            drive.saw_record,
            "the password was recorded before being set"
        );
        assert!(drive.erased && !drive.enabled);
        assert!(record.password.is_none(), "record cleared after success");
        assert_eq!(drive.log, ["set", "erase"]);
    }

    #[test]
    fn interrupted_erase_unlocks_disables_and_reports_failure() {
        let mut record = Record::default();
        let mut drive = FakeDrive {
            fail_erase: true,
            ..FakeDrive::new(&record)
        };
        assert!(!security_erase(&mut drive, &mut record, false).expect("restored"));
        assert!(!drive.enabled && !drive.locked, "drive given back usable");
        assert!(record.password.is_none());
        assert_eq!(drive.log, ["set", "erase", "unlock", "disable"]);
    }

    #[test]
    fn erase_that_leaves_the_password_is_not_trusted() {
        let mut record = Record::default();
        let mut drive = FakeDrive {
            erase_keeps_password: true,
            ..FakeDrive::new(&record)
        };
        assert!(!security_erase(&mut drive, &mut record, false).expect("restored"));
        assert!(!drive.enabled);
    }

    #[test]
    fn unrecoverable_drive_keeps_the_record() {
        let mut record = Record::default();
        let mut drive = FakeDrive {
            fail_erase: true,
            fail_disable: true,
            ..FakeDrive::new(&record)
        };
        assert!(security_erase(&mut drive, &mut record, false).is_err());
        assert_eq!(
            record.password, drive.password,
            "the recorded password is the one left on the drive"
        );
        // Later, at the next start, the record restores the drive.
        drive.fail_disable = false;
        let password = Password::from_bytes(record.password.expect("kept"));
        assert!(restore(&mut drive, &password).expect("restore"));
        assert!(!drive.enabled && !drive.locked);
    }

    #[test]
    fn failed_set_password_is_cleaned_up() {
        let mut record = Record::default();
        let mut drive = FakeDrive {
            fail_set: true,
            ..FakeDrive::new(&record)
        };
        assert!(security_erase(&mut drive, &mut record, false).is_err());
        assert!(!drive.enabled, "a half-applied password is removed");
        assert!(record.password.is_none());
        assert!(!drive.erased);
    }

    #[test]
    fn foreign_password_frozen_or_locked_drives_are_refused_untouched() {
        for setup in [
            FakeDrive {
                enabled: true,
                ..FakeDrive::default()
            },
            FakeDrive {
                frozen: true,
                ..FakeDrive::default()
            },
            FakeDrive {
                enabled: true,
                locked: true,
                ..FakeDrive::default()
            },
        ] {
            let mut record = Record::default();
            let mut drive = setup;
            assert!(matches!(
                security_erase(&mut drive, &mut record, false),
                Err(DeviceError::Refused(_))
            ));
            assert!(drive.log.is_empty(), "nothing sent to the drive");
            assert!(record.password.is_none());
        }
    }

    #[test]
    fn timeout_is_twice_the_estimate_and_at_least_an_hour() {
        assert_eq!(erase_timeout_ms(None), 60 * 60_000);
        assert_eq!(erase_timeout_ms(Some(10)), 60 * 60_000);
        assert_eq!(erase_timeout_ms(Some(300)), 600 * 60_000);
    }
}
