//! Issues a development license for an activation request, signed with the
//! public development root key:
//! `cargo run -p disphas-license --example issue_dev_license -- 'DSA1:…' [days] [offer]`.
//! The offer defaults to `Dev`; `Pro`, for instance, checks its limits.
//!
//! Only development builds of disphas accept the result.

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(request) = args.next() else {
        eprintln!("usage: issue_dev_license 'DSA1:…' [days] [offer]");
        return ExitCode::from(2);
    };
    let days = args.next().and_then(|days| days.parse().ok()).unwrap_or(30);
    let offer = args.next().unwrap_or_else(|| "Dev".to_owned());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    match disphas_license::dev_root::issue_for_offer(&request, now, days, &offer) {
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
