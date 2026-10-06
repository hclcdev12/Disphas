//! An erasure job: execution, verification, validation and signing, run on
//! its own thread while the session keeps answering (`docs/protocole_ipc.md` §7, §8).
//!
//! Phases: Preparing → Canaries (firmware methods) → Executing → Verifying →
//! Validating → Signing → Done. A failed check is retried as the rule set
//! allows (`evaluate_attempt`); whatever happens, the job ends with a signed
//! attestation (CONFORME, NON CONFORME or REFUS).
//!
//! With a jobs directory, the state is kept on disk at every phase so a
//! firmware Sanitize can be finished after a crash ([`crate::jobfile`]).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use oblivio_attestation::Attestation;
use oblivio_attestation::display::uuid;
use oblivio_attestation::model::{
    Category as AttCategory, Code, Device, Disposition as AttDisposition, DispositionKind,
    DriveType, Sanitization, Status, Tool, Validation as AttValidation, ValidationMethod,
};
use oblivio_core::RuleSet;
use oblivio_core::force::Forced;
use oblivio_core::method::{Category, MethodOption};
use oblivio_core::outcome::{
    AttemptReport, Evaluation, Validation, evaluate_attempt, evaluate_forced_attempt,
};
use oblivio_core::profile::{DeviceProfile, Inspection};
use oblivio_core::verdict::{Disposition, Ready};
use oblivio_protocol::ServerMessage;
use oblivio_protocol::command::{DeviceRef, JobId};
use oblivio_protocol::event::{
    Event, JobFinishedData, JobLogData, JobPhaseData, JobProgressData, LogLevel, ProgressSource,
};
use oblivio_protocol::response::{FinalStatus, JobPhase};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::erasure::{self, Check, ErasureTarget, HostProgress};
use crate::jobfile::{AttemptFile, JobFile, JobHandle, Left};
use crate::licensing::Signer;

/// What the job executes: a READY method, or a forced SAY NO.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Plan {
    /// A method allowed by a READY verdict.
    Ready {
        /// The verdict.
        ready: Ready,
        /// The method chosen by the operator.
        option: MethodOption,
    },
    /// A forced erasure, certified NON CONFORME at best.
    Forced(Forced),
}

impl Plan {
    pub(crate) fn option(&self) -> MethodOption {
        match self {
            Self::Ready { option, .. } => *option,
            Self::Forced(forced) => forced.option,
        }
    }

    /// Whether the method erases every namespace of the NVMe controller.
    /// A forced plan is a host write: one namespace only.
    pub(crate) fn erases_all_namespaces(&self) -> bool {
        match self {
            Self::Ready { ready, option } => ready.erases_all_namespaces(option.method),
            Self::Forced(_) => false,
        }
    }
}

/// Everything a job needs, gathered once every Sanitize check passed.
pub struct JobSpec {
    /// Report identifier (UUID v7), also the job identifier.
    pub id: [u8; 16],
    /// The drive as named on the wire.
    pub device: DeviceRef,
    /// Its profile at inspection.
    pub profile: DeviceProfile,
    /// Capacity reported by the backend.
    pub capacity_bytes: Option<u64>,
    /// What to execute.
    pub plan: Plan,
    /// Rule set of the engine.
    pub rules: RuleSet,
    /// Workstation key and license.
    pub signer: Signer,
    /// The opened drive.
    pub target: Box<dyn ErasureTarget>,
    /// Declarative operator name.
    pub operator: Option<String>,
    /// Declarative organization.
    pub organization: Option<String>,
    /// Batch the drive is erased in.
    pub batch: Option<oblivio_attestation::model::Batch>,
    /// Host overwrite only: end with a validated pass of zeros.
    pub zero_pass: bool,
    /// SHA-256 of the helper binary.
    pub binary_sha256: [u8; 32],
    /// Where signed reports are stored.
    pub reports_dir: PathBuf,
    /// Where job state is kept for crash recovery; `None` keeps nothing.
    pub jobs_dir: Option<PathBuf>,
}

