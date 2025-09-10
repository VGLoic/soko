use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use soko::{
    Config,
    routes::{PostgresAccountRepository, app_router},
    third_party::MailingService,
};
use sqlx::postgres::PgPoolOptions;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use tracing::{Level, info, level_filters::LevelFilter};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

#[allow(dead_code)]
pub struct TestState {
    pub mailing_service: FakeMailingService,
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
    let mailing_service = FakeMailingService::new();

    let app = app_router(&config, account_repository, mailing_service.clone())
        .layer(TraceLayer::new_for_http());

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
        mailing_service,
        server_url: format!("http://{}:{}", addr.ip(), addr.port()),
    })
}

#[derive(Clone, Debug)]
pub struct FakeMailingService {
    verification_secrets: Arc<RwLock<HashMap<String, String>>>,
}

impl FakeMailingService {
    fn new() -> Self {
        Self {
            verification_secrets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[allow(dead_code)]
    pub fn get_verification_secret(&self, email: &str) -> Result<Option<String>, anyhow::Error> {
        let secret = self
            .verification_secrets
            .try_read()?
            .get(email)
            .map(|v| v.to_owned());
        Ok(secret)
    }
}

#[async_trait]
impl MailingService for FakeMailingService {
    async fn send_email(&self, email: &str, content: &str) -> Result<(), anyhow::Error> {
        self.verification_secrets
            .try_write()?
            .insert(email.to_string(), content.to_owned());
        Ok(())
    }
}
