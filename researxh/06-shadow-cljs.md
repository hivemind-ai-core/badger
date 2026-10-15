# shadow-cljs

> Category on clojure-toolbox.com: **ClojureScript Development**
> Repo: <https://github.com/thheller/shadow-cljs>
> Author: Thomas Heller ("thheller"), since 2017
> License: Eclipse Public License
> Bundled: as a JS package on npm (`shadow-cljs`)
> Dep coord on CLJ side: `[thheller/shadow-cljs "VERSION"]`

## What it is

**shadow-cljs** is a build tool / compiler-toolkit for ClojureScript that "just works" — a single `shadow-cljs.edn` config and one CLI command produces a fast, correct, optimized JS bundle with full CLJS REPL, hot code reload, npm interop, code splitting, and React Native support out of the box.

It is the modern de-facto answer to "how do I compile ClojureScript today". In 2024+ it has effectively replaced `lein-cljsbuild`, `figwheel`, `boot-cljs`, and `cljs-devtools` for new projects — these are all superseded.

Despite the name, it is not a fork of any `cljs.*`. It is a *different* way to drive the official `org.clojure/clojurescript` Clojure compiler:

- The Clojure compiler is invoked through its exposed `cljs.build.api`, but the entire surrounding toolchain — classpath, watcher, npm install, bundling, asset server, REPL driver, browser extension — is shadow's own.

## Design

The README's bullet list is the design statement:

> *"Good configuration defaults so you don't have to sweat the details; Seamless `npm` integration; Fast builds, reliable caching; Supporting various targets `:browser`, `:node-script`, `:npm-module`, `:react-native`, `:chrome-extension`; Live Reload (CLJS + CSS); CLJS REPL; Code splitting (via `:modules`)."*

What this actually means:

1. **Single binary pipeline.** Start with `npx shadow-cljs compile frontend`, get a `:browser` build output; the same CLI does `watch`, `release`, `browser-repl`, `node-repl`, `do`, `jar`, `compile-classpath-info`, `init`, `server`/`stop`.
2. **Configuration as data.** A single `shadow-cljs.edn` EDN file describes builds, dev server, npm dependencies, source paths, and target-specific options. No `project.clj` `lein-cljsbuild` plugins, no `:hooks`, no `:notify-command`.
3. **Runs *and* consumes `npm`.** The project sits in a normal Node package; CLJS code calls JS deps through `js/require`. shadow handles the JS bundling via a build-time dependency on **`npm`** + **`webpack`** (legacy) or **esbuild** (since v2.20.x). It uses the npm package's `package.json` for what to bundle; CLJS namespaces resolve JS deps through CLJ-side metadata.
4. **Stable caching.** Each CLJS source gets a stable cache key (content-hash, compile-input hash, build-config hash); unchanged sources don't re-compile. Build times drop from 30 s to <2 s on rebuilds.
5. **Static-analysis closures.** When you call `(my-js-fn x)` from CLJS, shadow rewrites the call to `my_js_fn(x).call(null, x)` (plus analyzer info) so the bundler can produce a static-call graph.
6. **No novelty on the CLJS language side.** Only standard `cljs.core`, the official analyzer, and the official JS backend are used. Newer compiler releases (`:closure-warnings`, `:cljs-rewrite-js-targets`) get picked up automatically.
7. **Modular:** a separate `shadow.grove` (env files), `shadow.css` (CSS dependency graph), `shadow.arborist` (npm dep introspection), `shadow.resource` (bundling CLJ resources), etc. Composable.

## Implementation

shadow-cljs is a JVM Clojure program that wraps the ClojureScript compiler with a different world:

### Classpath and source resolution

- Reads `:source-paths` (default `src/main` and `src/test`) for `.cljs` / `.cljc` files.
- Reads `package.json` for npm packages.
- Computes a *closure-resolved* classpath via `tools.reader` and the official `cljs.analyzer` API.
- Augments that with library JAR entries listed under `:dependencies` (a CLJ-side vector of deps).
- Exposes a JS-facing `shadow.cljs.devtools.runtime` (a JS library, vendored) that lives in the browser and is the *client* of the REPL and hot-reload socket.

### ClojureScript compilation flow

For each "build" (e.g., `:frontend`):

1. Bootstrap a fresh `cljs.env/default-compiler-env`.
2. Watch `:source-paths` for changes.
3. Run analyzer + emitter on every changed namespace (`cljs.build.api/build`).
4. Hand the resulting JS strings to the bundler.
5. Write the output to `:output-dir` (default `public/js`).

The CLJS side of REPL works by injecting the bootstrap scripts into the page, then opening a long-lived WebSocket between the browser and the JVM-side REPL server (`shadow.cljs.devtools.server`). All `eval`, `js/console.log`, `tap>` etc. cross that websocket.

### Build modes (targets)

Each target defines its own closure:

