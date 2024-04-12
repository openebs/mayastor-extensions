use crate::{
    common::constants::{
        CHART_VERSION_LABEL_KEY, CORE_CHART_NAME, PRODUCT, TO_UMBRELLA_SEMVER, UMBRELLA_CHART_NAME,
        UMBRELLA_CHART_UPGRADE_DOCS_URL,
    },
    events::event_recorder::EventNote,
    helm::chart::PromtailConfigClient,
};
use k8s_openapi::api::core::v1::Container;
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
    #[snafu(display(
        ".metadata.ownerReferences empty for Pod {} in {} namespace, while trying to find Pod's Job owner",
        pod_name,
        pod_namespace
    ))]
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
    #[snafu(display(
        "Pod {} in {} namespace has an owner which is not a Job, while trying to find Pod's Job owner",
        pod_name,
        pod_namespace
    ))]
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

    /// Error for when yaml could not be parsed from bytes.
    #[snafu(display("Failed to parse unsupported versions yaml: {}", source))]
    YamlParseBufferForUnsupportedVersion { source: serde_yaml::Error },

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

    /// Error when reading a file.
    #[snafu(display("Failed to read from file {}: {}", filepath.display(), source))]
    ReadingFile {
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

    /// Error for when a Kubernetes API request for GET-ing a list of Nodes filtered by label(s)
    /// fails.
    #[snafu(display("Failed to list Nodes with label {}: {}", label, source))]
    ListNodesWithLabel { source: kube::Error, label: String },

    /// Error for when a Kubernetes API request for GET-ing a list of Pods filtered by label(s)
    /// and field(s) fails.
    #[snafu(display(
        "Failed to list Pods with label '{}', and field '{}' in namespace {}: {}",
        label,
        field,
        namespace,
        source
    ))]
    ListPodsWithLabelAndField {
        source: kube::Error,
        label: String,
        field: String,
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

    /// Error for when the metadata.uid of a Pod is empty.
    #[snafu(display(
        "Failed to get .metadata.uid from Pod {} in Namespace {}",
        name,
        namespace
    ))]
    EmptyPodUid { name: String, namespace: String },

    /// Error for when an uncordon request for a storage node fails.
    #[snafu(display("Failed to uncordon {} Node {}: {}", PRODUCT, node_id, source))]
    StorageNodeUncordon {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
        node_id: String,
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
    #[snafu(display("Failed to list {} Node {}: {}", PRODUCT, node_id, source))]
    GetStorageNode {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
        node_id: String,
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
    #[snafu(display("Failed to drain {} Node {}: {}", PRODUCT, node_id, source))]
    DrainStorageNode {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
        node_id: String,
    },

    /// Error for when a storage node cordon request fails.
    #[snafu(display("Failed to cordon {} Node {}: {}", PRODUCT, node_id, source))]
    CordonStorageNode {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
        node_id: String,
    },

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
        "'{}' is not a known {} helm chart, only helm charts '{}-<version-tag>' and '{}-<version-tag>' \
        are supported",
        chart_name,
        PRODUCT,
        CORE_CHART_NAME,
        UMBRELLA_CHART_NAME
    ))]
    NotAKnownHelmChart { chart_name: String },

    /// Error for when mandatory options for an EventRecorder are missing when building.
    #[snafu(display("Mandatory options for EventRecorder were not given"))]
    EventRecorderOptionsAbsent,

    /// Error for when pod uid is not present.
    #[snafu(display("Pod Uid is None"))]
    PodUidIsNone,

    /// Error for mandatory options for a HelmClient are missing when building.
    #[snafu(display("Setting namespace is mandatory for HelmClient"))]
    HelmClientNs,

    /// Error for when the helm release name is missing when building a HelmUpgrade.
    #[snafu(display("A mandatory options for helm upgrade was not given: no release name"))]
    HelmUpgradeOptionReleaseNameAbsent,

    /// Error for when the kubernetes namesapce is missing when building a HelmUpgrade.
    #[snafu(display(
        "A mandatory options for helm upgrade was not given: no helm release namespace"
    ))]
    HelmUpgradeOptionNamespaceAbsent,

    /// Error for failures in generating semver::Value from a &str input.
    #[snafu(display("Failed to parse {} as a valid semver: {}", version_string, source))]
    SemverParse {
        source: semver::Error,
        version_string: String,
    },

    /// Error for when the detected upgrade path for PRODUCT is not supported.
    #[snafu(display("The upgrade path is invalid"))]
    InvalidUpgradePath,

    /// Error in serializing crate::event::event_recorder::EventNote to JSON string.
    #[snafu(display("Failed to serialize event note {:?}: {}", note, source))]
    SerializeEventNote {
        source: serde_json::Error,
        note: EventNote,
    },

    /// Error in serializing a helm::chart::PromtailConfigClient to a JSON string.
    #[snafu(display(
        "Failed to serialize .loki-stack.promtail.config.client {:?}: {}",
        object,
        source
    ))]
    SerializePromtailConfigClientToJson {
        source: serde_json::Error,
        object: PromtailConfigClient,
    },

    /// Error in serializing a k8s_openapi::api::core::v1::Container to a JSON string.
    #[snafu(display(
        "Failed to serialize .loki-stack.promtail.initContainer {:?}: {}",
        object,
        source
    ))]
    SerializePromtailInitContainerToJson {
        source: serde_json::Error,
        object: Container,
    },

    /// Error in deserializing a promtail helm chart's deprecated extraClientConfig to a
    /// serde_json::Value.
    #[snafu(display(
        "Failed to deserialize .loki-stack.promtail.config.snippets.extraClientConfig to a serde_json::Value {}: {}",
        config,
        source
    ))]
    DeserializePromtailExtraConfig {
        source: serde_yaml::Error,
        config: String,
    },

    /// Error in serializing a promtail helm chart's deprecated extraClientConfig, in a
    /// serde_json::Value, to JSON.
    #[snafu(display("Failed to serialize to JSON {:?}: {}", config, source))]
    SerializePromtailExtraConfigToJson {
        source: serde_json::Error,
        config: serde_json::Value,
    },

    /// Error in serializing the deprecated config.snippets.extraClientConfig from the promtail
    /// helm chart v3.11.0.
    #[snafu(display(
        "Failed to serialize object to a serde_json::Value {}: {}",
        object,
        source
    ))]
    SerializePromtailExtraClientConfigToJson {
        source: serde_json::Error,
        object: String,
    },

    /// Error for when there are too many io-engine Pods in one single node;
    #[snafu(display("Too many io-engine Pods in Node '{}'", node_name))]
    TooManyIoEnginePods { node_name: String },

    /// Error for when the thin-provisioning options are absent, but still tried to fetch it.
    #[snafu(display("The agents.core.capacity yaml object is absent amongst the helm values"))]
    ThinProvisioningOptionsAbsent,

    /// Error when trying to send Events through the tokio::sync::channel::Sender<Event>
    /// synchronisation tool.
    #[snafu(display("Failed to send Event over the channel"))]
    EventChannelSend,

    /// Error for the Umbrella chart is not upgraded.
    #[snafu(display(
        "The {} helm chart is not upgraded to version {}: Upgrade for helm chart {} is not \
        supported, refer to the instructions at {} to upgrade your release of the {} helm \
        chart to version {}",
        UMBRELLA_CHART_NAME,
        TO_UMBRELLA_SEMVER,
        UMBRELLA_CHART_NAME,
        UMBRELLA_CHART_UPGRADE_DOCS_URL,
        UMBRELLA_CHART_NAME,
        TO_UMBRELLA_SEMVER,
    ))]
    UmbrellaChartNotUpgraded,

    /// Error for when the helm upgrade for the Core chart does not have a chart directory.
    #[snafu(display(
        "The {} helm chart could not be upgraded as input chart directory is absent",
        CORE_CHART_NAME
    ))]
    CoreChartUpgradeNoneChartDir,

    /// Error for when the Storage REST API Deployment is absent.
    #[snafu(display(
        "Found no {} REST API Deployments in the namespace {} with labelSelector {}",
        PRODUCT,
        namespace,
        label_selector
    ))]
    NoRestDeployment {
        namespace: String,
        label_selector: String,
    },

    /// Error for when the CHART_VERSION_LABEL_KEY is missing amongst the labels in a Deployment.
    #[snafu(display(
        "A label with the key {} was not found for Deployment {} in namespace {}",
        CHART_VERSION_LABEL_KEY,
        deployment_name,
        namespace
    ))]
    NoVersionLabelInDeployment {
        deployment_name: String,
        namespace: String,
    },

    /// Error for when a Kubernetes API request for GET-ing a list of Deployments filtered by
    /// label(s) fails.
    #[snafu(display(
        "Failed to list Deployments with label {} in namespace {}: {}",
        label_selector,
        namespace,
        source
    ))]
    ListDeploymentsWithLabel {
        source: kube::Error,
        namespace: String,
        label_selector: String,
    },

    /// Error for when the helm upgrade run is that of an invalid chart configuration.
    #[snafu(display("Invalid helm upgrade request"))]
    InvalidHelmUpgrade,

    /// Error for when the helm upgrade's target version is lower the source version.
    #[snafu(display(
        "Failed to upgrade from {} to {}: upgrade to an earlier-released version is forbidden",
        source_version,
        target_version
    ))]
    RollbackForbidden {
        source_version: String,
        target_version: String,
    },

    /// Error for when yq command execution fails.
    #[snafu(display(
        "Failed to run yq command,\ncommand: {},\nargs: {:?},\ncommand_error: {}",
        command,
        args,
        source
    ))]
    YqCommandExec {
        source: std::io::Error,
        command: String,
        args: Vec<String>,
    },

    /// Error for when the `yq -V` command returns an error.
    #[snafu(display(
        "`yq -V` command return an error,\ncommand: {},\narg: {},\nstd_err: {}",
        command,
        arg,
        std_err,
    ))]
    YqVersionCommand {
        command: String,
        arg: String,
        std_err: String,
    },

    /// Error for when the `yq eq` command returns an error.
    #[snafu(display(
        "`yq ea` command return an error,\ncommand: {},\nargs: {:?},\nstd_err: {}",
        command,
        args,
        std_err,
    ))]
    YqMergeCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when the yq version present is not v4.x.y.
    #[snafu(display("yq version is not v4"))]
    NotYqV4,

    /// Error for when temporary file creation fails.
    #[snafu(display("Failed to create temporary file: {}", source))]
    TempFileCreation { source: std::io::Error },

    /// Error for when we fail to write to a temporary file.
    #[snafu(display("Failed to write to temporary file {}: {}", filepath.display(), source))]
    WriteToTempFile {
        source: std::io::Error,
        filepath: PathBuf,
    },

    /// Error for when the input yaml key for a string value isn't a valid one.
    #[snafu(display("{} is not a valid yaml key for a string value", key))]
    NotAValidYamlKeyForStringValue { key: String },

    /// Error for when the yq command to update the value of a yaml field returns an error.
    #[snafu(display(
        "`yq` set-value-command returned an error,\ncommand: {},\nargs: {:?},\nstd_err: {}",
        command,
        args,
        std_err,
    ))]
    YqSetCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when the yq command to delete an object path returns an error.
    #[snafu(display(
        "`yq` delete-object-command returned an error,\ncommand: {},\nargs: {:?},\nstd_err: {}",
        command,
        args,
        std_err,
    ))]
    YqDeleteObjectCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when the yq command to append to an array returns an error.
    #[snafu(display(
        "`yq` append-to-array-command returned an error,\ncommand: {},\nargs: {:?},\nstd_err: {}",
        command,
        args,
        std_err,
    ))]
    YqAppendToArrayCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when the yq command to append to an object returns an error.
    #[snafu(display(
        "`yq` append-to-object-command returned an error,\ncommand: {},\nargs: {:?},\nstd_err: {}",
        command,
        args,
        std_err,
    ))]
    YqAppendToObjectCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    #[snafu(display("failed to list CustomResourceDefinitions: {source}"))]
    ListCrds { source: kube::Error },

    #[snafu(display("Partial rebuild must be disabled for upgrades from {chart_name} chart versions >= {lower_extent}, <= {upper_extent}"))]
    PartialRebuildNotAllowed {
        chart_name: String,
        lower_extent: String,
        upper_extent: String,
    },
}

/// A wrapper type to remove repeated Result<T, Error> returns.
pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
