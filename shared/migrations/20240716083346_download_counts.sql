CREATE TABLE version_download_counts_4h (
  scope TEXT NOT NULL REFERENCES scopes (scope) ON DELETE CASCADE,
  package TEXT NOT NULL,
  version TEXT NOT NULL,
  time_bucket TIMESTAMP WITH TIME ZONE NOT NULL,
  count INTEGER NOT NULL,
  PRIMARY KEY (scope, package, version, time_bucket),
  FOREIGN KEY (scope, package) REFERENCES packages (scope, name) ON DELETE CASCADE,
  FOREIGN KEY (scope, package, version) REFERENCES package_versions (scope, name, version) ON DELETE CASCADE
);

CREATE TABLE version_download_counts_24h (
  scope TEXT NOT NULL REFERENCES scopes (scope) ON DELETE CASCADE,
  package TEXT NOT NULL,
  version TEXT NOT NULL,
  time_bucket TIMESTAMP WITH TIME ZONE NOT NULL,
  count INTEGER NOT NULL,
  PRIMARY KEY (scope, package, version, time_bucket),
  FOREIGN KEY (scope, package) REFERENCES packages (scope, name) ON DELETE CASCADE,
  FOREIGN KEY (scope, package, version) REFERENCES package_versions (scope, name, version) ON DELETE CASCADE
);
