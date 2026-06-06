import { describe, it, expect, afterEach } from "zero/test";
import { render, find, fire, cleanup } from "zero/test";
import { HttpError } from "zero/http";
import { createForm } from "./form.ts";
import Input from "./Input.ts";

describe("createForm", () => {
  it("exposes each field's initial value", () => {
    const form = createForm({
      fields: {
        code: { initial: "" },
        slots: { initial: "10" },
      },
    });
    expect(form.fields.code.value.val).toBe("");
    expect(form.fields.slots.value.val).toBe("10");
  });

  it("marks a field touched on first set and keeps it across edits", () => {
    const form = createForm({ fields: { code: { initial: "" } } });
    expect(form.fields.code.touched.val).toBe(false);
    form.fields.code.value.set("a");
    expect(form.fields.code.touched.val).toBe(true);
    form.fields.code.value.set("ab");
    expect(form.fields.code.touched.val).toBe(true);
    form.reset();
    expect(form.fields.code.touched.val).toBe(false);
  });

  it("marks a field touched on update()", () => {
    const form = createForm({ fields: { code: { initial: "x" } } });
    form.fields.code.value.update((v) => v + "y");
    expect(form.fields.code.value.val).toBe("xy");
    expect(form.fields.code.touched.val).toBe(true);
  });

  it("keeps isValid live without populating field errors", () => {
    const form = createForm({
      fields: {
        name: {
          initial: "",
          validate: (v) => (v.trim() === "" ? "Name is required." : null),
        },
      },
    });
    expect(form.isValid.val).toBe(false);
    expect(form.fields.name.error.val).toBe(null);
    form.fields.name.value.set("Ada");
    expect(form.isValid.val).toBe(true);
    expect(form.fields.name.error.val).toBe(null);
  });

  it("passes (value, values) to per-field validators", () => {
    const seen: Array<[string, Record<string, string>]> = [];
    const form = createForm({
      fields: {
        a: { initial: "1" },
        b: {
          initial: "2",
          validate: (v, vals) => {
            seen.push([v, vals]);
            return null;
          },
        },
      },
    });
    void form.isValid.val;
    expect(seen.length).toBe(1);
    expect(seen[0]![0]).toBe("2");
    expect(seen[0]![1]).toEqual({ a: "1", b: "2" });
  });

  it("lets the cross-field validator fill only un-errored keys", () => {
    const form = createForm({
      fields: {
        critical: {
          initial: "",
          validate: (v) => (v === "" ? "Critical is required." : null),
        },
        reorder: { initial: "5" },
      },
      validate: () => ({
        critical: "cross message must lose",
        reorder: "Critical must be ≤ reorder point.",
      }),
    });
    // Surface validator output through the errored-edit re-validate path.
    form.setErrors({ critical: "stale", reorder: "stale" });
    form.fields.critical.value.set("");
    expect(form.fields.critical.error.val).toBe("Critical is required.");
    form.fields.reorder.value.set("5");
    expect(form.fields.reorder.error.val).toBe(
      "Critical must be ≤ reorder point.",
    );
  });

  it("setErrors sets named fields and clears unnamed ones", () => {
    const form = createForm({
      fields: { a: { initial: "" }, b: { initial: "" } },
    });
    form.setErrors({ a: "first", b: "second" });
    expect(form.fields.a.error.val).toBe("first");
    expect(form.fields.b.error.val).toBe("second");
    form.setErrors({ b: "only b" });
    expect(form.fields.a.error.val).toBe(null);
    expect(form.fields.b.error.val).toBe("only b");
  });

  it("reset restores initials and clears errors, touched, and form error", () => {
    const form = createForm({
      fields: { code: { initial: "init" } },
    });
    form.fields.code.value.set("edited");
    form.setErrors({ code: "bad" });
    form.error.set("form-level");
    form.reset();
    expect(form.fields.code.value.val).toBe("init");
    expect(form.fields.code.error.val).toBe(null);
    expect(form.fields.code.touched.val).toBe(false);
    expect(form.error.val).toBe(null);
  });

  it("clears an errored field's message live once the value is fixed", () => {
    const form = createForm({
      fields: {
        name: {
          initial: "",
          validate: (v) => (v.trim() === "" ? "Name is required." : null),
        },
        other: { initial: "" },
      },
    });
    form.setErrors({ name: "Name is required.", other: "untouched" });
    form.fields.name.value.set("Ada");
    expect(form.fields.name.error.val).toBe(null);
    // Only the edited field re-validates.
    expect(form.fields.other.error.val).toBe("untouched");
  });

  it("switches an errored field's message when a different rule now fails", () => {
    const form = createForm({
      fields: {
        code: {
          initial: "",
          validate: (v) =>
            v.trim() === ""
              ? "Code is required."
              : v.trim().length > 3
                ? "Code must be 3 characters or fewer."
                : null,
        },
      },
    });
    form.setErrors({ code: "Code is required." });
    form.fields.code.value.set("toolong");
    expect(form.fields.code.error.val).toBe(
      "Code must be 3 characters or fewer.",
    );
  });

  it("runs an array of validators in order; first non-null wins and stops", () => {
    const ran: string[] = [];
    const form = createForm({
      fields: {
        code: {
          initial: "",
          validate: [
            (v) => {
              ran.push("first");
              return v === "" ? "First message." : null;
            },
            () => {
              ran.push("second");
              return "Second message.";
            },
          ],
        },
      },
    });
    form.setErrors({ code: "stale" });
    form.fields.code.value.set("");
    expect(form.fields.code.error.val).toBe("First message.");
    expect(ran).toEqual(["first"]);
  });

  it("passes an array of validators when every validator returns null", () => {
    const form = createForm({
      fields: {
        code: {
          initial: "ok",
          validate: [
            (v) => (v.trim() === "" ? "Code is required." : null),
            (v) => (v.trim().length > 3 ? "Too long." : null),
          ],
        },
      },
    });
    expect(form.isValid.val).toBe(true);
    form.fields.code.value.set("toolong");
    expect(form.isValid.val).toBe(false);
  });

  it("treats validate: [fn] exactly like validate: fn", () => {
    const rule = (v: string): string | null =>
      v.trim() === "" ? "Name is required." : null;
    const single = createForm({
      fields: { name: { initial: "", validate: rule } },
    });
    const listed = createForm({
      fields: { name: { initial: "", validate: [rule] } },
    });
    expect(single.isValid.val).toBe(false);
    expect(listed.isValid.val).toBe(false);
    single.fields.name.value.set("Ada");
    listed.fields.name.value.set("Ada");
    expect(single.isValid.val).toBe(true);
    expect(listed.isValid.val).toBe(true);
  });

  it("treats an empty validate array like no validator", () => {
    const form = createForm({
      fields: { name: { initial: "", validate: [] } },
    });
    expect(form.isValid.val).toBe(true);
    form.setErrors({ name: "stale" });
    form.fields.name.value.set("anything");
    expect(form.fields.name.error.val).toBe(null);
  });

  it("surfaces nothing when editing an un-errored field", () => {
    const form = createForm({
      fields: {
        name: {
          initial: "x",
          validate: (v) => (v.trim() === "" ? "Name is required." : null),
        },
      },
    });
    form.fields.name.value.set("");
    expect(form.fields.name.error.val).toBe(null);
  });

  describe("submit", () => {
    /**
     * Build a minimal submit-shaped event recording `preventDefault`.
     *
     * @returns The fake event plus a flag getter.
     */
    function fakeEvent(): { event: Event; prevented: () => boolean } {
      let prevented = false;
      const event = {
        preventDefault: () => {
          prevented = true;
        },
      } as unknown as Event;
      return { event, prevented: () => prevented };
    }

    it("prevents default and gates the action on client-side errors", async () => {
      const calls: unknown[] = [];
      const form = createForm({
        fields: {
          name: {
            initial: "",
            validate: (v) => (v === "" ? "Name is required." : null),
          },
          ok: { initial: "x" },
        },
      });
      const handler = form.submit((vals) => {
        calls.push(vals);
      });
      const { event, prevented } = fakeEvent();
      await handler(event);
      expect(prevented()).toBe(true);
      expect(calls.length).toBe(0);
      expect(form.fields.name.error.val).toBe("Name is required.");
      expect(form.fields.name.touched.val).toBe(true);
      expect(form.fields.ok.touched.val).toBe(true);
    });

    it("calls the action with a values snapshot when valid", async () => {
      const calls: Array<Record<string, string>> = [];
      const form = createForm({
        fields: { name: { initial: "Ada" }, slots: { initial: "10" } },
      });
      const handler = form.submit((vals) => {
        calls.push(vals);
      });
      await handler(fakeEvent().event);
      expect(calls.length).toBe(1);
      expect(calls[0]).toEqual({ name: "Ada", slots: "10" });
      expect(form.fields.name.error.val).toBe(null);
      expect(form.error.val).toBe(null);
    });

    it("maps a 400 {errors} body onto matching fields", async () => {
      const form = createForm({ fields: { name: { initial: "Ada" } } });
      const handler = form.submit(() => {
        throw new HttpError(400, "Bad Request", {
          errors: { name: "Name is taken." },
        });
      });
      await handler(fakeEvent().event);
      expect(form.fields.name.error.val).toBe("Name is taken.");
      expect(form.error.val).toBe(null);
    });

    it("surfaces unmatched 409 error keys in the form-level error", async () => {
      const form = createForm({ fields: { name: { initial: "Ada" } } });
      const handler = form.submit(() => {
        throw new HttpError(409, "Conflict", {
          errors: { name: "taken", partId: "gone", other: "x" },
        });
      });
      await handler(fakeEvent().event);
      expect(form.fields.name.error.val).toBe("taken");
      expect(form.error.val).toBe("gone x");
    });

    it("sets the generic message for a 400 with empty or missing errors", async () => {
      const form = createForm({ fields: { name: { initial: "Ada" } } });
      const empty = form.submit(() => {
        throw new HttpError(400, "Bad Request", { errors: {} });
      });
      await empty(fakeEvent().event);
      expect(form.error.val).toBe("Could not save. Try again.");
      form.error.set(null);
      const missing = form.submit(() => {
        throw new HttpError(400, "Bad Request", {});
      });
      await missing(fakeEvent().event);
      expect(form.error.val).toBe("Could not save. Try again.");
    });

    it("sets the generic message for other statuses and plain errors", async () => {
      const form = createForm({ fields: { name: { initial: "Ada" } } });
      const server = form.submit(() => {
        throw new HttpError(500, "Internal Server Error", {
          errors: { name: "ignored" },
        });
      });
      await server(fakeEvent().event);
      expect(form.error.val).toBe("Could not save. Try again.");
      expect(form.fields.name.error.val).toBe(null);
      const plain = form.submit(() => {
        throw new Error("network down");
      });
      await plain(fakeEvent().event);
      expect(form.error.val).toBe("Could not save. Try again.");
    });

    it("clears the previous form-level error on a new submit", async () => {
      const form = createForm({ fields: { name: { initial: "Ada" } } });
      const failing = form.submit(() => {
        throw new Error("boom");
      });
      await failing(fakeEvent().event);
      expect(form.error.val).toBe("Could not save. Try again.");
      const ok = form.submit(() => {});
      await ok(fakeEvent().event);
      expect(form.error.val).toBe(null);
    });
  });

  describe("binding through components", () => {
    afterEach(cleanup);

    it("binds the façade value signal through Input", () => {
      const form = createForm({ fields: { code: { initial: "abc" } } });
      const el = render(
        Input({ value: form.fields.code.value, error: form.fields.code.error }),
      );
      const input = find(el, "input")!;
      expect(input.getAttribute("value")).toBe("abc");
      fire(input, "input", { target: { value: "abcd" } });
      expect(form.fields.code.value.val).toBe("abcd");
      expect(form.fields.code.touched.val).toBe(true);
    });
  });
});
