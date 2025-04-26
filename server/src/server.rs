use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use clap::Parser;
use rcgen::generate_simple_self_signed;
use quinn::{rustls, Connection, Endpoint, RecvStream, SendStream, ServerConfig};
use quinn::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::runtime::Runtime;
use common::protocol::{UserId, UserInfo};
use log::{error, info};

const SERVER_NAME : &str = "csi4321.ascii-webcam.server";
const MAX_CONCURRENT_UNI_STREAMS: u64 = 10;
const MAX_IDLE_TIMEOUT: u64 = 30;

// #[derive(Parser, Debug)]
// #[clap(name = "server")]
// struct Opt {
//     /// file to log TLS keys to for debugging
//     #[clap(long = "keylog")]
//     keylog: bool,
//     /// directory to serve files from
//     root: PathBuf,
//     /// TLS private key in PEM format
//     #[clap(short = 'k', long = "key", requires = "cert")]
//     key: Option<PathBuf>,
//     /// TLS certificate in PEM format
//     #[clap(short = 'c', long = "cert", requires = "key")]
//     cert: Option<PathBuf>,
//     /// Enable stateless retries
//     #[clap(long = "stateless-retry")]
//     stateless_retry: bool,
//     /// Address to listen on
//     #[clap(long = "listen", default_value = "[::1]:4433")]
//     listen: SocketAddr,
//     /// Client address to block
//     #[clap(long = "block")]
//     block: Option<SocketAddr>,
//     /// Maximum number of concurrent connections to allow
//     #[clap(long = "connection-limit")]
//     connection_limit: Option<usize>,
// }

// TODO: In future, if we want to use example further, need to pass users and call requests to share state

pub struct Server {
    endpoint: Endpoint,
    users: Arc<Mutex<HashMap<UserId, UserInfo>>>,
    call_requests: Arc<Mutex<HashMap<UserId, UserId>>>,
}

impl Server {
    pub fn new(addr: SocketAddr) -> Result<Self, Box<dyn Error>> {
        let (cert, key) = Self::generate_self_signed_cert()?;
        let server_config = Self::configure_server(cert, key)?;
        let endpoint = Endpoint::server(server_config, addr)?;

        Ok(Self {
            endpoint,
            users: Arc::new(Mutex::new(HashMap::new())),
            call_requests: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    pub fn local_addr(&self) -> Result<SocketAddr, Box<dyn Error>> {
        Ok(self.endpoint.local_addr()?)
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        info!("listening on {}", self.endpoint.local_addr()?);
        
        loop {
            let conn = self.endpoint.accept().await;

            match conn { 
                Some(connecting) => {
                    tokio::spawn(async move {
                        match connecting.await {
                            Ok(connection) => {
                                info!("connection established from: {}", connection.remote_address());
                                Self::handle_connection(connection).await;
                            },
                            Err(e) => {
                                error!("connection failed: {}", e);
                            }
                        }
                    });
                },
                None => {
                    // endpoint was closed
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_connection(conn: Connection,) {
        while let Ok((send, recv)) = conn.accept_bi().await {
            info!("bi connection established");
            Self::handle_stream(send, recv).await;
        }

        info!("connection closed");
    }

    async fn handle_stream(mut send: SendStream, mut recv: RecvStream) {
        match recv.read_to_end(64 * 1024).await {
            Ok(data) => {
                if let Ok(str_data) = std::str::from_utf8(&data) {
                    info!("received data: {:?}", str_data);
                }

                // protocol & app logic here!

                if let Err(e) = send.write_all(b"Hello from QUIC server!").await {
                    error!("failed to send response: {}", e)
                }

                if let Err(e) = send.finish() {
                    error!("stream has closed: {}", e);
                }
            },
            Err(e) => {
                error!("failed to read from stream: {}", e)
            }
        }
    }
    
    fn generate_self_signed_cert() -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), Box<dyn Error>> {
        let subject_alt_names = vec![
            SERVER_NAME.to_string(),
            "quinn.server.demo".to_string(),
            "localhost".to_string(),
        ];
        let cert_keys = generate_simple_self_signed(subject_alt_names)?;
        let cert_der = CertificateDer::from(cert_keys.cert.der().to_vec());
        let key_der = PrivateKeyDer::Pkcs8(
            rustls::pki_types::PrivatePkcs8KeyDer::from(
                cert_keys.key_pair.serialize_der()
            )
        );
        
        Ok((vec![cert_der], key_der))
    }

    fn configure_server(
        certs: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
    ) -> Result<ServerConfig,  Box<dyn Error>> {

        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("failed to install rustls crypto provider");

        let mut server_crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;
        server_crypto.alpn_protocols = vec![b"h3".to_vec(), b"h2".to_vec(), b"http/1.1".to_vec()];

        let crypto = quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)?;

        let server_config = ServerConfig::with_crypto(Arc::new(crypto));

        Ok(server_config)
    }
}