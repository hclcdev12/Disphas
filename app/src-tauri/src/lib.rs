//! Oblivio desktop application: the Tauri shell between the React interface
//! and the root helper. Runs as the normal user; renders, translates and
//! relays, but never touches a device and never decides.

mod certificate;
mod helper;
mod history;
mod settings;
mod trust;
mod verification;

use std::collections::HashMap;

use oblivio_attestation::display::{KNOWN_CODES, Lang, code};
use oblivio_protocol::command::Command;
use oblivio_protocol::response::{ErrorBody, ErrorCode};
use tauri::{Emitter, Manager, State};

use crate::helper::Helper;

/// The helper of this session, or why it could not be launched.
struct HelperState(Result<Helper, String>);

/// Forwards one command to the helper and returns its result, or its error.
#[tauri::command]
async fn helper(
    command: Command,
    state: State<'_, HelperState>,
) -> Result<serde_json::Value, ErrorBody> {
    let helper = state.0.as_ref().map_err(|reason| ErrorBody {
        code: ErrorCode::Internal,
        detail: Some(reason.clone()),
    })?;
    helper.request(command).await?.outcome
}

/// Renders the PDF certificate of a finished job in `lang` and saves it in
/// `~/Documents/Oblivio`. Returns the path of the file.
#[tauri::command]
async fn render_certificate(
    job_id: String,
    lang: String,
    state: State<'_, HelperState>,
    app: tauri::AppHandle,
) -> Result<String, ErrorBody> {
    let failure = |detail: String| ErrorBody {
        code: ErrorCode::Internal,
        detail: Some(detail),
    };
    let helper = state.0.as_ref().map_err(|reason| failure(reason.clone()))?;
    let command = Command::GetJobResult(oblivio_protocol::command::JobArgs {
        job_id: oblivio_protocol::command::JobId(job_id),
    });
    let value = helper.request(command).await?.outcome?;
    let result: oblivio_protocol::response::JobResult =
        serde_json::from_value(value).map_err(|error| failure(error.to_string()))?;
    let lang = Lang::from_tag(&lang).unwrap_or(Lang::En);
    // Typst layout is CPU-bound: keep it off the async runtime.
    let logo = custom_logo(&app);
    let path =
        tauri::async_runtime::spawn_blocking(move || certificate::save(&result, lang, &logo))
            .await
            .map_err(|error| failure(error.to_string()))?
            .map_err(failure)?;
    Ok(path.to_string_lossy().into_owned())
}

/// The certificate history, newest first, each report verified.
#[tauri::command]
async fn list_reports() -> Result<Vec<history::Entry>, String> {
    // Verifying thousands of signatures: off the async runtime.
    tauri::async_runtime::spawn_blocking(|| {
        let trust = trust::Trust::shipped()?;
        history::list(&history::reports_dir(), &trust)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Renders the PDF certificate of report `id` from the history, in `lang`.
#[tauri::command]
async fn render_report(
    id: String,
    lang: String,
    app: tauri::AppHandle,
) -> Result<String, ErrorBody> {
    let failure = |detail: String| ErrorBody {
        code: ErrorCode::Internal,
        detail: Some(detail),
    };
    let lang = Lang::from_tag(&lang).unwrap_or(Lang::En);
    let logo = custom_logo(&app);
    let path = tauri::async_runtime::spawn_blocking(move || {
        let (signed, journal) =
            history::read(&history::reports_dir(), &id).ok_or("report not found")?;
        let trust = trust::Trust::shipped()?;
        certificate::save_signed(&id, &signed, &journal, lang, &trust, &logo)
    })
    .await
    .map_err(|error| failure(error.to_string()))?
    .map_err(failure)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Renders the batch certificate of `batch_id` from the history; `None` for
/// a batch without any certificate.
#[tauri::command]
async fn render_batch(
    batch_id: String,
    lang: String,
    app: tauri::AppHandle,
) -> Result<Option<String>, ErrorBody> {
    let failure = |detail: String| ErrorBody {
        code: ErrorCode::Internal,
        detail: Some(detail),
    };
    let lang = Lang::from_tag(&lang).unwrap_or(Lang::En);
    let logo = custom_logo(&app);
    let path = tauri::async_runtime::spawn_blocking(move || {
        let trust = trust::Trust::shipped()?;
        certificate::save_batch(&batch_id, lang, &trust, &logo)
    })
    .await
    .map_err(|error| failure(error.to_string()))?
    .map_err(failure)?;
    Ok(path.map(|path| path.to_string_lossy().into_owned()))
}

/// Folder of this account's settings (`~/.config/org.oblivio.app`).
fn config_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| error.to_string())
}

/// The customer's logo from the settings, if any.
fn custom_logo(app: &tauri::AppHandle) -> certificate::CustomLogo {
    config_dir(app).ok().and_then(|dir| settings::logo(&dir))
}

/// The operator settings.
#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> Result<settings::Settings, String> {
    Ok(settings::load(&config_dir(&app)?))
}

/// Saves the organization and the operator written into certificates.
#[tauri::command]
fn save_settings(
    organization: Option<String>,
    operator: Option<String>,
    app: tauri::AppHandle,
) -> Result<settings::Settings, String> {
    settings::save(&config_dir(&app)?, organization, operator)
}

/// Stores the customer's logo (PNG or SVG, base64).
#[tauri::command]
fn set_logo(data: String, app: tauri::AppHandle) -> Result<settings::Settings, String> {
    use base64::Engine;
    if data.len() > settings::MAX_LOGO_BYTES / 3 * 4 + 4 {
        return Err("logo: 1 MiB at most".to_owned());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|error| error.to_string())?;
    settings::set_logo(&config_dir(&app)?, &bytes)
}

/// Removes the customer's logo.
#[tauri::command]
fn clear_logo(app: tauri::AppHandle) -> Result<settings::Settings, String> {
    settings::clear_logo(&config_dir(&app)?)
}

/// Verifies a certificate given by the user (PDF, COSE or QR text, base64).
#[tauri::command]
async fn verify_certificate(data: String) -> Result<verification::Report, String> {
    use base64::Engine;
    // Base64 is 4/3 of the input: refuse oversized input before decoding it.
    if data.len() > verification::MAX_INPUT_BYTES / 3 * 4 + 4 {
        return Err("input too large".to_owned());
    }
    let input = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let trust = trust::Trust::shipped()?;
        Ok(verification::check(&input, &trust))
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Opens a certificate saved by `render_certificate`.
#[tauri::command]
fn open_certificate(path: String) -> Result<(), String> {
    certificate::open(std::path::Path::new(&path))
}

/// Labels of the attestation codes in `lang` (`en`, `fr` or `de`).
#[tauri::command]
fn code_labels(lang: String) -> HashMap<&'static str, &'static str> {
    let lang = Lang::from_tag(&lang).unwrap_or(Lang::En);
    KNOWN_CODES
        .iter()
        .filter_map(|id| code(id).map(|label| (*id, label.get(lang))))
        .collect()
}

/// Starts the application.
pub fn run() {
    let result = tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            let helper = Helper::spawn(move |event| {
                let _ = handle.emit("oblivio://event", event);
            });
            if let Err(reason) = &helper {
                eprintln!("oblivio-app: {reason}");
            }
            app.manage(HelperState(helper));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            helper,
            code_labels,
            render_certificate,
            open_certificate,
            list_reports,
            render_report,
            render_batch,
            verify_certificate,
            get_settings,
            save_settings,
            set_logo,
            clear_logo
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("oblivio-app: {error}");
        std::process::exit(1);
    }
}
