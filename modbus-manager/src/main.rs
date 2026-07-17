use modbus_manager::{
    config::APP_CONFIG,
    modbus::{build_modbus_service, log_startup},
    observability::{log_retention, telemetry},
    web,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = telemetry::init_log();
    log_retention::spawn_cleanup_task(APP_CONFIG.logging.clone());
    let modbus_service = build_modbus_service();
    log_startup(&modbus_service);
    web::serve(APP_CONFIG.server.listen_addr.clone()).await?;
    Ok(())
}
