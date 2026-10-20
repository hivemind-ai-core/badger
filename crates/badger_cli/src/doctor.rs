//! `badger doctor`: a report of the environment, for bug reports.
//!
//! Only what can be determined is reported: a fact that is not known is
//! left out, never guessed.

use std::path::PathBuf;

use badger_driver::Session;

/// The project the current directory belongs to (when it is one).
struct Project {
    /// The project's directory.
    root: PathBuf,
    /// The package's name (when the manifest has one).
    name: Option<String>,
    /// The manifest's path.
    config: PathBuf,
    /// The number of `.bdg` files in the project.
    source_files: usize,
    /// Why the manifest could not be read (when it could not).
    error: Option<String>,
}

/// The report of the environment.
pub struct Report {
    /// The version of the tool.
    version: String,
    /// The current working directory.
    cwd: PathBuf,
    /// The project of the current directory, when it is one.
    project: Option<Project>,
    /// The effective runtime settings (the worker count and the HTTP
    /// limits).
    runtime: Vec<(String, String)>,
    /// Why the runtime settings could not be computed (when they could
    /// not).
    runtime_error: Option<String>,
    /// The `BADGER_*` environment variables that are set.
    env: Vec<(String, String)>,
}

/// The runtime settings the report holds: the worker count and the HTTP
/// limits (the rest is not needed to reproduce a failure).
fn reported_settings(all: &[(String, String, String)]) -> Vec<(String, String)> {
    all.iter()
        .filter(|(k, _, _)| k == "workers" || k.starts_with("http_"))
        .map(|(k, v, _)| (k.clone(), v.clone()))
        .collect()
}

/// The environment the tool runs in.
pub fn report() -> Report {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    // the project of the current directory: a manifest beside it
    let config = cwd.join("badger.toml");
    let is_project = config.is_file();
    let project = if is_project {
        let manifest = badger_driver::manifest::Manifest::in_dir(&cwd);
        let name = manifest.as_ref().ok().map(|m| m.name.clone());
        let error = manifest.err();
        let mut files = vec![];
        crate::source_files(&cwd, &mut files);
        Some(Project { root: cwd.clone(), name, config: config.clone(), source_files: files.len(), error })
    } else {
        None
    };
    // the effective runtime settings: the same ones `badger inspect
    // runtime` prints. The driver reads `[runtime]` from the manifest of
    // a directory that has one; a directory that is not a project runs
    // with the built-in defaults (and the environment's overrides), so
    // it is asked with a path that is not a directory.
    let settings_path = if is_project { cwd.clone() } else { cwd.join("badger.toml") };
    let (runtime, runtime_error) = match Session::runtime_settings_with_sources(&settings_path, None) {
        Ok(all) => (reported_settings(&all), None),
        Err(e) => (vec![], Some(e)),
    };
    let mut env: Vec<(String, String)> = std::env::vars().filter(|(k, _)| k.starts_with("BADGER_")).collect();
    env.sort();
    Report { version: env!("CARGO_PKG_VERSION").to_string(), cwd, project, runtime, runtime_error, env }
}

impl Report {
    /// The report as plain text: a `key: value` per line, under small
    /// headings.
    pub fn text(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("badger: {}\n", self.version));
        s.push_str(&format!("cwd: {}\n", self.cwd.display()));
        s.push('\n');
        match &self.project {
            None => {
                s.push_str("project: none (no badger.toml here)\n");
            }
            Some(p) => {
                s.push_str("project:\n");
                s.push_str(&format!("  root: {}\n", p.root.display()));
                if let Some(n) = &p.name {
                    s.push_str(&format!("  name: {n}\n"));
                }
                s.push_str(&format!("  config: {}\n", p.config.display()));
                s.push_str(&format!("  source files: {}\n", p.source_files));
                if let Some(e) = &p.error {
                    s.push_str(&format!("  manifest error: {e}\n"));
                }
            }
        }
        s.push('\n');
        s.push_str("runtime (as `badger inspect runtime` computes them):\n");
        match &self.runtime_error {
            Some(e) => s.push_str(&format!("  error: {e}\n")),
            None => {
                for (k, v) in &self.runtime {
                    s.push_str(&format!("  {k}: {v}\n"));
                }
            }
        }
        s.push('\n');
        s.push_str("environment:\n");
        if self.env.is_empty() {
            s.push_str("  (no BADGER_* variables)\n");
        }
        for (k, v) in &self.env {
            s.push_str(&format!("  {k}: {v}\n"));
        }
        s
    }

    /// The report as a JSON object.
    pub fn json(&self) -> serde_json::Value {
        let project = match &self.project {
            None => serde_json::Value::Null,
            Some(p) => serde_json::json!({
                "root": p.root.to_string_lossy(),
                "name": p.name,
                "config": p.config.to_string_lossy(),
                "source_files": p.source_files,
                "error": p.error,
            }),
        };
        let runtime: serde_json::Map<String, serde_json::Value> =
            self.runtime.iter().map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone()))).collect();
        let env: serde_json::Map<String, serde_json::Value> =
            self.env.iter().map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone()))).collect();
        serde_json::json!({
            "version": self.version,
            "cwd": self.cwd.to_string_lossy(),
            "project": project,
            "runtime": runtime,
            "runtime_error": self.runtime_error,
            "env": env,
        })
    }
}
