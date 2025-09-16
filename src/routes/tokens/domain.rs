use anyhow::anyhow;
use base64::{Engine, prelude::BASE64_STANDARD_NO_PAD};
use chrono::{DateTime, TimeDelta, Utc};
use hmac::{Hmac, Mac};
use rand::{Rng, SeedableRng};
use sha3::Sha3_256;
use sqlx::prelude::FromRow;
use thiserror::Error;

use crate::routes::{accounts::Account, newtypes::OpaqueToken};

use super::CreateAccessTokenBody;

// ###############################################
// ################## RETRIEVAL ##################
// ###############################################

/// Errors for everything related to querying
#[derive(Error, Debug)]
pub enum TokenQueryError {
    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

// ############################################
// ################## ENTITY ##################
// ############################################

#[derive(FromRow, Debug)]
pub struct AccessToken {
    pub id: uuid::Uuid,
    pub account_id: uuid::Uuid,
    pub name: String,
    pub mac: Vec<u8>,
    // This field is automatically set at creation at the database level
    pub created_at: DateTime<Utc>,
    // This field is automatically updated at the database level
    pub updated_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

// ###########################################################
// ################## ACCESS TOKEN CREATION ##################
// ###########################################################

#[derive(Clone, Debug)]
pub struct CreateAccessTokenRequest {
    pub account_id: uuid::Uuid,
    pub name: String,
    pub token: OpaqueToken,
    pub mac: [u8; 32],
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum CreateAccessTokenRequestError {
    #[error("invalid password")]
    InvalidPassword,
    #[error("invalid name")]
    InvalidName,
    #[error("invalid lifetime")]
    InvalidLifetime,
    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

#[derive(Error, Debug)]
pub enum CreateAccessTokenError {
    #[error("account has reached its access token limit: {0}")]
    ActiveTokenLimitReached(u8),
    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

impl CreateAccessTokenRequest {
    pub fn try_from_body(
        body: CreateAccessTokenBody,
        account: &Account,
        hmac_secret: &str,
    ) -> Result<Self, CreateAccessTokenRequestError> {
        if body.password.verify(&account.password_hash).is_err() {
            return Err(CreateAccessTokenRequestError::InvalidPassword);
        }

        let trimmed_name = body.name.trim();
        if trimmed_name.is_empty() {
            return Err(CreateAccessTokenRequestError::InvalidName);
        }
        if trimmed_name.len() > 40 {
            return Err(CreateAccessTokenRequestError::InvalidName);
        }

        if body.lifetime == 0 {
            return Err(CreateAccessTokenRequestError::InvalidLifetime);
        }
        if body.lifetime > 90 * 24 * 60 * 60 {
            return Err(CreateAccessTokenRequestError::InvalidLifetime);
        }

        let mut rng = rand_chacha::ChaCha20Rng::from_os_rng();
        let token_bytes: [u8; 64] = rng.random();
        let token = format!("soko__{}", BASE64_STANDARD_NO_PAD.encode(token_bytes));
        let mut hmac = Hmac::<Sha3_256>::new_from_slice(hmac_secret.as_bytes())
            .map_err(|e| anyhow!(e).context("failed to initialize hmac"))?;
        hmac.update(token.as_bytes());
        let mac = hmac.finalize().into_bytes().into();

        let expires_at = Utc::now()
            .checked_add_signed(TimeDelta::seconds(body.lifetime.into()))
            .ok_or(anyhow!("failed to derive expiration date"))?;

        Ok(CreateAccessTokenRequest {
            account_id: account.id,
            name: trimmed_name.to_string(),
            token: OpaqueToken::new(&token),
            mac,
            expires_at,
        })
    }
}
