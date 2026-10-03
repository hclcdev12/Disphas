//! Writes sample certificates with fictional data and test keys:
//! `cargo run -p oblivio-report --example sample -- <out-dir>`.

use oblivio_attestation::display::Lang;
use oblivio_attestation::model::Status;
use oblivio_report::fixture;
use oblivio_report::{RenderOptions, render};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".to_owned());
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
