//! End-to-end tests of the `oblivio-verify` binary on fictional certificates.

// Test-only file: helpers may panic on setup failures.
#![allow(clippy::expect_used)]

use std::path::Path;
use std::process::{Command, Output};

use oblivio_attestation::ed25519_dalek::SigningKey;
use oblivio_attestation::model::{
    Category, Code, Device, Disposition, DispositionKind, DriveType, Sanitization, Status, Tool,
    Validation, ValidationMethod,
};
use oblivio_attestation::{Attestation, License, cose, sign_attestation, to_qr_text};
use sha2::{Digest, Sha256};

const ISSUED_AT: u64 = 1_791_158_400; // 2026-10-05T00:00:00Z
const JOURNAL: &[u8] = br#"{"timeline":[]}"#;

fn root() -> SigningKey {
    SigningKey::from_bytes(&[1; 32])
}

fn root_hex(key: &SigningKey) -> String {
    key.verifying_key()
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn signed_attestation() -> Vec<u8> {
    signed_attestation_of(DriveType::Nvme)
}

fn signed_attestation_of(drive_type: DriveType) -> Vec<u8> {
    let workstation = SigningKey::from_bytes(&[2; 32]);
    let license = License {
        id: "LIC-0001".to_owned(),
        customer: "Atelier XYZ".to_owned(),
        offer: "Pro".to_owned(),
        not_before: ISSUED_AT - 86_400,
        expires: None,
        workstation_key: workstation.verifying_key().to_bytes(),
        issuer: None,
    };
    let code = |text: &str| Code::new(text).expect("valid code");
    let attestation = Attestation {
        id: [
            0x01, 0x92, 0x8c, 0x3e, 0, 0, 0x70, 0, 0x80, 0, 0, 0, 0, 0, 0, 1,
        ],
        issued_at: ISSUED_AT,
        status: Status::Compliant,
        device: Device {
            model: "Fictional NVMe 1TB".to_owned(),
            serial: "SN-TEST-4567".to_owned(),
            firmware: "1.0".to_owned(),
            capacity_bytes: 1_000_204_886_016,
            drive_type,
            wwn: None,
            eui64: Some("0011223344556677".to_owned()),
            nguid: None,
        },
        sanitization: Sanitization {
            category: Category::Purge,
            method: Some(code("NvmeSanitizeBlockErase")),
            zero_finish: false,
            all_namespaces: false,
            references: vec!["NIST SP 800-88r2".to_owned(), "IEEE 2883-2022".to_owned()],
        },
        verified: Some(true),
        validation: Some(Validation {
            method: ValidationMethod::Canaries,
            samples: 1000,
            seed: [7; 32],
            passed: true,
        }),
        disposition: Some(Disposition {
            kind: DispositionKind::DestroyRecommended,
            codes: vec![code("D4")],
        }),
        reserves: vec![code("R4"), code("Z99")],
        refusals: vec![],
        justification: None,
        organization: Some("Atelier XYZ".to_owned()),
        operator: Some("J. Martin".to_owned()),
        batch: None,
        tool: Tool {
            version: "0.1.0".to_owned(),
            rules_version: "1.0.0".to_owned(),
            binary_sha256: [9; 32],
        },
        journal_sha256: Sha256::digest(JOURNAL).into(),
        license: cose::sign(license.to_cbor(), &root()).expect("signed license"),
    };
    sign_attestation(&attestation, &workstation).expect("signed attestation")
}

fn verify_cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oblivio-verify"))
        .args(args)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "C")
        .output()
        .expect("binary runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn write(dir: &Path, name: &str, data: &[u8]) -> String {
    let path = dir.join(name);
    std::fs::write(&path, data).expect("test file");
    path.to_string_lossy().into_owned()
}

