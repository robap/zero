import { html, inject } from "zero";
import type { Signal, TemplateResult } from "zero";

type ThemeMode = "auto" | "light" | "dark";

const components: Array<{ name: string; href: string }> = [
  { name: "Avatar", href: "/avatar" },
  { name: "Badge", href: "/badge" },
  { name: "Button", href: "/button" },
  { name: "Card", href: "/card" },
  { name: "Checkbox", href: "/checkbox" },
  { name: "Combobox", href: "/combobox" },
  { name: "Dialog", href: "/dialog" },
  { name: "Input", href: "/input" },
  { name: "Pagination", href: "/pagination" },
  { name: "Radio", href: "/radio" },
  { name: "Select", href: "/select" },
  { name: "Spinner", href: "/spinner" },
  { name: "Table", href: "/table" },
  { name: "Tabs", href: "/tabs" },
  { name: "TextArea", href: "/textarea" },
  { name: "Toast", href: "/toast" },
  { name: "Toggle", href: "/toggle" },
];

/**
 * Showcase home — theme switcher + navigation to every component route.
 *
 * @returns
 */
export default function Home(): TemplateResult {
  const theme = inject<Signal<ThemeMode>>("theme");
  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-display">zero showcase</h1>
      <p class="text-body">
        Every component shipped by zero, rendered in its variants and
        sizes. Use the theme switcher to flip between auto, light, and
        dark; the choice rides the route stack.
      </p>
      <section class="stack gap-sm">
        <h2 class="text-h2">Theme</h2>
        <div class="cluster gap-md">
          <button class="button button-secondary button-sm" @click=${() => theme.set("auto")}>Auto</button>
          <button class="button button-secondary button-sm" @click=${() => theme.set("light")}>Light</button>
          <button class="button button-secondary button-sm" @click=${() => theme.set("dark")}>Dark</button>
          <span>Active: ${() => theme.val}</span>
        </div>
      </section>
      <section class="stack gap-sm">
        <h2 class="text-h2">Components</h2>
        <nav class="cluster gap-md">
          ${components.map((c) => html`<a class="showcase-nav-link" href=${c.href}>${c.name}</a>`)}
        </nav>
      </section>
    </main>
  `;
}
