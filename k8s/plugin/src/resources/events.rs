use anyhow::anyhow;
use chrono::{DateTime, Utc};
use clap::builder::TypedValueParser;
use events_api::event::{
    Component, EventAction, EventCategory, EventDetails, EventMessage, RebuildStatus,
};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{AttachParams, ListParams},
    Api,
};
use plugin::resources::utils::{optional_cell, print_table, CreateRow, GetHeaderRow, OutputFormat};
use prettytable::{row, Row};
use serde::Serialize;
use strum::IntoEnumIterator;
use supportability::KubeConfigArgs;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, BufReader};

const EVENTS_CONTAINER: &str = "eventing-aggregator";
const EVENTS_LABEL_SELECTOR: &str = "app=eventing-aggregator";
const MBUS_EVENT_TYPE: &str = "mbus_event";
const DEFAULT_LIMIT: usize = 1000;
const EVENTS_VOLUME_DIR: &str = "/var/events";

/// Args for `kubectl mayastor get events`.
#[derive(Debug, clap::Args)]
pub struct EventsArgs {
    /// Loki base URL. If omitted, Loki is auto-discovered via the K8s service;
    /// if discovery also fails, events are read from the eventing-aggregator pod volume.
    #[arg(long)]
    loki_endpoint: Option<String>,

    /// Filter by event category (repeatable or comma-separated).
    #[arg(long = "category", value_delimiter = ',', value_parser = category_parser())]
    categories: Vec<EventCategory>,

    /// Filter by event action (repeatable or comma-separated).
    #[arg(long = "action", value_delimiter = ',', value_parser = action_parser())]
    actions: Vec<EventAction>,

    /// Filter by node name.
    #[arg(long)]
    node: Option<String>,

    /// Filter by target resource ID.
    #[arg(long)]
    target: Option<String>,

    /// Filter by source component (repeatable or comma-separated).
    #[arg(long = "component", value_delimiter = ',', value_parser = component_parser())]
    components: Vec<Component>,

    /// Filter events for a pool (substring match on target).
    #[arg(long)]
    pool: Option<String>,

    /// Filter events touching a volume (target, snapshot, volume_id).
    #[arg(long)]
    volume: Option<uuid::Uuid>,

    /// Filter events by replica UUID.
    #[arg(long)]
    replica: Option<uuid::Uuid>,

    /// Filter rebuild events by outcome (repeatable or comma-separated).
    #[arg(long = "rebuild-status", value_delimiter = ',', value_parser = rebuild_status_parser())]
    rebuild_statuses: Vec<RebuildStatus>,

    /// Filter state-change events by state value (substring on previous or next state).
    #[arg(long)]
    state: Option<String>,

    /// Filter by JSON field path and pattern: path=value (repeatable, AND logic).
    /// Path is relative to the event payload, e.g.
    /// --filter "metadata.source.eventDetails.replicaDetails.poolUuid=abc*".
    /// Supports leading/trailing * wildcards. Events with an unknown or absent path
    /// are excluded.
    #[arg(long = "filter")]
    filters: Vec<String>,

    /// Events from last duration (e.g. 1h, 30m).
    #[arg(long, default_value = "24h")]
    since: humantime::Duration,

    /// Maximum number of events to return (0 = unlimited).
    #[arg(long, default_value_t = DEFAULT_LIMIT)]
    limit: usize,

    /// Loki tenant ID / X-Scope-OrgID header (loki source only).
    #[arg(long, default_value = "openebs")]
    tenant_id: String,
}

/// A single parsed event record.
///
/// Display fields (timestamp, category, …) are used for table output.
/// The original `EventMessage` is kept for JSON/YAML output so the full
/// typed payload (as defined in events-api) is always serialised.
#[derive(Debug, Clone, Serialize)]
pub struct EventRecord {
    // Table-display fields — skipped when serialising to JSON/YAML.
    #[serde(skip)]
    pub timestamp: String,
    #[serde(skip)]
    pub category: String,
    #[serde(skip)]
    pub action: String,
    #[serde(skip)]
    pub target: String,
    #[serde(skip)]
    pub node: String,
    #[serde(skip)]
    pub component: String,
    /// Raw outer JSON — used by --filter dot-path traversal.
    #[serde(skip)]
    raw: serde_json::Value,
    /// Full typed payload; serialised for JSON/YAML output.
    #[serde(flatten)]
    pub event_message: EventMessage,
}

