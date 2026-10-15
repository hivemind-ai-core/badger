# Instaparse

> Category on clojure-toolbox.com: **Parsing**
> Repo: <https://github.com/Engelberg/instaparse>
> Author: Mark Engelberg (a.k.a. "Engelberg")
> License: Eclipse Public License
> Latest release: 1.5.0
> Dep coord: `[instaparse "1.5.0"]`
> Tagline: *"What if context-free grammars were as easy to use as regular expressions?"*

## What it is

**Instaparse** turns a grammar written in BNF / EBNF / ABNF into a working parser with one function call: `(insta/parser "S = …")`. It returns a function that maps strings → parse trees (vectors of keywords/strings/vectors). For Clojure (host) it works in pure Clojure; for ClojureScript a macro lets you embed the parser at compile time.

Its big differentiator from `clr-parse`, `lexode`, or parser combinator libraries:

- It handles **every context-free grammar**: left-recursive, right-recursive, ambiguous, deeply nested.
- It accepts PEG-like extensions (`&` lookahead, `!` neg-lookahead, `/` ordered-choice) on top of CFG.
- It produces parse trees in the same shape you would write by hand: hiccup-style `(tag child1 child2 …)` or enlive-style `{:tag :tag :content (...)}`.

Used everywhere Clojure needs to parse non-trivial text: query languages (Datomic, Crux, HoneySQL pipe → S-expression), config formats (EDN readers, Hiccup, CLJ/CLJS for tooling like clj-kondo), DSLs (Krell, Conjure, Calva diagnostics), and even real languages (Fluorine, Parlance, WebSocket frame parsers).

## Design

### Author's stated goals (from the README intro)

> *"Instaparse aims to be the simplest way to build parsers in Clojure."*

- **No Grammar Left Behind.** Works for *any* context-free grammar — left-recursive, right-recursive, ambiguous. Most other parser-generator libraries require LL(1), LALR(1), or non-left-recursive.
- **EBNF / ABNF in.** The grammar is just a string. You don't write a parser-combinator tree or a code-generation macro.
- **PEG-like extensions** for power: `&A` (lookahead), `!A` (negative lookahead), `A / B` (ordered choice).
- **Tree shapes you actually want.** Both hiccup (default) and enlive trees.
- **Detailed error reporting** — failure object includes the furthest point, line/column, and what was expected.
- **Lazy list of all parses** for ambiguous grammars (invaluable for debugging).
- **Total mode** — embed the failing-unparsed substring back into the tree as a `:instaparse/failure` node, for IDE-style error recovery.
- **Programmatic combinators** as an alternative to the string syntax (`c/plus`, `c/star`, `c/alt`, …) for grammars that need to be assembled at runtime.

### Why these choices

- Strings as grammar input (rather than s-expressions) mean Instaparse grammars are *visually identical* to textbook BNF. CS students can read `S = 'a'+ 'b'+` instead of `(seq (plus "a") (plus "b"))`.
- A hieupo tree (hiccup) for parse trees means your parser output *looks like* the data you'd want: `[:sentence [:word "Hello"] [:word "World"]]` — destructurable with normal Clojure idioms.
- Lazy parsings (`parses` returns a sequence) for ambiguous grammars; *no* conventional parser generator exposes this. It is the most powerful debugging feature.
- The `:total true` mode, plus character-span metadata on every node, plus optional line/column computation — this triad is what enables IDE-quality error reporting.

## Implementation

### Parsing strategy: GLL

Instaparse uses **GLL (Generalized LL) parsing** with **RNGLR**-style graph-structured stacks. The reference paper: Scott and Johnstone, "GLL Parsing" (2010). GLL is the *only* practical algorithm known to handle arbitrary CFGs in polynomial time while keeping the implementation small and giving all parses for ambiguous grammars.

Key facts about the algorithm:

- Uses a *graph-structured stack (GSS)* of parsing states: each `(state, position)` pair is memoized so the parser can attempt the same action many times without re-exploring.
- For each grammar rule, builds an **AND/OR recursive-descent equivalent** where AND nodes sequence sub-parsers and OR nodes try each alternative.
- Yields *all* parses by Backtracking/DFS over the GSS; this is why `parses` can return many trees.
- Time complexity is O(n³) in the worst case for highly ambiguous grammars — fast for typical grammars; brutal for genuinely ambiguous ones with large N.

### Architecture

- **`instaparse.cfg`** — utilities for representing grammar productions.
- **`instaparse.gll`** — the GLL implementation, ~5000 lines.
- **`instaparse.augment`** — grammar transformers (e.g., turning `A*` into a sub-rule `A-star = A A-star | ε`).
- **`instaparse.combinators`** — the parser-combinator API.
- **`instaparse.transform`**, **`insta/visualize`**, **`add-line-and-column-info-to-metadata`** — post-processing helpers.
- **`defparser`** macro — pre-compiles the grammar at macro-expand time, bypassing the runtime parser-build cost.

