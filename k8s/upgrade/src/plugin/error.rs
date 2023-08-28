use snafu::Snafu;
use std::path::PathBuf;

/// For use with multiple fallible operations which may fail for different reasons, but are
/// defined withing the same scope and must return to the outer scope (calling scope) using
/// the try operator -- '?'.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
#[snafu(context(suffix(false)))]
pub enum Error {
    /// Error for no upgrade event present.
    #[snafu(display("No upgrade event present."))]
    UpgradeEventNotPresent,

    /// Error for no Deployment present.
    #[snafu(display("No deployment present."))]
    NoDeploymentPresent,

    /// No message in upgrade event.
    #[snafu(display("No Message present in event."))]
    MessageInEventNotPresent,

    /// Nodes are in cordoned state.
    #[snafu(display("Nodes are in cordoned state."))]
    NodesInCordonedState,

    /// Single replica volume present in cluster.
    #[snafu(display("Single replica volume present in cluster."))]
    SingleReplicaVolumeErr,

    /// Cluster is rebuilding replica of some volumes.
    #[snafu(display("Cluster is rebuilding replica of some volumes."))]
    VolumeRebuildInProgress,

    /// K8s client error.
    #[snafu(display("K8Client Error: {}", source))]
    K8sClient { source: kube::Error },

    /// Deserialization error for event.
    #[snafu(display("Error in deserializing upgrade event {} Error {}", event, source))]
    EventSerdeDeserialization {
        event: String,
        source: serde_json::Error,
    },

    /// Failed in creating service account.
    #[snafu(display("Service account: {} creation failed Error: {}", name, source))]
    ServiceAccountCreate { name: String, source: kube::Error },

    /// Failed in deletion service account.
    #[snafu(display("Service account: {} deletion failed Error: {}", name, source))]
    ServiceAccountDelete { name: String, source: kube::Error },

    /// Failed in creating cluster role.
    #[snafu(display("Cluster role: {} creation failed Error: {}", name, source))]
    ClusterRoleCreate { name: String, source: kube::Error },

    /// Failed in deletion cluster role.
    #[snafu(display("Cluster role: {} deletion Error: {}", name, source))]
    ClusterRoleDelete { name: String, source: kube::Error },

    /// Failed in deletion cluster role binding.
    #[snafu(display("Cluster role binding: {} deletion failed Error: {}", name, source))]
    ClusterRoleBindingDelete { name: String, source: kube::Error },

    /// Failed in creating cluster role binding.
    #[snafu(display("Cluster role binding: {} creation failed Error: {}", name, source))]
    ClusterRoleBindingCreate { name: String, source: kube::Error },

    /// Failed in creating upgrade job.
    #[snafu(display("Upgrade Job: {} creation failed Error: {}", name, source))]
    UpgradeJobCreate { name: String, source: kube::Error },

    /// Failed in deleting upgrade job.
    #[snafu(display("Upgrade Job: {} deletion failed Error: {}", name, source))]
    UpgradeJobDelete { name: String, source: kube::Error },

    /// Error for when the image format is invalid.
    #[snafu(display("Failed to find a valid image in Deployment."))]
    ReferenceDeploymentInvalidImage,

    /// Error for when the .spec.template.spec.contains[0].image is a None.
    #[snafu(display("Failed to find an image in Deployment."))]
    ReferenceDeploymentNoImage,

    /// Error for when .spec is None for the reference Deployment.
    #[snafu(display("No .spec found for the reference Deployment"))]
    ReferenceDeploymentNoSpec,

    /// Error for when .spec.template.spec is None for the reference Deployment.
    #[snafu(display("No .spec.template.spec found for the reference Deployment"))]
    ReferenceDeploymentNoPodTemplateSpec,

    /// Error for when .spec.template.spec.contains[0] does not exist.
    #[snafu(display("Failed to find the first container of the Deployment."))]
    ReferenceDeploymentNoContainers,

