// Copyright 2024 the JSR authors. All rights reserved. MIT license.
locals {
  postgres_url = "postgres://${google_sql_user.api.name}:${google_sql_user.api.password}@${google_sql_database_instance.main_pg15.private_ip_address}/${google_sql_database.database.name}"
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

      # The Cloudflare Container reaches Cloud SQL over the public IP (its egress
      # isn't pinnable without a Zero Trust dedicated-egress add-on, so the
      # published ranges can't be used as an allowlist). On staging the public IP
      # is therefore left open, and a valid client certificate (mTLS) is the real
      # access boundary instead of a network ACL. Prod keeps the private VPC only
      # and stays on ENCRYPTED_ONLY.
      ssl_mode = var.production ? "ENCRYPTED_ONLY" : "TRUSTED_CLIENT_CERTIFICATE_REQUIRED"

      dynamic "authorized_networks" {
        for_each = var.production ? toset([]) : toset(["0.0.0.0/0"])
        content {
          name  = "container-public-egress"
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

# Client certificate the API presents when connecting over TLS. The Cloudflare
# Container reaches Cloud SQL over the public IP where, once ssl_mode is flipped
# to TRUSTED_CLIENT_CERTIFICATE_REQUIRED, this cert is the access boundary (the
# IP is open but a valid client cert is required). Delivered to both Cloud Run
# (env, see cloud_run_api.tf) and the container (worker secrets, see lb.tf).
resource "google_sql_ssl_cert" "api" {
  common_name = "api-client"
  instance    = google_sql_database_instance.main_pg15.name
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
