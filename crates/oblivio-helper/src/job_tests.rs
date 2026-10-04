//! End-to-end erasure tests: a real workstation key and development license,
//! image files as drives, every attestation verified against the dev root.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use oblivio_attestation::model::{Category, DriveType, Status};
use oblivio_attestation::{Verified, verify};
use oblivio_core::RuleSet;
use oblivio_core::method::Method as CoreMethod;
use oblivio_core::profile::{
    DeviceIdentity, DeviceProfile, Inspection, NvmeProfile, NvmeSanitizeCaps, ProtectionFacts,
    QuirkFlags,
};
use oblivio_device::block::{BlockIo, FileImage};
use oblivio_license::LicenseStore;
use oblivio_protocol::codes::{Acknowledgement, Method, ReserveCode, ScopeWarning};
use oblivio_protocol::command::{
    Command, DeviceArgs, DeviceIdentity as WireIdentity, DeviceRef, ForceArgs, HelloArgs,
    InstallLicenseArgs, JobArgs, SanitizeArgs,
};
use oblivio_protocol::event::Event;
use oblivio_protocol::response::{
    ActivationRequestResult, ErrorCode, FinalStatus, InspectResult, JobPhase, JobResult,
    SanitizeResult,
};
use oblivio_protocol::{PROTOCOL_VERSION, Request, ServerMessage};

use crate::backend::{Device, DeviceBackend};
use crate::convert;
use crate::erasure::{self, ErasureTarget};
use crate::licensing::{self, StoreLicensing};
use crate::session::{Session, SessionConfig};

/// Image size: 4096 blocks of 512 bytes (2 MiB), so tests stay fast.
const BLOCKS: u64 = 4096;

/// What the firmware of a test drive does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Firmware {
    /// Erases the image.
    Honest,
    /// Reports success without erasing anything.
    Lying,
}

struct TestTarget {
    image: FileImage,
    firmware: Firmware,
}

impl ErasureTarget for TestTarget {
    fn io(&mut self) -> &mut dyn BlockIo {
        &mut self.image
    }

    fn firmware(&mut self, _: CoreMethod, progress: &mut dyn FnMut(u8)) -> Result<bool, String> {
        if self.firmware == Firmware::Honest {
            erasure::host_overwrite(&mut self.image, &mut |_| {}).map_err(|e| e.to_string())?;
        }
        progress(100);
        Ok(true)
    }

    fn smart_after(&mut self) -> Option<oblivio_core::profile::AtaSmart> {
        None
    }

    fn simulated(&self) -> bool {
        false
    }
}

struct ImageBackend {
    devices: Vec<Device>,
    dir: PathBuf,
    firmware: Firmware,
}

impl DeviceBackend for ImageBackend {
    fn list(&mut self) -> Vec<Device> {
        self.devices.clone()
    }

    fn find(&mut self, identity: &WireIdentity) -> Option<Device> {
        self.devices
            .iter()
            .find(|device| convert::identity(&device.profile.identity) == *identity)
            .cloned()
    }

    fn open(&mut self, identity: &WireIdentity) -> Result<Box<dyn ErasureTarget>, String> {
        let path = self.dir.join(format!("{}.img", identity.serial));
        let mut image = FileImage::create(&path, 512, BLOCKS).map_err(|e| e.to_string())?;
        // Data to destroy.
        image
            .write_blocks(0, &vec![0x5a; (BLOCKS * 512) as usize])
            .map_err(|e| e.to_string())?;
        Ok(Box::new(TestTarget {
            image,
            firmware: self.firmware,
        }))
    }
}

fn drive(serial: &str, inspection: Inspection) -> Device {
    Device {
        node_hint: Some("/dev/test0".to_owned()),
        capacity_bytes: Some(BLOCKS * 512),
        profile: DeviceProfile {
            identity: DeviceIdentity {
                model: "Test NVMe".to_owned(),
                serial: serial.to_owned(),
                firmware: "T1".to_owned(),
                wwn: None,
                eui64: Some("0011223344556677".to_owned()),
                nguid: None,
            },
            protection: ProtectionFacts::default(),
            inspection,
            failed_unblock_attempts: 0,
            quirks: QuirkFlags::default(),
        },
    }
}

fn nvme() -> Inspection {
    Inspection::Nvme(NvmeProfile {
        sanitize: NvmeSanitizeCaps {
            block_erase: true,
            crypto_erase: false,
        },
        ..NvmeProfile::default()
    })
}

struct Bench {
    dir: tempfile::TempDir,
    session: Session<ImageBackend>,
    events: mpsc::Receiver<ServerMessage>,
    next: u64,
}

