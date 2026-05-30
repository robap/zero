# Plan: `http` — parse JSON on Content-Type-less 2xx responses

## Summary
`runtime/http.js`'s `_readResponse` currently decides whether to parse a body
solely from the `Content-Type` header, so a response with **no** content-type
silently falls into the raw-`Response` branch — a silent-wrong-result footgun,
especially in tests where the fetch shim sets no default content-type. This
plan adds a third "header-less" case (content-type absent or empty) that reads
the body text **once** and attempts `JSON.parse`, resolving with the parsed
value on success and falling back to the raw `Response` (success path) or the
text string (error path) on failure. Explicit content-types — JSON and non-JSON
alike — keep their exact current behavior; the body is never sniffed when a
content-type is present. The change is confined to `_readResponse` plus one
small extracted helper, with new tests, a docs clarification, and a cross-repo
follow-up note.

## Prerequisites
Both spec Open Questions are resolved (confirmed during planning):
- **Requirement 3 (symmetric error-path handling): INCLUDED.** A header-less
  JSON error body resolves as parsed `HttpError.body`; test 5d is in scope.
- **Empty header-less 2xx body: resolves to the raw `Response`** (parse fails →
  fallback), the spec's safer default.

No blocking dependencies on other issues.

## Steps

- [x] **Step 1: Implement header-less JSON parsing in `_readResponse` + tests**
- [x] **Step 2: Update `docs/http.md` JSON I/O section**
- [x] **Step 3: Follow-up — flip FRAMEWORK_NOTES.md L60 (zero_demo repo)**

---

## Step Details

### Step 1: Implement header-less JSON parsing in `_readResponse` + tests
**Goal:** Make `_readResponse` parse JSON for header-less 2xx and non-2xx
responses while leaving every explicit-content-type path byte-for-byte
unchanged. This is the core behavior change; everything else documents it.

**Files:**
- `runtime/http.js` (modify `_readResponse`; add helpers)
- `runtime/http.test.js` (add tests; the existing escape-hatch test stays
  unchanged)

**Changes:**

1. Add a single-consume parse helper. Reading body text exactly once and
   `JSON.parse`-ing it satisfies Requirement 4 (no double-consume against a
   spec-compliant single-use body):

   ```js
   /**
    * Read a response body as text exactly once and try to JSON-parse it.
    * A single consume that is safe against a spec-compliant single-use body.
    * @internal
    * @param {Response} response
    * @returns {Promise<{ parsed: boolean, value: unknown, text: string }>}
    */
   async function _readJsonOrText(response) {
     let text;
     try {
       text = await response.text();
     } catch (_) {
       return { parsed: false, value: undefined, text: "" };
     }
     try {
       return { parsed: true, value: JSON.parse(text), text };
     } catch (_) {
       return { parsed: false, value: undefined, text };
     }
   }
   ```
   (An empty body yields `text === ""`, `JSON.parse("")` throws → `parsed:
   false` → fallback, which is exactly the desired empty-body behavior.)

2. Extract the error-body branch into its own helper to keep both functions
   under ~80 lines and keep the three content-type categories explicit:

   ```js
   /**
    * Build the `HttpError` body for a non-2xx response. An explicit
    * content-type drives json-vs-text as before; a header-less body is
    * parsed-then-fallback (parsed value, else the raw text string).
    * @internal
    * @param {Response} response
    * @param {boolean} isJson
    * @param {boolean} headerLess
    * @returns {Promise<never>}
    */
   async function _throwHttpError(response, isJson, headerLess) {
     let body;
     if (isJson) {
       try { body = await response.json(); } catch (_) { body = undefined; }
     } else if (headerLess) {
       const { parsed, value, text } = await _readJsonOrText(response);
       body = parsed ? value : text;
     } else {
       try { body = await response.text(); } catch (_) { body = undefined; }
     }
     throw new HttpError(response.status, response.statusText, body);
   }
   ```

3. Rewrite `_readResponse` to compute a `headerLess` flag and route to the
   three cases. Note the explicit-JSON path keeps `response.json()` (which
   *rejects* on a malformed JSON body — unchanged), so the parse-then-fallback
   logic touches **only** the header-less case:

   ```js
   /**
    * @internal
    * @param {Response} response
    * @returns {Promise<unknown>}
    */
   async function _readResponse(response) {
     const contentType = response.headers.get("Content-Type") || "";
     const isJson = /\bjson\b/i.test(contentType);
     const headerLess = contentType === "";
     if (!response.ok) {
       return _throwHttpError(response, isJson, headerLess);
     }
     if (isJson) {
       return response.json();
     }
     if (headerLess) {
       const { parsed, value } = await _readJsonOrText(response);
       return parsed ? value : response;
     }
     return response;
   }
   ```

   Key invariants this preserves:
   - Explicit non-JSON content-type (`text/plain`, `application/octet-stream`)
     → `isJson` false, `headerLess` false → returns raw `Response`. No sniffing.
   - Explicit JSON content-type → `response.json()`, unchanged (still rejects on
     bad JSON, never falls back).
   - The three success branches and the three error branches are mutually
     exclusive, so the body is consumed at most once per response.

