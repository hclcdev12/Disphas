//! Content of the signed attestation and of the license (`docs/certificat.md` §2, §4).
//!
//! Codes only, never translated text: the reader's language is a rendering
//! concern. Free text typed by the operator is kept as is.

use ciborium::Value;

use crate::cbor::{self, CborError, Fields, MapBuilder};

/// Version of the attestation schema (`v`).
pub const ATTESTATION_VERSION: u64 = 1;
/// Version of the license schema (`v`).
pub const LICENSE_VERSION: u64 = 1;
/// Version of the activation request schema (`v`).
pub const ACTIVATION_REQUEST_VERSION: u64 = 1;

/// A short machine identifier: a decision code (`R4`) or a method name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Code(String);

impl Code {
    /// Accepts 1 to 64 ASCII letters, digits or underscores.
    pub fn new(code: impl Into<String>) -> Option<Self> {
        let code = code.into();
        let valid = (1..=64).contains(&code.len())
            && code
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        valid.then_some(Self(code))
    }

    /// The code.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Declares an enum stored as a fixed text value.
macro_rules! text_enum {
    ($(#[$meta:meta])* $name:ident { $( $(#[$vmeta:meta])* $variant:ident = $text:literal, )+ }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
        }

        impl $name {
            /// Stable text value.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $text, )+
                }
            }

            fn from_value(value: Value, name: &'static str) -> Result<Self, CborError> {
                match cbor::text(value, name)?.as_str() {
                    $( $text => Ok(Self::$variant), )+
                    _ => Err(CborError::Invalid(name)),
                }
            }

            fn to_value(self) -> Value {
                Value::Text(self.as_str().to_owned())
            }
        }
    };
}

text_enum! {
    /// Global status of the certificate (`st`).
    Status {
        /// READY verdict, operation executed, verified and validated.
        Compliant = "CONFORME",
        /// Forced operation on a SAY NO drive: physical destruction required.
        NonCompliant = "NON_CONFORME",
        /// Erasure not provable: physical destruction required.
        Refused = "REFUS",
    }
}

text_enum! {
    /// NIST SP 800-88 category reached (`san.cat`).
    Category {
        /// Clear.
        Clear = "Clear",
        /// Purge.
        Purge = "Purge",
        /// No sanitization was certified.
        None = "None",
    }
}

text_enum! {
    /// Kind of drive (`dev.typ`).
    DriveType {
        /// NVMe.
        Nvme = "NVMe",
        /// SATA solid-state drive.
        SataSsd = "SATA-SSD",
        /// SATA hard disk drive.
        SataHdd = "SATA-HDD",
        /// Not identified: the inspection failed (forced N1).
        Unknown = "Unknown",
    }
}

text_enum! {
    /// How the result was validated (`val.m`).
    ValidationMethod {
        /// Random canaries written before a firmware erasure, none left after.
        Canaries = "canaris",
        /// Expected pattern read back after a host write.
        Pattern = "motif",
    }
}

text_enum! {
    /// Recommended future of the drive (`dis.k`).
    DispositionKind {
        /// The drive can be reused.
        Reuse = "Reuse",
        /// Physical destruction is recommended.
        DestroyRecommended = "DestroyRecommended",
    }
}

/// The erased drive (`dev`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// Model.
    pub model: String,
    /// Full serial number: it binds the proof to a physical drive.
    pub serial: String,
    /// Firmware revision.
    pub firmware: String,
    /// Capacity in bytes.
    pub capacity_bytes: u64,
    /// Kind of drive.
    pub drive_type: DriveType,
    /// ATA World Wide Name.
    pub wwn: Option<String>,
    /// NVMe EUI-64.
    pub eui64: Option<String>,
    /// NVMe NGUID.
    pub nguid: Option<String>,
}

/// What was done (`san`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sanitization {
    /// Category reached.
    pub category: Category,
    /// Method executed, absent if none ran.
    pub method: Option<Code>,
    /// Standards referred to, e.g. `NIST SP 800-88r2`, `IEEE 2883-2022`.
    pub references: Vec<String>,
    /// A host overwrite ended with a validated pass of zeros (`z`, present
    /// only when true).
    pub zero_finish: bool,
}

