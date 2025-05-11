use std::{io::{self, stdout, Write}, sync::Arc};
use rand::Rng;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket}
};

const HELLO_BYTE: u8 = 0x69;
const INVALID_RESPONSE_BYTE: u8 = 0x01;
const CONNECTION_REQUEST_BYTE: u8 = 0x42;
const UDP_MESSAGE_BYTE: u8 = 0x34;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let username = prompt_for_username()?;

    let username_to_connect;
    let mut stream;

    loop {

        let addr = "127.0.0.1:8080";
        stream = TcpStream::connect(addr).await?;

        send_username(&mut stream, &username).await?;
        let all_active_usernames = receive_user_list(&mut stream).await?;
        let other_usernames = all_active_usernames
            .iter()
            .filter(|&user| *user != username)
            .cloned()
            .collect::<Vec<_>>();

        println!("Connected to server at {} as {}", addr, username);

        if other_usernames.is_empty() {            
            // Prompt the to try again?
            println!("No other users available. Please try again.");
            println!("Press Enter to try again or Ctrl+C to exit.");
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            continue;
        } else {
            username_to_connect = prompt_for_username_to_connect(other_usernames);
            break;
        }
    }

    println!("Connecting to user: {}", username_to_connect);

    let mut connection_request = vec![
        CONNECTION_REQUEST_BYTE,
        username_to_connect.len() as u8,
    ];
    connection_request.extend_from_slice(username_to_connect.as_bytes());
    stream.write_all(&connection_request).await?;

    handle_connection_response(&mut stream).await?;

    println!("Connection established with user: {}", username_to_connect);

    let udp_addr = "0.0.0.0:0";
    let udp_socket = Arc::new(UdpSocket::bind(udp_addr).await?);

    //send hello udp message as [HELLO_BYTE, username.len() as u8, username]

    let mut hello_message = vec![HELLO_BYTE, username.len() as u8];
    hello_message.extend_from_slice(username.as_bytes());
    udp_socket.send_to(&hello_message, "127.0.0.1:4433").await?;

    let udp_socket_clone = Arc::clone(&udp_socket);
    tokio::task::spawn(async move {
        loop {

            // create randomized message
            let message = format!("Hello from UDP client!{}", rand::random::<u8>());
            let mut udp_message = vec![UDP_MESSAGE_BYTE, message.len() as u8];
            udp_message.extend_from_slice(message.as_bytes());
            let _ = udp_socket_clone.send_to(&udp_message, "127.0.0.1:4433").await;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    });
    
    loop {
        let mut buf = vec![0; 1024];
        let (len, _) = udp_socket.recv_from(&mut buf).await?;
        if len == 0 {
            println!("No data received");
            continue;
        }
        let response = String::from_utf8_lossy(&buf[..len]);
        
        if cfg!(target_os = "macos") {
            print!("\x1B[2J\x1B[1;1H");
        } else if cfg!(target_os = "windows") {
            print!("{}[2J", 27 as char);
        }

        println!("{}", response);
    }
}

async fn handle_connection_response(
    stream: &mut TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = vec![0; 1024];
    let n = stream.read_buf(&mut buf).await?;
    if n == 0 {
        return Err("Connection closed by server".into());
    }

    let response_byte = buf[0];
    if response_byte == INVALID_RESPONSE_BYTE {
        return Err("Invalid connection request".into());
    }

    Ok(())
}

fn prompt_for_username_to_connect(other_usernames: Vec<String>) -> String {
    println!("Current available users:");
    for username in &other_usernames {
        println!(" - {}", username);
    }

    loop {
        print!("Enter a username to connect to: ");
        io::stdout().flush().unwrap(); // Make sure prompt shows before input

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            println!("Error reading input. Please try again.");
            continue;
        }

        let input = input.trim();

        if other_usernames.iter().any(|u| u == input) {
            return input.to_string();
        }

        println!("Invalid selection. Please try again.");
    }
}

fn prompt_for_username() -> Result<String, io::Error> {
    print!("Please enter your username: ");
    stdout().flush()?;

    let mut username = String::new();
    io::stdin().read_line(&mut username)?;
    let username = username.trim();

    if username.is_empty() || username.len() > 256 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Username must be between 1 and 256 characters.",
        ));
    }

    let invalid_chars = ['\n', '\r', '\0', '\t', ' '];
    if username.chars().any(|c| invalid_chars.contains(&c)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Username contains invalid characters.",
        ));
    }

    Ok(username.to_string())
}

async fn send_username(stream: &mut TcpStream, username: &str) -> io::Result<()> {
    let username_bytes = username.as_bytes();
    let username_length = username_bytes.len() as u8;

    let mut buffer = Vec::with_capacity(2 + username_bytes.len());
    buffer.push(HELLO_BYTE);
    buffer.push(username_length);
    buffer.extend_from_slice(username_bytes);

    stream.write_all(&buffer).await?;

    Ok(())
}

async fn receive_user_list(stream: &mut TcpStream) -> io::Result<Vec<String>> {
    let mut buf = Vec::with_capacity(2048);

    loop {
        let n = stream.read_buf(&mut buf).await?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Server closed the connection",
            ));
        }

        let response_byte = buf.get(0).copied().unwrap_or(0);
        let usernames_length = buf.get(1).copied().unwrap_or(0);

        if response_byte == INVALID_RESPONSE_BYTE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Server rejected the username.",
            ));
        }

        if response_byte != HELLO_BYTE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unexpected response byte: {}", response_byte),
            ));
        }

        if usernames_length == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Received invalid usernames length (0).",
            ));
        }

        let mut offset = 2;
        let mut usernames = Vec::new();

        while offset < buf.len() {
            if offset >= buf.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Unexpected end of buffer.",
                ));
            }

            let name_len = buf[offset] as usize;
            offset += 1;

            if offset + name_len > buf.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid username length: {}", name_len),
                ));
            }

            let name_bytes = &buf[offset..offset + name_len];
            usernames.push(String::from_utf8_lossy(name_bytes).to_string());
            offset += name_len;
        }

        return Ok(usernames);
    }
}