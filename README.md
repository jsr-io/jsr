# jsr.io

<img src="./frontend/static/logo.png" width="200" align="right" alt="the jsr logo">

This is the source code for https://jsr.io, the new JavaScript registry.

<!--deno-fmt-ignore-start-->
> [!IMPORTANT]
> The rest of this README is only relevant to those interested in contributing
> to the jsr.io registry. If you are looking for information on how to use the
> registry, please see https://jsr.io/docs.
<!--deno-fmt-ignore-end-->

## Project Information

**Goals**

- Robust
- Low maintenance
- Cheap
- Open source

**Implementation details**

- Modules and package metadata are stored on Google Cloud Storage (GCS)
- npm compatibility tarballs are stored on Google Cloud Storage (GCS)
- Management API is implemented in Rust and runs on Google Cloud Run
- Frontend uses Fresh and is running on Google Cloud Run in 6 regions
- https://jsr.io, https://api.jsr.io, and https://npm.jsr.io are served by a
  Google Cloud Load Balancer
  - Google Cloud CDN is used for caching
  - Module, package metadata, and npm tarballs is served directly from GCS
  - /api requests are proxied to the management API
  - All other requests are proxied to the frontend
- Data is stored in PostgreSQL (using Google Cloud SQL)
  - The database is highly available
  - Not used for serving registry requests
- Distributed tracing using Google Cloud Trace (and Jaeger in development)

## Getting started (frontend only)

If you are just interested in making changes to the frontend, you can run the
frontend in a development mode that connects to the production API.

### Prerequisites

- Clone this repo
- Install Deno (https://deno.land/#installation)
- Add the following to your `/etc/hosts`
  ```
  127.0.0.1       jsr.test
  127.0.0.1       api.jsr.test
  127.0.0.1       npm.jsr.test
  ```

### Running jsr

1. `deno task prod:frontend`

You can view the registry at `http://jsr.test`. This frontend is connected to
the production API - use it with the same care that you would use the live
registry.

## Getting started (entire stack)

In this mode, you will run the frontend and the API locally. This is useful for
making changes to the API.

### Prerequisites

- Clone this repo
- Install Deno (https://deno.land/#installation)
- Install Rust (https://rustup.rs/)
- Add the following to your `/etc/hosts`
  ```
  127.0.0.1       jsr.test
  127.0.0.1       api.jsr.test
  127.0.0.1       npm.jsr.test
  ```

- Set up `api/.env` file:
  - For **@denoland employees**: Download the `.env` file from 1Password (it's
    named `jsr local .env`), and set up `DATABASE_URL` to point to your local
    Postgres database.
  - For **everyone else**:
    1. Create a GitHub App (https://github.com/settings/apps/new)
       - Callback URL: "http://jsr.test/login/callback"
       - Check "Request user authorization (OAuth) during installation"
       - Disable "Webhook"
       - Set "Account permissions" > "Email addresses" to "Read-only"
    2. Copy `api/.env.example` to `api/.env`
    3. Set `GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET` to the values from the
       GitHub App you created in step 1.
    4. Set `DATABASE_URL` to point to your local Postgres database.
- Install `sqlx` by running `cargo install sqlx-cli`

**macOS**

- Postgres installed and running: `brew install postgresql`
- Postgres database created with `createdb registry`
- Postgres user created and granted access to the database
- Run `cd api`
- Run `cargo sqlx migrate run`
  - If you get the error `role "postgres" does not exist`, run
    `createuser -s postgres`.

**Linux**

- `docker` & `docker-compose` installed and running
- Run `cd api`
- Run `cargo sqlx migrate run`
  - If you get the error `role "postgres" does not exist`, run
    `createuser -s postgres`.

### Running jsr

1. `deno task services:macos`, `deno task services:macos:amd` or `deno task services:linux` in one terminal
2. `deno task dev:api` in another terminal
3. `deno task dev:frontend` in another terminal

You can view the registry at `http://jsr.test`. The API can be found at
`http://api.jsr.test`.

### Publishing a package to the local dev environment

1. Create a new directory with a `deno.json`
2. `cd` into that directory
3. Run `JSR_URL=http://jsr.test deno publish`

### Populating local dev environment with additional data

It may be helpful to have a large variety of packages published to your local
dev environment to simulate a live environment. The quickest way to fill the
registry with data is to publish
[deno_std](https://github.com/denoland/deno_std) to the registry. This can be
done via the following steps:

1. Clone https://github.com/denoland/deno_std in the same parent folder as the
   `jsr` project
2. Make sure to [make yourself a staff user/admin](#making-yourself-a-staff-useradmin)
3. Run `JSR_URL=http://jsr.test deno publish` to publish all of the @std
   packages to your local dev environment.

### Making yourself a staff user/admin

1. Run `psql registry`
2. Run `SELECT name,github_id from users;`
3. You should see a table with your name and GitHub ID. Copy your GitHub ID.
4. Run `UPDATE users SET is_staff = true WHERE github_id = xxxxxxx;`, replacing
   `xxxxxxx` with your copied GitHub ID from the previous step.
5. You should see a success message confirming one row has been updated.

### Migrating the database

When the database schema has been changed, you can migrate the local database by
running this command:

```sh
cd api; sqlx migrate run
```

### Loading bad words

To load bad words into the database:

1. Download https://cloud.google.com/sql/docs/postgres/sql-proxy
2. Run in a terminal `cloud-sql-proxy -g [database connection string] -p 5433`
3. Create a `bad_words.sql` file, with the contents as:

```sql
INSERT INTO bad_words (word) VALUES
    ('word_1'),
    -- more words
    ('word_2');
```

4. In a separate terminal window run
   `psql postgres://127.0.0.1:5433/registry --user [your username] -f bad_words.sql`,
   and provide the password for the provided username.

### Other

During local dev, traces are sent to Jaeger. You can view them at
http://localhost:16686. You can find traces in API HTTP requests by inspecting
the `x-deno-ray` header.
