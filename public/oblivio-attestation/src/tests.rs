//! End-to-end tests of the trust chain, on fictional keys and drives.

use ciborium::Value;
use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};

use crate::cbor::CborError;
use crate::cose::{self, CoseError};
use crate::model::{
    Category, Code, Device, Disposition, DispositionKind, DriveType, HOST_WRITE_METHOD,
    Sanitization, Status, Tool, Validation, ValidationMethod,
};
use crate::verify::MAX_ATTESTATION_BYTES;
use crate::*;

const ISSUED_AT: u64 = 1_791_158_400; // 2026-10-05T00:00:00Z
const JOURNAL: &[u8] = br#"{"timeline":[]}"#;

fn root() -> SigningKey {
    SigningKey::from_bytes(&[1; 32])
}

fn workstation() -> SigningKey {
    SigningKey::from_bytes(&[2; 32])
}

fn license(expires: Option<u64>) -> License {
    License {
        id: "LIC-0001".to_owned(),
        customer: "Atelier XYZ".to_owned(),
        offer: "Pro".to_owned(),
        not_before: ISSUED_AT - 86_400,
        expires,
        workstation_key: workstation().verifying_key().to_bytes(),
    }
}

fn signed_license(license: &License, root: &SigningKey) -> Vec<u8> {
    cose::sign(license.to_cbor(), root).expect("signed license")
}

fn code(text: &str) -> Code {
    Code::new(text).expect("valid code")
}

