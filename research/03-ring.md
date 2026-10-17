# Ring

> Category on clojure-toolbox.com: **HTTP Servers** (closely related entries: **Web Server Abstraction**, **Ring Adapters**)
> Repo: <https://github.com/ring-clojure/ring>
> Authors: Mark McGranaghan, James Reeves & contributors, since 2009
> License: MIT
> Spec: <https://github.com/ring-clojure/ring/blob/master/SPEC.md>
> Latest ring-core: 1.15.5

## What it is

Ring is Clojure's abstraction layer over HTTP. Inspired by Python's WSGI and Ruby's Rack, it is the closest thing the JVM-Lisp world has to a *lingua franca* for web applications. Every Clojure web framework — Compojure, Pedestal, Reitit, Luminus, Duct, Biff, Coast — compiles down to a Ring handler. Every HTTP server integration — Jetty, HTTP Kit, Netty (Aleph), Immutant, Undertow — is a Ring adapter. Many large Clojure codebases consist of hand-written Ring handlers wrapped in three or four pieces of middleware, and call it a day.

It's small: a *handler* is a function from request to response, *middleware* is a higher-order function that wraps a handler, and an *adapter* is the side-effecting glue that hooks a handler into an actual HTTP server.

## Design

### The 30-second version

```
HTTP server (adapter)        Request map            (synchronous Ring)
 ╔══════════════════════╗    ╔══════════════════╗    ╔══════════════════╗
 ║  parses HTTP request  ║ →  ║  {:request-method ║ →  ║  handler         ║
 ║  into a Clojure map  ║    ║   :get            ║    ║   (fn [req] ...)  ║
 ║  and invokes handler  ║    ║   :uri "/foo"     ║    ║   -> response map║
 ║  with the response    ║ ←  ║   :headers ...}   ║ ←  ║  }               ║
 ║  serialized back      ║    ╚══════════════════╝    ╚══════════════════╝
 ╚══════════════════════╝
       middleware: (defn wrap-foo [h] (fn [r] (if ... (h r) (foo-response))))
```

### Concepts

1. **Handler** — `(fn [request] response-map)`. Pure logic; takes a request, returns a response. The synchronous arity is one arg.
2. **Middleware** — `(defn wrap-X [handler] (fn [request] (maybe-mutate request) (let [resp (handler request)] (maybe-mutate resp))))`. Higher-order function from handler to handler. Composable: `(-> handler wrap-params wrap-session wrap-keyword-params wrap-json-body wrap-json-response)`.
3. **Adapter** — the I/O glue. `(defn run-jetty [handler opts] (let [server (Server. ...)] (.start server) server))`. Lives in `ring-jetty-adapter` (default), `ring-servlet`, `ring-jakarta-servlet`, or third-party adapters (Aleph, http-kit, etc.).
4. **Request map** — a plain Clojure map. Keys per `SPEC.md` §1.4 (see below).
5. **Response map** — plain Clojure map with `:status :headers :body`. `:body` must satisfy the `StreamableResponseBody` protocol so adapters can stream it.

### Why this design wins

