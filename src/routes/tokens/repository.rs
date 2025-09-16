use anyhow::anyhow;
use async_trait::async_trait;
use sqlx::{Pool, Postgres};

use super::domain::{AccessToken, CreateAccessTokenError, CreateAccessTokenRequest};

#[async_trait]
pub trait AccessTokenRepository: Send + Sync {
    /// Create an access token
    ///
    /// # Arguments
    /// * `req` - DTO for create an access token
    /// * `max_active_token` - maximum number of active token allowed
    ///
    /// # Errors
    /// * `CreateAccessTokenError::Unknown` - unknown error
    async fn create_token(
        &self,
        req: &CreateAccessTokenRequest,
        max_active_token: u8,
    ) -> Result<AccessToken, CreateAccessTokenError>;
}

pub struct PostgresAccessTokenRepository {
    pool: Pool<Postgres>,
}

impl From<Pool<Postgres>> for PostgresAccessTokenRepository {
    fn from(value: Pool<Postgres>) -> Self {
        Self { pool: value }
    }
}

#[async_trait]
impl AccessTokenRepository for PostgresAccessTokenRepository {
    async fn create_token(
        &self,
        req: &CreateAccessTokenRequest,
        max_active_token: u8,
    ) -> Result<AccessToken, CreateAccessTokenError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|e| anyhow!(e).context("failed to start transaction"))?;

        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "access_token"
            WHERE "account_id" = $1 AND "revoked_at" IS NULL AND "expires_at" > CURRENT_TIMESTAMP
        "#,
        )
        .bind(req.account_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|e| anyhow!(e).context("failed to retrieve active access token count"))?;

        if count >= max_active_token.into() {
            return Err(CreateAccessTokenError::ActiveTokenLimitReached(
                max_active_token,
            ));
        }

        let access_token = sqlx::query_as::<_, AccessToken>(
            r#"
            INSERT INTO "access_token" (
                "account_id",
                "name",
                "mac",
                "expires_at"
            ) VALUES (
                $1,
                $2,
                $3,
                $4
            ) RETURNING
                id,
                account_id,
                name,
                mac,
                created_at,
                updated_at,
                expires_at,
                revoked_at
        "#,
        )
        .bind(req.account_id)
        .bind(&req.name)
        .bind(req.mac)
        .bind(req.expires_at)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|e| anyhow!(e).context("failed to insert access token"))?;

        transaction
            .commit()
            .await
            .map_err(|e| anyhow!(e).context("failed to commit transaction"))?;

        Ok(access_token)
    }
}
