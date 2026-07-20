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
    #[serde(default)]
    pub conveyor: ConveyorConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "default_log_retained_days")]
    pub retained_days: u64,
    #[serde(default = "default_log_cleanup_interval_hours")]
    pub cleanup_interval_hours: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModbusConfig {
    #[serde(default = "default_modbus_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
    #[serde(default = "default_modbus_reconnect_delay_ms")]
    pub reconnect_delay_ms: u64,
    #[serde(default = "default_modbus_max_connect_attempts")]
    pub max_connect_attempts: usize,
    #[serde(default = "default_modbus_pool_max_size")]
    pub pool_max_size: usize,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ConveyorConfig {
    #[serde(default)]
    pub send_routes: Vec<ConveyorSendRoute>,
    #[serde(default)]
    pub can_putdown_checks: Vec<ConveyorReadCheck>,
    #[serde(default)]
    pub need_putdown_routes: Vec<ConveyorNeedPutdownRoute>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct ConveyorSendRoute {
    pub source: String,
    pub destination: String,
    pub device: String,
    #[serde(default = "default_modbus_slave_id")]
    pub slave_id: u8,
    #[serde(default)]
    pub function: ConveyorWriteFunction,
    pub register_address: u16,
    pub value: u16,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct ConveyorReadCheck {
    pub location: String,
    pub device: String,
    #[serde(default = "default_modbus_slave_id")]
    pub slave_id: u8,
    #[serde(default)]
    pub function: ConveyorReadFunction,
    pub register_address: u16,
    #[serde(default = "default_modbus_read_quantity")]
    pub quantity: u16,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct ConveyorNeedPutdownRoute {
    pub location: String,
    pub device: String,
    #[serde(default = "default_modbus_slave_id")]
    pub slave_id: u8,
    #[serde(default)]
    pub function: ConveyorWriteFunction,
    pub register_address: u16,
    pub value: u16,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum ConveyorWriteFunction {
    #[serde(rename = "0x05", alias = "write_single_coil")]
    WriteSingleCoil,
    #[serde(rename = "0x06", alias = "write_single_register")]
    WriteSingleRegister,
    #[serde(rename = "0x0F", alias = "write_multiple_coils")]
    WriteMultipleCoils,
    #[serde(rename = "0x10", alias = "write_multiple_registers")]
    WriteMultipleRegisters,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum ConveyorReadFunction {
    #[serde(rename = "0x01", alias = "read_coils")]
    ReadCoils,
    #[serde(rename = "0x02", alias = "read_discrete_inputs")]
    ReadDiscreteInputs,
    #[serde(rename = "0x03", alias = "read_holding_registers")]
    ReadHoldingRegisters,
    #[serde(rename = "0x04", alias = "read_input_registers")]
    ReadInputRegisters,
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

impl ConveyorConfig {
    pub fn find_send_route(&self, source: &str, destination: &str) -> Option<&ConveyorSendRoute> {
        self.send_routes
            .iter()
            .find(|route| route.source == source && route.destination == destination)
    }

    pub fn find_can_putdown_check(&self, location: &str) -> Option<&ConveyorReadCheck> {
        self.can_putdown_checks
            .iter()
            .find(|check| check.location == location)
    }

    pub fn find_need_putdown_route(&self, location: &str) -> Option<&ConveyorNeedPutdownRoute> {
        self.need_putdown_routes
            .iter()
            .find(|route| route.location == location)
    }
}

impl ConveyorWriteFunction {
    pub fn as_str(self) -> &'static str {
        match self {
            ConveyorWriteFunction::WriteSingleCoil => "0x05",
            ConveyorWriteFunction::WriteSingleRegister => "0x06",
            ConveyorWriteFunction::WriteMultipleCoils => "0x0F",
            ConveyorWriteFunction::WriteMultipleRegisters => "0x10",
        }
    }
}

impl Default for ConveyorWriteFunction {
    fn default() -> Self {
        Self::WriteSingleRegister
    }
}

impl ConveyorReadFunction {
    pub fn as_str(self) -> &'static str {
        match self {
            ConveyorReadFunction::ReadCoils => "0x01",
            ConveyorReadFunction::ReadDiscreteInputs => "0x02",
            ConveyorReadFunction::ReadHoldingRegisters => "0x03",
            ConveyorReadFunction::ReadInputRegisters => "0x04",
        }
    }
}

impl Default for ConveyorReadFunction {
    fn default() -> Self {
        Self::ReadHoldingRegisters
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

fn default_modbus_read_quantity() -> u16 {
    1
}

#[cfg(test)]
mod tests {
    use super::{ConveyorConfig, LoggingConfig, ModbusConfig, ServerConfig};

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
    fn conveyor_send_route_defaults_to_slave_id_one() {
        let route: super::ConveyorSendRoute = config::Config::builder()
            .add_source(config::File::from_str(
                r#"
                source = "5104-1-1-1"
                destination = "5104-1-1-1"
                device = "192.168.70.102:2000"
                register_address = 10
                value = 6
                "#,
                config::FileFormat::Toml,
            ))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        assert_eq!(route.slave_id, 1);
    }

    #[test]
    fn conveyor_need_putdown_route_defaults_to_slave_id_one_and_single_register_write() {
        let route: super::ConveyorNeedPutdownRoute = config::Config::builder()
            .add_source(config::File::from_str(
                r#"
                location = "5107-1-1-1"
                device = "192.168.70.100:2000"
                register_address = 16
                value = 6
                "#,
                config::FileFormat::Toml,
            ))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        assert_eq!(route.slave_id, 1);
        assert_eq!(route.function.as_str(), "0x06");
        assert_eq!(route.register_address, 16);
        assert_eq!(route.value, 6);
    }

    #[test]
    fn conveyor_config_resolves_send_task_write_value_and_register() {
        let config = ConveyorConfig {
            send_routes: vec![super::ConveyorSendRoute {
                source: "5104-1-1-1".to_string(),
                destination: "5104-1-1-1".to_string(),
                device: "192.168.70.102:2000".to_string(),
                slave_id: 1,
                function: super::ConveyorWriteFunction::WriteSingleRegister,
                register_address: 10,
                value: 6,
            }],
            can_putdown_checks: Vec::new(),
            need_putdown_routes: Vec::new(),
        };

        let route = config.find_send_route("5104-1-1-1", "5104-1-1-1").unwrap();

        assert_eq!(route.value, 6);
        assert_eq!(route.device, "192.168.70.102:2000");
        assert_eq!(route.slave_id, 1);
        assert_eq!(route.function.as_str(), "0x06");
        assert_eq!(route.register_address, 10);
    }

    #[test]
    fn conveyor_read_check_defaults_to_holding_register_len_one_and_value_six() {
        let check: super::ConveyorReadCheck = config::Config::builder()
            .add_source(config::File::from_str(
                r#"
                location = "5107-1-1-1"
                device = "192.168.70.100:2000"
                register_address = 4
                "#,
                config::FileFormat::Toml,
            ))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        assert_eq!(check.slave_id, 1);
        assert_eq!(check.function.as_str(), "0x03");
        assert_eq!(check.quantity, 1);
    }

    #[test]
    fn repository_config_contains_legacy_conveyor_write_example() {
        let app_config: super::AppConfig = config::Config::builder()
            .add_source(config::File::from_str(
                include_str!("../../../config.toml"),
                config::FileFormat::Toml,
            ))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        let route = app_config
            .conveyor
            .find_send_route("5104-1-1-1", "5104-1-1-1")
            .unwrap();

        assert_eq!(route.value, 6);
        assert_eq!(route.device, "192.168.70.102:2000");
        assert_eq!(route.slave_id, 1);
        assert_eq!(route.function.as_str(), "0x06");
        assert_eq!(route.register_address, 10);
    }

    #[test]
    fn repository_config_contains_localhost_test_route() {
        let app_config: super::AppConfig = config::Config::builder()
            .add_source(config::File::from_str(
                include_str!("../../../config.toml"),
                config::FileFormat::Toml,
            ))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        let route = app_config
            .conveyor
            .find_send_route("TEST-SOURCE", "TEST-DEST")
            .unwrap();

        assert_eq!(route.device, "127.0.0.1:5000");
        assert_eq!(route.function.as_str(), "0x06");
        assert_eq!(route.register_address, 10);
        assert_eq!(route.value, 6);
    }

    #[test]
    fn repository_config_contains_legacy_can_putdown_mapping() {
        let app_config: super::AppConfig = config::Config::builder()
            .add_source(config::File::from_str(
                include_str!("../../../config.toml"),
                config::FileFormat::Toml,
            ))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        let check = app_config
            .conveyor
            .find_can_putdown_check("5107-1-1-1")
            .unwrap();

        assert_eq!(check.location, "5107-1-1-1");
        assert_eq!(check.device, "192.168.70.100:2000");
        assert_eq!(check.function.as_str(), "0x03");
        assert_eq!(check.register_address, 4);
    }

    #[test]
    fn repository_config_contains_legacy_need_putdown_mapping() {
        let app_config: super::AppConfig = config::Config::builder()
            .add_source(config::File::from_str(
                include_str!("../../../config.toml"),
                config::FileFormat::Toml,
            ))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();

        let route = app_config
            .conveyor
            .find_need_putdown_route("5107-1-1-1")
            .unwrap();

        assert_eq!(route.location, "5107-1-1-1");
        assert_eq!(route.device, "192.168.70.100:2000");
        assert_eq!(route.function.as_str(), "0x06");
        assert_eq!(route.register_address, 16);
        assert_eq!(route.value, 6);
    }
}
