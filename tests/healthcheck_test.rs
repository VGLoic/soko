use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use soko::{Healthcheck, app_router};
use tower::ServiceExt;

#[tokio::test]
async fn test_healthcheck() {
    let app = app_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: Healthcheck = serde_json::from_slice(&body).unwrap();
    assert!(body.ok);
}
