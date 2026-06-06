// Module-level effect() MUST fire R03 — it starts at import time and
// nothing ever disposes it.
import { effect, signal } from "zero";

/** @internal backing state read by the leaky effect. */
const count = signal(0);

effect(() => {
  console.log(count.val);
});

export { count };
