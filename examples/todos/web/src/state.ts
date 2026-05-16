// state.ts — typed key registry. Component code reads state via
// `inject(Keys.Todos)` and the value's type is inferred from the augmentation
// below — no generic argument required at the call site.

import type { Signal } from "zero";
import type { TodosState } from "./stores/todos.ts";
import type { Theme } from "./stores/theme.ts";

export const Keys = {
  Todos: "todos" as const,
  Theme: "theme" as const,
} as const;

declare module "zero" {
  interface StateTypes {
    [Keys.Todos]: Signal<TodosState>;
    [Keys.Theme]: Signal<Theme>;
  }
}
