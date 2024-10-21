use crate::{
    common::constants::{
        product_train, CORE_CHART_NAME, UMBRELLA_CHART_NAME, UMBRELLA_CHART_UPGRADE_DOCS_URL,
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
        "Failed to parse {} REST API URL {rest_endpoint}: {source}",
        product_train(),
    ))]
    RestUrlParse {
        source: url::ParseError,
        rest_endpoint: String,
    },

    /// Error for when Kubernetes API client generation fails.
    #[snafu(display("Failed to generate kubernetes client: {source}"))]
    K8sClientGeneration { source: kube::Error },

    /// Error for a Kubernetes API GET request for a namespace resource fails.
    #[snafu(display("Failed to GET Kubernetes namespace '{namespace}': {source}"))]
    GetNamespace {
        source: kube::Error,
        namespace: String,
    },

    /// Error for when REST API configuration fails.
    #[snafu(display(
        "Failed to configure {} REST API client with endpoint '{rest_endpoint}': {source:?}",
        product_train(),
    ))]
    RestClientConfiguration {
        #[snafu(source(false))]
        source: openapi::clients::tower::configuration::Error,
        rest_endpoint: Url,
    },

    /// Error for when a Helm command fails.
    #[snafu(display(
        "Failed to run Helm command,\ncommand: {command},\nargs: {args:?},\ncommand_error: {source}",
    ))]
    HelmCommand {
        source: std::io::Error,
        command: String,
        args: Vec<String>,
    },

    /// Error for when regular expression parsing or compilation fails.
    #[snafu(display("Failed to compile regex {expression}: {source}"))]
    RegexCompile {
        source: regex::Error,
        expression: String,
    },

    /// Error for when Helm v3.x.y is not present in $PATH.
    #[snafu(display("Helm version {version} does not start with 'v3.x.y'"))]
    HelmVersion { version: String },

    /// Error for when input Helm release is not found in the input namespace.
    #[snafu(display("'deployed' Helm release {name} not found in Namespace '{namespace}'"))]
    HelmRelease { name: String, namespace: String },

    /// Error for when no value for helm storage driver is set.
    #[snafu(display("No helm storage driver specified"))]
    NoHelmStorageDriver,

    /// Error for when there's too few or too many helm secrets for a release in a namespace.
    #[snafu(display(
        "'{count}' is an invalid no. of helm Secrets for release '{release_name}' in namespace '{namespace}'"
    ))]
    InvalidNoOfHelmSecrets {
        release_name: String,
        namespace: String,
        count: usize,
    },

    /// Error for when there's too few or too many helm configmaps for a release in a namespace.
    #[snafu(display(
        "'{count}' is an invalid no. of helm ConfigMaps for release '{release_name}' in namespace '{namespace}'"
    ))]
    InvalidNoOfHelmConfigMaps {
        release_name: String,
        namespace: String,
        count: usize,
    },

    /// Error for when there's no data in helm storage driver.
    #[snafu(display("No data in helm {driver}"))]
    HelmStorageNoData { driver: &'static str },

    /// Error for when there's no value for the release key in helm storage data.
    #[snafu(display("No value mapped to the 'release' key in helm {driver}"))]
    HelmStorageNoReleaseValue { driver: &'static str },

    /// Error for when the helm storage driver is not supported.
    #[snafu(display("'{driver}' is not a supported helm storage driver"))]
    UnsupportedStorageDriver { driver: String },

    /// Error for when there is a lack of valid input for the Helm chart directory for the chart to
    /// be upgraded to.
    #[snafu(display("No input for {chart_name} helm chart's directory path"))]
    NoInputHelmChartDir { chart_name: String },

    /// Error for when the input Pod's owner does not exists.
    #[snafu(display(
        ".metadata.ownerReferences empty for Pod '{pod_name}' in '{pod_namespace}' namespace, while trying to find Pod's Job owner",
    ))]
    JobPodOwnerNotFound {
        pod_name: String,
        pod_namespace: String,
    },

    /// Error for when the number of ownerReferences for this Pod is more than 1.
    #[snafu(display(
        "Pod '{pod_name}' in '{pod_namespace}' namespace has too many owners, while trying to find Pod's Job owner",
    ))]
    JobPodHasTooManyOwners {
        pod_name: String,
        pod_namespace: String,
    },

    /// Error for when the owner of this Pod is not a Job.
    #[snafu(display(
        "Pod '{pod_name}' in '{pod_namespace}' namespace has an owner which is not a Job, while trying to find Pod's Job owner",
    ))]
    JobPodOwnerIsNotJob {
        pod_name: String,
        pod_namespace: String,
    },

    /// Error for when yaml could not be parsed from a slice.
    #[snafu(display("Failed to parse YAML {input_yaml}: {source}"))]
    YamlParseFromSlice {
        source: serde_yaml::Error,
        input_yaml: String,
    },

    /// Error for when yaml could not be parsed from a file (Reader).
    #[snafu(display("Failed to parse YAML at {}: {source}", filepath.display()))]
    YamlParseFromFile {
        source: serde_yaml::Error,
        filepath: PathBuf,
    },

    /// Error for when yaml could not be parsed from bytes.
    #[snafu(display("Failed to parse unsupported versions yaml: {source}"))]
    YamlParseBufferForUnsupportedVersion { source: serde_yaml::Error },

    /// Error for when the Helm chart installed in the cluster is not of the umbrella or core
    /// variant.
    #[snafu(display(
        "Helm chart release {release_name} in Namespace '{namespace}' has an unsupported chart variant: {chart_name}",
    ))]
    DetermineChartVariant {
        release_name: String,
        namespace: String,
        chart_name: String,
    },

    /// Error for when the path to a directory cannot be validated.
    #[snafu(display("Failed to validate directory path {}: {source}", path.display()))]
    ValidateDirPath {
        source: std::io::Error,
        path: PathBuf,
    },

    /// Error for when the path to a file cannot be validated.
    #[snafu(display("Failed to validate filepath {}: {source}", path.display()))]
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
    #[snafu(display("Failed to read from file {}: {source}", filepath.display()))]
    ReadingFile {
        source: std::io::Error,
        filepath: PathBuf,
    },

    /// Error for when the helm chart found in a path is not of the correct variant.
    #[snafu(display("Failed to find valid Helm chart in path {}", path.display()))]
    FindingHelmChart { path: PathBuf },

    /// Error for when a Kubernetes API request for GET-ing a Pod fails.
    #[snafu(display(
        "Failed to GET Kubernetes Pod '{pod_name}' in namespace '{pod_namespace}': {source}",
    ))]
    GetPod {
        source: kube::Error,
        pod_name: String,
        pod_namespace: String,
    },

    /// Error for when a Kubernetes API request for GET-ing a list of Pods filtered by label(s)
    /// and field(s) fails.
    #[snafu(display(
        "Failed to list Pods with label '{label}', and field '{field}' in namespace '{namespace}': {source}",
    ))]
    ListPodsWithLabelAndField {
        source: kube::Error,
        label: String,
        field: String,
        namespace: String,
    },

    /// Error for when listing Kubernetes Secrets from a the kubeapi fails.
    #[snafu(display(
        "Failed to list Secrets with label '{label}', and field '{field}' in namespace '{namespace}': {source}",
    ))]
    ListSecretsWithLabelAndField {
        source: kube::Error,
        label: String,
        field: String,
        namespace: String,
    },

    #[snafu(display(
        "Failed to list ConfigMaps with label '{label}', and field '{field}' in namespace '{namespace}': {source}",
    ))]
    ListConfigMapsWithLabelAndField {
        source: kube::Error,
        label: String,
        field: String,
        namespace: String,
    },

    /// Error for when a Kubernetes API request for GET-ing a list of ControllerRevisions
    /// filtered by label(s) and field(s) fails.
    #[snafu(display(
        "Failed to list ControllerRevisions with label '{label}', and field '{field}' in Namespace '{namespace}': {source}",
    ))]
    ListCtrlRevsWithLabelAndField {
        source: kube::Error,
        label: String,
        field: String,
        namespace: String,
    },

    /// Error for when a Kubernetes API request for GET-ing a list of Nodes filtered by label(s)
    /// and field(s) fails.
    #[snafu(display(
        "Failed to list Kubernetes Nodes with label '{label}', and field '{field}': {source}",
    ))]
    ListNodesWithLabelAndField {
        source: kube::Error,
        label: String,
        field: String,
    },

    /// Error for when a Pod does not have a PodSpec struct member.
    #[snafu(display("Failed get .spec from Pod {name} in Namespace '{namespace}'"))]
    EmptyPodSpec { name: String, namespace: String },

    /// Error for when the spec.nodeName of a Pod is empty.
    #[snafu(display("Failed get .spec.nodeName from Pod {name} in Namespace '{namespace}'",))]
    EmptyPodNodeName { name: String, namespace: String },

    /// Error for when the metadata.uid of a Pod is empty.
    #[snafu(display("Failed to get .metadata.uid from Pod {name} in Namespace '{namespace}'",))]
    EmptyPodUid { name: String, namespace: String },

    /// Error for when an uncordon request for a storage node fails.
    #[snafu(display("Failed to uncordon {} Node {node_id}: {source}", product_train()))]
    StorageNodeUncordon {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
        node_id: String,
    },

    /// Error for when an Pod-delete Kubernetes API request fails.
    #[snafu(display("Failed get delete Pod {name} from Node {node}: {source}"))]
    PodDelete {
        source: kube::Error,
        name: String,
        node: String,
    },

    /// Error for when listing storage nodes fails.
    #[snafu(display("Failed to list {} Nodes: {source}", product_train()))]
    ListStorageNodes {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
    },

    /// Error for when GET-ing a storage node fails.
    #[snafu(display("Failed to list {} Node {node_id}: {source}", product_train()))]
    GetStorageNode {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
        node_id: String,
    },

    /// Error for when the storage node's Spec is empty.
    #[snafu(display("Failed to get {} Node {node_id}", product_train()))]
    EmptyStorageNodeSpec { node_id: String },

    /// Error for when a GET request for a list of storage volumes fails.
    #[snafu(display("Failed to list {} Volumes: {source}", product_train()))]
    ListStorageVolumes {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
    },

    /// Error for when a storage node drain request fails.
    #[snafu(display("Failed to drain {} Node {node_id}: {source}", product_train()))]
    DrainStorageNode {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
        node_id: String,
    },

    /// Error for when a storage node cordon request fails.
    #[snafu(display("Failed to cordon {} Node {node_id}: {source}", product_train()))]
    CordonStorageNode {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
        node_id: String,
    },

    /// Error for when the requested YAML key is invalid.
    #[snafu(display("Failed to parse YAML path {yaml_path}"))]
    YamlStructure { yaml_path: String },

    /// Error for use when converting Vec<> to String.
    #[snafu(display("Failed to convert Vec<u8> to UTF-8 formatted String: {source}"))]
    U8VectorToString { source: std::str::Utf8Error },

    /// Error when publishing kube-events for the Job object.
    #[snafu(display("Failed to publish Event: {source}"))]
    EventPublish { source: kube::Error },

    /// Error for when the 'chart' member of a crate::helm::client::HelmReleaseElement cannot be
    /// split at the first occurrence of '-', e.g. <chart-name>-2.1.0-rc8.
    #[snafu(display(
        "Failed to split helm chart name '{chart_name}', at the first occurrence of '{delimiter}'",
    ))]
    HelmChartNameSplit { chart_name: String, delimiter: char },

    /// Error for when a Helm list command execution succeeds, but with an error.
    #[snafu(display(
        "`helm list` command return an error,\ncommand: {command},\nargs: {args:?},\nstd_err: {std_err}",
    ))]
    HelmListCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when a Helm version command execution succeeds, but with an error.
    #[snafu(display(
        "`helm version` command return an error,\ncommand: {command},\nargs: {args:?},\nstd_err: {std_err}",
    ))]
    HelmVersionCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when a Helm upgrade command execution succeeds, but with an error.
    #[snafu(display(
        "`helm upgrade` command return an error,\ncommand: {command},\nargs: {args:?},\nstd_err: {std_err}",
    ))]
    HelmUpgradeCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when a Helm get values command execution succeeds, but with an error.
    #[snafu(display(
        "`helm get values` command return an error,\ncommand: {command},\nargs: {args:?},\nstd_err: {std_err}",
    ))]
    HelmGetValuesCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when detected helm chart name is not known helm chart.
    #[snafu(display(
        "'{chart_name}' is not a known {} helm chart, only helm charts '{CORE_CHART_NAME}-<version-tag>' and '{UMBRELLA_CHART_NAME}-<version-tag>' are supported",
        product_train(),
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
    #[snafu(display("Failed to parse {version_string} as a valid semver: {source}"))]
    SemverParse {
        source: semver::Error,
        version_string: String,
    },

    /// Error for when the detected upgrade path for product_train() is not supported.
    #[snafu(display("The upgrade path is invalid"))]
    InvalidUpgradePath,

    /// Error in serializing crate::event::event_recorder::EventNote to JSON string.
    #[snafu(display("Failed to serialize event note {note:?}: {source}"))]
    SerializeEventNote {
        source: serde_json::Error,
        note: EventNote,
    },

    /// Error in serializing a helm::chart::PromtailConfigClient to a JSON string.
    #[snafu(display(
        "Failed to serialize .loki-stack.promtail.config.client {object:?}: {source}",
    ))]
    SerializePromtailConfigClientToJson {
        source: serde_json::Error,
        object: PromtailConfigClient,
    },

    /// Error in serializing a k8s_openapi::api::core::v1::Container to a JSON string.
    #[snafu(display(
        "Failed to serialize .loki-stack.promtail.initContainer {object:?}: {source}",
    ))]
    SerializePromtailInitContainerToJson {
        source: serde_json::Error,
        object: Container,
    },

    /// Error in deserializing a promtail helm chart's deprecated extraClientConfig to a
    /// serde_json::Value.
    #[snafu(display(
        "Failed to deserialize .loki-stack.promtail.config.snippets.extraClientConfig to a serde_json::Value {config}: {source}",
    ))]
    DeserializePromtailExtraConfig {
        source: serde_yaml::Error,
        config: String,
    },

    /// Error in serializing a promtail helm chart's deprecated extraClientConfig, in a
    /// serde_json::Value, to JSON.
    #[snafu(display("Failed to serialize to JSON {config:?}: {source}"))]
    SerializePromtailExtraConfigToJson {
        source: serde_json::Error,
        config: serde_json::Value,
    },

    /// Error in serializing the deprecated config.snippets.extraClientConfig from the promtail
    /// helm chart v3.11.0.
    #[snafu(display("Failed to serialize object to a serde_json::Value {object}: {source}",))]
    SerializePromtailExtraClientConfigToJson {
        source: serde_json::Error,
        object: String,
    },

    /// Error for when there are too many io-engine Pods in one single node;
    #[snafu(display("Too many io-engine Pods in Node '{node_name}'"))]
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
        "The '{UMBRELLA_CHART_NAME}' helm chart is not upgraded to a version with '{CORE_CHART_NAME}' dependency helm chart version '{target_version}': Upgrade for helm chart {UMBRELLA_CHART_NAME} is not supported, refer to the instructions at {UMBRELLA_CHART_UPGRADE_DOCS_URL} to upgrade your release of the '{UMBRELLA_CHART_NAME}' helm chart.",
    ))]
    UmbrellaChartNotUpgraded { target_version: String },

    /// Error for when the helm upgrade for the Core chart does not have a chart directory.
    #[snafu(display(
        "The {CORE_CHART_NAME} helm chart could not be upgraded as input chart directory is absent",
    ))]
    CoreChartUpgradeNoneChartDir,

    /// Error for when the helm upgrade run is that of an invalid chart configuration.
    #[snafu(display("Invalid helm upgrade request"))]
    InvalidHelmUpgrade,

    /// Error for when the helm upgrade's target version is lower the source version.
    #[snafu(display(
        "Failed to upgrade from {source_version} to {target_version}: upgrade to an earlier-released version is forbidden",
    ))]
    RollbackForbidden {
        source_version: String,
        target_version: String,
    },

    /// Error for when yq command execution fails.
    #[snafu(display(
        "Failed to run yq command,\ncommand: {command},\nargs: {args:?},\ncommand_error: {source}",
    ))]
    YqCommandExec {
        source: std::io::Error,
        command: String,
        args: Vec<String>,
    },

    /// Error for when the `yq -V` command returns an error.
    #[snafu(display(
        "`yq -V` command return an error,\ncommand: {command},\narg: {arg},\nstd_err: {std_err}",
    ))]
    YqVersionCommand {
        command: String,
        arg: String,
        std_err: String,
    },

    /// Error for when the `yq eq` command returns an error.
    #[snafu(display(
        "`yq ea` command return an error,\ncommand: {command},\nargs: {args:?},\nstd_err: {std_err}",
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
    #[snafu(display("Failed to create temporary file: {source}"))]
    TempFileCreation { source: std::io::Error },

    /// Error for when we fail to write to a temporary file.
    #[snafu(display("Failed to write to temporary file {}: {source}", filepath.display()))]
    WriteToTempFile {
        source: std::io::Error,
        filepath: PathBuf,
    },

    /// Error for when the input yaml key for a string value isn't a valid one.
    #[snafu(display("{key} is not a valid yaml key for a string value"))]
    NotAValidYamlKeyForStringValue { key: String },

    /// Error for when the yq command to update the value of a yaml field returns an error.
    #[snafu(display(
        "`yq` set-value-command returned an error,\ncommand: {command},\nargs: {args:?},\nstd_err: {std_err}",
    ))]
    YqSetCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when the yq command to delete an object path returns an error.
    #[snafu(display(
        "`yq` delete-object-command returned an error,\ncommand: {command},\nargs: {args:?},\nstd_err: {std_err}",
    ))]
    YqDeleteObjectCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when the yq command to append to an array returns an error.
    #[snafu(display(
        "`yq` append-to-array-command returned an error,\ncommand: {command},\nargs: {args:?},\nstd_err: {std_err}",
    ))]
    YqAppendToArrayCommand {
        command: String,
        args: Vec<String>,
        std_err: String,
    },

    /// Error for when the yq command to append to an object returns an error.
    #[snafu(display(
        "`yq` append-to-object-command returned an error,\ncommand: {command},\nargs: {args:?},\nstd_err: {std_err}",
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

    /// Error for when the list of ControllerRevisions for a controller's resource is empty.
    #[snafu(display(
        "No ControllerRevisions found in namespace '{namespace}' with label selector '{label_selector}' and field selector '{field_selector}'"
    ))]
    ControllerRevisionListEmpty {
        namespace: String,
        label_selector: String,
        field_selector: String,
    },

    /// Error for when a ControllerRevision doesn't have a label key containing the controller
    /// revision hash.
    #[snafu(display(
        "ControllerRevisions '{name}' in namespace '{namespace}' doesn't have label key '{hash_label_key}'"
    ))]
    ControllerRevisionDoesntHaveHashLabel {
        name: String,
        namespace: String,
        hash_label_key: String,
    },

    /// Error for when base64 decode fails for helm storage data.
    #[snafu(display("Failed to decode helm storage data"))]
    Base64DecodeHelmStorage { source: base64::DecodeError },

    /// Error for when Gzip decompressed data fails copy to byte buffer.
    #[snafu(display("Failed to copy gzip decompressed data to byte buffer: {source}"))]
    GzipDecoderReadToEnd { source: std::io::Error },

    /// Error for when Deserializing the JSON stored in a helm storage driver (secret or cm) fails.
    #[snafu(display("Failed to deserialize helm storage data from JSON: {source}"))]
    DeserializaHelmStorageData { source: serde_json::Error },

    /// Error for when an expected JSON member in the helm storage data is missing.
    #[snafu(display("Couldn't find '{member}' in helm storage data"))]
    MissingMemberInHelmStorageData { member: &'static str },

    /// Error for when helm dependency data in a helm storage driver contains an invalid/missing
    /// entry for the CORE_CHART version.
    #[snafu(display("Helm release data doesn't have chart version or contains an invalid version for dependency chart '{CORE_CHART_NAME}'"))]
    InvalidDependencyVersionInHelmReleaseData,
}

/// A wrapper type to remove repeated Result<T, Error> returns.
pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
