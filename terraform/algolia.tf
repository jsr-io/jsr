// Copyright 2024 the JSR authors. All rights reserved. MIT license.

# Algolia search indices and their settings are managed declaratively here.
# Reindex tools (tools/algolia_*_reindex.ts) only push documents — they inherit
# these settings via the atomic replace/move, so settings live in one place.

# Staging and prod share one Algolia app; indices are namespaced by environment
# (e.g. `staging_packages`, `prod_packages`). The reindex workflows derive the
# same `<env>_<corpus>` names from their environment input.
locals {
  algolia_env            = var.production ? "prod" : "staging"
  algolia_packages_index = "${local.algolia_env}_packages"
  algolia_symbols_index  = "${local.algolia_env}_symbols"
  algolia_docs_index     = "${local.algolia_env}_docs"
}

resource "algolia_index" "packages" {
  name = local.algolia_packages_index

  attributes_config {
    searchable_attributes = ["name", "scope", "description"]
    attributes_for_faceting = [
      "filterOnly(scope)",
      "filterOnly(runtimeCompat.browser)",
      "filterOnly(runtimeCompat.deno)",
      "filterOnly(runtimeCompat.node)",
      "filterOnly(runtimeCompat.workerd)",
      "filterOnly(runtimeCompat.bun)",
    ]
    # Everything the frontend reads off a hit (see PackageHit.tsx). objectID is
    # always returned. Without this, faceted attributes get dropped from results
    # and the package card crashes on a missing runtimeCompat.
    attributes_to_retrieve = [
      "scope",
      "name",
      "description",
      "runtimeCompat",
      "score",
    ]
  }

  ranking_config {
    custom_ranking = ["desc(score)"]
  }
}

resource "algolia_index" "symbols" {
  name = local.algolia_symbols_index

  attributes_config {
    attributes_for_faceting = [
      "filterOnly(scope)",
      "filterOnly(package)",
    ]
  }
}

resource "algolia_index" "docs" {
  name = local.algolia_docs_index

  attributes_config {
    searchable_attributes = ["header", "headerParts", "content"]
    # Everything the frontend reads off a docs hit (see GlobalSearch.tsx).
    attributes_to_retrieve = [
      "path",
      "header",
      "headerParts",
      "slug",
      "content",
    ]
  }
}

# Write key used by the API to index on publish (add/delete objects, and
# deleteByQuery for symbols). Stored in Secret Manager (see secrets.tf) and
# injected into Cloud Run. The reindex tools, which create transient `*_tmp`
# indices, authenticate with the admin key instead, so this key stays scoped to
# the live indices and to object writes only.
resource "algolia_api_key" "write" {
  description = "JSR ${local.algolia_env} API indexing (write)"
  acl         = ["addObject", "deleteObject"]
  indexes = [
    algolia_index.packages.name,
    algolia_index.symbols.name,
    algolia_index.docs.name,
  ]
}

# Search-only keys shipped to the browser (see cloudflare_frontend.tf). Public
# by design — they can only run search queries against their index.
resource "algolia_api_key" "packages_search" {
  description = "JSR ${local.algolia_env} frontend packages search (public)"
  acl         = ["search"]
  indexes     = [algolia_index.packages.name]
}

resource "algolia_api_key" "docs_search" {
  description = "JSR ${local.algolia_env} frontend docs search (public)"
  acl         = ["search"]
  indexes     = [algolia_index.docs.name]
}
