//! disphas root helper.
//!
//! Launched once per session via `pkexec`, it talks to the UI over a private
//! stdin/stdout JSON pipe. See `docs/protocole_ipc.md`.
//!
//! Release builds work on the real drives of the machine; development
//! builds use demo drives or none (see the `demo` and `real-devices` features).

use std::io;
use std::process::ExitCode;

use disphas_core::RuleSet;
#[cfg(all(not(feature = "demo"), debug_assertions, not(feature = "real-devices")))]
use disphas_helper::backend::NoDevices;
use disphas_helper::licensing::{self, StoreLicensing};
use disphas_helper::serve::serve;
use disphas_helper::session::{Session, SessionConfig};
use disphas_license::LicenseStore;

fn main() -> ExitCode {
    // Files are private unless a mode is set explicitly (docs/securite.md, S1).
    disphas_helper::statedir::restrict_umask();
    // Release builds: root, started by pkexec, or nothing (protocole_ipc.md §3).
    #[cfg(not(debug_assertions))]
    if let Err(error) = disphas_helper::startup::check_this_process() {
        eprintln!("disphas-helper: {error}");
        return ExitCode::FAILURE;
    }
    // The state folder must be ours alone before anything is read from it.
    let owner = match std::fs::read_to_string("/proc/self/status")
        .map_err(|error| error.to_string())
        .and_then(|status| {
            disphas_helper::startup::effective_uid(&status).map_err(|error| error.to_string())
        }) {
        Ok(uid) => uid,
        Err(error) => {
            eprintln!("disphas-helper: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = disphas_helper::statedir::prepare(&licensing::state_dir(), owner) {
        eprintln!("disphas-helper: unsafe state folder, refusing to start: {error}");
        return ExitCode::FAILURE;
    }
    let config = SessionConfig {
        rules: RuleSet::baseline(),
        entitlements: disphas_core::EntitlementTable::baseline(),
        binary_sha256: own_sha256(),
        reports_dir: licensing::state_dir().join("reports"),
        jobs_dir: Some(licensing::state_dir().join("jobs")),
        licensing: Box::new(StoreLicensing::new(
            LicenseStore::new(licensing::state_dir()),
            disphas_license::trusted_roots(),
        )),
    };
    // Development builds never touch real drives unless `real-devices` is
    // enabled explicitly (test bench); release builds always use them.
    #[cfg(feature = "demo")]
    let backend =
        disphas_helper::demo::DemoDevices::with_images(licensing::state_dir().join("demo-images"));
    #[cfg(all(
        not(feature = "demo"),
        any(not(debug_assertions), feature = "real-devices")
    ))]
    let mut backend =
        disphas_helper::system::SystemBackend::new(licensing::state_dir().join("recovery"));
    #[cfg(all(
        not(feature = "demo"),
        any(not(debug_assertions), feature = "real-devices")
    ))]
    for line in backend.restore_pending() {
        eprintln!("disphas-helper: Security Erase recovery: {line}");
    }
    #[cfg(all(not(feature = "demo"), debug_assertions, not(feature = "real-devices")))]
    let backend = NoDevices;
    let mut session = Session::new(backend, config);
    match serve(io::stdin().lock(), io::stdout(), &mut session) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("disphas-helper: I/O error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// SHA-256 of this executable, written into every attestation. Zero if the
/// executable cannot be read, which the attestation then shows plainly.
fn own_sha256() -> [u8; 32] {
    use sha2::{Digest, Sha256};
    std::fs::read("/proc/self/exe")
        .map(|bytes| Sha256::digest(&bytes).into())
        .unwrap_or([0; 32])
}
