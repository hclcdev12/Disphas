//! disphas desktop application: the Tauri shell between the React interface
//! and the root helper. Runs as the normal user; renders, translates and
//! relays, but never touches a device and never decides.

mod certificate;
mod drives;
mod helper;
mod history;
mod notifications;
mod settings;
mod trust;
mod verification;

use std::collections::HashMap;

use disphas_attestation::display::{KNOWN_CODES, Lang, code};
use disphas_protocol::command::Command;
use disphas_protocol::response::{ErrorBody, ErrorCode};
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
/// `~/Documents/disphas`. Returns the path of the file.
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
    let command = Command::GetJobResult(disphas_protocol::command::JobArgs {
        job_id: disphas_protocol::command::JobId(job_id),
    });
    let value = helper.request(command).await?.outcome?;
    let result: disphas_protocol::response::JobResult =
        serde_json::from_value(value).map_err(|error| failure(error.to_string()))?;
    let lang = Lang::from_tag(&lang).unwrap_or(Lang::En);
    // Typst layout is CPU-bound: keep it off the async runtime.
    let logo = custom_logo(&app, helper).await;
    let path =
        tauri::async_runtime::spawn_blocking(move || certificate::save(&result, lang, &logo))
            .await
            .map_err(|error| failure(error.to_string()))?
            .map_err(failure)?;
    Ok(path.to_string_lossy().into_owned())
}

fn internal(detail: impl ToString) -> ErrorBody {
    ErrorBody {
        code: ErrorCode::Internal,
        detail: Some(detail.to_string()),
    }
}

fn helper_of(state: &HelperState) -> Result<&Helper, ErrorBody> {
    state.0.as_ref().map_err(internal)
}

/// Every stored report, newest first, page by page through the helper:
/// the reports are root-only (docs/protocole_ipc.md §10).
async fn fetch_reports(
    helper: &Helper,
) -> Result<Vec<disphas_protocol::response::StoredReport>, ErrorBody> {
    use disphas_protocol::command::ListReportsArgs;
    use disphas_protocol::response::ListReportsResult;
    let mut reports = Vec::new();
    let mut before = None;
    loop {
        let value = helper
            .request(Command::ListReports(ListReportsArgs { before }))
            .await?
            .outcome?;
        let page: ListReportsResult = serde_json::from_value(value).map_err(internal)?;
        let more = page.more && !page.reports.is_empty();
        before = page.reports.last().map(|report| report.id.clone());
        reports.extend(page.reports);
        if !more || reports.len() >= history::MAX_ENTRIES {
            return Ok(reports);
        }
    }
}

/// The signed attestation and journal of report `id`, through the helper.
async fn fetch_report(helper: &Helper, id: &str) -> Result<(Vec<u8>, Vec<u8>), ErrorBody> {
    use base64::Engine;
    use disphas_protocol::command::ReportArgs;
    use disphas_protocol::response::ReportResult;
    if !history::is_report_id(id) {
        return Err(internal("invalid report identifier"));
    }
    let value = helper
        .request(Command::GetReport(ReportArgs { id: id.to_owned() }))
        .await?
        .outcome?;
    let report: ReportResult = serde_json::from_value(value).map_err(internal)?;
    let signed = base64::engine::general_purpose::STANDARD
        .decode(&report.attestation)
        .map_err(internal)?;
    Ok((signed, report.journal.into_bytes()))
}

/// The certificate history, newest first, each report verified.
#[tauri::command]
async fn list_reports(state: State<'_, HelperState>) -> Result<Vec<history::Entry>, ErrorBody> {
    let reports = fetch_reports(helper_of(&state)?).await?;
    // Verifying thousands of signatures: off the async runtime.
    tauri::async_runtime::spawn_blocking(move || {
        let trust = trust::Trust::shipped()?;
        Ok::<_, String>(history::check_all(&reports, &trust))
    })
    .await
    .map_err(internal)?
    .map_err(internal)
}

