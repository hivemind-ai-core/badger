//! Compiler pipeline orchestration:
//!
//! ```text
//! source ─layout+pest─▶ AST ─resolve/desugar─▶ HIR ─infer─▶ typed HIR ─mono─▶ MIR ─▶ CLIF ─JIT─▶ run
//! ```
//!
//! A `Session` owns everything that outlives a single phase (sources,
//! interner, diagnostics). Each phase reports into `Session::diagnostics`; the
//! driver stops before code generation if any errors were reported.
#![allow(clippy::type_complexity)]

use std::path::{Path, PathBuf};

use lang_base::{Diagnostic, Diagnostics, FileId, Interner, SourceMap, Symbol};
use lang_hir::{Hir, ParsedModule};
use lang_syntax::ast;

pub mod analysis;
pub mod authority;
pub mod contracts;
pub mod deps;
pub mod inspect;
pub mod live;
pub mod manifest;
pub mod release;
pub mod sql;
pub mod sqlcheck;
pub mod sqlgen;

#[derive(Default)]
pub struct Session {
    pub sources: SourceMap,
    pub interner: Interner,
    pub diagnostics: Diagnostics,
    /// Source appended to the entry module (remote REPL definitions).
    pub entry_suffix: String,
    /// Unsaved editor buffers, by canonical path: read instead of the file.
    pub overlays: std::collections::HashMap<PathBuf, String>,
    /// Native modules the last `compile_binary` program calls.
    pub natives: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    Errors,
}

/// A loaded program: the entry module plus every reachable user module and
/// the standard library.
pub struct Loaded {
    pub modules: Vec<ParsedModule>,
    pub entry: usize,
    pub root: PathBuf,
}

/// A named entry (`[entries]`) or user task (`[tasks]`) selected for this
/// process, and the function it starts at (`main` unless a task).
static ENTRY: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
static ENTRY_FN: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Select a manifest entry (`[entries]` name, or `task:NAME`) for project
/// paths.
pub fn select_entry(name: Option<String>) {
    *ENTRY.lock().unwrap() = name;
    *ENTRY_FN.lock().unwrap() = None;
}

/// Select a user task (`[tasks]` name): its module becomes the entry and
/// its function is run instead of `main`.
pub fn select_task(name: &str) {
    *ENTRY.lock().unwrap() = Some(format!("task:{name}"));
}

fn entry_fn() -> String {
    ENTRY_FN.lock().unwrap().clone().unwrap_or_else(|| "main".into())
}

/// Load the project's `[env] dotenv` files into the environment, before
/// the program (and its runtime threads) start: `NAME=value` lines, with
/// `#` comments, an optional `export ` prefix, and quoted values (`"..."`
/// with `\n` escapes, or `'...'` taken literally). A variable already set
/// keeps its value; a listed file that does not exist is skipped.
pub fn load_dotenv(project: &Path) -> Result<(), String> {
    let Ok(m) = manifest::Manifest::load(&project.join("lang.toml")) else { return Ok(()) };
    for f in &m.dotenv {
        let p = project.join(f);
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        for (k, v) in parse_dotenv(&text).map_err(|e| format!("{}: {e}", p.display()))? {
            if std::env::var_os(&k).is_none() {
                // SAFETY: before the program starts, while this process
                // runs no other threads that read the environment
                unsafe { std::env::set_var(&k, &v) };
            }
        }
    }
    Ok(())
}

/// The `NAME=value` pairs of a dotenv file.
pub fn parse_dotenv(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = vec![];
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((k, v)) = line.split_once('=') else {
            return Err(format!("line {}: expected NAME=value", i + 1));
        };
        let k = k.trim();
        if k.is_empty() || !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!("line {}: `{k}` is not a variable name", i + 1));
        }
        let v = v.trim();
        let v = if let Some(q) = v.strip_prefix('"') {
            let Some(end) = q.rfind('"') else { return Err(format!("line {}: unclosed quote", i + 1)) };
            q[..end].replace("\\n", "\n").replace("\\\"", "\"")
        } else if let Some(q) = v.strip_prefix('\'') {
            let Some(end) = q.rfind('\'') else { return Err(format!("line {}: unclosed quote", i + 1)) };
            q[..end].to_string()
        } else {
            // an unquoted value ends at a ` #` comment
            v.split(" #").next().unwrap_or(v).trim().to_string()
        };
        out.push((k.to_string(), v));
    }
    Ok(out)
}

/// The packages a command at `path` covers: every package of the
/// workspace whose root `path` is, else `path` itself.
pub fn packages_of(path: &Path) -> Vec<PathBuf> {
    if path.is_dir()
        && let Ok(m) = manifest::Manifest::load(&path.join("lang.toml"))
        && m.workspace.is_some()
    {
        return deps::workspace_packages(path, &m);
    }
    vec![path.to_path_buf()]
}

/// Entry selections besides the main entry: `[entries]` names and
/// `task:NAME` for `[tasks]` (empty for a single file).
pub fn project_entry_points(path: &Path) -> Vec<String> {
    if !path.is_dir() {
        return vec![];
    }
    let Ok(m) = manifest::Manifest::load(&path.join("lang.toml")) else { return vec![] };
    let mut out: Vec<String> = m.entries.iter().map(|(n, _)| n.clone()).collect();
    out.extend(m.tasks.iter().map(|(n, _, _)| format!("task:{n}")));
    out
}

/// Start the sampling profiler if `LANG_PROFILE` is set.
pub fn start_profile() {
    if std::env::var("LANG_PROFILE_SCOPE").is_ok_and(|v| v == "runtime") {
        return;
    }
    lang_runtime::profile::start_from_env();
}

/// Flush trace export and report trace sink overload (at process exit).
pub fn finish() {
    lang_runtime::intrinsics_codec::report_overload();
    lang_runtime::profile::finish();
    lang_runtime::intrinsics_os::cleanup_at_exit();
}

/// The standard library's native modules, linked into the executable (a
/// library of the same name on the search path is not loaded for them).
pub fn register_native_modules() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        lang_runtime::native::register_builtin("lang_sqlite", lang_sqlite::native_entry);
        lang_runtime::native::register_builtin("lang_fetch", lang_fetch::native_entry);
        lang_runtime::native::register_builtin("lang_http", lang_http::native_entry);
        lang_runtime::native::register_builtin("lang_formats", lang_formats::native_entry);
        lang_runtime::native::register_builtin("lang_template", lang_template::native_entry);
        lang_runtime::native::register_builtin("lang_crypto", lang_crypto::native_entry);
        lang_runtime::native::register_builtin("lang_valkey", lang_valkey::native_entry);
        lang_runtime::native::register_builtin("lang_postgres", lang_postgres::native_entry);
        lang_runtime::native::register_builtin("lang_datetime", lang_datetime::native_entry);
        lang_runtime::native::register_builtin("lang_compress", lang_compress::native_entry);
        lang_runtime::native::register_builtin("lang_fs", lang_fs::native_entry);
    });
}

/// The key an overlay (unsaved or proposed file content) is stored under:
/// the canonical path, or for a file that does not exist yet, its
/// directory's canonical path and its name.
pub fn overlay_key(path: &Path) -> PathBuf {
    if let Ok(p) = std::fs::canonicalize(path) {
        return p;
    }
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    match (abs.parent().and_then(|d| std::fs::canonicalize(d).ok()), abs.file_name()) {
        (Some(d), Some(n)) => d.join(n),
        _ => abs,
    }
}

