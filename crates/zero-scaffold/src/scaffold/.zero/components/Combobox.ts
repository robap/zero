import { html, signal, effect, ref } from "zero";
import type { Signal, TemplateResult, Ref } from "zero";

export type ComboboxSize = "sm" | "md" | "lg";

export type ComboboxOption = {
  value: string;
  label: string;
};

export type ComboboxProps = {
  value: Signal<string>;
  loadOptions: (query: string) => Promise<ComboboxOption[]>;
  initialLabel?: string;
  size?: ComboboxSize;
  placeholder?: string;
  label?: string;
  disabled?: Signal<boolean> | boolean;
  debounceMs?: number;
  minQueryLength?: number;
  noResultsLabel?: string;
  loadingLabel?: string;
  onChange?: (value: string, option: ComboboxOption) => void;
};

/**
 * Duck-types a prop value as a `Signal<T>` (has both `.val` and `.set`).
 *
 * @template T
 * @param p Prop value, either signal-wrapped or plain.
 * @returns
 * @internal
 */
function isSignal<T>(p: Signal<T> | T): p is Signal<T> {
  return (
    typeof p === "object" &&
    p !== null &&
    "val" in p &&
    typeof (p as { set?: unknown }).set === "function"
  );
}

/**
 * Read a signal-or-plain prop, returning the underlying value.
 *
 * @template T
 * @param p
 * @returns
 * @internal
 */
function read<T>(p: Signal<T> | T): T {
  return isSignal(p) ? p.val : p;
}

let _comboboxIdCounter = 0;

type ComboboxCtx = {
  props: ComboboxProps;
  debounceMs: number;
  minQueryLength: number;
  noResultsLabel: string;
  loadingLabel: string;
  query: Signal<string>;
  options: Signal<ComboboxOption[]>;
  highlight: Signal<number>;
  open: Signal<boolean>;
  busy: Signal<boolean>;
  lastLabel: Signal<string>;
  resolved: Signal<boolean>;
  inputRef: Ref<HTMLInputElement>;
  state: {
    timer: ReturnType<typeof setTimeout> | null;
    serial: number;
    lastPrefix: string;
    allowGhost: boolean;
  };
};

/**
 * Apply the ghost-completion: set the input's `.value` to the matched
 * option's label and select the tail `[prefix.length, label.length]`.
 *
 * @param ctx
 * @param prefix
 * @param opts
 * @returns
 * @internal
 */
function applyGhost(
  ctx: ComboboxCtx,
  prefix: string,
  opts: ComboboxOption[],
): void {
  const el = ctx.inputRef.el;
  if (el == null) return;
  const needle = prefix.toLowerCase();
  const match = opts.find((o) => o.label.toLowerCase().startsWith(needle));
  if (match && prefix.length > 0) {
    el.value = match.label;
    el.setSelectionRange?.(prefix.length, match.label.length);
  } else {
    el.value = prefix;
  }
}

/**
 * Drive a debounced fetch for `prefix`. Latest-serial-wins race safety
 * keeps stale results from rendering.
 *
 * @param ctx
 * @param prefix
 * @returns
 * @internal
 */
function scheduleFetch(ctx: ComboboxCtx, prefix: string): void {
  if (read(ctx.props.disabled) === true) return;
  if (ctx.state.timer != null) clearTimeout(ctx.state.timer);
  // Bump serial on every keystroke so any in-flight fetch's resolution
  // is dropped if the user kept typing after it started. Without this,
  // a fetch that fires on the leading edge of the debounce window can
  // resolve mid-typing and `applyGhost` clobbers the user's later keys.
  ++ctx.state.serial;
  if (prefix.length < ctx.minQueryLength) {
    ctx.options.set([]);
    ctx.busy.set(false);
    ctx.highlight.set(-1);
    ctx.open.set(false);
    return;
  }
  ctx.state.timer = setTimeout(() => doFetch(ctx, prefix), ctx.debounceMs);
}

/**
 * Run the actual `loadOptions` call for `prefix`. Captures a serial so
 * stale resolutions are dropped.
 *
 * @param ctx
 * @param prefix
 * @returns
 * @internal
 */
function doFetch(ctx: ComboboxCtx, prefix: string): void {
  // Serial is already bumped in scheduleFetch; just capture the current
  // value so the resolution handler can reject stale fetches.
  const mySerial = ctx.state.serial;
  ctx.busy.set(true);
  ctx.open.set(true);
  ctx.props.loadOptions(prefix).then(
    (opts) => onFetchResolved(ctx, prefix, mySerial, opts),
    () => onFetchRejected(ctx, mySerial),
  );
}

