use axum::{
    Json, Router,
    extract::{FromRequest, State, rejection::JsonRejection},
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

use verification_code_strategy::VerificationCodeStrategy;

use super::AppState;
mod password_strategy;
use password_strategy::PasswordStrategy;
mod verification_code_strategy;

pub fn account_router() -> Router<AppState> {
    Router::new()
        .route("/signup", post(signup_account))
        .route("/verify-email", post(verify_email))
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
    #[error("Account not found for email: {0}")]
    AccountNotFound(String),
    #[error("Invalid verification code for email: {0}")]
    InvalidVerificationCode(String),
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
                        .with_message("Email is already associated with a verified account".into()),
                );
                (StatusCode::BAD_REQUEST, Json(errors)).into_response()
            }
            Self::AccountNotFound(_) => (StatusCode::NOT_FOUND, "Not found").into_response(),
            Self::InvalidVerificationCode(_) => {
                (StatusCode::BAD_REQUEST, "Invalid code").into_response()
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

        existing_account.update_password_hash(PasswordStrategy::hash_password(&payload.password)?);
        let (code, code_cyphertext) =
            VerificationCodeStrategy::generate_verification_code(&payload.email)?;

        existing_account = app_state
            .account_repository
            .reset_account_creation(
                existing_account.id,
                &existing_account.password_hash,
                &code_cyphertext,
            )
            .await
            .map_err(anyhow::Error::from)?;

        let _ = app_state
            .mailing_service
            .send_email(&payload.email, code.to_string().as_str())
            .await;

        return Ok((StatusCode::CREATED, Json(existing_account.into())));
    }

    let (code, code_cyphertext) =
        VerificationCodeStrategy::generate_verification_code(&payload.email)?;

    let created_account = app_state
        .account_repository
        .create_account(
            &payload.email,
            &PasswordStrategy::hash_password(&payload.password)?,
            &code_cyphertext,
        )
        .await
        .map_err(anyhow::Error::from)?;

    let _ = app_state
        .mailing_service
        .send_email(&payload.email, code.to_string().as_str())
        .await;

    Ok((StatusCode::CREATED, Json(created_account.into())))
}

#[derive(Debug, Validate, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEmailPayload {
    #[validate(email(message = "invalid email format"))]
    pub email: String,
    #[validate(range(min = 1, exclusive_max = 100_000_000))]
    pub code: u32,
}

async fn verify_email(
    State(app_state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<VerifyEmailPayload>,
) -> Result<(StatusCode, Json<AccountResponse>), AccountError> {
    let (mut existing_account, verification_request) = app_state
        .account_repository
        .get_account_by_email_with_verification_request(&payload.email)
        .await
        .map_err(anyhow::Error::from)?
        .ok_or_else(|| AccountError::AccountNotFound(payload.email.clone()))?;

    if existing_account.email_verified {
        return Err(AccountError::AccountAlreadyVerified(payload.email));
    }

    let mut verification_request = verification_request
        .ok_or_else(|| anyhow::anyhow!("Active verification request not found"))?;

    if verification_request.is_expired() {
        return Err(AccountError::InvalidVerificationCode(payload.email));
    }

    if !VerificationCodeStrategy::verify_verification_code(
        payload.code,
        &existing_account.email,
        &verification_request.cyphertext,
    )? {
        return Err(AccountError::InvalidVerificationCode(payload.email));
    }

    existing_account.verify_email();
    verification_request.confirm();

    existing_account = app_state
        .account_repository
        .verify_account(existing_account.id)
        .await
        .map_err(anyhow::Error::from)?;

    Ok((StatusCode::OK, Json(existing_account.into())))
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
                return Err(match e {
                    JsonRejection::JsonDataError(_) => (
                        StatusCode::BAD_REQUEST,
                        "Valid JSON body but not the expected JSON data format",
                    )
                        .into_response(),
                    _ => (StatusCode::BAD_REQUEST, "Invalid JSON body").into_response(),
                });
            }
        };
        if let Err(e) = payload.validate() {
            return Err((StatusCode::BAD_REQUEST, Json(e)).into_response());
        }

        Ok(Self(payload.0))
    }
}
