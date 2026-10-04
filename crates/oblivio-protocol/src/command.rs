//! Requests from the UI to the helper: a closed list of commands (§6).
//!
//! Validation and signing are deliberately absent: they only happen inside
//! `Sanitize`, so nothing can be signed that was not erased.

use serde::{Deserialize, Serialize};

use crate::codes::{Acknowledgement, Method};
use crate::secret::AtaPassword;

/// A command with its arguments. Serialized as `"cmd"` and `"args"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(tag = "cmd", content = "args", deny_unknown_fields)]
pub enum Command {
    /// Opens the session and checks versions.
    Hello(HelloArgs),
    /// Lists drives with their quick state.
    ListDevices,
    /// Inspects a drive and returns its verdict.
    Inspect(DeviceArgs),
    /// Unlocks a Locked drive (B2).
    Unlock(UnlockArgs),
    /// Runs the full erasure: execution, verification, validation, signing.
    Sanitize(SanitizeArgs),
    /// Cancels a job, when its current phase allows it.
    Cancel(JobArgs),
    /// Lists running jobs and finished ones not yet collected.
    ListJobs,
    /// Returns the final result of a job.
    GetJobResult(JobArgs),
    /// Returns the license status.
    LicenseStatus,
    /// Creates an offline activation request.
    CreateActivationRequest,
    /// Installs a `.lic` license.
    InstallLicense(InstallLicenseArgs),
    /// Deletes stored reports (retention chosen by the customer).
    DeleteReports(DeleteReportsArgs),
    /// Stops the helper; refused while a job is running.
    Shutdown,
}

impl Command {
    /// Every command name accepted on the wire.
    pub const NAMES: &'static [&'static str] = &[
        "Hello",
        "ListDevices",
        "Inspect",
        "Unlock",
        "Sanitize",
        "Cancel",
        "ListJobs",
        "GetJobResult",
        "LicenseStatus",
        "CreateActivationRequest",
        "InstallLicense",
        "DeleteReports",
        "Shutdown",
    ];
}

/// Arguments of `DeleteReports`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct DeleteReportsArgs {
    /// Report identifiers (UUID, lowercase), at most 5000.
    pub ids: Vec<String>,
}

/// Arguments of `Hello`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct HelloArgs {
    /// Protocol version spoken by the app.
    pub protocol_version: u32,
    /// Version of the app.
    pub app_version: String,
}

/// Arguments naming a single drive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct DeviceArgs {
    /// The drive.
    pub device: DeviceRef,
}

/// Arguments of `Unlock`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct UnlockArgs {
    /// The drive.
    pub device: DeviceRef,
    /// Which password the operator gives.
    pub kind: PasswordKind,
    /// ATA user or master password. Never logged, wiped after use.
    #[cfg_attr(feature = "ts-rs", ts(as = "String"))]
    pub password: AtaPassword,
}

/// Which ATA password is given to `Unlock`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum PasswordKind {
    /// The user password.
    User,
    /// The master password (works only in High security level).
    Master,
}

/// Arguments of `Sanitize` (§7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct SanitizeArgs {
    /// Verdict this request relies on, as returned by `Inspect`.
    pub verdict_id: VerdictId,
    /// The drive.
    pub device: DeviceRef,
    /// Method to execute; `None` for the recommended one.
    pub method: Option<Method>,
    /// Last four characters of the serial number, typed by the operator.
    pub confirmation: String,
    /// Warnings the operator acknowledged.
    pub acknowledged: Vec<Acknowledgement>,
    /// Present only to force a SAY NO verdict.
    pub force: Option<ForceArgs>,
    /// Operator name, declarative, certified as typed.
    pub operator: Option<String>,
    /// Organization, declarative, certified as typed.
    pub organization: Option<String>,
    /// Batch the drive is erased in, certified in its attestation.
    #[serde(default)]
    pub batch: Option<BatchArgs>,
    /// Host overwrite only: end with a validated pass of zeros, so the
    /// drive looks blank; doubles the duration. Refused with other methods.
    #[serde(default)]
    pub zero_pass: bool,
}

/// Batch of a `Sanitize`, chosen by the operator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct BatchArgs {
    /// Identifier: 1 to 64 ASCII letters, digits, `-` or `_`.
    pub id: String,
    /// Label, such as a customer order number: at most 128 characters.
    pub label: Option<String>,
}

/// Request to force a SAY NO verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct ForceArgs {
    /// Operator justification, certified as typed.
    pub justification: String,
}

/// Arguments naming a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct JobArgs {
    /// The job.
    pub job_id: JobId,
}

/// Arguments of `InstallLicense`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct InstallLicenseArgs {
    /// Content of the `.lic` file.
    pub license: String,
}

/// A drive as named by the UI. The helper finds it by `identity`, never by path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct DeviceRef {
    /// Path such as `/dev/nvme0n1`: a hint only, never used as is.
    pub node_hint: Option<String>,
    /// Hardware identity.
    pub identity: DeviceIdentity,
}

/// Hardware identity of a drive (§5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct DeviceIdentity {
    /// Model string.
    pub model: String,
    /// Full serial number.
    pub serial: String,
    /// ATA World Wide Name.
    pub wwn: Option<String>,
    /// NVMe EUI-64.
    pub eui64: Option<String>,
    /// NVMe NGUID.
    pub nguid: Option<String>,
}

/// Identifier of a verdict issued by the helper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(transparent)]
pub struct VerdictId(pub String);

/// Identifier of a job.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(transparent)]
pub struct JobId(pub String);
