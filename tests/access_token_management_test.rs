use crate::common::{TestCreateAccessTokenBody, TestSignupBody, TestVerifyAccountBody};
use chrono::{DateTime, Utc};
use fake::{Fake, Faker};
use reqwest::StatusCode;
use serde::Deserialize;
use soko::routes::tokens::{MAX_LIFETIME, MAX_NAME_LENGTH};

mod common;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
struct TestAccessTokenCreatedResponse {
    pub id: uuid::Uuid,
    pub name: String,
    pub access_token: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

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
        name: (1..MAX_NAME_LENGTH).fake(),
        lifetime: (1..MAX_LIFETIME).fake(),
    };
    let response = client
        .post(format!("{}/tokens", &test_state.server_url))
        .json(&create_access_token_body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let json_response = response
        .json::<TestAccessTokenCreatedResponse>()
        .await
        .unwrap();

    assert_eq!(json_response.name, create_access_token_body.name);
    assert!(!json_response.access_token.is_empty());
    assert!(json_response.revoked_at.is_none());
}

#[tokio::test]
async fn test_create_too_many_access_tokens() {
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

    // The first three are successful
    for _ in 0..3 {
        let create_access_token_body = TestCreateAccessTokenBody {
            email: signup_body.email.clone(),
            password: signup_body.password.clone(),
            name: (1..MAX_NAME_LENGTH).fake(),
            lifetime: (1..MAX_LIFETIME).fake(),
        };
        let response = client
            .post(format!("{}/tokens", &test_state.server_url))
            .json(&create_access_token_body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    // The last one fails
    let create_access_token_body = TestCreateAccessTokenBody {
        email: signup_body.email.clone(),
        password: signup_body.password.clone(),
        name: (1..MAX_NAME_LENGTH).fake(),
        lifetime: (1..MAX_LIFETIME).fake(),
    };
    let response = client
        .post(format!("{}/tokens", &test_state.server_url))
        .json(&create_access_token_body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
