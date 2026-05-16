import { html } from "zero";
import type { Signal, TemplateResult } from "zero";

export type TabsTab = {
  id: string;
  label: string;
};

export type TabsProps = {
  active: Signal<string>;
  tabs: TabsTab[];
  panels: Record<string, TemplateResult>;
};

/**
 * Tabs — tablist + reactive panel. The currently-rendered panel is
 * looked up from `panels` by the `active` signal's value. Keyboard
 * navigation: Left/Right cycles, Home/End jump.
 *
 * @param props
 * @returns
 */
export default function Tabs(props: TabsProps): TemplateResult {
  const indexOf = (id: string): number => props.tabs.findIndex((t) => t.id === id);
  const setByIndex = (i: number): void => {
    const wrapped = ((i % props.tabs.length) + props.tabs.length) % props.tabs.length;
    props.active.set(props.tabs[wrapped]!.id);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    const current = indexOf(props.active.val);
    switch (e.key) {
      case "ArrowLeft":
        setByIndex(current - 1);
        break;
      case "ArrowRight":
        setByIndex(current + 1);
        break;
      case "Home":
        setByIndex(0);
        break;
      case "End":
        setByIndex(props.tabs.length - 1);
        break;
    }
  };

  const buttons = props.tabs.map(
    (t) =>
      html`<button class="tabs-tab" role="tab" aria-selected=${() => props.active.val === t.id} @click=${() => props.active.set(t.id)}>${t.label}</button>`,
  );

  return html`<div class="tabs"><div class="tabs-list" role="tablist" @keydown=${onKeyDown}>${buttons}</div><div class="tabs-panel" role="tabpanel">${() => props.panels[props.active.val] ?? null}</div></div>`;
}
