use deadpool::managed::{self, RecycleError};
use std::{fmt, net::SocketAddr, time::Duration};
use tokio::time::{sleep, timeout};
use tokio_modbus::{client::Context, prelude::*};
use tracing::{error, info};

pub type Pool = managed::Pool<ModbusManager>;

#[derive(Clone, Debug)]
pub struct ModbusManager {
    pub addr: String,
    pub slave_id: u8,
    pub connect_timeout: Duration,
    pub reconnect_delay: Duration,
    pub max_connect_attempts: usize,
}

pub struct ManagedConnection {
    pub addr: String,
    pub slave_id: u8,
    pub context: Context,
    pub status: bool,
    disconnected_logged: bool,
}

#[derive(Debug)]
pub enum Error {
    InvalidAddress(String),
    ConnectFailed(String, String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidAddress(addr) => write!(f, "invalid socket address: {addr}"),
            Error::ConnectFailed(addr, err) => {
                write!(f, "failed to connect modbus tcp address {addr}: {err}")
            }
        }
    }
}

impl std::error::Error for Error {}

impl managed::Manager for ModbusManager {
    type Type = ManagedConnection;
    type Error = Error;

    async fn create(&self) -> Result<ManagedConnection, Error> {
        let socket_addr = self.parse_socket_addr()?;

        let max_connect_attempts = self.max_connect_attempts.max(1);
        let mut last_error = None;
        for attempt in 1..=max_connect_attempts {
            match timeout(self.connect_timeout, tcp::connect(socket_addr)).await {
                Ok(Ok(mut context)) => {
                    context.set_slave(Slave(self.slave_id));
                    info!(
                        modbus_addr = %self.addr,
                        slave_id = self.slave_id,
                        "Modbus TCP连接成功 modbus_addr={} slave_id={}",
                        self.addr,
                        self.slave_id
                    );
                    return Ok(ManagedConnection {
                        addr: self.addr.clone(),
                        slave_id: self.slave_id,
                        context,
                        status: true,
                        disconnected_logged: false,
                    });
                }
                Ok(Err(err)) => {
                    let message = err.to_string();
                    error!(
                        modbus_addr = %self.addr,
                        attempt,
                        max_attempts = max_connect_attempts,
                        error = %message,
                        "Modbus TCP连接失败 modbus_addr={} attempt={} max_attempts={} error={}",
                        self.addr,
                        attempt,
                        max_connect_attempts,
                        message
                    );
                    last_error = Some(message);
                }
                Err(_) => {
                    error!(
                        modbus_addr = %self.addr,
                        attempt,
                        max_attempts = max_connect_attempts,
                        "Modbus TCP连接超时 modbus_addr={} attempt={} max_attempts={}",
                        self.addr,
                        attempt,
                        max_connect_attempts
                    );
                    last_error = Some("timeout".to_string());
                }
            }

            if attempt < max_connect_attempts {
                sleep(self.reconnect_delay).await;
            }
        }

        let last_error = last_error.unwrap_or_else(|| "unknown error".to_string());
        error!(
            modbus_addr = %self.addr,
            max_attempts = max_connect_attempts,
            error = %last_error,
            "Modbus TCP重试后仍连接失败 modbus_addr={} max_attempts={} error={}",
            self.addr,
            max_connect_attempts,
            last_error
        );

        Err(Error::ConnectFailed(self.addr.clone(), last_error))
    }

    async fn recycle(
        &self,
        conn: &mut ManagedConnection,
        _: &managed::Metrics,
    ) -> managed::RecycleResult<Error> {
        if conn.status {
            Ok(())
        } else {
            conn.log_disconnect("unhealthy");
            if let Err(err) = conn.context.disconnect().await {
                error!(
                    modbus_addr = %self.addr,
                    error = ?err,
                    "关闭Modbus TCP连接失败 modbus_addr={} error={:?}",
                    self.addr,
                    err
                );
            }
            Err(RecycleError::Message("can't recycle".into()))
        }
    }

    fn detach(&self, obj: &mut Self::Type) {
        obj.log_disconnect("detached");
    }
}

impl ModbusManager {
    fn parse_socket_addr(&self) -> Result<SocketAddr, Error> {
        self.addr
            .parse::<SocketAddr>()
            .map_err(|_| Error::InvalidAddress(self.addr.clone()))
    }
}

impl ManagedConnection {
    fn log_disconnect(&mut self, reason: &'static str) {
        if !self.disconnected_logged {
            info!(
                modbus_addr = %self.addr,
                reason,
                "Modbus TCP连接已断开 modbus_addr={} reason={}",
                self.addr,
                reason
            );
            self.disconnected_logged = true;
        }
    }
}

impl Drop for ManagedConnection {
    fn drop(&mut self) {
        self.log_disconnect("dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, ModbusManager};
    use deadpool::managed::Manager;
    use std::time::Duration;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn create_rejects_invalid_socket_address() {
        let manager = test_manager("not-an-address");

        let err = match manager.create().await {
            Ok(_) => panic!("expected invalid address"),
            Err(err) => err,
        };

        assert!(matches!(err, Error::InvalidAddress(addr) if addr == "not-an-address"));
    }

    #[tokio::test]
    async fn create_returns_error_after_connect_attempts_fail() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let manager = test_manager(&addr.to_string());

        let err = match manager.create().await {
            Ok(_) => panic!("expected connect failure"),
            Err(err) => err,
        };

        assert!(matches!(err, Error::ConnectFailed(_, _)));
    }

    fn test_manager(addr: &str) -> ModbusManager {
        ModbusManager {
            addr: addr.to_string(),
            slave_id: 1,
            connect_timeout: Duration::from_millis(10),
            reconnect_delay: Duration::from_millis(1),
            max_connect_attempts: 1,
        }
    }
}