/// A new temporary directory only this user can use, with an
/// unpredictable name (never an existing directory or a symlink), removed
/// when the process exits.
pub fn private_temp_dir(prefix: &str) -> std::io::Result<PathBuf> {
    use std::io::Read;
    use std::os::unix::fs::DirBuilderExt;
    for _ in 0..16 {
        let mut b = [0u8; 8];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut b)?;
        let suffix: String = b.iter().map(|x| format!("{x:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("{prefix}-{suffix}"));
        match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => {
                lang_runtime::intrinsics_os::remove_at_exit(dir.clone());
                return Ok(dir);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("cannot create a private temporary directory"))
}

impl Session {
    pub fn new() -> Self {
        register_native_modules();
        Self::default()
    }

    fn sym(&self, s: &str) -> Symbol {
        self.interner.intern(s)
    }

    /// Resolve a CLI path (file, or project directory with `lang.toml`) to the
    /// entry file and module root directory.
    pub fn entry_of(path: &Path) -> Result<(PathBuf, PathBuf, Option<String>), String> {
        if path.is_dir() {
            let manifest = path.join("lang.toml");
            let m = manifest::Manifest::load(&manifest)?;
            let selected = ENTRY.lock().unwrap().clone();
            *ENTRY_FN.lock().unwrap() = None;
            let file = match selected.as_deref() {
                None => m.entry.clone(),
                Some(t) if t.starts_with("task:") => {
                    let name = &t["task:".len()..];
                    let Some((_, file, func)) = m.tasks.iter().find(|(n, _, _)| n == name) else {
                        let known: Vec<&str> = m.tasks.iter().map(|t| t.0.as_str()).collect();
                        return Err(format!(
                            "unknown task `{name}` (declared in [tasks]: {})",
                            if known.is_empty() { "none".into() } else { known.join(", ") }
                        ));
                    };
                    *ENTRY_FN.lock().unwrap() = Some(func.clone());
                    file.clone()
                }
                Some(name) => match m.entries.iter().find(|(n, _)| n == name) {
                    Some((_, file)) => file.clone(),
                    None => {
                        let known: Vec<&str> = m.entries.iter().map(|e| e.0.as_str()).collect();
                        return Err(format!(
                            "unknown entry `{name}` (declared in [entries]: {})",
                            if known.is_empty() { "none".into() } else { known.join(", ") }
                        ));
                    }
                },
            };
            let entry = path.join(&file);
            let root = entry.parent().map(Path::to_path_buf).unwrap_or_else(|| path.to_path_buf());
            return Ok((entry, root, Some(m.name)));
        }
        let root = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        Ok((path.to_path_buf(), root, None))
    }

    /// Effective runtime settings with their sources: built-in defaults,
    /// `[runtime]`, the profile's `[profile.NAME.runtime]`, then
    /// `LANG_<SETTING>` environment overrides (e.g. `LANG_WORKERS`,
    /// `LANG_HTTP_MAX_IN_FLIGHT`), each validated like the manifest's.
    pub fn runtime_settings_with_sources(
        path: &Path,
        profile: Option<&str>,
    ) -> Result<Vec<(String, String, String)>, String> {
        let mut out: Vec<(String, String, String)> = manifest::RUNTIME_DEFAULTS
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string(), "default".to_string()))
            .collect();
        if path.is_dir() {
            let m = manifest::Manifest::load(&path.join("lang.toml"))?;
            if let Some(p) = profile
                && !m.profiles.iter().any(|(n, _)| n == p)
            {
                return Err(format!("unknown profile `{p}`"));
            }
            for (k, v, src) in m.effective_runtime(profile) {
                let src = match src {
                    manifest::Source::Base => "runtime".to_string(),
                    manifest::Source::Profile(p) => format!("profile.{p}.runtime"),
                };
                match out.iter_mut().find(|e| e.0 == k) {
                    Some(e) => {
                        e.1 = v;
                        e.2 = src;
                    }
                    None => out.push((k, v, src)),
                }
            }
        }
        for key in manifest::RUNTIME_KEYS {
            let var = format!("LANG_{}", key.to_uppercase());
            if let Ok(v) = std::env::var(&var) {
                manifest::check_runtime(key, &v, 0)
                    .map_err(|e| format!("{var}: {}", e.trim_start_matches("line 0: ")))?;
                if let Some(e) = out.iter_mut().find(|e| e.0 == *key) {
                    e.1 = v;
                    e.2 = "env".into();
                }
            }
        }
        Ok(out)
    }

    /// Effective runtime settings (see `runtime_settings_with_sources`);
    /// invalid values are left to that function's error.
    pub fn runtime_settings(path: &Path, profile: Option<&str>) -> Vec<(String, String)> {
        Self::runtime_settings_with_sources(path, profile)
            .map(|v| v.into_iter().map(|(k, v, _)| (k, v)).collect())
            .unwrap_or_default()
    }

    /// The project's effective configuration for a profile (empty for a
    /// single file without a profile).
    pub fn effective_config(
        path: &Path,
        profile: Option<&str>,
    ) -> Result<Vec<(String, String, manifest::Source)>, String> {
        if !path.is_dir() {
            return match profile {
                Some(p) => Err(format!("profile `{p}` requires a project directory with `lang.toml`")),
                None => Ok(vec![]),
            };
        }
        let m = manifest::Manifest::load(&path.join("lang.toml"))?;
        m.effective(profile)
    }

    /// A source file's text: its unsaved editor buffer or proposed content,
    /// if any, else the file.
    fn read_source(&self, path: &Path) -> std::io::Result<String> {
        if !self.overlays.is_empty()
            && let Some(t) = self.overlays.get(&overlay_key(path))
        {
            return Ok(t.clone());
        }
        std::fs::read_to_string(path)
    }

    /// Load and parse the standard library and every user module reachable
    /// from `path`.
    pub fn load(&mut self, path: &Path) -> Option<Loaded> {
        let (entry_file, root, package) = match Self::entry_of(path) {
            Ok(x) => x,
            Err(e) => {
                self.diagnostics.push(Diagnostic::error(e).with_code("E0010"));
                return None;
            }
        };
        let deps = if path.is_dir() {
            let verified =
                manifest::Manifest::load(&path.join("lang.toml")).and_then(|m| deps::verified(path, &m));
            match verified {
                Ok(d) => d,
                Err(e) => {
                    self.diagnostics.push(Diagnostic::error(e).with_code("E0015"));
                    return None;
                }
            }
        } else {
            vec![]
        };
        // packages: the root project, then each dependency (module roots)
        let packages: Vec<(Option<String>, PathBuf)> = std::iter::once((package.clone(), root.clone()))
            .chain(deps.iter().map(|d| (Some(d.name.clone()), d.src_root.clone())))
            .collect();
        let mut modules = vec![];
        let std_sym = self.sym("std");
        let text = match self.read_source(&entry_file) {
            Ok(t) => t,
            Err(e) => {
                self.diagnostics.push(
                    Diagnostic::error(format!("cannot read {}: {e}", entry_file.display()))
                        .with_code("E0011"),
                );
                return None;
            }
        };
        let stem = entry_file.file_stem().and_then(|s| s.to_str()).unwrap_or("main").to_string();
        let suffix_start = text.len() + 1;
        let text = if self.entry_suffix.is_empty() { text } else { format!("{text}\n{}", self.entry_suffix) };
        let file = self.sources.add(entry_file.display().to_string(), text);
        // known before parsing, so syntax errors name their module too
        self.sources.set_module(file, stem.clone());
        let mut ast = self.parse(file)?;
        if !self.entry_suffix.is_empty() {
            // a REPL definition replaces the program's item of the same name
            let later: Vec<Symbol> = ast
                .items
                .iter()
                .filter(|it| it.span.range().start >= suffix_start)
                .filter_map(|it| item_name(&ast, it))
                .collect();
            let items = std::mem::take(&mut ast.items);
            ast.items = items
                .into_iter()
                .filter(|it| {
                    it.span.range().start >= suffix_start
                        || !item_name(&ast, it).is_some_and(|n| later.contains(&n))
                })
                .collect();
        }
        let entry = modules.len();
        modules.push(ParsedModule { path: vec![self.sym(&stem)], file, ast, is_std: false });
        // breadth-first discovery of imported user modules; each module
        // resolves imports relative to its own package
        let mut module_pkg: Vec<usize> = vec![0; modules.len()];
        let mut i = entry;
        while i < modules.len() {
            let pkg = module_pkg[i];
            let imports: Vec<Vec<Symbol>> = modules[i]
                .ast
                .items
                .iter()
                .filter_map(|it| match &it.kind {
                    ast::ItemKind::Import(imp) => Some(imp.path.iter().map(|n| n.sym).collect()),
                    _ => None,
                })
                .collect();
            for p in imports {
                if p.first() == Some(&std_sym) {
                    continue;
                }
                let mut segs: Vec<String> = p.iter().map(|s| self.interner.resolve(*s).to_string()).collect();
                // a dependency's modules: `import dep` (its entry) or `import dep.module`
                let dep = deps.iter().position(|d| d.name.replace('-', "_") == segs[0]);
                let (target, file_segs): (usize, Option<Vec<String>>) = match dep {
                    Some(di) if di + 1 != pkg => {
                        (di + 1, if segs.len() > 1 { Some(segs[1..].to_vec()) } else { None })
                    }
                    _ => {
                        let own = packages[pkg].0.as_deref();
                        if (own.is_some_and(|pk| pk.replace('-', "_") == segs[0])
                            || (pkg == 0 && segs[0] == "app"))
                            && segs.len() > 1
                        {
                            segs.remove(0);
                        }
                        (pkg, Some(segs.clone()))
                    }
                };
                if modules.iter().any(|m| m.path == p) {
                    continue;
                }
                if target == pkg && pkg == 0 {
                    let mpath: Vec<Symbol> = segs.iter().map(|s| self.sym(s)).collect();
                    if modules.iter().any(|m| m.path == mpath) {
                        continue;
                    }
                }
                let fpath = match &file_segs {
                    Some(fs) => packages[target].1.join(fs.join("/") + ".lang"),
                    None => deps[target - 1].entry.clone(),
                };
                let Ok(text) = self.read_source(&fpath) else { continue };
                let file = self.sources.add(fpath.display().to_string(), text);
                let mname: Vec<String> = p.iter().map(|s| self.interner.resolve(*s).to_string()).collect();
                self.sources.set_module(file, mname.join("."));
                if let Some(ast) = self.parse(file) {
                    // modules are addressable by both the written and stripped path
                    modules.push(ParsedModule { path: p.clone(), file, ast, is_std: false });
                    module_pkg.push(target);
                }
            }
            i += 1;
        }
        // the standard library: the modules the program can reach (imported
        // directly or by other standard modules) and the core modules whose
        // implementations apply to prelude types; std modules come first
        let std_mods = self.load_std(&modules);
        let n = std_mods.len();
        let mut all = std_mods;
        all.extend(modules);
        Some(Loaded { modules: all, entry: entry + n, root })
    }

    /// Parse the standard-library modules a program needs.
    fn load_std(&mut self, user: &[ParsedModule]) -> Vec<ParsedModule> {
        // implementations on prelude types and items the compiler uses
        // without an import (`Data` conversion, record operations)
        const CORE: &[&str] = &[
            "prelude", "list", "seq", "text", "map", "set", "option", "result", "num", "bytes", "buffer",
            "array", "codec", "records",
        ];
        let std_sym = self.sym("std");
        let names: Vec<&str> = lang_hir::stdlib::STD_MODULES.iter().map(|(n, _)| *n).collect();
        let mut wanted: Vec<String> = CORE.iter().map(|s| s.to_string()).collect();
        let add_imports = |ast: &ast::Ast, text: &str, wanted: &mut Vec<String>, interner: &Interner| {
            for it in &ast.items {
                if let ast::ItemKind::Import(imp) = &it.kind
                    && imp.path.first().map(|n| n.sym) == Some(std_sym)
                    && let Some(m) = imp.path.get(1)
                {
                    let m = interner.resolve(m.sym).to_string();
                    if !wanted.contains(&m) {
                        wanted.push(m);
                    }
                }
            }
            // `system { ... }` and `pipeline { ... }` expand into their modules
            for (kw, m) in [("system", "system"), ("pipeline", "pipeline")] {
                let mut rest = text;
                while let Some(i) = rest.find(kw) {
                    let after = rest[i + kw.len()..].trim_start();
                    let word_start =
                        i == 0 || !rest[..i].ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == '.');
                    if word_start && after.starts_with('{') && !wanted.iter().any(|w| w == m) {
                        wanted.push(m.to_string());
                    }
                    rest = &rest[i + kw.len()..];
                }
            }
        };
        for m in user {
            let text = self.sources.get(m.file).text.clone();
            add_imports(&m.ast, &text, &mut wanted, &self.interner);
        }
        // close over the standard modules' own imports
        let mut parsed: Vec<(usize, ParsedModule)> = vec![];
        let mut done: Vec<String> = vec![];
        while let Some(name) = wanted.iter().find(|w| !done.contains(w)).cloned() {
            done.push(name.clone());
            let Some(idx) = names.iter().position(|n| *n == name) else { continue };
            let src = lang_hir::stdlib::STD_MODULES[idx].1;
            let file = self.sources.add(format!("<std>/{name}.lang"), src);
            if let Some(ast) = self.parse(file) {
                add_imports(&ast, src, &mut wanted, &self.interner);
                let mut mpath = vec![std_sym];
                mpath.extend(name.split('/').map(|s| self.sym(s)));
                parsed.push((idx, ParsedModule { path: mpath, file, ast, is_std: true }));
            }
        }
        // in the library's own order
        parsed.sort_by_key(|(i, _)| *i);
        parsed.into_iter().map(|(_, m)| m).collect()
    }

    fn parse(&mut self, file: FileId) -> Option<ast::Ast> {
        let text = self.sources.get(file).text.clone();
        let mut diags = vec![];
        let r = lang_syntax::parse(file, &text, &mut self.interner, &mut diags);
        self.diagnostics.extend(diags);
        match r {
            Ok(ast) => Some(ast),
            Err(e) => {
                self.diagnostics.push(e.into());
                None
            }
        }
    }

    /// Resolve names and build HIR.
    pub fn lower(&mut self, loaded: &Loaded) -> Hir {
        let mut diags = vec![];
        let hir =
            lang_hir::lower_program(&loaded.modules, Some(loaded.entry), &mut self.interner, &mut diags);
        self.diagnostics.extend(diags);
        hir
    }

    /// Front end + static analysis only (every package, for a workspace
    /// root).
    /// Check `path` and report each contract clause and what the static
    /// analysis decided about it (`lang contracts`).
    pub fn contract_report(&mut self, path: &Path) -> (Outcome, Vec<contracts::Clause>) {
        let outcome = self.check(path);
        let mut clauses = contracts::last_report();
        for c in &mut clauses {
            let f = self.sources.get(c.span.file);
            c.file = f.name.clone();
            c.line = f.line_col(c.span.start as usize).0;
        }
        (outcome, clauses)
    }

    /// Run the tests of `path` observing the program, then report each
    /// clause with its static status and test coverage, and candidate
    /// clauses that held in every observed call (`lang contracts --tests`).
    pub fn contract_test_report(
        &mut self,
        path: &Path,
    ) -> Result<(Vec<contracts::Clause>, Vec<contracts::Suggestion>, (usize, usize)), Outcome> {
        let opts = RunOptions { tests: true, json: true, observe: true, ..Default::default() };
        select_entry(None);
        self.run(path, vec![], &opts)?;
        let (ran, failed, _) = take_tests();
        let (slots, outcomes) = lang_runtime::observe::take();
        let (mut clauses, mut suggestions) = contracts::merge_observations(&slots, &outcomes);
        for c in &mut clauses {
            let f = self.sources.get(c.span.file);
            c.file = f.name.clone();
            c.line = f.line_col(c.span.start as usize).0;
        }
        for s in &mut suggestions {
            let f = self.sources.get(s.span.file);
            s.file = f.name.clone();
            s.line = f.line_col(s.span.start as usize).0;
        }
        Ok((clauses, suggestions, (ran, failed)))
    }

    pub fn check(&mut self, path: &Path) -> Outcome {
        let packages = packages_of(path);
        if packages.len() != 1 || packages[0] != path {
            let mut outcome = Outcome::Ok;
            for p in packages {
                if self.check_package(&p) == Outcome::Errors {
                    outcome = Outcome::Errors;
                }
            }
            return outcome;
        }
        self.check_package(path)
    }

    fn check_package(&mut self, path: &Path) -> Outcome {
        // a project: its main entry, every `[entries]` module, and every
        // `[tasks]` module
        let others = project_entry_points(path);
        if !others.is_empty() {
            let mut outcome = self.check_one(path);
            for sel in others {
                *ENTRY.lock().unwrap() = Some(sel);
                if self.check_one(path) == Outcome::Errors {
                    outcome = Outcome::Errors;
                }
            }
            *ENTRY.lock().unwrap() = None;
            *ENTRY_FN.lock().unwrap() = None;
            self.diagnostics.dedupe(&self.sources);
            return outcome;
        }
        self.check_one(path)
    }

    fn check_one(&mut self, path: &Path) -> Outcome {
        let Some(loaded) = timed("parse", || self.load(path)) else { return Outcome::Errors };
        if self.diagnostics.has_errors() {
            return Outcome::Errors;
        }
        let hir = timed("resolve", || self.lower(&loaded));
        if self.diagnostics.has_errors() {
            // name errors also report their module and enclosing definition
            self.annotate_scopes(&hir);
            return Outcome::Errors;
        }
        let (checked, diags) = timed("typecheck", || check_typed(path, &hir, &self.interner));
        self.diagnostics.extend(diags);
        // a project's entry points: the function the tool runs exists and
        // takes no parameters or the arguments as `List<String>`
        if let Some(entry) = hir.entry {
            let name = ENTRY_FN.lock().unwrap().clone().unwrap_or_else(|| "main".into());
            let file = self.sources.get(hir.modules[entry].file).name.clone();
            match hir.modules[entry].items.get(&self.interner.intern(&name)).copied() {
                // a single file need not be a program (a library module),
                // and a library package has no `main`
                None if !path.is_dir() => {}
                None if name == "main"
                    && manifest::Manifest::load(&path.join("lang.toml")).is_ok_and(|m| m.library) => {}
                None => self
                    .diagnostics
                    .push(Diagnostic::error(format!("no `{name}` function in {file}")).with_code("E0012")),
                Some(d) if !matches!(hir.defs[d].kind, lang_hir::DefKind::Fn(_)) => self.diagnostics.push(
                    Diagnostic::error(format!("`{name}` in {file} is not a function"))
                        .with_code("E0012")
                        .with_label(hir.defs[d].span, "the tool invokes a typed function"),
                ),
                Some(d) => {
                    if let Some(s) = checked.g.scheme(d)
                        && let lang_typeck::TyKind::Fn(f) = &*s.ty
                    {
                        let ok = match f.params.as_slice() {
                            [] => true,
                            [p] => lang_typeck::show_ty(&checked.g, p) == "List<String>",
                            _ => false,
                        };
                        if !ok {
                            let sig = lang_typeck::show_ty(&checked.g, &s.ty);
                            self.diagnostics.push(
                                Diagnostic::error(format!("`{name}` must take no parameters or one `List<String>` of arguments (found {sig})"))
                                    .with_code("E0012")
                                    .with_label(hir.defs[d].span, "invoked by the tool"),
                            );
                        }
                    }
                }
            }
        }
        drop(checked);
        self.annotate_scopes(&hir);
        if self.diagnostics.has_errors() { Outcome::Errors } else { Outcome::Ok }
    }

    /// Give each diagnostic without one a readable scope: the innermost
    /// enclosing definition (`fn app.main`, `fn app.Point.norm`,
    /// `let app.settings`), and record each file's module path.
    pub(crate) fn annotate_scopes(&mut self, hir: &lang_hir::Hir) {
        annotate_scopes(&self.interner, &mut self.sources, &mut self.diagnostics, hir)
    }
}

