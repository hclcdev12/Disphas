//! NDJSON envelope: one JSON message per line, 1 MiB at most (§4).
//!
//! Decoding is two-step. The envelope (`type`, `id`, `cmd`…) is read strictly
//! first, so a rejected request can still be answered with its `id`; the
//! arguments are then decoded for the named command, strictly as well.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::command::Command;
use crate::event::Event;
use crate::response::{ErrorBody, ErrorCode};

/// Largest message on the wire, trailing newline included.
pub const MAX_MESSAGE_BYTES: usize = 1 << 20;

/// A request from the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Identifier echoed by the response.
    pub id: u64,
    /// The command.
    pub command: Command,
}

/// A response to exactly one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// Identifier of the request.
    pub id: u64,
    /// Command-specific result, or error.
    pub outcome: Result<Value, ErrorBody>,
}

/// Anything the helper writes on stdout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerMessage {
    /// A response.
    Response(Response),
    /// A spontaneous event.
    Event(Event),
}

/// A message could not be encoded.
#[derive(Debug, Error)]
pub enum EncodeError {
    /// Serialization failed.
    #[error("serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    /// The encoded message exceeds [`MAX_MESSAGE_BYTES`].
    #[error("message of {0} bytes exceeds the limit")]
    TooLarge(usize),
}

/// A line could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{kind}")]
pub struct DecodeError {
    /// `id` of the offending request, when it could be read.
    pub request_id: Option<u64>,
    /// What went wrong.
    pub kind: DecodeErrorKind,
}

/// What went wrong while decoding.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeErrorKind {
    /// The line exceeds [`MAX_MESSAGE_BYTES`]; the connection must be closed.
    #[error("message of {0} bytes exceeds the limit")]
    TooLarge(usize),
    /// Malformed JSON or envelope.
    #[error("malformed envelope: {0}")]
    Envelope(String),
    /// Command outside the closed list.
    #[error("unknown command {0:?}")]
    UnknownCommand(String),
    /// Invalid arguments for a known command.
    #[error("invalid arguments for {command}: {reason}")]
    Arguments {
        /// The command.
        command: String,
        /// Why the arguments were rejected.
        reason: String,
    },
    /// Invalid result or event payload.
    #[error("invalid payload: {0}")]
    Payload(String),
}

impl DecodeError {
    fn new(request_id: Option<u64>, kind: DecodeErrorKind) -> Self {
        Self { request_id, kind }
    }

    /// Error code to answer with.
    pub fn error_code(&self) -> ErrorCode {
        match self.kind {
            DecodeErrorKind::UnknownCommand(_) => ErrorCode::UnknownCommand,
            _ => ErrorCode::BadRequest,
        }
    }

    /// Whether the helper must close the connection rather than answer (§4).
    pub fn closes_connection(&self) -> bool {
        matches!(self.kind, DecodeErrorKind::TooLarge(_))
    }
}

impl Request {
    /// Encodes the request as one line, without the trailing newline.
    pub fn encode(&self) -> Result<String, EncodeError> {
        #[derive(Serialize)]
        struct Out<'a> {
            #[serde(rename = "type")]
            kind: &'static str,
            id: u64,
            #[serde(flatten)]
            command: &'a Command,
        }
        finish(&Out {
            kind: "request",
            id: self.id,
            command: &self.command,
        })
    }

    /// Decodes one line, with or without its trailing newline.
    ///
    /// For `Unlock`, the caller must wipe `line` after use: it holds the password.
    /// The arguments are never parsed into an intermediate JSON tree, which
    /// would drop a copy of the password unwiped: they are kept as a slice of
    /// `line`, then decoded once from a wiped buffer.
    pub fn decode(line: &str) -> Result<Self, DecodeError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw<'a> {
            #[serde(rename = "type")]
            _kind: RequestTag,
            id: u64,
            cmd: String,
            #[serde(borrow)]
            args: Option<&'a serde_json::value::RawValue>,
        }
        #[derive(Deserialize)]
        enum RequestTag {
            #[serde(rename = "request")]
            Request,
        }

        let line = check_size(line)?;
        let raw: Raw = serde_json::from_str(line).map_err(|error| {
            DecodeError::new(probe_id(line), DecodeErrorKind::Envelope(error.to_string()))
        })?;
        let error = |kind| DecodeError::new(Some(raw.id), kind);
        if !Command::NAMES.contains(&raw.cmd.as_str()) {
            return Err(error(DecodeErrorKind::UnknownCommand(raw.cmd)));
        }
        // The command name is one of `Command::NAMES`: plain ASCII, no escaping.
        let tagged = zeroize::Zeroizing::new(match raw.args {
            Some(args) => format!(r#"{{"cmd":"{}","args":{}}}"#, raw.cmd, args.get()),
            None => format!(r#"{{"cmd":"{}"}}"#, raw.cmd),
        });
        let command = serde_json::from_str(&tagged).map_err(|reason| {
            error(DecodeErrorKind::Arguments {
                command: raw.cmd.clone(),
                reason: reason.to_string(),
            })
        })?;
        Ok(Self {
            id: raw.id,
            command,
        })
    }
}

