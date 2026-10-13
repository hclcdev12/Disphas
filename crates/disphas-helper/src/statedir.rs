//! The helper's state folder (`/var/lib/disphas`) and its subfolders must be
//! the helper's own: owned by the account it runs as (root in release),
//! never a symbolic link, never writable by anyone else (`docs/securite.md`,
//! S1). Otherwise a user could swap the workstation key, plant a job resumed
//! without confirmation, or redirect root's writes. Checked before anything
//! is read or written; the helper refuses to start when a check fails.

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

/// Subfolders the helper uses.
pub const SUBFOLDERS: &[&str] = &["keys", "license", "jobs", "recovery", "reports"];

/// Restricts every file the process creates to its owner.
pub fn restrict_umask() {
    rustix::process::umask(rustix::fs::Mode::from_raw_mode(0o077));
}

/// Checks one folder: a real directory owned by `owner`, writable by no one
/// else.
fn check(path: &Path, owner: u32) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("{} is a symbolic link", path.display()));
    }
    if !metadata.is_dir() {
        return Err(format!("{} is not a directory", path.display()));
    }
    if metadata.uid() != owner {
        return Err(format!(
            "{} is owned by uid {}, not {owner}",
            path.display(),
            metadata.uid()
        ));
    }
    if metadata.mode() & 0o022 != 0 {
        return Err(format!(
            "{} is writable by its group or by others",
            path.display()
        ));
    }
    Ok(())
}

/// Creates the state folder if needed (0700: root only; operators read the
/// reports through the helper), then checks it and every existing subfolder.
pub fn prepare(dir: &Path, owner: u32) -> Result<(), String> {
    let exists = fs::symlink_metadata(dir).is_ok();
    if !exists {
        fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("{}: {error}", dir.display()))?;
    }
    check(dir, owner)?;
    for name in SUBFOLDERS {
        let path = dir.join(name);
        if fs::symlink_metadata(&path).is_ok() {
            check(&path, owner)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own_uid() -> u32 {
        crate::startup::effective_uid(&fs::read_to_string("/proc/self/status").expect("status"))
            .expect("uid")
    }

    #[test]
    fn a_new_state_folder_is_created_0700_and_accepted() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let state = dir.path().join("disphas");
        prepare(&state, own_uid()).expect("prepared");
        let mode = fs::metadata(&state).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        fs::create_dir(state.join("keys")).expect("keys");
        fs::set_permissions(state.join("keys"), fs::Permissions::from_mode(0o700)).expect("chmod");
        prepare(&state, own_uid()).expect("still fine");
    }

    #[test]
    fn writable_linked_or_foreign_folders_are_refused() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let state = dir.path().join("disphas");
        prepare(&state, own_uid()).expect("prepared");

        fs::set_permissions(&state, fs::Permissions::from_mode(0o777)).expect("chmod");
        assert!(prepare(&state, own_uid()).is_err(), "world-writable state");
        fs::set_permissions(&state, fs::Permissions::from_mode(0o755)).expect("chmod");

        std::os::unix::fs::symlink(dir.path(), state.join("jobs")).expect("symlink");
        assert!(prepare(&state, own_uid()).is_err(), "linked subfolder");
        fs::remove_file(state.join("jobs")).expect("unlink");

        fs::create_dir(state.join("recovery")).expect("recovery");
        fs::set_permissions(state.join("recovery"), fs::Permissions::from_mode(0o770))
            .expect("chmod");
        assert!(
            prepare(&state, own_uid()).is_err(),
            "group-writable subfolder"
        );
        fs::set_permissions(state.join("recovery"), fs::Permissions::from_mode(0o700))
            .expect("chmod");

        assert!(prepare(&state, own_uid() + 1).is_err(), "another owner");
        prepare(&state, own_uid()).expect("back to normal");
    }

    #[test]
    fn a_linked_state_folder_is_refused() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let real = dir.path().join("real");
        fs::create_dir(&real).expect("dir");
        fs::set_permissions(&real, fs::Permissions::from_mode(0o755)).expect("chmod");
        let link = dir.path().join("disphas");
        std::os::unix::fs::symlink(&real, &link).expect("symlink");
        assert!(prepare(&link, own_uid()).is_err());
    }
}