/// State of a job, as listed by `ListJobs` and `GetJobResult`.
#[derive(Debug, Clone)]
pub struct JobRecord {
    /// The drive.
    pub device: DeviceRef,
    /// Current phase.
    pub phase: JobPhase,
    /// Final status, once finished.
    pub status: Option<FinalStatus>,
    /// Signed attestation (COSE) and journal (JSON), once finished.
    pub result: Option<(Vec<u8>, Vec<u8>)>,
    /// Why the job could not produce an attestation, if it failed.
    pub failure: Option<String>,
}

impl JobRecord {
    /// Whether the job is still running.
    pub fn running(&self) -> bool {
        self.status.is_none() && self.failure.is_none()
    }
}

/// Jobs of the session, shared with their threads.
pub type JobTable = Arc<Mutex<HashMap<JobId, JobRecord>>>;

/// A new UUID v7: 48-bit Unix time in milliseconds, then random bits.
pub fn uuid_v7() -> Result<[u8; 16], String> {
    let mut id = [0u8; 16];
    getrandom::fill(&mut id).map_err(|error| format!("random generator: {error}"))?;
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64);
    id[..6].copy_from_slice(&millis.to_be_bytes()[2..]);
    id[6] = 0x70 | (id[6] & 0x0f);
    id[8] = 0x80 | (id[8] & 0x3f);
    Ok(id)
}

/// Starts the job on its own thread; `resumed` is the state left by a
/// crashed helper, whose firmware Sanitize is to be finished.
pub fn spawn(
    spec: JobSpec,
    jobs: JobTable,
    events: Option<SyncSender<ServerMessage>>,
    resumed: Option<Left>,
) -> JoinHandle<()> {
    let job_id = JobId(uuid(&spec.id));
    let phase = resumed
        .as_ref()
        .map_or(JobPhase::Preparing, |left| left.file.phase);
    if let Ok(mut table) = jobs.lock() {
        table.insert(
            job_id.clone(),
            JobRecord {
                device: spec.device.clone(),
                phase,
                status: None,
                result: None,
                failure: None,
            },
        );
    }
    std::thread::spawn(move || {
        let mut runner = Runner {
            job_id: job_id.clone(),
            jobs: Arc::clone(&jobs),
            events,
            timeline: Vec::new(),
            persist: None,
        };
        // A panic must not leave the drive "running" for the whole session:
        // it ends the job as a failure, without attestation, and its files
        // stay for the next start (§10), like a crash.
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runner.run(spec, resumed)))
                .unwrap_or_else(|_| Err("internal error: the job panicked".to_owned()));
        // Files of a job without attestation stay for the next start (§10).
        if outcome.is_ok()
            && let Some((handle, _)) = runner.persist.take()
            && let Err(error) = handle.finish()
        {
            eprintln!("oblivio-helper: job {}: {error}", job_id.0);
        }
        // A panic while the table was held poisons it; the record is still
        // what this job last wrote, so it is updated and the poison cleared.
        let mut table = jobs.lock().unwrap_or_else(|poisoned| {
            jobs.clear_poison();
            poisoned.into_inner()
        });
        let Some(record) = table.get_mut(&job_id) else {
            return;
        };
        match outcome {
            Ok((status, signed, journal)) => {
                record.phase = JobPhase::Done;
                record.status = Some(status);
                record.result = Some((signed, journal));
                drop(table);
                runner.emit(Event::JobPhase(JobPhaseData {
                    job_id: job_id.clone(),
                    phase: JobPhase::Done,
                }));
                runner.emit(Event::JobFinished(JobFinishedData { job_id, status }));
            }
            Err(failure) => {
                eprintln!("oblivio-helper: job {} failed: {failure}", job_id.0);
                record.failure = Some(failure.clone());
                drop(table);
                runner.log(LogLevel::Error, "job_failed", &[("reason", failure)]);
            }
        }
    })
}

struct Runner {
    job_id: JobId,
    jobs: JobTable,
    events: Option<SyncSender<ServerMessage>>,
    timeline: Vec<serde_json::Value>,
    /// The job's files and their last content.
    persist: Option<(JobHandle, JobFile)>,
}

/// One attempt, kept for the journal.
type Attempt = AttemptFile;

impl Runner {
    /// Never blocks: when the output queue is full (the UI does not read),
    /// the event is dropped and the job goes on; its state stays available
    /// through `ListJobs` (`docs/protocole_ipc.md` §3).
    fn emit(&self, event: Event) {
        if let Some(events) = &self.events {
            let _ = events.try_send(ServerMessage::Event(event));
        }
    }

