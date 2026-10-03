//! Small helpers to read sysfs attributes. Read-only.

use std::fs;
use std::io;
use std::path::Path;
use std::str::FromStr;

/// A block device number, as in sysfs `dev` files and mountinfo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DevNum {
    /// Major number.
    pub major: u32,
    /// Minor number.
    pub minor: u32,
}

impl FromStr for DevNum {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (major, minor) = text.trim().split_once(':').ok_or(())?;
        Ok(Self {
            major: major.parse().map_err(|_| ())?,
            minor: minor.parse().map_err(|_| ())?,
        })
    }
}

/// Reads an attribute, trimmed.
pub(crate) fn read_trimmed(path: &Path) -> io::Result<String> {
    Ok(fs::read_to_string(path)?.trim().to_owned())
}

/// Reads an optional text attribute: `None` if absent, unreadable or blank.
pub(crate) fn read_optional(path: &Path) -> Option<String> {
    read_trimmed(path).ok().filter(|text| !text.is_empty())
}

/// Names of the entries of a directory, sorted.
pub(crate) fn entry_names(path: &Path) -> io::Result<Vec<String>> {
    let mut names = fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<io::Result<Vec<_>>>()?;
    names.sort();
    Ok(names)
}

/// Serial number from a SCSI VPD page 0x80 (Unit Serial Number), as exposed in
/// `device/vpd_pg80`. For ATA drives behind libata, it is the IDENTIFY serial.
pub(crate) fn vpd_pg80_serial(page: &[u8]) -> Option<String> {
    let length = usize::from(*page.get(3)?);
    let serial = page.get(4..4 + length)?;
    let text = String::from_utf8_lossy(serial).trim().to_owned();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_device_numbers() {
        assert_eq!(
            "259:3\n".parse(),
            Ok(DevNum {
                major: 259,
                minor: 3
            })
        );
        assert_eq!("8".parse::<DevNum>(), Err(()));
    }

    #[test]
    fn reads_vpd_serial() {
        let mut page = vec![0x00, 0x80, 0x00, 0x14];
        page.extend_from_slice(b"     WD-WCC4N1234567");
        assert_eq!(vpd_pg80_serial(&page).as_deref(), Some("WD-WCC4N1234567"));
        assert_eq!(vpd_pg80_serial(&[0, 0x80, 0, 10, b'x']), None);
    }
}
