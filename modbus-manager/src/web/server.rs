use crate::{
    config::{
        ConveyorConfig, ConveyorNeedPutdownRoute, ConveyorReadFunction, ConveyorSendRoute,
        ConveyorWriteFunction,
    },
    modbus::{ModbusError, ModbusService},
    web::{api_response::ApiResponse, http_trace::trace_http_request},
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::{net::TcpListener, signal};
use tracing::{error, info};

#[derive(Clone)]
pub struct AppState {
    modbus_service: Arc<ModbusService>,
    conveyor: ConveyorConfig,
}

pub fn build_router(modbus_service: Arc<ModbusService>, conveyor: ConveyorConfig) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/conveyor/write", post(write_conveyor))
        .route("/api/conveyor/can-putdown", get(can_putdown))
        .route("/api/conveyor/need-putdown", post(need_putdown))
        .layer(middleware::from_fn(trace_http_request))
        .with_state(AppState {
            modbus_service,
            conveyor,
        })
}

pub async fn serve(
    listen_addr: String,
    modbus_service: Arc<ModbusService>,
    conveyor: ConveyorConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(&listen_addr).await?;
    let local_addr = listener.local_addr()?;
    info!(
        listen_addr = %local_addr,
        "Modbus HTTP服务已监听 listen_addr={}",
        local_addr
    );

    axum::serve(
        listener,
        build_router(modbus_service, conveyor).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn health() -> Json<ApiResponse<&'static str>> {
    Json(ApiResponse {
        success: true,
        data: Some("ok"),
        message: "ok".to_string(),
    })
}

#[derive(Debug, Deserialize)]
pub struct ConveyorWriteRequest {
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Serialize)]
pub struct ConveyorWriteResponse {
    pub source: String,
    pub destination: String,
    pub device: String,
    pub slave_id: u8,
    pub function: String,
    pub register_address: u16,
    pub value: u16,
}

#[derive(Debug, Deserialize)]
pub struct ConveyorCanPutdownQuery {
    pub location: String,
}

#[derive(Debug, Deserialize)]
pub struct ConveyorNeedPutdownRequest {
    pub location: String,
}

#[derive(Debug, Serialize)]
pub struct ConveyorNeedPutdownResponse {
    pub location: String,
    pub device: String,
    pub slave_id: u8,
    pub function: String,
    pub register_address: u16,
    pub value: u16,
}

async fn write_conveyor(
    State(state): State<AppState>,
    Json(request): Json<ConveyorWriteRequest>,
) -> Result<Json<ApiResponse<ConveyorWriteResponse>>, ApiError> {
    let route = resolve_conveyor_write(&state.conveyor, &request.source, &request.destination)?;

    execute_conveyor_write(&state.modbus_service, &route).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(ConveyorWriteResponse {
            source: route.source,
            destination: route.destination,
            device: route.device,
            slave_id: route.slave_id,
            function: route.function.as_str().to_string(),
            register_address: route.register_address,
            value: route.value,
        }),
        message: "ok".to_string(),
    }))
}

async fn can_putdown(
    State(state): State<AppState>,
    Query(query): Query<ConveyorCanPutdownQuery>,
) -> Result<Json<ApiResponse<Vec<u16>>>, ApiError> {
    let check = resolve_can_putdown(&state.conveyor, &query.location)?;

    let values = execute_can_putdown_check(&state.modbus_service, &check).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(values),
        message: "ok".to_string(),
    }))
}

async fn need_putdown(
    State(state): State<AppState>,
    Json(request): Json<ConveyorNeedPutdownRequest>,
) -> Result<Json<ApiResponse<ConveyorNeedPutdownResponse>>, ApiError> {
    let route = resolve_need_putdown(&state.conveyor, &request.location)?;

    execute_need_putdown_write(&state.modbus_service, &route).await?;

    Ok(Json(ApiResponse {
        success: true,
        data: Some(ConveyorNeedPutdownResponse {
            location: route.location,
            device: route.device,
            slave_id: route.slave_id,
            function: route.function.as_str().to_string(),
            register_address: route.register_address,
            value: route.value,
        }),
        message: "ok".to_string(),
    }))
}

async fn execute_conveyor_write(
    modbus_service: &ModbusService,
    route: &ConveyorSendRoute,
) -> Result<(), ApiError> {
    match route.function {
        ConveyorWriteFunction::WriteSingleRegister => {
            modbus_service
                .write_single_register(
                    &route.device,
                    route.slave_id,
                    route.register_address,
                    route.value,
                )
                .await?;
            Ok(())
        }
        function => Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            format!(
                "unsupported conveyor modbus write function: {}",
                function.as_str()
            ),
        )),
    }
}

async fn execute_can_putdown_check(
    modbus_service: &ModbusService,
    check: &crate::config::ConveyorReadCheck,
) -> Result<Vec<u16>, ApiError> {
    match check.function {
        ConveyorReadFunction::ReadHoldingRegisters => {
            let values = modbus_service
                .read_holding_registers(
                    &check.device,
                    check.slave_id,
                    check.register_address,
                    check.quantity,
                )
                .await?;
            Ok(values)
        }
        function => Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("unsupported conveyor read function: {}", function.as_str()),
        )),
    }
}

async fn execute_need_putdown_write(
    modbus_service: &ModbusService,
    route: &ConveyorNeedPutdownRoute,
) -> Result<(), ApiError> {
    match route.function {
        ConveyorWriteFunction::WriteSingleRegister => {
            modbus_service
                .write_single_register(
                    &route.device,
                    route.slave_id,
                    route.register_address,
                    route.value,
                )
                .await?;
            Ok(())
        }
        function => Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            format!(
                "unsupported need-putdown modbus write function: {}",
                function.as_str()
            ),
        )),
    }
}

