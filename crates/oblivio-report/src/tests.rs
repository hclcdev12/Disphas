//! Rendering tests on fictional attestations.

use oblivio_attestation::ed25519_dalek::SigningKey;
use oblivio_attestation::model::Status;

use super::*;
use crate::fixture::{JOURNAL, root, signed};

fn render_with(status: Status, organization: &str, lang: Lang) -> Result<Vec<u8>, ReportError> {
    render(
        &signed(status, organization),
        JOURNAL,
        &root().verifying_key(),
        &[],
        &RenderOptions { lang, logo: None },
    )
}

#[test]
fn renders_every_status_in_every_language() {
    for status in [Status::Compliant, Status::NonCompliant, Status::Refused] {
        for lang in [Lang::En, Lang::Fr, Lang::De] {
            let pdf = render_with(status, "Atelier XYZ", lang)
                .unwrap_or_else(|error| panic!("{status:?} {lang:?}: {error}"));
            assert!(pdf.starts_with(b"%PDF-"));
            let xmp = String::from_utf8_lossy(&pdf);
            assert!(
                xmp.contains("<pdfaid:part>3</pdfaid:part>"),
                "PDF/A-3 declared"
            );
            assert!(xmp.contains("<pdfaid:conformance>B</pdfaid:conformance>"));
        }
    }
}

#[test]
fn rendering_is_deterministic() {
    let first = render_with(Status::Compliant, "Atelier XYZ", Lang::Fr).expect("rendered");
    let second = render_with(Status::Compliant, "Atelier XYZ", Lang::Fr).expect("rendered");
    assert_eq!(first, second);
}

#[test]
fn operator_text_is_never_evaluated_as_markup() {
    for hostile in [
        r#"#panic("injected")"#,
        "] #panic(\"x\") [",
        "$ x^2 $ * _ ` <label> @ref",
        "Œuvre & Łódź — «guillemets»",
    ] {
        render_with(Status::NonCompliant, hostile, Lang::Fr)
            .unwrap_or_else(|error| panic!("{hostile:?}: {error}"));
    }
}

#[test]
fn forged_attestation_is_not_rendered() {
    let other_root = SigningKey::from_bytes(&[3; 32]).verifying_key();
    let result = render(
        &signed(Status::Compliant, "x"),
        JOURNAL,
        &other_root,
        &[],
        &RenderOptions {
            lang: Lang::En,
            logo: None,
        },
    );
    assert!(matches!(result, Err(ReportError::Verify(_))));
}

#[test]
fn foreign_journal_is_not_attached() {
    let result = render(
        &signed(Status::Compliant, "x"),
        b"{}",
        &root().verifying_key(),
        &[],
        &RenderOptions {
            lang: Lang::En,
            logo: None,
        },
    );
    assert!(matches!(result, Err(ReportError::JournalMismatch)));
}

#[test]
fn custom_logo_is_accepted() {
    let logo = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 10"><rect width="40" height="10" fill="#6366F1"/></svg>"##;
    let pdf = render(
        &signed(Status::Compliant, "x"),
        JOURNAL,
        &root().verifying_key(),
        &[],
        &RenderOptions {
            lang: Lang::De,
            logo: Some(Logo {
                data: logo,
                format: LogoFormat::Svg,
            }),
        },
    )
    .expect("rendered");
    assert!(pdf.starts_with(b"%PDF-"));
}

#[test]
fn remote_packages_cannot_be_imported() {
    let template = "#import \"@preview/example:0.1.0\": *\nHello";
    let result = compile_pdf(template, Dict::new(), fixture::ISSUED_AT, "test");
    assert!(matches!(result, Err(ReportError::Layout(_))));
}
