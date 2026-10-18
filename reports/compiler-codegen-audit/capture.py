#!/usr/bin/env python3
"""Capture codegen evidence without editing the compiler under review.

Usage: python3 reports/compiler-codegen-audit/capture.py --out /tmp/lang-codegen-evidence
Requires the workspace's cached Cargo dependencies and Rust toolchain.
"""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[2]
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    workspace = tomllib.loads((repo / "Cargo.toml").read_text())
    codegen = tomllib.loads((repo / "crates/lang_codegen/Cargo.toml").read_text())
    env = {k: v for k, v in os.environ.items() if not k.startswith("LANG_")}
    with tempfile.TemporaryDirectory(prefix="lang-codegen-capture-") as temp:
        temp = Path(temp)
        dependencies = []
        for name in sorted(set(codegen["dependencies"]) | {"lang_driver"}):
            value = workspace["workspace"]["dependencies"][name]
            if isinstance(value, str):
                dependencies.append(f"{name} = {json.dumps(value)}")
            else:
                assert set(value) == {"path"}, (name, value)
                path = str(repo / value["path"])
                dependencies.append(f"{name} = {{ path = {json.dumps(path)} }}")
        manifest = '\n'.join([
            '[package]', 'name = "lang-codegen-capture"', 'version = "0.0.0"',
            'edition = "2024"', '[dependencies]', *dependencies,
            '[profile.release]', 'lto = "thin"', 'codegen-units = 1',
        ]) + '\n'
        (temp / "Cargo.toml").write_text(manifest)
        shutil.copyfile(repo / "Cargo.lock", temp / "Cargo.lock")
        original = (repo / "crates/lang_codegen/src/lib.rs").read_text()
        tracked = ["Cargo.lock", "crates/lang_mir/src/lower.rs", "crates/lang_mir/src/sra.rs",
                   "crates/lang_mir/src/layout.rs", "crates/lang_codegen/src/lib.rs",
                   "crates/lang_driver/src/lib.rs", "reports/compiler-codegen-audit/probes.lang"]
        source_hashes = {name: hashlib.sha256((repo / name).read_bytes()).hexdigest() for name in tracked}
        signature = '        jit.ctx.func.signature = func_sig(jit, f);'
        before = '''        if f.name.starts_with("probes.") {
            audit_append("before.clif", &format!(";; {}\\n{}\\n", f.name, jit.ctx.func.display()));
        }
'''
        after = '''        if f.name.starts_with("probes.") {
            audit_append("after.clif", &format!(";; {}\\n;; runtime imports: {:?}\\n{}\\n", f.name, import_refs, jit.ctx.func.display()));
            let code = jit.ctx.compiled_code().expect("compiled code");
            audit_append("machine.txt", &format!(";; {} ({} bytes)\\n{}\\n", f.name,
                code.code_buffer().len(), code.vcode.as_deref().expect("disassembly")));
        }
'''
        define = '        jit.module\n            .define_function(self.funcs[id], &mut jit.ctx)'
        clear = '        jit.module.clear_context(&mut jit.ctx);\n        let _ = self.call_conv;'
        assert original.count(signature) == original.count(define) == original.count(clear) == 1
        modified = original.replace(signature, signature + '\n        jit.ctx.set_disasm(true);')
        modified = modified.replace(define, before + define)
        modified = modified.replace(clear, after + clear)
        modified += '''
fn audit_append(name: &str, data: &str) {
    use std::io::Write;
    let path = std::path::PathBuf::from(std::env::var_os("CODEGEN_AUDIT_OUT").unwrap()).join(name);
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
    file.write_all(data.as_bytes()).unwrap();
}
'''
        (temp / "backend.rs").write_text(modified)
        (temp / "main.rs").write_text('''
#[allow(dead_code)]
#[path = "backend.rs"]
mod backend;
fn main() {
    let path = std::env::args().nth(1).expect("source path");
    let workers: usize = std::env::args().nth(2).expect("workers").parse().unwrap();
    let mut session = lang_driver::Session::new();
    let options = lang_driver::RunOptions { optimize: true, workers: Some(workers), ..Default::default() };
    let built = session.build(std::path::Path::new(&path), &options).expect("source build");
    let _captured = backend::compile(&built.prog, true).expect("instrumented compilation");
    std::fs::write(std::path::PathBuf::from(std::env::var_os("CODEGEN_AUDIT_OUT").unwrap()).join("program.mir"),
        lang_mir::mir::dump(&built.prog)).unwrap();
}
''')
        with (temp / "Cargo.toml").open("a") as file:
            file.write('\n[[bin]]\nname = "lang-codegen-capture"\npath = "main.rs"\n')
        command = ["cargo", "build", "--release", "--offline", "--manifest-path", str(temp / "Cargo.toml"),
                   "--target-dir", str(repo / "target")]
        subprocess.run(command, cwd=repo, env=env, check=True)
        for workers in [1, 4]:
            dest = out / f"workers-{workers}"
            dest.mkdir(exist_ok=True)
            for name in ["before.clif", "after.clif", "machine.txt", "program.mir"]:
                path = dest / name
                if path.exists():
                    raise FileExistsError(f"Use a fresh output directory: {path}")
            subprocess.run([str(repo / "target/release/lang-codegen-capture"),
                            str(repo / "reports/compiler-codegen-audit/probes.lang"), str(workers)],
                           cwd=repo, env={**env, "CODEGEN_AUDIT_OUT": str(dest)}, check=True)
        (out / "metadata.json").write_text(json.dumps({
            "captured_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(),
            "rustc": subprocess.check_output(["rustc", "--version"], cwd=repo, text=True).strip(),
            "git_status": subprocess.check_output(["git", "status", "--short"], cwd=repo, text=True),
            "backend_source_sha256": hashlib.sha256(original.encode()).hexdigest(),
            "source_sha256_at_start": source_hashes,
            "sources_unchanged_during_capture": all(
                hashlib.sha256((repo / name).read_bytes()).hexdigest() == source_hashes[name]
                for name in tracked
            ),
            "method": "Unchanged backend copied to a temporary crate; only before/after IR and machine-output capture added. No optimization changes.",
            "backend": "Cranelift 0.136.3; opt_level=speed; host features detected",
        }, indent=2) + '\n')


if __name__ == "__main__":
    main()
