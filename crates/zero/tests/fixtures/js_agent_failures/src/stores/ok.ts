// Module-level signal() must NOT fire R03 — top-of-module state is
// exactly what a store is.
import { signal } from "zero";

export const items = signal<number[]>([]);
