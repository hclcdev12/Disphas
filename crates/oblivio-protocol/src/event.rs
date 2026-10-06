//! Spontaneous events from the helper (§8).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::command::{DeviceRef, JobId};
use crate::response::{DeviceChangedData, FinalStatus, JobPhase};

/// An event. Serialized as `"event"` and `"data"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(tag = "event", content = "data", deny_unknown_fields)]
pub enum Event {
    /// A drive appeared.
    DeviceAdded(DeviceData),
    /// A drive disappeared.
    DeviceRemoved(DeviceData),
    /// A drive changed; its pending verdicts are invalid.
    DeviceChanged(DeviceChangedData),
    /// A job entered a new phase.
    JobPhase(JobPhaseData),
    /// Progress of a job.
    JobProgress(JobProgressData),
    /// A log line, as a code to translate.
    JobLog(JobLogData),
    /// A job finished.
    JobFinished(JobFinishedData),
}

/// Payload of `DeviceAdded` and `DeviceRemoved`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct DeviceData {
    /// The drive.
    pub device: DeviceRef,
}

/// Payload of `JobPhase`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct JobPhaseData {
    /// The job.
    pub job_id: JobId,
    /// The new phase.
    pub phase: JobPhase,
}

/// Who reports the progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum ProgressSource {
    /// The drive firmware.
    Firmware,
    /// The host, for an overwrite.
    Host,
}

/// Payload of `JobProgress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct JobProgressData {
    /// The job.
    pub job_id: JobId,
    /// Completion, 0 to 100.
    pub percent: u8,
    /// Who reports the progress.
    pub source: ProgressSource,
    /// Host overwrite: bytes written so far.
    pub bytes_done: Option<u64>,
    /// Host overwrite: current throughput.
    pub bytes_per_second: Option<u64>,
    /// Estimated seconds left: measured during a host overwrite; for a
    /// firmware command that reports nothing until it ends (ATA Security
    /// Erase), the drive's own estimate, sent once at its start.
    pub remaining_seconds: Option<u64>,
}

/// Severity of a log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Information.
    Info,
    /// Warning.
    Warning,
    /// Error.
    Error,
}

/// Payload of `JobLog`. Never carries text: the app translates `code`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct JobLogData {
    /// The job.
    pub job_id: JobId,
    /// Severity.
    pub level: LogLevel,
    /// Message code.
    pub code: String,
    /// Message parameters.
    pub params: BTreeMap<String, String>,
}

/// Payload of `JobFinished`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[serde(deny_unknown_fields)]
pub struct JobFinishedData {
    /// The job.
    pub job_id: JobId,
    /// Final status.
    pub status: FinalStatus,
}
