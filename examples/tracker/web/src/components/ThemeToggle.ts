import { signal, effect } from "zero";
import type { TemplateResult } from "zero";
import { Toggle } from "zero/components";
import { theme, setTheme } from "../stores/theme.ts";

/**
 * Read the current "prefers dark" media query in a way that survives the
 * test DOM shim (which has no `matchMedia`).
 *
 * @returns Whether the host prefers dark mode.
 */
function prefersDark(): boolean {
  const mm = (
    globalThis as { matchMedia?: (q: string) => { matches: boolean } }
  ).matchMedia;
  if (typeof mm !== "function") return false;
  return mm("(prefers-color-scheme: dark)").matches;
}

/**
 * ThemeToggle — light/dark switch built on the shipped `Toggle` primitive.
 * Wraps a local `Signal<boolean>` (true = dark) and mirrors changes into the
 * app-level theme store. The startup "follow system" state is encoded as
 * `theme.val === null`; the toggle seeds its initial position from
 * `prefers-color-scheme`.
 *
 * @returns Template.
 */
export default function ThemeToggle(): TemplateResult {
  const initial = theme.val ?? (prefersDark() ? "dark" : "light");
  const dark = signal(initial === "dark");
  effect(() => {
    setTheme(dark.val ? "dark" : "light");
  });
  return Toggle({ checked: dark, label: "Dark mode" });
}
