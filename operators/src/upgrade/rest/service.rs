use crate::upgrade::{
    common::error::{Error, RestError},
    config::UpgradeOperatorConfig,
    controller::reconciler::upgrade_controller,
    k8s::crd::v0::UpgradePhase,
};
use actix_web::{
    body::BoxBody,
    get,
    http::{header::ContentType, StatusCode},
    put, HttpRequest, HttpResponse, Responder, ResponseError,
};
use kube::ResourceExt;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};
use tracing::{error, info};

/// Upgrade to be returned for get calls.
#[derive(Serialize, Deserialize, Default)]
pub(crate) struct Upgrade {
    name: String,
    current_version: String,
    target_version: String,
    components_state: HashMap<String, HashMap<String, UpgradePhase>>,
    state: String,
}

impl Upgrade {
    /// This adds a name to the Upgrade instance.
    fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    /// This adds a source version to the Upgrade instance.
    fn with_current_version(mut self, current_version: String) -> Self {
        self.current_version = current_version;
        self
    }

    /// This adds a target version to the Upgrade instance.
    fn with_target_version(mut self, target_version: String) -> Self {
        self.target_version = target_version;
        self
    }

    /// This adds a state to the Upgrade instance.
    fn with_state(mut self, state: String) -> Self {
        self.state = state;
        self
    }
}

impl Responder for Upgrade {
    type Body = BoxBody;

    fn respond_to(self, _req: &HttpRequest) -> HttpResponse<Self::Body> {
        let res_body = serde_json::to_string(&self)
            .map_err(|err| Error::SerdeDeserialization { source: err })
            .unwrap();

        // Create HttpResponse and set Content Type
        HttpResponse::Ok()
            .content_type(ContentType::json())
            .body(res_body)
    }
}

/// Implement ResponseError for RestError.
impl ResponseError for RestError {
    fn status_code(&self) -> StatusCode {
        StatusCode::NOT_FOUND
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        let body = serde_json::to_string(&self)
            .map_err(|err| Error::SerdeDeserialization { source: err })
            .unwrap();
        let res = HttpResponse::new(self.status_code());
        res.set_body(BoxBody::new(body))
    }
}

/// Implement Display for RestError.
impl Display for RestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Put request for upgrade.
#[put("/upgrade")]
pub async fn apply_upgrade() -> Result<HttpResponse, RestError> {
    match UpgradeOperatorConfig::get_config()
        .k8s_client()
        .create_upgrade_action_crd()
        .await
    {
        Ok(()) => {
            info!("UpgradeAction CRD created");
        }
        Err(error) => {
            error!(?error, "failed to create UpgradeAction CRD");
            let err =
                RestError::default().with_error("unable to create UpgradeAction crd".to_string());
            return Err(err);
        }
    }

    match UpgradeOperatorConfig::get_config()
        .k8s_client()
        .create_upgrade_action_resource()
        .await
    {
        Ok(u) => {
            info!(
                name = u.metadata.name.as_ref().unwrap(),
                namespace = u.metadata.namespace.as_ref().unwrap(),
                "Created UpgradeAction CustomResource"
            );
            let res = Upgrade::default()
                .with_name(u.name_any())
                .with_current_version(u.spec.current_version().to_string())
                .with_target_version(u.spec.target_version().to_string());
            let res_body = serde_json::to_string(&res).map_err(|error| {
                RestError::default().with_error(format!(
                    "error: {}",
                    Error::SerdeDeserialization { source: error }
                ))
            })?;

            info!("Starting Upgrade controller...");
            upgrade_controller().await.map_err(|error| {
                error!(?error, "failed to run Upgrade controller");
                RestError::default().with_error("failed to run Upgrade controller".to_string())
            })?;

            return Ok(HttpResponse::Ok()
                .content_type(ContentType::json())
                .json(res_body));
        }
        Err(error) => {
            error!(?error, "failed to create UpgradeAction resource");
            let err = RestError::default()
                .with_error("unable to create UpgradeAction resource".to_string());
            Err(err)
        }
    }
}

/// Get  upgrade.
#[get("/upgrade")]
pub async fn get_upgrade() -> impl Responder {
    match UpgradeOperatorConfig::get_config()
        .k8s_client()
        .get_upgrade_action_resource()
        .await
    {
        Ok(u) => {
            let status = match &u.status {
                Some(status) => status.state().to_string(),
                None => "<Empty>".to_string(),
            };

            let res = Upgrade::default()
                .with_name(u.name_any())
                .with_current_version(u.spec.current_version().to_string())
                .with_target_version(u.spec.target_version().to_string())
                .with_state(status);
            Ok(res)
        }
        Err(error) => {
            error!(?error, "failed to GET UpgradeAction resource");
            let err = RestError::default()
                .with_error("unable to create UpgradeAction resource".to_string());
            Err(err)
        }
    }
}
