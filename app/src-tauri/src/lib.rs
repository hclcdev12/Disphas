//! Oblivio desktop application: the Tauri shell between the React interface
//! and the root helper. Runs as the normal user; renders, translates and
//! relays, but never touches a device and never decides.

mod certificate;
mod helper;
mod history;

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
    let path = tauri::async_runtime::spawn_blocking(move || certificate::save(&result, lang))
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
        history::list(&history::reports_dir(), &oblivio_license::trusted_roots())
    })
    .await
    .map_err(|error| error.to_string())
}

/// Renders the PDF certificate of report `id` from the history, in `lang`.
#[tauri::command]
async fn render_report(id: String, lang: String) -> Result<String, ErrorBody> {
    let failure = |detail: String| ErrorBody {
        code: ErrorCode::Internal,
        detail: Some(detail),
    };
    let lang = Lang::from_tag(&lang).unwrap_or(Lang::En);
    let path = tauri::async_runtime::spawn_blocking(move || {
        let (signed, journal) =
            history::read(&history::reports_dir(), &id).ok_or("report not found")?;
        certificate::save_signed(&id, &signed, &journal, lang)
    })
    .await
    .map_err(|error| failure(error.to_string()))?
    .map_err(failure)?;
    Ok(path.to_string_lossy().into_owned())
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
            render_report
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("oblivio-app: {error}");
        std::process::exit(1);
    }
}
