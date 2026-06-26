/// Channel capacity for internal message passing and exporter buffering.
pub const CHANNEL_CAPACITY: usize = 512;

/// Maximum number of events to batch in a single export flush.
pub const BATCH_MAX_SIZE: usize = 100;

/// Maximum time in milliseconds to wait before flushing a non-empty batch.
pub const BATCH_TIMEOUT_MS: u64 = 10000; // 10 seconds

/// Maximum number of write retries before failing the exporter operation.
pub const MAX_RETRIES: usize = 3;

/// Default service name used for tracing and application identification.
pub const SERVICE_NAME: &str = "eventing-aggregator";

/// Durable name for the JetStream pull consumer.
pub const JETSTREAM_CONSUMER_NAME: &str = "eventing-aggregator-consumer";

/// Base filename for the local event export file.
pub const EVENTS_JSON_FILE: &str = "events.json";

/// Filename for the rotated (previous) local event export file.
pub const EVENTS_JSON_ROTATED_FILE: &str = "events.1.json";
