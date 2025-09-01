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
    /// * `password_hash` - Updated password hash
    pub fn update_password_hash(&mut self, password_hash: String) -> &mut Self {
        self.password_hash = password_hash;
        self.updated_at = Utc::now();
        self
    }
}