### Pipeline

1. **Parse the EBNF string** into a Clojure data structure (the grammar). Uses standard regex/string-splitting, not Instaparse itself.
2. **Apply augmentations**: e.g., `A*` → `A-star = A A-star | ε`; `A?` → `(A | ε)`. Lets the rest of the algorithm operate on a normalized BNF where every production is exactly `A = ε | B C D | B | C | …`.
3. **Run GLL on input.** Produces a forest of all derivations, structured as a tree of vectors/hiccup/enlive. Each tree node carries `:instaparse.gll/start-index` and `:instaparse.gll/end-index` metadata (and `:start-line :start-column :end-line :end-column` after `add-line-and-column-info`).
4. **Format**. Default hiccup; switch to `:enlive` for Enlive-friendly shapes.

### Lookahead / ordered choice

`:A` (`&A` in PEG) and `:!A` are implemented as **`A` compiled into the GFST** with a flag that says "don't consume". `A / B` is just `A | B` with a priority hint *to the GLL traversal* — the priority is just "try A first", which is enough for common cases. (Author admits it's *not* fully PEG, on purpose.)

### Ambiguity

When the grammar is ambiguous, every successful parse is captured. `parses` is a lazy sequence over all of them. For pathological grammars (e.g., `A = A | 'a'`) this is exponential, but the lazy nature means consumers can `take` only what they want. Most real-world grammars have low ambiguity (≤ handful of parses); if yours doesn't, simplify the grammar.

### Pre-compiled grammar (ClojureScript)

`(insta/parser "S = …")` rebuilds and runs at call-time. For ClojureScript where each rebuild is slow, the `defparser` macro takes the grammar string, calls `insta/parser` *at macro-expansion time*, and emits the *function call* directly:

```clojure
(defparser p "S = 'a'+ 'b'+")
;; macroexpands to a def containing (insta/parser …) pre-computed
;; (also wraps the result in a memoized closure if you pass a string URL)
```

So `(p "aabb")` is just a regular function call, with the parser machinery baked in at compile time. This is why Instaparse is the standard on the JS side.

## API

The main namespace is `instaparse.core`:

```clojure
(require '[instaparse.core :as insta])

;; Build a parser
(def p (insta/parser
         "S = 'a'+ 'b'+"
         :output-format :hiccup       ; or :enlive
         :start :S                    ; default start rule (first rule listed otherwise)
         :auto-whitespace :standard)) ; whitespace handling strategy

;; Run a parser — `p` is callable
(p "aabb")                   ; => [:S "a" "a" "b" "b"]
(insta/parse p "aabb")       ; same thing
(insta/parses p "aabb")      ; => a lazy seq of all derivations
(p "x" :start :A)            ; start rule override
(p "aabb" :partial true)     ; allow unparsed suffix
(p "aabb" :total true)       ; embed failed regions as [:instaparse/failure ...]
(p "aabb" :unhide :content)  ; bypass <...> hiding

;; Transform the tree
(insta/transform
  {:add +, :mul *, :number clojure.edn/read-string}
  parsed-tree)

;; Walk / visualize
(insta/span tree)                                  ; [start-idx end-idx]
(insta/add-line-and-column-info-to-metadata s t)   ; adds line/column metadata
(insta/visualize parsed-tree)                      ; draw tree via rhizome + graphviz

;; Failure handling
(insta/failure? result)     ; true if the result is a Failure or empty seq
(insta/get-failure result)  ; always returns the failure object (or nil)
```

Grammar syntax:

| Syntactic category | Notation |
|---|---|
| End of rule | `;` or `.` (optional) |
| Rule arrow | `:`, `:=`, `::=`, `=` |
| Concatenation | whitespace or `,` |
| Alternation | `\|` |
| Grouping | `( )` |
| Optional | `?` or `[ ]` |
| Zero or more | `*` or `{ }` |
| One or more | `+` |
| String terminal | `"x"` or `'x'` |
| Regex terminal | `#""` or `#''` |
| Epsilon | `Epsilon`, `eps`, `ε`, or empty string |
| Comment | `(* ... *)` |
| Hide content | `<x>` |
| Hide tag | `<Name>` on left-hand-side |
| Lookahead | `&X` |
| Negative lookahead | `!X` |
| Ordered choice | `X / Y` |

## How it works end-to-end (a worked example)

A tokenizer for a tiny expression language that distinguishes keywords from identifiers via negative lookahead:

```clojure
(def tokenizer
  (insta/parser
    "sentence = token (<whitespace> token)*
     <token> = keyword / !keyword identifier
     whitespace = #'\\s+'
     identifier = #'[a-zA-Z]+'
     keyword = 'cond' | 'defn'"))

(tokenizer "defn my cond")
;; => [:sentence [:keyword "defn"] [:identifier "my"] [:keyword "cond"]]
```

What's happening internally:

1. The grammar string is parsed into a `{:sentence ..., :token ..., :whitespace ..., :identifier ..., :keyword ...}` production map.
2. `token` has `<...>` → hides that node from the output.
3. `identifier` and `keyword` both match `[a-zA-Z]+`. `keyword = 'cond' | 'defn'` makes 'cond' both a keyword and an identifier.
4. `!keyword identifier` says: "if this also parses as a keyword, don't call it an identifier".
5. The order is "try keyword first", `!keyword` then frees the matching identifier from picking up keywords.
6. GLL runs on "defn my cond": for each input position, it tries `identifier` first; if successful, checks `keyword`; if it doesn't match keyword, the identifier is OK.

The output is a regular Clojure vector — easy to destructure, walk with `walk/postwalk`, or transform with `clojure.core.match` / `core.match`.

### Arithmetic evaluator, fully self-contained

```clojure
(def arithmetic
  (insta/parser
    "expr = add-sub
     <add-sub> = mul-div | add | sub
     add = add-sub <'+'> mul-div
     sub = add-sub <'-'> mul-div
     <mul-div> = term | mul | div
     mul = mul-div <'*'> term
     div = mul-div <'/'> term
     <term> = number | <'('> add-sub <')'>
     number = #'[0-9]+'"))

(arithmetic "1-2/(3-4)+5*6")
;; [:expr [:add [:sub [:number "1"] [:div [:number "2"] [:sub [:number "3"] [:number "4"]]]]
;;               [:mul [:number "5"] [:number "6"]]]]

(->> (arithmetic "1-2/(3-4)+5*6")
     (insta/transform
       {:add +, :sub -, :mul *, :div /,
        :number clojure.edn/read-string}))
;; => 33
```

A complete arithmetic engine in 15 lines, and the parser is *data* and can be inspected or assembled dynamically.

### Hand-rolled ambigulator (diagnose ambiguity)

```clojure
(insta/parses (insta/parser "S = 'a' 'a'") "aaaa")
;; =>
;; ([:S "a" "a" [:S "a" "a"]]     ;; last two a's grouped
;;  [:S [:S "a" "a"] "a" "a"]     ;; first two a's grouped
;;  … etc)
```

Iterating this against your expected behavior is the idiomatic way to write a parser in Instaparse: build, run, fix grammar if multiple parses appear, switch from `parses` to `parse` for production use.

## Why it matters

- **Any grammar works.** It is the most permissive parser generator in production in the JVM. If you've ever been defeated by an LL(k) conflict or a left-recursive explosion, Instaparse solves it.
- **The grammar is your documentation.** A 5-line EBNF is more readable than 50 lines of combinator code. New developers can read your parser without understanding the parser library.
- **Hiccup output** integrates with the rest of Clojure's value: `clojure.walk`, `clojure.zip`, `clojure.match`, `core.match`, `meander` — all work for free.
- **CLJS story is solved.** `defparser` macro compiles the parser into the build; browser bundle is small (~30 KB).
- **Errors are friendly.** `:total true` + `add-line-and-column-info-to-metadata` produces IDE-quality error messages.

## Caveats and alternatives

- Pure CFGs (not PEG-style) means some styles of grammar are easier than others; e.g., C-style `if/else` ambiguity is not resolved "for free" by Instaparse — you have to rewrite the grammar using ordered choice or a lookahead.
- Ambiguous grammars can explode exponentially. `parses` is your friend for diagnosis but a hazard in production.
- `defparser` only works for string-given grammars. If you build the grammar programmatically (`insta/combinators`), you lose the macro pre-compile benefit.
- **Alternatives**:
  - **`Instaparse-clj` family** (the only Instaparse) — but if you want smaller, there are parser combinators: **parsatron** (CLJ PEG), **kern** (PEG, focused on speed), **parcera** (also PEG).
  - For C-like languages, **lezer-clojure** (a Lezer grammar by the same author) parses into incremental trees suitable for editor tooling.
  - For "I just want a lexer", **clojure.edn/read** is built into Clojure, and **gloss** (binary packet parsers) is purpose-built for byte streams.

## Sources

- <https://github.com/Engelberg/instaparse> (README by Engelberg is the canonical reference; this whole note is largely extracted from it)
- The source code: `instaparse.gll` for the algorithm
- "GLL Parsing" by Adrian Johnstone and Elizabeth Scott (the algorithm)
- Mark Engelberg, "Instaparse: Easy context-free grammars", Clojure/conj 2014 (talks)