impl EventRecord {
    /// Build an `EventRecord` from a deserialized `EventMessage`.
    /// Used by all event sources (Loki, pod exec, file, NATS).
    pub fn from_event_message(msg: EventMessage) -> Self {
        let (timestamp, node, component) = match &msg.metadata {
            Some(meta) => {
                let ts = meta
                    .timestamp
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_default();
                let (node, component) = match &meta.source {
                    Some(src) => {
                        let comp = Component::try_from(src.component)
                            .map(|c| format!("{c:?}"))
                            .unwrap_or_else(|_| src.component.to_string());
                        (src.node.clone(), comp)
                    }
                    None => (String::new(), String::new()),
                };
                (ts, node, component)
            }
            None => (String::new(), String::new(), String::new()),
        };

        let category = EventCategory::try_from(msg.category)
            .map(|c| format!("{c:?}"))
            .unwrap_or_else(|_| msg.category.to_string());

        let action = EventAction::try_from(msg.action)
            .map(|a| format!("{a:?}"))
            .unwrap_or_else(|_| msg.action.to_string());

        let target = msg.target.clone();

        Self {
            timestamp,
            category,
            action,
            target,
            node,
            component,
            raw: serde_json::Value::Null,
            event_message: msg,
        }
    }
}

impl GetHeaderRow for EventRecord {
    fn get_header_row(&self) -> Row {
        row![
            "TIMESTAMP",
            "CATEGORY",
            "ACTION",
            "TARGET",
            "NODE",
            "COMPONENT"
        ]
    }
}

impl CreateRow for EventRecord {
    fn row(&self) -> Row {
        row![
            self.timestamp,
            self.category,
            self.action,
            self.target,
            optional_cell(if self.node.is_empty() {
                None::<&str>
            } else {
                Some(&self.node)
            }),
            optional_cell(if self.component.is_empty() {
                None::<&str>
            } else {
                Some(&self.component)
            }),
        ]
    }
    // We pre-sort by timestamp before calling print_table.
    fn sort_rows(&self) -> bool {
        false
    }
}

/// Parse a single raw log line (from Loki or an NDJSON file) into an `EventRecord`.
///
/// Lines from Loki carry a tracing prefix before the JSON; lines from the
/// ephemeral-volume file are raw NDJSON.  Both start the JSON at the first `{`.
pub fn parse_line(line: &str) -> Option<EventRecord> {
    let json_start = line.find('{')?;
    let json_str = &line[json_start..];

    let outer: serde_json::Value = serde_json::from_str(json_str).ok()?;

    if outer.get("type")?.as_str()? != MBUS_EVENT_TYPE {
        return None;
    }

    let payload = outer.get("payload")?;
    let msg: EventMessage = serde_json::from_value(payload.clone()).ok()?;
    let mut record = EventRecord::from_event_message(msg);
    record.raw = outer;
    Some(record)
}

/// Stream NDJSON lines from any async reader, parse each into an `EventRecord` immediately.
/// Used by the volume fallback (exec stdout) and `--from-file` (file handle).
/// Raw JSON strings are never accumulated — each line is parsed and dropped.
pub async fn read_events_from_reader<R: AsyncBufRead + Unpin>(reader: R) -> Vec<EventRecord> {
    let mut lines = reader.lines();
    let mut records = Vec::new();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(record) = parse_line(&line) {
            records.push(record);
        }
    }
    records
}

impl EventsArgs {
    /// Fetch and print events according to the output format.
    pub async fn get_events(
        &self,
        namespace: &str,
        kube_client: kube::Client,
        kubeconfig_args: KubeConfigArgs,
        timeout: humantime::Duration,
        output: &OutputFormat,
    ) -> anyhow::Result<()> {
        // Silence the "LogFile not initialised" noise from supportability's log()
        // utility — that subsystem is only wired up in system-dump context.
        supportability::init_no_log_file();

        let maybe_client = supportability::LokiClient::new(
            self.loki_endpoint.clone(),
            kubeconfig_args.clone(),
            namespace.to_string(),
            self.since,
            timeout,
            self.tenant_id.clone(),
        )
        .await;

        let (records, source_label) = match maybe_client {
            Some(client) => {
                let lines = client
                    .with_logql_filters(build_logql_filters())
                    .fetch_lines(
                        EVENTS_LABEL_SELECTOR.to_string(),
                        EVENTS_CONTAINER.to_string(),
                        self.limit,
                    )
                    .await
                    .map_err(|e| anyhow!("Failed to fetch events from Loki: {e:?}"))?;
                let records: Vec<EventRecord> =
                    lines.iter().filter_map(|l| parse_line(l)).collect();
                (records, "Loki")
            }
            None if self.loki_endpoint.is_some() => {
                return Err(anyhow!(
                    "Could not connect to Loki at {}. Check the endpoint and try again.",
                    self.loki_endpoint.as_deref().unwrap_or_default()
                ));
            }
            None => {
                if output.none() {
                    eprintln!(
                        "Loki not found in cluster; falling back to eventing-aggregator pod volume."
                    );
                }
                let since_cutoff =
                    Utc::now() - chrono::Duration::from_std(*self.since).unwrap_or_default();
                let records = fetch_from_volume(kube_client, since_cutoff)
                    .await?
                    .into_iter()
                    .filter(|r| {
                        DateTime::parse_from_rfc3339(&r.timestamp)
                            .map(|t| t.to_utc() >= since_cutoff)
                            .unwrap_or(true)
                    })
                    .collect();
                (records, "eventing-aggregator volume")
            }
        };

        let raw_count = records.len();
        let mut records = records;

        records = apply_filters(records, self);

        // Sort newest-first.
        records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let truncated = self.limit > 0 && records.len() >= self.limit;

        if records.is_empty() && output.none() {
            if raw_count == 0 {
                eprintln!(
                    "No events found in {source_label}. \
                     Verify the eventing-aggregator pod is running and has written events."
                );
            } else {
                eprintln!(
                    "No events matched the applied filters ({raw_count} lines read from {source_label}). \
                     Try relaxing filters."
                );
            }
            return Ok(());
        }

        // Table: use the display fields via CreateRow / GetHeaderRow.
        // JSON / YAML: EventRecord serialises via #[serde(flatten)] on event_message,
        // so print_table produces the full EventMessage structure for those formats.
        print_table(output, records);

        if truncated && output.none() {
            eprintln!(
                "(output truncated at {} events; use --limit to increase)",
                self.limit
            );
        }

        Ok(())
    }
}