/**
 * Apply a successful fetch result if its serial is still current.
 *
 * @param ctx
 * @param prefix
 * @param mySerial
 * @param opts
 * @returns
 * @internal
 */
function onFetchResolved(
  ctx: ComboboxCtx,
  prefix: string,
  mySerial: number,
  opts: ComboboxOption[],
): void {
  if (mySerial !== ctx.state.serial) return;
  ctx.busy.set(false);
  ctx.resolved.set(true);
  ctx.options.set(opts);
  ctx.highlight.set(opts.length > 0 ? 0 : -1);
  if (ctx.state.allowGhost) applyGhost(ctx, prefix, opts);
}

/**
 * Treat a fetch rejection as an empty result if its serial is still
 * current.
 *
 * @param ctx
 * @param mySerial
 * @returns
 * @internal
 */
function onFetchRejected(ctx: ComboboxCtx, mySerial: number): void {
  if (mySerial !== ctx.state.serial) return;
  ctx.busy.set(false);
  ctx.resolved.set(true);
  ctx.options.set([]);
  ctx.highlight.set(-1);
}

/**
 * Pick `opt`: update `value`, internal `lastLabel`, close dropdown,
 * set input visible value, and fire `onChange`.
 *
 * @param ctx
 * @param opt
 * @returns
 * @internal
 */
function pick(ctx: ComboboxCtx, opt: ComboboxOption): void {
  ctx.props.value.set(opt.value);
  ctx.lastLabel.set(opt.label);
  ctx.highlight.set(-1);
  ctx.open.set(false);
  const el = ctx.inputRef.el;
  if (el != null) {
    el.value = opt.label;
    el.setSelectionRange?.(opt.label.length, opt.label.length);
  }
  ctx.props.onChange?.(opt.value, opt);
}

/**
 * Apply the strict-revert rule. If visible text is not a known option
 * label, revert it to `lastLabel.val`. Never writes to `value`.
 *
 * @param ctx
 * @returns
 * @internal
 */
function revertOnBlur(ctx: ComboboxCtx): void {
  const el = ctx.inputRef.el;
  if (el != null) {
    const cur = el.value;
    if (!ctx.options.val.some((o) => o.label === cur)) {
      el.value = ctx.lastLabel.val;
    }
  }
  ctx.open.set(false);
  ctx.highlight.set(-1);
}

/**
 * Move highlight by `delta` (wrapping), and re-apply the ghost to match.
 *
 * @param ctx
 * @param delta
 * @returns
 * @internal
 */
function moveHighlight(ctx: ComboboxCtx, delta: number): void {
  const opts = ctx.options.val;
  if (opts.length === 0) return;
  if (!ctx.open.val && ctx.resolved.val) ctx.open.set(true);
  const cur = ctx.highlight.val;
  const next = (cur + delta + opts.length) % opts.length;
  ctx.highlight.set(next);
  const opt = opts[next];
  if (opt) applyGhost(ctx, ctx.query.val, [opt]);
}

/**
 * ArrowDown handler — move highlight forward, ghost the new label.
 *
 * @param ctx
 * @param e
 * @returns
 * @internal
 */
function onKeyArrowDown(ctx: ComboboxCtx, e: KeyboardEvent): void {
  e.preventDefault();
  moveHighlight(ctx, 1);
}

/**
 * ArrowUp handler — move highlight back, ghost the new label.
 *
 * @param ctx
 * @param e
 * @returns
 * @internal
 */
function onKeyArrowUp(ctx: ComboboxCtx, e: KeyboardEvent): void {
  e.preventDefault();
  moveHighlight(ctx, -1);
}

/**
 * Enter handler — pick the currently highlighted option, if any.
 *
 * @param ctx
 * @param e
 * @returns
 * @internal
 */
function onKeyEnter(ctx: ComboboxCtx, e: KeyboardEvent): void {
  e.preventDefault();
  const opt = ctx.options.val[ctx.highlight.val];
  if (opt) pick(ctx, opt);
}

/**
 * Escape handler — close the dropdown without picking.
 *
 * @param ctx
 * @param e
 * @returns
 * @internal
 */
function onKeyEscape(ctx: ComboboxCtx, e: KeyboardEvent): void {
  e.preventDefault();
  ctx.open.set(false);
  ctx.highlight.set(-1);
}

/**
 * Tab handler — accept the highlight as a pick when a ghost is showing,
 * otherwise just close the dropdown and let native focus move.
 *
 * @param ctx
 * @param e
 * @returns
 * @internal
 */
