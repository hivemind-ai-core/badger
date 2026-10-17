# Reitit

> Category on clojure-toolbox.com: **Web Frameworks** (also listed under **HTTP Routing**)
> Repo: <https://github.com/metosin/reitit>
> Author: Metosin (Tommi Lähde, Joonas Javanainen, Miikka Koskinen) & contributors, since 2017
> License: Eclipse Public License
> Latest release: `metosin/reitit` 0.11.0
> Dep coord: `[metosin/reitit "0.11.0"]`

## What it is

Reitit (Finnish: "route", nominative "reitti") is Clojure(Script)'s **fast, data-driven router**. It is a single integrated library that can:

- serve as a *Ring router* (HTTP server-side) — drop-in replacement for Compojure + compojure-api + ring-swagger;
- serve as a *frontend router* — replaces bidi, secretary, krell/router;
- serve as an *HTTP interceptors router* (Pedestal-style);
- generate OpenAPI / Swagger docs from the routes;
- validate routes, params, bodies and responses against **clojure.spec**, **Malli**, or **Schema**.

It's the de-facto modern choice for new Clojure HTTP APIs. The author's talk "Reitit, The Ancient Art of Data-Driven" at Clojure/North 2019 explains the philosophy in full.

It's small (each submodule is ~1-2 K lines), very fast (named for httprouter's benchmark), and exposes *everything* as plain Clojure data — the same router works on CLJ and CLJS.

## Design

The README pulls no punches:

> *"A fast data-driven router for Clojure(Script). Simple data-driven route syntax; Route conflict resolution; First-class route data; Bi-directional routing; Pluggable coercion; Helpers for Ring, http, pedestal & frontend; Friendly error messages; Extendable; Modular; Fast."*

### 1. Routes are data

```clojure
[["/api"
  ["/ping"                       {:get {::handler       (fn [_] {:status 200 :body "pong"})
                                      ::parameters {:query {:x int?}
                                                   :body {:y int?}}
                                      ::responses  {200 {:body {:total int?}}
                                                    400 {:body {:error string?}}}}}}]
  ["/orders/:id"                 {:get {:handler      (fn [{{{:keys [id]} :path} ::req}]
                                                      {:status 200 :body (get-order id)})}}]
  ["/users/:user-id/orders/:id"  {:get {::handler ...}
                                  ::coercion ::spec}]]
 ["/admin"
  [""                            {:middleware [admin-auth] ...}]]]
```

The routes are just Clojure data: nested vectors, keywords, and per-route metadata. There is no DSL, no macros required to *define* a route, no parens-after-parens DSL. Reitit consumes this vector, derives a router, and provides `match-by-path` / `match-by-name`.

### 2. Routes are bi-directional

The same router can go path → handler (request-time) **or** name → path (URL generation):

```clojure
(def router (r/router [["/api/orders/:id" ::order]]))

(r/match-by-path router "/api/orders/42")
;; #Match{:template "/api/orders/:id" :data {:name ::order}
;;         :result nil :path-params {:id "42"} :path "/api/orders/42"}

(r/match-by-name router ::order {:id 2})
;; #Match{:template "/api/orders/:id" :path-params {:id 2} :path "/api/orders/2"}
```

This is what makes the frontend router trivial: calling `(r/match-by-name router ::profile {:user-id 42})` returns the URL the link should point at, so history navigation and form action URLs are generated from the same source as the routes.

### 3. Route data is first-class

Every node in the route tree carries a `:data` map. The `data` map is used by middleware (catches `:coercion`, `:middleware`, `:parameters`, `:responses`), by validators, and by user code. Composed by walking up the tree: child merges with parent.

### 4. Conflict resolution

Multiple patterns can match the same path; Reitit detects it, picks one based on priority, and **prints a warning** when ambiguous:

```
WARNING: Reitit routes conflict for /api/orders/42 — both :order and :order-bak match.
        Consider disambiguating with constraints.
```