**Tests** (add to the `describe('zero/http — createHttp', …)` block in
`runtime/http.test.js`, mirroring the existing `makeStubFetch` pattern):

- **(a) header-less 2xx JSON body resolves parsed data.**
  `new Response(JSON.stringify({ value: 42 }), { status: 200 })` (no `headers`)
  → `expect(body).toEqual({ value: 42 })`.
- **(b) header-less 2xx non-JSON/garbage body falls back to the raw Response.**
  `new Response('not json at all', { status: 200 })` → `result instanceof
  Response` truthy and `await result.text()` is `'not json at all'`.
- **(b') header-less 2xx empty body falls back to the raw Response.**
  `new Response('', { status: 200 })` (or `new Response(undefined, { status: 200
  })`) → `result instanceof Response` truthy. Locks the resolved empty-body
  decision.
- **(c) existing escape-hatch test unchanged.** `http.test.js:175`
  (`application/octet-stream`) must still pass with no edits — call it out, do
  not modify it.
- **(d) header-less non-2xx JSON error body surfaces as parsed
  `HttpError.body`.** `new Response(JSON.stringify({ message: 'nope' }), {
  status: 404, statusText: 'Not Found' })` (no `headers`) → caught `HttpError`
  with `err.status === 404` and `err.body` `toEqual({ message: 'nope' })`.
  (Optionally also assert a header-less non-2xx *non-JSON* body yields the raw
  text string as `err.body`, exercising the error-path fallback.)

Run `cargo run -p zero -- test http.test.js` (and `cargo test --workspace` for
the Rust harness) to confirm green.

---

### Step 2: Update `docs/http.md` JSON I/O section
**Goal:** Make the user-facing parse contract match the new behavior without
disturbing the explicit-content-type description.

**Files:** `docs/http.md` (the "JSON I/O" section, lines ~58-68).

**Changes:** Revise the sentence at lines 58-60 ("Responses with a JSON
`Content-Type` are parsed before the promise resolves; other responses return
the raw `Response` object so you can stream binary, text, etc.") to additionally
state that a response with **no `Content-Type` at all** is parsed as JSON when
the body parses, and otherwise returned as the raw `Response`. Keep the existing
explicit-content-type contract intact and the surrounding code examples (lines
62-68) unchanged. Suggested wording:

> Responses with a JSON `Content-Type` are parsed before the promise resolves;
> responses with an explicit non-JSON `Content-Type` return the raw `Response`
> object so you can stream binary, text, etc. A response with **no
> `Content-Type` header** is parsed as JSON when the body parses, and otherwise
> falls back to the raw `Response`.

**Tests:** None (docs only). No `d.ts` change needed — `runtime/zero-http.d.ts`
describes the generic `Promise<T>` surface, not content-type routing, and the
contract there is unaffected. Per the
[avoid-overvalidating-simple-changes] memory, ship after a sanity read rather
than spinning up a docs build.

---

### Step 3: Follow-up — flip FRAMEWORK_NOTES.md L60 (zero_demo repo)
**Goal:** Close the friction-log entry that flagged this footgun, per the log's
"How to mark an entry fixed" convention.

**Files:** `FRAMEWORK_NOTES.md` L60 — **in the separate `zero_demo` repository,
not this one.** This step cannot be completed from within `code/zero`.

**Changes:** Flip the entry from `- [ ]` to `- [x]` and add a `**FIXED
YYYY-MM-DD**` annotation (use today's date when executed) summarizing the
resolution: header-less 2xx/error responses now attempt JSON parse and fall
back to the raw `Response`/text. Follow the exact convention documented in that
file.

**Tests:** None. This is a cross-repo bookkeeping follow-up; flag it to the user
and execute it in the `zero_demo` working tree, or leave it for the user to
apply there.

---

## Risks and Assumptions
- **Consumed-body fallback.** In the header-less *fallback* cases (garbage/empty
  body on the success path), the helper has already consumed `response.text()`
  before returning the raw `Response`. The fetch shim allows re-reading, but a
  spec-compliant single-use body would be exhausted. This matches the spec's
  explicit decision (Requirement 4 sanctions reading text once; Open Question 2
  accepts raw `Response` on empty/unparseable bodies) and only affects the
  already-degenerate "couldn't parse" path — the parsed-success path returns the
  value, not the response. Documented as an accepted tradeoff.
- **`headerLess` defined as `contentType === ""`.** This deliberately treats
  only an absent or empty header as ambiguous; any non-empty explicit
  content-type (including non-JSON) keeps today's behavior. If a server sends a
  whitespace-only content-type it would be treated as explicit non-JSON, not
  header-less — an acceptable and unlikely edge.
- **Explicit-JSON path unchanged on purpose.** `response.json()` still rejects
  on malformed JSON for an explicit `application/json` response; the new
  fallback is intentionally *not* applied there, preserving byte-for-byte
  behavior (Constraint: "explicit content-types … byte-for-byte unchanged"). If
  a reviewer expects explicit-JSON to also fall back, that would contradict the
  spec and require replanning.
- **Cross-repo Step 3** depends on access to the `zero_demo` checkout; if it's
  unavailable the runtime fix still stands on its own and the note can be
  applied later.
