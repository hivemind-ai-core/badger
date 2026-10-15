# Component (Stuart Sierra)

> Category on clojure-toolbox.com: **Application Frameworks**
> Repo: <https://github.com/stuartsierra/component>
> Author: Stuart Sierra (Cognitect)
> License: MIT
> Latest release: 1.2.0
> Dep coord: `[com.stuartsierra/component "1.2.0"]`

## What it is

**Component** is a tiny framework — really a *design pattern* with a few helper functions — for managing the **lifecycle and dependencies** of stateful pieces of software in Clojure. It is the canonical answer to "how do I write a long-running Clojure server with a database connection, an HTTP client pool, a queue consumer, and a web server, and not have it all turn into a tangle of global atoms?"

It pairs with Stuart Sierra's "Reloaded Workflow" — `(init) → (start) → (reset) → use REPL → (reset) → …` — and is the substrate under Pedestal, Component-coherent libraries, and most Clojure shops' production systems.

The whole library fits in your head: one protocol (`Lifecycle`), three functions (`system-map`, `using`, `system-using`) and three more (`start`, `stop`, `update-system`). Everything else is a docstring.

## Design

Sierra's stated motivation: *"Large applications often consist of many stateful processes which must be started and stopped in a particular order. The component model makes those relationships explicit and declarative, instead of implicit in imperative code."*

### Core ideas

1. **A *component* is a collection of functions sharing runtime state.**
   *Examples*: database access (query/insert functions sharing a connection pool), HTTP API client (request functions sharing an HTTP connection pool), web server (route handlers sharing session store config), in-memory cache (get/set sharing an atom).
   *Not* an "object" in the OO sense — there are no methods or inheritance; components are just records or maps that carry both *configuration data* (host, port) and *runtime state* (open connection). All component functions take the component record as an argument. No global lookups.

2. **`Lifecycle` protocol — two methods only.**
   ```clojure
   (defprotocol Lifecycle
     (start [component] "Begin resource acquisition. Return updated component.")
     (stop  [component] "Release external resources. Return updated component."))
   ```
   Default implementation is no-op. `start` is called once when the system comes up, `stop` when it shuts down. Components can be **idempotent** — guard with `(if connection …)` — so cleanup after partial-start failures is safe.

3. **Declarative dependency graph via `using`.**
   Each component can declare its dependencies as metadata:
   ```clojure
   (component/system-map
     :db        (new-database host port)
     :scheduler (new-scheduler)
     :app       (component/using
                  (example-component opts)
                  {:database  :db
                   :scheduler :scheduler}))
   ```
   The system computes the topological order, `assoc`s the dependencies in *before* calling `start`, then calls `start` in dependency order. `stop` runs in *reverse* order. Dependencies must form a DAG; cycles are forbidden by `update-system` machinery.

4. **One global per-concern, none per-system.**
   Instead of `def db-connection (atom nil)` scattered through the codebase, there is exactly one place where the database lives: inside the system's `:db` component. The REPL can reach in with `(get-in system [:db :connection])`. Tests can swap a stub with `(assoc-in system [:db :connection] stub-conn)` *before* `start`.

5. **The system is a record.**
   It implements `Lifecycle` itself. Its `start` walks the dependency tree and starts each component; `stop` walks it in reverse. The same record can be used in dev (held in a `def` for REPL inspection) and in prod (constructed once in `main` and discarded).

6. **No magic.** The library does not dictate how you store or invoke the system. It does not provide a logger, a config loader, or an HTTP helper. It is intentionally minimal so libraries can be "Component-friendly" (no top-level side effects, no global atoms) without depending on the framework itself.

### What Component is not

- Not an actor system. No mailboxes, no supervisors.
- Not a dependency injection container with scopes/annotations. There is exactly one scope: the system map.
- Not an inversion-of-control container. The user writes a `defrecord` and the lifecycle methods.
- Not magical: nothing prevents you from misusing a global var; Component just punishes it architecturally (you can't have two system instances share a `def`'d atom).

## Implementation

The library is small. Key files in `src/com/stuartsierra/component/`:

- **`Lifecycle.clj`** — the protocol and its no-op extensions.
- **`dependency_graph.clj`** — a tiny topological sort. Uses a separate library `com.stuartsierra/dependency` for the algorithm; `:dependencies` is stored as metadata on each value in the system map.
- **`system.clj`** — `system-map`, `using`, `system-using`, `update-system`, `update-system-reverse`, `start-system`, `stop-system`, `subsystem`.
- **`exception.clj`** — `ex-without-components`, `ex-component?` helpers that strip the (potentially huge) system map from wrapped exceptions so logs stay readable.

### `start-system` algorithm (simplified)

```
update-system systemMaps in dependency order
  for each component c in system (topo-sorted):
    wait on its dependencies (already started)
    assoc dependencies into c from the system
    call (start c); if any throws, wrap in ex-info
                       with {:system s :component c}; halt.
```

Because data is persistent, "assoc dependencies in" is just a cheap `(assoc component :database the-db-component)`. After `stop`, the dependencies remain associated — the next `start` is fine because `start` is idempotent or clears state.

### Dependency graph

`s/component/dependency` uses Kahn's algorithm: build an in-degree map, pop zero-in-degree nodes into the queue, emit them in order. So `(deps-of app)` returns `{:database :db :scheduler :scheduler}` and the system computes `app` needs `:db` and `:scheduler`. Cycles throw `IllegalStateException`.

### Subsystems

