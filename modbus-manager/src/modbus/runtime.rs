use crate::{
    config::{APP_CONFIG, ModbusConfig, ModbusDevice},
    modbus::pool::{ModbusManager, Pool},
};
use deadpool::managed::Object;
use std::{collections::HashMap, hash::Hash};
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
    devices: Vec<ModbusDevice>,
    runtimes: HashMap<ModbusTarget, ModbusRuntime>,
    config: ModbusConfig,
}

pub fn build_modbus_service() -> ModbusService {
    ModbusService::new(&APP_CONFIG.modbus)
}

impl ModbusService {
    pub fn new(config: &ModbusConfig) -> Self {
        let runtimes = config
            .devices
            .iter()
            .map(|device| {
                let runtime = build_modbus_runtime(device, config);
                (
                    ModbusTarget {
                        address: runtime.address.clone(),
                        slave_id: runtime.slave_id,
                    },
                    runtime,
                )
            })
            .collect();

        Self {
            devices: config.devices.clone(),
            runtimes,
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

    pub fn configured_device_count(&self) -> usize {
        self.runtimes.len()
    }

    fn runtime(&self, address: &str, slave_id: u8) -> ModbusRuntime {
        let target = ModbusTarget {
            address: address.to_string(),
            slave_id,
        };
        if let Some(runtime) = self.runtimes.get(&target) {
            runtime.clone()
        } else {
            build_modbus_runtime(
                &ModbusDevice {
                    address: address.to_string(),
                    slave_id,
                },
                &self.config,
            )
        }
    }
}

fn build_modbus_runtime(device: &ModbusDevice, config: &ModbusConfig) -> ModbusRuntime {
    let address = device.address.clone();
    ModbusRuntime {
        pool: build_modbus_pool(device, config),
        address,
        slave_id: device.slave_id,
    }
}

fn build_modbus_pool(device: &ModbusDevice, config: &ModbusConfig) -> Pool {
    Pool::builder(ModbusManager {
        addr: device.address.clone(),
        slave_id: device.slave_id,
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
        devices = ?service.devices,
        "Modbus管理器已启动 devices={:?}",
        service.devices
    );

    if service.configured_device_count() == 0 {
        warn!("未配置固定Modbus设备，仍可通过传入地址动态建立连接");
    }
}

#[derive(Debug)]
pub enum ModbusError {
    ConnectionPool(String, String),
}

impl std::fmt::Display for ModbusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModbusError::ConnectionPool(addr, err) => {
                write!(f, "failed to acquire modbus connection {addr}: {err}")
            }
        }
    }
}

impl std::error::Error for ModbusError {}

#[cfg(test)]
mod tests {
    use super::ModbusService;
    use crate::config::{ModbusConfig, ModbusDevice};

    #[test]
    fn service_builds_runtime_for_each_configured_device() {
        let config = ModbusConfig {
            devices: vec![
                ModbusDevice {
                    address: "127.0.0.1:502".to_string(),
                    slave_id: 1,
                },
                ModbusDevice {
                    address: "127.0.0.1:503".to_string(),
                    slave_id: 2,
                },
            ],
            ..ModbusConfig::default()
        };

        let service = ModbusService::new(&config);

        assert_eq!(service.configured_device_count(), 2);
        assert_eq!(
            service
                .runtimes
                .get(&super::ModbusTarget {
                    address: "127.0.0.1:503".to_string(),
                    slave_id: 2
                })
                .unwrap()
                .slave_id,
            2
        );
    }

    #[test]
    fn service_keeps_same_address_with_different_slave_ids_separate() {
        let config = ModbusConfig {
            devices: vec![
                ModbusDevice {
                    address: "127.0.0.1:502".to_string(),
                    slave_id: 1,
                },
                ModbusDevice {
                    address: "127.0.0.1:502".to_string(),
                    slave_id: 2,
                },
            ],
            ..ModbusConfig::default()
        };

        let service = ModbusService::new(&config);

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
