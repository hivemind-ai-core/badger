//! `badger.toml` project manifests (a small TOML subset: sections and
//! `key = "string"` / `key = number` entries).
//!
//! `[config]` holds base configuration data and `[profile.NAME]` tables
//! overlay it; `badger run --profile NAME` selects one, and
//! `badger inspect config` prints the effective result. `[dependencies]`
//! lists dependencies as inline tables, by path or by git repository:
//! `textutil = { path = "../textutil", version = "0.1" }`,
//! `json = { git = "https://example.com/json.git", rev = "v1.2", subdir = "lib" }`.
//! `[workspace] members = ["app", "libs/*"]` makes a directory a workspace
//! of packages sharing one `badger.lock`; its `badger.toml` may also describe a
//! package of its own. `[package] kind = "library"` marks a package that
//! is used by others and has no `main`.
//!
//! `[runtime]` holds runtime settings (workers, cleanup grace, HTTP
//! limits, trace sink policy) and `[profile.NAME.runtime]` overrides them.
//! `[entries]` names additional entry modules (`badger run NAME`), and
//! `[tasks]` names user tasks: typed functions the tool invokes
//! (`badger task NAME`), written `name = "src/file.bdg#function"`.
//! Unknown sections and keys are errors.

use std::path::Path;

#[derive(Debug, Clone)]
pub struct Manifest {
    pub name: String,
    pub entry: String,
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
    /// `[package] restricted`: only the entry module may use ambient
    /// authority (see `authority`).
    pub restricted: bool,
    /// `[database] schema`: SQL file the program's literal statements are
    /// checked against (relative to the project root).
    pub schema: Option<String>,
    /// `[package] kind = "library"`: used by other packages, not run.
    pub library: bool,
    /// Whether the manifest has a `[package]` (a workspace root may not).
    pub has_package: bool,
    /// `[workspace] members`: member directories (a final `*` names every
    /// package directory below that path).
    pub workspace: Option<Vec<String>>,
    /// `[env] dotenv`: files of `NAME=value` lines loaded into the
    /// environment before the program starts (variables already set win).
    pub dotenv: Vec<String>,
}

/// Runtime settings accepted in `[runtime]` / `[profile.NAME.runtime]`.
pub const RUNTIME_KEYS: &[&str] = &[
    "workers",
    "cleanup_grace_ms",
    "http_max_body_bytes",
    "http_max_header_bytes",
    "http_max_in_flight",
    "http_max_stream_buffer_bytes",
    "trace_capacity",
    "trace_overflow",
    "log_level",
    "log_format",
    "task_stack_bytes",
    "blocking_threads",
    "heap_max_bytes",
];

