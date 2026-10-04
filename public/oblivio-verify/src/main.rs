//! `oblivio-verify`: offline verification of Oblivio erasure certificates.
//!
//! ```text
//! oblivio-verify [--lang en|fr|de] [--journal FILE] [--revocations FILE] [--root-key HEX]
//!                (FILE | --qr TEXT)
//! ```
//!
//! FILE holds a certificate PDF, the QR text (`OBV1:…`) or the binary COSE
//! attestation. From a PDF, the attached attestation is verified, and the
//! attached journal unless `--journal` is given.
//! Exit status: 0 authentic, 1 not authentic, 2 usage or input error.

mod labels;
mod render;

use std::process::ExitCode;

use oblivio_attestation::ed25519_dalek::VerifyingKey;
use oblivio_attestation::revocation::{RevocationList, merge};
use oblivio_attestation::{VerifyError, batch, from_qr_text, pdf, verify};

use crate::labels::Lang;

/// Largest certificate file read (PDF).
const MAX_INPUT_BYTES: u64 = 32 << 20;
/// Largest journal read.
const MAX_JOURNAL_BYTES: u64 = 16 << 20;

const USAGE: &str = "\
usage: oblivio-verify [--lang en|fr|de] [--journal FILE] [--revocations FILE] [--root-key HEX]
                      (FILE | --qr TEXT)

  FILE            certificate PDF, QR text (OBV1:...) or binary COSE attestation
  --qr TEXT       QR text given directly
  --journal FILE  detailed journal to check (default for a PDF: the attached one)
  --revocations FILE  newer revocation list (OBR1:...), added to the one built in
  --root-key HEX  root public key (64 hex digits) instead of the official one
  --lang LANG     report language (default: system language, else English)

exit status: 0 authentic, 1 not authentic, 2 usage or input error";

/// Parsed command line.
#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
    lang: Option<Lang>,
    journal: Option<String>,
    revocations: Option<String>,
    root_key: Option<String>,
    qr: Option<String>,
    file: Option<String>,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args::default();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--lang" => {
                let tag = value("--lang")?;
                parsed.lang =
                    Some(Lang::from_tag(&tag).ok_or(format!("unknown language {tag:?}"))?);
            }
            "--journal" => parsed.journal = Some(value("--journal")?),
            "--revocations" => parsed.revocations = Some(value("--revocations")?),
            "--root-key" => parsed.root_key = Some(value("--root-key")?),
            "--qr" => parsed.qr = Some(value("--qr")?),
            "-h" | "--help" => return Err(String::new()),
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}")),
            file if parsed.file.is_none() => parsed.file = Some(file.to_owned()),
            _ => return Err("only one FILE can be verified at a time".to_owned()),
        }
    }
    if parsed.file.is_some() == parsed.qr.is_some() {
        return Err("give either FILE or --qr".to_owned());
    }
    Ok(parsed)
}

fn parse_key(hex: &str) -> Result<VerifyingKey, String> {
    let hex = hex.trim();
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("--root-key needs 64 hexadecimal digits".to_owned());
    }
    let mut key = [0u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * index..2 * index + 2], 16)
            .map_err(|_| "--root-key needs 64 hexadecimal digits".to_owned())?;
    }
    VerifyingKey::from_bytes(&key).map_err(|_| "--root-key is not a valid Ed25519 key".to_owned())
}

fn read_bounded(path: &str, limit: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|error| format!("{path}: {error}"))?;
    let mut data = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut data)
        .map_err(|error| format!("{path}: {error}"))?;
    if data.len() as u64 > limit {
        return Err(format!("{path}: larger than {limit} bytes"));
    }
    Ok(data)
}

/// The signed attestation in `data`: QR text or binary COSE.
fn attestation_bytes(data: Vec<u8>) -> Result<Vec<u8>, VerifyError> {
    match std::str::from_utf8(&data) {
        Ok(text) if text.trim_start().starts_with("OBV1:") => from_qr_text(text.trim()),
        _ => Ok(data),
    }
}