fn attestation(license: Vec<u8>) -> Attestation {
    Attestation {
        id: [
            0x01, 0x92, 0x8c, 0x3e, 0, 0, 0x70, 0, 0x80, 0, 0, 0, 0, 0, 0, 1,
        ],
        issued_at: ISSUED_AT,
        status: Status::Compliant,
        device: Device {
            model: "Fictional NVMe SSD 1TB".to_owned(),
            serial: "FICT0000A001".to_owned(),
            firmware: "FICT01FW".to_owned(),
            capacity_bytes: 1_024_209_543_168,
            drive_type: DriveType::Nvme,
            wwn: None,
            eui64: Some("00a07501fe0c0001".to_owned()),
            nguid: None,
        },
        sanitization: Sanitization {
            category: Category::Purge,
            method: Some(code("NvmeSanitizeBlockErase")),
            zero_finish: false,
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
        reserves: vec![code("R4")],
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
        license,
    }
}

fn valid_signed() -> Vec<u8> {
    let attestation = attestation(signed_license(&license(None), &root()));
    sign_attestation(&attestation, &workstation()).expect("signed attestation")
}

fn root_key() -> ed25519_dalek::VerifyingKey {
    root().verifying_key()
}

// --- Content.

#[test]
fn attestation_round_trips_through_cbor() {
    let original = attestation(signed_license(&license(None), &root()));
    assert_eq!(Attestation::from_cbor(&original.to_cbor()), Ok(original));
}

#[test]
fn license_round_trips_through_cbor() {
    let original = license(Some(ISSUED_AT + 1));
    assert_eq!(License::from_cbor(&original.to_cbor()), Ok(original));
}

fn as_map(bytes: &[u8]) -> Vec<(Value, Value)> {
    let value: Value = ciborium::de::from_reader(bytes).expect("valid CBOR");
    value.into_map().expect("map")
}

fn raw_encode(entries: Vec<(Value, Value)>) -> Vec<u8> {
    let mut out = Vec::new();
    ciborium::ser::into_writer(&Value::Map(entries), &mut out).expect("serializable");
    out
}

#[test]
fn unknown_field_is_rejected() {
    let mut entries = as_map(&license(None).to_cbor());
    entries.push((Value::Text("x".to_owned()), Value::Bool(true)));
    let result = License::from_cbor(&raw_encode(entries));
    assert!(matches!(
        result,
        Err(CborError::Unknown | CborError::NotCanonical)
    ));
}

#[test]
fn null_instead_of_absent_field_is_rejected() {
    let mut entries = as_map(&license(None).to_cbor());
    entries.push((Value::Text("exp".to_owned()), Value::Null));
    assert!(License::from_cbor(&raw_encode(entries)).is_err());
}

#[test]
fn unsorted_keys_are_rejected() {
    let mut entries = as_map(&license(None).to_cbor());
    entries.reverse();
    assert_eq!(
        License::from_cbor(&raw_encode(entries)),
        Err(CborError::NotCanonical)
    );
}

#[test]
fn trailing_bytes_are_rejected() {
    let mut bytes = license(None).to_cbor();
    bytes.push(0x00);
    assert_eq!(License::from_cbor(&bytes), Err(CborError::NotCanonical));
}

#[test]
fn invalid_code_is_rejected() {
    assert!(Code::new("R4").is_some());
    assert!(Code::new("").is_none());
    assert!(Code::new("R4; DROP").is_none());
}

// --- Chain of trust.

#[test]
fn verifies_the_whole_chain_from_a_qr_code() {
    let qr = to_qr_text(&valid_signed());
    assert!(qr.starts_with("OBV1:"));
    assert!(qr.len() < 1000, "QR text is {} characters", qr.len());
    let verified = verify(&from_qr_text(&qr).expect("valid QR"), &root_key(), &[])
        .expect("authentic attestation");
    assert_eq!(verified.attestation.device.serial, "FICT0000A001");
    assert_eq!(verified.license.customer, "Atelier XYZ");
    assert!(verified.matches_journal(JOURNAL));
    assert!(!verified.matches_journal(b"{}"));
}

#[test]
fn another_root_key_is_rejected() {
    let other_root = SigningKey::from_bytes(&[3; 32]).verifying_key();
    assert_eq!(
        verify(&valid_signed(), &other_root, &[]),
        Err(VerifyError::License(CoseError::BadSignature))
    );
}

#[test]
fn attestation_signed_by_an_unlicensed_key_is_rejected() {
    let attestation = attestation(signed_license(&license(None), &root()));
    let intruder = SigningKey::from_bytes(&[4; 32]);
    let signed = sign_attestation(&attestation, &intruder).expect("signed");
    assert_eq!(
        verify(&signed, &root_key(), &[]),
        Err(VerifyError::AttestationSignature(CoseError::BadSignature))
    );
}

#[test]
fn self_issued_license_is_rejected() {
    // A workstation signing its own license instead of the root key.
    let attestation = attestation(signed_license(&license(None), &workstation()));
    let signed = sign_attestation(&attestation, &workstation()).expect("signed");
    assert_eq!(
        verify(&signed, &root_key(), &[]),
        Err(VerifyError::License(CoseError::BadSignature))
    );
}

#[test]
fn license_must_be_valid_at_the_attestation_date() {
    for license in [
        license(Some(ISSUED_AT - 1)),
        License {
            not_before: ISSUED_AT + 1,
            ..license(None)
        },
    ] {
        let attestation = attestation(signed_license(&license, &root()));
        let signed = sign_attestation(&attestation, &workstation()).expect("signed");
        assert_eq!(
            verify(&signed, &root_key(), &[]),
            Err(VerifyError::LicenseNotValid)
        );
    }
}

#[test]
fn revocation_only_affects_later_attestations() {
    let revoked = |revoked_at| Revocation {
        license_id: "LIC-0001".to_owned(),
        revoked_at,
    };
    assert_eq!(
        verify(&valid_signed(), &root_key(), &[revoked(ISSUED_AT - 1)]),
        Err(VerifyError::LicenseRevoked)
    );
    assert!(verify(&valid_signed(), &root_key(), &[revoked(ISSUED_AT + 1)]).is_ok());
}

#[test]
fn any_altered_byte_is_detected() {
    let signed = valid_signed();
    for index in (0..signed.len()).step_by(7) {
        let mut altered = signed.clone();
        altered[index] ^= 0x01;
        assert!(
            verify(&altered, &root_key(), &[]).is_err(),
            "byte {index} altered without detection"
        );
    }
}

// --- QR transport.

#[test]
fn foreign_qr_code_is_rejected() {
    assert_eq!(from_qr_text("HC1:6BFOXN"), Err(VerifyError::NotOblivio));
    assert_eq!(from_qr_text("OBV1:aa"), Err(VerifyError::Corrupted));
}

#[test]
fn decompression_is_bounded() {
    let bomb = vec![0u8; MAX_ATTESTATION_BYTES + 1];
    let qr = to_qr_text(&bomb);
    assert!(qr.len() < 1000, "a bomb is small once compressed");
    assert_eq!(from_qr_text(&qr), Err(VerifyError::Corrupted));
}

#[test]
fn batch_round_trips_and_is_bounded() {
    use crate::model::Batch;
    let mut with_batch = attestation(vec![1, 2, 3]);
    with_batch.batch = Some(Batch {
        id: "LOT-2026_001".to_owned(),
        label: Some("Commande 4512 — Atelier Œuvre".to_owned()),
    });
    assert_eq!(
        Attestation::from_cbor(&with_batch.to_cbor()),
        Ok(with_batch.clone())
    );

    for (id, label) in [
        ("", None),
        ("lot with spaces", None),
        (&"x".repeat(65)[..], None),
        ("LOT-1", Some("y".repeat(129))),
    ] {
        let mut invalid = with_batch.clone();
        invalid.batch = Some(Batch {
            id: id.to_owned(),
            label,
        });
        assert!(
            Attestation::from_cbor(&invalid.to_cbor()).is_err(),
            "{id:?} accepted"
        );
    }
}

mod batch_verification {
    use super::*;
    use crate::batch::{DriveError, is_batch, verify_batch};
    use crate::display::uuid;
    use crate::model::Batch;

    fn drive(number: u8, batch: Option<&str>) -> (String, Vec<u8>) {
        let mut attestation = attestation(signed_license(&license(None), &root()));
        attestation.id[15] = number;
        attestation.batch = batch.map(|id| Batch {
            id: id.to_owned(),
            label: None,
        });
        let signed = sign_attestation(&attestation, &workstation()).expect("signed");
        (uuid(&attestation.id), signed)
    }

    fn files(drives: &[(String, Vec<u8>)]) -> Vec<(String, Vec<u8>)> {
        drives
            .iter()
            .flat_map(|(report, signed)| {
                [
                    (format!("{report}.cose"), signed.clone()),
                    (format!("{report}.json"), JOURNAL.to_vec()),
                ]
            })
            .collect()
    }

    #[test]
    fn a_consistent_batch_is_authentic() {
        let all = files(&[drive(1, Some("LOT-A")), drive(2, Some("LOT-A"))]);
        assert!(is_batch(&all));
        let result = verify_batch(&all, &root().verifying_key(), &[]);
        assert!(result.authentic());
        assert_eq!(result.drives.len(), 2);
        assert_eq!(result.batch.map(|batch| batch.id).as_deref(), Some("LOT-A"));
    }

    #[test]
    fn any_bad_drive_fails_the_batch() {
        // Another batch, no batch, a renamed file, a replaced journal.
        let mixed = files(&[drive(1, Some("LOT-A")), drive(2, Some("LOT-B"))]);
        let result = verify_batch(&mixed, &root().verifying_key(), &[]);
        assert!(!result.authentic());
        assert!(matches!(
            result.drives[1].outcome,
            Err(DriveError::OtherBatch)
        ));

        let missing = files(&[drive(1, Some("LOT-A")), drive(2, None)]);
        assert!(!verify_batch(&missing, &root().verifying_key(), &[]).authentic());

        let (report, signed) = drive(1, Some("LOT-A"));
        let (other, _) = drive(9, Some("LOT-A"));
        let renamed = vec![
            (format!("{other}.cose"), signed.clone()),
            (format!("{other}.json"), JOURNAL.to_vec()),
        ];
        let result = verify_batch(&renamed, &root().verifying_key(), &[]);
        assert!(matches!(
            result.drives[0].outcome,
            Err(DriveError::NameMismatch)
        ));

        let tampered = vec![
            (format!("{report}.cose"), signed),
            (format!("{report}.json"), b"{}".to_vec()),
        ];
        let result = verify_batch(&tampered, &root().verifying_key(), &[]);
        assert_eq!(result.drives[0].journal_matches, Some(false));
        assert!(!result.authentic());

        assert!(!verify_batch(&[], &root().verifying_key(), &[]).authentic());
    }
}

/// A compliant host overwrite: Clear, validated by pattern.
fn host_write(license: Vec<u8>) -> Attestation {
    let mut host = attestation(license);
    host.sanitization.category = Category::Clear;
    host.sanitization.method = Some(code(HOST_WRITE_METHOD));
    if let Some(validation) = &mut host.validation {
        validation.method = ValidationMethod::Pattern;
    }
    host.reserves = vec![code("R7")];
    host
}

#[test]
fn zero_finish_is_present_only_when_true() {
    let mut finished = host_write(vec![1]);
    finished.sanitization.zero_finish = true;
    assert_eq!(
        Attestation::from_cbor(&finished.to_cbor()),
        Ok(finished.clone())
    );
    // An explicit `z: false` is not canonical: rejected.
    let mut entries = as_map(&finished.to_cbor());
    for (key, value) in &mut entries {
        if key.as_text() == Some("san")
            && let Value::Map(san) = value
        {
            for (field, flag) in san.iter_mut() {
                if field.as_text() == Some("z") {
                    *flag = Value::Bool(false);
                }
            }
        }
    }
    assert!(Attestation::from_cbor(&raw_encode(entries)).is_err());
}

#[test]
fn contradictory_attestations_are_rejected_even_when_signed() {
    type Change = fn(&mut Attestation);
    let refuse = |status| {
        move |a: &mut Attestation| {
            a.status = status;
            a.sanitization.category = Category::None;
            a.disposition = None;
            a.refusals = vec![code("N10")];
        }
    };
    // Each change alone contradicts the certificate rules.
    let cases: [(&str, Change); 14] = [
        ("host write certified Purge", |a| {
            a.sanitization.category = Category::Purge;
        }),
        ("pattern validation certified Purge", |a| {
            a.sanitization.method = Some(code("Anything"));
            a.sanitization.category = Category::Purge;
        }),
        ("zero pass after a firmware erasure", |a| {
            a.sanitization.method = Some(code("NvmeSanitizeBlockErase"));
            if let Some(v) = &mut a.validation {
                v.method = ValidationMethod::Canaries;
            }
            a.sanitization.zero_finish = true;
        }),
        ("zero pass not validated", |a| {
            a.sanitization.zero_finish = true;
            a.status = Status::NonCompliant;
            a.justification = Some("x".to_owned());
            a.refusals = vec![code("N1")];
            a.disposition = None;
            if let Some(v) = &mut a.validation {
                v.passed = false;
            }
        }),
        ("CONFORME not verified", |a| a.verified = Some(false)),
        ("CONFORME not validated", |a| {
            if let Some(v) = &mut a.validation {
                v.passed = false;
            }
        }),
        ("CONFORME without validation", |a| a.validation = None),
        ("CONFORME without method", |a| a.sanitization.method = None),
        ("CONFORME without category", |a| {
            a.sanitization.category = Category::None;
        }),
        ("CONFORME without disposition", |a| a.disposition = None),
        ("CONFORME with refusal", |a| a.refusals = vec![code("N1")]),
        ("CONFORME forced", |a| {
            a.justification = Some("x".to_owned())
        }),
        ("NON CONFORME without justification", |a| {
            a.status = Status::NonCompliant;
            a.refusals = vec![code("N1")];
        }),
        ("REFUS with a category", |a| {
            a.status = Status::Refused;
            a.disposition = None;
            a.refusals = vec![code("N10")];
        }),
    ];
    let key = workstation();
    for (name, change) in cases {
        let mut contradictory = host_write(signed_license(&license(None), &root()));
        change(&mut contradictory);
        let signed = sign_attestation(&contradictory, &key).expect("signed");
        assert!(
            matches!(
                verify(&signed, &root_key(), &[]),
                Err(VerifyError::AttestationContent(CborError::Inconsistent(_)))
            ),
            "{name}"
        );
    }

    // The forms Oblivio produces are accepted.
    let mut refused: Attestation = host_write(signed_license(&license(None), &root()));
    refuse(Status::Refused)(&mut refused);
    let mut forced = host_write(signed_license(&license(None), &root()));
    forced.status = Status::NonCompliant;
    forced.disposition = None;
    forced.justification = Some("inspection impossible".to_owned());
    forced.refusals = vec![code("N1")];
    let mut zeros = host_write(signed_license(&license(None), &root()));
    zeros.sanitization.zero_finish = true;
    for fine in [
        attestation(signed_license(&license(None), &root())),
        host_write(signed_license(&license(None), &root())),
        zeros,
        refused,
        forced,
    ] {
        let signed = sign_attestation(&fine, &key).expect("signed");
        verify(&signed, &root_key(), &[]).expect("consistent");
    }
}

#[test]
fn declarative_text_must_be_displayable() {
    use crate::model::{Batch, is_displayable};
    assert!(is_displayable("Atelier XYZ — Lyon", false));
    assert!(is_displayable("ligne 1\nligne 2", true));
    assert!(!is_displayable("ligne 1\nligne 2", false));
    for bad in [
        "\u{202E}",
        "\u{200B}",
        "\u{2067}",
        "\u{FEFF}",
        "\u{E0041}",
        "\t",
        "\u{7}",
    ] {
        assert!(!is_displayable(&format!("a{bad}b"), true), "{bad:?}");
    }
    let batch = |label: &str| Batch {
        id: "LOT-1".to_owned(),
        label: Some(label.to_owned()),
    };
    assert!(batch("Commande 4512").is_valid());
    assert!(!batch("Commande \u{202E}2154").is_valid());

    let key = workstation();
    let mut spoofed = attestation(signed_license(&license(None), &root()));
    spoofed.organization = Some("Atelier \u{202E}ZYX".to_owned());
    let signed = sign_attestation(&spoofed, &key).expect("signed");
    assert!(matches!(
        verify(&signed, &root_key(), &[]),
        Err(VerifyError::AttestationContent(CborError::Inconsistent(_)))
    ));
}
