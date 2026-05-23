import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Pagination } from "zero/components";

/**
 * Showcase route for Pagination — four instances exercise the size
 * variants, the summary slot, and the single-page auto-disabled state.
 * Async-driven Pagination (signal-typed `totalPages` / `disabled`) is
 * demonstrated on the Table route, paired with a real Table.
 *
 * @returns
 */
export default function PaginationRoute(): TemplateResult {
  const a = signal(1);
  const b = signal(1);
  const c = signal(1);
  const d = signal(1);

  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Pagination</h1>

      <section class="stack gap-sm">
        <h2 class="text-h2">Default (md)</h2>
        ${Pagination({ page: a, totalPages: 12 })}
        <p class="text-body">Current page: ${() => a.val}</p>
      </section>

      <section class="stack gap-sm">
        <h2 class="text-h2">Small</h2>
        ${Pagination({ page: b, totalPages: 20, size: "sm" })}
        <p class="text-body">Current page: ${() => b.val}</p>
      </section>

      <section class="stack gap-sm">
        <h2 class="text-h2">Large with summary</h2>
        ${Pagination({
          page: c,
          totalPages: 5,
          size: "lg",
          summary: (p, t) => `Page ${p} of ${t}`,
        })}
      </section>

      <section class="stack gap-sm">
        <h2 class="text-h2">Single page (auto-disabled)</h2>
        ${Pagination({ page: d, totalPages: 1 })}
      </section>

      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
