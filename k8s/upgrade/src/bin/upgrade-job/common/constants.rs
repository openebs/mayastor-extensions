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
pub(crate) const TO_UMBRELLA_SEMVER: &str = "3.9.0";

/// This is the user docs URL for the Umbrella chart.
pub(crate) const UMBRELLA_CHART_UPGRADE_DOCS_URL: &str =
    "https://openebs.io/docs/user-guides/upgrade#mayastor-upgrade";

/// Version value for the earliest possible 2.0 release.
pub(crate) const TWO_DOT_O_RC_ONE: &str = "2.0.0-rc.1";

/// Version value for the earliest possible 2.1 release (there were no pre-releases).
pub(crate) const TWO_DOT_ONE: &str = "2.1.0";

/// Version value for the earliest possible 2.3 release (there were no pre-releases).
pub(crate) const TWO_DOT_THREE: &str = "2.3.0";

/// Version value for the earliest possible 2.4 release (there were no pre-releases).
pub(crate) const TWO_DOT_FOUR: &str = "2.4.0";
