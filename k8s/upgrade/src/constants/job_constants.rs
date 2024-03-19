/// This is the name of the project that is being upgraded.
pub const PRODUCT: &str = "Mayastor";

/// This is the name of the Helm chart which included the core chart as a sub-chart.
/// Under the hood, this installs the Core Helm chart (see below).
pub const UMBRELLA_CHART_NAME: &str = "openebs";

/// This is the name of the Helm chart of this project.
pub const CORE_CHART_NAME: &str = "mayastor";

/// This is the shared Pod label of the <helm-release>-io-engine DaemonSet.
pub const IO_ENGINE_LABEL: &str = "app=io-engine";

/// This is the shared Pod label of the <helm-release>-agent-core Deployment.
pub const AGENT_CORE_LABEL: &str = "app=agent-core";

/// This is the shared label across the helm chart components which carries the chart version.
pub const CHART_VERSION_LABEL_KEY: &str = "openebs.io/version";

/// This is the label set on a storage API Node resource when a 'Node Drain' is issued.
pub const DRAIN_FOR_UPGRADE: &str = "mayastor-upgrade";

/// This is the label set on a storage API Node resource when a 'Node Drain' is issued.
pub const CORDON_FOR_ANA_CHECK: &str = "mayastor-upgrade-nvme-ana-check";

/// This is the allowed upgrade to-version/to-version-range for the Umbrella chart.
pub const TO_UMBRELLA_SEMVER: &str = "3.10.0";

/// This is the user docs URL for the Umbrella chart.
pub const UMBRELLA_CHART_UPGRADE_DOCS_URL: &str =
    "https://openebs.io/docs/user-guides/upgrade#mayastor-upgrade";

/// Version value for the earliest possible 2.0 release.
pub const TWO_DOT_O_RC_ONE: &str = "2.0.0-rc.1";

/// Version value for the earliest possible 2.1 release (there were no pre-releases).
pub const TWO_DOT_ONE: &str = "2.1.0";

/// Version value for the earliest possible 2.3 release (there were no pre-releases).
pub const TWO_DOT_THREE: &str = "2.3.0";

/// Version value for the earliest possible 2.4 release (there were no pre-releases).
pub const TWO_DOT_FOUR: &str = "2.4.0";

/// Version value for the earliest possible 2.5 release.
pub const TWO_DOT_FIVE: &str = "2.5.0";

/// Version value for the earliest possible 2.6 release.
pub const TWO_DOT_SIX: &str = "2.6.0";