fn run(args: Args) -> Result<bool, String> {
    let lang = args.lang.unwrap_or_else(Lang::from_env);
    // Until the official root key exists, a root key must be given with
    // `--root-key`, and the report says so.
    let (root, unofficial) = match (
        &args.root_key,
        oblivio_attestation::trust::official_root_key(),
    ) {
        (Some(hex), _) => (parse_key(hex)?, true),
        (None, Some(official)) => (official, false),
        (None, None) => {
            return Err("no official root key in this build: give --root-key".to_owned());
        }
    };
    let input = match (&args.qr, &args.file) {
        (Some(qr), _) => qr.clone().into_bytes(),
        (None, Some(file)) => read_bounded(file, MAX_INPUT_BYTES)?,
        (None, None) => return Err("give either FILE or --qr".to_owned()),
    };
    let given = args
        .journal
        .as_deref()
        .map(|path| read_bounded(path, MAX_JOURNAL_BYTES))
        .transpose()?;

    // Revocations: the list built into this version (official root only),
    // plus a newer one given by the user, signed by the same root.
    let mut lists = Vec::new();
    if !unofficial && let Some(text) = oblivio_attestation::trust::OFFICIAL_REVOCATIONS {
        lists.push(
            RevocationList::decode(text, &root)
                .map_err(|error| format!("built-in revocation list: {error}"))?,
        );
    }
    if let Some(path) = &args.revocations {
        let text = String::from_utf8(read_bounded(path, MAX_JOURNAL_BYTES)?)
            .map_err(|_| format!("{path}: not a revocation list"))?;
        lists.push(
            RevocationList::decode(&text, &root).map_err(|error| format!("{path}: {error}"))?,
        );
    }
    let revocations = merge(&lists);
    let newest = lists.iter().map(|list| (list.seq, list.issued_at)).max();

    // A PDF only carries the proof: its attachments are what is verified.
    if input.starts_with(b"%PDF-")
        && let Ok(files) = pdf::all_attachments(&input)
        && batch::is_batch(&files)
    {
        let verification = batch::verify_batch(&files, &root, &revocations);
        print!("{}", render::batch(&verification, newest, unofficial, lang));
        return Ok(verification.authentic());
    }
    let (input, attached) = if input.starts_with(b"%PDF-") {
        match pdf::attachments(&input) {
            Ok(found) => (found.attestation, found.journal),
            Err(error) => {
                print!("{}", render::rejected_pdf(&error, lang));
                return Ok(false);
            }
        }
    } else {
        (input, None)
    };
    let journal = match (given, attached) {
        (Some(given), _) => Some((given, false)),
        (None, Some(attached)) => Some((attached, true)),
        (None, None) => None,
    };

    let result = attestation_bytes(input).and_then(|signed| verify(&signed, &root, &revocations));
    match result {
        Ok(verified) => {
            let check = journal.map(|(journal, from_pdf)| render::JournalCheck {
                matches: verified.matches_journal(&journal),
                from_pdf,
            });
            print!(
                "{}",
                render::authentic(&verified, check, newest, unofficial, lang)
            );
            Ok(check.is_none_or(|check| check.matches))
        }
        Err(error) => {
            print!("{}", render::rejected(&error, lang));
            Ok(false)
        }
    }
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            if !message.is_empty() {
                eprintln!("oblivio-verify: {message}\n");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(message) => {
            eprintln!("oblivio-verify: {message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        parse_args(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn parses_a_full_command_line() {
        let args = parse(&[
            "--lang",
            "de",
            "--journal",
            "j.json",
            "--root-key",
            "ab",
            "cert.txt",
        ])
        .expect("valid");
        assert_eq!(args.lang, Some(Lang::De));
        assert_eq!(args.journal.as_deref(), Some("j.json"));
        assert_eq!(args.file.as_deref(), Some("cert.txt"));
    }

    #[test]
    fn rejects_bad_command_lines() {
        for args in [
            &[][..],
            &["a", "b"],
            &["a", "--qr", "OBV1:"],
            &["--lang", "it", "a"],
            &["--sign", "a"],
            &["--journal"],
        ] {
            assert!(parse(args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn parses_root_keys() {
        let public = oblivio_attestation::ed25519_dalek::SigningKey::from_bytes(&[1; 32])
            .verifying_key()
            .to_bytes();
        let key: String = public.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(parse_key(&key).map(|key| key.to_bytes()), Ok(public));
        assert!(parse_key(&key[..62]).is_err());
        assert!(parse_key(&format!("g{}", &key[1..])).is_err());
    }
}
