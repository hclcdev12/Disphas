//! End-to-end erasure tests: a real workstation key and development license,
//! image files as drives, every attestation verified against the dev root.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use disphas_attestation::model::{Category, DriveType, Status};
use disphas_attestation::{Verified, verify};
use disphas_core::RuleSet;
use disphas_core::method::Method as CoreMethod;
use disphas_core::profile::{
    DeviceIdentity, DeviceProfile, Inspection, NvmeProfile, NvmeSanitizeCaps, ProtectionFacts,
    QuirkFlags,
};
use disphas_device::block::{BlockIo, FileImage};
use disphas_license::LicenseStore;
use disphas_protocol::codes::{Acknowledgement, Method, ReserveCode, ScopeWarning};
use disphas_protocol::command::{
    Command, DeviceArgs, DeviceIdentity as WireIdentity, DeviceRef, ForceArgs, HelloArgs,
    InstallLicenseArgs, JobArgs, SanitizeArgs,
};
use disphas_protocol::event::{Event, JobProgressData};
use disphas_protocol::response::{
    ActivationRequestResult, Entitlements, ErrorCode, FinalStatus, InspectResult, JobPhase,
    JobResult, LicenseStatusResult, SanitizeResult,
};
use disphas_protocol::{PROTOCOL_VERSION, Request, ServerMessage};

use crate::backend::{Device, DeviceBackend, OpenError};
use crate::convert;
use crate::erasure::{self, ErasureTarget};
use crate::licensing::StoreLicensing;
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
    /// The helper dies while the command runs; the drive carries on and
    /// erases the image (as a real Sanitize does after a power cycle).
    Crash,
    /// Honest firmware, but blocks of zeros written by the host are
    /// silently dropped: a zero pass cannot be validated.
    DropsZeros,
}

/// A disk image that may drop host writes of zeros ([`Firmware::DropsZeros`]).
struct Image {
    file: FileImage,
    drops_zeros: bool,
}

impl BlockIo for Image {
    fn block_size(&self) -> u64 {
        self.file.block_size()
    }

    fn block_count(&self) -> u64 {
        self.file.block_count()
    }

    fn read_blocks(&mut self, lba: u64, buffer: &mut [u8]) -> std::io::Result<()> {
        self.file.read_blocks(lba, buffer)
    }

    fn write_blocks(&mut self, lba: u64, data: &[u8]) -> std::io::Result<()> {
        if self.drops_zeros && data.iter().all(|&byte| byte == 0) {
            return Ok(());
        }
        self.file.write_blocks(lba, data)
    }

    fn sync(&mut self) -> std::io::Result<()> {
        self.file.sync()
    }
}

/// Holds firmware erasures until released, to keep jobs running.
#[derive(Debug, Default)]
struct Gate {
    open: std::sync::Mutex<bool>,
    changed: std::sync::Condvar,
}

impl Gate {
    fn wait(&self) {
        let mut open = self.open.lock().expect("gate");
        while !*open {
            open = self.changed.wait(open).expect("gate");
        }
    }

    fn release(&self) {
        *self.open.lock().expect("gate") = true;
        self.changed.notify_all();
    }
}

struct TestTarget {
    image: Image,
    firmware: Firmware,
    gate: Option<std::sync::Arc<Gate>>,
}

impl ErasureTarget for TestTarget {
    fn io(&mut self) -> &mut dyn BlockIo {
        &mut self.image
    }

    fn firmware(&mut self, _: CoreMethod, progress: &mut dyn FnMut(u8)) -> Result<bool, String> {
        // Like ATA Security Erase: 0 at the start, nothing more until done.
        progress(0);
        if let Some(gate) = &self.gate {
            gate.wait();
        }
        match self.firmware {
            Firmware::Honest | Firmware::DropsZeros => {
                erasure::host_overwrite(&mut self.image, &[0x11; 32], &mut |_| {})
                    .map_err(|e| e.to_string())?;
            }
            Firmware::Lying => {}
            Firmware::Crash => panic!("simulated helper crash"),
        }
        progress(100);
        Ok(true)
    }

    fn resume_firmware(
        &mut self,
        _: CoreMethod,
        progress: &mut dyn FnMut(u8),
    ) -> Result<bool, String> {
        if let Some(gate) = &self.gate {
            gate.wait();
        }
        if self.firmware != Firmware::Lying {
            erasure::host_overwrite(&mut self.image, &[0x11; 32], &mut |_| {})
                .map_err(|e| e.to_string())?;
        }
        progress(100);
        Ok(true)
    }

    fn smart_after(&mut self) -> Option<disphas_core::profile::AtaSmart> {
        None
    }

    fn simulated(&self) -> bool {
        false
    }

