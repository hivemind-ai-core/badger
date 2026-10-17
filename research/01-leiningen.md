# Leiningen

> Category on clojure-toolbox.com: **Build Tools**
> Repo: <https://github.com/technomancy/leiningen> (canonical mirror: <https://codeberg.org/leiningen/leiningen>)
> Author: Phil Hagelberg ("technomancy") and contributors, since 2009
> License: Eclipse Public License 1.0
> Tagline: *"for automating Clojure projects without setting your hair on fire"*

## What it is

Leiningen is the de-facto build tool for Clojure (the "Maven meets Ant without the pain"). It was created in 2009 to give Clojure the same kind of standardized, dependency-aware, repeatable build that Ruby has RubyGems/Bundler/Rake and Python has pip/Fabric. For more than a decade it was installed on virtually every Clojure dev's machine and it bootstrapped almost every Clojure project ever shipped to Clojars.

It is a single-purpose tool. Its job is:

- create new projects from templates,
- resolve and download library dependencies from Clojars / Maven Central,
- start a fully-configured REPL,
- compile Java and Clojure sources,
- run tests,
- package applications as uberjars,
- publish libraries to Maven repositories,
- run any custom automation task written in Clojure (via plugins).

It does not compile ClojureScript, run a web server, or build an OS package — those are plugin concerns.

## Design

Conceptually Leiningen is "Maven + Ant + Clojure" cooked into one tool:

- **Project model is a Clojure map.** Every project is described by a single file, `project.clj`, containing a `(defproject ...)` form. The whole map *is* the build configuration. There are no XML files; no Maven phase XML; no rule-graph DSL. Metadata is data.
- **Maven under the hood.** Leiningen reuses the JVM's artifact-repository infrastructure. Each library is identified by a `[group/name "version"]` triple identical to a Maven coordinate. Dependencies are pulled via Aether/Resolver (Leiningen's Maven facade) and cached in `~/.m2/repository`. By default it queries `clojars.org` and `search.maven.org`.
- **Plugins for extension.** Anything not built-in (uberjar optimization, ClojureScript compilation, packaging, code coverage, ...) is a plugin — itself a regular Clojure project that hangs new tasks off `lein`. Leiningen discovers plugins by adding them to `:plugins` in `project.clj`; the plugin's namespace is loaded and any `defplugin` / `defn` task becomes available.
- **Profiles for context.** Profiles are maps that get merged into `project.clj` at startup. The default profile set is `[:base :user :dev :default <active-cli-profile>]`. `:base` always adds nREPL and dev optimizations; `:user` is `~/.lein/profiles.clj` for personal defaults; `:dev`/`default` come from the project. CLI flags like `with-profile +foo,-bar` add or remove specific profiles. This makes per-task (dev, test, uberjar, production) configuration first-class.
- **Trampoline.** Java has slow startup; Clojure has slower startup. For long-running deployments, `lein trampoline run -m my.ns` lets the Leiningen JVM exit before launching the project's JVM, saving tens of MB of memory in production. The mechanism is a simple pipe/handshake protocol.
- **Reproducibility.** Leiningen cares about repeatable builds. Production deployments should pin dependency versions, set `:offline? true`, and ship `~/.m2/repository` along with the jar so the deployed artifact is byte-identical to the one that passed CI.

## Implementation

The interesting pieces of how it actually works:

