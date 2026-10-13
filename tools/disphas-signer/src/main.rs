//! `disphas-signer`: the license signing service of the license server
//! (`docs/licences_en_ligne.md` §6). Holds the shop keys and their issuer
//! certificates, signs licenses for the site over a local Unix socket, and
//! records every signature. Never shipped to customers; no network access.
//!
//! ```text
//! disphas-signer [--state DIR] keygen
//! disphas-signer [--state DIR] install 'DSI1:…'
//! disphas-signer [--state DIR] status
//! disphas-signer [--state DIR] serve [--socket PATH]
//! ```

mod register;
mod serve;
mod signer;
mod store;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::register::Register;
use crate::signer::{Reply, Request, Signer};
use crate::store::Store;

const USAGE: &str = "\
usage:
  disphas-signer [--state DIR] keygen
  disphas-signer [--state DIR] install 'DSI1:...'
  disphas-signer [--state DIR] status
  disphas-signer [--state DIR] serve [--socket PATH]

defaults: --state /var/lib/disphas-signer, --socket /run/disphas-signer/signer.sock";

/// A current certificate expiring sooner than this is reported by `status`:
/// the next one must be installed 90 days before (rotation overlap).
const ROTATION_WARNING: u64 = 120 * 86_400;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Opens the store, its certificates and its register.
fn signer(store: &Store) -> Result<Signer, String> {
    let roots = disphas_license::trusted_roots();
    if roots.is_empty() {
        return Err("this build trusts no root key".to_owned());
    }
    store.prepare().map_err(|error| error.to_string())?;
    let certified = store.certified(&roots)?;
    let register = Register::open(&store.register_path()).map_err(|error| error.to_string())?;
    Ok(Signer::new(certified, roots, register))
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1).peekable();
    let mut state = PathBuf::from("/var/lib/disphas-signer");
    if args.peek().map(String::as_str) == Some("--state") {
        args.next();
        state = PathBuf::from(args.next().ok_or("--state needs a value")?);
    }
    let store = Store::new(state);
    let command = args.next().ok_or("")?;
    match command.as_str() {
        "keygen" => {
            let public = store.keygen()?;
            let hex: String = public
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            eprintln!("new shop key; certify it on the offline machine with");
            eprintln!("  disphas-issuer certify ROOT.key --id ISS-… --shop-key {hex} …");
            println!("{hex}");
            Ok(())
        }
        "install" => {
            let text = args
                .next()
                .ok_or("the issuer certificate (DSI1:…) is required")?;
            let issuer = store.install(&text, &disphas_license::trusted_roots(), now())?;
            eprintln!(
                "{} installed: {} until {}; restart the service to use it",
                issuer.id,
                issuer.offers.join(", "),
                issuer.expires
            );
            Ok(())
        }
        "status" => {
            let now = now();
            let reply = signer(&store)?.handle(Request::Status, now);
            println!(
                "{}",
                serde_json::to_string_pretty(&reply).map_err(|error| error.to_string())?
            );
            if let Reply::Status { issuers, .. } = &reply {
                let latest = issuers.iter().map(|issuer| issuer.expires).max();
                match latest {
                    None => eprintln!("warning: no issuer certificate installed"),
                    Some(end) if end <= now + ROTATION_WARNING => eprintln!(
                        "warning: the last certificate ends within 120 days: \
                         generate, certify and install the next one"
                    ),
                    Some(_) => {}
                }
            }
            Ok(())
        }
        "serve" => {
            let socket = match args.next().as_deref() {
                Some("--socket") => PathBuf::from(args.next().ok_or("--socket needs a value")?),
                Some(other) => return Err(format!("unexpected argument {other:?}")),
                None => PathBuf::from("/run/disphas-signer/signer.sock"),
            };
            serve::serve(signer(&store)?, &socket, now)
        }
        other => Err(format!("unknown command {other:?}")),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            if !message.is_empty() {
                eprintln!("disphas-signer: {message}\n");
            }
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
