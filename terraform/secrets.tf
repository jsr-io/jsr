// Copyright 2024 the JSR authors. All rights reserved. MIT license.
resource "google_secret_manager_secret" "github_client_secret" {
  secret_id = "github-client-secret"
  replication {
    auto {}
  }
}

resource "google_secret_manager_secret_version" "github_client_secret" {
  secret      = google_secret_manager_secret.github_client_secret.id
  secret_data = var.github_client_secret
}

resource "google_secret_manager_secret" "postmark_token" {
  secret_id = "postmark-token"
  replication {
    auto {}
  }
}

resource "google_secret_manager_secret_version" "postmark_token" {
  secret      = google_secret_manager_secret.postmark_token.id
  secret_data = var.postmark_token
}

resource "google_secret_manager_secret" "orama_private_api_key" {
  secret_id = "orama-private-api-key"
  replication {
    auto {}
  }
}

resource "google_secret_manager_secret_version" "orama_private_api_key" {
  secret      = google_secret_manager_secret.orama_private_api_key.id
  secret_data = var.orama_private_api_key
}

resource "google_secret_manager_secret" "orama_index_id" {
  secret_id = "orama-index-id"
  replication {
    auto {}
  }
}

resource "google_secret_manager_secret_version" "orama_index_id" {
  secret      = google_secret_manager_secret.orama_index_id.id
  secret_data = var.orama_index_id
}

