//! Dependencies, workspaces, and `badger.lock`.
//!
//! Dependencies (transitively) are packages named in `[dependencies]`: a
//! directory (`path`), or a git repository (`git`, at `rev`, optionally in
//! `subdir`). Each must satisfy its version requirement (a prefix of the
//! dependency's `[package] version`). A repository is cloned once into the
//! cache (`$BADGER_HOME`, else `$XDG_CACHE_HOME/badger`, else `~/.cache/badger`),
//! and each commit used is checked out into a directory of its own there.
//!
//! `badger.lock` records every dependency's version, source (a relative path,
//! or `git+URL?rev=REV@COMMIT` with `#subdir`), and a content hash of its manifest
//! and sources. A build verifies them and refuses changed or unlisted
//! dependencies until `badger update` rewrites the lockfile; git dependencies
//! build from their locked commit (so builds repeat, and need no network
//! once cached), and `badger update` resolves their `rev` again.
//!
//! The packages of a workspace (`[workspace] members`) share the lockfile
//! at the workspace root: one resolution of each dependency for all.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::manifest::Manifest;

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Path,
    /// A repository at a commit (resolved from `rev`), with the package in
    /// `subdir` of it.
    Git {
        url: String,
        rev: String,
        commit: String,
        subdir: String,
    },
}

#[derive(Debug, Clone)]
pub struct Dep {
    pub name: String,
    pub version: String,
    /// Package directory (holding `badger.toml`).
    pub dir: PathBuf,
    /// Directory of the entry module: the package's module root.
    pub src_root: PathBuf,
    pub entry: PathBuf,
    pub hash: String,
    pub source: Source,
}

const LOCK: &str = "badger.lock";

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// FNV-1a over the manifest and every `.bdg` file (sorted relative paths).
fn content_hash(dir: &Path) -> String {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            // a symlinked directory is not followed (a link back up would
            // never end)
            let link_dir = e.file_type().is_ok_and(|t| t.is_symlink()) && p.is_dir();
            if p.is_dir() {
                if link_dir || p.file_name().is_some_and(|n| n == ".git") {
                    continue;
                }
                walk(base, &p, out);
            } else if p.extension().is_some_and(|x| x == "bdg")
                || p.file_name().is_some_and(|n| n == "badger.toml")
            {
                let rel = p.strip_prefix(base).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                out.push((rel, std::fs::read(&p).unwrap_or_default()));
            }
        }
    }
    let mut files = vec![];
    walk(dir, dir, &mut files);
    files.sort();
    let mut bytes = vec![];
    for (name, content) in files {
        bytes.extend(name.bytes());
        bytes.push(0);
        bytes.extend(content);
        bytes.push(0);
    }
    format!("fnv1a64:{:016x}", fnv(&bytes))
}

/// Where cloned repositories and their checkouts live.
pub fn cache_dir() -> PathBuf {
    if let Some(h) = std::env::var_os("BADGER_HOME") {
        return PathBuf::from(h);
    }
    if let Some(x) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(x).join("badger");
    }
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join(".cache/badger")
}

