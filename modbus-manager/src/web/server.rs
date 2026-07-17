use crate::{
    config::{ConveyorConfig, ConveyorSendRoute, ConveyorWriteFunction},
    modbus::{ModbusError, ModbusService},
    web::{api_response::ApiResponse, http_trace::trace_http_request},
};
use axum::{
    Json, Router,
    extract::State,
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
    use super::{ApiError, resolve_conveyor_write};
    use crate::config::{ConveyorConfig, ConveyorSendRoute, ConveyorWriteFunction};
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
}
