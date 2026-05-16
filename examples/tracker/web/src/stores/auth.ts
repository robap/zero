// stores/auth.ts — status-tagged auth signal. Components branch on
// `auth.val.status`; `user` is only present in the `loggedIn` variant, so
// TypeScript narrows correctly without the per-field optional-check dance.
//
// `login` simulates a network round-trip by transitioning to `loading`
// synchronously and committing `loggedIn` one microtask later. The
// intermediate `loading` state is observable to any code that reads
// `auth.val` between the call and the first `await`.

import { signal } from "zero";
import type { Signal } from "zero";

export interface User {
  id: string;
  name: string;
}

export type AuthState =
  | { status: "loggedOut" }
  | { status: "loading" }
  | { status: "loggedIn"; user: User };

export const auth: Signal<AuthState> = signal<AuthState>({ status: "loggedOut" });

/**
 * Sign in the given name. Transitions auth through `loading` and resolves to
 * `loggedIn`. Rejects (and leaves auth at `loggedOut`) on blank input.
 *
 * @param name Display name.
 * @returns Resolves when the simulated login completes.
 */
export function login(name: string): Promise<void> {
  const trimmed = name.trim();
  if (!trimmed) {
    return Promise.reject(new Error("login: name is required"));
  }
  auth.set({ status: "loading" });
  return Promise.resolve().then(() => {
    auth.set({
      status: "loggedIn",
      user: { id: _userId(trimmed), name: trimmed },
    });
  });
}

/** Drop the session and return to the logged-out state. */
export function logout(): void {
  auth.set({ status: "loggedOut" });
}

function _userId(name: string): string {
  return `u-${name.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`;
}
