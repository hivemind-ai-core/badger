//! `lang mcp`: the tools over the stdio protocol, including code mode with
//! `std.compiler`, output capture, and a runaway evaluation that is
//! cancelled without losing the session.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

#[test]
fn mcp_tools() {
    let dir = std::env::temp_dir().join(format!("lang-mcp-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("demo.lang");
    std::fs::write(
        &file,
        "import std.io\n\nfn double(x: Int) -> Int { x * 2 }\n\npub fn main() -> Unit {\n    let _ = io.println($\"{double(21)}\")\n}\n",
    )
    .unwrap();
    let path = file.display().to_string();
    let mut child = Command::new(env!("CARGO_BIN_EXE_lang"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut next = 0;
    let mut req = |method: &str, params: Value| -> Value {
        next += 1;
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": next, "method": method, "params": params }))
            .unwrap();
        stdin.flush().unwrap();
        loop {
            let mut line = String::new();
            stdout.read_line(&mut line).unwrap();
            let m: Value = serde_json::from_str(&line).unwrap();
            if m["id"] == json!(next) {
                return m;
            }
        }
    };
    let init = req("initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {} }));
    assert_eq!(init["result"]["capabilities"]["tools"]["listChanged"], json!(false));
    let tools = req("tools/list", json!({}));
    let names: Vec<&str> =
        tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["eval", "check", "query", "contracts", "run"]);
    let mut call = |name: &str, args: Value| -> (String, bool) {
        let r = req("tools/call", json!({ "name": name, "arguments": args }));
        (
            r["result"]["content"][0]["text"].as_str().unwrap().to_string(),
            r["result"]["isError"] == json!(true),
        )
    };

    // the compiler reports its contract decisions, and with tests the gaps
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/contracts/inventory.lang");
    let (out, err) = call("contracts", json!({ "path": root.display().to_string(), "tests": true }));
    assert!(
        !err && out.contains("2 proven") && out.contains("1 unproven clause(s) no test reaches"),
        "{out}"
    );

    // code mode: definitions accumulate; printed output is returned
    assert!(!call("eval", json!({ "code": "fn sq(x: Int) -> Int { x * x }" })).1);
    assert_eq!(call("eval", json!({ "code": "sq(12) + 1" })), ("145".to_string(), false));
    let (out, err) = call("eval", json!({ "code": "io.println(\"printed\")" }));
    assert!(!err && out.starts_with("printed\n"), "{out}");
    let (out, err) = call("eval", json!({ "code": "sq(\"no\")" }));
    assert!(err && out.contains("E0300"), "{out}");
    // a second impl of a protocol for another type keeps the first
    call("eval", json!({ "code": "struct A { v: Int }" }));
    call("eval", json!({ "code": "struct B { v: Int }" }));
    call("eval", json!({ "code": "impl Display for A { fn display(self: &Self) -> String { \"a\" } }" }));
    call("eval", json!({ "code": "impl Display for B { fn display(self: &Self) -> String { \"b\" } }" }));
    let (out, err) = call("eval", json!({ "code": "let a: A = { v: 1 }\nlet b: B = { v: 2 }\n$\"{a}{b}\"" }));
    assert!(!err && out.contains("ab"), "{out}");
    call("eval", json!({ "code": "import std.process" }));
    // the compiler from code
    call("eval", json!({ "code": "import std.compiler" }));
    call("eval", json!({ "code": "import std.map" }));
    let (out, err) = call(
        "eval",
        json!({ "code": format!("compiler.symbols(\"{path}\").map(fn(ss) {{ ss.map(fn(s) {{ $\"{{s.name}}: {{s.detail}}\" }}) }})") }),
    );
    assert!(!err && out.contains("double: fn(Int) -> Int"), "{out}");
    // a runaway evaluation is cancelled and the session keeps working
    let (out, err) = call("eval", json!({ "code": "loop { }", "timeout_secs": 0.5 }));
    assert!(err && out.contains("cancelled"), "{out}");
    assert_eq!(call("eval", json!({ "code": "sq(3)" })), ("9".to_string(), false));

    // values that look like errors are values; failures are failures
    assert_eq!(call("eval", json!({ "code": "[1, 2, 3]" })), ("[1, 2, 3]".to_string(), false));
    let (out, err) = call("eval", json!({ "code": "1 / 0" }));
    assert!(err && out.contains("division by zero"), "{out}");
    // the session outlives code that tries to end the process
    let (out, err) = call("eval", json!({ "code": "process.exit(3)" }));
    assert!(err, "{out}");
    assert_eq!(call("eval", json!({ "code": "sq(4)" })), ("16".to_string(), false));

    // check, query, run
    let (out, err) = call("check", json!({ "source": "pub fn main() -> Int { \"x\" }" }));
    assert!(err && out.contains("E0300"), "{out}");
    assert_eq!(call("check", json!({ "path": path })), ("no diagnostics: it checks".to_string(), false));
    // proposed edits checked in their project, nothing written
    let project = format!("{}/../../tests/projects/overlay", env!("CARGO_MANIFEST_DIR"));
    let files = json!({ "src/shapes.lang": "pub fn area(w: Int, h: Int, d: Int) -> Int { w * h * d }\n" });
    let (out, err) = call("check", json!({ "path": project, "files": files }));
    assert!(err && out.contains("E0309") && out.contains("\"proposed\":true"), "{out}");
    let (out, err) = call("check", json!({ "path": project, "files": {} }));
    assert!(!err && out.starts_with("no diagnostics: it checks\n"), "{out}");
    let (out, err) = call(
        "eval",
        json!({ "code": format!("compiler.check_project(\"{project}\", map.from_list([(\"src/shapes.lang\", \"pub fn area(w: Int) -> Int {{ w }}\")])).map(fn(c) {{ $\"{{c.ok}} {{c.diagnostics.len()}} {{c.roots.len()}}\" }})") }),
    );
    assert!(!err && out.contains("false 2 2"), "{out}");
    let (out, err) = call(
        "query",
        json!({ "kind": "rename", "path": format!("{project}/src/jobs.lang"), "line": 5, "col": 40, "name": "surface" }),
    );
    assert!(!err && out.matches("\"text\":\"surface\"").count() == 3, "{out}");
    let (out, err) =
        call("eval", json!({ "code": "compiler.api(\"std.option\").map(fn(xs) { xs.len() > 0 })" }));
    assert!(!err && out.contains("true"), "{out}");
    let (out, err) = call(
        "eval",
        json!({ "code": format!("compiler.graph(\"calls\", \"{project}\").map(fn(g) {{ g.edges.len() }})") }),
    );
    assert!(!err && out.contains("2"), "{out}");
    let (out, err) = call(
        "eval",
        json!({ "code": format!("compiler.rename(\"{project}/src/shapes.lang\", 3, 8, \"surface\").map(fn(es) {{ es.len() }})") }),
    );
    assert!(!err && out.contains("3"), "{out}");
    let (out, _) = call("query", json!({ "kind": "hover", "path": path, "line": 6, "col": 30 }));
    assert_eq!(out, "{\"text\":\"double: fn(Int) -> Int\"}");
    let (out, _) = call("query", json!({ "kind": "module", "name": "list" }));
    assert!(out.contains("pub fn"), "{out}");
    let (out, err) = call("run", json!({ "path": path }));
    assert!(!err && out.contains("42"), "{out}");

    drop(stdin);
    assert!(child.wait().unwrap().success());
    let _ = std::fs::remove_dir_all(&dir);
}