fn annotate_scopes(
    interner: &Interner,
    sources: &mut SourceMap,
    diagnostics: &mut Diagnostics,
    hir: &lang_hir::Hir,
) {
    {
        for (_, m) in hir.modules.iter() {
            let path: Vec<&str> = m.path.iter().map(|s| interner.resolve(*s)).collect();
            sources.set_module(m.file, path.join("."));
        }
        let mut ranges: Vec<(lang_base::FileId, u32, u32, String)> = vec![];
        for (_, def) in hir.defs.iter() {
            let m = &hir.modules[def.module];
            if m.is_std {
                continue;
            }
            let module: Vec<&str> = m.path.iter().map(|s| interner.resolve(*s)).collect();
            let module = module.join(".");
            let name = interner.resolve(def.name).to_string();
            let (kind, body) = match &def.kind {
                lang_hir::DefKind::Fn(f) => match f.body {
                    Some(b) => {
                        let owner = f.owner.and_then(|o| match &hir.defs[o].kind {
                            lang_hir::DefKind::Impl(i) => {
                                let t = &sources.get(i.self_ty.span.file).text;
                                t.get(i.self_ty.span.range())
                                    .map(|x| x.split('<').next().unwrap_or(x).trim().to_string())
                            }
                            _ => Some(interner.resolve(hir.defs[o].name).to_string()),
                        });
                        let full = match owner {
                            Some(o) => format!("{o}.{name}"),
                            None => name.clone(),
                        };
                        (format!("fn {module}.{full}"), hir.exprs[b].span)
                    }
                    None => continue,
                },
                lang_hir::DefKind::Global(g) => (format!("let {module}.{name}"), hir.exprs[g.value].span),
                _ => continue,
            };
            if body.is_dummy() || body.file != def.span.file {
                continue;
            }
            ranges.push((def.span.file, def.span.start.min(body.start), body.end.max(def.span.end), kind));
        }
        for d in diagnostics.iter_mut() {
            if d.scope.is_some() {
                continue;
            }
            let Some(p) = d.primary() else { continue };
            let inner = ranges
                .iter()
                .filter(|(f, a, b, _)| *f == p.file && *a <= p.start && p.end <= *b)
                .min_by_key(|(_, a, b, _)| b - a);
            if let Some((_, _, _, name)) = inner {
                d.scope = Some(name.clone());
            }
        }
    }
}