fn resolve_conveyor_write(
    conveyor: &ConveyorConfig,
    source: &str,
    destination: &str,
) -> Result<ConveyorSendRoute, ApiError> {
    conveyor
        .find_send_route(source, destination)
        .cloned()
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                format!("unknown conveyor route: {source}->{destination}"),
            )
        })
}

fn resolve_can_putdown(
    conveyor: &ConveyorConfig,
    location: &str,
) -> Result<crate::config::ConveyorReadCheck, ApiError> {
    conveyor
        .find_can_putdown_check(location)
        .cloned()
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                format!("unknown can-putdown location: {location}"),
            )
        })
}

fn resolve_need_putdown(
    conveyor: &ConveyorConfig,
    location: &str,
) -> Result<ConveyorNeedPutdownRoute, ApiError> {
    conveyor
        .find_need_putdown_route(location)
        .cloned()
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                format!("unknown need-putdown location: {location}"),
            )
        })
}

async fn shutdown_signal() {
    if let Err(err) = signal::ctrl_c().await {
        error!(
            error = ?err,
            "监听关闭信号失败 error={:?}",
            err
        );
    }
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl From<ModbusError> for ApiError {
    fn from(error: ModbusError) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ApiResponse::<()> {
            success: false,
            data: None,
            message: self.message,
        });

        (self.status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiError, resolve_can_putdown, resolve_conveyor_write, resolve_need_putdown};
    use crate::config::{
        ConveyorConfig, ConveyorNeedPutdownRoute, ConveyorReadCheck, ConveyorReadFunction,
        ConveyorSendRoute, ConveyorWriteFunction,
    };
    use axum::{Json, http::StatusCode, response::IntoResponse};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct ErrorBody {
        success: bool,
        data: Option<()>,
        message: String,
    }

    #[tokio::test]
    async fn api_error_returns_standard_error_response() {
        let response = ApiError::new(StatusCode::BAD_REQUEST, "invalid request").into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let Json(body) = Json::<ErrorBody>::from_bytes(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(!body.success);
        assert_eq!(body.data, None);
        assert_eq!(body.message, "invalid request");
    }

    #[test]
    fn resolves_conveyor_write_target_from_source_and_destination() {
        let conveyor = ConveyorConfig {
            send_routes: vec![ConveyorSendRoute {
                source: "5104-1-1-1".to_string(),
                destination: "5104-1-1-1".to_string(),
                device: "localhost:5000".to_string(),
                slave_id: 1,
                function: ConveyorWriteFunction::WriteSingleRegister,
                register_address: 10,
                value: 6,
            }],
            can_putdown_checks: Vec::new(),
            need_putdown_routes: Vec::new(),
        };

        let route = resolve_conveyor_write(&conveyor, "5104-1-1-1", "5104-1-1-1").unwrap();

        assert_eq!(route.device, "localhost:5000");
        assert_eq!(route.function.as_str(), "0x06");
        assert_eq!(route.register_address, 10);
        assert_eq!(route.value, 6);
    }

    #[test]
    fn missing_conveyor_write_route_returns_bad_request() {
        let err = resolve_conveyor_write(&ConveyorConfig::default(), "a", "b").unwrap_err();
        let response = err.into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn resolves_can_putdown_station_to_read_check() {
        let conveyor = ConveyorConfig {
            send_routes: Vec::new(),
            can_putdown_checks: vec![ConveyorReadCheck {
                location: "5107-1-1-1".to_string(),
                device: "localhost:5000".to_string(),
                slave_id: 1,
                function: ConveyorReadFunction::ReadHoldingRegisters,
                register_address: 10,
                quantity: 1,
            }],
            need_putdown_routes: Vec::new(),
        };

        let check = resolve_can_putdown(&conveyor, "5107-1-1-1").unwrap();

        assert_eq!(check.location, "5107-1-1-1");
        assert_eq!(check.device, "localhost:5000");
        assert_eq!(check.function.as_str(), "0x03");
        assert_eq!(check.register_address, 10);
    }

    #[test]
    fn missing_can_putdown_station_returns_bad_request() {
        let err = resolve_can_putdown(&ConveyorConfig::default(), "UNKNOWN").unwrap_err();
        let response = err.into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn resolves_need_putdown_location_to_write_route() {
        let conveyor = ConveyorConfig {
            send_routes: Vec::new(),
            can_putdown_checks: Vec::new(),
            need_putdown_routes: vec![ConveyorNeedPutdownRoute {
                location: "5107-1-1-1".to_string(),
                device: "localhost:5000".to_string(),
                slave_id: 1,
                function: ConveyorWriteFunction::WriteSingleRegister,
                register_address: 16,
                value: 6,
            }],
        };

        let route = resolve_need_putdown(&conveyor, "5107-1-1-1").unwrap();

        assert_eq!(route.location, "5107-1-1-1");
        assert_eq!(route.device, "localhost:5000");
        assert_eq!(route.function.as_str(), "0x06");
        assert_eq!(route.register_address, 16);
        assert_eq!(route.value, 6);
    }

    #[test]
    fn missing_need_putdown_location_returns_bad_request() {
        let err = resolve_need_putdown(&ConveyorConfig::default(), "UNKNOWN").unwrap_err();
        let response = err.into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
