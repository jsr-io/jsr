// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Migrates the database and upserts the service account token
// (`registry_api setup`), from the machine running terraform, whenever the
// API's build changes. The workers depend on this, so a new
// version only starts serving once the schema is migrated; nothing sets up
// on its own (`--setup=false`).

resource "terraform_data" "api_setup" {
  triggers_replace = [cloudflare_worker_version.jsr_api.id]

  provisioner "local-exec" {
    # Terraform hides the output of a command it gives secrets to, so it goes
    # to a file, which CI prints.
    command = "${path.module}/../target/release/registry_api setup > ${path.module}/api-setup.log 2>&1"

    environment = {
      # Over the public IP, as Hyperdrive does: only a holder of the client
      # certificate can connect, and the server has to present a certificate
      # of the instance's CA.
      DATABASE_URL   = "postgres://${google_sql_user.api.name}:${google_sql_user.api.password}@${google_sql_database_instance.main_pg15.public_ip_address}/${google_sql_database.database.name}"
      DB_CLIENT_CERT = google_sql_ssl_cert.api.cert
      DB_CLIENT_KEY  = google_sql_ssl_cert.api.private_key
      DB_SERVER_CA   = google_sql_database_instance.main_pg15.server_ca_cert[0].cert
      NO_COLOR       = "true"
    }
  }
}