    fn firmware_estimate(&mut self, _method: CoreMethod) -> Option<u64> {
        Some(78 * 60)
    }
}

struct ImageBackend {
    devices: Vec<Device>,
    dir: PathBuf,
    firmware: Firmware,
    gate: Option<std::sync::Arc<Gate>>,
    /// Every opening: serial and whether the whole controller was held.
    opened: Vec<(String, bool)>,
    /// Opening fails with this error, as when another program holds the
    /// drive or it became protected since the verdict.
    refuse: Option<OpenError>,
}

impl DeviceBackend for ImageBackend {
    fn list(&mut self) -> Vec<Device> {
        self.devices.clone()
    }

    fn find(&mut self, identity: &WireIdentity) -> Result<Device, &'static str> {
        self.devices
            .iter()
            .find(|device| convert::identity(&device.profile.identity) == *identity)
            .cloned()
            .ok_or("drive not found")
    }

    fn open(
        &mut self,
        identity: &WireIdentity,
        whole_controller: bool,
    ) -> Result<Box<dyn ErasureTarget>, OpenError> {
        self.opened
            .push((identity.serial.clone(), whole_controller));
        if let Some(error) = self.refuse.clone() {
            return Err(error);
        }
        let path = self.dir.join(format!("{}.img", identity.serial));
        if path.exists() {
            // Reopened after a crash: the drive keeps its content.
            let image = FileImage::open(&path, 512).map_err(|e| e.to_string())?;
            return Ok(Box::new(TestTarget {
                image: Image {
                    file: image,
                    drops_zeros: self.firmware == Firmware::DropsZeros,
                },
                firmware: self.firmware,
                gate: self.gate.clone(),
            }));
        }
        let mut image = FileImage::create(&path, 512, BLOCKS).map_err(|e| e.to_string())?;
        // Data to destroy.
        image
            .write_blocks(0, &vec![0x5a; (BLOCKS * 512) as usize])
            .map_err(|e| e.to_string())?;
        Ok(Box::new(TestTarget {
            image: Image {
                file: image,
                drops_zeros: self.firmware == Firmware::DropsZeros,
            },
            firmware: self.firmware,
            gate: self.gate.clone(),
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

/// An NVMe drive with Format NVM only, whose erasure cannot be resumed.
fn nvme_format_only() -> Inspection {
    Inspection::Nvme(NvmeProfile {
        format: disphas_core::profile::NvmeFormatCaps {
            supported: true,
            crypto_erase: false,
            erases_all_namespaces: false,
        },
        ..NvmeProfile::default()
    })
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
    /// Shared by the helpers of one workstation ([`Bench::second_helper`]).
    dir: std::sync::Arc<tempfile::TempDir>,
    session: Session<ImageBackend>,
    events: mpsc::Receiver<ServerMessage>,
    next: u64,
}

impl Bench {
    /// A licensed session over `devices`.
    fn new(devices: Vec<Device>, firmware: Firmware) -> Self {
        Self::with_offer(devices, firmware, "Dev")
    }

    /// A session licensed for `offer` over `devices`.
    fn with_offer(devices: Vec<Device>, firmware: Firmware, offer: &str) -> Self {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
            .expect("chmod");
        let mut bench = Self::open(std::sync::Arc::new(dir), devices, firmware);
        let request: ActivationRequestResult = bench.ok(Command::CreateActivationRequest);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let license = disphas_license::dev_root::issue_for_offer(&request.request, now, 30, offer)
            .expect("issued");
        let _: serde_json::Value =
            bench.ok(Command::InstallLicense(InstallLicenseArgs { license }));
        bench
    }

    /// A new session over the state directory `dir`, as after a restart.
    fn open(
        dir: std::sync::Arc<tempfile::TempDir>,
        devices: Vec<Device>,
        firmware: Firmware,
    ) -> Self {
        let licensing = StoreLicensing::new(
            LicenseStore::new(dir.path().join("state")),
            disphas_license::trusted_roots(),
        );
        let backend = ImageBackend {
            devices,
            dir: dir.path().to_path_buf(),
            firmware,
            gate: None,
            opened: Vec::new(),
            refuse: None,
        };
        let mut session = Session::new(
            backend,
            SessionConfig {
                rules: RuleSet::baseline(),
                entitlements: disphas_core::EntitlementTable::baseline(),
                binary_sha256: [0xb1; 32],
                reports_dir: dir.path().join("reports"),
                licensing: Box::new(licensing),
                jobs_dir: Some(dir.path().join("jobs")),
            },
        );
        let (sender, events) = mpsc::sync_channel(4096);
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
        bench
    }

    /// Simulates a restart: a new session over the same state and drives.
    fn restart(self, firmware: Firmware) -> Self {
        let devices = self.session.backend_mut_devices();
        Self::open(self.dir, devices, firmware)
    }

    /// Another helper on the same workstation (a second window of the
    /// application): same state directory, license and jobs directory.
    fn second_helper(&self, devices: Vec<Device>) -> Self {
        Self::open(std::sync::Arc::clone(&self.dir), devices, Firmware::Honest)
    }

    /// Files of jobs left in `jobs/` (the start guard stays for good).
    fn job_files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.dir.path().join("jobs"))
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|name| name != "start.guard")
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    fn send(
        &mut self,
        command: Command,
    ) -> Result<serde_json::Value, disphas_protocol::response::ErrorBody> {
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
        let verified = verify(&signed, &disphas_license::dev_root::verifying_key(), &[])
            .expect("attestation verifies against the dev root");
        assert!(verified.matches_journal(result.journal.as_bytes()));
        (verified, result)
    }

    fn progress(&self) -> Vec<JobProgressData> {
        self.events
            .try_iter()
            .filter_map(|message| match message {
                ServerMessage::Event(Event::JobProgress(data)) => Some(data),
                _ => None,
            })
            .collect()
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
        confirmation: None,
        acknowledged,
        force: None,
        operator: Some("J. Martin".to_owned()),
        organization: Some("Atelier XYZ".to_owned()),
        batch: None,
        zero_pass: false,
    }
}

/// Whether none of the original data (blocks of 0x5a) is left in the image.
fn image_is_erased(path: &Path) -> bool {
    std::fs::read(path)
        .expect("image")
        .chunks(512)
        .all(|block| block.iter().any(|&byte| byte != 0x5a))
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
    assert!(image_is_erased(&bench.dir.path().join("SN-A001.img")));
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
    assert!(
        attestation.sanitization.all_namespaces,
        "an NVMe Sanitize certifies every namespace"
    );
    assert_eq!(
        bench.session.backend_mut().opened,
        [("SN-A001".to_owned(), true)],
        "a Sanitize holds the whole controller"
    );
}

#[test]
fn a_drive_silent_on_its_media_type_is_certified_sata_not_ssd() {
    let silent = Inspection::Ata(disphas_core::profile::AtaProfile {
        solid_state: true,
        media_type_unknown: true,
        ..Default::default()
    });
    let mut bench = Bench::new(vec![drive("SN-S001", silent)], Firmware::Honest);
    let inspection = bench.inspect("SN-S001");
    let args = sanitize(
        &inspection,
        Some(Method::HostOverwrite),
        vec![Acknowledgement::Reserve(ReserveCode::HostWriteCoverage)],
    );
    let (verified, _) = bench.erase(args);
    assert_eq!(verified.attestation.device.drive_type, DriveType::Sata);
    assert_eq!(verified.attestation.sanitization.category, Category::Clear);
}

#[test]
fn a_firmware_command_starts_with_the_drive_estimate_only() {
    let mut bench = Bench::new(vec![drive("SN-T001", nvme())], Firmware::Honest);
    let inspection = bench.inspect("SN-T001");
    let args = sanitize(
        &inspection,
        None,
        vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
    );
    bench.erase(args);
    let progress = bench.progress();
    let firmware: Vec<(u8, Option<u64>)> = progress
        .iter()
        .filter(|p| p.source == disphas_protocol::event::ProgressSource::Firmware)
        .map(|p| (p.percent, p.remaining_seconds))
        .collect();
    assert_eq!(firmware, [(0, Some(78 * 60)), (100, None)]);
}

#[test]
fn stored_reports_are_listed_and_read_through_the_helper() {
    use disphas_protocol::command::{ListReportsArgs, ReportArgs};
    use disphas_protocol::response::{ListReportsResult, ReportResult};
    use sha2::Digest;
    let mut bench = Bench::new(vec![drive("SN-R001", nvme())], Firmware::Honest);
    let inspection = bench.inspect("SN-R001");
    let args = sanitize(
        &inspection,
        None,
        vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
    );
    let (_, result) = bench.erase(args);
    let id = result.job_id.0.clone();

    let page: ListReportsResult = bench.ok(Command::ListReports(ListReportsArgs { before: None }));
    assert!(!page.more);
    assert_eq!(page.reports.len(), 1);
    let listed = &page.reports[0];
    assert_eq!(listed.id, id);
    let attestation = base64::engine::general_purpose::STANDARD
        .decode(listed.attestation.as_deref().expect("readable"))
        .expect("base64");
    assert_eq!(
        attestation,
        base64::engine::general_purpose::STANDARD
            .decode(&result.attestation)
            .expect("b64")
    );
    let journal_hash: String = sha2::Sha256::digest(result.journal.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        listed.journal_sha256.as_deref(),
        Some(journal_hash.as_str())
    );

    let report: ReportResult = bench.ok(Command::GetReport(ReportArgs { id: id.clone() }));
    assert_eq!(report.journal, result.journal);
    assert_eq!(report.attestation, result.attestation);

    // Strict identifiers; a link is never followed.
    for bad in [
        "../keys/workstation",
        "01A11152-2F07-7973-B606-2CFAEEB58B2D",
    ] {
        let error = bench
            .send(Command::GetReport(ReportArgs { id: bad.to_owned() }))
            .expect_err("refused");
        assert_eq!(error.code, ErrorCode::BadRequest);
    }
    let reports = bench.dir.path().join("reports");
    let linked = "01a11152-0000-7000-8000-000000000001";
    std::os::unix::fs::symlink(
        reports.join(format!("{id}.cose")),
        reports.join(format!("{linked}.cose")),
    )
    .expect("link");
    std::os::unix::fs::symlink(
        reports.join(format!("{id}.json")),
        reports.join(format!("{linked}.json")),
    )
    .expect("link");
    assert!(
        bench
            .send(Command::GetReport(ReportArgs {
                id: linked.to_owned()
            }))
            .is_err()
    );
}

#[test]
fn stored_reports_come_in_pages_newest_first() {
    use disphas_protocol::command::ListReportsArgs;
    use disphas_protocol::response::ListReportsResult;
    let mut bench = Bench::new(Vec::new(), Firmware::Honest);
    let reports = bench.dir.path().join("reports");
    std::fs::create_dir_all(&reports).expect("reports");
    for index in 0..450u32 {
        let id = format!("01a11152-0000-7000-8000-{index:012}");
        std::fs::write(reports.join(format!("{id}.cose")), b"x").expect("cose");
        std::fs::write(reports.join(format!("{id}.json")), b"{}").expect("json");
    }
    let mut seen = Vec::new();
    let mut before = None;
    loop {
        let page: ListReportsResult = bench.ok(Command::ListReports(ListReportsArgs { before }));
        assert!(page.reports.len() <= 200);
        seen.extend(page.reports.iter().map(|report| report.id.clone()));
        if !page.more {
            break;
        }
        before = page.reports.last().map(|report| report.id.clone());
    }
    assert_eq!(seen.len(), 450);
    assert!(
        seen.windows(2).all(|pair| pair[0] > pair[1]),
        "newest first, no repeat"
    );
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
    assert!(
        !attestation.sanitization.all_namespaces,
        "a host write stays on its namespace"
    );
    assert!(image_is_erased(&bench.dir.path().join("SN-B002.img")));
    assert!(
        !bench.phases().contains(&JobPhase::Canaries),
        "no canaries for a host write"
    );
    assert_eq!(
        bench.session.backend_mut().opened,
        [("SN-B002".to_owned(), false)],
        "a host write stays on its namespace"
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
    assert!(image_is_erased(&bench.dir.path().join("SN-D004.img")));
}

#[test]
fn a_drive_held_or_protected_at_opening_is_refused_with_its_code() {
    // Found on the test bench (S9): these refusals came back as INTERNAL.
    for (refusal, code) in [
        (
            OpenError::InUse("sdz is open in another process (pid [4242])".to_owned()),
            ErrorCode::DeviceInUse,
        ),
        (
            OpenError::Protected("sdz is protected".to_owned()),
            ErrorCode::Protected,
        ),
    ] {
        let mut bench = Bench::new(vec![drive("SN-E010", nvme())], Firmware::Honest);
        let inspection = bench.inspect("SN-E010");
        let args = sanitize(
            &inspection,
            None,
            vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
        );
        bench.session.backend_mut().refuse = Some(refusal.clone());
        let error = bench
            .send(Command::Sanitize(args.clone()))
            .expect_err("refused at opening");
        assert_eq!(error.code, code, "{refusal:?}");
        assert_eq!(error.detail.as_deref(), Some(refusal.to_string().as_str()));
        // Nothing started: once the cause is gone, the same verdict erases.
        assert!(bench.reports().is_empty());
        bench.session.backend_mut().refuse = None;
        bench.erase(args);
    }
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
fn an_erasure_invalidates_every_verdict_of_its_drive() {
    let mut bench = Bench::new(
        vec![drive("SN-E006", nvme()), drive("SN-E007", nvme())],
        Firmware::Honest,
    );
    let first = bench.inspect("SN-E006");
    let second = bench.inspect("SN-E006");
    let other = bench.inspect("SN-E007");
    let ack = || vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)];
    bench.erase(sanitize(&first, None, ack()));
    let error = bench
        .send(Command::Sanitize(sanitize(&second, None, ack())))
        .expect_err("issued before the erasure");
    assert_eq!(error.code, ErrorCode::VerdictExpired);
    // Another drive's verdict is untouched.
    bench.erase(sanitize(&other, None, ack()));
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

impl Session<ImageBackend> {
    fn backend_mut_devices(mut self) -> Vec<Device> {
        self.backend_mut().devices.clone()
    }
}

#[test]
fn finished_job_leaves_no_job_file() {
    let mut bench = Bench::new(vec![drive("SN-J001", nvme())], Firmware::Honest);
    let inspection = bench.inspect("SN-J001");
    let args = sanitize(
        &inspection,
        None,
        vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
    );
    let (_, result) = bench.erase(args);
    assert_eq!(result.status, FinalStatus::Compliant);
    assert!(bench.job_files().is_empty(), "{:?}", bench.job_files());
}

#[test]
fn sanitize_interrupted_by_a_crash_is_finished_and_certified() {
    let mut bench = Bench::new(vec![drive("SN-C001", nvme())], Firmware::Crash);
    let inspection = bench.inspect("SN-C001");
    let args = sanitize(
        &inspection,
        None,
        vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
    );
    let started: SanitizeResult = bench.ok(Command::Sanitize(args));
    bench.session.wait_for_jobs();
    let left = bench.job_files();
    assert_eq!(
        left.len(),
        2,
        "state and lock stay after the crash: {left:?}"
    );
    let state: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            bench
                .dir
                .path()
                .join("jobs")
                .join(format!("{}.json", started.job_id.0)),
        )
        .expect("job file"),
    )
    .expect("json");
    assert_eq!(state["phase"], "Executing");
    assert!(
        !bench
            .reports()
            .iter()
            .any(|path| path.extension().is_some_and(|e| e == "cose"))
    );
    // The panic ended the job as a failure: the drive is not left busy.
    let result = bench.send(Command::GetJobResult(JobArgs {
        job_id: started.job_id.clone(),
    }));
    assert_ne!(
        result.map(|_| ()).map_err(|error| error.code),
        Err(ErrorCode::Busy),
        "the job is no longer running"
    );
    assert!(
        !bench.session.drive_busy_for_tests("SN-C001"),
        "the drive is free again"
    );

    let mut bench = bench.restart(Firmware::Honest);
    let _: serde_json::Value = bench.ok(Command::ListJobs);
    bench.session.wait_for_jobs();
    let result: JobResult = bench.ok(Command::GetJobResult(JobArgs {
        job_id: started.job_id.clone(),
    }));
    assert_eq!(result.status, FinalStatus::Compliant);
    let signed = base64::engine::general_purpose::STANDARD
        .decode(&result.attestation)
        .expect("base64");
    let verified = verify(&signed, &disphas_license::dev_root::verifying_key(), &[])
        .expect("attestation verifies");
    assert_eq!(verified.attestation.sanitization.category, Category::Purge);
    assert!(verified.attestation.validation.is_some_and(|v| v.passed));
    assert!(result.journal.contains("\"resumed\""));
    assert!(image_is_erased(&bench.dir.path().join("SN-C001.img")));
    assert!(bench.job_files().is_empty(), "{:?}", bench.job_files());
    assert_eq!(
        bench.session.backend_mut().opened,
        [("SN-C001".to_owned(), true)],
        "a resumed Sanitize holds the whole controller again"
    );
}

#[test]
fn resumed_sanitize_that_did_not_erase_is_not_certified() {
    let mut bench = Bench::new(vec![drive("SN-C002", nvme())], Firmware::Crash);
    let inspection = bench.inspect("SN-C002");
    let args = sanitize(
        &inspection,
        None,
        vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
    );
    let started: SanitizeResult = bench.ok(Command::Sanitize(args));
    bench.session.wait_for_jobs();

    // After the restart the drive says the Sanitize succeeded, but the
    // canaries are still there: retried, then refused, never CONFORME.
    let mut bench = bench.restart(Firmware::Lying);
    let _: serde_json::Value = bench.ok(Command::ListJobs);
    bench.session.wait_for_jobs();
    let result: JobResult = bench.ok(Command::GetJobResult(JobArgs {
        job_id: started.job_id,
    }));
    assert_eq!(result.status, FinalStatus::Refused);
}

#[test]
fn crash_during_a_non_resumable_method_is_interrupted() {
    let mut bench = Bench::new(vec![drive("SN-C003", nvme_format_only())], Firmware::Crash);
    let inspection = bench.inspect("SN-C003");
    let started: SanitizeResult = bench.ok(Command::Sanitize(sanitize(&inspection, None, vec![])));
    bench.session.wait_for_jobs();
    assert_eq!(bench.job_files().len(), 2);

    let mut bench = bench.restart(Firmware::Honest);
    let jobs: disphas_protocol::response::ListJobsResult = bench.ok(Command::ListJobs);
    assert_eq!(jobs.jobs.len(), 1);
    assert_eq!(jobs.jobs[0].phase, JobPhase::Interrupted);
    assert_eq!(jobs.jobs[0].status, None);
    let error = bench
        .send(Command::GetJobResult(JobArgs {
            job_id: started.job_id,
        }))
        .expect_err("no attestation");
    assert_eq!(error.code, ErrorCode::JobInterrupted);
    assert!(bench.job_files().is_empty());
    assert!(bench.reports().is_empty());
}

#[test]
fn different_drives_are_erased_in_parallel_but_a_drive_only_once() {
    let mut bench = Bench::new(
        vec![drive("SN-P001", nvme()), drive("SN-P002", nvme())],
        Firmware::Honest,
    );
    let gate = std::sync::Arc::new(Gate::default());
    bench.session.backend_mut().gate = Some(std::sync::Arc::clone(&gate));
    let ack = || vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)];

    let first = bench.inspect("SN-P001");
    let one: SanitizeResult = bench.ok(Command::Sanitize(sanitize(&first, None, ack())));

    // The first job is held by its firmware: the same drive is refused…
    let again = bench.inspect("SN-P001");
    let error = bench
        .send(Command::Sanitize(sanitize(&again, None, ack())))
        .expect_err("drive busy");
    assert_eq!(error.code, ErrorCode::Busy);

    // …while another drive starts at once.
    let second = bench.inspect("SN-P002");
    let two: SanitizeResult = bench.ok(Command::Sanitize(sanitize(&second, None, ack())));
    let running: disphas_protocol::response::ListJobsResult = bench.ok(Command::ListJobs);
    assert_eq!(
        running
            .jobs
            .iter()
            .filter(|job| job.status.is_none())
            .count(),
        2,
        "both jobs run at the same time"
    );

    gate.release();
    bench.session.wait_for_jobs();
    for job in [one.job_id, two.job_id] {
        let result: JobResult = bench.ok(Command::GetJobResult(JobArgs { job_id: job }));
        assert_eq!(result.status, FinalStatus::Compliant);
    }
    assert!(image_is_erased(&bench.dir.path().join("SN-P001.img")));
    assert!(image_is_erased(&bench.dir.path().join("SN-P002.img")));
}

