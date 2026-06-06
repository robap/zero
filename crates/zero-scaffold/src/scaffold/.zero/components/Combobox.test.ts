import { describe, it, expect, afterEach } from "zero/test";
import { render, find, findAll, fire, cleanup, spy } from "zero/test";
import { signal, computed } from "zero";
import Combobox from "./Combobox.ts";
import type { ComboboxOption } from "./Combobox.ts";

/**
 * Real-setTimeout wait helper. Tests use small `debounceMs` values
 * (default 5) and ~20ms waits to keep the suite honest about real
 * async behaviour without paying significant wall-clock cost.
 *
 * @param {number} ms
 * @returns {Promise<void>}
 */
function wait(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

/**
 * Build a `loadOptions` that filters an in-memory list by case-
 * insensitive prefix and resolves immediately.
 *
 * @param {ComboboxOption[]} opts
 * @returns {(q: string) => Promise<ComboboxOption[]>}
 */
function staticLoader(
  opts: ComboboxOption[],
): (q: string) => Promise<ComboboxOption[]> {
  return async (q: string): Promise<ComboboxOption[]> =>
    opts.filter((o) => o.label.toLowerCase().startsWith(q.toLowerCase()));
}

/**
 * Mutate the input's `.value` and `selectionStart` directly, then
 * dispatch an `input` event. Mirrors the real-browser sequence where
 * the keystroke has already been applied to the input before the
 * event fires.
 *
 * @param {Element} input
 * @param {string} value
 * @param {number} [selectionStart]
 * @returns {void}
 */
function fireInput(
  input: Element,
  value: string,
  selectionStart?: number,
): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (input as any).value = value;
  if (selectionStart != null) {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (input as any).selectionStart = selectionStart;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (input as any).selectionEnd = selectionStart;
  }
  fire(input, "input", { target: input });
}

const ABC: ComboboxOption[] = [
  { value: "a1", label: "alpha" },
  { value: "a2", label: "alphabet" },
  { value: "a3", label: "alps" },
];

