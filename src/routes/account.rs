use axum::{
    Json, Router,
    extract::FromRequest,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;
use tracing::{error, warn};
use validator::Validate;

pub fn router() -> Router {
    Router::new().route("/accounts/signup", post(signup_account))
}

#[derive(Error, Debug)]
pub enum AccountError {
    #[error(transparent)]
    Unhandled(#[from] anyhow::Error),
}

impl IntoResponse for AccountError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Unhandled(e) => {
                error!("{e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResponse {
    pub email: String,
}

#[derive(Debug, Validate, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignupPayload {
    #[validate(email(message = "invalid email format"))]
    pub email: String,
    #[validate(length(
        min = 10,
        max = 40,
        message = "password must contain between 10 and 40 characters"
    ))]
    pub password: String,
}

async fn signup_account(
    ValidatedJson(payload): ValidatedJson<SignupPayload>,
) -> Result<(StatusCode, Json<AccountResponse>), AccountError> {
    Ok((
        StatusCode::CREATED,
        Json(AccountResponse {
            email: payload.email,
        }),
    ))
}

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
                return Err((StatusCode::BAD_REQUEST, "Invalid JSON body").into_response());
            }
        };
        if let Err(e) = payload.validate() {
            return Err((StatusCode::BAD_REQUEST, Json(e)).into_response());
        }

        Ok(Self(payload.0))
    }
}