#[test]
fn batch_is_certified_in_the_attestation() {
    let mut bench = Bench::new(
        vec![drive("SN-L001", nvme()), drive("SN-L002", nvme())],
        Firmware::Honest,
    );
    let inspection = bench.inspect("SN-L001");
    let mut args = sanitize(
        &inspection,
        None,
        vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
    );
    args.batch = Some(disphas_protocol::command::BatchArgs {
        id: "LOT-2026-001".to_owned(),
        label: Some("PO 4512".to_owned()),
    });
    let (verified, _) = bench.erase(args);
    let batch = verified.attestation.batch.expect("batch certified");
    assert_eq!(batch.id, "LOT-2026-001");
    assert_eq!(batch.label.as_deref(), Some("PO 4512"));

    let other = bench.inspect("SN-L002");
    let mut invalid = sanitize(
        &other,
        None,
        vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
    );
    invalid.batch = Some(disphas_protocol::command::BatchArgs {
        id: "not a valid id".to_owned(),
        label: None,
    });
    let error = bench
        .send(Command::Sanitize(invalid))
        .expect_err("invalid batch refused");
    assert_eq!(error.code, ErrorCode::BadRequest);
}

#[test]
fn reports_are_deleted_on_request_and_logged() {
    let mut bench = Bench::new(vec![drive("SN-R001", nvme())], Firmware::Honest);
    let inspection = bench.inspect("SN-R001");
    let (_, result) = bench.erase(sanitize(
        &inspection,
        None,
        vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
    ));
    let id = result.job_id.0.clone();
    assert_eq!(bench.reports().len(), 2);

    let deleted: disphas_protocol::response::DeleteReportsResult = bench.ok(
        Command::DeleteReports(disphas_protocol::command::DeleteReportsArgs {
            ids: vec![
                id.clone(),
                "01928c3e-5a1b-7000-8000-00000000ffff".to_owned(),
            ],
        }),
    );
    assert_eq!(deleted.deleted, 1, "an absent report is not counted");
    assert!(bench.reports().is_empty());
    let log = std::fs::read_to_string(bench.dir.path().join("reports-deletions.log")).expect("log");
    assert!(log.trim_end().ends_with(&format!("deleted {id}")));
    assert!(!log.contains("SN-R001"), "no personal data in the log");

    let error = bench
        .send(Command::DeleteReports(
            disphas_protocol::command::DeleteReportsArgs {
                ids: vec!["../../etc/passwd".to_owned()],
            },
        ))
        .expect_err("refused");
    assert_eq!(error.code, ErrorCode::BadRequest);
}

