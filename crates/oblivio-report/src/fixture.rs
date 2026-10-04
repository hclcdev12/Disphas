//! Fictional signed attestations, for tests and the sample example only.
//! The keys are fixed test keys, never used anywhere else.

use oblivio_attestation::ed25519_dalek::SigningKey;
use oblivio_attestation::model::{
    Batch, Category, Code, Device, Disposition, DispositionKind, DriveType, Sanitization, Status,
    Tool, Validation, ValidationMethod,
};
use oblivio_attestation::{Attestation, License, cose, sign_attestation};
use sha2::{Digest, Sha256};

/// Issue date of the fixtures: 2026-10-05T00:00:00Z.
pub const ISSUED_AT: u64 = 1_791_158_400;

/// Journal attached to the fixtures.
pub const JOURNAL: &[u8] = br#"{"timeline":[{"phase":"Executing"}]}"#;

/// Test root key.
pub fn root() -> SigningKey {
    SigningKey::from_bytes(&[1; 32])
}

fn code(text: &str) -> Code {
    Code::new(text).unwrap_or_else(|| panic!("invalid fixture code {text}"))
}

/// A signed attestation with `status`, and fields typical of that status.
pub fn signed(status: Status, organization: &str) -> Vec<u8> {
    signed_drive(status, organization, 1, None)
}

/// Drive `number` (distinct report id, serial and time) erased in `batch`.
pub fn signed_in_batch(status: Status, number: u8, batch: Option<Batch>) -> Vec<u8> {
    signed_drive(status, "Atelier XYZ", number, batch)
}

fn signed_drive(status: Status, organization: &str, number: u8, batch: Option<Batch>) -> Vec<u8> {
    let workstation = SigningKey::from_bytes(&[2; 32]);
    let license = License {
        id: "LIC-0001".to_owned(),
        customer: "Atelier XYZ".to_owned(),
        offer: "Pro".to_owned(),
        not_before: ISSUED_AT - 86_400,
        expires: None,
        workstation_key: workstation.verifying_key().to_bytes(),
    };
    let ran = status != Status::Refused;
    let attestation = Attestation {
        id: [
            0x01, 0x92, 0x8c, 0x3e, 0x5a, 0x1b, 0x70, 0x00, 0x80, 0, 0, 0, 0, 0, 0x42, number,
        ],
        issued_at: ISSUED_AT + u64::from(number) * 600 - 600,
        status,
        device: Device {
            model: "Fictional NVMe SSD 1TB".to_owned(),
            serial: format!("FICT0000A{number:03}"),
            firmware: "FICT01FW".to_owned(),
            capacity_bytes: 1_024_209_543_168,
            drive_type: DriveType::Nvme,
            wwn: None,
            eui64: Some("00a07501fe0c0001".to_owned()),
            nguid: None,
        },
        sanitization: Sanitization {
            category: match status {
                Status::Compliant => Category::Purge,
                Status::NonCompliant => Category::Clear,
                Status::Refused => Category::None,
            },
            method: match status {
                Status::Compliant => Some(code("NvmeSanitizeBlockErase")),
                Status::NonCompliant => Some(code("HostOverwrite")),
                Status::Refused => None,
            },
            references: vec!["NIST SP 800-88r2".to_owned(), "IEEE 2883-2022".to_owned()],
        },
        verified: ran.then_some(true),
        validation: ran.then_some(Validation {
            method: if status == Status::Compliant {
                ValidationMethod::Canaries
            } else {
                ValidationMethod::Pattern
            },
            samples: 1000,
            seed: [7; 32],
            passed: true,
        }),
        disposition: (status == Status::Compliant).then(|| Disposition {
            kind: DispositionKind::DestroyRecommended,
            codes: vec![code("D4")],
        }),
        reserves: match status {
            Status::Compliant => vec![code("R4")],
            Status::NonCompliant => vec![code("R7")],
            Status::Refused => vec![],
        },
        refusals: match status {
            Status::Compliant => vec![],
            Status::NonCompliant | Status::Refused => vec![code("N1")],
        },
        justification: (status == Status::NonCompliant).then(|| {
            "IDENTIFY sans réponse ; le client exige un effacement avant destruction.".to_owned()
        }),
        organization: Some(organization.to_owned()),
        operator: Some("J. Martin".to_owned()),
        batch,
        tool: Tool {
            version: "0.1.0".to_owned(),
            rules_version: "1.0.0".to_owned(),
            binary_sha256: [9; 32],
        },
        journal_sha256: Sha256::digest(JOURNAL).into(),
        license: cose::sign(license.to_cbor(), &root())
            .unwrap_or_else(|error| panic!("fixture license: {error}")),
    };
    sign_attestation(&attestation, &workstation)
        .unwrap_or_else(|error| panic!("fixture attestation: {error}"))
}
