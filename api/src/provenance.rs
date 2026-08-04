// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use anyhow::{Result, bail};
use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use base64::prelude::BASE64_URL_SAFE;
use serde::Deserialize;
use serde::Serialize;
use x509_parser::parse_x509_certificate;
use x509_parser::pem::parse_x509_pem;
use x509_parser::prelude::ASN1Time;
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
/// be valid at `now`, carry the GitHub Actions OIDC issuer, and have a
/// `github.com/<owner>/<repo>/...` SAN. Returns the repository identity.
fn verify_certificate_identity(
  cert: &x509_parser::certificate::X509Certificate,
  now: ASN1Time,
) -> Result<RepoIdentity> {
  if !cert.validity().is_valid_at(now) {
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

/// Whether a SLSA subject digest matches the digest of the published version
/// manifest. Both sides are bare hex; comparison is case-insensitive.
fn digest_matches(expected_sha256: &str, subject_sha256: &str) -> bool {
  !expected_sha256.is_empty()
    && !subject_sha256.is_empty()
    && subject_sha256.eq_ignore_ascii_case(expected_sha256)
}

/// Verify a provenance bundle and return the Rekor transparency-log index.
///
/// `subject_name` is the package coordinate (`pkg:jsr/@scope/name@version`) the
/// attestation must be for. `expected_manifest_digest` is the hex SHA-256 of the
/// published `<version>_meta.json`; the attestation's subject digest must match
/// it. That manifest lists a checksum for every file in the version, so binding
/// the attestation to it transitively binds it to the published contents.
/// `expected_repo`, when present, is the GitHub repository linked to the
/// package; the signing certificate's identity must match it.
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
///  4. Only then is the now-trusted payload parsed and its subject name and
///     digest checked. The digest check binds the attestation to the exact
///     manifest published for the version, not merely to the name@version
///     string.
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
  expected_manifest_digest: &str,
  bundle: ProvenanceBundle,
) -> Result<String> {
  verify_at(
    subject_name,
    expected_repo,
    expected_manifest_digest,
    bundle,
    ASN1Time::now(),
  )
}

/// [`verify`], with the instant the signing certificate's validity window is
/// checked against made explicit. Fulcio certificates are only valid for ten
/// minutes, so tests that use a real captured bundle must pin `now` to the time
/// the bundle was produced.
fn verify_at(
  subject_name: String,
  expected_repo: Option<(String, String)>,
  expected_manifest_digest: &str,
  bundle: ProvenanceBundle,
  now: ASN1Time,
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
  let repo = verify_certificate_identity(&x509, now)?;
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

  // The attested digest must match the manifest actually published.
  if !digest_matches(expected_manifest_digest, &subject.digest.sha256) {
    bail!(
      "Invalid subject digest: attested {}, published manifest is {}",
      subject.digest.sha256,
      expected_manifest_digest
    );
  }

  let tls = &bundle.verification_material.tlog_entries[0];
  Ok(tls.log_index.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;
  use base64::prelude::BASE64_URL_SAFE;

  /// A real provenance bundle, reassembled from Rekor log entry 2313255666:
  /// the publish of `@stsoftware/neat-ai@6.2.0` from `stSoftwareAU/NEAT-AI` on
  /// 2026-08-01. Fulcio certificates live for ten minutes, so tests using this
  /// bundle pin the verification instant to `NEAT_AI_SIGNED_AT`.
  const NEAT_AI_CERT: &str =
    include_str!("../testdata/provenance/neat-ai-6.2.0.cert.pem");
  const NEAT_AI_PAYLOAD: &str =
    include_str!("../testdata/provenance/neat-ai-6.2.0.payload.json");
  const NEAT_AI_SIG: &str =
    include_str!("../testdata/provenance/neat-ai-6.2.0.sig.b64");
  /// 2026-08-01T17:17:00Z — inside the certificate's 17:12:30–17:22:30 window.
  const NEAT_AI_SIGNED_AT: i64 = 1785604620;
  /// SHA-256 of `https://jsr.io/@stsoftware/neat-ai/6.2.0_meta.json`, which is
  /// what the bundle's `subject.digest.sha256` attests.
  const NEAT_AI_MANIFEST_DIGEST: &str =
    "9141591496a48c5b815722bf6aa16e85083729f0ed1a5abbccacbbd1d4b41e82";

  fn neat_ai_bundle() -> ProvenanceBundle {
    ProvenanceBundle {
      media_type: "application/vnd.dev.sigstore.bundle+json;version=0.1"
        .to_string(),
      content: SignatureBundle {
        case: "dsseEnvelope".to_string(),
        dsse_envelope: Envelope {
          payload_type: "application/vnd.in-toto+json".to_string(),
          payload: BASE64_STANDARD.encode(NEAT_AI_PAYLOAD),
          signatures: [Signature {
            keyid: String::new(),
            sig: NEAT_AI_SIG.trim().to_string(),
          }],
        },
      },
      verification_material: VerificationMaterial {
        content: VerificationMaterialContent {
          case: "x509CertificateChain".to_string(),
          x509_certificate_chain: X509CertificateChain {
            certificates: [X509Certificate {
              raw_bytes: NEAT_AI_CERT.to_string(),
            }],
          },
        },
        tlog_entries: [TlogEntry {
          log_index: 2313255666,
        }],
      },
    }
  }

  fn verify_neat_ai(
    expected_repo: Option<(String, String)>,
    expected_manifest_digest: &str,
  ) -> Result<String> {
    verify_at(
      "pkg:jsr/@stsoftware/neat-ai@6.2.0".to_string(),
      expected_repo,
      expected_manifest_digest,
      neat_ai_bundle(),
      ASN1Time::from_timestamp(NEAT_AI_SIGNED_AT).unwrap(),
    )
  }

  /// The happy path. Regression test for jsr-io/jsr#1474: every check in
  /// `verify` was individually covered, but nothing asserted that a genuine
  /// bundle is *accepted*, so a wrong `expected_digest` (the tarball hash rather
  /// than the manifest digest the Deno CLI actually attests) silently rejected
  /// all provenance for a month.
  #[test]
  fn verify_accepts_a_real_github_actions_bundle() {
    let log_index = verify_neat_ai(
      Some(("stSoftwareAU".to_string(), "NEAT-AI".to_string())),
      NEAT_AI_MANIFEST_DIGEST,
    )
    .unwrap();
    assert_eq!(log_index, "2313255666");

    // The linked repository is optional; without one the bundle still verifies.
    assert!(verify_neat_ai(None, NEAT_AI_MANIFEST_DIGEST).is_ok());
    // Repository comparison is case-insensitive.
    assert!(
      verify_neat_ai(
        Some(("stsoftwareau".to_string(), "neat-ai".to_string())),
        NEAT_AI_MANIFEST_DIGEST,
      )
      .is_ok()
    );
  }

  #[test]
  fn verify_rejects_a_real_bundle_for_another_repository() {
    let err = verify_neat_ai(
      Some(("evil".to_string(), "NEAT-AI".to_string())),
      NEAT_AI_MANIFEST_DIGEST,
    )
    .unwrap_err();
    assert!(err.to_string().contains("does not match"), "{err}");
  }

  #[test]
  fn verify_rejects_a_real_bundle_with_the_wrong_digest() {
    let err = verify_neat_ai(
      None,
      "0000000000000000000000000000000000000000000000000000000000000000",
    )
    .unwrap_err();
    assert!(err.to_string().contains("Invalid subject digest"), "{err}");
  }

  #[test]
  fn verify_rejects_an_expired_certificate() {
    let err = verify_at(
      "pkg:jsr/@stsoftware/neat-ai@6.2.0".to_string(),
      None,
      NEAT_AI_MANIFEST_DIGEST,
      neat_ai_bundle(),
      ASN1Time::from_timestamp(NEAT_AI_SIGNED_AT + 3600).unwrap(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("expired"), "{err}");
  }

  #[test]
  fn verify_rejects_a_tampered_payload() {
    // The payload is attacker-controlled until the DSSE signature verifies, so
    // swapping in a subject for a different package must be caught there.
    let mut bundle = neat_ai_bundle();
    bundle.content.dsse_envelope.payload = BASE64_STANDARD.encode(
      NEAT_AI_PAYLOAD.replace("@stsoftware/neat-ai", "@attacker/neat-ai"),
    );
    let err = verify_at(
      "pkg:jsr/@attacker/neat-ai@6.2.0".to_string(),
      None,
      NEAT_AI_MANIFEST_DIGEST,
      bundle,
      ASN1Time::from_timestamp(NEAT_AI_SIGNED_AT).unwrap(),
    )
    .unwrap_err();
    assert!(
      err
        .to_string()
        .contains("DSSE signature verification failed"),
      "{err}"
    );
  }

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

  #[test]
  fn digest_matches_compares_against_published_manifest_digest() {
    let hex =
      "1c3b44ea2ac86f7133791a4a004f633993784da783a3e0f5c226dd7a4141f9f5";

    assert!(digest_matches(hex, hex));
    // Case-insensitive: subject digests may arrive uppercase.
    assert!(digest_matches(hex, &hex.to_uppercase()));

    // A different digest must not match (the core gap this closes).
    let other =
      "0000000000000000000000000000000000000000000000000000000000000000";
    assert!(!digest_matches(hex, other));
    // Neither side may be empty, so an absent digest never verifies.
    assert!(!digest_matches(hex, ""));
    assert!(!digest_matches("", hex));
    assert!(!digest_matches("", ""));
    // Regression test for jsr-io/jsr#1474: the Deno CLI attests the digest of
    // the published `<version>_meta.json`, not of the uploaded tarball, so a
    // `sha256-<hex>` tarball hash must not be conflated with it.
    assert!(!digest_matches(&format!("sha256-{hex}"), hex));
  }
}
