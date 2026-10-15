//! `lang.toml` project manifests (a small TOML subset: sections and
//! `key = "string"` / `key = number` entries).
//!
//! `[config]` holds base configuration data and `[profile.NAME]` tables
//! overlay it; `lang run --profile NAME` selects one, and
//! `lang inspect config` prints the effective result. `[dependencies]`
//! lists path dependencies as inline tables:
//! `textutil = { path = "../textutil", version = "0.1" }`.
//!
//! `[runtime]` holds runtime settings (workers, cleanup grace, HTTP
//! limits, trace sink policy) and `[profile.NAME.runtime]` overrides them.
//! `[entries]` names additional entry modules (`lang run NAME`), and
//! `[tasks]` names user tasks: typed functions the tool invokes
//! (`lang task NAME`), written `name = "src/file.lang#function"`.
//! Unknown sections and keys are errors.

use std::path::Path;

#[derive(Debug, Clone)]
pub struct Manifest {
    pub name: String,
    pub entry: String,
    pub workers: usize,
    /// `[runtime] cleanup_grace_ms`: how long cancelled tasks may take to
    /// finish cleanup before the waiting owner escalates.
    pub cleanup_grace_ms: Option<u64>,
    /// Base `[config]` entries (raw values, strings unquoted).
    pub config: Vec<(String, String)>,
    /// `[profile.NAME]` overlays, in file order.
    pub profiles: Vec<(String, Vec<(String, String)>)>,
    /// `[package] version` (defaults to `0.0.0`).
    pub version: String,
    /// `[dependencies]`: name, path, required version prefix.
    pub dependencies: Vec<Dependency>,
    /// `[runtime]` settings (validated keys, raw values).
    pub runtime: Vec<(String, String)>,
    /// `[profile.NAME.runtime]` overrides.
    pub profile_runtime: Vec<(String, Vec<(String, String)>)>,
    /// `[entries]`: named entry modules.
    pub entries: Vec<(String, String)>,
    /// `[tasks]`: name, module file, function.
    pub tasks: Vec<(String, String, String)>,
}

/// Runtime settings accepted in `[runtime]` / `[profile.NAME.runtime]`.
pub const RUNTIME_KEYS: &[&str] = &[
    "workers",
    "cleanup_grace_ms",
    "http_max_body_bytes",
    "http_max_header_bytes",
    "http_max_in_flight",
    "trace_capacity",
    "trace_overflow",
    "task_stack_bytes",
    "blocking_threads",
];

/// Defaults of the runtime settings (what applies when nothing sets them).
pub const RUNTIME_DEFAULTS: &[(&str, &str)] = &[
    ("workers", "1"),
    ("cleanup_grace_ms", "5000"),
    ("http_max_body_bytes", "16777216"),
    ("http_max_header_bytes", "65536"),
    ("http_max_in_flight", "1024"),
    ("trace_capacity", "1024"),
    ("trace_overflow", "drop_oldest"),
    ("task_stack_bytes", "8388608"),
    ("blocking_threads", "16"),
];

