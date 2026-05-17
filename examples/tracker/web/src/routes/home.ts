import { html, inject } from "zero";
import type { TemplateResult } from "zero";
import { Card } from "zero/components";
import { Keys } from "../state.ts";

/**
 * Home — static landing page. Branches the call-to-action by auth status:
 * a logged-out visitor sees "Sign in"; a logged-in one sees "Go to issues".
 * The branching is reactive: a successful login on `/login` flipping the
 * auth signal updates this view on the next navigation back.
 *
 * @returns Template.
 */
export default function Home(): TemplateResult {
  const status = () => inject(Keys.Auth).val.status;
  return html`
    <section class="stack pad-xl gap-md align-center">
      ${Card({
        children: html`
          <div class="stack gap-md">
            <h1 class="text-display">tracker</h1>
            <p class="text-body">A small issue tracker that exercises the canonical zero patterns.</p>
            ${() =>
              status() === "loggedIn"
                ? html`<a class="button button-primary" href="/issues">Go to issues</a>`
                : html`<a class="button button-primary" href="/login">Sign in</a>`}
          </div>
        `,
      })}
    </section>
  `;
}
