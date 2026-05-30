# Spec: `http` — parse JSON on Content-Type-less 2xx responses

## Problem Statement

`zero/http` decides whether to parse a response body purely from the
response's `Content-Type` header: a JSON content-type is parsed and the
data resolves the promise; anything else returns the raw `Response`
object (a documented escape hatch for binary/text streaming). When a 2xx
response carries **no `Content-Type` header at all**, it falls into the
raw-`Response` branch. The caller, expecting parsed data, then reads
fields off a `Response` instance — every one is `undefined`, the promise
having resolved in `kind === "ok"`. The failure is silent: no error, no
warning, just "expected 1, got undefined" further downstream.

This bites hardest in tests. The runtime fetch shim's `Response`
(`runtime/fetch-shim.js`) sets no default `Content-Type` (a real browser
defaults a string-body `Response` to `text/plain;charset=UTF-8`), so the
natural test fixture `new Response(JSON.stringify(body), { status: 200 })`
— no `headers` — silently produces undefined fields. The friction log
(FRAMEWORK_NOTES.md L60) flagged it after a debugging session chasing
exactly this dead end. It matters now because the demo app and its tests
are the framework's real-world proving ground, and a silent-wrong-result
footgun in the HTTP client is the highest-cost class of bug to rediscover.

## Background

- `runtime/http.js` `_readResponse(response)` is the single decision
  point. It computes `contentType = response.headers.get("Content-Type")
  || ""` and `isJson = /\bjson\b/i.test(contentType)`. For `response.ok`:
  `isJson ? response.json() : response` (the raw-Response branch). For
  `!response.ok`: it builds an `HttpError` whose `body` is
  `response.json()` when `isJson`, else `response.text()`.
- The raw-`Response`-for-non-JSON behavior is **intentional and
  documented** (docs/http.md lines 58-68: "other responses return the raw
  `Response` object so you can stream binary, text, etc.") and **locked by
  test** (`runtime/http.test.js:175`, "non-JSON 2xx response returns the
  raw Response object (escape hatch)", using an explicit
  `Content-Type: application/octet-stream`). Any change must preserve this:
  an explicit non-JSON content-type still returns the raw `Response`.
- The only ambiguous case is a response with an **absent or empty**
  `Content-Type`. That is the sole target of this change.
- The fetch shim `Response` constructor (`runtime/fetch-shim.js:405`)
  stores the body verbatim and initializes `Headers` from `init.headers`
  only — no default content-type. This is left unchanged; the client
  handles the header-less case gracefully instead.
- `runtime/zero-http.d.ts` and `docs/http.md` are the user-facing surface
  that describe the parse contract.

## Requirements

1. In `_readResponse`, when `response.ok` and the `Content-Type` is
   **absent or empty** (not merely non-JSON), attempt to parse the body as
   JSON: on success, resolve with the parsed value; on parse failure
   (including an empty body), fall back to returning the raw `Response`.
2. When `response.ok` and an **explicit** `Content-Type` is present:
   - JSON content-type → parse (unchanged).
   - Any non-JSON content-type (e.g. `text/plain`, `application/octet-stream`)
     → return the raw `Response` (unchanged escape hatch). The body is
     **not** sniffed; an explicit non-JSON content-type is honored as-is.
3. Apply the same absent/empty-Content-Type, parse-then-fallback logic
   symmetrically to the `!response.ok` error-body path so that a
   header-less JSON error body resolves as parsed `HttpError.body` rather
   than a raw text string. An explicit content-type continues to drive
   json-vs-text exactly as today.
4. Reading the body must be done once per response (no double-consume).
   The shim allows re-reading, but the implementation must not rely on
   that — read the text once and `JSON.parse` it in the fallback path, or
   otherwise guarantee a single consume that works against a spec-compliant
   single-use body.
5. New/updated tests in `runtime/http.test.js` covering: (a) header-less
   2xx JSON body now resolves parsed data; (b) header-less 2xx
   non-JSON/garbage body falls back to the raw `Response`; (c) the existing
   explicit-`application/octet-stream` escape-hatch test still passes
   unchanged; (d) header-less non-2xx JSON error body surfaces as parsed
   `HttpError.body`.
6. Update `docs/http.md` (the "JSON I/O" section, lines ~58-68) to state
   that a response with no `Content-Type` is parsed as JSON when possible
   and otherwise returned raw — keeping the explicit-content-type contract
   description intact.
7. Flip FRAMEWORK_NOTES.md L60 from `- [ ]` to `- [x]` with a
   `**FIXED YYYY-MM-DD**` annotation summarizing the resolution, per the
   log's "How to mark an entry fixed" convention. (This is in the
   `zero_demo` repo, not this one — note it as a follow-up step.)

## Constraints

- Preserve the documented raw-`Response` escape hatch for any explicit
  non-JSON content-type; `http.test.js:175` must pass without edits.
- Confine the runtime change to `_readResponse` in `runtime/http.js`. Do
  not change the fetch shim's `Response` default headers.
- All JS stays fully JSDoc-annotated; keep functions under ~80 lines
  (extracting a small helper for "parse body, fall back" is acceptable and
  likely cleaner than inlining into both branches).
- No new dependencies; this is a zero-dependency runtime.
- Behavior for explicit content-types (JSON and non-JSON alike) is
  byte-for-byte unchanged.

## Out of Scope

- Body-sniffing when an explicit non-JSON content-type is present (the
  rejected "parse anything starting with `{`/`[`" option).
- Throwing an error for header-less responses (the rejected "loud error"
  option).
- Changing the fetch shim's `Response` to emit a default `Content-Type`
  (the rejected "fix the test shim" option) — real fetch defaults string
  bodies to `text/plain`, which would not help and would diverge further.
- Streaming/`blob`/`arrayBuffer` parsing or any change to the binary
  escape hatch beyond what's stated.
- Middleware, abort-signal threading, request-side JSON encoding — all
  untouched.

## Open Questions

- Requirement 3 (symmetric handling on the error-body path) is included as
  the consistent choice. If the intent is to scope this strictly to the
  success path, drop requirement 3 and its test (5d) and leave the
  `!response.ok` branch as-is.
- On a header-less 2xx with an **empty** body (e.g. a 200 with no content),
  the spec resolves with the raw `Response` (parse fails → fallback). If a
  resolved `undefined`/`null` is preferred for that case, call it out
  during planning — but raw `Response` is the safer default and matches the
  "couldn't parse → here's the response" mental model.