/// How the result was validated (`val`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Validation {
    /// Method.
    pub method: ValidationMethod,
    /// Number of sample blocks.
    pub samples: u32,
    /// Seed of the sample selection, to replay it.
    pub seed: [u8; 32],
    /// Whether every sample passed.
    pub passed: bool,
}

/// Recommended future of the drive (`dis`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disposition {
    /// Reuse or destruction.
    pub kind: DispositionKind,
    /// Disposition codes (`D1`…), empty for reuse.
    pub codes: Vec<Code>,
}

/// The batch a drive was erased in (`lot`): lets a batch certificate prove
/// that each drive belongs to it. Declarative, chosen by the operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    /// Identifier, 1 to 64 ASCII letters, digits, `-` or `_` (`id`).
    pub id: String,
    /// Label, such as a customer order number, at most 128 characters (`lbl`).
    pub label: Option<String>,
}

impl Batch {
    /// Largest label, in characters.
    pub const MAX_LABEL_CHARS: usize = 128;

    /// Whether `id` is a valid batch identifier.
    pub fn valid_id(id: &str) -> bool {
        (1..=64).contains(&id.len())
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    }

    /// Whether the identifier and label are within bounds.
    pub fn is_valid(&self) -> bool {
        Self::valid_id(&self.id)
            && self
                .label
                .as_ref()
                .is_none_or(|label| label.chars().count() <= Self::MAX_LABEL_CHARS)
    }
}

/// The tool that produced the attestation (`tool`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    /// Oblivio version.
    pub version: String,
    /// `rules_version` of the decision engine.
    pub rules_version: String,
    /// SHA-256 of the helper binary.
    pub binary_sha256: [u8; 32],
}

/// The signed attestation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attestation {
    /// Report identifier, a UUID v7 (`id`).
    pub id: [u8; 16],
    /// End of the operation, UTC epoch seconds (`iat`).
    pub issued_at: u64,
    /// Global status (`st`).
    pub status: Status,
    /// The drive (`dev`).
    pub device: Device,
    /// What was done (`san`).
    pub sanitization: Sanitization,
    /// Firmware verification result, absent if nothing ran (`ver`).
    pub verified: Option<bool>,
    /// Validation, absent if nothing ran (`val`).
    pub validation: Option<Validation>,
    /// Recommended future, absent on a refusal (`dis`).
    pub disposition: Option<Disposition>,
    /// Reserve codes (`res`).
    pub reserves: Vec<Code>,
    /// Refusal codes (`rsn`), for REFUS and NON_CONFORME.
    pub refusals: Vec<Code>,
    /// Forcing justification, as typed (`frc`).
    pub justification: Option<String>,
    /// Organization, as typed (`org`).
    pub organization: Option<String>,
    /// Operator, as typed (`op`).
    pub operator: Option<String>,
    /// Batch the drive was erased in (`lot`).
    pub batch: Option<Batch>,
    /// Producing tool (`tool`).
    pub tool: Tool,
    /// SHA-256 of the detailed journal (`jh`).
    pub journal_sha256: [u8; 32],
    /// Workstation license, a COSE_Sign1 signed by the root key (`lic`).
    pub license: Vec<u8>,
}

/// The workstation license, signed by the Oblivio root key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct License {
    /// License identifier (`id`).
    pub id: String,
    /// Customer (`cli`).
    pub customer: String,
    /// Commercial offer (`off`).
    pub offer: String,
    /// Start of validity, UTC epoch seconds (`nbf`).
    pub not_before: u64,
    /// End of validity, UTC epoch seconds; absent if perpetual (`exp`).
    pub expires: Option<u64>,
    /// Workstation public key, Ed25519 (`wpk`).
    pub workstation_key: [u8; 32],
}

