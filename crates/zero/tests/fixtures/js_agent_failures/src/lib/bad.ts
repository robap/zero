// I01 — bare npm specifier (node: protocol)
import "node:fs";
// I02 — relative import into .zero/
import "../../.zero/components/Button.ts";

// C02 — customElements.define
customElements.define("my-el", class {} as any);
