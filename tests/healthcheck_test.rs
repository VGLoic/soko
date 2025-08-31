use axum::http::StatusCode;
use soko::routes::GetHealthcheckResponse;

mod common;

#[tokio::test]
async fn test_healthcheck() {
    let test_state = common::setup().await.unwrap();

    let response = reqwest::get(format!("{}/health", &test_state.server_url))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.json::<GetHealthcheckResponse>().await.unwrap().ok);
}
