//! `badger doc`: reference documentation in Markdown, generated from checked
//! interfaces (the `api` query): each type with its methods, each
//! function with its declaration and `///` documentation, and a module's
//! leading comment as its introduction. Generated pages name the toolchain
//! that produced them.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::Value;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Write the reference for `target` as one HTML page and open it: the
/// Markdown the generator produces is rendered, and the page's links
/// point at the sections of the same page. The page is written into
/// `out` when a directory is given, and into a private directory
/// otherwise.
pub fn open(target: &str, out: Option<&Path>) -> Result<String, String> {
    let pages = pages(target)?;
    let dir = match out {
        Some(d) => d.to_path_buf(),
        None => badger_driver::private_temp_dir("badger-doc")
            .map_err(|e| format!("cannot create a documentation directory: {e}"))?,
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = dir.join("index.html");
    std::fs::write(&file, html(&pages)).map_err(|e| format!("{}: {e}", file.display()))?;
    open_in_viewer(&file);
    Ok(file.display().to_string())
}

/// The pages as one HTML document: every page is a section, and the
/// links between the pages point at the sections.
fn html(pages: &[(String, String)]) -> String {
    let mut body = String::new();
    for (name, text) in pages {
        let anchor = name.strip_suffix(".md").unwrap_or(name);
        let mut text = text.clone();
        for (other, _) in pages {
            let other = other.strip_suffix(".md").unwrap_or(other);
            text = text.replace(&format!("]({other}.md)"), &format!("](#{other})"));
        }
        body.push_str(&format!("<section id=\"{anchor}\">\n"));
        body.push_str(&render(&text));
        body.push_str("\n</section>\n");
    }
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>Badger reference</title>\n\
         <style>\nbody {{ font: 15px/1.5 system-ui, sans-serif; margin: 2rem auto; max-width: 52rem; padding: 0 1rem; }}\n\
         code, pre {{ font-family: ui-monospace, monospace; }}\n\
         pre {{ background: #f4f4f5; padding: .75rem; overflow-x: auto; border-radius: 4px; }}\n\
         table {{ border-collapse: collapse; }}\n\
         td, th {{ border: 1px solid #d4d4d8; padding: .25rem .5rem; text-align: left; }}\n\
         h1, h2, h3 {{ line-height: 1.25; }}\n\
         </style>\n</head>\n<body>\n{body}</body>\n</html>\n"
    )
}

/// Markdown as HTML.
fn render(text: &str) -> String {
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    let parser = pulldown_cmark::Parser::new_ext(text, options);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);
    html
}

/// Open a file in the system's viewer (a browser, or whatever handles
/// the file). A failure is not an error: the file is written either way.
fn open_in_viewer(file: &Path) {
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = std::process::Command::new("cmd");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut cmd = std::process::Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    {
        cmd.arg("/C").arg("start").arg("");
    }
    cmd.arg(file);
    let _ = cmd.spawn();
}

/// Generate the reference for `target` (`std`, a standard module such as
/// `std.list`, or a program path): written to `out` as one page per module
/// plus an index, or printed. With `check`, compare with `out` instead and
/// fail on any difference.
pub fn run(target: &str, out: Option<&Path>, check: bool) -> Result<(), String> {
    let pages = pages(target)?;
    let Some(dir) = out else {
        if check {
            return Err("`--check` needs `--out DIR`".into());
        }
        for (_, text) in &pages {
            print!("{text}");
        }
        return Ok(());
    };
    let mut stale = vec![];
    for (name, text) in &pages {
        let p = dir.join(name);
        if check {
            if std::fs::read_to_string(&p).ok().as_deref() != Some(text.as_str()) {
                stale.push(p.display().to_string());
            }
        } else {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
        }
    }
    if check {
        // pages for modules that no longer exist
        let wanted: Vec<PathBuf> = pages.iter().map(|(n, _)| dir.join(n)).collect();
        for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "md") && !wanted.contains(&p) {
                stale.push(format!("{} (no such module)", p.display()));
            }
        }
        if !stale.is_empty() {
            stale.sort();
            return Err(format!(
                "reference out of date (regenerate with `badger doc {target} --out {}`):\n  {}",
                dir.display(),
                stale.join("\n  ")
            ));
        }
    }
    Ok(())
}

/// File name and Markdown of each page.
fn pages(target: &str) -> Result<Vec<(String, String)>, String> {
    // a namespace, or the whole library: `std` covers both `std` and `stdx`
    if badger_stdlib::is_root(target) {
        let roots: Vec<&str> =
            if target == badger_stdlib::STD { badger_stdlib::ROOTS.to_vec() } else { vec![target] };
        let mut out = vec![];
        // (namespace, module, the module's first line)
        let mut index: Vec<(&str, String, String)> = vec![];
        for (root, m) in badger_stdlib::names().filter(|(r, _)| roots.contains(r)) {
            let qualified = format!("{root}.{m}");
            let items = api(&qualified)?;
            if items.is_empty() {
                continue;
            }
            let intro = badger_stdlib::source(root, m).map(intro).unwrap_or_default();
            index.push((root, m.to_string(), intro.lines().next().unwrap_or("").to_string()));
            out.push((format!("{m}.md"), page(&qualified, &intro, &items, target)));
        }
        let mut t = header("Standard library reference", target);
        let blurb = |root: &str| match root {
            badger_stdlib::STDX => "The extended library: the application framework and the integrations.",
            _ => {
                "The language's foundation: the modules the compiler needs, the data structures, text, \
                  number, time, and concurrency facilities, and the plain OS and process access."
            }
        };
        for root in &roots {
            let section: Vec<_> = index.iter().filter(|(r, _, _)| r == root).collect();
            if section.is_empty() {
                continue;
            }
            let _ = writeln!(t, "## `{root}`\n\n{}\n", blurb(root));
            t.push_str("| Module | |\n|---|---|\n");
            for (_, m, first) in section {
                let _ = writeln!(t, "| [`{root}.{m}`]({m}.md) | {} |", first.replace('|', "\\|"));
            }
            t.push('\n');
        }
        return Ok(out.into_iter().chain(std::iter::once(("README.md".into(), t))).collect());
    }
    // one standard module, `std.list` or `stdx.http`
    if let Some((root, m)) = badger_stdlib::split(target) {
        if !badger_stdlib::has(root, m) {
            return Err(format!("no standard module `{target}`"));
        }
        let items = api(target)?;
        let intro = badger_stdlib::source(root, m).map(intro).unwrap_or_default();
        return Ok(vec![(format!("{m}.md"), page(target, &intro, &items, target))]);
    }
    // a program: one page per module file
    let items = api(target)?;
    let mut by_module: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for i in items {
        by_module.entry(i["module"].as_str().unwrap_or("").to_string()).or_default().push(i);
    }
    let root = files_root(by_module.keys());
    Ok(by_module
        .into_iter()
        .map(|(file, items)| {
            let name = module_name(&file, &root);
            let intro = std::fs::read_to_string(&file).map(|s| intro(&s)).unwrap_or_default();
            (format!("{name}.md"), page(&name, &intro, &items, target))
        })
        .collect())
}

/// The directory holding all of a program's files: module names are
/// relative to it (two `util.bdg` in different directories differ).
pub(crate) fn files_root(files: impl IntoIterator<Item = impl AsRef<str>>) -> PathBuf {
    let dirs: Vec<PathBuf> =
        files.into_iter().filter_map(|f| Path::new(f.as_ref()).parent().map(Path::to_path_buf)).collect();
    let mut root = dirs.first().cloned().unwrap_or_default();
    while !dirs.iter().all(|d| d.starts_with(&root)) && root.pop() {}
    root
}

/// A program file's module name, by its path below `root`:
/// `ROOT/a/util.bdg` -> `a.util`.
pub(crate) fn module_name(file: &str, root: &Path) -> String {
    let rel = Path::new(file).strip_prefix(root).unwrap_or(Path::new(file)).with_extension("");
    rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join(".")
}

fn api(target: &str) -> Result<Vec<Value>, String> {
    let v = crate::lsp::api(Path::new(target))?;
    Ok(v.as_array().cloned().unwrap_or_default())
}

fn header(title: &str, target: &str) -> String {
    format!(
        "# {title}\n\n<!-- Generated by `badger doc {target}` from the checked interfaces (badger {VERSION}). Do not edit by hand. -->\n\n"
    )
}

/// A source file's leading `//` comment, as prose.
fn intro(src: &str) -> String {
    let mut out = vec![];
    for line in src.lines() {
        let t = line.trim_start();
        let Some(rest) = t.strip_prefix("//") else { break };
        if rest.starts_with('/') {
            break;
        }
        out.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
    }
    let text = out.join("\n").trim().to_string();
    if text.is_empty() { text } else { text + "\n" }
}

fn declaration(i: &Value) -> String {
    let d = i["declaration"].as_str().unwrap_or("");
    if d.is_empty() { i["signature"].as_str().unwrap_or("").to_string() } else { d.to_string() }
}

fn entry(t: &mut String, level: &str, name: &str, i: &Value) {
    let _ = writeln!(t, "{level} `{name}`\n\n```badger\n{}\n```\n", declaration(i));
    let doc = i["doc"].as_str().unwrap_or("").trim();
    if !doc.is_empty() {
        let _ = writeln!(t, "{doc}\n");
    }
}

fn page(module: &str, intro: &str, items: &[Value], target: &str) -> String {
    let mut t = header(module, target);
    if !intro.is_empty() {
        t.push_str(intro);
        t.push('\n');
    }
    let kind = |i: &Value| i["kind"].as_str().unwrap_or("").to_string();
    let name = |i: &Value| i["name"].as_str().unwrap_or("").to_string();
    let types: Vec<&Value> = items
        .iter()
        .filter(|i| matches!(kind(i).as_str(), "struct" | "enum" | "protocol" | "type"))
        .collect();
    let fns: Vec<&Value> =
        items.iter().filter(|i| matches!(kind(i).as_str(), "function" | "value")).collect();
    let mut methods: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for i in items.iter().filter(|i| kind(i) == "method") {
        let n = name(i);
        let ty = n.split('.').next().unwrap_or("").to_string();
        methods.entry(ty).or_default().push(i);
    }
    if !types.is_empty() {
        t.push_str("## Types\n\n");
        for i in &types {
            let n = name(i);
            entry(&mut t, "###", &n, i);
            for m in methods.remove(&n).unwrap_or_default() {
                let mn = name(m);
                entry(&mut t, "####", &mn, m);
            }
        }
    }
    if !fns.is_empty() {
        t.push_str("## Functions\n\n");
        for i in &fns {
            entry(&mut t, "###", &name(i), i);
        }
    }
    let attrs: Vec<&Value> = items.iter().filter(|i| kind(i) == "attribute").collect();
    if !attrs.is_empty() {
        // used as `@http.status`: by the module's last name
        let file = module.rsplit('/').next().unwrap_or(module);
        let last = file.trim_end_matches(".bdg").rsplit('.').next().unwrap_or(file);
        t.push_str("## Attributes\n\n");
        for i in &attrs {
            entry(&mut t, "###", &format!("@{last}.{}", name(i)), i);
        }
    }
    if !methods.is_empty() {
        t.push_str("## Methods on other types\n\n");
        for ms in methods.values() {
            for m in ms {
                entry(&mut t, "###", &name(m), m);
            }
        }
    }
    t
}
