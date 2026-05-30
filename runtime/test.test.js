import {
  describe,
  it,
  expect,
  afterEach,
  render,
  find,
  findAll,
  text,
  fire,
  cleanup,
  spy,
} from "zero/test";
import { signal, html, inject, effect } from "zero";

describe("expect matchers", () => {
  it("toBe passes on strict equality", () => {
    expect(1).toBe(1);
  });

  it("toBe throws on mismatch with actual and expected in message", () => {
    let caught;
    try { expect(1).toBe(2); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("1");
    expect(caught.message).toContain("2");
  });

  it("toEqual passes on deep equality", () => {
    expect({ a: 1 }).toEqual({ a: 1 });
  });

  it("toEqual throws on deep mismatch", () => {
    let caught;
    try { expect({ a: 1 }).toEqual({ a: 2 }); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("deeply equal");
  });

  it("toContain passes for array contains", () => {
    expect([1, 2, 3]).toContain(2);
  });

  it("toContain throws when item missing from array", () => {
    let caught;
    try { expect([1, 2, 3]).toContain(4); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("does not contain");
  });

  it("toThrow passes when function throws with matching message", () => {
    expect(() => { throw new Error("boom"); }).toThrow("boom");
  });

  it("toThrow throws when message does not match", () => {
    let caught;
    try {
      expect(() => { throw new Error("boom"); }).toThrow("nope");
    } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("does not contain");
  });

  it("toEqual with signal-shaped objects compares .val", () => {
    const a = signal(0);
    const b = signal(0);
    expect(a).toEqual(b);
    b.set(1);
    let caught;
    try { expect(a).toEqual(b); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("deeply equal");
  });

  it("toBeTemplateResult passes for html`` result", () => {
    expect(html`<p>x</p>`).toBeTemplateResult();
  });

  it("toMatchSnapshot throws with deferred-feature message", () => {
    let caught;
    try { expect(42).toMatchSnapshot(); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("snapshot testing is not in this slice yet");
  });

  it("toBeUndefined passes when actual is undefined", () => {
    expect(undefined).toBeUndefined();
  });

  it("toBeUndefined throws when actual is null", () => {
    let caught;
    try { expect(null).toBeUndefined(); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("toBeUndefined");
    expect(caught.message).toContain("is not undefined");
  });

  it("toBeUndefined throws for 0, '', false", () => {
    for (const v of [0, "", false]) {
      let caught;
      try { expect(v).toBeUndefined(); } catch (e) { caught = e; }
      expect(caught).toBeTruthy();
      expect(caught.message).toContain("is not undefined");
    }
  });

  it("toBeDefined passes for null, 0, '', false, object, number", () => {
    expect(null).toBeDefined();
    expect(0).toBeDefined();
    expect("").toBeDefined();
    expect(false).toBeDefined();
    expect({}).toBeDefined();
    expect(1).toBeDefined();
  });

  it("toBeDefined throws when actual is undefined", () => {
    let caught;
    try { expect(undefined).toBeDefined(); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("toBeDefined");
    expect(caught.message).toContain("is undefined");
  });
});

describe(".not chain", () => {
  it(".not.toBe passes when values differ", () => {
    expect(1).not.toBe(2);
  });

  it(".not.toBe throws when values are strictly equal", () => {
    let caught;
    try { expect(1).not.toBe(1); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("strictly equal");
  });

  it(".not.toEqual passes when values are not deeply equal", () => {
    expect({ a: 1 }).not.toEqual({ a: 2 });
  });

  it(".not.toEqual throws when values are deeply equal", () => {
    let caught;
    try { expect({ a: 1 }).not.toEqual({ a: 1 }); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("deeply equal");
  });

  it(".not.toBeNull passes for a non-null value", () => {
    expect(0).not.toBeNull();
  });

  it(".not.toBeNull throws when value is null", () => {
    let caught;
    try { expect(null).not.toBeNull(); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("null");
  });

  it(".not.toBeTruthy passes for a falsy value", () => {
    expect(0).not.toBeTruthy();
  });

  it(".not.toBeFalsy passes for a truthy value", () => {
    expect(1).not.toBeFalsy();
  });

  it(".not.toContain passes when string lacks substring", () => {
    expect("hello").not.toContain("xyz");
  });

  it(".not.toContain throws when array contains item", () => {
    let caught;
    try { expect([1, 2, 3]).not.toContain(2); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("contains");
  });

  it(".not.toThrow passes when fn does not throw", () => {
    expect(() => {}).not.toThrow();
  });

  it(".not.toThrow throws when fn throws", () => {
    let caught;
    try { expect(() => { throw new Error("boom"); }).not.toThrow(); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("threw");
  });

  it(".not.toThrow(msg) passes when message does not match", () => {
    expect(() => { throw new Error("boom"); }).not.toThrow("nope");
  });

  it(".not.toThrow(msg) throws when message matches", () => {
    let caught;
    try { expect(() => { throw new Error("boom"); }).not.toThrow("boom"); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
  });

  it(".not.toBeTemplateResult passes for plain object", () => {
    expect({}).not.toBeTemplateResult();
  });

  it(".not.toBeTemplateResult throws for html`` result", () => {
    let caught;
    try { expect(html`<p>x</p>`).not.toBeTemplateResult(); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
  });

  it(".not.toHaveBeenCalled passes for fresh spy", () => {
    const s = spy();
    expect(s).not.toHaveBeenCalled();
  });

  it(".not.toHaveBeenCalled throws when spy was called", () => {
    const s = spy();
    s();
    let caught;
    try { expect(s).not.toHaveBeenCalled(); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("was called");
  });

  it(".not.toHaveBeenCalledTimes passes when count differs", () => {
    const s = spy();
    s();
    expect(s).not.toHaveBeenCalledTimes(2);
  });

  it(".not.toHaveBeenCalledWith passes when no recorded call matches", () => {
    const s = spy();
    s(1, 2);
    expect(s).not.toHaveBeenCalledWith(9, 9);
  });

  it(".not.toHaveBeenCalledWith throws when a recorded call matches", () => {
    const s = spy();
    s(1, 2);
    let caught;
    try { expect(s).not.toHaveBeenCalledWith(1, 2); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
  });

  it(".not.toHaveBeenLastCalledWith passes when last call differs", () => {
    const s = spy();
    s(1);
    s(2);
    expect(s).not.toHaveBeenLastCalledWith(1);
  });

  it(".not.toBeUndefined passes for a non-undefined value", () => {
    expect(null).not.toBeUndefined();
  });

  it(".not.toBeUndefined throws when value is undefined", () => {
    let caught;
    try { expect(undefined).not.toBeUndefined(); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("is undefined");
  });

  it(".not.toBeDefined passes for undefined", () => {
    expect(undefined).not.toBeDefined();
  });

  it(".not.toBeDefined throws for a defined value", () => {
    let caught;
    try { expect(0).not.toBeDefined(); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("is defined");
  });

  it("negation failure error has the same _userFrame shape as positive matchers", () => {
    // The harness derives location from Boa's shadow stack, not `_userFrame`
    // (Boa doesn't populate `Error.stack`). The decoration is still applied
    // so the property exists on negation errors with the same shape as
    // positive-matcher errors — same path through `_fail`.
    let pos;
    try { expect(1).toBe(2); } catch (e) { pos = e; }
    let neg;
    try { expect(1).not.toBe(1); } catch (e) { neg = e; }
    expect("_userFrame" in pos).toBe(true);
    expect("_userFrame" in neg).toBe(true);
    expect(typeof neg._userFrame).toBe(typeof pos._userFrame);
  });
});

describe("numeric matchers", () => {
  it("toBeGreaterThan passes when actual > n", () => {
    expect(5).toBeGreaterThan(3);
  });

  it("toBeGreaterThan throws when actual === n", () => {
    let caught;
    try { expect(3).toBeGreaterThan(3); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("not greater than");
  });

  it("toBeGreaterThan throws when actual < n", () => {
    let caught;
    try { expect(1).toBeGreaterThan(3); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
  });

  it("toBeGreaterThanOrEqual passes at the boundary", () => {
    expect(3).toBeGreaterThanOrEqual(3);
  });

  it("toBeGreaterThanOrEqual throws when actual < n", () => {
    let caught;
    try { expect(2).toBeGreaterThanOrEqual(3); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
  });

  it("toBeLessThan passes when actual < n", () => {
    expect(1).toBeLessThan(3);
  });

  it("toBeLessThan throws at the boundary", () => {
    let caught;
    try { expect(3).toBeLessThan(3); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
  });

  it("toBeLessThanOrEqual passes at the boundary", () => {
    expect(3).toBeLessThanOrEqual(3);
  });

  it("toBeLessThanOrEqual throws when actual > n", () => {
    let caught;
    try { expect(4).toBeLessThanOrEqual(3); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
  });

  it("toBeGreaterThan throws when actual is not a number", () => {
    let caught;
    try { expect("hi").toBeGreaterThan(0); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("not a number");
  });

  it("toBeGreaterThan throws when argument is not a number", () => {
    let caught;
    try { expect(5).toBeGreaterThan("0"); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("not a number");
  });

  it(".not.toBeGreaterThan passes when actual <= n", () => {
    expect(3).not.toBeGreaterThan(3);
  });

  it(".not.toBeLessThan passes at the boundary", () => {
    expect(3).not.toBeLessThan(3);
  });
});

describe("DOM helpers", () => {
  afterEach(cleanup);

  it("render returns a container element holding all rendered children", () => {
    const el = render(html`<p>hi</p>`);
    expect(el).toBeTruthy();
    expect(find(el, "p")).toBeTruthy();
    expect(text(el)).toBe("hi");
  });

  it("render with opts.state: inject resolves the registered value", () => {
    const count = signal(42);
    const Component = () => html`<span>${() => inject("count").val}</span>`;
    const el = render(Component(), { state: { count } });
    expect(el).toBeTruthy();
    expect(find(el, "span")).toBeTruthy();
    expect(text(el)).toBe("42");
  });

  it("cleanup disposes scopes so effects no longer fire after cleanup()", () => {
    const counter = signal(0);
    let renderCount = 0;
    render(html`<span>${() => { renderCount++; return counter.val; }}</span>`);
    const countBefore = renderCount;
    cleanup();
    counter.set(99);
    expect(renderCount).toBe(countBefore);
  });

  it("fire dispatches event and handler is called", () => {
    const handler = spy();
    const el = render(html`<button @click=${handler}>click</button>`);
    fire(find(el, "button"), "click");
    expect(handler).toHaveBeenCalledTimes(1);
  });
});

describe("cleanup() disposes unowned effects", () => {
  it("top-level effect does not re-fire after cleanup()", () => {
    const s = signal(0);
    let runs = 0;
    effect(() => { s.val; runs++; });
    expect(runs).toBe(1);
    cleanup();
    s.set(1);
    expect(runs).toBe(1);
  });

  it("render-scope effects are still disposed (regression)", () => {
    const counter = signal(0);
    let renderRuns = 0;
    render(html`<span>${() => { renderRuns++; return counter.val; }}</span>`);
    const baseline = renderRuns;
    cleanup();
    counter.set(99);
    expect(renderRuns).toBe(baseline);
  });

  it("calling cleanup() twice is safe (no double-stop error)", () => {
    let runs = 0;
    effect(() => { runs++; });
    expect(runs).toBe(1);
    cleanup();
    cleanup();
    expect(runs).toBe(1);
  });

  it("unowned effect's cleanup callback runs on cleanup()", () => {
    let cleaned = false;
    effect(() => () => { cleaned = true; });
    cleanup();
    expect(cleaned).toBeTruthy();
  });
});

describe("cleanup() extensions", () => {
  it("clears localStorage and sessionStorage", () => {
    localStorage.setItem("a", "1");
    sessionStorage.setItem("b", "2");
    cleanup();
    expect(localStorage.length).toBe(0);
    expect(sessionStorage.length).toBe(0);
  });

  it("resets document.title and document.activeElement", () => {
    document.title = "hi";
    const el = document.createElement("input");
    el.focus();
    cleanup();
    expect(document.title).toBe("");
    expect(document.activeElement).toBeNull();
  });

  it("empties document.body / head childNodes", () => {
    document.body.appendChild(document.createElement("div"));
    document.head.appendChild(document.createElement("meta"));
    cleanup();
    expect(document.body.childNodes.length).toBe(0);
    expect(document.head.childNodes.length).toBe(0);
  });

  it("cancels pending timers via __clearAllTimers__ when present", () => {
    let called = 0;
    const orig = globalThis.__clearAllTimers__;
    globalThis.__clearAllTimers__ = () => { called++; };
    try {
      cleanup();
      expect(called).toBe(1);
    } finally {
      globalThis.__clearAllTimers__ = orig;
    }
  });
});

describe("selector grammar", () => {
  afterEach(cleanup);

  it("empty selector throws /empty selector/", () => {
    const el = render(html`<div><span></span></div>`);
    expect(() => find(el, "")).toThrow("empty selector");
  });

  it("class selector matches element with that class", () => {
    const el = render(html`<div><span class="foo">a</span></div>`);
    expect(find(el, ".foo")).toBeTruthy();
  });

  it("class selector matches element with multiple classes in class attribute", () => {
    const el = render(html`<div><span class="foo bar">a</span></div>`);
    expect(find(el, ".foo")).toBeTruthy();
  });

  it("class selector returns null when class not present", () => {
    const el = render(html`<div><span class="bar">a</span></div>`);
    expect(find(el, ".foo")).toBeNull();
  });

  it("multi-class: .foo.bar requires both classes", () => {
    const el = render(html`<div><span class="foo bar">a</span><span class="foo">b</span></div>`);
    const result = find(el, ".foo.bar");
    expect(result).toBeTruthy();
    expect(text(result)).toBe("a");
  });

  it("multi-class: .foo.bar and .bar.foo both match class='foo bar'", () => {
    const el = render(html`<div><span class="foo bar">a</span></div>`);
    expect(find(el, ".foo.bar")).toBeTruthy();
    expect(find(el, ".bar.foo")).toBeTruthy();
  });

  it("attribute existence: [data-x] matches when attribute is set", () => {
    const el = render(html`<div><span data-x="anything">a</span></div>`);
    expect(find(el, "[data-x]")).toBeTruthy();
  });

  it("attribute existence: [data-x] matches when attribute value is empty string", () => {
    const container = render(html`<div><span>a</span></div>`);
    find(container, "span").setAttribute("data-x", "");
    expect(find(container, "[data-x]")).toBeTruthy();
  });

  it("attribute equality unquoted: [data-x=y]", () => {
    const el = render(html`<div><span data-x="y">a</span></div>`);
    expect(find(el, "[data-x=y]")).toBeTruthy();
    expect(find(el, "[data-x=z]")).toBeNull();
  });

  it("attribute equality double-quoted: [data-x=\"y z\"]", () => {
    const container = render(html`<div><span>a</span></div>`);
    find(container, "span").setAttribute("data-x", "y z");
    expect(find(container, '[data-x="y z"]')).toBeTruthy();
  });

  it("attribute equality single-quoted: [data-x='y z']", () => {
    const container = render(html`<div><span>a</span></div>`);
    find(container, "span").setAttribute("data-x", "y z");
    expect(find(container, "[data-x='y z']")).toBeTruthy();
  });

  it("attribute name is case-insensitive: [DATA-X=y] matches data-x=\"y\"", () => {
    const el = render(html`<div><span data-x="y">a</span></div>`);
    expect(find(el, "[DATA-X=y]")).toBeTruthy();
  });

  it("compound tag+class+attr: button.btn[type=submit] matches correct element", () => {
    const el = render(html`<div><button class="btn" type="submit">ok</button><button class="btn">no</button></div>`);
    const result = find(el, "button.btn[type=submit]");
    expect(result).toBeTruthy();
    expect(text(result)).toBe("ok");
  });

  it("compound tag+id+class+attr-existence: a#home.nav-link[data-active]", () => {
    const el = render(html`<div><a id="home" class="nav-link" data-active="true">home</a><a id="home" class="nav-link">other</a></div>`);
    const result = find(el, "a#home.nav-link[data-active]");
    expect(result).toBeTruthy();
    expect(text(result)).toBe("home");
  });

  it("findAll returns all class matches in document order", () => {
    const el = render(html`<div><span class="x">1</span><p class="x">2</p><span class="y">3</span></div>`);
    const results = findAll(el, ".x");
    expect(results.length).toBe(2);
    expect(text(results[0])).toBe("1");
    expect(text(results[1])).toBe("2");
  });

  it("closest with compound selector walks up the tree", () => {
    const el = render(html`<div class="box outer"><div class="inner"><span>leaf</span></div></div>`);
    const span = find(el, "span");
    const box = span.closest("div.box");
    expect(box).toBeTruthy();
    expect(box.getAttribute("class")).toContain("outer");
  });

  it("tolerates leading/trailing whitespace around a single compound", () => {
    const el = render(html`<div><span></span></div>`);
    expect(find(el, " span ")).toBe(find(el, "span"));
  });

  it("malformed: unclosed bracket throws", () => {
    const el = render(html`<div><span></span></div>`);
    expect(() => find(el, ".foo[")).toThrow("malformed selector");
  });

  it("malformed: duplicate id throws", () => {
    const el = render(html`<div><span></span></div>`);
    expect(() => find(el, "##id")).toThrow("malformed selector");
  });

  it("regression: tag-only selector still works", () => {
    const el = render(html`<div><a>link</a></div>`);
    expect(find(el, "a")).toBeTruthy();
  });

  it("regression: #id selector still works", () => {
    const el = render(html`<div><span id="nav">nav</span></div>`);
    expect(find(el, "#nav")).toBeTruthy();
  });
});

describe("spy primitive", () => {
  it("spy() returns a function; .calls/.callCount track invocations", () => {
    const s = spy();
    expect(typeof s).toBe("function");
    expect(s()).toBe(undefined);
    expect(s.callCount).toBe(1);
    expect(s.calls[0]).toEqual([]);
  });

  it("spy(fn) calls through to fn, records return value in .results", () => {
    const s = spy((x) => x * 2);
    const result = s(5);
    expect(result).toBe(10);
    expect(s.calls[0]).toEqual([5]);
    expect(s.results[0]).toEqual({ type: "return", value: 10 });
  });

  it("spy(fn) rethrows when fn throws and records a throw result", () => {
    const err = new Error("boom");
    const s = spy(() => { throw err; });
    let caught;
    try { s(); } catch (e) { caught = e; }
    expect(caught).toBe(err);
    expect(s.results[0].type).toBe("throw");
    expect(s.results[0].value).toBe(err);
    expect(s.callCount).toBe(1);
  });

  it(".mockReturnValue(v) overrides impl; subsequent calls return v and .calls grows", () => {
    const s = spy((x) => x);
    s.mockReturnValue(42);
    expect(s("ignored")).toBe(42);
    expect(s.callCount).toBe(1);
    expect(s.calls[0]).toEqual(["ignored"]);
  });

  it(".mockResolvedValue(v) makes subsequent calls return a promise resolving to v", async () => {
    const s = spy();
    s.mockResolvedValue("ok");
    const result = await s();
    expect(result).toBe("ok");
  });

  it(".mockRejectedValue(e) makes subsequent calls return a rejecting promise", async () => {
    const s = spy();
    const err = new Error("nope");
    s.mockRejectedValue(err);
    let caught;
    try { await s(); } catch (e) { caught = e; }
    expect(caught).toBe(err);
  });

  it(".mockImplementation(fn2) replaces impl; .calls accumulates across impl swaps", () => {
    const s = spy(() => "first");
    s("a");
    s.mockImplementation(() => "second");
    s("b");
    expect(s.callCount).toBe(2);
    expect(s.calls[0]).toEqual(["a"]);
    expect(s.calls[1]).toEqual(["b"]);
    expect(s.results[0].value).toBe("first");
    expect(s.results[1].value).toBe("second");
  });

  it(".reset() clears calls/results/instances but keeps implementation", () => {
    const s = spy(() => "impl");
    s("before");
    s.reset();
    expect(s.callCount).toBe(0);
    expect(s.results.length).toBe(0);
    expect(s.instances.length).toBe(0);
    const r = s("after");
    expect(r).toBe("impl");
    expect(s.callCount).toBe(1);
  });

  it("this-binding is recorded in .instances", () => {
    const obj = {};
    obj.method = spy();
    obj.method();
    expect(obj.method.instances[0]).toBe(obj);
  });
});

describe("spy matchers", () => {
  it("toHaveBeenCalled passes after one invocation", () => {
    const s = spy();
    s();
    expect(s).toHaveBeenCalled();
  });

  it("toHaveBeenCalled throws on a fresh spy", () => {
    const s = spy();
    let caught;
    try { expect(s).toHaveBeenCalled(); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("spy was not called");
  });

  it("toHaveBeenCalledTimes(2) passes after exactly two calls", () => {
    const s = spy();
    s();
    s();
    expect(s).toHaveBeenCalledTimes(2);
  });

  it("toHaveBeenCalledTimes fails off-by-one with expected and actual in message", () => {
    const s = spy();
    s();
    let caught;
    try { expect(s).toHaveBeenCalledTimes(2); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("2");
    expect(caught.message).toContain("1");
  });

  it("toHaveBeenCalledWith(a,b) passes when any recorded call matches", () => {
    const s = spy();
    s(1, 2);
    s(3, 4);
    s(5, 6);
    expect(s).toHaveBeenCalledWith(3, 4);
  });

  it("toHaveBeenCalledWith fails with recorded calls in message when no match", () => {
    const s = spy();
    s(1, 2);
    let caught;
    try { expect(s).toHaveBeenCalledWith(9, 9); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message.includes("1") || caught.message.includes("recorded calls")).toBeTruthy();
  });

  it("toHaveBeenCalledWith uses deep equality", () => {
    const s = spy();
    s({ a: 1 });
    expect(s).toHaveBeenCalledWith({ a: 1 });
  });

  it("toHaveBeenLastCalledWith passes for the last call only", () => {
    const s = spy();
    s(1);
    s(2);
    expect(s).toHaveBeenLastCalledWith(2);
  });

  it("toHaveBeenLastCalledWith fails when earlier (not last) call matches", () => {
    const s = spy();
    s(1);
    s(2);
    let caught;
    try { expect(s).toHaveBeenLastCalledWith(1); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain("last call did not match");
  });

  it("all spy matchers throw /value is not a spy/ for non-spy", () => {
    let c1; try { expect(42).toHaveBeenCalled(); } catch (e) { c1 = e; }
    expect(c1.message).toContain("value is not a spy");
    let c2; try { expect(42).toHaveBeenCalledTimes(1); } catch (e) { c2 = e; }
    expect(c2.message).toContain("value is not a spy");
    let c3; try { expect(42).toHaveBeenCalledWith(); } catch (e) { c3 = e; }
    expect(c3.message).toContain("value is not a spy");
    let c4; try { expect(42).toHaveBeenLastCalledWith(); } catch (e) { c4 = e; }
    expect(c4.message).toContain("value is not a spy");
  });
});

describe("matcher .d.ts ↔ runtime parity", () => {
  // The harness project root is `runtime/`, so the .d.ts is read by its
  // basename. The .d.ts is contract-formatted: one method signature per line
  // with no nested braces inside interface bodies. This parser depends on that
  // shape — reformatting to multi-line signatures would break it.
  const dts = globalThis.__readWorkspaceFile__("zero-test.d.ts");

  function interfaceBody(name) {
    const opener = `interface ${name} {`;
    const start = dts.indexOf(opener);
    if (start < 0) throw new Error(`interface ${name} not found in zero-test.d.ts`);
    const bodyStart = start + opener.length;
    const end = dts.indexOf("}", bodyStart);
    if (end < 0) throw new Error(`closing brace for interface ${name} not found`);
    return dts.slice(bodyStart, end);
  }

  function matcherNames(body) {
    const names = new Set();
    const re = /^\s*(\w+)\s*\(/gm;
    let m;
    while ((m = re.exec(body)) !== null) names.add(m[1]);
    return names;
  }

  const declaredPositive = matcherNames(interfaceBody("Matcher"));
  const declaredNegated = matcherNames(interfaceBody("NegatedMatcher"));

  it("every matcher declared on Matcher is implemented on expect()", () => {
    const m = expect(0);
    const missing = [];
    for (const name of declaredPositive) {
      if (typeof m[name] !== "function") missing.push(name);
    }
    if (missing.length > 0) {
      throw new Error(`matcher(s) declared on Matcher but not implemented: ${missing.join(", ")}`);
    }
  });

  it("every matcher declared on NegatedMatcher is implemented on expect().not", () => {
    expect(typeof expect(0).not).toBe("object");
    const n = expect(0).not;
    const missing = [];
    for (const name of declaredNegated) {
      if (typeof n[name] !== "function") missing.push(name);
    }
    if (missing.length > 0) {
      throw new Error(`matcher(s) declared on NegatedMatcher but not implemented: ${missing.join(", ")}`);
    }
  });

  it("every matcher implemented on expect() is declared in Matcher", () => {
    const m = expect(0);
    const undeclared = [];
    for (const key of Object.keys(m)) {
      if (key === "not") continue;
      if (typeof m[key] !== "function") continue;
      if (!declaredPositive.has(key)) undeclared.push(key);
    }
    if (undeclared.length > 0) {
      throw new Error(`matcher(s) implemented on expect() but not declared in Matcher: ${undeclared.join(", ")}`);
    }
  });

  it("every matcher implemented on expect().not is declared in NegatedMatcher", () => {
    const n = expect(0).not;
    const undeclared = [];
    for (const key of Object.keys(n)) {
      if (typeof n[key] !== "function") continue;
      if (!declaredNegated.has(key)) undeclared.push(key);
    }
    if (undeclared.length > 0) {
      throw new Error(`matcher(s) implemented on expect().not but not declared in NegatedMatcher: ${undeclared.join(", ")}`);
    }
  });
});
