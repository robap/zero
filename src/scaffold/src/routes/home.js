import { html, inject } from "zero";

/**
 * @typedef {import("zero").TemplateResult} TemplateResult
 */

/**
 * @returns {TemplateResult}
 */
function Counter() {
  return html`<p>Count: ${() => inject("count").val}</p>`;
}

/**
 * @returns {TemplateResult}
 */
export default function Home() {
  return html`
    <h1>Hello from zero</h1>
    <button @click=${() => inject("count").update(n => n + 1)}>Increment</button>
    ${Counter()}
  `;
}
