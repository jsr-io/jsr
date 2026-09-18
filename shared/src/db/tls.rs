// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use std::sync::Arc;

use rustls::DigitallySignedStruct;
use rustls::SignatureScheme;
use rustls::client::danger::HandshakeSignatureValid;
use rustls::client::danger::ServerCertVerified;
use rustls::client::danger::ServerCertVerifier;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivateKeyDer;
use rustls::pki_types::ServerName;
use rustls::pki_types::UnixTime;
use rustls::pki_types::pem::PemObject;
use tokio_postgres_rustls::MakeRustlsConnect;

use super::DbTls;

/// TLS for connecting to the database with a client certificate.
///
/// The server certificate is not verified, the same as libpq's
/// `sslmode=require`: Cloud Run connects to Cloud SQL by private IP, but the
/// server certificate is only valid for the instance's `*.sql.goog` DNS name.
/// The client certificate (mTLS) is the access boundary and the link stays
/// inside the VPC.
pub fn make_tls_connect(tls: &DbTls) -> anyhow::Result<MakeRustlsConnect> {
  let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
  let client_cert = CertificateDer::pem_slice_iter(tls.client_cert.as_bytes())
    .collect::<Result<Vec<_>, _>>()?;
  let client_key = PrivateKeyDer::from_pem_slice(tls.client_key.as_bytes())?;
  let config = rustls::ClientConfig::builder_with_provider(provider.clone())
    .with_safe_default_protocol_versions()?
    .dangerous()
    .with_custom_certificate_verifier(Arc::new(NoServerVerification(provider)))
    .with_client_auth_cert(client_cert, client_key)?;
  Ok(MakeRustlsConnect::new(config))
}

#[derive(Debug)]
struct NoServerVerification(Arc<CryptoProvider>);

impl ServerCertVerifier for NoServerVerification {
  fn verify_server_cert(
    &self,
    _end_entity: &CertificateDer<'_>,
    _intermediates: &[CertificateDer<'_>],
    _server_name: &ServerName<'_>,
    _ocsp_response: &[u8],
    _now: UnixTime,
  ) -> Result<ServerCertVerified, rustls::Error> {
    Ok(ServerCertVerified::assertion())
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    rustls::crypto::verify_tls12_signature(
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
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    rustls::crypto::verify_tls13_signature(
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
