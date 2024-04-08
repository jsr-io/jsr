pub mod helpers {
  use crate::tarball::ConfigFile;
  use std::path::Path;
  use std::path::PathBuf;
  use url::Url;

  pub struct Spec {
    pub jsr_json: ConfigFile,
    pub files: Vec<SpecFile>,
    pub output_file: SpecFile,
  }

  impl Spec {
    pub fn emit(&self) -> String {
      let mut text = String::new();
      for file in &self.files {
        text.push_str(&file.emit());
        text.push('\n');
      }
      text.push_str(&self.output_file.emit());
      text
    }
  }

  #[derive(Debug)]
  pub struct SpecFile {
    pub specifier: String,
    pub text: String,
    /// Text to use when emitting the spec file.
    pub emit_text: Option<String>,
  }

  impl SpecFile {
    pub fn emit(&self) -> String {
      let mut text = format!("# {}\n", self.specifier);
      text.push_str(self.emit_text.as_ref().unwrap_or(&self.text));
      text
    }

    pub fn url(&self) -> Url {
      if !self.specifier.starts_with("file")
        && !self.specifier.starts_with("http")
        && !self.specifier.starts_with("npm")
      {
        Url::parse(&format!("file:///{}", self.specifier)).unwrap()
      } else {
        Url::parse(&self.specifier).unwrap()
      }
    }
  }

  pub fn get_specs_in_dir(path: &Path) -> Vec<(PathBuf, Spec)> {
    let files = collect_files_in_dir_recursive(path);
    let files: Vec<_> = if files
      .iter()
      .any(|file| file.path.to_string_lossy().to_lowercase().contains("_only"))
    {
      files
        .into_iter()
        .filter(|file| {
          file.path.to_string_lossy().to_lowercase().contains("_only")
        })
        .collect()
    } else {
      files
        .into_iter()
        .filter(|file| {
          !file.path.to_string_lossy().to_lowercase().contains("_skip")
        })
        .collect()
    };
    files
      .into_iter()
      .map(|file| {
        let spec = parse_spec(file.text);
        (file.path, spec)
      })
      .collect()
  }

  fn parse_spec(text: String) -> Spec {
    let mut files = Vec::new();
    let mut current_file = None;
    for line in text.split('\n') {
      if let Some(specifier) = line.strip_prefix("# ") {
        if let Some(file) = current_file.take() {
          files.push(file);
        }
        current_file = Some(SpecFile {
          specifier: specifier.to_string(),
          text: String::new(),
          emit_text: None,
        });
      } else {
        let current_file = current_file.as_mut().unwrap();
        if !current_file.text.is_empty() {
          current_file.text.push('\n');
        }
        current_file.text.push_str(line);
      }
    }
    files.push(current_file.unwrap());
    let output_file = files.remove(
      files
        .iter()
        .position(|f| f.specifier == "output")
        .expect("missing output in spec file"),
    );
    let mut jsr_json = None;
    for file in &files {
      if file.specifier == "jsr.json" {
        jsr_json = Some(serde_json::from_str(&file.text).unwrap());
      }
    }
    Spec {
      jsr_json: jsr_json.expect("jsr.json not found in spec file"),
      files,
      output_file,
    }
  }

  struct CollectedFile {
    pub path: PathBuf,
    pub text: String,
  }

  fn collect_files_in_dir_recursive(path: &Path) -> Vec<CollectedFile> {
    let mut result = Vec::new();

    for entry in path.read_dir().unwrap().flatten() {
      let entry_path = entry.path();
      if entry_path.is_file() {
        let text = std::fs::read_to_string(&entry_path).unwrap();
        result.push(CollectedFile {
          path: entry_path,
          text,
        });
      } else {
        result.extend(collect_files_in_dir_recursive(&entry_path));
      }
    }

    result
  }
}
