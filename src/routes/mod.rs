use axum::{Json, Router, routing::get};
use serde::{Deserialize, Serialize};

pub fn app_router() -> Router {
    Router::new().route("/health", get(get_healthcheck))
}

#[derive(Serialize, Deserialize)]
pub struct GetHealthcheckResponse {
    pub ok: bool,
}
async fn get_healthcheck() -> Json<GetHealthcheckResponse> {
    Json(GetHealthcheckResponse { ok: true })
}
