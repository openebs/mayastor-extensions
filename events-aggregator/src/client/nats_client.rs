use anyhow::bail;
use async_nats::jetstream::consumer::pull;
use futures::StreamExt;
use rand::Rng;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub struct NatsManager {
    client: async_nats::Client,
}

pub enum UnifiedMessage {
    JetStream(async_nats::jetstream::Message),
    Core(async_nats::Message),
}

// JetStream consumer name (used for both lookup and durable identity)
const JETSTREAM_CONSUMER_NAME: &str = "events-aggregator-consumer";
// JetStream stream name (used for lookup and validation)
const JETSTREAM_STREAM_NAME: &str = "events-stream";

// NATS Manager Implementation
// 1. Connection Management with Strict Timeouts and Retry Strategy
// 2. JetStream Verification with Focused Backoff and Timeout Controls
// 3. Dual Subscription Setup: Core NATS with Resilient Reconnection Logic,
// and JetStream Pull Consumer with Backpressure Handling
impl NatsManager {
    // Calculate exponential backoff delay in milliseconds.
    // Starts at 500ms for attempt=1, doubles each subsequent attempt, caps at 30 seconds.
    fn calculate_backoff_delay(attempt: u32) -> u64 {
        let base_delay = 500u64;
        let exponential_delay = base_delay << std::cmp::min(attempt.saturating_sub(1), 6);
        std::cmp::min(exponential_delay, 30000)
    }

    // Apply exponential backoff and sleep.
    async fn apply_backoff(attempt: u32) {
        let delay = Self::calculate_backoff_delay(attempt);
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }

    pub async fn new(url: &str) -> anyhow::Result<Self> {
        info!("Attempting to connect to NATS at {}...", url);

        // Configure native NATS backoff and retries
        let options = async_nats::ConnectOptions::new()
            .connection_timeout(Duration::from_secs(1))
            .request_timeout(Some(Duration::from_secs(5)))
            // Enable retries on initial connection failure
            .retry_on_initial_connect()
            .reconnect_delay_callback(|attempts| {
                // Reuse the same capped exponential backoff logic used elsewhere in this type
                // so reconnect delays remain safe and consistent.
                let base_delay = Self::calculate_backoff_delay(attempts.saturating_add(1) as u32);
                let jitter = rand::thread_rng().gen_range(0..=(base_delay / 2));
                Duration::from_millis(std::cmp::min(base_delay.saturating_add(jitter), 30_000))
            });

        // A single deterministic connect call. It will inherently respect
        // the timeouts and backoffs configured above without race conditions.
        let client = options.connect(url).await.map_err(|e| {
            anyhow::anyhow!("Failed to connect to NATS after configured attempts: {}", e)
        })?;

        Ok(Self { client })
    }
    pub async fn start_subscribing(
        &self,
        subject: String,
        jetstream_enabled: bool,
        tx: mpsc::Sender<UnifiedMessage>,
    ) -> anyhow::Result<()> {
        // Check the environment-derived flag (e.g., JETSTREAM_ENABLED=true)
        if !jetstream_enabled {
            info!(
                "JetStream is explicitly disabled via env config. Routing directly to Core NATS loop."
            );
            self.setup_core_nats(subject, tx).await?;
            return Ok(());
        }

        info!("JetStream is enabled. Booting JetStream consumer with retry support...");
        self.setup_jetstream(subject, tx).await?;
        Ok(())
    }

