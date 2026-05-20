import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Input, Button, Card } from "zero/components";
import { login } from "../stores/auth.ts";

/**
 * Login — name-only sign-in form. The submit handler invokes the auth
 * store's `login(name)` mutator, which transitions the auth signal through
 * `loading` to `loggedIn`. The actual redirect to `/issues` is driven by a
 * top-level effect in `app.ts` so the route stays a pure presentation
 * component (and unit tests don't need a router).
 *
 * @returns Template.
 */
export default function Login(): TemplateResult {
  const name = signal("");
  const error = signal<string | null>(null);

  const onSubmit = async (e: Event) => {
    e.preventDefault();
    const trimmed = name.val.trim();
    if (!trimmed) {
      error.set("Please enter a name to sign in.");
      return;
    }
    error.set(null);
    try {
      await login(trimmed);
    } catch (err) {
      error.set(err instanceof Error ? err.message : String(err));
    }
  };

  return html`
    <section class="stack pad-xl align-center">
      ${Card({
        children: html`
          <form class="login-form stack gap-md" @submit=${onSubmit}>
            <h1 class="text-h1">Sign in</h1>
            ${Input({
              value: name,
              label: "Name",
              placeholder: "Your display name",
            })}
            ${() =>
              error.val
                ? html`<p class="login-error" role="alert">${error}</p>`
                : html``}
            ${Button({ children: "Sign in" })}
          </form>
        `,
      })}
    </section>
  `;
}