#[test]
fn zero_finish_proves_both_passes_and_leaves_a_blank_drive() {
    let mut bench = Bench::new(
        vec![drive("SN-Z001", nvme()), drive("SN-Z002", nvme())],
        Firmware::Honest,
    );
    let inspection = bench.inspect("SN-Z001");
    let mut args = sanitize(
        &inspection,
        Some(Method::HostOverwrite),
        vec![Acknowledgement::Reserve(ReserveCode::HostWriteCoverage)],
    );
    args.zero_pass = true;
    let (verified, result) = bench.erase(args);
    assert_eq!(verified.attestation.status, Status::Compliant);
    assert!(verified.attestation.sanitization.zero_finish);
    let image = std::fs::read(bench.dir.path().join("SN-Z001.img")).expect("image");
    assert!(
        image.iter().all(|&byte| byte == 0),
        "blank after the zero pass"
    );
    let _ = result;

    // With a firmware method, the option is refused.
    let other = bench.inspect("SN-Z002");
    let mut firmware = sanitize(
        &other,
        None,
        vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)],
    );
    firmware.zero_pass = true;
    let error = bench
        .send(Command::Sanitize(firmware))
        .expect_err("refused");
    assert_eq!(error.code, ErrorCode::BadRequest);
}

#[test]
fn a_zero_pass_that_fails_validation_is_never_certified() {
    let mut bench = Bench::new(vec![drive("SN-Z003", nvme())], Firmware::DropsZeros);
    let inspection = bench.inspect("SN-Z003");
    let mut args = sanitize(
        &inspection,
        Some(Method::HostOverwrite),
        vec![Acknowledgement::Reserve(ReserveCode::HostWriteCoverage)],
    );
    args.zero_pass = true;
    let (verified, result) = bench.erase(args);
    assert_eq!(result.status, FinalStatus::Refused);
    assert_eq!(verified.attestation.status, Status::Refused);
    assert!(
        !verified.attestation.sanitization.zero_finish,
        "z only when the zero pass is validated"
    );
    assert!(verified.attestation.validation.is_some_and(|v| !v.passed));
}

