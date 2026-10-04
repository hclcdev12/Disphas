//! A whole helper session fed arbitrary bytes on its input. No drive (the
//! `NoDevices` backend) and a licensing that refuses everything: the fuzzer
//! can neither reach a device nor write a file.
#![no_main]

use libfuzzer_sys::fuzz_target;
use oblivio_core::RuleSet;
use oblivio_helper::backend::NoDevices;
use oblivio_helper::licensing::{Licensing, Signer};
use oblivio_helper::serve::serve;
use oblivio_helper::session::{Session, SessionConfig};
use oblivio_protocol::response::{ErrorBody, ErrorCode, LicenseState, LicenseStatusResult};

#[derive(Debug)]
struct NoLicensing;

fn refused() -> ErrorBody {
    ErrorBody {
        code: ErrorCode::LicenseRequired,
        detail: None,
    }
}

impl Licensing for NoLicensing {
    fn status(&self) -> LicenseStatusResult {
        LicenseStatusResult {
            state: LicenseState::Missing,
            offer: None,
            license_id: None,
            valid_until: None,
        }
    }
    fn activation_request(&self) -> Result<String, ErrorBody> {
        Err(refused())
    }
    fn install(&self, _: &str) -> Result<LicenseStatusResult, ErrorBody> {
        Err(refused())
    }
    fn signer(&self) -> Result<Signer, ErrorBody> {
        Err(refused())
    }
}

fuzz_target!(|data: &[u8]| {
    let mut session = Session::new(
        NoDevices,
        SessionConfig {
            rules: RuleSet::baseline(),
            binary_sha256: [0; 32],
            reports_dir: std::path::PathBuf::from("/nonexistent/oblivio-fuzz"),
            jobs_dir: None,
            licensing: Box::new(NoLicensing),
        },
    );
    let _ = serve(data, std::io::sink(), &mut session);
});