- **Bootstrapper script.** `bin/lein` is a shell script (and there are also `.bat`, PowerShell, Windows binaries) that does only one thing: locate or download `leiningen-VERSION-standalone.jar` into `~/.lein/self-installs/`, then `java -cp ... clojure.main -m leiningen.core.main`. After first run, a cached copy is used.
- **Two JVMs.** A developer-typing JVM (Leiningen's own) and the project JVM. Leiningen's JVM hosts the build process — dependency resolution, AOT, REPL launch — and `eval-in-project` *spawns* a child JVM with the project's classpath. Communication between them happens through a socket (the nREPL port) for `:repl`, and through fork/exec pipe streams otherwise.
- **Tasks are functions.** A task is just a function `task-name [project & args] task-body` registered in a multimethod. The root multimethod is `leiningen.core.main/dispatch-task`, dispatched on the first argument. Built-in tasks (compile, test, repl, uberjar, ...) live in `src/leiningen/<task>.clj`. Each task receives the already-resolved project map so the task itself doesn't repeat the resolution work.
- **Dependency resolution.** `leiningen.deps` wraps Aether (Sonatype's Java dependency-resolver library) with a Clojure-friendly API. `:dependencies` vectors in the project map are passed verbatim to Aether which handles transitive graph resolution, conflict detection, snapshot update policies, and POM reading. `lein deps :tree-data` exposes the full resolved graph as data.
- **Classpath construction.** `leiningen.classpath` builds a project's `:dependencies :dev-dependencies :plugins :source-paths :resource-paths :test-paths ...` into a single ordered vector of jar paths and directories. This is what gets passed to `java -cp` for the project JVM.
- **AOT (Ahead-of-Time) compilation.** `:aot` namespaces are compiled to `.class` files with the standard Clojure compiler before the project's main JVM starts. `:target-path` (default `target/`) is where the output goes. AOT is opt-in — most library code is not AOT'd.
- **Project steps as data.** Because the project map is data and Aether returns data, lein can evaluate the whole project pipeline without an imperative class hierarchy. The whole thing is small enough to read end-to-end; most "complexity" is upstream Apache libraries.

## API (how a user interacts with it)

### The `project.clj` file

```clojure
(defproject myproject "0.5.0-SNAPSHOT"
  :description "A project for doing things."
  :license "Eclipse Public License 1.0"
  :url "https://example.com/"
  :dependencies [[org.clojure/clojure "1.11.4"]
                 [clj-http "3.13.1"]]
  :plugins      [[lein-tar "3.2.0"]]
  :main         ^:skip-aot myproject.core
  :target-path  "target/%s"
  :profiles     {:uberjar {:aot :all}
                 :dev     {:dependencies [[ring/ring-devel "1.4.0"]]}}
  :aliases      {"test" ["run" "-m" "kaocha.runner"]})
```

### Common tasks

```
lein new [TEMPLATE] NAME      ; generate a project skeleton
lein test [TESTS]             ; run clojure.test in given namespaces (or all)
lein repl                     ; launch nREPL, auto-resolved port
lein run -m my.namespace      ; require the ns and call (my.namespace/-main)
lein uberjar                  ; build a self-contained jar with all deps
lein jar                      ; build a slim jar (no bundled deps)
lein deploy clojars           ; publish to Clojars (or Maven Central)
lein deps :tree               ; show the resolved dependency tree
lein do clean, test, jar      ; chain tasks together
lein trampoline run -m ...    ; forking mode for low-memory production
lein checkouts                ; live-edit dependencies via symlinks
lein search TERM              ; search Clojars + Maven Central
lein help                     ; list all available tasks
lein help <task>              ; detailed usage of a task
```

### Profiles (`~/.lein/profiles.clj`)

```clojure
{:user {:plugins [[lein-pprint "1.3.2"]
                  [lein-ancient "0.6.3"]]}}
```

### Plugins

Plugins are normal Clojure/JVM libraries that hang new tasks off `lein`. Over 300 exist (full list at <https://wiki.leiningen.org/Plugins>); notable ones include `lein-ring` (web app packaging), `lein-cljsbuild` (legacy ClojureScript), `lein-ancient` (update check), `lein-doo` (browser test runner), `codox` (API docs), `clj-kondo` integration, and `deps.edn` interop shims.

## How it works end-to-end (a `lein test` invocation)

1. The shell script reads `LEIN_HOME`, downloads `leiningen-VERSION-standalone.jar` if missing.
2. JVM starts with `clojure.main -m leiningen.core.main test ...`.
3. `main` climbs up the directory tree looking for `project.clj`.
4. The form `(defproject ...)` is read, evaluated with `eval-in-leiningen`, and the resulting project map is *expanded*: profiles are merged in dependency order (`[:base :user :dev :default <active>]`, with later ones overriding), `:hooks` and `:middleware` (profile functions that transform the map) are run.
5. `leiningen.deps/resolve-dependencies` calls Aether with the merged `:dependencies` to download missing jars into `~/.m2/repository` and produce a fully-resolved coordinate list with versions.
6. `leiningen.classpath/resolve-classpath` concatenates that with `:source-paths`, `:resource-paths`, `:test-paths`, `:java-source-paths`, plus all plugin classpaths, into one ordered classpath vector.
7. `leiningen.test` is the `test` task. It shells out to a *new* project JVM with the resolved classpath, passing the namespaces to run as an argument. The child JVM bootstraps Clojure, requires the test namespace, and calls `clojure.test/run-tests`. Output is captured and echoed.
8. Exit codes propagate; the parent JVM exits with the child's status.

For `lein repl` the picture is similar except instead of starting the child process and exiting, leiningen starts an nREPL *server* inside the project JVM and prints `nREPL server started on port N` for clients (CIDER, Calva, Cursive, lein-monkeys) to attach to.

For `lein uberjar` the child JVM is skipped entirely — the Leiningen JVM does AOT directly into `:target-path` (or `target/uberjar`), assembles the jar with all transitive deps using `leiningen.jar`, and writes the uberjar. Note: the AOT reuses the *project* compiler, not Leiningen's internal one; hence the need to add `lein` to `:plugins` for some compile-time hooks.

## Why it mattered (and still matters)

- It established the Maven coordinate standard for Clojure libraries, so the ecosystem grew on shared `[group/name "version"]` identifiers.
- It defined the conventional project layout (`src/`, `test/`, `resources/`, `project.clj`) that every Clojure IDE learned to recognize.
- It made `(defproject ...)` a runnable test: the project map *is* data, can be inspected from the REPL, and is easy to programmatically mutate (profiles, plugin post-process, aliases).
- The current trend is `tools.deps` / `deps.edn` as the lightweight CLI tool, but lein still ships almost every Clojure company, still owns `lein ring/uberjar/docker`, and remains the path of least surprise for newcomers (`apt install leiningen` works).

## Sources

- <https://codeberg.org/leiningen/leiningen> (canonical)
- <https://codeberg.org/leiningen/leiningen/raw/branch/main/README.md>
- <https://codeberg.org/leiningen/leiningen/raw/branch/main/doc/TUTORIAL.md>
- <https://leiningen.org>
- Phil Hagelberg, "Building Leiningen" — see CONTRIBUTING.md in repo