#[test]
fn operator_and_organization_are_bounded_and_trimmed() {
    let mut bench = Bench::new(
        vec![drive("SN-O001", nvme()), drive("SN-O002", nvme())],
        Firmware::Honest,
    );
    let ack = || vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)];
    let inspection = bench.inspect("SN-O001");
    let mut args = sanitize(&inspection, None, ack());
    args.operator = Some("  OP-042  ".to_owned());
    args.organization = Some("   ".to_owned());
    let (verified, _) = bench.erase(args);
    assert_eq!(verified.attestation.operator.as_deref(), Some("OP-042"));
    assert_eq!(
        verified.attestation.organization, None,
        "blank means absent"
    );

    let other = bench.inspect("SN-O002");
    for (operator, organization) in [
        (Some("x".repeat(65)), None),
        (None, Some("y".repeat(129))),
        (Some("line\nbreak".to_owned()), None),
    ] {
        let mut args = sanitize(&other, None, ack());
        args.operator = operator;
        args.organization = organization;
        let error = bench.send(Command::Sanitize(args)).expect_err("refused");
        assert_eq!(error.code, ErrorCode::BadRequest);
    }
}

/// `docs/offres.md` §4.4: `Pro` erases two drives at once; the third waits.
#[test]
fn pro_refuses_a_third_erasure_at_once_then_accepts_it() {
    let mut bench = Bench::with_offer(
        vec![
            drive("SN-Q001", nvme()),
            drive("SN-Q002", nvme()),
            drive("SN-Q003", nvme()),
        ],
        Firmware::Honest,
        "Pro",
    );
    let status: LicenseStatusResult = bench.ok(Command::LicenseStatus);
    assert_eq!(
        status.entitlements,
        Some(Entitlements {
            max_parallel: Some(2),
            customer_logo: false,
        })
    );
    let gate = std::sync::Arc::new(Gate::default());
    bench.session.backend_mut().gate = Some(std::sync::Arc::clone(&gate));
    let ack = || vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)];

    for serial in ["SN-Q001", "SN-Q002"] {
        let inspection = bench.inspect(serial);
        let _: SanitizeResult = bench.ok(Command::Sanitize(sanitize(&inspection, None, ack())));
    }
    let third = bench.inspect("SN-Q003");
    let error = bench
        .send(Command::Sanitize(sanitize(&third, None, ack())))
        .expect_err("limit reached");
    assert_eq!(error.code, ErrorCode::ParallelLimit);
    assert_eq!(
        error.detail.as_deref(),
        Some("2 erasures running, the offer allows 2")
    );

    // Once the running erasures end, the same verdict is still usable:
    // the refusal consumed nothing.
    gate.release();
    bench.session.wait_for_jobs();
    let started: SanitizeResult = bench.ok(Command::Sanitize(sanitize(&third, None, ack())));
    bench.session.wait_for_jobs();
    let result: JobResult = bench.ok(Command::GetJobResult(JobArgs {
        job_id: started.job_id,
    }));
    assert_eq!(result.status, FinalStatus::Compliant);
}

