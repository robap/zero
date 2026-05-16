import { describe, it, expect, afterEach, beforeEach } from "zero/test";
import { render, find, fire, cleanup } from "zero/test";
import Login from "./login.ts";
import { auth } from "../stores/auth.ts";

describe("Login (route)", () => {
  beforeEach(() => {
    auth.set({ status: "loggedOut" });
  });
  afterEach(() => {
    cleanup();
    auth.set({ status: "loggedOut" });
  });

  it("renders an input and a submit button", () => {
    const el = render(Login());
    expect(find(el, "input")).toBeTruthy();
    expect(find(el, "button")).toBeTruthy();
  });

  it("submitting an empty form shows an error and does not transition auth", () => {
    const el = render(Login());
    fire(find(el, "form")!, "submit");
    expect(auth.val.status).toBe("loggedOut");
    expect(find(el, ".login-error")).toBeTruthy();
  });

  it("submitting a name puts auth into the loading state synchronously", () => {
    const el = render(Login());
    fire(find(el, "input")!, "input", { target: { value: "Robin" } });
    fire(find(el, "form")!, "submit");
    expect(auth.val.status).toBe("loading");
  });

  it("login resolves with a loggedIn state and the entered name", async () => {
    const el = render(Login());
    fire(find(el, "input")!, "input", { target: { value: "Robin" } });
    fire(find(el, "form")!, "submit");
    // Drain microtasks. `login` commits the loggedIn transition in a
    // `.then(...)`; two `await Promise.resolve()` rounds covers the
    // handler's own `await` plus the store's microtask.
    await Promise.resolve();
    await Promise.resolve();
    expect(auth.val.status).toBe("loggedIn");
    if (auth.val.status === "loggedIn") {
      expect(auth.val.user.name).toBe("Robin");
    }
  });
});