- `:browser` — outputs HTML entry point, calls `cljs.main`, packages CLJS into a webpack bundle.
- `:node-script` — outputs a runnable `.js` (or set of `.js` files) for Node.
- `:npm-module` — outputs a portable JS package that JS projects can install via npm.
- `:react-native` — bundles JS for React Native (Metro replaced by shadow's own bundler).
- `:chrome-extension` — packages service-worker bundles.
- `:script` — quick single-file build.
- `:esm` / `esm-bundle` — ESM-flavored bundle.

### Asset server / dev HTTP

The `dev-http` config is intentionally simple:

```clojure
{:dev-http {8080 "public"}}
```

shadow starts a small Clojure HTTP server (not Jetty, just `ring-jetty-adapter`-style or its own `httpkit`/`aleph` derivative) that serves the build output and forwards non-asset requests to the bundled `index.html` (single-page app dev fallback). When `:dev-http` is running and you save a `.cljs` file, the watcher re-compiles and sends a `chrome.runtime.reload` or websocket push to clients — instant feedback.

### Static call optimization

The CLJS compiler's emitter normally emits `foo.bar(baz, qux)` as a `foo.bar.call(null, baz, qux)` *or* as `foo_bar.call(null, baz, qux)` depending on `:elide-asserts`, `:optimizations`, and the value of `bar`. This trips up JS bundlers because they can't see what `foo.bar` will resolve to at compile time. The shadow JVM re-runs the analyzer, then writes a `cljs_deps.js` file that pre-collects what `foo.bar` *statically* maps to (it reads `(goog/obj/get ...)` calls and `:as-alias`es), so bundlers like esbuild can do static renaming.

This is the source of most of shadow's "it just works" feeling. (Other CLJS tooling requires users to manually maintain externs.)

### Long-running dev server

`shadow-cljs watch build-id`:

- Starts the HTTP asset server.
- Starts `shadow.cljs.devtools.server` REPL server (websocket port by default 9630).
- Starts a `clojure.tools.logging` for compilation diagnostics.
- Watches all `:source-paths`, every `npm` package's `dist/` for HMR, and the CLJC classpath for re-compilation.
- The watcher runs in a `go` loop on a virtual-thread-friendly executor for low overhead.

### Bundling

- By default uses `esbuild` for bundling (faster, smaller), but legacy `:bundle-cmd` allowed invoking `webpack`. Modern builds prefer esbuild.
- Can do code splitting: `:modules {:main {:entries [acme.frontend.app]} :admin {:entries [acme.frontend.admin]}}`. Each module is its own bundle.
- `:release` builds run `release` mode (`:advanced` optimizations, static FP, dead code elimination); `:dev` builds run with `:none` or `:simple` so REPL works.

### Browser Extension

A Chrome / Firefox browser extension (`shadow-cljs-ext`) hooks into Chrome DevTools to send shadow-cljs server commands and inspect CLJS state. The browser opens a websocket to the dev server; commands are dispatched server-side; results returned. Optional, but high productivity.

### Browser-side runtime

`shadow/cljs/devtools/runtime/client.js` is a tiny library injected into the page at compile time:

- Bridges browser `console.log` → server `println`.
- Handles `:notify-compile` / `:notify-rebuild`.
- Provides `shadow.cljs.devtools.api/repl` so REPL prompt is available in the browser.
- Provides runtime meta-info (compiled CLJS version, build-hash, etc.).

## API (configuration cheatsheet)

`shadow-cljs.edn`:

```clojure
{;; Source paths — defaults to ["src/main" "src/test"]
 :paths ["src"]
 ;; Extra Clojure source paths
 :extra-classpaths []
 ;; NPM deps — installed by npm
 :dependencies [[reagent "1.2.0"]]
 :dev-dependencies [[thheller/shadow-cljsjs "REACT-MAPPING"]]
 ;; Dev HTTP server
 :dev-http {8080 "public"}
 ;; Build config
 :builds
 {:app
  {:target     :browser
   :output-dir "public/js"
   :asset-path "/js"
   :modules    {:main {:init-fn acme.frontend.app/init}}
   :devtools   {:repl true :preloads [] :install-chrome-ext? true
                :loader-mode :closure ; or :esbuild
                :hud true}
   :release    {:modules   {:main {:simple true}} ; advanced opts
                :output-dir "public/release"}}
  :server
  {:target :node-script
   :main acme.backend.server/start
   :output-to "target/server.js"}}

 ;; Hooks, JS engines, JSX, etc.
 :js-provider  :closure  ; legacy; default is auto
 :cache-path   ".shadow-cljs"
 :open-file-command-fn (fn [path] ...)}
```

CLI (via `npx shadow-cljs`):

```
shadow-cljs compile app            ; one-shot compile of build :app
shadow-cljs release app            ; compile + advanced optimizations
shadow-cljs watch app              ; live-reload REPL
shadow-cljs browser-repl           ; open CLJS REPL talking to the running watch
shadow-cljs node-repl              ; start CLJS in a Node REPL
shadow-cljs server                 ; run the daemon (manages watch sessions)
shadow-cljs stop                   ; stop the daemon
shadow-cljs init                   ; scaffold a fresh shadow-cljs.edn
shadow-cljs info                   ; print versions / configs / cache state
shadow-cljs do watch,browser-repl  ; "do" pipeline alias
shadow-cljs jar app                ; build a Clojure JAR for the project
```

REPL workflow:

1. `npx shadow-cljs server` starts the JVM Clojure process.
2. `npx shadow-cljs watch app` starts a `:browser` watcher.
3. Open the page at `http://localhost:8080`.
4. In Emacs/VSCodium/IntelliJ, connect CIDER/Calva/Cursive to `localhost:9630` — get a ClojureScript REPL.
5. `(js/alert "hi")` and `(in-ns 'acme.frontend.app)` (re-)work as expected.

## How it works end-to-end (a "hello world" browser build)

1. **Project layout**:

   ```
   .
   ├── package.json
   ├── shadow-cljs.edn
   └── src/main/acme/frontend/app.cljs
   ```

2. **package.json** declares npm deps; shadow will use the same bundler pipeline (`npm install` populates `node_modules`).

3. **`shadow-cljs.edn`**:

   ```clojure
   {:deps true
    :builds
    {:frontend
     {:target :browser
      :modules {:main {:init-fn acme.frontend.app/init}}}}}
   ```

4. **`src/main/acme/frontend/app.cljs`**:

   ```clojure
   (ns acme.frontend.app)

   (defn init []
     (js/console.log "Hello World"))
   ```

5. **Compile**: `npx shadow-cljs watch frontend`.
   - JVM boots, instantiates the analyzer, scans source paths.
   - esbuild picks up `target :browser`, creates a webpack-style bundle.
   - Output: `public/js/main.js` (and lazy-loaded sibling JS).

6. **HTML** at `public/index.html`:

   ```html
   <script src="/js/main.js"></script>
   ```

7. **Live reload**: Edit `app.cljs` → file watcher fires → analyzer re-runs the changed namespace → esbuild increments bundle → browser extension or websocket push tells the page to refresh.

8. **NPM deps**: Add `[react "18.2.0"]` to `shadow-cljs.edn` `:dependencies` (acts like a `package.json` dep with `npm install`). In CLJS, use `(defonce React (js/require "react"))`.

## How the npm pipeline works internally (deep)

1. shadow-cljs JVM launches a **`shadow.cljs.npm`** subprocess or co-uses an in-process npm harness.
2. `:dependencies` are added to a project-local `package.json` (or shadows its keys to a separate JSON the user manages).
3. On every build, shadow generates a `target/npm-deps.js` shim that does `module.exports = require('foo')` for each declared dep.
4. The CLJS compiler writes `cljs_deps.js` next to it, with hardcoded JS class/macro aliases that bypass `goog.object.get` etc. — so esbuild can statically trace every call.
5. esbuild produces `main.js`. Tiny and fast.

This is "the magic" that lets a `(:require [re-frame.core :as rf])` plus `:dependencies [[reagent "..."] [re-frame "..."]]` produce a working, fully-named, tree-shakable React app.

## Why it matters

- **The single tool ClojureScript devs use in 2024+.** Every tutorial, every starter kit, every plugin targets it. Officially recommended by the maintainer of Reagent (`re-frame-template`), Re-frame, Lacinia, Fulcro, etc.
- **CLJS without compromise.** Zero-config npm, fast cold-build caches, sane defaults, good error messages, full REPL.
- **Compiles to a single bundle.** No CLJS special runtime that JS devs have to understand (Google Closure Compiler output is fine, but CLJS folks usually use esbuild's modern output).
- **Maintainer (thheller) maintains multiple high-impact CLJS libs** — `shadow-css`, `shadow-grove`, `re-frame` co-maintenance. The `shadow-cljs-ext` browser extension provides Chrome DevTools-like CLJS dev experience.

## Caveats and alternatives

- **ClojureScript source files only.** For Java/etc., compile in a separate JVM step (e.g., leiningen or `:extra-classpaths`).
- **Requires Node + npm.** (Or Bun.) Unavoidable for modern JS supply chain.
- **Bundler can mask errors.** If you write code that works with `:none` optimizations but breaks in `:advanced`, `:release` will tell you with Closure warnings — but you may need to add `:closure-defines` or `:install-npm-deps true` (a.k.a. `:install-deps`).
- **Alternatives**: `Figwheel-main` (Bruce Hauman, the spiritual predecessor — `lein-figwheel` + `figwheel.main`), `Webpack-based` setups, `vite-plugin` integrations (`shadow-cljs-vite-plugin`). shadow-cljs is the only one that is BOTH npm-aware AND strongly integrated with the CLJS REPL.

## Sources

- Repo — <https://github.com/thheller/shadow-cljs>
- User manual — <https://shadow-cljs.github.io/docs/UsersGuide.html> (canonical documentation)
- Thomas Heller, "shadow-cljs" talks and videos — see links section of README
- Companion projects: `shadow-cljsjs` (npm packages wrapped for CLJS), `shadow-cljs-ext` (Chrome extension), `shadow-css`, `shadow-grove`
