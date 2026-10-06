//! Parsers for `/proc/self/mountinfo` and `/proc/swaps`. Pure: no file access.

use thiserror::Error;

use crate::sysfs::DevNum;

/// One line of `/proc/self/mountinfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountEntry {
    /// `major:minor` of the mounted device; major 0 for virtual or btrfs subvolumes.
    pub dev: DevNum,
    /// Mount point, escapes decoded.
    pub mount_point: String,
    /// Filesystem type.
    pub fs_type: String,
    /// Mount source, such as `/dev/nvme0n1p2`, escapes decoded.
    pub source: String,
    /// Filesystem-specific options (overlay layers…), escapes decoded.
    pub super_options: String,
}

/// One active swap area of `/proc/swaps`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapEntry {
    /// Path of the swap device or file, escapes decoded.
    pub path: String,
    /// `partition` or `file`.
    pub kind: String,
}

/// A line that does not follow the kernel format.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("line {line}: {reason}")]
pub struct ParseError {
    /// 1-based line number.
    pub line: usize,
    /// What is wrong.
    pub reason: &'static str,
}

/// Parses `/proc/self/mountinfo` (`man 5 proc_pid_mountinfo`).
pub fn parse_mountinfo(text: &str) -> Result<Vec<MountEntry>, ParseError> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            let error = |reason| ParseError {
                line: index + 1,
                reason,
            };
            let fields: Vec<&str> = line.split(' ').collect();
            let separator = fields
                .iter()
                .position(|field| *field == "-")
                .ok_or_else(|| error("missing separator"))?;
            if separator < 6 || fields.len() < separator + 3 {
                return Err(error("too few fields"));
            }
            Ok(MountEntry {
                dev: fields[2]
                    .parse()
                    .map_err(|_| error("invalid major:minor"))?,
                mount_point: unescape(fields[4]),
                fs_type: fields[separator + 1].to_owned(),
                source: unescape(fields[separator + 2]),
                super_options: fields
                    .get(separator + 3)
                    .map(|options| unescape(options))
                    .unwrap_or_default(),
            })
        })
        .collect()
}

/// Parses `/proc/swaps`, skipping its header line.
pub fn parse_swaps(text: &str) -> Result<Vec<SwapEntry>, ParseError> {
    text.lines()
        .enumerate()
        .skip(1)
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            let mut fields = line.split_whitespace();
            match (fields.next(), fields.next()) {
                (Some(path), Some(kind)) => Ok(SwapEntry {
                    path: unescape(path),
                    kind: kind.to_owned(),
                }),
                _ => Err(ParseError {
                    line: index + 1,
                    reason: "too few fields",
                }),
            }
        })
        .collect()
}

/// Decodes the kernel's octal escapes (`\040` for a space, `\134` for `\`…).
fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let octal = bytes
            .get(index + 1..index + 4)
            .filter(|digits| {
                bytes[index] == b'\\' && digits.iter().all(|d| (b'0'..=b'7').contains(d))
            })
            .and_then(|digits| {
                let value = digits
                    .iter()
                    .fold(0u32, |acc, digit| acc * 8 + u32::from(digit - b'0'));
                u8::try_from(value).ok()
            });
        match octal {
            Some(byte) => {
                out.push(byte);
                index += 4;
            }
            None => {
                out.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mountinfo_lines() {
        let text = "\
22 1 259:2 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw
25 22 259:1 / /boot/efi rw,relatime shared:5 - vfat /dev/nvme0n1p1 rw,fmask=0077
40 22 0:35 / /mnt/My\\040Disk rw - btrfs /dev/sdb1 rw,subvol=/
";
        let entries = parse_mountinfo(text).expect("valid mountinfo");
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries[0].dev,
            DevNum {
                major: 259,
                minor: 2
            }
        );
        assert_eq!(entries[0].mount_point, "/");
        assert_eq!(entries[1].mount_point, "/boot/efi");
        assert_eq!(entries[2].mount_point, "/mnt/My Disk");
        assert_eq!(entries[2].fs_type, "btrfs");
        assert_eq!(entries[2].source, "/dev/sdb1");
    }

    #[test]
    fn optional_fields_are_skipped() {
        let text = "30 22 8:1 / /data rw shared:7 master:2 - xfs /dev/sda1 rw\n";
        let entries = parse_mountinfo(text).expect("valid mountinfo");
        assert_eq!(entries[0].fs_type, "xfs");
        assert_eq!(entries[0].source, "/dev/sda1");
    }

    #[test]
    fn malformed_mountinfo_is_an_error() {
        assert!(parse_mountinfo("22 1 259:2 / / rw ext4 /dev/x rw\n").is_err());
        assert!(parse_mountinfo("22 1 nope / / rw - ext4 /dev/x rw\n").is_err());
    }

    #[test]
    fn parses_swaps() {
        let text = "\
Filename\t\t\t\tType\t\tSize\t\tUsed\t\tPriority
/dev/dm-1                               partition\t8388604\t\t0\t\t-2
/swap\\040file                           file\t\t1048572\t\t0\t\t-3
";
        let entries = parse_swaps(text).expect("valid swaps");
        assert_eq!(
            entries,
            [
                SwapEntry {
                    path: "/dev/dm-1".to_owned(),
                    kind: "partition".to_owned()
                },
                SwapEntry {
                    path: "/swap file".to_owned(),
                    kind: "file".to_owned()
                },
            ]
        );
    }

    #[test]
    fn unescape_leaves_invalid_sequences_alone() {
        assert_eq!(unescape("a\\9b\\"), "a\\9b\\");
        assert_eq!(unescape("\\134x"), "\\x");
    }
}