impl Session {
    /// Compile with Cranelift and run `main` in-process. Returns the process
    /// exit code.
    pub fn run(&mut self, path: &Path, args: Vec<String>, opts: &RunOptions) -> Result<i32, Outcome> {
        let started = std::time::Instant::now();
        if opts.debug {
            lang_codegen::set_debug_info(true);
        }
        // a program saved by `lang compile`
        if path.is_file()
            && let Ok(bytes) = std::fs::read(path)
            && lang_mir::serial::is_binary(&bytes)
        {
            if opts.tests || opts.watch || opts.inspect.is_some() || opts.profile.is_some() {
                self.diagnostics.push(
                    Diagnostic::error("a compiled program runs as saved: `--watch`, `--inspect`, `--profile`, and tests need the sources")
                        .with_code("E0013"),
                );
                return Err(Outcome::Errors);
            }
            if opts.record.is_some() || opts.replay.is_some() {
                self.diagnostics.push(
                    Diagnostic::error("reproduction bundles record and replay programs from their sources")
                        .with_code("E0013"),
                );
                return Err(Outcome::Errors);
            }
            return self.run_binary(&bytes, args, opts, started);
        }
        // an invalid manifest is a project error (as `lang check` reports it)
        if path.is_dir() {
            match manifest::Manifest::load(&path.join("lang.toml")) {
                Err(e) => {
                    self.diagnostics.push(Diagnostic::error(e).with_code("E0010"));
                    return Err(Outcome::Errors);
                }
                Ok(m) if !m.has_package => {
                    let members: Vec<String> = deps::workspace_packages(path, &m)
                        .iter()
                        .map(|p| {
                            p.strip_prefix(std::fs::canonicalize(path).unwrap_or_default())
                                .unwrap_or(p)
                                .display()
                                .to_string()
                        })
                        .collect();
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "{} is a workspace, not a package: run one of its packages ({})",
                            path.display(),
                            members.join(", ")
                        ))
                        .with_code("E0010"),
                    );
                    return Err(Outcome::Errors);
                }
                Ok(m) if m.library && !opts.tests => {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "`{}` is a library: it has no `main` to run (`lang test` runs its tests)",
                            m.name
                        ))
                        .with_code("E0010"),
                    );
                    return Err(Outcome::Errors);
                }
                Ok(_) => {}
            }
        }
        let mut config: Vec<(String, String)> = vec![];
        if path.is_dir()
            && let Err(e) = load_dotenv(path)
        {
            self.diagnostics.push(Diagnostic::error(e).with_code("E0013"));
            return Err(Outcome::Errors);
        }
        match Self::effective_config(path, opts.profile.as_deref()) {
            Ok(entries) => {
                config = entries.into_iter().map(|(k, v, _)| (k, v)).collect();
                lang_runtime::intrinsics_codec::set_profile(config.clone())
            }
            Err(e) => {
                self.diagnostics.push(Diagnostic::error(e).with_code("E0013"));
                return Err(Outcome::Errors);
            }
        }
        if let Err(e) = Self::runtime_settings_with_sources(path, opts.profile.as_deref()) {
            self.diagnostics.push(Diagnostic::error(e).with_code("E0013"));
            return Err(Outcome::Errors);
        }
        let read_at = std::time::SystemTime::now();
        let built = self.build(path, opts)?;
        lang_runtime::task::set_workers(built.prog.workers);
        for (k, v) in Self::runtime_settings(path, opts.profile.as_deref()) {
            if k != "workers" {
                apply_runtime_setting(&k, &v);
            }
        }
        if opts.record.is_some() || opts.replay.is_some() {
            if opts.tests || opts.watch || opts.inspect.is_some() {
                self.diagnostics.push(
                    Diagnostic::error(
                        "`--record` and replays run a program once, without tests, `--watch`, or `--inspect`",
                    )
                    .with_code("E0013"),
                );
                return Err(Outcome::Errors);
            }
            // one worker, so the interleaving of tasks is the scheduler's
            // own and not the machine's
            lang_runtime::task::set_workers(1);
            let identity = program_identity(path, &built.files, &args, opts.profile.as_deref(), &config);
            if let Some(file) = &opts.record {
                lang_runtime::repro::start_record(file.clone(), identity);
            } else if let Some((file, compare)) = &opts.replay {
                let bundle = match load_bundle(file) {
                    Ok(b) => b,
                    Err(e) => {
                        self.diagnostics.push(Diagnostic::error(e).with_code("E0016"));
                        return Err(Outcome::Errors);
                    }
                };
                let changes = identity_changes(&bundle["program"], &identity);
                if !changes.is_empty() && !compare {
                    let mut d = Diagnostic::error(format!(
                        "the program is not the one {} recorded, so it cannot be replayed exactly",
                        file.display()
                    ))
                    .with_code("E0016")
                    .with_note("`lang replay --compare` runs the changed program against the recording");
                    for c in changes {
                        d = d.with_note(c);
                    }
                    self.diagnostics.push(d);
                    return Err(Outcome::Errors);
                }
                for c in &changes {
                    eprintln!("comparison: {c}");
                }
                if let Err(e) = lang_runtime::repro::start_replay(file.clone(), &bundle, *compare) {
                    self.diagnostics.push(Diagnostic::error(e).with_code("E0016"));
                    return Err(Outcome::Errors);
                }
            }
        }
        lang_runtime::intrinsics_io::set_args(args);
        if opts.tests {
            return Ok(run_tests(&built.compiled, &built.prog, opts));
        }
        if let Some(addr) = &opts.inspect
            && let Err(e) = inspect::serve(addr, path.to_path_buf(), opts.optimize)
        {
            self.diagnostics.push(Diagnostic::error(e).with_code("E0014"));
            return Err(Outcome::Errors);
        }
        if opts.watch || opts.inspect.is_some() {
            live::publish_version(&built);
        }
        if opts.watch {
            live::watch(path.to_path_buf(), built.files.clone(), opts.optimize, opts.reconstruct, read_at);
        }
        Ok(run_entry(built, opts.bench.then(|| started.elapsed())))
    }
}

/// Run a built program's entry (its arguments already set) and report how
/// it ended. With `bench` (the time compilation took), the run is timed
/// from the entry's start to its end and reported on stderr.
fn run_entry(built: Built, bench: Option<std::time::Duration>) -> i32 {
    {
        // an entry taking `List<String>` receives the program arguments
        if let Some(desc) = built.prog.entry_args {
            let l = lang_runtime::list::new(desc);
            lang_runtime::gc::pin(l);
            for a in lang_runtime::intrinsics_io::args() {
                let w = lang_runtime::string::new_string(&a);
                lang_runtime::list::push(l, w);
            }
            lang_runtime::task::set_main_args(Some(l));
        }
        let run_start = std::time::Instant::now();
        if let Some(compile) = bench {
            // a program ending itself with `process.exit` is reported too
            lang_runtime::intrinsics_os::set_exit_hook(Box::new(move || {
                eprintln!(
                    "bench: run {:.3} ms (compilation {:.3} ms, not included)",
                    run_start.elapsed().as_secs_f64() * 1e3,
                    compile.as_secs_f64() * 1e3
                );
            }));
        }
        let exit = lang_codegen::run_main(&built.compiled, &built.prog);
        let ran = run_start.elapsed();
        let (code, failure) = report_exit(exit, &built.prog);
        let code = lang_runtime::repro::finish(code, failure);
        if let Some(compile) = bench {
            eprintln!(
                "bench: run {:.3} ms (compilation {:.3} ms, not included)",
                ran.as_secs_f64() * 1e3,
                compile.as_secs_f64() * 1e3
            );
        }
        // The JIT image, task stacks, and heap live until process exit; leaving
        // without running destructors avoids tearing them down under parked tasks.
        std::mem::forget(built);
        code
    }
}

