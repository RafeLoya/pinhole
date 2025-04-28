use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use quinn::{ClientConfig, Connection, Endpoint};
use rustls::client::danger;
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::{DigitallySignedStruct, SignatureScheme};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tracing::info;
use quinn::SendStream;
use quinn::RecvStream;
#[derive(Debug)]
struct SkipServerVerification(Arc<CryptoProvider>);

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
    }
}

impl danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<danger::ServerCertVerified, rustls::Error> {
        Ok(danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<danger::HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<danger::HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

pub struct Client {
    endpoint: Endpoint,
    connection: Option<Connection>,
}

impl Client {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let client_config = Self::configure_client()?;

        let bind_addr = "[::]:0".parse::<SocketAddr>()?;
        let mut endpoint = Endpoint::client(bind_addr)?;
        endpoint.set_default_client_config(client_config);

        Ok(Self {
            endpoint,
            connection: None,
        })
    }

    pub async fn connect(&mut self, server_addr: SocketAddr, server_name: &str) -> Result<(), Box<dyn Error>> {
        let connecting = self.endpoint
            .connect(server_addr, server_name)?;
        let connection = connecting.await?;
        
        info!("connected to server: {}", connection.remote_address());
        
        self.connection = Some(connection);
        
        Ok(())
    }
    
    pub async fn send_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        if let Some(conn) = self.connection.as_ref() {
            let (mut send, mut recv) = conn.open_bi().await?;
            
            send.write_all(&message).await?;
            send.finish()?;
            
            let response = recv.read_to_end(1024).await?;
            info!("received message: {:?}", response);
            
            Ok(response)
        } else {
            Err("not connected to any server".into())
        }
    }
    
    pub fn close(&self) {
        if let Some(conn) = &self.connection {
            conn.close(0u32.into(), b"done");
        }
    }
    
    pub async fn wait_idle(&self) {
        self.endpoint.wait_idle().await;
    }

    fn configure_client() -> Result<ClientConfig, Box<dyn Error>> {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("failed to install rustls crypto provider");

        let mut crypto = rustls::ClientConfig::builder()
            .dangerous() // wot is this?
            .with_custom_certificate_verifier(SkipServerVerification::new())
            .with_no_client_auth();

        crypto.alpn_protocols = vec![b"h3".to_vec()];

        let quinn_crypto = quinn::crypto::rustls::QuicClientConfig::try_from(crypto)?;

        let client_config =  ClientConfig::new(Arc::new(quinn_crypto));

        Ok(client_config)
    }
}