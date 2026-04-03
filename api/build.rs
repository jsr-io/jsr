// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
  let license_data_path =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("license-list-data");
  let details_path = license_data_path.join("json/details");

  println!("cargo::rerun-if-changed={}", details_path.display());

  let mut store = askalono::Store::new();
  store.load_spdx(&details_path, false).unwrap();

  let out_dir = env::var("OUT_DIR").unwrap();
  let cache_path = Path::new(&out_dir).join("license-store.cache");
  let file = fs::File::create(&cache_path).unwrap();
  store.to_cache(file).unwrap();
}