impl Session {
    /// Check and compile a program (every phase up to native code).
    pub fn build(&mut self, path: &Path, opts: &RunOptions) -> Result<Built, Outcome> {
        let (prog, files, types) = self.lower_to_mir(path, opts)?;
        let compiled = match timed("codegen", || lang_codegen::compile(&prog, opts.optimize)) {
            Ok(c) => c,
            Err(e) => {
                self.diagnostics
                    .push(Diagnostic::error(format!("code generation failed: {e}")).with_code("E9002"));
                return Err(Outcome::Errors);
            }
        };
        Ok(Built { prog, compiled, files, types })
    }

    /// Every phase before code generation: load, check, and lower to MIR.
    #[allow(clippy::type_complexity)]
    fn lower_to_mir(
        &mut self,
        path: &Path,
        opts: &RunOptions,
    ) -> Result<(lang_mir::Program, Vec<PathBuf>, String), Outcome> {
        let loaded = timed("parse", || self.load(path)).ok_or(Outcome::Errors)?;
        if self.diagnostics.has_errors() {
            return Err(Outcome::Errors);
        }
        let hir = timed("resolve", || self.lower(&loaded));
        if self.diagnostics.has_errors() {
            return Err(Outcome::Errors);
        }
        let (checked, diags) = timed("typecheck", || check_typed(path, &hir, &self.interner));
        self.diagnostics.extend(diags);
        annotate_scopes(&self.interner, &mut self.sources, &mut self.diagnostics, &hir);
        if self.diagnostics.has_errors() {
            return Err(Outcome::Errors);
        }
        // `lang contracts --tests`: instrument the program's functions and
        // clauses, and check every clause (no elision)
        if opts.observe {
            lang_mir::lower::set_observe(contracts::observe_plan(&hir));
            contracts::resolve_names(&self.interner);
            lang_mir::lower::set_elided(Default::default());
            lang_runtime::observe::reset();
        } else {
            lang_mir::lower::set_observe(Default::default());
        }
        let entry_fn = entry_fn();
        let (mut prog, errors) = timed("lower", || {
            lang_mir::lower_program_optimized(
                &checked.g,
                &self.sources,
                opts.tests,
                &entry_fn,
                opts.optimize
                    && !opts.watch
                    && opts.inspect.is_none()
                    && std::env::var_os("LANG_NO_INLINE").is_none(),
            )
        });
        for e in errors {
            self.diagnostics
                .push(Diagnostic::error(format!("internal compiler error: {e}")).with_code("E9001"));
        }
        for w in std::mem::take(&mut prog.warnings) {
            self.diagnostics.push(
                Diagnostic::warning(w)
                    .with_code("E0147")
                    .with_note("calling it fails at run time; give the recursion a bounded type, or remove the protocol constraint so it can use the erased fallback"),
            );
        }
        if let Some(e) = &prog.entry_error
            && !opts.tests
        {
            self.diagnostics.push(Diagnostic::error(e.clone()).with_code("E0012"));
        }
        if prog.entry.is_none() && !opts.tests {
            self.diagnostics.push(
                Diagnostic::error(format!("no `{entry_fn}` function in the entry module")).with_code("E0012"),
            );
        }
        if self.diagnostics.has_errors() {
            return Err(Outcome::Errors);
        }
        // live dispatch: `--watch` replaces functions; `--inspect` serves a
        // remote REPL whose definitions do the same
        prog.live = opts.watch || opts.inspect.is_some();
        let runtime_workers = Self::runtime_settings(path, opts.profile.as_deref())
            .into_iter()
            .find(|(k, _)| k == "workers")
            .and_then(|(_, v)| v.parse().ok());
        if let Some(w) = opts.workers
            && !(1..=64).contains(&w)
        {
            self.diagnostics.push(
                Diagnostic::error(format!("--workers must be between 1 and 64 (got {w})")).with_code("E0013"),
            );
            return Err(Outcome::Errors);
        }
        prog.workers = opts.workers.or(runtime_workers).unwrap_or(1).clamp(1, 64);
        // optimized builds inline small helpers (not under live dispatch,
        // where each function stays replaceable)
        if opts.optimize && !prog.live && std::env::var_os("LANG_NO_INLINE").is_none() {
            timed("inline", || lang_mir::inline::inline(&mut prog));
            // receives whose result is only examined skip building it
            timed("split", || lang_mir::split::split_receives(&mut prog));
            timed("niche", || lang_mir::niche::elide_local_options(&mut prog));
            timed("enums", || lang_mir::enums::scalarize(&mut prog));
            timed("reach", || lang_mir::reach::prune(&mut prog));
            for e in lang_mir::verify::verify(&prog) {
                self.diagnostics
                    .push(Diagnostic::error(format!("internal compiler error: {e}")).with_code("E9001"));
            }
            if self.diagnostics.has_errors() {
                return Err(Outcome::Errors);
            }
        }
        if opts.dump_mir {
            eprintln!("{}", lang_mir::mir::dump(&prog));
        }
        let files = loaded
            .modules
            .iter()
            .filter(|m| !m.is_std)
            .map(|m| PathBuf::from(&self.sources.get(m.file).name))
            .collect();
        let types = live::type_fingerprint(&checked.g);
        Ok((prog, files, types))
    }

    /// `lang compile`: check and lower a program, then save it as a binary
    /// MIR file with its effective configuration and runtime settings, so
    /// `lang run FILE` only generates code (environment overrides of the
    /// runtime settings still apply when it runs).
    pub fn compile_binary(&mut self, path: &Path, opts: &RunOptions) -> Result<Vec<u8>, Outcome> {
        if path.is_dir()
            && let Err(e) = manifest::Manifest::load(&path.join("lang.toml"))
        {
            self.diagnostics.push(Diagnostic::error(e).with_code("E0010"));
            return Err(Outcome::Errors);
        }
        let config = match Self::effective_config(path, opts.profile.as_deref()) {
            Ok(c) => c,
            Err(e) => {
                self.diagnostics.push(Diagnostic::error(e).with_code("E0013"));
                return Err(Outcome::Errors);
            }
        };
        let settings = match Self::runtime_settings_with_sources(path, opts.profile.as_deref()) {
            Ok(s) => s,
            Err(e) => {
                self.diagnostics.push(Diagnostic::error(e).with_code("E0013"));
                return Err(Outcome::Errors);
            }
        };
        let (prog, _, _) = self.lower_to_mir(path, opts)?;
        self.natives = prog.natives.iter().cloned().collect();
        let mut meta: Vec<(String, String)> =
            config.into_iter().map(|(k, v, _)| (format!("config.{k}"), v)).collect();
        // the effective settings at compile time (manifest, profile, and
        // environment), with `--workers` deciding the saved worker count
        meta.extend(settings.into_iter().map(|(k, v, _)| {
            let v = match (k.as_str(), opts.workers) {
                ("workers", Some(w)) => w.to_string(),
                _ => v,
            };
            (format!("runtime.{k}"), v)
        }));
        match lang_mir::serial::encode(&prog, &meta) {
            Ok(bytes) => Ok(bytes),
            Err(e) => {
                self.diagnostics
                    .push(Diagnostic::error(format!("cannot save the program: {e}")).with_code("E9002"));
                Err(Outcome::Errors)
            }
        }
    }

    /// Run a program saved by `lang compile`: no parsing or checking, only
    /// code generation.
    pub fn run_binary(
        &mut self,
        bytes: &[u8],
        args: Vec<String>,
        opts: &RunOptions,
        started: std::time::Instant,
    ) -> Result<i32, Outcome> {
        let (mut prog, meta) = match lang_mir::serial::decode(bytes) {
            Ok(x) => x,
            Err(e) => {
                self.diagnostics.push(Diagnostic::error(e).with_code("E0011"));
                return Err(Outcome::Errors);
            }
        };
        // runtime entry points code generation knows
        let unknown = prog.funcs.iter().flat_map(|f| f.blocks.iter()).flat_map(|b| b.stmts.iter()).find_map(
            |s| match s {
                lang_mir::mir::Stmt::Assign(
                    _,
                    lang_mir::mir::Rvalue::Call { callee: lang_mir::mir::Callee::Runtime(n), .. },
                ) if !lang_codegen::is_runtime_function(n) => Some(n.to_string()),
                _ => None,
            },
        );
        if let Some(n) = unknown {
            self.diagnostics.push(
                Diagnostic::error(format!("invalid compiled program: unknown runtime function `{n}`"))
                    .with_code("E0011"),
            );
            return Err(Outcome::Errors);
        }
        let config: Vec<(String, String)> = meta
            .iter()
            .filter_map(|(k, v)| k.strip_prefix("config.").map(|k| (k.to_string(), v.clone())))
            .collect();
        lang_runtime::intrinsics_codec::set_profile(config);
        let mut settings: Vec<(String, String)> = meta
            .iter()
            .filter_map(|(k, v)| k.strip_prefix("runtime.").map(|k| (k.to_string(), v.clone())))
            .collect();
        for key in manifest::RUNTIME_KEYS {
            let var = format!("LANG_{}", key.to_uppercase());
            if let Ok(v) = std::env::var(&var) {
                if let Err(e) = manifest::check_runtime(key, &v, 0) {
                    self.diagnostics.push(
                        Diagnostic::error(format!("{var}: {}", e.trim_start_matches("line 0: ")))
                            .with_code("E0013"),
                    );
                    return Err(Outcome::Errors);
                }
                match settings.iter_mut().find(|s| s.0 == *key) {
                    Some(s) => s.1 = v,
                    None => settings.push((key.to_string(), v)),
                }
            }
        }
        let saved_workers = settings.iter().find(|s| s.0 == "workers").and_then(|s| s.1.parse().ok());
        if let Some(w) = opts.workers
            && !(1..=64).contains(&w)
        {
            self.diagnostics.push(
                Diagnostic::error(format!("--workers must be between 1 and 64 (got {w})")).with_code("E0013"),
            );
            return Err(Outcome::Errors);
        }
        prog.workers = opts
            .workers
            .or(saved_workers)
            .or_else(|| meta.iter().find(|m| m.0 == "workers").and_then(|m| m.1.parse().ok()))
            .unwrap_or(1)
            .clamp(1, 64);
        if opts.dump_mir {
            eprintln!("{}", lang_mir::mir::dump(&prog));
        }
        let compiled = match timed("codegen", || lang_codegen::compile(&prog, opts.optimize)) {
            Ok(c) => c,
            Err(e) => {
                self.diagnostics
                    .push(Diagnostic::error(format!("code generation failed: {e}")).with_code("E9002"));
                return Err(Outcome::Errors);
            }
        };
        lang_runtime::task::set_workers(prog.workers);
        for (k, v) in &settings {
            if k != "workers" {
                apply_runtime_setting(k, v);
            }
        }
        lang_runtime::intrinsics_io::set_args(args);
        let built = Built { prog, compiled, files: vec![], types: Default::default() };
        Ok(run_entry(built, opts.bench.then(|| started.elapsed())))
    }
}

