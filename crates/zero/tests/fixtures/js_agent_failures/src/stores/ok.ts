// Module-level reactive primitives in stores/ must NOT fire R03.
import { signal } from "zero";

export const items = signal<number[]>([]);