/// `Parc` has no limit: more drives at once than the largest Atelier offer.
#[test]
fn parc_erases_more_than_thirty_two_drives_at_once() {
    let serials: Vec<String> = (1..=33).map(|n| format!("SN-U{n:03}")).collect();
    let mut bench = Bench::with_offer(
        serials.iter().map(|serial| drive(serial, nvme())).collect(),
        Firmware::Honest,
        "Parc",
    );
    let gate = std::sync::Arc::new(Gate::default());
    bench.session.backend_mut().gate = Some(std::sync::Arc::clone(&gate));
    let ack = || vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)];
    for serial in &serials {
        let inspection = bench.inspect(serial);
        let _: SanitizeResult = bench.ok(Command::Sanitize(sanitize(&inspection, None, ack())));
    }
    let running: disphas_protocol::response::ListJobsResult = bench.ok(Command::ListJobs);
    assert_eq!(
        running
            .jobs
            .iter()
            .filter(|job| job.status.is_none())
            .count(),
        33
    );
    gate.release();
    bench.session.wait_for_jobs();
}

/// A job resumed after a crash takes one of the places of the offer.
#[test]
fn a_resumed_erasure_counts_against_the_limit() {
    let devices = vec![
        drive("SN-V001", nvme()),
        drive("SN-V002", nvme()),
        drive("SN-V003", nvme()),
    ];
    let mut bench = Bench::with_offer(devices, Firmware::Crash, "Pro");
    let ack = || vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)];
    let crashed = bench.inspect("SN-V001");
    let _: SanitizeResult = bench.ok(Command::Sanitize(sanitize(&crashed, None, ack())));
    bench.session.wait_for_jobs();

    // After the restart, the drive carries on: the resumed job is running.
    let mut bench = bench.restart(Firmware::Honest);
    let gate = std::sync::Arc::new(Gate::default());
    bench.session.backend_mut().gate = Some(std::sync::Arc::clone(&gate));
    let _: serde_json::Value = bench.ok(Command::ListJobs);

    let second = bench.inspect("SN-V002");
    let _: SanitizeResult = bench.ok(Command::Sanitize(sanitize(&second, None, ack())));
    let third = bench.inspect("SN-V003");
    let error = bench
        .send(Command::Sanitize(sanitize(&third, None, ack())))
        .expect_err("resumed job and second erasure fill the offer");
    assert_eq!(error.code, ErrorCode::ParallelLimit);

    gate.release();
    bench.session.wait_for_jobs();
    let _: SanitizeResult = bench.ok(Command::Sanitize(sanitize(&third, None, ack())));
    bench.session.wait_for_jobs();
}

