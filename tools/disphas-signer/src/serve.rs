//! The local Unix socket the site talks to (`docs/licences_en_ligne.md`
//! §6.3): one JSON request per line, one JSON reply per line. Never exposed
//! to the network.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::signer::{ErrorCode, Reply, Request, Signer};

/// Longest request line, in bytes (an activation request is about 150).
const MAX_LINE: u64 = 16 * 1024;
/// A connection idle this long is closed.
const IDLE: Duration = Duration::from_secs(30);

/// Serves `signer` on `socket` until the process is stopped.
pub fn serve(signer: Signer, socket: &Path, now: fn() -> u64) -> Result<(), String> {
    // Replace a socket left by a previous run, never any other file.
    if let Ok(metadata) = std::fs::symlink_metadata(socket) {
        if !metadata.file_type().is_socket() {
            return Err(format!("{} exists and is not a socket", socket.display()));
        }
        std::fs::remove_file(socket).map_err(|error| error.to_string())?;
    }
    let listener = UnixListener::bind(socket).map_err(|error| error.to_string())?;
    // The site's group connects; nobody else.
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o660))
        .map_err(|error| error.to_string())?;
    eprintln!("disphas-signer: listening on {}", socket.display());
    let signer = Arc::new(Mutex::new(signer));
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let signer = Arc::clone(&signer);
        std::thread::spawn(move || {
            if let Err(error) = connection(stream, &signer, now) {
                eprintln!("disphas-signer: connection: {error}");
            }
        });
    }
    Ok(())
}

/// Answers the requests of one connection, in order.
pub fn connection(
    stream: UnixStream,
    signer: &Mutex<Signer>,
    now: fn() -> u64,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(IDLE))?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        let read = Read::by_ref(&mut reader)
            .take(MAX_LINE + 1)
            .read_line(&mut line)?;
        if read == 0 {
            return Ok(());
        }
        let reply = if line.len() as u64 > MAX_LINE || !line.ends_with('\n') {
            // Too long or cut: answer once, then close.
            let reply = Reply::Error {
                error: ErrorCode::BadRequest,
                detail: "request too long".to_owned(),
            };
            write_reply(&mut writer, &reply)?;
            return Ok(());
        } else {
            match serde_json::from_str::<Request>(&line) {
                Ok(request) => match signer.lock() {
                    Ok(mut signer) => signer.handle(request, now()),
                    Err(_) => Reply::Error {
                        error: ErrorCode::Internal,
                        detail: "service state unavailable".to_owned(),
                    },
                },
                Err(error) => Reply::Error {
                    error: ErrorCode::BadRequest,
                    detail: error.to_string(),
                },
            }
        };
        write_reply(&mut writer, &reply)?;
    }
}

fn write_reply(writer: &mut UnixStream, reply: &Reply) -> std::io::Result<()> {
    let mut text = serde_json::to_string(reply).map_err(std::io::Error::other)?;
    text.push('\n');
    writer.write_all(text.as_bytes())?;
    writer.flush()
}
