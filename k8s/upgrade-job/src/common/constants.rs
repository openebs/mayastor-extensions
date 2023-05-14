/// This is the name of the project that is being upgraded.
pub(crate) const PRODUCT: &str = "Mayastor";

/// This is the name of the Helm chart which included the core chart as a sub-chart.
/// Under the hood, this installs the Core Helm chart (see below).
pub(crate) const UMBRELLA_CHART_NAME: &str = "openebs";

/// This is the name of the Helm chart of this project.
pub(crate) const CORE_CHART_NAME: &str = "mayastor";

/// This is the shared Pod label of the <helm-release>-io-engine DaemonSet.
pub(crate) const IO_ENGINE_LABEL: &str = "app=io-engine";

/// This is the shared Pod label of the <helm-release>-agent-core Deployment.
pub(crate) const AGENT_CORE_LABEL: &str = "app=agent-core";

/// This is the shared label across the helm chart components which carries the chart version.
pub(crate) const CHART_VERSION_LABEL_KEY: &str = "openebs.io/version";

/// This is the label set on a storage API Node resource when a 'Node Drain' is issued.
pub(crate) const DRAIN_FOR_UPGRADE: &str = "mayastor-upgrade";

/// This is the allowed upgrade to-version/to-version-range for the Umbrella chart.
pub(crate) const TO_UMBRELLA_SEMVER: &str = "3.6.0";

/// This version set will be only allowed to upgrade to TO_UMBRELLA_SEMVER above. This range applies
/// to the Umbrella chart.
pub(crate) const FROM_UMBRELLA_SEMVER: &str = ">=3.4.0, <=3.5.0";
