use quinn::{ClientConfig, Endpoint, ServerConfig};
use rustls::{Certificate, PrivateKey};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;

pub fn make_server_endpoint(bind_addr: SocketAddr) -> Result<(Endpoint, Vec<u8>), Box<dyn Error>> {
  let (server_config, server_cert) = configure_server()?;
  let endpoint = Endpoint::server(server_config, bind_addr)?;
  Ok((endpoint, server_cert))
}

pub fn make_client_endpoint(
  bind_addr: SocketAddr,
  server_cert: &[u8],
) -> Result<Endpoint, Box<dyn Error>> {
  let client_config = configure_client(server_cert)?;
  let mut endpoint = Endpoint::client(bind_addr)?;
  endpoint.set_default_client_config(client_config);
  endpoint.set_server_config(None);
  Ok(endpoint)
}

fn configure_server() -> Result<(ServerConfig, Vec<u8>), Box<dyn Error>> {
  let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
  let cert_der = cert.serialize_der()?;
  let priv_key = cert.serialize_private_key_der();
  let priv_key = PrivateKey(priv_key);
  let cert_chain = vec![Certificate(cert_der.clone())];

  let mut server_config = ServerConfig::with_single_cert(cert_chain, priv_key)?;
  let transport_config = std::sync::Arc::get_mut(&mut server_config.transport).unwrap();
  transport_config.max_concurrent_uni_streams(1024_u32.into());

  Ok((server_config, cert_der))
}

fn configure_client(_server_cert: &[u8]) -> Result<ClientConfig, Box<dyn Error>> {
  let crypto = rustls::ClientConfig::builder()
    .with_safe_defaults()
    .with_custom_certificate_verifier(SkipServerVerification::new())
    .with_no_client_auth();

  Ok(ClientConfig::new(Arc::new(crypto)))
}

struct SkipServerVerification;

impl SkipServerVerification {
  fn new() -> Arc<Self> {
    Arc::new(Self)
  }
}

impl rustls::client::ServerCertVerifier for SkipServerVerification {
  fn verify_server_cert(
    &self,
    _end_entity: &Certificate,
    _intermediates: &[Certificate],
    _server_name: &rustls::ServerName,
    _scts: &mut dyn Iterator<Item = &[u8]>,
    _ocsp_response: &[u8],
    _now: std::time::SystemTime,
  ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
    Ok(rustls::client::ServerCertVerified::assertion())
  }
}
