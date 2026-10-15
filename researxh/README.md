# Selected projects from clojure-toolbox.com — index

This directory collects research notes on the most important Clojure ecosystem projects, chosen from the categories on <https://clojure-toolbox.com>.

## How selection was made

clojure-toolbox.com lists 243 categories. Rather than pick favorites heuristically, I scanned the canonical Clojure ecosystem for the **foundational pillars** — libraries that everything else either builds on, integrates with, or substantially replaces. These are the projects that a working Clojure developer (writer of an HTTP service, a ClojureScript SPA, a CLI tool, or a generic library) is most likely to use *daily*. Concretely:

| # | Category on clojure-toolbox | Selected project | Why this one |
|---|---|---|---|
| 1 | Build Tools | **Leiningen** | The original Clojure build tool (since 2009); still installed everywhere; defines the project map and conventional layout everyone uses. |
| 2 | Application Frameworks | **Component (Stuart Sierra)** | The canonical state-management pattern; foundation for Pedestal, Duct, Donut, Kit. |
| 3 | HTTP Servers / Web Server Abstraction | **Ring** | Clojure's WSGI/Rack. Every web framework on the JVM compiles down to a Ring handler. |
| 4 | Asynchronous Programming | **core.async** | Clojure's standard CSP-channel + `go`-block system; the *standard* way to do async on JVM *and* CLJS. |
| 5 | Parsing | **Instaparse** | "What if CFGs were as easy as regex?" — GLL parser that handles every CFG (left-recursive, ambiguous, etc.). |
| 6 | ClojureScript Development | **shadow-cljs** | Modern de-facto CLJS compiler-toolkit; single EDN config, npm + REPL + live reload out of the box. |
| 7 | Web Frameworks / HTTP Routing | **Reitit** | Fast, data-driven router with coercion, Ring adapter, frontend router, OpenAPI — all from the same data. |
| 8 | *(synthesis)* | **Runtime requirements** | Follow-up: which runtime features the seven libraries imply, and the smallest viable blessed extension library. |

This is a *cross-section through the stack*: build → lifecycle → HTTP plumbing → async runtime → parsing → JS compilation → routing/handler composition. Taken together they cover what almost every Clojure production system consists of.

## Reading order

If you read all seven files top-to-bottom, you'll cover the standard Clojure dev stack from the bottom up:

1. **Leiningen** — builds and runs the project.
2. **Component** — shapes the long-running service.
3. **Ring** — handles every HTTP request.
4. **core.async** — coordinate concurrently.
5. **Instaparse** — turn text into data.
6. **shadow-cljs** — push the UI to the browser.
7. **Reitit** — compose the request handlers.

## Per-project notes

Each file in this directory covers:

- **What it is** — the one-paragraph summary.
- **Design** — why it works the way it does (concepts, abstractions, trade-offs).
- **Implementation** — how it's actually built (algorithms, data structures, code paths).
- **API** — the surface a user interacts with.
- **How it works end-to-end** — a worked example / pipeline.
- **Why it matters / caveats / alternatives**.

All material is sourced from each project's own README, source comments, official documentation, and the design rationale documents where they exist (Rich Hickey's "core.async Rationale", Stuart Sierra's blog, etc.). Where web sources were quoted directly, they are noted at the end of each file.

## Notes about the source

clojure-toolbox.com is a community-curated index of Clojure libraries organized by topic. It is the best starting point for *finding* libraries, even today (2026). The homepage HTML is on Codeberg — see <https://github.com/clojure-toolbox.com> for the project itself.
