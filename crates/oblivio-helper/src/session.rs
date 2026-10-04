//! One helper session: from `Hello` to `Shutdown` (`docs/protocole_ipc.md`).
//!
//! Pure request handling: no stdin/stdout here, and drives are reached only
//! through a [`DeviceBackend`]. The clock is passed in, so expiry is testable.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use oblivio_core::RuleSet;
use oblivio_core::verdict::Verdict;
use oblivio_protocol::command::{Command, DeviceIdentity, HelloArgs, SanitizeArgs, VerdictId};
use oblivio_protocol::response::{
    ActivationRequestResult, DeviceSummary, Empty, ErrorBody, ErrorCode, HelloResult,
    InspectResult, LicenseState, ListDevicesResult, ListJobsResult,
};
use oblivio_protocol::verdict as wire_verdict;
use oblivio_protocol::{PROTOCOL_VERSION, Request, Response};
use serde::Serialize;
use serde_json::Value;

use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

use oblivio_protocol::ServerMessage;
use oblivio_protocol::command::JobId;
use oblivio_protocol::response::{JobResult, JobSummary, SanitizeResult};

use crate::backend::DeviceBackend;
use crate::convert;
use crate::job::{self, JobRecord, JobSpec, JobTable, Plan};
use crate::licensing::Licensing;

/// How long a verdict can be quoted by `Sanitize` (§6).
pub const VERDICT_TTL: Duration = Duration::from_secs(600);

/// Fixed facts about this helper.
#[derive(Debug)]
pub struct SessionConfig {
    /// Rule set of the decision engine.
    pub rules: RuleSet,
    /// SHA-256 of the helper binary.
    pub binary_sha256: [u8; 32],
    /// Where signed reports are stored (`reports/` of the state directory).
    pub reports_dir: std::path::PathBuf,
    /// License status, activation and installation.
    pub licensing: Box<dyn Licensing + Send>,
}

/// A verdict issued by `Inspect`, kept until it expires.
#[derive(Debug, Clone)]
struct IssuedVerdict {
    identity: DeviceIdentity,
    verdict: Verdict,
    issued_at: Instant,
}

/// A rejected request.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Failure {
    code: ErrorCode,
    detail: Option<String>,
}

impl From<ErrorBody> for Failure {
    fn from(error: ErrorBody) -> Self {
        Self {
            code: error.code,
            detail: error.detail,
        }
    }
}

impl Failure {
    fn new(code: ErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: Some(detail.into()),
        }
    }
}

type Handled = Result<Value, Failure>;

/// State of one session.
#[derive(Debug)]
pub struct Session<B> {
    backend: B,
    config: SessionConfig,
    greeted: bool,
    verdicts: HashMap<VerdictId, IssuedVerdict>,
    issued: u64,
    jobs: JobTable,
    threads: Vec<JoinHandle<()>>,
    events: Option<Sender<ServerMessage>>,
    shut_down: bool,
}

impl<B: DeviceBackend> Session<B> {
    /// A new session, waiting for `Hello`.
    pub fn new(backend: B, config: SessionConfig) -> Self {
        Self {
            backend,
            config,
            greeted: false,
            verdicts: HashMap::new(),
            issued: 0,
            jobs: JobTable::default(),
            threads: Vec::new(),
            events: None,
            shut_down: false,
        }
    }

    /// Where job events are sent, or `None` to drop them.
    pub fn set_events(&mut self, events: Option<Sender<ServerMessage>>) {
        self.events = events;
    }

    /// Waits until every job ended: a job always runs to its end (§3).
    pub fn wait_for_jobs(&mut self) {
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }

    fn running_job(&self) -> bool {
        self.jobs
            .lock()
            .map_or(true, |jobs| jobs.values().any(JobRecord::running))
    }

    /// The drive backend, for tests that change drives between requests.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Whether `Shutdown` was accepted.
    pub fn is_shut_down(&self) -> bool {
        self.shut_down
    }

    /// Handles one request at time `now` and returns its response.
    pub fn handle(&mut self, request: Request, now: Instant) -> Response {
        let id = request.id;
        match self.dispatch(request.command, now) {
            Ok(result) => Response {
                id,
                outcome: Ok(result),
            },
            Err(failure) => Response::failure(id, failure.code, failure.detail),
        }
    }

