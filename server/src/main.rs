use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncWriteExt, Interest},
    net::{TcpListener, TcpStream}, sync::Mutex,
};

pub const HELLO_BYTE: u8 = 0x69;
const CONNECTION_REQUEST_BYTE: u8 = 0x42;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let tcp_addr = "0.0.0.0:8080";
    let tcp_listener = TcpListener::bind(tcp_addr).await?;

    println!("TCP Server listening on {}", tcp_addr);

    let udp_addr = "0.0.0.0:4433";
    let udp_listener = tokio::net::UdpSocket::bind(udp_addr).await?;

    println!("UDP Server listening on {}", udp_addr);

    let usernames: Arc<tokio::sync::Mutex<Vec<String>>> =
        Arc::new(tokio::sync::Mutex::new(Vec::new()));

    let user_to_user_connections: Arc<tokio::sync::Mutex<Vec<(String, String)>>> =
        Arc::new(tokio::sync::Mutex::new(Vec::new()));

    //listen to udp
    let user_to_user_connections_clone = user_to_user_connections.clone();
    tokio::spawn(async move {

        let user_to_user_connections = user_to_user_connections_clone;

        let mut usernames_to_addresses: std::collections::HashMap<String, SocketAddr> = std::collections::HashMap::new();

        let mut buf = [0u8; 257];
        loop {

            match udp_listener.recv_from(&mut buf).await {
                Ok((n, addr)) => {

                    if n < 2 {
                        continue;
                    }
                    let hello_byte = buf[0];
                    let username_length = buf[1];
                    if username_length < 1 || username_length + 2 > n as u8 {
                        continue;
                    }
                    let username = &buf[2..(2 + username_length as usize)];
                    let username_str = String::from_utf8_lossy(username).to_string();
                    if hello_byte == HELLO_BYTE {
                        usernames_to_addresses.insert(username_str.clone(), addr);

                    } else {
                        let client_a_addr = addr;

                        // Get username from the address
                        if let Some((client_a_username, _)) = usernames_to_addresses
                            .iter()
                            .find(|(_, v)| *v == &client_a_addr)
                        {
                            // Find the corresponding user-to-user connection
                            let client_b_username = {
                                let connections = user_to_user_connections.lock().await;
                                connections
                                    .iter()
                                    .find(|(user_a, user_b)| user_a == client_a_username || user_b == client_a_username)
                                    .map(|(user_a, user_b)| {
                                        if user_a == client_a_username {
                                            user_b.clone()
                                        } else {
                                            user_a.clone()
                                        }
                                    })
                            };

                            if let Some(client_b_username) = client_b_username {
                                if let Some(client_b_addr) = usernames_to_addresses.get(&client_b_username) {
                                    let _ = udp_listener.send_to(&buf[2..n], client_b_addr).await;
                                }
                            }
                        }
                    }
                }
                Err(_) => {}
            }
        }
    });

    loop {
        let (socket, addr) = tcp_listener.accept().await?;
        println!("Accepted connection from {:?}", addr);

        let usernames = usernames.clone();
        let user_to_user_connections = user_to_user_connections.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_tcp_socket(socket, usernames.clone(), user_to_user_connections.clone()).await {
                eprintln!("Error handling socket for {}: {}", addr, e);
            }

            println!("Connection closed from {:?}", addr);
        });
    }
}

