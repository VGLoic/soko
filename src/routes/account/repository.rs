use super::domain::{
    Account, AccountQueryError, AccountVerificationTicket, SignupError, SignupRequest,
    VerifyAccountError,
};
use anyhow::anyhow;
use async_trait::async_trait;
use sqlx::{Pool, Postgres, types::uuid};

#[async_trait]
pub trait AccountRepository: Send + Sync {
    /// Get an account by email
    ///
    /// # Arguments
    /// * `email` - Email of the account
    ///
    /// # Errors
    /// * `AccountQueryError::Unknown` - unknown error
    /// * `AccountQueryError::AccountNotFound` - account not found
    async fn get_account_by_email(&self, email: &str) -> Result<Account, AccountQueryError>;

    /// Get an account by email with active verification ticket
    ///
    /// # Arguments
    /// * `email` - Email of the account
    ///
    /// # Errors
    /// * `AccountQueryError::Unknown` - unknown error
    /// * `AccountQueryError::AccountNotFound` - account not found
    async fn get_account_by_email_with_verification_ticket(
        &self,
        email: &str,
    ) -> Result<(Account, Option<AccountVerificationTicket>), AccountQueryError>;

    /// Create an account and creates an active verification ticket
    ///
    /// # Arguments
    /// * `email` - Email of the account,
    /// * `password_hash` - Hash of the password,
    /// * `verification_cyphertext` - Cyphertext of the verification ticket
    ///
    /// # Errors
    /// * `SignupError::Unknown` - unknown error
    async fn create_account(&self, signup_request: &SignupRequest) -> Result<Account, SignupError>;

    /// Reset an account creation:
    /// - update the password hash,
    /// - cancel last active verification ticket,
    /// - creates a new active verification ticket
    ///
    /// # Arguments
    /// * `password_hash` - Hash of the new password,
    /// * `verification_cyphertext` - Cyphertext of the verification ticket
    ///
    /// # Errors
    /// * `SignupError::Unknown` - unknown error
    async fn reset_account_creation(
        &self,
        signup_request: &SignupRequest,
    ) -> Result<Account, SignupError>;

    /// Verify an account:
    /// - update the `verified` to true,
    /// - confirm the verification ticket
    ///
    /// # Arguments
    /// * `account_id` - ID of the account,
    ///
    /// # Errors
    /// * `VerifyAccountError::Unknown` - unknown error
    async fn verify_account(&self, account_id: uuid::Uuid) -> Result<Account, VerifyAccountError>;
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
    async fn get_account_by_email(&self, email: &str) -> Result<Account, AccountQueryError> {
        let query_result = sqlx::query_as::<_, Account>(
            r#"
                SELECT
                    id,
                    email,
                    password_hash,
                    verified,
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
            Ok(v) => Ok(v),
            Err(e) => {
                if let sqlx::Error::RowNotFound = e {
                    Err(AccountQueryError::AccountNotFound)
                } else {
                    Err(anyhow!(e)
                        .context(format!("failed query for account with email: {email}"))
                        .into())
                }
            }
        }
    }

    async fn get_account_by_email_with_verification_ticket(
        &self,
        email: &str,
    ) -> Result<(Account, Option<AccountVerificationTicket>), AccountQueryError> {
        let account = self.get_account_by_email(email).await?;
        let verification_ticket = match sqlx::query_as::<_, AccountVerificationTicket>(
            r#"
                SELECT
                    id,
                    account_id,
                    cyphertext,
                    status,
                    created_at,
                    updated_at
                FROM "account_verification_ticket"
                WHERE "account_id" = $1 AND "status" = 'active'
            "#,
        )
        .bind(account.id)
        .fetch_one(&self.pool)
        .await
        {
            Ok(v) => Some(v),
            Err(e) => {
                if let sqlx::Error::RowNotFound = e {
                    None
                } else {
                    return Err(anyhow!(e)
                        .context(format!(
                            "failed query for active verification ticket with account ID: {}",
                            account.id
                        ))
                        .into());
                }
            }
        };

        Ok((account, verification_ticket))
    }

    async fn create_account(&self, req: &SignupRequest) -> Result<Account, SignupError> {
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
                    verified,
                    created_at,
                    updated_at
            "#,
        )
        .bind(&req.email)
        .bind(&req.password_hash)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|e| {
            anyhow!(e).context(format!(
                "failed to insert account with email: {}",
                req.email
            ))
        })?;

        sqlx::query(
            r#"
        INSERT INTO "account_verification_ticket" (
            "account_id",
            "cyphertext"
        ) VALUES (
            $1,
            $2
        );
    "#,
        )
        .bind(account.id)
        .bind(&req.verification_cyphertext)
        .execute(&mut *transaction)
        .await
        .map_err(|e| {
            anyhow!(e).context(format!(
                "failed to insert active verification ticket for created account with email: {}",
                req.email
            ))
        })?;

        transaction
            .commit()
            .await
            .map_err(|e| anyhow!(e).context("failed to commit transaction"))?;

        Ok(account)
    }

    async fn reset_account_creation(&self, req: &SignupRequest) -> Result<Account, SignupError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|e| anyhow!(e).context("failed to start transaction"))?;

        let account = sqlx::query_as::<_, Account>(
            r#"
            UPDATE "account"
            SET "password_hash" = $2
            WHERE "email" = $1
            RETURNING
                id,
                email,
                password_hash,
                verified,
                created_at,
                updated_at
        "#,
        )
        .bind(req.email.as_str())
        .bind(&req.password_hash)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|e| {
            anyhow!(e).context(format!(
                "failed to update account with email: {}",
                req.email
            ))
        })?;

        sqlx::query(
            r#"
            UPDATE "account_verification_ticket"
            SET "status" = 'cancelled'
            WHERE "account_id" = $1 AND "status" = 'active';
            "#,
        )
        .bind(account.id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| {
            anyhow!(e).context(format!(
                "failed to cancel previous active verification ticket for account ID: {}",
                account.id
            ))
        })?;

        sqlx::query(
            r#"
            INSERT INTO "account_verification_ticket" (
                "account_id",
                "cyphertext"
            ) VALUES (
                $1,
                $2
            );
        "#,
        )
        .bind(account.id)
        .bind(&req.verification_cyphertext)
        .execute(&mut *transaction)
        .await
        .map_err(|e| {
            anyhow!(e).context(format!(
                "failed to create new active verification ticket for ID: {}",
                account.id
            ))
        })?;

        transaction
            .commit()
            .await
            .map_err(|e| anyhow!(e).context("failed to commit transaction"))?;

        Ok(account)
    }

    async fn verify_account(&self, account_id: uuid::Uuid) -> Result<Account, VerifyAccountError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|e| anyhow!(e).context("failed to start transaction"))?;

        let account = sqlx::query_as::<_, Account>(
            r#"
            UPDATE "account"
            SET "verified" = TRUE
            WHERE "id" = $1
            RETURNING
                id,
                email,
                password_hash,
                verified,
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
            UPDATE "account_verification_ticket"
            SET "status" = 'confirmed'
            WHERE "account_id" = $1 AND "status" = 'active'
        "#,
        )
        .bind(account_id)
        .execute(&mut *transaction)
        .await
        .map_err(|e| {
            anyhow!(e).context(format!(
                "failed to confirm verification ticket for account with ID: {account_id}"
            ))
        })?;

        transaction
            .commit()
            .await
            .map_err(|e| anyhow!(e).context("failed to commit transaction"))?;

        Ok(account)
    }
}
