import { describe, it, expect, beforeEach } from "zero/test";
import { auth, login, logout } from "./auth.ts";

describe("auth store", () => {
  beforeEach(() => {
    auth.set({ status: "loggedOut" });
  });

  it("starts in loggedOut state", () => {
    expect(auth.val.status).toBe("loggedOut");
  });

  it("login transitions through loading to loggedIn", async () => {
    const p = login("Robin");
    // After the synchronous part of login, the state is `loading`.
    expect(auth.val.status).toBe("loading");
    await p;
    expect(auth.val.status).toBe("loggedIn");
    if (auth.val.status === "loggedIn") {
      expect(auth.val.user.name).toBe("Robin");
    }
  });

  it("login refuses an empty name", async () => {
    let threw = false;
    try {
      await login("   ");
    } catch (_) {
      threw = true;
    }
    expect(threw).toBeTruthy();
    expect(auth.val.status).toBe("loggedOut");
  });

  it("logout returns the state to loggedOut", async () => {
    await login("Robin");
    logout();
    expect(auth.val.status).toBe("loggedOut");
  });
});