    fn dispatch(&mut self, command: Command, now: Instant) -> Handled {
        if let Command::Hello(args) = &command {
            return self.hello(args);
        }
        if !self.greeted {
            return Err(Failure::new(ErrorCode::BadRequest, "Hello expected first"));
        }
        match command {
            Command::Hello(_) => Err(Failure::new(ErrorCode::Internal, "unreachable")),
            Command::ListDevices => self.list_devices(),
            Command::Inspect(args) => self.inspect(&args.device.identity, now),
            Command::Sanitize(args) => self.sanitize(&args, now),
            Command::ListJobs => self.list_jobs(),
            Command::GetJobResult(args) => self.job_result(&args.job_id),
            Command::Cancel(args) => {
                let known = self
                    .jobs
                    .lock()
                    .is_ok_and(|jobs| jobs.contains_key(&args.job_id));
                if known {
                    // V1: an erasure always runs to its end once started.
                    Err(Failure::new(
                        ErrorCode::NotCancellable,
                        "erasures cannot be cancelled",
                    ))
                } else {
                    Err(Failure::new(ErrorCode::BadRequest, "unknown job"))
                }
            }
            Command::LicenseStatus => to_value(&self.config.licensing.status()),
            Command::Unlock(_) => Err(Failure::new(
                ErrorCode::UnlockFailed,
                "unlock not implemented yet",
            )),
            Command::CreateActivationRequest => {
                let request = self
                    .config
                    .licensing
                    .activation_request()
                    .map_err(Failure::from)?;
                to_value(&ActivationRequestResult { request })
            }
            Command::InstallLicense(args) => {
                if self.running_job() {
                    return Err(Failure::new(ErrorCode::Busy, "a job is running"));
                }
                to_value(
                    &self
                        .config
                        .licensing
                        .install(&args.license)
                        .map_err(Failure::from)?,
                )
            }
            Command::Shutdown => {
                if self.running_job() {
                    return Err(Failure::new(ErrorCode::Busy, "a job is running"));
                }
                self.shut_down = true;
                to_value(&Empty {})
            }
        }
    }

    fn hello(&mut self, args: &HelloArgs) -> Handled {
        if args.protocol_version != PROTOCOL_VERSION {
            return Err(Failure::new(
                ErrorCode::ProtocolMismatch,
                format!(
                    "app speaks {}, helper speaks {PROTOCOL_VERSION}",
                    args.protocol_version
                ),
            ));
        }
        self.greeted = true;
        to_value(&HelloResult {
            protocol_version: PROTOCOL_VERSION,
            helper_version: env!("CARGO_PKG_VERSION").to_owned(),
            rules_version: self.config.rules.version.to_string(),
            binary_sha256: self
                .config
                .binary_sha256
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            license: self.config.licensing.status(),
        })
    }

    fn list_devices(&mut self) -> Handled {
        let devices = self
            .backend
            .list()
            .into_iter()
            .map(|device| {
                let protection = match oblivio_core::decide(&device.profile, &self.config.rules) {
                    Verdict::Protected { reasons } => {
                        reasons.into_iter().map(convert::protect_code).collect()
                    }
                    _ => Vec::new(),
                };
                DeviceSummary {
                    device: oblivio_protocol::command::DeviceRef {
                        node_hint: device.node_hint,
                        identity: convert::identity(&device.profile.identity),
                    },
                    kind: convert::drive_kind(&device.profile.inspection),
                    capacity_bytes: device.capacity_bytes,
                    protection,
                }
            })
            .collect();
        to_value(&ListDevicesResult { devices })
    }

    fn inspect(&mut self, identity: &DeviceIdentity, now: Instant) -> Handled {
        self.verdicts
            .retain(|_, issued| now.saturating_duration_since(issued.issued_at) < VERDICT_TTL);
        let device = self
            .backend
            .find(identity)
            .filter(|device| convert::identity(&device.profile.identity) == *identity)
            .ok_or_else(|| {
                Failure::new(ErrorCode::DeviceNotFound, "no drive with this identity")
            })?;
        let verdict = oblivio_core::decide(&device.profile, &self.config.rules);

        self.issued += 1;
        let verdict_id = VerdictId(format!("v{}", self.issued));
        self.verdicts.insert(
            verdict_id.clone(),
            IssuedVerdict {
                identity: identity.clone(),
                verdict: verdict.clone(),
                issued_at: now,
            },
        );
        to_value(&InspectResult {
            verdict_id,
            valid_for_seconds: u32::try_from(VERDICT_TTL.as_secs()).unwrap_or(u32::MAX),
            device: oblivio_protocol::command::DeviceRef {
                node_hint: device.node_hint,
                identity: identity.clone(),
            },
            kind: convert::drive_kind(&device.profile.inspection),
            capacity_bytes: device.capacity_bytes,
            firmware: device.profile.identity.firmware.clone(),
            verdict: convert::verdict(&verdict),
        })
    }

