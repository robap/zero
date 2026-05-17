import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { document, window } from "./dom-shim.js";
import { signal } from "./reactivity.js";
import { html } from "./template.js";
import { inject } from "./app.js";
import {
  describe as zeroDescribe,
  it as zeroIt,
  beforeEach as zeroBeforeEach,
  afterEach as zeroAfterEach,
  beforeAll as zeroBeforeAll,
  afterAll as zeroAfterAll,
  expect as zeroExpect,
  render,
  find,
  findAll,
  text,
  fire,
  cleanup,
  spy as zeroSpy,
  __getTestTree__,
  __resetTestTree__,
} from "./test.js";

// Ensure a clean env before the suite
beforeEach(() => {
  __resetTestTree__();
  document.childNodes.length = 0;
  document._listeners.clear();
  window._listeners.clear();
});

describe("test tree structure", () => {
  it("__getTestTree__ returns root with empty name", () => {
    const tree = __getTestTree__();
    assert.equal(tree.name, "");
  });

  it("describe nests; it registers; tree has expected shape", () => {
    zeroDescribe("outer", () => {
      zeroIt("test1", () => {});
      zeroDescribe("inner", () => {
        zeroIt("test2", () => {});
      });
    });
    const root = __getTestTree__();
    assert.equal(root.children.length, 1);
    const outer = root.children[0];
    assert.equal(outer.name, "outer");
    assert.equal(outer.children.length, 2);
    assert.equal(outer.children[0].name, "test1");
    const inner = outer.children[1];
    assert.equal(inner.name, "inner");
    assert.equal(inner.children.length, 1);
    assert.equal(inner.children[0].name, "test2");
  });

  it("hook arrays accumulate in registration order", () => {
    const calls = [];
    zeroDescribe("g", () => {
      zeroBeforeAll(() => calls.push("ba1"));
      zeroBeforeAll(() => calls.push("ba2"));
      zeroBeforeEach(() => calls.push("be1"));
      zeroAfterEach(() => calls.push("ae1"));
      zeroAfterAll(() => calls.push("aa1"));
    });
    const root = __getTestTree__();
    const g = root.children[0];
    assert.equal(g.beforeAll.length, 2);
    assert.equal(g.beforeEach.length, 1);
    assert.equal(g.afterEach.length, 1);
    assert.equal(g.afterAll.length, 1);
    // Confirm ordering by calling them
    for (const fn of g.beforeAll) fn();
    assert.deepEqual(calls, ["ba1", "ba2"]);
  });
});

describe("expect matchers", () => {
  it("toBe passes on strict equality, throws on mismatch with actual and expected in message", () => {
    assert.doesNotThrow(() => zeroExpect(1).toBe(1));
    assert.throws(
      () => zeroExpect(1).toBe(2),
      err => {
        assert.ok(err.message.includes("1"), "message should contain actual");
        assert.ok(err.message.includes("2"), "message should contain expected");
        return true;
      },
    );
  });

  it("toEqual passes on deep equality, throws on mismatch", () => {
    assert.doesNotThrow(() => zeroExpect({ a: 1 }).toEqual({ a: 1 }));
    assert.throws(
      () => zeroExpect({ a: 1 }).toEqual({ a: 2 }),
      /deeply equal/,
    );
  });

  it("toContain passes for array contains, throws if missing", () => {
    assert.doesNotThrow(() => zeroExpect([1, 2, 3]).toContain(2));
    assert.throws(() => zeroExpect([1, 2, 3]).toContain(4), /does not contain/);
  });

  it("toThrow passes when function throws with matching message", () => {
    assert.doesNotThrow(() =>
      zeroExpect(() => { throw new Error("boom"); }).toThrow("boom"),
    );
    assert.throws(
      () => zeroExpect(() => { throw new Error("boom"); }).toThrow("nope"),
      /does not contain/,
    );
  });

  it("toEqual with signal-shaped objects compares .val", () => {
    const a = signal(0);
    const b = signal(0);
    assert.doesNotThrow(() => zeroExpect(a).toEqual(b));
    b.set(1);
    assert.throws(() => zeroExpect(a).toEqual(b), /deeply equal/);
  });

  it("toBeTemplateResult passes for html`` result", () => {
    assert.doesNotThrow(() => zeroExpect(html`<p>x</p>`).toBeTemplateResult());
  });

  it("toMatchSnapshot throws with deferred-feature message", () => {
    assert.throws(
      () => zeroExpect(42).toMatchSnapshot(),
      /snapshot testing is not in this slice yet/,
    );
  });

  it("matcher error carries _userFrame pointing at the assertion call site", () => {
    let captured;
    try {
      zeroExpect(1).toBe(2);
    } catch (e) {
      captured = e;
    }
    assert.ok(captured, "expected toBe(2) to throw");
    assert.ok(
      typeof captured._userFrame === "string" && captured._userFrame.length > 0,
      `expected _userFrame string, got: ${captured && captured._userFrame}`,
    );
    assert.ok(
      /test\.test\.js:\d+:\d+$/.test(captured._userFrame),
      `_userFrame should point at this test file: ${captured._userFrame}`,
    );
  });
});

