import { describe, it, expect, afterEach } from "zero/test";
import { render, findAll, fire, cleanup, text } from "zero/test";
import { signal, html } from "zero";
import Tabs from "./Tabs.ts";

describe("Tabs", () => {
  afterEach(cleanup);

  it("renders the active panel and switches when a tab is clicked", () => {
    const active = signal("a");
    const el = render(
      Tabs({
        active,
        tabs: [
          { id: "a", label: "A" },
          { id: "b", label: "B" },
        ],
        panels: {
          a: html`<p>A panel</p>`,
          b: html`<p>B panel</p>`,
        },
      }),
    );
    expect(text(el, ".tabs-panel")).toBe("A panel");
    const buttons = findAll(el, '[role="tab"]');
    fire(buttons[1]!, "click");
    expect(active.val).toBe("b");
    expect(text(el, ".tabs-panel")).toBe("B panel");
  });
});