/// Validate a runtime setting (line 0: from the environment).
pub fn check_runtime(k: &str, v: &str, line: usize) -> Result<(), String> {
    if !RUNTIME_KEYS.contains(&k) {
        return Err(format!(
            "line {line}: unknown runtime setting `{k}` (known: {})",
            RUNTIME_KEYS.join(", ")
        ));
    }
    match k {
        "trace_overflow" if !matches!(v, "drop_newest" | "drop_oldest") => {
            Err(format!("line {line}: trace_overflow must be drop_newest or drop_oldest"))
        }
        "trace_overflow" => Ok(()),
        _ => {
            let Ok(n) = v.parse::<u64>() else {
                return Err(format!("line {line}: runtime setting `{k}` must be a non-negative integer"));
            };
            // the smallest value each setting can work with
            let min = match k {
                "workers" | "blocking_threads" | "http_max_in_flight" | "trace_capacity" => 1,
                "task_stack_bytes" => 256 << 10,
                "http_max_header_bytes" => 1024,
                _ => 0,
            };
            if n < min {
                return Err(format!("line {line}: runtime setting `{k}` must be at least {min}"));
            }
            let max = match k {
                "workers" => 64,
                "task_stack_bytes" => 1 << 30,
                "blocking_threads" => 4096,
                _ => u64::MAX,
            };
            if n > max {
                return Err(format!("line {line}: runtime setting `{k}` must be at most {max}"));
            }
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub struct Dependency {
    pub name: String,
    pub path: String,
    pub version: Option<String>,
}

/// The toolchain language accepted by this implementation.
pub const LANGUAGE: &str = "draft-0";

/// Where an effective configuration value came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Base,
    Profile(String),
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Manifest, String> {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Manifest, String> {
        let mut section = String::new();
        let mut m = Manifest {
            name: String::new(),
            entry: "src/main.lang".into(),
            workers: 1,
            cleanup_grace_ms: None,
            config: vec![],
            profiles: vec![],
            version: "0.0.0".into(),
            dependencies: vec![],
            runtime: vec![],
            profile_runtime: vec![],
            entries: vec![],
            tasks: vec![],
        };
        // TOML forbids defining a table or a key twice
        let mut sections_seen: Vec<String> = vec![];
        let mut keys_seen: Vec<(String, String)> = vec![];
        for (i, raw) in text.lines().enumerate() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            if let Some(s) = line.strip_prefix('[') {
                section = s.trim_end_matches(']').trim().to_string();
                if sections_seen.contains(&section) {
                    return Err(format!("line {}: section `[{section}]` is defined twice", i + 1));
                }
                sections_seen.push(section.clone());
                let known = matches!(
                    section.as_str(),
                    "package" | "toolchain" | "runtime" | "dependencies" | "entries" | "tasks" | "config"
                ) || section.starts_with("config.")
                    || section.starts_with("profile.");
                if !known {
                    return Err(format!("line {}: unknown section `[{section}]`", i + 1));
                }
                // `[profile.NAME]` and `[profile.NAME.table]`
                if let Some(rest) = section.strip_prefix("profile.") {
                    let p = rest.split('.').next().unwrap_or(rest);
                    if !m.profiles.iter().any(|(n, _)| n == p) {
                        m.profiles.push((p.to_string(), vec![]));
                    }
                }
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                return Err(format!("line {}: expected `key = value`", i + 1));
            };
            let (k, v) = (k.trim(), v.trim());
            if keys_seen.iter().any(|(sec, key)| *sec == section && key == k) {
                let place = if section.is_empty() { String::new() } else { format!(" in `[{section}]`") };
                return Err(format!("line {}: key `{k}` is defined twice{place}", i + 1));
            }
            keys_seen.push((section.clone(), k.to_string()));
            let s = v.trim_matches('"').to_string();
            match (section.as_str(), k) {
                ("package", "name") => m.name = s,
                ("package", "entry") => m.entry = s,
                ("package", "version") => m.version = s,
                ("package", "edition") if s != "0" => {
                    return Err(format!("line {}: this toolchain implements edition `0`, not `{s}`", i + 1));
                }
                ("package", "edition") => {}
                ("package", other) => {
                    return Err(format!(
                        "line {}: unknown package key `{other}` (known: name, entry, version, edition)",
                        i + 1
                    ));
                }
                ("toolchain", "language") if s != LANGUAGE => {
                    return Err(format!(
                        "line {}: this toolchain implements language `{LANGUAGE}`, not `{s}`",
                        i + 1
                    ));
                }
                ("toolchain", "language") => {}
                ("toolchain", other) => {
                    return Err(format!("line {}: unknown toolchain key `{other}` (known: language)", i + 1));
                }
                ("entries", name) => m.entries.push((name.to_string(), s)),
                ("tasks", name) => {
                    let Some((file, func)) = s.split_once('#') else {
                        return Err(format!(
                            "line {}: task `{name}` must be written \"path/to/file.lang#function\"",
                            i + 1
                        ));
                    };
                    m.tasks.push((name.to_string(), file.to_string(), func.to_string()));
                }
                ("dependencies", _) => {
                    let table =
                        v.trim().strip_prefix('{').and_then(|t| t.strip_suffix('}')).ok_or_else(|| {
                            format!(
                                "line {}: a dependency is an inline table such as `{{ path = \"../lib\" }}`",
                                i + 1
                            )
                        })?;
                    let mut dep = Dependency { name: k.to_string(), path: String::new(), version: None };
                    for part in table.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                        let Some((dk, dv)) = part.split_once('=') else {
                            return Err(format!(
                                "line {}: expected `key = value` in dependency `{k}`",
                                i + 1
                            ));
                        };
                        let dv = dv.trim().trim_matches('"').to_string();
                        match dk.trim() {
                            "path" => dep.path = dv,
                            "version" => dep.version = Some(dv),
                            other => {
                                return Err(format!(
                                    "line {}: unknown dependency key `{other}` (only path dependencies are supported)",
                                    i + 1
                                ));
                            }
                        }
                    }
                    if dep.path.is_empty() {
                        return Err(format!("line {}: dependency `{k}` needs a `path`", i + 1));
                    }
                    m.dependencies.push(dep);
                }
                ("runtime", key) => {
                    check_runtime(key, &s, i + 1)?;
                    match key {
                        "workers" => m.workers = s.parse().unwrap(),
                        "cleanup_grace_ms" => m.cleanup_grace_ms = Some(s.parse().unwrap()),
                        _ => {}
                    }
                    set(&mut m.runtime, key, s);
                }
                // `[config]`, `[config.table]`, inline tables: dotted keys
                (sec, _) if sec == "config" || sec.starts_with("config.") => {
                    let pre = sec.strip_prefix("config").unwrap().trim_start_matches('.');
                    for (key, value) in flatten(pre, k, v) {
                        set(&mut m.config, &key, value);
                    }
                }
                (sec, key)
                    if sec.starts_with("profile.")
                        && sec.ends_with(".runtime")
                        && sec.matches('.').count() == 2 =>
                {
                    let name = sec["profile.".len()..sec.len() - ".runtime".len()].to_string();
                    check_runtime(key, &s, i + 1)?;
                    match m.profile_runtime.iter_mut().find(|(n, _)| *n == name) {
                        Some(p) => set(&mut p.1, key, s),
                        None => m.profile_runtime.push((name, vec![(key.to_string(), s)])),
                    }
                }
                (sec, _) if sec.starts_with("profile.") => {
                    let rest = &sec["profile.".len()..];
                    let (name, pre) = match rest.split_once('.') {
                        Some((n, p)) => (n, p),
                        None => (rest, ""),
                    };
                    let p = m.profiles.iter_mut().find(|(n, _)| n == name).unwrap();
                    for (key, value) in flatten(pre, k, v) {
                        set(&mut p.1, &key, value);
                    }
                }
                _ => {}
            }
        }
        if m.name.is_empty() {
            return Err("missing [package] name".into());
        }
        Ok(m)
    }

    /// `[runtime]` overlaid with the selected profile's runtime table.
    pub fn effective_runtime(&self, profile: Option<&str>) -> Vec<(String, String, Source)> {
        let mut out: Vec<(String, String, Source)> =
            self.runtime.iter().map(|(k, v)| (k.clone(), v.clone(), Source::Base)).collect();
        if let Some(name) = profile
            && let Some((_, entries)) = self.profile_runtime.iter().find(|(n, _)| n == name)
        {
            for (k, v) in entries {
                match out.iter_mut().find(|e| e.0 == *k) {
                    Some(e) => {
                        e.1 = v.clone();
                        e.2 = Source::Profile(name.to_string());
                    }
                    None => out.push((k.clone(), v.clone(), Source::Profile(name.to_string()))),
                }
            }
        }
        out
    }

    /// Merge `[config]` with the selected profile.
    pub fn effective(&self, profile: Option<&str>) -> Result<Vec<(String, String, Source)>, String> {
        let mut out: Vec<(String, String, Source)> =
            self.config.iter().map(|(k, v)| (k.clone(), v.clone(), Source::Base)).collect();
        if let Some(name) = profile {
            let Some((_, entries)) = self.profiles.iter().find(|(n, _)| n == name) else {
                let known: Vec<&str> = self.profiles.iter().map(|(n, _)| n.as_str()).collect();
                return Err(if known.is_empty() {
                    format!("unknown profile `{name}`: the manifest declares no `[profile.NAME]` tables")
                } else {
                    format!("unknown profile `{name}` (declared: {})", known.join(", "))
                });
            };
            for (k, v) in entries {
                match out.iter_mut().find(|e| e.0 == *k) {
                    Some(e) => {
                        e.1 = v.clone();
                        e.2 = Source::Profile(name.to_string());
                    }
                    None => out.push((k.clone(), v.clone(), Source::Profile(name.to_string()))),
                }
            }
        }
        Ok(out)
    }
}

/// A configuration entry as dotted keys: `db = { path = "x" }` in table
/// `pre` gives `pre.db.path = x`.
fn flatten(pre: &str, k: &str, v: &str) -> Vec<(String, String)> {
    let key = if pre.is_empty() { k.to_string() } else { format!("{pre}.{k}") };
    let v = v.trim();
    if let Some(inner) = v.strip_prefix('{').and_then(|x| x.strip_suffix('}')) {
        let mut out = vec![];
        for part in inner.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            if let Some((ik, iv)) = part.split_once('=') {
                out.extend(flatten(&key, ik.trim(), iv.trim()));
            }
        }
        return out;
    }
    vec![(key, v.trim_matches('"').to_string())]
}

/// The line without a trailing `#` comment (`#` inside a string is kept).
fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

fn set(entries: &mut Vec<(String, String)>, k: &str, v: String) {
    match entries.iter_mut().find(|e| e.0 == k) {
        Some(e) => e.1 = v,
        None => entries.push((k.to_string(), v)),
    }
}

#[cfg(test)]
mod tests {
    use super::Manifest;

    #[test]
    fn duplicate_keys_and_sections_are_errors() {
        let dup_key = Manifest::parse("[package]\nname = \"a\"\nname = \"b\"\n").unwrap_err();
        assert!(dup_key.contains("key `name` is defined twice"), "{dup_key}");
        let dup_section =
            Manifest::parse("[package]\nname = \"a\"\n[runtime]\nworkers = 1\n[runtime]\nworkers = 2\n")
                .unwrap_err();
        assert!(dup_section.contains("section `[runtime]` is defined twice"), "{dup_section}");
        // the same key in different sections is fine
        assert!(Manifest::parse("[package]\nname = \"a\"\n[entries]\nname = \"src/x.lang\"\n").is_ok());
    }
}
