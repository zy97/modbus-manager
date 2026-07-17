use crate::web::{api_response::ApiResponse, http_trace::trace_http_request};
use axum::{
    Json, Router,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use std::net::SocketAddr;
use tokio::{net::TcpListener, signal};
use tracing::{error, info};

pub fn build_router() -> Router {
    Router::new()
        .route("/health", get(health))
        .layer(middleware::from_fn(trace_http_request))
}

pub async fn serve(listen_addr: String) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(&listen_addr).await?;
    let local_addr = listener.local_addr()?;
    info!(
        listen_addr = %local_addr,
        "Modbus HTTP服务已监听 listen_addr={}",
        local_addr
    );

    axum::serve(
        listener,
        build_router().into_make_service_with_connect_info::<SocketAddr>(),
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
    use super::ApiError;
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
}
