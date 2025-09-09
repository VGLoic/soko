use super::model::{Account, VerificationCodeRequest};
use anyhow::anyhow;
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

    /// Get an account by email with active verification request
    ///
    /// # Arguments
    /// * `email` - Email of the account
    ///
    /// # Errors
    /// * `Unclassified` - fallback error type
    async fn get_account_by_email_with_verification_request(
        &self,
        email: &str,
    ) -> Result<Option<(Account, Option<VerificationCodeRequest>)>, AccountRepositoryError>;

    /// Create an account and creates an active verification request
    ///
    /// # Arguments
    /// * `email` - Email of the account,
    /// * `password_hash` - Hash of the password,
    /// * `verification_cyphertext` - Cyphertext of the verification request
    ///
    /// # Errors
    /// * `Unclassified` - fallback error type
    async fn create_account(
        &self,
        email: &str,
        password_hash: &str,
        verification_cyphertext: &str,
    ) -> Result<Account, AccountRepositoryError>;

    /// Reset an account creation:
    /// - update the password hash,
    /// - cancel last active verification request,
    /// - creates a new active verification request
    ///
    /// # Arguments
    /// * `account_id` - ID of the account,
    /// * `password_hash` - Hash of the new password,
    /// * `verification_cyphertext` - Cyphertext of the verification request
    ///
    /// # Errors
    /// * `Unclassified` - fallback error type
    async fn reset_account_creation(
        &self,
        account_id: uuid::Uuid,
        password_hash: &str,
        verification_cyphertext: &str,
    ) -> Result<Account, AccountRepositoryError>;

    /// Verify an account:
    /// - update the `email_verified` to true,
    /// - confirm the verification request
    ///
    /// # Arguments
    /// * `account_id` - ID of the account,
    ///
    /// # Errors
    /// * `Unclassified` - fallback error type
    async fn verify_account(
        &self,
        account_id: uuid::Uuid,
    ) -> Result<Account, AccountRepositoryError>;
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

    async fn get_account_by_email_with_verification_request(
        &self,
        email: &str,
    ) -> Result<Option<(Account, Option<VerificationCodeRequest>)>, AccountRepositoryError> {
        let account = self.get_account_by_email(email).await?;

        match account {
            None => return Ok(None),
            Some(a) => {
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
                .bind(a.id)
                .fetch_one(&self.pool)
                .await;

                match query_result {
                    Ok(v) => return Ok(Some((a, Some(v)))),
                    Err(e) => match e {
                        sqlx::Error::RowNotFound => return Ok(Some((a, None))),
                        other => return Err(anyhow!(other).into()),
                    },
                };
            }
        };
    }

    async fn create_account(
        &self,
        email: &str,
        password_hash: &str,
        verification_cyphertext: &str,
    ) -> Result<Account, AccountRepositoryError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|e| anyhow!(e).context("failed to start transaction"))?;

        let account = sqlx::query_as::<_, Account>(
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
        .fetch_one(&mut *transaction)
        .await
        .map_err(|e| anyhow!(e).context(format!("failed to insert account with email: {email}")))?;

        sqlx::query(r#"
        INSERT INTO "verification_code_request" (
            "account_id",
            "cyphertext"
        ) VALUES (
            $1,
            $2
        );
    "#).bind(account.id).bind(verification_cyphertext).execute(&mut *transaction).await.map_err(|e| anyhow!(e).context(format!("failed to insert active verification request for created account with email: {email}")))?;

        transaction
            .commit()
            .await
            .map_err(|e| anyhow!(e).context("failed to commit transaction"))?;

        Ok(account)
    }

    async fn reset_account_creation(
        &self,
        account_id: uuid::Uuid,
        password_hash: &str,
        verification_cyphertext: &str,
    ) -> Result<Account, AccountRepositoryError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|e| anyhow!(e).context("failed to start transaction"))?;

        let account = sqlx::query_as::<_, Account>(
            r#"
            UPDATE "account"
            SET "password_hash" = $2
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
        .bind(account_id)
        .bind(password_hash)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|e| {
            anyhow!(e).context(format!("failed to update account with ID: {account_id}"))
        })?;

        sqlx::query(
            r#"
            UPDATE "verification_code_request"
            SET "status" = 'cancelled'
            WHERE "account_id" = $1 AND "status" = 'active';
            "#,
        )
        .bind(account_id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| {
            anyhow!(e).context(format!(
                "failed to cancel previous active verification request for account ID: {account_id}"
            ))
        })?;

        sqlx::query(
            r#"
            INSERT INTO "verification_code_request" (
                "account_id",
                "cyphertext"
            ) VALUES (
                $1,
                $2
            );
        "#,
        )
        .bind(account_id)
        .bind(verification_cyphertext)
        .execute(&mut *transaction)
        .await
        .map_err(|e| {
            anyhow!(e).context(format!(
                "failed to create new active verification request for ID: {account_id}"
            ))
        })?;

        transaction
            .commit()
            .await
            .map_err(|e| anyhow!(e).context("failed to commit transaction"))?;

        Ok(account)
    }

    async fn verify_account(
        &self,
        account_id: uuid::Uuid,
    ) -> Result<Account, AccountRepositoryError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|e| anyhow!(e).context("failed to start transaction"))?;

        let account = sqlx::query_as::<_, Account>(
            r#"
            UPDATE "account"
            SET "email_verified" = TRUE
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
        .bind(account_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|e| {
            anyhow!(e).context(format!("failed to update account with ID: {account_id}"))
        })?;

        sqlx::query(
            r#"
            UPDATE "verification_code_request"
            SET "status" = 'confirmed'
            WHERE "account_id" = $1 AND "status" = 'active'
        "#,
        )
        .bind(account_id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| {
            anyhow!(e).context(format!(
                "failed to confirm verification request for account with ID: {account_id}"
            ))
        })?;

        transaction
            .commit()
            .await
            .map_err(|e| anyhow!(e).context("failed to commit transaction"))?;

        Ok(account)
    }
}
