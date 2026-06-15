// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use anyhow::{Result, bail};
use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use base64::prelude::BASE64_URL_SAFE;
use serde::Deserialize;
use serde::Serialize;
use x509_parser::parse_x509_certificate;
use x509_parser::pem::parse_x509_pem;
use x509_parser::prelude::GeneralName;
use x509_parser::public_key::PublicKey;

/// The OIDC issuer that GitHub Actions uses when requesting a Fulcio signing
/// certificate. JSR provenance is only ever produced by GitHub Actions, so the
/// signing certificate must carry this issuer.
const GITHUB_ACTIONS_ISSUER: &str =
  "https://token.actions.githubusercontent.com";

/// Sigstore/Fulcio X.509v3 extension OIDs that carry the OIDC issuer. `.1` holds
/// the issuer as a raw string (v1), `.1.8` wraps it in a DER `UTF8String` (v2).
/// See <https://github.com/sigstore/fulcio/blob/main/docs/oid-info.md>.
const FULCIO_OID_ISSUER_V1: &str = "1.3.6.1.4.1.57264.1.1";
const FULCIO_OID_ISSUER_V2: &str = "1.3.6.1.4.1.57264.1.8";

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
  pub subject: ProvenanceAttestationSubject,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProvenanceAttestationSubject {
  Subjects(Vec<Subject>),
  // NOTE: this should be removed in the future. It is only here to support
  // old Deno CLI versions that sent invalid SLSA attestations where the subject
  // was not wrapped in an array.
  Subject(Subject),
}

/// Decode a base64 value, tolerating both the standard (`+`/`/`) and URL-safe
/// (`-`/`_`) alphabets. DSSE payloads and signatures are base64-encoded, but
/// some clients emit them using the URL-safe alphabet.
fn decode_base64(value: &str) -> Result<Vec<u8>> {
  match BASE64_STANDARD.decode(value) {
    Ok(bytes) => Ok(bytes),
    Err(_) => Ok(BASE64_URL_SAFE.decode(value)?),
  }
}

/// DSSE Pre-Authentication Encoding (PAE) of an envelope, per the DSSE spec:
/// `"DSSEv1" SP LEN(type) SP type SP LEN(payload) SP payload`, where `LEN` is
/// the ASCII-decimal byte length and `SP` is a single space. This is the exact
/// byte string that the signature is computed over.
fn dsse_pae(payload_type: &str, payload: &[u8]) -> Vec<u8> {
  let mut pae = Vec::new();
  pae.extend_from_slice(b"DSSEv1 ");
  pae.extend_from_slice(payload_type.len().to_string().as_bytes());
  pae.push(b' ');
  pae.extend_from_slice(payload_type.as_bytes());
  pae.push(b' ');
  pae.extend_from_slice(payload.len().to_string().as_bytes());
  pae.push(b' ');
  pae.extend_from_slice(payload);
  pae
}

/// Verify an ECDSA P-256 signature over `msg` against the raw (uncompressed) EC
/// point `ec_point`. Sigstore DSSE signatures are DER (ASN.1) encoded, but we
/// also accept the fixed-width (P1363) encoding for robustness.
fn verify_ecdsa_p256(ec_point: &[u8], msg: &[u8], sig: &[u8]) -> bool {
  use ring::signature;
  let algs: [&dyn signature::VerificationAlgorithm; 2] = [
    &signature::ECDSA_P256_SHA256_ASN1,
    &signature::ECDSA_P256_SHA256_FIXED,
  ];
  algs.into_iter().any(|alg| {
    signature::UnparsedPublicKey::new(alg, ec_point)
      .verify(msg, sig)
      .is_ok()
  })
}

/// A repository identity extracted from a Fulcio signing certificate's SAN.
struct RepoIdentity {
  owner: String,
  name: String,
}

