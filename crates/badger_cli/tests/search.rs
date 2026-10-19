//! `badger search`: finding a definition in the standard library and in
//! the project's modules.

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

/// A definition of the standard library is found by its name.
#[test]
fn finds_a_standard_definition() {
    let out = lang().args(["search", "bimap"]).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.contains("std.result.bimap"), "{text}");
}

/// A definition is found by a word of its documentation too.
#[test]
fn finds_a_definition_by_its_documentation() {
    let out = lang().args(["search", "percent-decoded"]).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.contains("std.route.query_params"), "{text}");
}

/// A query that matches nothing says so, and still succeeds.
#[test]
fn no_match() {
    let out = lang().args(["search", "no-such-definition-anywhere"]).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.is_empty(), "{text}");
}

/// The matches are JSON as well.
#[test]
fn json_matches() {
    let out = lang().args(["search", "bimap", "--json"]).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let v: serde_json::Value = serde_json::from_str(&text).expect("json");
    let items = v.as_array().expect("an array");
    assert!(!items.is_empty());
    assert!(items[0]["name"].as_str().unwrap().contains("bimap"));
}
