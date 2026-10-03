//! The NDJSON loop over stdin/stdout (`docs/protocole_ipc.md` §3, §4).

use std::io::{self, BufRead, Read, Write};
use std::time::Instant;

use oblivio_protocol::wire::DecodeError;
use oblivio_protocol::{ErrorCode, MAX_MESSAGE_BYTES, Request, Response, ServerMessage};
use zeroize::{Zeroize, Zeroizing};

use crate::backend::DeviceBackend;
use crate::session::Session;

/// Outcome of reading one line.
enum LineRead {
    /// A complete line, newline stripped.
    Line,
    /// End of input: the UI closed the pipe.
    Eof,
    /// The line exceeds [`MAX_MESSAGE_BYTES`].
    TooLong,
}

/// Serves requests until end of input, `Shutdown`, or a message that forces
/// the connection closed. Diagnostics go to stderr, never message contents.
pub fn serve<R: BufRead, W: Write, B: DeviceBackend>(
    mut reader: R,
    mut writer: W,
    session: &mut Session<B>,
) -> io::Result<()> {
    // The line may hold an ATA password: wipe it after every message.
    let mut line = Zeroizing::new(Vec::new());
    loop {
        let read = read_line(&mut reader, &mut line);
        let response = match read? {
            LineRead::Eof => return Ok(()),
            LineRead::TooLong => {
                eprintln!("oblivio-helper: message over {MAX_MESSAGE_BYTES} bytes, closing");
                return Ok(());
            }
            LineRead::Line => respond(&line, session),
        };
        line.zeroize();
        let Some(response) = response else {
            eprintln!("oblivio-helper: unanswerable message, closing");
            return Ok(());
        };
        write_message(&mut writer, &ServerMessage::Response(response))?;
        if session.is_shut_down() {
            return Ok(());
        }
    }
}

/// The response to one line, or `None` when it cannot be answered.
fn respond<B: DeviceBackend>(line: &[u8], session: &mut Session<B>) -> Option<Response> {
    let text = std::str::from_utf8(line).ok()?;
    match Request::decode(text) {
        Ok(request) => Some(session.handle(request, Instant::now())),
        Err(error) => rejection(&error),
    }
}

fn rejection(error: &DecodeError) -> Option<Response> {
    if error.closes_connection() {
        return None;
    }
    let id = error.request_id?;
    Some(Response::failure(
        id,
        error.error_code(),
        Some(error.to_string()),
    ))
}

fn read_line<R: BufRead>(reader: &mut R, buffer: &mut Vec<u8>) -> io::Result<LineRead> {
    buffer.clear();
    let limit = MAX_MESSAGE_BYTES as u64;
    let read = reader.by_ref().take(limit).read_until(b'\n', buffer)?;
    if read == 0 {
        return Ok(LineRead::Eof);
    }
    if buffer.last() == Some(&b'\n') {
        buffer.pop();
        return Ok(LineRead::Line);
    }
    if read as u64 == limit {
        return Ok(LineRead::TooLong);
    }
    // Last line without a trailing newline.
    Ok(LineRead::Line)
}

fn write_message<W: Write>(writer: &mut W, message: &ServerMessage) -> io::Result<()> {
    let encoded = message.encode().or_else(|error| {
        let id = match message {
            ServerMessage::Response(response) => response.id,
            ServerMessage::Event(_) => 0,
        };
        ServerMessage::Response(Response::failure(
            id,
            ErrorCode::Internal,
            Some(error.to_string()),
        ))
        .encode()
    });
    let line = encoded.map_err(io::Error::other)?;
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NoDevices;
    use crate::session::SessionConfig;
    use oblivio_core::RuleSet;
    use oblivio_protocol::response::{LicenseState, LicenseStatusResult};

    fn session() -> Session<NoDevices> {
        Session::new(
            NoDevices,
            SessionConfig {
                rules: RuleSet::baseline(),
                binary_sha256: "test".to_owned(),
                license: LicenseStatusResult {
                    state: LicenseState::Missing,
                    offer: None,
                    license_id: None,
                    valid_until: None,
                },
            },
        )
    }

    fn run(input: &str) -> Vec<Response> {
        let mut output = Vec::new();
        serve(input.as_bytes(), &mut output, &mut session()).expect("in-memory I/O");
        String::from_utf8(output)
            .expect("UTF-8 output")
            .lines()
            .map(
                |line| match ServerMessage::decode(line).expect("valid message") {
                    ServerMessage::Response(response) => response,
                    ServerMessage::Event(event) => panic!("unexpected event {event:?}"),
                },
            )
            .collect()
    }

    const HELLO: &str = r#"{"type":"request","id":1,"cmd":"Hello","args":{"protocol_version":1,"app_version":"test"}}"#;

    #[test]
    fn answers_each_request_in_order_and_stops_at_end_of_input() {
        let input = format!(
            "{HELLO}\n{}\n",
            r#"{"type":"request","id":2,"cmd":"ListDevices"}"#
        );
        let responses = run(&input);
        assert_eq!(responses.iter().map(|r| r.id).collect::<Vec<_>>(), [1, 2]);
        assert!(responses.iter().all(|r| r.outcome.is_ok()));
    }

    #[test]
    fn malformed_request_with_id_gets_an_error_and_the_session_goes_on() {
        let input = format!(
            "{HELLO}\n{}\n{}\n",
            r#"{"type":"request","id":2,"cmd":"Erase"}"#,
            r#"{"type":"request","id":3,"cmd":"ListJobs"}"#
        );
        let responses = run(&input);
        assert_eq!(responses.len(), 3);
        assert_eq!(
            responses[1].outcome.as_ref().expect_err("rejected").code,
            ErrorCode::UnknownCommand
        );
        assert!(responses[2].outcome.is_ok());
    }

    #[test]
    fn message_without_readable_id_closes_the_connection() {
        let input = format!("{HELLO}\nnot json\n{HELLO}\n");
        assert_eq!(run(&input).len(), 1);
    }

    #[test]
    fn oversized_message_closes_the_connection() {
        let input = format!("{HELLO}\n{}\n{HELLO}\n", "x".repeat(MAX_MESSAGE_BYTES));
        assert_eq!(run(&input).len(), 1);
    }

    #[test]
    fn shutdown_stops_serving() {
        let input = format!(
            "{HELLO}\n{}\n{HELLO}\n",
            r#"{"type":"request","id":2,"cmd":"Shutdown"}"#
        );
        assert_eq!(run(&input).len(), 2);
    }
}
