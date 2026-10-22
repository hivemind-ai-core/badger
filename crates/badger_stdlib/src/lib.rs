//! The Badger standard library: its sources, embedded in the compiler.
//!
//! Every module is an ordinary Badger file, in one of two namespaces:
//!
//! - `std` (`std/*.bdg`) is the language's foundation: the modules the
//!   compiler itself needs (the prelude, the collections, the items `system
//!   { }` and `pipeline { }` elaborate into), the pure data structures and
//!   text, number, time, and concurrency facilities, and the plain OS and
//!   process access every program uses.
//! - `stdx` (`stdx/*.bdg`) is the extended library: the application framework
//!   (operations, endpoints, routing, decisions, admission, jobs, workers),
//!   the integrations that need a substrate (HTTP, databases, outbound
//!   requests, database clients, telemetry export, formats, templates,
//!   crypto, compression, UUIDs), and the language's own tooling.
//!
//! A module lives in exactly one namespace (its last path segment names it),
//! so `import std.list` and `import stdx.http` name different files and no
//! module name is ambiguous. A program loads the core modules it needs
//! (`badger_driver`), plus the ones it imports, closed over the standard
//! modules' own imports.
//!
//! A new module needs a file in the namespace's directory, an entry in
//! [`STD_MODULES`], and nothing else: `tests/parity.rs` fails when the two
//! disagree.

/// The foundation namespace.
pub const STD: &str = "std";

/// The extended namespace.
pub const STDX: &str = "stdx";

/// Both namespaces, `std` first, in the order their modules load.
pub const ROOTS: &[&str] = &[STD, STDX];

macro_rules! std_modules {
    ($($root:literal { $($name:literal),* $(,)? }),* $(,)?) => {
        pub const STD_MODULES: &[(&str, &str, &str)] = &[
            $( $( ( $root, $name, include_str!(concat!("../", $root, "/", $name, ".bdg")) ) ),* ),*
        ];
    };
}

std_modules! {
    // The foundation: the modules the compiler needs first, then the data
    // structures, text and number facilities, and runtime primitives.
    "std" {
        "prelude",
        "io",
        "list",
        "seq",
        "text",
        "map",
        "set",
        "option",
        "result",
        "num",
        "bytes",
        "buffer",
        "array",
        "records",
        "codec",
        "heap",
        "deque",
        "sorted",
        "lru",
        "bits",
        "kv",
        "default",
        "path",
        "time",
        "datetime",
        "fs",
        "process",
        "env",
        "cli",
        "inspect",
        "ffi",
        "native",
        "random",
        "regex",
        "parse",
        "test",
        "validate",
        "schema",
        "task",
        "channel",
        "broadcast",
        "stream",
        "system",
        "pipeline",
        "trace",
        "log",
        "metrics",
        "worker",
    },
    // The extended library: the application framework, then the integrations.
    "stdx" {
        "app",
        "endpoint",
        "route",
        "command",
        "decide",
        "admission",
        "jobs",
        "http",
        "websocket",
        "fetch",
        "sqlite",
        "postgres",
        "valkey",
        "otel",
        "config",
        "template",
        "markdown",
        "toml",
        "yaml",
        "csv",
        "compress",
        "crypto",
        "encoding",
        "uuid",
        "compiler",
    },
}

/// Is `name` a standard-library namespace (`std` or `stdx`)?
pub fn is_root(name: &str) -> bool {
    name == STD || name == STDX
}

/// The source of the module `root.name`, if it is a standard module.
pub fn source(root: &str, name: &str) -> Option<&'static str> {
    STD_MODULES.iter().find(|(r, n, _)| *r == root && *n == name).map(|(_, _, s)| *s)
}

/// Is `name` a standard module of the namespace `root`?
pub fn has(root: &str, name: &str) -> bool {
    STD_MODULES.iter().any(|(r, n, _)| *r == root && *n == name)
}

/// Every standard module, as `(namespace, name)` pairs, in load order.
pub fn names() -> impl Iterator<Item = (&'static str, &'static str)> {
    STD_MODULES.iter().map(|(r, n, _)| (*r, *n))
}

/// A qualified name's namespace and module name: `("stdx", "http")` for
/// `"stdx.http"`, and `None` when the first segment is not a namespace.
pub fn split(qualified: &str) -> Option<(&'static str, &str)> {
    let (root, name) = qualified.split_once('.')?;
    let root = match root {
        STD => STD,
        STDX => STDX,
        _ => return None,
    };
    Some((root, name))
}
