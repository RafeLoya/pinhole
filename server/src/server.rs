use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use clap::Parser;
use rcgen::generate_simple_self_signed;
use quinn::{rustls, Connection, Endpoint, ServerConfig};
use quinn::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::runtime::Runtime;
use common::protocol::{UserId, UserInfo};

const SERVER_NAME : &str = "csi4321.ascii-webcam.server";
const MAX_CONCURRENT_UNI_STREAMS: u64 = 10;
const MAX_IDLE_TIMEOUT: u64 = 30;

#[derive(Parser, Debug)]
#[clap(name = "server")]
struct Opt {
    /// file to log TLS keys to for debugging
    #[clap(long = "keylog")]
    keylog: bool,
    /// directory to serve files from
    root: PathBuf,
    /// TLS private key in PEM format
    #[clap(short = 'k', long = "key", requires = "cert")]
    key: Option<PathBuf>,
    /// TLS certificate in PEM format
    #[clap(short = 'c', long = "cert", requires = "key")]
    cert: Option<PathBuf>,
    /// Enable stateless retries
    #[clap(long = "stateless-retry")]
    stateless_retry: bool,
    /// Address to listen on
    #[clap(long = "listen", default_value = "[::1]:4433")]
    listen: SocketAddr,
    /// Client address to block
    #[clap(long = "block")]
    block: Option<SocketAddr>,
    /// Maximum number of concurrent connections to allow
    #[clap(long = "connection-limit")]
    connection_limit: Option<usize>,
}

pub struct Server {
    endpoint: Endpoint,
    runtime: Runtime,
    users: Arc<Mutex<HashMap<UserId, UserInfo>>>,
    call_requests: Arc<Mutex<HashMap<UserId, UserId>>>,
}

impl Server {
    pub fn new(addr: SocketAddr) -> Result<Self, Box<dyn Error>> {
        let (cert, key) = Self::generate_self_signed_cert()?;
        
    }
    
    pub fn local_addr(&self) -> Result<SocketAddr, Box<dyn Error>> {
        Ok(self.endpoint.local_addr()?)
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