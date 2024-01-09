use crate::common::error::{ReadingFile, U8VectorToString, YamlParseFromFile, YamlParseFromSlice};
use k8s_openapi::api::core::v1::{Container, Probe};
use semver::Version;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::{fs::read, path::Path, str};

/// This struct is used to deserialize helm charts' Chart.yaml file.
#[derive(Deserialize)]
pub(crate) struct Chart {
    /// This is the name of the helm chart.
    name: String,
    /// This is the version of the helm chart.
    version: Version,
}

impl Chart {
    /// This is a getter for the helm chart name.
    pub(crate) fn name(&self) -> &str {
        self.name.as_str()
    }

    /// This is a getter for the helm chart version.
    pub(crate) fn version(&self) -> &Version {
        &self.version
    }
}

/// This is a set of tools for types whose instances are created
/// by deserializing a helm chart's values.yaml files.
pub(crate) trait HelmValuesCollection {
    /// This is a getter for state of the 'ha' feature (enabled/disabled).
    fn ha_is_enabled(&self) -> bool;
}

/// UmbrellaValues is used to deserialize the helm values.yaml for the Umbrella chart. The Core
/// chart is a sub-chart for the Umbrella chart, so the Core chart values structure is embedded
/// into the UmbrellaValues structure.
#[derive(Deserialize)]
pub(crate) struct UmbrellaValues {
    #[serde(rename(deserialize = "mayastor"))]
    core: CoreValues,
}

impl TryFrom<&[u8]> for UmbrellaValues {
    type Error = crate::common::error::Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        serde_yaml::from_slice(buf).context(YamlParseFromSlice {
            input_yaml: str::from_utf8(buf).context(U8VectorToString)?.to_string(),
        })
    }
}

impl HelmValuesCollection for UmbrellaValues {
    fn ha_is_enabled(&self) -> bool {
        self.core.ha_is_enabled()
    }
}

/// This is used to deserialize the values.yaml of the Core chart.
#[derive(Deserialize)]
pub(crate) struct CoreValues {
    /// This contains values for all of the agents.
    agents: Agents,
    /// This is the yaml object which contains values for the container image registry, repository,
    /// tag, etc.
    image: Image,
    /// This is the yaml object which contains the configuration for the io-engine DaemonSet.
    io_engine: IoEngine,
    /// This toggles installation of eventing components.
    #[serde(default)]
    eventing: Eventing,
    /// This contains Kubernetes CSI sidecar container image details.
    csi: Csi,
    /// This contains loki-stack details.
    #[serde(rename(deserialize = "loki-stack"))]
    loki_stack: LokiStack,
}

impl TryFrom<&Path> for CoreValues {
    type Error = crate::common::error::Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let buf = read(path).context(ReadingFile {
            filepath: path.to_path_buf(),
        })?;

        serde_yaml::from_slice(buf.as_slice()).context(YamlParseFromFile {
            filepath: path.to_path_buf(),
        })
    }
}

impl TryFrom<&[u8]> for CoreValues {
    type Error = crate::common::error::Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        serde_yaml::from_slice(buf).context(YamlParseFromSlice {
            input_yaml: str::from_utf8(buf).context(U8VectorToString)?.to_string(),
        })
    }
}

impl HelmValuesCollection for CoreValues {
    fn ha_is_enabled(&self) -> bool {
        self.agents.ha_is_enabled()
    }
}

impl CoreValues {
    /// This is a getter for state of the 'ha' feature (enabled/disabled).
    pub(crate) fn ha_is_enabled(&self) -> bool {
        self.agents.ha_is_enabled()
    }

    /// This is a getter for the container image tag of the Core chart.
    pub(crate) fn image_tag(&self) -> &str {
        self.image.tag()
    }

    /// This is a getter for the control-plane repoTag image tag set on a helm chart.
    pub(crate) fn control_plane_repotag(&self) -> &str {
        self.image.control_plane_repotag()
    }

    /// This is a getter for the data-plane repoTag image tag set on a helm chart.
    pub(crate) fn data_plane_repotag(&self) -> &str {
        self.image.data_plane_repotag()
    }

    /// This is a getter for the extensions repoTag image tag set on a helm chart.
    pub(crate) fn extensions_repotag(&self) -> &str {
        self.image.extensions_repotag()
    }

