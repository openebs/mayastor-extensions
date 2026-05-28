mod client;
use async_nats::jetstream::message::AckKind;
use client::nats_client::{NatsManager, UnifiedMessage};
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

/// Result of message processing.
/// `Ok` = successfully processed, ACK the message
/// `Err(Transient)` = processing failed but might succeed on retry, NAK the message
/// `Err(Permanent)` = unrecoverable failure (e.g., invalid JSON), ACK and discard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum ProcessingResult {
    Success,
    TransientFailure,
    PermanentFailure,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize structured logging with tracing-subscriber
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    // Fetch configuration from environment variables.
    let nats_url = match std::env::var("NATS_URL") {
        Ok(value) => value.trim().to_string(),
        Err(_) => anyhow::bail!("Environment variable 'NATS_URL' is not set"),
    };

    let jetstream_enabled = std::env::var("JETSTREAM_ENABLED")
        .unwrap_or_else(|_| "false".to_string())
        .trim()
        .eq_ignore_ascii_case("true");
    let subject = "events.>".to_string();

    // Setup Internal Channel
    let (tx, mut rx) = mpsc::channel::<UnifiedMessage>(512);

    info!("⚡🚀 [EVENT AGGREGATOR] Engine online and listening for cluster events...");

    // Initialize NATS Manager
    let nats_mgr = NatsManager::new(&nats_url).await?;
    nats_mgr
        .start_subscribing(subject, jetstream_enabled, tx)
        .await?;

    // Unified Processor Loop
    while let Some(unified_msg) = rx.recv().await {
        match unified_msg {
            UnifiedMessage::JetStream(js_msg) => {
                let subject_str = &js_msg.message.subject;
                let payload_bytes = &js_msg.message.payload;

                let result = process_message(subject_str, payload_bytes);

                match result {
                    ProcessingResult::Success => {
                        if let Err(err) = js_msg.ack().await {
                            warn!(
                                subject = %subject_str,
                                error = %err,
                                "Failed to ACK JetStream message"
                            );
                        }
                    }
                    ProcessingResult::TransientFailure => {
                        // Transient failure: NAK with backoff for retry/redelivery
                        let backoff_secs = 1 + rand::random::<u64>() % 5;
                        let delay = Duration::from_secs(backoff_secs);
                        if let Err(err) = js_msg.ack_with(AckKind::Nak(Some(delay))).await {
                            warn!(
                                subject = %subject_str,
                                error = %err,
                                "Transient failure: failed to NAK JetStream message for retry"
                            );
                        } else {
                            warn!(
                                subject = %subject_str,
                                "Transient failure; NAKed JetStream message for retry"
                            );
                        }
                    }
                    ProcessingResult::PermanentFailure => {
                        // Permanent failure (e.g., invalid JSON): ACK and discard to avoid wasted redeliveries
                        if let Err(err) = js_msg.ack().await {
                            warn!(
                                subject = %subject_str,
                                error = %err,
                                "Permanent failure: failed to ACK JetStream message"
                            );
                        } else {
                            warn!(
                                subject = %subject_str,
                                "Permanent failure; ACKed JetStream message (no retry)"
                            );
                        }
                    }
                }
            }
            UnifiedMessage::Core(core_msg) => {
                // Core NATS is fire-and-forget: there is no ACK/NAK or retry path,
                // so we intentionally ignore the result here.
                let _ = process_message(&core_msg.subject, &core_msg.payload);
            }
        }
    }

    Ok(())
}

fn process_message(subject_str: &str, payload_bytes: &[u8]) -> ProcessingResult {
    match serde_json::from_slice::<Value>(payload_bytes) {
        Ok(json_payload) => {
            let log_line = serde_json::json!({
                "fields": {
                    "subject": subject_str,
                    "payload": json_payload
                },
            });

            match colored_json::to_colored_json_auto(&log_line) {
                Ok(colored_str) => {
                    info!("\n{colored_str}\n");
                }
                Err(err) => {
                    warn!(
                        subject = %subject_str,
                        error = %err,
                        "Failed to format event as colored JSON; falling back to plain JSON"
                    );
                    match serde_json::to_string_pretty(&log_line) {
                        Ok(plain_json) => {
                            info!("\n{plain_json}\n");
                        }
                        Err(serialize_err) => {
                            warn!(
                                subject = %subject_str,
                                error = %serialize_err,
                                "Failed to serialize event for plain JSON console output"
                            );
                        }
                    }
                }
            }

            // Successfully parsed and processed the JSON payload
            ProcessingResult::Success
        }
        Err(e) => {
            const MAX_NON_JSON_PAYLOAD_LOG_BYTES: usize = 4096;
            let payload_size_bytes = payload_bytes.len();

            // Safely truncate at UTF-8 boundary to avoid splitting multi-byte characters
            let preview_len = payload_size_bytes.min(MAX_NON_JSON_PAYLOAD_LOG_BYTES);
            let safe_preview_len = if preview_len < payload_size_bytes {
                // If truncating, find the last valid UTF-8 boundary
                payload_bytes[..preview_len]
                    .iter()
                    .rposition(|&b| (b & 0xC0) != 0x80)
                    .map(|pos| pos + 1)
                    .unwrap_or(preview_len)
            } else {
                preview_len
            };

            let payload_preview = String::from_utf8_lossy(&payload_bytes[..safe_preview_len]);
            let payload_truncated = payload_size_bytes > MAX_NON_JSON_PAYLOAD_LOG_BYTES;

            warn!(
                subject = %subject_str,
                payload_preview = %payload_preview,
                payload_size_bytes = payload_size_bytes,
                payload_truncated = payload_truncated,
                error = %e,
                "Received non-JSON event payload; treating as permanent failure (will ACK, not retry)",
            );

            // Non-JSON payloads are permanent failures: ACK without retry to avoid wasted redelivery cycles
            ProcessingResult::PermanentFailure
        }
    }
}