function onKeyTab(ctx: ComboboxCtx, e: KeyboardEvent): void {
  const opt = ctx.options.val[ctx.highlight.val];
  const el = ctx.inputRef.el;
  if (opt && el && el.value === opt.label) {
    e.preventDefault();
    pick(ctx, opt);
  } else {
    ctx.open.set(false);
  }
}

/**
 * Top-level keydown dispatcher. Each key gets its own branch handler so
 * the parsed-but-unexecuted-branch pattern that triggers Boa's MapLock
 * finalizer panic in `runtime/*.js`-loaded code stays out of reach.
 *
 * @param ctx
 * @param e
 * @returns
 * @internal
 */
function handleKey(ctx: ComboboxCtx, e: KeyboardEvent): void {
  if (read(ctx.props.disabled) === true) return;
  if (e.key === "ArrowDown") {
    onKeyArrowDown(ctx, e);
    return;
  }
  if (e.key === "ArrowUp") {
    onKeyArrowUp(ctx, e);
    return;
  }
  if (e.key === "Enter") {
    onKeyEnter(ctx, e);
    return;
  }
  if (e.key === "Escape") {
    onKeyEscape(ctx, e);
    return;
  }
  if (e.key === "Tab") {
    onKeyTab(ctx, e);
  }
}

/**
 * Process an `input` event: extract the caret prefix and schedule a
 * fetch.
 *
 * @param ctx
 * @param e
 * @returns
 * @internal
 */
function handleInput(ctx: ComboboxCtx, e: Event): void {
  if (read(ctx.props.disabled) === true) return;
  const t = e.target as HTMLInputElement;
  const sel = t.selectionStart;
  const prefix = t.value.slice(0, sel ?? t.value.length);
  // Only allow ghost completion when the user grew the typed prefix.
  // If the prefix is the same length or shorter than the previous one,
  // the user just backspaced (either the ghost suffix or a real char) —
  // re-applying the ghost in that case would make backspace look stuck.
  ctx.state.allowGhost = prefix.length > ctx.state.lastPrefix.length;
  ctx.state.lastPrefix = prefix;
  ctx.query.set(prefix);
  scheduleFetch(ctx, prefix);
}

/**
 * `focus` handler: opens the dropdown if a previous fetch resolved with
 * options and the control is not disabled.
 *
 * @param ctx
 * @returns
 * @internal
 */
function handleFocus(ctx: ComboboxCtx): void {
  if (read(ctx.props.disabled) === true) return;
  if (ctx.resolved.val && ctx.options.val.length > 0) ctx.open.set(true);
}

/**
 * Render the loading-state list row.
 *
 * @param ctx
 * @returns
 * @internal
 */
function dropdownLoading(ctx: ComboboxCtx): TemplateResult {
  return html`<li class="combobox-loading" aria-busy="true">${ctx.loadingLabel}</li>`;
}

/**
 * Render the empty-state list row.
 *
 * @param ctx
 * @returns
 * @internal
 */
function dropdownEmpty(ctx: ComboboxCtx): TemplateResult {
  return html`<li class="combobox-empty" aria-disabled="true">${ctx.noResultsLabel}</li>`;
}

/**
 * Render the populated option list.
 *
 * @param ctx
 * @param optionId
 * @returns
 * @internal
 */
function dropdownList(
  ctx: ComboboxCtx,
  optionId: (i: number) => string,
): TemplateResult {
  return html`${ctx.options.val.map(
    (o, i) => html`
      <li
        class=${() =>
          "combobox-option" +
          (ctx.highlight.val === i ? " combobox-option-active" : "")}
        id=${optionId(i)}
        role="option"
        aria-selected=${() => (ctx.highlight.val === i ? "true" : "false")}
        @mousedown.prevent=${noop}
        @click=${() => pick(ctx, o)}
      >${o.label}</li>
    `,
  )}`;
}

/**
 * No-op handler used to satisfy `@mousedown.prevent` whose only purpose
 * is to keep the click from triggering blur before it lands.
 *
 * @returns
 * @internal
 */
function noop(): void {
  /* no-op */
}

/**
 * Top-level dispatcher for the dropdown body: loading vs. empty vs.
 * populated. Each branch lives in its own function so the
 * parsed-but-unexecuted-branch pattern stays out of reach of Boa's GC
 * finalizer.
 *
 * @param ctx
 * @param optionId
 * @returns
 * @internal
 */
function dropdownBody(
  ctx: ComboboxCtx,
  optionId: (i: number) => string,
): TemplateResult {
  if (ctx.busy.val && ctx.options.val.length === 0) {
    return dropdownLoading(ctx);
  }
  if (ctx.resolved.val && ctx.options.val.length === 0) {
    return dropdownEmpty(ctx);
  }
  return dropdownList(ctx, optionId);
}