    /// This is a getter for the io-engine DaemonSet Pods' logLevel.
    pub(crate) fn io_engine_log_level(&self) -> &str {
        self.io_engine.log_level()
    }

    /// This is a getter for the eventing installation enable/disable state.
    pub(crate) fn eventing_enabled(&self) -> bool {
        self.eventing.enabled()
    }

    /// This is a getter for the sig-storage/csi-provisioner image tag.
    pub(crate) fn csi_provisioner_image_tag(&self) -> &str {
        self.csi.provisioner_image_tag()
    }

    /// This is a getter for the sig-storage/csi-attacher image tag.
    pub(crate) fn csi_attacher_image_tag(&self) -> &str {
        self.csi.attacher_image_tag()
    }

    /// This is a getter for the sig-storage/csi-snapshotter image tag.
    pub(crate) fn csi_snapshotter_image_tag(&self) -> &str {
        self.csi.snapshotter_image_tag()
    }

    /// This is a getter for the sig-storage/snapshot-controller image tag.
    pub(crate) fn csi_snapshot_controller_image_tag(&self) -> &str {
        self.csi.snapshot_controller_image_tag()
    }

    /// This is a getter for the sig-storage/csi-node-driver-registrar image tag.
    pub(crate) fn csi_node_driver_registrar_image_tag(&self) -> &str {
        self.csi.node_driver_registrar_image_tag()
    }

    /// This is a getter for the CSI node's NVMe io_timeout.
    pub(crate) fn csi_node_nvme_io_timeout(&self) -> &str {
        self.csi.node_nvme_io_timeout()
    }

    /// This is a getter for the grafana/loki container image tag.
    pub(crate) fn loki_stack_loki_image_tag(&self) -> &str {
        self.loki_stack.loki_image_tag()
    }

    /// This is a getter for the promtail scrapeConfigs.
    pub(crate) fn loki_stack_promtail_scrape_configs(&self) -> &str {
        self.loki_stack.promtail_scrape_configs()
    }

    /// This returns the value of 'promtail.config.file'.
    pub(crate) fn loki_stack_promtail_config_file(&self) -> &str {
        self.loki_stack.promtail_config_file()
    }

    /// This returns the value of the deprecated promtail helm chart field 'config.lokiAddress'.
    pub(crate) fn loki_stack_promtail_loki_address(&self) -> &str {
        self.loki_stack.deprecated_promtail_loki_address()
    }

    /// This returns the config.snippets.extraClientConfigs from the promtail helm chart v3.11.0.
    pub(crate) fn promtail_extra_client_configs(&self) -> &str {
        self.loki_stack.deprecated_promtail_extra_client_configs()
    }

    /// This returns the initContainers array from the promtail chart v6.13.1.
    pub(crate) fn promtail_init_container(&self) -> Vec<Container> {
        self.loki_stack.promtail_init_container()
    }

    /// This returns the readinessProbe HTTP Get path from the promtail chart v6.13.1.
    pub(crate) fn promtail_readiness_probe_http_get_path(&self) -> String {
        self.loki_stack.promtail_readiness_probe_http_get_path()
    }

    /// This returns the image tag from the filebeat helm chart. Filebeat is a part of the
    /// loki-stack chart.
    pub(crate) fn filebeat_image_tag(&self) -> &str {
        self.loki_stack.filebeat_image_tag()
    }

    /// This returns the image tag from the logstash helm chart. Logstash is a part of the
    /// loki-stack chart.
    pub(crate) fn logstash_image_tag(&self) -> &str {
        self.loki_stack.logstash_image_tag()
    }

    /// This returns the image tag for the curlimages/curl container.
    pub(crate) fn grafana_download_dashboards_image_tag(&self) -> &str {
        self.loki_stack.grafana_download_dashboards_image_tag()
    }

    /// This returns the image tag for the grafana/grafana container.
    pub(crate) fn grafana_image_tag(&self) -> &str {
        self.loki_stack.grafana_image_tag()
    }

    /// This returns the image tag for the kiwigrid/k8s-sidecar container.
    pub(crate) fn grafana_sidecar_image_tag(&self) -> &str {
        self.loki_stack.grafana_sidecar_image_tag()
    }
}

