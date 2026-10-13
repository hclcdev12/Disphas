//! What the service signs, and what it refuses (`docs/licences_en_ligne.md`
//! §6.3, §6.4). The site decides who gets a license; this decides what can
//! be signed: within the issuer certificate, for a proven workstation, at a
//! bounded rate, recorded before it is handed out.

use std::collections::VecDeque;

use disphas_attestation::License;
use disphas_attestation::display::civil;
use disphas_attestation::ed25519_dalek::VerifyingKey;
use disphas_license::{decode_license, license_text, read_activation_request};
use serde::{Deserialize, Serialize};

use crate::register::Register;
use crate::store::Certified;

/// Longest customer name, in characters.
pub const MAX_CUSTOMER_CHARS: usize = 128;
/// Most signatures in any 60 seconds.
pub const MAX_PER_MINUTE: usize = 30;
/// Most signatures in one UTC day.
pub const MAX_PER_DAY: usize = 2_000;
/// A license starts this many seconds before it is signed (clock skew).
const START_MARGIN: u64 = 60;

/// A request from the site, one JSON object per line.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "cmd", deny_unknown_fields)]
pub enum Request {
    /// A new license for the workstation of `request` (`DSA1:…`).
    Activate {
        /// Activation request, signed by the workstation key it carries.
        request: String,
        /// Customer name, printed on every certificate.
        customer: String,
        /// Commercial offer.
        offer: String,
        /// End of validity, UTC epoch seconds.
        expires: u64,
    },
    /// The same license with a new end date.
    Renew {
        /// License identifier, from a previous `Activate`.
        id: String,
        /// New end of validity, UTC epoch seconds.
        expires: u64,
    },
    /// Installed certificates and signature counts.
    Status,
}

/// Why a request is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ErrorCode {
    /// Malformed request or activation request, or an end date already past.
    BadRequest,
    /// `Renew` of a license this service never signed.
    UnknownLicense,
    /// Outside the issuer certificate or the service rules.
    OutOfBounds,
    /// Too many signatures: try later, and look into it.
    RateLimited,
    /// No valid issuer certificate is installed.
    NoIssuer,
    /// Register or signature failure: nothing was handed out.
    Internal,
}

/// An installed certificate, as reported by `Status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IssuerStatus {
    /// Issuer identifier.
    pub id: String,
    /// Start of validity.
    pub not_before: u64,
    /// End of validity.
    pub expires: u64,
    /// Offers it may sign.
    pub offers: Vec<String>,
    /// Longest license, in days.
    pub max_days: u64,
    /// Whether it signs new licenses now.
    pub current: bool,
}

/// A reply to the site, one JSON object per line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Reply {
    /// A new license.
    Activated {
        /// New license identifier.
        id: String,
        /// Workstation public key, hexadecimal: the site stores it with the seat.
        workstation_key: String,
        /// The license (`DSL1:…`).
        license: String,
    },
    /// A renewed license.
    Renewed {
        /// The license (`DSL1:…`).
        license: String,
    },
    /// Service state.
    Status {
        /// Installed certificates.
        issuers: Vec<IssuerStatus>,
        /// Signatures since 00:00 UTC.
        signed_today: usize,
        /// Signatures since the first day of the month, UTC.
        signed_this_month: usize,
    },
    /// Refused.
    Error {
        /// Why.
        error: ErrorCode,
        /// Detail for the site's logs.
        detail: String,
    },
}

fn refuse(error: ErrorCode, detail: impl Into<String>) -> Reply {
    Reply::Error {
        error,
        detail: detail.into(),
    }
}

/// The signing service state.
#[derive(Debug)]
pub struct Signer {
    certified: Vec<Certified>,
    roots: Vec<VerifyingKey>,
    register: Register,
    recent: VecDeque<u64>,
}

impl Signer {
    /// A service signing with `certified` keys, checked against `roots`,
    /// recording into `register`.
    pub fn new(certified: Vec<Certified>, roots: Vec<VerifyingKey>, register: Register) -> Self {
        Self {
            certified,
            roots,
            register,
            recent: VecDeque::new(),
        }
    }

    /// Answers one request at `now` (UTC epoch seconds).
    pub fn handle(&mut self, request: Request, now: u64) -> Reply {
        match request {
            Request::Activate {
                request,
                customer,
                offer,
                expires,
            } => self.activate(&request, customer, offer, expires, now),
            Request::Renew { id, expires } => self.renew(&id, expires, now),
            Request::Status => self.status(now),
        }
    }

    fn activate(
        &mut self,
        request: &str,
        customer: String,
        offer: String,
        expires: u64,
        now: u64,
    ) -> Reply {
        let Ok(request) = read_activation_request(request) else {
            return refuse(ErrorCode::BadRequest, "invalid activation request");
        };
        let length = customer.chars().count();
        if !(1..=MAX_CUSTOMER_CHARS).contains(&length) || customer.chars().any(char::is_control) {
            return refuse(
                ErrorCode::OutOfBounds,
                "customer name: 1 to 128 printable characters",
            );
        }
        let id = match self.new_id(now) {
            Ok(id) => id,
            Err(reply) => return reply,
        };
        let license = License {
            id: id.clone(),
            customer,
            offer,
            not_before: now.saturating_sub(START_MARGIN),
            expires: Some(expires),
            workstation_key: request.workstation_key,
            issuer: None,
        };
        match self.sign(license, now) {
            Ok(text) => Reply::Activated {
                id,
                workstation_key: request
                    .workstation_key
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
                license: text,
            },
            Err(reply) => reply,
        }
    }

