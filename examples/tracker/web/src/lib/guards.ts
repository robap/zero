// lib/guards.ts — reusable route guards.
//
// `requireAuth` reads the auth state from the typed-key registry and
// redirects unauthenticated visitors to `/login`. Apply it via
// `app.route(..., { guard: requireAuth })`.

import { Keys } from "../state.ts";
import type { AuthState } from "../stores/auth.ts";
import type { Signal } from "zero";

export interface GuardContext {
  state: Record<string, unknown>;
  redirect: (path: string) => void;
}

/**
 * Allow the navigation only when the auth store is in `loggedIn`. Otherwise
 * redirect to `/login`. Returns `false` on redirect so the route pipeline
 * short-circuits cleanly.
 *
 * @param ctx Guard context provided by the router.
 * @returns `true` to allow, `false` to block.
 */
export function requireAuth(ctx: GuardContext): boolean {
  const auth = ctx.state[Keys.Auth] as Signal<AuthState> | undefined;
  if (!auth || auth.val.status !== "loggedIn") {
    ctx.redirect("/login");
    return false;
  }
  return true;
}
