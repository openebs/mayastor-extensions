use std::{collections::HashMap, fmt::Display};

use actix_web::{
    body::BoxBody,
    get,
    http::{header::ContentType, StatusCode},
    put, HttpRequest, HttpResponse, Responder, ResponseError,
};
use serde::{Deserialize, Serialize};

use crate::upgrade::k8s::crd::v0::UpgradePhase;

#[derive(Serialize, Deserialize)]
pub(crate) struct Upgrade {
    id: String,

    current_version: String,

    target_version: String,

    components_state: HashMap<String, HashMap<String, UpgradePhase>>,

    state: String,
}

impl Responder for Upgrade {
    type Body = BoxBody;

    fn respond_to(self, _req: &HttpRequest) -> HttpResponse<Self::Body> {
        let res_body = serde_json::to_string(&self).unwrap();

        // Create HttpResponse and set Content Type
        HttpResponse::Ok()
            .content_type(ContentType::json())
            .body(res_body)
    }
}

#[derive(Debug, Serialize)]
struct ErrNoId {
    id: u32,
    err: String,
}

// Implement ResponseError for ErrNoId
impl ResponseError for ErrNoId {
    fn status_code(&self) -> StatusCode {
        StatusCode::NOT_FOUND
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        let body = serde_json::to_string(&self).unwrap();
        let res = HttpResponse::new(self.status_code());
        res.set_body(BoxBody::new(body))
    }
}

// Implement Display for ErrNoId
impl Display for ErrNoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

// Put request for upgrade.
#[put("/upgrade")]
async fn apply_upgrade() -> Result<HttpResponse, ErrNoId> {

    // TODO: Start upgrade controller
    let response = "success".to_string();
    Ok(HttpResponse::Ok()
                .content_type(ContentType::json())
                .body(response))
}

// Get  upgrade.
#[get("/upgrade")]
async fn get_upgrade() -> impl Responder {
    // TODO: Implement get call for upgrade
    let response = "".to_string();

    HttpResponse::Ok()
        .content_type(ContentType::json())
        .body(response)
}
