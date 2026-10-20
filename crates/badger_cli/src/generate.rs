//! `badger generate feature NAME`: a typed feature module (its types, an
//! in-memory store, operations, an HTTP binding, and tests) from templates.
//! The generated files are checked and their tests run in a copy of the
//! project before anything is written, and existing files are never
//! replaced; the plan is printed, and nothing else in the project changes
//! (wiring the feature into an entry is left to the author).

use std::path::{Path, PathBuf};

const MODEL: &str = r#"// The @name@ feature's vocabulary: a @item@, what can go wrong, the storage
// it needs, and the operations it offers.
import std.app

pub type @Item@ = { id: Int, title: String }
pub type New@Item@ = { title: String }

@derive(Error)
pub enum Error {
    invalid(message: String),
    not_found(id: Int),
    storage(message: String),
}

pub type Store = {
    insert: fn(New@Item@) -> Result<@Item@, Error> !{Io, Wait, State},
    find: fn(Int) -> Result<Option<@Item@>, Error> !{Io, Wait, State},
    all: fn() -> Result<List<@Item@>, Error> !{Io, Wait, State},
}

pub type Operations = {
    create: app.Operation<New@Item@, @Item@, Error, !{Io, Wait, State}>,
    find: app.Operation<Int, @Item@, Error, !{Io, Wait, State}>,
    list: app.Operation<{}, List<@Item@>, Error, !{Io, Wait, State}>,
}
"#;

const MEMORY: &str = r#"// An in-memory store for @name@ (tests and development).
import std.list
import @name@.model

pub provider store(config: {}) -> Result<model.Store, model.Error> {
    let rows: Atom<List<model.@Item@>> = Atom.new([])
    ok({
        insert: fn(new: model.New@Item@) -> Result<model.@Item@, model.Error> {
            let all = rows.update(fn(current) { current.append({ id: current.len() + 1, title: new.title }) })
            ok(all[all.len() - 1])
        },
        find: fn(id: Int) -> Result<Option<model.@Item@>, model.Error> { ok(list.find(rows.get(), fn(n) { n.id == id })) },
        all: fn() -> Result<List<model.@Item@>, model.Error> { ok(rows.get()) },
    })
}
"#;

const SERVICE: &str = r#"// The @name@ feature: its operations over a store.
import std.app
import std.system
import std.text
import @name@.memory
import @name@.model

pub provider @name@(config: { store: model.Store }) -> Result<model.Operations, model.Error> {
    let store = config.store
    ok({
        create: app.from_fn({ id: #@name@/create, version: 1, doc: "Create a @item@." }, fn(_call, input: model.New@Item@) -> Result<model.@Item@, model.Error> {
            let title = text.trim(input.title)
            if title == "" {
                return err(model.Error.invalid("a @item@ needs a title"))
            }
            store.insert({ title })
        }),
        find: app.from_fn({ id: #@name@/find, version: 1, doc: "Find a @item@ by id." }, fn(_call, id: Int) -> Result<model.@Item@, model.Error> {
            match store.find(id)? {
                .some(n) => ok(n),
                .none => err(model.Error.not_found(id)),
            }
        }),
        list: app.from_fn({ id: #@name@/list, version: 1, doc: "Every @item@." }, fn(_call, _input: {}) -> Result<List<model.@Item@>, model.Error> {
            store.all()
        }),
    })
}

@derive(Error, From)
pub enum TestError {
    @name@(source: model.Error),
    system(source: system.Error),
}

