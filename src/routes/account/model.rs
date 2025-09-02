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
    pub fn update_password_hash(&mut self, password_hash: String) {
        self.password_hash = password_hash;
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use fake::{Fake, Faker, faker};

    use super::*;

    #[test]
    fn test_update_password_hash() {
        let mut account = Account {
            id: uuid::Uuid::new_v4(),
            email: faker::internet::en::SafeEmail().fake(),
            password_hash: Faker.fake(),
            email_verified: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let new_password_hash: String = Faker.fake();
        account.update_password_hash(new_password_hash.clone());
        assert_eq!(account.password_hash, new_password_hash);
    }
}
