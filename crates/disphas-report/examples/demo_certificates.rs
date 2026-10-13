//! Fictional certificates for the website's interactive demo (docs/site.md):
//! one PDF per demo drive, method and language, rendered by the real
//! renderer with the public test keys, so they look real and never verify
//! against the official root.
//!
//! `cargo run -p disphas-report --features fixtures --example demo_certificates -- <out-dir>`
//!
//! Files are named `<serial>-<method>[-zero]-<lang>.pdf`, the name the demo
//! (`app/src/demo/`) asks for. Drives, methods and outcomes mirror it.

use disphas_attestation::display::Lang;
use disphas_attestation::ed25519_dalek::SigningKey;
use disphas_attestation::model::{
    Category, Code, Device, Disposition, DispositionKind, DriveType, Sanitization, Status, Tool,
    Validation, ValidationMethod,
};
use disphas_attestation::{Attestation, License, cose, sign_attestation};
use disphas_report::{RenderOptions, fixture, render};
use sha2::{Digest, Sha256};

/// How an erasure of the demo ends.
#[derive(Clone, Copy)]
enum Outcome {
    /// Erased, verified, validated.
    Compliant,
    /// Erased, but samples could not be read back during validation (N8).
    ReadErrors,
    /// A SAY NO drive erased on the operator's insistence (N1).
    Forced,
}

struct Drive {
    model: &'static str,
    serial: &'static str,
    capacity_bytes: u64,
    drive_type: DriveType,
    wwn: Option<&'static str>,
    eui64: Option<&'static str>,
    destroy: Option<&'static str>,
    outcome: Outcome,
    /// Methods offered by the demo, with their category and reserves.
    methods: &'static [(&'static str, Category, &'static [&'static str], bool)],
}

const DRIVES: &[Drive] = &[
    Drive {
        model: "DEMO NVMe Data 2TB",
        serial: "DEMO-NVME-4567",
        capacity_bytes: 2_000_398_934_016,
        drive_type: DriveType::Nvme,
        wwn: None,
        eui64: Some("0011223344556677"),
        destroy: None,
        outcome: Outcome::Compliant,
        methods: &[
            ("NvmeSanitizeBlockErase", Category::Purge, &[], true),
            ("NvmeSanitizeCryptoErase", Category::Purge, &[], true),
            ("NvmeFormatCryptoErase", Category::Purge, &[], false),
            ("NvmeFormatUserDataErase", Category::Clear, &[], false),
            ("HostOverwrite", Category::Clear, &["R7"], false),
        ],
    },
    Drive {
        model: "DEMO HDD 4TB",
        serial: "DEMO-HDD-8C21",
        capacity_bytes: 4_000_787_030_016,
        drive_type: DriveType::SataHdd,
        wwn: Some("5000c500deadbeef"),
        eui64: None,
        destroy: Some("D4"),
        outcome: Outcome::Compliant,
        methods: &[
            ("AtaSanitizeOverwrite", Category::Purge, &["R6"], false),
            ("AtaSecurityEraseEnhanced", Category::Purge, &["R6"], false),
            ("HostOverwrite", Category::Clear, &["R6", "R7"], false),
        ],
    },
    Drive {
        model: "DEMO SATA SSD 500GB",
        serial: "DEMO-SSD-F0Z3",
        capacity_bytes: 500_107_862_016,
        drive_type: DriveType::SataSsd,
        wwn: Some("5002538e00000001"),
        eui64: None,
        destroy: None,
        outcome: Outcome::ReadErrors,
        methods: &[
            ("AtaSanitizeBlockErase", Category::Purge, &[], false),
            ("AtaSecurityEraseEnhanced", Category::Purge, &[], false),
            ("HostOverwrite", Category::Clear, &["R7"], false),
        ],
    },
    Drive {
        model: "DEMO Unresponsive 1TB",
        serial: "DEMO-MUTE-0N1X",
        // Never identified: the helper certifies what it read, nothing.
        capacity_bytes: 0,
        drive_type: DriveType::Unknown,
        wwn: None,
        eui64: None,
        destroy: None,
        outcome: Outcome::Forced,
        methods: &[("HostOverwrite", Category::Clear, &["R7"], false)],
    },
    Drive {
        model: "DEMO SATA SSD Locked 256GB",
        serial: "DEMO-LOCK-7A11",
        capacity_bytes: 256_060_514_304,
        drive_type: DriveType::SataSsd,
        wwn: Some("5002538e00000002"),
        eui64: None,
        destroy: None,
        outcome: Outcome::Compliant,
        methods: &[
            ("AtaSanitizeBlockErase", Category::Purge, &[], false),
            ("AtaSecurityEraseEnhanced", Category::Purge, &[], false),
            ("HostOverwrite", Category::Clear, &["R7"], false),
        ],
    },
];

fn code(text: &str) -> Code {
    Code::new(text).unwrap_or_else(|| panic!("invalid code {text}"))
}

