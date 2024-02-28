// Copyright 2024 the JSR authors. All rights reserved. MIT license.
resource "google_artifact_registry_repository" "registry" {
  location      = "us-central1"
  repository_id = "registry"
  format        = "DOCKER"
  docker_config {
    immutable_tags = true
  }
}

resource "google_artifact_registry_repository_iam_member" "registry_reader" {
  for_each   = var.registry_reader_service_accounts
  location   = google_artifact_registry_repository.registry.location
  project    = google_artifact_registry_repository.registry.project
  repository = google_artifact_registry_repository.registry.name
  role       = "roles/artifactregistry.reader"
  member     = "serviceAccount:${each.value}"
}

output "registry_repository_url" {
  value = "${google_artifact_registry_repository.registry.location}-docker.pkg.dev/${google_artifact_registry_repository.registry.project}/${google_artifact_registry_repository.registry.name}"
}