    /// Checks of `docs/protocole_ipc.md` §7, in order; the first failure wins.
    fn sanitize(&mut self, args: &SanitizeArgs, now: Instant) -> Handled {
        // 1. Known, unexpired verdict.
        let issued = self
            .verdicts
            .get(&args.verdict_id)
            .filter(|issued| now.saturating_duration_since(issued.issued_at) < VERDICT_TTL)
            .cloned()
            .ok_or_else(|| Failure::new(ErrorCode::VerdictExpired, "unknown or expired verdict"))?;

        // 2. Identity read again from the drive.
        if args.device.identity != issued.identity {
            return Err(Failure::new(
                ErrorCode::DeviceChanged,
                "request identity differs from the verdict",
            ));
        }
        let device = self
            .backend
            .find(&issued.identity)
            .filter(|device| convert::identity(&device.profile.identity) == issued.identity)
            .ok_or_else(|| Failure::new(ErrorCode::DeviceChanged, "drive identity changed"))?;

        // 3. Protections evaluated again.
        if let Verdict::Protected { reasons } =
            oblivio_core::decide(&device.profile, &self.config.rules)
        {
            let codes: Vec<&str> = reasons.iter().map(|code| code.as_str()).collect();
            return Err(Failure::new(ErrorCode::Protected, codes.join(",")));
        }

        // 4. Last four characters of the serial number.
        if args.confirmation != last_chars(&issued.identity.serial, 4) {
            return Err(Failure::new(
                ErrorCode::ConfirmationMismatch,
                "confirmation does not match the serial number",
            ));
        }

        // 5. Every warning of the chosen method acknowledged.
        let choice = chosen_method(&issued.verdict, args);
        if let Some((choice, disposition)) = &choice {
            let missing: Vec<&str> = choice
                .required_acknowledgements(disposition)
                .into_iter()
                .filter(|warning| !args.acknowledged.contains(warning))
                .map(acknowledgement_str)
                .collect();
            if !missing.is_empty() {
                return Err(Failure::new(ErrorCode::AckMissing, missing.join(",")));
            }
        }

        // 6. Method allowed by the verdict.
        if choice.is_none() {
            return Err(Failure::new(
                ErrorCode::VerdictMismatch,
                "method not allowed by the verdict",
            ));
        }

        // 7. Forcing only on a forcible SAY NO, with a long enough justification.
        let forced = match &args.force {
            Some(force) => Some(
                oblivio_core::force(&issued.verdict, &force.justification, &self.config.rules)
                    .map_err(|error| Failure::new(ErrorCode::ForceNotAllowed, error.to_string()))?,
            ),
            None => None,
        };

        // 8. License.
        if self.config.licensing.status().state != LicenseState::Valid {
            return Err(Failure::new(
                ErrorCode::LicenseRequired,
                "no valid license installed",
            ));
        }

        // 9. One job at a time.
        if self.running_job() {
            return Err(Failure::new(ErrorCode::Busy, "a job is running"));
        }

        // Every check passed: plan, sign and run.
        let wanted = choice.map(|(choice, _)| choice.method);
        let plan = match (forced, &issued.verdict) {
            (Some(forced), _) => Plan::Forced(forced),
            (None, Verdict::Ready(ready)) => {
                let option = ready
                    .options()
                    .find(|option| Some(convert::method(option.method)) == wanted)
                    .ok_or_else(|| {
                        Failure::new(
                            ErrorCode::VerdictMismatch,
                            "method not allowed by the verdict",
                        )
                    })?;
                Plan::Ready {
                    ready: ready.clone(),
                    option,
                }
            }
            (None, _) => {
                return Err(Failure::new(
                    ErrorCode::VerdictMismatch,
                    "method not allowed by the verdict",
                ));
            }
        };
        let signer = self.config.licensing.signer().map_err(Failure::from)?;
        let id = job::uuid_v7().map_err(|error| Failure::new(ErrorCode::Internal, error))?;
        let target = self
            .backend
            .open(&issued.identity)
            .map_err(|error| Failure::new(ErrorCode::Internal, error))?;
        // A verdict is used once.
        self.verdicts.remove(&args.verdict_id);
        let spec = JobSpec {
            id,
            device: oblivio_protocol::command::DeviceRef {
                node_hint: device.node_hint.clone(),
                identity: issued.identity.clone(),
            },
            profile: device.profile.clone(),
            capacity_bytes: device.capacity_bytes,
            plan,
            rules: self.config.rules.clone(),
            signer,
            target,
            operator: args.operator.clone(),
            organization: args.organization.clone(),
            binary_sha256: self.config.binary_sha256,
            reports_dir: self.config.reports_dir.clone(),
        };
        self.threads.push(job::spawn(
            spec,
            Arc::clone(&self.jobs),
            self.events.clone(),
        ));
        to_value(&SanitizeResult {
            job_id: JobId(oblivio_attestation::display::uuid(&id)),
        })
    }

