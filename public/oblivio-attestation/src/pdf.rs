//! Reading back the files attached to a certificate PDF: the signed
//! attestation (`attestation.cose`) and the journal (`journal.json`). The PDF
//! itself proves nothing; only these attachments are verified.
//!
//! Behind the `pdf` feature (adds `lopdf`, Rust 1.88).

use lopdf::{Document, Object};

/// Largest decoded attachment accepted (an attestation is a few KiB, a
/// journal well under this).
const MAX_ATTACHMENT_BYTES: usize = 16 << 20;

/// The attachments of a certificate PDF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachments {
    /// `attestation.cose`, the signed attestation.
    pub attestation: Vec<u8>,
    /// `journal.json`, if attached.
    pub journal: Option<Vec<u8>>,
}

/// Why attachments could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExtractError {
    /// Not a readable PDF.
    #[error("not a readable PDF: {0}")]
    NotPdf(String),
    /// A readable PDF without `attestation.cose`: not an Oblivio certificate.
    #[error("no attestation attached")]
    NoAttestation,
    /// The same file name is attached twice: ambiguous, refused.
    #[error("{0} attached more than once")]
    Duplicate(String),
}

/// Text of a PDF string: UTF-16BE with a byte order mark, else bytes as is.
fn pdf_text(bytes: &[u8]) -> String {
    match bytes.strip_prefix(&[0xfe, 0xff]) {
        Some(utf16) => {
            let units: Vec<u16> = utf16
                .chunks_exact(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        }
        None => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Content of the embedded file a file specification points to.
fn embedded(document: &Document, spec: &lopdf::Dictionary) -> Option<Vec<u8>> {
    let files = spec.get(b"EF").ok()?;
    let files = match files {
        Object::Reference(id) => document.get_object(*id).ok()?.as_dict().ok()?,
        other => other.as_dict().ok()?,
    };
    let stream = match files.get(b"F").ok()? {
        Object::Reference(id) => document.get_object(*id).ok()?.as_stream().ok()?,
        other => other.as_stream().ok()?,
    };
    stream
        .decompressed_content_with_limit(MAX_ATTACHMENT_BYTES)
        .ok()
}

/// Most files read from one PDF (a batch of 2000 drives has 4000).
const MAX_FILES: usize = 4096;
/// Largest total of decoded files.
const MAX_TOTAL_BYTES: usize = 256 << 20;

fn read_all(pdf: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ExtractError> {
    let document =
        Document::load_mem(pdf).map_err(|error| ExtractError::NotPdf(error.to_string()))?;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total = 0usize;
    for object in document.objects.values() {
        let Ok(spec) = object.as_dict() else {
            continue;
        };
        if spec.get(b"Type").and_then(Object::as_name).ok() != Some(b"Filespec".as_slice()) {
            continue;
        }
        let name = spec
            .get(b"UF")
            .or_else(|_| spec.get(b"F"))
            .and_then(Object::as_str)
            .map(pdf_text)
            .unwrap_or_default();
        let Some(content) = embedded(&document, spec) else {
            continue;
        };
        if files.iter().any(|(other, _)| *other == name) {
            return Err(ExtractError::Duplicate(name));
        }
        total = total.saturating_add(content.len());
        if files.len() >= MAX_FILES || total > MAX_TOTAL_BYTES {
            return Err(ExtractError::NotPdf(
                "too many or too large attachments".to_owned(),
            ));
        }
        files.push((name, content));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

fn read(pdf: &[u8]) -> Result<Attachments, ExtractError> {
    let mut attestation = None;
    let mut journal = None;
    for (name, content) in read_all(pdf)? {
        match name.as_str() {
            "attestation.cose" => attestation = Some(content),
            "journal.json" => journal = Some(content),
            _ => {}
        }
    }
    Ok(Attachments {
        attestation: attestation.ok_or(ExtractError::NoAttestation)?,
        journal,
    })
}

/// Every file attached to a PDF, sorted by name: a batch certificate attaches
/// `<report>.cose` and `<report>.json` per drive. Same bounds as
/// [`attachments`]; a name attached twice is refused.
pub fn all_attachments(pdf: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ExtractError> {
    std::panic::catch_unwind(|| read_all(pdf))
        .unwrap_or_else(|_| Err(ExtractError::NotPdf("malformed PDF".to_owned())))
}

/// Reads the attachments of a certificate PDF. Untrusted input: decoded
/// sizes are bounded, and a parser panic is turned into an error.
pub fn attachments(pdf: &[u8]) -> Result<Attachments, ExtractError> {
    std::panic::catch_unwind(|| read(pdf))
        .unwrap_or_else(|_| Err(ExtractError::NotPdf("malformed PDF".to_owned())))
}
