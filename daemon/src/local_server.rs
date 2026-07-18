use std::fs::{self, DirBuilder, Permissions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::{
    LocalPublishRequest, LocalPublishResponse, LocalPublishService, MAX_LOCAL_FRAME_BYTES,
};

const IO_TIMEOUT: Duration = Duration::from_secs(5);

pub struct PrivateUnixListener {
    listener: UnixListener,
    path: PathBuf,
    device: u64,
    inode: u64,
}

#[derive(Clone)]
pub struct LocalServerShutdown {
    requested: Arc<AtomicBool>,
    socket_path: PathBuf,
}

impl LocalServerShutdown {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            requested: Arc::new(AtomicBool::new(false)),
            socket_path: socket_path.into(),
        }
    }

    pub fn request(&self) {
        if !self.requested.swap(true, Ordering::AcqRel) {
            let _ = UnixStream::connect(&self.socket_path);
        }
    }

    fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }
}

impl PrivateUnixListener {
    pub fn bind(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let parent = path
            .parent()
            .filter(|value| !value.as_os_str().is_empty())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "socket path needs a parent directory",
                )
            })?;
        prepare_parent(parent)?;
        remove_stale_socket(&path)?;
        let listener = UnixListener::bind(&path)?;
        fs::set_permissions(&path, Permissions::from_mode(0o600))?;
        let metadata = fs::symlink_metadata(&path)?;
        Ok(Self {
            listener,
            path,
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn accept(&self) -> io::Result<UnixStream> {
        self.listener.accept().map(|(stream, _)| stream)
    }
}

impl Drop for PrivateUnixListener {
    fn drop(&mut self) {
        let _ = remove_if_same_socket(&self.path, self.device, self.inode);
    }
}

pub fn serve_forever<S: LocalPublishService>(
    listener: &PrivateUnixListener,
    service: &mut S,
) -> io::Result<()> {
    loop {
        let stream = listener.accept()?;
        let _ = serve_stream(stream, service);
    }
}

pub fn serve_until_shutdown<S: LocalPublishService>(
    listener: &PrivateUnixListener,
    service: &mut S,
    shutdown: &LocalServerShutdown,
) -> io::Result<()> {
    while !shutdown.is_requested() {
        let stream = listener.accept()?;
        if shutdown.is_requested() {
            return Ok(());
        }
        let _ = serve_stream(stream, service);
    }
    Ok(())
}

pub fn serve_one<S: LocalPublishService>(
    listener: &PrivateUnixListener,
    service: &mut S,
) -> io::Result<()> {
    serve_stream(listener.accept()?, service)
}

fn serve_stream<S: LocalPublishService>(mut stream: UnixStream, service: &mut S) -> io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let response = match read_bounded(&mut stream) {
        Ok(bytes) if bytes.is_empty() => {
            LocalPublishResponse::error("malformed_request", "local request is empty")
        }
        Ok(bytes) => match serde_json::from_slice::<LocalPublishRequest>(&bytes) {
            Ok(request) => match request.validate() {
                Ok(()) => service.publish_local(request),
                Err(error) => LocalPublishResponse::error(error.code, error.message),
            },
            Err(_) => LocalPublishResponse::error(
                "malformed_request",
                "local request is not valid protocol JSON",
            ),
        },
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            LocalPublishResponse::error("request_too_large", error.to_string())
        }
        Err(error) => LocalPublishResponse::error("request_read_failed", error.to_string()),
    };
    let bytes = serde_json::to_vec(&response).map_err(invalid_data)?;
    stream.write_all(&bytes)?;
    stream.shutdown(std::net::Shutdown::Write)
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

fn prepare_parent(parent: &Path) -> io::Result<()> {
    if !parent.exists() {
        let mut builder = DirBuilder::new();
        builder.recursive(true).mode(0o700).create(parent)?;
    }
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "socket parent must be a real directory",
        ));
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socket parent must not be accessible by group or other users",
        ));
    }
    Ok(())
}

fn remove_stale_socket(path: &Path) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_socket() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "socket path exists and is not a Unix socket",
        ));
    }
    match UnixStream::connect(path) {
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "another daemon is already listening",
        )),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
            ) =>
        {
            remove_if_same_socket(path, metadata.dev(), metadata.ino())
        }
        Err(error) => Err(error),
    }
}

fn remove_if_same_socket(path: &Path, device: u64, inode: u64) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_socket() && metadata.dev() == device && metadata.ino() == inode {
        fs::remove_file(path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "socket path changed while checking ownership",
        ))
    }
}

fn invalid_data(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
