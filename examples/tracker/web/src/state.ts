// state.ts — typed key registry. Component code reads state via
// `inject(Keys.Auth)` and the value's type is inferred from the augmentation
// below — no generic argument required at the call site.

import type { Signal } from "zero";
import type { AuthState } from "./stores/auth.ts";
import type { Theme } from "./stores/theme.ts";
import type { IssuesState } from "./stores/issues.ts";

export const Keys = {
  Auth: "auth" as const,
  Theme: "theme" as const,
  Issues: "issues" as const,
} as const;

declare module "zero" {
  interface StateTypes {
    [Keys.Auth]: Signal<AuthState>;
    [Keys.Theme]: Signal<Theme>;
    [Keys.Issues]: Signal<IssuesState>;
  }
}
