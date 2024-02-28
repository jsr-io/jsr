-- Add migration script here
CREATE TABLE aliases (
    name text NOT NULL,
    major_version integer NOT NULL CHECK (major_version >= 1),

    -- The alias can either point to some internal package, or to an external
    -- npm package.
    target_deno_scope text,
    target_deno_name text,
    target_npm text,

    updated_at timestamptz DEFAULT now() NOT NULL,
    created_at timestamptz DEFAULT now() NOT NULL,

    PRIMARY KEY (name, major_version),
    FOREIGN KEY (target_deno_scope, target_deno_name) REFERENCES packages (scope, name),

    -- Add a CHECK constraint to ensure that either target_npm is non-null or
    -- (target_deno_scope, target_deno_name) is non-null, but not both.
    CONSTRAINT valid_target CHECK (
      (target_npm IS NOT NULL AND target_deno_scope IS NULL AND target_deno_name IS NULL) 
      OR
      (target_npm IS NULL AND target_deno_scope IS NOT NULL AND target_deno_name IS NOT NULL)
    )
);
SELECT manage_updated_at('aliases');