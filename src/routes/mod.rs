use std::sync::Arc;
use tracing::{error, warn};

use axum::{
    Json, Router,
    extract::FromRequest,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use validator::{Validate, ValidationErrors};
mod accounts;
mod newtypes;
mod tokens;

use super::{Config, third_party::MailingService};
pub use accounts::{AccountRepository, AccountResponse, PostgresAccountRepository};
pub use tokens::{AccessTokenRepository, PostgresAccessTokenRepository};

pub fn app_router(
    config: &Config,
    account_repository: impl AccountRepository + 'static,
    access_token_repository: impl AccessTokenRepository + 'static,
    mailing_service: impl MailingService + 'static,
) -> Router {
    let app_state = AppState {
        account_repository: Arc::new(account_repository),
        access_token_repository: Arc::new(access_token_repository),
        mailing_service: Arc::new(mailing_service),
    };
    Router::new()
        .nest("/accounts", accounts::accounts_router())
        .nest(
            "/tokens",
            tokens::tokens_router(config.access_token_secret.to_string()),
        )
        .route("/health", get(get_healthcheck))
        .fallback(not_found_handler)
        .with_state(app_state)
}

#[derive(Clone)]
pub struct AppState {
    account_repository: Arc<dyn AccountRepository>,
    access_token_repository: Arc<dyn AccessTokenRepository>,
    mailing_service: Arc<dyn MailingService>,
}

// ############################################
// ################## ERRORS ##################
// ############################################

#[derive(Debug)]
enum ApiError {
    InternalServerError(anyhow::Error),
    BadRequest(ValidationErrors),
    NotFound,
    Unauthorized,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::InternalServerError(e) => {
                error!("{e:?}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            }
            Self::BadRequest(errors) => (StatusCode::BAD_REQUEST, Json(errors)).into_response(),
            Self::NotFound => (StatusCode::NOT_FOUND, "Not found").into_response(),
            Self::Unauthorized => StatusCode::UNAUTHORIZED.into_response(),
        }
    }
}

// ###########################################
// ################## UTILS ##################
// ###########################################

struct ValidatedJson<T>(T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        let payload: Json<T> = match Json::from_request(req, state).await {
            Ok(p) => p,
            Err(e) => {
                warn!("{e}");
                return Err((StatusCode::BAD_REQUEST, e.body_text()).into_response());
            }
        };
        if let Err(e) = payload.validate() {
            return Err((StatusCode::BAD_REQUEST, Json(e)).into_response());
        }

        Ok(Self(payload.0))
    }
}

// #################################################
// ################## HEALTHCHECK ##################
// #################################################

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
