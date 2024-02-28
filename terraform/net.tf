// Copyright 2024 the JSR authors. All rights reserved. MIT license.
resource "google_compute_network" "main" {
  name = "main"
}

resource "google_vpc_access_connector" "default" {
  name          = "vpc-connector"
  region        = "us-central1"
  ip_cidr_range = "10.8.0.0/28"
  network       = google_compute_network.main.name
}