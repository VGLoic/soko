use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError, ValidationErrors};

use crate::newtypes::Email;
mod domain;
use super::{ApiError, ValidatedJson};
use domain::{
    CreateAccessTokenError, CreateAccessTokenRequest, CreateAccessTokenRequestError,
    TokenQueryError,
};
mod repository;
pub use repository::{AccessTokenRepository, PostgresAccessTokenRepository};

use super::{
    AppState,
    newtypes::{OpaqueToken, Password},
};

pub fn tokens_router() -> Router<AppState> {
    Router::new().route("/", post(create_access_token))
}

// ############################################
// ################## ERRORS ##################
// ############################################

impl From<TokenQueryError> for ApiError {
    fn from(value: TokenQueryError) -> Self {
        match value {
            TokenQueryError::Unknown(e) => ApiError::InternalServerError(e),
        }
    }
}

// ###########################################################
// ################## ACCESS TOKEN CREATION ##################
// ###########################################################

#[derive(Debug, Clone, Validate, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccessTokenBody {
    email: Email,
    password: Password,
    name: String,
    lifetime: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessTokenCreatedResponse {
    pub id: uuid::Uuid,
    pub name: String,
    pub access_token: OpaqueToken,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

async fn create_access_token(
    State(app_state): State<AppState>,
    ValidatedJson(body): ValidatedJson<CreateAccessTokenBody>,
) -> Result<(StatusCode, Json<AccessTokenCreatedResponse>), ApiError> {
    let account = app_state
        .account_repository
        .get_verified_account_by_email(&body.email)
        .await?;

    let req = CreateAccessTokenRequest::try_from_body(body, &account, "coucou I am a secret")?;

    let access_token = app_state
        .access_token_repository
        .create_token(&req, 3)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(AccessTokenCreatedResponse {
            id: access_token.id,
            name: access_token.name,
            access_token: req.token,
            created_at: access_token.created_at,
            updated_at: access_token.updated_at,
            expires_at: access_token.expires_at,
            revoked_at: access_token.revoked_at,
        }),
    ))
}

impl From<CreateAccessTokenError> for ApiError {
    fn from(value: CreateAccessTokenError) -> Self {
        match value {
            CreateAccessTokenError::ActiveTokenLimitReached(_) => {
                let mut validation_errors = ValidationErrors::new();
                validation_errors.add(
                    "name",
                    ValidationError::new("too-many-tokens")
                        .with_message("limit of active access token reached".into()),
                );
                ApiError::BadRequest(validation_errors)
            }
            CreateAccessTokenError::Unknown(e) => ApiError::InternalServerError(e),
        }
    }
}

impl From<CreateAccessTokenRequestError> for ApiError {
    fn from(value: CreateAccessTokenRequestError) -> Self {
        match value {
            CreateAccessTokenRequestError::InvalidPassword => ApiError::Unauthorized,
            CreateAccessTokenRequestError::InvalidName => {
                let mut validation_errors = ValidationErrors::new();
                let error = ValidationError::new("invalid-length").with_message(
                    "name must not be empty and must be less than 40 characters long".into(),
                );
                validation_errors.add("name", error);
                ApiError::BadRequest(validation_errors)
            }
            CreateAccessTokenRequestError::InvalidLifetime => {
                let mut validation_errors = ValidationErrors::new();
                let error = ValidationError::new("invalid-range")
                    .with_message("lifetime must be more than 0 and less than 90 days".into());
                validation_errors.add("lifetime", error);
                ApiError::BadRequest(validation_errors)
            }
            CreateAccessTokenRequestError::Unknown(e) => ApiError::InternalServerError(e),
        }
    }
}
