//! Oblivio root helper.
//!
//! Launched once per session via `pkexec`, it talks to the UI over a private
//! stdin/stdout JSON pipe. See `docs/protocole_ipc.md`.
//!
//! Not wired to `oblivio-device` yet: it sees no drive and cannot touch one.

use std::io;
use std::process::ExitCode;

use oblivio_core::RuleSet;
#[cfg(not(feature = "demo"))]
use oblivio_helper::backend::NoDevices;
use oblivio_helper::licensing::{self, StoreLicensing};
use oblivio_helper::serve::serve;
use oblivio_helper::session::{Session, SessionConfig};
use oblivio_license::LicenseStore;

fn main() -> ExitCode {
    // Release builds: root, started by pkexec, or nothing (protocole_ipc.md §3).
    #[cfg(not(debug_assertions))]
    if let Err(error) = oblivio_helper::startup::check_this_process() {
        eprintln!("oblivio-helper: {error}");
        return ExitCode::FAILURE;
    }
    let config = SessionConfig {
        rules: RuleSet::baseline(),
        binary_sha256: own_sha256(),
        reports_dir: licensing::state_dir().join("reports"),
        licensing: Box::new(StoreLicensing::new(
            LicenseStore::new(licensing::state_dir()),
            oblivio_license::trusted_roots(),
        )),
    };
    #[cfg(feature = "demo")]
    let backend =
        oblivio_helper::demo::DemoDevices::with_images(licensing::state_dir().join("demo-images"));
    #[cfg(not(feature = "demo"))]
    let backend = NoDevices;
    let mut session = Session::new(backend, config);
    match serve(io::stdin().lock(), io::stdout(), &mut session) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("oblivio-helper: I/O error: {error}");
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
