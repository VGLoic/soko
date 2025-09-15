use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{prelude::FromRow, types::uuid};
use thiserror::Error;
use tracing::warn;
use validator::{ValidationError, ValidationErrors};

use crate::newtypes::{Email, EmailError, Password, PasswordError};

use super::{
    SignupBody, VerifyEmailBody, verification_secret_strategy::VerificationSecretStrategy,
};

#[derive(FromRow, Clone, Debug)]
pub struct Account {
    pub id: uuid::Uuid,
    pub email: Email,
    pub password_hash: String,
    pub verified: bool,
    // This field is automatically set at creation at the database level
    pub created_at: DateTime<Utc>,
    // This field is automatically updated at the database level
    pub updated_at: DateTime<Utc>,
}

#[derive(FromRow, Clone, Debug)]
pub struct AccountVerificationTicket {
    pub id: uuid::Uuid,
    pub account_id: uuid::Uuid,
    pub cyphertext: String,
    pub status: AccountVerificationTicketStatus,
    // This field is automatically set at creation at the database level
    pub created_at: DateTime<Utc>,
    // This field is automatically updated at the database level
    pub updated_at: DateTime<Utc>,
}

#[derive(sqlx::Type, Clone, Debug)]
#[sqlx(
    type_name = "account_verification_ticket_status",
    rename_all = "lowercase"
)]
pub enum AccountVerificationTicketStatus {
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
    pub email: Email,
    pub password_hash: String,
    pub verification_plaintext: String,
    pub verification_cyphertext: String,
}

