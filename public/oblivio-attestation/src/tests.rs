//! End-to-end tests of the trust chain, on fictional keys and drives.

use ciborium::Value;
use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};

use crate::cbor::CborError;
use crate::cose::{self, CoseError};
use crate::model::{
    Category, Code, Device, Disposition, DispositionKind, DriveType, Sanitization, Status, Tool,
    Validation, ValidationMethod,
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
            model: "Micron MTFDKCD1T0TGE-1BK1AABLA".to_owned(),
            serial: "241248C8AF57".to_owned(),
            firmware: "1002V6LN".to_owned(),
            capacity_bytes: 1_024_209_543_168,
            drive_type: DriveType::Nvme,
            wwn: None,
            eui64: Some("00a0750148c8af57".to_owned()),
            nguid: None,
        },
        sanitization: Sanitization {
            category: Category::Purge,
            method: Some(code("NvmeSanitizeBlockErase")),
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
    assert_eq!(verified.attestation.device.serial, "241248C8AF57");
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
