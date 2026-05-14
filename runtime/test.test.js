import { describe, it, beforeEach } from "node:test";
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
