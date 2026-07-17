use config::{Config, ConfigError};
use serde::Deserialize;
use std::sync::LazyLock;
use tracing::debug;

pub static APP_CONFIG: LazyLock<AppConfig> = LazyLock::new(|| {
    let config = load_config().expect("failed to load application config");
    debug!("加载配置成功：{:#?}", config);
    config
});

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "default_log_retained_days")]
    pub retained_days: u64,
    #[serde(default = "default_log_cleanup_interval_hours")]
    pub cleanup_interval_hours: u64,
}

pub fn load_config() -> Result<AppConfig, ConfigError> {
    let settings = Config::builder()
        .add_source(config::File::with_name("./config"))
        .build()?;
    settings.try_deserialize::<AppConfig>()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: default_listen_addr(),
        }
    }
}

fn default_listen_addr() -> String {
    "0.0.0.0:3000".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            retained_days: default_log_retained_days(),
            cleanup_interval_hours: default_log_cleanup_interval_hours(),
        }
    }
}

fn default_log_retained_days() -> u64 {
    30
}

fn default_log_cleanup_interval_hours() -> u64 {
    24
}

#[cfg(test)]
mod tests {
    use super::{LoggingConfig, ServerConfig};

    #[test]
    fn server_config_uses_default_listen_addr() {
        let config = ServerConfig::default();

        assert_eq!(config.listen_addr, "0.0.0.0:3000");
    }

    #[test]
    fn logging_config_uses_default_retention_settings() {
        let config = LoggingConfig::default();

        assert_eq!(config.retained_days, 30);
        assert_eq!(config.cleanup_interval_hours, 24);
    }
}
