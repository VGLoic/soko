use fake::{Dummy, Fake, Faker, faker};
use reqwest::StatusCode;
use serde::Serialize;
use soko::routes::AccountResponse;

mod common;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TestSignupBody {
    email: String,
    password: String,
}

impl<T> Dummy<T> for TestSignupBody {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &T, rng: &mut R) -> Self {
        let mut password: String = faker::internet::en::Password(10..36).fake_with_rng(rng);
        password += "6;9+";
        TestSignupBody {
            email: faker::internet::en::SafeEmail().fake_with_rng(rng),
            password,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TestVerifyAccountBody {
    email: String,
    secret: String,
}

#[tokio::test]
async fn test_account_signup() {
    let test_state = common::setup().await.unwrap();

    let signup_body = Faker.fake::<TestSignupBody>();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&signup_body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        response
            .json::<AccountResponse>()
            .await
            .unwrap()
            .email
            .as_str(),
        signup_body.email.to_lowercase()
    );
}

#[tokio::test]
async fn test_account_email_verification() {
    let test_state = common::setup().await.unwrap();

    let signup_body = Faker.fake::<TestSignupBody>();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&signup_body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = client
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
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_forbidden_signup_once_verified() {
    let test_state = common::setup().await.unwrap();

    let signup_body = Faker.fake::<TestSignupBody>();

    let client = reqwest::Client::new();
    client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&signup_body)
        .send()
        .await
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
        .unwrap();

    assert_eq!(
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
            .status(),
        StatusCode::BAD_REQUEST
    );

    let mut another_signup_body = Faker.fake::<TestSignupBody>();
    another_signup_body.email = signup_body.email;

    assert_eq!(
        client
            .post(format!("{}/accounts/signup", &test_state.server_url))
            .json(&another_signup_body)
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

    let signup_body = Faker.fake::<TestSignupBody>();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&signup_body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let mut new_signup_body = Faker.fake::<TestSignupBody>();
    new_signup_body.email = signup_body.email.clone();

    let update_response = client
        .post(format!("{}/accounts/signup", &test_state.server_url))
        .json(&new_signup_body)
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
