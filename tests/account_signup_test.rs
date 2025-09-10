use fake::{Fake, faker};
use reqwest::StatusCode;
use soko::routes::{AccountResponse, SignupBody, VerifyEmailBody};

mod common;

#[tokio::test]
async fn test_account_signup() {
    let test_state = common::setup().await.unwrap();

    let email: String = faker::internet::en::SafeEmail().fake();
    let password = faker::internet::en::Password(10..40).fake();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&SignupBody {
            email: email.clone(),
            password,
        })
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        response.json::<AccountResponse>().await.unwrap().email,
        email
    );
}

#[tokio::test]
async fn test_account_email_verification() {
    let test_state = common::setup().await.unwrap();

    let email: String = faker::internet::en::SafeEmail().fake();
    let password = faker::internet::en::Password(10..40).fake();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&SignupBody {
            email: email.clone(),
            password,
        })
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = client
        .post(format!("{}/accounts/verify-email", &test_state.server_url))
        .json(&VerifyEmailBody {
            email: email.clone(),
            secret: test_state
                .mailing_service
                .get_verification_secret(&email)
                .unwrap()
                .unwrap(),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_forbidden_signup_once_verified() {
    let test_state = common::setup().await.unwrap();

    let email: String = faker::internet::en::SafeEmail().fake();
    let password = faker::internet::en::Password(10..40).fake();

    let client = reqwest::Client::new();
    client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&SignupBody {
            email: email.clone(),
            password,
        })
        .send()
        .await
        .unwrap();
    client
        .post(format!("{}/accounts/verify-email", &test_state.server_url))
        .json(&VerifyEmailBody {
            email: email.clone(),
            secret: test_state
                .mailing_service
                .get_verification_secret(&email)
                .unwrap()
                .unwrap(),
        })
        .send()
        .await
        .unwrap();

    assert_eq!(
        client
            .post(format!("{}/accounts/verify-email", &test_state.server_url))
            .json(&VerifyEmailBody {
                email: email.clone(),
                secret: test_state
                    .mailing_service
                    .get_verification_secret(&email)
                    .unwrap()
                    .unwrap(),
            })
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );

    assert_eq!(
        client
            .post(format!("{}/accounts/signup", &test_state.server_url))
            .json(&SignupBody {
                email: email.clone(),
                password: faker::internet::en::Password(10..40).fake(),
            })
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    )
}

#[tokio::test]
async fn test_account_signup_two_successive_times() {
    let test_state = common::setup().await.unwrap();

    let email: String = faker::internet::en::SafeEmail().fake();
    let password = faker::internet::en::Password(10..40).fake();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&SignupBody {
            email: email.clone(),
            password,
        })
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let updated_password = faker::internet::en::Password(10..40).fake();

    let update_response = client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&SignupBody {
            email: email.clone(),
            password: updated_password,
        })
        .send()
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::CREATED);

    let account = response.json::<AccountResponse>().await.unwrap();
    let updated_account = update_response.json::<AccountResponse>().await.unwrap();
    assert_eq!(account.created_at, updated_account.created_at);
    assert!(
        account.updated_at.timestamp_micros() < updated_account.updated_at.timestamp_micros(),
        "{} is equal or after {}",
        account.updated_at,
        updated_account.updated_at
    );
}
