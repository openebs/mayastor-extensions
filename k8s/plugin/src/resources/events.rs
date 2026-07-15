use anyhow::anyhow;
use clap::builder::TypedValueParser;
use events_api::event::{Component, EventAction, EventCategory, EventMessage};
use plugin::resources::utils::{optional_cell, print_table, CreateRow, GetHeaderRow, OutputFormat};
use prettytable::{row, Row};
use serde::Serialize;
use std::path::PathBuf;
use strum::IntoEnumIterator;
use supportability::KubeConfigArgs;

const EVENTS_CONTAINER: &str = "eventing-aggregator";
const EVENTS_LABEL_SELECTOR: &str = "app=eventing-aggregator";
const MBUS_EVENT_TYPE: &str = "mbus_event";
const DEFAULT_LIMIT: usize = 1000;

/// Args for `kubectl mayastor get events`.
#[derive(Debug, clap::Args)]
pub struct EventsArgs {
    /// Loki base URL. If omitted, Loki is auto-discovered via the K8s service.
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

    /// Events from last duration (e.g. 1h, 30m).
    #[arg(long, default_value = "24h")]
    since: humantime::Duration,

    /// Maximum number of events to return (0 = unlimited).
    #[arg(long, default_value_t = DEFAULT_LIMIT)]
    limit: usize,

    /// Loki tenant ID (X-Scope-OrgID header).
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
    Some(EventRecord::from_event_message(msg))
}

impl EventsArgs {
    /// Fetch events from Loki and print them according to the output format.
    pub async fn get_events(
        &self,
        namespace: &str,
        kubeconfig: Option<PathBuf>,
        context: Option<String>,
        timeout: humantime::Duration,
        output: &OutputFormat,
    ) -> anyhow::Result<()> {
        let kubeconfig_args = KubeConfigArgs {
            path: kubeconfig,
            opts: kube_proxy::kubeconfig_options_from_context(context),
        };

        let mut loki_client = supportability::LokiClient::new(
            self.loki_endpoint.clone(),
            kubeconfig_args,
            namespace.to_string(),
            self.since,
            timeout,
            self.tenant_id.clone(),
        )
        .await
        .ok_or_else(|| {
            anyhow!(
                "Loki not found. Provide --loki-endpoint or ensure Loki is deployed in the cluster."
            )
        })?
        .with_logql_filters(build_logql_filters());

        let lines = loki_client
            .fetch_lines(
                EVENTS_LABEL_SELECTOR.to_string(),
                EVENTS_CONTAINER.to_string(),
                self.limit,
            )
            .await
            .map_err(|e| anyhow!("Failed to fetch events from Loki: {e:?}"))?;

        let raw_count = lines.len();
        let mut records: Vec<EventRecord> = lines.iter().filter_map(|l| parse_line(l)).collect();

        records = apply_filters(records, self);

        // Sort newest-first.
        records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let truncated = self.limit > 0 && records.len() >= self.limit;

        if records.is_empty() && output.none() {
            if raw_count == 0 {
                eprintln!(
                    "No events found. Try widening the time range with --since (e.g. --since 24h) \
                     or verify that Loki is scraping the eventing-aggregator pod."
                );
            } else {
                eprintln!(
                    "No events matched the applied filters ({raw_count} log lines fetched from Loki). \
                     Try relaxing --category, --action, --node, or --target filters."
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
            true
        })
        .collect()
}