/// Renders the PDF certificate of report `id` from the history, in `lang`.
#[tauri::command]
async fn render_report(
    id: String,
    lang: String,
    state: State<'_, HelperState>,
    app: tauri::AppHandle,
) -> Result<String, ErrorBody> {
    let helper = helper_of(&state)?;
    let (signed, journal) = fetch_report(helper, &id).await?;
    let lang = Lang::from_tag(&lang).unwrap_or(Lang::En);
    let logo = custom_logo(&app, helper).await;
    let path = tauri::async_runtime::spawn_blocking(move || {
        let trust = trust::Trust::shipped()?;
        certificate::save_signed(&id, &signed, &journal, lang, &trust, &logo)
    })
    .await
    .map_err(internal)?
    .map_err(internal)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Renders the batch certificate of `batch_id` from the history; `None` for
/// a batch without any certificate.
#[tauri::command]
async fn render_batch(
    batch_id: String,
    lang: String,
    state: State<'_, HelperState>,
    app: tauri::AppHandle,
) -> Result<Option<String>, ErrorBody> {
    let helper = helper_of(&state)?;
    let reports = fetch_reports(helper).await?;
    // Only verified reports of this batch go into its certificate.
    let wanted = batch_id.clone();
    let ids: Vec<String> = tauri::async_runtime::spawn_blocking(move || {
        let trust = trust::Trust::shipped()?;
        Ok::<_, String>(
            history::check_all(&reports, &trust)
                .into_iter()
                .filter(|entry| {
                    entry.check == history::Check::Valid
                        && entry
                            .summary
                            .as_ref()
                            .is_some_and(|summary| summary.batch_id.as_deref() == Some(&wanted))
                })
                .map(|entry| entry.id)
                .collect(),
        )
    })
    .await
    .map_err(internal)?
    .map_err(internal)?;
    let mut files = Vec::with_capacity(ids.len());
    for id in &ids {
        files.push(fetch_report(helper, id).await?);
    }
    let lang = Lang::from_tag(&lang).unwrap_or(Lang::En);
    let logo = custom_logo(&app, helper).await;
    let path = tauri::async_runtime::spawn_blocking(move || {
        let trust = trust::Trust::shipped()?;
        certificate::save_batch(&batch_id, &files, lang, &trust, &logo)
    })
    .await
    .map_err(internal)?
    .map_err(internal)?;
    Ok(path.map(|path| path.to_string_lossy().into_owned()))
}

/// Folder of this account's settings (`~/.config/org.disphas.app`).
fn config_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| error.to_string())
}

/// The customer's logo from the settings, only while the license allows it
/// (`docs/offres.md` §4.5). Any doubt prints the disphas wordmark instead;
/// a stored logo is kept, only ignored.
async fn custom_logo(app: &tauri::AppHandle, helper: &Helper) -> certificate::CustomLogo {
    if !logo_allowed(helper).await {
        return None;
    }
    config_dir(app).ok().and_then(|dir| settings::logo(&dir))
}

/// Whether the helper says the license allows the customer's logo.
async fn logo_allowed(helper: &Helper) -> bool {
    let Ok(response) = helper.request(Command::LicenseStatus).await else {
        return false;
    };
    response
        .outcome
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
        .is_some_and(|status| allows_logo(&status))
}

