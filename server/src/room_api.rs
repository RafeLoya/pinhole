//! HTTP API for room code registration and lookup.

use crate::room_registry::RoomRegistry;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;

/// Shared state for the room API.
#[derive(Clone)]
pub struct ApiState {
    pub registry: Arc<RoomRegistry>,
}

/// Request body for creating a room.
#[derive(Debug, Deserialize)]
pub struct CreateRoomRequest {
    /// Optional host address (for tracking).
    #[serde(default)]
    pub host_addr: Option<String>,
}

/// Response for room creation.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRoomResponse {
    /// The generated room code.
    pub room_code: String,
    /// The session ID (same as room code).
    pub session_id: String,
}

/// Response for room lookup.
#[derive(Debug, Serialize, Deserialize)]
pub struct LookupRoomResponse {
    /// The room code.
    pub room_code: String,
    /// The session ID to join.
    pub session_id: String,
}

/// Error response.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Creates the room API router.
pub fn room_router(registry: Arc<RoomRegistry>) -> Router {
    let state = ApiState { registry };

    Router::new()
        .route("/rooms", post(create_room))
        .route("/rooms/{code}", get(lookup_room))
        .route("/rooms/{code}", axum::routing::delete(delete_room))
        .route("/health", get(health_check))
        .with_state(state)
}

/// POST /rooms - Create a new room with auto-generated code.
async fn create_room(
    State(state): State<ApiState>,
    Json(req): Json<CreateRoomRequest>,
) -> impl IntoResponse {
    let host_addr: Option<SocketAddr> = req
        .host_addr
        .as_ref()
        .and_then(|s| s.parse().ok());

    let room_code = state.registry.create_room(host_addr).await;

    let response = CreateRoomResponse {
        session_id: room_code.clone(),
        room_code,
    };

    (StatusCode::CREATED, Json(response))
}

/// GET /rooms/:code - Lookup a room by code.
async fn lookup_room(
    State(state): State<ApiState>,
    Path(code): Path<String>,
) -> Result<Json<LookupRoomResponse>, (StatusCode, Json<ErrorResponse>)> {
    // validate format first
    if !common::room_code::validate(&code) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid room code format".to_string(),
            }),
        ));
    }

    match state.registry.lookup(&code).await {
        Some(entry) => Ok(Json(LookupRoomResponse {
            room_code: common::room_code::normalize(&code).unwrap(),
            session_id: entry.session_id,
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "room not found or expired".to_string(),
            }),
        )),
    }
}

/// DELETE /rooms/:code - Delete a room.
async fn delete_room(
    State(state): State<ApiState>,
    Path(code): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    if !common::room_code::validate(&code) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid room code format".to_string(),
            }),
        ));
    }

    match state.registry.remove(&code).await {
        Some(_) => Ok(StatusCode::NO_CONTENT),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "room not found".to_string(),
            }),
        )),
    }
}

/// GET /health - Health check endpoint.
async fn health_check() -> impl IntoResponse {
    StatusCode::OK
}

/// Starts the HTTP API server.
pub async fn run_api_server(
    addr: SocketAddr,
    registry: Arc<RoomRegistry>,
) -> std::io::Result<()> {
    let app = room_router(registry);

    println!("[HTTP] room API listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app() -> Router {
        let registry = Arc::new(RoomRegistry::with_default_ttl());
        room_router(registry)
    }

    #[tokio::test]
    async fn health_check_returns_ok() {
        let app = test_app();

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn create_room_returns_code() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rooms")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let resp: CreateRoomResponse = serde_json::from_slice(&body).unwrap();

        assert!(common::room_code::validate(&resp.room_code));
        assert_eq!(resp.room_code, resp.session_id);
    }

    #[tokio::test]
    async fn lookup_nonexistent_returns_404() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/rooms/swift-river-42")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn lookup_invalid_code_returns_400() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/rooms/invalid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_then_lookup_succeeds() {
        let registry = Arc::new(RoomRegistry::with_default_ttl());
        let app = room_router(registry);

        // create room
        let create_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rooms")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create_resp.status(), StatusCode::CREATED);

        let body = create_resp.into_body().collect().await.unwrap().to_bytes();
        let created: CreateRoomResponse = serde_json::from_slice(&body).unwrap();

        // lookup room
        let lookup_resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/rooms/{}", created.room_code))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(lookup_resp.status(), StatusCode::OK);

        let body = lookup_resp.into_body().collect().await.unwrap().to_bytes();
        let looked_up: LookupRoomResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(looked_up.session_id, created.session_id);
    }
}
