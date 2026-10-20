//! The NDJSON loop over stdin/stdout (`docs/protocole_ipc.md` §3, §4).

use std::io::{self, BufRead, Read, Write};
use std::sync::mpsc;
use std::time::Instant;

use disphas_protocol::wire::DecodeError;
use disphas_protocol::{ErrorCode, MAX_MESSAGE_BYTES, Request, Response, ServerMessage};
use zeroize::{Zeroize, Zeroizing};

use crate::backend::DeviceBackend;
use crate::session::Session;

/// Messages waiting for the writer thread. Responses wait for room (the
/// helper stops reading requests from a client that does not read its
/// answers); job events are dropped instead, so a job never stalls.
const OUTPUT_QUEUE: usize = 256;

/// Longest error detail echoed in a rejection: the detail may quote the
/// offending input.
const MAX_DETAIL_BYTES: usize = 256;

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
/// the connection closed, then waits for running jobs: a job always runs to
/// its end, even if the UI is gone (§3). Responses and job events share one
/// writer thread. Diagnostics go to stderr, never message contents.
/// Returns the writer once everything is written.
pub fn serve<R: BufRead, W: Write + Send + 'static, B: DeviceBackend>(
    mut reader: R,
    writer: W,
    session: &mut Session<B>,
) -> io::Result<W> {
    let (sender, receiver) = mpsc::sync_channel::<ServerMessage>(OUTPUT_QUEUE);
    let output = std::thread::spawn(move || -> io::Result<W> {
        let mut writer = writer;
        for message in receiver {
            write_message(&mut writer, &message)?;
        }
        Ok(writer)
    });
    session.set_events(Some(sender.clone()));

    let read = read_requests(&mut reader, &sender, session);

    session.wait_for_jobs();
    session.set_events(None);
    drop(sender);
    let writer = output
        .join()
        .map_err(|_| io::Error::other("output thread panicked"))??;
    read.map(|()| writer)
}

fn read_requests<R: BufRead, B: DeviceBackend>(
    reader: &mut R,
    output: &mpsc::SyncSender<ServerMessage>,
    session: &mut Session<B>,
) -> io::Result<()> {
    // The line may hold an ATA password: wipe it after every message.
    let mut line = Zeroizing::new(Vec::new());
    loop {
        let read = read_line(reader, &mut line);
        let response = match read? {
            LineRead::Eof => return Ok(()),
            LineRead::TooLong => {
                eprintln!("disphas-helper: message over {MAX_MESSAGE_BYTES} bytes, closing");
                return Ok(());
            }
            LineRead::Line => respond(&line, session),
        };
        line.zeroize();
        let Some(response) = response else {
            eprintln!("disphas-helper: unanswerable message, closing");
            return Ok(());
        };
        if output.send(ServerMessage::Response(response)).is_err() {
            return Err(io::Error::other("output closed"));
        }
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
        Some(truncated(error.to_string(), MAX_DETAIL_BYTES)),
    ))
}

/// `text` cut to at most `max` bytes, on a character boundary.
fn truncated(mut text: String, max: usize) -> String {
    if text.len() > max {
        let mut end = max;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push('…');
    }
    text
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
    use disphas_core::RuleSet;

    fn session() -> Session<NoDevices> {
        Session::new(
            NoDevices,
            SessionConfig {
                rules: RuleSet::baseline(),
                entitlements: disphas_core::EntitlementTable::baseline(),
                binary_sha256: [0; 32],
                reports_dir: std::env::temp_dir().join("disphas-unused-reports"),
                jobs_dir: None,
                licensing: Box::new(crate::licensing::StoreLicensing::new(
                    disphas_license::LicenseStore::new("/nonexistent/disphas-test"),
                    Vec::new(),
                )),
            },
        )
    }

    fn run(input: &str) -> Vec<Response> {
        let output = serve(input.as_bytes(), Vec::new(), &mut session()).expect("in-memory I/O");
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

    const HELLO: &str = r#"{"type":"request","id":1,"cmd":"Hello","args":{"protocol_version":2,"app_version":"test"}}"#;

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
    fn echoed_input_is_truncated() {
        let long = "é".repeat(MAX_MESSAGE_BYTES / 4);
        let input = format!("{HELLO}\n{{\"type\":\"request\",\"id\":2,\"cmd\":\"{long}\"}}\n");
        let responses = run(&input);
        let error = responses[1].outcome.as_ref().expect_err("rejected");
        assert_eq!(error.code, ErrorCode::UnknownCommand);
        let detail = error.detail.as_deref().expect("detail");
        assert!(
            detail.len() <= MAX_DETAIL_BYTES + '…'.len_utf8(),
            "{}",
            detail.len()
        );
    }

    #[test]
    fn a_client_that_does_not_read_blocks_the_helper_without_growing_memory() {
        // A writer that never accepts anything stands for a client that
        // never reads: the queue fills, then reading stops.
        struct Stuck(std::sync::mpsc::Receiver<()>);
        impl Write for Stuck {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                let _ = self.0.recv();
                Err(io::Error::other("closed"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let request = r#"{"type":"request","id":2,"cmd":"ListJobs"}"#;
        let count = OUTPUT_QUEUE * 4;
        let input = format!("{HELLO}\n{}", format!("{request}\n").repeat(count));
        let consumed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        struct Counting<'a>(&'a [u8], std::sync::Arc<std::sync::atomic::AtomicUsize>);
        impl Read for Counting<'_> {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                let read = self.0.read(buffer)?;
                self.1.fetch_add(read, std::sync::atomic::Ordering::SeqCst);
                Ok(read)
            }
        }
        let (release, stuck) = std::sync::mpsc::channel();
        let total = input.len();
        let seen = consumed.clone();
        let server = std::thread::spawn(move || {
            let reader = io::BufReader::with_capacity(64, Counting(input.as_bytes(), seen));
            serve(reader, Stuck(stuck), &mut session()).map(|_| ())
        });
        std::thread::sleep(std::time::Duration::from_millis(300));
        let read = consumed.load(std::sync::atomic::Ordering::SeqCst);
        assert!(read < total / 2, "read {read} of {total} bytes");
        drop(release);
        assert!(server.join().expect("no panic").is_err(), "output closed");
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
