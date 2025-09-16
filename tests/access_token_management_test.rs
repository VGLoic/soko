use fake::{Fake, Faker};
use reqwest::StatusCode;

use crate::common::{TestCreateAccessTokenBody, TestSignupBody, TestVerifyAccountBody};

mod common;

#[tokio::test]
async fn test_access_token_creation() {
    let test_state = common::setup().await.unwrap();

    let signup_body = Faker.fake::<TestSignupBody>();

    let client = reqwest::Client::new();
    client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&signup_body)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .post(format!("{}/accounts/verify-email", &test_state.server_url))
        .json(&TestVerifyAccountBody {
            email: signup_body.email.clone(),
            secret: test_state
                .mailing_service
                .get_verification_secret(&signup_body.email)
                .unwrap()
                .unwrap(),
        })
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let create_access_token_body = TestCreateAccessTokenBody {
        email: signup_body.email.clone(),
        password: signup_body.password.clone(),
        name: (1..40).fake(),
        lifetime: (1..(90 * 24 * 3600)).fake(),
    };
    let response = client
        .post(format!("{}/tokens", &test_state.server_url))
        .json(&create_access_token_body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}
