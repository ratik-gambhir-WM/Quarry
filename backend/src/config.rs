use std::{
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use axum::http::HeaderValue;

const DEFAULT_API_PORT: u16 = 3001;
const DEFAULT_REQUEST_TIMEOUT_SECONDS: u64 = 120;
const DEFAULT_CORS_ORIGINS: &str = "http://127.0.0.1:1420,http://localhost:1420";

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub bind_address: SocketAddr,
    pub cors_origins: Vec<HeaderValue>,
    pub request_timeout: Duration,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        let host = env::var("PATHFINDER_API_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let host = host
            .parse::<IpAddr>()
            .map_err(|err| format!("PATHFINDER_API_HOST must be an IP address: {err}"))?;
        let port = parse_env("PATHFINDER_API_PORT", DEFAULT_API_PORT)?;
        let timeout_seconds = parse_env(
            "PATHFINDER_REQUEST_TIMEOUT_SECONDS",
            DEFAULT_REQUEST_TIMEOUT_SECONDS,
        )?;
        if timeout_seconds == 0 {
            return Err("PATHFINDER_REQUEST_TIMEOUT_SECONDS must be greater than zero".to_string());
        }

        let cors_origins = env::var("PATHFINDER_CORS_ORIGINS")
            .unwrap_or_else(|_| DEFAULT_CORS_ORIGINS.to_string())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(|origin| {
                HeaderValue::from_str(origin)
                    .map_err(|err| format!("invalid CORS origin `{origin}`: {err}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if cors_origins.is_empty() {
            return Err("PATHFINDER_CORS_ORIGINS must include at least one origin".to_string());
        }

        Ok(Self {
            bind_address: SocketAddr::new(host, port),
            cors_origins,
            request_timeout: Duration::from_secs(timeout_seconds),
        })
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bind_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_API_PORT),
            cors_origins: DEFAULT_CORS_ORIGINS
                .split(',')
                .map(|origin| HeaderValue::from_str(origin).expect("default CORS origin is valid"))
                .collect(),
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECONDS),
        }
    }
}

fn parse_env<T>(name: &str, default: T) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => value
            .parse::<T>()
            .map_err(|err| format!("invalid {name}: {err}")),
        _ => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_local_and_has_a_timeout() {
        let config = AppConfig::default();

        assert_eq!(config.bind_address, "127.0.0.1:3001".parse().unwrap());
        assert_eq!(config.request_timeout, Duration::from_secs(120));
        assert_eq!(config.cors_origins.len(), 2);
    }
}
