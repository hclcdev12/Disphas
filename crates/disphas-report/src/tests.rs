//! Rendering tests on fictional attestations.

use disphas_attestation::ed25519_dalek::SigningKey;
use disphas_attestation::model::Status;

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
fn largest_certificate_keeps_a_readable_qr_code() {
    let signed = crate::fixture::signed_largest();
    let text = disphas_attestation::to_qr_text(&signed);
    let module = crate::qr::printed_module_mm(&text).expect("encodable");
    assert!(
        module >= crate::qr::MIN_MODULE_MM,
        "{} QR characters print {module:.3} mm modules",
        text.len()
    );
    render(
        &signed,
        JOURNAL,
        &root().verifying_key(),
        &[],
        &RenderOptions {
            lang: Lang::De,
            logo: None,
        },
    )
    .expect("largest certificate rendered");
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

#[test]
fn attachments_are_read_back_from_the_pdf() {
    let signed_bytes = signed(Status::Compliant, "Atelier XYZ");
    let pdf = render(
        &signed_bytes,
        JOURNAL,
        &root().verifying_key(),
        &[],
        &RenderOptions {
            lang: Lang::Fr,
            logo: None,
        },
    )
    .expect("rendered");
    let attached = disphas_attestation::pdf::attachments(&pdf).expect("attachments");
    assert_eq!(attached.attestation, signed_bytes);
    assert_eq!(attached.journal.as_deref(), Some(JOURNAL));
}

#[test]
fn foreign_or_broken_pdfs_are_refused() {
    use disphas_attestation::pdf::{ExtractError, attachments};
    assert!(matches!(
        attachments(b"not a pdf"),
        Err(ExtractError::NotPdf(_))
    ));
    assert!(matches!(
        attachments(b"%PDF-1.7\n%%EOF"),
        Err(ExtractError::NotPdf(_))
    ));
    // Truncated certificate: never a panic.
    let pdf = render_with(Status::Refused, "x", Lang::En).expect("rendered");
    for cut in [10, pdf.len() / 3, pdf.len() / 2, pdf.len() - 10] {
        let _ = attachments(&pdf[..cut]);
    }
}

mod batch {
    use super::*;
    use crate::batch::{BatchEntry, BatchError, render_batch};
    use crate::fixture::signed_in_batch;
    use disphas_attestation::model::Batch;

    fn lot(id: &str) -> Option<Batch> {
        Some(Batch {
            id: id.to_owned(),
            label: Some("Commande 4512".to_owned()),
        })
    }

    fn render_all(signed: &[Vec<u8>], lang: Lang) -> Result<Vec<u8>, BatchError> {
        let entries: Vec<BatchEntry<'_>> = signed
            .iter()
            .map(|signed| BatchEntry {
                signed,
                journal: JOURNAL,
            })
            .collect();
        render_batch(
            &entries,
            &root().verifying_key(),
            &[],
            &RenderOptions { lang, logo: None },
        )
    }

    #[test]
    fn batch_certificate_carries_every_drive() {
        let drives = vec![
            signed_in_batch(Status::Compliant, 1, lot("LOT-2026-001")),
            signed_in_batch(Status::NonCompliant, 2, lot("LOT-2026-001")),
            signed_in_batch(Status::Refused, 3, lot("LOT-2026-001")),
        ];
        for lang in [Lang::En, Lang::Fr, Lang::De] {
            let pdf = render_all(&drives, lang).expect("rendered");
            assert!(pdf.starts_with(b"%PDF-"));
            assert!(String::from_utf8_lossy(&pdf).contains("<pdfaid:part>3</pdfaid:part>"));
        }
        let pdf = render_all(&drives, Lang::Fr).expect("rendered");
        assert_eq!(
            pdf,
            render_all(&drives, Lang::Fr).expect("again"),
            "deterministic"
        );
        // Every attestation is attached, byte for byte.
        let files = disphas_attestation::pdf::all_attachments(&pdf).expect("attachments");
        for signed in &drives {
            assert!(files.iter().any(|(_, data)| data == signed));
        }
        assert_eq!(files.len(), 6, "an attestation and a journal per drive");
    }

    #[test]
    fn mixed_missing_or_duplicate_batches_are_refused() {
        let one = signed_in_batch(Status::Compliant, 1, lot("LOT-A"));
        let other = signed_in_batch(Status::Compliant, 2, lot("LOT-B"));
        let none = signed_in_batch(Status::Compliant, 3, None);
        assert!(matches!(
            render_all(&[one.clone(), other], Lang::En),
            Err(BatchError::MixedBatches(2))
        ));
        assert!(matches!(
            render_all(&[one.clone(), none], Lang::En),
            Err(BatchError::NoBatch(2))
        ));
        assert!(matches!(
            render_all(&[one.clone(), one], Lang::En),
            Err(BatchError::Duplicate(2))
        ));
        assert!(matches!(render_all(&[], Lang::En), Err(BatchError::Size)));
    }

    #[test]
    fn forged_drive_blocks_the_whole_batch() {
        let mut forged = signed_in_batch(Status::Compliant, 2, lot("LOT-A"));
        let last = forged.len() - 1;
        forged[last] ^= 1;
        let drives = vec![signed_in_batch(Status::Compliant, 1, lot("LOT-A")), forged];
        assert!(matches!(
            render_all(&drives, Lang::En),
            Err(BatchError::Drive { index: 2, .. })
        ));
    }
}

#[test]
fn custom_logos_render_in_pdf_a() {
    let png = include_bytes!("test-logo.png");
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="10"><rect width="40" height="10" fill="#6366F1"/></svg>"##;
    for logo in [
        Logo {
            data: png,
            format: LogoFormat::Png,
        },
        Logo {
            data: svg,
            format: LogoFormat::Svg,
        },
    ] {
        let pdf = render(
            &signed(Status::Compliant, "Atelier XYZ"),
            JOURNAL,
            &root().verifying_key(),
            &[],
            &RenderOptions {
                lang: Lang::Fr,
                logo: Some(logo),
            },
        )
        .unwrap_or_else(|error| panic!("{:?}: {error}", logo.format));
        assert!(pdf.starts_with(b"%PDF-"));
    }
}
