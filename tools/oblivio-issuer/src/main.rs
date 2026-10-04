//! `oblivio-issuer`: the publisher's tool. Generates the Oblivio root key and
//! signs licenses. It is never shipped with the product; run it on an
//! offline machine and keep `root.key` and its passphrase apart.
//!
//! ```text
//! oblivio-issuer keygen ROOT.key
//! oblivio-issuer pubkey ROOT.key
//! oblivio-issuer issue ROOT.key --customer NAME --offer OFFER (--days N | --perpetual)
//!                      [--id ID] [--register FILE] 'OBA1:…'
//! oblivio-issuer revoke ROOT.key --id ID [--at EPOCH] [--list CURRENT.obr]
//!                       [--register FILE] --out NEW.obr
//! ```

mod issue;
mod revoke;
mod root_key;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use oblivio_attestation::display::civil;
use oblivio_attestation::ed25519_dalek::VerifyingKey;
use zeroize::Zeroizing;

use crate::issue::Terms;
use crate::root_key::Cost;

const USAGE: &str = "\
usage:
  oblivio-issuer keygen ROOT.key
  oblivio-issuer pubkey ROOT.key
  oblivio-issuer issue ROOT.key --customer NAME --offer OFFER (--days N | --perpetual)
                       [--id ID] [--register FILE] 'OBA1:...'
  oblivio-issuer revoke ROOT.key --id ID [--at EPOCH] [--list CURRENT.obr]
                        [--register FILE] --out NEW.obr";

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn passphrase(prompt: &str) -> Result<Zeroizing<String>, String> {
    rpassword::prompt_password(prompt)
        .map(Zeroizing::new)
        .map_err(|error| format!("cannot read the passphrase: {error}"))
}

fn print_public(public: &VerifyingKey) {
    let bytes = public.as_bytes();
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    println!("root public key: {hex}");
    println!();
    println!("for public/oblivio-attestation/src/trust.rs:");
    let array: Vec<String> = bytes.iter().map(|byte| format!("0x{byte:02x}")).collect();
    println!("pub const OFFICIAL_ROOT_KEY: Option<[u8; 32]> = Some([");
    for chunk in array.chunks(8) {
        println!("    {},", chunk.join(", "));
    }
    println!("]);");
}

fn keygen(path: &Path) -> Result<(), String> {
    eprintln!(
        "Choose a passphrase of at least {} characters.",
        root_key::MIN_PASSPHRASE_CHARS
    );
    eprintln!("Without it the root key is lost: no license can ever be issued again.");
    let first = passphrase("passphrase: ")?;
    let second = passphrase("again: ")?;
    if *first != *second {
        return Err("the passphrases differ".to_owned());
    }
    eprintln!("deriving the encryption key (Argon2id, 256 MiB)…");
    let public = root_key::create(path, &first, Cost::ROOT).map_err(|error| error.to_string())?;
    eprintln!("root key written to {} (mode 0600)", path.display());
    print_public(&public);
    Ok(())
}

fn license_id() -> Result<String, String> {
    let (year, month, day, ..) = civil(now());
    let mut suffix = [0u8; 2];
    getrandom::fill(&mut suffix).map_err(|error| error.to_string())?;
    Ok(format!(
        "OBL-{year:04}{month:02}{day:02}-{:02X}{:02X}",
        suffix[0], suffix[1]
    ))
}

