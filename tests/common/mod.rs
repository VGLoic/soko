use std::{net::SocketAddr, time::Duration};

use soko::{
    Config,
    routes::{PostgresAccountRepository, app_router},
};
use sqlx::postgres::PgPoolOptions;
use tower_http::trace::TraceLayer;
use tracing::{Level, info, level_filters::LevelFilter};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

pub struct TestState {
    pub server_url: String,
}

const INTEGRATION_DATABASE_URL: &str = "postgresql://admin:admin@localhost:5433/soko";

pub async fn setup() -> Result<TestState, anyhow::Error> {
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(LevelFilter::TRACE))
        .try_init();

    let config = Config {
        port: 0,
        log_level: Level::TRACE,
        database_url: INTEGRATION_DATABASE_URL.to_string(),
        password_salt: "abc123".to_string(),
    };

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to establish connection to database: {e}"))?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run database migrations: {e}"))?;

    let account_repository = PostgresAccountRepository::from(pool);

    let app = app_router(&config, account_repository).layer(TraceLayer::new_for_http());

    // Giving 0 as port here will let the system dynamically find an available port
    // This is needed in order to let our test run in parallel
    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|err| {
        anyhow::anyhow!("Failed to bind the TCP listener to address {addr}: {err}")
    })?;

    let addr = listener.local_addr().unwrap();

    info!("Successfully bound the TCP listener to address {addr}\n");

    // Start a server, the handle is kept in order to abort it if needed
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    Ok(TestState {
        server_url: format!("http://{}:{}", addr.ip(), addr.port()),
    })
}