impl Response {
    /// A successful response carrying `result`.
    pub fn success<T: Serialize>(id: u64, result: &T) -> Result<Self, EncodeError> {
        Ok(Self {
            id,
            outcome: Ok(serde_json::to_value(result)?),
        })
    }

    /// A failed response.
    pub fn failure(id: u64, code: ErrorCode, detail: Option<String>) -> Self {
        Self {
            id,
            outcome: Err(ErrorBody { code, detail }),
        }
    }

    /// Decodes the result as the type expected for the request's command.
    pub fn into_result<T: DeserializeOwned>(self) -> Result<Result<T, ErrorBody>, DecodeError> {
        match self.outcome {
            Ok(value) => serde_json::from_value(value).map(Ok).map_err(|reason| {
                DecodeError::new(Some(self.id), DecodeErrorKind::Payload(reason.to_string()))
            }),
            Err(error) => Ok(Err(error)),
        }
    }
}

impl ServerMessage {
    /// Encodes the message as one line, without the trailing newline.
    pub fn encode(&self) -> Result<String, EncodeError> {
        #[derive(Serialize)]
        struct OutResponse<'a> {
            #[serde(rename = "type")]
            kind: &'static str,
            id: u64,
            ok: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            result: Option<&'a Value>,
            #[serde(skip_serializing_if = "Option::is_none")]
            error: Option<&'a ErrorBody>,
        }
        #[derive(Serialize)]
        struct OutEvent<'a> {
            #[serde(rename = "type")]
            kind: &'static str,
            #[serde(flatten)]
            event: &'a Event,
        }

        match self {
            Self::Response(response) => finish(&OutResponse {
                kind: "response",
                id: response.id,
                ok: response.outcome.is_ok(),
                result: response.outcome.as_ref().ok(),
                error: response.outcome.as_ref().err(),
            }),
            Self::Event(event) => finish(&OutEvent {
                kind: "event",
                event,
            }),
        }
    }

    /// Decodes one line, with or without its trailing newline.
    pub fn decode(line: &str) -> Result<Self, DecodeError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            #[serde(rename = "type")]
            kind: ServerTag,
            id: Option<u64>,
            ok: Option<bool>,
            result: Option<Value>,
            error: Option<ErrorBody>,
            event: Option<String>,
            data: Option<Value>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum ServerTag {
            Response,
            Event,
        }

        let line = check_size(line)?;
        let envelope =
            |reason: &str| DecodeError::new(None, DecodeErrorKind::Envelope(reason.to_owned()));
        let raw: Raw = serde_json::from_str(line).map_err(|error| envelope(&error.to_string()))?;
        match raw.kind {
            ServerTag::Response => {
                if raw.event.is_some() || raw.data.is_some() {
                    return Err(envelope("event fields in a response"));
                }
                let id = raw.id.ok_or_else(|| envelope("response without id"))?;
                let outcome = match (raw.ok, raw.result, raw.error) {
                    (Some(true), Some(result), None) => Ok(result),
                    (Some(false), None, Some(error)) => Err(error),
                    _ => return Err(envelope("ok must match exactly one of result or error")),
                };
                Ok(Self::Response(Response { id, outcome }))
            }
            ServerTag::Event => {
                if raw.id.is_some()
                    || raw.ok.is_some()
                    || raw.result.is_some()
                    || raw.error.is_some()
                {
                    return Err(envelope("response fields in an event"));
                }
                let name = raw.event.ok_or_else(|| envelope("event without name"))?;
                let mut tagged = Map::new();
                tagged.insert("event".to_owned(), Value::String(name));
                if let Some(data) = raw.data {
                    tagged.insert("data".to_owned(), data);
                }
                serde_json::from_value(Value::Object(tagged))
                    .map(Self::Event)
                    .map_err(|reason| {
                        DecodeError::new(None, DecodeErrorKind::Payload(reason.to_string()))
                    })
            }
        }
    }
}

