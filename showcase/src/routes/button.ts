import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Button, Input } from "zero/components";

/**
 * @returns
 */
export default function ButtonRoute(): TemplateResult {
  const clicks = signal(0);
  const saved = signal(0);
  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Button</h1>
      <section class="cluster gap-md">
        ${Button({ variant: "primary", children: "Primary" })}
        ${Button({ variant: "secondary", children: "Secondary" })}
        ${Button({ variant: "ghost", children: "Ghost" })}
        ${Button({ variant: "danger", children: "Danger" })}
      </section>
      <section class="cluster gap-md align-center">
        ${Button({ size: "sm", children: "Small" })}
        ${Button({ size: "md", children: "Medium" })}
        ${Button({ size: "lg", children: "Large" })}
      </section>
      <section class="cluster gap-md">
        ${Button({ disabled: true, children: "Disabled" })}
        ${Button({ loading: true, children: "Loading" })}
      </section>
      <section class="stack gap-sm">
        ${Button({ onClick: () => clicks.update((n) => n + 1), children: "Click me" })}
        <p class="text-body">Clicks: ${clicks}</p>
      </section>
      <section class="stack gap-sm">
        <p class="text-body">
          A <code>type="submit"</code> button can live outside its
          <code>&lt;form&gt;</code> via <code>form="&lt;id&gt;"</code>. The
          Save button below sits outside the form yet submits it; the
          loading button stays non-interactive while busy.
        </p>
        <form
          id="showcase-edit-form"
          @submit=${(e: Event) => {
            e.preventDefault();
            saved.update((n) => n + 1);
          }}
        >
          ${Input({ value: signal(""), label: "Name", placeholder: "Ada" })}
        </form>
        <div class="cluster gap-md">
          ${Button({ type: "submit", form: "showcase-edit-form", children: "Save" })}
          ${Button({ type: "submit", loading: true, children: "Saving…" })}
        </div>
        <p class="text-body">Saved: ${saved}</p>
      </section>
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
