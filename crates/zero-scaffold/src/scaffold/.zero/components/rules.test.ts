import { describe, it, expect } from "zero/test";
import {
  email,
  intRange,
  maxLength,
  minLength,
  pattern,
  required,
} from "./rules.ts";
import { createForm } from "./form.ts";

describe("validation rules", () => {
  describe("required", () => {
    it("passes a non-empty value and fails empty with the default message", () => {
      const rule = required();
      expect(rule("a")).toBe(null);
      expect(rule("")).toBe("This field is required.");
    });

    it("fails a whitespace-only value", () => {
      expect(required()("   ")).toBe("This field is required.");
    });

    it("uses a custom message when given", () => {
      expect(required("Code is required.")("")).toBe("Code is required.");
    });
  });

  describe("minLength", () => {
    it("passes at the boundary and fails below it with the default message", () => {
      const rule = minLength(2);
      expect(rule("ab")).toBe(null);
      expect(rule("a")).toBe("Must be at least 2 characters.");
    });

    it("uses the singular default message for n = 1", () => {
      expect(minLength(1)("")).toBe(null);
      expect(minLength(1, { allowEmpty: false })("")).toBe(
        "Must be at least 1 character.",
      );
    });

    it("passes empty and whitespace-only values by default", () => {
      const rule = minLength(3);
      expect(rule("")).toBe(null);
      expect(rule("   ")).toBe(null);
    });

    it("fails empty values with { allowEmpty: false }", () => {
      const rule = minLength(2, { allowEmpty: false });
      expect(rule("")).toBe("Must be at least 2 characters.");
    });

    it("uses a custom message via string shorthand and { message }", () => {
      expect(minLength(2, "Too short.")("a")).toBe("Too short.");
      expect(minLength(2, { message: "Too short." })("a")).toBe("Too short.");
    });
  });

  describe("maxLength", () => {
    it("passes at the boundary and fails above it with the default message", () => {
      const rule = maxLength(2);
      expect(rule("ab")).toBe(null);
      expect(rule("abc")).toBe("Must be 2 characters or fewer.");
    });

    it("uses the singular default message for n = 1", () => {
      expect(maxLength(1)("ab")).toBe("Must be 1 character or fewer.");
    });

    it("measures the trimmed length", () => {
      expect(maxLength(2)(" ab ")).toBe(null);
    });

    it("passes empty and whitespace-only values by default", () => {
      const rule = maxLength(2);
      expect(rule("")).toBe(null);
      expect(rule("    ")).toBe(null);
    });

    it("uses a custom message via string shorthand and { message }", () => {
      expect(maxLength(2, "Too long.")("abc")).toBe("Too long.");
      expect(maxLength(2, { message: "Too long." })("abc")).toBe("Too long.");
    });
  });

  describe("intRange", () => {
    it("passes integers inside the bounds and fails outside with the default message", () => {
      const rule = intRange(1, 999);
      expect(rule("1")).toBe(null);
      expect(rule("999")).toBe(null);
      expect(rule("0")).toBe("Must be a whole number between 1 and 999.");
      expect(rule("1000")).toBe("Must be a whole number between 1 and 999.");
    });

    it("accepts leading zeros, a leading +, and surrounding whitespace", () => {
      const rule = intRange(1, 999);
      expect(rule("010")).toBe(null);
      expect(rule("+5")).toBe(null);
      expect(rule(" 42 ")).toBe(null);
    });

    it("rejects exponents, decimals, and non-numeric input", () => {
      const rule = intRange(1, 999);
      const msg = "Must be a whole number between 1 and 999.";
      expect(rule("1e3")).toBe(msg);
      expect(rule("3.5")).toBe(msg);
      expect(rule("abc")).toBe(msg);
      expect(rule("-1")).toBe(msg);
    });

    it("accepts negative integers when the bounds allow them", () => {
      expect(intRange(-5, 5)("-3")).toBe(null);
    });

    it("passes empty values by default and fails them with { allowEmpty: false }", () => {
      expect(intRange(1, 999)("")).toBe(null);
      expect(intRange(1, 999, { allowEmpty: false })("")).toBe(
        "Must be a whole number between 1 and 999.",
      );
    });

    it("uses a custom message via string shorthand and { message }", () => {
      expect(intRange(1, 9, "Out of range.")("10")).toBe("Out of range.");
      expect(intRange(1, 9, { message: "Out of range." })("10")).toBe(
        "Out of range.",
      );
    });
  });

  describe("pattern", () => {
    it("passes a matching value and fails a non-match with the default message", () => {
      const rule = pattern(/^[A-Z]+$/);
      expect(rule("ABC")).toBe(null);
      expect(rule("abc")).toBe("Invalid format.");
    });

    it("returns stable results for a /g regex across consecutive calls", () => {
      const rule = pattern(/[A-Z]+/g);
      expect(rule("ABC")).toBe(null);
      expect(rule("ABC")).toBe(null);
      expect(rule("abc")).toBe("Invalid format.");
      expect(rule("abc")).toBe("Invalid format.");
    });

    it("tests the raw value without trimming", () => {
      expect(pattern(/^[A-Z]+$/)(" ABC ")).toBe("Invalid format.");
    });

    it("passes empty values by default and fails them with { allowEmpty: false }", () => {
      expect(pattern(/^[A-Z]+$/)("")).toBe(null);
      expect(pattern(/^[A-Z]+$/, { allowEmpty: false })("")).toBe(
        "Invalid format.",
      );
    });

    it("uses a custom message via string shorthand and { message }", () => {
      expect(pattern(/^[A-Z]+$/, "Uppercase only.")("abc")).toBe(
        "Uppercase only.",
      );
      expect(pattern(/^[A-Z]+$/, { message: "Uppercase only." })("abc")).toBe(
        "Uppercase only.",
      );
    });
  });

  describe("email", () => {
    it("passes an address and fails a non-address with the default message", () => {
      const rule = email();
      expect(rule("a@b.co")).toBe(null);
      expect(rule("a@b")).toBe("Enter a valid email address.");
    });

    it("trims the value before testing", () => {
      expect(email()(" a@b.co ")).toBe(null);
    });

    it("rejects missing parts and embedded whitespace", () => {
      const rule = email();
      const msg = "Enter a valid email address.";
      expect(rule("a b@c.d")).toBe(msg);
      expect(rule("@b.co")).toBe(msg);
      expect(rule("a@")).toBe(msg);
    });

    it("passes empty values by default and fails them with { allowEmpty: false }", () => {
      expect(email()("")).toBe(null);
      expect(email({ allowEmpty: false })("")).toBe(
        "Enter a valid email address.",
      );
    });

    it("uses a custom message via string shorthand and { message }", () => {
      expect(email("Bad address.")("a@b")).toBe("Bad address.");
      expect(email({ message: "Bad address." })("a@b")).toBe("Bad address.");
    });
  });

  describe("createForm integration", () => {
    it("composes rules in a field's validate array", async () => {
      const form = createForm({
        fields: {
          code: { initial: "", validate: [required(), maxLength(10)] },
        },
      });
      const handler = form.submit(() => {});
      const event = {
        preventDefault: () => {},
      } as unknown as Event;
      await handler(event);
      expect(form.fields.code.error.val).toBe("This field is required.");
      form.fields.code.value.set("ok");
      expect(form.fields.code.error.val).toBe(null);
      form.fields.code.value.set("longer-than-ten");
      await handler(event);
      expect(form.fields.code.error.val).toBe(
        "Must be 10 characters or fewer.",
      );
    });
  });
});
