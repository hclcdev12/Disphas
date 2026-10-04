//! Recovery records of ATA Security Erase passwords, in `<state>/recovery/`
//! (directory 0700, files 0600, root only).
//!
//! A record exists only while Oblivio's password may be set on a drive. It is
//! written durably before the password is set and deleted once the drive
//! no longer has it; a record found at start-up means a drive may be left
//! locked, and is used to restore it. The password is 32 ASCII alphanumeric
//! characters, so a technician can also use it with standard tools.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use oblivio_core::profile::DeviceIdentity;
use oblivio_device::DeviceError;
use oblivio_device::security::{Password, RecoveryRecord};
use zeroize::Zeroizing;

/// The recovery record of one drive.
#[derive(Debug, Clone)]
pub struct FileRecord {
    dir: PathBuf,
    path: PathBuf,
    identity: DeviceIdentity,
}

fn io(error: std::io::Error) -> DeviceError {
    DeviceError::Io(error)
}

/// File name of a drive's record: its serial number, hex-encoded so any
/// character is safe in a path.
fn file_name(serial: &str) -> String {
    let hex: String = serial.bytes().map(|byte| format!("{byte:02x}")).collect();
    format!("ata-{hex}.json")
}

/// JSON string literal of a text (the fields are plain identifiers).
fn quoted(text: &str) -> String {
    serde_json::Value::String(text.to_owned()).to_string()
}

impl FileRecord {
    /// The record of `identity` in `dir`.
    pub fn new(dir: &Path, identity: &DeviceIdentity) -> Self {
        Self {
            dir: dir.to_path_buf(),
            path: dir.join(file_name(&identity.serial)),
            identity: identity.clone(),
        }
    }
}

impl RecoveryRecord for FileRecord {
    fn save(&mut self, password: &Password) -> Result<(), DeviceError> {
        fs::create_dir_all(&self.dir).map_err(io)?;
        fs::set_permissions(&self.dir, fs::Permissions::from_mode(0o700)).map_err(io)?;
        let secret = std::str::from_utf8(password.bytes())
            .map_err(|_| DeviceError::Command("password is not ASCII".to_owned()))?;
        let content = Zeroizing::new(format!(
            "{{\"model\":{},\"serial\":{},\"wwn\":{},\"password\":\"{secret}\"}}\n",
            quoted(&self.identity.model),
            quoted(&self.identity.serial),
            quoted(self.identity.wwn.as_deref().unwrap_or_default()),
        ));
        let temporary = self.path.with_extension("tmp");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(io)?;
        file.write_all(content.as_bytes()).map_err(io)?;
        file.sync_all().map_err(io)?;
        fs::rename(&temporary, &self.path).map_err(io)?;
        File::open(&self.dir)
            .and_then(|dir| dir.sync_all())
            .map_err(io)
    }

    fn clear(&mut self) -> Result<(), DeviceError> {
        match fs::remove_file(&self.path) {
            Ok(()) => File::open(&self.dir)
                .and_then(|dir| dir.sync_all())
                .map_err(io),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io(error)),
        }
    }
}

/// A record left by an interrupted Security Erase.
#[derive(Debug)]
pub struct Pending {
    /// Serial number of the drive.
    pub serial: String,
    /// The password possibly left on it.
    pub password: Password,
    /// The record, to delete once the drive is restored.
    pub record: FileRecord,
}

/// Records left in `dir`. Unreadable records are reported and skipped:
/// they stay on disk for a technician.
pub fn pending(dir: &Path) -> Vec<Pending> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| {
            let text = Zeroizing::new(fs::read_to_string(&path).ok()?);
            let value: serde_json::Value = match serde_json::from_str(&text) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!(
                        "oblivio-helper: unreadable recovery record {}: {error}",
                        path.display()
                    );
                    return None;
                }
            };
            let field = |name: &str| value.get(name).and_then(|v| v.as_str()).map(str::to_owned);
            let password: [u8; 32] = field("password")?.into_bytes().try_into().ok()?;
            let identity = DeviceIdentity {
                model: field("model").unwrap_or_default(),
                serial: field("serial")?,
                firmware: String::new(),
                wwn: field("wwn").filter(|wwn| !wwn.is_empty()),
                eui64: None,
                nguid: None,
            };
            Some(Pending {
                serial: identity.serial.clone(),
                password: Password::from_bytes(password),
                record: FileRecord {
                    dir: dir.to_path_buf(),
                    path,
                    identity,
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> DeviceIdentity {
        DeviceIdentity {
            model: "Test \"SSD\"".to_owned(),
            serial: "SN/../42".to_owned(),
            firmware: "1".to_owned(),
            wwn: Some("5000c500deadbeef".to_owned()),
            eui64: None,
            nguid: None,
        }
    }

    #[test]
    fn saved_record_is_private_durable_and_read_back() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let recovery = dir.path().join("recovery");
        let mut record = FileRecord::new(&recovery, &identity());
        let password = Password::generate().expect("random");
        record.save(&password).expect("saved");

        let mode = |path: &Path| fs::metadata(path).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode(&recovery), 0o700);
        let files: Vec<PathBuf> = fs::read_dir(&recovery)
            .expect("listed")
            .map(|entry| entry.expect("entry").path())
            .collect();
        assert_eq!(files.len(), 1, "no temporary file left");
        assert_eq!(mode(&files[0]), 0o600);
        assert!(
            files[0]
                .file_name()
                .is_some_and(|name| !name.to_string_lossy().contains('/')),
            "serial hex-encoded"
        );

        let found = pending(&recovery);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].serial, "SN/../42");
        assert_eq!(found[0].password.bytes(), password.bytes());
    }

    #[test]
    fn cleared_record_is_gone() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let mut record = FileRecord::new(dir.path(), &identity());
        record
            .save(&Password::generate().expect("random"))
            .expect("saved");
        record.clear().expect("cleared");
        record.clear().expect("clearing twice is fine");
        assert!(pending(dir.path()).is_empty());
    }
}
