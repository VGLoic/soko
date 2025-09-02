use fake::{Fake, faker};
use reqwest::StatusCode;
use soko::routes::{AccountResponse, SignupPayload};

mod common;

#[tokio::test]
async fn test_account_signup() {
    let test_state = common::setup().await.unwrap();

    let email: String = faker::internet::en::SafeEmail().fake();
    let password = "1234abcd5678".to_owned();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&SignupPayload {
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
async fn test_account_signup_two_successive_times() {
    let test_state = common::setup().await.unwrap();

    let email: String = faker::internet::en::SafeEmail().fake();
    let password = "1234abcd5678".to_owned();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&SignupPayload {
            email: email.clone(),
            password,
        })
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let updated_password = "abcdefgh1234".to_owned();

    let update_response = client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&SignupPayload {
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
