// Copyright 2024 the JSR authors. All rights reserved. MIT license.
locals {
  postgres_url = "postgres://${google_sql_user.api.name}:${google_sql_user.api.password}@${google_sql_database_instance.main_pg15.private_ip_address}/${google_sql_database.database.name}"

  # Cloudflare's published egress IPv4 ranges, fetched live so the list stays
  # current. The API container connects to Cloud SQL over the public IP from
  # Cloudflare's network (Cloud Run uses the private IP via the VPC connector
  # instead). Allowlisting these — rather than 0.0.0.0/0 — limits inbound to
  # Cloudflare's network; the real boundary is still TLS + the DB password
  # (and, to follow, a client certificate).
  cloudflare_ipv4 = toset(
    split("\n", trimspace(data.http.cloudflare_ipv4.response_body))
  )
}

data "http" "cloudflare_ipv4" {
  url = "https://www.cloudflare.com/ips-v4"
}

resource "google_sql_database_instance" "main_pg15" {
  name             = "main-pg15"
  database_version = "POSTGRES_15"
  region           = "us-central1"

  settings {
    edition           = "ENTERPRISE"
    tier              = var.production ? "db-custom-4-8192" : "db-f1-micro"
    availability_type = var.production ? "REGIONAL" : "ZONAL"

    disk_type             = "PD_SSD"
    disk_autoresize       = true
    disk_autoresize_limit = 0

    deletion_protection_enabled = true

    ip_configuration {
      ipv4_enabled    = true
      private_network = google_compute_network.main.self_link
      ssl_mode        = "ENCRYPTED_ONLY"

      # Staging only for now: allow the API container to reach the public IP
      # from Cloudflare's network. Not enabled on prod until connectivity is
      # confirmed and a client certificate is in place.
      dynamic "authorized_networks" {
        for_each = var.production ? toset([]) : local.cloudflare_ipv4
        content {
          name  = "cloudflare-${replace(replace(authorized_networks.value, ".", "-"), "/", "_")}"
          value = authorized_networks.value
        }
      }
    }

    backup_configuration {
      enabled                        = true
      point_in_time_recovery_enabled = true
      transaction_log_retention_days = 7
    }

    database_flags {
      name  = "max_connections"
      value = "150"
    }

    insights_config {
      query_insights_enabled = true
    }
  }

  depends_on = [google_service_networking_connection.db_private_vpc_connection]
}

resource "google_sql_database" "database" {
  name     = "registry"
  instance = google_sql_database_instance.main_pg15.name
}

resource "google_sql_user" "api" {
  name     = "api"
  instance = google_sql_database_instance.main_pg15.name
  password = random_password.db_password.result
}

resource "random_password" "db_password" {
  length  = 16
  special = false
}

resource "google_compute_global_address" "db" {
  name          = "db-private-ip-address"
  purpose       = "VPC_PEERING"
  address_type  = "INTERNAL"
  prefix_length = 16
  network       = google_compute_network.main.name
}

resource "google_service_networking_connection" "db_private_vpc_connection" {
  network                 = google_compute_network.main.self_link
  service                 = "servicenetworking.googleapis.com"
  reserved_peering_ranges = [google_compute_global_address.db.name]
}
