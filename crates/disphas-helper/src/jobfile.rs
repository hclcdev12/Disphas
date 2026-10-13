//! Job state on disk, in `<state>/jobs/` (directory 0700, files 0600, root
//! only), so a job survives a helper crash (`docs/protocole_ipc.md` §10).
//!
//! `<id>.json` is rewritten atomically at every phase change and after every
//! attempt, and deleted once the attestation is stored. `<id>.lock` is held
//! (`flock`) by the process running the job: a second helper never resumes or
//! discards a job that is still alive.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use disphas_core::method::Method;
use disphas_core::outcome::AttemptReport;
use disphas_core::profile::DeviceProfile;
use disphas_protocol::command::DeviceRef;
use disphas_protocol::response::JobPhase;
use serde::{Deserialize, Serialize};

use crate::job::Plan;

/// Version of the file layout. A file of another version is not resumed.
pub const FORMAT: u32 = 1;

/// One attempt, as recorded.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptFile {
    /// What the engine evaluates.
    pub report: AttemptReport,
    /// Why the attempt failed, if it did.
    pub error: Option<String>,
}

/// Everything needed to finish a job after a crash.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobFile {
    /// Layout version, [`FORMAT`].
    pub format: u32,
    /// Report identifier, hex.
    pub id: String,
    /// The drive as named on the wire.
    pub device: DeviceRef,
    /// Its profile at inspection.
    pub profile: DeviceProfile,
    /// Capacity reported by the backend.
    pub capacity_bytes: Option<u64>,
    /// What is executed.
    pub plan: Plan,
    /// Rule set the verdict was issued with.
    pub rules_version: String,
    /// Declarative operator name.
    pub operator: Option<String>,
    /// Declarative organization.
    pub organization: Option<String>,
    /// Batch identifier and label.
    #[serde(default)]
    pub batch: Option<(String, Option<String>)>,
    /// Host overwrite ends with a pass of zeros.
    #[serde(default)]
    pub zero_pass: bool,
    /// Last phase entered.
    pub phase: JobPhase,
    /// Validation seed, hex.
    pub seed: String,
    /// Sample blocks.
    pub samples: Vec<u64>,
    /// Finished attempts.
    pub attempts: Vec<AttemptFile>,
    /// Timeline of the journal.
    pub timeline: Vec<serde_json::Value>,
}

impl JobFile {
    /// Whether a crash during this job can be recovered: only a firmware
    /// Sanitize that was started, because the drive carries it on by itself
    /// and reports its outcome in the sanitize status log. Anything else is
    /// interrupted without attestation.
    pub fn resumable(&self, rules_version: &str) -> bool {
        let method = match &self.plan {
            Plan::Ready { option, .. } => option.method,
            Plan::Forced(forced) => forced.option.method,
        };
        let sanitize = matches!(
            method,
            Method::NvmeSanitizeBlockErase
                | Method::NvmeSanitizeCryptoErase
                | Method::AtaSanitizeBlockErase
                | Method::AtaSanitizeCryptoScramble
                | Method::AtaSanitizeOverwrite
        );
        let started = matches!(
            self.phase,
            JobPhase::Executing | JobPhase::Verifying | JobPhase::Validating | JobPhase::Signing
        );
        self.format == FORMAT && sanitize && started && self.rules_version == rules_version
    }
}

/// A job file owned by this process, through its lock.
#[derive(Debug)]
pub struct JobHandle {
    json: PathBuf,
    lock_path: PathBuf,
    /// Held for as long as the job runs; released when dropped.
    _lock: File,
}

fn create_dir(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

/// Takes the lock of job `id` in `dir`, or `None` if another process holds it.
fn lock(dir: &Path, id: &str) -> std::io::Result<Option<JobHandle>> {
    let lock_path = dir.join(format!("{id}.lock"));
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&lock_path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(JobHandle {
            json: dir.join(format!("{id}.json")),
            lock_path,
            _lock: file,
        })),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(error)) => Err(error),
    }
}

/// Writes `content` to `path` atomically and durably, mode 0600.
pub fn write_atomic(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let temporary = path.with_extension("tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(content)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    sync_parent(path)
}

fn sync_parent(path: &Path) -> std::io::Result<()> {
    match path.parent() {
        Some(dir) => File::open(dir)?.sync_all(),
        None => Ok(()),
    }
}

impl JobHandle {
    /// Creates the files of a new job. Fails rather than run a job that
    /// could not be recovered.
    pub fn create(dir: &Path, file: &JobFile) -> Result<Self, String> {
        let fail = |error: std::io::Error| format!("jobs: {error}");
        create_dir(dir).map_err(fail)?;
        let handle = lock(dir, &file.id)
            .map_err(fail)?
            .ok_or("jobs: identifier already in use")?;
        handle.save(file)?;
        Ok(handle)
    }

    /// Records the current state.
    pub fn save(&self, file: &JobFile) -> Result<(), String> {
        let content = serde_json::to_vec_pretty(file).map_err(|error| error.to_string())?;
        write_atomic(&self.json, &content).map_err(|error| format!("jobs: {error}"))
    }

    /// Deletes the files: the job ended (attestation stored, or discarded as
    /// interrupted).
    pub fn finish(self) -> Result<(), String> {
        let fail = |error: std::io::Error| format!("jobs: {error}");
        match fs::remove_file(&self.json) {
            Ok(()) => sync_parent(&self.json).map_err(fail)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(fail(error)),
        }
        // Removed while still locked: no other process can take it meanwhile.
        fs::remove_file(&self.lock_path).map_err(fail)
    }
}

/// A job left by a previous helper, now owned by this one.
#[derive(Debug)]
pub struct Left {
    /// Its state.
    pub file: JobFile,
    /// Its lock.
    pub handle: JobHandle,
}

/// Jobs left in `dir` by helpers that are gone. Jobs still locked by a live
/// helper are skipped; unreadable files are reported and kept.
pub fn left(dir: &Path) -> Vec<Left> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let handle = match lock(dir, id) {
            Ok(Some(handle)) => handle,
            Ok(None) => continue,
            Err(error) => {
                eprintln!("disphas-helper: job {id}: {error}");
                continue;
            }
        };
        let parsed = fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                serde_json::from_slice::<JobFile>(&bytes).map_err(|error| error.to_string())
            });
        match parsed {
            Ok(file) if file.id == id => found.push(Left { file, handle }),
            Ok(_) => eprintln!("disphas-helper: job {id}: identifier mismatch, kept"),
            Err(error) => eprintln!("disphas-helper: job {id}: unreadable, kept: {error}"),
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_locked_by_a_live_helper_is_left_alone() {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::write(dir.path().join("0001.json"), b"{}").expect("file");
        let live = lock(dir.path(), "0001").expect("lock").expect("free");
        assert!(left(dir.path()).is_empty(), "locked: skipped");
        assert!(lock(dir.path(), "0001").expect("lock").is_none());
        drop(live);
        // Free again, but unreadable: reported and kept, never deleted.
        assert!(left(dir.path()).is_empty());
        assert!(dir.path().join("0001.json").exists());
    }

    #[test]
    fn atomic_write_is_private() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("state.json");
        write_atomic(&path, b"one").expect("written");
        write_atomic(&path, b"two").expect("rewritten");
        assert_eq!(std::fs::read(&path).expect("read"), b"two");
        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        assert!(!path.with_extension("tmp").exists());
    }
}