fn issue_command(path: &Path, mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut customer = None;
    let mut offer = None;
    let mut days: Option<u64> = None;
    let mut perpetual = false;
    let mut id = None;
    let mut register = None;
    let mut request = None;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--customer" => customer = Some(value("--customer")?),
            "--offer" => offer = Some(value("--offer")?),
            "--days" => {
                days = Some(
                    value("--days")?
                        .parse()
                        .map_err(|_| "--days needs a number")?,
                );
            }
            "--perpetual" => perpetual = true,
            "--id" => id = Some(value("--id")?),
            "--register" => register = Some(PathBuf::from(value("--register")?)),
            text if text.starts_with("OBA1:") && request.is_none() => {
                request = Some(text.to_owned())
            }
            other => return Err(format!("unexpected argument {other:?}")),
        }
    }
    let customer = customer.ok_or("--customer is required")?;
    let offer = offer.ok_or("--offer is required")?;
    let request = request.ok_or("the activation request (OBA1:…) is required")?;
    let start = now().saturating_sub(60);
    let expires = match (days, perpetual) {
        (Some(days), false) => Some(start + days.saturating_mul(86_400)),
        (None, true) => None,
        _ => return Err("give either --days N or --perpetual".to_owned()),
    };
    let terms = Terms {
        id: match id {
            Some(id) => id,
            None => license_id()?,
        },
        customer,
        offer,
        not_before: start,
        expires,
    };
    let register = register.unwrap_or_else(|| {
        path.parent()
            .unwrap_or(Path::new("."))
            .join("issued-licenses.csv")
    });

    let secret = passphrase("root key passphrase: ")?;
    let root = root_key::unlock(path, &secret).map_err(|error| error.to_string())?;
    let (license, text) = issue::issue(&root, &request, &terms)?;
    issue::record(&register, &license, now())
        .map_err(|error| format!("{}: {error}", register.display()))?;
    eprintln!(
        "license {} for {} ({}), recorded in {}",
        license.id,
        license.customer,
        license.offer,
        register.display()
    );
    println!("{text}");
    Ok(())
}

/// Revokes a license: writes the next revocation list, to ship with the next
/// version (`OFFICIAL_REVOCATIONS`) and to give to `oblivio-verify --revocations`.
fn revoke_command(path: &Path, mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut id = None;
    let mut at = None;
    let mut list = None;
    let mut register = None;
    let mut out = None;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--id" => id = Some(value("--id")?),
            "--at" => {
                at = Some(
                    value("--at")?
                        .parse::<u64>()
                        .map_err(|_| "--at needs a UTC epoch time in seconds")?,
                );
            }
            "--list" => list = Some(PathBuf::from(value("--list")?)),
            "--register" => register = Some(PathBuf::from(value("--register")?)),
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            other => return Err(format!("unexpected argument {other:?}")),
        }
    }
    let id = id.ok_or("--id is required")?;
    let out = out.ok_or("--out is required")?;
    let now = now();
    // Back-dating (compromise date) rejects every certificate signed since.
    let at = at.unwrap_or(now);
    if at > now {
        return Err("--at is in the future".to_owned());
    }
    let register = register.unwrap_or_else(|| {
        path.parent()
            .unwrap_or(Path::new("."))
            .join("issued-licenses.csv")
    });
    if !revoke::registered(&register, &id)
        .map_err(|error| format!("{}: {error}", register.display()))?
    {
        return Err(format!("{id} is not in {}", register.display()));
    }

    let public = root_key::public_key(path).map_err(|error| error.to_string())?;
    let current = match &list {
        Some(list) => {
            let text = std::fs::read_to_string(list)
                .map_err(|error| format!("{}: {error}", list.display()))?;
            Some(
                oblivio_attestation::revocation::RevocationList::decode(&text, &public)
                    .map_err(|error| format!("{}: {error}", list.display()))?,
            )
        }
        None => None,
    };
    let next = revoke::next_list(current.as_ref(), &id, at, now)?;

    let secret = passphrase("root key passphrase: ")?;
    let root = root_key::unlock(path, &secret).map_err(|error| error.to_string())?;
    let text = next.sign(&root).map_err(|error| error.to_string())?;
    revoke::write_new(&out, &text).map_err(|error| format!("{}: {error}", out.display()))?;
    eprintln!(
        "revocation list #{} with {} license(s) written to {}",
        next.seq,
        next.entries.len(),
        out.display()
    );
    eprintln!("ship it: copy it to public/oblivio-attestation/revocations.obr and set");
    eprintln!("OFFICIAL_REVOCATIONS = Some(include_str!(\"../revocations.obr\")) in trust.rs");
    Ok(())
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or("")?;
    let path = PathBuf::from(args.next().ok_or("the root key file is required")?);
    match command.as_str() {
        "keygen" => keygen(&path),
        "pubkey" => {
            print_public(&root_key::public_key(&path).map_err(|error| error.to_string())?);
            eprintln!("(read from the file header; `issue` authenticates it with the passphrase)");
            Ok(())
        }
        "issue" => issue_command(&path, args),
        "revoke" => revoke_command(&path, args),
        other => Err(format!("unknown command {other:?}")),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            if !message.is_empty() {
                eprintln!("oblivio-issuer: {message}\n");
            }
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
