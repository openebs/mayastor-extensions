/// Label for release name.
pub(crate) const HELM_RELEASE_NAME_LABEL: &str = "openebs.io/release";

/// Defines the default helm chart release name.
pub(crate) const DEFAULT_RELEASE_NAME: &str = "mayastor";

/// Defines the Label select for mayastor REST API.
pub(crate) const API_REST_LABEL_SELECTOR: &str = "app=api-rest";

/// Defines the Label key for event store.
pub(crate) const EVENT_STORE_LABLE_KEY: &str = "app";

/// Defines the suffix name for event store .
pub(crate) const EVENT_STORE: &str = "event-store";

/// Defines the key for comfig map.
pub(crate) const EVENT_STATS_DATA: &str = "stats";

/// Field manager for Patch param, required for [`Patch::Apply`].
pub(crate) const PATCH_PARAM_FILED_MANAGER: &str = "events_store_configmap";
