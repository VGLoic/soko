use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::anyhow;
use async_trait::async_trait;
use base64::prelude::*;
use fake::{Dummy, Fake, faker};
use serde::Serialize;
use soko::{
    Config,
    newtypes::{Email, Opaque},
    routes::{
        accounts::PostgresAccountRepository, app_router, tokens::PostgresAccessTokenRepository,
    },
    third_party::MailingService,
};
use sqlx::postgres::PgPoolOptions;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use tracing::{Level, info, level_filters::LevelFilter};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

// ################################################################
// ####################### REQUEST PAYLOADS #######################
// ################################################################

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct TestSignupBody {
    pub email: String,
    pub password: String,
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
#[allow(dead_code)]
pub struct TestVerifyAccountBody {
    pub email: String,
    pub secret: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct TestCreateAccessTokenBody {
    pub email: String,
    pub password: String,
    pub name: String,
    pub lifetime: u32,
}

// ##########################################################
// ####################### TEST STATE #######################
// ##########################################################

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
        database_url: Opaque::new(INTEGRATION_DATABASE_URL.to_string()),
        access_token_secret: Opaque::new(BASE64_STANDARD_NO_PAD.encode("hello-world")),
    };

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(config.database_url.extract_inner())
        .await
        .map_err(|e| anyhow::anyhow!("Failed to establish connection to database: {e}"))?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run database migrations: {e}"))?;

    let account_repository = PostgresAccountRepository::from(pool.clone());
    let access_token_repository = PostgresAccessTokenRepository::from(pool.clone());
    let mailing_service = FakeMailingService::new();

    let app = app_router(
        &config,
        account_repository,
        access_token_repository,
        mailing_service.clone(),
    )
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
    verification_secrets: Arc<RwLock<HashMap<Email, String>>>,
}

impl FakeMailingService {
    fn new() -> Self {
        Self {
            verification_secrets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[allow(dead_code)]
    pub fn get_verification_secret(&self, email: &str) -> Result<Option<String>, anyhow::Error> {
        let email = Email::new(email).map_err(|_| anyhow!("failed to map str email to email"))?;
        let secret = self
            .verification_secrets
            .try_read()?
            .get(&email)
            .map(|v| v.to_owned());
        Ok(secret)
    }
}

#[async_trait]
impl MailingService for FakeMailingService {
    async fn send_email(&self, email: &Email, content: &str) -> Result<(), anyhow::Error> {
        self.verification_secrets
            .try_write()?
            .insert(email.clone(), content.to_owned());
        Ok(())
    }
}
