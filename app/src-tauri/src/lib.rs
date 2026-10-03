//! Oblivio desktop application: the Tauri shell between the React interface
//! and the root helper. Runs as the normal user; renders, translates and
//! relays, but never touches a device and never decides.

mod helper;

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
        .invoke_handler(tauri::generate_handler![helper, code_labels])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("oblivio-app: {error}");
        std::process::exit(1);
    }
}
