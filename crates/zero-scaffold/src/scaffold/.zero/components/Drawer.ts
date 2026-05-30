import { html } from "zero";
import type { Signal, TemplateResult } from "zero";

export type DrawerSide = "left" | "right" | "top" | "bottom";
export type DrawerMode = "overlay" | "push";
export type DrawerSize = "sm" | "md" | "lg";

export type DrawerSlot =
  | TemplateResult
  | string
  | null
  | undefined
  | (() => TemplateResult | string | null);

export type DrawerProps = {
  open: Signal<boolean>;
  side: DrawerSide;
  mode?: DrawerMode;
  size?: DrawerSize;
  title?: DrawerSlot;
  body?: DrawerSlot;
  controls?: DrawerSlot;
};

/**
 * Drawer — controlled, edge-anchored side panel that slides in from one of
 * the four sides in either `overlay` (fixed over content, with a
 * non-interactive backdrop) or `push` (in-flow flex sibling whose size
 * animates) mode. Pure visual container: three caller-owned slots
 * (`title`, `body`, `controls`), no built-in close affordances, no focus
 * trap, no scroll lock. The only stateful prop is `open`; the parent owns
 * close. The DOM is always mounted so CSS animates both open and close;
 * the only reactive surface is the `class` / `hidden` bindings driven by
 * `open.val` and the slot values. No `effect`, listeners, timers, or refs.
 *
 * @param props
 * @returns
 */
export default function Drawer(props: DrawerProps): TemplateResult {
  const mode: DrawerMode = props.mode ?? "overlay";
  const size: DrawerSize = props.size ?? "md";
  const side: DrawerSide = props.side;
  const { open } = props;

  const modeCls = mode === "push" ? "drawer-push" : "drawer-overlay";
  const panelCls = (): string =>
    `drawer ${modeCls} drawer-${side} drawer-${size}` +
    (open.val ? " drawer-open" : "");

  /**
   * Resolve a slot (calling it if it is a function) and report whether the
   * resulting value is empty (`null`, `undefined`, or an empty string).
   *
   * @param {DrawerSlot} slot
   * @returns {boolean}
   */
  const slotEmpty = (slot: DrawerSlot): boolean => {
    const v = typeof slot === "function" ? slot() : slot;
    return v == null || v === "";
  };

  const sections = html`
    <header class="drawer-title" hidden=${() => slotEmpty(props.title)}>${props.title}</header>
    <div class="drawer-body" hidden=${() => slotEmpty(props.body)}>${props.body}</div>
    <footer class="drawer-controls" hidden=${() => slotEmpty(props.controls)}>${props.controls}</footer>`;

  const backdrop =
    mode === "overlay"
      ? html`<div class=${() => "drawer-backdrop" + (open.val ? " drawer-backdrop-open" : "")}></div>`
      : null;

  const panel =
    mode === "overlay"
      ? html`<aside class=${panelCls} role="dialog" aria-modal="true">${sections}</aside>`
      : html`<aside class=${panelCls} role="complementary">${sections}</aside>`;

  return html`${backdrop}${panel}`;
}