#[test]
fn authentic_qr_text_in_french() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let qr = write(
        dir.path(),
        "cert.txt",
        to_qr_text(&signed_attestation()).as_bytes(),
    );
    let journal = write(dir.path(), "journal.json", JOURNAL);
    let output = verify_cli(&[
        "--lang",
        "fr",
        "--root-key",
        &root_hex(&root()),
        "--journal",
        &journal,
        &qr,
    ]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text}");
    assert!(text.contains("AUTHENTIQUE"));
    assert!(text.contains("clé racine fournie en paramètre"));
    assert!(text.contains("SN-TEST-4567"));
    assert!(text.contains("Purge"));
    assert!(text.contains("2026-10-05 00:00:00 UTC"));
    assert!(text.contains("Destruction physique recommandée"));
    assert!(text.contains("Nombre élevé de secteurs réalloués (D4)"));
    assert!(text.contains("Z99 (pas de libellé dans cette version)"));
    assert!(text.contains("correspond à l'attestation"));
}

#[test]
fn authentic_binary_cose_in_german() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let cose = write(dir.path(), "cert.cose", &signed_attestation());
    let output = verify_cli(&["--lang", "de", "--root-key", &root_hex(&root()), &cose]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("ECHT"));
    assert!(stdout(&output).contains("KONFORM"));
}

#[test]
fn a_drive_type_from_a_newer_oblivio_is_authentic_and_shown_as_is() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let newer = DriveType::parse("NVMe-ZNS").expect("well-formed type");
    let cose = write(dir.path(), "cert.cose", &signed_attestation_of(newer));
    let output = verify_cli(&["--lang", "en", "--root-key", &root_hex(&root()), &cose]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("AUTHENTIC"));
    assert!(stdout(&output).contains("NVMe-ZNS"));
}

#[test]
fn qr_given_on_the_command_line_defaults_to_english() {
    let qr = to_qr_text(&signed_attestation());
    let output = verify_cli(&["--root-key", &root_hex(&root()), "--qr", &qr]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("AUTHENTIC"));
}

#[test]
fn wrong_root_key_is_not_authentic() {
    let other = SigningKey::from_bytes(&[3; 32]);
    let qr = to_qr_text(&signed_attestation());
    let output = verify_cli(&["--lang", "en", "--root-key", &root_hex(&other), "--qr", &qr]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("NOT AUTHENTIC"));
    assert!(stdout(&output).contains("not signed by Oblivio"));
}