    fn log(&self, level: LogLevel, code: &str, params: &[(&str, String)]) {
        self.emit(Event::JobLog(JobLogData {
            job_id: self.job_id.clone(),
            level,
            code: code.to_owned(),
            params: params
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect(),
        }));
    }

    fn phase(&mut self, phase: JobPhase) {
        if let Ok(mut table) = self.jobs.lock()
            && let Some(record) = table.get_mut(&self.job_id)
        {
            record.phase = phase;
        }
        self.timeline
            .push(json!({ "phase": format!("{phase:?}"), "at": now() }));
        let timeline = self.timeline.clone();
        self.save(|file| {
            file.phase = phase;
            file.timeline = timeline;
        });
        self.emit(Event::JobPhase(JobPhaseData {
            job_id: self.job_id.clone(),
            phase,
        }));
    }

    /// Updates the job file. A failure is logged: the job goes on, but may
    /// not be recoverable after a crash.
    fn save(&mut self, update: impl FnOnce(&mut JobFile)) {
        let Some((handle, file)) = &mut self.persist else {
            return;
        };
        update(file);
        if let Err(error) = handle.save(file) {
            eprintln!("oblivio-helper: job {}: {error}", self.job_id.0);
            self.log(LogLevel::Warning, "job_state_not_saved", &[]);
        }
    }

    fn progress(&self, percent: u8, source: ProgressSource, host: Option<(HostProgress, f64)>) {
        self.emit(Event::JobProgress(JobProgressData {
            job_id: self.job_id.clone(),
            percent,
            source,
            bytes_done: host.map(|(progress, _)| progress.bytes_done),
            bytes_per_second: host.map(|(progress, seconds)| {
                (progress.bytes_done as f64 / seconds.max(0.001)) as u64
            }),
            remaining_seconds: host.map(|(progress, seconds)| {
                let rate = progress.bytes_done as f64 / seconds.max(0.001);
                ((progress.bytes_total - progress.bytes_done) as f64 / rate.max(1.0)) as u64
            }),
        }));
    }

    fn run(
        &mut self,
        mut spec: JobSpec,
        resumed: Option<Left>,
    ) -> Result<(FinalStatus, Vec<u8>, Vec<u8>), String> {
        let option = spec.plan.option();
        let method = option.method;
        let host = method.is_host_write();
        let check = if host {
            Check::Pattern
        } else {
            Check::NoCanary
        };
        let geometry = {
            let io = spec.target.io();
            (io.block_size(), io.block_count())
        };
        let (seed, lbas, mut attempts, mut resuming) = match resumed {
            Some(Left { file, handle }) => {
                let seed = unhex(&file.seed).ok_or("jobs: invalid seed")?;
                let lbas = file.samples.clone();
                let attempts = file.attempts.clone();
                self.timeline = file.timeline.clone();
                self.timeline
                    .push(json!({ "event": "resumed", "at": now() }));
                self.persist = Some((handle, file));
                self.log(LogLevel::Warning, "job_resumed", &[]);
                (seed, lbas, attempts, true)
            }
            None => {
                self.phase(JobPhase::Preparing);
                let seed = erasure::random_seed()?;
                let random = spec
                    .rules
                    .validation_samples
                    .max(spec.rules.min_validation_samples);
                let lbas = erasure::samples(&seed, geometry.1, random);
                if let Some(dir) = &spec.jobs_dir {
                    // Recorded before anything is written to the drive.
                    let file = job_file(&spec, &seed, &lbas, &self.timeline);
                    let handle = JobHandle::create(dir, &file)?;
                    self.persist = Some((handle, file));
                }
                (seed, lbas, Vec::new(), false)
            }
        };
        self.log(
            LogLevel::Info,
            "samples_selected",
            &[("count", lbas.len().to_string())],
        );

        let evaluation = loop {
            let retries_used = u8::try_from(attempts.len()).unwrap_or(u8::MAX);
            let attempt = self.attempt(&mut spec, method, check, &seed, &lbas, resuming);
            resuming = false;
            let evaluation = match &spec.plan {
                Plan::Ready { ready, .. } => evaluate_attempt(
                    &spec.profile,
                    ready,
                    method,
                    retries_used,
                    &attempt.report,
                    &spec.rules,
                )
                .map_err(|error| error.to_string())?,
                Plan::Forced(forced) => {
                    evaluate_forced_attempt(forced, retries_used, &attempt.report, &spec.rules)
                }
            };
            attempts.push(attempt);
            let recorded = attempts.clone();
            self.save(|file| file.attempts = recorded);
            if evaluation == Evaluation::Retry {
                self.log(LogLevel::Warning, "attempt_retry", &[]);
                continue;
            }
            break evaluation;
        };

        self.phase(JobPhase::Signing);
        let last = attempts.last().map(|attempt| attempt.report);
        let journal = self.journal(&spec, geometry, &seed, &lbas, &attempts);
        let journal = serde_json::to_vec_pretty(&journal).map_err(|error| error.to_string())?;
        let (status, attestation) = attestation(
            &spec,
            &evaluation,
            option,
            last,
            &seed,
            lbas.len(),
            &journal,
        )?;
        // Never sign what a verifier would reject as contradictory.
        attestation
            .check_consistency()
            .map_err(|error| format!("attestation: {error}"))?;
        let signed = spec
            .signer
            .key
            .sign(attestation.to_cbor())
            .map_err(|error| error.to_string())?;
        store(&spec.reports_dir, &uuid(&spec.id), &signed, &journal)?;
        Ok((status, signed, journal))
    }