    fn list_jobs(&self) -> Handled {
        let jobs = self
            .jobs
            .lock()
            .map_err(|_| Failure::new(ErrorCode::Internal, "job table poisoned"))?
            .iter()
            .map(|(job_id, record)| JobSummary {
                job_id: job_id.clone(),
                device: record.device.clone(),
                phase: record.phase,
                status: record.status,
            })
            .collect();
        to_value(&ListJobsResult { jobs })
    }

    fn job_result(&self, job_id: &JobId) -> Handled {
        use base64::Engine;
        let jobs = self
            .jobs
            .lock()
            .map_err(|_| Failure::new(ErrorCode::Internal, "job table poisoned"))?;
        let record = jobs
            .get(job_id)
            .ok_or_else(|| Failure::new(ErrorCode::BadRequest, "unknown job"))?;
        if let Some(failure) = &record.failure {
            return Err(Failure::new(ErrorCode::Internal, failure.clone()));
        }
        match (&record.status, &record.result) {
            (Some(status), Some((signed, journal))) => to_value(&JobResult {
                job_id: job_id.clone(),
                status: *status,
                attestation: base64::engine::general_purpose::STANDARD.encode(signed),
                journal: String::from_utf8_lossy(journal).into_owned(),
            }),
            _ => Err(Failure::new(ErrorCode::Busy, "job still running")),
        }
    }
}

/// The method `args` asks for, if the verdict allows it, with the disposition
/// its acknowledgements depend on.
fn chosen_method(
    verdict: &Verdict,
    args: &SanitizeArgs,
) -> Option<(wire_verdict::MethodChoice, wire_verdict::Disposition)> {
    match (convert::verdict(verdict), &args.force) {
        (
            wire_verdict::Verdict::Ready {
                recommended,
                alternatives,
                disposition,
            },
            _,
        ) => {
            let wanted = args.method.unwrap_or(recommended.method);
            std::iter::once(recommended)
                .chain(alternatives)
                .find(|choice| choice.method == wanted)
                .map(|choice| (choice, disposition))
        }
        (
            wire_verdict::Verdict::SayNo {
                forcible: Some(forcible),
                ..
            },
            Some(_),
        ) if args.method.is_none_or(|method| method == forcible.method) => {
            Some((forcible, wire_verdict::Disposition::Reuse {}))
        }
        _ => None,
    }
}

fn acknowledgement_str(warning: oblivio_protocol::codes::Acknowledgement) -> &'static str {
    use oblivio_protocol::codes::Acknowledgement;
    match warning {
        Acknowledgement::Disposition(code) => code.as_str(),
        Acknowledgement::Reserve(code) => code.as_str(),
        Acknowledgement::Scope(code) => code.as_str(),
    }
}

/// The last `count` characters of `text`, or all of it if shorter.
fn last_chars(text: &str, count: usize) -> &str {
    if count == 0 {
        return "";
    }
    let start = text
        .char_indices()
        .rev()
        .nth(count - 1)
        .map_or(0, |(index, _)| index);
    &text[start..]
}

