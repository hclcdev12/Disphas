//! Oblivio root helper.
//!
//! Launched once per session via `pkexec`, it talks to the UI over a private
//! stdin/stdout JSON pipe. See `docs/protocole_ipc.md`.
//!
//! Not wired to `oblivio-device` yet: it sees no drive and cannot touch one.

use std::io;
use std::process::ExitCode;

use oblivio_core::RuleSet;
use oblivio_helper::backend::NoDevices;
use oblivio_helper::serve::serve;
use oblivio_helper::session::{Session, SessionConfig};
use oblivio_protocol::response::{LicenseState, LicenseStatusResult};

fn main() -> ExitCode {
    let config = SessionConfig {
        rules: RuleSet::baseline(),
        // TODO: hash /proc/self/exe once the helper is packaged.
        binary_sha256: String::new(),
        // TODO: read the installed license once oblivio-license exists.
        license: LicenseStatusResult {
            state: LicenseState::Missing,
            offer: None,
            license_id: None,
            valid_until: None,
        },
    };
    let mut session = Session::new(NoDevices, config);
    match serve(io::stdin().lock(), io::stdout().lock(), &mut session) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("oblivio-helper: I/O error: {error}");
            ExitCode::FAILURE
        }
    }
}
