use std::time::Duration;

use axum::{
    body::Body,
    extract::{MatchedPath, Request},
    http::Response,
};
use dotenvy::dotenv;
use soko::{Config, app_router};
use sqlx::postgres::PgPoolOptions;
use tokio::signal;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};
use tracing::{Span, error, info, info_span, level_filters::LevelFilter};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    if let Err(err) = dotenv()
        && !err.not_found()
    {
        return Err(anyhow::anyhow!("Error while loading .env file: {err}"));
    }

    let config = match Config::build() {
        Ok(c) => c,
        Err(e) => {
            return Err(anyhow::anyhow!("Error while building configuration: {e}"));
        }
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_filter(Into::<LevelFilter>::into(config.log_level)),
        )
        .init();

    let pool = match PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await
    {
        Ok(c) => c,
        Err(e) => {
            let err = format!("Fail to establish connection to database {e}");
            error!(err);
            return Err(anyhow::anyhow!(err));
        }
    };

    if let Err(e) = sqlx::migrate!("db/migrations").run(&pool).await {
        let err = format!("Fail to execute database migrations: {e}");
        error!(err);
        return Err(anyhow::anyhow!(err));
    };

    info!("Successfully ran migrations");

    let app = app_router().layer((
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

    let addr = format!("0.0.0.0:{}", config.port);
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