impl Bench {
    /// A licensed session over `devices`.
    fn new(devices: Vec<Device>, firmware: Firmware) -> Self {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
            .expect("chmod");
        let licensing = StoreLicensing::new(
            LicenseStore::new(dir.path().join("state")),
            licensing::root_key(),
        );
        let backend = ImageBackend {
            devices,
            dir: dir.path().to_path_buf(),
            firmware,
        };
        let mut session = Session::new(
            backend,
            SessionConfig {
                rules: RuleSet::baseline(),
                binary_sha256: [0xb1; 32],
                reports_dir: dir.path().join("reports"),
                licensing: Box::new(licensing),
            },
        );
        let (sender, events) = mpsc::channel();
        session.set_events(Some(sender));
        let mut bench = Self {
            dir,
            session,
            events,
            next: 0,
        };
        let _: serde_json::Value = bench.ok(Command::Hello(HelloArgs {
            protocol_version: PROTOCOL_VERSION,
            app_version: "test".to_owned(),
        }));
        let request: ActivationRequestResult = bench.ok(Command::CreateActivationRequest);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let license = oblivio_license::dev_root::issue(&request.request, now, 30).expect("issued");
        let _: serde_json::Value =
            bench.ok(Command::InstallLicense(InstallLicenseArgs { license }));
        bench
    }

    fn send(
        &mut self,
        command: Command,
    ) -> Result<serde_json::Value, oblivio_protocol::response::ErrorBody> {
        self.next += 1;
        self.session
            .handle(
                Request {
                    id: self.next,
                    command,
                },
                Instant::now(),
            )
            .outcome
    }

    fn ok<T: serde::de::DeserializeOwned>(&mut self, command: Command) -> T {
        let value = self
            .send(command)
            .unwrap_or_else(|error| panic!("refused: {error:?}"));
        serde_json::from_value(value).expect("typed result")
    }

    fn inspect(&mut self, serial: &str) -> InspectResult {
        let device = self
            .session
            .backend_mut()
            .devices
            .iter()
            .find(|device| device.profile.identity.serial == serial)
            .map(|device| DeviceRef {
                node_hint: None,
                identity: convert::identity(&device.profile.identity),
            })
            .expect("known drive");
        self.ok(Command::Inspect(DeviceArgs { device }))
    }

    /// Runs Sanitize to its end and returns the verified attestation.
    fn erase(&mut self, args: SanitizeArgs) -> (Verified, JobResult) {
        let started: SanitizeResult = self.ok(Command::Sanitize(args));
        self.session.wait_for_jobs();
        let result: JobResult = self.ok(Command::GetJobResult(JobArgs {
            job_id: started.job_id.clone(),
        }));
        assert_eq!(result.job_id, started.job_id);
        let signed = base64::engine::general_purpose::STANDARD
            .decode(&result.attestation)
            .expect("base64");
        let verified = verify(&signed, &oblivio_license::dev_root::verifying_key(), &[])
            .expect("attestation verifies against the dev root");
        assert!(verified.matches_journal(result.journal.as_bytes()));
        (verified, result)
    }

    fn phases(&self) -> Vec<JobPhase> {
        self.events
            .try_iter()
            .filter_map(|message| match message {
                ServerMessage::Event(Event::JobPhase(data)) => Some(data.phase),
                _ => None,
            })
            .collect()
    }

    fn reports(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(self.dir.path().join("reports"))
            .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default();
        files.sort();
        files
    }
}

fn sanitize(
    inspection: &InspectResult,
    method: Option<Method>,
    acknowledged: Vec<Acknowledgement>,
) -> SanitizeArgs {
    SanitizeArgs {
        verdict_id: inspection.verdict_id.clone(),
        device: inspection.device.clone(),
        method,
        confirmation: inspection.device.identity.serial
            [inspection.device.identity.serial.len() - 4..]
            .to_owned(),
        acknowledged,
        force: None,
        operator: Some("J. Martin".to_owned()),
        organization: Some("Atelier XYZ".to_owned()),
    }
}

fn image_is_zero(path: &Path) -> bool {
    std::fs::read(path)
        .expect("image")
        .iter()
        .all(|&byte| byte == 0)
}

#[test]
fn firmware_erasure_is_compliant_purge() {
    let mut bench = Bench::new(vec![drive("SN-A001", nvme())], Firmware::Honest);
    let inspection = bench.inspect("SN-A001");
    let args = sanitize(
        &inspection,
        None,
        vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
    );
    let (verified, result) = bench.erase(args);
    let attestation = verified.attestation;
    assert_eq!(result.status, FinalStatus::Compliant);
    assert_eq!(attestation.status, Status::Compliant);
    assert_eq!(attestation.sanitization.category, Category::Purge);
    assert_eq!(
        attestation
            .sanitization
            .method
            .as_ref()
            .map(|code| code.as_str()),
        Some("NvmeSanitizeBlockErase")
    );
    assert_eq!(attestation.verified, Some(true));
    assert!(
        attestation
            .validation
            .as_ref()
            .is_some_and(|v| v.passed && v.samples >= 100)
    );
    assert_eq!(attestation.device.serial, "SN-A001");
    assert_eq!(attestation.tool.binary_sha256, [0xb1; 32]);
    assert_eq!(attestation.organization.as_deref(), Some("Atelier XYZ"));
    assert!(image_is_zero(&bench.dir.path().join("SN-A001.img")));
    assert_eq!(
        bench.phases(),
        [
            JobPhase::Preparing,
            JobPhase::Canaries,
            JobPhase::Executing,
            JobPhase::Verifying,
            JobPhase::Validating,
            JobPhase::Signing,
            JobPhase::Done,
        ]
    );
    assert_eq!(bench.reports().len(), 2, "report .cose and .json stored");
}

