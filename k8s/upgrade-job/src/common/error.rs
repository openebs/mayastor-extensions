use crate::{
    common::constants::{CORE_CHART_NAME, PRODUCT, UMBRELLA_CHART_NAME},
    events::event_recorder::EventNote,
};
use snafu::Snafu;
use std::path::PathBuf;
use url::Url;

/// For use with multiple fallible operations which may fail for different reasons, but are
/// defined withing the same scope and must return to the outer scope (calling scope) using
/// the try operator -- '?'.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
#[snafu(context(suffix(false)))]
pub(crate) enum Error {
    /// Error for when the storage REST API URL is parsed.
    #[snafu(display(
        "Failed to parse {} REST API URL {}: {}",
        PRODUCT,
        rest_endpoint,
        source
    ))]
    RestUrlParse {
        source: url::ParseError,
        rest_endpoint: String,
    },

    /// Error for when Kubernetes API client generation fails.
    #[snafu(display("Failed to generate kubernetes client: {}", source))]
    K8sClientGeneration { source: kube_client::Error },

    /// Error for a Kubernetes API GET request for a namespace resource fails.
    #[snafu(display("Failed to GET Kubernetes namespace {}: {}", namespace, source))]
    GetNamespace {
        source: kube::Error,
        namespace: String,
    },

    /// Error for when REST API configuration fails.
    #[snafu(display(
        "Failed to configure {} REST API client with endpoint {}: {:?}",
        PRODUCT,
        rest_endpoint,
        source,
    ))]
    RestClientConfiguration {
        #[snafu(source(false))]
        source: openapi::clients::tower::configuration::Error,
        rest_endpoint: Url,
    },

    /// Error for when a Helm command fails.
    #[snafu(display(
        "Failed to run Helm command,\ncommand: {},\nargs: {:?},\ncommand_error: {}",
        command,
        args,
        source
    ))]
    HelmCommand {
        source: std::io::Error,
        command: String,
        args: Vec<String>,
    },

    /// Error for when regular expression parsing or compilation fails.
    #[snafu(display("Failed to compile regex {}: {}", expression, source))]
    RegexCompile {
        source: regex::Error,
        expression: String,
    },

    /// Error for when Helm v3.x.y is not present in $PATH.
    #[snafu(display("Helm version {} does not start with 'v3.x.y'", version))]
    HelmVersion { version: String },

    /// Error for when input Helm release is not found in the input namespace.
    #[snafu(display(
        "'deployed' Helm release {} not found in Namespace {}",
        name,
        namespace
    ))]
    HelmRelease { name: String, namespace: String },

    /// Error for when there is a lack of valid input for the Helm chart directory for the chart to
    /// be upgraded to.
    #[snafu(display("No input for {} helm chart's directory path", chart_name))]
    NoInputHelmChartDir { chart_name: String },

    /// Error for when the input Pod's owner does not exists.
    #[snafu(display(".metadata.ownerReferences empty for Pod {} in {} namespace, while trying to find Pod's Job owner", pod_name, pod_namespace))]
    JobPodOwnerNotFound {
        pod_name: String,
        pod_namespace: String,
    },

    /// Error for when the number of ownerReferences for this Pod is more than 1.
    #[snafu(display(
        "Pod {} in {} namespace has too many owners, while trying to find Pod's Job owner",
        pod_name,
        pod_namespace
    ))]
    JobPodHasTooManyOwners {
        pod_name: String,
        pod_namespace: String,
    },

    /// Error for when the owner of this Pod is not a Job.
    #[snafu(display("Pod {} in {} namespace has an owner which is not a Job, while trying to find Pod's Job owner", pod_name, pod_namespace))]
    JobPodOwnerIsNotJob {
        pod_name: String,
        pod_namespace: String,
    },

    /// Error for when yaml could not be parsed from a slice.
    #[snafu(display("Failed to parse YAML {}: {}", input_yaml, source))]
    YamlParseFromSlice {
        source: serde_yaml::Error,
        input_yaml: String,
    },

    /// Error for when yaml could not be parsed from a file (Reader).
    #[snafu(display("Failed to parse YAML at {}: {}", filepath.display(), source))]
    YamlParseFromFile {
        source: serde_yaml::Error,
        filepath: PathBuf,
    },

    /// Error for when the Helm chart installed in the cluster is not of the umbrella or core
    /// variant.
    #[snafu(display(
        "Helm chart release {} in Namespace {} has an unsupported chart variant: {}",
        release_name,
        namespace,
        chart_name
    ))]
    DetermineChartVariant {
        release_name: String,
        namespace: String,
        chart_name: String,
    },

    /// Error for when the path to a directory cannot be validated.
    #[snafu(display("Failed to validate directory path {}: {}", path.display(), source))]
    ValidateDirPath {
        source: std::io::Error,
        path: PathBuf,
    },

    /// Error for when the path to a file cannot be validated.
    #[snafu(display("Failed to validate filepath {}: {}", path.display(), source))]
    ValidateFilePath {
        source: std::io::Error,
        path: PathBuf,
    },

    /// Error for when the path is not that of a directory.
    #[snafu(display("{} is not a directory", path.display()))]
    NotADirectory { path: PathBuf },

    /// Error for when the path is not that of a file.
    #[snafu(display("{} is not a file", path.display()))]
    NotAFile { path: PathBuf },

    /// Error when opening a file.
    #[snafu(display("Failed to open file {}: {}", filepath.display(), source))]
    OpeningFile {
        source: std::io::Error,
        filepath: PathBuf,
    },

    /// Error for when the helm chart found in a path is not of the correct variant.
    #[snafu(display("Failed to find valid Helm chart in path {}", path.display()))]
    FindingHelmChart { path: PathBuf },

    /// Error for when a Kubernetes API request for GET-ing a Pod fails.
    #[snafu(display(
        "Failed to GET Kubernetes Pod {} in namespace {}: {}",
        pod_name,
        pod_namespace,
        source
    ))]
    GetPod {
        source: kube::Error,
        pod_name: String,
        pod_namespace: String,
    },

    /// Error for when a Kubernetes API request for GET-ing a list of Pods filtered by label(s)
    /// fails.
    #[snafu(display(
        "Failed to list Pods with label {} in namespace {}: {}",
        label,
        namespace,
        source
    ))]
    ListPodsWithLabel {
        source: kube::Error,
        label: String,
        namespace: String,
    },

    /// Error for when a Pod does not have a PodSpec struct member.
    #[snafu(display("Failed get .spec from Pod {} in Namespace {}", name, namespace))]
    EmptyPodSpec { name: String, namespace: String },

    /// Error for when the spec.nodeName of a Pod is empty.
    #[snafu(display(
        "Failed get .spec.nodeName from Pod {} in Namespace {}",
        name,
        namespace
    ))]
    EmptyPodNodeName { name: String, namespace: String },

    /// Error for when an uncordon request for a storage node fails.
    #[snafu(display("Failed to uncordon {} Node {}: {}", PRODUCT, node_name, source))]
    StorageNodeUncordon {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
        node_name: String,
    },

    /// Error for when an Pod-delete Kubernetes API request fails.
    #[snafu(display("Failed get delete Pod {} from Node {}: {}", name, node, source))]
    PodDelete {
        source: kube::Error,
        name: String,
        node: String,
    },

    /// Error for when listing storage nodes fails.
    #[snafu(display("Failed to list {} Nodes: {}", PRODUCT, source))]
    ListStorageNodes {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
    },

    /// Error for when GET-ing a storage node fails.
    #[snafu(display("Failed to list {} Node {}: {}", PRODUCT, node_name, source))]
    GetStorageNode {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
        node_name: String,
    },

    /// Error for when the storage node's Spec is empty.
    #[snafu(display("Failed to get {} Node {}", PRODUCT, node_id))]
    EmptyStorageNodeSpec { node_id: String },

    /// Error for when a GET request for a list of storage volumes fails.
    #[snafu(display("Failed to list {} Volumes: {}", PRODUCT, source))]
    ListStorageVolumes {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
    },

    /// Error for when a storage node drain request fails.
    #[snafu(display("Failed to drain {} Node {}: {}", PRODUCT, node_name, source))]
    DrainStorageNode {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
        node_name: String,
    },

    /// Error for when a Pod's Ready status validated to be 'true'.
    #[snafu(display("Pod {} in Namespace {} is not running", name, namespace))]
    ValidatingPodReadyStatus { name: String, namespace: String },

    /// Error for when the requested YAML key is invalid.
    #[snafu(display("Failed to parse YAML path {}", yaml_path))]
    YamlStructure { yaml_path: String },

    /// Error for use when converting Vec<> to String.
    #[snafu(display("Failed to convert Vec<u8> to UTF-8 formatted String: {}", source))]
    U8VectorToString { source: std::str::Utf8Error },

    /// Error when publishing kube-events for the Job object.
    #[snafu(display("Failed to publish Event: {}", source))]
    EventPublish { source: kube_client::Error },

    /// Error for when a Helm list command execution succeeds, but with an error.
    #[snafu(display(
        "`helm list` command return an error,\ncommand: {},\nargs: {:?},\nstd_err: {}",
        command,
        args,
        std_err,
    ))]
    HelmListCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when a Helm version command execution succeeds, but with an error.
    #[snafu(display(
        "`helm version` command return an error,\ncommand: {},\nargs: {:?},\nstd_err: {}",
        command,
        args,
        std_err,
    ))]
    HelmVersionCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when a Helm upgrade command execution succeeds, but with an error.
    #[snafu(display(
        "`helm upgrade` command return an error,\ncommand: {},\nargs: {:?},\nstd_err: {}",
        command,
        args,
        std_err,
    ))]
    HelmUpgradeCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when a Helm get values command execution succeeds, but with an error.
    #[snafu(display(
        "`helm get values` command return an error,\ncommand: {},\nargs: {:?},\nstd_err: {}",
        command,
        args,
        std_err,
    ))]
    HelmGetValuesCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when detected helm chart name is not known helm chart.
    #[snafu(display(
        "'{}' is not a known {} helm chart, only the '{}' and '{}' charts are supported",
        chart_name,
        PRODUCT,
        CORE_CHART_NAME,
        UMBRELLA_CHART_NAME
    ))]
    NotAKnownHelmChart { chart_name: String },

    /// Error for when namespace option is not set when building KubeClientSet.
    #[snafu(display("Mandatory KubeClientSetBuilder option 'namespace' not set"))]
    KubeClientSetBuilderNs,

    /// Error for when mandatory options for an EventRecorder are missing when building.
    #[snafu(display("Mandatory options for EventRecorder were not given"))]
    EventRecorderOptionsAbsent,

    /// Error for mandatory options for a HelmClient are missing when building.
    #[snafu(display("Setting namespace is mandatory for HelmClient"))]
    HelmClientNs,

    /// Error for mandatory options for a HelmUpgrade are missing when building.
    #[snafu(display("Mandatory options for EventRecorder were not given"))]
    HelmUpgradeOptionsAbsent,

    #[snafu(display("Failed to parse {} as a valid semver: {}", version_string, source))]
    SemverParse {
        source: semver::Error,
        version_string: String,
    },

    #[snafu(display(
        "'{}' chart is not a subchart of '{}' chart",
        CORE_CHART_NAME,
        UMBRELLA_CHART_NAME
    ))]
    CoreNotASubchartOfUmbrella,

    #[snafu(display(
        "Upgrade for {} chart v{} is not supported",
        UMBRELLA_CHART_NAME,
        version
    ))]
    UmbrellaChartVersionInvalid { version: String },

    #[snafu(display("The upgrade path is invalid"))]
    InvalidUpgradePath,

    #[snafu(display("Failed to serialize event note {:?}: {}", note, source))]
    SerializeEventNote {
        source: serde_json::Error,
        note: EventNote,
    },
}
/// A wrapper type to remove repeated Result<T, Error> returns.
pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
