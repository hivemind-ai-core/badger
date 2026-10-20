//! The project templates `badger init --template=NAME` offers: the files
//! a project starts from.
//!
//! The files live in `templates/` at the repository root, one directory
//! per template, and are embedded in the binary (`include_str!`), so a
//! project can be created without the repository being there. Every file
//! may use the placeholder `@name@`, which is replaced with the package
//! name of the project.

/// The placeholder the template files use for the package name.
const NAME: &str = "@name@";

/// The files of the `cli` template: a program that parses its options.
const CLI: &[(&str, &str)] = &[
    ("badger.toml", include_str!("../../../templates/cli/badger.toml")),
    ("src/main.bdg", include_str!("../../../templates/cli/src/main.bdg")),
    ("README.md", include_str!("../../../templates/cli/README.md")),
];

/// The files of the `service` template: a service with routes.
const SERVICE: &[(&str, &str)] = &[
    ("badger.toml", include_str!("../../../templates/service/badger.toml")),
    ("src/main.bdg", include_str!("../../../templates/service/src/main.bdg")),
    ("src/service.bdg", include_str!("../../../templates/service/src/service.bdg")),
    ("src/routes.bdg", include_str!("../../../templates/service/src/routes.bdg")),
    ("README.md", include_str!("../../../templates/service/README.md")),
];

/// The files of the `framework` template: a store, a service, and routes.
const FRAMEWORK: &[(&str, &str)] = &[
    ("badger.toml", include_str!("../../../templates/framework/badger.toml")),
    ("src/main.bdg", include_str!("../../../templates/framework/src/main.bdg")),
    ("src/store.bdg", include_str!("../../../templates/framework/src/store.bdg")),
    ("src/service.bdg", include_str!("../../../templates/framework/src/service.bdg")),
    ("src/routes.bdg", include_str!("../../../templates/framework/src/routes.bdg")),
    ("README.md", include_str!("../../../templates/framework/README.md")),
];

/// The files of the `library` template: a library package.
const LIBRARY: &[(&str, &str)] = &[
    ("badger.toml", include_str!("../../../templates/library/badger.toml")),
    ("src/lib.bdg", include_str!("../../../templates/library/src/lib.bdg")),
    ("README.md", include_str!("../../../templates/library/README.md")),
];

/// The files of the `worker` template: a background worker.
const WORKER: &[(&str, &str)] = &[
    ("badger.toml", include_str!("../../../templates/worker/badger.toml")),
    ("src/main.bdg", include_str!("../../../templates/worker/src/main.bdg")),
    ("src/worker.bdg", include_str!("../../../templates/worker/src/worker.bdg")),
    ("README.md", include_str!("../../../templates/worker/README.md")),
];

/// The templates, in the order they are listed in the help.
const TEMPLATES: &[(&str, &[(&str, &str)])] = &[
    ("cli", CLI),
    ("service", SERVICE),
    ("framework", FRAMEWORK),
    ("library", LIBRARY),
    ("worker", WORKER),
];

/// The templates `badger init --template=NAME` offers, in the order they
/// are listed in the help.
pub fn names() -> Vec<&'static str> {
    TEMPLATES.iter().map(|(name, _)| *name).collect()
}

/// The files a project named `name` from `template` holds, as
/// (path relative to the project directory, contents). An unknown
/// template is an error naming the ones that exist.
pub fn plan(name: &str, template: &str) -> Result<Vec<(String, String)>, String> {
    let files = TEMPLATES
        .iter()
        .find(|(t, _)| *t == template)
        .map(|(_, files)| *files)
        .ok_or_else(|| {
            format!("unknown template `{template}` (available: {})", names().join(", "))
        })?;
    Ok(files
        .iter()
        .map(|(path, text)| (path.replace(NAME, name), text.replace(NAME, name)))
        .collect())
}
