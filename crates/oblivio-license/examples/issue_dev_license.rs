//! Issues a development license for an activation request, signed with the
//! public development root key:
//! `cargo run -p oblivio-license --example issue_dev_license -- 'OBA1:…' [days]`.
//!
//! Only development builds of Oblivio accept the result.

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(request) = args.next() else {
        eprintln!("usage: issue_dev_license 'OBA1:…' [days]");
        return ExitCode::from(2);
    };
    let days = args.next().and_then(|days| days.parse().ok()).unwrap_or(30);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    match oblivio_license::dev_root::issue(&request, now, days) {
        Ok(license) => {
            println!("{license}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("issue_dev_license: {error}");
            ExitCode::FAILURE
        }
    }
}
