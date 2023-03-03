/// Contains constant values which are used as arguments to functions and in log messages.
pub(crate) mod constants;

/// Contains the error handling tooling.
pub(crate) mod error;

/// Contains tools to create Kubernetes API clients.
pub(crate) mod kube_client;

/// Contains macros.
pub(crate) mod macros;

/// Contains tools to create storage API clients.
pub(crate) mod rest_client;
