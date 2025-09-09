use super::model::{Account, VerificationCodeRequest};
use async_trait::async_trait;
use sqlx::{Pool, Postgres, types::uuid};
use thiserror::Error;

#[async_trait]
pub trait AccountRepository: Send + Sync {
    /// Get an account by email
    ///
    /// # Arguments
    /// * `email` - Email of the account
    ///
    /// # Errors
    /// * `Unclassified` - fallback error type
    async fn get_account_by_email(
        &self,
        email: &str,
    ) -> Result<Option<Account>, AccountRepositoryError>;

    /// Update an account identified by its ID
    ///
    /// # Arguments
    /// * `account` - Updated account,
    ///
    /// # Errors
    /// * `Unclassified` - fallback error type
    async fn update_account(&self, account: &Account) -> Result<Account, AccountRepositoryError>;

    /// Create an account
    ///
    /// # Arguments
    /// * `email` - Email of the account,
    /// * `password_hash` - Hash of the password
    ///
    /// # Errors
    /// * `Unclassified` - fallback error type
    async fn create_account(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<Account, AccountRepositoryError>;

    /// Cancel the last "active" request if any and create a new request linked to the account
    ///
    /// # Arguments
    /// * `account_id` - ID of the account,
    /// * `code_cyphertext` - cyphertext of the verification code
    ///
    /// # Errors
    /// * `Unclassified` - fallback error type
    async fn cancel_last_and_create_verification_request(
        &self,
        account_id: uuid::Uuid,
        code_cyphertext: &str,
    ) -> Result<VerificationCodeRequest, AccountRepositoryError>;

    /// Get the active verification request for an account
    ///
    /// # Arguments
    /// * `account_id` - ID of the account,
    ///
    /// # Errors
    /// * `Unclassified` - fallback error type
    async fn get_active_validation_request(
        &self,
        account_id: uuid::Uuid,
    ) -> Result<Option<VerificationCodeRequest>, AccountRepositoryError>;

    /// Update a verification request
    ///
    /// # Arguments
    /// * `verification_request` - Updated verification request
    ///
    /// # Errors
    /// * `Unclassified` - fallback error type
    async fn update_verification_request(
        &self,
        verification_request: &VerificationCodeRequest,
    ) -> Result<VerificationCodeRequest, AccountRepositoryError>;
}

#[derive(Error, Debug)]
pub enum AccountRepositoryError {
    #[error(transparent)]
    Unclassified(#[from] anyhow::Error),
}

pub struct PostgresAccountRepository {
    pool: Pool<Postgres>,
}

impl From<Pool<Postgres>> for PostgresAccountRepository {
    fn from(value: Pool<Postgres>) -> Self {
        PostgresAccountRepository { pool: value }
    }
}

#[async_trait]
impl AccountRepository for PostgresAccountRepository {
    async fn get_account_by_email(
        &self,
        email: &str,
    ) -> Result<Option<Account>, AccountRepositoryError> {
        let query_result = sqlx::query_as::<_, Account>(
            r#"
                SELECT
                    id,
                    email,
                    password_hash,
                    email_verified,
                    created_at,
                    updated_at
                FROM "account"
                WHERE "email" = $1
                "#,
        )
        .bind(email)
        .fetch_one(&self.pool)
        .await;

        match query_result {
            Ok(v) => Ok(Some(v)),
            Err(e) => {
                if let sqlx::Error::RowNotFound = e {
                    Ok(None)
                } else {
                    Err(anyhow::Error::from(e).into())
                }
            }
        }
    }

    async fn update_account(&self, account: &Account) -> Result<Account, AccountRepositoryError> {
        sqlx::query_as::<_, Account>(
            r#"
                UPDATE "account"
                SET
                    "password_hash" = $2,
                    "email_verified" = $3,
                    "updated_at" = $4
                WHERE "id" = $1
                RETURNING 
                    id,
                    email,
                    password_hash,
                    email_verified,
                    created_at,
                    updated_at
            "#,
        )
        .bind(account.id)
        .bind(&account.password_hash)
        .bind(account.email_verified)
        .bind(account.updated_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }

    async fn create_account(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<Account, AccountRepositoryError> {
        sqlx::query_as::<_, Account>(
            r#"
                INSERT INTO "account" (
                    "email",
                    "password_hash"
                ) VALUES (
                    $1,
                    $2
                ) RETURNING 
                    id,
                    email,
                    password_hash,
                    email_verified,
                    created_at,
                    updated_at
            "#,
        )
        .bind(email)
        .bind(password_hash)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }

    async fn cancel_last_and_create_verification_request(
        &self,
        account_id: uuid::Uuid,
        cyphertext: &str,
    ) -> Result<VerificationCodeRequest, AccountRepositoryError> {
        sqlx::query(
            r#"
                UPDATE "verification_code_request"
                SET "status" = 'cancelled'
                WHERE "account_id" = $1 AND "status" = 'active';
            "#,
        )
        .bind(account_id)
        .execute(&self.pool)
        .await
        .map_err(anyhow::Error::from)?;

        sqlx::query_as::<_, VerificationCodeRequest>(
            r#"
                INSERT INTO "verification_code_request" (
                    "account_id",
                    "cyphertext"
                ) VALUES (
                    $1,
                    $2
                ) RETURNING
                    id,
                    account_id,
                    cyphertext,
                    status,
                    created_at,
                    updated_at
            "#,
        )
        .bind(account_id)
        .bind(cyphertext)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }

    async fn get_active_validation_request(
        &self,
        account_id: uuid::Uuid,
    ) -> Result<Option<VerificationCodeRequest>, AccountRepositoryError> {
        let query_result = sqlx::query_as::<_, VerificationCodeRequest>(
            r#"
                SELECT
                    id,
                    account_id,
                    cyphertext,
                    status,
                    created_at,
                    updated_at
                FROM "verification_code_request"
                WHERE "account_id" = $1 AND "status" = 'active'
            "#,
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await;
    
        match query_result {
            Ok(v) => Ok(Some(v)),
            Err(e) => match e {
                sqlx::Error::RowNotFound => Ok(None),
                other => return Err(anyhow::Error::from(other).into()),
            },
        }
    }

    async fn update_verification_request(
        &self,
        verification_request: &VerificationCodeRequest,
    ) -> Result<VerificationCodeRequest, AccountRepositoryError> {
        sqlx::query_as::<_, VerificationCodeRequest>(
            r#"
            UPDATE "verification_code_request"
            SET "status" = $2
            WHERE "id" = $1
            RETURNING 
                id,
                account_id,
                cyphertext,
                status,
                created_at,
                updated_at
        "#,
        )
        .bind(verification_request.id)
        .bind(&verification_request.status)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
    }
}
