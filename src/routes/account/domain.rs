use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{prelude::FromRow, types::uuid};
use thiserror::Error;
use tracing::warn;

use super::{
    SignupBody, VerifyEmailBody, password_strategy::PasswordStrategy,
    verification_code_strategy::VerificationCodeStrategy,
};

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

// ###############################################
// ################## RETRIEVAL ##################
// ###############################################

/// Errors for everything related to querying in the account domain
#[derive(Error, Debug)]
pub enum AccountQueryError {
    #[error("Account not found")]
    AccountNotFound,
    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

// #############################################
// ################## SIGN UP ##################
// #############################################

#[derive(Debug)]
pub struct SignupRequest {
    pub email: String,
    pub password_hash: String,
    pub verification_plaintext: u32,
    pub verification_cyphertext: String,
}

#[derive(Error, Debug)]
pub enum SignupRequestError {
    #[error("A verified account already exist for the email: {email}")]
    AccountAlreadyVerified { email: String },
    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

impl SignupRequest {
    pub fn try_from_body_existing_account(
        account: Account,
        body: SignupBody,
    ) -> Result<Self, SignupRequestError> {
        if account.email_verified {
            return Err(SignupRequestError::AccountAlreadyVerified {
                email: account.email,
            });
        }
        Self::try_from_body(body)
    }

    pub fn try_from_body(body: SignupBody) -> Result<Self, SignupRequestError> {
        let password_hash = PasswordStrategy::hash_password(&body.password)?;
        let (verification_plaintext, verification_cyphertext) =
            VerificationCodeStrategy::generate_verification_code(&body.email)?;
        Ok(Self {
            email: body.email,
            password_hash,
            verification_plaintext,
            verification_cyphertext,
        })
    }
}

#[derive(Error, Debug)]
pub enum SignupError {
    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

// ##########################################################
// ################## ACCOUNT VERIFICATION ##################
// ##########################################################

#[derive(Debug)]
pub struct VerifyAccountRequest {
    pub account_id: uuid::Uuid,
}

#[derive(Error, Debug)]
pub enum VerifyAccountRequestError {
    #[error("invalid verification code")]
    InvalidVerificationCode,
    #[error("account is already verified for email: {email}")]
    AccountAlreadyVerified { email: String },
    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

impl VerifyAccountRequest {
    pub fn try_from_body(
        body: VerifyEmailBody,
        account: Account,
        verification_request: Option<VerificationCodeRequest>,
    ) -> Result<VerifyAccountRequest, VerifyAccountRequestError> {
        if account.email_verified {
            return Err(VerifyAccountRequestError::AccountAlreadyVerified { email: body.email });
        }
        let verification_request =
            verification_request.ok_or(VerifyAccountRequestError::InvalidVerificationCode)?;

        if Utc::now()
            .signed_duration_since(verification_request.created_at)
            .gt(&TimeDelta::minutes(15))
        {
            return Err(VerifyAccountRequestError::InvalidVerificationCode);
        }

        VerificationCodeStrategy::verify_verification_code(
            body.code,
            &account.email,
            &verification_request.cyphertext,
        )
        .map_err(|e| {
            warn!("{e}");
            VerifyAccountRequestError::InvalidVerificationCode
        })?;

        Ok(VerifyAccountRequest {
            account_id: account.id,
        })
    }
}

/// Errors that may occur while using connectors
#[derive(Error, Debug)]
pub enum VerifyAccountError {
    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

// #############################################
// ################### TESTS ###################
// #############################################

#[cfg(test)]
mod tests {
    use chrono::Days;
    use fake::{Dummy, Fake, faker};

    use crate::routes::account::verification_code_strategy::VerificationCodeStrategy;

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
                VerificationCodeStrategy::generate_verification_code("abc@def.com").unwrap();
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

    // #[test]
    // fn test_update_password_hash() {
    //     let mut account: Account = Faker.fake();
    //     let new_password_hash: String = Faker.fake();
    //     account.update_password_hash(new_password_hash.clone());
    //     assert_eq!(account.password_hash, new_password_hash);
    // }

    // #[test]
    // fn test_verify_email() {
    //     let mut account: Account = Faker.fake();
    //     account.verify_email();
    //     assert!(account.email_verified);
    // }

    // #[test]
    // fn test_confirm_verification_request() {
    //     let mut verification_request: VerificationCodeRequest = Faker.fake();
    //     verification_request.confirm();
    //     match verification_request.status {
    //         VerificationCodeRequestStatus::Confirmed => {}
    //         _ => {
    //             panic!("Expected `confirmed` verification request")
    //         }
    //     };
    // }

    // #[test]
    // fn test_verification_request_expiration() {
    //     let mut verification_request: VerificationCodeRequest = Faker.fake();
    //     verification_request.created_at = Utc::now()
    //         .checked_sub_signed(TimeDelta::minutes(14))
    //         .unwrap();
    //     assert!(!verification_request.is_expired());

    //     verification_request.created_at = Utc::now()
    //         .checked_sub_signed(TimeDelta::minutes(16))
    //         .unwrap();
    //     assert!(verification_request.is_expired());
    // }
}
