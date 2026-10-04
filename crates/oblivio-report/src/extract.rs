//! Reading back the files attached to a certificate PDF: the signed
//! attestation (`attestation.cose`) and the journal (`journal.json`). The PDF
//! itself proves nothing; only these attachments are verified.

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
    Duplicate(&'static str),
}

/// Text of a PDF string: UTF-16BE with a byte order mark, else bytes as is.
fn pdf_text(bytes: &[u8]) -> String {
    match bytes.strip_prefix(&[0xfe, 0xff]) {
        Some(utf16) => {
            let units: Vec<u16> = utf16
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_be_bytes(*pair))
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

fn read(pdf: &[u8]) -> Result<Attachments, ExtractError> {
    let document =
        Document::load_mem(pdf).map_err(|error| ExtractError::NotPdf(error.to_string()))?;
    let mut attestation = None;
    let mut journal = None;
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
        let (slot, label) = match name.as_str() {
            "attestation.cose" => (&mut attestation, "attestation.cose"),
            "journal.json" => (&mut journal, "journal.json"),
            _ => continue,
        };
        let Some(content) = embedded(&document, spec) else {
            continue;
        };
        if slot.is_some() {
            return Err(ExtractError::Duplicate(label));
        }
        *slot = Some(content);
    }
    Ok(Attachments {
        attestation: attestation.ok_or(ExtractError::NoAttestation)?,
        journal,
    })
}

/// Reads the attachments of a certificate PDF. Untrusted input: decoded
/// sizes are bounded, and a parser panic is turned into an error.
pub fn attachments(pdf: &[u8]) -> Result<Attachments, ExtractError> {
    std::panic::catch_unwind(|| read(pdf))
        .unwrap_or_else(|_| Err(ExtractError::NotPdf("malformed PDF".to_owned())))
}
