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
    /// This is a getter for the partial-rebuild toggle value.
    fn partial_rebuild_is_enabled(&self) -> bool;
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

    fn partial_rebuild_is_enabled(&self) -> bool {
        self.core.partial_rebuild_is_enabled()
    }
}

/// This is used to deserialize the values.yaml of the Core chart.
#[derive(Deserialize)]
pub(crate) struct CoreValues {
    /// This contains values for all the agents.
    agents: Agents,
    /// This contains values for all the base components.
    base: Base,
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
    /// The contains the values for the jaegertracing/jaeger-operator chart.
    #[serde(rename(deserialize = "jaeger-operator"))]
    jaeger_operator: JaegerOperator,
    /// This contains loki-stack details.
    #[serde(rename(deserialize = "loki-stack"))]
    loki_stack: LokiStack,
    /// This contains the sub-chart values for the hostpath provisioner's helm chart.
    #[serde(default, rename(deserialize = "localpv-provisioner"))]
    localpv_provisioner: LocalpvProvisioner,
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

    fn partial_rebuild_is_enabled(&self) -> bool {
        self.agents.partial_rebuild_is_enabled()
    }
}

impl CoreValues {
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

    /// This is a getter for the sig-storage/csi-resizer image tag.
    pub(crate) fn csi_resizer_image_tag(&self) -> &str {
        self.csi.resizer_image_tag()
    }

    /// This is a getter for the CSI node's NVMe io_timeout.
    pub(crate) fn csi_node_nvme_io_timeout(&self) -> &str {
        self.csi.node_nvme_io_timeout()
    }

    /// This returns the value of the removed key for CSI socket mount path.
    pub(crate) fn deprecated_node_csi_mount_path(&self) -> &str {
        self.csi.deprecated_node_csi_mount_path()
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

    /// This is a getter for the localpv-provisioner sub-chart's release version.
    pub(crate) fn localpv_release_version(&self) -> &str {
        self.localpv_provisioner.release_version()
    }

    /// This is a getter for the container image tag of the hostpath localpv provisioner.
    pub(crate) fn localpv_provisioner_image_tag(&self) -> &str {
        self.localpv_provisioner.provisioner_image_tag()
    }

    /// This is a getter for the image tag of the localpv helper container.
    pub(crate) fn localpv_helper_image_tag(&self) -> &str {
        self.localpv_provisioner.helper_image_tag()
    }

    /// This is a getter for the prometheus/alertmanager container's image tag.
    pub(crate) fn prometheus_alertmanager_image_tag(&self) -> &str {
        self.loki_stack.prometheus_alertmanager_image_tag()
    }

    /// This is a getter for the prometheus/node-exporter container's image tag.
    pub(crate) fn prometheus_node_exporter_image_tag(&self) -> &str {
        self.loki_stack.prometheus_node_exporter_image_tag()
    }

    /// This is a getter for the prom/pushgateway container's image tag.
    pub(crate) fn prometheus_pushgateway_image_tag(&self) -> &str {
        self.loki_stack.prometheus_pushgateway_image_tag()
    }

    /// This is a getter for the prometheus/prometheus container's image tag.
    pub(crate) fn prometheus_server_image_tag(&self) -> &str {
        self.loki_stack.prometheus_server_image_tag()
    }

    /// This is the value of the deprecated key for log silence configuration.
    pub(crate) fn deprecated_log_silence_level(&self) -> &str {
        self.base.deprecated_log_silence_level()
    }

    pub(crate) fn jaeger_operator_image_tag(&self) -> &str {
        self.jaeger_operator.image_tag()
    }
}

/// This is used to deserialize the yaml object agents.
#[derive(Deserialize)]
struct Agents {
    core: Core,
    ha: Ha,
}

impl Agents {
    /// This is a getter for state of the 'ha' feature (enabled/disabled).
    fn ha_is_enabled(&self) -> bool {
        self.ha.enabled()
    }

