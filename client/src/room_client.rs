//! HTTP client for room code registration and lookup.

use serde::{Deserialize, Serialize};

/// Request body for creating a room.
#[derive(Debug, Serialize)]
pub struct CreateRoomRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_addr: Option<String>,
}

/// Response from room creation.
#[derive(Debug, Deserialize)]
pub struct CreateRoomResponse {
    pub room_code: String,
    pub session_id: String,
}

/// Response from room lookup.
#[derive(Debug, Deserialize)]
pub struct LookupRoomResponse {
    pub room_code: String,
    pub session_id: String,
}

/// Error response from the API.
#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Client for interacting with the room API.
pub struct RoomClient {
    base_url: String,
    http: reqwest::Client,
}

/// Errors that can occur when interacting with the room API.
#[derive(Debug)]
pub enum RoomError {
    /// Room code not found.
    NotFound,
    /// Invalid room code format.
    InvalidCode,
    /// Network or server error.
    RequestFailed(String),
}

impl std::fmt::Display for RoomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoomError::NotFound => write!(f, "room not found or expired"),
            RoomError::InvalidCode => write!(f, "invalid room code format"),
            RoomError::RequestFailed(msg) => write!(f, "request failed: {}", msg),
        }
    }
}

impl std::error::Error for RoomError {}

impl RoomClient {
    /// Creates a new room client with the given API base URL.
    ///
    /// # Example
    ///
    /// ```
    /// use pinhole::room_client::RoomClient;
    ///
    /// let client = RoomClient::new("http://localhost:8000");
    /// ```
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    /// Creates a new room and returns the room code.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pinhole::room_client::RoomClient;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let client = RoomClient::new("http://localhost:8000");
    ///     let response = client.create_room().await.unwrap();
    ///     println!("Room code: {}", response.room_code);
    /// }
    /// ```
    pub async fn create_room(&self) -> Result<CreateRoomResponse, RoomError> {
        let url = format!("{}/rooms", self.base_url);
        let req = CreateRoomRequest { host_addr: None };

        let response = self
            .http
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| RoomError::RequestFailed(e.to_string()))?;

        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| RoomError::RequestFailed(e.to_string()))
        } else {
            Err(RoomError::RequestFailed(format!(
                "server returned {}",
                response.status()
            )))
        }
    }

    /// Looks up a room by code and returns the session ID.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pinhole::room_client::RoomClient;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let client = RoomClient::new("http://localhost:8000");
    ///     let response = client.lookup_room("swift-river-42").await.unwrap();
    ///     println!("Session ID: {}", response.session_id);
    /// }
    /// ```
    pub async fn lookup_room(&self, code: &str) -> Result<LookupRoomResponse, RoomError> {
        // validate locally first
        if !common::room_code::validate(code) {
            return Err(RoomError::InvalidCode);
        }

        let code = common::room_code::normalize(code).ok_or(RoomError::InvalidCode)?;
        let url = format!("{}/rooms/{}", self.base_url, code);

        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| RoomError::RequestFailed(e.to_string()))?;

        match response.status().as_u16() {
            200 => response
                .json()
                .await
                .map_err(|e| RoomError::RequestFailed(e.to_string())),
            400 => Err(RoomError::InvalidCode),
            404 => Err(RoomError::NotFound),
            status => Err(RoomError::RequestFailed(format!(
                "server returned {}",
                status
            ))),
        }
    }

    /// Checks if the room API server is healthy.
    pub async fn health_check(&self) -> bool {
        let url = format!("{}/health", self.base_url);
        self.http
            .get(&url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}