impl License {
    /// Whether the license is valid at `time` (UTC epoch seconds).
    pub fn is_valid_at(&self, time: u64) -> bool {
        self.not_before <= time && self.expires.is_none_or(|expires| time <= expires)
    }

    /// Deterministic CBOR encoding.
    pub fn to_cbor(&self) -> Vec<u8> {
        cbor::encode(
            MapBuilder::default()
                .field("v", uint(LICENSE_VERSION))
                .field("id", text(&self.id))
                .field("cli", text(&self.customer))
                .field("off", text(&self.offer))
                .field("nbf", uint(self.not_before))
                .optional("exp", self.expires.map(uint))
                .field("wpk", Value::Bytes(self.workstation_key.to_vec()))
                .build(),
        )
    }

    /// Strict decoding: deterministic encoding, known fields only.
    pub fn from_cbor(bytes: &[u8]) -> Result<Self, CborError> {
        cbor::decode_canonical(bytes, Self::from_value, Self::to_cbor)
    }

    fn from_value(value: Value) -> Result<Self, CborError> {
        let mut fields = Fields::of(value, "license")?;
        if cbor::uint(fields.required("v")?, "v")? != LICENSE_VERSION {
            return Err(CborError::Invalid("v"));
        }
        let license = Self {
            id: cbor::text(fields.required("id")?, "id")?,
            customer: cbor::text(fields.required("cli")?, "cli")?,
            offer: cbor::text(fields.required("off")?, "off")?,
            not_before: cbor::uint(fields.required("nbf")?, "nbf")?,
            expires: fields
                .optional("exp")
                .map(|value| cbor::uint(value, "exp"))
                .transpose()?,
            workstation_key: cbor::fixed_bytes(fields.required("wpk")?, "wpk")?,
        };
        fields.finish()?;
        Ok(license)
    }
}

/// Request sent by a workstation to obtain its license, signed by the
/// workstation key itself to prove possession.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationRequest {
    /// Workstation public key, Ed25519 (`wpk`).
    pub workstation_key: [u8; 32],
    /// Creation time, UTC epoch seconds (`iat`).
    pub issued_at: u64,
}

impl ActivationRequest {
    /// Deterministic CBOR encoding.
    pub fn to_cbor(&self) -> Vec<u8> {
        cbor::encode(
            MapBuilder::default()
                .field("v", uint(ACTIVATION_REQUEST_VERSION))
                .field("wpk", Value::Bytes(self.workstation_key.to_vec()))
                .field("iat", uint(self.issued_at))
                .build(),
        )
    }

    /// Strict decoding: deterministic encoding, known fields only.
    pub fn from_cbor(bytes: &[u8]) -> Result<Self, CborError> {
        cbor::decode_canonical(bytes, Self::from_value, Self::to_cbor)
    }

    fn from_value(value: Value) -> Result<Self, CborError> {
        let mut fields = Fields::of(value, "activation request")?;
        if cbor::uint(fields.required("v")?, "v")? != ACTIVATION_REQUEST_VERSION {
            return Err(CborError::Invalid("v"));
        }
        let request = Self {
            workstation_key: cbor::fixed_bytes(fields.required("wpk")?, "wpk")?,
            issued_at: cbor::uint(fields.required("iat")?, "iat")?,
        };
        fields.finish()?;
        Ok(request)
    }
}