/// This is used to deserialize the yaml object agents.
#[derive(Deserialize)]
struct Agents {
    ha: Ha,
}

impl Agents {
    /// This is a getter for state of the 'ha' feature (enabled/disabled).
    fn ha_is_enabled(&self) -> bool {
        self.ha.enabled()
    }
}

/// This is used to deserialize the yaml object 'agents.ha'.
#[derive(Deserialize)]
struct Ha {
    enabled: bool,
}

impl Ha {
    /// This returns the value of 'ha.enabled' from the values set. Defaults to 'true' is absent
    /// from the yaml.
    fn enabled(&self) -> bool {
        self.enabled
    }
}

/// This is used to deserialize the yaml object "image", which contains details required for pulling
/// container images.
#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
struct Image {
    /// The container image tag.
    tag: String,
    /// This contains image tags set based on which PRODUCT repository the microservice originates
    /// from.
    #[serde(default)]
    repo_tags: RepoTags,
}

impl Image {
    /// This is a getter for the container image tag used across the helm chart release.
    fn tag(&self) -> &str {
        self.tag.as_str()
    }

    /// This is a getter for the control-plane repoTag set on a helm chart.
    fn control_plane_repotag(&self) -> &str {
        self.repo_tags.control_plane()
    }

    /// This is a getter for the data-plane repoTag set on a helm chart.
    fn data_plane_repotag(&self) -> &str {
        self.repo_tags.data_plane()
    }

    /// This is a getter for the extensions repoTag set on a helm chart.
    fn extensions_repotag(&self) -> &str {
        self.repo_tags.extensions()
    }
}

/// This contains image tags for PRODUCT components based on the repository for the specific
/// component.
#[derive(Deserialize, Default)]
#[serde(rename_all(deserialize = "camelCase"))]
struct RepoTags {
    /// This member of repoTags is used to set image tags for components from the control-plane
    /// repo.
    control_plane: String,
    /// This member of repoTags is used to set image tags for components from the data-plane repo.
    data_plane: String,
    /// This member of repoTags is used to set image tags for components from the extensions repo.
    extensions: String,
}

impl RepoTags {
    /// This is a getter for the control-plane image tag set on a helm chart.
    fn control_plane(&self) -> &str {
        self.control_plane.as_str()
    }

    /// This is a getter for the data-plane image tag set on a helm chart.
    fn data_plane(&self) -> &str {
        self.data_plane.as_str()
    }

    /// This is a getter for the extensions image tag set on a helm chart.
    fn extensions(&self) -> &str {
        self.extensions.as_str()
    }
}

/// This is used to deserialize the yaml object "io_engine", which contains configuration for the
/// io-engine DaemonSet.
#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
struct IoEngine {
    /// Tracing Loglevel details for the io-engine DaemonSet Pods.
    log_level: String,
}

impl IoEngine {
    /// This is a getter for the io-engine DaemonSet Pod's tracing logLevel.
    fn log_level(&self) -> &str {
        self.log_level.as_str()
    }
}

/// This is used to deserialize the yaml object 'eventing', v2.3.0 has it disabled by default,
/// the default thereafter has it enabled.
#[derive(Deserialize, Default)]
struct Eventing {
    // This value is defaulted to 'false' when 'Eventing' is absent in the yaml.
    // This works fine because we don't use the serde deserialized values during
    // the values.yaml merge. The merge is done with 'yq'. These are assumed values,
    // in case the value is absent (usually due to added features). This is used
    // to compare against new values (those bundled with the chart in the upgrade-job's
    // local filesystem) and decide if a yq 'set' is required. This default is not a
    // fallback value that is set in case the user's value's yaml is missing the value.
    /// This enables eventing and components when enabled.
    enabled: bool,
}

impl Eventing {
    /// This is a predicate for the installation setting for eventing.
    fn enabled(&self) -> bool {
        self.enabled
    }
}

/// This is used to deserialize the yaml object 'csi'.
#[derive(Deserialize)]
struct Csi {
    /// This contains the image tags for the kubernetes-csi sidecar containers.
    image: CsiImage,
    /// This contains configuration for the CSI node.
    node: CsiNode,
}

