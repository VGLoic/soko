use std::sync::Arc;

use axum::{Json, Router, http::StatusCode, response::IntoResponse, routing::get};
use serde::{Deserialize, Serialize};
mod account;
use super::Config;
pub use account::{AccountRepository, AccountResponse, PostgresAccountRepository, SignupPayload};

pub fn app_router(_: &Config, account_repository: impl AccountRepository + 'static) -> Router {
    let app_state = AppState {
        account_repository: Arc::new(account_repository),
    };
    Router::new()
        .nest("/accounts", account::account_router())
        .route("/health", get(get_healthcheck))
        .fallback(not_found_handler)
        .with_state(app_state)
}

#[derive(Clone)]
pub struct AppState {
    account_repository: Arc<dyn AccountRepository>,
}

#[derive(Serialize, Deserialize)]
pub struct GetHealthcheckResponse {
    pub ok: bool,
}
async fn get_healthcheck() -> (StatusCode, Json<GetHealthcheckResponse>) {
    (StatusCode::OK, Json(GetHealthcheckResponse { ok: true }))
}

async fn not_found_handler() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Not found")
}
