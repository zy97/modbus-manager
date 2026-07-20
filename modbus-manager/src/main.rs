use modbus_manager::{
    config::APP_CONFIG,
    modbus::{build_modbus_service, log_startup},
    observability::{log_retention, telemetry},
    web::{self, spawn_can_putdown_webhook_monitor},
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = telemetry::init_log();
    log_retention::spawn_cleanup_task(APP_CONFIG.logging.clone());
    let modbus_service = Arc::new(build_modbus_service());
    log_startup(&modbus_service);
    let _monitor =
        spawn_can_putdown_webhook_monitor(modbus_service.clone(), APP_CONFIG.conveyor.clone());
    web::serve(
        APP_CONFIG.server.listen_addr.clone(),
        modbus_service,
        APP_CONFIG.conveyor.clone(),
    )
    .await?;
    Ok(())
}