/// Returns LogQL pipeline stages that pre-filter Loki lines to only event records.
fn build_logql_filters() -> Vec<String> {
    vec![format!("|= \"{MBUS_EVENT_TYPE}\"")]
}

/// Exec into the eventing-aggregator pod and stream its ephemeral event files.
///
/// Passes `--since` to the binary so it filters at the source, reducing both the
/// in-pod dedup HashSet and the data streamed over exec.
/// Lines are parsed into EventRecords as they arrive; raw JSON is never buffered.
async fn fetch_from_volume(
    client: kube::Client,
    since_cutoff: DateTime<Utc>,
) -> anyhow::Result<Vec<EventRecord>> {
    let namespace = client.default_namespace().to_owned();
    let pods: Api<Pod> = Api::namespaced(client, &namespace);
    let lp = ListParams::default().labels(EVENTS_LABEL_SELECTOR);
    let pod_list = pods
        .list(&lp)
        .await
        .map_err(|e| anyhow!("Failed to list eventing-aggregator pods: {e}"))?;

    let pod_name = pod_list
        .items
        .into_iter()
        .find(|p| {
            p.status
                .as_ref()
                .and_then(|s| s.phase.as_deref())
                .map(|ph| ph == "Running")
                .unwrap_or(false)
        })
        .and_then(|p| p.metadata.name)
        .ok_or_else(|| {
            anyhow!("No running eventing-aggregator pod found in namespace {namespace}")
        })?;

    let ap = AttachParams::default()
        .stdin(false)
        .stdout(true)
        .stderr(false);

    let since_arg = format!("--since={}", since_cutoff.to_rfc3339());
    let mut attached = pods
        .exec(
            &pod_name,
            vec![
                "/bin/eventing-aggregator",
                "--print-events",
                &format!("--events-dir={EVENTS_VOLUME_DIR}"),
                &since_arg,
            ],
            &ap,
        )
        .await
        .map_err(|e| anyhow!("Failed to exec into pod {pod_name}: {e}"))?;

    let stdout = attached
        .stdout()
        .ok_or_else(|| anyhow!("No stdout from pod exec"))?;

    Ok(read_events_from_reader(BufReader::new(stdout)).await)
}

fn category_parser() -> impl clap::builder::TypedValueParser<Value = EventCategory> {
    clap::builder::PossibleValuesParser::new(
        EventCategory::iter()
            .filter(|c| *c != EventCategory::UnknownCategory)
            .map(|c| c.to_string()),
    )
    .map(|s: String| {
        s.parse::<EventCategory>()
            .expect("validated by PossibleValuesParser")
    })
}

fn action_parser() -> impl clap::builder::TypedValueParser<Value = EventAction> {
    clap::builder::PossibleValuesParser::new(
        EventAction::iter()
            .filter(|a| *a != EventAction::UnknownAction)
            .map(|a| a.to_string()),
    )
    .map(|s: String| {
        s.parse::<EventAction>()
            .expect("validated by PossibleValuesParser")
    })
}

