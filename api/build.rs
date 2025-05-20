use std::fs;
use std::path::Path;

fn main() {
  let static_dir = Path::new("../frontend/static/ddoc");
  fs::create_dir_all(static_dir).unwrap();
  
  fs::write(
    static_dir.join("style.css"),
    deno_doc::html::STYLESHEET,
  ).unwrap();
  
  fs::write(
    static_dir.join("comrak.css"),
    deno_doc::html::comrak::COMRAK_STYLESHEET,
  ).unwrap();
  
  fs::write(
    static_dir.join("script.js"),
    deno_doc::html::SCRIPT_JS,
  ).unwrap();
}