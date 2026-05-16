import { describe, it, expect } from "zero/test";
import { formatDate, statusLabel } from "./format.ts";

describe("formatDate", () => {
  it("renders an ISO timestamp as YYYY-MM-DD HH:MM in UTC", () => {
    expect(formatDate("2026-04-12T11:42:00Z")).toBe("2026-04-12 11:42");
  });

  it("pads single-digit fields", () => {
    expect(formatDate("2026-01-02T03:04:00Z")).toBe("2026-01-02 03:04");
  });

  it("returns the input verbatim when unparseable", () => {
    expect(formatDate("not-a-date")).toBe("not-a-date");
  });
});

describe("statusLabel", () => {
  it("capitalizes open", () => {
    expect(statusLabel("open")).toBe("Open");
  });

  it("capitalizes closed", () => {
    expect(statusLabel("closed")).toBe("Closed");
  });
});
