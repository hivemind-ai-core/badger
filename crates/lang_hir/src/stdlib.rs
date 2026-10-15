//! Embedded standard-library sources. Every module is always loaded; `import`
//! only binds a name for it.

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
    "broadcast",
    "array",
    "schema"
];
