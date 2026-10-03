//! Certificate texts in English, French and German (Project Fluent).
//!
//! Code labels come from `oblivio_attestation::display`; only the wording of
//! the certificate itself lives here.

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use oblivio_attestation::display::Lang;
use unic_langid::LanguageIdentifier;

use crate::ReportError;

const EN: &str = include_str!("i18n/en.ftl");
const FR: &str = include_str!("i18n/fr.ftl");
const DE: &str = include_str!("i18n/de.ftl");

/// Translations for one language.
pub(crate) struct Translator {
    lang: Lang,
    bundle: FluentBundle<FluentResource>,
}

impl Translator {
    pub(crate) fn new(lang: Lang) -> Result<Self, ReportError> {
        let (tag, source) = match lang {
            Lang::En => ("en", EN),
            Lang::Fr => ("fr", FR),
            Lang::De => ("de", DE),
        };
        let langid: LanguageIdentifier = tag
            .parse()
            .map_err(|_| ReportError::Translation(format!("language {tag}")))?;
        let resource = FluentResource::try_new(source.to_owned())
            .map_err(|_| ReportError::Translation(format!("{tag}.ftl does not parse")))?;
        let mut bundle = FluentBundle::new(vec![langid]);
        // No Unicode isolation marks: they would end up in the PDF text.
        bundle.set_use_isolating(false);
        bundle
            .add_resource(resource)
            .map_err(|_| ReportError::Translation(format!("{tag}.ftl has duplicates")))?;
        Ok(Self { lang, bundle })
    }

    pub(crate) fn lang(&self) -> Lang {
        self.lang
    }

    /// The text of `id`; a missing text is an error, never a silent blank.
    pub(crate) fn text(&self, id: &str) -> Result<String, ReportError> {
        self.format(id, None)
    }

    /// The text of `id` with named arguments.
    pub(crate) fn with(&self, id: &str, args: &[(&str, String)]) -> Result<String, ReportError> {
        let mut fluent_args = FluentArgs::new();
        for (name, value) in args {
            fluent_args.set(*name, value.clone());
        }
        self.format(id, Some(&fluent_args))
    }

    fn format(&self, id: &str, args: Option<&FluentArgs>) -> Result<String, ReportError> {
        let missing = || ReportError::Translation(format!("missing text {id:?}"));
        let pattern = self
            .bundle
            .get_message(id)
            .and_then(|message| message.value())
            .ok_or_else(missing)?;
        let mut errors = Vec::new();
        let text = self.bundle.format_pattern(pattern, args, &mut errors);
        if errors.is_empty() {
            Ok(text.into_owned())
        } else {
            Err(ReportError::Translation(format!("text {id:?}: {errors:?}")))
        }
    }

    /// An integer with the thousands separator of the language.
    pub(crate) fn integer(&self, value: u64) -> String {
        group(&value.to_string(), self.thousands())
    }

    /// A value with one decimal, with the separators of the language.
    pub(crate) fn decimal(&self, tenths: u64) -> String {
        let decimal = match self.lang {
            Lang::En => '.',
            Lang::Fr | Lang::De => ',',
        };
        format!(
            "{}{decimal}{}",
            group(&(tenths / 10).to_string(), self.thousands()),
            tenths % 10
        )
    }

    fn thousands(&self) -> &'static str {
        match self.lang {
            Lang::En => ",",
            Lang::Fr => "\u{202f}", // narrow no-break space
            Lang::De => ".",
        }
    }
}

fn group(digits: &str, separator: &str) -> String {
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push_str(separator);
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDS: &[&str] = &[
        "title",
        "report",
        "issued",
        "status-compliant",
        "status-non-compliant",
        "status-refused",
        "banner-compliant",
        "banner-non-compliant",
        "banner-refused",
        "banner-destroy-recommended",
        "section-drive",
        "model",
        "serial",
        "firmware",
        "capacity",
        "drive-type",
        "identifiers",
        "section-sanitization",
        "category",
        "method",
        "standards",
        "verification",
        "validation",
        "passed",
        "failed",
        "not-run",
        "canaries",
        "pattern",
        "section-future",
        "future",
        "reuse",
        "destroy-recommended",
        "reserves",
        "refusals",
        "none",
        "justification",
        "section-issuer",
        "organization",
        "operator",
        "tool",
        "license",
        "fingerprint",
        "verify-hint",
        "annex",
        "annex-intro",
        "seed",
        "rules-version",
        "binary-hash",
        "journal-hash",
        "attestation-hash",
        "attachments",
        "attachments-value",
        "legal",
    ];

    #[test]
    fn every_text_exists_in_every_language() {
        for lang in [Lang::En, Lang::Fr, Lang::De] {
            let translator = Translator::new(lang).expect("bundle");
            for id in IDS {
                let text = translator
                    .text(id)
                    .unwrap_or_else(|error| panic!("{error}"));
                assert!(!text.is_empty(), "{id}");
            }
            translator
                .with(
                    "capacity-value",
                    &[("bytes", "1".to_owned()), ("gb", "0".to_owned())],
                )
                .expect("capacity-value");
        }
    }

    #[test]
    fn missing_text_is_an_error() {
        let translator = Translator::new(Lang::Fr).expect("bundle");
        assert!(translator.text("does-not-exist").is_err());
    }

    #[test]
    fn numbers_follow_the_language() {
        let en = Translator::new(Lang::En).expect("bundle");
        let fr = Translator::new(Lang::Fr).expect("bundle");
        let de = Translator::new(Lang::De).expect("bundle");
        assert_eq!(en.integer(1_024_209_543_168), "1,024,209,543,168");
        assert_eq!(
            fr.integer(1_024_209_543_168),
            "1\u{202f}024\u{202f}209\u{202f}543\u{202f}168"
        );
        assert_eq!(de.integer(999), "999");
        assert_eq!(en.decimal(10_242), "1,024.2");
        assert_eq!(de.decimal(10_242), "1.024,2");
        assert_eq!(fr.decimal(5), "0,5");
    }
}
