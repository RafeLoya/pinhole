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
    pub client_1_udp: Option<SocketAddr>,
    pub client_2_udp: Option<SocketAddr>,
    /// TCP -> UDP
    pub udp_addrs: HashMap<SocketAddr, SocketAddr>
}

impl Session {
    pub fn new(id: String) -> Self {
        Self {
            id,
            client_1: None,
            client_2: None,
            client_1_udp: None,
            client_2_udp: None,
            udp_addrs: HashMap::new(),
        }
    }

    pub fn get_peer_udp(&self, requester: &SocketAddr) -> Option<SocketAddr> {
        if let Some((addr, _)) = &self.client_1 {
            if addr == requester {
                return self.client_2_udp;
            }
        }
        if let Some((addr, _)) = &self.client_2 {
            if addr == requester {
                return self.client_1_udp;
            }
        }
        None
    }

    /// Adds client to first available slot
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

    /// Associates client's TCP address w/ its UDP address
    pub fn register_udp(&mut self, tcp_addr: &SocketAddr, udp_port: u16) -> bool {
        // Check if the TCP address exists in this session
        let is_client_in_session = (self.client_1.is_some() && self.client_1.as_ref().unwrap().0 == *tcp_addr) ||
            (self.client_2.is_some() && self.client_2.as_ref().unwrap().0 == *tcp_addr);

        if !is_client_in_session {
            // Can't register UDP for a client that's not in this session
            return false;
        }

        // Valid port range check
        if udp_port == 0 || udp_port > 65535 {
            return false;
        }

        let udp_addr = SocketAddr::new(tcp_addr.ip(), udp_port);
        self.udp_addrs.insert(*tcp_addr, udp_addr);
        true
    }



    /// Returns peer's message channel for given client
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

/// Holds all active session & maps clients to their session IDs.
/// Also tracks UDP-to-TCP associations for UDP forwarding.
pub struct SessionManager {
    /// map of active sessions, where the key is a given session's ID
    pub sessions: HashMap<String, Session>,
    /// reverse map of client addresses -> session ID
    pub client_sessions: HashMap<SocketAddr, String>,
    pub udp_to_tcp: HashMap<SocketAddr, SocketAddr>
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            client_sessions: HashMap::new(),
            udp_to_tcp: HashMap::new()
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
                println!("added client {} to session {}", addr, session_id);
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

    pub fn get_session_for_client_mut(&mut self, addr: &SocketAddr) -> Option<&mut Session> {
        let session_id = self.client_sessions.get(addr)?;
        self.sessions.get_mut(session_id)
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

    pub fn get_tcp_addr_from_udp(&self, udp_addr: &SocketAddr) -> Option<SocketAddr> {
        for (_, session) in &self.sessions {
            for (tcp_addr, registered_udp) in &session.udp_addrs {
                if registered_udp == udp_addr {
                    return Some(*tcp_addr);
                }
            }
        }
        None
    }

    pub fn get_peer_udp_addr(&self, tcp_addr: &SocketAddr) -> Option<SocketAddr> {
        if let Some(session) = self.get_session_for_client(tcp_addr) {
            // Use the existing get_peer_tx method to identify the peer,
            // but we only need the address, not the sender
            if let Some(_) = session.get_peer_tx(tcp_addr) {
                // Now find which client is the peer
                if let Some((client_addr, _)) = &session.client_1 {
                    if client_addr != tcp_addr {
                        return session.udp_addrs.get(client_addr).cloned();
                    }
                }

                if let Some((client_addr, _)) = &session.client_2 {
                    if client_addr != tcp_addr {
                        return session.udp_addrs.get(client_addr).cloned();
                    }
                }
            }
        }
        None
    }
}