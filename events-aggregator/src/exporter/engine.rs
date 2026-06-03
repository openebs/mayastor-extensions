use crate::constant::{
    BATCH_MAX_SIZE, BATCH_TIMEOUT_MS, EVENTS_JSON_FILE, EVENTS_JSON_ROTATED_FILE, MAX_RETRIES,
};
use parse_size::parse_size;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{io::AsyncWriteExt, sync::mpsc};
use tracing::warn;

/// Parse a CLI directory size limit string into a `u64` byte count.
/// Supports human-readable values such as `10MiB`, `1.5GiB`, `10000`, and `8 KB`.
pub fn dir_size_limit(value: &str) -> Result<u64, String> {
    parse_size(value).map_err(|e| format!("invalid size '{value}': {e}"))
}

/// A single event produced by the aggregator pipeline.
/// `line` contains compact JSON for file export.
#[derive(Debug)]
pub struct LogEvent {
    /// Compact single-line JSON string to be written to the export file.
    pub line: String,
}

/// ExporterMode defines the target destination for log events.
pub enum ExporterMode {
    /// File mode writes events to log files with rotation based on size limits.
    File { dir: PathBuf, size_limit: u64 },
}

/// Main exporter loop that receives log events, batches them, and flushes to the
/// configured destination based on size or timeout triggers.
pub async fn run_exporter(mode: ExporterMode, mut rx: mpsc::Receiver<LogEvent>) {
    let mut batch: Vec<LogEvent> = Vec::with_capacity(BATCH_MAX_SIZE);
    let flush_timeout = Duration::from_millis(BATCH_TIMEOUT_MS);
    let mut timer_active = false;
    let mut sleep = Box::pin(tokio::time::sleep(Duration::MAX));

    let ExporterMode::File { dir, .. } = &mode;
    tracing::info!(target_path = ?dir, "Batch Exporter started in file mode.");

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                if batch.is_empty() && !timer_active {
                    sleep.as_mut().reset(tokio::time::Instant::now() + flush_timeout);
                    timer_active = true;
                }
                batch.push(event);
                if batch.len() >= BATCH_MAX_SIZE {
                    timer_active = false;
                    tracing::debug!(
                        action = "flush_trigger",
                        reason = "max_batch_size",
                        batch_len = batch.len(),
                        "Triggering flush due to max batch size"
                    );
                    flush_batch_with_retry(&mode, &mut batch).await;
                }
            }
            _ = &mut sleep, if timer_active => {
                timer_active = false;
                if !batch.is_empty() {
                    tracing::debug!(
                        action = "flush_trigger",
                        reason = "timeout",
                        batch_len = batch.len(),
                        "Triggering flush due to timeout"
                    );
                    flush_batch_with_retry(&mode, &mut batch).await;
                }
            }
            else => break,
        }
    }

    if !batch.is_empty() {
        tracing::debug!(
            action = "flush_trigger",
            reason = "shutdown",
            batch_len = batch.len(),
            "Triggering final flush on shutdown"
        );
        flush_batch_with_retry(&mode, &mut batch).await;
    }
}

async fn flush_batch_with_retry(mode: &ExporterMode, batch: &mut Vec<LogEvent>) {
    let mut attempts = 0;
    let mut backoff = Duration::from_millis(500);
    let mut success = false;

    while attempts < MAX_RETRIES {
        let ExporterMode::File { dir, size_limit } = mode;
        let result = write_to_disk(dir.as_path(), *size_limit, batch).await;

        match result {
            Ok(_) => {
                success = true;
                break;
            }
            Err(e) => {
                attempts += 1;
                warn!(attempt = attempts, error = %e, "Egress write failed; backing off before retry");
                tokio::time::sleep(backoff).await;
                backoff *= 2;
            }
        }
    }

    if !success {
        tracing::error!(
            "Failed to flush batch of {} events after {} retries; dropping batch",
            batch.len(),
            MAX_RETRIES
        );
    }

    batch.clear();
}

async fn write_to_disk(dir: &Path, size_limit: u64, batch: &[LogEvent]) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(dir).await?;

    let mut content = String::new();
    for event in batch {
        content.push_str(&event.line);
        content.push('\n');
    }

    let pending = content.len() as u64;
    tracing::debug!(dir = ?dir, pending_bytes = pending, "Preparing to write batch to disk");
    rotate_file_if_needed(dir, size_limit, pending).await?;

    let path = dir.join(EVENTS_JSON_FILE);
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await?;
    file.write_all(content.as_bytes()).await?;

    Ok(())
}

async fn rotate_file_if_needed(
    dir: &Path,
    size_limit: u64,
    pending_bytes: u64,
) -> anyhow::Result<()> {
    // Each file slot gets 40% of the total limit, leaving headroom for both current and rotated.
    let per_file_limit = size_limit * 40 / 100;

    if per_file_limit == 0 {
        anyhow::bail!(
            "--dir-size-limit ({size_limit} bytes) is too small; per-file limit rounds to 0"
        );
    }
    if pending_bytes > per_file_limit {
        anyhow::bail!(
            "pending batch ({pending_bytes} bytes) exceeds per-file limit ({per_file_limit} bytes); \
             refusing to write to avoid violating the configured size limit"
        );
    }

    let base_path = dir.join(EVENTS_JSON_FILE);
    let rotated_path = dir.join(EVENTS_JSON_ROTATED_FILE);

    let current_size = tokio::fs::metadata(&base_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    let mut rotated_size = tokio::fs::metadata(&rotated_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    tracing::debug!(
        current_size,
        rotated_size,
        pending_bytes,
        per_file_limit,
        "Checking whether rotation is required"
    );

    if current_size + pending_bytes > per_file_limit {
        if rotated_size > 0 {
            tokio::fs::remove_file(&rotated_path).await.ok();
            rotated_size = 0;
        }
        if current_size > 0 {
            tokio::fs::rename(&base_path, &rotated_path).await?;
            rotated_size = current_size;
        }
    }

    if rotated_size > per_file_limit {
        tokio::fs::remove_file(&rotated_path).await.ok();
    }

    Ok(())
}
