use std::{
    env::{self, VarError},
    str::FromStr,
};

use axum::{Router, routing::get};
use tracing::{Level, info};

pub fn app_router() -> Router {
    Router::new().route("/health", get(healthcheck))
}

async fn healthcheck() -> &'static str {
    info!("Healthcheck called");
    "OK"
}

pub struct Config {
    pub port: u16,
    pub log_level: Level,
}

impl Config {
    pub fn build() -> Result<Config, anyhow::Error> {
        let port = parse_env_variable_with_default("PORT", 3000u16)?;
        // `LOG_LEVEL` has priority over `RUST_LOG`
        let log_level = parse_env_variable::<Level>("LOG_LEVEL")
            .unwrap_or(None)
            .or_else(|| parse_env_variable::<Level>("RUST_LOG").unwrap_or(None))
            .unwrap_or(Level::INFO);
        Ok(Config { port, log_level })
    }
}

fn parse_env_variable_with_default<T>(key: &str, default: T) -> Result<T, anyhow::Error>
where
    T: FromStr,
    <T as FromStr>::Err: std::error::Error + Send + Sync + 'static,
{
    parse_env_variable(key).map(|v| v.unwrap_or(default))
}

fn parse_env_variable<T>(key: &str) -> Result<Option<T>, anyhow::Error>
where
    T: FromStr,
    <T as FromStr>::Err: std::error::Error + Send + Sync + 'static,
{
    match read_optional_env_variable(key)? {
        Some(v) => v.parse::<T>().map_err(anyhow::Error::from).map(|v| Some(v)),
        None => Ok(None),
    }
    .map_err(|e| anyhow::anyhow!("{key}: {e}"))
}

fn read_optional_env_variable(key: &str) -> Result<Option<String>, anyhow::Error> {
    match env::var(key) {
        Ok(v) => {
            if v.is_empty() {
                Ok(None)
            } else {
                Ok(Some(v))
            }
        }
        Err(e) => {
            if e == VarError::NotPresent {
                Ok(None)
            } else {
                Err(e.into())
            }
        }
    }
}