fn git(args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        // never ask for credentials on a terminal; use the user's own git
        // configuration otherwise
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// A repository location as written, made absolute when it is a local path
/// (relative to the package that names it).
fn repository(url: &str, base: &Path) -> String {
    let remote = url.contains("://") || url.contains('@') && url.contains(':');
    if remote {
        return url.to_string();
    }
    // lexically, so the cache key stays the same if the directory goes away
    let mut out = PathBuf::new();
    let abs = std::path::absolute(base.join(url)).unwrap_or_else(|_| base.join(url));
    for c in abs.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out.display().to_string()
}

/// Check out `url` at `locked` (a commit), or at `rev` (resolved now,
/// fetching first when `refresh`): the checkout directory and the commit.
fn checkout(
    name: &str,
    url: &str,
    rev: Option<&str>,
    locked: Option<&str>,
    refresh: bool,
) -> Result<(PathBuf, String), String> {
    let fail = |e: String| format!("dependency `{name}` ({url}): {e}");
    let key = format!("{:016x}", fnv(url.as_bytes()));
    let repos = cache_dir().join("git/repos");
    std::fs::create_dir_all(&repos).map_err(|e| fail(format!("cannot create {}: {e}", repos.display())))?;
    let repo = repos.join(&key);
    let repo_s = repo.display().to_string();
    let mut fetched = false;
    if !repo.join("HEAD").exists() {
        let tmp = repos.join(format!("{key}.tmp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        git(&["clone", "--bare", "--quiet", "--", url, &tmp.display().to_string()]).map_err(fail)?;
        if std::fs::rename(&tmp, &repo).is_err() {
            let _ = std::fs::remove_dir_all(&tmp);
        }
        fetched = true;
    }
    let fetch = || {
        git(&[
            "-C",
            &repo_s,
            "fetch",
            "--quiet",
            "--force",
            "--tags",
            "--",
            url,
            "+refs/heads/*:refs/heads/*",
        ])
    };
    if refresh && !fetched {
        fetch().map_err(fail)?;
        fetched = true;
    }
    let want = locked.or(rev).unwrap_or("HEAD");
    let resolve = || git(&["-C", &repo_s, "rev-parse", "--verify", "--quiet", &format!("{want}^{{commit}}")]);
    let commit = match resolve() {
        Ok(c) if !c.is_empty() => c,
        _ if !fetched => {
            fetch().map_err(fail)?;
            resolve().map_err(|_| fail(format!("no revision `{want}`")))?
        }
        _ => return Err(fail(format!("no revision `{want}`"))),
    };
    let dir = cache_dir().join("git/checkouts").join(format!("{key}-{}", &commit[..commit.len().min(16)]));
    if !dir.join(".git").exists() {
        let tmp = dir.with_extension(format!("tmp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.parent().unwrap()).map_err(|e| fail(e.to_string()))?;
        let tmp_s = tmp.display().to_string();
        git(&["clone", "--quiet", "--no-checkout", "--", &repo_s, &tmp_s]).map_err(fail)?;
        git(&["-C", &tmp_s, "checkout", "--quiet", "--detach", &commit]).map_err(fail)?;
        if std::fs::rename(&tmp, &dir).is_err() {
            let _ = std::fs::remove_dir_all(&tmp);
        }
    }
    Ok((dir, commit))
}

/// Git sources recorded in a lockfile, by dependency name: the repository
/// and `rev` asked for, and the commit they resolved to.
fn locked_commits(lock: &str) -> HashMap<String, (String, String)> {
    lock.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            // `name version source hash`: the source (a path or URL) may
            // hold spaces, so it is what lies between the version and hash
            let (name, rest) = l.split_once(' ')?;
            let (_version, rest) = rest.split_once(' ')?;
            let (source, _hash) = rest.rsplit_once(' ')?;
            let (asked, commit) = source.strip_prefix("git+")?.rsplit_once('@')?;
            let commit = commit.split('#').next()?;
            Some((name.to_string(), (asked.to_string(), commit.to_string())))
        })
        .collect()
}

fn asked(url: &str, rev: &str) -> String {
    if rev.is_empty() { url.to_string() } else { format!("{url}?rev={rev}") }
}

/// A package being resolved: its directory, manifest, and the repository
/// checkout it lives in (for a git dependency).
struct Work {
    dir: PathBuf,
    manifest: Manifest,
    repo: Option<(String, String, String, PathBuf)>,
}

/// Resolve the dependency graph of the package at `project`. Git
/// dependencies use their `locked` commit when there is one.
fn resolve_with(
    project: &Path,
    m: &Manifest,
    locked: &HashMap<String, (String, String)>,
    refresh: bool,
) -> Result<Vec<Dep>, String> {
    let mut out: Vec<Dep> = vec![];
    let mut work: Vec<Work> = vec![Work { dir: project.to_path_buf(), manifest: m.clone(), repo: None }];
    while let Some(w) = work.pop() {
        for d in &w.manifest.dependencies {
            let (ddir, repo, source) = match &d.git {
                Some(url) => {
                    let url = repository(url, &w.dir);
                    let rev = d.rev.clone().unwrap_or_default();
                    // the locked commit, while the manifest asks for the same
                    let pinned =
                        locked.get(&d.name).filter(|(a, _)| *a == asked(&url, &rev)).map(|(_, c)| c.as_str());
                    let (co, commit) = checkout(&d.name, &url, d.rev.as_deref(), pinned, refresh)?;
                    let subdir = d.subdir.clone().unwrap_or_default();
                    let dir = co.join(&subdir);
                    let dir = dir.canonicalize().map_err(|e| {
                        format!("dependency `{}`: no `{subdir}` in {url} at {commit}: {e}", d.name)
                    })?;
                    let co = co.canonicalize().unwrap_or(co);
                    let repo = Some((url.clone(), rev.clone(), commit.clone(), co));
                    (dir, repo, Source::Git { url, rev, commit, subdir })
                }
                None => {
                    let ddir = w.dir.join(&d.path);
                    let ddir = ddir.canonicalize().map_err(|e| {
                        format!("dependency `{}`: cannot open {}: {e}", d.name, ddir.display())
                    })?;
                    match &w.repo {
                        // a path dependency of a git dependency: part of its repository
                        Some((url, rev, commit, root)) => {
                            let Ok(sub) = ddir.strip_prefix(root) else {
                                return Err(format!(
                                    "dependency `{}` of a git dependency leaves its repository ({url})",
                                    d.name
                                ));
                            };
                            let subdir = sub.to_string_lossy().replace('\\', "/");
                            let source = Source::Git {
                                url: url.clone(),
                                rev: rev.clone(),
                                commit: commit.clone(),
                                subdir,
                            };
                            (ddir, w.repo.clone(), source)
                        }
                        None => (ddir, None, Source::Path),
                    }
                }
            };
            let dm = Manifest::load(&ddir.join("badger.toml"))
                .map_err(|e| format!("dependency `{}`: {e}", d.name))?;
            if dm.name != d.name {
                return Err(format!(
                    "dependency `{}` at {} is the package `{}`",
                    d.name,
                    ddir.display(),
                    dm.name
                ));
            }
            if let Some(req) = &d.version
                && !(dm.version == *req || dm.version.starts_with(&format!("{req}.")))
            {
                return Err(format!("dependency `{}` requires version {req}, found {}", d.name, dm.version));
            }
            if let Some(prev) = out.iter().find(|x| x.name == d.name) {
                if prev.dir != ddir {
                    return Err(format!(
                        "dependency `{}` resolves to two packages: {} and {}",
                        d.name,
                        prev.dir.display(),
                        ddir.display()
                    ));
                }
                continue;
            }
            let entry = ddir.join(&dm.entry);
            let src_root = entry.parent().map(Path::to_path_buf).unwrap_or_else(|| ddir.clone());
            out.push(Dep {
                name: d.name.clone(),
                version: dm.version.clone(),
                hash: content_hash(&ddir),
                dir: ddir.clone(),
                src_root,
                entry,
                source,
            });
            work.push(Work { dir: ddir, manifest: dm, repo });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Resolve the dependency graph of the project at `project`.
pub fn resolve(project: &Path, m: &Manifest) -> Result<Vec<Dep>, String> {
    resolve_with(project, m, &HashMap::new(), false)
}

fn lock_line(base: &Path, d: &Dep) -> String {
    let source = match &d.source {
        Source::Path => pathdiff(&d.dir, base),
        Source::Git { url, rev, commit, subdir } if subdir.is_empty() => {
            format!("git+{}@{commit}", asked(url, rev))
        }
        Source::Git { url, rev, commit, subdir } => format!("git+{}@{commit}#{subdir}", asked(url, rev)),
    };
    format!("{} {} {} {}", d.name, d.version, source, d.hash)
}

fn lock_text(base: &Path, deps: &[Dep]) -> String {
    let mut s =
        String::from("# Generated by `badger update`: dependency versions, sources, and content hashes.\n");
    for d in deps {
        s.push_str(&lock_line(base, d));
        s.push('\n');
    }
    s
}

fn pathdiff(p: &Path, base: &Path) -> String {
    let base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let (pc, bc): (Vec<_>, Vec<_>) = (p.components().collect(), base.components().collect());
    let common = pc.iter().zip(&bc).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".into(); bc.len() - common];
    parts.extend(pc[common..].iter().map(|c| c.as_os_str().to_string_lossy().to_string()));
    if parts.is_empty() { ".".into() } else { parts.join("/") }
}

/// The packages of the workspace whose root is `root` (its manifest has
/// `[workspace]`): each member directory, a final `*` naming every package
/// directory below that path, and the root itself if it is a package.
pub fn workspace_packages(root: &Path, m: &Manifest) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = vec![];
    for pat in m.workspace.iter().flatten() {
        if let Some(prefix) = pat.strip_suffix("/*").or_else(|| (pat == "*").then_some("")) {
            let base = root.join(prefix);
            let mut found: Vec<PathBuf> = std::fs::read_dir(&base)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.join("badger.toml").is_file())
                .collect();
            found.sort();
            out.extend(found);
        } else {
            out.push(root.join(pat));
        }
    }
    if m.has_package {
        out.push(root.to_path_buf());
    }
    let mut seen = vec![];
    out.into_iter()
        .map(|p| p.canonicalize().unwrap_or(p))
        .filter(|p| {
            let new = !seen.contains(p);
            seen.push(p.clone());
            new
        })
        .collect()
}

/// The workspace `project` belongs to: its root and packages.
pub fn workspace_of(project: &Path) -> Option<(PathBuf, Vec<PathBuf>)> {
    let project = project.canonicalize().ok()?;
    let mut dir = Some(project.as_path());
    while let Some(d) = dir {
        if let Ok(m) = Manifest::load(&d.join("badger.toml"))
            && m.workspace.is_some()
        {
            let packages = workspace_packages(d, &m);
            if d == project || packages.contains(&project) {
                return Some((d.to_path_buf(), packages));
            }
        }
        dir = d.parent();
    }
    None
}

/// Resolve every package that shares `project`'s lockfile (its workspace,
/// or just itself): the lockfile's directory, the dependencies of
/// `project`, and of all of them.
fn resolve_shared(
    project: &Path,
    m: &Manifest,
    refresh: bool,
) -> Result<(PathBuf, Vec<Dep>, Vec<Dep>), String> {
    let (base, packages) = match workspace_of(project) {
        Some((root, packages)) => (root, packages),
        None => (project.to_path_buf(), vec![project.to_path_buf()]),
    };
    let locked = if refresh {
        HashMap::new()
    } else {
        std::fs::read_to_string(base.join(LOCK)).map(|t| locked_commits(&t)).unwrap_or_default()
    };
    let me = project.canonicalize().unwrap_or_else(|_| project.to_path_buf());
    let mut mine = vec![];
    let mut all: Vec<Dep> = vec![];
    for p in &packages {
        let pm = if *p == me { m.clone() } else { Manifest::load(&p.join("badger.toml"))? };
        if !pm.has_package {
            continue;
        }
        let deps = resolve_with(p, &pm, &locked, refresh)?;
        for d in &deps {
            match all.iter().find(|x| x.name == d.name) {
                Some(x) if x.dir != d.dir => {
                    return Err(format!(
                        "dependency `{}` resolves to two packages in this workspace: {} and {}",
                        d.name,
                        x.dir.display(),
                        d.dir.display()
                    ));
                }
                Some(_) => {}
                None => all.push(d.clone()),
            }
        }
        if *p == me {
            mine = deps;
        }
    }
    if !packages.contains(&me) && m.has_package {
        mine = resolve_with(project, m, &locked, refresh)?;
        all = mine.clone();
    }
    all.sort_by(|a, b| a.name.cmp(&b.name));
    Ok((base, mine, all))
}

/// Resolve and verify against `badger.lock` (writing it when absent).
pub fn verified(project: &Path, m: &Manifest) -> Result<Vec<Dep>, String> {
    let (base, mine, all) = resolve_shared(project, m, false)?;
    let lock = base.join(LOCK);
    let expected = lock_text(&base, &all);
    match std::fs::read_to_string(&lock) {
        // only a lockfile that is not there is written fresh (one that
        // cannot be read is never overwritten)
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("cannot read {}: {e}", lock.display()));
        }
        Err(_) => {
            if !all.is_empty() {
                std::fs::write(&lock, &expected)
                    .map_err(|e| format!("cannot write {}: {e}", lock.display()))?;
            }
        }
        Ok(found) => {
            let entries = |t: &str| -> Vec<String> {
                t.lines()
                    .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
                    .map(str::to_string)
                    .collect()
            };
            let (want, have) = (entries(&expected), entries(&found));
            for w in &want {
                let name = w.split(' ').next().unwrap_or("");
                match have.iter().find(|h| h.split(' ').next() == Some(name)) {
                    None => {
                        return Err(format!(
                            "dependency `{name}` is not in badger.lock; run `badger update`"
                        ));
                    }
                    Some(h) if h != w => {
                        return Err(format!(
                            "dependency `{name}` changed since badger.lock was written (locked `{h}`, found `{w}`); run `badger update`"
                        ));
                    }
                    _ => {}
                }
            }
            for h in &have {
                let name = h.split(' ').next().unwrap_or("");
                if !want.iter().any(|w| w.split(' ').next() == Some(name)) {
                    return Err(format!(
                        "badger.lock lists `{name}`, which is no longer a dependency; run `badger update`"
                    ));
                }
            }
        }
    }
    Ok(mine)
}

/// Rewrite `badger.lock` (of `project`'s workspace, if any) from the current
/// dependencies, resolving git revisions again.
pub fn update(project: &Path) -> Result<Vec<Dep>, String> {
    let m = Manifest::load(&project.join("badger.toml"))?;
    let (base, _, all) = resolve_shared(project, &m, true)?;
    let lock = base.join(LOCK);
    std::fs::write(&lock, lock_text(&base, &all))
        .map_err(|e| format!("cannot write {}: {e}", lock.display()))?;
    Ok(all)
}
