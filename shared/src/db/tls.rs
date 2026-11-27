// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use std::sync::Arc;

use rustls::CertificateError;
use rustls::DigitallySignedStruct;
use rustls::SignatureScheme;
use rustls::client::WebPkiServerVerifier;
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
/// Connections are to Cloud SQL by IP, but its server certificate is only
/// valid for the instance's `*.sql.goog` DNS name, so the name is never
/// checked. With a `server_ca` the certificate has to chain to it, the same
/// as libpq's `sslmode=verify-ca`. Without one it is not verified at all, the
/// same as `sslmode=require`, which is for links that stay inside the VPC.
pub fn make_tls_connect(tls: &DbTls) -> anyhow::Result<MakeRustlsConnect> {
  let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
  let verifier: Arc<dyn ServerCertVerifier> = match &tls.server_ca {
    Some(server_ca) => {
      let mut roots = rustls::RootCertStore::empty();
      for cert in CertificateDer::pem_slice_iter(server_ca.as_bytes()) {
        roots.add(cert?)?;
      }
      Arc::new(VerifyCa(
        WebPkiServerVerifier::builder_with_provider(
          Arc::new(roots),
          provider.clone(),
        )
        .build()?,
      ))
    }
    None => Arc::new(NoServerVerification(provider.clone())),
  };
  let client_cert = CertificateDer::pem_slice_iter(tls.client_cert.as_bytes())
    .collect::<Result<Vec<_>, _>>()?;
  let client_key = PrivateKeyDer::from_pem_slice(tls.client_key.as_bytes())?;
  let config = rustls::ClientConfig::builder_with_provider(provider.clone())
    .with_safe_default_protocol_versions()?
    .dangerous()
    .with_custom_certificate_verifier(verifier)
    .with_client_auth_cert(client_cert, client_key)?;
  Ok(MakeRustlsConnect::new(config))
}

/// Verifies that the server's certificate chains to the roots, but not that
/// it is for the name connected to.
#[derive(Debug)]
struct VerifyCa(Arc<WebPkiServerVerifier>);

impl ServerCertVerifier for VerifyCa {
  fn verify_server_cert(
    &self,
    end_entity: &CertificateDer<'_>,
    intermediates: &[CertificateDer<'_>],
    server_name: &ServerName<'_>,
    ocsp_response: &[u8],
    now: UnixTime,
  ) -> Result<ServerCertVerified, rustls::Error> {
    // The name is checked last, so this error means the chain is valid.
    match self.0.verify_server_cert(
      end_entity,
      intermediates,
      server_name,
      ocsp_response,
      now,
    ) {
      Err(rustls::Error::InvalidCertificate(
        CertificateError::NotValidForName
        | CertificateError::NotValidForNameContext { .. },
      )) => Ok(ServerCertVerified::assertion()),
      result => result,
    }
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    self.0.verify_tls12_signature(message, cert, dss)
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    self.0.verify_tls13_signature(message, cert, dss)
  }

  fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
    self.0.supported_verify_schemes()
  }
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