/// Defaults of the runtime settings (what applies when nothing sets them).
pub const RUNTIME_DEFAULTS: &[(&str, &str)] = &[
    ("workers", "1"),
    ("cleanup_grace_ms", "5000"),
    ("http_max_body_bytes", "16777216"),
    ("http_max_header_bytes", "65536"),
    ("http_max_in_flight", "1024"),
    ("http_max_stream_buffer_bytes", "1048576"),
    ("trace_capacity", "1024"),
    ("trace_overflow", "drop_oldest"),
    ("log_level", "info"),
    ("log_format", "text"),
    ("task_stack_bytes", "8388608"),
    ("blocking_threads", "16"),
    ("heap_max_bytes", "0"),
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
        "log_level" if !matches!(v, "trace" | "debug" | "info" | "warn" | "error" | "off") => {
            Err(format!("line {line}: log_level must be trace, debug, info, warn, error, or off"))
        }
        "log_format" if !matches!(v, "text" | "json" | "none") => {
            Err(format!("line {line}: log_format must be text, json, or none"))
        }
        "log_level" | "log_format" => Ok(()),
        _ => {
            let Ok(n) = v.parse::<u64>() else {
                return Err(format!("line {line}: runtime setting `{k}` must be a non-negative integer"));
            };
            // the smallest value each setting can work with
            let min = match k {
                "workers" | "blocking_threads" | "http_max_in_flight" | "trace_capacity" => 1,
                "http_max_stream_buffer_bytes" => 1024,
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
    /// A package directory, relative to the depending package.
    pub path: String,
    pub version: Option<String>,
    /// A git repository (URL or local path) instead of `path`, at `rev` (a
    /// tag, branch, or commit; the default branch otherwise), with the
    /// package in `subdir` of it.
    pub git: Option<String>,
    pub rev: Option<String>,
    pub subdir: Option<String>,
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
    /// The manifest of the project directory `dir` (its `badger.toml`).
    pub fn in_dir(dir: &Path) -> Result<Manifest, String> {
        Self::load(&dir.join("badger.toml"))
    }

    pub fn load(path: &Path) -> Result<Manifest, String> {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Manifest, String> {
        let m = Self::parse_entries(text)?;
        // exact TOML as well: syntax, duplicate keys, and conflicting tables
        // (a key and a table of one name, inline tables extended later)
        let table = match text.parse::<toml::Table>() {
            Ok(t) => t,
            Err(e) => {
                let line =
                    e.span().map(|r| text[..r.start.min(text.len())].matches('\n').count() + 1).unwrap_or(1);
                let msg = e.message().trim_end_matches('\n').to_string();
                return Err(format!("line {line}: {msg}"));
            }
        };
        let mut m = m;
        if let Some(ws) = table.get("workspace") {
            let Some(ws) = ws.as_table() else { return Err("`workspace` must be a table".into()) };
            for k in ws.keys() {
                if k != "members" {
                    return Err(format!("unknown workspace key `{k}` (known: members)"));
                }
            }
            let members = ws
                .get("members")
                .and_then(|v| v.as_array())
                .ok_or("`[workspace] members` must be a list of directories")?
                .iter()
                .map(|v| {
                    v.as_str().map(str::to_string).ok_or("`[workspace] members` must be a list of strings")
                })
                .collect::<Result<Vec<_>, _>>()?;
            m.workspace = Some(members);
        }
        if let Some(env) = table.get("env") {
            let Some(env) = env.as_table() else { return Err("`env` must be a table".into()) };
            for (k, v) in env {
                match k.as_str() {
                    "dotenv" => {
                        let files = match v {
                            toml::Value::String(s) => vec![s.clone()],
                            toml::Value::Array(xs) => xs
                                .iter()
                                .map(|x| {
                                    x.as_str()
                                        .map(str::to_string)
                                        .ok_or("`[env] dotenv` must list file names")
                                })
                                .collect::<Result<Vec<_>, _>>()?,
                            _ => return Err("`[env] dotenv` is a file name or a list of them".into()),
                        };
                        m.dotenv = files;
                    }
                    other => return Err(format!("unknown env key `{other}` (known: dotenv)")),
                }
            }
        }
        Ok(m)
    }

    /// The manifest's entries, with this format's own diagnostics.
    fn parse_entries(text: &str) -> Result<Manifest, String> {
        let mut section = String::new();
        let mut m = Manifest {
            name: String::new(),
            entry: "src/main.bdg".into(),
            config: vec![],
            profiles: vec![],
            version: "0.0.0".into(),
            dependencies: vec![],
            runtime: vec![],
            profile_runtime: vec![],
            entries: vec![],
            tasks: vec![],
            restricted: false,
            schema: None,
            library: false,
            has_package: false,
            workspace: None,
            dotenv: vec![],
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
                    "package"
                        | "toolchain"
                        | "runtime"
                        | "dependencies"
                        | "entries"
                        | "tasks"
                        | "config"
                        | "database"
                        | "workspace"
                        | "env"
                ) || section.starts_with("config.")
                    || section.starts_with("profile.");
                if !known {
                    return Err(format!("line {}: unknown section `[{section}]`", i + 1));
                }
                if section == "package" {
                    m.has_package = true;
                }
                // `[profile.NAME]` and `[profile.NAME.table]`
                if let Some(rest) = section.strip_prefix("profile.") {
                    let p = rest.split('.').next().unwrap_or(rest);
                    // a profile overlays `[config]` itself: `[profile.dev.config]`
                    // would be a configuration table named `config`
                    if rest.split('.').nth(1) == Some("config") {
                        return Err(format!(
                            "line {}: `[{section}]`: a profile overlays `[config]` directly; put these keys in `[profile.{p}]`",
                            i + 1
                        ));
                    }
                    if !m.profiles.iter().any(|(n, _)| n == p) {
                        m.profiles.push((p.to_string(), vec![]));
                    }
                }
                continue;
            }
            // `[workspace]` and `[env]` are read from the exact TOML (arrays
            // may span lines)
            if section == "workspace" || section == "env" {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                return Err(format!("line {}: expected `key = value`", i + 1));
            };
            // a quoted key names the same key (`"workers" = 1`, `'workers' = 1`)
            let raw_key = k.trim();
            let quoted = raw_key.len() >= 2
                && ((raw_key.starts_with('"') && raw_key.ends_with('"'))
                    || (raw_key.starts_with('\'') && raw_key.ends_with('\'')));
            let (k, v) = (if quoted { &raw_key[1..raw_key.len() - 1] } else { raw_key }, v.trim());
            // keys are paths of names (configuration keys become record
            // fields): a quoted key containing `.` has no such meaning
            if quoted && k.contains('.') {
                return Err(format!(
                    "line {}: quoted key `{raw_key}` contains `.`; write a dotted key or a table instead",
                    i + 1
                ));
            }
            // `a.b = 1` in `[config]` and `b = 2` in `[config.a]` are one key
            let full = if section.is_empty() { k.to_string() } else { format!("{section}.{k}") };
            if keys_seen.iter().any(|(_, key)| *key == full) {
                let place = if section.is_empty() { String::new() } else { format!(" in `[{section}]`") };
                return Err(format!("line {}: key `{k}` is defined twice{place}", i + 1));
            }
            keys_seen.push((section.clone(), full));
            // the value as text: a string's contents, anything else as written
            let s = match toml_value(v) {
                Some(toml::Value::String(x)) => x,
                _ => v.trim_matches('"').to_string(),
            };
            match (section.as_str(), k) {
                ("package", "name") => m.name = s,
                ("package", "entry") => m.entry = s,
                ("package", "version") => m.version = s,
                ("package", "edition") if s != "0" => {
                    return Err(format!("line {}: this toolchain implements edition `0`, not `{s}`", i + 1));
                }
                ("package", "edition") => {}
                ("package", "restricted") => {
                    m.restricted = match v {
                        "true" => true,
                        "false" => false,
                        _ => return Err(format!("line {}: `restricted` is `true` or `false`", i + 1)),
                    }
                }
                ("package", "kind") => {
                    m.library = match s.as_str() {
                        "library" => true,
                        "application" => false,
                        _ => {
                            return Err(format!("line {}: `kind` is \"application\" or \"library\"", i + 1));
                        }
                    }
                }
                ("package", other) => {
                    return Err(format!(
                        "line {}: unknown package key `{other}` (known: name, entry, version, edition, kind, restricted)",
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
                ("database", "schema") => m.schema = Some(s),
                ("database", other) => {
                    return Err(format!("line {}: unknown database key `{other}` (known: schema)", i + 1));
                }
                ("entries", name) => m.entries.push((name.to_string(), s)),
                ("tasks", name) => {
                    let Some((file, func)) = s.split_once('#') else {
                        return Err(format!(
                            "line {}: task `{name}` must be written \"path/to/file.bdg#function\"",
                            i + 1
                        ));
                    };
                    m.tasks.push((name.to_string(), file.to_string(), func.to_string()));
                }
                ("dependencies", _) => {
                    let Some(toml::Value::Table(table)) = toml_value(v) else {
                        return Err(format!(
                            "line {}: a dependency is an inline table such as `{{ path = \"../lib\" }}`",
                            i + 1
                        ));
                    };
                    let mut dep = Dependency {
                        name: k.to_string(),
                        path: String::new(),
                        version: None,
                        git: None,
                        rev: None,
                        subdir: None,
                    };
                    for (dk, dv) in &table {
                        let Some(dv) = dv.as_str().map(str::to_string) else {
                            return Err(format!("line {}: dependency `{k}`: `{dk}` is a string", i + 1));
                        };
                        match dk.as_str() {
                            "path" => dep.path = dv,
                            "version" => dep.version = Some(dv),
                            "git" => dep.git = Some(dv),
                            "rev" => dep.rev = Some(dv),
                            "subdir" => dep.subdir = Some(dv),
                            other => {
                                return Err(format!(
                                    "line {}: unknown dependency key `{other}` (known: path, git, rev, subdir, version)",
                                    i + 1
                                ));
                            }
                        }
                    }
                    match (dep.path.is_empty(), &dep.git) {
                        (true, None) => {
                            return Err(format!(
                                "line {}: dependency `{k}` needs a `path` or a `git` repository",
                                i + 1
                            ));
                        }
                        (false, Some(_)) => {
                            return Err(format!(
                                "line {}: dependency `{k}` has both a `path` and a `git` repository",
                                i + 1
                            ));
                        }
                        (false, None) if dep.rev.is_some() || dep.subdir.is_some() => {
                            return Err(format!(
                                "line {}: `rev` and `subdir` go with a `git` dependency",
                                i + 1
                            ));
                        }
                        _ => {}
                    }
                    if dep.git.as_deref().is_some_and(|g| g.starts_with('-') || g.is_empty()) {
                        return Err(format!(
                            "line {}: dependency `{k}` has an invalid git repository",
                            i + 1
                        ));
                    }
                    if dep.rev.as_deref().is_some_and(|r| r.starts_with('-') || r.is_empty()) {
                        return Err(format!("line {}: dependency `{k}` has an invalid `rev`", i + 1));
                    }
                    m.dependencies.push(dep);
                }
                ("runtime", key) => {
                    check_runtime(key, &s, i + 1)?;
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
        if m.name.is_empty() && !sections_seen.iter().any(|s| s == "workspace") {
            return Err("missing [package] name".into());
        }
        // the name names files (saved programs, releases): no paths
        if !m.name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
            return Err(format!("package name `{}` may contain only letters, digits, `_`, and `-`", m.name));
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

    /// `[config]` followed by the selected profile's entries, unmerged
    /// (later entries take precedence), for per-setting provenance.
    pub fn layers(&self, profile: Option<&str>) -> Result<Vec<(String, String, Source)>, String> {
        self.effective(profile)?;
        let mut out: Vec<(String, String, Source)> =
            self.config.iter().map(|(k, v)| (k.clone(), v.clone(), Source::Base)).collect();
        if let Some(name) = profile
            && let Some((_, entries)) = self.profiles.iter().find(|(n, _)| n == name)
        {
            out.extend(
                entries.iter().map(|(k, v)| (k.clone(), v.clone(), Source::Profile(name.to_string()))),
            );
        }
        Ok(out)
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
/// The line without its comment (a `#` inside a `"…"` or `'…'` string is
/// text).
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (Some('"'), '\\') if !escaped => {
                escaped = true;
                continue;
            }
            (Some(q), c) if c == q && !escaped => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, '#') => return &line[..i],
            _ => {}
        }
        escaped = false;
    }
    line
}

/// A value written in TOML (`"a"`, `'a'`, `1`, `{ path = "x" }`), read by
/// the TOML parser so quoting and escapes mean what they mean in TOML.
fn toml_value(v: &str) -> Option<toml::Value> {
    format!("x = {v}").parse::<toml::Table>().ok()?.remove("x")
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
        assert!(Manifest::parse("[package]\nname = \"a\"\n[entries]\nname = \"src/x.bdg\"\n").is_ok());
        // a dotted key and a table entry name the same key; quotes do not matter
        let dotted =
            Manifest::parse("[package]\nname = \"a\"\n[config]\na.b = 1\n[config.a]\nb = 2\n").unwrap_err();
        assert!(dotted.contains("key `b` is defined twice"), "{dotted}");
        assert!(Manifest::parse("[package]\nname = \"a\"\n[runtime]\n\"workers\" = 2\n").is_ok());
    }

    /// `[profile.NAME.config]` reads like a profile's `[config]` but would be
    /// a configuration table named `config`: refused with the fix.
    #[test]
    fn profile_config_table_is_an_error() {
        let e = Manifest::parse("[package]\nname = \"a\"\n[profile.dev.config]\nport = 2\n").unwrap_err();
        assert!(e.contains("put these keys in `[profile.dev]`"), "{e}");
        let ok = Manifest::parse(
            "[package]\nname = \"a\"\n[profile.dev]\nport = 2\n[profile.dev.db]\nurl = \"x\"\n",
        );
        assert!(ok.is_ok());
    }

    #[test]
    fn toml_semantics() {
        let p = |body: &str| Manifest::parse(&format!("[package]\nname = \"a\"\n{body}"));
        // duplicates inside inline tables, keys extended as tables, and
        // inline tables reopened as sections are TOML errors
        assert!(p("[config]\nx = { a = 1, a = 2 }\n").unwrap_err().contains("line 4"));
        assert!(p("[config]\na = 1\na.b = 2\n").is_err());
        assert!(p("[config]\na = { b = 1 }\n[config.a]\nc = 2\n").is_err());
        // single-quoted keys name the same key
        assert!(
            p("[runtime]\n'workers' = 3\n").unwrap().runtime.iter().any(|(k, v)| k == "workers" && v == "3")
        );
        // a quoted key with `.` is not a path of names
        assert!(p("[config]\n\"a.b\" = 1\n").unwrap_err().contains("contains `.`"));
        assert!(p("[config\nx = 1\n").unwrap_err().contains("line 3"));
    }
}

#[cfg(test)]
mod value_tests {
    use super::Manifest;

    /// Values mean what they mean in TOML: single quotes, `#` inside a
    /// string, and commas inside a dependency's path.
    #[test]
    fn toml_values() {
        let m = Manifest::parse(
            "[package]\nname = 'app' # the app\nentry = 'src/main.bdg'\n[dependencies]\nlib = { path = \"../x,y\" }\n",
        )
        .unwrap();
        assert_eq!(m.name, "app");
        assert_eq!(m.entry, "src/main.bdg");
        assert_eq!(m.dependencies[0].path, "../x,y");
        let m = Manifest::parse("[package]\nname = \"a\"\n[entries]\ntool = \"src/a#b.bdg\" # a comment\n")
            .unwrap();
        assert_eq!(m.entries[0].1, "src/a#b.bdg");
    }
}