/// The name an item defines (for REPL replacement).
fn item_name(ast: &ast::Ast, it: &ast::Item) -> Option<Symbol> {
    match &it.kind {
        ast::ItemKind::Fn(d) | ast::ItemKind::Extern { decl: d, .. } => Some(d.name.sym),
        ast::ItemKind::Struct(d) => Some(d.name.sym),
        ast::ItemKind::Enum(d) => Some(d.name.sym),
        ast::ItemKind::Protocol(d) => Some(d.name.sym),
        ast::ItemKind::TypeAlias { name, .. } => Some(name.sym),
        ast::ItemKind::Global { pat, .. } => match &ast.pats[*pat].kind {
            ast::PatKind::Bind(n) => Some(n.sym),
            _ => None,
        },
        _ => None,
    }
}

/// A compiled program with what live reload needs to compare versions.
pub struct Built {
    pub prog: lang_mir::Program,
    pub compiled: lang_codegen::Compiled,
    /// User source files (watched in `--watch` mode).
    pub files: Vec<PathBuf>,
    /// Fingerprint of user type definitions.
    pub types: String,
}

fn apply_runtime_setting(k: &str, v: &str) {
    match k {
        "cleanup_grace_ms" => {
            if let Ok(ms) = v.parse() {
                lang_runtime::task::set_cleanup_grace(ms);
            }
        }
        "blocking_threads" => {
            if let Ok(n) = v.parse() {
                lang_runtime::task::set_blocking_threads(n);
            }
        }
        "task_stack_bytes" => {
            if let Ok(n) = v.parse() {
                lang_runtime::task::set_stack_size(n);
            }
        }
        "heap_max_bytes" => {
            if let Ok(n) = v.parse() {
                lang_runtime::gc::set_heap_max(n);
            }
        }
        "trace_capacity" | "trace_overflow" => {
            lang_runtime::intrinsics_codec::set_trace_setting(k, v);
        }
        "log_level" | "log_format" => {
            lang_runtime::telemetry::configure(k, v);
        }
        _ if k.starts_with("http_") => {
            if let Ok(n) = v.parse() {
                lang_runtime::http::set_default_limit(k, n);
            }
        }
        _ => {}
    }
}

/// `LANG_TIMINGS=1`: report each compiler phase's duration on stderr
/// (`[timings] typecheck: 12.345 ms`).
pub fn timed<T>(phase: &str, f: impl FnOnce() -> T) -> T {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ON.get_or_init(|| std::env::var_os("LANG_TIMINGS").is_some_and(|v| v == "1")) {
        return f();
    }
    let start = std::time::Instant::now();
    let r = f();
    eprintln!("[timings] {phase}: {:.3} ms", start.elapsed().as_secs_f64() * 1e3);
    r
}

#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub optimize: bool,
    pub dump_mir: bool,
    pub tests: bool,
    /// Test name filter.
    pub filter: Option<String>,
    /// Machine-readable test report.
    pub json: bool,
    /// Manifest profile to merge into the effective configuration.
    pub profile: Option<String>,
    /// Watch the sources and replace compatible functions while running.
    pub watch: bool,
    /// Serve inspection snapshots on this address while running.
    pub inspect: Option<String>,
    /// Worker threads (overrides `[runtime] workers`).
    pub workers: Option<usize>,
    /// With `watch`: restart on incompatible versions (controlled
    /// reconstruction) instead of rejecting them.
    pub reconstruct: bool,
    /// Time the program's execution (after all compilation) and report it.
    pub bench: bool,
    /// Register debug information for generated code with debuggers.
    pub debug: bool,
    /// Record a reproduction bundle to this file.
    pub record: Option<PathBuf>,
    /// Replay this reproduction bundle (with `true`: compare a changed
    /// program against it).
    pub replay: Option<(PathBuf, bool)>,
    /// Observe integer arguments, results, and contract outcomes while
    /// the tests run (`lang contracts --tests`).
    pub observe: bool,
}

/// Read a reproduction bundle written by `lang run --record`.
pub fn load_bundle(path: &Path) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not a reproduction bundle: {e}", path.display()))?;
    if v["format"] != lang_runtime::repro::FORMAT {
        return Err(format!(
            "{} is not a reproduction bundle of this toolchain (format {}, expected {})",
            path.display(),
            v["format"],
            lang_runtime::repro::FORMAT
        ));
    }
    Ok(v)
}

/// What identifies a recorded program: the toolchain, every user source
/// file, the arguments, and the effective configuration (as a digest).
fn program_identity(
    entry: &Path,
    files: &[PathBuf],
    args: &[String],
    profile: Option<&str>,
    config: &[(String, String)],
) -> serde_json::Value {
    use lang_runtime::repro::digest;
    let exe = std::env::current_exe().ok().and_then(|p| std::fs::read(p).ok()).map(|b| digest(&b));
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut sources: Vec<serde_json::Value> = files
        .iter()
        .map(|f| {
            let shown = f.strip_prefix(&cwd).unwrap_or(f).to_string_lossy().into_owned();
            let d = std::fs::read(f).map(|b| digest(&b)).unwrap_or_default();
            serde_json::json!({ "path": shown, "digest": d })
        })
        .collect();
    sources.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    let mut cfg: Vec<String> = config.iter().map(|(k, v)| format!("{k}={v}")).collect();
    cfg.sort();
    serde_json::json!({
        "toolchain": { "version": env!("CARGO_PKG_VERSION"), "executable": exe },
        "entry": entry.to_string_lossy(),
        "selected": *ENTRY.lock().unwrap(),
        "sources": sources,
        "args": args,
        "profile": profile,
        "config": digest(cfg.join("\n").as_bytes()),
    })
}

