use semver::Version;
use serde::Deserialize;

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

/// This is used to deserialize the values.yaml of the Core chart.
#[derive(Deserialize)]
pub(crate) struct CoreValues {
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
}

impl CoreValues {
    /// This is a getter for the container image tag of the Core chart.
    pub(crate) fn image_tag(&self) -> &str {
        self.image.tag()
    }

    /// This is a getter for the io-engine DaemonSet Pods' logLevel.
    pub(crate) fn io_engine_log_level(&self) -> &str {
        self.io_engine.log_level()
    }

    /// This is a getter for the eventing installation enable/disable state.
    pub(crate) fn eventing_enabled(&self) -> bool {
        self.eventing.enabled()
    }

    /// This is a getter or the sig-storage/csi-provisioner image tag.
    pub(crate) fn csi_provisioner_image_tag(&self) -> &str {
        self.csi.provisioner_image_tag()
    }

    /// This is a getter or the sig-storage/csi-attacher image tag.
    pub(crate) fn csi_attacher_image_tag(&self) -> &str {
        self.csi.attacher_image_tag()
    }

    /// This is a getter or the sig-storage/csi-snapshotter image tag.
    pub(crate) fn csi_snapshotter_image_tag(&self) -> &str {
        self.csi.snapshotter_image_tag()
    }

    /// This is a getter or the sig-storage/snapshot-controller image tag.
    pub(crate) fn csi_snapshot_controller_image_tag(&self) -> &str {
        self.csi.snapshot_controller_image_tag()
    }

    /// This is a getter or the sig-storage/csi-node-driver-registrar image tag.
    pub(crate) fn csi_node_driver_registrar_image_tag(&self) -> &str {
        self.csi.node_driver_registrar_image_tag()
    }
}

/// This is used to deserialize the yaml object "image", which contains details required for pulling
/// container images.
#[derive(Deserialize)]
pub(crate) struct Image {
    /// The container image tag.
    tag: String,
}

impl Image {
    /// This is a getter for the container image tag used across the helm chart release.
    pub(crate) fn tag(&self) -> &str {
        self.tag.as_str()
    }
}

/// This is used to deserialize the yaml object "io_engine", which contains configuration for the
/// io-engine DaemonSet.
#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub(crate) struct IoEngine {
    /// Tracing Loglevel details for the io-engine DaemonSet Pods.
    log_level: String,
}

impl IoEngine {
    /// This is a getter for the io-engine DaemonSet Pod's tracing logLevel.
    pub(crate) fn log_level(&self) -> &str {
        self.log_level.as_str()
    }
}

/// This is used to deserialize the yaml object 'eventing', v2.3.0 has it disabled by default,
/// the default thereafter has it enabled.
#[derive(Deserialize, Default)]
pub(crate) struct Eventing {
    // This value is defaulted to 'false' when 'Eventing' is absent in the yaml.
    // This works fine because we don't use the serde deserialized values during
    // the values.yaml merge. The merge is done with 'yq'. These are assumed values,
    // in case the value is absent (usually due to added features). This is used
    // to compare against new values (those bundled with the chart in the upgrade-job's
    // local filesystem) and decide if a yq 'set' is required. This default is not a
    // fallback value that is set in case the user's value's yaml is missing the value.
    enabled: bool,
}

impl Eventing {
    /// This is a predicate for the installation setting for eventing.
    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }
}

/// This is used to deserialize the yaml object 'csi'.
#[derive(Deserialize)]
pub(crate) struct Csi {
    image: CsiImage,
}

impl Csi {
    /// This is a getter for the sig-storage/csi-provisioner image tag.
    pub(crate) fn provisioner_image_tag(&self) -> &str {
        self.image.provisioner_tag()
    }

    /// This is a getter for the sig-storage/csi-attacher image tag.
    pub(crate) fn attacher_image_tag(&self) -> &str {
        self.image.attacher_tag()
    }

    /// This is a getter for the sig-storage/csi-snapshotter image tag.
    pub(crate) fn snapshotter_image_tag(&self) -> &str {
        self.image.snapshotter_tag()
    }

    /// This is a getter for the sig-storage/snapshot-controller image tag.
    pub(crate) fn snapshot_controller_image_tag(&self) -> &str {
        self.image.snapshot_controller_tag()
    }

    /// This is a getter for the sig-storage/csi-node-driver-registrar image tag.
    pub(crate) fn node_driver_registrar_image_tag(&self) -> &str {
        self.image.node_driver_registrar_tag()
    }
}

#[derive(Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub(crate) struct CsiImage {
    provisioner_tag: String,
    attacher_tag: String,
    #[serde(default)]
    snapshotter_tag: String,
    #[serde(default)]
    snapshot_controller_tag: String,
    registrar_tag: String,
}

impl CsiImage {
    /// This is a getter for provisionerTag.
    pub(crate) fn provisioner_tag(&self) -> &str {
        self.provisioner_tag.as_str()
    }

    /// This is a getter for attacherTag.
    pub(crate) fn attacher_tag(&self) -> &str {
        self.attacher_tag.as_str()
    }

    /// This is a getter for snapshotterTag.
    pub(crate) fn snapshotter_tag(&self) -> &str {
        self.snapshotter_tag.as_str()
    }

    /// This is a getter for snapshotControllerTag.
    pub(crate) fn snapshot_controller_tag(&self) -> &str {
        self.snapshot_controller_tag.as_str()
    }

    /// This is a getter for registrarTag.
    pub(crate) fn node_driver_registrar_tag(&self) -> &str {
        self.registrar_tag.as_str()
    }
}
