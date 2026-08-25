use crate::{
    collect::{constants::X_SCOPE_ORGID, utils::write_to_log_file},
    log,
};

use chrono::Utc;
use hyper::body::Buf;
use serde::{Deserialize, Serialize};
use std::{io::Write, path::PathBuf};
use tower::{Service, ServiceExt};

/// Loki endpoint to query for logs
const ENDPOINT: &str = "/loki/api/v1/query_range";

const SERVICE_NAME: &str = "loki";

/// Possible errors can occur while interacting with Loki service.
#[derive(Debug)]
#[allow(unused)]
pub enum LokiError {
    Request(http::Error),
    Response(String),
    Tower(tower::BoxError),
    Serde(serde_json::Error),
    Hyper(hyper::Error),
    IOError(std::io::Error),
}

impl From<http::Error> for LokiError {
    fn from(e: http::Error) -> LokiError {
        LokiError::Request(e)
    }
}
impl From<tower::BoxError> for LokiError {
    fn from(e: tower::BoxError) -> LokiError {
        LokiError::Tower(e)
    }
}
impl From<serde_json::Error> for LokiError {
    fn from(e: serde_json::Error) -> LokiError {
        LokiError::Serde(e)
    }
}
impl From<hyper::Error> for LokiError {
    fn from(e: hyper::Error) -> LokiError {
        LokiError::Hyper(e)
    }
}
impl From<std::io::Error> for LokiError {
    fn from(e: std::io::Error) -> LokiError {
        LokiError::IOError(e)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct StreamMetaData {
    #[serde(rename = "hostname")]
    host_name: String,
    #[serde(rename = "pod")]
    pod_name: String,
    #[serde(rename = "container")]
    container_name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct StreamContent {
    #[serde(rename = "stream")]
    stream_metadata: StreamMetaData,
    values: Vec<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Data {
    result: Vec<StreamContent>,
}

// Response structure obtained from Loki after making http request.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct LokiResponse {
    status: String,
    data: Data,
}

type SinceTime = u128;

impl LokiResponse {
    // fetch last stream log epoch timestamp in nanoseconds
    fn get_last_stream_unix_time(&self) -> SinceTime {
        let unix_time = match self.data.result.last() {
            Some(last_stream) => last_stream
                .values
                .last()
                .unwrap_or(&vec![])
                .first()
                .unwrap_or(&"0".to_string())
                .parse::<SinceTime>()
                .unwrap_or(0),
            None => {
                return 0;
            }
        };
        unix_time
    }
}

// Determines the sort order of logs.
#[derive(Debug, Clone)]
enum LogDirection {
    Forward,
}

impl LogDirection {
    fn as_string(&self) -> String {
        match self {
            LogDirection::Forward => "forward".to_string(),
        }
    }
}

/// Http client to interact with Loki (a log management system)
/// to fetch historical log information.
#[derive(Debug)]
pub struct LokiClient {
    /// Address of Loki service.
    uri: String,
    /// Loki client
    inner_client: kube_proxy::LokiClient,
    /// Endpoint of Loki logs service.
    logs_endpoint: String,
    /// Defines period from which logs needs to collect.
    since: SinceTime,
    /// Determines the sort order of logs. Supported values are "forward" or "backward".
    /// Defaults to forward
    direction: LogDirection,
    /// Maximum number of entries to return per HTTP request (page size).
    limit: u64,
    /// Tenant id to be used for querying.
    tenant_id: String,
    /// LogQL pipeline filter expressions appended after the stream selector,
    /// e.g. `vec!["| json", "| payload_category=\"Volume\""]`.
    logql_filters: Vec<String>,
}

impl LokiClient {
    /// Instantiate new instance of Http Loki client.
    pub async fn new(
        uri: Option<String>,
        kubeconfig_args: crate::KubeConfigArgs,
        namespace: String,
        since: humantime::Duration,
        timeout: humantime::Duration,
        tenant_id: String,
    ) -> Option<Self> {
        let (uri, client) = match uri {
            None => {
                // Without this, ConfigBuilder's own Default impl applies a
                // hardcoded 5s timeout here regardless of the CLI's --timeout
                // value - fine for a short --since window, but Loki
                // genuinely needs longer than that to search a large one
                // (e.g. 30d), so every query timed out well before Loki
                // could respond.
                let (uri, svc) = match kube_proxy::ConfigBuilder::default_loki()
                    .with_kube_config(kubeconfig_args.path.clone())
                    .with_context(kubeconfig_args.opts.context.clone())
                    .with_target_mod(|t| t.with_namespace(namespace))
                    .with_timeout(*timeout)
                    .build()
                    .await
                {
                    Ok(result) => result,
                    Err(error) => {
                        match matches!(
                            error,
                            kube_proxy::Error::Forward {
                                source: kube_forward::Error::ServiceNotFound { .. }
                            }
                        ) {
                            true => log("Loki is not found, continuing..."),
                            false => log(format!(
                                "Failed to create loki client ({error}). Continuing..."
                            )),
                        }
                        return None;
                    }
                };
                (uri.to_string(), svc)
            }
            Some(uri) => {
                let mut connector = hyper_util::client::legacy::connect::HttpConnector::new();

                connector.set_connect_timeout(Some(*timeout));
                let client = hyper_util::client::legacy::Client::builder(
                    hyper_util::rt::TokioExecutor::new(),
                )
                .http2_keep_alive_timeout(*timeout)
                .http2_keep_alive_interval(*timeout / 2)
                .build(connector);
                let service = tower::ServiceBuilder::new().timeout(*timeout).service(
                    tower::util::MapResponse::new(
                        client,
                        |r: hyper::Response<hyper::body::Incoming>| {
                            r.map(hyper_body::Body::wrap_body)
                        },
                    ),
                );
                (uri, kube_proxy::LokiClient::new(service))
            }
        };

        Some(LokiClient {
            uri,
            inner_client: client,
            since: get_epoch_unix_time(since),
            logs_endpoint: ENDPOINT.to_string(),
            direction: LogDirection::Forward,
            limit: 3000,
            tenant_id,
            logql_filters: vec![],
        })
    }

    /// Set LogQL pipeline filter expressions appended after the stream selector.
    /// Each entry is a full pipeline stage string, e.g. `"| json"` or
    /// `"| payload_category=\"Volume\""`. Stages are appended in order.
    pub fn with_logql_filters(mut self, filters: Vec<String>) -> Self {
        self.logql_filters = filters;
        self
    }

    /// Override the per-request page size (default: 3000).
    /// This controls how many log entries Loki returns in a single HTTP call.
    /// Use `fetch_lines()` to enforce a total result cap across pages.
    pub fn with_page_size(mut self, page_size: u64) -> Self {
        self.limit = page_size;
        self
    }

    /// Fetch raw log lines from Loki for the given label selector and container,
    /// applying any `logql_filters` set via `with_logql_filters()`.
    ///
    /// Paginates automatically using the per-request page size set on this client
    /// (default 3000, overridable via `with_page_size()`).
    ///
    /// `limit` is the maximum total number of lines to return across all pages —
    /// it is a caller-enforced ceiling, not the Loki page size. Pass `0` to
    /// fetch all available lines with no cap.
    ///
    /// Returns one string per log line, in forward time order.
    pub async fn fetch_lines(
        &mut self,
        label_selector: String,
        container_name: String,
        limit: usize,
    ) -> Result<Vec<String>, LokiError> {
        let label_filters = label_selector_to_logql(&label_selector);

        let mut query_field = format!("{{{label_filters},container=\"{container_name}\"}}");
        for filter in &self.logql_filters {
            query_field.push_str(filter);
        }

        let encoded_query = urlencoding::encode(&query_field).into_owned();

        let mut poller = LokiPoll {
            uri: self.uri.clone(),
            endpoint: self.logs_endpoint.clone(),
            since: self.since,
            encoded_query,
            page_size: self.limit,
            next_start_epoch_timestamp: 0,
            client: self,
        };

        let mut lines = Vec::with_capacity(limit);
        loop {
            if limit > 0 {
                // Cap the page size to how many lines we still need so we don't
                // ask Loki for more entries than the caller wants.
                poller.page_size = poller.page_size.min((limit - lines.len()) as u64);
            }
            match poller.poll_next().await? {
                Some(batch) => {
                    lines.extend(batch);
                    if limit > 0 && lines.len() >= limit {
                        return Ok(lines);
                    }
                }
                None => break,
            }
        }
        Ok(lines)
    }

    /// fetch_and_dump_logs will do the following steps:
    /// 1. Creates poller to interact with Loki service based on provided arguments 1.1. Use poller
    ///    to fetch all available logs 1.2. Write fetched logs into file Continue above steps till
    ///    extraction all logs
    pub(crate) async fn fetch_and_dump_logs(
        &mut self,
        label_selector: String,
        container_name: String,
        host_name: Option<String>,
        service_dir: PathBuf,
    ) -> Result<(), LokiError> {
        let label_filters = label_selector_to_logql(&label_selector);
        let (file_name, new_query_field) = match host_name {
            Some(host_name) => {
                let file_name = format!("{host_name}-{SERVICE_NAME}-{container_name}.log");
                let new_query_field = format!(
                    "{{{label_filters},container=\"{container_name}\",hostname=~\"{host_name}.*\"}}"
                );
                (file_name, new_query_field)
            }
            None => {
                let file_name = format!("{SERVICE_NAME}-{container_name}.log");
                let new_query_field = format!("{{{label_filters},container=\"{container_name}\"}}");
                (file_name, new_query_field)
            }
        };
        let encoded_query = urlencoding::encode(&new_query_field).into_owned();

        let mut poller = LokiPoll {
            uri: self.uri.clone(),
            endpoint: self.logs_endpoint.clone(),
            since: self.since,
            encoded_query,
            page_size: self.limit,
            next_start_epoch_timestamp: 0,
            client: self,
        };
        let mut is_written = false;
        let file_path = service_dir.join(file_name.clone());
        let mut log_file: std::fs::File = std::fs::File::create(file_path.clone())?;

        loop {
            let result = match poller.poll_next().await {
                Ok(value) => match value {
                    Some(v) => v,
                    None => {
                        break;
                    }
                },
                Err(e) => {
                    if !is_written {
                        if let Err(e) = std::fs::remove_file(file_path) {
                            log(format!(
                                "[Warning] Failed to remove empty historic log file {e}"
                            ));
                        }
                    }
                    write_to_log_file(format!("[Warning] While fetching logs from Loki {e:?}"))?;
                    return Err(e);
                }
            };
            is_written = true;
            for msg in result.iter() {
                writeln!(log_file, "{}", msg.trim_end())?;
            }
        }
        Ok(())
    }
}

/// Convert a Kubernetes label selector string into a comma-separated list of
/// Loki label matchers, e.g. `"app=io-engine,openebs.io/logging=true"` becomes
/// `app="io-engine",openebs_io_logging="true"`.
fn label_selector_to_logql(selector: &str) -> String {
    selector
        .split(',')
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            Some(format!("{}=\"{}\"", k.replace(['.', '/'], "_"), v))
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn get_epoch_unix_time(since: humantime::Duration) -> SinceTime {
    // should be ok for ~584 years since epoch
    let timestamp = Utc::now()
        .timestamp_nanos_opt()
        .expect("value can not be represented in a timestamp with nanosecond precision.");
    timestamp as SinceTime - since.as_nanos()
}

struct LokiPoll<'a> {
    client: &'a mut LokiClient,
    uri: String,
    endpoint: String,
    since: SinceTime,
    /// URL-encoded LogQL expression (without limit/direction).
    encoded_query: String,
    /// Number of entries to request from Loki per HTTP call.
    /// Updated before each call in `fetch_lines` to avoid over-fetching.
    page_size: u64,
    next_start_epoch_timestamp: SinceTime,
}

use http_body_util::BodyExt;

impl<'a> LokiPoll<'a> {
    // poll_next will extract response from Loki service and perform following actions:
    // 1. Get last log epoch timestamp
    // 2. Extract logs from response
    async fn poll_next(&mut self) -> Result<Option<Vec<String>>, LokiError> {
        let mut start_time = self.since;
        if self.next_start_epoch_timestamp != 0 {
            start_time = self.since;
        }
        let query_params = format!(
            "?query={}&limit={}&direction={}",
            self.encoded_query,
            self.page_size,
            self.client.direction.as_string(),
        );
        let request_str = format!(
            "{}{}{}&start={}",
            self.uri, self.endpoint, query_params, start_time
        );

        // TODO: Test timeouts when Loki service is dropped unexpectedly
        let request = http::Request::builder()
            .method("GET")
            .uri(&request_str)
            .header(X_SCOPE_ORGID, self.client.tenant_id.clone())
            .body(hyper_body::Body::empty())?;

        let response = self.client().ready().await?.call(request).await?;
        if !response.status().is_success() {
            let body_bytes = response.into_body().collect().await?.to_bytes();
            let text = String::from_utf8(body_bytes.to_vec()).unwrap_or_default();
            return Err(LokiError::Response(text));
        }

        let body = response.collect().await?.aggregate();
        let loki_response: LokiResponse = serde_json::from_reader(body.reader())?;

        if loki_response.status == "success" && loki_response.data.result.is_empty() {
            return Ok(None);
        }
        let last_unix_time = loki_response.get_last_stream_unix_time();
        if last_unix_time == 0 {
            return Ok(None);
        }
        // Next time when poll_next is invoked it will continue to fetch logs after last timestamp
        // TODO: Do we need to just add 1 nanosecond instead of 1 mill second?
        self.since = last_unix_time + (1000000);
        let logs = loki_response
            .data
            .result
            .iter()
            .flat_map(|stream| -> Vec<String> {
                stream
                    .values
                    .iter()
                    .map(|value| value.get(1).unwrap_or(&"".to_string()).to_owned())
                    .filter(|val| !val.is_empty())
                    .collect::<Vec<String>>()
            })
            .collect::<Vec<String>>();
        Ok(Some(logs))
    }
    fn client(&mut self) -> &mut kube_proxy::LokiClient {
        &mut self.client.inner_client
    }
}