impl Attestation {
    /// Deterministic CBOR encoding.
    pub fn to_cbor(&self) -> Vec<u8> {
        let device = &self.device;
        let device = MapBuilder::default()
            .field("mdl", text(&device.model))
            .field("sn", text(&device.serial))
            .field("fw", text(&device.firmware))
            .field("cap", uint(device.capacity_bytes))
            .field("typ", device.drive_type.to_value())
            .optional("wwn", device.wwn.as_deref().map(text))
            .optional("eui", device.eui64.as_deref().map(text))
            .optional("ngu", device.nguid.as_deref().map(text))
            .build();
        let sanitization = MapBuilder::default()
            .field("cat", self.sanitization.category.to_value())
            .optional("m", self.sanitization.method.as_ref().map(code))
            .optional(
                "z",
                self.sanitization.zero_finish.then_some(Value::Bool(true)),
            )
            .field(
                "ref",
                Value::Array(
                    self.sanitization
                        .references
                        .iter()
                        .map(|r| text(r))
                        .collect(),
                ),
            )
            .build();
        let validation = self.validation.as_ref().map(|validation| {
            MapBuilder::default()
                .field("m", validation.method.to_value())
                .field("n", uint(u64::from(validation.samples)))
                .field("seed", Value::Bytes(validation.seed.to_vec()))
                .field("ok", Value::Bool(validation.passed))
                .build()
        });
        let disposition = self.disposition.as_ref().map(|disposition| {
            MapBuilder::default()
                .field("k", disposition.kind.to_value())
                .field("c", codes(&disposition.codes))
                .build()
        });
        let tool = MapBuilder::default()
            .field("ver", text(&self.tool.version))
            .field("rules", text(&self.tool.rules_version))
            .field("bin", Value::Bytes(self.tool.binary_sha256.to_vec()))
            .build();
        cbor::encode(
            MapBuilder::default()
                .field("v", uint(ATTESTATION_VERSION))
                .field("id", Value::Bytes(self.id.to_vec()))
                .field("iat", uint(self.issued_at))
                .field("st", self.status.to_value())
                .field("dev", device)
                .field("san", sanitization)
                .optional("ver", self.verified.map(Value::Bool))
                .optional("val", validation)
                .optional("dis", disposition)
                .field("res", codes(&self.reserves))
                .field("rsn", codes(&self.refusals))
                .optional("frc", self.justification.as_deref().map(text))
                .optional("org", self.organization.as_deref().map(text))
                .optional("op", self.operator.as_deref().map(text))
                .optional(
                    "lot",
                    self.batch.as_ref().map(|batch| {
                        MapBuilder::default()
                            .field("id", text(&batch.id))
                            .optional("lbl", batch.label.as_deref().map(text))
                            .build()
                    }),
                )
                .field("tool", tool)
                .field("jh", Value::Bytes(self.journal_sha256.to_vec()))
                .field("lic", Value::Bytes(self.license.clone()))
                .build(),
        )
    }

    /// Strict decoding: deterministic encoding, known fields only.
    pub fn from_cbor(bytes: &[u8]) -> Result<Self, CborError> {
        cbor::decode_canonical(bytes, Self::from_value, Self::to_cbor)
    }