fn to_value<T: Serialize>(result: &T) -> Handled {
    serde_json::to_value(result)
        .map_err(|error| Failure::new(ErrorCode::Internal, error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Device;
    use oblivio_core::profile::{
        AtaProfile, AtaSanitizeCaps, AtaSecurity, AtaSmart, DeviceIdentity as EngineIdentity,
        DeviceProfile, Inspection, NvmeProfile, NvmeSanitizeCaps, ProtectionFacts, QuirkFlags,
    };
    use oblivio_protocol::codes::{
        Acknowledgement, DispositionCode, Method, ReserveCode, ScopeWarning,
    };
    use oblivio_protocol::command::{DeviceArgs, DeviceRef, ForceArgs};
    use oblivio_protocol::response::LicenseStatusResult;

    /// Error of a fixed licensing asked for its key: reaching it proves that
    /// a request passed every Sanitize check.
    const NO_SIGNER: &str = "fixed licensing has no key";

    /// Licensing with a fixed state, for session tests.
    #[derive(Debug)]
    struct FixedLicensing(LicenseStatusResult);

    impl Licensing for FixedLicensing {
        fn status(&self) -> LicenseStatusResult {
            self.0.clone()
        }

        fn activation_request(&self) -> Result<String, ErrorBody> {
            Ok("OBA1:TEST".to_owned())
        }

        fn install(&self, _text: &str) -> Result<LicenseStatusResult, ErrorBody> {
            Err(ErrorBody {
                code: ErrorCode::LicenseInvalid,
                detail: None,
            })
        }

        fn signer(&self) -> Result<crate::licensing::Signer, ErrorBody> {
            Err(ErrorBody {
                code: ErrorCode::LicenseRequired,
                detail: Some(NO_SIGNER.to_owned()),
            })
        }
    }

    /// Backend over fictional drives; tests edit them between requests.
    #[derive(Debug, Default)]
    struct FakeBackend {
        devices: Vec<Device>,
    }

    impl DeviceBackend for FakeBackend {
        fn list(&mut self) -> Vec<Device> {
            self.devices.clone()
        }

        fn find(&mut self, identity: &DeviceIdentity) -> Option<Device> {
            self.devices
                .iter()
                .find(|device| convert::identity(&device.profile.identity) == *identity)
                .cloned()
        }
    }

    fn engine_identity(serial: &str) -> EngineIdentity {
        EngineIdentity {
            model: "Fictional NVMe 1TB".to_owned(),
            serial: serial.to_owned(),
            firmware: "1.0".to_owned(),
            wwn: None,
            eui64: Some("0011223344556677".to_owned()),
            nguid: None,
        }
    }

    fn nvme_device(serial: &str) -> Device {
        Device {
            node_hint: Some("/dev/nvme9n1".to_owned()),
            capacity_bytes: Some(1_000_000_000_000),
            profile: DeviceProfile {
                identity: engine_identity(serial),
                protection: ProtectionFacts::default(),
                inspection: Inspection::Nvme(NvmeProfile {
                    sanitize: NvmeSanitizeCaps {
                        block_erase: true,
                        crypto_erase: false,
                    },
                    ..NvmeProfile::default()
                }),
                failed_unblock_attempts: 0,
                quirks: QuirkFlags::default(),
            },
        }
    }

    fn worn_hdd(serial: &str) -> Device {
        let mut device = nvme_device(serial);
        device.profile.inspection = Inspection::Ata(AtaProfile {
            solid_state: false,
            sanitize: AtaSanitizeCaps {
                overwrite: true,
                ..AtaSanitizeCaps::default()
            },
            security: AtaSecurity::default(),
            smart: AtaSmart {
                reallocated_sectors: Some(60),
                ..AtaSmart::default()
            },
            ..AtaProfile::default()
        });
        device
    }

    fn license(state: LicenseState) -> LicenseStatusResult {
        LicenseStatusResult {
            state,
            offer: None,
            license_id: None,
            valid_until: None,
        }
    }

    struct Harness {
        session: Session<FakeBackend>,
        start: Instant,
        next_id: u64,
    }

    impl Harness {
        fn new(devices: Vec<Device>, license_state: LicenseState) -> Self {
            let config = SessionConfig {
                rules: RuleSet::baseline(),
                binary_sha256: [0; 32],
                reports_dir: std::env::temp_dir().join("oblivio-unused-reports"),
                licensing: Box::new(FixedLicensing(license(license_state))),
            };
            Self {
                session: Session::new(FakeBackend { devices }, config),
                start: Instant::now(),
                next_id: 0,
            }
        }

        fn greeted(devices: Vec<Device>, license_state: LicenseState) -> Self {
            let mut harness = Self::new(devices, license_state);
            harness
                .send(Command::Hello(HelloArgs {
                    protocol_version: PROTOCOL_VERSION,
                    app_version: "test".to_owned(),
                }))
                .expect("Hello accepted");
            harness
        }

        fn send_at(&mut self, command: Command, elapsed: Duration) -> Result<Value, ErrorBody> {
            self.next_id += 1;
            let request = Request {
                id: self.next_id,
                command,
            };
            let response = self.session.handle(request, self.start + elapsed);
            assert_eq!(response.id, self.next_id);
            response.outcome
        }

        fn send(&mut self, command: Command) -> Result<Value, ErrorBody> {
            self.send_at(command, Duration::ZERO)
        }

        fn inspect(&mut self, serial: &str) -> InspectResult {
            let value = self
                .send(Command::Inspect(DeviceArgs {
                    device: device_ref(serial),
                }))
                .expect("Inspect accepted");
            serde_json::from_value(value).expect("InspectResult")
        }
    }

    fn device_ref(serial: &str) -> DeviceRef {
        DeviceRef {
            node_hint: None,
            identity: convert::identity(&engine_identity(serial)),
        }
    }

    fn sanitize_args(inspected: &InspectResult, confirmation: &str) -> SanitizeArgs {
        SanitizeArgs {
            verdict_id: inspected.verdict_id.clone(),
            device: inspected.device.clone(),
            method: None,
            confirmation: confirmation.to_owned(),
            acknowledged: vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
            force: None,
            operator: None,
            organization: None,
        }
    }

    fn code(result: Result<Value, ErrorBody>) -> ErrorCode {
        result.expect_err("request must be rejected").code
    }

    // --- Session.

    #[test]
    fn hello_is_required_first() {
        let mut harness = Harness::new(vec![], LicenseState::Missing);
        assert_eq!(
            code(harness.send(Command::ListDevices)),
            ErrorCode::BadRequest
        );
    }

    #[test]
    fn incompatible_protocol_is_rejected() {
        let mut harness = Harness::new(vec![], LicenseState::Missing);
        let result = harness.send(Command::Hello(HelloArgs {
            protocol_version: PROTOCOL_VERSION + 1,
            app_version: "future".to_owned(),
        }));
        assert_eq!(code(result), ErrorCode::ProtocolMismatch);
        assert_eq!(
            code(harness.send(Command::ListDevices)),
            ErrorCode::BadRequest
        );
    }

    #[test]
    fn hello_reports_versions() {
        let mut harness = Harness::new(vec![], LicenseState::Missing);
        let value = harness
            .send(Command::Hello(HelloArgs {
                protocol_version: PROTOCOL_VERSION,
                app_version: "test".to_owned(),
            }))
            .expect("Hello accepted");
        let hello: HelloResult = serde_json::from_value(value).expect("HelloResult");
        assert_eq!(hello.rules_version, "1.0.0");
        assert_eq!(hello.license.state, LicenseState::Missing);
    }

    #[test]
    fn list_devices_shows_protections() {
        let mut mounted = nvme_device("SN-MOUNTED");
        mounted.profile.protection.has_mounted_partition = true;
        let mut harness = Harness::greeted(
            vec![nvme_device("SN-FREE-0001"), mounted],
            LicenseState::Missing,
        );
        let value = harness.send(Command::ListDevices).expect("listed");
        let list: ListDevicesResult = serde_json::from_value(value).expect("ListDevicesResult");
        let protections: Vec<_> = list.devices.iter().map(|d| d.protection.clone()).collect();
        assert_eq!(
            protections,
            [
                vec![],
                vec![oblivio_protocol::codes::ProtectCode::MountedPartition]
            ]
        );
    }

    #[test]
    fn inspect_unknown_drive_is_not_found() {
        let mut harness = Harness::greeted(vec![], LicenseState::Missing);
        let result = harness.send(Command::Inspect(DeviceArgs {
            device: device_ref("SN-NOWHERE"),
        }));
        assert_eq!(code(result), ErrorCode::DeviceNotFound);
    }

    #[test]
    fn shutdown_ends_the_session() {
        let mut harness = Harness::greeted(vec![], LicenseState::Missing);
        harness.send(Command::Shutdown).expect("Shutdown accepted");
        assert!(harness.session.is_shut_down());
    }

    // --- Sanitize checks, in the order of §7.

    #[test]
    fn check_1_unknown_verdict_is_expired() {
        let mut harness = Harness::greeted(vec![nvme_device("SN-0001")], LicenseState::Valid);
        let mut inspected = harness.inspect("SN-0001");
        inspected.verdict_id = VerdictId("forged".to_owned());
        let result = harness.send(Command::Sanitize(sanitize_args(&inspected, "0001")));
        assert_eq!(code(result), ErrorCode::VerdictExpired);
    }

    #[test]
    fn check_1_verdict_expires_after_ten_minutes() {
        let mut harness = Harness::greeted(vec![nvme_device("SN-0001")], LicenseState::Valid);
        let inspected = harness.inspect("SN-0001");
        let result = harness.send_at(
            Command::Sanitize(sanitize_args(&inspected, "0001")),
            VERDICT_TTL,
        );
        assert_eq!(code(result), ErrorCode::VerdictExpired);
    }

    #[test]
    fn check_2_swapped_drive_is_detected() {
        let mut harness = Harness::greeted(vec![nvme_device("SN-0001")], LicenseState::Valid);
        let inspected = harness.inspect("SN-0001");
        harness.session.backend_mut().devices = vec![nvme_device("SN-0002")];
        let result = harness.send(Command::Sanitize(sanitize_args(&inspected, "0001")));
        assert_eq!(code(result), ErrorCode::DeviceChanged);
    }

    #[test]
    fn check_2_request_identity_must_match_the_verdict() {
        let mut harness = Harness::greeted(
            vec![nvme_device("SN-0001"), nvme_device("SN-0002")],
            LicenseState::Valid,
        );
        let inspected = harness.inspect("SN-0001");
        let mut args = sanitize_args(&inspected, "0002");
        args.device = device_ref("SN-0002");
        assert_eq!(
            code(harness.send(Command::Sanitize(args))),
            ErrorCode::DeviceChanged
        );
    }

    #[test]
    fn check_3_drive_mounted_after_inspection_is_protected() {
        let mut harness = Harness::greeted(vec![nvme_device("SN-0001")], LicenseState::Valid);
        let inspected = harness.inspect("SN-0001");
        harness.session.backend_mut().devices[0]
            .profile
            .protection
            .has_mounted_partition = true;
        let result = harness.send(Command::Sanitize(sanitize_args(&inspected, "0001")));
        assert_eq!(code(result), ErrorCode::Protected);
    }

    #[test]
    fn check_4_confirmation_must_be_the_last_four_characters() {
        let mut harness = Harness::greeted(vec![nvme_device("SN-0001")], LicenseState::Valid);
        let inspected = harness.inspect("SN-0001");
        for wrong in ["001", "N-0001", "SN-0", "0001 "] {
            let result = harness.send(Command::Sanitize(sanitize_args(&inspected, wrong)));
            assert_eq!(code(result), ErrorCode::ConfirmationMismatch, "{wrong:?}");
        }
    }

    #[test]
    fn check_4_comes_before_check_5() {
        let mut harness = Harness::greeted(vec![nvme_device("SN-0001")], LicenseState::Valid);
        let inspected = harness.inspect("SN-0001");
        let mut args = sanitize_args(&inspected, "9999");
        args.acknowledged.clear();
        assert_eq!(
            code(harness.send(Command::Sanitize(args))),
            ErrorCode::ConfirmationMismatch
        );
    }

    #[test]
    fn check_5_every_warning_must_be_acknowledged() {
        let mut harness = Harness::greeted(vec![worn_hdd("SN-HDD1")], LicenseState::Valid);
        let inspected = harness.inspect("SN-HDD1");
        let mut args = sanitize_args(&inspected, "HDD1");
        args.acknowledged = vec![];
        let error = harness
            .send(Command::Sanitize(args.clone()))
            .expect_err("D4 not acknowledged");
        assert_eq!(error.code, ErrorCode::AckMissing);
        assert_eq!(error.detail.as_deref(), Some("D4"));

        args.acknowledged = vec![Acknowledgement::Disposition(
            DispositionCode::ReallocatedAboveThreshold,
        )];
        let error = harness
            .send(Command::Sanitize(args))
            .expect_err("no signing key");
        assert_eq!(
            error.detail.as_deref(),
            Some(NO_SIGNER),
            "acknowledged: the request passes every check"
        );
    }

    #[test]
    fn check_5_host_write_alternative_needs_r7() {
        let mut harness = Harness::greeted(vec![nvme_device("SN-0001")], LicenseState::Valid);
        let inspected = harness.inspect("SN-0001");
        let mut args = sanitize_args(&inspected, "0001");
        args.method = Some(Method::HostOverwrite);
        args.acknowledged = vec![];
        let error = harness
            .send(Command::Sanitize(args))
            .expect_err("R7 not acknowledged");
        assert_eq!(error.code, ErrorCode::AckMissing);
        assert_eq!(error.detail.as_deref(), Some("R7"));
    }

    #[test]
    fn check_6_method_outside_the_verdict_is_a_mismatch() {
        let mut harness = Harness::greeted(vec![nvme_device("SN-0001")], LicenseState::Valid);
        let inspected = harness.inspect("SN-0001");
        let mut args = sanitize_args(&inspected, "0001");
        args.method = Some(Method::NvmeFormatCryptoErase);
        assert_eq!(
            code(harness.send(Command::Sanitize(args))),
            ErrorCode::VerdictMismatch
        );
    }

    #[test]
    fn check_6_say_no_without_force_is_a_mismatch() {
        let mut failed = nvme_device("SN-DEAD");
        failed.profile.inspection = Inspection::Failed;
        let mut harness = Harness::greeted(vec![failed], LicenseState::Valid);
        let inspected = harness.inspect("SN-DEAD");
        assert_eq!(
            code(harness.send(Command::Sanitize(sanitize_args(&inspected, "DEAD")))),
            ErrorCode::VerdictMismatch
        );
    }

    #[test]
    fn check_7_force_on_a_ready_verdict_is_not_allowed() {
        let mut harness = Harness::greeted(vec![nvme_device("SN-0001")], LicenseState::Valid);
        let inspected = harness.inspect("SN-0001");
        let mut args = sanitize_args(&inspected, "0001");
        args.force = Some(ForceArgs {
            justification: "Je veux forcer ce disque sain.".to_owned(),
        });
        assert_eq!(
            code(harness.send(Command::Sanitize(args))),
            ErrorCode::ForceNotAllowed
        );
    }

    #[test]
    fn check_7_short_justification_is_not_allowed() {
        let mut failed = nvme_device("SN-DEAD");
        failed.profile.inspection = Inspection::Failed;
        let mut harness = Harness::greeted(vec![failed], LicenseState::Valid);
        let inspected = harness.inspect("SN-DEAD");
        let mut args = sanitize_args(&inspected, "DEAD");
        args.acknowledged = vec![Acknowledgement::Reserve(ReserveCode::HostWriteCoverage)];
        args.force = Some(ForceArgs {
            justification: "trop court".to_owned(),
        });
        assert_eq!(
            code(harness.send(Command::Sanitize(args))),
            ErrorCode::ForceNotAllowed
        );
    }

    #[test]
    fn check_8_license_is_required() {
        let mut harness = Harness::greeted(vec![nvme_device("SN-0001")], LicenseState::Missing);
        let inspected = harness.inspect("SN-0001");
        assert_eq!(
            code(harness.send(Command::Sanitize(sanitize_args(&inspected, "0001")))),
            ErrorCode::LicenseRequired
        );
    }

    #[test]
    fn valid_request_passes_every_check() {
        let mut harness = Harness::greeted(vec![nvme_device("SN-0001")], LicenseState::Valid);
        let inspected = harness.inspect("SN-0001");
        let error = harness
            .send(Command::Sanitize(sanitize_args(&inspected, "0001")))
            .expect_err("no signing key");
        assert_eq!(error.detail.as_deref(), Some(NO_SIGNER));
    }

    #[test]
    fn forced_n1_request_passes_every_check() {
        let mut failed = nvme_device("SN-DEAD");
        failed.profile.inspection = Inspection::Failed;
        let mut harness = Harness::greeted(vec![failed], LicenseState::Valid);
        let inspected = harness.inspect("SN-DEAD");
        let mut args = sanitize_args(&inspected, "DEAD");
        args.acknowledged = vec![Acknowledgement::Reserve(ReserveCode::HostWriteCoverage)];
        args.force = Some(ForceArgs {
            justification: "IDENTIFY muet, écriture forcée".to_owned(),
        });
        let error = harness
            .send(Command::Sanitize(args))
            .expect_err("no signing key");
        assert_eq!(error.detail.as_deref(), Some(NO_SIGNER));
    }

    #[test]
    fn last_chars_counts_characters_not_bytes() {
        assert_eq!(last_chars("S5GXNX0T1234567", 4), "4567");
        assert_eq!(last_chars("ÉÈ12", 4), "ÉÈ12");
        assert_eq!(last_chars("ab", 4), "ab");
    }
}

#[cfg(test)]
mod licensing_commands {
    use super::*;
    use crate::backend::NoDevices;
    use crate::licensing::StoreLicensing;
    use oblivio_license::LicenseStore;
    use oblivio_protocol::command::InstallLicenseArgs;
    use oblivio_protocol::response::LicenseStatusResult;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn activation_then_installation_enables_erasure_checks() {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
            .expect("chmod");
        let licensing =
            StoreLicensing::new(LicenseStore::new(dir.path()), crate::licensing::root_key());
        let mut session = Session::new(
            NoDevices,
            SessionConfig {
                rules: RuleSet::baseline(),
                binary_sha256: [0; 32],
                reports_dir: std::env::temp_dir().join("oblivio-unused-reports"),
                licensing: Box::new(licensing),
            },
        );
        let now = Instant::now();
        let mut send = |id, command| session.handle(Request { id, command }, now).outcome;
        send(
            1,
            Command::Hello(HelloArgs {
                protocol_version: PROTOCOL_VERSION,
                app_version: "test".to_owned(),
            }),
        )
        .expect("hello");
        let request: ActivationRequestResult =
            serde_json::from_value(send(2, Command::CreateActivationRequest).expect("request"))
                .expect("ActivationRequestResult");
        let license = oblivio_license::dev_root::issue(
            &request.request,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs()),
            30,
        )
        .expect("issued");
        let status: LicenseStatusResult = serde_json::from_value(
            send(3, Command::InstallLicense(InstallLicenseArgs { license })).expect("installed"),
        )
        .expect("LicenseStatusResult");
        assert_eq!(status.state, LicenseState::Valid);
        let status: LicenseStatusResult =
            serde_json::from_value(send(4, Command::LicenseStatus).expect("status"))
                .expect("LicenseStatusResult");
        assert_eq!(status.state, LicenseState::Valid);
    }
}
