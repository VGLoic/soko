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