impl Csi {
    /// This is a getter for the sig-storage/csi-provisioner image tag.
    fn provisioner_image_tag(&self) -> &str {
        self.image.provisioner_tag()
    }

    /// This is a getter for the sig-storage/csi-attacher image tag.
    fn attacher_image_tag(&self) -> &str {
        self.image.attacher_tag()
    }

    /// This is a getter for the sig-storage/csi-snapshotter image tag.
    fn snapshotter_image_tag(&self) -> &str {
        self.image.snapshotter_tag()
    }

    /// This is a getter for the sig-storage/snapshot-controller image tag.
    fn snapshot_controller_image_tag(&self) -> &str {
        self.image.snapshot_controller_tag()
    }

    /// This is a getter for the sig-storage/csi-node-driver-registrar image tag.
    fn node_driver_registrar_image_tag(&self) -> &str {
        self.image.node_driver_registrar_tag()
    }

    /// This is a getter for the CSI node NVMe io_timeout.
    fn node_nvme_io_timeout(&self) -> &str {
        self.node.nvme_io_timeout()
    }
}

/// This contains the image tags for the CSI sidecar containers.
#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
struct CsiImage {
    /// This is the image tag for the csi-provisioner container.
    provisioner_tag: String,
    /// This is the image tag for the csi-attacher container.
    attacher_tag: String,
    /// This is the image tag for the csi-snapshotter container.
    #[serde(default)]
    snapshotter_tag: String,
    /// This is the image tag for the snapshot-controller container.
    #[serde(default)]
    snapshot_controller_tag: String,
    /// This is the image tag for the csi-node-driver-registrar container.
    registrar_tag: String,
}

impl CsiImage {
    /// This is a getter for provisionerTag.
    fn provisioner_tag(&self) -> &str {
        self.provisioner_tag.as_str()
    }

    /// This is a getter for attacherTag.
    fn attacher_tag(&self) -> &str {
        self.attacher_tag.as_str()
    }

    /// This is a getter for snapshotterTag.
    fn snapshotter_tag(&self) -> &str {
        self.snapshotter_tag.as_str()
    }

    /// This is a getter for snapshotControllerTag.
    fn snapshot_controller_tag(&self) -> &str {
        self.snapshot_controller_tag.as_str()
    }

    /// This is a getter for registrarTag.
    fn node_driver_registrar_tag(&self) -> &str {
        self.registrar_tag.as_str()
    }
}

/// This is used to deserialize the yaml object 'csi.node'.
#[derive(Deserialize)]
struct CsiNode {
    nvme: CsiNodeNvme,
}

impl CsiNode {
    /// This is a getter for the NVMe IO timeout.
    fn nvme_io_timeout(&self) -> &str {
        self.nvme.io_timeout()
    }
}

/// This is used to deserialize the yaml object 'csi.node.nvme'.
#[derive(Deserialize)]
struct CsiNodeNvme {
    io_timeout: String,
}

impl CsiNodeNvme {
    /// This is a getter for the IO timeout configuration.
    fn io_timeout(&self) -> &str {
        self.io_timeout.as_str()
    }
}

/// This is used to deserialize the yaml object 'loki-stack'.
#[derive(Deserialize)]
struct LokiStack {
    filebeat: Filebeat,
    grafana: Grafana,
    logstash: Logstash,
    loki: Loki,
    promtail: Promtail,
}

impl LokiStack {
    /// This is a getter for the promtail scrapeConfigs value.
    fn promtail_scrape_configs(&self) -> &str {
        self.promtail.scrape_configs()
    }

    /// This is a getter for the value of 'promtail.config.file'.
    fn promtail_config_file(&self) -> &str {
        self.promtail.config_file()
    }

    /// This returns the config.snippets.extraClientConfigs from the promtail helm chart v3.11.0.
    fn deprecated_promtail_extra_client_configs(&self) -> &str {
        self.promtail.deprecated_extra_client_configs()
    }

    /// This is a getter for the deprecated 'lokiAddress' field in the promtail helm chart v3.11.0.
    fn deprecated_promtail_loki_address(&self) -> &str {
        self.promtail.deprecated_loki_address()
    }