describe("DOM helpers", () => {
  it("render returns a container element holding all rendered children", () => {
    const el = render(html`<p>hi</p>`);
    assert.ok(el != null, "render should return a container");
    assert.ok(find(el, "p") != null, "container should contain the <p>");
    assert.equal(text(el), "hi");
  });

  it("render with opts.state: inject resolves the registered value", () => {
    const count = signal(42);
    // inject is called lazily inside the reactive closure so it runs
    // after render() installs the stub app via _setCurrentApp
    const Component = () => html`<span>${() => inject("count").val}</span>`;
    const el = render(Component(), { state: { count } });
    assert.ok(el != null);
    assert.ok(find(el, "span") != null, "container should contain the <span>");
    assert.equal(text(el), "42");
  });

  it("cleanup disposes scopes so effects no longer fire after cleanup()", () => {
    const counter = signal(0);
    let renderCount = 0;
    render(html`<span>${() => { renderCount++; return counter.val; }}</span>`);
    const countBefore = renderCount;
    cleanup();
    counter.set(99);
    // After cleanup, the effect should be disposed and not fire
    assert.equal(renderCount, countBefore, "effect should not fire after cleanup");
  });

  it("fire dispatches event and handler is called", () => {
    let clicked = false;
    const el = render(html`<button @click=${() => { clicked = true; }}>click</button>`);
    fire(find(el, "button"), "click");
    assert.ok(clicked, "click handler should have been called");
  });
});