/// Validate that `cert` is a GitHub Actions Fulcio signing certificate: it must
/// be within its validity window, carry the GitHub Actions OIDC issuer, and
/// have a `github.com/<owner>/<repo>/...` SAN. Returns the repository identity.
fn verify_certificate_identity(
  cert: &x509_parser::certificate::X509Certificate,
) -> Result<RepoIdentity> {
  if !cert.validity().is_valid() {
    bail!("provenance certificate is expired or not yet valid");
  }

  // The certificate must have been issued to a GitHub Actions OIDC identity.
  let has_github_issuer = cert.extensions().iter().any(|ext| {
    let oid = ext.oid.to_id_string();
    (oid == FULCIO_OID_ISSUER_V1 || oid == FULCIO_OID_ISSUER_V2)
      && find_subslice(ext.value, GITHUB_ACTIONS_ISSUER.as_bytes())
  });
  if !has_github_issuer {
    bail!("provenance certificate was not issued to GitHub Actions");
  }

  // The SAN encodes the signing workflow identity as a URI such as
  // `https://github.com/<owner>/<repo>/.github/workflows/<file>@<ref>`.
  let san = cert
    .subject_alternative_name()?
    .ok_or_else(|| anyhow::anyhow!("provenance certificate has no SAN"))?;
  for name in &san.value.general_names {
    if let GeneralName::URI(uri) = name
      && let Some(repo) = parse_github_repo(uri)
    {
      return Ok(repo);
    }
  }
  bail!("provenance certificate SAN is not a github.com workflow identity")
}

/// Parse `https://github.com/<owner>/<repo>/...` into its owner and repo.
fn parse_github_repo(uri: &str) -> Option<RepoIdentity> {
  let rest = uri.strip_prefix("https://github.com/")?;
  let mut parts = rest.splitn(3, '/');
  let owner = parts.next().filter(|s| !s.is_empty())?;
  let name = parts.next().filter(|s| !s.is_empty())?;
  Some(RepoIdentity {
    owner: owner.to_string(),
    name: name.to_string(),
  })
}

/// Whether `haystack` contains `needle`.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> bool {
  haystack.windows(needle.len()).any(|w| w == needle)
}

/// Verify a provenance bundle and return the Rekor transparency-log index.
///
/// `subject_name` is the package coordinate (`pkg:jsr/@scope/name@version`) the
/// attestation must be for. `expected_repo`, when present, is the GitHub
/// repository linked to the package; the signing certificate's identity must
/// match it.
///
/// Verification:
///  1. The signing (leaf) certificate chains to the Fulcio intermediate.
///  2. The leaf certificate is a valid GitHub Actions identity (within validity
///     window, GitHub Actions OIDC issuer, `github.com/<owner>/<repo>` SAN) and,
///     if the package is linked to a repository, that the repository matches.
///  3. The DSSE envelope signature is valid for the leaf certificate's key. This
///     is what binds the (otherwise attacker-supplied) payload to the
///     certificate: without the certificate's private key the signature cannot
///     be forged.
///  4. Only then is the now-trusted payload parsed and its subject name checked.
///
/// NOTE: Rekor transparency-log inclusion is not yet cryptographically verified
/// here (it would require embedding Sigstore's Rekor public key and replicating
/// the Signed-Entry-Timestamp canonicalization). The steps above already bind
/// the attestation to a real GitHub Actions build of this package, which is what
/// closes badge forgery; inclusion-proof verification is defense-in-depth that
/// must be landed with real bundle fixtures to avoid rejecting valid provenance.
pub fn verify(
  subject_name: String,
  expected_repo: Option<(String, String)>,
  bundle: ProvenanceBundle,
) -> Result<String> {
  let key = &bundle
    .verification_material
    .content
    .x509_certificate_chain
    .certificates[0]
    .raw_bytes;
  let (_, pem) = parse_x509_pem(key.as_bytes())?;
  let (_, x509) = parse_x509_certificate(&pem.contents)?;

  let (_, fulcio_pem) = parse_x509_pem(FULCIO_CERT)?;
  let (_, fulcio) = parse_x509_certificate(&fulcio_pem.contents)?;

  // 1. The signing certificate must be issued by the Fulcio intermediate.
  x509.verify_signature(Some(fulcio.public_key()))?;

  // 2. The signing certificate must be a GitHub Actions identity, optionally
  //    matching the repository linked to the package.
  let repo = verify_certificate_identity(&x509)?;
  if let Some((owner, name)) = expected_repo
    && (!owner.eq_ignore_ascii_case(&repo.owner)
      || !name.eq_ignore_ascii_case(&repo.name))
  {
    bail!(
      "provenance certificate identity {}/{} does not match the package's linked repository {}/{}",
      repo.owner,
      repo.name,
      owner,
      name
    );
  }

  // 3. The DSSE envelope signature must verify against the certificate's key.
  //    Until this passes, the payload is attacker-controlled and untrusted.
  let envelope = &bundle.content.dsse_envelope;
  let payload = decode_base64(&envelope.payload)?;
  let signature = decode_base64(&envelope.signatures[0].sig)?;
  let pae = dsse_pae(&envelope.payload_type, &payload);
  let ec_point = match x509.public_key().parsed()? {
    PublicKey::EC(ec) => ec.data().to_vec(),
    _ => bail!("provenance certificate does not use an EC key"),
  };
  if !verify_ecdsa_p256(&ec_point, &pae, &signature) {
    bail!("provenance DSSE signature verification failed");
  }

  // 4. The (now trusted) payload must attest exactly this package version.
  let subject =
    match serde_json::from_slice::<ProvenanceAttestation>(&payload)?.subject {
      ProvenanceAttestationSubject::Subjects(subjects) => {
        if subjects.len() != 1 {
          bail!("Invalid subject");
        }
        subjects.into_iter().next().unwrap()
      }
      ProvenanceAttestationSubject::Subject(subject) => subject,
    };
  if subject.name != subject_name {
    bail!("Invalid subject name");
  }

  let tls = &bundle.verification_material.tlog_entries[0];
  Ok(tls.log_index.to_string())
}