#[test]
fn altered_qr_is_not_authentic() {
    let qr = to_qr_text(&signed_attestation());
    let middle = qr.len() / 2;
    let replacement = if &qr[middle..=middle] == "A" {
        "B"
    } else {
        "A"
    };
    let altered = format!("{}{replacement}{}", &qr[..middle], &qr[middle + 1..]);
    let output = verify_cli(&["--root-key", &root_hex(&root()), "--qr", &altered]);
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn mismatched_journal_fails() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let journal = write(dir.path(), "journal.json", b"{}");
    let qr = to_qr_text(&signed_attestation());
    let output = verify_cli(&[
        "--lang",
        "en",
        "--root-key",
        &root_hex(&root()),
        "--journal",
        &journal,
        "--qr",
        &qr,
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("DOES NOT match"));
}

#[test]
fn usage_errors_exit_with_2() {
    assert_eq!(verify_cli(&[]).status.code(), Some(2));
    assert_eq!(
        verify_cli(&["--root-key", "zz", "--qr", "OBV1:"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn official_root_is_used_by_default_and_rejects_test_certificates() {
    // Without --root-key, the official Oblivio root applies: a certificate
    // chaining to a test root is not authentic, and no "unofficial" warning.
    let qr = to_qr_text(&signed_attestation());
    let output = verify_cli(&["--lang", "en", "--qr", &qr]);
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output);
    assert!(text.contains("NOT AUTHENTIC"));
    assert!(text.contains("not signed by Oblivio"));
    assert!(!text.contains("root key given on the command line"));
    assert_eq!(verify_cli(&["--qr", "OBV1:"]).status.code(), Some(1));
}

/// Certificate PDF rendered by Oblivio from fictional data and test keys
/// (root key `[1; 32]`, the one of these tests). See `fixtures/README.md`.
const PDF: &[u8] = include_bytes!("fixtures/certificate.pdf");

#[test]
fn authentic_pdf_with_its_attached_journal() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let pdf = write(dir.path(), "certificate.pdf", PDF);
    let output = verify_cli(&["--lang", "en", "--root-key", &root_hex(&root()), &pdf]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text}");
    assert!(text.contains("AUTHENTIC"));
    assert!(text.contains("FICT0000A001"));
    assert!(text.contains("matches the attestation (attached to the PDF)"));
}

#[test]
fn pdf_with_a_foreign_journal_is_not_accepted() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let pdf = write(dir.path(), "certificate.pdf", PDF);
    let journal = write(dir.path(), "journal.json", b"{}");
    let output = verify_cli(&[
        "--lang",
        "en",
        "--root-key",
        &root_hex(&root()),
        "--journal",
        &journal,
        &pdf,
    ]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{text}");
    assert!(text.contains("DOES NOT match"));
}

#[test]
fn pdf_without_attestation_or_truncated_is_rejected() {
    let dir = tempfile::tempdir().expect("temporary directory");
    for (name, data) in [
        ("plain.pdf", &b"%PDF-1.7\n1 0 obj << >> endobj\n%%EOF\n"[..]),
        ("truncated.pdf", &PDF[..PDF.len() / 2]),
    ] {
        let path = write(dir.path(), name, data);
        let output = verify_cli(&["--lang", "en", "--root-key", &root_hex(&root()), &path]);
        let text = stdout(&output);
        assert_eq!(output.status.code(), Some(1), "{name}: {text}");
        assert!(text.contains("NOT AUTHENTIC"), "{name}: {text}");
    }
}

fn revocation_list(key: &SigningKey, revoked_at: u64) -> String {
    use oblivio_attestation::Revocation;
    use oblivio_attestation::revocation::RevocationList;
    RevocationList {
        seq: 7,
        issued_at: ISSUED_AT + 86_400,
        entries: vec![Revocation {
            license_id: "LIC-0001".to_owned(),
            revoked_at,
        }],
    }
    .sign(key)
    .expect("signed list")
}

#[test]
fn revocation_applies_from_its_date_only() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let cose = write(dir.path(), "cert.cose", &signed_attestation());

    // Revoked before the certificate was issued: not authentic.
    let before = write(
        dir.path(),
        "before.obr",
        revocation_list(&root(), ISSUED_AT - 1).as_bytes(),
    );
    let output = verify_cli(&[
        "--lang",
        "en",
        "--root-key",
        &root_hex(&root()),
        "--revocations",
        &before,
        &cose,
    ]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{text}");
    assert!(text.contains("revoked"), "{text}");

    // Revoked after: the certificate stays valid, and the list is named.
    let after = write(
        dir.path(),
        "after.obr",
        revocation_list(&root(), ISSUED_AT + 1).as_bytes(),
    );
    let output = verify_cli(&[
        "--lang",
        "en",
        "--root-key",
        &root_hex(&root()),
        "--revocations",
        &after,
        &cose,
    ]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text}");
    assert!(text.contains("#7"), "{text}");
}

#[test]
fn revocation_list_from_another_key_is_refused() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let cose = write(dir.path(), "cert.cose", &signed_attestation());
    let forged = write(
        dir.path(),
        "forged.obr",
        revocation_list(&SigningKey::from_bytes(&[9; 32]), 0).as_bytes(),
    );
    let output = verify_cli(&[
        "--root-key",
        &root_hex(&root()),
        "--revocations",
        &forged,
        &cose,
    ]);
    assert_eq!(output.status.code(), Some(2));
}

/// Batch certificate of six fictional drives, same test keys.
const BATCH_PDF: &[u8] = include_bytes!("fixtures/batch.pdf");

#[test]
fn batch_pdf_is_verified_drive_by_drive() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let pdf = write(dir.path(), "batch.pdf", BATCH_PDF);
    let output = verify_cli(&["--lang", "en", "--root-key", &root_hex(&root()), &pdf]);
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text}");
    assert!(text.contains("AUTHENTIC BATCH"), "{text}");
    assert!(text.contains("LOT-2026-001"));
    assert_eq!(text.matches("  ✔ ").count(), 6, "{text}");

    // Another root: every drive is rejected, so is the batch.
    let stranger = SigningKey::from_bytes(&[9; 32]);
    let output = verify_cli(&["--lang", "en", "--root-key", &root_hex(&stranger), &pdf]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("BATCH NOT AUTHENTIC"));
}