fn codes(texts: &[&str]) -> Vec<Code> {
    texts.iter().map(|text| code(text)).collect()
}

fn attestation(
    drive: &Drive,
    (method, category, reserves, all_namespaces): (&str, Category, &[&str], bool),
    zero_pass: bool,
    number: u8,
    license: Vec<u8>,
) -> Attestation {
    let host = method == "HostOverwrite";
    let validated = !matches!(drive.outcome, Outcome::ReadErrors);
    let (status, category, disposition, reserves, refusals, justification) = match drive.outcome {
        Outcome::Compliant => (
            Status::Compliant,
            category,
            Some(match drive.destroy {
                Some(reason) => Disposition {
                    kind: DispositionKind::DestroyRecommended,
                    codes: codes(&[reason]),
                },
                None => Disposition {
                    kind: DispositionKind::Reuse,
                    codes: Vec::new(),
                },
            }),
            codes(reserves),
            Vec::new(),
            None,
        ),
        Outcome::ReadErrors => (
            Status::Refused,
            Category::None,
            None,
            Vec::new(),
            codes(&["N8"]),
            None,
        ),
        Outcome::Forced => (
            Status::NonCompliant,
            category,
            None,
            codes(reserves),
            codes(&["N1"]),
            Some(
                "Démonstration : disque muet effacé à la demande du client avant destruction."
                    .to_owned(),
            ),
        ),
    };
    Attestation {
        id: [
            0x01, 0x92, 0x8c, 0x3e, 0x5a, 0x1b, 0x70, 0x00, 0x80, 0, 0, 0, 0, 0xde, 0x40, number,
        ],
        issued_at: fixture::ISSUED_AT,
        status,
        device: Device {
            model: drive.model.to_owned(),
            serial: drive.serial.to_owned(),
            firmware: "DEMO01".to_owned(),
            capacity_bytes: drive.capacity_bytes,
            drive_type: drive.drive_type.clone(),
            wwn: drive.wwn.map(str::to_owned),
            eui64: drive.eui64.map(str::to_owned),
            nguid: None,
        },
        sanitization: Sanitization {
            category,
            method: Some(code(method)),
            zero_finish: zero_pass && host && validated,
            all_namespaces,
            references: vec!["NIST SP 800-88r2".to_owned(), "IEEE 2883-2022".to_owned()],
        },
        verified: Some(true),
        validation: Some(Validation {
            method: if host {
                ValidationMethod::Pattern
            } else {
                ValidationMethod::Canaries
            },
            samples: 1000,
            seed: [7; 32],
            passed: validated,
        }),
        disposition,
        reserves,
        refusals,
        justification,
        organization: Some("Atelier XYZ".to_owned()),
        operator: Some("OP-042".to_owned()),
        batch: None,
        tool: Tool {
            version: "0.1.0".to_owned(),
            rules_version: "1.0.0".to_owned(),
            binary_sha256: [9; 32],
        },
        journal_sha256: Sha256::digest(fixture::JOURNAL).into(),
        license,
    }
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".to_owned());
    let root = fixture::root();
    let workstation = SigningKey::from_bytes(&[2; 32]);
    let license = License {
        id: "DEMO-0001".to_owned(),
        customer: "disphas — démonstration".to_owned(),
        offer: "Demo".to_owned(),
        not_before: fixture::ISSUED_AT - 86_400,
        expires: None,
        workstation_key: workstation.verifying_key().to_bytes(),
        issuer: None,
    };
    let license =
        cose::sign(license.to_cbor(), &root).unwrap_or_else(|error| panic!("license: {error}"));
    let mut number = 0u8;
    let mut written = 0;
    for drive in DRIVES {
        for &choice in drive.methods {
            let zero_variants: &[bool] = if choice.0 == "HostOverwrite" {
                &[false, true]
            } else {
                &[false]
            };
            for &zero_pass in zero_variants {
                number += 1;
                let attestation = attestation(drive, choice, zero_pass, number, license.clone());
                let signed = sign_attestation(&attestation, &workstation)
                    .unwrap_or_else(|error| panic!("{}: {error}", drive.serial));
                for (lang, tag) in [(Lang::En, "en"), (Lang::Fr, "fr"), (Lang::De, "de")] {
                    let options = RenderOptions { lang, logo: None };
                    let pdf = render(
                        &signed,
                        fixture::JOURNAL,
                        &root.verifying_key(),
                        &[],
                        &options,
                    )
                    .unwrap_or_else(|error| panic!("{} {}: {error}", drive.serial, choice.0));
                    let zero = if zero_pass { "-zero" } else { "" };
                    let path = format!("{dir}/{}-{}{zero}-{tag}.pdf", drive.serial, choice.0);
                    std::fs::write(&path, pdf).unwrap_or_else(|error| panic!("{path}: {error}"));
                    written += 1;
                }
            }
        }
    }
    eprintln!("{written} demo certificates written to {dir}");
}
