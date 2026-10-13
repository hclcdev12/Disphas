//! Fictional signed attestations, for tests and the sample example only.
//! The keys are fixed test keys, never used anywhere else.

use disphas_attestation::ed25519_dalek::SigningKey;
use disphas_attestation::model::{
    Batch, Category, Code, Device, Disposition, DispositionKind, DriveType, Sanitization, Status,
    Tool, Validation, ValidationMethod,
};
use disphas_attestation::{Attestation, Issuer, License, cose, sign_attestation};
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

fn workstation() -> SigningKey {
    SigningKey::from_bytes(&[2; 32])
}

fn sign(attestation: &Attestation) -> Vec<u8> {
    sign_attestation(attestation, &workstation())
        .unwrap_or_else(|error| panic!("fixture attestation: {error}"))
}

fn signed_drive(status: Status, organization: &str, number: u8, batch: Option<Batch>) -> Vec<u8> {
    sign(&drive(status, organization, number, batch))
}

/// The largest certificate disphas can produce: NON COMPLIANT, every free
/// text at its maximum length with accents, a real license id format, and a
/// license signed by a shop key (version 2, issuer certificate embedded).
/// Sizes the QR code (readable when printed) and the first page layout.
pub fn signed_largest() -> Vec<u8> {
    let phrase = "Disque forcé à la demande du client malgré le refus ; données non sensibles, \
                  réécriture hôte complète. ";
    let fill = |chars: usize| phrase.chars().cycle().take(chars).collect::<String>();
    let workstation = workstation();
    let shop = SigningKey::from_bytes(&[5; 32]);
    let issuer = Issuer {
        id: "ISS-2026-01".to_owned(),
        key: shop.verifying_key().to_bytes(),
        not_before: ISSUED_AT - 30 * 86_400,
        expires: ISSUED_AT + 365 * 86_400,
        offers: vec!["Atelier".to_owned(), "Pro".to_owned()],
        max_duration: 90 * 86_400,
    };
    let license = License {
        id: "DSL-20261011-0123456789abcdef".to_owned(),
        customer: "Société de Reconditionnement Informatique du Grand Est SARL".to_owned(),
        offer: "Atelier".to_owned(),
        not_before: ISSUED_AT - 86_400,
        expires: Some(ISSUED_AT + 37 * 86_400),
        workstation_key: workstation.verifying_key().to_bytes(),
        issuer: Some(
            cose::sign(issuer.to_cbor(), &root())
                .unwrap_or_else(|error| panic!("fixture issuer: {error}")),
        ),
    };
    let mut attestation = drive(
        Status::NonCompliant,
        &fill(Attestation::MAX_ORGANIZATION_CHARS),
        1,
        Some(Batch {
            id: "L".repeat(64),
            label: Some(fill(Batch::MAX_LABEL_CHARS)),
        }),
    );
    attestation.device.model = "Samsung SSD 870 EVO 4TB with a long model string pad".to_owned();
    attestation.device.serial = "S6PNNX0T123456789ABCDEF".to_owned();
    attestation.device.wwn = Some("5002538f42a1b2c3".to_owned());
    attestation.sanitization.zero_finish = true;
    attestation.refusals = vec![code("N1"), code("N3")];
    attestation.justification = Some(fill(Attestation::MAX_JUSTIFICATION_CHARS));
    attestation.operator = Some(fill(Attestation::MAX_OPERATOR_CHARS));
    attestation.license = cose::sign(license.to_cbor(), &shop)
        .unwrap_or_else(|error| panic!("fixture license: {error}"));
    sign(&attestation)
}

fn drive(status: Status, organization: &str, number: u8, batch: Option<Batch>) -> Attestation {
    let workstation = workstation();
    let license = License {
        id: "LIC-0001".to_owned(),
        customer: "Atelier XYZ".to_owned(),
        offer: "Pro".to_owned(),
        not_before: ISSUED_AT - 86_400,
        expires: None,
        workstation_key: workstation.verifying_key().to_bytes(),
        issuer: None,
    };
    let ran = status != Status::Refused;
    Attestation {
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
            zero_finish: false,
            // An NVMe Sanitize erases every namespace of the controller.
            all_namespaces: status == Status::Compliant,
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
    }
}
