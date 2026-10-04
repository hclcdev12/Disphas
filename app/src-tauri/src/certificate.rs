//! PDF certificates, rendered on the user side from the signed attestation
//! (`docs/certificat.md` §6). The app never signs: it renders what the
//! helper signed, after verifying it.

use std::path::{Path, PathBuf};

use base64::Engine;
use oblivio_attestation::display::Lang;
use oblivio_attestation::ed25519_dalek::VerifyingKey;
use oblivio_protocol::response::JobResult;
use oblivio_report::{RenderOptions, render};

/// The official Oblivio root public key. Not generated yet.
const OFFICIAL_ROOT_KEY: Option<[u8; 32]> = None;

/// Root key certificates must chain to: the official one; in development
/// builds only, the public development root as a fallback.
pub fn root_key() -> Option<VerifyingKey> {
    if let Some(bytes) = OFFICIAL_ROOT_KEY {
        return VerifyingKey::from_bytes(&bytes).ok();
    }
    #[cfg(debug_assertions)]
    {
        Some(oblivio_license::dev_root::verifying_key())
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

/// Folder where certificates are saved: `~/Documents/Oblivio`.
pub fn output_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join("Documents/Oblivio"))
}

/// Renders the certificate of a finished job in `lang` and saves it.
pub fn save(result: &JobResult, lang: Lang) -> Result<PathBuf, String> {
    let signed = base64::engine::general_purpose::STANDARD
        .decode(&result.attestation)
        .map_err(|error| format!("attestation: {error}"))?;
    let root = root_key().ok_or("this build has no root key to verify certificates")?;
    let pdf = render(
        &signed,
        result.journal.as_bytes(),
        &root,
        &[],
        &RenderOptions { lang, logo: None },
    )
    .map_err(|error| error.to_string())?;
    let dir = output_dir()?;
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let tag = match lang {
        Lang::En => "en",
        Lang::Fr => "fr",
        Lang::De => "de",
    };
    let path = dir.join(format!("oblivio-{}-{tag}.pdf", result.job_id.0));
    std::fs::write(&path, pdf).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(path)
}

/// Opens a saved certificate with the desktop's PDF viewer. Only files in
/// the certificate folder can be opened.
pub fn open(path: &Path) -> Result<(), String> {
    let dir = output_dir()?
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let path = path.canonicalize().map_err(|error| error.to_string())?;
    if !path.starts_with(&dir) || path.extension().is_none_or(|ext| ext != "pdf") {
        return Err("only certificates in the Oblivio folder can be opened".to_owned());
    }
    std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("xdg-open: {error}"))
}