    /// Node spec not present error.
    #[snafu(display("Node spec not present, node: {}", node))]
    NodeSpecNotPresent { node: String },

    /// Error for when the pod.metadata.name is a None.
    #[snafu(display("Pod name not present."))]
    PodNameNotPresent,

    /// Error for when the job.status is a None.
    #[snafu(display("Upgrade Job: {} status not present.", name))]
    UpgradeJobStatusNotPresent { name: String },

    /// Error for when the upgrade job is not present.
    #[snafu(display("Upgrade Job: {} in namespace {} does not exist.", name, namespace))]
    UpgradeJobNotPresent { name: String, namespace: String },

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

    /// Error for when a Kubernetes API request for GET-ing a list of Deployments filtered by
    /// label(s) fails.
    #[snafu(display(
        "Failed to list Deployments with label {} in namespace {}: {}",
        label,
        namespace,
        source
    ))]
    ListDeploymantsWithLabel {
        source: kube::Error,
        label: String,
        namespace: String,
    },

    /// Error for when a Kubernetes API request for GET-ing a list of events filtered by
    /// filed selector fails.
    #[snafu(display("Failed to list Events with field selector {}: {}", field, source))]
    ListEventsWithFieldSelector { source: kube::Error, field: String },

    /// Error for when a Kubernetes API request for Deleting a list of events filtered by
    /// filed selector fails.
    #[snafu(display("Failed to delete Events with field selector {}: {}", field, source))]
    DeleteEventsWithFieldSelector { source: kube::Error, field: String },

    /// Error listing the pvc list.
    #[snafu(display("Failed to list pvc : {}", source))]
    ListPVC { source: kube::Error },

    /// Error listing the volumes.
    #[snafu(display("Failed to list volumes : {}", source))]
    ListVolumes {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
    },

    /// Error when a Get Upgrade job fails.
    #[snafu(display("Failed to get Upgrade Job {}: {}", name, source))]
    GetUpgradeJob { source: kube::Error, name: String },

    /// Error when a Get ServiceAccount fails.
    #[snafu(display("Failed to get service account {}: {}", name, source))]
    GetServiceAccount { source: kube::Error, name: String },

    /// Error when a Get ClusterRole fails.
    #[snafu(display("Failed to get cluster role {}: {}", name, source))]
    GetClusterRole { source: kube::Error, name: String },

    /// Error when a Get CLusterRoleBinding fails.
    #[snafu(display("Failed to get cluster role binding {}: {}", name, source))]
    GetClusterRoleBinding { source: kube::Error, name: String },

    /// Error for when Kubernetes API client generation fails.
    #[snafu(display("Failed to generate kubernetes client: {}", source))]
    K8sClientGeneration { source: kube::Error },

    /// Error for when REST API configuration fails.
    #[snafu(display("Failed to configure REST API client : {:?}", source,))]
    RestClientConfiguration {
        #[snafu(source(false))]
        source: openapi::clients::tower::configuration::Error,
    },

    /// Error for when listing storage nodes fails.
    #[snafu(display("Failed to list Nodes: {}", source))]
    ListStorageNodes {
        source: openapi::tower::client::Error<openapi::models::RestJsonError>,
    },

    /// Openapi configuration error.
    #[snafu(display("openapi configuration Error: {}", source))]
    OpenapiClientConfiguration { source: anyhow::Error },

    /// Error when opening a file.
    #[snafu(display("Failed to open file {}: {}", filepath.display(), source))]
    OpeningFile {
        source: std::io::Error,
        filepath: PathBuf,
    },

    /// Error for when yaml could not be parsed from a file (Reader).
    #[snafu(display("Failed to parse YAML at {}: {}", filepath.display(), source))]
    YamlParseFromFile {
        source: serde_yaml::Error,
        filepath: PathBuf,
    },

    /// Error when reading the entire contents of a file into a string.
    #[snafu(display("Failed to read file at {}: {}", filepath.display(), source))]
    ReadFromFile {
        source: std::io::Error,
        filepath: PathBuf,
    },

    /// Error for when yaml could not be parsed from bytes.
    #[snafu(display("Failed to parse unsupported versions yaml: {}", source))]
    YamlParseBufferForUnsupportedVersion { source: serde_yaml::Error },

    /// Error for failures in generating semver::Value from a &str input.
    #[snafu(display("Failed to parse {} as a valid semver: {}", version_string, source))]
    SemverParse {
        source: semver::Error,
        version_string: String,
    },

    /// Source and target version are same.
    #[snafu(display("Source and target version are same for upgrade."))]
    SourceTargetVersionSame,

    /// Error when source version is not a valid for upgrade.
    #[snafu(display("Not a valid source version for upgrade."))]
    NotAValidSourceForUpgrade,

    /// Error for when the detected upgrade path for PRODUCT is not supported.
    #[snafu(display("The upgrade path is invalid"))]
    InvalidUpgradePath,
}

