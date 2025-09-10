use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{prelude::FromRow, types::uuid};
use thiserror::Error;
use tracing::warn;

use super::{
    SignupBody, VerifyEmailBody, password_strategy::PasswordStrategy,
    verification_code_strategy::VerificationCodeStrategy,
};

#[derive(FromRow, Clone, Debug)]
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

#[derive(FromRow, Clone, Debug)]
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

#[derive(sqlx::Type, Clone, Debug)]
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

/// DTO of the signup action
/// It carries the needed informations in order to perform the signup action.
#[derive(Debug)]
pub struct SignupRequest {
    pub email: String,
    pub password_hash: String,
    pub verification_plaintext: u32,
    pub verification_cyphertext: String,
}

/// Errors in the construction of the [SignupRequest]
#[derive(Error, Debug)]
pub enum SignupRequestError {
    #[error("A verified account already exist for the email: {email}")]
    AccountAlreadyVerified { email: String },
    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

impl SignupRequest {
    /// Build a [SignupRequest] using a [SignupBody] HTTP body
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

    /// Build a [SignupRequest] using a [SignupBody] HTTP body and a previously signed up account
    pub fn try_from_body_with_existing_account(
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
}

/// Errors in the interactions with adapters, e.g. database repository
#[derive(Error, Debug)]
pub enum SignupError {
    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

#[cfg(test)]
mod signup_tests {
    use chrono::Days;
    use fake::{Dummy, Fake, Faker, faker};

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

    #[test]
    fn test_signup_request_from_body() {
        let email: String = faker::internet::en::SafeEmail().fake();
        let password: String = faker::internet::en::Password(10..40).fake();
        let signup_body = SignupBody {
            email: email.clone(),
            password: password.clone(),
        };
        let request = SignupRequest::try_from_body(signup_body.clone()).unwrap();
        assert_eq!(request.email, email);
        assert!(
            VerificationCodeStrategy::verify_verification_code(
                request.verification_plaintext,
                &email,
                &request.verification_cyphertext
            )
            .is_ok()
        );
        assert!(PasswordStrategy::verify_password(&password, &request.password_hash).is_ok());
    }

    #[test]
    fn test_signup_request_from_body_and_account() {
        let mut account: Account = Faker.fake();
        account.email_verified = false;
        let email: String = faker::internet::en::SafeEmail().fake();
        let password: String = faker::internet::en::Password(10..40).fake();
        let signup_body = SignupBody {
            email: email.clone(),
            password: password.clone(),
        };
        let request =
            SignupRequest::try_from_body_with_existing_account(account, signup_body.clone())
                .unwrap();
        assert_eq!(request.email, email);
        assert!(
            VerificationCodeStrategy::verify_verification_code(
                request.verification_plaintext,
                &email,
                &request.verification_cyphertext
            )
            .is_ok()
        );
        assert!(PasswordStrategy::verify_password(&password, &request.password_hash).is_ok());
    }

    #[test]
    fn test_signup_request_from_body_and_verified_account_must_fail() {
        let mut account: Account = Faker.fake();
        account.email_verified = true;
        let email: String = faker::internet::en::SafeEmail().fake();
        let password: String = faker::internet::en::Password(10..40).fake();
        let signup_body = SignupBody {
            email: email.clone(),
            password: password.clone(),
        };

        let err = SignupRequest::try_from_body_with_existing_account(account, signup_body.clone())
            .unwrap_err();
        if let SignupRequestError::AccountAlreadyVerified { email: _email } = err {
        } else {
            panic!("Invalid error, expected `AccountAlreadyVerified` variant, got {err}");
        }
    }
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

#[cfg(test)]
mod verify_account_tests {
    use chrono::Days;
    use fake::{Dummy, Fake, Faker, faker};

    use crate::routes::account::verification_code_strategy::VerificationCodeStrategy;

    use super::*;

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

    fn setup() -> (Account, VerificationCodeRequest, VerifyEmailBody) {
        let email: String = faker::internet::en::SafeEmail().fake();
        let password: String = faker::internet::en::Password(10..40).fake();
        let signup_body = SignupBody {
            email: email.clone(),
            password: password.clone(),
        };
        let signup_request = SignupRequest::try_from_body(signup_body).unwrap();

        let verify_account_body = VerifyEmailBody {
            email: email.clone(),
            code: signup_request.verification_plaintext,
        };

        let mut account: Account = Faker.fake();
        account.email_verified = false;

        let mut verification_request: VerificationCodeRequest = Faker.fake();
        verification_request.created_at = Utc::now();
        verification_request.cyphertext = signup_request.verification_cyphertext.clone();

        (account, verification_request, verify_account_body)
    }

    #[test]
    fn test_verify_account_request_from_body() {
        let (account, verification_request, verify_account_body) = setup();

        let verify_account_request = VerifyAccountRequest::try_from_body(
            verify_account_body,
            account.clone(),
            Some(verification_request),
        )
        .unwrap();

        assert_eq!(verify_account_request.account_id, account.id);
    }

    #[test]
    fn test_verify_account_request_from_body_with_verified_account_must_fail() {
        let (mut account, verification_request, verify_account_body) = setup();
        account.email_verified = true;

        let err = VerifyAccountRequest::try_from_body(
            verify_account_body,
            account.clone(),
            Some(verification_request),
        )
        .unwrap_err();

        if let VerifyAccountRequestError::AccountAlreadyVerified { email: _email } = err {
        } else {
            panic!("Invalid error, expected `AccountAlreadyVerified` variant, got {err}");
        }
    }

    #[test]
    fn test_verify_account_request_from_body_with_no_active_verification_request_must_fail() {
        let (account, _verification_request, verify_account_body) = setup();

        let err = VerifyAccountRequest::try_from_body(verify_account_body, account.clone(), None)
            .unwrap_err();

        if let VerifyAccountRequestError::InvalidVerificationCode = err {
        } else {
            panic!("Invalid error, expected `InvalidVerificationCode` variant, got {err}");
        }
    }

    #[test]
    fn test_verify_account_request_from_body_with_expired_verification_request_must_fail() {
        let (account, mut verification_request, verify_account_body) = setup();

        verification_request.created_at = Utc::now()
            .checked_sub_signed(TimeDelta::minutes(16))
            .unwrap();

        let err = VerifyAccountRequest::try_from_body(
            verify_account_body,
            account.clone(),
            Some(verification_request),
        )
        .unwrap_err();

        if let VerifyAccountRequestError::InvalidVerificationCode = err {
        } else {
            panic!("Invalid error, expected `InvalidVerificationCode` variant, got {err}");
        }
    }

    #[test]
    fn test_verify_account_request_from_body_with_invalid_plaintext_must_fail() {
        let (account, verification_request, mut verify_account_body) = setup();

        let (other_plaintext, _) =
            VerificationCodeStrategy::generate_verification_code(&account.email).unwrap();
        verify_account_body.code = other_plaintext;

        let err = VerifyAccountRequest::try_from_body(
            verify_account_body,
            account.clone(),
            Some(verification_request),
        )
        .unwrap_err();

        if let VerifyAccountRequestError::InvalidVerificationCode = err {
        } else {
            panic!("Invalid error, expected `InvalidVerificationCode` variant, got {err}");
        }
    }
}
