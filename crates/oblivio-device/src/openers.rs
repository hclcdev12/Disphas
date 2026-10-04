//! Processes holding a block device open (S9). `O_EXCL` only stops other
//! exclusive openers (mounts, swap, device-mapper…); a program that opened
//! the drive plainly (a disk image tool, a virtual machine) keeps writing to
//! it. Such a drive is refused, as in use.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

use crate::sysfs::DevNum;

/// Process ids, other than `me`, with an open file descriptor on one of
/// `devices`, found through `<proc>/<pid>/fd`. `device_of` gives the block
/// device behind a descriptor link. A process that exits meanwhile is
/// skipped; any other unreadable process fails closed.
pub fn openers(
    proc: &Path,
    devices: &BTreeSet<DevNum>,
    me: u32,
    device_of: impl Fn(&Path) -> Option<DevNum>,
) -> io::Result<Vec<u32>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(proc)? {
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == me {
            continue;
        }
        let descriptors = match fs::read_dir(entry.path().join("fd")) {
            Ok(descriptors) => descriptors,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for descriptor in descriptors {
            let descriptor = match descriptor {
                Ok(descriptor) => descriptor,
                Err(error) if error.kind() == io::ErrorKind::NotFound => break,
                Err(error) => return Err(error),
            };
            if device_of(&descriptor.path()).is_some_and(|device| devices.contains(&device)) {
                found.push(pid);
                break;
            }
        }
    }
    found.sort_unstable();
    Ok(found)
}

/// The block device a descriptor link points to, following it.
pub fn block_device_of(link: &Path) -> Option<DevNum> {
    let metadata = fs::metadata(link).ok()?;
    if !metadata.file_type().is_block_device() {
        return None;
    }
    let rdev = metadata.rdev();
    Some(DevNum {
        major: libc::major(rdev),
        minor: libc::minor(rdev),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn dev(minor: u32) -> DevNum {
        DevNum { major: 8, minor }
    }

    #[test]
    fn processes_with_the_drive_or_a_partition_open_are_found() {
        let proc = tempfile::tempdir().expect("temporary directory");
        // Each descriptor links to a fake node named by its minor number.
        let open = |pid: &str, fd: &str, minor: &str| {
            let dir = proc.path().join(pid).join("fd");
            fs::create_dir_all(&dir).expect("fd dir");
            symlink(format!("/dev/fake-{minor}"), dir.join(fd)).expect("link");
        };
        open("100", "3", "0"); // the drive itself
        open("200", "4", "1"); // one of its partitions
        open("300", "5", "16"); // another drive
        open("42", "6", "0"); // the helper itself
        fs::create_dir_all(proc.path().join("self")).expect("self");
        fs::create_dir_all(proc.path().join("400")).expect("exited: no fd/");
        let device_of = |link: &Path| {
            let target = fs::read_link(link).ok()?;
            let minor = target.to_str()?.strip_prefix("/dev/fake-")?.parse().ok()?;
            Some(dev(minor))
        };
        let drive: BTreeSet<DevNum> = [dev(0), dev(1), dev(2)].into();
        assert_eq!(
            openers(proc.path(), &drive, 42, device_of).expect("scan"),
            [100, 200]
        );
        let other: BTreeSet<DevNum> = [dev(32)].into();
        assert!(
            openers(proc.path(), &other, 42, device_of)
                .expect("scan")
                .is_empty()
        );
    }

    #[test]
    fn a_regular_file_is_not_a_block_device() {
        let file = tempfile::NamedTempFile::new().expect("file");
        assert_eq!(block_device_of(file.path()), None);
    }
}
