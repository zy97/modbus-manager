use crate::{
    config::{APP_CONFIG, ModbusConfig},
    modbus::pool::{ModbusManager, Pool},
};
use deadpool::managed::Object;
use std::{collections::HashMap, hash::Hash, sync::RwLock};
use tokio_modbus::prelude::*;
use tracing::{info, warn};

#[derive(Clone)]
pub struct ModbusRuntime {
    pub address: String,
    pub slave_id: u8,
    pub pool: Pool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ModbusTarget {
    address: String,
    slave_id: u8,
}

pub struct ModbusService {
    runtimes: RwLock<HashMap<ModbusTarget, ModbusRuntime>>,
    config: ModbusConfig,
}

pub fn build_modbus_service() -> ModbusService {
    ModbusService::new(&APP_CONFIG.modbus)
}

impl ModbusService {
    pub fn new(config: &ModbusConfig) -> Self {
        Self {
            runtimes: RwLock::new(HashMap::new()),
            config: config.clone(),
        }
    }

    pub async fn connection(
        &self,
        address: &str,
        slave_id: u8,
    ) -> Result<Object<ModbusManager>, ModbusError> {
        let runtime = self.runtime(address, slave_id);
        runtime
            .pool
            .get()
            .await
            .map_err(|err| ModbusError::ConnectionPool(address.to_string(), err.to_string()))
    }

    pub async fn write_single_register(
        &self,
        address: &str,
        slave_id: u8,
        register_address: u16,
        value: u16,
    ) -> Result<(), ModbusError> {
        let mut connection = self.connection(address, slave_id).await?;

        match connection
            .context
            .write_single_register(register_address, value)
            .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(exception)) => Err(ModbusError::WriteException(
                address.to_string(),
                register_address,
                value,
                format!("{exception:?}"),
            )),
            Err(err) => {
                connection.status = false;
                Err(ModbusError::WriteTransport(
                    address.to_string(),
                    register_address,
                    value,
                    err.to_string(),
                ))
            }
        }
    }

    pub async fn read_holding_registers(
        &self,
        address: &str,
        slave_id: u8,
        register_address: u16,
        quantity: u16,
    ) -> Result<Vec<u16>, ModbusError> {
        let mut connection = self.connection(address, slave_id).await?;

        match connection
            .context
            .read_holding_registers(register_address, quantity)
            .await
        {
            Ok(Ok(values)) => Ok(values),
            Ok(Err(exception)) => Err(ModbusError::ReadException(
                address.to_string(),
                register_address,
                quantity,
                format!("{exception:?}"),
            )),
            Err(err) => {
                connection.status = false;
                Err(ModbusError::ReadTransport(
                    address.to_string(),
                    register_address,
                    quantity,
                    err.to_string(),
                ))
            }
        }
    }

    pub fn configured_device_count(&self) -> usize {
        self.runtimes
            .read()
            .expect("modbus runtime lock poisoned")
            .len()
    }

    fn runtime(&self, address: &str, slave_id: u8) -> ModbusRuntime {
        let target = ModbusTarget {
            address: address.to_string(),
            slave_id,
        };
        if let Some(runtime) = self
            .runtimes
            .read()
            .expect("modbus runtime lock poisoned")
            .get(&target)
        {
            runtime.clone()
        } else {
            let mut runtimes = self.runtimes.write().expect("modbus runtime lock poisoned");
            runtimes
                .entry(target)
                .or_insert_with(|| build_modbus_runtime(address, slave_id, &self.config))
                .clone()
        }
    }
}

fn build_modbus_runtime(address: &str, slave_id: u8, config: &ModbusConfig) -> ModbusRuntime {
    ModbusRuntime {
        pool: build_modbus_pool(address, slave_id, config),
        address: address.to_string(),
        slave_id,
    }
}

fn build_modbus_pool(address: &str, slave_id: u8, config: &ModbusConfig) -> Pool {
    Pool::builder(ModbusManager {
        addr: address.to_string(),
        slave_id,
        connect_timeout: config.connect_timeout(),
        reconnect_delay: config.reconnect_delay(),
        max_connect_attempts: config.max_connect_attempts(),
    })
    .max_size(config.pool_max_size())
    .build()
    .expect("failed to build modbus pool")
}

pub fn log_startup(service: &ModbusService) {
    info!(
        configured_device_count = service.configured_device_count(),
        "Modbus管理器已启动 configured_device_count={}",
        service.configured_device_count()
    );

    if service.configured_device_count() == 0 {
        warn!("未配置固定Modbus设备池，将根据业务配置中的设备地址动态建立连接");
    }
}

#[derive(Debug)]
pub enum ModbusError {
    ConnectionPool(String, String),
    WriteTransport(String, u16, u16, String),
    WriteException(String, u16, u16, String),
    ReadTransport(String, u16, u16, String),
    ReadException(String, u16, u16, String),
}

impl std::fmt::Display for ModbusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModbusError::ConnectionPool(addr, err) => {
                write!(f, "failed to acquire modbus connection {addr}: {err}")
            }
            ModbusError::WriteTransport(addr, register_address, value, err) => write!(
                f,
                "failed to write modbus register {addr}:{register_address} value {value}: {err}"
            ),
            ModbusError::WriteException(addr, register_address, value, exception) => write!(
                f,
                "modbus exception writing register {addr}:{register_address} value {value}: {exception}"
            ),
            ModbusError::ReadTransport(addr, register_address, quantity, err) => write!(
                f,
                "failed to read modbus holding registers {addr}:{register_address} quantity {quantity}: {err}"
            ),
            ModbusError::ReadException(addr, register_address, quantity, exception) => write!(
                f,
                "modbus exception reading holding registers {addr}:{register_address} quantity {quantity}: {exception}"
            ),
        }
    }
}

impl std::error::Error for ModbusError {}

#[cfg(test)]
mod tests {
    use super::ModbusService;
    use crate::config::ModbusConfig;

    #[test]
    fn service_does_not_require_preconfigured_devices() {
        let config = ModbusConfig::default();

        let service = ModbusService::new(&config);

        assert_eq!(service.configured_device_count(), 0);
    }

    #[test]
    fn dynamic_runtime_is_cached_by_address_and_slave_id() {
        let config = ModbusConfig::default();
        let service = ModbusService::new(&config);

        let first = service.runtime("127.0.0.1:502", 1);
        let second = service.runtime("127.0.0.1:502", 1);
        let third = service.runtime("127.0.0.1:502", 2);

        assert_eq!(first.address, second.address);
        assert_eq!(first.slave_id, second.slave_id);
        assert_eq!(third.slave_id, 2);
        assert_eq!(service.configured_device_count(), 2);
    }

    #[tokio::test]
    async fn dynamic_connection_uses_same_pool_settings() {
        let config = ModbusConfig {
            connect_timeout_ms: 10,
            reconnect_delay_ms: 1,
            max_connect_attempts: 1,
            ..ModbusConfig::default()
        };
        let service = ModbusService::new(&config);

        let err = match service.connection("not-an-address", 1).await {
            Ok(_) => panic!("expected invalid address"),
            Err(err) => err,
        };

        assert!(
            err.to_string()
                .contains("failed to acquire modbus connection")
        );
    }
}