`subsystem` (since 1.1.0) lets you grab a subset of the dependency closure:
```clojure
(component/start (component/subsystem system [:app]))
```
It transitively pulls in only the components `app` actually needs (e.g., its `:db`), leaving the rest of the system untouched.

### Errors

If any component's `start` throws, `start-system` wraps it in `ex-info` with the partially-built system attached. There is no automatic retry or rollback. The user is expected to: log, then `stop` the partially-started system (relies on idempotence) or discard it. This is intentionally primitive.

## API (in `com.stuartsierra.component`)

### Constructors

- `(system-map & key/values)` — make a system; values are components.
- `(using component dep-keys-or-map)` — attach dependency metadata.
  ```clojure
  (using c [:a :b])              ; component and system share :a, :b keys
  (using c {:a :db :b :sched})   ; component keys :a/:b, system keys :db/:sched
  ```
- `(system-using system dep-map)` — attach `using` to many components at once:
  ```clojure
  (-> (system-map :db ... :app ...)
      (system-using {:app {:database :db}}))
  ```

### Lifecycle

- `(start system)`, `(stop system)` — walk the dependency tree.
- `Lifecycle` protocol with default no-op impl — extend only on records that need it.
- `(subsystem system ks)` — extract closure.

### Helper error fns

- `ex-without-components` — strip large system map from exception for logging.
- `ex-component?` — true if the exception was wrapped by `start-system`/`stop-system`.

## How it works (end-to-end)

### A worked example

```clojure
(ns com.example.app
  (:require [com.stuartsierra.component :as component]))

(defrecord Database [host port connection]
  component/Lifecycle
  (start [this]
    (println ";; connecting db")
    (assoc this :connection (connect-to-database host port)))
  (stop  [this]
    (when connection (.close ^java.sql.Connection connection))
    (assoc this :connection nil)))

(defrecord WebServer [port app-component db http-server]
  component/Lifecycle
  (start [this]
    (assoc this :http-server
                 (run-jetty {:port port
                             :handler (routes
                                        (GET "/users/:id" [id]
                                          {:body (get-user (:connection db) id)}))})))
  (stop  [this]
    (when http-server (.stop ^org.eclipse.jetty.server.Server http-server))
    (assoc this :http-server nil)))

(defn new-system [cfg]
  (-> (component/system-map
        :db    (->Database (:db-host cfg) (:db-port cfg))
        :app   {:config cfg}
        :server (component/using
                  (map->WebServer {:port (:http-port cfg)})
                  {:app-component :app :db :db}))
      component/start))
```

When `(start system)` runs:

1. The dependency graph resolves: `:db` and `:app` have no deps; `:server` depends on `:app` and `:db`.
2. `:db.start` opens a JDBC connection.
3. `:app.start` runs (e.g., bootstraps a connection pool or in-memory DB schema using the config map).
4. For `:server.start`, the system first `assoc`s `:db` into `:server` (so `:server` now has `:db` as the started DB record). Then `(start server)` runs and the closure `(routes ...)` sees the live connection.
5. The same system can be re-started by calling `(start` on a `stop`ped system) — each component's lifecycle methods act on its own runtime state.

For development, Sierra's standard workflow:

```clojure
;; user.clj
(def system nil)

(defn init [] (alter-var-root #'system
              (constantly (app/new-system cfg))))
(defn start [] (alter-var-root #'system component/start))
(defn stop  [] (alter-var-root #'system component/stop))
(defn go    [] (init) (start))
(defn reset [] (stop) (refresh :after 'user/go))
```

Now you can change a file, hit `(reset)`, and the whole graph is rebuilt without restarting the JVM.

## Why this matters

- Component is the foundation of professional Clojure service architectures. It is small enough to read in an afternoon but expressive enough to model any stateful system.
- It's *boringly correct*. It does not invent a vocabulary — components, dependencies, lifecycle, system. You already know what these mean.
- It is **the** reference architecture for Clojure DevOps tooling: Pedestal, Duct, Donut, Integrant, Kit, Trapperkeeper — every modern Clojure service framework either uses Component directly or follows the same pattern.
- Combined with `tools.namespace`'s `refresh`, it produces a tight REPL-first development loop.

## Caveats and alternatives

- The system map is large and visually repetitive (the same component appears in many places because dependencies are `assoc`'d). Memory cost is negligible; visual cost is real.
- Dependency graph must be declared by hand; auto-discovery is not supported (intentionally).
- Cyclic dependencies are rejected — usually that's correct, sometimes it's annoying. Workaround: use an atom shared across components, or step one component down to a Closure over an atom.
- Competing design: **Integrant** (by J. S. Snarr, now maintained by Weavejester). Integrant is *data-driven* — the system is just a map from keys to config, and a separate `init` step computes the dependency graph. Less boilerplate, more "let the data decide" feel; many consider Integrant strictly nicer than Component for greenfield projects. Stuart Sierra has said he prefers Component because its explicit dependency graph is more debuggable. Both ship in production.
- Competing design: **Mount** by tolitius — uses `defstate` macros and a global ordered list of states. Picked up fast by some, criticized for the same global-state issues Component avoids.

## Sources

- Repository README, <https://github.com/stuartsierra/component>
- Stuart Sierra, "Components, Systems, and the Reloaded Workflow", Clojure/West 2014 (YouTube, 40 min)
- Stuart Sierra, "Lifecycle Composition", <https://stuartsierra.com/2013/09/15/lifecycle-composition/>
- Stuart Sierra, "My Clojure Workflow, Reloaded", <https://stuartsierra.com/2013/06/04/clojure-workflow-reloaded/>
- Manual and screencast at <https://github.com/stuartsierra/reloaded>
