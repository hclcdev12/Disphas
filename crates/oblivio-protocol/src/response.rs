//! Response payloads and error codes (§6, §9).

use serde::{Deserialize, Serialize};

use crate::codes::ProtectCode;
use crate::command::{DeviceIdentity, DeviceRef, JobId, VerdictId};
use crate::verdict::Verdict;

/// Error codes returned by the helper (§9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// Incompatible protocol versions.
    ProtocolMismatch,
    /// Malformed message or arguments.
    BadRequest,
    /// Command outside the closed list.
    UnknownCommand,
    /// No drive with this identity.
    DeviceNotFound,
    /// The drive identity no longer matches.
    DeviceChanged,
    /// The drive is protected.
    Protected,
    /// Unknown or expired verdict.
    VerdictExpired,
    /// The request does not match the verdict.
    VerdictMismatch,
    /// Wrong serial number confirmation.
    ConfirmationMismatch,
    /// A warning was not acknowledged.
    AckMissing,
    /// Forcing is not allowed for this verdict or justification.
    ForceNotAllowed,
    /// The license does not allow erasure.
    LicenseRequired,
    /// Another job is running.
    Busy,
    /// The current job phase cannot be cancelled.
    NotCancellable,
    /// The drive could not be unlocked.
    UnlockFailed,
    /// Unexpected helper failure.
    Internal,
}

/// Error payload of a failed response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorBody {
    /// Error code, translated by the app.
    pub code: ErrorCode,
    /// Technical detail for logs, never shown untranslated.
    pub detail: Option<String>,
}

/// Result of commands that only acknowledge success.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

/// Result of `Hello`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelloResult {
    /// Protocol version spoken by the helper.
    pub protocol_version: u32,
    /// Version of the helper.
    pub helper_version: String,
    /// `rules_version` of the decision engine.
    pub rules_version: String,
    /// SHA-256 of the helper binary, hexadecimal.
    pub binary_sha256: String,
    /// License status.
    pub license: LicenseStatusResult,
}

/// Kind of drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DriveKind {
    /// NVMe.
    Nvme,
    /// SATA solid-state drive.
    SataSsd,
    /// SATA hard disk drive.
    SataHdd,
}

/// One entry of `ListDevices`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceSummary {
    /// The drive.
    pub device: DeviceRef,
    /// Kind of drive, if known.
    pub kind: Option<DriveKind>,
    /// Capacity in bytes, if known.
    pub capacity_bytes: Option<u64>,
    /// Protections found by the quick check; empty if none.
    pub protection: Vec<ProtectCode>,
}

/// Result of `ListDevices`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListDevicesResult {
    /// Every drive found.
    pub devices: Vec<DeviceSummary>,
}

/// Result of `Inspect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectResult {
    /// Identifier to quote in `Sanitize`.
    pub verdict_id: VerdictId,
    /// Seconds before the verdict expires.
    pub valid_for_seconds: u32,
    /// The drive, as identified by the helper.
    pub device: DeviceRef,
    /// Kind of drive.
    pub kind: DriveKind,
    /// Capacity in bytes.
    pub capacity_bytes: u64,
    /// Firmware revision.
    pub firmware: String,
    /// The decision.
    pub verdict: Verdict,
}

/// Result of `Sanitize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SanitizeResult {
    /// The job started.
    pub job_id: JobId,
}

/// Phases of a job, in order (§8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum JobPhase {
    /// Checks before execution.
    Preparing,
    /// Writing validation canaries.
    Canaries,
    /// Running the method.
    Executing,
    /// Firmware verification.
    Verifying,
    /// Sample validation.
    Validating,
    /// Signing the attestation.
    Signing,
    /// Finished.
    Done,
}

/// Final status of a job, as in the attestation (`docs/certificat.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FinalStatus {
    /// Erasure proven.
    #[serde(rename = "CONFORME")]
    Compliant,
    /// Forced erasure: physical destruction required.
    #[serde(rename = "NON_CONFORME")]
    NonCompliant,
    /// Erasure not provable.
    #[serde(rename = "REFUS")]
    Refused,
}

/// One entry of `ListJobs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobSummary {
    /// The job.
    pub job_id: JobId,
    /// The drive.
    pub device: DeviceRef,
    /// Current phase.
    pub phase: JobPhase,
    /// Final status, once finished.
    pub status: Option<FinalStatus>,
}

/// Result of `ListJobs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListJobsResult {
    /// Running jobs and finished ones not yet collected.
    pub jobs: Vec<JobSummary>,
}

/// Result of `GetJobResult`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobResult {
    /// The job.
    pub job_id: JobId,
    /// Final status.
    pub status: FinalStatus,
    /// Signed attestation, COSE_Sign1, base64.
    pub attestation: String,
    /// Detailed journal, JSON text.
    pub journal: String,
}

/// State of the installed license.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LicenseState {
    /// No license installed.
    Missing,
    /// Valid license.
    Valid,
    /// Expired license.
    Expired,
    /// Revoked license.
    Revoked,
}

/// Result of `LicenseStatus`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenseStatusResult {
    /// State of the license.
    pub state: LicenseState,
    /// Commercial offer.
    pub offer: Option<String>,
    /// License identifier.
    pub license_id: Option<String>,
    /// End of validity, UTC epoch seconds.
    pub valid_until: Option<u64>,
}

/// Result of `CreateActivationRequest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivationRequestResult {
    /// Activation request carrying the workstation public key.
    pub request: String,
}

/// Payload of `DeviceChanged`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceChangedData {
    /// Identity of the drive whose verdicts are now invalid.
    pub identity: DeviceIdentity,
}