fn finish<T: Serialize>(message: &T) -> Result<String, EncodeError> {
    let line = serde_json::to_string(message)?;
    if line.len() + 1 > MAX_MESSAGE_BYTES {
        return Err(EncodeError::TooLarge(line.len() + 1));
    }
    Ok(line)
}

fn check_size(line: &str) -> Result<&str, DecodeError> {
    let stripped = line.strip_suffix('\n').unwrap_or(line);
    if stripped.len() + 1 > MAX_MESSAGE_BYTES {
        return Err(DecodeError::new(
            None,
            DecodeErrorKind::TooLarge(stripped.len() + 1),
        ));
    }
    Ok(stripped)
}

/// Best-effort read of `id` from a rejected request, to answer it.
fn probe_id(line: &str) -> Option<u64> {
    #[derive(Deserialize)]
    struct Probe {
        id: Option<u64>,
    }
    serde_json::from_str::<Probe>(line).ok()?.id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codes::{
        Acknowledgement, Category, DispositionCode, Method, RefusalCode, ReserveCode, ScopeWarning,
    };
    use crate::command::{
        DeviceArgs, DeviceIdentity, DeviceRef, ForceArgs, HelloArgs, InstallLicenseArgs, JobArgs,
        JobId, SanitizeArgs, UnlockArgs, VerdictId,
    };
    use crate::event::{JobPhaseData, JobProgressData, ProgressSource};
    use crate::response::{InspectResult, JobPhase};
    use crate::secret::AtaPassword;
    use crate::verdict::{Disposition, MethodChoice, Verdict};

    fn device() -> DeviceRef {
        DeviceRef {
            node_hint: Some("/dev/nvme0n1".to_owned()),
            identity: DeviceIdentity {
                model: "Samsung SSD 980 PRO 1TB".to_owned(),
                serial: "S5GXNX0T1234567".to_owned(),
                wwn: None,
                eui64: Some("002538b111b22c33".to_owned()),
                nguid: None,
            },
        }
    }

    fn every_command() -> Vec<Command> {
        let job = JobArgs {
            job_id: JobId("job-1".to_owned()),
        };
        vec![
            Command::Hello(HelloArgs {
                protocol_version: crate::PROTOCOL_VERSION,
                app_version: "0.1.0".to_owned(),
            }),
            Command::ListDevices,
            Command::Inspect(DeviceArgs { device: device() }),
            Command::Unlock(UnlockArgs {
                device: device(),
                kind: crate::command::PasswordKind::Master,
                password: AtaPassword::new("hunter2".to_owned()).expect("short password"),
            }),
            Command::Sanitize(SanitizeArgs {
                verdict_id: VerdictId("b2f1".to_owned()),
                device: device(),
                method: Some(Method::NvmeSanitizeBlockErase),
                confirmation: "4567".to_owned(),
                acknowledged: vec![
                    Acknowledgement::Disposition(DispositionCode::ReallocatedAboveThreshold),
                    Acknowledgement::Scope(ScopeWarning::AllNamespaces),
                ],
                force: Some(ForceArgs {
                    justification: "Inspection impossible, écriture forcée".to_owned(),
                }),
                operator: Some("J. Martin".to_owned()),
                organization: None,
                batch: Some(crate::command::BatchArgs {
                    id: "LOT-2026-001".to_owned(),
                    label: Some("PO 4512".to_owned()),
                }),
                zero_pass: false,
            }),
            Command::Cancel(job.clone()),
            Command::ListJobs,
            Command::GetJobResult(job),
            Command::LicenseStatus,
            Command::CreateActivationRequest,
            Command::InstallLicense(InstallLicenseArgs {
                license: "DSL1:…".to_owned(),
            }),
            Command::DeleteReports(crate::command::DeleteReportsArgs {
                ids: vec!["01928c3e-5a1b-7000-8000-000000004201".to_owned()],
            }),
            Command::ListReports(crate::command::ListReportsArgs {
                before: Some("01928c3e-5a1b-7000-8000-000000004201".to_owned()),
            }),
            Command::GetReport(crate::command::ReportArgs {
                id: "01928c3e-5a1b-7000-8000-000000004201".to_owned(),
            }),
            Command::Shutdown,
        ]
    }

    fn decode_error(line: &str) -> DecodeError {
        Request::decode(line).expect_err("line must be rejected")
    }

    // --- Requests.

    #[test]
    fn every_command_round_trips_and_is_listed() {
        let commands = every_command();
        assert_eq!(commands.len(), Command::NAMES.len());
        for (id, command) in (1..).zip(commands) {
            let request = Request { id, command };
            let line = request.encode().expect("encodable");
            assert!(!line.contains('\n'));
            let value: Value = serde_json::from_str(&line).expect("valid JSON");
            assert_eq!(value["type"], "request");
            assert!(Command::NAMES.contains(&value["cmd"].as_str().expect("cmd")));
            assert_eq!(Request::decode(&line), Ok(request));
        }
    }

    #[test]
    fn decodes_the_spec_examples() {
        let inspect = r#"{"type":"request","id":42,"cmd":"Inspect","args":{"device":{"node_hint":"/dev/nvme0n1","identity":{"model":"Samsung SSD 980 PRO 1TB","serial":"S5GXNX0T1234567","wwn":null,"eui64":"002538b111b22c33","nguid":null}}}}"#;
        assert_eq!(
            Request::decode(inspect),
            Ok(Request {
                id: 42,
                command: Command::Inspect(DeviceArgs { device: device() })
            })
        );

        let sanitize = r#"{"type":"request","id":7,"cmd":"Sanitize","args":{
            "verdict_id":"b2f1","device":{"node_hint":"/dev/nvme0n1","identity":{"model":"Samsung SSD 980 PRO 1TB","serial":"S5GXNX0T1234567","wwn":null,"eui64":"002538b111b22c33","nguid":null}},
            "method":null,"confirmation":"4567","acknowledged":["D4","R4","NS_ALL"],"force":null,
            "operator":"J. Martin","organization":"Atelier XYZ"}}"#;
        let Request {
            command: Command::Sanitize(args),
            ..
        } = Request::decode(sanitize).expect("valid Sanitize")
        else {
            panic!("expected Sanitize");
        };
        assert_eq!(args.method, None);
        assert_eq!(
            args.acknowledged,
            [
                Acknowledgement::Disposition(DispositionCode::ReallocatedAboveThreshold),
                Acknowledgement::Reserve(ReserveCode::ReallocatedSectors),
                Acknowledgement::Scope(ScopeWarning::AllNamespaces),
            ]
        );
    }

    #[test]
    fn unknown_envelope_field_is_rejected_with_the_request_id() {
        let error = decode_error(r#"{"type":"request","id":5,"cmd":"ListDevices","sudo":true}"#);
        assert_eq!(error.request_id, Some(5));
        assert_eq!(error.error_code(), ErrorCode::BadRequest);
    }

    #[test]
    fn unknown_argument_field_is_rejected() {
        let error = decode_error(
            r#"{"type":"request","id":6,"cmd":"Cancel","args":{"job_id":"j","force":true}}"#,
        );
        assert_eq!(error.request_id, Some(6));
        assert!(matches!(error.kind, DecodeErrorKind::Arguments { .. }));
    }

    #[test]
    fn unknown_command_is_reported_as_such() {
        let error = decode_error(r#"{"type":"request","id":8,"cmd":"Sign","args":{}}"#);
        assert_eq!(error.request_id, Some(8));
        assert_eq!(error.error_code(), ErrorCode::UnknownCommand);
    }

    #[test]
    fn arguments_on_a_command_without_arguments_are_rejected() {
        let error =
            decode_error(r#"{"type":"request","id":9,"cmd":"Shutdown","args":{"now":true}}"#);
        assert_eq!(error.error_code(), ErrorCode::BadRequest);
    }

    #[test]
    fn wrong_message_type_is_rejected() {
        let error = decode_error(r#"{"type":"response","id":1,"cmd":"ListDevices"}"#);
        assert_eq!(error.request_id, Some(1));
        assert_eq!(error.error_code(), ErrorCode::BadRequest);
    }

    #[test]
    fn missing_arguments_are_rejected() {
        let error = decode_error(r#"{"type":"request","id":2,"cmd":"Inspect"}"#);
        assert!(matches!(error.kind, DecodeErrorKind::Arguments { .. }));
    }

    #[test]
    fn unknown_acknowledgement_code_is_rejected() {
        let line = Request {
            id: 3,
            command: every_command().swap_remove(4),
        }
        .encode()
        .expect("encodable")
        .replace("\"NS_ALL\"", "\"R99\"");
        assert!(matches!(
            decode_error(&line).kind,
            DecodeErrorKind::Arguments { .. }
        ));
    }

    #[test]
    fn oversized_line_closes_the_connection() {
        let line = format!(
            r#"{{"type":"request","id":1,"cmd":"InstallLicense","args":{{"license":"{}"}}}}"#,
            "x".repeat(MAX_MESSAGE_BYTES)
        );
        let error = decode_error(&line);
        assert!(error.closes_connection());
    }

    #[test]
    fn oversized_password_is_rejected_without_echoing_it() {
        let secret = "s".repeat(40);
        let line = format!(
            r#"{{"type":"request","id":4,"cmd":"Unlock","args":{{"device":{},"password":"{secret}"}}}}"#,
            serde_json::to_string(&device()).expect("device")
        );
        let error = decode_error(&line);
        assert!(matches!(error.kind, DecodeErrorKind::Arguments { .. }));
        assert!(!error.to_string().contains(&secret));
    }

    // --- Responses and events.

    #[test]
    fn responses_round_trip() {
        let result = crate::response::SanitizeResult {
            job_id: JobId("job-1".to_owned()),
        };
        for response in [
            Response::success(7, &result).expect("encodable"),
            Response::failure(
                7,
                ErrorCode::DeviceChanged,
                Some("serial differs".to_owned()),
            ),
        ] {
            let message = ServerMessage::Response(response);
            let line = message.encode().expect("encodable");
            assert_eq!(ServerMessage::decode(&line), Ok(message));
        }
        let line = ServerMessage::Response(Response::failure(1, ErrorCode::DeviceChanged, None))
            .encode()
            .expect("encodable");
        assert!(line.contains(r#""code":"DEVICE_CHANGED""#));
    }

    #[test]
    fn typed_result_is_decoded_for_the_expected_command() {
        let verdict = Verdict::SayNo {
            reasons: vec![RefusalCode::InspectionFailed],
            forcible: Some(MethodChoice {
                method: Method::HostOverwrite,
                category: Category::Clear,
                reserves: vec![ReserveCode::HostWriteCoverage],
                all_namespaces: false,
            }),
        };
        let inspect = InspectResult {
            verdict_id: VerdictId("v1".to_owned()),
            valid_for_seconds: 600,
            device: device(),
            kind: Some(crate::response::DriveKind::Nvme),
            capacity_bytes: Some(1_000_204_886_016),
            firmware: "5B2QGXA7".to_owned(),
            verdict,
        };
        let line = ServerMessage::Response(Response::success(3, &inspect).expect("encodable"))
            .encode()
            .expect("encodable");
        assert!(line.contains(r#""state":"SAY_NO""#));
        let ServerMessage::Response(response) = ServerMessage::decode(&line).expect("decodable")
        else {
            panic!("expected a response");
        };
        assert_eq!(response.into_result::<InspectResult>(), Ok(Ok(inspect)));
    }

    #[test]
    fn inconsistent_response_is_rejected() {
        for line in [
            r#"{"type":"response","id":1,"ok":true}"#,
            r#"{"type":"response","id":1,"ok":false,"result":{}}"#,
            r#"{"type":"response","id":1,"ok":true,"result":{},"error":{"code":"BUSY","detail":null}}"#,
            r#"{"type":"response","ok":true,"result":{}}"#,
            r#"{"type":"response","id":1,"ok":true,"result":{},"event":"JobPhase"}"#,
        ] {
            assert!(ServerMessage::decode(line).is_err(), "{line}");
        }
    }

    #[test]
    fn events_round_trip() {
        let job_id = JobId("job-1".to_owned());
        for event in [
            Event::JobPhase(JobPhaseData {
                job_id: job_id.clone(),
                phase: JobPhase::Canaries,
            }),
            Event::JobProgress(JobProgressData {
                job_id,
                percent: 37,
                source: ProgressSource::Host,
                bytes_done: Some(1 << 30),
                bytes_per_second: Some(500 << 20),
                remaining_seconds: Some(120),
            }),
        ] {
            let message = ServerMessage::Event(event);
            let line = message.encode().expect("encodable");
            assert!(line.starts_with(r#"{"type":"event","event":"#));
            assert_eq!(ServerMessage::decode(&line), Ok(message));
        }
    }

    #[test]
    fn unknown_event_or_field_is_rejected() {
        for line in [
            r#"{"type":"event","event":"Reboot","data":{}}"#,
            r#"{"type":"event","event":"JobPhase","data":{"job_id":"j","phase":"Done","extra":1}}"#,
            r#"{"type":"event","id":1,"event":"JobPhase","data":{"job_id":"j","phase":"Done"}}"#,
        ] {
            assert!(ServerMessage::decode(line).is_err(), "{line}");
        }
    }

    // --- Verdict helpers.

    #[test]
    fn verdict_and_disposition_reject_unknown_fields() {
        for json in [
            r#"{"state":"PROTECTED","reasons":["P2"],"override":true}"#,
            r#"{"state":"READY","recommended":{"method":"HostOverwrite","category":"Clear","reserves":[],"all_namespaces":false},"alternatives":[],"disposition":{"kind":"Reuse","note":"x"}}"#,
            r#"{"state":"READY","recommended":{"method":"HostOverwrite","category":"Purge2","reserves":[],"all_namespaces":false},"alternatives":[],"disposition":{"kind":"Reuse"}}"#,
        ] {
            assert!(serde_json::from_str::<Verdict>(json).is_err(), "{json}");
        }
        let valid = r#"{"state":"PROTECTED","reasons":["P2"]}"#;
        assert!(serde_json::from_str::<Verdict>(valid).is_ok());
    }

    #[test]
    fn required_acknowledgements_cover_dispositions_reserves_and_scope() {
        let choice = MethodChoice {
            method: Method::NvmeSanitizeBlockErase,
            category: Category::Purge,
            reserves: vec![ReserveCode::MediaErrors],
            all_namespaces: true,
        };
        let disposition = Disposition::DestroyRecommended {
            reasons: vec![DispositionCode::ReliabilityDegraded],
        };
        assert_eq!(
            choice.required_acknowledgements(&disposition),
            [
                Acknowledgement::Disposition(DispositionCode::ReliabilityDegraded),
                Acknowledgement::Reserve(ReserveCode::MediaErrors),
                Acknowledgement::Scope(ScopeWarning::AllNamespaces),
            ]
        );
    }
}
