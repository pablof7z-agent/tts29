use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use crate::{LocalPublishRequest, LocalPublishResponse, MAX_LOCAL_FRAME_BYTES};

pub fn submit_local(
    socket_path: impl AsRef<Path>,
    request: &LocalPublishRequest,
) -> io::Result<LocalPublishResponse> {
    submit(socket_path.as_ref(), request, None)
}

pub fn submit_local_with_timeout(
    socket_path: impl AsRef<Path>,
    request: &LocalPublishRequest,
    timeout: Duration,
) -> io::Result<LocalPublishResponse> {
    submit(socket_path.as_ref(), request, Some(timeout))
}

fn submit(
    socket_path: &Path,
    request: &LocalPublishRequest,
    timeout: Option<Duration>,
) -> io::Result<LocalPublishResponse> {
    let payload = serde_json::to_vec(request).map_err(invalid_data)?;
    if payload.len() > MAX_LOCAL_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local request exceeds the frame limit",
        ));
    }
    let mut stream = UnixStream::connect(socket_path)?;
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    stream.write_all(&payload)?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let response = read_bounded(&mut stream)?;
    serde_json::from_slice(&response).map_err(invalid_data)
}

fn read_bounded(stream: &mut UnixStream) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    stream
        .take(MAX_LOCAL_FRAME_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_LOCAL_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "local frame exceeds the byte limit",
        ));
    }
    Ok(bytes)
}

fn invalid_data(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
