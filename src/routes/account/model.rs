use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{prelude::FromRow, types::uuid};

#[derive(FromRow)]
pub struct Account {
    pub id: uuid::Uuid,
    pub email: String,
    pub password_hash: String,
    pub email_verified: bool,
    // This field is automatically set at creation at the database level
    pub created_at: DateTime<Utc>,
    // This field is automatically updated at the database level
    pub updated_at: DateTime<Utc>,
}

impl Account {
    /// Update the password hash of an account
    ///
    /// # Arguments
    /// * `password_hash` - Updated password hash
    pub fn update_password_hash(&mut self, password_hash: String) {
        self.password_hash = password_hash;
    }

    /// Verify the email of an account
    pub fn verify_email(&mut self) {
        self.email_verified = true;
    }
}

#[derive(FromRow)]
pub struct VerificationCodeRequest {
    pub id: uuid::Uuid,
    pub account_id: uuid::Uuid,
    pub cyphertext: String,
    pub status: VerificationCodeRequestStatus,
    // This field is automatically set at creation at the database level
    pub created_at: DateTime<Utc>,
    // This field is automatically updated at the database level
    pub updated_at: DateTime<Utc>,
}

#[derive(sqlx::Type)]
#[sqlx(
    type_name = "verification_code_request_status",
    rename_all = "lowercase"
)]
pub enum VerificationCodeRequestStatus {
    Active,
    Cancelled,
    Confirmed,
}

impl VerificationCodeRequest {
    pub fn confirm(&mut self) {
        self.status = VerificationCodeRequestStatus::Confirmed
    }

    pub fn is_expired(&self) -> bool {
        Utc::now()
            .signed_duration_since(self.created_at)
            .gt(&TimeDelta::minutes(15))
    }
}

#[cfg(test)]
mod tests {
    use chrono::Days;
    use fake::{Dummy, Fake, Faker, faker};

    use crate::routes::account::verification_code_strategy::VerificationCodeStategy;

    use super::*;

    impl<T> Dummy<T> for Account {
        fn dummy_with_rng<R: fake::Rng + ?Sized>(_: &T, rng: &mut R) -> Self {
            let created_at = faker::chrono::en::DateTimeBefore(
                Utc::now().checked_sub_days(Days::new(2)).unwrap(),
            )
            .fake_with_rng(rng);
            Account {
                id: uuid::Uuid::new_v4(),
                email: faker::internet::en::SafeEmail().fake_with_rng(rng),
                password_hash: "$2y$10$EZGQ6TDVUAicnOu4LgVoI.kFmcbFkT9nlOXeLfnKZtJYF8YjMM3mG"
                    .to_string(),
                email_verified: true,
                created_at,
                updated_at: faker::chrono::en::DateTimeBetween(created_at, Utc::now())
                    .fake_with_rng(rng),
            }
        }
    }

    impl<T> Dummy<T> for VerificationCodeRequest {
        fn dummy_with_rng<R: fake::Rng + ?Sized>(_: &T, rng: &mut R) -> Self {
            let created_at = faker::chrono::en::DateTimeBefore(
                Utc::now().checked_sub_days(Days::new(2)).unwrap(),
            )
            .fake_with_rng(rng);
            let (_, cyphertext) =
                VerificationCodeStategy::generate_verification_code("abc@def.com").unwrap();
            VerificationCodeRequest {
                id: uuid::Uuid::new_v4(),
                account_id: uuid::Uuid::new_v4(),
                cyphertext,
                status: VerificationCodeRequestStatus::Active,
                created_at,
                updated_at: faker::chrono::en::DateTimeBetween(created_at, Utc::now())
                    .fake_with_rng(rng),
            }
        }
    }

    #[test]
    fn test_update_password_hash() {
        let mut account: Account = Faker.fake();
        let new_password_hash: String = Faker.fake();
        account.update_password_hash(new_password_hash.clone());
        assert_eq!(account.password_hash, new_password_hash);
    }

    #[test]
    fn test_verify_email() {
        let mut account: Account = Faker.fake();
        account.verify_email();
        assert!(account.email_verified);
    }

    #[test]
    fn test_confirm() {
        let mut verification_request: VerificationCodeRequest = Faker.fake();
        verification_request.confirm();
        match verification_request.status {
            VerificationCodeRequestStatus::Confirmed => {}
            _ => {
                panic!("Expected `confirmed` verification request")
            }
        };
    }
}
