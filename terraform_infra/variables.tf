// Copyright 2024 the JSR authors. All rights reserved. MIT license.
variable "gcp_project" {
  type = string
}

variable "registry_reader_service_accounts" {
  type = set(string)
}
