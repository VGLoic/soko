use axum::{
    Json, Router,
    extract::{FromRequest, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tracing::{error, warn};
use validator::{Validate, ValidationError, ValidationErrors};

pub mod domain;
mod repository;
pub use repository::{AccountRepository, PostgresAccountRepository};

use domain::{
    Account, AccountQueryError, SignupError, SignupRequest, SignupRequestError, VerifyAccountError,
    VerifyAccountRequest, VerifyAccountRequestError,
};

use super::AppState;
mod password_strategy;
mod verification_code_strategy;

pub fn account_router() -> Router<AppState> {
    Router::new()
        .route("/signup", post(signup_account))
        .route("/verify-email", post(verify_email))
}

// ############################################
// ################## ERRORS ##################
// ############################################

#[derive(Debug)]
pub enum ApiError {
    InternalServerError(anyhow::Error),
    BadRequest(ValidationErrors),
    NotFound,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::InternalServerError(e) => {
                error!("{e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            }
            Self::BadRequest(errors) => (StatusCode::BAD_REQUEST, Json(errors)).into_response(),
            Self::NotFound => (StatusCode::NOT_FOUND, "Not found").into_response(),
        }
    }
}

impl From<AccountQueryError> for ApiError {
    fn from(value: AccountQueryError) -> Self {
        match value {
            AccountQueryError::AccountNotFound => ApiError::NotFound,
            AccountQueryError::Unknown(e) => ApiError::InternalServerError(e),
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

impl From<domain::Account> for AccountResponse {
    fn from(value: domain::Account) -> Self {
        AccountResponse {
            email: value.email,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

// ##############################################
// ################## SIGN UP ###################
// ##############################################

impl From<SignupError> for ApiError {
    fn from(value: SignupError) -> Self {
        match value {
            SignupError::Unknown(e) => ApiError::InternalServerError(e),
        }
    }
}

impl From<SignupRequestError> for ApiError {
    fn from(value: SignupRequestError) -> ApiError {
        match value {
            SignupRequestError::Unknown(e) => ApiError::InternalServerError(e),
            SignupRequestError::AccountAlreadyVerified { email: _email } => {
                let mut errors = ValidationErrors::new();
                errors.add(
                    "email",
                    ValidationError::new("existing-email")
                        .with_message("Email is already associated with a verified account".into()),
                );
                ApiError::BadRequest(errors)
            }
        }
    }
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignupBody {
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
    ValidatedJson(body): ValidatedJson<SignupBody>,
) -> Result<(StatusCode, Json<AccountResponse>), ApiError> {
    let signup_request: SignupRequest;
    let signed_up_account: Account;

    let existing_account_opt = match app_state
        .account_repository
        .get_account_by_email(&body.email)
        .await
    {
        Ok(v) => Some(v),
        Err(e) => {
            if let AccountQueryError::AccountNotFound = e {
                None
            } else {
                return Err(e.into());
            }
        }
    };

    if let Some(existing_account) = existing_account_opt {
        signup_request =
            SignupRequest::try_from_body_with_existing_account(existing_account, body)?;

        signed_up_account = app_state
            .account_repository
            .reset_account_creation(&signup_request)
            .await?;
    } else {
        signup_request = SignupRequest::try_from_body(body)?;
        signed_up_account = app_state
            .account_repository
            .create_account(&signup_request)
            .await?
    };

    if let Err(e) = app_state
        .mailing_service
        .send_email(
            &signup_request.email,
            signup_request.verification_plaintext.to_string().as_str(),
        )
        .await
    {
        error!(
            "failed to send email to email \"{}\" with error {e}",
            &signup_request.email
        );
    }

    Ok((StatusCode::CREATED, Json(signed_up_account.into())))
}

// ####################################################
// ################## VERIFY ACCOUNT ##################
// ####################################################

#[derive(Debug, Validate, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEmailBody {
    #[validate(email(message = "invalid email format"))]
    pub email: String,
    #[validate(range(min = 1, exclusive_max = 100_000_000))]
    pub code: u32,
}

impl From<VerifyAccountRequestError> for ApiError {
    fn from(value: VerifyAccountRequestError) -> Self {
        match value {
            VerifyAccountRequestError::Unknown(e) => ApiError::InternalServerError(e),
            VerifyAccountRequestError::AccountAlreadyVerified { email: _email } => {
                let mut errors = ValidationErrors::new();
                errors.add(
                    "email",
                    ValidationError::new("email-verified")
                        .with_message("Account is already verified".into()),
                );
                ApiError::BadRequest(errors)
            }
            VerifyAccountRequestError::InvalidVerificationCode => {
                let mut errors = ValidationErrors::new();
                errors.add(
                    "code",
                    ValidationError::new("code-validity").with_message("Code is invalid".into()),
                );
                ApiError::BadRequest(errors)
            }
        }
    }
}

impl From<VerifyAccountError> for ApiError {
    fn from(value: VerifyAccountError) -> Self {
        match value {
            VerifyAccountError::Unknown(e) => ApiError::InternalServerError(e),
        }
    }
}

async fn verify_email(
    State(app_state): State<AppState>,
    ValidatedJson(body): ValidatedJson<VerifyEmailBody>,
) -> Result<(StatusCode, Json<AccountResponse>), ApiError> {
    let (existing_account, verification_request) = app_state
        .account_repository
        .get_account_by_email_with_verification_request(&body.email)
        .await?;

    let verify_account_request =
        VerifyAccountRequest::try_from_body(body, existing_account, verification_request)?;

    let updated_account = app_state
        .account_repository
        .verify_account(verify_account_request.account_id)
        .await?;

    Ok((StatusCode::OK, Json(updated_account.into())))
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
