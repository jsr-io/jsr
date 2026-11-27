// Copyright 2024 the JSR authors. All rights reserved. MIT license.

fn main() {
  // regex-automata overflows worker-build's 1MB stack. Set here rather than
  // in RUSTFLAGS, which worker-build appends its own `-sSTACK_SIZE` to: the
  // last one on the link wins, and a build script's arguments come last.
  if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten") {
    println!("cargo::rustc-link-arg=-sSTACK_SIZE=8MB");
  }
}
