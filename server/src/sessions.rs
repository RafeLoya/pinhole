use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::sync::{RwLock, mpsc};

pub enum Message {
    AsciiFrame(Vec<u8>),
    Connect(String),
    Disconnect,
}

/// session between two peer clients, created by the SFU
pub struct Session {
    pub id: String,
    pub client_a: Option<(SocketAddr, mpsc::UnboundedSender<Message>)>,
    pub client_b: Option<(SocketAddr, mpsc::UnboundedSender<Message>)>,
    pub udp_a: Option<SocketAddr>,
    pub udp_b: Option<SocketAddr>,
}

impl Session {
    pub fn new(id: String) -> Self {
        Self {
            id,
            client_a: None,
            client_b: None,
            udp_a: None,
            udp_b: None,
        }
    }

    /// Adds client to first available slot
    pub fn add_client(&mut self, addr: SocketAddr, tx: mpsc::UnboundedSender<Message>) -> bool {
        match (&self.client_a, &self.client_b) {
            // client A is not occupied
            (None, _) => {
                self.client_a = Some((addr, tx));
                true
            }
            // client b is not occupied
            (_, None) => {
                self.client_b = Some((addr, tx));
                true
            }
            // no available slots
            _ => false,
        }
    }

    /// Returns peer's message channel for given client
    pub fn get_peer_tx(&self, addr: &SocketAddr) -> Option<mpsc::UnboundedSender<Message>> {
        match (&self.client_a, &self.client_b) {
            (Some((a, _)), Some((_, tx))) if a == addr => Some(tx.clone()),
            (Some((_, tx)), Some((b, _))) if b == addr => Some(tx.clone()),
            _ => None,
        }
    }

    /// Associates client's TCP address w/ its UDP address
    pub fn register_udp(&mut self, tcp_addr: SocketAddr, udp_port: SocketAddr) {
        if self
            .client_a
            .as_ref()
            .map(|(a, _)| *a == tcp_addr)
            .unwrap_or(false)
        {
            self.udp_a = Some(udp_port)
        } else if self
            .client_b
            .as_ref()
            .map(|(b, _)| *b == tcp_addr)
            .unwrap_or(false)
        {
            self.udp_b = Some(udp_port)
        }
    }

    pub fn get_peer_udp(&self, tcp_addr: &SocketAddr) -> Option<SocketAddr> {
        if self
            .client_a
            .as_ref()
            .map(|(a, _)| a == tcp_addr)
            .unwrap_or(false)
        {
            return self.udp_b;
        } else if self
            .client_b
            .as_ref()
            .map(|(b, _)| b == tcp_addr)
            .unwrap_or(false)
        {
            return self.udp_a;
        }
        None
    }

    pub fn remove_client(&mut self, addr: &SocketAddr) {
        if self
            .client_a
            .as_ref()
            .map(|(a, _)| a == addr)
            .unwrap_or(false)
        {
            self.client_a = None;
            self.udp_a = None;
        } else if self
            .client_b
            .as_ref()
            .map(|(b, _)| b == addr)
            .unwrap_or(false)
        {
            self.client_b = None;
            self.udp_b = None;
        }
    }

    pub fn has_open_slot(&self) -> bool {
        self.client_a.is_none() || self.client_b.is_none()
    }

    pub fn is_empty(&self) -> bool {
        self.client_a.is_none() && self.client_b.is_none()
    }
}

/// Holds all active session & maps clients to their session IDs.
/// Also tracks UDP-to-TCP associations for UDP forwarding.
pub struct SessionManager {
    inner: RwLock<Inner>,
}

struct Inner {
    /// map of active sessions, where the key is a given session's ID
    pub sessions: HashMap<String, Session>,
    /// reverse map of client addresses -> session ID
    pub client_sessions: HashMap<SocketAddr, String>,
    pub udp_to_tcp: HashMap<SocketAddr, SocketAddr>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Inner {
                sessions: HashMap::new(),
                client_sessions: HashMap::new(),
                udp_to_tcp: HashMap::new(),
            }),
        }
    }

    /// Creates a session if it doesn't already exist
    pub async fn ensure_session(&self, id: &str) {
        let mut inner = self.inner.write().await;
        
        // essentially, insert if absent
        inner
            .sessions
            .entry(id.to_owned())
            .or_insert_with(|| Session::new(id.to_owned()));
    }
    
    pub async fn add_client(
        &self,
        session_id: &str,
        tcp_addr: SocketAddr,
        tx: mpsc::UnboundedSender<Message>,
    ) -> bool {
        let mut inner = self.inner.write().await;
        
        if let Some(s) = inner.sessions.get_mut(session_id) {
            if s.add_client(tcp_addr, tx) {
                inner.client_sessions.insert(tcp_addr, session_id.to_owned());
                return true;
            }
        }
        
        false
    }
    
    pub async fn register_udp(&self, tcp: SocketAddr, udp: SocketAddr) {
        let mut inner = self.inner.write().await;
        
        if let Some(id) = inner.client_sessions.get(&tcp).cloned() {
            if let Some(s) = inner.sessions.get_mut(&id) {
                s.register_udp(tcp, udp);
                inner.udp_to_tcp.insert(udp, tcp);
            }
        }
    }
    
    pub async fn get_peer_udp(&self, udp_src: &SocketAddr) -> Option<SocketAddr> {
        let inner = self.inner.read().await;
        let tcp = inner.udp_to_tcp.get(&udp_src)?;
        let id = inner.client_sessions.get(tcp)?;
        
        inner.sessions.get(id)?.get_peer_udp(tcp)
    }
    
    pub async fn notify_peer(&self, tcp: &SocketAddr, msg: Message) {
        // let inner = self.inner.read().await;
        // 
        // if let Some(id) = inner.client_sessions.get(tcp) {
        //     if let Some(s) = inner.sessions.get(id) {
        //         if let Some(peer_tx) = s.get_peer_tx(tcp) {
        //             let _ = peer_tx.send(msg);
        //         }
        //     }
        // }

        let peer_tx = {
            let inner = self.inner.read().await;
            inner
                .client_sessions
                .get(tcp)
                .and_then(|id| inner.sessions.get(id))
                .and_then(|s| s.get_peer_tx(tcp))
        };

        if let Some(tx) = peer_tx {
            let _ = tx.send(msg);          // no lock held here
        }
    }
    
    pub async fn remove_client(&self, tcp: &SocketAddr) {
        let mut inner = self.inner.write().await;
        if let Some(id) = inner.client_sessions.remove(tcp) {
            if let Some(s) = inner.sessions.get_mut(&id) {
                s.remove_client(tcp);
                if s.is_empty() {
                    inner.sessions.remove(&id);
                }
            }
        }
    }
    
    /// Return peer's UDP address given your own TCP address
    /// (both clients are present & peer already registered there UDP port)
    pub async fn get_peer_udp_from_tcp(&self, tcp: &SocketAddr) -> Option<SocketAddr> {
        let inner = self.inner.read().await;
        let id = inner.client_sessions.get(tcp)?;
        let room =  inner.sessions.get(id)?;
        room.get_peer_udp(tcp)
    }
    
    pub async fn session_full(&self, id: &str) -> bool {
        let inner = self.inner.read().await;
        inner.sessions.get(id)
            .map(|s| !s.has_open_slot())
            .unwrap_or(false)
    }
    
    pub async fn session_id_for(&self, tcp: &SocketAddr) -> Option<String> {
        let inner = self.inner.read().await;
        inner.client_sessions.get(tcp).cloned()
    }
}
