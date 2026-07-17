use config::{Config, ConfigError};
use serde::Deserialize;
use std::sync::LazyLock;
use std::time::Duration;
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
    #[serde(default)]
    pub modbus: ModbusConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "default_log_retained_days")]
    pub retained_days: u64,
    #[serde(default = "default_log_cleanup_interval_hours")]
    pub cleanup_interval_hours: u64,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct ModbusDevice {
    pub address: String,
    #[serde(default = "default_modbus_slave_id")]
    pub slave_id: u8,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModbusConfig {
    #[serde(default)]
    pub devices: Vec<ModbusDevice>,
    #[serde(default = "default_modbus_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    #[serde(default = "default_modbus_reconnect_delay_ms")]
    pub reconnect_delay_ms: u64,
    #[serde(default = "default_modbus_max_connect_attempts")]
    pub max_connect_attempts: usize,
    #[serde(default = "default_modbus_pool_max_size")]
    pub pool_max_size: usize,
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

impl Default for ModbusConfig {
    fn default() -> Self {
        Self {
            devices: Vec::new(),
            connect_timeout_ms: default_modbus_connect_timeout_ms(),
            reconnect_delay_ms: default_modbus_reconnect_delay_ms(),
            max_connect_attempts: default_modbus_max_connect_attempts(),
            pool_max_size: default_modbus_pool_max_size(),
        }
    }
}

impl ModbusConfig {
    pub fn connect_timeout(&self) -> Duration {
        Duration::from_millis(self.connect_timeout_ms)
    }

    pub fn reconnect_delay(&self) -> Duration {
        Duration::from_millis(self.reconnect_delay_ms)
    }

    pub fn max_connect_attempts(&self) -> usize {
        self.max_connect_attempts.max(1)
    }

    pub fn pool_max_size(&self) -> usize {
        self.pool_max_size.max(1)
    }
}

fn default_modbus_connect_timeout_ms() -> u64 {
    1000
}

fn default_modbus_reconnect_delay_ms() -> u64 {
    500
}

fn default_modbus_max_connect_attempts() -> usize {
    3
}

fn default_modbus_pool_max_size() -> usize {
    1
}

fn default_modbus_slave_id() -> u8 {
    1
}

#[cfg(test)]
mod tests {
    use super::{LoggingConfig, ModbusConfig, ModbusDevice, ServerConfig};

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

    #[test]
    fn modbus_config_uses_default_pool_settings() {
        let config = ModbusConfig::default();

        assert_eq!(config.devices, Vec::new());
        assert_eq!(config.connect_timeout_ms, 1000);
        assert_eq!(config.reconnect_delay_ms, 500);
        assert_eq!(config.max_connect_attempts(), 3);
        assert_eq!(config.pool_max_size(), 1);
    }

    #[test]
    fn modbus_config_clamps_pool_and_retry_counts() {
        let config = ModbusConfig {
            pool_max_size: 0,
            max_connect_attempts: 0,
            ..ModbusConfig::default()
        };

        assert_eq!(config.pool_max_size(), 1);
        assert_eq!(config.max_connect_attempts(), 1);
    }

    #[test]
    fn modbus_device_defaults_to_slave_id_one() {
        let device: ModbusDevice = config::Config::builder()
            .add_source(config::File::from_str(
                r#"address = "127.0.0.1:502""#,
                config::FileFormat::Toml,
            ))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        assert_eq!(device.slave_id, 1);
    }
}
