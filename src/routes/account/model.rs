use chrono::{DateTime, Utc};
use sqlx::{prelude::FromRow, types::uuid};

#[derive(FromRow)]
pub struct Account {
    pub id: uuid::Uuid,
    pub email: String,
    pub password_hash: String,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Account {
    /// Update the password hash of an account
    ///
    /// # Arguments
    /// * `password` - Updated password to be hashed
    pub fn update_password_hash(&mut self, password: &str) -> &mut Self {
        self.password_hash = Self::hash_password(password);
        self.updated_at = Utc::now();
        self
    }

    /// Hash a password
    ///
    /// # Arguments
    /// * `password` - Password to be hashed
    pub fn hash_password(password: &str) -> String {
        format!("{password}1234")
    }
}