    /// This is a getter for the loki/loki container's image tag.
    fn loki_image_tag(&self) -> &str {
        self.loki.image_tag()
    }

    /// This is a getter for the initContainer array from promtail chart v6.13.1.
    fn promtail_init_container(&self) -> Vec<Container> {
        self.promtail.init_container()
    }

    /// This is a getter for the readinessProbe HTTP GET path from promtail chart v6.13.1.
    fn promtail_readiness_probe_http_get_path(&self) -> String {
        self.promtail.readiness_probe_http_get_path()
    }

    /// This is a getter for the filebeat image tag from the loki-stack helm chart.
    fn filebeat_image_tag(&self) -> &str {
        self.filebeat.image_tag()
    }

    /// This is a getter for the logstash image tag from the loki-stack helm chart.
    fn logstash_image_tag(&self) -> &str {
        self.logstash.image_tag()
    }

    /// This is a getter for the curlimages/curl container's image tag from the grafana chart.
    fn grafana_download_dashboards_image_tag(&self) -> &str {
        self.grafana.download_dashboards_image_tag()
    }

    /// This is a getter for the grafana/grafana container's image tag from the grafana chart.
    fn grafana_image_tag(&self) -> &str {
        self.grafana.image_tag()
    }

    /// This is a getter for the kiwigrid/k8s-sidecar container's image tag from the grafana chart.
    fn grafana_sidecar_image_tag(&self) -> &str {
        self.grafana.sidecar_image_tag()
    }
}

/// This is used to deserialize the YAML object 'loki-stack.filebeat'.
#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
struct Filebeat {
    image_tag: String,
}

impl Filebeat {
    /// This is a getter for the Filebeat image tag.
    fn image_tag(&self) -> &str {
        self.image_tag.as_str()
    }
}

/// This is used to deserialize the YAML object 'loki-stack.grafana'.
#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
struct Grafana {
    download_dashboards_image: GrafanaDownloadDashboardsImage,
    image: GrafanaImage,
    sidecar: GrafanaSidecar,
}

impl Grafana {
    /// This is getter for the curlimages/curl container image tag.
    fn download_dashboards_image_tag(&self) -> &str {
        self.download_dashboards_image.tag()
    }

    /// This is getter for the grafana/grafana container image tag.
    fn image_tag(&self) -> &str {
        self.image.tag()
    }

    /// This is a getter for the kiwigrid/k8s-sidecar sidecar container image tag.
    fn sidecar_image_tag(&self) -> &str {
        self.sidecar.image_tag()
    }
}

/// This is used to deserialize the YAML object 'loki-stack.grafana.downloadDashboardsImage'.
#[derive(Deserialize)]
struct GrafanaDownloadDashboardsImage {
    tag: String,
}

impl GrafanaDownloadDashboardsImage {
    /// This is a getter for the curlimages/curl container image on the grafana chart.
    fn tag(&self) -> &str {
        self.tag.as_str()
    }
}

/// This is used to deserialize the YAML object 'loki-stack.grafana.image'.
#[derive(Deserialize)]
struct GrafanaImage {
    tag: String,
}

impl GrafanaImage {
    /// This is a getter for the grafana/grafana container image on the grafana chart.
    fn tag(&self) -> &str {
        self.tag.as_str()
    }
}

/// This is used to deserialize the YAML object 'loki-stack.grafana.sidecar'.
#[derive(Deserialize)]
struct GrafanaSidecar {
    image: GrafanaSidecarImage,
}

impl GrafanaSidecar {
    /// This is a getter for the kiwigrid/k8s-sidecar sidecar container image tag.
    fn image_tag(&self) -> &str {
        self.image.tag()
    }
}

/// This is used to deserialize the YAML object 'loki-stack.grafana.sidecar.image'.
#[derive(Deserialize)]
struct GrafanaSidecarImage {
    tag: String,
}

impl GrafanaSidecarImage {
    /// This is a getter for the kiwigrid/k8s-sidecar container image on the grafana chart.
    fn tag(&self) -> &str {
        self.tag.as_str()
    }
}

/// This is used to deserialize the YAML object 'loki-stack.logstash'.
#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
struct Logstash {
    image_tag: String,
}