describe("selector grammar", () => {
  afterEach(cleanup);

  it("empty selector throws /empty selector/", () => {
    const el = render(html`<div><span></span></div>`);
    assert.throws(() => find(el, ""), /empty selector/);
  });

  it("class selector matches element with that class", () => {
    const el = render(html`<div><span class="foo">a</span></div>`);
    assert.ok(find(el, ".foo") != null);
  });

  it("class selector matches element with multiple classes in class attribute", () => {
    const el = render(html`<div><span class="foo bar">a</span></div>`);
    assert.ok(find(el, ".foo") != null);
  });

  it("class selector returns null when class not present", () => {
    const el = render(html`<div><span class="bar">a</span></div>`);
    assert.equal(find(el, ".foo"), null);
  });

  it("multi-class: .foo.bar requires both classes", () => {
    const el = render(html`<div><span class="foo bar">a</span><span class="foo">b</span></div>`);
    const result = find(el, ".foo.bar");
    assert.ok(result != null);
    assert.equal(text(result), "a");
  });

  it("multi-class: .foo.bar and .bar.foo both match class='foo bar'", () => {
    const el = render(html`<div><span class="foo bar">a</span></div>`);
    assert.ok(find(el, ".foo.bar") != null);
    assert.ok(find(el, ".bar.foo") != null);
  });

  it("attribute existence: [data-x] matches when attribute is set", () => {
    const el = render(html`<div><span data-x="anything">a</span></div>`);
    assert.ok(find(el, "[data-x]") != null);
  });

  it("attribute existence: [data-x] matches when attribute value is empty string", () => {
    const container = render(html`<div><span>a</span></div>`);
    find(container, "span").setAttribute("data-x", "");
    assert.ok(find(container, "[data-x]") != null);
  });

  it("attribute equality unquoted: [data-x=y]", () => {
    const el = render(html`<div><span data-x="y">a</span></div>`);
    assert.ok(find(el, "[data-x=y]") != null);
    assert.equal(find(el, "[data-x=z]"), null);
  });

  it("attribute equality double-quoted: [data-x=\"y z\"]", () => {
    const container = render(html`<div><span>a</span></div>`);
    find(container, "span").setAttribute("data-x", "y z");
    assert.ok(find(container, '[data-x="y z"]') != null);
  });

  it("attribute equality single-quoted: [data-x='y z']", () => {
    const container = render(html`<div><span>a</span></div>`);
    find(container, "span").setAttribute("data-x", "y z");
    assert.ok(find(container, "[data-x='y z']") != null);
  });

  it("attribute name is case-insensitive: [DATA-X=y] matches data-x=\"y\"", () => {
    const el = render(html`<div><span data-x="y">a</span></div>`);
    assert.ok(find(el, "[DATA-X=y]") != null);
  });

  it("compound tag+class+attr: button.btn[type=submit] matches correct element", () => {
    const el = render(html`<div><button class="btn" type="submit">ok</button><button class="btn">no</button></div>`);
    const result = find(el, "button.btn[type=submit]");
    assert.ok(result != null);
    assert.equal(text(result), "ok");
  });

  it("compound tag+id+class+attr-existence: a#home.nav-link[data-active]", () => {
    const el = render(html`<div><a id="home" class="nav-link" data-active="true">home</a><a id="home" class="nav-link">other</a></div>`);
    const result = find(el, "a#home.nav-link[data-active]");
    assert.ok(result != null);
    assert.equal(text(result), "home");
  });

  it("findAll returns all class matches in document order", () => {
    const el = render(html`<div><span class="x">1</span><p class="x">2</p><span class="y">3</span></div>`);
    const results = findAll(el, ".x");
    assert.equal(results.length, 2);
    assert.equal(text(results[0]), "1");
    assert.equal(text(results[1]), "2");
  });

  it("closest with compound selector walks up the tree", () => {
    const el = render(html`<div class="box outer"><div class="inner"><span>leaf</span></div></div>`);
    const span = find(el, "span");
    const box = span.closest("div.box");
    assert.ok(box != null);
    assert.ok(box.getAttribute("class").includes("outer"));
  });

  it("malformed: leading whitespace throws /malformed selector/", () => {
    const el = render(html`<div><span></span></div>`);
    assert.throws(() => find(el, " a"), err => {
      assert.ok(err.message.includes(" a"));
      assert.ok(err.message.includes("position"));
      return true;
    });
  });

  it("malformed: unclosed bracket throws /malformed selector/", () => {
    const el = render(html`<div><span></span></div>`);
    assert.throws(() => find(el, ".foo["), /malformed selector/);
  });

  it("malformed: duplicate id throws /malformed selector/", () => {
    const el = render(html`<div><span></span></div>`);
    assert.throws(() => find(el, "##id"), /malformed selector/);
  });

  it("regression: tag-only selector still works", () => {
    const el = render(html`<div><a>link</a></div>`);
    assert.ok(find(el, "a") != null);
  });

  it("regression: #id selector still works", () => {
    const el = render(html`<div><span id="nav">nav</span></div>`);
    assert.ok(find(el, "#nav") != null);
  });
});

describe("spy primitive", () => {
  it("spy() returns a function and calling it returns undefined; .calls and .callCount track invocations", () => {
    const s = zeroSpy();
    assert.equal(typeof s, "function");
    assert.equal(s(), undefined);
    assert.equal(s.callCount, 1);
    assert.deepEqual(s.calls[0], []);
  });

  it("spy(fn) calls through to fn, records return value in .results", () => {
    const s = zeroSpy((x) => x * 2);
    const result = s(5);
    assert.equal(result, 10);
    assert.deepEqual(s.calls[0], [5]);
    assert.deepEqual(s.results[0], { type: "return", value: 10 });
  });

  it("spy(fn) rethrows when fn throws and records a throw result", () => {
    const err = new Error("boom");
    const s = zeroSpy(() => { throw err; });
    assert.throws(() => s(), /boom/);
    assert.deepEqual(s.results[0], { type: "throw", value: err });
    assert.equal(s.callCount, 1);
  });

  it(".mockReturnValue(v) overrides impl; subsequent calls return v and .calls still grows", () => {
    const s = zeroSpy((x) => x);
    s.mockReturnValue(42);
    assert.equal(s("ignored"), 42);
    assert.equal(s.callCount, 1);
    assert.deepEqual(s.calls[0], ["ignored"]);
  });

  it(".mockResolvedValue(v) makes subsequent calls return a promise resolving to v", async () => {
    const s = zeroSpy();
    s.mockResolvedValue("ok");
    const result = await s();
    assert.equal(result, "ok");
  });

  it(".mockRejectedValue(e) makes subsequent calls return a promise rejecting with e", async () => {
    const s = zeroSpy();
    const err = new Error("nope");
    s.mockRejectedValue(err);
    await assert.rejects(s(), (e) => { assert.equal(e, err); return true; });
  });

  it(".mockImplementation(fn2) replaces impl; .calls accumulates across impl swaps", () => {
    const s = zeroSpy(() => "first");
    s("a");
    s.mockImplementation(() => "second");
    s("b");
    assert.equal(s.callCount, 2);
    assert.deepEqual(s.calls[0], ["a"]);
    assert.deepEqual(s.calls[1], ["b"]);
    assert.equal(s.results[0].value, "first");
    assert.equal(s.results[1].value, "second");
  });

  it(".reset() clears calls/results/instances but keeps implementation", () => {
    const s = zeroSpy(() => "impl");
    s("before");
    s.reset();
    assert.equal(s.callCount, 0);
    assert.equal(s.results.length, 0);
    assert.equal(s.instances.length, 0);
    const r = s("after");
    assert.equal(r, "impl");
    assert.equal(s.callCount, 1);
  });

  it("this-binding is recorded in .instances", () => {
    const obj = {};
    obj.method = zeroSpy();
    obj.method();
    assert.equal(obj.method.instances[0], obj);
  });
});

