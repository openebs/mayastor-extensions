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
    fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    fn with_current_version(mut self, current_version: String) -> Self {
        self.current_version = current_version;
        self
    }

    fn with_target_version(mut self, target_version: String) -> Self {
        self.target_version = target_version;
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
            println!("UpgradeAction CRD created");
        }
        Err(err) => {
            println!("Error:{} ", err);
            let err =
                RestError::default().with_error("unable to create upgradeaction crd".to_string());
            return Err(err);
        }
    }

    match UpgradeOperatorConfig::get_config()
        .k8s_client()
        .create_upgrade_action_resource()
        .await
    {
        Ok(u) => {
            let res = Upgrade::default()
                .with_name(u.name_any())
                .with_current_version(u.spec.current_version().to_string())
                .with_target_version(u.spec.target_version().to_string());
            let res_body = serde_json::to_string(&res)
                .map_err(|err| Error::SerdeDeserialization { source: err })
                .unwrap();

            upgrade_controller()
                .await
                .expect("Error while running controller");

            return Ok(HttpResponse::Ok()
                .content_type(ContentType::json())
                .body(res_body));
        }
        Err(err) => {
            println!("Error:{} ", err);
            let err = RestError::default()
                .with_error("unable to create upgradeaction resource".to_string());
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
            let res = Upgrade::default()
                .with_name(u.name_any())
                .with_current_version(u.spec.current_version().to_string())
                .with_target_version(u.spec.target_version().to_string());
            Ok(res)
        }
        Err(e) => {
            println!("Error:{} ", e);
            let err = RestError::default()
                .with_error("unable to create upgradeaction resource".to_string());
            Err(err)
        }
    }
}
