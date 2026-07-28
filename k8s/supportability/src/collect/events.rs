use crate::collect::{
    error::Error,
    k8s_resources::client::ClientSet,
    logs::LokiClient,
    utils::{log, write_to_log_file},
};

use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{AttachParams, ListParams},
    Api,
};
use std::{io::Write, path::Path};

const EVENTS_LABEL_SELECTOR: &str = "app=eventing-aggregator";
const EVENTS_CONTAINER: &str = "eventing-aggregator";
const MBUS_EVENT_TYPE: &str = "mbus_event";
const EVENTS_VOLUME_DIR: &str = "/var/events";

fn build_logql_filters() -> Vec<String> {
    vec![format!("|= \"{MBUS_EVENT_TYPE}\"")]
}

/// Collect events into `path/events.ndjson`.
///
/// Prefers Loki when a client is provided (eventing-aggregator was deployed in Loki mode).
/// Falls back to exec-ing the eventing-aggregator pod volume when Loki is unavailable.
/// `since` is passed to the aggregator exec so only events within the dump window are streamed.
pub(crate) async fn collect_events_to_file(
    loki_client: Option<&mut LokiClient>,
    client_set: &ClientSet,
    path: &Path,
    since: humantime::Duration,
) -> Result<(), Error> {
    std::fs::create_dir_all(path)?;
    match loki_client {
        Some(client) => collect_events_from_loki(client, path).await,
        None => collect_events_from_volume(client_set, path, since).await,
    }
}

/// Collect events from Loki into `path/events.ndjson`.
///
/// Queries Loki for the eventing-aggregator stream and applies a `|= "mbus_event"` substring
/// filter so only event lines are written (tracing/other log lines are dropped server-side).
/// Loki lines carry a tracing prefix before the JSON payload; this function strips everything
/// before the first `{` so the output file contains valid NDJSON.
/// Lines are written page-by-page as they arrive from Loki — no full result set is held in
/// memory, keeping allocation bounded to a single page (~3000 lines) at a time.
async fn collect_events_from_loki(loki_client: &mut LokiClient, path: &Path) -> Result<(), Error> {
    log("Collecting events from Loki...");

    let events_path = path.join("events.ndjson");
    let mut file = std::fs::File::create(&events_path)?;
    let mut count = 0usize;

    loki_client.set_logql_filters(build_logql_filters());
    loki_client
        .fetch_lines_paged(
            EVENTS_LABEL_SELECTOR.to_string(),
            EVENTS_CONTAINER.to_string(),
            |page| {
                for line in page {
                    // Loki lines have a tracing prefix before the JSON payload
                    // (e.g. "2024-01-01 INFO eventing_aggregator: {...}").
                    // Strip everything before the first '{' so the file contains
                    // the same raw NDJSON format as the pod-volume path.
                    if let Some(json_start) = line.find('{') {
                        writeln!(file, "{}", line[json_start..].trim_end())?;
                        count += 1;
                    }
                }
                Ok(())
            },
        )
        .await
        .map_err(|e| Error::Generic(format!("Failed to fetch events from Loki: {e:?}")))?;

    std::fs::write(path.join("events-source.txt"), "loki\n")
        .map_err(|e| Error::Generic(format!("Failed to write events-source.txt: {e}")))?;

    log(format!("Collected {count} events from Loki"));
    let _ = write_to_log_file(format!("Events written to {}\n", events_path.display()));

    Ok(())
}

/// Collect events from the eventing-aggregator pod volume into `path/events.ndjson`.
///
/// Finds a running eventing-aggregator pod and execs
/// `/bin/eventing-aggregator --print-events --events-dir=/var/events --since=<rfc3339>`.
/// The aggregator binary reads its on-disk event files, deduplicates, and streams NDJSON to
/// stdout. Bytes are copied directly to disk via `tokio::io::copy` with no deserialization,
/// making memory usage O(1) regardless of how many events are stored on the volume.
/// If no running pod is found the function logs a warning and returns `Ok(())` so the
/// rest of the dump can continue.
async fn collect_events_from_volume(
    client_set: &ClientSet,
    path: &Path,
    since: humantime::Duration,
) -> Result<(), Error> {
    let kube_client = client_set.kube_client();
    let namespace = client_set.namespace();

    let pods: Api<Pod> = Api::namespaced(kube_client, namespace);
    let lp = ListParams::default().labels(EVENTS_LABEL_SELECTOR);

    let pod_list = pods
        .list(&lp)
        .await
        .map_err(|e| Error::Generic(format!("Failed to list eventing-aggregator pods: {e}")))?;

    let pod_name = match pod_list
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
    {
        Some(name) => name,
        None => {
            log(format!(
                "No running eventing-aggregator pod found in namespace {namespace}; skipping events collection"
            ));
            return Ok(());
        }
    };

    log(format!("Collecting events from pod volume ({pod_name})..."));

    let ap = AttachParams::default()
        .stdin(false)
        .stdout(true)
        .stderr(false);

    let since_cutoff = chrono::Utc::now()
        - chrono::Duration::from_std(*since)
            .map_err(|e| Error::Generic(format!("--since duration is out of range: {e}")))?;
    let events_dir_arg = format!("--events-dir={EVENTS_VOLUME_DIR}");
    let since_arg = format!("--since={}", since_cutoff.to_rfc3339());
    let mut attached = pods
        .exec(
            &pod_name,
            vec![
                "/bin/eventing-aggregator",
                "--print-events",
                &events_dir_arg,
                &since_arg,
            ],
            &ap,
        )
        .await
        .map_err(|e| Error::Generic(format!("Failed to exec into pod {pod_name}: {e}")))?;

    let mut stdout = attached
        .stdout()
        .ok_or_else(|| Error::Generic("No stdout from pod exec".to_string()))?;

    let events_path = path.join("events.ndjson");
    let mut file = tokio::fs::File::create(&events_path)
        .await
        .map_err(|e| Error::Generic(format!("Failed to create {}: {e}", events_path.display())))?;

    // Stream bytes directly from exec stdout — no deserialization, O(1) memory.
    tokio::io::copy(&mut stdout, &mut file)
        .await
        .map_err(|e| Error::Generic(format!("Failed to stream events from pod {pod_name}: {e}")))?;

    std::fs::write(path.join("events-source.txt"), format!("{pod_name}\n"))
        .map_err(|e| Error::Generic(format!("Failed to write events-source.txt: {e}")))?;

    log(format!("Collected events from pod {pod_name}"));
    let _ = write_to_log_file(format!("Events written to {}\n", events_path.display()));

    Ok(())
}