describe("spy matchers", () => {
  it("toHaveBeenCalled passes after one invocation", () => {
    const s = zeroSpy();
    s();
    assert.doesNotThrow(() => zeroExpect(s).toHaveBeenCalled());
  });

  it("toHaveBeenCalled throws /spy was not called/ on fresh spy", () => {
    const s = zeroSpy();
    assert.throws(() => zeroExpect(s).toHaveBeenCalled(), /spy was not called/);
  });

  it("toHaveBeenCalledTimes(2) passes after exactly two calls", () => {
    const s = zeroSpy();
    s();
    s();
    assert.doesNotThrow(() => zeroExpect(s).toHaveBeenCalledTimes(2));
  });

  it("toHaveBeenCalledTimes fails off-by-one with expected and actual in message", () => {
    const s = zeroSpy();
    s();
    assert.throws(() => zeroExpect(s).toHaveBeenCalledTimes(2), err => {
      assert.ok(err.message.includes("2"), "message should contain expected n");
      assert.ok(err.message.includes("1"), "message should contain actual callCount");
      return true;
    });
  });

  it("toHaveBeenCalledWith(a,b) passes when any recorded call matches", () => {
    const s = zeroSpy();
    s(1, 2);
    s(3, 4);
    s(5, 6);
    assert.doesNotThrow(() => zeroExpect(s).toHaveBeenCalledWith(3, 4));
  });

  it("toHaveBeenCalledWith fails with recorded calls in message when no match", () => {
    const s = zeroSpy();
    s(1, 2);
    assert.throws(() => zeroExpect(s).toHaveBeenCalledWith(9, 9), err => {
      assert.ok(err.message.includes("1") || err.message.includes("recorded calls"), "message should include call info");
      return true;
    });
  });

  it("toHaveBeenCalledWith uses deep equality: {a:1} matches recorded {a:1}", () => {
    const s = zeroSpy();
    s({ a: 1 });
    assert.doesNotThrow(() => zeroExpect(s).toHaveBeenCalledWith({ a: 1 }));
  });

  it("toHaveBeenLastCalledWith passes for the last call only", () => {
    const s = zeroSpy();
    s(1);
    s(2);
    assert.doesNotThrow(() => zeroExpect(s).toHaveBeenLastCalledWith(2));
  });

  it("toHaveBeenLastCalledWith fails when earlier (not last) call matches", () => {
    const s = zeroSpy();
    s(1);
    s(2);
    assert.throws(() => zeroExpect(s).toHaveBeenLastCalledWith(1), /last call did not match/);
  });

  it("all matchers throw /value is not a spy/ when given a non-spy", () => {
    assert.throws(() => zeroExpect(42).toHaveBeenCalled(), /value is not a spy/);
    assert.throws(() => zeroExpect(42).toHaveBeenCalledTimes(1), /value is not a spy/);
    assert.throws(() => zeroExpect(42).toHaveBeenCalledWith(), /value is not a spy/);
    assert.throws(() => zeroExpect(42).toHaveBeenLastCalledWith(), /value is not a spy/);
  });
});