#[cfg(test)]
mod tests {
  use super::decode_base64;
  use super::dsse_pae;
  use super::parse_github_repo;
  use base64::Engine as _;
  use base64::prelude::BASE64_STANDARD;
  use base64::prelude::BASE64_URL_SAFE;

  #[test]
  fn decode_base64_accepts_standard_and_url_safe() {
    // These bytes encode to "+/8=" in standard base64 and "-_8=" in URL-safe
    // base64, exercising both alphabet-specific characters (`+`/`/` vs `-`/`_`).
    let raw = [0xfb_u8, 0xff];

    let standard = BASE64_STANDARD.encode(raw);
    assert!(standard.contains('+') && standard.contains('/'));
    assert_eq!(decode_base64(&standard).unwrap(), raw);

    // Regression test for jsr-io/jsr#1312: some clients emit the DSSE payload
    // using the URL-safe alphabet, which the standard decoder rejected with
    // "Invalid symbol 45, offset ..." (45 being `-`).
    let url_safe = BASE64_URL_SAFE.encode(raw);
    assert!(url_safe.contains('-') && url_safe.contains('_'));
    assert_eq!(decode_base64(&url_safe).unwrap(), raw);
  }

  #[test]
  fn dsse_pae_matches_spec() {
    // Example from the DSSE spec (SERIALIZED_BODY = "hello world", type = "http://example.com/HelloWorld").
    let pae = dsse_pae("http://example.com/HelloWorld", b"hello world");
    assert_eq!(
      pae,
      b"DSSEv1 29 http://example.com/HelloWorld 11 hello world"
    );
  }

  #[test]
  fn parse_github_repo_extracts_owner_and_name() {
    let repo = parse_github_repo(
      "https://github.com/littledivy/test_provenance/.github/workflows/publish.yml@refs/heads/main",
    )
    .unwrap();
    assert_eq!(repo.owner, "littledivy");
    assert_eq!(repo.name, "test_provenance");

    assert!(parse_github_repo("https://gitlab.com/foo/bar").is_none());
    assert!(parse_github_repo("https://github.com/").is_none());
    assert!(parse_github_repo("https://github.com/onlyowner").is_none());
  }
}
