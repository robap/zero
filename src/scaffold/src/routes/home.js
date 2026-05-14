import { html, inject } from "zero";

/**
 * @typedef {import("zero").TemplateResult} TemplateResult
 */

/**
 * @returns {TemplateResult}
 */
function Counter() {
  const count = inject("count");
  return html`<p>Count: ${count}</p>`;
}

/**
 * @returns {TemplateResult}
 */
export default function Home() {
  const count = inject("count");

  return html`
    <h1>Hello from zero</h1>
    <button @click=${() => count.update(n => n + 1)}>Increment</button>
    ${Counter()}
  `;
}