impl Logstash {
    /// This is a getter for the Logstash image tag.
    fn image_tag(&self) -> &str {
        self.image_tag.as_str()
    }
}

/// This is used to deserialize the YAML object 'loki-stack.loki'.
#[derive(Deserialize)]
struct Loki {
    image: LokiImage,
}

impl Loki {
    fn image_tag(&self) -> &str {
        self.image.tag()
    }
}

/// This is used to deserialize the YAML object 'loki-stack.loki.image'.
#[derive(Deserialize)]
struct LokiImage {
    tag: String,
}

impl LokiImage {
    fn tag(&self) -> &str {
        self.tag.as_str()
    }
}

/// This is used to deserialize the yaml object 'promtail'.
#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
struct Promtail {
    config: PromtailConfig,
    init_container: PromtailInitContainer,
    readiness_probe: Probe,
}

impl Promtail {
    /// This returns the promtail.config.snippets.scrapeConfigs as an &str.
    fn scrape_configs(&self) -> &str {
        self.config.scrape_configs()
    }

    /// This returns 'promtail.config.file'.
    fn config_file(&self) -> &str {
        self.config.file()
    }

    /// This returns the config.snippets.extraClientConfigs from the promtail helm chart v3.11.0.
    fn deprecated_extra_client_configs(&self) -> &str {
        self.config.deprecated_extra_client_configs()
    }

    fn deprecated_loki_address(&self) -> &str {
        self.config.deprecated_loki_address()
    }

    fn init_container(&self) -> Vec<Container> {
        match &self.init_container {
            PromtailInitContainer::DeprecatedInitContainer {} => Vec::<Container>::default(),
            PromtailInitContainer::InitContainer(containers) => containers.clone(),
        }
    }

    fn readiness_probe_http_get_path(&self) -> String {
        self.readiness_probe
            .http_get
            .clone()
            .unwrap_or_default()
            .path
            .unwrap_or_default()
    }
}

/// This is used to deserialize the promtail.config yaml object.
#[derive(Deserialize)]
struct PromtailConfig {
    #[serde(default, rename(deserialize = "lokiAddress"))]
    deprecated_loki_address: String,
    file: String,
    snippets: PromtailConfigSnippets,
}

impl PromtailConfig {
    /// This returns the config.snippets.scrapeConfigs as an &str.
    fn scrape_configs(&self) -> &str {
        self.snippets.scrape_configs()
    }

    /// This returns the config.file multi-line literal.
    fn file(&self) -> &str {
        self.file.as_str()
    }

    /// This returns the snippets.extraClientConfigs from the promtail helm chart v3.11.0.
    fn deprecated_extra_client_configs(&self) -> &str {
        self.snippets.deprecated_extra_client_configs()
    }

    /// This is a getter for the lokiAddress in the loki helm chart v2.6.4.
    fn deprecated_loki_address(&self) -> &str {
        self.deprecated_loki_address.as_str()
    }
}

/// This is used to deserialize the config.snippets yaml object.
#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
struct PromtailConfigSnippets {
    #[serde(default, rename(deserialize = "extraClientConfigs"))]
    deprecated_extra_client_configs: String,
    scrape_configs: String,
}

impl PromtailConfigSnippets {
    /// This returns the snippets.scrapeConfigs as an &str.
    fn scrape_configs(&self) -> &str {
        self.scrape_configs.as_str()
    }

    /// This returns the snippets.extraClientConfigs from the promtail helm chart v3.11.0.
    fn deprecated_extra_client_configs(&self) -> &str {
        self.deprecated_extra_client_configs.as_str()
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PromtailInitContainer {
    DeprecatedInitContainer {},
    InitContainer(Vec<Container>),
}

/// This is used to serialize the config.clients yaml object in promtail chart v6.13.1
/// when migrating from promtail v3.11.0 to v6.13.1.
#[derive(Debug, Serialize)]
pub(crate) struct PromtailConfigClient {
    url: String,
}

impl PromtailConfigClient {
    /// Create a new PromtailConfigClient with a url.
    pub(crate) fn with_url<U>(url: U) -> Self
    where
        U: ToString,
    {
        Self {
            url: url.to_string(),
        }
    }
}