/// The limit holds for the workstation: a second helper (second window of the
/// application) sees the erasures of the first (`docs/offres.md` §4.4).
#[test]
fn the_limit_counts_the_erasures_of_other_helpers() {
    let mut first = Bench::with_offer(vec![drive("SN-W001", nvme())], Firmware::Honest, "Pro");
    let mut second = first.second_helper(vec![drive("SN-W002", nvme()), drive("SN-W003", nvme())]);
    let gate = std::sync::Arc::new(Gate::default());
    first.session.backend_mut().gate = Some(std::sync::Arc::clone(&gate));
    second.session.backend_mut().gate = Some(std::sync::Arc::clone(&gate));
    let ack = || vec![Acknowledgement::Scope(ScopeWarning::AllNamespaces)];

    let one = first.inspect("SN-W001");
    let _: SanitizeResult = first.ok(Command::Sanitize(sanitize(&one, None, ack())));
    let two = second.inspect("SN-W002");
    let _: SanitizeResult = second.ok(Command::Sanitize(sanitize(&two, None, ack())));
    let three = second.inspect("SN-W003");
    let error = second
        .send(Command::Sanitize(sanitize(&three, None, ack())))
        .expect_err("one erasure in each helper fills the offer");
    assert_eq!(error.code, ErrorCode::ParallelLimit);
    assert_eq!(
        error.detail.as_deref(),
        Some("2 erasures running, the offer allows 2")
    );

    gate.release();
    first.session.wait_for_jobs();
    second.session.wait_for_jobs();
    let _: SanitizeResult = second.ok(Command::Sanitize(sanitize(&three, None, ack())));
    second.session.wait_for_jobs();
}