- **Pure functions all the way down.** A handler is a pure function. Middleware is a higher-order function. Adapter is the only impure thing (and it's a thin shell).
- **Maps as data.** Both directions — request *and* response — are data. You `assoc`, `dissoc`, `update`, `merge`, `select-keys` them. You can `pr-str` them. You can store them in a database.
- **Async is the same model.** Spec §2 adds a 3-arg arity: `(fn [request respond raise] (respond response))`. Adapters detect this arity and go async. A handler can support both via multi-arity. This is what powers `ring-async`, `reitit-async`, and the integration with Aleph.
- **Works on every JVM web server.** Because the abstraction is so thin, every mainstream Java HTTP server has a Ring adapter within weeks.
- **WebSockets are also spec'd.** Spec §3 defines a `Listener` protocol (`on-open`, `on-message`, ...) and a `Socket` protocol (`-send`, `-close`, `-ping`, ...) so any adapter that supports websockets exposes the same interface.

### Request map (Spec §1.4)

| Key | Type | Required |
|---|---|---|
| `:server-port` | Integer | yes |
| `:server-name` | String | yes |
| `:remote-addr` | String | yes |
| `:uri` | String (starts `/`) | yes |
| `:query-string` | String | |
| `:scheme` | keyword (`:http :https :ws :wss`) | yes |
| `:request-method` | lowercase keyword (`:get :post :put :delete ...`) | yes |
| `:protocol` | "HTTP/1.1", "HTTP/2", ... | yes |
| `:headers` | `{String String}` (lowercased keys; comma-joined for repeats) | yes |
| `:body` | `java.io.InputStream` | |
| `:ssl-client-cert` | `java.security.cert.X509Certificate` | |

Deprecated legacy keys: `:character-encoding`, `:content-length`, `:content-type` — derivable from `:headers`.

### Response map (Spec §1.5)

| Key | Type | Required |
|---|---|---|
| `:status` | Integer, 100–599 | yes |
| `:headers` | `{String String}` or `{String [String]}` | yes |
| `:body` | anything implementing `StreamableResponseBody` | |

Built-in body types: `String`, `byte[]`, `InputStream`, `File`, `ISeq`, `nil`. Custom types just implement `write-body-to-stream`.

### Async handler (Spec §2)

```clojure
(fn
  ([request]              ;; sync arity
    {:status 200 :body "hi"})
  ([request respond raise] ;; async arity
    (future
      (try
        (respond {:status 200 :body (slow-compute request)})
        (catch Throwable t (raise t))))))
```

Adapters like Aleph or `:async? true` in Jetty 12 / `run-jetty` invoke the 3-arg arity so the original request thread can return immediately.

### Websocket (Spec §3)

A handler may return a *websocket response* map (`#:ring.websocket{:listener ...}`) instead of an HTTP response. The `Listener` protocol gets called by the adapter:

```clojure
(defprotocol Listener
  (on-open    [listener socket])
  (on-message [listener socket message])
  (on-pong    [listener socket data])
  (on-error   [listener socket throwable])
  (on-close   [listener socket code reason]))
```

So any websocket feature is just a `Listener` implementation that uses a `Socket` (which has `-send`, `-send-async`, `-close`) to talk back.

## Implementation

Ring is small enough to read end-to-end. Concrete pieces in `ring-core`:

- `ring/middleware/*` — the standard middleware: `params`, `keyword-params`, `nested-params`, `multipart-params`, `session`, `cookies`, `flash`, `file`, `file-info`, `head`, `resource`, `content-type`, `charset`, `x-www-form-urlencoded`, `json`, `gzip`, `etag`, `head`, `ssl`, `reload`. Each is a small fn `wrap-X [handler] handler'` doing one transformation.
- `ring/util/*` — `response` (constructor helper that adds sane defaults: charset UTF-8, content-length, etc.), `request` (constructor), `get-header`, `header-names`, `parse-query-params` (via `URLDecoder`), `set-context`, `set-response`.
- `ring/core/protocols.clj` — `StreamableResponseBody` protocol.
- `ring/core/async.clj` — helpers to interop sync/async handlers (`ring.util.async/pipeline-blocking`, `->>`, `let-flow`, `threaded` etc., back-ported mostly from `core.async` flow).
- `ring/ring-jetty-adapter/*` — Jetty adapter, built on top of Eclipse Jetty's standard Servlet API. Modern versions expose Jetty 12's native servlet 6.0 API; older ones used Servlets 4.0.

### Middleware composition

Middleware is just threading:

```clojure
(def handler
  (-> routes
      wrap-keyword-params
      wrap-params
      wrap-session
      wrap-cookies
      wrap-csrf
      wrap-content-type
      wrap-not-modified))
```

Each `wrap-X` is `(defn wrap-X [handler] (fn [req] ...))`. The order matters: middlewares that *add things to the request* (params, session) wrap the inner handler; middlewares that *observe the response* (etag, content-type) wrap the outer.

### Adapters — what actually makes a server

`ring-jetty-adapter` does (simplified):

```clojure
(defn run-jetty
  ([handler] (run-jetty handler {}))
  ([handler {:keys [port async? join?] :or {port 8080 join? true}}]
   (let [servlet  (ring-servlet/servlet handler)
         server   (doto (Server. (QueuedThreadPool.))
                    ...) ]
     (.start server)
     (when join? (.join server))
     server)))
```

Servlet 4.0 era: hand-roll a `Servlet` that intercepts `service` and converts HttpServletRequest into a Ring request map. Servlet 6.0 (Jakarta EE 10, Jetty 12) era: same idea with the new `jakarta.servlet.*` packages.

For Aleph (`clj-commons/aleph`), the adapter is built on Netty's `ChannelHandler`s; it implements the same `RingHandler` Java interface as the Jetty adapter so the abstract layer is honored.

### Body streaming

To keep memory low, `:body` is *not* a `String`. It's anything implementing:

```clojure
(defprotocol StreamableResponseBody
  (write-body-to-stream [body response output-stream]))
```

So `(response :body 1e6-line-csv-file)` doesn't slurp the file — the adapter opens the file and streams it when the response is written. (`ring.util.response/file-response` is the standard helper.)

## API (cheat-sheet)

```clojure
(ns my-app.web
  (:require [ring.adapter.jetty :as jetty]
            [ring.middleware.params :refer [wrap-params]]
            [ring.middleware.keyword-params :refer [wrap-keyword-params]]
            [ring.middleware.json :refer [wrap-json-body wrap-json-response]]
            [ring.util.response :as response]))

(defn hello [req]
  (response/response
    {:hello (get-in req [:query-params :name] "world")}))

(def app
  (-> hello
      wrap-keyword-params
      wrap-params
      wrap-json-body
      wrap-json-response))

(defn -main [& _]
  (jetty/run-jetty app {:port 3000 :join? true}))
```

The standard toolkit:

```clojure
;; Construct
(response/response  body)             ; -> {:status 200, :body body, ...}
(response/content-type  resp "application/json")
(response/header        resp "X-Foo" "bar")
(response/status        resp 201)
(response/file-response path))
;; Read request
(get-in req [:headers "host"])
(:query-params req)
(:form-params req)
(:json-params req)
(:multipart-params req)
(:session req)
```

## How Ring works in deployment

A typical production flow:

1. Build an uberjar (`lein uberjar`) that contains your handler, all middleware, and an embedded Jetty.
2. JVM starts, `-main` calls `jetty/run-jetty` with `{:port (env "PORT" 8080) :async? false}`.
3. Jetty accepts a TCP connection, parses the HTTP request into a request map, calls your wrapped handler.
4. Handler's response map is rendered back to the HTTP response (status line, headers, body streamed).
5. Connections are pooled; static files come from a Jetty `DefaultServlet` rather than going through Ring at all.

Or in a web container (Tomcat, Wildfly, etc.): drop the uberjar's main Ring handler into a `Servlet` (via `ring-servlet`), ship as `WAR`. Same handler code.

For WebSocket: same handler may return `#:ring.websocket{:listener my-listener}`. Jetty upgrades the connection; the adapter hands `on-message` events to your Listener implementation, which writes back via `-send`.

## Why it matters

- Ring is the layer that lets the Clojure web ecosystem talk to itself. A library author writes `(defn page-handler [req] ...)`; consumers can wrap it in middleware, expose it under Jetty or Aleph or http-kit, trace it, time it, or wedge it under an existing reverse proxy — all without changing the handler.
- Almost every Clojure web library — Compojure, Bidi, Reitit, Ataraxy, Pedestal, Keechma, fulcro, Lacinia, Yada, Luminus, Duct — "is" a way of producing Ring handlers. Studying those libraries is mostly learning how they map their DSL to a Ring handler.
- The data-driven request/response model pairs naturally with spec/Schema/Malli validation and OpenAPI generation. Most of Reitit's value-add (coercion, swagger, spec errors) rides on top of "the data is just a map".

## Sources

- `ring-clojure/ring` repo on GitHub
- `SPEC.md` — <https://github.com/ring-clojure/ring/blob/master/SPEC.md>
- Wiki — <https://github.com/ring-clojure/ring/wiki>
- API reference — <https://ring-clojure.github.io/ring/>
