CREATE TABLE npm_tarballs (
  scope text NOT NULL,
  name text NOT NULL,
  version text NOT NULL,
  revision integer NOT NULL,
  sha1 text NOT NULL,
  sha512 text NOT NULL,
  size integer CHECK (size >= 0) NOT NULL,
  PRIMARY KEY (scope, name, version, revision),
  FOREIGN KEY (scope, name, version) REFERENCES package_versions (scope, name, version) ON DELETE CASCADE
);
SELECT manage_updated_at('npm_tarballs');
CREATE INDEX npm_tarballs_by_package_version ON npm_tarballs (scope, name, version);