/// How the program now differs from the one a bundle recorded.
fn identity_changes(recorded: &serde_json::Value, now: &serde_json::Value) -> Vec<String> {
    let mut out = vec![];
    if recorded["toolchain"] != now["toolchain"] {
        out.push(format!(
            "the toolchain differs (recorded {} {}, now {} {})",
            recorded["toolchain"]["version"],
            recorded["toolchain"]["executable"],
            now["toolchain"]["version"],
            now["toolchain"]["executable"]
        ));
    }
    let files = |v: &serde_json::Value| -> Vec<(String, String)> {
        v["sources"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|s| {
                        (
                            s["path"].as_str().unwrap_or("").to_string(),
                            s["digest"].as_str().unwrap_or("").to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let (was, is) = (files(recorded), files(now));
    for (p, d) in &is {
        match was.iter().find(|(q, _)| q == p) {
            Some((_, e)) if e == d => {}
            Some(_) => out.push(format!("{p} changed")),
            None => out.push(format!("{p} was not part of the recorded program")),
        }
    }
    for (p, _) in &was {
        if !is.iter().any(|(q, _)| q == p) {
            out.push(format!("{p} is no longer part of the program"));
        }
    }
    if recorded["config"] != now["config"] {
        out.push("the effective configuration differs".into());
    }
    out
}

/// Report how a program ended; returns its exit code and what failed.
fn report_exit(exit: lang_runtime::task::Exit, prog: &lang_mir::Program) -> (i32, Option<String>) {
    use lang_runtime::task::Exit;
    match exit {
        Exit::Returned(w) => {
            // `main` returning `Result.err(e)` reports the error: with its
            // code, causes, and details when it implements `Error`, and
            // where it started propagating
            if let Some(d) = prog.entry_ret
                && d.name == "Result"
            {
                let (tag, fields) = lang_runtime::enum_parts(w);
                if tag == 1 {
                    let origin = lang_runtime::abi::error_origin(w);
                    let report = prog.error_report.and_then(|f| {
                        let closure = prog.static_closures.iter().find(|(_, g)| *g == f).map(|(o, _)| *o)?;
                        lang_runtime::gc::pin(fields[0]);
                        lang_runtime::task::set_main_args(Some(fields[0]));
                        match lang_runtime::task::run_main(closure) {
                            Exit::Returned(s) => Some(lang_runtime::string::str_of(s).to_string()),
                            _ => None,
                        }
                    });
                    let msg = report.unwrap_or_else(|| {
                        let ed = d.variant(1).unwrap().fields[0].desc;
                        format!(
                            "error: {}",
                            lang_runtime::value::display(std::ptr::null_mut(), ed, fields[0], true)
                        )
                    });
                    eprintln!("{msg}");
                    let mut failure = msg;
                    if let Some(at) = origin {
                        eprintln!("  at {at}");
                        failure = format!("{failure} at {at}");
                    }
                    return (1, Some(failure));
                }
            }
            (0, None)
        }
        Exit::Failed(p) => {
            let failure = if p.location.is_empty() {
                format!("panic: {}", p.message)
            } else {
                format!("panic: {} at {}", p.message, p.location)
            };
            eprintln!("{failure}");
            for n in p.notes {
                eprintln!("  note: {n}");
            }
            (101, Some(failure))
        }
        Exit::Cancelled => {
            eprintln!("main task cancelled");
            (101, Some("main task cancelled".into()))
        }
    }
}

/// Results of the tests run so far in this process (a project's entries
/// may share modules: each test runs once).
struct TestTally {
    seen: std::collections::HashSet<String>,
    ran: usize,
    failed: usize,
    results: Vec<String>,
}

static TALLY: std::sync::Mutex<Option<TestTally>> = std::sync::Mutex::new(None);

/// Print the summary of every test run in this process; returns the exit code.
/// The tests run so far: how many, how many failed, and their JSON
/// results (consumed).
pub fn take_tests() -> (usize, usize, Vec<String>) {
    let t = TALLY.lock().unwrap().take();
    t.map(|t| (t.ran, t.failed, t.results)).unwrap_or((0, 0, vec![]))
}

pub fn finish_tests(json: bool) -> i32 {
    let t = TALLY.lock().unwrap().take();
    let (ran, failed, results) = t.map(|t| (t.ran, t.failed, t.results)).unwrap_or((0, 0, vec![]));
    if json {
        println!("{{\"passed\":{},\"failed\":{failed},\"tests\":[{}]}}", ran - failed, results.join(","));
    } else {
        println!(
            "\ntest result: {}. {} passed; {failed} failed",
            if failed == 0 { "ok" } else { "FAILED" },
            ran - failed
        );
    }
    if failed == 0 { 0 } else { 1 }
}

fn run_tests(c: &lang_codegen::Compiled, prog: &lang_mir::Program, opts: &RunOptions) -> i32 {
    use lang_runtime::task::Exit;
    let mut tally = TALLY.lock().unwrap().take().unwrap_or(TestTally {
        seen: Default::default(),
        ran: 0,
        failed: 0,
        results: vec![],
    });
    let (mut ran, mut failed) = (0, 0);
    let mut results: Vec<String> = vec![];
    for (name, f) in &prog.tests {
        if opts.filter.as_ref().is_some_and(|flt| !name.contains(flt.as_str()))
            || !tally.seen.insert(name.clone())
        {
            continue;
        }
        ran += 1;
        let closure = prog.static_closures.iter().find(|(_, g)| g == f).map(|(o, _)| *o).unwrap();
        let _ = c;
        lang_runtime::task::reset_test_state();
        let exit = lang_runtime::task::run_main(closure);
        lang_runtime::task::print_stats();
        let ok = match &exit {
            Exit::Returned(w) => {
                let r = prog.funcs[*f].ret;
                // tests may return Unit or Result<Unit, E>
                if r == lang_runtime::Repr::Ptr { lang_runtime::enum_parts(*w).0 == 0 } else { true }
            }
            _ => false,
        };
        let detail = match &exit {
            Exit::Failed(p) => format!("{} at {}", p.message, p.location),
            Exit::Cancelled => "cancelled".into(),
            Exit::Returned(_) if !ok => "returned an error".into(),
            _ => String::new(),
        };
        if !ok {
            failed += 1;
        }
        if opts.json {
            results.push(format!(
                "{{\"name\":{},\"ok\":{ok},\"detail\":{}}}",
                lang_base::diagnostics::json_str(name),
                lang_base::diagnostics::json_str(&detail)
            ));
        } else if ok {
            println!("test {name} ... ok");
        } else {
            println!("test {name} ... FAILED ({detail})");
        }
    }
    tally.ran += ran;
    tally.failed += failed;
    tally.results.extend(results);
    *TALLY.lock().unwrap() = Some(tally);
    0
}

impl Session {
    /// Type scheme and effect summary of a named definition in the entry
    /// module (`lang explain type|effect`).
    pub fn explain_def(&mut self, path: &Path, name: &str) -> Option<(String, String)> {
        let loaded = self.load(path)?;
        let hir = timed("resolve", || self.lower(&loaded));
        if self.diagnostics.has_errors() {
            return None;
        }
        let (checked, diags) = timed("typecheck", || check_typed(path, &hir, &self.interner));
        self.diagnostics.extend(diags);
        let entry = hir.entry?;
        let sym = self.interner.intern(name);
        let def = hir.modules[entry].items.get(&sym).copied()?;
        describe_def(&checked.g, &hir, &self.interner, &self.sources, def)
    }

    /// System plans in the entry module: node keys, providers, references.
    pub fn inspect_plans(&mut self, path: &Path) -> Option<Vec<PlanInfo>> {
        let loaded = self.load(path)?;
        let m = &loaded.modules[loaded.entry];
        let text = &self.sources.get(m.file).text;
        let mut plans = vec![];
        for (_, e) in m.ast.exprs.iter() {
            if let ast::ExprKind::System(entries) = &e.kind {
                let mut nodes = vec![];
                for (label, value) in entries {
                    let mut refs = vec![];
                    collect_ast_refs(&m.ast, *value, &mut refs);
                    let provider = match &m.ast.exprs[*value].kind {
                        ast::ExprKind::Call { args, .. } if !args.is_empty() => {
                            let sp = m.ast.exprs[args[0]].span;
                            text[sp.range()].to_string()
                        }
                        _ => "?".into(),
                    };
                    nodes.push(PlanNode {
                        key: self.interner.resolve(label.sym).to_string(),
                        provider,
                        refs: refs.iter().map(|r| self.interner.resolve(*r).to_string()).collect(),
                    });
                }
                let (line, _) = lang_base::diagnostics::line_col(text, e.span.start as usize);
                plans.push(PlanInfo { kind: "system", line, nodes });
            }
            // pipelines: their stages in order (the last entry is the handler)
            if let ast::ExprKind::Pipeline(entries) = &e.kind {
                let nodes = entries
                    .iter()
                    .enumerate()
                    .map(|(i, x)| {
                        let sp = m.ast.exprs[*x].span;
                        PlanNode { key: i.to_string(), provider: text[sp.range()].to_string(), refs: vec![] }
                    })
                    .collect();
                let (line, _) = lang_base::diagnostics::line_col(text, e.span.start as usize);
                plans.push(PlanInfo { kind: "pipeline", line, nodes });
            }
        }
        Some(plans)
    }
}

#[derive(Debug, Clone)]
pub struct PlanNode {
    pub key: String,
    pub provider: String,
    pub refs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PlanInfo {
    /// `system` or `pipeline`.
    pub kind: &'static str,
    pub line: usize,
    pub nodes: Vec<PlanNode>,
}

impl PlanInfo {
    /// Startup order: dependencies first, ties broken by source order.
    pub fn startup_order(&self) -> Vec<String> {
        let mut done: Vec<String> = vec![];
        while done.len() < self.nodes.len() {
            let next = self.nodes.iter().find(|n| {
                !done.contains(&n.key)
                    && n.refs.iter().all(|r| done.contains(r) || !self.nodes.iter().any(|m| &m.key == r))
            });
            match next {
                Some(n) => done.push(n.key.clone()),
                None => break,
            }
        }
        done
    }
}

fn collect_ast_refs(a: &ast::Ast, e: ast::ExprId, out: &mut Vec<Symbol>) {
    // walk the configuration expression for `ref(:key)`
    for (_, x) in a.exprs.iter() {
        let _ = x;
    }
    fn go(a: &ast::Ast, e: ast::ExprId, out: &mut Vec<Symbol>) {
        use ast::ExprKind as K;
        match &a.exprs[e].kind {
            K::Ref(l) => out.push(l.sym),
            K::Call { callee, args, .. } => {
                go(a, *callee, out);
                args.iter().for_each(|x| go(a, *x, out));
            }
            K::Record { spread, entries } => {
                spread.iter().for_each(|s| go(a, *s, out));
                for en in entries {
                    if let ast::RecordEntry::Field { value, .. } = en {
                        go(a, *value, out);
                    }
                }
            }
            K::List(xs) | K::Tuple(xs) | K::Set(xs) => xs.iter().for_each(|x| go(a, *x, out)),
            K::Field { base, .. } | K::Try(base) => go(a, *base, out),
            K::Index { base, index } => {
                go(a, *base, out);
                go(a, *index, out);
            }
            K::Unary { operand, .. } => go(a, *operand, out),
            K::Binary { lhs, rhs, .. } => {
                go(a, *lhs, out);
                go(a, *rhs, out);
            }
            K::If { cond, then, els } => {
                go(a, *cond, out);
                go(a, *then, out);
                els.iter().for_each(|x| go(a, *x, out));
            }
            K::Map(kv) => kv.iter().for_each(|(k, v)| {
                go(a, *k, out);
                go(a, *v, out);
            }),
            _ => {}
        }
    }
    go(a, e, out);
}

/// Mark this process as a local REPL session (see
/// `lang_typeck::set_repl_session`).
pub fn set_repl_session() {
    lang_typeck::set_repl_session();
}

/// A session serving others (`lang mcp`): `process.exit` in evaluated code
/// is a defect instead of ending the process.
pub fn forbid_exit() {
    lang_runtime::intrinsics_os::forbid_exit();
}

/// A definition's declared shape (types, protocols) or type and effects
/// (functions and values), as `lang explain` and editor hovers show it.
pub fn describe_def(
    g: &lang_typeck::Global<'_>,
    hir: &lang_hir::Hir,
    interner: &Interner,
    sources: &SourceMap,
    def: lang_hir::DefId,
) -> Option<(String, String)> {
    let name = interner.resolve(hir.defs[def].name).to_string();
    let name = name.as_str();
    let names = lang_typeck::program::Names { g, skolems: &[] };
    let gen_names = |gs: &[lang_hir::GenericId]| -> Vec<String> {
        gs.iter().map(|g| interner.resolve(hir.generics[*g].name).to_string()).collect()
    };
    let gen_list = |ns: &[String]| if ns.is_empty() { String::new() } else { format!("<{}>", ns.join(", ")) };
    // type definitions: their declared shape
    match &hir.defs[def].kind {
        lang_hir::DefKind::Struct(sd) => {
            let gn = gen_names(&sd.generics);
            let mut pr = lang_typeck::ty::Printer::new(&names);
            pr.gen_names = gn.clone();
            let info = g.env.structs.get(&def)?;
            let fields: Vec<String> = info
                .fields
                .iter()
                .map(|(n, t, public)| {
                    format!("{}{}: {}", if *public { "pub " } else { "" }, interner.resolve(*n), pr.print(t))
                })
                .collect();
            let derives: Vec<String> =
                sd.derives.iter().map(|d| interner.resolve(hir.defs[d.protocol].name).to_string()).collect();
            let der =
                if derives.is_empty() { String::new() } else { format!(" derive({})", derives.join(", ")) };
            return Some((
                format!("struct {name}{} {{ {} }}{der}", gen_list(&gn), fields.join(", ")),
                "(a type)".into(),
            ));
        }
        lang_hir::DefKind::Enum(ed) => {
            let gn = gen_names(&ed.generics);
            let mut pr = lang_typeck::ty::Printer::new(&names);
            pr.gen_names = gn.clone();
            let info = g.env.enums.get(&def)?;
            let vs: Vec<String> = info
                .variants
                .iter()
                .map(|v| {
                    let n = interner.resolve(v.name).to_string();
                    if v.fields.is_empty() {
                        return n;
                    }
                    let fs: Vec<String> = v
                        .fields
                        .iter()
                        .map(|(l, t)| match l {
                            Some(l) => format!("{}: {}", interner.resolve(*l), pr.print(t)),
                            None => pr.print(t),
                        })
                        .collect();
                    format!("{n}({})", fs.join(", "))
                })
                .collect();
            return Some((
                format!("enum {name}{} {{ {} }}", gen_list(&gn), vs.join(", ")),
                "(a type)".into(),
            ));
        }
        lang_hir::DefKind::Protocol(pd) => {
            let info = g.env.protocols.get(&def)?;
            let mut pg = vec!["Self".to_string()];
            pg.extend(gen_names(&pd.generics));
            let mut ms: Vec<String> = info
                .methods
                .iter()
                .filter_map(|(n, m)| {
                    let sc = g.scheme(*m)?;
                    let mut pr = lang_typeck::ty::Printer::new(&names);
                    pr.gen_names = pg.clone();
                    Some(format!("{}: {}", interner.resolve(*n), pr.print(&sc.ty)))
                })
                .collect();
            ms.sort();
            let supers: Vec<String> =
                info.supers.iter().map(|d| interner.resolve(hir.defs[*d].name).to_string()).collect();
            let sup = if supers.is_empty() { String::new() } else { format!(": {}", supers.join(" + ")) };
            return Some((format!("protocol {name}{sup} {{ {} }}", ms.join(", ")), "(a protocol)".into()));
        }
        lang_hir::DefKind::Alias(_) | lang_hir::DefKind::Opaque(_) => {
            // the declaration's source line(s), whitespace-normalized
            let m = &hir.modules[hir.defs[def].module];
            let text = &sources.get(m.file).text;
            let sp = hir.defs[def].span.range();
            let start = text[..sp.start].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let end = text[sp.end..].find('\n').map(|i| sp.end + i).unwrap_or(text.len());
            let src = text[start..end].split_whitespace().collect::<Vec<_>>().join(" ");
            return Some((src, "(a type)".into()));
        }
        _ => {}
    }
    let scheme = g.scheme(def)?;
    let fn_generics = match &hir.defs[def].kind {
        lang_hir::DefKind::Fn(f) => gen_names(&f.generics),
        _ => vec![],
    };
    // a method's scheme numbers its impl's (or protocol's `Self` and)
    // generics first, then its own
    let all_generics = match &hir.defs[def].kind {
        lang_hir::DefKind::Fn(f) => {
            let (gs, protocol_self) = lang_typeck::program::fn_generics(hir, f);
            let mut ns: Vec<String> = if protocol_self { vec!["Self".into()] } else { vec![] };
            ns.extend(gen_names(&gs));
            ns
        }
        _ => vec![],
    };
    let mut pr = lang_typeck::ty::Printer::new(&names);
    pr.gen_names = all_generics;
    let mut ty = pr.print(&scheme.ty);
    // where-clause constraints on the generic parameters
    let preds: Vec<String> = scheme
        .constraints
        .iter()
        .filter_map(|c| match c {
            lang_typeck::solve::Constraint::Impl { proto, args, ty, .. } => {
                let pn = interner.resolve(hir.defs[*proto].name).to_string();
                let a = if args.is_empty() {
                    String::new()
                } else {
                    format!("<{}>", args.iter().map(|x| pr.print(x)).collect::<Vec<_>>().join(", "))
                };
                Some(format!("{}: {pn}{a}", pr.print(ty)))
            }
            _ => None,
        })
        .collect();
    if !fn_generics.is_empty() && ty.starts_with("fn(") {
        ty = format!("fn{}{}", gen_list(&fn_generics), &ty[2..]);
    }
    if !preds.is_empty() {
        ty = format!("{ty} where {}", preds.join(", "));
    }
    let eff = match &*scheme.ty {
        lang_typeck::TyKind::Fn(f) => {
            let (labels, tail) = lang_typeck::ty::row_fields(&f.eff);
            let mut ls: Vec<String> = labels
                .iter()
                .map(|(l, _)| interner.resolve(*l).to_string())
                .filter(|l| !l.starts_with('%'))
                .collect();
            ls.sort();
            // the effects of functions passed in are charged to the caller
            let _ = tail;
            let open = f
                .params
                .iter()
                .any(|p| lang_typeck::ty::ty_any(p, &mut |k| matches!(k, lang_typeck::TyKind::Fn(_))));
            format!("!{{{}}}{}", ls.join(", "), if open { " plus the effects of its callbacks" } else { "" })
        }
        _ => "(not a function)".to_string(),
    };
    Some((ty, eff))
}

/// The source of standard module `std.NAME` (its API and documentation).
pub fn std_source(name: &str) -> Option<&'static str> {
    lang_hir::stdlib::STD_MODULES.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// Names of the standard modules.
pub fn std_modules() -> Vec<&'static str> {
    lang_hir::stdlib::STD_MODULES.iter().map(|(n, _)| *n).collect()
}

/// Whether the project `path` belongs to is restricted (`[package]
/// restricted = true`).
pub fn restricted(path: &Path) -> bool {
    let root = if path.is_dir() {
        path.to_path_buf()
    } else {
        match Session::entry_of(path) {
            Ok((_, root, _)) => root,
            Err(_) => return false,
        }
    };
    manifest::Manifest::load(&root.join("lang.toml")).is_ok_and(|m| m.restricted)
}

/// The `[database] schema` file of the project `path` belongs to.
pub(crate) fn database_schema(path: &Path) -> Option<PathBuf> {
    let root = if path.is_dir() { path.to_path_buf() } else { Session::entry_of(path).ok()?.1 };
    let m = manifest::Manifest::load(&root.join("lang.toml")).ok()?;
    m.schema.map(|s| root.join(s))
}

/// Type-check, adding the ambient-authority check of restricted projects
/// and the checks of literal SQL.
fn check_typed<'a>(
    path: &Path,
    hir: &'a lang_hir::Hir,
    interner: &'a Interner,
) -> (lang_typeck::Checked<'a>, Vec<Diagnostic>) {
    let restricted = restricted(path);
    let sql = sql::wanted(hir);
    let contracts = contracts::wanted(hir);
    // proven contract checks belong to this program only (expression ids
    // are per program): reset before anything is lowered
    lang_mir::lower::set_elided(Default::default());
    if restricted || sql || contracts {
        lang_typeck::set_record_types(true);
    }
    let (checked, mut diags) = lang_typeck::check_program(hir, interner);
    if restricted || sql || contracts {
        lang_typeck::set_record_types(false);
        if let Some(t) = &checked.types {
            if restricted {
                diags.extend(authority::check(hir, t, interner));
            }
            let clean = !diags.iter().any(|d| d.severity == lang_base::Severity::Error);
            if sql && clean {
                diags.extend(sql::check(hir, t, interner, database_schema(path).as_deref()));
            }
            if contracts && clean {
                let (found, elided) = contracts::check(hir, t, interner);
                diags.extend(found);
                if std::env::var_os("LANG_NO_CONTRACT_ELISION").is_none() {
                    lang_mir::lower::set_elided(elided);
                }
            }
        }
    }
    (checked, diags)
}
