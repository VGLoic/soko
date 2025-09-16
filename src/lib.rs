use std::{
    env::{self, VarError},
    str::FromStr,
};
use tracing::Level;

pub mod newtypes;
pub mod routes;
pub mod third_party;
use newtypes::OpaqueString;

pub struct Config {
    pub port: u16,
    pub log_level: Level,
    pub database_url: OpaqueString,
    pub access_token_secret: OpaqueString,
}

impl Config {
    pub fn parse_environment() -> Result<Config, anyhow::Error> {
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
        let database_url = match parse_required_env_variable::<String>("DATABASE_URL") {
            Ok(v) => v,
            Err(e) => {
                errors.push(e.to_string());
                "".to_string()
            }
        };

        let access_token_secret = match parse_required_env_variable::<String>("ACCESS_TOKEN_SECRET")
        {
            Ok(v) => v,
            Err(e) => {
                errors.push(e.to_string());
                "".to_string()
            }
        };

        if !errors.is_empty() {
            return Err(anyhow::anyhow!(errors.join(", ")));
        }
        Ok(Config {
            port,
            log_level,
            database_url: OpaqueString::new(database_url),
            access_token_secret: OpaqueString::new(access_token_secret),
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
    fn map_err<E>(key: &str, e: E) -> anyhow::Error
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        anyhow::anyhow!("[{key}]: {e}")
    }

    let env_value = match env::var(key) {
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
                Err(map_err(key, e))
            }
        }
    }?;
    env_value
        .map(|v| v.parse::<T>().map_err(|e| map_err(key, e)))
        .transpose()
}
