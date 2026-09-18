CREATE UNIQUE INDEX scopes_name_unique_no_dash ON scopes (replace(scope, '-', ''));
CREATE UNIQUE INDEX packages_name_unique_no_dash ON packages (scope, replace(name, '-', ''));