    fn renew(&mut self, id: &str, expires: u64, now: u64) -> Reply {
        // Only the end date moves: workstation, customer and offer come from
        // the register, never from the site.
        let Some(seat) = self.register.seat(id).cloned() else {
            return refuse(ErrorCode::UnknownLicense, id.to_owned());
        };
        let license = License {
            id: id.to_owned(),
            customer: seat.customer,
            offer: seat.offer,
            not_before: now.saturating_sub(START_MARGIN),
            expires: Some(expires),
            workstation_key: seat.workstation_key,
            issuer: None,
        };
        match self.sign(license, now) {
            Ok(text) => Reply::Renewed { license: text },
            Err(reply) => reply,
        }
    }

    /// A new license identifier: the date, then 64 random bits, never
    /// already in the register.
    fn new_id(&self, now: u64) -> Result<String, Reply> {
        let (year, month, day, ..) = civil(now);
        for _ in 0..4 {
            let mut suffix = [0u8; 8];
            getrandom::fill(&mut suffix)
                .map_err(|error| refuse(ErrorCode::Internal, error.to_string()))?;
            let suffix: String = suffix.iter().map(|byte| format!("{byte:02X}")).collect();
            let id = format!("DSL-{year:04}{month:02}{day:02}-{suffix}");
            if !self.register.contains(&id) {
                return Ok(id);
            }
        }
        Err(refuse(ErrorCode::Internal, "no free license identifier"))
    }

    /// The certificate that signs now: valid now, latest end date.
    fn current(&self, now: u64) -> Option<&Certified> {
        self.certified
            .iter()
            .filter(|entry| entry.issuer.not_before <= now && now < entry.issuer.expires)
            .max_by_key(|entry| entry.issuer.expires)
    }

    fn sign(&mut self, mut license: License, now: u64) -> Result<String, Reply> {
        if license.expires.is_none_or(|expires| expires <= now) {
            return Err(refuse(ErrorCode::BadRequest, "end date already past"));
        }
        while self.recent.front().is_some_and(|at| *at + 60 <= now) {
            self.recent.pop_front();
        }
        let today = now - now % 86_400;
        if self.recent.len() >= MAX_PER_MINUTE || self.register.signed_since(today) >= MAX_PER_DAY {
            return Err(refuse(ErrorCode::RateLimited, "signature rate exceeded"));
        }
        let Some(certified) = self.current(now) else {
            return Err(refuse(ErrorCode::NoIssuer, "no valid issuer certificate"));
        };
        if !certified.issuer.allows(&license) {
            return Err(refuse(
                ErrorCode::OutOfBounds,
                format!(
                    "{} signs {} for {} days at most, until {}",
                    certified.issuer.id,
                    certified.issuer.offers.join(", "),
                    certified.issuer.max_duration / 86_400,
                    certified.issuer.expires
                ),
            ));
        }

        let issuer_id = certified.issuer.id.clone();
        license.issuer = Some(certified.signed.clone());
        let signed = certified
            .key
            .sign(license.to_cbor())
            .map_err(|error| refuse(ErrorCode::Internal, error.to_string()))?;
        let text = license_text(&signed);
        // Fail closed: hand out only what a workstation will accept.
        if !self
            .roots
            .iter()
            .any(|root| decode_license(&text, root).is_ok())
        {
            return Err(refuse(
                ErrorCode::Internal,
                "signed license does not verify",
            ));
        }
        self.register
            .append(&license, &issuer_id, now)
            .map_err(|error| refuse(ErrorCode::Internal, error.to_string()))?;
        self.recent.push_back(now);
        Ok(text)
    }

    fn status(&self, now: u64) -> Reply {
        let current = self.current(now).map(|entry| entry.issuer.id.clone());
        let (year, month, ..) = civil(now);
        let month_start = (0..31u64)
            .map(|days| now - now % 86_400 - days * 86_400)
            .take_while(|day| civil(*day).0 == year && civil(*day).1 == month)
            .last()
            .unwrap_or(now);
        Reply::Status {
            issuers: self
                .certified
                .iter()
                .map(|entry| IssuerStatus {
                    id: entry.issuer.id.clone(),
                    not_before: entry.issuer.not_before,
                    expires: entry.issuer.expires,
                    offers: entry.issuer.offers.clone(),
                    max_days: entry.issuer.max_duration / 86_400,
                    current: current.as_deref() == Some(entry.issuer.id.as_str()),
                })
                .collect(),
            signed_today: self.register.signed_since(now - now % 86_400),
            signed_this_month: self.register.signed_since(month_start),
        }
    }
}

#[cfg(test)]
mod tests;
