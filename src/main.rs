use std::time::Duration;

use axum::{
    Router,
    body::Body,
    extract::{MatchedPath, Request},
    http::Response,
    routing::get,
};
use dotenvy::dotenv;
use tokio::signal;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};
use tracing::{Span, error, info, info_span};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    if let Err(err) = dotenv()
        && !err.not_found()
    {
        let err = format!("Error while loading .env file: {err}");
        error!(err);
        return Err(anyhow::anyhow!(err));
    }

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new().route("/health", get(healthcheck)).layer((
        TraceLayer::new_for_http()
            .make_span_with(|request: &Request<_>| {
                let matched_path = request
                    .extensions()
                    .get::<MatchedPath>()
                    .map(MatchedPath::as_str);

                let request_id = Uuid::new_v4();
                info_span!(
                    "http_request",
                    method = ?request.method(),
                    matched_path,
                    request_id = %request_id
                )
            })
            .on_response(
                |response: &Response<Body>, latency: Duration, _span: &Span| {
                    if response.status().is_server_error() {
                        error!("response: {} {latency:?}", response.status())
                    } else {
                        info!("response: {} {latency:?}", response.status())
                    }
                },
            ),
        TimeoutLayer::new(Duration::from_secs(10)),
    ));

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|err| {
        let err = format!("Error while binding the TCP listener to address {addr}: {err}");

        error!(err);
        anyhow::anyhow!(err)
    })?;

    info!("Successfully bind the TCP listener to address {addr}\n");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|err| {
            let err = format!("Error while serving the routes: {err}");
            error!(err);
            anyhow::anyhow!(err)
        })?;

    info!("App has been gracefully shutdown");

    Ok(())
}

async fn healthcheck() -> &'static str {
    info!("Healthcheck called");
    "OK"
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