This makes "I added a route and the wrong handler is being called" invisible — the router tells you at compile time.

### 5. Pluggable coercion

`:coercion` is a value in `:data`. The supported adaptors are `reitit.coercion.spec`, `reitit.coercion.malli`, and `reitit.coercion.schema`. Adding support for a new validator is one impl of `reitit.coercion/Coercion`.

The coercion system takes:
- `:parameters` (from query / path / form / body / header)
- `:responses` (`200 :body {...schema}`)

…and applies the validation/transform at runtime. For CLJS, the same coercion works (since it's just data and a runtime validator).

### 6. Friendly error messages

Reitit prints human-friendly error messages for common problems:
- conflicting routes
- invalid `:parameters` syntax
- failed coercion
- misspellings in `:name`

This is a small but very real DX improvement.

### 7. Modular

The library is a set of namespaces under a single artifact; you require what you want:

```clojure
(reitit.core)         ;; just the routing logic
(reitit.ring)         ;; Ring handler production
(reitit.ring.middleware.parameters) ; query / form params
(reitit.ring.middleware.muuntaja)   ; content negotiation
(reitit.ring.middleware.exception)  ; exception → response
(reitit.coercion.spec) ;; coercion
(reitit.frontend)     ;; CLJS / pushState router
```

So a ClojureScript SPA can `import` just the frontend bits without dragging in Ring or muuntaja.

### 8. Targets

Reitit targets four runtime models with the same core API:

| Target | Adds |
|---|---|
| **Ring** | produces a Ring handler from the routes, integrates with all `ring-*` middleware. |
| **HTTP interceptors** (Pedestal) | produces a Pedestal-style interceptor chain. |
| **Frontend** | produces a `(r/match-by-name router ...)` URL builder plus a small listener for `popstate` / click interception. |
| **Just routes** | provide your own dispatch; many people just `match-by-path` and call a multimethod. |

## Implementation

### Route representation

Internally, a route is just `(r/router data)`:

- The input tree is normalized into a flat list of `Route` records, each with `:path`, `:parts` (the path split on `/`), `:data`, `:template`, `:handler`.
- `Route` records also carry `:required` (path-params required by the template) and `:result` (the data transformation function for `:path-params` → injected data).
- Then `expand-path-params` and `find-by-path` builds a *radix-tree* (actually a trie with matched-param merging).

### Matching algorithm

Reitit builds a **radix tree / prefix trie** where each `/:param` node falls back to a wildcard match. For `:host`-based routing, the trie is keyed on the host first.

For each path:

1. Split on `/`.
2. Walk the trie, matching each segment against a literal node, then trying each parameter node, then the catch-all node.
3. The matching node's `:result fn` builds the match object: `:path-params`, `:data`, `:template`, `:name`, `:path`.

When multiple patterns apply (e.g., `/orders/new` could be a "new" literal or `:id`), priority is determined by:

- literal prefix before param,
- number of literal segments before the first param,
- then by registration order.

Reitit's README documents the algorithm; the source uses a fast recursive descent that returns the first matching route unless `[:multiple` is requested.

### Coercion

Coercion's runtime model:

1. On `match`, the route's `:parameters` schema is located.
2. Before invoking the handler, `reitit.ring.coercion/coerce-request` runs the schema over the request map's parts (`query`, `body`, `path`, `header`), throws ex-info on failure, returns a `::parameters` map.
3. After the handler returns, `coerce-response` validates the body/headers against `:responses`. Failures become 500 responses with the failed coercion info.

Each coercion module implements:

```clojure
(defprotocol Coercion
  (-compile [_spec _options] ; => request-coercion-fn-and-response-coercion-fn
    {:request (fn [request] ::parameters)
     :response (fn [response] ::response)}))
```

So coercion is just a function from schema to "do this validation at match time". The protocol makes it pluggable.

### Performance

Reitit's prefix trie is the same algorithm `httprouter` (Go) uses. The Clojure implementation:

- Allocations are minimized: the `Route` records are produced at compile time, so runtime work is only dispatch + parameter extraction.
- Match time is O(number of segments + param-keys). For typical APIs <2 µs/match on modern JVM.
- The author benchmarks against `bidi`, `Compojure`, `Pedestal`, and `httprouter`; consistent wins or ties across the board.

### Generators

The `reitit.openapi` / `reitit-swagger` submodules generate OpenAPI/Swagger specs by walking the route tree, picking up `:parameters :body :responses`, and producing either a complete OpenAPI 3 spec (reitit-openapi) or a Swagger-2 spec. That's what makes it easy to expose `/swagger.json` from a Reitit-ring app.

### Frontend router

The `reitit.frontend` namespace provides:

- `(use-router! router-id-handler on-navigate-fn)` to listen for `popstate` and link clicks.
- `(href router :route-name params)` to compute a URL — pure function.
- `(navigation goto! router)` to push a new URL onto `history.pushState`.

The core router is shared with the server-side one: the same `r/router` provides both views of the routes.

### Hooks (extension)

You can extend matching via `:hooks` in route data. Hooks are functions that wrap the dispatcher; used for things like CORS, ad-hoc middleware on specific routes, or authentication. Reitit ships `:reitit/router-coercion` and `:reitit/router-parameters` as built-in hooks.

## API (Common surface)

The minimum useful API:

```clojure
(require '[reitit.core :as r])

(def router (r/router data opts?))

(r/match-by-path  router "/api/orders/42")        ;; #Match
(r/match-by-name  router :order {:id 42})         ;; #Match
(r/compensate     router partial-match)           ;; insert computed params
(r/routes         router)                         ;; list of all routes
(r/route-names    router)                         ;; set of names
(r/expand-routes  data)                           ;; normalize data tree
```

### Ring wrapper

```clojure
(require '[reitit.ring :as ring])

(def app
  (ring/ring-handler
    (ring/router routes data)
    (ring/create-default-handler
      {:not-found    (constantly {:status 404 :body "Not Found"})
       :method-not-allowed (constantly {:status 405})
       :param-missing (fn [{:keys [type]}]
                       {:status 400
                        :body {:error type}})})))
```

The router produces a per-request handler. The optional default-handler handles unmatched paths, method-not-allowed, etc.

### Middleware

- `reitit.ring.middleware.parameters` — `parameters-middleware` decodes URL-encoded query/body params into keyword-keyed maps.
- `reitit.ring.middleware.muuntaja` — content negotiation over JSON / EDN / Transit / Msgpack.
- `reitit.ring.middleware.exception` — turns exceptions into HTTP responses (uses `clojure.ex-data` for the message).
- `reitit.ring.coercion/coerce-request-middleware` and `/coerce-response-middleware` — runs the `:parameters` and `:responses` coercions.

### Frontend

```clojure
(require '[reitit.frontend :as rf])

(rf/router routes)
(rf/match-by-name router :profile {:user-id 42})   ;; "/users/42/profile"
(rf/href router :profile {:user-id 42})           ;; same
;; Hook up the auto-routing:
(goog.events/listener js/window "popstate" #(rf/match-by-path router (.-pathname (.-location js/window))))
```

## How it works end-to-end (an API example)

```clojure
(require '[muuntaja.core :as m])
(require '[reitit.ring :as ring]
         '[reitit.coercion.spec]
         '[reitit.ring.coercion :as rrc]
         '[reitit.ring.middleware.exception :as exception]
         '[reitit.ring.middleware.muuntaja :as muuntaja]
         '[reitit.ring.middleware.parameters :as parameters])

(def app
  (ring/ring-handler
    (ring/router
      ["/api"
       ["/math" {:get
                 {:parameters {:query {:x int?, :y int?}}
                  :responses  {200 {:body {:total int?}}}
                  :handler    (fn [{{{:keys [x y]} :query} :parameters}]
                                {:status 200 :body {:total (+ x y)}})}}]]
      {:data {:coercion   reitit.coercion.spec/coercion
              :muuntaja   m/instance
              :middleware [parameters/parameters-middleware
                           muuntaja/format-middleware
                           exception/exception-middleware
                           rrc/coerce-request-middleware
                           rrc/coerce-response-middleware]}})))
```

Concrete request:

```
GET /api/math?x=1&y=2
```

Pipeline:

1. Jetty → `(app {...})`
2. `exception-middleware` wraps everything in `try/catch`.
3. `parameters-middleware` decodes query string → `{:query {"x" "1" "y" "2"}}` (still strings).
4. `muuntaja/format-middleware` content-negotiates — for empty `:headers "Accept"` body it does nothing special.
5. `rrc/coerce-request-middleware` runs `:parameters :query {...}` spec, which turns `{"x" "1" "y" "2"}` into `{:x 1, :y 2}` and `:params` map is attached to the request under `:parameters`.
6. The router resolves `/api/math` and dispatches to the handler.
7. The handler returns `{:status 200 :body {:total 3}}`.
8. `rrc/coerce-response-middleware` validates against `:responses {200 {:body {:total int?}}}` — pass.
9. `muuntaja/format-middleware` encodes `{:total 3}` as JSON (because `Accept: application/json` was sent).
10. Back out to Jetty → client.

Error path (`GET /api/math?x=1&y=a`):

- Step 5 fails. `coerce-request-middleware` throws an `ex-info` with `:type :reitit.coercion/request-coercion`.
- Step 2's `exception-middleware` catches it, returns:

  ```json
  {
    "type": "reitit.coercion/request-coercion",
    "problems": [{"in":["y"], "val":"a", "pred":"clojure.core/int?", "via":["spec$.../y"]}],
    "spec": "...",
    "value": {"x":"1","y":"a"},
    "in": ["request","query-params"]
  }
  ```

- with HTTP 400.

## Why it matters

- **One library for the whole routing problem.** Routes-as-data, Ring adapter, frontend router, coercion, OpenAPI, exception middleware, plus performance leadership — all maintained in one place.
- **Data-driven routes integrate with Reitit-related libraries** (Spectacle, swagger-ui, etc.). The data is the spec.
- **Bi-directional matching** is a 5% of code in libraries like bidi/compojure but is *the* feature for frontend SPAs (URL generation from name + params is what saves you from string-typed URLs everywhere).
- **Sponsored by Metosin**, an OSS-friendly consultancy that uses the library on customer work.

## Caveats and alternatives

- Per the README: requires Clojure 1.11+ and Java 11+.
- `:parameters` schema discipline can be awkward to retrofit to a project with many hand-written handlers; the docs recommend writing the route data first and the handler second.
- For pure CLJ historical projects, Compojure / bidi (and Compojure-API's spiritual successor lingo) still work fine. Reitit wins on greenfield and on consolidation.
- **Alternatives**:
  - **Ataraxy** (Weavejester) — experimental spec-driven router; very similar in spirit, simpler API, smaller surface.
  - **bidi** (juxt) — the ancestor; bi-directional without opinion; no coercion built in.
  - **Compojure** (Weavejester) — older macro-based DSL; down-level routes, no data-driven.
  - **calfpath** — minimalistic; replaced by Ataraxy.
  - **Pedestal's router** — Pedestal comes with its own router/interceptor system; if you're using Pedestal, you don't usually reach for Reitit.

## Sources

- Repo — <https://github.com/metosin/reitit>
- Full docs (cljdoc) — <https://cljdoc.org/d/metosin/reitit/>
- Slides / video "Reitit, The Ancient Art of Data-Driven", Clojure/North 2019 — <https://www.slideshare.net/mobile/metosin/reitit-clojurenorth-2019-141438093>
- Blog: <https://www.metosin.fi/blog/reitit/>