    fn partial_rebuild_is_enabled(&self) -> bool {
        self.core.partial_rebuild_is_enabled()
    }
}

/// This is used to deserialize the yaml object base.
#[derive(Deserialize)]
struct Base {
    #[serde(default, rename(deserialize = "logSilenceLevel"))]
    deprecated_log_silence_level: String,
}

impl Base {
    /// This returns the value for the removed base.logSilenceLevel YAML key.
    fn deprecated_log_silence_level(&self) -> &str {
        self.deprecated_log_silence_level.as_str()
    }
}

/// This is used to deserialize the yaml object 'agents.core'.
#[derive(Deserialize)]
struct Core {
    #[serde(default)]
    rebuild: Rebuild,
}

impl Core {
    fn partial_rebuild_is_enabled(&self) -> bool {
        self.rebuild.partial_is_enabled()
    }
}

/// This is used to deserialize the yaml object 'agents.core.rebuild'.
#[derive(Default, Deserialize)]
struct Rebuild {
    partial: RebuildPartial,
}

impl Rebuild {
    fn partial_is_enabled(&self) -> bool {
        self.partial.enabled()
    }
}

/// This is used to deserialize the yaml object 'agents.core.rebuild.partial'.
#[derive(Deserialize)]
struct RebuildPartial {
    enabled: bool,
}

impl Default for RebuildPartial {
    /// We've never shipped with partial rebuild set to off. Also for a good while after
    /// the feature was introduced, it was enabled without an option to disable it. So
    /// assuming that partial rebuild is enabled, if the YAML object for Rebuild is missing.
    /// The Rebuild type will be deserialized with a default value if it's absent from the
    /// helm values.
    ///
    ///     #[serde(default)]
    ///     rebuild: Rebuild,
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl RebuildPartial {
    fn enabled(&self) -> bool {
        self.enabled
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

    /// This is a getter for the sig-storage/csi-resizer image tag.
    fn resizer_image_tag(&self) -> &str {
        self.image.resizer_tag()
    }

    /// This is a getter for the CSI node NVMe io_timeout.
    fn node_nvme_io_timeout(&self) -> &str {
        self.node.nvme_io_timeout()
    }

    /// This returns the mount path value's key, the old one with the typo.
    fn deprecated_node_csi_mount_path(&self) -> &str {
        self.node.deprecated_plugin_mount_path()
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
    /// This is the image tag for the csi-resizer container.
    #[serde(default)]
    resizer_tag: String,
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

    /// This is a getter for resizerTag.
    fn resizer_tag(&self) -> &str {
        self.resizer_tag.as_str()
    }
}

/// This is used to deserialize the yaml object 'csi.node'.
#[derive(Deserialize)]
struct CsiNode {
    nvme: CsiNodeNvme,
    #[serde(default, rename(deserialize = "pluginMounthPath"))]
    deprecated_plugin_mount_path: String,
}

impl CsiNode {
    /// This is a getter for the NVMe IO timeout.
    fn nvme_io_timeout(&self) -> &str {
        self.nvme.io_timeout()
    }

    /// This returns the csi node mount path key's value. The key had a typo, it's been removed.
    fn deprecated_plugin_mount_path(&self) -> &str {
        self.deprecated_plugin_mount_path.as_str()
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
    prometheus: Prometheus,
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

    /// This is a getter for the prometheus/alertmanager container's image tag.
    fn prometheus_alertmanager_image_tag(&self) -> &str {
        self.prometheus.alertmanager_image_tag()
    }

    /// This is a getter for the prometheus/node-exporter container's image tag.
    fn prometheus_node_exporter_image_tag(&self) -> &str {
        self.prometheus.node_exporter_image_tag()
    }

    /// This is a getter for the prom/pushgateway container's image tag.
    fn prometheus_pushgateway_image_tag(&self) -> &str {
        self.prometheus.pushgateway_image_tag()
    }

    /// This is a getter for the prometheus/prometheus container's image tag.
    fn prometheus_server_image_tag(&self) -> &str {
        self.prometheus.server_image_tag()
    }
}

/// This is used to deserialize the YAML object 'loki-stack.filebeat'.
#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
struct Filebeat {
    #[serde(default)]
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
    #[serde(default)]
    download_dashboards_image: GrafanaDownloadDashboardsImage,
    image: GenericImage,
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
#[derive(Default, Deserialize)]
struct GrafanaDownloadDashboardsImage {
    #[serde(default)]
    tag: String,
}

impl GrafanaDownloadDashboardsImage {
    /// This is a getter for the curlimages/curl container image on the grafana chart.
    fn tag(&self) -> &str {
        self.tag.as_str()
    }
}

/// This is used to deserialize the YAML object 'loki-stack.grafana.sidecar'.
#[derive(Deserialize)]
struct GrafanaSidecar {
    #[serde(default)]
    image: GenericImage,
}

impl GrafanaSidecar {
    /// This is a getter for the kiwigrid/k8s-sidecar sidecar container image tag.
    fn image_tag(&self) -> &str {
        self.image.tag()
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
    image: GenericImage,
}

impl Loki {
    fn image_tag(&self) -> &str {
        self.image.tag()
    }
}

/// This is used to deserialize the yaml object 'loki-stack.prometheus'.
#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
struct Prometheus {
    #[serde(default)]
    alertmanager: PrometheusAlertmanager,
    #[serde(default)]
    node_exporter: PrometheusNodeExporter,
    #[serde(default)]
    pushgateway: PrometheusPushgateway,
    #[serde(default)]
    server: PrometheusServer,
}

impl Prometheus {
    /// Returns the image tag of the alertmanager container.
    fn alertmanager_image_tag(&self) -> &str {
        self.alertmanager.image_tag()
    }

    /// Returns the image tag of the nodeExporter container.
    fn node_exporter_image_tag(&self) -> &str {
        self.node_exporter.image_tag()
    }

    /// Returns the pushgateway container's image tag.
    fn pushgateway_image_tag(&self) -> &str {
        self.pushgateway.image_tag()
    }

    /// Returns the prometheus server's container image tag.
    fn server_image_tag(&self) -> &str {
        self.server.image_tag()
    }
}

/// This is used to deserialize the prometheus chart's alertmanager YAML object.
#[derive(Default, Deserialize)]
struct PrometheusAlertmanager {
    #[serde(default)]
    image: GenericImage,
}

impl PrometheusAlertmanager {
    fn image_tag(&self) -> &str {
        self.image.tag()
    }
}

/// This is used to deserialize the prometheus chart's nodeExporter YAML object.
#[derive(Default, Deserialize)]
struct PrometheusNodeExporter {
    #[serde(default)]
    image: GenericImage,
}

impl PrometheusNodeExporter {
    fn image_tag(&self) -> &str {
        self.image.tag()
    }
}

/// This is used to deserialize the prometheus chart's pushgateway YAML object.
#[derive(Default, Deserialize)]
struct PrometheusPushgateway {
    #[serde(default)]
    image: GenericImage,
}

impl PrometheusPushgateway {
    fn image_tag(&self) -> &str {
        self.image.tag()
    }
}

/// This is used to deserialize the prometheus chart's server YAML object.
#[derive(Default, Deserialize)]
struct PrometheusServer {
    #[serde(default)]
    image: GenericImage,
}

impl PrometheusServer {
    fn image_tag(&self) -> &str {
        self.image.tag()
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

/// This is used to deserialize the helm values of the localpv-provisioner helm chart.
#[derive(Default, Deserialize)]
#[serde(default, rename_all(deserialize = "camelCase"))]
struct LocalpvProvisioner {
    release: LocalpvProvisionerRelease,
    localpv: LocalpvProvisionerLocalpv,
    helper_pod: LocalpvProvisionerHelperPod,
}

impl LocalpvProvisioner {
    /// This is a getter for the localpv-provisioner helm chart's release version.
    fn release_version(&self) -> &str {
        self.release.version()
    }

    /// This is a getter for the container image tag of the provisioner-localpv container.
    fn provisioner_image_tag(&self) -> &str {
        self.localpv.image_tag()
    }

    /// This is a getter for the linux-utils helper container's image tag.
    fn helper_image_tag(&self) -> &str {
        self.helper_pod.image_tag()
    }
}

/// This is used to deserialize the 'release.version' yaml object in the localpv-provisioner helm
/// chart.
#[derive(Default, Deserialize)]
struct LocalpvProvisionerRelease {
    #[serde(default)]
    version: String,
}

impl LocalpvProvisionerRelease {
    /// This is a getter for the release version for the localpv-provisioner helm chart.
    /// This value is set as the value of the 'openebs.io/version' label.
    fn version(&self) -> &str {
        self.version.as_str()
    }
}

/// This is used to deserialize the 'localpv' yaml object in the localpv-provisioner helm chart.
#[derive(Default, Deserialize)]
struct LocalpvProvisionerLocalpv {
    #[serde(default)]
    image: GenericImage,
}

impl LocalpvProvisionerLocalpv {
    /// This is getter for the openebs/provisioner-localpv container's image tag.
    fn image_tag(&self) -> &str {
        self.image.tag()
    }
}

/// This is used to deserialize various 'image' yaml objects in the localpv-provisioner helm
/// chart.
#[derive(Default, Deserialize)]
struct GenericImage {
    #[serde(default)]
    tag: String,
}

impl GenericImage {
    /// This is getter for the various container image tags in the localpv-provisioner helm chart.
    fn tag(&self) -> &str {
        self.tag.as_str()
    }
}

/// This is used to deserialize the 'helperPod' yaml object in the localpv-provisioner helm chart.
#[derive(Default, Deserialize)]
struct LocalpvProvisionerHelperPod {
    #[serde(default)]
    image: GenericImage,
}

impl LocalpvProvisionerHelperPod {
    /// This is getter for the openebs/linux-utils helper pod container's image tag.
    fn image_tag(&self) -> &str {
        self.image.tag()
    }
}

/// This is used to deserialize the '.jaeger-operator' yaml object.
#[derive(Deserialize)]
struct JaegerOperator {
    #[serde(default)]
    image: GenericImage,
}

impl JaegerOperator {
    /// This returns the image tag of the jaeger-operator from the jaeger-operator helm chart.
    fn image_tag(&self) -> &str {
        self.image.tag()
    }
}
