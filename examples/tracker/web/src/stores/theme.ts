// Theme store. `null` means "follow the system preference" — the canonical
// untouched state. As soon as the user flips the toggle the value becomes
// "light" or "dark" and the app writes `data-theme` onto `<html>`.

import { signal } from "zero";
import type { Signal } from "zero";

export type Theme = "light" | "dark" | null;

export const theme: Signal<Theme> = signal<Theme>(null);

/**
 * Replace the theme. Components should call this rather than `theme.set()`
 * directly — keeps the store the one place mutations originate.
 *
 * @param t New theme value.
 */
export function setTheme(t: Theme): void {
  theme.set(t);
}
