//! Reusing the checked standard-library prefix across a project's entries.
//!
//! A project checks its main entry and every `[entries]`/`[tasks]` selection
//! in one session. When they reach the same standard modules the standard
//! prefix is checked once and reused; when the module sets differ it must be
//! rechecked, and per-program effect propagation must still run.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn badger() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_badger"));
    c.current_dir(root());
    c
}

/// The line after the last `typecheck.std-cache:` marker (the cumulative
/// hit/miss counter for the last check).
fn counter_line(stderr: &str) -> &str {
    stderr.rsplit_once("typecheck.std-cache:").expect("no cache counter").1
}

/// Four entries with the same standard module set reuse the checked prefix:
/// the first check misses, the rest hit.
#[test]
fn entries_reuse_the_checked_prefix() {
    let out =
        badger().env("BADGER_TIMINGS", "1").args(["check", "tests/projects/stdreuse"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(counter_line(&err).starts_with(" hit (hits 4, misses 1)"), "{}", counter_line(&err));
}

/// A user `Display` implementation whose body performs `Io` must still
/// widen the standard `Display::display` (declared unannotated) when its
/// entry is checked from the cached prefix: the `!{}` caller is rejected.
#[test]
fn user_impl_effect_still_widens_a_cached_protocol_method() {
    let out = badger().args(["check", "tests/projects/effectcache"]).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("E0501"), "effect widening lost:\n{err}");
    assert!(err.contains("pure_show"), "annotated caller not reported:\n{err}");
}

/// An entry reaching a different standard module set must miss the cache.
#[test]
fn differing_std_sets_miss() {
    let out = badger().env("BADGER_TIMINGS", "1").args(["check", "tests/projects/stdmix"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(counter_line(&err).starts_with(" miss (hits 0, misses 2)"), "{}", counter_line(&err));
}
