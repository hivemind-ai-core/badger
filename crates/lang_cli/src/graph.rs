//! `lang graph KIND PATH`: program structure as a graph, in DOT (Graphviz),
//! Mermaid, or JSON. Kinds: `calls` (function calls), `modules` (imports),
//! `types` (types used by struct fields and enum variants), `systems`
//! (system plan nodes and the nodes they reference).

use std::path::Path;

use serde_json::json;

pub fn graph(kind: &str, path: &Path, format: &str, with_std: bool) -> Result<String, String> {
    let edges: Vec<(String, String)> = match kind {
        "systems" => {
            let mut s = lang_driver::Session::new();
            let plans = s.inspect_plans(path).ok_or("the program does not parse")?;
            let mut out = vec![];
            for (i, p) in plans.iter().enumerate() {
                if p.kind != "system" {
                    continue;
                }
                for n in &p.nodes {
                    let node = format!("system{}:{}", i + 1, n.key);
                    if n.refs.is_empty() {
                        out.push((node.clone(), node));
                    }
                    for r in &n.refs {
                        out.push((format!("system{}:{}", i + 1, n.key), format!("system{}:{r}", i + 1)));
                    }
                }
            }
            out
        }
        "calls" | "modules" | "types" => {
            let a = lang_driver::analysis::analyze(path, Default::default());
            if a.files.iter().all(|f| f.refs.is_empty()) && !a.diagnostics.is_empty() {
                return Err("the program does not check (see `lang check`)".into());
            }
            let g = match kind {
                "calls" => a.graphs.calls,
                "modules" => a.graphs.modules,
                _ => a.graphs.types,
            };
            g.into_iter().filter(|e| with_std || !e.2).map(|(f, t, _)| (f, t)).collect()
        }
        other => return Err(format!("unknown graph `{other}` (calls, modules, types, systems)")),
    };
    let mut nodes: Vec<&str> = edges.iter().flat_map(|(a, b)| [a.as_str(), b.as_str()]).collect();
    nodes.sort();
    nodes.dedup();
    Ok(match format {
        "json" => json!({
            "nodes": nodes,
            "edges": edges.iter().filter(|e| e.0 != e.1).map(|(a, b)| json!({ "from": a, "to": b })).collect::<Vec<_>>(),
        })
        .to_string(),
        "mermaid" => {
            let id = |n: &str| nodes.iter().position(|x| *x == n).unwrap_or(0);
            let mut s = String::from("graph LR\n");
            for (i, n) in nodes.iter().enumerate() {
                s.push_str(&format!("  n{i}[\"{}\"]\n", n.replace('"', "'")));
            }
            for (a, b) in edges.iter().filter(|e| e.0 != e.1) {
                s.push_str(&format!("  n{} --> n{}\n", id(a), id(b)));
            }
            s
        }
        "dot" => {
            let mut s = format!("digraph {kind} {{\n  rankdir=LR;\n  node [shape=box, fontname=\"monospace\"];\n");
            for n in &nodes {
                s.push_str(&format!("  \"{n}\";\n"));
            }
            for (a, b) in edges.iter().filter(|e| e.0 != e.1) {
                s.push_str(&format!("  \"{a}\" -> \"{b}\";\n"));
            }
            s.push_str("}\n");
            s
        }
        other => return Err(format!("unknown format `{other}` (dot, mermaid, json)")),
    })
}