describe("Combobox", () => {
  afterEach(cleanup);

  it("renders the base markup", () => {
    const value = signal("");
    const el = render(
      Combobox({ value, loadOptions: staticLoader([]) }),
    );
    expect(find(el, ".combobox")).toBeTruthy();
    expect(find(el, ".combobox-input")).toBeTruthy();
    const list = find(el, ".combobox-list")!;
    expect(list).toBeTruthy();
    expect(list.hasAttribute("hidden")).toBe(true);
  });

  it("renders no error node and aria-invalid 'false' without an error prop", () => {
    const value = signal("");
    const el = render(Combobox({ value, loadOptions: staticLoader([]) }));
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "input")!.getAttribute("aria-invalid")).toBe("false");
  });

  it("renders the error message with aria wiring when errored", () => {
    const value = signal("");
    const error = signal<string | null>("Unknown part.");
    const el = render(
      Combobox({ value, error, loadOptions: staticLoader([]) }),
    );
    const node = find(el, "[data-field-error]")!;
    expect(node).toBeTruthy();
    expect((node.textContent ?? "").trim()).toBe("Unknown part.");
    const input = find(el, "input")!;
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(input.getAttribute("aria-describedby")).toBe(
      node.getAttribute("id"),
    );
  });

  it("clears the error node and aria-invalid when the signal goes null", () => {
    const value = signal("");
    const error = signal<string | null>("Unknown part.");
    const el = render(
      Combobox({ value, error, loadOptions: staticLoader([]) }),
    );
    expect(find(el, "[data-field-error]")).toBeTruthy();
    error.set(null);
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "input")!.getAttribute("aria-invalid")).toBe("false");
  });

  it("typing triggers a debounced fetch", async () => {
    const value = signal("");
    const loader = spy(staticLoader(ABC));
    const el = render(
      Combobox({ value, loadOptions: loader, debounceMs: 5 }),
    );
    fireInput(find(el, "input")!, "a", 1);
    await wait(25);
    expect(loader.callCount).toBe(1);
    expect(loader.calls[0]![0]).toBe("a");
  });

  it("minQueryLength gates the fetch", async () => {
    const value = signal("");
    const loader = spy(staticLoader(ABC));
    const el = render(
      Combobox({
        value,
        loadOptions: loader,
        debounceMs: 5,
        minQueryLength: 2,
      }),
    );
    fireInput(find(el, "input")!, "a", 1);
    await wait(25);
    expect(loader.callCount).toBe(0);
    fireInput(find(el, "input")!, "al", 2);
    await wait(25);
    expect(loader.callCount).toBe(1);
  });

  it("race safety: only the latest fetch's result renders", async () => {
    const value = signal("");
    type R = (opts: ComboboxOption[]) => void;
    const resolvers: R[] = [];
    const loader = (_q: string): Promise<ComboboxOption[]> =>
      new Promise<ComboboxOption[]>((r) => resolvers.push(r));
    const el = render(
      Combobox({ value, loadOptions: loader, debounceMs: 5 }),
    );
    fireInput(find(el, "input")!, "a", 1);
    await wait(25);
    fireInput(find(el, "input")!, "ab", 2);
    await wait(25);
    expect(resolvers.length).toBe(2);
    const A: ComboboxOption[] = [{ value: "stale", label: "stale" }];
    const B: ComboboxOption[] = [{ value: "fresh", label: "fresh" }];
    resolvers[1]!(B);
    await wait(5);
    resolvers[0]!(A);
    await wait(5);
    const opts = findAll(el, ".combobox-option").map(
      (o) => (o.textContent ?? "").trim(),
    );
    expect(opts).toEqual(["fresh"]);
  });

  it("ghost completion fills the matched tail and selects it", async () => {
    const value = signal("");
    const el = render(
      Combobox({
        value,
        loadOptions: staticLoader([{ value: "foobar", label: "foobar" }]),
        debounceMs: 5,
      }),
    );
    const input = find(el, "input")!;
    fireInput(input, "foo", 3);
    await wait(25);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).value).toBe("foobar");
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).selectionStart).toBe(3);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).selectionEnd).toBe(6);
  });

  it("ArrowDown / ArrowUp move highlight and update the ghost", async () => {
    const value = signal("");
    const el = render(
      Combobox({ value, loadOptions: staticLoader(ABC), debounceMs: 5 }),
    );
    const input = find(el, "input")!;
    fireInput(input, "a", 1);
    await wait(25);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).value).toBe("alpha");
    fire(input, "keydown", { key: "ArrowDown" });
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).value).toBe("alphabet");
    fire(input, "keydown", { key: "ArrowDown" });
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).value).toBe("alps");
    fire(input, "keydown", { key: "ArrowUp" });
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).value).toBe("alphabet");
    fire(input, "keydown", { key: "ArrowUp" });
    fire(input, "keydown", { key: "ArrowUp" });
    // Wrap from top → last
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).value).toBe("alps");
  });

  it("Enter accepts the highlight", async () => {
    const value = signal("");
    const onChange = spy<(v: string, o: ComboboxOption) => void>();
    const el = render(
      Combobox({
        value,
        loadOptions: staticLoader(ABC),
        debounceMs: 5,
        onChange,
      }),
    );
    const input = find(el, "input")!;
    fireInput(input, "a", 1);
    await wait(25);
    fire(input, "keydown", { key: "ArrowDown" });
    fire(input, "keydown", { key: "Enter" });
    expect(value.val).toBe("a2");
    expect(onChange.callCount).toBe(1);
    expect(onChange.calls[0]![0]).toBe("a2");
    expect((onChange.calls[0]![1] as ComboboxOption).label).toBe("alphabet");
    expect(find(el, ".combobox-list")!.hasAttribute("hidden")).toBe(true);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).value).toBe("alphabet");
  });

  it("Tab-to-complete accepts when a ghost is showing", async () => {
    const value = signal("");
    const onChange = spy<(v: string, o: ComboboxOption) => void>();
    const el = render(
      Combobox({
        value,
        loadOptions: staticLoader([{ value: "foobar", label: "foobar" }]),
        debounceMs: 5,
        onChange,
      }),
    );
    const input = find(el, "input")!;
    fireInput(input, "foo", 3);
    await wait(25);
    fire(input, "keydown", { key: "Tab" });
    expect(value.val).toBe("foobar");
    expect(onChange.callCount).toBe(1);
  });

  it("Tab without a matching ghost does not pick", async () => {
    const value = signal("");
    const onChange = spy<(v: string, o: ComboboxOption) => void>();
    const el = render(
      Combobox({
        value,
        loadOptions: staticLoader([]),
        debounceMs: 5,
        onChange,
      }),
    );
    const input = find(el, "input")!;
    fireInput(input, "zz", 2);
    await wait(25);
    fire(input, "keydown", { key: "Tab" });
    expect(value.val).toBe("");
    expect(onChange.callCount).toBe(0);
  });

  it("Escape closes the dropdown without picking", async () => {
    const value = signal("");
    const onChange = spy<(v: string, o: ComboboxOption) => void>();
    const el = render(
      Combobox({
        value,
        loadOptions: staticLoader(ABC),
        debounceMs: 5,
        onChange,
      }),
    );
    const input = find(el, "input")!;
    fireInput(input, "a", 1);
    await wait(25);
    expect(find(el, ".combobox-list")!.hasAttribute("hidden")).toBe(false);
    fire(input, "keydown", { key: "Escape" });
    expect(find(el, ".combobox-list")!.hasAttribute("hidden")).toBe(true);
    expect(value.val).toBe("");
    expect(onChange.callCount).toBe(0);
  });

  it("blur strict-revert restores lastLabel / initialLabel / empty", async () => {
    const value = signal("");
    const el = render(
      Combobox({ value, loadOptions: staticLoader([]), debounceMs: 5 }),
    );
    const input = find(el, "input")!;
    fireInput(input, "xyz", 3);
    await wait(25);
    fire(input, "blur");
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).value).toBe("");
    expect(value.val).toBe("");

    const value2 = signal("default-id");
    const el2 = render(
      Combobox({
        value: value2,
        loadOptions: staticLoader([]),
        initialLabel: "Default",
        debounceMs: 5,
      }),
    );
    const input2 = find(el2, "input")!;
    fireInput(input2, "xyz", 3);
    await wait(25);
    fire(input2, "blur");
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input2 as any).value).toBe("Default");
  });

  it("clicking a dropdown option picks it", async () => {
    const value = signal("");
    const onChange = spy<(v: string, o: ComboboxOption) => void>();
    const el = render(
      Combobox({
        value,
        loadOptions: staticLoader(ABC),
        debounceMs: 5,
        onChange,
      }),
    );
    const input = find(el, "input")!;
    fireInput(input, "a", 1);
    await wait(25);
    const items = findAll(el, ".combobox-option");
    expect(items.length).toBe(3);
    fire(items[1]!, "click");
    expect(value.val).toBe("a2");
    expect(onChange.callCount).toBe(1);
    expect(find(el, ".combobox-list")!.hasAttribute("hidden")).toBe(true);
  });

  it("initialLabel displays until first pick", async () => {
    const value = signal("u-42");
    const el = render(
      Combobox({
        value,
        loadOptions: staticLoader(ABC),
        initialLabel: "Alice",
        debounceMs: 5,
      }),
    );
    const input = find(el, "input")!;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).value).toBe("Alice");
    fireInput(input, "a", 1);
    await wait(25);
    fire(input, "keydown", { key: "Enter" });
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).value).toBe("alpha");
  });

  it("disabled (plain boolean) blocks fetches and gates the input attr", async () => {
    const value = signal("");
    const loader = spy(staticLoader(ABC));
    const el = render(
      Combobox({
        value,
        loadOptions: loader,
        debounceMs: 5,
        disabled: true,
      }),
    );
    const input = find(el, "input")!;
    expect(input.hasAttribute("disabled")).toBe(true);
    fireInput(input, "a", 1);
    await wait(25);
    expect(loader.callCount).toBe(0);
  });

  it("accepts a computed disabled and toggles when it flips", () => {
    const value = signal("");
    const guard = signal(false);
    const disabled = computed(() => guard.val);
    const el = render(
      Combobox({ value, loadOptions: staticLoader(ABC), disabled }),
    );
    const input = find(el, "input.combobox-input")!;
    expect(input.hasAttribute("disabled")).toBe(false);
    guard.set(true);
    expect(input.hasAttribute("disabled")).toBe(true);
  });

  it("disabled (signal) toggles state without remount", async () => {
    const value = signal("");
    const disabled = signal(false);
    const loader = spy(staticLoader(ABC));
    const el = render(
      Combobox({ value, loadOptions: loader, debounceMs: 5, disabled }),
    );
    const input = find(el, "input")!;
    expect(input.hasAttribute("disabled")).toBe(false);
    fireInput(input, "a", 1);
    await wait(25);
    expect(loader.callCount).toBe(1);
    expect(find(el, ".combobox-list")!.hasAttribute("hidden")).toBe(false);
    disabled.set(true);
    expect(input.hasAttribute("disabled")).toBe(true);
    expect(find(el, ".combobox-list")!.hasAttribute("hidden")).toBe(true);
    fireInput(input, "ab", 2);
    await wait(25);
    expect(loader.callCount).toBe(1);
  });

  it("no-results state renders .combobox-empty with the configured label", async () => {
    const value = signal("");
    const el = render(
      Combobox({
        value,
        loadOptions: staticLoader([]),
        debounceMs: 5,
        noResultsLabel: "Nothing here",
      }),
    );
    fireInput(find(el, "input")!, "zz", 2);
    await wait(25);
    const empty = find(el, ".combobox-empty")!;
    expect(empty).toBeTruthy();
    expect((empty.textContent ?? "").trim()).toBe("Nothing here");
  });

  it("loading state renders spinner + .combobox-loading until resolution", async () => {
    const value = signal("");
    type R = (opts: ComboboxOption[]) => void;
    let resolveIt: R | null = null;
    const loader = (_q: string): Promise<ComboboxOption[]> =>
      new Promise<ComboboxOption[]>((r) => {
        resolveIt = r;
      });
    const el = render(
      Combobox({ value, loadOptions: loader, debounceMs: 5 }),
    );
    fireInput(find(el, "input")!, "a", 1);
    await wait(25);
    expect(find(el, ".combobox-list")!.hasAttribute("hidden")).toBe(false);
    expect(find(el, ".combobox-spinner")!.hasAttribute("hidden")).toBe(false);
    expect(find(el, ".combobox-loading")).toBeTruthy();
    resolveIt!([]);
    await wait(5);
    expect(find(el, ".combobox-spinner")!.hasAttribute("hidden")).toBe(true);
    expect(find(el, ".combobox-empty")).toBeTruthy();
  });

  it("size variant class applies to the wrapper", () => {
    const sm = render(
      Combobox({
        value: signal(""),
        loadOptions: staticLoader([]),
        size: "sm",
      }),
    );
    expect(
      (find(sm, ".combobox") as HTMLElement).classList.contains("combobox-sm"),
    ).toBe(true);
    const md = render(
      Combobox({ value: signal(""), loadOptions: staticLoader([]) }),
    );
    expect(
      (find(md, ".combobox") as HTMLElement).classList.contains("combobox-md"),
    ).toBe(true);
  });

  it("a stale fetch resolving mid-typing does not clobber later keystrokes", async () => {
    const value = signal("");
    type R = (opts: ComboboxOption[]) => void;
    const resolvers: R[] = [];
    const loader = (_q: string): Promise<ComboboxOption[]> =>
      new Promise<ComboboxOption[]>((r) => resolvers.push(r));
    const el = render(
      Combobox({ value, loadOptions: loader, debounceMs: 5 }),
    );
    const input = find(el, "input")!;
    fireInput(input, "U", 1);
    await wait(25);
    // Fetch for "U" is now in flight. User keeps typing.
    fireInput(input, "Un", 2);
    await wait(25);
    expect(resolvers.length).toBe(2);
    // The original "U" fetch resolves with results that DO start with
    // the new "Un" prefix. Without the serial-bump fix, this would
    // call applyGhost("U", …) and overwrite the visible "Un" with
    // "United Kingdom" — eating the user's later keystroke.
    resolvers[0]!([{ value: "uk", label: "United Kingdom" }]);
    await wait(5);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const visible = (input as any).value as string;
    expect(visible.startsWith("Un")).toBe(true);
  });

  it("backspace clears the ghost without re-applying it", async () => {
    const value = signal("");
    const el = render(
      Combobox({
        value,
        loadOptions: staticLoader([{ value: "foobar", label: "foobar" }]),
        debounceMs: 5,
      }),
    );
    const input = find(el, "input")!;
    fireInput(input, "foo", 3);
    await wait(25);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).value).toBe("foobar");
    // Browser semantics: Backspace with a non-empty selection deletes
    // the selection. The component must NOT re-ghost the visible text
    // back to "foobar" — otherwise backspace appears stuck.
    fireInput(input, "foo", 3);
    await wait(25);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((input as any).value).toBe("foo");
  });

  it("onChange fires exactly once per pick", async () => {
    const value = signal("");
    const onChange = spy<(v: string, o: ComboboxOption) => void>();
    const el = render(
      Combobox({
        value,
        loadOptions: staticLoader(ABC),
        debounceMs: 5,
        onChange,
      }),
    );
    const input = find(el, "input")!;
    fireInput(input, "a", 1);
    await wait(25);
    fire(input, "keydown", { key: "Escape" });
    fire(input, "blur");
    expect(onChange.callCount).toBe(0);
    fireInput(input, "a", 1);
    await wait(25);
    fire(input, "keydown", { key: "Enter" });
    expect(onChange.callCount).toBe(1);
    expect(onChange.calls[0]![0]).toBe("a1");
    expect((onChange.calls[0]![1] as ComboboxOption).value).toBe("a1");
  });
});