/// The helper only sends entitlements while the license is valid.
fn allows_logo(status: &disphas_protocol::response::LicenseStatusResult) -> bool {
    status
        .entitlements
        .as_ref()
        .is_some_and(|entitlements| entitlements.customer_logo)
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

/// Changes the operator only (quick switch in the header).
#[tauri::command]
fn set_operator(
    operator: Option<String>,
    app: tauri::AppHandle,
) -> Result<settings::Settings, String> {
    settings::set_operator(&config_dir(&app)?, operator)
}

/// Removes an operator from the quick switch.
#[tauri::command]
fn forget_operator(operator: String, app: tauri::AppHandle) -> Result<settings::Settings, String> {
    settings::forget_operator(&config_dir(&app)?, &operator)
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

/// Turns the strict confirmation mode on or off.
#[tauri::command]
fn set_strict_confirmation(
    enabled: bool,
    app: tauri::AppHandle,
) -> Result<settings::Settings, String> {
    settings::set_strict_confirmation(&config_dir(&app)?, enabled)
}

/// Records that the first-launch assistant was completed or skipped.
#[tauri::command]
fn set_onboarded(app: tauri::AppHandle) -> Result<settings::Settings, String> {
    settings::set_onboarded(&config_dir(&app)?)
}

/// Remembers the language of the interface.
#[tauri::command]
fn set_language(language: String, app: tauri::AppHandle) -> Result<settings::Settings, String> {
    settings::set_language(&config_dir(&app)?, &language)
}

/// Turns the desktop notifications on or off.
#[tauri::command]
fn set_notifications(enabled: bool, app: tauri::AppHandle) -> Result<settings::Settings, String> {
    settings::set_notifications(&config_dir(&app)?, enabled)
}

/// The last desktop notification, kept while the app runs: GNOME withdraws a
/// notification as soon as the D-Bus connection that sent it closes
/// (gnome-shell `notificationDaemon.js`, `_onNameVanished`), and the handle
/// holds that connection. A new notification replaces the previous one.
#[derive(Default)]
struct LastNotification(std::sync::Mutex<Option<notifications::Handle>>);

/// Shows a desktop notification when the window is in the background: in
/// front, the app shows its own summary. The interface calls it only when
/// the operator turned notifications on (off by default). Goes through the
/// desktop's notification service (D-Bus on Linux), never the network.
#[tauri::command]
async fn notify(
    title: String,
    body: String,
    app: tauri::AppHandle,
    last: State<'_, LastNotification>,
) -> Result<(), String> {
    let in_front = app
        .get_webview_window("main")
        .and_then(|window| window.is_focused().ok())
        .unwrap_or(false);
    if in_front {
        return Ok(());
    }
    let handle = notifications::show(&title, &body).await?;
    if let Ok(mut slot) = last.0.lock() {
        *slot = Some(handle);
    }
    Ok(())
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
        // First plugin, as its documentation requires. A second launch brings
        // the open window forward instead of starting a second helper: one
        // window, one helper, one parallel limit (docs/offres.md §4.4).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let handle = app.handle().clone();
            let helper = Helper::spawn(move |event| {
                let _ = handle.emit("disphas://event", event);
            });
            if let Err(reason) = &helper {
                eprintln!("disphas-app: {reason}");
            }
            app.manage(HelperState(helper));
            app.manage(LastNotification::default());
            let handle = app.handle().clone();
            drives::watch(move || {
                let _ = handle.emit("disphas://drives-changed", ());
            });
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
            clear_logo,
            set_strict_confirmation,
            set_notifications,
            set_language,
            set_onboarded,
            set_operator,
            forget_operator,
            notify
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("disphas-app: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use disphas_protocol::response::{Entitlements, LicenseState, LicenseStatusResult};

    use super::allows_logo;

    fn status(entitlements: Option<Entitlements>) -> LicenseStatusResult {
        LicenseStatusResult {
            state: LicenseState::Valid,
            offer: Some("Atelier8".to_owned()),
            license_id: Some("LIC-0001".to_owned()),
            valid_until: None,
            entitlements,
        }
    }

    #[test]
    fn the_logo_needs_the_right_of_the_offer() {
        let with = |customer_logo| {
            Some(Entitlements {
                max_parallel: Some(2),
                customer_logo,
            })
        };
        assert!(allows_logo(&status(with(true))));
        assert!(!allows_logo(&status(with(false))));
        // No entitlements: no valid license, the disphas wordmark is printed.
        assert!(!allows_logo(&status(None)));
    }
}
