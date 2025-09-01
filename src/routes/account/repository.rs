use super::model::Account;
use async_trait::async_trait;
use sqlx::{Pool, Postgres};
use thiserror::Error;

#[async_trait]
pub trait AccountRepository: Send + Sync {
    /// Get an account by email
    ///
    /// # Arguments
    /// * `email` - Email of the account
    ///
    /// # Errors
    /// - `Unclassified`: fallback error type
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
    /// - `AccountNotFound`: account not found
    /// - `Unclassified`: fallback error type
    async fn update_account(&self, account: &Account) -> Result<(), AccountRepositoryError>;

    /// Crate an account
    ///
    /// # Arguments
    /// * `email` - Email of the account,
    /// * `password_hash` - Hash of the password
    ///
    /// # Errors
    /// - `AccountNotFound`: account not found after creation
    /// - `Unclassified`: fallback error type
    async fn create_account(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<Account, AccountRepositoryError>;
}

#[derive(Error, Debug)]
pub enum AccountRepositoryError {
    #[error(transparent)]
    Unclassified(#[from] anyhow::Error),
    #[error("Account not found using search param: {0}")]
    AccountNotFound(String),
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
        match sqlx::query_as::<_, Account>(
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
        .await
        {
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

    async fn update_account(&self, account: &Account) -> Result<(), AccountRepositoryError> {
        let rows_affected = sqlx::query(
            r#"
                UPDATE "account"
                SET
                    "password_hash" = $2,
                    "email_verified" = $3,
                    "updated_at" = $4
                WHERE "id" = $1
            "#,
        )
        .bind(account.id)
        .bind(&account.password_hash)
        .bind(account.email_verified)
        .bind(account.updated_at)
        .execute(&self.pool)
        .await
        .map_err(anyhow::Error::from)?
        .rows_affected();

        if rows_affected == 0 {
            return Err(AccountRepositoryError::AccountNotFound(format!(
                "id = {}",
                account.id
            )));
        }

        Ok(())
    }

    async fn create_account(
        &self,
        email: &str,
        password_hash: &str,
    ) -> Result<Account, AccountRepositoryError> {
        sqlx::query(
            r#"
                INSERT INTO "account" (
                    "email",
                    "password_hash"
                ) VALUES (
                    $1,
                    $2
                )
            "#,
        )
        .bind(email)
        .bind(password_hash)
        .execute(&self.pool)
        .await
        .map_err(anyhow::Error::from)?;

        self.get_account_by_email(email)
            .await?
            .ok_or(AccountRepositoryError::AccountNotFound(format!(
                "email = {email}"
            )))
    }
}