    fn attempt(
        &mut self,
        spec: &mut JobSpec,
        method: oblivio_core::method::Method,
        check: Check,
        seed: &[u8; 32],
        lbas: &[u64],
        resuming: bool,
    ) -> Attempt {
        let host = method.is_host_write();
        let mut error = None;

        // A resumed Sanitize already has its canaries, and is running or done.
        if !host && !resuming {
            self.phase(JobPhase::Canaries);
            if let Err(failure) = erasure::write_canaries(spec.target.io(), seed, lbas) {
                return Attempt {
                    report: AttemptReport {
                        verification_passed: false,
                        validation: Validation::ReadErrors,
                        smart_after: spec.target.smart_after(),
                    },
                    error: Some(format!("canaries: {failure}")),
                };
            }
        }

        self.phase(JobPhase::Executing);
        let started = Instant::now();
        // With the zero finish, the pseudo-random pass is validated before
        // the zero pass erases its trace: both passes are proven.
        let mut first_pass = Validation::Passed;
        let verification_passed = if host {
            let passes: u64 = if spec.zero_pass { 2 } else { 1 };
            let mut last_percent = None;
            let mut report = |pass: u64, progress: HostProgress| {
                let total = progress.bytes_total.max(1) * passes;
                let done = progress.bytes_total * pass + progress.bytes_done;
                let percent = (done * 100 / total) as u8;
                if last_percent != Some(percent) {
                    last_percent = Some(percent);
                    let seconds = started.elapsed().as_secs_f64();
                    let overall = HostProgress {
                        bytes_done: done,
                        bytes_total: total,
                    };
                    self.progress(percent, ProgressSource::Host, Some((overall, seconds)));
                }
            };
            let mut result = erasure::host_overwrite(spec.target.io(), seed, &mut |progress| {
                report(0, progress)
            });
            if result.is_ok() && spec.zero_pass {
                first_pass = erasure::validate(spec.target.io(), seed, lbas, Check::Pattern);
                self.log(
                    if first_pass == Validation::Passed {
                        LogLevel::Info
                    } else {
                        LogLevel::Error
                    },
                    "validation_random_pass",
                    &[("result", format!("{first_pass:?}"))],
                );
                result = erasure::zero_fill(spec.target.io(), &mut |progress| report(1, progress));
            }
            result
                .map_err(|failure| error = Some(format!("write: {failure}")))
                .is_ok()
        } else {
            let job_id = self.job_id.clone();
            let events = self.events.clone();
            // A command that reports nothing until it ends: the drive's own
            // estimate goes with its start, for the operator.
            let estimate = if resuming {
                None
            } else {
                spec.target.firmware_estimate(method)
            };
            let mut report = |percent: u8| {
                if let Some(events) = &events {
                    let _ = events.try_send(ServerMessage::Event(Event::JobProgress(
                        JobProgressData {
                            job_id: job_id.clone(),
                            percent,
                            source: ProgressSource::Firmware,
                            bytes_done: None,
                            bytes_per_second: None,
                            remaining_seconds: estimate.filter(|_| percent == 0),
                        },
                    )));
                }
            };
            let outcome = if resuming {
                spec.target.resume_firmware(method, &mut report)
            } else {
                spec.target.firmware(method, &mut report)
            };
            match outcome {
                Ok(passed) => passed,
                Err(failure) => {
                    error = Some(format!("firmware: {failure}"));
                    false
                }
            }
        };

        self.phase(JobPhase::Verifying);
        self.log(
            if verification_passed {
                LogLevel::Info
            } else {
                LogLevel::Error
            },
            "verification",
            &[("passed", verification_passed.to_string())],
        );

        self.phase(JobPhase::Validating);
        let final_check = if host && spec.zero_pass {
            Check::Zeros
        } else {
            check
        };
        let validation = match erasure::validate(spec.target.io(), seed, lbas, final_check) {
            Validation::Passed => first_pass,
            failed => failed,
        };
        self.log(
            if validation == Validation::Passed {
                LogLevel::Info
            } else {
                LogLevel::Error
            },
            "validation",
            &[("result", format!("{validation:?}"))],
        );

        Attempt {
            report: AttemptReport {
                verification_passed,
                validation,
                smart_after: spec.target.smart_after(),
            },
            error,
        }
    }

