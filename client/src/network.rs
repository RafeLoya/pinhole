use local_ip_address::local_ip;
use std::net::UdpSocket;

pub struct NetworkInfo {
    pub ip_address: String,
    pub udp_port: u16,
}

impl NetworkInfo {
    pub fn new() -> Self {
        NetworkInfo {
            ip_address: "Unknown".to_string(),
            udp_port: 0,
        }
    }

    // Refresh network info: get local IP and available UDP port
    pub fn get_network_info(&mut self) -> Result<(), String> {
        // Get local IP
        match local_ip() {
            Ok(ip) => {
                self.ip_address = ip.to_string();

                // Bind a UDP socket to a random port
                match UdpSocket::bind("0.0.0.0:0") {
                    Ok(socket) => {
                        match socket.local_addr() {
                            Ok(addr) => {
                                self.udp_port = addr.port();
                                Ok(())
                            },
                            Err(e) => Err(format!("Failed to get local address: {}", e)),
                        }
                    },
                    Err(e) => Err(format!("Failed to bind UDP socket: {}", e)),
                }
            },
            Err(e) => Err(format!("Failed to get local IP: {}", e)),
        }
    }
}
