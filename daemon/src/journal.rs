use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use crate::JobRecord;

static TEMP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub enum JournalError {
    Io(std::io::Error),
    InvalidId(String),
    Corrupt(String),
}

impl std::fmt::Display for JournalError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(output, "job journal I/O failed: {error}"),
            Self::InvalidId(id) => write!(output, "job journal request id is invalid: {id}"),
            Self::Corrupt(id) => write!(output, "job journal record is corrupt: {id}"),
        }
    }
}

impl std::error::Error for JournalError {}

impl From<std::io::Error> for JournalError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted,
    AlreadyExists,
}

pub trait JobJournal {
    fn load(&mut self, request_id: &str) -> Result<Option<JobRecord>, JournalError>;
    fn insert(&mut self, job: &JobRecord) -> Result<InsertOutcome, JournalError>;
    fn save(&mut self, job: &JobRecord) -> Result<(), JournalError>;
}

pub struct FileJobJournal {
    root: PathBuf,
}

impl FileJobJournal {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, JournalError> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn record_path(&self, request_id: &str) -> Result<PathBuf, JournalError> {
        if !valid_request_id(request_id) {
            return Err(JournalError::InvalidId(request_id.to_string()));
        }
        Ok(self.root.join(format!("{request_id}.json")))
    }

    fn staged_path(&self, request_id: &str) -> PathBuf {
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        self.root
            .join(format!(".{request_id}.{}.{}.tmp", std::process::id(), id))
    }

    fn write_staged(&self, job: &JobRecord) -> Result<PathBuf, JournalError> {
        let path = self.staged_path(&job.request.request_id);
        let bytes = serde_json::to_vec_pretty(job)
            .map_err(|_| JournalError::Corrupt(job.request.request_id.clone()))?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&path)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        Ok(path)
    }

    fn sync_root(&self) -> Result<(), JournalError> {
        File::open(&self.root)?.sync_all()?;
        Ok(())
    }
}

impl JobJournal for FileJobJournal {
    fn load(&mut self, request_id: &str) -> Result<Option<JobRecord>, JournalError> {
        let path = self.record_path(request_id)?;
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| JournalError::Corrupt(request_id.to_string()))
    }

    fn insert(&mut self, job: &JobRecord) -> Result<InsertOutcome, JournalError> {
        let staged = self.write_staged(job)?;
        let destination = self.record_path(&job.request.request_id)?;
        let result = match fs::hard_link(&staged, destination) {
            Ok(()) => {
                self.sync_root()?;
                InsertOutcome::Inserted
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                InsertOutcome::AlreadyExists
            }
            Err(error) => return Err(error.into()),
        };
        remove_staged(&staged)?;
        Ok(result)
    }

    fn save(&mut self, job: &JobRecord) -> Result<(), JournalError> {
        let staged = self.write_staged(job)?;
        fs::rename(&staged, self.record_path(&job.request.request_id)?)?;
        self.sync_root()
    }
}

fn remove_staged(path: &Path) -> Result<(), JournalError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}
