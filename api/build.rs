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

  embed_migrations(Path::new(&out_dir));
}

/// Writes `migrations.rs`, the list of migrations `db::migrate` applies. A
/// migration is a `{version}_{description}.sql` file in `migrations/`.
fn embed_migrations(out_dir: &Path) {
  let migrations_path =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
  println!("cargo::rerun-if-changed={}", migrations_path.display());

  let mut migrations = fs::read_dir(&migrations_path)
    .unwrap()
    .map(|entry| entry.unwrap().path())
    .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
    .map(|path| {
      let stem = path.file_stem().unwrap().to_str().unwrap();
      let (version, description) = stem
        .split_once('_')
        .unwrap_or_else(|| panic!("invalid migration name: {stem}"));
      let version: i64 = version
        .parse()
        .unwrap_or_else(|_| panic!("invalid migration version: {stem}"));
      // Same description as sqlx's migrator recorded.
      (version, description.replace('_', " "), path)
    })
    .collect::<Vec<_>>();
  migrations.sort_by_key(|(version, ..)| *version);

  let mut out = String::from("pub static MIGRATIONS: &[Migration] = &[\n");
  for (version, description, path) in migrations {
    out.push_str(&format!(
      "  Migration {{ version: {version}, description: {description:?}, sql: include_str!({:?}) }},\n",
      path.display().to_string()
    ));
  }
  out.push_str("];\n");
  fs::write(out_dir.join("migrations.rs"), out).unwrap();
}