    fn from_value(value: Value) -> Result<Self, CborError> {
        let mut fields = Fields::of(value, "attestation")?;
        if cbor::uint(fields.required("v")?, "v")? != ATTESTATION_VERSION {
            return Err(CborError::Invalid("v"));
        }
        let id = cbor::fixed_bytes(fields.required("id")?, "id")?;
        let issued_at = cbor::uint(fields.required("iat")?, "iat")?;
        let status = Status::from_value(fields.required("st")?, "st")?;

        let mut dev = Fields::of(fields.required("dev")?, "dev")?;
        let optional_text = |fields: &mut Fields, key| {
            fields
                .optional(key)
                .map(|value| cbor::text(value, key))
                .transpose()
        };
        let device = Device {
            model: cbor::text(dev.required("mdl")?, "mdl")?,
            serial: cbor::text(dev.required("sn")?, "sn")?,
            firmware: cbor::text(dev.required("fw")?, "fw")?,
            capacity_bytes: cbor::uint(dev.required("cap")?, "cap")?,
            drive_type: DriveType::from_value(dev.required("typ")?, "typ")?,
            wwn: optional_text(&mut dev, "wwn")?,
            eui64: optional_text(&mut dev, "eui")?,
            nguid: optional_text(&mut dev, "ngu")?,
        };
        dev.finish()?;

        let mut san = Fields::of(fields.required("san")?, "san")?;
        let sanitization = Sanitization {
            category: Category::from_value(san.required("cat")?, "cat")?,
            method: san
                .optional("m")
                .map(|value| to_code(value, "m"))
                .transpose()?,
            references: cbor::array(san.required("ref")?, "ref")?
                .into_iter()
                .map(|value| cbor::text(value, "ref"))
                .collect::<Result<_, _>>()?,
            // Canonical form: present only when true.
            zero_finish: match san.optional("z").map(|value| cbor::boolean(value, "z")) {
                None => false,
                Some(Ok(true)) => true,
                Some(Ok(false)) => return Err(CborError::Invalid("z")),
                Some(Err(error)) => return Err(error),
            },
        };
        san.finish()?;

        let verified = fields
            .optional("ver")
            .map(|value| cbor::boolean(value, "ver"))
            .transpose()?;

        let validation = fields
            .optional("val")
            .map(|value| {
                let mut val = Fields::of(value, "val")?;
                let validation = Validation {
                    method: ValidationMethod::from_value(val.required("m")?, "m")?,
                    samples: u32::try_from(cbor::uint(val.required("n")?, "n")?)
                        .map_err(|_| CborError::Invalid("n"))?,
                    seed: cbor::fixed_bytes(val.required("seed")?, "seed")?,
                    passed: cbor::boolean(val.required("ok")?, "ok")?,
                };
                val.finish()?;
                Ok(validation)
            })
            .transpose()?;

        let disposition = fields
            .optional("dis")
            .map(|value| {
                let mut dis = Fields::of(value, "dis")?;
                let disposition = Disposition {
                    kind: DispositionKind::from_value(dis.required("k")?, "k")?,
                    codes: to_codes(dis.required("c")?, "c")?,
                };
                dis.finish()?;
                Ok(disposition)
            })
            .transpose()?;

        let reserves = to_codes(fields.required("res")?, "res")?;
        let refusals = to_codes(fields.required("rsn")?, "rsn")?;
        let justification = optional_text(&mut fields, "frc")?;
        let organization = optional_text(&mut fields, "org")?;
        let operator = optional_text(&mut fields, "op")?;
        let batch = fields
            .optional("lot")
            .map(|value| {
                let mut lot = Fields::of(value, "lot")?;
                let batch = Batch {
                    id: cbor::text(lot.required("id")?, "id")?,
                    label: optional_text(&mut lot, "lbl")?,
                };
                lot.finish()?;
                if !batch.is_valid() {
                    return Err(CborError::Invalid("lot"));
                }
                Ok(batch)
            })
            .transpose()?;

        let mut tool_fields = Fields::of(fields.required("tool")?, "tool")?;
        let tool = Tool {
            version: cbor::text(tool_fields.required("ver")?, "ver")?,
            rules_version: cbor::text(tool_fields.required("rules")?, "rules")?,
            binary_sha256: cbor::fixed_bytes(tool_fields.required("bin")?, "bin")?,
        };
        tool_fields.finish()?;

        let attestation = Self {
            id,
            issued_at,
            status,
            device,
            sanitization,
            verified,
            validation,
            disposition,
            reserves,
            refusals,
            justification,
            organization,
            operator,
            batch,
            tool,
            journal_sha256: cbor::fixed_bytes(fields.required("jh")?, "jh")?,
            license: cbor::bytes(fields.required("lic")?, "lic")?,
        };
        fields.finish()?;
        Ok(attestation)
    }
}

fn uint(value: u64) -> Value {
    Value::Integer(value.into())
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn code(value: &Code) -> Value {
    text(value.as_str())
}

fn codes(values: &[Code]) -> Value {
    Value::Array(values.iter().map(code).collect())
}

fn to_code(value: Value, name: &'static str) -> Result<Code, CborError> {
    Code::new(cbor::text(value, name)?).ok_or(CborError::Invalid(name))
}

fn to_codes(value: Value, name: &'static str) -> Result<Vec<Code>, CborError> {
    cbor::array(value, name)?
        .into_iter()
        .map(|value| to_code(value, name))
        .collect()
}
