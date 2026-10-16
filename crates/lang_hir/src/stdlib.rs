//! Embedded standard-library sources. A program loads the core modules
//! and those it imports (directly or through other standard modules).

macro_rules! std_modules {
    ($($name:literal),* $(,)?) => {
        /// `(module name, source)` pairs, e.g. `("io", "...")` for `std.io`.
        pub const STD_MODULES: &[(&str, &str)] = &[
            $(($name, include_str!(concat!("../std/", $name, ".lang")))),*
        ];
    };
}

std_modules![
    "prelude",
    "io",
    "list",
    "seq",
    "text",
    "env",
    "fs",
    "map",
    "set",
    "option",
    "result",
    "task",
    "channel",
    "time",
    "records",
    "pipeline",
    "trace",
    "log",
    "codec",
    "config",
    "system",
    "sqlite",
    "http",
    "buffer",
    "worker",
    "inspect",
    "path",
    "process",
    "bytes",
    "random",
    "regex",
    "test",
    "stream",
    "route",
    "parse",
    "num",
    "ffi",
    "native",
    "broadcast",
    "array",
    "schema",
    "compiler",
    "fetch",
    "jobs"
];
