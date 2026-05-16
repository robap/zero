import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { TextArea } from "zero/components";

/**
 * @returns
 */
export default function TextAreaRoute(): TemplateResult {
  const note = signal("");
  return html`
    <main class="showcase-page stack pad-xl">
      <h1>TextArea</h1>
      <section class="stack gap-sm">
        ${TextArea({ value: note, label: "Note", rows: 6, placeholder: "Write something…" })}
      </section>
      <p>Length: ${() => String(note.val.length)}</p>
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