    fn journal(
        &self,
        spec: &JobSpec,
        (block_size, block_count): (u64, u64),
        seed: &[u8; 32],
        lbas: &[u64],
        attempts: &[Attempt],
    ) -> serde_json::Value {
        let option = spec.plan.option();
        json!({
            "report_id": uuid(&spec.id),
            "simulated_firmware": spec.target.simulated() && !option.method.is_host_write(),
            "device": {
                "node_hint": spec.device.node_hint,
                "identity": spec.device.identity,
                "block_size": block_size,
                "block_count": block_count,
            },
            "decision": {
                "rules_version": spec.rules.version.to_string(),
                "method": format!("{:?}", option.method),
                "category": format!("{:?}", option.category),
                "forced": matches!(spec.plan, Plan::Forced(_)),
            },
            "validation": {
                "method": if option.method.is_host_write() { "motif" } else { "canaris" },
                "seed": hex(seed),
                "samples": lbas,
            },
            "attempts": attempts.iter().map(|attempt| json!({
                "verification_passed": attempt.report.verification_passed,
                "validation": format!("{:?}", attempt.report.validation),
                "error": attempt.error,
            })).collect::<Vec<_>>(),
            "timeline": self.timeline,
        })
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 || !text.is_ascii() {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

/// The job file of a job about to start.
fn job_file(
    spec: &JobSpec,
    seed: &[u8; 32],
    lbas: &[u64],
    timeline: &[serde_json::Value],
) -> JobFile {
    JobFile {
        format: crate::jobfile::FORMAT,
        id: uuid(&spec.id),
        device: spec.device.clone(),
        profile: spec.profile.clone(),
        capacity_bytes: spec.capacity_bytes,
        plan: spec.plan.clone(),
        rules_version: spec.rules.version.to_string(),
        operator: spec.operator.clone(),
        organization: spec.organization.clone(),
        batch: spec
            .batch
            .as_ref()
            .map(|batch| (batch.id.clone(), batch.label.clone())),
        zero_pass: spec.zero_pass,
        phase: JobPhase::Preparing,
        seed: hex(seed),
        samples: lbas.to_vec(),
        attempts: Vec::new(),
        timeline: timeline.to_vec(),
    }
}

/// The spec of a job left by a crashed helper, or `None` if its identifier
/// is not one Oblivio issues.
pub fn resumed_spec(
    file: &JobFile,
    rules: RuleSet,
    signer: Signer,
    target: Box<dyn ErasureTarget>,
    binary_sha256: [u8; 32],
    reports_dir: PathBuf,
    jobs_dir: Option<PathBuf>,
) -> Option<JobSpec> {
    let digits: String = file.id.chars().filter(|c| *c != '-').collect();
    if digits.len() != 32 {
        return None;
    }
    let mut id = [0u8; 16];
    for (index, byte) in id.iter_mut().enumerate() {
        *byte = u8::from_str_radix(digits.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(JobSpec {
        id,
        device: file.device.clone(),
        profile: file.profile.clone(),
        capacity_bytes: file.capacity_bytes,
        plan: file.plan.clone(),
        rules,
        signer,
        target,
        operator: file.operator.clone(),
        organization: file.organization.clone(),
        batch: file
            .batch
            .as_ref()
            .map(|(id, label)| oblivio_attestation::model::Batch {
                id: id.clone(),
                label: label.clone(),
            }),
        zero_pass: file.zero_pass,
        binary_sha256,
        reports_dir,
        jobs_dir,
    })
}

fn code(text: &str) -> Result<Code, String> {
    Code::new(text).ok_or_else(|| format!("invalid code {text}"))
}

fn category(category: Category) -> AttCategory {
    match category {
        Category::Clear => AttCategory::Clear,
        Category::Purge => AttCategory::Purge,
    }
}

/// The attestation of a finished job, and its status.
fn attestation(
    spec: &JobSpec,
    evaluation: &Evaluation,
    option: MethodOption,
    last: Option<AttemptReport>,
    seed: &[u8; 32],
    samples: usize,
    journal: &[u8],
) -> Result<(FinalStatus, Attestation), String> {
    let (status, final_status, sanitized, disposition, reserves, refusals) = match evaluation {
        Evaluation::Compliant(compliant) => (
            Status::Compliant,
            FinalStatus::Compliant,
            category(compliant.executed.category),
            Some(match &compliant.disposition {
                Disposition::Reuse => AttDisposition {
                    kind: DispositionKind::Reuse,
                    codes: Vec::new(),
                },
                Disposition::DestroyRecommended { reasons } => AttDisposition {
                    kind: DispositionKind::DestroyRecommended,
                    codes: reasons
                        .iter()
                        .map(|reason| code(reason.as_str()))
                        .collect::<Result<_, _>>()?,
                },
            }),
            compliant
                .reserves
                .iter()
                .map(|reserve| code(reserve.as_str()))
                .collect::<Result<Vec<_>, _>>()?,
            Vec::new(),
        ),
        Evaluation::NonCompliant(forced) => (
            Status::NonCompliant,
            FinalStatus::NonCompliant,
            category(forced.executed.category),
            None,
            forced
                .reserves
                .iter()
                .map(|reserve| code(reserve.as_str()))
                .collect::<Result<Vec<_>, _>>()?,
            forced
                .refusals
                .iter()
                .map(|refusal| code(refusal.as_str()))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Evaluation::Refused { reasons } => (
            Status::Refused,
            FinalStatus::Refused,
            AttCategory::None,
            None,
            Vec::new(),
            reasons
                .iter()
                .map(|reason| code(reason.as_str()))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Evaluation::Retry => return Err("job ended on a retry".to_owned()),
    };
    let identity = &spec.profile.identity;
    let drive_type = match &spec.profile.inspection {
        Inspection::Nvme(_) => DriveType::Nvme,
        // Erased as an SSD, but never certified as one without the drive saying so.
        Inspection::Ata(ata) if ata.media_type_unknown => DriveType::Sata,
        Inspection::Ata(ata) if ata.solid_state => DriveType::SataSsd,
        Inspection::Ata(_) => DriveType::SataHdd,
        Inspection::Failed => DriveType::Unknown,
    };
    let attestation = Attestation {
        id: spec.id,
        issued_at: now(),
        status,
        device: Device {
            model: identity.model.clone(),
            serial: identity.serial.clone(),
            firmware: identity.firmware.clone(),
            capacity_bytes: spec.capacity_bytes.unwrap_or_default(),
            drive_type,
            wwn: identity.wwn.clone(),
            eui64: identity.eui64.clone(),
            nguid: identity.nguid.clone(),
        },
        sanitization: Sanitization {
            category: sanitized,
            method: Some(code(&format!("{:?}", option.method))?),
            references: vec!["NIST SP 800-88r2".to_owned(), "IEEE 2883-2022".to_owned()],
            // Only a proven erasure ends with a proven pass of zeros.
            zero_finish: spec.zero_pass
                && option.method.is_host_write()
                && status != Status::Refused
                && last.is_some_and(|report| {
                    report.verification_passed && report.validation == Validation::Passed
                }),
            // Certified as executed: only once the method actually ran.
            all_namespaces: spec.plan.erases_all_namespaces() && last.is_some(),
        },
        verified: last.map(|report| report.verification_passed),
        validation: last.map(|report| AttValidation {
            method: if option.method.is_host_write() {
                ValidationMethod::Pattern
            } else {
                ValidationMethod::Canaries
            },
            samples: u32::try_from(samples).unwrap_or(u32::MAX),
            seed: *seed,
            passed: report.validation == Validation::Passed,
        }),
        disposition,
        reserves,
        refusals,
        justification: match &spec.plan {
            Plan::Forced(forced) => Some(forced.justification.clone()),
            Plan::Ready { .. } => None,
        },
        organization: spec.organization.clone(),
        operator: spec.operator.clone(),
        batch: spec.batch.clone(),
        tool: Tool {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            rules_version: spec.rules.version.to_string(),
            binary_sha256: spec.binary_sha256,
        },
        journal_sha256: Sha256::digest(journal).into(),
        license: spec.signer.license.clone(),
    };
    Ok((final_status, attestation))
}

/// Stores `<id>.cose` and `<id>.json` in the reports directory. Reports are
/// the operators' history (§10): directory 0750, files 0640, group
/// `oblivio`, never overwritten. Keys and licenses stay root-only.
fn store(dir: &std::path::Path, id: &str, signed: &[u8], journal: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let fail = |error: std::io::Error| format!("reports: {error}");
    let group = group_id(REPORTS_GROUP);
    std::fs::create_dir_all(dir).map_err(fail)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o750)).map_err(fail)?;
    if let Some(gid) = group {
        std::os::unix::fs::chown(dir, None, Some(gid)).map_err(fail)?;
    }
    for (extension, content) in [("cose", signed), ("json", journal)] {
        let path = dir.join(format!("{id}.{extension}"));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o640)
            .open(&path)
            .map_err(fail)?;
        // On the open file, not the path: the umask (077) would otherwise
        // keep the group out, and no path can be swapped in between.
        file.set_permissions(std::fs::Permissions::from_mode(0o640))
            .map_err(fail)?;
        if let Some(gid) = group {
            std::os::unix::fs::fchown(&file, None, Some(gid)).map_err(fail)?;
        }
        file.write_all(content).map_err(fail)?;
        file.sync_all().map_err(fail)?;
    }
    Ok(())
}

/// Group allowed to read the reports: the operators (`docs/conformite.md`,
/// R1). Created by the package; members are added by the administrator.
pub const REPORTS_GROUP: &str = "oblivio";

/// Numeric id of group `name`, read from `/etc/group`. `None` when the
/// group does not exist (development builds): reports stay readable by
/// their owner only.
pub fn group_id(name: &str) -> Option<u32> {
    group_id_in(&std::fs::read_to_string("/etc/group").ok()?, name)
}

fn group_id_in(text: &str, name: &str) -> Option<u32> {
    text.lines().find_map(|line| {
        let mut fields = line.split(':');
        (fields.next()? == name)
            .then(|| fields.nth(1)?.parse().ok())
            .flatten()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_ids_are_read_from_etc_group() {
        let text = "root:x:0:\nsudo:x:27:laj\noblivio:x:998:alice,bob\nbroken\n";
        assert_eq!(group_id_in(text, "oblivio"), Some(998));
        assert_eq!(group_id_in(text, "root"), Some(0));
        assert_eq!(group_id_in(text, "obliv"), None);
        assert_eq!(group_id_in(text, "broken"), None);
    }

    #[test]
    fn reports_are_private_to_owner_and_group() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("temporary directory");
        let reports = dir.path().join("reports");
        store(&reports, "0001", b"cose", b"json").expect("stored");
        let mode = |path: &std::path::Path| {
            std::fs::metadata(path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(mode(&reports), 0o750);
        assert_eq!(mode(&reports.join("0001.cose")), 0o640);
        assert_eq!(mode(&reports.join("0001.json")), 0o640);
    }
}