/**
 * Compute the outer wrapper class string.
 *
 * @param ctx
 * @param size
 * @returns
 * @internal
 */
function wrapperCls(ctx: ComboboxCtx, size: ComboboxSize): string {
  let cls = `combobox combobox-${size}`;
  if (ctx.open.val) cls += " combobox-open";
  if (read(ctx.props.disabled) === true) cls += " combobox-disabled";
  return cls;
}

/**
 * Register the outside-mousedown effect that runs strict-revert when
 * the dropdown is open and the user clicks outside the root.
 *
 * @param ctx
 * @param rootRef
 * @returns
 * @internal
 */
function registerOutsideClick(
  ctx: ComboboxCtx,
  rootRef: Ref<HTMLElement>,
): void {
  effect(() => {
    if (!ctx.open.val) return;
    const onDown = (e: MouseEvent): void => {
      const root = rootRef.el;
      if (!root) return;
      const target = e.target as Node | null;
      if (target && root.contains?.(target)) return;
      revertOnBlur(ctx);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  });
}

/**
 * Register the effect that closes the dropdown when `props.disabled`
 * flips to true.
 *
 * @param ctx
 * @returns
 * @internal
 */
function registerDisabledWatch(ctx: ComboboxCtx): void {
  effect(() => {
    if (read(ctx.props.disabled) === true) {
      ctx.open.set(false);
      ctx.highlight.set(-1);
    }
  });
}

/**
 * Combobox — controlled, single-select typeahead with inline ghost-text
 * completion, debounced/race-safe `loadOptions(query)` callback, and
 * strict-revert-on-blur semantics. The parent owns `value: Signal<string>`
 * and the I/O via `loadOptions`. Dropdown open/closed is internal state.
 *
 * @param props
 * @returns
 */
export default function Combobox(props: ComboboxProps): TemplateResult {
  const size: ComboboxSize = props.size ?? "md";
  const id = ++_comboboxIdCounter;
  const inputId = `combobox-input-${id}`;
  const listId = `combobox-list-${id}`;
  const optionId = (i: number): string => `combobox-option-${id}-${i}`;

  const ctx: ComboboxCtx = {
    props,
    debounceMs: props.debounceMs ?? 200,
    minQueryLength: props.minQueryLength ?? 1,
    noResultsLabel: props.noResultsLabel ?? "No results",
    loadingLabel: props.loadingLabel ?? "Loading…",
    query: signal(""),
    options: signal<ComboboxOption[]>([]),
    highlight: signal(-1),
    open: signal(false),
    busy: signal(false),
    lastLabel: signal(props.initialLabel ?? ""),
    resolved: signal(false),
    inputRef: ref<HTMLInputElement>(),
    state: { timer: null, serial: 0, lastPrefix: "", allowGhost: false },
  };
  const rootRef: Ref<HTMLElement> = ref();

  registerOutsideClick(ctx, rootRef);
  registerDisabledWatch(ctx);

  const labelNode: TemplateResult | null = props.label
    ? html`<label class="combobox-label" for=${inputId}>${props.label}</label>`
    : null;

  return html`
    <div
      class=${() => wrapperCls(ctx, size)}
      ref=${rootRef}
      role="combobox"
      aria-haspopup="listbox"
      aria-expanded=${() => (ctx.open.val ? "true" : "false")}
      aria-owns=${listId}
    >
      ${labelNode}
      <div class="combobox-field">
        <input
          ref=${ctx.inputRef}
          class=${`input input-${size} combobox-input`}
          id=${inputId}
          type="text"
          role="combobox"
          autocomplete="off"
          aria-autocomplete="both"
          aria-controls=${listId}
          aria-activedescendant=${() =>
            ctx.highlight.val >= 0 ? optionId(ctx.highlight.val) : null}
          placeholder=${props.placeholder ?? ""}
          value=${() => ctx.lastLabel.val}
          disabled=${() => read(props.disabled) === true}
          @input=${(e: Event) => handleInput(ctx, e)}
          @keydown=${(e: KeyboardEvent) => handleKey(ctx, e)}
          @focus=${() => handleFocus(ctx)}
          @blur=${() => revertOnBlur(ctx)}
        >
        <span class="combobox-spinner" hidden=${() => !ctx.busy.val} aria-hidden="true"></span>
      </div>
      <ul
        class="combobox-list border pad-0"
        id=${listId}
        role="listbox"
        hidden=${() => !ctx.open.val}
        aria-busy=${() => (ctx.busy.val ? "true" : "false")}
      >${() => dropdownBody(ctx, optionId)}</ul>
    </div>
  `;
}
