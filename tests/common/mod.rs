use std::net::SocketAddr;

use soko::app_router;
use tower_http::trace::TraceLayer;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

pub struct TestState {
    pub server_url: String,
}

pub async fn setup() -> Result<TestState, anyhow::Error> {
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(LevelFilter::DEBUG))
        .try_init();

    let app = app_router().layer(TraceLayer::new_for_http());

    // Giving 0 as port here will let the system dynamically find an available port
    // This is needed in order to let our test run in parallel
    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|err| {
        anyhow::anyhow!("Error while binding the TCP listener to address {addr}: {err}")
    })?;

    let addr = listener.local_addr().unwrap();

    info!("Successfully bind the TCP listener to address {addr}\n");

    // Start a server, the handle is kept in order to abort it if needed
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    Ok(TestState {
        server_url: format!("http://{}:{}", addr.ip(), addr.port()),
    })
}