/// Errors in the construction of the [SignupRequest]
#[derive(Error, Debug)]
pub enum SignupRequestError {
    #[error("Invalid body, got errors: {0}")]
    InvalidBody(ValidationErrors),
    #[error("A verified account already exist for the email: {email}")]
    AccountAlreadyVerified { email: Email },
    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

impl From<EmailError> for SignupRequestError {
    fn from(value: EmailError) -> Self {
        let mut validation_errors = ValidationErrors::new();
        let error =
            match value {
                EmailError::Empty => ValidationError::new("invalid-email")
                    .with_message("empty is not allowed".into()),
                EmailError::InvalidFormat => ValidationError::new("invalid-email")
                    .with_message("invalid email format".into()),
            };
        validation_errors.add("email", error);
        SignupRequestError::InvalidBody(validation_errors)
    }
}

impl From<PasswordError> for SignupRequestError {
    fn from(value: PasswordError) -> Self {
        let mut validation_errors = ValidationErrors::new();
        let error = match value {
            PasswordError::Empty => {
                ValidationError::new("invalid-password").with_message("empty is not allowed".into())
            }
            PasswordError::InvalidPassword(reason) => {
                ValidationError::new("invalid-password").with_message(reason.into())
            }
        };
        validation_errors.add("password", error);
        SignupRequestError::InvalidBody(validation_errors)
    }
}

impl SignupRequest {
    /// Build a [SignupRequest] using a [SignupBody] HTTP body
    pub fn try_from_body(body: SignupBody) -> Result<Self, SignupRequestError> {
        let password_hash = Password::new(body.password)?.hash()?;
        let (verification_plaintext, verification_cyphertext) =
            VerificationSecretStrategy::generate_verification_secret(&body.email)?;
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
        if account.verified {
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

    use crate::routes::account::verification_secret_strategy::VerificationSecretStrategy;

    use super::*;

    impl<T> Dummy<T> for Account {
        fn dummy_with_rng<R: fake::Rng + ?Sized>(_: &T, rng: &mut R) -> Self {
            let created_at = faker::chrono::en::DateTimeBefore(
                Utc::now().checked_sub_days(Days::new(2)).unwrap(),
            )
            .fake_with_rng(rng);
            Account {
                id: uuid::Uuid::new_v4(),
                email: Faker.fake_with_rng(rng),
                password_hash: "$2y$10$EZGQ6TDVUAicnOu4LgVoI.kFmcbFkT9nlOXeLfnKZtJYF8YjMM3mG"
                    .to_string(),
                verified: true,
                created_at,
                updated_at: faker::chrono::en::DateTimeBetween(created_at, Utc::now())
                    .fake_with_rng(rng),
            }
        }
    }

    #[test]
    fn test_signup_request_from_body() {
        let signup_body = SignupBody {
            email: faker::internet::en::SafeEmail().fake(),
            password: Faker.fake(),
        };
        let request = SignupRequest::try_from_body(signup_body.clone()).unwrap();
        assert_eq!(request.email, signup_body.email);
        assert!(
            VerificationSecretStrategy::verify_verification_secret(
                &request.verification_plaintext,
                &request.email,
                &request.verification_cyphertext
            )
            .is_ok()
        );
        assert!(
            Password::new(signup_body.password)
                .unwrap()
                .verify(&request.password_hash)
                .is_ok()
        );
    }

    #[test]
    fn test_signup_request_from_body_and_account() {
        let mut account: Account = Faker.fake();
        account.verified = false;
        let signup_body = SignupBody {
            email: Faker.fake(),
            password: Faker.fake(),
        };
        let request =
            SignupRequest::try_from_body_with_existing_account(account, signup_body.clone())
                .unwrap();
        assert_eq!(request.email, signup_body.email);
        assert!(
            VerificationSecretStrategy::verify_verification_secret(
                &request.verification_plaintext,
                &request.email,
                &request.verification_cyphertext
            )
            .is_ok()
        );
        assert!(
            Password::new(signup_body.password)
                .unwrap()
                .verify(&request.password_hash)
                .is_ok()
        );
    }

    #[test]
    fn test_signup_request_from_body_and_verified_account_must_fail() {
        let mut account: Account = Faker.fake();
        account.verified = true;
        let signup_body = SignupBody {
            email: faker::internet::en::SafeEmail().fake(),
            password: Faker.fake(),
        };

        let err =
            SignupRequest::try_from_body_with_existing_account(account, signup_body).unwrap_err();
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
    #[error("invalid verification secret")]
    InvalidVerificationSecret,
    #[error("account is already verified for email: {email}")]
    AccountAlreadyVerified { email: Email },
    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

impl VerifyAccountRequest {
    pub fn try_from_body(
        body: VerifyEmailBody,
        account: Account,
        verification_ticket: Option<AccountVerificationTicket>,
    ) -> Result<VerifyAccountRequest, VerifyAccountRequestError> {
        if account.verified {
            return Err(VerifyAccountRequestError::AccountAlreadyVerified { email: body.email });
        }
        let verification_ticket =
            verification_ticket.ok_or(VerifyAccountRequestError::InvalidVerificationSecret)?;

        if Utc::now()
            .signed_duration_since(verification_ticket.created_at)
            .gt(&TimeDelta::minutes(15))
        {
            return Err(VerifyAccountRequestError::InvalidVerificationSecret);
        }

        VerificationSecretStrategy::verify_verification_secret(
            &body.secret,
            &account.email,
            &verification_ticket.cyphertext,
        )
        .map_err(|e| {
            warn!("{e}");
            VerifyAccountRequestError::InvalidVerificationSecret
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

    use crate::routes::account::verification_secret_strategy::VerificationSecretStrategy;

    use super::*;

    impl<T> Dummy<T> for AccountVerificationTicket {
        fn dummy_with_rng<R: fake::Rng + ?Sized>(_: &T, rng: &mut R) -> Self {
            let created_at = faker::chrono::en::DateTimeBefore(
                Utc::now().checked_sub_days(Days::new(2)).unwrap(),
            )
            .fake_with_rng(rng);
            let (_, cyphertext) =
                VerificationSecretStrategy::generate_verification_secret(&Faker.fake::<Email>())
                    .unwrap();
            AccountVerificationTicket {
                id: uuid::Uuid::new_v4(),
                account_id: uuid::Uuid::new_v4(),
                cyphertext,
                status: AccountVerificationTicketStatus::Active,
                created_at,
                updated_at: faker::chrono::en::DateTimeBetween(created_at, Utc::now())
                    .fake_with_rng(rng),
            }
        }
    }

    fn setup() -> (Account, AccountVerificationTicket, VerifyEmailBody) {
        let signup_body = SignupBody {
            email: Faker.fake(),
            password: Faker.fake(),
        };
        let signup_request = SignupRequest::try_from_body(signup_body.clone()).unwrap();

        let verify_account_body = VerifyEmailBody {
            email: signup_body.email.clone(),
            secret: signup_request.verification_plaintext,
        };

        let mut account: Account = Faker.fake();
        account.verified = false;

        let mut verification_ticket: AccountVerificationTicket = Faker.fake();
        verification_ticket.created_at = Utc::now();
        verification_ticket.cyphertext = signup_request.verification_cyphertext.clone();

        (account, verification_ticket, verify_account_body)
    }

    #[test]
    fn test_verify_account_request_from_body() {
        let (account, verification_ticket, verify_account_body) = setup();

        let verify_account_request = VerifyAccountRequest::try_from_body(
            verify_account_body,
            account.clone(),
            Some(verification_ticket),
        )
        .unwrap();

        assert_eq!(verify_account_request.account_id, account.id);
    }

    #[test]
    fn test_verify_account_request_from_body_with_verified_account_must_fail() {
        let (mut account, verification_ticket, verify_account_body) = setup();
        account.verified = true;

        let err = VerifyAccountRequest::try_from_body(
            verify_account_body,
            account.clone(),
            Some(verification_ticket),
        )
        .unwrap_err();

        if let VerifyAccountRequestError::AccountAlreadyVerified { email: _email } = err {
        } else {
            panic!("Invalid error, expected `AccountAlreadyVerified` variant, got {err}");
        }
    }

    #[test]
    fn test_verify_account_request_from_body_with_no_active_verification_ticket_must_fail() {
        let (account, _verification_ticket, verify_account_body) = setup();

        let err = VerifyAccountRequest::try_from_body(verify_account_body, account.clone(), None)
            .unwrap_err();

        if let VerifyAccountRequestError::InvalidVerificationSecret = err {
        } else {
            panic!("Invalid error, expected `InvalidVerificationSecret` variant, got {err}");
        }
    }

    #[test]
    fn test_verify_account_request_from_body_with_expired_verification_ticket_must_fail() {
        let (account, mut verification_ticket, verify_account_body) = setup();

        verification_ticket.created_at = Utc::now()
            .checked_sub_signed(TimeDelta::minutes(16))
            .unwrap();

        let err = VerifyAccountRequest::try_from_body(
            verify_account_body,
            account.clone(),
            Some(verification_ticket),
        )
        .unwrap_err();

        if let VerifyAccountRequestError::InvalidVerificationSecret = err {
        } else {
            panic!("Invalid error, expected `InvalidVerificationSecret` variant, got {err}");
        }
    }

    #[test]
    fn test_verify_account_request_from_body_with_invalid_plaintext_must_fail() {
        let (account, verification_ticket, mut verify_account_body) = setup();

        let (other_plaintext, _) =
            VerificationSecretStrategy::generate_verification_secret(&account.email).unwrap();
        verify_account_body.secret = other_plaintext;

        let err = VerifyAccountRequest::try_from_body(
            verify_account_body,
            account.clone(),
            Some(verification_ticket),
        )
        .unwrap_err();

        if let VerifyAccountRequestError::InvalidVerificationSecret = err {
        } else {
            panic!("Invalid error, expected `InvalidVerificationSecret` variant, got {err}");
        }
    }
}
