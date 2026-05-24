import { html, signal } from "zero";
import type { Signal, TemplateResult } from "zero";
import { Combobox } from "zero/components";
import type { ComboboxOption } from "zero/components";

const COUNTRIES: ComboboxOption[] = [
  { value: "ar", label: "Argentina" },
  { value: "au", label: "Australia" },
  { value: "br", label: "Brazil" },
  { value: "ca", label: "Canada" },
  { value: "ch", label: "Switzerland" },
  { value: "cl", label: "Chile" },
  { value: "cn", label: "China" },
  { value: "co", label: "Colombia" },
  { value: "cz", label: "Czechia" },
  { value: "de", label: "Germany" },
  { value: "dk", label: "Denmark" },
  { value: "eg", label: "Egypt" },
  { value: "es", label: "Spain" },
  { value: "fi", label: "Finland" },
  { value: "fr", label: "France" },
  { value: "gb", label: "United Kingdom" },
  { value: "gr", label: "Greece" },
  { value: "ie", label: "Ireland" },
  { value: "in", label: "India" },
  { value: "it", label: "Italy" },
  { value: "jp", label: "Japan" },
  { value: "kr", label: "South Korea" },
  { value: "mx", label: "Mexico" },
  { value: "nl", label: "Netherlands" },
  { value: "no", label: "Norway" },
  { value: "nz", label: "New Zealand" },
  { value: "pe", label: "Peru" },
  { value: "pl", label: "Poland" },
  { value: "pt", label: "Portugal" },
  { value: "se", label: "Sweden" },
  { value: "us", label: "United States" },
];

/**
 * Static loader: pretends to be a backend with ~120ms latency.
 *
 * @param {string} q
 * @returns {Promise<ComboboxOption[]>}
 */
async function filterCountries(q: string): Promise<ComboboxOption[]> {
  await new Promise((r) => setTimeout(r, 120));
  return COUNTRIES.filter((c) =>
    c.label.toLowerCase().startsWith(q.toLowerCase()),
  );
}

/**
 * Mocked-backend loader: ~500ms latency, substring match. A real app
 * would call `fetch` / `createHttp` / GraphQL here — Combobox doesn't
 * care, the contract is `Promise<ComboboxOption[]>`.
 *
 * @param {string} q
 * @returns {Promise<ComboboxOption[]>}
 */
async function slowFetch(q: string): Promise<ComboboxOption[]> {
  await new Promise((r) => setTimeout(r, 500));
  const needle = q.toLowerCase();
  return COUNTRIES.filter((c) => c.label.toLowerCase().includes(needle));
}

/**
 * Default / size-variant section.
 *
 * @param {Signal<string>} v1
 * @param {Signal<string>} v2
 * @param {Signal<string>} v3
 * @returns {TemplateResult}
 */
function sizeSections(
  v1: Signal<string>,
  v2: Signal<string>,
  v3: Signal<string>,
): TemplateResult {
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Default (md)</h2>
      ${Combobox({
        value: v1,
        label: "Country",
        placeholder: "Type a country…",
        loadOptions: filterCountries,
      })}
      <p class="text-body">Picked: ${() => v1.val}</p>
    </section>

    <section class="stack gap-sm">
      <h2 class="text-h2">Small</h2>
      ${Combobox({
        value: v2,
        size: "sm",
        placeholder: "Type a country…",
        loadOptions: filterCountries,
      })}
      <p class="text-body">Picked: ${() => v2.val}</p>
    </section>

    <section class="stack gap-sm">
      <h2 class="text-h2">Large</h2>
      ${Combobox({
        value: v3,
        size: "lg",
        placeholder: "Type a country…",
        loadOptions: filterCountries,
      })}
      <p class="text-body">Picked: ${() => v3.val}</p>
    </section>
  `;
}

/**
 * Initial-label / URL-restore section. Reset increments `resetKey` so
 * the wrapper closure re-creates the Combobox and re-seeds
 * `initialLabel`.
 *
 * @param {Signal<string>} v4
 * @param {Signal<number>} resetKey
 * @returns {TemplateResult}
 */
function initialLabelSection(
  v4: Signal<string>,
  resetKey: Signal<number>,
): TemplateResult {
  const onReset = (): void => {
    v4.set("us");
    resetKey.set(resetKey.val + 1);
  };
  const body = (): TemplateResult => {
    const _ = resetKey.val;
    void _;
    return Combobox({
      value: v4,
      initialLabel: "United States",
      label: "Country",
      loadOptions: filterCountries,
    });
  };
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Initial label (URL-restore)</h2>
      ${body}
      <button class="button button-secondary button-sm" @click=${onReset}>Reset</button>
      <p class="text-body">Picked: ${() => v4.val}</p>
    </section>
  `;
}

/**
 * Async + disabled-signal section.
 *
 * @param {Signal<string>} v5
 * @param {Signal<boolean>} busy
 * @returns {TemplateResult}
 */
function asyncSection(
  v5: Signal<string>,
  busy: Signal<boolean>,
): TemplateResult {
  const simulate = (): void => {
    busy.set(true);
    setTimeout(() => busy.set(false), 1500);
  };
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Async (mocked) + disabled signal</h2>
      ${Combobox({
        value: v5,
        disabled: busy,
        placeholder: "Slow search…",
        debounceMs: 300,
        loadOptions: slowFetch,
      })}
      <div class="cluster gap-sm">
        <button class="button button-secondary button-sm" @click=${simulate}>Simulate busy</button>
        <span class="text-small">busy: ${() => String(busy.val)}</span>
      </div>
      <p class="text-body">Picked: ${() => v5.val}</p>
    </section>
  `;
}

/**
 * Showcase route for Combobox — five instances exercise the size
 * variants, the `initialLabel` URL-restore pattern, and the
 * `disabled: Signal<boolean>` async-disable seam.
 *
 * @returns
 */
export default function ComboboxRoute(): TemplateResult {
  const v1 = signal("");
  const v2 = signal("");
  const v3 = signal("");
  const v4 = signal("us");
  const v5 = signal("");
  const busy = signal(false);
  const resetKey = signal(0);
  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Combobox</h1>
      ${sizeSections(v1, v2, v3)}
      ${initialLabelSection(v4, resetKey)}
      ${asyncSection(v5, busy)}
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