fn test_create_and_find() -> Result<Unit, TestError> {
    let plan = system {
        store: memory.store(),
        @name@: @name@({ store: ref(#store) }),
    }
    use running <- system.run(plan)
    let call = app.call()
    let created = running.@name@.create.call(call, { title: "  first " })?
    assert(created.title == "first")
    assert(running.@name@.find.call(call, created.id)? == created)
    assert(running.@name@.list.call(call, {})?.len() == 1)
    ok(())
}

fn test_invalid_and_missing() -> Result<Unit, TestError> {
    let plan = system {
        store: memory.store(),
        @name@: @name@({ store: ref(#store) }),
    }
    use running <- system.run(plan)
    let call = app.call()
    match running.@name@.create.call(call, { title: " " }) {
        .err(.invalid(_)) => (),
        _ => panic("expected invalid"),
    }
    match running.@name@.find.call(call, 9) {
        .err(.not_found(9)) => (),
        _ => panic("expected not_found"),
    }
    ok(())
}
"#;

const HTTP: &str = r#"// The HTTP binding of the @name@ feature.
import std.endpoint
import std.http
import std.route
import std.system
import @name@.memory
import @name@.model
import @name@.service

fn failure(e: model.Error) -> endpoint.Failure {
    match e {
        .invalid(message) => endpoint.failure(400, #@name@/invalid, message),
        .not_found(id) => endpoint.failure(404, #@name@/not_found, $"no @item@ {id}"),
        .storage(_) => endpoint.failure(503, #@name@/unavailable, "@name@ are unavailable"),
    }
}

pub fn routes(@name@: model.Operations) -> List<route.Route> {
    [
        endpoint.bind({ method: #post, path: "/@name@", operation: @name@.create, input: fn(_params: {}, body: model.New@Item@) { body }, status: 201, errors: failure }),
        endpoint.bind({ method: #get, path: "/@name@/:id", operation: @name@.find, input: fn(params: { id: Int }, _body: {}) { params.id }, status: 200, errors: failure }),
        endpoint.bind({ method: #get, path: "/@name@", operation: @name@.list, input: fn(_params: {}, _body: {}) { {} }, status: 200, errors: failure }),
    ]
}

@derive(Error, From)
pub enum TestError {
    @name@(source: model.Error),
    route(source: route.RouteError),
    system(source: system.Error),
}

fn test_routes() -> Result<Unit, TestError> {
    let plan = system {
        store: memory.store(),
        @name@: service.@name@({ store: ref(#store) }),
    }
    use running <- system.run(plan)
    let api = route.table(routes(running.@name@))?
    let created = http.call(api, http.request(#post, "/@name@", "{\"title\": \"first\"}"))
    assert(created.status == 201)
    assert(http.call(api, http.request(#get, "/@name@/1", "")).status == 200)
    assert(http.call(api, http.request(#get, "/@name@/9", "")).status == 404)
    assert(http.call(api, http.request(#post, "/@name@", "{\"title\": \"\"}")).status == 400)
    ok(())
}
"#;

const WORKER: &str = r#"// The @name@ worker: durable jobs of kind "@name@" (std.jobs), each run by
// a supervised worker that retries a failed attempt and survives restarts.
// `Job` and `step` are placeholders for the job's own state and work.
import std.jobs
import std.system
import std.task
import std.time
import std.worker

/// A job's state (what the work needs; stored as JSON).
pub type Job = { id: Int }

@derive(Error)
pub enum Failure {
    failed(message: String),
}

/// One step of a job: do the work and decide what happens next (`done`,
/// `again`, `after(delay)`, or `failed(reason)`); external effects go in
/// `sends`, dispatched once the step commits. An `err` is a failed attempt.
fn step(_claim: jobs.Claim, job: Job) -> Result<jobs.Step<Job>, Failure> {
    ok({ step: "done", state: job, next: jobs.Next.done, sends: [] })
}

/// How the jobs run: their kind, the state version `step` reads, the
/// lease each attempt holds, the delay before a retry, and how often to
/// look for work.
pub let processing: jobs.Processing = { kind: "@name@", owner: "@name@", version: 1, lease: 30s, backoff: 5s, idle: 100ms }

/// A supervised worker processing the jobs in `store`.
pub provider processor(scope, config: { store: jobs.Store }) -> Result<Unit, Failure> {
    scope.worker(jobs.processor(config.store, processing, step), worker.on_failure({ max_restarts: 3, window: 1min, backoff: 1s }))
    ok(())
}

/// Submit a job, once per `event` (a repeated event is a duplicate).
pub fn submit(store: jobs.Store, event: String, job: Job) -> Result<jobs.Submitted, jobs.Error> {
    store.submit(processing.kind, event, processing.version, job, 5)
}

@derive(Error, From)
pub enum TestError {
    jobs(source: jobs.Error),
    system(source: system.Error),
    timeout,
}

fn test_a_submitted_job_is_processed() -> Result<Unit, TestError> {
    let plan = system {
        store: jobs.store({ path: ":memory:" }),
        work: processor({ store: ref(#store) }),
    }
    use running <- system.run(plan)
    let _ = submit(running.store, "first", { id: 1 })?
    let started = time.now()
    while running.store.count("done")? < 1 {
        if time.now() - started > 5s {
            return err(TestError.timeout)
        }
        task.sleep(5ms)
    }
    ok(())
}
"#;

const SERVICE_MODEL: &str = r#"// The @name@ service's vocabulary: a @item@, what can go wrong, and the
// operations the service offers.
import std.app

/// One @item@.
pub type @Item@ = { id: Int, title: String }

/// A new @item@.
pub type New@Item@ = { title: String }

/// What can go wrong.
@derive(Error)
pub enum Error {
    invalid(message: String),
    not_found(id: Int),
}

/// The service's operations.
pub type Operations = {
    create: app.Operation<New@Item@, @Item@, Error, !{Io, Wait, State}>,
    find: app.Operation<Int, @Item@, Error, !{Io, Wait, State}>,
    list: app.Operation<{}, List<@Item@>, Error, !{Io, Wait, State}>,
}
"#;

const SERVICE_STORE: &str = r#"// The @name@ service's store: the @item@ records it keeps. Another
// adapter (SQLite, Postgres) would provide the same interface; the
// service does not know which one it has.
import std.list
import @name@.model

/// What the service needs from storage.
pub type Store = {
    insert: fn(model.New@Item@) -> Result<model.@Item@, model.Error> !{Io, Wait, State},
    find: fn(Int) -> Result<Option<model.@Item@>, model.Error> !{Io, Wait, State},
    all: fn() -> Result<List<model.@Item@>, model.Error> !{Io, Wait, State},
}

/// An in-memory store (tests and development).
pub provider memory(config: {}) -> Result<Store, model.Error> {
    let rows: Atom<List<model.@Item@>> = Atom.new([])
    ok({
        insert: fn(new: model.New@Item@) -> Result<model.@Item@, model.Error> {
            let all = rows.update(fn(current) { current.append({ id: current.len() + 1, title: new.title }) })
            ok(all[all.len() - 1])
        },
        find: fn(id: Int) -> Result<Option<model.@Item@>, model.Error> { ok(list.find(rows.get(), fn(n) { n.id == id })) },
        all: fn() -> Result<List<model.@Item@>, model.Error> { ok(rows.get()) },
    })
}
"#;

const SERVICE_SERVICE: &str = r#"// The @name@ service: its operations over the store. Every caller (the
// routes, the command line, a worker) calls the same ones.
import std.app
import std.system
import std.text
import @name@.model
import @name@.store

pub provider @name@(config: { store: store.Store }) -> Result<model.Operations, model.Error> {
    let records = config.store
    ok({
        create: app.from_fn({ id: #@name@/create, version: 1, doc: "Create a @item@." }, fn(_call, input: model.New@Item@) -> Result<model.@Item@, model.Error> {
            let title = text.trim(input.title)
            if title == "" {
                return err(model.Error.invalid("a @item@ needs a title"))
            }
            records.insert({ title })
        }),
        find: app.from_fn({ id: #@name@/find, version: 1, doc: "Find a @item@ by id." }, fn(_call, id: Int) -> Result<model.@Item@, model.Error> {
            match records.find(id)? {
                .some(n) => ok(n),
                .none => err(model.Error.not_found(id)),
            }
        }),
        list: app.from_fn({ id: #@name@/list, version: 1, doc: "Every @item@." }, fn(_call, _input: {}) -> Result<List<model.@Item@>, model.Error> {
            records.all()
        }),
    })
}

@derive(Error, From)
pub enum TestError {
    @name@(source: model.Error),
    system(source: system.Error),
}

fn test_create_and_find() -> Result<Unit, TestError> {
    let plan = system {
        store: store.memory(),
        @name@: @name@({ store: ref(#store) }),
    }
    use running <- system.run(plan)
    let call = app.call()
    let created = running.@name@.create.call(call, { title: "  first " })?
    assert(created.title == "first")
    assert(running.@name@.find.call(call, created.id)? == created)
    assert(running.@name@.list.call(call, {})?.len() == 1)
    ok(())
}
"#;

const SERVICE_HTTP: &str = r#"// The HTTP binding of the @name@ service: its route table. The errors
// carry their status, so the service's own errors answer with the status
// the service means.
import std.app
import std.http
import std.route
import std.system
import @name@.model
import @name@.service
import @name@.store

/// The service's errors, as the HTTP layer sees them.
@derive(Error, From)
pub enum Error {
    @name@(source: model.Error),
    route(source: route.RouteError),
    system(source: system.Error),
}

/// The status each error is answered with (500 unless the error chooses
/// one): the route system reads it.
impl HttpStatus for Error {
    fn http_status(self: &Self) -> Int {
        match self {
            .@name@(e) => match e {
                .invalid(_) => 400,
                .not_found(_) => 404,
            },
            .route(_) => 500,
            .system(_) => 500,
        }
    }
}

/// The service's route table.
pub fn table(@name@: model.Operations) -> List<route.Route> {
    [
        route.json(#post, "/@name@", #@name@/create, fn(_request, _params, body: model.New@Item@) -> Result<(Int, model.@Item@), Error> {
            match @name@.create.call(app.call(), body) {
                .ok(item) => ok((201, item)),
                .err(e) => err(Error.@name@(e)),
            }
        }),
        route.json_get("/@name@", #@name@/list, fn(_request, _params) -> Result<(Int, List<model.@Item@>), Error> {
            match @name@.list.call(app.call(), {}) {
                .ok(all) => ok((200, all)),
                .err(e) => err(Error.@name@(e)),
            }
        }),
        route.typed(#get, "/@name@/:id", #@name@/find, fn(_request, params: { id: Int }, _body: {}) -> Result<(Int, model.@Item@), Error> {
            match @name@.find.call(app.call(), params.id) {
                .ok(item) => ok((200, item)),
                .err(e) => err(Error.@name@(e)),
            }
        }),
    ]
}

fn test_the_route_table() -> Result<Unit, Error> {
    let plan = system {
        store: store.memory(),
        @name@: service.@name@({ store: ref(#store) }),
    }
    use running <- system.run(plan)
    let api = route.table(table(running.@name@))?
    assert(http.call(api, http.request(#post, "/@name@", "{\"title\": \"first\"}")).status == 201)
    assert(http.call(api, http.request(#get, "/@name@", "")).status == 200)
    assert(http.call(api, http.request(#get, "/@name@/1", "")).status == 200)
    assert(http.call(api, http.request(#get, "/@name@/9", "")).status == 404)
    assert(http.call(api, http.request(#post, "/@name@", "{\"title\": \"\"}")).status == 400)
    ok(())
}
"#;

const COMMAND: &str = r#"// The @name@ command: its options and how they are parsed.
import std.cli
import std.env

/// The command's options: every field is an option, spelled in kebab
/// case (`--dry-run` for `dry_run`); `args` collects the positional
/// arguments.
pub type Options = {
    name: String,
    count: Int,
    verbose: Bool,
    args: List<String>,
}

/// The values the options start from.
pub fn defaults() -> Options {
    { name: cli.text("world"), count: cli.integer(1), verbose: cli.flag(false), args: cli.text_list([]) }
}

/// Parse the command's arguments.
pub fn parse(args: List<String>) -> Result<Options, cli.Error> {
    cli.parse_or<Options>(args, defaults())
}

/// Parse the program's own arguments.
pub fn parse_env() -> Result<Options, cli.Error> {
    cli.parse_or<Options>(env.args(), defaults())
}

fn test_the_options_are_parsed() -> Unit {
    let options = match parse(["--name", "Ada", "--count", "2", "extra"]) {
        .ok(o) => o,
        .err(e) => panic($"the options should parse: {e}"),
    }
    assert(options.name == "Ada")
    assert(options.count == 2)
    assert(!options.verbose)
    assert(options.args == ["extra"])
}
"#;

const LIBRARY: &str = r#"// The @name@ library: a public type, the operation over it, and a test.
import std.text

/// One @item@.
pub type @Item@ = { id: Int, name: String }

/// A new @item@: the first identifier, and a trimmed name.
pub fn create(name: String) -> @Item@ {
    { id: 1, name: text.trim(name) }
}

/// The @item@'s name, or `fallback` when it is empty.
pub fn name_or(item: @Item@, fallback: String) -> String {
    let name = text.trim(item.name)
    if name == "" { fallback } else { name }
}

fn test_the_library() -> Unit {
    let item = create("  a name  ")
    assert(item.id == 1)
    assert(item.name == "a name")
    assert(name_or(item, "none") == "a name")
    assert(name_or({ id: 2, name: " " }, "none") == "none")
}
"#;

const KEYWORDS: &[&str] = &[
    "and", "as", "break", "const", "continue", "derive", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "import", "in", "let", "loop", "match", "once", "or", "protocol", "provider", "pub",
    "return", "stage", "struct", "system", "pipeline", "true", "type", "unsafe", "use", "var", "where",
    "while",
];

/// Names the generated files use for their own imports and variants.
const RESERVED: &[&str] = &[
    "app",
    "endpoint",
    "http",
    "route",
    "model",
    "memory",
    "service",
    "list",
    "text",
    "std",
    "generate_probe",
];

fn pascal(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// The generated feature: each file's path (relative to the project) and text.
pub struct Plan {
    pub root: PathBuf,
    pub files: Vec<(PathBuf, String)>,
    /// The modules a probe imports to check the files (and run their tests).
    pub imports: Vec<String>,
}

/// Plan worker `name`: one module, `NAME.bdg` under the entry's directory,
/// with a durable job kind, its supervised processor, `submit`, and a test.
pub fn plan_worker(dir: &Path, name: &str) -> Result<Plan, String> {
    let manifest = badger_driver::manifest::Manifest::in_dir(dir)?;
    if !name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        || !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        || KEYWORDS.contains(&name)
    {
        return Err(format!("`{name}` is not a module name: use a lowercase identifier such as `emails`"));
    }
    if RESERVED.contains(&name) || ["jobs", "system", "task", "time", "worker"].contains(&name) {
        return Err(format!("`{name}` is a module the generated file imports: choose another name"));
    }
    let src = Path::new(&manifest.entry).parent().map(Path::to_path_buf).unwrap_or_default();
    let file = src.join(format!("{name}.bdg"));
    // (name the path that is there: the module file or its directory)
    if let Some(taken) = [dir.join(&file), dir.join(src.join(name))].into_iter().find(|p| p.exists()) {
        return Err(format!("{} already exists: a worker is generated only into new files", taken.display()));
    }
    Ok(Plan {
        root: src.join(name),
        files: vec![(file, WORKER.replace("@name@", name))],
        imports: vec![name.to_string()],
    })
}

/// Plan feature `name` in the project at `dir`. `item` names one record
/// (default: `name` without a final `s`).
pub fn plan(dir: &Path, name: &str, item: Option<&str>) -> Result<Plan, String> {
    let manifest = badger_driver::manifest::Manifest::in_dir(dir)?;
    if !name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        || !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        || KEYWORDS.contains(&name)
    {
        return Err(format!("`{name}` is not a feature name: use a lowercase identifier such as `orders`"));
    }
    if RESERVED.contains(&name) {
        return Err(format!("`{name}` is a module the generated files import: choose another feature name"));
    }
    let item = match item {
        Some(i) => i.to_string(),
        None => name.strip_suffix('s').filter(|s| !s.is_empty()).unwrap_or(name).to_string(),
    };
    if !item.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') || item.is_empty() {
        return Err(format!("`{item}` is not a record name: use a lowercase identifier such as `order`"));
    }
    // modules resolve from the entry's directory
    let src = Path::new(&manifest.entry).parent().map(Path::to_path_buf).unwrap_or_default();
    let root = src.join(name);
    if let Some(taken) =
        [dir.join(&root), dir.join(src.join(format!("{name}.bdg")))].into_iter().find(|p| p.exists())
    {
        return Err(format!(
            "{} already exists: a feature is generated only into new files",
            taken.display()
        ));
    }
    let fill = |t: &str| {
        t.replace("@name@", name).replace("@Item@", &pascal(&item)).replace("@item@", &item.replace('_', " "))
    };
    let files = [("model", MODEL), ("memory", MEMORY), ("service", SERVICE), ("http", HTTP)]
        .iter()
        .map(|(f, t)| (root.join(format!("{f}.bdg")), fill(t)))
        .collect();
    let imports = ["http", "memory", "model", "service"].iter().map(|m| format!("{name}.{m}")).collect();
    Ok(Plan { root, files, imports })
}

/// The names the generated files import for themselves: a generated
/// module may not have one of them.
const RESERVED_SERVICE: &[&str] = &[
    "app",
    "endpoint",
    "generate_probe",
    "http",
    "list",
    "model",
    "route",
    "service",
    "std",
    "store",
    "system",
    "text",
];

/// The names the generated command's files import for themselves.
const RESERVED_COMMAND: &[&str] = &["cli", "env", "generate_probe", "list", "std", "text"];

/// The names the generated library's files import for themselves.
const RESERVED_LIBRARY: &[&str] = &["generate_probe", "list", "std", "text"];

/// Check that `name` may name a generated module: a lowercase identifier
/// that is not one of the modules the generated files import.
fn check_name(name: &str, kind: &str, reserved: &[&str]) -> Result<(), String> {
    if !name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        || !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        || KEYWORDS.contains(&name)
    {
        return Err(format!("`{name}` is not a {kind} name: use a lowercase identifier such as `orders`"));
    }
    if reserved.contains(&name) {
        return Err(format!("`{name}` is a module the generated files import: choose another {kind} name"));
    }
    Ok(())
}

/// The record's name for a feature, a service, or a library `name` (the
/// name without a final `s`).
fn item_of(name: &str) -> String {
    name.strip_suffix('s').filter(|s| !s.is_empty()).unwrap_or(name).to_string()
}

/// The text of a generated file: the name and the record's name are
/// filled in.
fn fill(name: &str, item: &str, t: &str) -> String {
    t.replace("@name@", name).replace("@Item@", &pascal(item)).replace("@item@", &item.replace('_', " "))
}

/// The generator's files: the path each takes in the project, and its
/// text (the name and the record's name are filled in).
fn generated(root: &Path, name: &str, item: &str, files: &[(&str, &str)]) -> Vec<(PathBuf, String)> {
    files.iter().map(|(f, t)| (root.join(format!("{f}.bdg")), fill(name, item, t))).collect()
}

/// Plan service `name` in the project at `dir`: a module with the
/// service's vocabulary (`model.bdg`), its store (`store.bdg`), its
/// operations (`service.bdg`), and the route table that binds them to
/// HTTP (`http.bdg`), with tests.
pub fn plan_service(dir: &Path, name: &str) -> Result<Plan, String> {
    let manifest = badger_driver::manifest::Manifest::in_dir(dir)?;
    check_name(name, "service", RESERVED_SERVICE)?;
    let item = item_of(name);
    // modules resolve from the entry's directory
    let src = Path::new(&manifest.entry).parent().map(Path::to_path_buf).unwrap_or_default();
    let root = src.join(name);
    if dir.join(&root).exists() || dir.join(src.join(format!("{name}.bdg"))).exists() {
        return Err(format!(
            "{} already exists: a service is generated only into new files",
            dir.join(&root).display()
        ));
    }
    let files = generated(
        &root,
        name,
        &item,
        &[
            ("model", SERVICE_MODEL),
            ("store", SERVICE_STORE),
            ("service", SERVICE_SERVICE),
            ("http", SERVICE_HTTP),
        ],
    );
    let imports = ["http", "model", "service", "store"].iter().map(|m| format!("{name}.{m}")).collect();
    Ok(Plan { root, files, imports })
}

/// Plan command `name` in the project at `dir`: a module with the
/// command's options and their parsing (`std.cli`), and a test.
pub fn plan_cli(dir: &Path, name: &str) -> Result<Plan, String> {
    let manifest = badger_driver::manifest::Manifest::in_dir(dir)?;
    check_name(name, "command", RESERVED_COMMAND)?;
    // modules resolve from the entry's directory
    let src = Path::new(&manifest.entry).parent().map(Path::to_path_buf).unwrap_or_default();
    let file = src.join(format!("{name}.bdg"));
    if dir.join(&file).exists() || dir.join(src.join(name)).exists() {
        return Err(format!(
            "{} already exists: a command is generated only into new files",
            dir.join(&file).display()
        ));
    }
    Ok(Plan {
        root: src.join(name),
        files: vec![(file, COMMAND.replace("@name@", name))],
        imports: vec![name.to_string()],
    })
}

/// Plan library `name` in the project at `dir`: a module with a public
/// type, an operation over it, and a test.
pub fn plan_library(dir: &Path, name: &str) -> Result<Plan, String> {
    let manifest = badger_driver::manifest::Manifest::in_dir(dir)?;
    check_name(name, "library", RESERVED_LIBRARY)?;
    let item = item_of(name);
    // modules resolve from the entry's directory
    let src = Path::new(&manifest.entry).parent().map(Path::to_path_buf).unwrap_or_default();
    let file = src.join(format!("{name}.bdg"));
    if dir.join(&file).exists() || dir.join(src.join(name)).exists() {
        return Err(format!(
            "{} already exists: a library is generated only into new files",
            dir.join(&file).display()
        ));
    }
    let text = fill(name, &item, LIBRARY);
    Ok(Plan { root: src.join(name), files: vec![(file, text)], imports: vec![name.to_string()] })
}

fn copy_project(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let name = e.file_name();
        if name == ".git" || name == "target" {
            continue;
        }
        let (src, dst) = (e.path(), to.join(&name));
        if e.file_type()?.is_dir() {
            copy_project(&src, &dst)?;
        } else if e.file_type()?.is_file() {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Check the plan in a copy of the project: the generated modules compile
/// and their tests pass. Returns the test report.
pub fn verify(dir: &Path, plan: &Plan) -> Result<String, String> {
    let tmp = badger_driver::private_temp_dir("badger-generate").map_err(|e| e.to_string())?;
    let copy = tmp.join("project");
    let result = (|| {
        copy_project(dir, &copy).map_err(|e| format!("cannot copy the project: {e}"))?;
        for (p, text) in &plan.files {
            let f = copy.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&f, text).map_err(|e| e.to_string())?;
        }
        let probe = copy.join(plan.root.parent().unwrap_or(Path::new(""))).join("generate_probe.bdg");
        let imports: String = plan.imports.iter().map(|m| format!("import {m}\n")).collect();
        std::fs::write(&probe, format!("{imports}\npub fn main() -> Unit {{}}\n"))
            .map_err(|e| e.to_string())?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let out =
            std::process::Command::new(exe).arg("test").arg(&probe).output().map_err(|e| e.to_string())?;
        let report =
            format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        if out.status.success() {
            Ok(report)
        } else {
            Err(format!("the generated feature does not check or its tests fail in this project:\n{report}"))
        }
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

/// Write the plan's files (each must still be new).
pub fn write(dir: &Path, plan: &Plan) -> Result<(), String> {
    for (p, _) in &plan.files {
        if dir.join(p).exists() {
            return Err(format!("{} appeared meanwhile; nothing was written", dir.join(p).display()));
        }
    }
    for (p, text) in &plan.files {
        let f = dir.join(p);
        std::fs::create_dir_all(f.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&f, text).map_err(|e| format!("{}: {e}", f.display()))?;
    }
    Ok(())
}