    // JetStream Consumer Setup with Backpressure Handling
    async fn setup_jetstream(
        &self,
        subject: String,
        tx: mpsc::Sender<UnifiedMessage>,
    ) -> anyhow::Result<()> {
        let js = async_nats::jetstream::new(self.client.clone());
        const MAX_ATTEMPTS: u32 = 10;
        let mut attempt = 0;

        let consumer = loop {
            if attempt > 0 {
                let delay = Self::calculate_backoff_delay(attempt);
                warn!(
                    "JetStream setup attempt {}/{} failed. Retrying in {}ms...",
                    attempt + 1,
                    MAX_ATTEMPTS,
                    delay
                );
                Self::apply_backoff(attempt).await;
            }

            attempt += 1;

            if self.client.connection_state() != async_nats::connection::State::Connected {
                warn!(
                    "NATS client not connected on attempt {}/{}. Retrying...",
                    attempt, MAX_ATTEMPTS
                );
                if attempt >= MAX_ATTEMPTS {
                    bail!(
                        "JetStream setup failed after {} attempts due to disconnected client",
                        MAX_ATTEMPTS
                    );
                }
                continue;
            }

            match tokio::time::timeout(
                Duration::from_millis(1500),
                js.get_stream(JETSTREAM_STREAM_NAME),
            )
            .await
            {
                Ok(Ok(stream)) => match stream
                    .get_or_create_consumer(
                        JETSTREAM_CONSUMER_NAME,
                        pull::Config {
                            durable_name: Some(JETSTREAM_CONSUMER_NAME.to_string()),
                            filter_subject: subject.clone(),
                            max_deliver: 3,
                            ..Default::default()
                        },
                    )
                    .await
                {
                    Ok(consumer) => break consumer,
                    Err(e) => {
                        error!(
                            "JetStream consumer creation failed on attempt {}/{}: {:?}",
                            attempt, MAX_ATTEMPTS, e
                        );
                    }
                },
                Ok(Err(e)) => {
                    error!(
                        "JetStream API returned an execution error on attempt {}/{}: {:?}",
                        attempt, MAX_ATTEMPTS, e
                    );
                }
                Err(_) => {
                    error!(
                        "JetStream metadata discovery timed out on attempt {}/{}.",
                        attempt, MAX_ATTEMPTS
                    );
                }
            }

            if attempt >= MAX_ATTEMPTS {
                bail!("Unable to set up JetStream after {} attempts", MAX_ATTEMPTS);
            }
        };

        tokio::spawn(async move {
            let mut recovery_attempt = 0;

            loop {
                let mut messages = match consumer.messages().await {
                    Ok(msgs) => {
                        recovery_attempt = 0; // Reset counter on successful stream bind
                        msgs
                    }
                    Err(e) => {
                        recovery_attempt += 1;
                        let delay = Self::calculate_backoff_delay(recovery_attempt);

                        error!(
                            "Failed to fetch JetStream iterator (attempt {}): {}. Retrying in {}ms...",
                            recovery_attempt, e, delay
                        );

                        if recovery_attempt >= MAX_ATTEMPTS {
                            error!(
                                "JetStream consumer failed to bind after {} attempts. Exiting.",
                                MAX_ATTEMPTS
                            );
                            std::process::exit(1);
                        }

                        Self::apply_backoff(recovery_attempt).await;
                        continue;
                    }
                };

                while let Some(msg_result) = messages.next().await {
                    match msg_result {
                        Ok(message) => {
                            if tx.send(UnifiedMessage::JetStream(message)).await.is_err() {
                                info!(
                                    "Internal channel closed. Shutting down JetStream consumer loop thread."
                                );
                                return;
                            }
                        }
                        Err(e) => {
                            error!(
                                "Error reading next stream item: {}. Re-binding consumer...",
                                e
                            );
                            break;
                        }
                    }
                }

                recovery_attempt += 1;
                if recovery_attempt >= MAX_ATTEMPTS {
                    error!(
                        "JetStream consumer stream failed after {} recovery attempts. Exiting.",
                        MAX_ATTEMPTS
                    );
                    std::process::exit(1);
                }

                let delay = Self::calculate_backoff_delay(recovery_attempt);
                warn!(
                    "JetStream consumer stream ended unexpectedly. Re-binding in {}ms...",
                    delay
                );
                Self::apply_backoff(recovery_attempt).await;
            }
        });

        Ok(())
    }

    // Core NATS Subscription Setup with Resilient Reconnection Logic
    async fn setup_core_nats(
        &self,
        subject: String,
        tx: mpsc::Sender<UnifiedMessage>,
    ) -> anyhow::Result<()> {
        let client = self.client.clone();
        tokio::spawn(async move {
            const MAX_ATTEMPTS: u32 = 10;
            let mut attempt = 0;

            loop {
                match client.subscribe(subject.clone()).await {
                    Ok(mut subscription) => {
                        attempt = 0; // Reset backoff on successful subscription
                        while let Some(message) = subscription.next().await {
                            if tx.send(UnifiedMessage::Core(message)).await.is_err() {
                                return; // Global channel closed, terminate execution
                            }
                        }

                        // Subscription stream ended unexpectedly; apply backoff before re-subscribing
                        attempt += 1;
                        if attempt >= MAX_ATTEMPTS {
                            error!(
                                "Core NATS subscription failed after {} recovery attempts. Exiting.",
                                MAX_ATTEMPTS
                            );
                            std::process::exit(1);
                        }

                        let delay = Self::calculate_backoff_delay(attempt);

                        warn!(
                            "Core NATS subscription stream ended unexpectedly. Re-subscribing in {}ms...",
                            delay
                        );
                        Self::apply_backoff(attempt).await;
                    }
                    Err(e) => {
                        attempt += 1;
                        if attempt >= MAX_ATTEMPTS {
                            error!(
                                "Core NATS subscription failed after {} attempts. Exiting.",
                                MAX_ATTEMPTS
                            );
                            std::process::exit(1);
                        }

                        let delay = Self::calculate_backoff_delay(attempt);
                        error!(
                            "NATS subscription failed: {}. Retrying in {}ms...",
                            e, delay
                        );
                        Self::apply_backoff(attempt).await;
                    }
                }
            }
        });
        Ok(())
    }
}