async fn handle_tcp_socket(
    mut socket: TcpStream,
    usernames: Arc<Mutex<Vec<String>>>,
    user_to_user_connections: Arc<Mutex<Vec<(String, String)>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Try to register the username
    let current_username = match register_username(&mut socket, &usernames).await? {
        Some(name) => name,
        None => return Ok(()),
    };

    // Send the list of usernames back to the client
    send_usernames_list(&mut socket, &usernames).await?;

    loop {
        if let Some((user_a, user_b)) = handle_connection_request(&mut socket, &usernames, &current_username).await? {

            // add connection to the list of connections
            let mut connections = user_to_user_connections.lock().await;
            connections.push((user_a.clone(), user_b.clone()));
            drop(connections); // Release lock
            println!("Connection established between {} and {}", user_a, user_b);

            socket.writable().await?;
            socket.write_all(&[0x00]).await?;
        }
        else {
            break;
        }
    }

    let mut list = usernames.lock().await;
    if let Some(pos) = list.iter().position(|x| *x == current_username) {
        list.remove(pos);
    }
    drop(list);

    let mut connections = user_to_user_connections.lock().await;
    connections.retain(|(user_a, user_b)| {
        user_a != &current_username && user_b != &current_username
    });
    drop(connections);

    Ok(())
}

async fn handle_connection_request(
    socket: &mut tokio::net::TcpStream,
    usernames: &Arc<tokio::sync::Mutex<Vec<String>>>,
    current_username: &String,
) -> Result<Option<(String, String)>, Box<dyn std::error::Error + Send + Sync>> {
    let mut buf = [0u8; 257];

    // Wait for the socket to be readable
    socket.ready(Interest::READABLE).await?;

    // Try to read the connection request
    loop {
        match socket.try_read(&mut buf) {
            Ok(0) => {
                return Ok(None); // Connection closed
            }
            Ok(n) => {
                let connection_request_byte = buf[0];
                let username_length = buf[1];

                if username_length < 1 || username_length + 2 > n as u8 {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid connection request length",
                    )));
                }

                let username = &buf[2..(2 + username_length as usize)];
                let username_str = String::from_utf8_lossy(username).to_string();

                if connection_request_byte == CONNECTION_REQUEST_BYTE {
                    let usernames_lock = usernames.lock().await;

                    if usernames_lock.contains(&username_str) {
                        return Ok(Some((current_username.to_string(), username_str)));
                    } else {
                        return Err(Box::new(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "Username not found",
                        )));
                    }
                } else {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid connection request byte",
                    )));
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                return Err(Box::new(e));
            }
        }
    }
}

async fn register_username(
    socket: &mut tokio::net::TcpStream,
    usernames: &Arc<tokio::sync::Mutex<Vec<String>>>,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut buf = [0u8; 257];

    // Wait for the socket to be readable
    socket.ready(Interest::READABLE).await?;

    // Try to read the username
    loop {
        match socket.try_read(&mut buf) {
            Ok(0) => {
                return Ok(None); // Connection closed
            }
            Ok(n) => {
                let hello_byte = buf[0];
                let username_length = buf[1];

                if username_length < 1 || username_length + 2 > n as u8 {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid username length",
                    )));
                }

                let username = &buf[2..(2 + username_length as usize)];
                let username_str = String::from_utf8_lossy(username).to_string();

                if hello_byte == HELLO_BYTE {
                    let mut usernames_lock = usernames.lock().await;

                    if usernames_lock.contains(&username_str) {
                        socket.writable().await?;
                        socket.write_all(&[0x01]).await?;

                        return Err(Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Username already taken",
                        )));
                    }

                    usernames_lock.push(username_str.clone());
                    drop(usernames_lock); // Release lock

                    return Ok(Some(username_str));
                } else {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid hello byte",
                    )));
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                return Err(Box::new(e));
            }
        }
    }
}

async fn send_usernames_list(
    socket: &mut tokio::net::TcpStream,
    usernames: &Arc<tokio::sync::Mutex<Vec<String>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let usernames_lock = usernames.lock().await;
    let usernames_length = usernames_lock.len() as u8;

    if usernames_length < 1 {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid usernames length",
        )));
    }

    let mut response = vec![HELLO_BYTE, usernames_length];
    for username in usernames_lock.iter() {
        let username_bytes = username.as_bytes();
        let username_length = username_bytes.len() as u8;
        response.push(username_length);
        response.extend_from_slice(username_bytes);
    }

    drop(usernames_lock);

    socket.writable().await?;
    socket.write_all(&response).await?;

    Ok(())
}