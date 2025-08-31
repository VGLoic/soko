use axum::{Json, Router, http::StatusCode, response::IntoResponse, routing::get};
use serde::{Deserialize, Serialize};

pub fn app_router() -> Router {
    Router::new()
        .route("/health", get(get_healthcheck))
        .fallback(not_found_handler)
}

#[derive(Serialize, Deserialize)]
pub struct GetHealthcheckResponse {
    pub ok: bool,
}
async fn get_healthcheck() -> (StatusCode, Json<GetHealthcheckResponse>) {
    (StatusCode::OK, Json(GetHealthcheckResponse { ok: true }))
}

async fn not_found_handler() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Page not found")
}
