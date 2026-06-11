use peregrine_config::types::History;
use peregrine_config::types::HistoryPersistence;
use serde::Deserialize;
use serde::Serialize;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Result;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncReadExt;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const HISTORY_FILENAME: &str = "history.jsonl";
const HISTORY_SOFT_CAP_RATIO: f64 = 0.8;
const MAX_RETRIES: usize = 10;
const RETRY_SLEEP: Duration = Duration::from_millis(100);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct HistoryEntry {
    pub(crate) session_id: String,
    pub(crate) ts: u64,
    pub(crate) text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HistoryConfig {
    pub(crate) peregrine_home: PathBuf,
    pub(crate) persistence: HistoryPersistence,
    pub(crate) max_bytes: Option<usize>,
}

impl HistoryConfig {
    pub(crate) fn new(peregrine_home: impl Into<PathBuf>, history: &History) -> Self {
        Self {
            peregrine_home: peregrine_home.into(),
            persistence: history.persistence,
            max_bytes: history.max_bytes,
        }
    }
}

fn history_filepath(config: &HistoryConfig) -> PathBuf {
    config.peregrine_home.join(HISTORY_FILENAME)
}

pub(crate) async fn append_entry(
    text: &str,
    conversation_id: impl std::fmt::Display,
    config: &HistoryConfig,
) -> Result<()> {
    match config.persistence {
        HistoryPersistence::SaveAll => {}
        HistoryPersistence::None => return Ok(()),
    }

    let path = history_filepath(config);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| std::io::Error::other(format!("system clock before Unix epoch: {err}")))?
        .as_secs();

    let entry = HistoryEntry {
        session_id: conversation_id.to_string(),
        ts,
        text: text.to_string(),
    };
    let mut line = serde_json::to_string(&entry).map_err(|err| {
        std::io::Error::other(format!("failed to serialize history entry: {err}"))
    })?;
    line.push('\n');

    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        options.append(true);
        options.mode(0o600);
    }

    let mut history_file = options.open(&path)?;
    ensure_owner_only_permissions(&history_file).await?;
    let history_max_bytes = config.max_bytes;

    tokio::task::spawn_blocking(move || -> Result<()> {
        for _ in 0..MAX_RETRIES {
            match history_file.try_lock() {
                Ok(()) => {
                    history_file.seek(SeekFrom::End(0))?;
                    history_file.write_all(line.as_bytes())?;
                    history_file.flush()?;
                    enforce_history_limit(&mut history_file, history_max_bytes)?;
                    return Ok(());
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    std::thread::sleep(RETRY_SLEEP);
                }
                Err(err) => return Err(err.into()),
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::WouldBlock,
            "could not acquire exclusive lock on history file after multiple attempts",
        ))
    })
    .await??;

    Ok(())
}

fn enforce_history_limit(file: &mut File, max_bytes: Option<usize>) -> Result<()> {
    let Some(max_bytes) = max_bytes else {
        return Ok(());
    };

    if max_bytes == 0 {
        return Ok(());
    }

    let max_bytes = match u64::try_from(max_bytes) {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };

    let mut current_len = file.metadata()?.len();
    if current_len <= max_bytes {
        return Ok(());
    }

    let mut reader_file = file.try_clone()?;
    reader_file.seek(SeekFrom::Start(0))?;
    let mut buf_reader = BufReader::new(reader_file);
    let mut line_lengths = Vec::new();
    let mut line_buf = String::new();

    loop {
        line_buf.clear();
        let bytes = buf_reader.read_line(&mut line_buf)?;
        if bytes == 0 {
            break;
        }
        line_lengths.push(bytes as u64);
    }

    if line_lengths.is_empty() {
        return Ok(());
    }

    let last_index = line_lengths.len() - 1;
    let trim_target = trim_target_bytes(max_bytes, line_lengths[last_index]);
    let mut drop_bytes = 0u64;
    let mut idx = 0usize;

    while current_len > trim_target && idx < last_index {
        current_len = current_len.saturating_sub(line_lengths[idx]);
        drop_bytes += line_lengths[idx];
        idx += 1;
    }

    if drop_bytes == 0 {
        return Ok(());
    }

    let mut reader = buf_reader.into_inner();
    reader.seek(SeekFrom::Start(drop_bytes))?;
    let capacity = usize::try_from(current_len).unwrap_or(0);
    let mut tail = Vec::with_capacity(capacity);
    reader.read_to_end(&mut tail)?;

    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&tail)?;
    file.flush()?;
    Ok(())
}

