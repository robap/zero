// Intentional anti-pattern fixture for the JS/TS lint pass.
// Each construct trips a specific rule; do not "fix" these — the
// integration test asserts that each rule ID fires.
import { html, signal, each } from "zero";

export function Bad() {
  const count = signal(0);
  const items = signal<{ id: number }[]>([]);
  // R02 — assignment to signal.val
  count.val = 1;
  // T01 — addEventListener
  const el = {} as any;
  el.addEventListener("click", () => {});
  // T04 — document query selector
  document.querySelector(".x");
  // T03 — two-arg each
  return html`
    <div>
      <!-- T02 — unknown @event.modifier -->
      <button @click.foo=${() => {}}>Click</button>
      <!-- R01 — \${signal.val} inside html -->
      <span>${count.val}</span>
      <!-- T03 — each() without keyFn -->
      ${each(items, (i) => html`<i>${i}</i>`)}
    </div>
  `;
}

// C01 — class declared in components/
export class Widget {
  m() {}
}

// S01 — oversized function (well over 80 lines).
export function huge() {
  let a = 0;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  a += 1;
  return a;
}