/// A wrapper type to remove repeated Result<T, Error> returns.
pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

impl From<Error> for i32 {
    fn from(err: Error) -> Self {
        match err {
            Error::YamlParseBufferForUnsupportedVersion { .. } => 401,
            Error::UpgradeEventNotPresent { .. } => 402,
            Error::NoDeploymentPresent { .. } => 403,
            Error::MessageInEventNotPresent { .. } => 404,
            Error::NodesInCordonedState { .. } => 405,
            Error::SingleReplicaVolumeErr { .. } => 406,
            Error::VolumeRebuildInProgress { .. } => 407,
            Error::K8sClient { .. } => 408,
            Error::EventSerdeDeserialization { .. } => 409,
            Error::ServiceAccountCreate { .. } => 410,
            Error::ServiceAccountDelete { .. } => 411,
            Error::ClusterRoleCreate { .. } => 412,
            Error::ClusterRoleDelete { .. } => 413,
            Error::ClusterRoleBindingDelete { .. } => 414,
            Error::ClusterRoleBindingCreate { .. } => 415,
            Error::UpgradeJobCreate { .. } => 416,
            Error::UpgradeJobDelete { .. } => 417,
            Error::ReferenceDeploymentInvalidImage { .. } => 418,
            Error::ReferenceDeploymentNoImage { .. } => 419,
            Error::ReferenceDeploymentNoSpec { .. } => 420,
            Error::ReferenceDeploymentNoPodTemplateSpec { .. } => 421,
            Error::ReferenceDeploymentNoContainers { .. } => 422,
            Error::NodeSpecNotPresent { .. } => 423,
            Error::PodNameNotPresent { .. } => 424,
            Error::UpgradeJobStatusNotPresent { .. } => 425,
            Error::UpgradeJobNotPresent { .. } => 426,
            Error::ListPodsWithLabel { .. } => 427,
            Error::ListDeploymantsWithLabel { .. } => 428,
            Error::ListEventsWithFieldSelector { .. } => 429,
            Error::ListPVC { .. } => 430,
            Error::ListVolumes { .. } => 431,
            Error::GetUpgradeJob { .. } => 432,
            Error::GetServiceAccount { .. } => 433,
            Error::GetClusterRole { .. } => 434,
            Error::GetClusterRoleBinding { .. } => 435,
            Error::K8sClientGeneration { .. } => 436,
            Error::RestClientConfiguration { .. } => 437,
            Error::ListStorageNodes { .. } => 438,
            Error::OpenapiClientConfiguration { .. } => 439,
            Error::OpeningFile { .. } => 440,
            Error::YamlParseFromFile { .. } => 441,
            Error::SemverParse { .. } => 442,
            Error::SourceTargetVersionSame { .. } => 443,
            Error::NotAValidSourceForUpgrade { .. } => 444,
            Error::InvalidUpgradePath { .. } => 445,
            Error::DeleteEventsWithFieldSelector { .. } => 446,
            Error::ReadFromFile { .. } => 447,
        }
    }
}
