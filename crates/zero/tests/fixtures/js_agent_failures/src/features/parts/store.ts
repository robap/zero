// Module-level signal()/computed() must NOT fire R03 anywhere — a
// store is wherever the project says it is (feature-first layout).
import { computed, signal } from "zero";

export const parts = signal<number[]>([]);
export const total = computed(() => parts.val.length);
