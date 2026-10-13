//! Writes sample certificates with fictional data and test keys:
//! `cargo run -p disphas-report --example sample -- <out-dir>`.

use disphas_attestation::display::Lang;
use disphas_attestation::model::Status;
use disphas_report::fixture;
use disphas_report::{RenderOptions, render};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".to_owned());
    batch(&dir);
    largest(&dir);
    for (status, name) in [
        (Status::Compliant, "compliant"),
        (Status::NonCompliant, "non-compliant"),
        (Status::Refused, "refused"),
    ] {
        for (lang, tag) in [(Lang::En, "en"), (Lang::Fr, "fr"), (Lang::De, "de")] {
            let signed = fixture::signed(status, "Atelier Œuvre & Łódź");
            let options = RenderOptions { lang, logo: None };
            match render(
                &signed,
                fixture::JOURNAL,
                &fixture::root().verifying_key(),
                &[],
                &options,
            ) {
                Ok(pdf) => {
                    let path = format!("{dir}/{name}-{tag}.pdf");
                    if let Err(error) = std::fs::write(&path, pdf) {
                        eprintln!("{path}: {error}");
                    }
                }
                Err(error) => eprintln!("{name}-{tag}: {error}"),
            }
        }
    }
}

/// The largest certificate (every text at its maximum), in French and German:
/// print it to check that the QR code still scans.
fn largest(dir: &str) {
    let signed = fixture::signed_largest();
    for (lang, tag) in [(Lang::Fr, "fr"), (Lang::De, "de")] {
        let options = RenderOptions { lang, logo: None };
        match render(
            &signed,
            fixture::JOURNAL,
            &fixture::root().verifying_key(),
            &[],
            &options,
        ) {
            Ok(pdf) => {
                let path = format!("{dir}/largest-{tag}.pdf");
                if let Err(error) = std::fs::write(&path, pdf) {
                    eprintln!("{path}: {error}");
                }
            }
            Err(error) => eprintln!("largest-{tag}: {error}"),
        }
    }
}

/// A batch of six fictional drives, in French.
fn batch(dir: &str) {
    use disphas_attestation::model::Batch;
    use disphas_report::batch::{BatchEntry, render_batch};
    let lot = Batch {
        id: "LOT-2026-001".to_owned(),
        label: Some("Commande 4512 — parc Atelier XYZ".to_owned()),
    };
    let statuses = [
        Status::Compliant,
        Status::Compliant,
        Status::NonCompliant,
        Status::Compliant,
        Status::Refused,
        Status::Compliant,
    ];
    let signed: Vec<Vec<u8>> = statuses
        .iter()
        .zip(1u8..)
        .map(|(status, number)| fixture::signed_in_batch(*status, number, Some(lot.clone())))
        .collect();
    let entries: Vec<BatchEntry<'_>> = signed
        .iter()
        .map(|signed| BatchEntry {
            signed,
            journal: fixture::JOURNAL,
        })
        .collect();
    let options = RenderOptions {
        lang: Lang::Fr,
        logo: None,
    };
    match render_batch(&entries, &fixture::root().verifying_key(), &[], &options) {
        Ok(pdf) => {
            let path = format!("{dir}/batch-fr.pdf");
            if let Err(error) = std::fs::write(&path, pdf) {
                eprintln!("{path}: {error}");
            }
        }
        Err(error) => eprintln!("batch: {error}"),
    }
}
