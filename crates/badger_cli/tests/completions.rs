//! The shell completion scripts: they complete every command and every
//! flag the argument parser holds, so they cannot drift from
//! `badger --help`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn lang() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_badger"));
    c.current_dir(root());
    c
}

/// The completion script for `shell`.
fn script(shell: &str) -> String {
    let out = lang().args(["completions", shell]).output().unwrap();
    assert!(out.status.success(), "badger completions {shell}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The commands `badger --help` lists.
fn commands() -> Vec<String> {
    let out = lang().arg("--help").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut out = vec![];
    let mut in_commands = false;
    for line in text.lines() {
        if line.starts_with("Commands:") {
            in_commands = true;
            continue;
        }
        if in_commands {
            if line.starts_with("Options:") || line.trim().is_empty() {
                break;
            }
            if let Some((name, _)) = line.trim().split_once("  ") {
                let name = name.trim();
                if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase()) {
                    out.push(name.to_string());
                }
            }
        }
    }
    assert!(out.len() > 20, "the commands of `badger --help`: {out:?}");
    out
}

/// The flags `badger COMMAND --help` lists.
fn flags(command: &str) -> Vec<String> {
    let out = lang().args([command, "--help"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut out = vec![];
    let mut in_options = false;
    for line in text.lines() {
        if line.starts_with("Options:") {
            in_options = true;
            continue;
        }
        if in_options {
            if line.trim().is_empty() {
                break;
            }
            // "  -o, --out <FILE>  ..."
            let head = line.split("  ").next().unwrap_or("").trim();
            for part in head.split(',') {
                let name = part.trim();
                if name.starts_with("--") {
                    let name = name.split(' ').next().unwrap_or(name);
                    out.push(name.to_string());
                }
            }
        }
    }
    out
}

/// Every command of the parser is completed, with its flags.
#[test]
fn every_command_and_flag_is_completed() {
    let scripts = ["bash", "zsh", "fish", "powershell"];
    let cmds = commands();
    for shell in scripts {
        let text = script(shell);
        for c in &cmds {
            assert!(text.contains(c), "{shell}: the command `{c}` is not completed");
            for f in flags(c) {
                assert!(text.contains(&f), "{shell}: the flag `{f}` of `{c}` is not completed");
            }
        }
    }
}

/// The scripts name the values of the flags that have a fixed set.
#[test]
fn the_fixed_values_are_completed() {
    let text = script("bash");
    for value in ["release", "aggressive", "superopt", "dot", "mermaid", "json"] {
        assert!(text.contains(value), "the value `{value}` is not completed");
    }
    let text = script("zsh");
    for value in ["release", "aggressive", "superopt", "dot", "mermaid", "json"] {
        assert!(text.contains(value), "the value `{value}` is not completed");
    }
}

/// The bash script is valid shell.
#[test]
fn the_bash_script_parses() {
    let text = script("bash");
    let mut child = match Command::new("bash")
        .arg("-n")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        // no bash here: nothing to check
        Err(_) => return,
    };
    {
        use std::io::Write;
        child.stdin.as_mut().unwrap().write_all(text.as_bytes()).unwrap();
    }
    let status = child.wait().unwrap();
    assert!(status.success(), "the bash script does not parse");
}

/// The zsh script is valid shell (a description with a `'` in it once
/// ended its quoted word early).
#[test]
fn the_zsh_script_parses() {
    let text = script("zsh");
    let mut child = match Command::new("zsh")
        .arg("-n")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        // no zsh here: nothing to check
        Err(_) => return,
    };
    {
        use std::io::Write;
        child.stdin.as_mut().unwrap().write_all(text.as_bytes()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "the zsh script does not parse: {}", String::from_utf8_lossy(&out.stderr));
    // a flag's fixed values are completed after it
    assert!(text.contains(":LEVEL:(release aggressive superopt)"), "{text}");
}

/// After a command, bash offers that command's flags: not `badger`'s own
/// `--version`, and `--help` once.
#[test]
fn bash_offers_a_commands_own_flags() {
    let text = script("bash");
    let line = text.lines().find(|l| l.trim_start().starts_with("check) flags=")).unwrap();
    assert!(!line.contains("--version"), "{line}");
    assert_eq!(line.matches("--help").count(), 1, "{line}");
}

/// The commands the parser holds are the ones the docs list.
#[test]
fn the_documented_commands_exist() {
    let doc = std::fs::read_to_string(root().join("docs/reference/cli.md")).unwrap();
    for c in commands() {
        assert!(
            doc.contains(&format!("`badger {c} ")) || doc.contains(&format!("`badger {c}`")),
            "the command `{c}` is not in docs/reference/cli.md"
        );
    }
}