fn trim_target_bytes(max_bytes: u64, newest_entry_len: u64) -> u64 {
    let soft_cap_bytes = ((max_bytes as f64) * HISTORY_SOFT_CAP_RATIO)
        .floor()
        .clamp(1.0, max_bytes as f64) as u64;
    soft_cap_bytes.max(newest_entry_len)
}

pub(crate) async fn history_metadata(config: &HistoryConfig) -> (u64, usize) {
    let path = history_filepath(config);
    history_metadata_for_file(&path).await
}

pub(crate) fn lookup(log_id: u64, offset: usize, config: &HistoryConfig) -> Option<HistoryEntry> {
    let path = history_filepath(config);
    lookup_history_entry(&path, log_id, offset)
}

#[cfg(unix)]
async fn ensure_owner_only_permissions(file: &File) -> Result<()> {
    let metadata = file.metadata()?;
    let current_mode = metadata.permissions().mode() & 0o777;
    if current_mode != 0o600 {
        let mut perms = metadata.permissions();
        perms.set_mode(0o600);
        let perms_clone = perms.clone();
        let file_clone = file.try_clone()?;
        tokio::task::spawn_blocking(move || file_clone.set_permissions(perms_clone)).await??;
    }
    Ok(())
}

#[cfg(windows)]
async fn ensure_owner_only_permissions(_file: &File) -> Result<()> {
    Ok(())
}

async fn history_metadata_for_file(path: &Path) -> (u64, usize) {
    let log_id = match fs::metadata(path).await {
        Ok(metadata) => log_identity(&metadata).unwrap_or(0),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return (0, 0),
        Err(_) => return (0, 0),
    };

    let mut file = match fs::File::open(path).await {
        Ok(file) => file,
        Err(_) => return (log_id, 0),
    };

    let mut buf = [0u8; 8192];
    let mut count = 0usize;
    loop {
        match file.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                count += buf[..n].iter().filter(|&&b| b == b'\n').count();
            }
            Err(_) => return (log_id, 0),
        }
    }

    (log_id, count)
}

fn lookup_history_entry(path: &Path, log_id: u64, offset: usize) -> Option<HistoryEntry> {
    let file = match OpenOptions::new().read(true).open(path) {
        Ok(file) => file,
        Err(err) => {
            tracing::warn!(error = %err, "failed to open history file");
            return None;
        }
    };

    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(err) => {
            tracing::warn!(error = %err, "failed to stat history file");
            return None;
        }
    };

    let current_log_id = log_identity(&metadata)?;
    if log_id != 0 && current_log_id != log_id {
        return None;
    }

    for _ in 0..MAX_RETRIES {
        match file.try_lock_shared() {
            Ok(()) => {
                let reader = BufReader::new(&file);
                for (idx, line_res) in reader.lines().enumerate() {
                    let line = match line_res {
                        Ok(line) => line,
                        Err(err) => {
                            tracing::warn!(error = %err, "failed to read line from history file");
                            return None;
                        }
                    };

                    if idx == offset {
                        return match serde_json::from_str::<HistoryEntry>(&line) {
                            Ok(entry) => Some(entry),
                            Err(err) => {
                                tracing::warn!(error = %err, "failed to parse history entry");
                                None
                            }
                        };
                    }
                }
                return None;
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                std::thread::sleep(RETRY_SLEEP);
            }
            Err(err) => {
                tracing::warn!(error = %err, "failed to acquire shared lock on history file");
                return None;
            }
        }
    }

    None
}

#[cfg(unix)]
fn log_identity(metadata: &std::fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.ino())
}

#[cfg(windows)]
fn log_identity(metadata: &std::fs::Metadata) -> Option<u64> {
    use std::os::windows::fs::MetadataExt;
    Some(metadata.creation_time())
}

#[cfg(not(any(unix, windows)))]
fn log_identity(_metadata: &std::fs::Metadata) -> Option<u64> {
    None
}
