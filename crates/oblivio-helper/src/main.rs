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
    let config = SessionConfig {
        rules: RuleSet::baseline(),
        // TODO: hash /proc/self/exe once the helper is packaged.
        binary_sha256: String::new(),
        licensing: Box::new(StoreLicensing::new(
            LicenseStore::new(licensing::state_dir()),
            licensing::root_key(),
        )),
    };
    #[cfg(feature = "demo")]
    let backend = oblivio_helper::demo::DemoDevices::default();
    #[cfg(not(feature = "demo"))]
    let backend = NoDevices;
    let mut session = Session::new(backend, config);
    match serve(io::stdin().lock(), io::stdout().lock(), &mut session) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("oblivio-helper: I/O error: {error}");
            ExitCode::FAILURE
        }
    }
}
