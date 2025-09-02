use axum::{
    Json, Router,
    extract::{FromRequest, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;
use tracing::{error, warn};
use validator::{Validate, ValidationError, ValidationErrors};

pub mod model;
mod repository;
pub use repository::{AccountRepository, PostgresAccountRepository};

use super::AppState;
mod password_hasher;
use password_hasher::PasswordHasher;

pub fn account_router() -> Router<AppState> {
    Router::new().route("/signup", post(signup_account))
}

// ############################################
// ################## ERRORS ##################
// ############################################

#[derive(Error, Debug)]
pub enum AccountError {
    #[error(transparent)]
    Unclassified(#[from] anyhow::Error),
    #[error("A verified account already exist for the email: {0}")]
    AccountAlreadyVerified(String),
}

impl IntoResponse for AccountError {
    fn into_response(self) -> axum::response::Response {
        error!("{self}");
        match self {
            Self::Unclassified(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            }
            Self::AccountAlreadyVerified(_) => {
                let mut errors = ValidationErrors::new();
                errors.add(
                    "email",
                    ValidationError::new("existing-email")
                        .with_message("Email is already used for another account".into()),
                );
                (StatusCode::BAD_REQUEST, Json(errors)).into_response()
            }
        }
    }
}

// ######################################################
// ################## GENERIC RESPONSE ##################
// ######################################################

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResponse {
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<model::Account> for AccountResponse {
    fn from(value: model::Account) -> Self {
        AccountResponse {
            email: value.email,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

// ##############################################
// ################## HANDLERS ##################
// ##############################################

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
    State(app_state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<SignupPayload>,
) -> Result<(StatusCode, Json<AccountResponse>), AccountError> {
    if let Some(mut existing_account) = app_state
        .account_repository
        .get_account_by_email(&payload.email)
        .await
        .map_err(anyhow::Error::from)?
    {
        if existing_account.email_verified {
            return Err(AccountError::AccountAlreadyVerified(existing_account.email));
        }

        existing_account.update_password_hash(PasswordHasher::hash_password(&payload.password)?);

        existing_account = app_state
            .account_repository
            .update_account(&existing_account)
            .await
            .map_err(anyhow::Error::from)?;

        return Ok((StatusCode::CREATED, Json(existing_account.into())));
    }

    let created_account = app_state
        .account_repository
        .create_account(
            &payload.email,
            &PasswordHasher::hash_password(&payload.password)?,
        )
        .await
        .map_err(anyhow::Error::from)?;

    Ok((StatusCode::CREATED, Json(created_account.into())))
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