/// Apply in-memory filters from `EventsArgs` to a list of records.
fn apply_filters(records: Vec<EventRecord>, args: &EventsArgs) -> Vec<EventRecord> {
    records
        .into_iter()
        .filter(|r| {
            if !args.categories.is_empty()
                && !args
                    .categories
                    .iter()
                    .any(|c| c.as_str_name() == r.category)
            {
                return false;
            }
            if !args.actions.is_empty() && !args.actions.iter().any(|a| a.as_str_name() == r.action)
            {
                return false;
            }
            if let Some(node) = &args.node {
                if !r.node.eq_ignore_ascii_case(node) {
                    return false;
                }
            }
            if let Some(target) = &args.target {
                if !r.target.contains(target.as_str()) {
                    return false;
                }
            }
            if !args.components.is_empty()
                && !args
                    .components
                    .iter()
                    .any(|c| c.as_str_name() == r.component)
            {
                return false;
            }
            if let Some(pool) = &args.pool {
                if !r.target.contains(pool.as_str()) {
                    return false;
                }
            }
            if let Some(vol) = &args.volume {
                let vol_str = vol.to_string();
                let in_target = r.target == vol_str;
                let in_details = get_event_details(r)
                    .map(|d| {
                        d.snapshot_details
                            .as_ref()
                            .map(|sd| sd.volume_id == vol_str)
                            .unwrap_or(false)
                            || d.clone_details
                                .as_ref()
                                .map(|cd| cd.source_uuid == vol_str)
                                .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if !in_target && !in_details {
                    return false;
                }
            }
            if let Some(rep) = &args.replica {
                let rep_str = rep.to_string();
                let in_target = r.target == rep_str;
                let in_details = get_event_details(r)
                    .and_then(|d| d.replica_details.as_ref())
                    .map(|rd| rd.replica_name == rep_str)
                    .unwrap_or(false);
                if !in_target && !in_details {
                    return false;
                }
            }
            if !args.rebuild_statuses.is_empty() {
                let matched = get_event_details(r)
                    .and_then(|d| d.rebuild_details.as_ref())
                    .and_then(|rd| RebuildStatus::try_from(rd.rebuild_status).ok())
                    .map(|rs| {
                        args.rebuild_statuses
                            .iter()
                            .any(|s| s.as_str_name() == rs.as_str_name())
                    })
                    .unwrap_or(false);
                if !matched {
                    return false;
                }
            }
            if let Some(state) = &args.state {
                let state_lc = state.to_lowercase();
                let matched = get_event_details(r)
                    .and_then(|d| d.state_change_details.as_ref())
                    .map(|sd| {
                        sd.previous.to_lowercase().contains(&state_lc)
                            || sd.next.to_lowercase().contains(&state_lc)
                    })
                    .unwrap_or(false);
                if !matched {
                    return false;
                }
            }
            for expr in &args.filters {
                let Some((path, pattern)) = expr.split_once('=') else {
                    return false;
                };
                let payload = r.raw.get("payload").unwrap_or(&serde_json::Value::Null);
                let Some(value) = traverse_json(payload, path) else {
                    return false;
                };
                if !matches_pattern(&value, pattern) {
                    return false;
                }
            }
            true
        })
        .collect()
}

/// Shortcut to reach the `EventDetails` sub-message inside a record.
fn get_event_details(record: &EventRecord) -> Option<&EventDetails> {
    record
        .event_message
        .metadata
        .as_ref()?
        .source
        .as_ref()?
        .event_details
        .as_ref()
}

/// Walk a dot-separated path through a JSON value and return the leaf as a string.
/// Returns `None` if any key in the path is missing or the leaf is an object/array.
fn traverse_json(value: &serde_json::Value, path: &str) -> Option<String> {
    let mut current = value;
    for key in path.split('.') {
        current = current.get(key)?;
    }
    match current {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Pattern matching with leading/trailing `*` wildcards. Case-sensitive.
/// A bare `*` matches everything. Interior `*` is treated as a literal character.
fn matches_pattern(value: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    match (pattern.starts_with('*'), pattern.ends_with('*')) {
        (true, true) => value.contains(&pattern[1..pattern.len() - 1]),
        (true, false) => value.ends_with(&pattern[1..]),
        (false, true) => value.starts_with(&pattern[..pattern.len() - 1]),
        (false, false) => value == pattern,
    }
}

fn component_parser() -> impl clap::builder::TypedValueParser<Value = Component> {
    clap::builder::PossibleValuesParser::new(
        Component::iter()
            .filter(|c| *c != Component::UnknownComponent)
            .map(|c| c.to_string()),
    )
    .map(|s: String| {
        Component::iter()
            .find(|c| c.to_string() == s)
            .expect("validated by PossibleValuesParser")
    })
}

fn rebuild_status_parser() -> impl clap::builder::TypedValueParser<Value = RebuildStatus> {
    clap::builder::PossibleValuesParser::new(
        RebuildStatus::iter()
            .filter(|s| *s != RebuildStatus::Unknown)
            .map(|s| s.to_string()),
    )
    .map(|s: String| {
        s.parse::<RebuildStatus>()
            .expect("validated by PossibleValuesParser")
    })
}
