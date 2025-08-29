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
    pub database_url: String,
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
        let database_ssl_require = match parse_env_variable::<bool>("DATABASE_SSL_REQUIRE") {
            Ok(v) => v.unwrap_or(false),
            Err(e) => {
                errors.push(e.to_string());
                false
            }
        };
        let mut database_url = match parse_required_env_variable::<String>("DATABASE_URL") {
            Ok(v) => v,
            Err(e) => {
                errors.push(e.to_string());
                "".to_string()
            }
        };
        if database_ssl_require {
            database_url += "?ssl_mode=require"
        }
        if !errors.is_empty() {
            return Err(anyhow::anyhow!(errors.join(", ")));
        }
        Ok(Config {
            port,
            log_level,
            database_url,
        })
    }
}

fn parse_required_env_variable<T>(key: &str) -> Result<T, anyhow::Error>
where
    T: FromStr,
    <T as FromStr>::Err: std::error::Error + Send + Sync + 'static,
{
    match parse_env_variable::<T>(key)? {
        Some(v) => Ok(v),
        None => Err(anyhow::anyhow!("[{key}]: must be specified and non empty")),
    }
}

fn parse_env_variable<T>(key: &str) -> Result<Option<T>, anyhow::Error>
where
    T: FromStr,
    <T as FromStr>::Err: std::error::Error + Send + Sync + 'static,
{
    match read_env_variable(key)? {
        Some(v) => {
            if v.is_empty() {
                Ok(None)
            } else {
                v.parse::<T>().map_err(anyhow::Error::from).map(|v| Some(v))
            }
        }
        None => Ok(None),
    }
    .map_err(|e| anyhow::anyhow!("[{key}]: {e}"))
}

fn read_env_variable(key: &str) -> Result<Option<String>, anyhow::Error> {
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
