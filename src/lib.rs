use serde::{Deserialize, Serialize};
use std::{
    env::{self, VarError},
    str::FromStr,
};

use axum::{Json, Router, routing::get};
use tracing::Level;

pub fn app_router() -> Router {
    Router::new().route("/health", get(healthcheck))
}

#[derive(Serialize, Deserialize)]
pub struct Healthcheck {
    pub ok: bool,
}
async fn healthcheck() -> Json<Healthcheck> {
    Json(Healthcheck { ok: true })
}

pub struct Config {
    pub port: u16,
    pub log_level: Level,
}

impl Config {
    pub fn build() -> Result<Config, anyhow::Error> {
        let mut errors: Vec<String> = vec![];
        let port = match parse_env_variable("PORT") {
            Ok(v) => v.unwrap_or(3000_u16),
            Err(e) => {
                errors.push(e.to_string());
                3000
            }
        };
        // `LOG_LEVEL` has priority over `RUST_LOG`
        let log_level = match parse_env_variable::<Level>("LOG_LEVEL") {
            Ok(v) => v
                .or_else(|| parse_env_variable::<Level>("RUST_LOG").unwrap_or(None))
                .unwrap_or(Level::INFO),
            Err(e) => {
                errors.push(e.to_string());
                Level::INFO
            }
        };
        if !errors.is_empty() {
            return Err(anyhow::anyhow!(errors.join(", ")));
        }
        Ok(Config { port, log_level })
    }
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
    .map_err(|e| anyhow::anyhow!("[{key}]: {e}"))
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