#[test]
fn host_overwrite_is_compliant_clear_with_r7() {
    let mut bench = Bench::new(vec![drive("SN-B002", nvme())], Firmware::Honest);
    let inspection = bench.inspect("SN-B002");
    let args = sanitize(
        &inspection,
        Some(Method::HostOverwrite),
        vec![Acknowledgement::Reserve(ReserveCode::HostWriteCoverage)],
    );
    let (verified, _) = bench.erase(args);
    let attestation = verified.attestation;
    assert_eq!(attestation.status, Status::Compliant);
    assert_eq!(attestation.sanitization.category, Category::Clear);
    let reserves: Vec<&str> = attestation
        .reserves
        .iter()
        .map(|code| code.as_str())
        .collect();
    assert_eq!(reserves, ["R7"]);
    assert!(image_is_zero(&bench.dir.path().join("SN-B002.img")));
    assert!(
        !bench.phases().contains(&JobPhase::Canaries),
        "no canaries for a host write"
    );
}

#[test]
fn lying_firmware_is_caught_by_canaries_and_refused() {
    let mut bench = Bench::new(vec![drive("SN-C003", nvme())], Firmware::Lying);
    let inspection = bench.inspect("SN-C003");
    let args = sanitize(
        &inspection,
        None,
        vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
    );
    let (verified, result) = bench.erase(args);
    let attestation = verified.attestation;
    assert_eq!(result.status, FinalStatus::Refused);
    assert_eq!(attestation.status, Status::Refused);
    assert_eq!(attestation.sanitization.category, Category::None);
    let refusals: Vec<&str> = attestation
        .refusals
        .iter()
        .map(|code| code.as_str())
        .collect();
    assert_eq!(refusals, ["N10"]);
    let journal: serde_json::Value = serde_json::from_str(&result.journal).expect("journal JSON");
    assert_eq!(
        journal["attempts"].as_array().map(Vec::len),
        Some(2),
        "one retry"
    );
}

#[test]
fn forced_n1_is_non_compliant_with_justification() {
    let mut bench = Bench::new(vec![drive("SN-D004", Inspection::Failed)], Firmware::Honest);
    let inspection = bench.inspect("SN-D004");
    let mut args = sanitize(
        &inspection,
        None,
        vec![Acknowledgement::Reserve(ReserveCode::HostWriteCoverage)],
    );
    args.force = Some(ForceArgs {
        justification: "IDENTIFY muet, effacement exigé par le client".to_owned(),
    });
    let (verified, result) = bench.erase(args);
    let attestation = verified.attestation;
    assert_eq!(result.status, FinalStatus::NonCompliant);
    assert_eq!(attestation.sanitization.category, Category::Clear);
    assert_eq!(attestation.device.drive_type, DriveType::Unknown);
    assert_eq!(
        attestation.justification.as_deref(),
        Some("IDENTIFY muet, effacement exigé par le client")
    );
    let refusals: Vec<&str> = attestation
        .refusals
        .iter()
        .map(|code| code.as_str())
        .collect();
    assert_eq!(refusals, ["N1"]);
    assert!(image_is_zero(&bench.dir.path().join("SN-D004.img")));
}

#[test]
fn a_verdict_is_used_only_once() {
    let mut bench = Bench::new(vec![drive("SN-E005", nvme())], Firmware::Honest);
    let inspection = bench.inspect("SN-E005");
    let args = sanitize(
        &inspection,
        None,
        vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
    );
    bench.erase(args.clone());
    let error = bench
        .send(Command::Sanitize(args))
        .expect_err("verdict consumed");
    assert_eq!(error.code, ErrorCode::VerdictExpired);
}

#[test]
fn started_job_cannot_be_cancelled() {
    let mut bench = Bench::new(vec![drive("SN-F006", nvme())], Firmware::Honest);
    let inspection = bench.inspect("SN-F006");
    let args = sanitize(
        &inspection,
        None,
        vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
    );
    let started: SanitizeResult = bench.ok(Command::Sanitize(args));
    let error = bench
        .send(Command::Cancel(JobArgs {
            job_id: started.job_id,
        }))
        .expect_err("not cancellable");
    assert_eq!(error.code, ErrorCode::NotCancellable);
    bench.session.wait_for_jobs();
}
