//! Launch checks of a release helper (`docs/protocole_ipc.md` §3).
//!
//! A release helper must run as root and be started by pkexec, which sets
//! `PKEXEC_UID` to the user who asked. Anything else is refused before the
//! first message is read.
//!
//! The environment is not rewritten: `std::env::remove_var` is `unsafe` in
//! Rust 2024 and the helper forbids unsafe code. The equivalent guarantee is
//! that a release helper reads no environment variable other than
//! `PKEXEC_UID` and never starts a process; pkexec clears the environment too.

use thiserror::Error;

/// Why a helper refuses to start.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LaunchError {
    /// Not running as root.
    #[error("the helper must run as root (effective uid {0})")]
    NotRoot(u32),
    /// Not started by pkexec.
    #[error("the helper must be started by pkexec")]
    NotPkexec,
    /// `/proc/self/status` cannot be read or parsed.
    #[error("cannot determine the effective uid: {0}")]
    UnknownUid(String),
}

/// Effective uid from the text of `/proc/self/status`
/// (`Uid:` real, effective, saved set, filesystem).
pub fn effective_uid(status: &str) -> Result<u32, LaunchError> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|uids| uids.split_whitespace().nth(1))
        .and_then(|uid| uid.parse().ok())
        .ok_or_else(|| LaunchError::UnknownUid("no Uid line".to_owned()))
}

/// Checks the launch conditions; returns the uid of the user who asked.
pub fn check_launch(effective_uid: u32, pkexec_uid: Option<&str>) -> Result<u32, LaunchError> {
    if effective_uid != 0 {
        return Err(LaunchError::NotRoot(effective_uid));
    }
    pkexec_uid
        .and_then(|uid| uid.parse().ok())
        .ok_or(LaunchError::NotPkexec)
}

/// Checks the launch of this process.
pub fn check_this_process() -> Result<u32, LaunchError> {
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|error| LaunchError::UnknownUid(error.to_string()))?;
    let pkexec_uid = std::env::var("PKEXEC_UID").ok();
    check_launch(effective_uid(&status)?, pkexec_uid.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = "Name:\tdisphas-helper\nUmask:\t0022\nState:\tR (running)\nUid:\t1000\t0\t0\t0\nGid:\t1000\t0\t0\t0\n";

    #[test]
    fn reads_the_effective_uid() {
        assert_eq!(effective_uid(STATUS), Ok(0));
        assert!(effective_uid("Name:\tx\n").is_err());
    }

    #[test]
    fn root_started_by_pkexec_is_accepted() {
        assert_eq!(check_launch(0, Some("1000")), Ok(1000));
    }

    #[test]
    fn anything_else_is_refused() {
        assert_eq!(
            check_launch(1000, Some("1000")),
            Err(LaunchError::NotRoot(1000))
        );
        assert_eq!(check_launch(0, None), Err(LaunchError::NotPkexec));
        assert_eq!(check_launch(0, Some("root")), Err(LaunchError::NotPkexec));
    }
}
