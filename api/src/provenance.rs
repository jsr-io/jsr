// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use anyhow::{bail, Result};
use base64::prelude::BASE64_STANDARD;
use base64::Engine as _;
use serde::Deserialize;
use serde::Serialize;
use x509_parser::parse_x509_certificate;
use x509_parser::pem::parse_x509_pem;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Signature {
  pub keyid: String,
  pub sig: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
  pub payload_type: String,
  pub payload: String,
  pub signatures: [Signature; 1],
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureBundle {
  #[serde(rename = "$case")]
  pub case: String,
  pub dsse_envelope: Envelope,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X509Certificate {
  pub raw_bytes: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct X509CertificateChain {
  pub certificates: [X509Certificate; 1],
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationMaterialContent {
  #[serde(rename = "$case")]
  pub case: String,
  pub x509_certificate_chain: X509CertificateChain,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlogEntry {
  pub log_index: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationMaterial {
  pub content: VerificationMaterialContent,
  pub tlog_entries: [TlogEntry; 1],
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceBundle {
  pub media_type: String,
  pub content: SignatureBundle,
  pub verification_material: VerificationMaterial,
}

// Fulcio root cert
const FULCIO_CERT: &[u8] = b"-----BEGIN CERTIFICATE-----
MIICGjCCAaGgAwIBAgIUALnViVfnU0brJasmRkHrn/UnfaQwCgYIKoZIzj0EAwMw
KjEVMBMGA1UEChMMc2lnc3RvcmUuZGV2MREwDwYDVQQDEwhzaWdzdG9yZTAeFw0y
MjA0MTMyMDA2MTVaFw0zMTEwMDUxMzU2NThaMDcxFTATBgNVBAoTDHNpZ3N0b3Jl
LmRldjEeMBwGA1UEAxMVc2lnc3RvcmUtaW50ZXJtZWRpYXRlMHYwEAYHKoZIzj0C
AQYFK4EEACIDYgAE8RVS/ysH+NOvuDZyPIZtilgUF9NlarYpAd9HP1vBBH1U5CV7
7LSS7s0ZiH4nE7Hv7ptS6LvvR/STk798LVgMzLlJ4HeIfF3tHSaexLcYpSASr1kS
0N/RgBJz/9jWCiXno3sweTAOBgNVHQ8BAf8EBAMCAQYwEwYDVR0lBAwwCgYIKwYB
BQUHAwMwEgYDVR0TAQH/BAgwBgEB/wIBADAdBgNVHQ4EFgQU39Ppz1YkEZb5qNjp
KFWixi4YZD8wHwYDVR0jBBgwFoAUWMAeX5FFpWapesyQoZMi0CrFxfowCgYIKoZI
zj0EAwMDZwAwZAIwPCsQK4DYiZYDPIaDi5HFKnfxXx6ASSVmERfsynYBiX2X6SJR
nZU84/9DZdnFvvxmAjBOt6QpBlc4J/0DxvkTCqpclvziL6BCCPnjdlIB3Pu3BxsP
mygUY7Ii2zbdCdliiow=
-----END CERTIFICATE-----
";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectDigest {
  pub sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subject {
  pub name: String,
  pub digest: SubjectDigest,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceAttestation {
  pub subject: Subject,
}

pub fn verify(
  subject_name: String,
  bundle: ProvenanceBundle,
) -> Result<String> {
  // Extract subject from the DSSE envelope
  let subject = {
    let payload =
      BASE64_STANDARD.decode(&bundle.content.dsse_envelope.payload)?;
    serde_json::from_slice::<ProvenanceAttestation>(&payload)?.subject
  };

  if subject.name != subject_name {
    bail!("Invalid subject name");
  }

  let key = &bundle
    .verification_material
    .content
    .x509_certificate_chain
    .certificates[0]
    .raw_bytes;
  let (_, pem) = parse_x509_pem(key.as_bytes())?;
  let (_, x509) = parse_x509_certificate(&pem.contents)?;

  let (_, pem) = parse_x509_pem(FULCIO_CERT)?;
  let (_, fulcio) = parse_x509_certificate(&pem.contents)?;

  // Verify that the signing certifcate is signed by
  // the certificate chain.
  let issuer_pub_key = fulcio.public_key();
  x509.verify_signature(Some(issuer_pub_key))?;

  let tls = &bundle.verification_material.tlog_entries[0];
  Ok(tls.log_index.to_string())
}
