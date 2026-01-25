//! Room code registry for mapping human-friendly codes to session IDs.

use common::room_code;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Metadata for a registered room.
#[derive(Debug, Clone)]
pub struct RoomEntry {
    /// The session ID this room maps to (may be same as room code).
    pub session_id: String,
    /// TCP address of the host who created the room.
    pub host_addr: Option<SocketAddr>,
    /// When the room was created.
    pub created_at: Instant,
    /// When the room was last accessed (for TTL refresh).
    pub last_accessed: Instant,
}

impl RoomEntry {
    fn new(session_id: String, host_addr: Option<SocketAddr>) -> Self {
        let now = Instant::now();
        Self {
            session_id,
            host_addr,
            created_at: now,
            last_accessed: now,
        }
    }

    fn touch(&mut self) {
        self.last_accessed = Instant::now();
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.last_accessed.elapsed() > ttl
    }
}

/// Registry mapping room codes to session information.
pub struct RoomRegistry {
    inner: RwLock<HashMap<String, RoomEntry>>,
    ttl: Duration,
}

impl RoomRegistry {
    /// Creates a new registry with the specified TTL for room entries.
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    /// Creates a new registry with default 1-hour TTL.
    pub fn with_default_ttl() -> Self {
        Self::new(Duration::from_secs(3600))
    }

    /// Registers a new room with an auto-generated code.
    ///
    /// Returns the generated room code.
    pub async fn create_room(&self, host_addr: Option<SocketAddr>) -> String {
        let mut inner = self.inner.write().await;

        // generate unique code (retry on collision)
        let code = loop {
            let candidate = room_code::generate();
            if !inner.contains_key(&candidate) {
                break candidate;
            }
        };

        // use room code as session ID
        let entry = RoomEntry::new(code.clone(), host_addr);
        inner.insert(code.clone(), entry);

        code
    }

    /// Registers a room with a specific code.
    ///
    /// Returns `true` if registered, `false` if code already exists.
    pub async fn register(&self, code: &str, host_addr: Option<SocketAddr>) -> bool {
        if !room_code::validate(code) {
            return false;
        }

        let code = room_code::normalize(code).unwrap();
        let mut inner = self.inner.write().await;

        if inner.contains_key(&code) {
            return false;
        }

        let entry = RoomEntry::new(code.clone(), host_addr);
        inner.insert(code, entry);
        true
    }

    /// Looks up a room by code.
    ///
    /// Updates last_accessed time on successful lookup.
    pub async fn lookup(&self, code: &str) -> Option<RoomEntry> {
        let code = room_code::normalize(code)?;
        let mut inner = self.inner.write().await;

        if let Some(entry) = inner.get_mut(&code) {
            if entry.is_expired(self.ttl) {
                inner.remove(&code);
                return None;
            }
            entry.touch();
            return Some(entry.clone());
        }

        None
    }

    /// Gets the session ID for a room code without updating access time.
    pub async fn get_session_id(&self, code: &str) -> Option<String> {
        let code = room_code::normalize(code)?;
        let inner = self.inner.read().await;

        inner
            .get(&code)
            .filter(|e| !e.is_expired(self.ttl))
            .map(|e| e.session_id.clone())
    }

    /// Checks if a room code exists and is not expired.
    pub async fn exists(&self, code: &str) -> bool {
        self.get_session_id(code).await.is_some()
    }

    /// Removes a room by code.
    ///
    /// Returns the removed entry if it existed.
    pub async fn remove(&self, code: &str) -> Option<RoomEntry> {
        let code = room_code::normalize(code)?;
        let mut inner = self.inner.write().await;
        inner.remove(&code)
    }

    /// Removes all expired rooms.
    ///
    /// Returns the number of rooms removed.
    pub async fn cleanup_expired(&self) -> usize {
        let mut inner = self.inner.write().await;
        let before = inner.len();
        inner.retain(|_, entry| !entry.is_expired(self.ttl));
        before - inner.len()
    }

    /// Returns the number of active (non-expired) rooms.
    pub async fn len(&self) -> usize {
        let inner = self.inner.read().await;
        inner.values().filter(|e| !e.is_expired(self.ttl)).count()
    }

    /// Returns true if the registry has no active rooms.
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Spawns a background task that periodically cleans up expired rooms.
    pub fn spawn_cleanup_task(self: &std::sync::Arc<Self>, interval: Duration) {
        let registry = std::sync::Arc::clone(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                let removed = registry.cleanup_expired().await;
                if removed > 0 {
                    println!("[ROOMS] cleaned up {} expired room(s)", removed);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_room_generates_valid_code() {
        let registry = RoomRegistry::with_default_ttl();
        let code = registry.create_room(None).await;
        assert!(room_code::validate(&code));
    }

    #[tokio::test]
    async fn create_room_generates_unique_codes() {
        let registry = RoomRegistry::with_default_ttl();
        let code1 = registry.create_room(None).await;
        let code2 = registry.create_room(None).await;
        assert_ne!(code1, code2);
    }

    #[tokio::test]
    async fn lookup_returns_entry() {
        let registry = RoomRegistry::with_default_ttl();
        let code = registry.create_room(None).await;

        let entry = registry.lookup(&code).await;
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().session_id, code);
    }

    #[tokio::test]
    async fn lookup_is_case_insensitive() {
        let registry = RoomRegistry::with_default_ttl();
        let code = registry.create_room(None).await;

        let upper = code.to_uppercase();
        let entry = registry.lookup(&upper).await;
        assert!(entry.is_some());
    }

    #[tokio::test]
    async fn expired_rooms_not_returned() {
        let registry = RoomRegistry::new(Duration::from_millis(10));
        let code = registry.create_room(None).await;

        // wait for expiration
        tokio::time::sleep(Duration::from_millis(20)).await;

        assert!(registry.lookup(&code).await.is_none());
        assert!(!registry.exists(&code).await);
    }

    #[tokio::test]
    async fn cleanup_removes_expired() {
        let registry = RoomRegistry::new(Duration::from_millis(10));
        registry.create_room(None).await;
        registry.create_room(None).await;

        tokio::time::sleep(Duration::from_millis(20)).await;

        let removed = registry.cleanup_expired().await;
        assert_eq!(removed, 2);
        assert_eq!(registry.len().await, 0);
    }

    #[tokio::test]
    async fn register_with_specific_code() {
        let registry = RoomRegistry::with_default_ttl();

        assert!(registry.register("swift-river-42", None).await);
        assert!(registry.exists("swift-river-42").await);

        // duplicate fails
        assert!(!registry.register("swift-river-42", None).await);

        // invalid code fails
        assert!(!registry.register("invalid", None).await);
    }

    #[tokio::test]
    async fn remove_deletes_entry() {
        let registry = RoomRegistry::with_default_ttl();
        let code = registry.create_room(None).await;

        assert!(registry.exists(&code).await);
        registry.remove(&code).await;
        assert!(!registry.exists(&code).await);
    }
}
