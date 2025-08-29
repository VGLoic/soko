use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use soko::app_router;
use tower::ServiceExt;

#[tokio::test]
async fn test_not_found() {
    let app = app_router();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/unknown-route")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert!(body.is_empty());
}
