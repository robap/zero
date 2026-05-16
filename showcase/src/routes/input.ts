import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Input } from "zero/components";

/**
 * @returns
 */
export default function InputRoute(): TemplateResult {
  const name = signal("");
  const email = signal("");
  return html`
    <main class="showcase-page stack pad-xl">
      <h1>Input</h1>
      <section class="stack gap-sm">
        ${Input({ value: name, label: "Name", placeholder: "Ada" })}
        ${Input({ value: email, type: "email", label: "Email", placeholder: "ada@example.com" })}
      </section>
      <section class="cluster gap-md">
        ${Input({ value: signal(""), size: "sm", placeholder: "Small" })}
        ${Input({ value: signal(""), size: "md", placeholder: "Medium" })}
        ${Input({ value: signal(""), size: "lg", placeholder: "Large" })}
      </section>
      <p>Name: ${() => name.val}, Email: ${() => email.val}</p>
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
