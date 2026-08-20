use super::*;

#[test]
fn default_config_is_local_and_has_a_timeout() {
    let config = AppConfig::default();

    assert_eq!(config.bind_address, "127.0.0.1:3001".parse().unwrap());
    assert_eq!(config.request_timeout, Duration::from_secs(120));
    assert_eq!(config.cors_origins.len(), 2);
}
