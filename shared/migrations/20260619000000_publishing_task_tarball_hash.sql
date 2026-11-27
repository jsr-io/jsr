-- Persist the SHA-256 of the gzipped tarball that was uploaded for a publish.
-- This is the same artifact that SLSA provenance attests over (its
-- `subject.digest.sha256`), so storing it here lets the provenance endpoint
-- bind an attestation to the actual published bytes instead of trusting only
-- the attested package name@version. The value is `sha256-<hex>` to match the
-- form already used for the OIDC publish-token `tarball_hash` restriction.
-- NULL for tasks created before this migration (and for tasks whose upload
-- never completed).
ALTER TABLE publishing_tasks ADD COLUMN tarball_hash text;
