//! Prints the code labels as JSON, `{ "en": { "R4": "…" }, "fr": …, "de": … }`:
//! `cargo run -p oblivio-attestation --example labels_json`.

use oblivio_attestation::display::{KNOWN_CODES, Lang, code};

fn json_string(text: &str) -> String {
    let mut out = String::from('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn main() {
    let languages = [("en", Lang::En), ("fr", Lang::Fr), ("de", Lang::De)];
    let blocks: Vec<String> = languages
        .iter()
        .map(|(tag, lang)| {
            let entries: Vec<String> = KNOWN_CODES
                .iter()
                .filter_map(|id| {
                    code(id).map(|label| {
                        format!("    {}: {}", json_string(id), json_string(label.get(*lang)))
                    })
                })
                .collect();
            format!("  \"{tag}\": {{\n{}\n  }}", entries.join(",\n"))
        })
        .collect();
    println!("{{\n{}\n}}", blocks.join(",\n"));
}
