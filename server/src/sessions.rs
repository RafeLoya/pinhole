use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::sync::mpsc;

pub enum Message {
    AsciiFrame(Vec<u8>),
    Connect(String),
    Disconnect,
}

/// session between two peer clients, created by the SFU
pub struct Session {
    pub id: String,
    pub client_1: Option<(SocketAddr, mpsc::UnboundedSender<Message>)>,
    pub client_2: Option<(SocketAddr, mpsc::UnboundedSender<Message>)>,
}

impl Session {
    pub fn new(id: String) -> Self {
        Self {
            id,
            client_1: None,
            client_2: None,
        }
    }
    
    pub fn add_client(&mut self, addr: SocketAddr, tx: mpsc::UnboundedSender<Message>) -> bool {
        if self.client_1.is_none() {
            self.client_1 = Some((addr, tx));
            return true
        } else if  self.client_2.is_none() {
            self.client_2 = Some((addr, tx));
            return true
        }
        
        false
    }
    
    pub fn has_open_slot(&self) -> bool {
        self.client_1.is_none() || self.client_2.is_none()
    }
    
    /// Get access to a peer client's channel for forwarding network data
    pub fn get_peer_tx(&self, addr: &SocketAddr) -> Option<mpsc::UnboundedSender<Message>> {
        if let Some((client_addr, tx)) = &self.client_1 {
            if client_addr != addr {
                return Some(tx.clone());
            }
        }
        
        if let Some((client_addr, tx)) = &self.client_2 {
            if client_addr != addr {
                return Some(tx.clone());
            }
        }
        
        None
    }
    
    pub fn remove_client(&mut self, addr: &SocketAddr) -> bool {
        if let Some((client_addr, _)) = &self.client_1 {
            if client_addr == addr {
                self.client_1 = None;
                return true;
            }
        }
        
        if let Some((client_addr, _)) = &self.client_2 {
            if client_addr == addr {
                self.client_2 = None;
                return true;
            }
        }
        
        false
    }
    
    pub fn is_empty(&self) -> bool {
        self.client_1.is_none() && self.client_2.is_none()
    }
}

/// Data structure to store and retrieve active sessions,
/// along with forwarding data
pub struct SessionManager {
    /// map of active sessions, where the key is a given session's ID
    sessions: HashMap<String, Session>,
    /// reverse map of client addresses -> session ID
    client_sessions: HashMap<SocketAddr, String>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            client_sessions: HashMap::new(),
        }
    }
    
    pub fn create_session(&mut self, session_id: String) -> bool {
        if self.sessions.contains_key(&session_id) {
            return false;
        }
        
        self.sessions.insert(session_id.clone(), Session::new(session_id));
        true
    }
    
    pub fn add_client_to_session(&mut self, session_id: &str, addr: SocketAddr, tx: mpsc::UnboundedSender<Message>) -> bool {
        if let Some(session) = self.sessions.get_mut(session_id) {
            if session.add_client(addr, tx) {
                self.client_sessions.insert(addr, session_id.to_string());
                return true;
            }
        }
        false
    }
    
    pub fn get_session_for_client(&self, addr: &SocketAddr) -> Option<&Session> {
        if let Some(session_id) = self.client_sessions.get(addr) {
            return self.sessions.get(session_id);
        }
        
        None
    }
    
    pub fn forward_message(&self, from_addr: &SocketAddr, message: Message) -> bool {
        if let Some(session) = self.get_session_for_client(from_addr) {
            if let Some(peer_tx) = session.get_peer_tx(from_addr) {
                return peer_tx.send(message).is_ok();
            }
        }
        
        false
    }
    
    pub fn remove_client(&mut self, addr: &SocketAddr) {
        if let Some(session_id) = self.client_sessions.remove(addr) {
            if let Some(session) = self.sessions.get_mut(&session_id) {
                session.remove_client(addr);
                
                if session.is_empty() {
                    self.sessions.remove(&session_id);
                }
            }
        }
    }
}